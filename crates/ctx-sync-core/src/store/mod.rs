//! Storage of the shared context.
//!
//! v0.1 has a single backend, [`GistStore`]. The trait keeps the commands
//! independent of it so that other Git-based stores can be added later.

use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use crate::Result;
use crate::model::ContextSnapshot;
use crate::state::ConflictRecord;

pub mod gist;
pub mod lock;
pub mod remote;

pub use gist::GistStore;
pub use lock::{DEFAULT_LOCK_TIMEOUT, RepoLock};
pub use remote::{REMOTE_URL_OVERRIDE_ENV, RemoteSpec};

pub trait ContextStore {
    fn repo_dir(&self) -> &Path;
    /// Clones the context repository when it does not exist yet.
    fn ensure(&self) -> Result<()>;
    fn snapshot(&self) -> Result<ContextSnapshot>;
    fn read_file(&self, name: &str) -> Result<Option<String>>;
    fn write_file(&self, name: &str, content: &str) -> Result<()>;
    /// Commits every change and returns the short hash, or `None` when there
    /// was nothing to commit.
    fn commit(&self, message: &str) -> Result<Option<String>>;
    /// Short hash of HEAD, or `None` when there is no commit.
    fn revision(&self) -> Result<Option<String>>;
    fn pull(&self) -> Result<PullOutcome>;
    fn sync(&self, now: DateTime<FixedOffset>) -> Result<SyncOutcome>;
    /// Current state without touching the network.
    fn sync_state(&self) -> Result<SyncState>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PullOutcome {
    UpToDate,
    FastForwarded { from: String, to: String },
    Diverged { ahead: u32, behind: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncOutcome {
    pub committed: Option<String>,
    pub pushed: bool,
    pub revision: String,
    pub attempts: u32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyncState {
    pub branch: String,
    pub revision: Option<String>,
    /// Compared with `origin/<branch>` as of the last fetch.
    pub ahead: u32,
    pub behind: u32,
    /// There are uncommitted changes.
    pub dirty: bool,
    pub conflict: Option<ConflictRecord>,
}
