use async_trait::async_trait;
use anyhow::{bail, Context};
use crate::{
    release::{
        ReleaseStep,
        ReleaseContext,
    },
};
use tokio::fs;

pub struct CaptureChangelog;

#[async_trait]
impl ReleaseStep for CaptureChangelog {
    fn start_message(&self, ctx: &ReleaseContext) -> anyhow::Result<String> {
        let file = &ctx.changelog_config()?.file;
        Ok(format!("Capturing changelog from '{}'", file.display()))
    }

    fn success_message(&self, _: &ReleaseContext) -> anyhow::Result<String> {
        Ok("Changelog has been captured".to_owned())
    }

    async fn execute(&self, ctx: &mut ReleaseContext) -> anyhow::Result<()> {
        let changelog_config = ctx.changelog_config()?;

        let changelog_bytes = fs::read(&changelog_config.file).await?;
        let changelog =
            String::from_utf8(changelog_bytes).with_context(|| "Changelog is not a text file")?;

        let changelog = if changelog_config.start_marker_template.is_none() {
            changelog
        } else {
            let start_marker_template = changelog_config
                .start_marker_template
                .clone()
                .with_context(|| "start_marker_template is missing")?;
            let end_marker_template = changelog_config
                .end_marker_template
                .clone()
                .with_context(|| "end_marker_template is missing")?;

            let tempalte_context = ctx.text_template_context()?;

            let begin_marker = start_marker_template.render(&tempalte_context)?;
            let end_marker = end_marker_template.render(&tempalte_context)?;

            let changelog_lines = changelog.lines().collect::<Vec<_>>();

            let begin_line = changelog_lines
                .iter()
                .position(|l| l.contains(&begin_marker));
            let end_line = changelog_lines.iter().position(|l| l.contains(&end_marker));

            match (begin_line, end_line) {
                (Some(begin), Some(end)) => {
                    if end <= begin {
                        bail!(
                            "Changelog end barker should be placed \
                            after corresponding begin marker"
                        );
                    }

                    let first_line = begin + 1;
                    if first_line == end {
                        if changelog_config.allow_empty_changelog {
                            println!("\tWARN: empty changelog");
                        } else {
                            bail!("Changelog is empty");
                        }
                        String::new()
                    } else {
                        changelog_lines[first_line..end].join("\n")
                    }
                }
                (None, Some(_)) => {
                    bail!(
                        "Can't find required changelog begin marker {}",
                        begin_marker
                    );
                }
                (Some(_), None) => {
                    bail!("Can't find required changelog end marker {}", end_marker);
                }
                (None, None) => {
                    bail!(
                        "Can't find required changelog markers {} and {}",
                        begin_marker,
                        end_marker
                    );
                }
            }
        };
        if changelog_config.print_to_stdout {
            changelog.lines().for_each(|l| println!("\t{}", l))
        }

        ctx.changelog = Some(changelog);

        Ok(())
    }
}
