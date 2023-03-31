use crate::release::{ReleaseContext, ReleaseStep};
use crate::{github::upload_github_release_asset, utils::shorten_commit};
use anyhow::Context;
use async_trait::async_trait;
use octocrab::params::repos::Reference;

pub struct ValidateCommitPushedToGithub;

#[async_trait]
impl ReleaseStep for ValidateCommitPushedToGithub {
    fn start_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String> {
        let github_config = ctx.github_config()?;
        let commit = shorten_commit(ctx.current_commit()?);
        Ok(format!(
            "Checking that commit {} is pushed to {}",
            commit, github_config.repo
        ))
    }

    fn success_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        Ok("Success! Current commit is pushed to the remote".to_owned())
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let repo = ctx.github_config()?.repo.clone();
        let commit = ctx.current_commit()?;
        ctx.github_client()?
            .repos(repo.owner, repo.name)
            .combined_status_for_ref(&Reference::Commit(commit.clone()))
            .await
            .with_context(|| "Current commit is missing in the GitHub remote")?;
        Ok(())
    }
}

pub struct CreateTagOnGithub;

#[async_trait]
impl ReleaseStep for CreateTagOnGithub {
    fn start_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String> {
        let version = ctx.version()?;
        Ok(format!("Creating new tag for version {}", version))
    }

    fn success_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        Ok("Tag has been created".to_owned())
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let tempalte_context = ctx.text_template_context()?;

        let tag = ctx
            .release_github_config()?
            .tag_name_template
            .render(&tempalte_context)?;
        ctx.set_github_release_tag(tag.clone());

        let repo = ctx.github_config()?.repo.clone();
        let commit = ctx.current_commit()?;

        println!("\t Tag `{}` will be created for commit {}", tag, commit);

        if ctx.is_dry_run() {
            println!("Skipping tag creation in dry run mode");
            return Ok(());
        }

        ctx.github_client()?
            .repos(repo.owner, repo.name)
            .create_ref(&Reference::Tag(tag), commit)
            .await
            .with_context(|| "Failed to create new tag in GitHub repo")?;

        Ok(())
    }
}

pub struct CreateGithubRelease;

#[async_trait]
impl ReleaseStep for CreateGithubRelease {
    fn start_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String> {
        let tag = ctx.github_release_tag()?;
        Ok(format!("Creating new GitHub release for tag `{}`", tag))
    }

    fn success_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        Ok("GitHub release has been created".to_owned())
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let tempalte_context = ctx.text_template_context()?;

        let title = ctx
            .release_github_config()?
            .release_page_title_template
            .render(&tempalte_context)?;

        let body = ctx
            .release_github_config()?
            .release_page_body_template
            .render(&tempalte_context)?;

        let repo = ctx.github_config()?.repo.clone();
        let tag = ctx.github_release_tag()?;

        if ctx.release_github_config()?.print_to_stdout {
            println!("GitHub release title:");
            println!("{}", title);
            println!("GitHub release body:");
            println!("{}", body);
        }

        if ctx.is_dry_run() {
            println!("Skipping GitHub release creation in dry run mode");
            return Ok(());
        }

        let release = ctx
            .github_client()?
            .repos(&repo.owner, &repo.name)
            .releases()
            .create(&tag)
            .name(&title)
            .body(&body)
            .draft(false)
            .prerelease(false)
            .send()
            .await
            .with_context(|| "Failed to create GitHub release")?;

        if ctx.release_github_config()?.release_page_upload_artifacts
            && !ctx.artifacts()?.is_empty()
        {
            let artifacts = ctx.artifacts()?.to_vec();
            for artifact in artifacts {
                println!("Uploading release artifact {}", artifact.display());
                upload_github_release_asset(ctx.github_client()?, &repo, release.id, &artifact)
                    .await?;
            }
        }

        Ok(())
    }
}
