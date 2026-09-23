//! Lock that keeps ctx-sync processes on the same machine from operating on
//! the same context repository at once.
//!
//! This is a local file lock, not a distributed lock: concurrency between
//! machines is handled by Git's optimistic concurrency.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::{Error, Result};

pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

const RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// Held lock. Released when dropped.
#[derive(Debug)]
pub struct RepoLock {
    _file: File,
}

impl RepoLock {
    /// Creates `path` (and its parent directories) and locks it, retrying
    /// every 100ms until `timeout` has passed.
    pub fn acquire(path: &Path, timeout: Duration) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        let deadline = Instant::now() + timeout;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(RETRY_INTERVAL);
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(Error::General(format!(
                        "context repository is locked by another ctx-sync process: {}",
                        path.display()
                    )));
                }
                Err(TryLockError::Error(e)) => return Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_times_out_while_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/context-repo.lock");
        let first = RepoLock::acquire(&path, Duration::from_millis(300)).unwrap();
        let started = Instant::now();
        let err = RepoLock::acquire(&path, Duration::from_millis(300)).unwrap_err();
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert!(
            err.to_string()
                .contains("locked by another ctx-sync process")
        );
        drop(first);
        RepoLock::acquire(&path, Duration::from_millis(300)).unwrap();
    }
}
