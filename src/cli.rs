use clap::Parser;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(name = "vibe_codeGen", version)]
pub struct Cli {
    #[arg(long)]
    pub root: Option<String>,

    #[arg(long)]
    pub vibe_out: Option<String>,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub task: Option<String>,

    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    #[arg(long, default_value_t = false)]
    pub auto_approve: bool,

    #[arg(long, default_value_t = 2400)]
    pub timeout_secs: u64,

    #[arg(long, default_value_t = false)]
    pub save_request: bool,

    #[arg(long, default_value_t = false)]
    pub save_response: bool,

    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        println!("Running vibe_codeGen with task: {:?}", self.task);
        // Further logic will be added here
        Ok(())
    }
}
