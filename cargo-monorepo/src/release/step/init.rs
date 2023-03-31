use crate::{
    release::{ReleaseContext, ReleaseStep},
    utils::run_and_capture_stdout,
};
use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use cargo_metadata::{Metadata, MetadataCommand};
use std::env;
use tokio::process::Command;

pub struct Init;

impl Init {
    async fn acquire_tokens(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let registry = ctx.release_config()?.registry.clone();
        let crates_io_token = get_crate_registry_token(registry)?;
        ctx.crates_io_token = Some(crates_io_token);

        if ctx.config.github.is_some() {
            let github_token = get_github_token()?;
            ctx.set_github_token(github_token)?;
        }

        Ok(())
    }

    async fn process_git_state(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        if !git_installed().await {
            bail!("git is missing");
        }
        let current_commit = get_current_commit()
            .await
            .with_context(|| "Failed to get current git commit")?;
        println!("\tCurrent commit is {}", current_commit);
        ctx.current_commit = Some(current_commit);
        Ok(())
    }

    async fn process_metadata(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let medatada = query_metadata().await?;
        let root_crate_name = ctx.root_crate_name();

        let root_package = medatada
            .packages
            .iter()
            .find(|p| p.name == root_crate_name)
            .ok_or_else(|| {
                anyhow!(
                    "Failed to find root crate ({}) in workspace",
                    root_crate_name
                )
            })?;

        let version = root_package.version.clone();
        println!(
            "\tPending version of {} to release is {}",
            root_crate_name, version
        );
        ctx.metadata = Some(medatada);
        ctx.version = Some(version);

        Ok(())
    }
}

#[async_trait]
impl ReleaseStep for Init {
    fn start_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String> {
        Ok(format!(
            "Initializing release process for {}",
            ctx.config.workspace.root_crate
        ))
    }

    fn success_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        Ok("Initialization completed".to_owned())
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        self.acquire_tokens(ctx).await?;
        self.process_git_state(ctx).await?;
        self.process_metadata(ctx).await?;
        Ok(())
    }
}

fn get_github_token() -> anyhow::Result<String> {
    const VAR_NAME: &str = "GITHUB_TOKEN";
    let var = env::var(VAR_NAME).with_context(|| {
        format!(
            "GitHub token is missing, please provide it via {} env var",
            VAR_NAME
        )
    })?;

    Ok(var)
}

fn get_crate_registry_token(registry: Option<String>) -> anyhow::Result<String> {
    use convert_case::{Case, Casing};

    let var_name = registry
        .as_ref()
        .map(|r| format!("CARGO_REGISTRIES_{}_TOKEN", r.to_case(Case::UpperSnake)))
        .unwrap_or_else(|| "CARGO_REGISTRY_TOKEN".to_owned());

    let token = env::var(&var_name).with_context(|| {
        format!(
            "Crate resitry token is missing, please specify it via {} env var",
            var_name
        )
    })?;

    Ok(token)
}

async fn git_installed() -> bool {
    let mut cmd = Command::new("git");
    cmd.arg("--version");
    run_and_capture_stdout(&mut cmd).await.is_ok()
}

async fn query_metadata() -> anyhow::Result<Metadata> {
    MetadataCommand::new()
        .exec()
        .map_err(|e| anyhow!("Failed to parse cargo metadata: {}", e))
}

async fn get_current_commit() -> anyhow::Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "--verify", "HEAD"]);
    run_and_capture_stdout(&mut cmd)
        .await
        .map(|s| s.trim().to_owned())
}
