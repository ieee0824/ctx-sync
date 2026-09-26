//! `00-meta.json`: identity and schema version of the context Gist.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

pub const META_FILE: &str = "00-meta.json";
/// Schema version written by this ctx-sync. Used for future migrations.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub schema_version: u32,
    pub project_id: Uuid,
    pub project_name: String,
    pub created_at: DateTime<FixedOffset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx_sync_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture_file: Option<String>,
}

impl Meta {
    pub fn new(project_name: &str, now: DateTime<FixedOffset>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project_id: Uuid::new_v4(),
            project_name: project_name.to_string(),
            created_at: now,
            ctx_sync_version: Some(crate::VERSION.to_string()),
            project_file: None,
            architecture_file: None,
        }
    }

    /// Parses and validates `00-meta.json`. Every failure is `InvalidConfig`.
    ///
    /// Unknown fields are allowed so that newer minor additions do not break
    /// older readers.
    pub fn parse(json: &str) -> Result<Self> {
        let invalid = |msg: String| Error::InvalidConfig(format!("invalid {META_FILE}: {msg}"));
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        // Check the version first: a newer schema may not deserialize at all.
        match value.get("schema_version").and_then(|v| v.as_u64()) {
            Some(0) => return Err(invalid("schema_version must be at least 1".into())),
            Some(v) if v > u64::from(SCHEMA_VERSION) => {
                return Err(Error::InvalidConfig(format!(
                    "{META_FILE} has schema_version {v}, which requires a newer ctx-sync \
                     (this version supports {SCHEMA_VERSION})"
                )));
            }
            _ => {}
        }
        let meta: Meta = serde_json::from_value(value).map_err(|e| invalid(e.to_string()))?;
        if meta.project_name.trim().is_empty() {
            return Err(invalid("project_name is empty".into()));
        }
        Ok(meta)
    }

    /// Pretty JSON followed by a newline.
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).expect("Meta is always serializable");
        json.push('\n');
        json
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
  "schema_version": 1,
  "project_id": "6f1c2b7e-0000-4000-8000-000000000001",
  "project_name": "ctx-sync",
  "created_at": "2026-09-23T18:00:00+09:00",
  "ctx_sync_version": "0.1.0"
}"#;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-09-23T18:00:00+09:00").unwrap()
    }

    fn assert_invalid(json: &str) -> Error {
        let err = Meta::parse(json).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{json}: {err:?}");
        err
    }

    #[test]
    fn parses_the_documented_example() {
        let meta = Meta::parse(SAMPLE).unwrap();
        assert_eq!(meta.schema_version, 1);
        assert_eq!(meta.project_name, "ctx-sync");
        assert_eq!(
            meta.project_id.to_string(),
            "6f1c2b7e-0000-4000-8000-000000000001"
        );
        assert_eq!(meta.created_at, now());
        assert_eq!(meta.ctx_sync_version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn ctx_sync_version_is_optional() {
        let json = SAMPLE.replace(",\n  \"ctx_sync_version\": \"0.1.0\"", "");
        assert!(Meta::parse(&json).unwrap().ctx_sync_version.is_none());
    }

    #[test]
    fn unknown_fields_are_allowed() {
        let json = SAMPLE.replace(
            "\"schema_version\": 1,",
            "\"schema_version\": 1, \"extra\": 1,",
        );
        assert!(Meta::parse(&json).is_ok());
    }

    #[test]
    fn rejects_missing_required_fields() {
        for field in ["project_id", "project_name", "created_at", "schema_version"] {
            let mut value: serde_json::Value = serde_json::from_str(SAMPLE).unwrap();
            value.as_object_mut().unwrap().remove(field);
            assert_invalid(&value.to_string());
        }
    }

    #[test]
    fn rejects_newer_schema() {
        let err = assert_invalid(&SAMPLE.replace("\"schema_version\": 1", "\"schema_version\": 2"));
        assert!(
            err.to_string().contains("requires a newer ctx-sync"),
            "{err}"
        );
    }

    #[test]
    fn rejects_schema_zero_empty_name_and_broken_json() {
        assert_invalid(&SAMPLE.replace("\"schema_version\": 1", "\"schema_version\": 0"));
        assert_invalid(&SAMPLE.replace("\"ctx-sync\"", "\" \""));
        assert_invalid("{");
    }

    #[test]
    fn new_then_to_json_then_parse_round_trips() {
        let meta = Meta::new("demo", now());
        assert_eq!(meta.schema_version, SCHEMA_VERSION);
        assert_eq!(meta.ctx_sync_version.as_deref(), Some(crate::VERSION));
        let json = meta.to_json();
        assert!(json.ends_with("}\n"));
        assert_eq!(Meta::parse(&json).unwrap(), meta);
    }
}
