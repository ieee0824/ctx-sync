//! Worker files (`40-worker-<short-id>.md`).
//!
//! Each worker owns exactly one file and never edits another worker's file,
//! which keeps concurrent updates free of Git conflicts. The file describes
//! the worker's *current* state; it is not a log.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::md::{MdDoc, MdSection, parse_fields, parse_list, render_fields, render_list};
use crate::ids::short_id;
use crate::{Error, Result};

pub const WORKER_PREFIX: &str = "40-worker-";

const REPOSITORY: &str = "Repository";
const TASK: &str = "Task";
const WORKING_ON: &str = "Working On";
const CLAIMS: &str = "Claims";
const CHANGED: &str = "Changed";
const INTERFACE_CHANGES: &str = "Interface Changes";
const ATTENTION: &str = "Attention";
const BLOCKED_BY: &str = "Blocked By";
const SUMMARY: &str = "Summary";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerStatus {
    Working,
    Blocked,
    Done,
    Abandoned,
}

impl WorkerStatus {
    /// Working or blocked.
    pub fn is_active(&self) -> bool {
        matches!(self, WorkerStatus::Working | WorkerStatus::Blocked)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WorkerStatus::Working => "working",
            WorkerStatus::Blocked => "blocked",
            WorkerStatus::Done => "done",
            WorkerStatus::Abandoned => "abandoned",
        }
    }
}

impl fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WorkerStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "working" => Ok(WorkerStatus::Working),
            "blocked" => Ok(WorkerStatus::Blocked),
            "done" => Ok(WorkerStatus::Done),
            "abandoned" => Ok(WorkerStatus::Abandoned),
            _ => Err(Error::General(format!("invalid worker status: {s:?}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub status: WorkerStatus,
    pub last_updated: DateTime<FixedOffset>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub task: String,
    pub working_on: Vec<String>,
    pub claims: Vec<String>,
    pub changed: Vec<String>,
    pub interface_changes: Vec<String>,
    pub attention: Vec<String>,
    pub blocked_by: Vec<String>,
    pub summary: String,
    /// Unknown sections, kept when the file is written back.
    pub extra_sections: Vec<MdSection>,
}

impl Worker {
    pub fn new(id: Uuid, name: &str, now: DateTime<FixedOffset>) -> Self {
        Self {
            id,
            name: name.to_string(),
            status: WorkerStatus::Working,
            last_updated: now,
            branch: None,
            commit: None,
            task: String::new(),
            working_on: Vec::new(),
            claims: Vec::new(),
            changed: Vec::new(),
            interface_changes: Vec::new(),
            attention: Vec::new(),
            blocked_by: Vec::new(),
            summary: String::new(),
            extra_sections: Vec::new(),
        }
    }

    /// `40-worker-<short id>.md`.
    pub fn file_name_for(id: &Uuid) -> String {
        format!("{WORKER_PREFIX}{}.md", short_id(id))
    }

    pub fn file_name(&self) -> String {
        Self::file_name_for(&self.id)
    }

    pub fn parse(content: &str) -> Result<Self> {
        let doc = MdDoc::parse(content)?;
        let required = |key: &str| {
            doc.field(key)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| Error::General(format!("missing {key}")))
        };
        let id = required("ID")?;
        let id =
            Uuid::parse_str(id).map_err(|e| Error::General(format!("invalid ID {id:?}: {e}")))?;
        let name = required("Name")?.to_string();
        let status = required("Status")?.parse()?;
        let last_updated = required("Last Updated")?;
        let last_updated = DateTime::parse_from_rfc3339(last_updated)
            .map_err(|e| Error::General(format!("invalid Last Updated {last_updated:?}: {e}")))?;

        let mut worker = Worker {
            status,
            last_updated,
            ..Worker::new(id, &name, last_updated)
        };
        for section in doc.sections {
            let body = section.body.as_str();
            match section.heading.as_str() {
                REPOSITORY => {
                    for (key, value) in parse_fields(body) {
                        let value = Some(value.trim().to_string()).filter(|v| !v.is_empty());
                        match key.as_str() {
                            "Branch" => worker.branch = value,
                            "Commit" => worker.commit = value,
                            _ => {}
                        }
                    }
                }
                TASK => worker.task = section.body,
                WORKING_ON => worker.working_on = parse_list(body),
                CLAIMS => worker.claims = parse_list(body),
                CHANGED => worker.changed = parse_list(body),
                INTERFACE_CHANGES => worker.interface_changes = parse_list(body),
                ATTENTION => worker.attention = parse_list(body),
                BLOCKED_BY => worker.blocked_by = parse_list(body),
                SUMMARY => worker.summary = section.body,
                _ => worker.extra_sections.push(section),
            }
        }
        Ok(worker)
    }

    /// Every section is always written, in a fixed order, so that the
    /// output is predictable.
    pub fn render(&self) -> String {
        let section = |heading: &str, body: String| MdSection {
            heading: heading.to_string(),
            body,
        };
        let repository = render_fields(&[
            ("Branch".into(), self.branch.clone().unwrap_or_default()),
            ("Commit".into(), self.commit.clone().unwrap_or_default()),
        ]);
        let mut sections = vec![
            section(REPOSITORY, repository),
            section(TASK, self.task.clone()),
            section(WORKING_ON, render_list(&self.working_on)),
            section(CLAIMS, render_list(&self.claims)),
            section(CHANGED, render_list(&self.changed)),
            section(INTERFACE_CHANGES, render_list(&self.interface_changes)),
            section(ATTENTION, render_list(&self.attention)),
            section(BLOCKED_BY, render_list(&self.blocked_by)),
            section(SUMMARY, self.summary.clone()),
        ];
        sections.extend(self.extra_sections.iter().cloned());
        MdDoc {
            title: "Worker".into(),
            fields: vec![
                ("ID".into(), self.id.hyphenated().to_string()),
                ("Name".into(), self.name.clone()),
                ("Status".into(), self.status.to_string()),
                ("Last Updated".into(), self.last_updated.to_rfc3339()),
            ],
            preamble: String::new(),
            sections,
        }
        .render()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "6f1c2b7e-0000-4000-8000-000000000001";

    fn time(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }

    fn full() -> Worker {
        Worker {
            id: Uuid::parse_str(ID).unwrap(),
            name: "parser".into(),
            status: WorkerStatus::Blocked,
            last_updated: time("2026-09-23T18:20:00+09:00"),
            branch: Some("feat/parser".into()),
            commit: Some("abc1234".into()),
            task: "Parser を実装する。".into(),
            working_on: vec!["parser state machine".into(), "error handling".into()],
            claims: vec!["src/parser/**".into(), "crates/*/Cargo.toml".into()],
            changed: vec!["src/parser.rs".into(), "src/error.rs".into()],
            interface_changes: vec!["ParserResult に warnings を追加".into()],
            attention: vec!["UI 側で ParserResult の変更への対応が必要".into()],
            blocked_by: vec!["waiting for API review".into()],
            summary: "Parser を半分実装した。".into(),
            extra_sections: vec![],
        }
    }

    #[test]
    fn renders_the_documented_format() {
        let text = full().render();
        let expected_head = format!(
            "# Worker\n\nID: {ID}\nName: parser\nStatus: blocked\n\
             Last Updated: 2026-09-23T18:20:00+09:00\n\n\
             ## Repository\n\nBranch: feat/parser\nCommit: abc1234\n\n\
             ## Task\n\nParser を実装する。\n\n\
             ## Working On\n\n- parser state machine\n- error handling\n\n\
             ## Claims\n\n- src/parser/**\n- crates/*/Cargo.toml\n\n\
             ## Changed\n\n- src/parser.rs\n- src/error.rs\n\n"
        );
        assert!(text.starts_with(&expected_head), "{text}");
        assert!(
            text.ends_with("## Summary\n\nParser を半分実装した。\n"),
            "{text}"
        );
    }

    #[test]
    fn full_worker_round_trips() {
        let worker = full();
        let text = worker.render();
        let parsed = Worker::parse(&text).unwrap();
        assert_eq!(parsed, worker);
        assert_eq!(parsed.render(), text);
    }

    #[test]
    fn empty_worker_round_trips_with_all_sections() {
        let worker = Worker::new(Uuid::new_v4(), "gist", time("2026-09-23T18:00:00+09:00"));
        let text = worker.render();
        for heading in [
            "## Repository",
            "## Task",
            "## Working On",
            "## Claims",
            "## Changed",
            "## Interface Changes",
            "## Attention",
            "## Blocked By",
            "## Summary",
        ] {
            assert!(text.contains(heading), "{heading} missing:\n{text}");
        }
        assert_eq!(Worker::parse(&text).unwrap(), worker);
    }

    #[test]
    fn parses_v01_worker_without_claims_section() {
        let mut old = full();
        old.claims.clear();
        let old_format = old.render().replace("## Claims\n\n\n", "");
        let parsed = Worker::parse(&old_format).unwrap();
        assert!(parsed.claims.is_empty());
        assert!(parsed.render().contains("## Claims\n"));
    }

    #[test]
    fn keeps_unknown_sections() {
        let mut worker = full();
        worker.extra_sections.push(MdSection {
            heading: "Notes".into(),
            body: "extra".into(),
        });
        assert_eq!(Worker::parse(&worker.render()).unwrap(), worker);
    }

    #[test]
    fn plain_text_interface_changes_become_one_item() {
        let mut text = full().render();
        text = text.replace(
            "## Interface Changes\n\n- ParserResult に warnings を追加",
            "## Interface Changes\n\nParserResultにwarningsを追加。",
        );
        let worker = Worker::parse(&text).unwrap();
        assert_eq!(
            worker.interface_changes,
            vec!["ParserResultにwarningsを追加。"]
        );
    }

    #[test]
    fn file_name_uses_the_short_id() {
        let id = Uuid::parse_str(ID).unwrap();
        assert_eq!(Worker::file_name_for(&id), "40-worker-6f1c2b7e.md");
        assert_eq!(full().file_name(), "40-worker-6f1c2b7e.md");
    }

    #[test]
    fn invalid_or_missing_fields_are_errors() {
        let text = full().render();
        assert!(Worker::parse(&text.replace(ID, "a1b2c3")).is_err());
        assert!(Worker::parse(&text.replace("Name: parser\n", "")).is_err());
        assert!(Worker::parse(&text.replace("Status: blocked", "Status: sleeping")).is_err());
        assert!(Worker::parse(&text.replace("2026-09-23T18:20:00+09:00", "yesterday")).is_err());
    }

    #[test]
    fn status_parsing_and_activity() {
        assert_eq!(
            "Blocked".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Blocked
        );
        assert_eq!(WorkerStatus::Abandoned.to_string(), "abandoned");
        assert!(WorkerStatus::Working.is_active());
        assert!(WorkerStatus::Blocked.is_active());
        assert!(!WorkerStatus::Done.is_active());
        assert!(!WorkerStatus::Abandoned.is_active());
    }
}
