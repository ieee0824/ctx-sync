//! Local state kept outside of the project directory.

mod index;
mod local;
mod paths;

pub use index::Index;
pub use local::{LocalProject, Protocol};
pub use paths::{HOME_ENV, ProjectState, StateRoot, worktree_key};
