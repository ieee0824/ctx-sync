//! Location of the local state (outside of the project directory).
//!
//! ```text
//! <root>/
//!   index.json
//!   tmp/
//!   projects/<project-id>/
//!     context-repo/
//!     context-repo.lock
//!     local.json
//!     conflict.json
//!     workers/<worktree-key>.json
//! ```

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{Error, Result};

/// Environment variable that overrides the state root.
pub const HOME_ENV: &str = "CTX_SYNC_HOME";

/// Root directory of the local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRoot {
    root: PathBuf,
}

impl StateRoot {
    /// `resolve_from(std::env::var_os(HOME_ENV))`.
    pub fn resolve() -> Result<Self> {
        Self::resolve_from(std::env::var_os(HOME_ENV))
    }

    /// Uses `home` when it is set and not empty (relative paths are resolved
    /// against the current directory). Otherwise uses the OS state directory,
    /// falling back to the local data directory on platforms without one
    /// (macOS, Windows).
    pub fn resolve_from(home: Option<OsString>) -> Result<Self> {
        if let Some(home) = home.filter(|h| !h.is_empty()) {
            let path = PathBuf::from(home);
            let path = if path.is_absolute() {
                path
            } else {
                std::env::current_dir()?.join(path)
            };
            return Ok(Self::new(path));
        }
        let dirs = ProjectDirs::from("", "", "ctx-sync").ok_or_else(|| {
            Error::General("cannot determine state directory; set CTX_SYNC_HOME".into())
        })?;
        let dir = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());
        Ok(Self::new(dir))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn project(&self, project_id: &Uuid) -> ProjectState {
        ProjectState {
            dir: self
                .root
                .join("projects")
                .join(project_id.hyphenated().to_string()),
        }
    }
}

/// Local state of one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectState {
    dir: PathBuf,
}

impl ProjectState {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Clone of the context Gist.
    pub fn context_repo(&self) -> PathBuf {
        self.dir.join("context-repo")
    }

    pub fn local_json(&self) -> PathBuf {
        self.dir.join("local.json")
    }

    pub fn conflict_json(&self) -> PathBuf {
        self.dir.join("conflict.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.dir.join("context-repo.lock")
    }

    pub fn workers_dir(&self) -> PathBuf {
        self.dir.join("workers")
    }

    /// Worker identity of the given worktree.
    ///
    /// Identities are per worktree so that several agents running on the
    /// same machine do not share one identity.
    pub fn worker_json(&self, worktree_root: &Path) -> Result<PathBuf> {
        Ok(self
            .workers_dir()
            .join(format!("{}.json", worktree_key(worktree_root)?)))
    }
}

/// First 16 lowercase hex characters of the SHA-256 of the canonical path.
pub fn worktree_key(worktree_root: &Path) -> Result<String> {
    let canonical = worktree_root.canonicalize().map_err(|e| {
        Error::General(format!(
            "cannot resolve worktree path {}: {e}",
            worktree_root.display()
        ))
    })?;
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    Ok(digest.iter().take(8).map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_from_uses_given_home() {
        let root = StateRoot::resolve_from(Some("/tmp/x".into())).unwrap();
        assert_eq!(root.root(), Path::new("/tmp/x"));
    }

    #[test]
    fn resolve_from_makes_relative_home_absolute() {
        let root = StateRoot::resolve_from(Some("rel/home".into())).unwrap();
        assert!(root.root().is_absolute());
        assert!(root.root().ends_with("rel/home"));
    }

    #[test]
    fn resolve_from_falls_back_to_os_directory() {
        let root = StateRoot::resolve_from(None).unwrap();
        assert!(root.root().is_absolute());
        let empty = StateRoot::resolve_from(Some(OsString::new())).unwrap();
        assert_eq!(empty, root);
    }

    #[test]
    fn project_paths_are_under_projects_dir() {
        let root = StateRoot::new("/state");
        let id = Uuid::parse_str("6f1c2b7e-0000-4000-8000-000000000001").unwrap();
        let project = root.project(&id);
        let dir = Path::new("/state/projects/6f1c2b7e-0000-4000-8000-000000000001");
        assert_eq!(project.dir(), dir);
        assert_eq!(project.context_repo(), dir.join("context-repo"));
        assert_eq!(project.local_json(), dir.join("local.json"));
        assert_eq!(project.conflict_json(), dir.join("conflict.json"));
        assert_eq!(project.lock_path(), dir.join("context-repo.lock"));
        assert_eq!(project.workers_dir(), dir.join("workers"));
        assert_eq!(root.index_path(), Path::new("/state/index.json"));
        assert_eq!(root.tmp_dir(), Path::new("/state/tmp"));
    }

    #[test]
    fn worktree_key_is_stable_and_distinct() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let key_a = worktree_key(a.path()).unwrap();
        assert_eq!(key_a.len(), 16);
        assert!(
            key_a
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(worktree_key(a.path()).unwrap(), key_a);
        assert_eq!(worktree_key(&a.path().join(".")).unwrap(), key_a);
        assert_ne!(worktree_key(b.path()).unwrap(), key_a);
    }

    #[test]
    fn worker_json_is_keyed_by_worktree() {
        let worktree = tempfile::tempdir().unwrap();
        let project = StateRoot::new("/state").project(&Uuid::nil());
        let path = project.worker_json(worktree.path()).unwrap();
        let key = worktree_key(worktree.path()).unwrap();
        assert_eq!(path, project.workers_dir().join(format!("{key}.json")));
    }

    #[test]
    fn worktree_key_of_missing_path_is_an_error() {
        assert!(worktree_key(Path::new("/nonexistent/worktree")).is_err());
    }
}
