//! Storage of the shared context.

pub mod lock;
pub mod remote;

pub use lock::{DEFAULT_LOCK_TIMEOUT, RepoLock};
pub use remote::{REMOTE_URL_OVERRIDE_ENV, RemoteSpec};
