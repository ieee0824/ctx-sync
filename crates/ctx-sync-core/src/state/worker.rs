//! Worker identity of this machine and worktree.
//!
//! The identity lives in the local state and is never committed to the
//! project repository.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::fs_util::{read_json_opt, write_json};
use crate::ids::short_id;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerIdentity {
    /// Full UUID. Only the short form is shown to users.
    pub id: Uuid,
    pub name: String,
    pub registered_at: DateTime<FixedOffset>,
    pub worktree: PathBuf,
}

impl WorkerIdentity {
    pub fn new(name: &str, worktree: &Path, now: DateTime<FixedOffset>) -> Result<Self> {
        validate_worker_name(name)?;
        Ok(Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            registered_at: now,
            worktree: worktree.to_path_buf(),
        })
    }

    pub fn short_id(&self) -> String {
        short_id(&self.id)
    }

    pub fn load(path: &Path) -> Result<Option<Self>> {
        read_json_opt(path)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_json(path, self)
    }
}

/// 1 to 64 characters of `[A-Za-z0-9._-]`, starting with a letter or digit.
pub fn validate_worker_name(name: &str) -> Result<()> {
    let valid_chars = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    let starts_alnum = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric());
    if (1..=64).contains(&name.len()) && valid_chars && starts_alnum {
        Ok(())
    } else {
        Err(Error::InvalidConfig(format!(
            "invalid worker name {name:?}: use 1-64 characters of [A-Za-z0-9._-] starting with a letter or digit"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-09-23T18:00:00+09:00").unwrap()
    }

    #[test]
    fn accepts_valid_names() {
        for name in ["parser", "sandbox-1", "a.b_c", "A", &"x".repeat(64)] {
            assert!(validate_worker_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_invalid_names() {
        for name in ["", "-x", ".x", "a b", &"x".repeat(65), "日本語", "a/b"] {
            let err = validate_worker_name(name).unwrap_err();
            assert_eq!(err.exit_code(), 4, "{name}");
        }
    }

    #[test]
    fn new_validates_the_name() {
        assert!(WorkerIdentity::new("a b", Path::new("/w"), now()).is_err());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workers/key.json");
        let identity = WorkerIdentity::new("parser", Path::new("/w"), now()).unwrap();
        identity.save(&path).unwrap();
        assert_eq!(WorkerIdentity::load(&path).unwrap(), Some(identity));
        assert!(
            WorkerIdentity::load(&dir.path().join("none.json"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn short_id_has_eight_characters() {
        let identity = WorkerIdentity::new("parser", Path::new("/w"), now()).unwrap();
        assert_eq!(identity.short_id().len(), 8);
        assert!(
            identity
                .id
                .simple()
                .to_string()
                .starts_with(&identity.short_id())
        );
    }
}
