use async_trait::async_trait;
use anyhow::bail;
use tokio::process::Command;
use semver::Version;
use crate::{
    release::{
        ReleaseStep,
        ReleaseContext,
    },
    utils::run_and_capture_stdout,
};
use cargo_metadata::{DependencyKind, Package};

pub struct VaidateVersion;

pub const CRATES_IO_REGISTRY_NAME: &str = "crates-io";

impl VaidateVersion {
    async fn check_version_raised(&self, version: Version, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        if !ctx.release_config()?.check_version_raised {
            println!("\tVersion raise check was skipped");
            return Ok(())
        } else {
            println!("\tChecking that version has been raised...");
        }

        // If crate is not new, check that version has been raised
        let prev_version = query_last_released_version(&ctx.root_crate_name()).await?;
        ctx.prev_version = if let Some(prev_version) = prev_version {
            println!("\tQueried previous crate version: {}", prev_version);
            if version <= prev_version {
                bail!("Pending version is lower or equal to already published version")
            }
            Some(Some(prev_version))
        } else {
            println!("\tWARN: Previously published root crate not found");
            Some(None)
        };

        Ok(())
    }

    async fn check_dev_dependencies(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        if ctx.release_config()?.allow_non_path_dev_dependencies {
            return Ok(())
        }

        println!("\tChecking create workspace dependencies...");

        let workspace_packages = ctx.packages_to_publish()?;

        let workspace_package_names = ctx.workspace_package_names()?;

        let mut invalid_dev_dependencies = false;

        for package in workspace_packages {

            let mut package_validation_failed = false;
            let mut broken_dev_deps = vec![];

            for dep in &package.dependencies {
                if dep.kind != DependencyKind::Development
                    || !workspace_package_names.contains(&dep.name) {
                    continue;
                }

                if !dep.req.comparators.is_empty() {
                    broken_dev_deps.push(dep.name.clone());
                    package_validation_failed = true;
                }
            }

            if package_validation_failed {
                let package_name = full_package_name(&package);
                println!("\t❌ {} has invalid dev-dependencies ({:?})", package_name, broken_dev_deps);
                invalid_dev_dependencies = true;
            }
        }

        if invalid_dev_dependencies {
            bail!(
                "Detected invalid dev dependencies: version field should not be \
                specified for in-workspace dev-dependencies");
        }

        Ok(())
    }

    async fn check_registry_consistency(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        println!("\tChecking package registry consistency...");
        let workspace_packages = ctx.packages_to_publish()?;

        let registry = ctx.release_config()?.registry.clone();

        let mut inconsistent_registries = false;

        for p in &workspace_packages {
            let publish_allowed = p.publish.as_ref().map_or(true, |allowed| {
                match &registry {
                    Some(name) => allowed.contains(name),
                    None => allowed.contains(&CRATES_IO_REGISTRY_NAME.to_owned()),
                }
            });

            let registry_name = registry
                .clone()
                .unwrap_or(CRATES_IO_REGISTRY_NAME.to_owned());

            if !publish_allowed {
                let package_name = full_package_name(p);
                println!("\t❌ {} does not allow publish to `{}` registry", package_name, registry_name);
                inconsistent_registries = true;
            }
        }

        if inconsistent_registries {
            bail!("Package registry inconsistency detected");
        }

        Ok(())
    }

    async fn check_version_consistency(&self, version: Version, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        println!("\tChecking for crates version consistency...");

        let packages_to_publish = ctx.packages_to_publish()?;
        let workspace_package_names = ctx.workspace_package_names()?;

        let mut inconsistent = false;

        for package in packages_to_publish.iter() {
            let full_name = full_package_name(package);

            if package.version.clone() != version {
                inconsistent = true;
                println!("\t❌ {} have inconsistent version", full_name);
                continue;
            }

            let mut dependenies_inconsistent = false;
            let mut inconsistent_deps_list = vec![];

            for dep in &package.dependencies {
                let dep_inconsistent = workspace_package_names.contains(&dep.name)
                    && !dep.req.matches(&version);

                if dep_inconsistent {
                    inconsistent_deps_list.push(format!("{} {}", dep.name, dep.req));
                    dependenies_inconsistent = true;
                }
            }

            if dependenies_inconsistent {
                inconsistent = true;
                println!(
                    "\t❌ {} has inconsistent monorepo dependencies ({})",
                    full_name,
                    inconsistent_deps_list.join(", "),
                );
                continue;
            }

            println!("\t✅ {} is OK", full_name);
        }

        if inconsistent {
            bail!("Detected version inconsistency in crates");
        }

        Ok(())
    }
}

#[async_trait]
impl ReleaseStep for VaidateVersion {
    fn start_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        Ok(format!("Validating repo versioning"))
    }

    fn success_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        Ok(format!("Version validation done"))
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let version = ctx.version()?;
        self.check_registry_consistency(ctx).await?;
        self.check_version_raised(version.clone(), ctx).await?;
        self.check_dev_dependencies(ctx).await?;
        self.check_version_consistency(version, ctx).await?;

        Ok(())
    }
}

async fn query_last_released_version(crate_name: &str) -> anyhow::Result<Option<Version>> {
    let mut cmd = Command::new("cargo");
    cmd.args(["search", crate_name]);
    let stdout = run_and_capture_stdout(&mut cmd).await?;

    let crate_prefix = format!("{} = ", crate_name);

    let version_str = stdout
        .split("\n")
        .find(|s| s.starts_with(&crate_prefix))
        .map(|s| s.trim().split('"').nth(1))
        .flatten();

    let version = version_str
        .map(|s| Version::parse(s))
        .transpose()?;

    Ok(version)
}


fn full_package_name(p: &Package) -> String {
    format!("{} v{}", p.name, p.version)
}
