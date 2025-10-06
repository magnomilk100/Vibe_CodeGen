use async_trait::async_trait;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use crate::wire::{LlmRequest, LlmResponse};

/// OpenAI provider that sends the ENTIRE LlmRequest as a single user message,
/// with no extra system/developer messages.
pub struct OpenAIProvider {
    model: String,
    client: Client,
    timeout_secs: u64,
}

impl OpenAIProvider {
    pub fn new(model: String, timeout_secs: u64) -> Self {
        Self {
            model,
            client: Client::new(),
            timeout_secs,
        }
    }
}

#[async_trait]
impl super::Provider for OpenAIProvider {
    async fn send(&self, req: &LlmRequest, debug: bool) -> Result<LlmResponse> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow!("OPENAI_API_KEY env var is not set"))?;

        // Serialize the WHOLE request exactly as we want the model to see it.
        let request_json_str = serde_json::to_string(req)?;

        // Single user message, no system messages or added scaffolding.
        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": request_json_str
                }
            ],
            "temperature": 0.0,
            "top_p": 1.0,
            // Force a valid JSON object in the response.
            "response_format": { "type": "json_object" }
        });

        if debug {
            eprintln!(
                "debug[openai]: HTTP POST /v1/chat/completions body:\n{}",
                serde_json::to_string_pretty(&body)?
            );
        }

        let resp = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(api_key)
            .timeout(Duration::from_secs(self.timeout_secs))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if debug {
            eprintln!("debug[openai]: raw status: {}", status);
            eprintln!("debug[openai]: raw response:\n{}", &text);
        }

        if !status.is_success() {
            return Err(anyhow!("OpenAI API error ({}): {}", status, text));
        }

        // Minimal structs to parse the chat response
        #[derive(Deserialize)]
        struct ChatMessage {
            content: String,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: ChatMessage,
        }
        #[derive(Deserialize)]
        struct ChatResponse {
            choices: Vec<Choice>,
        }

        // Parse full HTTP JSON
        let parsed: ChatResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow!("Failed to parse OpenAI response: {e}\nRaw: {text}"))?;

        let content = parsed
            .choices
            .get(0)
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        // Try strict parse first
        match serde_json::from_str::<LlmResponse>(&content) {
            Ok(ok) => return Ok(ok),
            Err(_e) => {
                // Fallback: extract first {...} JSON object from the text, then parse it.
                if let Some(obj) = extract_first_json_object(&content) {
                    if let Ok(resp) = serde_json::from_str::<LlmResponse>(&obj) {
                        return Ok(resp);
                    }
                }
            }
        }

        Err(anyhow!(
            "Model did not return a valid JSON response body.\n--- content start ---\n{}\n--- content end ---",
            content
        ))
    }
}

/// Extracts the first top-level JSON object substring from a string.
/// Handles nested braces; returns None if not found.
fn extract_first_json_object(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut start = None;
    let mut depth = 0usize;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' {
            if start.is_none() {
                start = Some(i);
            }
            depth += 1;
        } else if b == b'}' {
            if depth > 0 {
                depth -= 1;
                if depth == 0 {
                    if let Some(st) = start {
                        let slice = &s[st..=i];
                        return Some(slice.to_string());
                    }
                }
            }
        }
    }
    None
}
