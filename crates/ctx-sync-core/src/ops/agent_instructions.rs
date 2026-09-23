//! Install ctx-sync guidance in the project's AGENTS.md without changing its existing text.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Serialize;

use crate::Result;

pub const AGENT_INSTRUCTIONS_HEADING: &str = "## Shared Context";

const SECTION: &str = r#"## Shared Context

This project uses ctx-sync.

Before starting work:

    ctx-sync agent start

Before finishing work:

    ctx-sync agent finish

Read shared architecture, decisions, worker state and attention items before changing overlapping code.

Do not place chain-of-thought or temporary debugging reasoning into ctx-sync.
"#;

pub fn agent_instructions_section() -> &'static str {
    SECTION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InstallOutcome {
    Created,
    Appended,
    AlreadyPresent,
}

/// Return the new document only when something needs to be written.
pub fn merge_agent_instructions(existing: Option<&str>) -> (InstallOutcome, Option<String>) {
    match existing {
        None => (
            InstallOutcome::Created,
            Some(format!("# AGENTS.md\n\n{}", agent_instructions_section())),
        ),
        Some(text)
            if text
                .lines()
                .any(|line| line.trim_end_matches('\r') == AGENT_INSTRUCTIONS_HEADING) =>
        {
            (InstallOutcome::AlreadyPresent, None)
        }
        Some(text) => {
            let separator =
                if text.is_empty() || text.ends_with("\n\n") || text.ends_with("\r\n\r\n") {
                    ""
                } else if text.ends_with('\n') {
                    "\n"
                } else {
                    "\n\n"
                };
            let mut merged = String::with_capacity(text.len() + separator.len() + SECTION.len());
            merged.push_str(text);
            merged.push_str(separator);
            merged.push_str(agent_instructions_section());
            (InstallOutcome::Appended, Some(merged))
        }
    }
}

pub fn install_agent_instructions(project_root: &Path) -> Result<InstallOutcome> {
    let path = project_root.join("AGENTS.md");
    let existing = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let (outcome, merged) = merge_agent_instructions(existing.as_deref());
    if let Some(text) = merged {
        fs::write(path, text)?;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_the_document_with_the_specified_section() {
        let (outcome, text) = merge_agent_instructions(None);
        assert_eq!(outcome, InstallOutcome::Created);
        assert_eq!(
            text.unwrap(),
            format!("# AGENTS.md\n\n{}", agent_instructions_section())
        );
        assert_eq!(
            agent_instructions_section(),
            "## Shared Context\n\nThis project uses ctx-sync.\n\nBefore starting work:\n\n    ctx-sync agent start\n\nBefore finishing work:\n\n    ctx-sync agent finish\n\nRead shared architecture, decisions, worker state and attention items before changing overlapping code.\n\nDo not place chain-of-thought or temporary debugging reasoning into ctx-sync.\n"
        );
    }

    #[test]
    fn appends_without_changing_existing_bytes() {
        for (existing, separator) in [
            ("# Existing\nKeep this line", "\n\n"),
            ("# Existing\nKeep this line\n", "\n"),
            ("# Existing\nKeep this line\n\n", ""),
        ] {
            let (outcome, merged) = merge_agent_instructions(Some(existing));
            assert_eq!(outcome, InstallOutcome::Appended);
            let merged = merged.unwrap();
            assert!(merged.starts_with(existing));
            assert_eq!(merged, format!("{existing}{separator}{SECTION}"));
        }
    }

    #[test]
    fn existing_heading_causes_no_change() {
        let existing = "# Existing\n\n## Shared Context\n\nCustom instructions\n";
        assert_eq!(
            merge_agent_instructions(Some(existing)),
            (InstallOutcome::AlreadyPresent, None)
        );
    }
}
