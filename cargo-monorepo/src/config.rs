use crate::{github, template::TextTemplate};
use anyhow::bail;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
pub struct Config {
    /// Workspace-related options
    pub workspace: Workspace,
    /// Github-related options
    pub github: Option<GitHub>,
    /// Changelog params
    pub changelog: Option<Changelog>,
    /// Artifacts params
    pub artifacts: Option<Artifacts>,
    /// Release command related options
    pub release: Option<Release>,
}

impl Config {
    fn validate_release(&self) -> anyhow::Result<()> {
        if self.release.is_none() {
            return Ok(());
        }
        let release = self.release.as_ref().unwrap();
        if release.registry.is_some() && release.check_version_raised {
            // `cargo search` allows to specify custom index/registry, however
            // some registries (e.g. Cloudsmith) don't implement cargo search properly.
            // More interestingly, Cloudsmith's publish succeeds even if same version
            // is already exist... So disable this for now to make sure everything is
            // fine
            bail!(
                "Querying last released version is not yet supported for custom registries, \
                set `release.check_version_raised` to false in the config to approve skip of this step"
            );
        }

        if let Some(release_github) = &release.github {
            if self.github.is_none() {
                bail!("github.repo should be specified to be able to use release.github");
            }
            if release_github.release_page_upload_artifacts && self.artifacts.is_none() {
                bail!(
                    "artifacts should be specified when \
                    release.github.release_page_upload_artifacts is set to true"
                );
            }
            if release_github.create_release_page && !release_github.create_tag {
                bail!(
                    "github.create_tag should be enabled when \
                    github.create_release_page is required"
                );
            }
        }

        Ok(())
    }

    fn validate_changelog(&self) -> anyhow::Result<()> {
        if self.changelog.is_none() {
            return Ok(());
        }
        let changelog = self.changelog.as_ref().unwrap();
        if changelog.start_marker_template.is_some() ^ changelog.end_marker_template.is_some() {
            bail!("Both changelog_start_pattern and changelog_end_pattern should be specified");
        }
        Ok(())
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.validate_release()?;
        self.validate_changelog()?;
        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct Workspace {
    /// Main workspace crate which will be used for validation and naming
    pub root_crate: String,
}

#[derive(Deserialize, Clone)]
pub struct GitHub {
    /// Repo in form "owner/repo-name"
    pub repo: github::Repo,
}

#[derive(Deserialize, Clone)]
pub struct Release {
    #[serde(default = "default_bool_true")]
    pub check_version_raised: bool,
    #[serde(default = "default_bool_true")]
    pub allow_non_path_dev_dependencies: bool,
    pub registry: Option<String>,
    #[serde(default = "default_publish_interval_seconds")]
    pub publish_interval_seconds: usize,
    pub github: Option<GithubRelease>,
}

#[derive(Deserialize, Clone)]
pub struct GithubRelease {
    #[serde(default = "default_bool_true")]
    pub check_commit_pushed: bool,
    #[serde(default)]
    pub create_tag: bool,
    #[serde(default = "default_tag_name_template")]
    pub tag_name_template: TextTemplate,
    #[serde(default)]
    pub create_release_page: bool,
    #[serde(default = "default_bool_true")]
    pub release_page_upload_artifacts: bool,
    #[serde(default = "default_release_page_title_template")]
    pub release_page_title_template: TextTemplate,
    #[serde(default = "default_release_page_body_template")]
    pub release_page_body_template: TextTemplate,
    #[serde(default)]
    pub print_to_stdout: bool,
}

#[derive(Deserialize, Clone)]
pub struct Changelog {
    pub file: PathBuf,
    pub start_marker_template: Option<TextTemplate>,
    pub end_marker_template: Option<TextTemplate>,
    #[serde(default)]
    pub print_to_stdout: bool,
    #[serde(default)]
    pub allow_empty_changelog: bool,
}

#[derive(Deserialize, Clone)]
pub struct Artifacts {
    pub directory: PathBuf,
    #[serde(default = "default_bool_true")]
    pub check_not_empty: bool,
}

fn default_bool_true() -> bool {
    true
}

fn default_tag_name_template() -> TextTemplate {
    TextTemplate::new("v{{version}}").unwrap()
}

fn default_release_page_title_template() -> TextTemplate {
    TextTemplate::new("{{root_crate}} v{{version}}").unwrap()
}

fn default_release_page_body_template() -> TextTemplate {
    TextTemplate::new("{{changelog}}").unwrap()
}

fn default_publish_interval_seconds() -> usize {
    30
}
