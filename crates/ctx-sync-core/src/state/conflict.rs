//! `conflict.json`: the last context conflict detected by `sync`.
//!
//! `status` shows it, and future `conflict show` / `conflict resolve`
//! commands will read it.

use std::io::ErrorKind;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::fs_util::{read_json_opt, write_json};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub detected_at: DateTime<FixedOffset>,
    /// Conflicting files (flat names inside the Gist).
    pub files: Vec<String>,
    /// Local HEAD before the rebase (full hash).
    pub local_head: String,
    /// `origin/<branch>` the rebase was onto (full hash).
    pub remote_head: String,
}

impl ConflictRecord {
    pub fn load(path: &Path) -> Result<Option<Self>> {
        read_json_opt(path)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_json(path, self)
    }

    /// Removes the record. Does nothing when there is none.
    pub fn clear(path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ConflictRecord {
        ConflictRecord {
            detected_at: DateTime::parse_from_rfc3339("2026-09-23T18:30:00+09:00").unwrap(),
            files: vec!["20-architecture.md".into()],
            local_head: "a".repeat(40),
            remote_head: "b".repeat(40),
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conflict.json");
        sample().save(&path).unwrap();
        assert_eq!(ConflictRecord::load(&path).unwrap(), Some(sample()));
    }

    #[test]
    fn clear_removes_the_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conflict.json");
        sample().save(&path).unwrap();
        ConflictRecord::clear(&path).unwrap();
        assert!(ConflictRecord::load(&path).unwrap().is_none());
    }

    #[test]
    fn clear_without_record_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        ConflictRecord::clear(&dir.path().join("conflict.json")).unwrap();
    }
}
