//! Reading every file of the context repository at once.

use std::io::ErrorKind;
use std::path::Path;

use super::{
    ARCHITECTURE_FILE, DECISION_PREFIX, Decision, META_FILE, MdDoc, Meta, PROJECT_FILE,
    WORKER_PREFIX, Worker,
};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Meta,
    Project,
    Architecture,
    Decision,
    Worker,
    Other,
}

pub fn classify_file(name: &str) -> FileKind {
    let markdown_with = |prefix: &str| name.starts_with(prefix) && name.ends_with(".md");
    match name {
        META_FILE => FileKind::Meta,
        PROJECT_FILE => FileKind::Project,
        ARCHITECTURE_FILE => FileKind::Architecture,
        _ if markdown_with(DECISION_PREFIX) => FileKind::Decision,
        _ if markdown_with(WORKER_PREFIX) => FileKind::Worker,
        _ => FileKind::Other,
    }
}

/// Parsed contents of the context repository.
#[derive(Debug, Clone)]
pub struct ContextSnapshot {
    pub meta: Meta,
    pub project: MdDoc,
    pub architecture: MdDoc,
    /// Sorted by id.
    pub decisions: Vec<Decision>,
    /// Sorted by (name, id).
    pub workers: Vec<Worker>,
    /// Files that were missing or could not be parsed.
    pub warnings: Vec<String>,
}

impl ContextSnapshot {
    /// Reads the files directly under `dir`.
    ///
    /// Only a missing or invalid `00-meta.json` is an error. Broken project,
    /// architecture, decision or worker files are reported in `warnings` so
    /// that one bad file does not hide the rest of the context.
    pub fn load(dir: &Path) -> Result<Self> {
        let meta = match std::fs::read_to_string(dir.join(META_FILE)) {
            Ok(text) => Meta::parse(&text)?,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Err(Error::InvalidConfig(format!(
                    "{META_FILE} not found; this gist is not a ctx-sync context"
                )));
            }
            Err(e) => return Err(e.into()),
        };

        let mut warnings = Vec::new();
        let project = load_doc(dir, PROJECT_FILE, "Project", &mut warnings);
        let architecture = load_doc(dir, ARCHITECTURE_FILE, "Architecture", &mut warnings);

        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            if let Some(name) = entry.file_name().to_str()
                && !name.starts_with('.')
            {
                names.push(name.to_string());
            }
        }
        names.sort();

        let mut decisions = Vec::new();
        let mut workers = Vec::new();
        for name in names {
            let kind = classify_file(&name);
            if !matches!(kind, FileKind::Decision | FileKind::Worker) {
                continue;
            }
            let parsed = std::fs::read_to_string(dir.join(&name))
                .map_err(Error::from)
                .and_then(|text| match kind {
                    FileKind::Decision => Decision::parse(&name, &text).map(|d| decisions.push(d)),
                    _ => Worker::parse(&text).map(|w| workers.push(w)),
                });
            if let Err(e) = parsed {
                warnings.push(format!("skipped {name}: {e}"));
            }
        }
        decisions.sort_by(|a, b| a.id.cmp(&b.id));
        workers.sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id)));

        Ok(Self {
            meta,
            project,
            architecture,
            decisions,
            workers,
            warnings,
        })
    }
}

fn load_doc(dir: &Path, file: &str, title: &str, warnings: &mut Vec<String>) -> MdDoc {
    let empty = || MdDoc {
        title: title.to_string(),
        ..Default::default()
    };
    match std::fs::read_to_string(dir.join(file)) {
        Ok(text) => MdDoc::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{file} could not be parsed: {e}"));
            empty()
        }),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            warnings.push(format!("{file} not found"));
            empty()
        }
        Err(e) => {
            warnings.push(format!("cannot read {file}: {e}"));
            empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use uuid::Uuid;

    use super::*;
    use crate::model::docs::{initial_architecture_md, initial_project_md};
    use crate::model::{DecisionId, DecisionStatus};

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-09-23T18:00:00+09:00").unwrap()
    }

    fn decision(seq: u32, slug: &str) -> Decision {
        let date = NaiveDate::from_ymd_opt(2026, 9, 23).unwrap();
        Decision {
            id: DecisionId::new(date, seq, slug),
            title: slug.into(),
            status: DecisionStatus::Accepted,
            date,
            author: None,
            supersedes: vec![],
            context: String::new(),
            decision: slug.into(),
            reason: String::new(),
            consequences: String::new(),
            extra_sections: vec![],
        }
    }

    fn write(dir: &Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).unwrap();
    }

    fn full_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(d, META_FILE, &Meta::new("demo", now()).to_json());
        write(d, PROJECT_FILE, &initial_project_md("demo"));
        write(d, ARCHITECTURE_FILE, &initial_architecture_md());
        for decision in [decision(2, "b"), decision(1, "a")] {
            write(d, &decision.file_name(), &decision.render());
        }
        for name in ["zeta", "alpha"] {
            let worker = Worker::new(Uuid::new_v4(), name, now());
            write(d, &worker.file_name(), &worker.render());
        }
        dir
    }

    #[test]
    fn loads_every_file_in_order() {
        let dir = full_repo();
        let snapshot = ContextSnapshot::load(dir.path()).unwrap();
        assert_eq!(snapshot.meta.project_name, "demo");
        assert_eq!(snapshot.project.title, "Project");
        assert_eq!(snapshot.architecture.title, "Architecture");
        let ids: Vec<_> = snapshot.decisions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["20260923-001-a", "20260923-002-b"]);
        let names: Vec<_> = snapshot.workers.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zeta"]);
        assert!(snapshot.warnings.is_empty(), "{:?}", snapshot.warnings);
    }

    #[test]
    fn missing_meta_is_invalid_config() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "README.md", "# gist\n");
        let err = ContextSnapshot::load(dir.path()).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
        assert!(err.to_string().contains("00-meta.json not found"));
    }

    #[test]
    fn broken_worker_is_skipped_with_a_warning() {
        let dir = full_repo();
        write(
            dir.path(),
            "40-worker-deadbeef.md",
            "# Worker\n\nName: broken\n",
        );
        let snapshot = ContextSnapshot::load(dir.path()).unwrap();
        assert_eq!(snapshot.workers.len(), 2);
        assert_eq!(snapshot.warnings.len(), 1);
        assert!(snapshot.warnings[0].starts_with("skipped 40-worker-deadbeef.md:"));
    }

    #[test]
    fn broken_decision_is_skipped_with_a_warning() {
        let dir = full_repo();
        write(
            dir.path(),
            "30-decision-20260923-009-x.md",
            "# Decision: x\n",
        );
        let snapshot = ContextSnapshot::load(dir.path()).unwrap();
        assert_eq!(snapshot.decisions.len(), 2);
        assert_eq!(snapshot.warnings.len(), 1);
    }

    #[test]
    fn missing_project_and_architecture_are_warnings() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), META_FILE, &Meta::new("demo", now()).to_json());
        let snapshot = ContextSnapshot::load(dir.path()).unwrap();
        assert_eq!(snapshot.project.title, "Project");
        assert!(snapshot.project.sections.is_empty());
        assert_eq!(snapshot.architecture.title, "Architecture");
        assert_eq!(snapshot.warnings.len(), 2, "{:?}", snapshot.warnings);
    }

    #[test]
    fn ignores_other_files_and_directories() {
        let dir = full_repo();
        write(dir.path(), "README.md", "# gist\n");
        write(dir.path(), ".hidden-40-worker-x.md", "junk");
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::create_dir(dir.path().join("40-worker-dir.md")).unwrap();
        let snapshot = ContextSnapshot::load(dir.path()).unwrap();
        assert_eq!(snapshot.workers.len(), 2);
        assert_eq!(snapshot.decisions.len(), 2);
        assert!(snapshot.warnings.is_empty(), "{:?}", snapshot.warnings);
    }

    #[test]
    fn classifies_file_names() {
        let cases = [
            ("00-meta.json", FileKind::Meta),
            ("10-project.md", FileKind::Project),
            ("20-architecture.md", FileKind::Architecture),
            ("30-decision-20260923-001-a.md", FileKind::Decision),
            ("40-worker-a1b2c3d4.md", FileKind::Worker),
            ("40-worker-a1b2c3d4.txt", FileKind::Other),
            ("README.md", FileKind::Other),
        ];
        for (name, kind) in cases {
            assert_eq!(classify_file(name), kind, "{name}");
        }
    }
}
