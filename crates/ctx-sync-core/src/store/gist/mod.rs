//! `ContextStore` backed by a GitHub Gist, operated with the `git` command.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};

use super::lock::{DEFAULT_LOCK_TIMEOUT, RepoLock};
use super::remote::RemoteSpec;
use super::{ContextStore, PullOutcome, SyncOutcome, SyncState};
use crate::git::Git;
use crate::model::ContextSnapshot;
use crate::state::ProjectState;
use crate::{Error, Result};

mod pull;
mod state;
mod sync;

/// Identity used for context commits when git has none configured
/// (common in fresh sandboxes).
const FALLBACK_NAME: &str = "user.name=ctx-sync";
const FALLBACK_EMAIL: &str = "user.email=ctx-sync@localhost";

pub struct GistStore {
    repo_dir: PathBuf,
    lock_path: PathBuf,
    conflict_path: PathBuf,
    remote: RemoteSpec,
    git_env: Vec<(String, String)>,
}

impl GistStore {
    pub fn new(state: &ProjectState, remote: RemoteSpec) -> Self {
        Self {
            repo_dir: state.context_repo(),
            lock_path: state.lock_path(),
            conflict_path: state.conflict_json(),
            remote,
            git_env: Vec::new(),
        }
    }

    /// Extra environment for every git child process (used by tests).
    pub fn with_git_env(mut self, env: Vec<(String, String)>) -> Self {
        self.git_env = env;
        self
    }

    pub fn remote(&self) -> &RemoteSpec {
        &self.remote
    }

    /// Current branch of the context repository (the Gist's default branch).
    pub fn branch(&self) -> Result<String> {
        self.git().run_line(&["symbolic-ref", "--short", "HEAD"])
    }

    pub fn conflict_path(&self) -> &Path {
        &self.conflict_path
    }

    pub(crate) fn git(&self) -> Git {
        Git::new(&self.repo_dir).with_env(&self.git_env)
    }

    pub(crate) fn lock(&self) -> Result<RepoLock> {
        RepoLock::acquire(&self.lock_path, DEFAULT_LOCK_TIMEOUT)
    }

    /// Clones the Gist when the context repository does not exist yet.
    /// The caller must hold the lock.
    pub(crate) fn ensure_unlocked(&self) -> Result<()> {
        if self.repo_dir.join(".git").exists() {
            return Ok(());
        }
        if self.repo_dir.exists() && std::fs::read_dir(&self.repo_dir)?.next().is_some() {
            return Err(Error::General(format!(
                "{} exists but is not a git repository",
                self.repo_dir.display()
            )));
        }
        let parent = self
            .repo_dir
            .parent()
            .ok_or_else(|| Error::General("invalid context repository path".into()))?;
        std::fs::create_dir_all(parent)?;
        let target = self.repo_dir.to_string_lossy();
        Git::new(parent).with_env(&self.git_env).run_checked(&[
            "clone",
            "-q",
            &self.remote.url(),
            &target,
        ])?;
        Ok(())
    }

    /// Commits every change. Returns the short hash, or `None` when there is
    /// nothing to commit. The caller must hold the lock.
    ///
    /// Git hooks are not skipped: a hook that rejects the commit is reported
    /// as an error.
    pub(crate) fn commit_unlocked(&self, message: &str) -> Result<Option<String>> {
        let git = self.git();
        git.run_checked(&["add", "-A"])?;
        if git.run(&["diff", "--cached", "--quiet"])?.success {
            return Ok(None);
        }
        let mut args = self.identity_args()?;
        args.extend(["commit", "-q", "-m", message]);
        git.run_checked(&args)?;
        git.run_line(&["rev-parse", "--short", "HEAD"]).map(Some)
    }

    /// `-c user.name=... -c user.email=...` for the parts git has no value for.
    pub(crate) fn identity_args(&self) -> Result<Vec<&'static str>> {
        let git = self.git();
        let mut args = Vec::new();
        for (key, fallback) in [("user.name", FALLBACK_NAME), ("user.email", FALLBACK_EMAIL)] {
            let out = git.run(&["config", key])?;
            if !out.success || out.stdout.trim().is_empty() {
                args.extend(["-c", fallback]);
            }
        }
        Ok(args)
    }

    fn check_name(name: &str) -> Result<()> {
        let invalid = name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name.starts_with('.')
            || name == "..";
        if invalid {
            Err(Error::General(format!(
                "invalid context file name: {name:?}"
            )))
        } else {
            Ok(())
        }
    }
}

impl ContextStore for GistStore {
    fn repo_dir(&self) -> &Path {
        &self.repo_dir
    }

    fn ensure(&self) -> Result<()> {
        let _lock = self.lock()?;
        self.ensure_unlocked()
    }

    fn snapshot(&self) -> Result<ContextSnapshot> {
        ContextSnapshot::load(&self.repo_dir)
    }

    fn read_file(&self, name: &str) -> Result<Option<String>> {
        Self::check_name(name)?;
        match std::fs::read_to_string(self.repo_dir.join(name)) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn write_file(&self, name: &str, content: &str) -> Result<()> {
        Self::check_name(name)?;
        std::fs::write(self.repo_dir.join(name), content)?;
        Ok(())
    }

    fn commit(&self, message: &str) -> Result<Option<String>> {
        let _lock = self.lock()?;
        self.commit_unlocked(message)
    }

    fn revision(&self) -> Result<Option<String>> {
        let out = self.git().run(&["rev-parse", "--short", "HEAD"])?;
        Ok(out.success.then(|| out.stdout.trim().to_string()))
    }

    fn pull(&self) -> Result<PullOutcome> {
        pull::pull(self)
    }

    fn sync(&self, now: DateTime<FixedOffset>) -> Result<SyncOutcome> {
        sync::sync(self, now)
    }

    fn sync_state(&self) -> Result<SyncState> {
        state::sync_state(self)
    }
}
