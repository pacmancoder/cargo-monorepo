use anyhow::bail;
use tokio::{
    io::{self, AsyncWriteExt},
    process::Command as OsCommand,
};

pub async fn run_and_capture_stdout(cmd: &mut OsCommand) -> anyhow::Result<String> {
    let out = cmd.output().await?;
    if !out.status.success() {
        io::stdout().write_all(&out.stdout).await?;
        io::stderr().write_all(&out.stderr).await?;
        bail!("Failed to query crates.io packages");
    }

    Ok(String::from_utf8(out.stdout)?)
}

pub fn shorten_commit(commit: impl AsRef<str>) -> String {
    commit.as_ref()[0..7].to_owned()
}
