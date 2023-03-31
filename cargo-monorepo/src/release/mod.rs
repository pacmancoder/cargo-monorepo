mod context;
mod step;

use self::context::ReleaseContext;
use crate::config::Config;
use async_trait::async_trait;
use std::collections::VecDeque;

#[derive(clap::Parser, Debug)]
#[structopt(about = "Automatically prepare new repo release")]
pub struct Command {
    /// Actually execute command instead of dry run
    #[structopt(long)]
    confirm: bool,
    /// Do not publish packages to the registry
    #[structopt(long)]
    nopublish: bool,
}

#[async_trait]
trait ReleaseStep {
    fn start_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String>;
    fn success_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String>;

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()>;
}

struct ReleaseExecutor {
    context: ReleaseContext,
    steps: VecDeque<Box<dyn ReleaseStep>>,
}

impl ReleaseExecutor {
    pub fn new(config: Config, dry_run: bool, nopublish: bool) -> Self {
        Self {
            context: ReleaseContext::new(config, dry_run, nopublish),
            steps: Default::default(),
        }
    }

    fn add_step(&mut self, step: impl ReleaseStep + 'static) {
        self.steps.push_back(Box::new(step));
    }

    fn build_steps(&mut self) -> anyhow::Result<()> {
        // Validation steps
        self.add_step(step::Init);
        if self.context.config.artifacts.is_some() {
            self.add_step(step::CollectArtifacts);
        }
        if self.context.config.changelog.is_some() {
            self.add_step(step::CaptureChangelog);
        }
        if let Some(github) = &self.context.release_config()?.github {
            if github.check_commit_pushed {
                self.add_step(step::ValidateCommitPushedToGithub);
            }
        }
        self.add_step(step::VaidateVersion);
        self.add_step(step::CargoPublish::validate_only());
        if !(self.context.is_dry_run() || self.context.is_nopublish()) {
            self.add_step(step::CargoPublish::new());
        }
        if self.context.release_config()?.github.is_some() {
            if self
                .context
                .release_config()?
                .github
                .as_ref()
                .unwrap()
                .create_tag
            {
                self.add_step(step::CreateTagOnGithub);
            }
            if self
                .context
                .release_config()?
                .github
                .as_ref()
                .unwrap()
                .create_release_page
            {
                self.add_step(step::CreateGithubRelease);
            }
        }
        // Release steps
        // TODO

        Ok(())
    }

    pub async fn execute(mut self) -> anyhow::Result<()> {
        self.build_steps()?;

        let Self {
            mut context, steps, ..
        } = self;

        for step in steps {
            println!("🧪️ {}", step.start_message(&context)?);
            step.execute(&mut context).await?;
            println!("✅ {}", step.success_message(&context)?);
        }

        println!(
            "🚀 Workspace version {} has been released!",
            context.version()?,
        );

        Ok(())
    }
}

impl Command {
    pub async fn run(self, config: Config) -> anyhow::Result<()> {
        if self.confirm {
            println!("📦 Running release in production mode!");
        } else {
            println!("🤖 Running release in dry-run mode!");
        }

        let executor = ReleaseExecutor::new(config, !self.confirm, self.nopublish);
        executor.execute().await?;

        Ok(())
    }
}
