//! Use cases behind the CLI commands.

mod attach;
mod handoff;
mod register;
mod runtime;
mod workspace;

pub use attach::{AttachOptions, AttachOutcome, attach};
pub use handoff::{HandoffInput, HandoffOutcome, apply_handoff, handoff};
pub use register::{RegisterOutcome, register};
pub use runtime::Runtime;
pub use workspace::Workspace;
