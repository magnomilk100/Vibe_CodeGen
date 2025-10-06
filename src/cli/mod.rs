use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(ValueEnum, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    #[value(alias = "open-ai", alias = "openai")]
    OpenAI,
    #[value(alias = "anthropic")]
    Anthropic,
    #[value(alias = "ollama")]
    Ollama,
}

#[derive(Parser, Debug)]
#[command(name="vibe_codeGen", version, about="LLM code generator/executor over .vibe/out artifacts")]
pub struct Args {
    #[arg(long, default_value = ".")]
    pub root: String,

    #[arg(long, default_value = "vibe-index/.vibe/out")]
    pub vibe_out: String,

    #[arg(long, value_enum, default_value_t = ProviderKind::OpenAI)]
    pub provider: ProviderKind,

    #[arg(long, default_value = "gpt-4.1-mini")]
    pub model: String,

    #[arg(long)]
    pub task: Option<String>,

    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    #[arg(long, default_value_t = false)]
    pub auto_approve: bool,

    #[arg(long, default_value_t = 2400)]
    pub timeout_secs: u64,

    #[arg(long, default_value_t = true)]
    pub save_request: bool,

    #[arg(long, default_value_t = true)]
    pub save_response: bool,

    #[arg(long, default_value_t = false)]
    pub debug: bool,

    #[arg(long, default_value_t = true)]
    pub progress: bool,

    #[arg(long)]
    pub config: Option<String>,
}
