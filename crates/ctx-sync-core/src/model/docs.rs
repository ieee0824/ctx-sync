//! `10-project.md` and `20-architecture.md`.
//!
//! Both are free-form `MdDoc`s; ctx-sync only creates their initial content.

pub const PROJECT_FILE: &str = "10-project.md";
pub const ARCHITECTURE_FILE: &str = "20-architecture.md";

pub fn initial_project_md(project_name: &str) -> String {
    format!(
        "# Project

## Goal

TBD: describe the goal of {project_name} in 1-3 lines.

## Non Goals

- TBD

## Tech Stack

- TBD
"
    )
}

pub fn initial_architecture_md() -> String {
    "# Architecture

## Components

TBD

## Rules

- Each worker owns its own `40-worker-*.md` file. Do not edit other workers' files.
- Decisions are append-only. Add a new decision with `Supersedes:` instead of editing an old one.
- Shared files are synchronized with Git optimistic concurrency. Conflicts are never resolved automatically.
"
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MdDoc;

    #[test]
    fn project_template_matches_exactly() {
        assert_eq!(
            initial_project_md("demo"),
            "# Project\n\n## Goal\n\nTBD: describe the goal of demo in 1-3 lines.\n\n\
             ## Non Goals\n\n- TBD\n\n## Tech Stack\n\n- TBD\n"
        );
    }

    #[test]
    fn architecture_template_matches_exactly() {
        assert_eq!(
            initial_architecture_md(),
            "# Architecture\n\n## Components\n\nTBD\n\n## Rules\n\n\
             - Each worker owns its own `40-worker-*.md` file. Do not edit other workers' files.\n\
             - Decisions are append-only. Add a new decision with `Supersedes:` instead of editing an old one.\n\
             - Shared files are synchronized with Git optimistic concurrency. Conflicts are never resolved automatically.\n"
        );
    }

    #[test]
    fn templates_parse_as_documents() {
        let project = MdDoc::parse(&initial_project_md("demo")).unwrap();
        let headings: Vec<_> = project
            .sections
            .iter()
            .map(|s| s.heading.as_str())
            .collect();
        assert_eq!(project.title, "Project");
        assert_eq!(headings, ["Goal", "Non Goals", "Tech Stack"]);
        assert_eq!(project.render(), initial_project_md("demo"));

        let architecture = MdDoc::parse(&initial_architecture_md()).unwrap();
        let headings: Vec<_> = architecture
            .sections
            .iter()
            .map(|s| s.heading.as_str())
            .collect();
        assert_eq!(architecture.title, "Architecture");
        assert_eq!(headings, ["Components", "Rules"]);
        assert_eq!(architecture.render(), initial_architecture_md());
    }
}
