use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::wire::{Instruction, LlmRequest, LlmResponse};
use super::Provider;

pub struct Ollama {
    pub model: String,
    pub url: String,
    pub timeout: Duration,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Msg>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
}

#[derive(Serialize)]
struct Msg {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: MsgOut,
}

#[derive(Deserialize)]
struct MsgOut {
    role: String,
    content: String,
}

fn to_messages(ins: &Instruction) -> Vec<Msg> {
    let mut sys = ins.system.clone();
    if let Some(dev) = &ins.developer {
        sys.push_str("\n\nDeveloper notes:\n");
        sys.push_str(dev);
    }
    vec![
        Msg { role: "system".into(), content: sys },
        Msg { role: "user".into(), content: ins.user.clone() },
    ]
}

#[async_trait]
impl Provider for Ollama {
    async fn send(&self, req: &LlmRequest, debug: bool) -> Result<LlmResponse> {
        let url = format!("{}/api/chat", self.url.trim_end_matches('/'));
        let client = Client::builder().timeout(self.timeout).build()?;
        let body = ChatRequest {
            model: &self.model,
            messages: to_messages(&req.instruction),
            stream: false,
            options: OllamaOptions { temperature: 0.1 },
        };

        if debug {
            eprintln!("debug/ollama: POST {}", url);
        }

        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("ollama request failed")?;

        let text = resp.text().await.context("ollama read body failed")?;

        if debug {
            eprintln!("debug/ollama: raw body:\n{}\n", text);
        }

        // Try to parse to standard ollama response first
        let parsed: Result<ChatResponse, _> = serde_json::from_str(&text);
        let content = match parsed {
            Ok(c) => c.message.content,
            Err(_) => text,
        };

        let llm_resp: LlmResponse = serde_json::from_str(&content)
            .map_err(|e| anyhow!("failed to parse LLM JSON: {}.\nContent was:\n{}", e, content))?;

        Ok(llm_resp)
    }
}
