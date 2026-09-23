//! Core library of ctx-sync.
//!
//! ctx-sync synchronizes the shared development context (project goals,
//! architecture, decisions and worker state) between multiple AI coding
//! agents through a GitHub Gist used as a Git repository.

/// Version of ctx-sync.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
