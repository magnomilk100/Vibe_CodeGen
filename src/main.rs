mod cli;
mod config;
mod context;
mod llm;
mod plan;
mod apply;
mod utils;

use anyhow::Result;
use cli::Cli;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run().await?;
    Ok(())
}
