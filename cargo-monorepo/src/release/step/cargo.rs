use crate::release::{ReleaseContext, ReleaseStep};
use anyhow::{anyhow, bail};
use async_trait::async_trait;
use std::time::Duration;
use tokio::process::Command;

pub struct CargoPublish {
    validate: bool,
}

impl CargoPublish {
    pub fn new() -> Self {
        Self { validate: false }
    }

    pub fn validate_only() -> Self {
        Self { validate: true }
    }

    async fn publish(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let dry_run = ctx.is_dry_run() || self.validate;

        let ordered_packages = ctx.ordered_packages_to_publish()?;

        if self.validate {
            println!("\tPackage publish order:");
            ordered_packages
                .iter()
                .for_each(|p| println!("\t- {}", p.name));
        }

        let registry = ctx.release_config()?.registry.clone();
        let publish_interval = ctx.release_config()?.publish_interval_seconds;

        if dry_run {
            'packages_loop: for p in ordered_packages {
                println!("Validating {}...", p.name);
                for target in &p.targets {
                    if target.kind.contains(&"bin".to_owned()) {
                        println!("WARN: Skipped validation of bin crate {}", p.name);
                        continue 'packages_loop;
                    }
                }
                execute_publish(&p.manifest_path.to_string(), &registry, true).await?;
                println!("{} has been successfully validated!", p.name);
            }

            // We don't need actual publish here
            return Ok(());
        }

        let mut previously_published = false;

        for p in ordered_packages {
            if previously_published {
                println!(
                    "Waiting for {} seconds before publishing next crate...",
                    publish_interval
                );
                tokio::time::sleep(Duration::from_secs(publish_interval as u64)).await;
            }
            println!("Publishing {}...", p.name);
            execute_publish(&p.manifest_path.to_string(), &registry, false).await?;
            previously_published = true;
            println!("{} has been successfully published!", p.name);
        }

        Ok(())
    }
}

#[async_trait]
impl ReleaseStep for CargoPublish {
    fn start_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        if self.validate {
            Ok(format!("Validating cargo publish (with --dry-run)"))
        } else {
            Ok(format!("Running cargo publish"))
        }
    }

    fn success_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        if self.validate {
            Ok(format!("Cargo publish validation passed"))
        } else {
            Ok(format!("Cargo publish succeeded"))
        }
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        if ctx.is_dry_run() && !self.validate {
            bail!(
                "BUG: CargoPublish should not be called \
                in non-validate mode when dry-run is specified"
            );
        }

        self.publish(ctx).await?;

        Ok(())
    }
}

async fn execute_publish(
    manifest_path: &str,
    registry: &Option<String>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    let mut args = vec!["publish", "--manifest-path", manifest_path];

    if let Some(registry) = registry {
        args.push("--registry");
        args.push(registry.as_str());
    }

    if dry_run {
        args.push("--dry-run");
        args.push("--no-verify");
    }

    println!("EXEC: cargo {}", args.join(" "));

    cmd.args(args);

    let result = cmd
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn cargo publish: {}", e))?
        .wait()
        .await
        .map_err(|e| anyhow!("Failed to start cargo publish: {}", e))?;

    if !result.success() {
        bail!("Cargo publish failed");
    }

    Ok(())
}
