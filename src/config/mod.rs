use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub root: String,
    pub max_actions: usize,
    pub max_patch_bytes: usize,
    pub path_allowlist: Vec<String>,
    pub command_allowlist: Vec<String>,
    pub ollama_url: Option<String>,
    pub timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: ".".into(),
            max_actions: 50,
            max_patch_bytes: 2_000_000,
            path_allowlist: vec![
                "src".into(),
                "app".into(),
                "pages".into(),
                "components".into(),
                "package.json".into(),
            ],
            command_allowlist: vec![
                "npm ci".into(),
                "npm run build".into(),
                "npm run dev".into(),
                "npm install".into(),
                "pnpm i".into(),
                "pnpm build".into(),
                "pnpm dev".into(),
                "pnpm install".into(),
                "yarn".into(),
                "yarn build".into(),
                "yarn dev".into(),
                "yarn install".into(),
            ],
            ollama_url: None,
            timeout_secs: 240, // matches long LLM ops
        }
    }
}
