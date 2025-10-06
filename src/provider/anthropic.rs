use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::wire::{Instruction, LlmRequest, LlmResponse};
use super::Provider;

pub struct Anthropic {
    pub model: String,
    pub api_key: String,
    pub timeout: Duration,
    pub api_base: String,
    pub api_version: String,
}

#[derive(Serialize)]
struct MsgRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<Msg<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
}

#[derive(Serialize)]
struct Msg<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MsgResponse {
    content: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(default)]
    text: String,
    #[serde(default)]
    r#type: String,
}

fn split_instruction<'a>(ins: &'a Instruction) -> (String, String) {
    let mut system = ins.system.clone();
    if let Some(dev) = &ins.developer {
        system.push_str("\n\nDeveloper notes:\n");
        system.push_str(dev);
    }
    (system, ins.user.clone())
}

#[async_trait]
impl Provider for Anthropic {
    async fn send(&self, req: &LlmRequest, debug: bool) -> Result<LlmResponse> {
        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));
        let client = Client::builder().timeout(self.timeout).build()?;
        let (system, user) = split_instruction(&req.instruction);
        let body = MsgRequest {
            model: &self.model,
            max_tokens: 4096,
            messages: vec![Msg { role: "user", content: &user }],
            system: Some(Box::leak(system.into_boxed_str())), // quick stable ref
        };

        if debug {
            eprintln!("debug/anthropic: POST {}", url);
        }

        let resp = client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.api_version)
            .json(&body)
            .send()
            .await
            .context("anthropic request failed")?;

        let text = resp.text().await.context("anthropic read body failed")?;
        if debug {
            eprintln!("debug/anthropic: raw body:\n{}\n", text);
        }

        // Try to parse standard response
        let parsed: MsgResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow!("anthropic response parse error: {}", e))?;

        let content = parsed
            .content
            .into_iter()
            .find(|b| b.r#type == "text" || !b.text.is_empty())
            .map(|b| b.text)
            .ok_or_else(|| anyhow!("anthropic: empty content"))?;

        let llm_resp: LlmResponse = serde_json::from_str(&content)
            .map_err(|e| anyhow!("failed to parse LLM JSON: {}.\nContent was:\n{}", e, content))?;

        Ok(llm_resp)
    }
}
