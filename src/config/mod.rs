use serde::{Serialize, Deserialize};
use crate::cli::ProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: String,
    pub root: String,
    pub vibe_out: String,
    pub provider: ProviderKind,
    pub model: String,
    pub command_allowlist: Vec<String>,
    pub path_allowlist: Vec<String>,
    pub max_actions: usize,
    pub max_patch_bytes: usize,
    pub auto_approve: bool,
    pub timeout_secs: u64,
    pub ollama_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: "2025-10-01".into(),
            root: ".".into(),
            vibe_out: "vibe-index/.vibe/out".into(),
            provider: ProviderKind::OpenAI,
            model: "gpt-4.1-mini".into(),
            command_allowlist: vec![
                "npm ci".into(), "npm run build".into(), "npm run dev".into(),
                "pnpm i".into(), "pnpm build".into(), "pnpm dev".into(),
                "yarn".into(), "yarn build".into(), "yarn dev".into()
            ],
            path_allowlist: vec!["src".into(), "app".into(), "pages".into(), "components".into(), "package.json".into()],
            max_actions: 50,
            max_patch_bytes: 2_000_000,
            auto_approve: false,
            timeout_secs: 2400,
            ollama_url: Some("http://localhost:11434".into()),
        }
    }
}
