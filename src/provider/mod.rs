use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::cli::ProviderKind;
use crate::wire::{LlmRequest, LlmResponse};

pub mod openai;
pub mod anthropic;
pub mod ollama;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn send(&self, req: &LlmRequest, debug: bool) -> Result<LlmResponse>;
}

pub type DynProvider = Box<dyn Provider + Send + Sync>;

pub fn make_provider(
    kind: ProviderKind,
    model: String,
    timeout_secs: u64,
    _ollama_url: Option<String>,
) -> Result<DynProvider> {
    match kind {
        ProviderKind::OpenAI => Ok(Box::new(openai::OpenAIProvider::new(
            model,
            timeout_secs,
        ))),

        // Keep these as explicit errors for now so the binary compiles even if
        // Anthropic/Ollama adapters are not implemented in your workspace.
        ProviderKind::Anthropic => Err(anyhow!("Anthropic provider not implemented in this build")),
        ProviderKind::Ollama => Err(anyhow!("Ollama provider not implemented in this build")),
    }
}
