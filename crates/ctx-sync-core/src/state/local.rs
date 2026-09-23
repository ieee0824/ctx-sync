//! `local.json`: how this machine connects to a project's Gist.

use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::fs_util::{read_json_opt, write_json};

/// Protocol used to clone and push the Gist.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Https,
    Ssh,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalProject {
    pub gist_id: String,
    pub protocol: Protocol,
    pub attached_at: DateTime<FixedOffset>,
}

impl LocalProject {
    pub fn load(path: &Path) -> Result<Option<Self>> {
        read_json_opt(path)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_json(path, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(protocol: Protocol) -> LocalProject {
        LocalProject {
            gist_id: "abc".into(),
            protocol,
            attached_at: DateTime::parse_from_rfc3339("2026-09-23T18:00:00+09:00").unwrap(),
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("local.json");
        let local = sample(Protocol::Ssh);
        local.save(&path).unwrap();
        assert_eq!(LocalProject::load(&path).unwrap(), Some(local));
    }

    #[test]
    fn missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            LocalProject::load(&dir.path().join("local.json"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn protocol_is_lowercase_in_json() {
        let https = serde_json::to_value(sample(Protocol::Https)).unwrap();
        let ssh = serde_json::to_value(sample(Protocol::Ssh)).unwrap();
        assert_eq!(https["protocol"], "https");
        assert_eq!(ssh["protocol"], "ssh");
        assert_eq!(Protocol::default(), Protocol::Https);
    }
}
