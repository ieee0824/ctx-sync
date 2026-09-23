//! Project configuration file (`.ctx-sync.toml`).
//!
//! The file is committed to the project repository. It must never hold
//! credentials, so the schema is fixed and unknown keys are rejected.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const CONFIG_FILE: &str = ".ctx-sync.toml";
pub const CONFIG_VERSION: u32 = 1;

const DEFAULT_PROJECT_FILE: &str = "10-project.md";
const DEFAULT_ARCHITECTURE_FILE: &str = "20-architecture.md";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub version: u32,
    pub remote: RemoteConfig,
    #[serde(default)]
    pub context: ContextFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteConfig {
    #[serde(rename = "type")]
    pub kind: RemoteKind,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemoteKind {
    Gist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFiles {
    pub project: String,
    pub architecture: String,
}

impl Default for ContextFiles {
    fn default() -> Self {
        Self {
            project: DEFAULT_PROJECT_FILE.into(),
            architecture: DEFAULT_ARCHITECTURE_FILE.into(),
        }
    }
}

impl ProjectConfig {
    pub fn new_gist(gist_id: impl Into<String>) -> Self {
        Self {
            version: CONFIG_VERSION,
            remote: RemoteConfig {
                kind: RemoteKind::Gist,
                id: gist_id.into(),
            },
            context: ContextFiles::default(),
        }
    }

    /// Reads and validates the configuration. Every failure is `InvalidConfig`.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::InvalidConfig(format!("cannot read {}: {e}", path.display())))?;
        let config: Self = toml::from_str(&text)
            .map_err(|e| Error::InvalidConfig(format!("invalid {}: {e}", path.display())))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self)
            .map_err(|e| Error::General(format!("cannot serialize {CONFIG_FILE}: {e}")))?;
        std::fs::write(path, text)?;
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.version != CONFIG_VERSION {
            return Err(Error::InvalidConfig(format!(
                "unsupported {CONFIG_FILE} version {} (expected {CONFIG_VERSION})",
                self.version
            )));
        }
        if self.remote.id.trim().is_empty() {
            return Err(Error::InvalidConfig(format!(
                "remote.id in {CONFIG_FILE} is empty"
            )));
        }
        if self.context != ContextFiles::default() {
            return Err(Error::InvalidConfig(
                "custom context file names are not supported in v0.1".into(),
            ));
        }
        Ok(())
    }
}

/// Walks up from `start` and returns the directory containing `.ctx-sync.toml`.
pub fn find_project_root(start: &Path) -> Result<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(CONFIG_FILE).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            Error::InvalidConfig(format!(
                "no {CONFIG_FILE} found; run `ctx-sync init` or `ctx-sync attach <gist>`"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: &str = r#"version = 1

[remote]
type = "gist"
id = "0123456789abcdef"

[context]
project = "10-project.md"
architecture = "20-architecture.md"
"#;

    const MINIMAL: &str = "version = 1\n\n[remote]\ntype = \"gist\"\nid = \"abc\"\n";

    fn write(dir: &Path, text: &str) -> PathBuf {
        let path = dir.join(CONFIG_FILE);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn assert_invalid(text: &str) {
        let dir = tempfile::tempdir().unwrap();
        let err = ProjectConfig::load(&write(dir.path(), text)).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{text}: {err:?}");
    }

    #[test]
    fn save_writes_the_documented_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        ProjectConfig::new_gist("0123456789abcdef")
            .save(&path)
            .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), EXPECTED);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        let config = ProjectConfig::new_gist("abc");
        config.save(&path).unwrap();
        assert_eq!(ProjectConfig::load(&path).unwrap(), config);
    }

    #[test]
    fn context_section_is_optional() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig::load(&write(dir.path(), MINIMAL)).unwrap();
        assert_eq!(config.context, ContextFiles::default());
    }

    #[test]
    fn rejects_unknown_keys() {
        assert_invalid(&format!("{MINIMAL}token = \"x\"\n"));
        assert_invalid(&format!("token = \"x\"\n{MINIMAL}"));
    }

    #[test]
    fn rejects_unsupported_version() {
        assert_invalid(&MINIMAL.replace("version = 1", "version = 2"));
    }

    #[test]
    fn rejects_missing_remote() {
        assert_invalid("version = 1\n");
    }

    #[test]
    fn rejects_empty_remote_id() {
        assert_invalid(&MINIMAL.replace("\"abc\"", "\"\""));
    }

    #[test]
    fn rejects_unknown_remote_type() {
        assert_invalid(&MINIMAL.replace("\"gist\"", "\"s3\""));
    }

    #[test]
    fn rejects_custom_context_files() {
        assert_invalid(&format!(
            "{MINIMAL}\n[context]\nproject = \"p.md\"\narchitecture = \"20-architecture.md\"\n"
        ));
    }

    #[test]
    fn rejects_invalid_toml_and_missing_file() {
        assert_invalid("version = ");
        let err = ProjectConfig::load(Path::new("/nonexistent/.ctx-sync.toml")).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
    }

    #[test]
    fn find_project_root_walks_up() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), EXPECTED);
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_project_root(&nested).unwrap(), dir.path());
    }

    #[test]
    fn find_project_root_reports_missing_config() {
        let dir = tempfile::tempdir().unwrap();
        let err = find_project_root(dir.path()).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
        assert!(err.to_string().contains("ctx-sync init"));
    }
}
