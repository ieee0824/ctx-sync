//! Use cases behind the CLI commands.

mod attach;
mod register;
mod runtime;
mod workspace;

pub use attach::{AttachOptions, AttachOutcome, attach};
pub use register::{RegisterOutcome, register};
pub use runtime::Runtime;
pub use workspace::Workspace;
