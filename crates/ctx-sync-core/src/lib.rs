//! Core library of ctx-sync.
//!
//! ctx-sync synchronizes the shared development context (project goals,
//! architecture, decisions and worker state) between multiple AI coding
//! agents through a GitHub Gist used as a Git repository.

pub mod clock;
pub mod config;
pub mod error;
pub mod fs_util;
pub mod gh;
pub mod gist_id;
pub mod git;
pub mod ids;
pub mod model;
pub mod state;
pub mod store;

pub use error::{Error, Result};

/// Version of ctx-sync.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
