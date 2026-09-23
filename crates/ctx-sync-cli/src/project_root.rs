//! Detection of the project (worktree) root.

use std::path::{Path, PathBuf};

use ctx_sync_core::git::Git;

/// The top level of the git worktree containing `cwd`, or `cwd` itself when
/// it is not inside a git repository.
pub fn detect_project_root(cwd: &Path) -> PathBuf {
    match Git::new(cwd).run_line(&["rev-parse", "--show-toplevel"]) {
        Ok(top) if !top.is_empty() => PathBuf::from(top),
        _ => cwd.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(p: &Path) -> PathBuf {
        p.canonicalize().unwrap()
    }

    #[test]
    fn returns_repo_root_from_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        Git::new(dir.path()).run_checked(&["init", "-q"]).unwrap();
        let sub = dir.path().join("a/b");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(canonical(&detect_project_root(&sub)), canonical(dir.path()));
    }

    #[test]
    fn returns_cwd_outside_git() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_project_root(dir.path()), dir.path());
    }
}
