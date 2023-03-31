pub(crate) mod cargo;
pub(crate) mod config;
pub(crate) mod github;
pub(crate) mod template;
pub(crate) mod utils;

mod release;

use crate::config::Config;
use anyhow::Context;
use clap::Parser as _;
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[structopt(about = env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    /// Explicitly set manifest to process instead of
    /// choosing manifest in current working directory
    #[structopt(long, default_value = "monorepo.toml")]
    manifest_path: PathBuf,
    #[structopt(subcommand)]
    subcommand: Subcommand,
}

#[derive(clap::Parser, Debug)]
#[structopt(about = env!("CARGO_PKG_DESCRIPTION"))]
enum Subcommand {
    Release(release::Command),
}

async fn run(args: Args) -> anyhow::Result<()> {
    let manifest_path_str = args.manifest_path.display();

    let config_content = tokio::fs::read_to_string(&args.manifest_path)
        .await
        .with_context(|| format!("Failed to read {} config", manifest_path_str))?;

    let config: Config = toml::from_str(&config_content)
        .with_context(|| format!("Failed to parse {}", manifest_path_str))?;

    config
        .validate()
        .with_context(|| "Config validation failed")?;

    if let Some(working_dir) = args.manifest_path.parent() {
        std::env::set_current_dir(working_dir).expect("Failed to set working dir");
    }

    match args.subcommand {
        Subcommand::Release(cmd) => cmd.run(config).await,
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();
    if let Err(e) = run(args).await {
        println!("❌ {:#}", e);
        std::process::exit(1);
    }
}
