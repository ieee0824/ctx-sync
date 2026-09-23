//! Local state kept outside of the project directory.

mod index;
mod local;
mod paths;
mod worker;

pub use index::Index;
pub use local::{LocalProject, Protocol};
pub use paths::{HOME_ENV, ProjectState, StateRoot, worktree_key};
pub use worker::{WorkerIdentity, validate_worker_name};
