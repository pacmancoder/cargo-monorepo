use crate::{
    config::{self, Config},
    template::TextTemplateContext,
};
use anyhow::{anyhow, Context};
use cargo_metadata::{Metadata, Package};
use octocrab::Octocrab as GithubClient;
use semver::Version;
use std::path::PathBuf;

pub struct ReleaseContext {
    pub dry_run: bool,
    pub nopublish: bool,
    pub config: Config,
    pub crates_io_token: Option<String>,
    github_token: Option<String>,
    pub current_commit: Option<String>,
    pub metadata: Option<Metadata>,
    pub version: Option<Version>,
    pub prev_version: Option<Option<Version>>,
    pub changelog: Option<String>,
    pub artifacts: Option<Vec<PathBuf>>,
    github_release_tag: Option<String>,
    github_client: Option<GithubClient>,
}

impl ReleaseContext {
    pub fn new(config: Config, dry_run: bool, nopublish: bool) -> Self {
        ReleaseContext {
            dry_run,
            nopublish,
            config,
            crates_io_token: None,
            github_token: None,
            current_commit: None,
            metadata: None,
            version: None,
            prev_version: None,
            changelog: None,
            artifacts: None,
            github_release_tag: None,
            github_client: None,
        }
    }

    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    pub fn is_nopublish(&self) -> bool {
        self.nopublish
    }

    pub fn root_crate_name(&self) -> String {
        self.config.workspace.root_crate.clone()
    }

    pub fn github_config(&self) -> anyhow::Result<&config::GitHub> {
        self.config
            .github
            .as_ref()
            .ok_or_else(|| anyhow!("github section is missing from the config"))
    }

    pub fn release_config(&self) -> anyhow::Result<&config::Release> {
        self.config
            .release
            .as_ref()
            .ok_or_else(|| anyhow!("release section is missing from the config"))
    }

    pub fn release_github_config(&self) -> anyhow::Result<&config::GithubRelease> {
        self.release_config()?
            .github
            .as_ref()
            .ok_or_else(|| anyhow!("release section is missing from the config"))
    }

    pub fn artifacts_config(&self) -> anyhow::Result<&config::Artifacts> {
        self.config
            .artifacts
            .as_ref()
            .ok_or_else(|| anyhow!("artifacts section is missing from the config"))
    }

    pub fn changelog_config(&self) -> anyhow::Result<&config::Changelog> {
        self.config
            .changelog
            .as_ref()
            .ok_or_else(|| anyhow!("changelog section is missing from the config"))
    }

    pub fn current_commit(&self) -> anyhow::Result<String> {
        self.current_commit
            .clone()
            .ok_or_else(|| anyhow!("Current commit is queried yet"))
    }

    pub fn cargo_metadata(&self) -> anyhow::Result<&Metadata> {
        self.metadata
            .as_ref()
            .ok_or_else(|| anyhow!("Cargo metadata is not yet queried"))
    }

    pub fn workspace_package_names(&self) -> anyhow::Result<Vec<String>> {
        let metadata = self.cargo_metadata()?;
        let workspace_package_ids = &metadata.workspace_members;
        let names = metadata
            .packages
            .iter()
            .filter_map(|p| {
                workspace_package_ids
                    .contains(&p.id)
                    .then(|| p.name.clone())
            })
            .collect();
        Ok(names)
    }

    pub fn packages_to_publish(&self) -> anyhow::Result<Vec<&Package>> {
        let metadata = self.cargo_metadata()?;

        let packages = metadata
            .packages
            .iter()
            .filter(|p| {
                // for publish = false, package.publish would contain Some(vec![])
                metadata.workspace_members.contains(&p.id)
                    && p.publish.as_ref().map_or(true, |r| !r.is_empty())
            })
            .collect();

        Ok(packages)
    }

    pub fn ordered_packages_to_publish(&self) -> anyhow::Result<Vec<&Package>> {
        let metadata = self.cargo_metadata()?;
        let sorted = crate::cargo::sort_workspace(metadata)?;
        let packages_to_publish = self.packages_to_publish()?;
        let mut ordered_packages = vec![];

        for s in sorted {
            let package_to_publish = packages_to_publish.iter().copied().find(|p| p.id == s);

            let package_to_publish = match package_to_publish {
                Some(p) => p,
                None => continue,
            };
            ordered_packages.push(package_to_publish);
        }

        Ok(ordered_packages)
    }

    pub fn version(&self) -> anyhow::Result<Version> {
        self.version
            .clone()
            .ok_or_else(|| anyhow!("Pending version is not queried yet"))
    }

    pub fn github_client(&self) -> anyhow::Result<&GithubClient> {
        self.github_client
            .as_ref()
            .ok_or_else(|| anyhow!("GitHub client is not initialized"))
    }

    pub fn artifacts(&self) -> anyhow::Result<&[PathBuf]> {
        self.artifacts
            .as_deref()
            .ok_or_else(|| anyhow!("Artifacts list is empty"))
    }

    pub fn text_template_context(&self) -> anyhow::Result<TextTemplateContext> {
        let ctx = TextTemplateContext {
            root_crate: self.root_crate_name(),
            version: self.version()?,
            changelog: self.changelog.clone(),
        };

        Ok(ctx)
    }

    pub fn set_github_token(&mut self, token: String) -> anyhow::Result<()> {
        let github_client = GithubClient::builder()
            .personal_token(token.clone())
            .build()
            .with_context(|| "Failed to create GitHub client")?;
        self.github_token = Some(token);
        self.github_client = Some(github_client);
        Ok(())
    }

    pub fn set_github_release_tag(&mut self, tag: String) {
        self.github_release_tag = Some(tag);
    }

    pub fn github_release_tag(&self) -> anyhow::Result<String> {
        self.github_release_tag
            .clone()
            .with_context(|| "GitHub tag is not created yet")
    }
}
