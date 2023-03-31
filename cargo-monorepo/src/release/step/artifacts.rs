use crate::release::{ReleaseContext, ReleaseStep};
use anyhow::bail;
use async_trait::async_trait;

pub struct CollectArtifacts;

#[async_trait]
impl ReleaseStep for CollectArtifacts {
    fn start_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String> {
        let directory = &ctx.artifacts_config()?.directory;
        Ok(format!(
            "Collecting artifacts from '{}'",
            directory.display()
        ))
    }

    fn success_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String> {
        let count = ctx.artifacts.as_ref().map(|a| a.len()).unwrap_or(0);
        Ok(format!("Collected {} artifact(s)", count))
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let artifacts_config = ctx.artifacts_config()?;

        let artifacts_folder = artifacts_config.directory.clone();

        if !artifacts_folder.exists() {
            bail!("Artifacts folder does not exist");
        }

        let artifacts = std::fs::read_dir(&artifacts_folder)?.collect::<Result<Vec<_>, _>>()?;

        if artifacts_config.check_not_empty && artifacts.is_empty() {
            bail!("Artifacts folder is empty");
        }

        let artifacts = artifacts
            .iter()
            .filter_map(|a| {
                let is_file = a.metadata().ok()?.is_file();
                is_file.then(|| {
                    let path = a.path();
                    println!("\tFound artifact: {}", path.display());
                    path
                })
            })
            .collect::<Vec<_>>();

        ctx.artifacts = Some(artifacts);

        Ok(())
    }
}
