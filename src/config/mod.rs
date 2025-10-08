use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub root: String,
    pub vibe_out: String,
    pub provider: crate::cli::ProviderKind,
    pub model: String,
    pub task: String,
    pub dry_run: bool,
    pub auto_approve: bool,
    pub timeout_secs: u64,
    pub save_request: bool,
    pub save_response: bool,
    pub debug: bool,

    // Safety allowlists used by exec and request-building
    pub path_allowlist: Vec<String>,
    pub command_allowlist: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            vibe_out: ".vibe/out".to_string(),
            provider: crate::cli::ProviderKind::OpenAI,
            model: "gpt-4o-mini".to_string(),
            task: String::new(),
            dry_run: false,
            auto_approve: false,
            timeout_secs: 2400,
            save_request: true,
            save_response: true,
            debug: false,
            path_allowlist: default_path_allowlist(),
            command_allowlist: default_command_allowlist(),
        }
    }
}

pub fn default_path_allowlist() -> Vec<String> {
    vec![
        "src".to_string(),
        "app".to_string(),
        "pages".to_string(),
        "components".to_string(),
        "public".to_string(),
        "package.json".to_string(),
        "tsconfig.json".to_string(),
        "next.config.js".to_string(),
        "next.config.ts".to_string(),
        "postcss.config.js".to_string(),
        "postcss.config.mjs".to_string(),
        "tailwind.config.js".to_string(),
        "tailwind.config.ts".to_string(),
        "eslint.config.js".to_string(),
        "eslint.config.mjs".to_string(),
    ]
}

pub fn default_command_allowlist() -> Vec<String> {
    // Base commands (no args) plus common install variants that often include args
    vec![
        // npm
        "npm ci".to_string(),
        "npm run build".to_string(),
        "npm run dev".to_string(),
        "npm install".to_string(),
        "npm i".to_string(), // new

        // pnpm
        "pnpm i".to_string(),
        "pnpm build".to_string(),
        "pnpm dev".to_string(),
        "pnpm install".to_string(),
        "pnpm add".to_string(), // new

        // yarn
        "yarn".to_string(),
        "yarn build".to_string(),
        "yarn dev".to_string(),
        "yarn install".to_string(),
        "yarn add".to_string(), // new
    ]
}
