//! Small file system helpers for local state files.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Error, Result};

/// Writes `contents` to a temporary file next to `path` and renames it into
/// place, creating parent directories as needed.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| Error::General(format!("invalid file path: {}", path.display())))?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

/// Reads a JSON file. Returns `Ok(None)` when it does not exist and
/// `InvalidConfig` when it is not valid JSON for `T`.
pub fn read_json_opt<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| Error::InvalidConfig(format!("invalid {}: {e}", path.display())))
}

/// Writes pretty JSON followed by a newline, atomically.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|e| Error::General(format!("cannot serialize {}: {e}", path.display())))?;
    text.push('\n');
    write_atomic(path, text.as_bytes())
}

/// Directory that is removed when dropped, on success and on every error
/// path. Used for temporary clones and files under the state root.
pub(crate) struct TempDirGuard(PathBuf);

impl TempDirGuard {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/file.txt");
        write_atomic(&path, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        write_atomic(&path, b"again").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "again");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("a/b"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec!["file.txt"]);
    }

    #[test]
    fn read_json_opt_of_missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let value: Option<Vec<u32>> = read_json_opt(&dir.path().join("nope.json")).unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn read_json_opt_of_broken_file_is_invalid_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.json");
        std::fs::write(&path, "{").unwrap();
        let err = read_json_opt::<Vec<u32>>(&path).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
        assert!(err.to_string().contains("broken.json"));
    }

    #[test]
    fn write_json_then_read_json_opt_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x/value.json");
        write_json(&path, &vec![1u32, 2, 3]).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().ends_with("]\n"));
        assert_eq!(
            read_json_opt::<Vec<u32>>(&path).unwrap(),
            Some(vec![1, 2, 3])
        );
    }
}
