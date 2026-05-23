#![allow(clippy::doc_markdown)]

use anyhow::Result;
use clap::Parser;

mod adapters;
mod audio;
mod cli;
mod config;
mod ffmpeg;
mod gemini;
mod models;
mod pdf;
mod pipeline;
mod util;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_level(true)
        .compact()
        .init();

    let cli = Cli::parse();
    cli.run().await
}
