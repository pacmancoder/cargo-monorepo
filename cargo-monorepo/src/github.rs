use anyhow::Context;
use octocrab::{models::ReleaseId, Octocrab};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::Path, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

impl Repo {
    #[cfg(test)]
    pub fn new(owner: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
        }
    }
}

impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

impl Serialize for Repo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let combined = format!("{}/{}", self.owner, self.name);
        combined.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Repo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let combinded = String::deserialize(deserializer)?;

        let result = combinded.parse().map_err(|_| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&combinded),
                &"repo name in 'owner/name' format",
            )
        })?;
        Ok(result)
    }
}

impl FromStr for Repo {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.splitn(2, '/').collect::<Vec<_>>();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            anyhow::bail!("Invalid repo name");
        }

        Ok(Self {
            owner: parts[0].to_owned(),
            name: parts[1].to_owned(),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePageBodySource {
    None,
    Changelog,
}

impl Default for ReleasePageBodySource {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct TestToml {
        repo: Repo,
    }

    #[test]
    fn repo_parsing() {
        expect![[r#"
            Ok(
                Repo {
                    owner: "user",
                    name: "repo",
                },
            )
        "#]]
        .assert_debug_eq(&"user/repo".parse::<Repo>());
        expect![[r#"
            Err(
                "Invalid repo name",
            )
        "#]]
        .assert_debug_eq(&"failure".parse::<Repo>());
        expect![[r#"
            Err(
                "Invalid repo name",
            )
        "#]]
        .assert_debug_eq(&"/".parse::<Repo>());
        expect![[r#"
            Err(
                "Invalid repo name",
            )
        "#]]
        .assert_debug_eq(&"owner/".parse::<Repo>());
        expect![[r#"
                Err(
                    "Invalid repo name",
                )
            "#]]
        .assert_debug_eq(&"/name".parse::<Repo>());
    }

    #[test]
    fn repo_roundtrip() {
        let test_toml = TestToml {
            repo: Repo::new("owner", "repo"),
        };

        let serialized = toml::to_string(&test_toml).unwrap();
        expect![[r#"
            repo = "owner/repo"
        "#]]
        .assert_eq(&serialized);
        let deserialized = toml::from_str::<TestToml>(&serialized).unwrap();
        assert_eq!(test_toml, deserialized);
    }

    #[test]
    fn repo_deserialize_failure() {
        let invalid_toml = r#"repo = "invalid""#;

        expect![[r#"
            Err(
                Error {
                    inner: ErrorInner {
                        kind: Custom,
                        line: Some(
                            0,
                        ),
                        col: 0,
                        at: Some(
                            0,
                        ),
                        message: "invalid value: string \"invalid\", expected repo name in 'owner/name' format",
                        key: [
                            "repo",
                        ],
                    },
                },
            )
        "#]]
            .assert_debug_eq(&toml::from_str::<TestToml>(invalid_toml));
    }
}

pub async fn upload_github_release_asset(
    octocrab: &Octocrab,
    repo: &Repo,
    release_id: ReleaseId,
    file_path: &Path,
) -> anyhow::Result<()> {
    let file = std::path::Path::new(file_path);
    let file_name = file.file_name().unwrap().to_str().unwrap();

    let release_upload_url = format!(
        "https://uploads.github.com/repos/{owner}/{repo}/releases/{release_id}/assets",
        owner = repo.owner,
        repo = repo.name,
        release_id = release_id,
    );
    let mut release_upload_url =
        url::Url::from_str(&release_upload_url).expect("BUG: Invalid asset upload url");
    release_upload_url.set_query(Some(format!("{}={}", "name", file_name).as_str()));
    let file_size = std::fs::metadata(file)
        .expect("Can't get asset metadata")
        .len();
    let file = tokio::fs::File::open(file)
        .await
        .expect("Failed to open asset file");
    let stream = tokio_util::codec::FramedRead::new(file, tokio_util::codec::BytesCodec::new());
    let body = reqwest::Body::wrap_stream(stream);
    let builder = octocrab
        .request_builder(release_upload_url.as_str(), reqwest::Method::POST)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", file_size.to_string());
    let resp = builder
        .body(body)
        .send()
        .await
        .with_context(|| "Failed to send upload artifact request")?;

    resp.error_for_status()
        .with_context(|| "Artifact upload failed")?;

    Ok(())
}
