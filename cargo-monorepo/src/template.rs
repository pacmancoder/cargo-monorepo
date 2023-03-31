use anyhow::{anyhow, Context};
use handlebars::Handlebars;
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct TextTemplateContext {
    pub root_crate: String,
    pub version: Version,
    pub changelog: Option<String>,
}

#[derive(Clone)]
pub struct TextTemplate {
    renderer: Handlebars<'static>,
}

impl TextTemplate {
    pub fn new(template: &str) -> anyhow::Result<Self> {
        let mut renderer = Handlebars::new();
        renderer.set_strict_mode(true);
        renderer
            .register_template_string("t", template)
            .with_context(|| format!("Invalid template: {}", template))?;

        Ok(Self { renderer })
    }

    pub fn render(&self, context: &TextTemplateContext) -> anyhow::Result<String> {
        self.renderer
            .render("t", context)
            .map_err(|e| anyhow!("Failed to render template: {}", e))
    }
}

impl<'de> Deserialize<'de> for TextTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        let template = String::deserialize(deserializer)?;
        Self::new(&template).map_err(|_| {
            de::Error::invalid_value(
                de::Unexpected::Str(&format!("invalid tempalte '{}'", template)),
                &"valid handlebars template",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[derive(Deserialize)]
    struct TestToml {
        template: TextTemplate,
    }

    #[test]
    fn check_templating_works() {
        let context = TextTemplateContext {
            root_crate: "monorepo".to_owned(),
            version: Version::new(1, 1, 1),
            changelog: None,
        };

        let template = toml::from_str::<TestToml>("template = \"{{root_crate}} - {{version}}\"")
            .unwrap()
            .template;

        let rendered = template.render(&context).unwrap();

        expect![[r#"monorepo - 1.1.1"#]].assert_eq(&rendered);
    }

    #[test]
    fn deserialize_fails() {
        let result = toml::from_str::<TestToml>("template = \"{{{{aaaaa\"").map(|_| ());
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
                        message: "invalid value: string \"invalid tempalte '{{{{aaaaa'\", expected valid handlebars template",
                        key: [
                            "template",
                        ],
                    },
                },
            )
        "#]]
            .assert_debug_eq(&result);
    }
}
