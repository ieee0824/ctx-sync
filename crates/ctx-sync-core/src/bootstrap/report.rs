//! Findings of `bootstrap`, split by how certain they are.
//!
//! Only `Confirmed` findings are facts. `Inferred` findings are guesses and
//! must never become decisions automatically.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Certainty {
    Confirmed,
    Inferred,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub certainty: Certainty,
    /// e.g. "Language".
    pub topic: String,
    /// e.g. "Rust".
    pub text: String,
    /// e.g. "Cargo.toml".
    pub source: Option<String>,
}

impl Finding {
    pub fn confirmed(topic: &str, text: &str, source: Option<&str>) -> Self {
        Self::new(Certainty::Confirmed, topic, text, source)
    }

    pub fn inferred(topic: &str, text: &str, source: Option<&str>) -> Self {
        Self::new(Certainty::Inferred, topic, text, source)
    }

    pub fn unknown(topic: &str, text: &str) -> Self {
        Self::new(Certainty::Unknown, topic, text, None)
    }

    fn new(certainty: Certainty, topic: &str, text: &str, source: Option<&str>) -> Self {
        Self {
            certainty,
            topic: topic.to_string(),
            text: text.to_string(),
            source: source.map(str::to_string),
        }
    }

    fn render(&self) -> String {
        match &self.source {
            Some(source) => format!("- {}: {} (source: {source})", self.topic, self.text),
            None => format!("- {}: {}", self.topic, self.text),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BootstrapReport {
    pub findings: Vec<Finding>,
}

impl BootstrapReport {
    pub fn extend(&mut self, findings: Vec<Finding>) {
        self.findings.extend(findings);
    }

    /// The whole document, starting with `# Initial Context`.
    pub fn render_markdown(&self) -> String {
        format!("# Initial Context\n\n{}", self.render_groups("##"))
    }

    /// Body for the `## Initial Context` section of `10-project.md`
    /// (headings are `###`).
    pub fn render_section_body(&self) -> String {
        self.render_groups("###")
    }

    fn render_groups(&self, heading: &str) -> String {
        let groups = [
            (Certainty::Confirmed, "Confirmed"),
            (Certainty::Inferred, "Inferred"),
            (Certainty::Unknown, "Unknown"),
        ];
        let mut blocks = Vec::new();
        for (certainty, title) in groups {
            let items: Vec<String> = self
                .findings
                .iter()
                .filter(|f| f.certainty == certainty)
                .map(Finding::render)
                .collect();
            blocks.push(format!("{heading} {title}"));
            blocks.push(if items.is_empty() {
                "- _None_".to_string()
            } else {
                items.join("\n")
            });
        }
        let mut out = blocks.join("\n\n");
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> BootstrapReport {
        let mut report = BootstrapReport::default();
        report.extend(vec![
            Finding::confirmed("Language", "Rust", Some("Cargo.toml")),
            Finding::inferred(
                "Async runtime",
                "Tokio appears to be used as the async runtime.",
                Some("Cargo.toml"),
            ),
            Finding::unknown("Goal", "The project goal has not been confirmed."),
            Finding::confirmed("Crate type", "binary", None),
        ]);
        report
    }

    #[test]
    fn renders_the_three_groups() {
        assert_eq!(
            report().render_markdown(),
            "# Initial Context

## Confirmed

- Language: Rust (source: Cargo.toml)
- Crate type: binary

## Inferred

- Async runtime: Tokio appears to be used as the async runtime. (source: Cargo.toml)

## Unknown

- Goal: The project goal has not been confirmed.
"
        );
    }

    #[test]
    fn empty_groups_say_none() {
        let mut report = BootstrapReport::default();
        report.extend(vec![Finding::confirmed("Language", "Go", None)]);
        let text = report.render_markdown();
        assert!(text.contains("## Inferred\n\n- _None_\n"));
        assert!(text.ends_with("## Unknown\n\n- _None_\n"));
    }

    #[test]
    fn section_body_uses_level_three_headings() {
        let body = report().render_section_body();
        assert!(body.starts_with("### Confirmed\n\n"));
        assert!(!body.contains("# Initial Context"));
        assert!(body.contains("\n### Unknown\n"));
    }
}
