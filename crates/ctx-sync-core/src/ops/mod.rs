//! Use cases behind the CLI commands.

mod attach;
mod decision;
mod handoff;
mod init;
mod register;
mod runtime;
mod workspace;

pub use attach::{AttachOptions, AttachOutcome, attach};
pub use decision::{DecisionAddOutcome, NewDecision, add_decision};
pub use handoff::{HandoffInput, HandoffOutcome, apply_handoff, handoff};
pub use init::{InitOptions, InitOutcome, InitRemote, init};
pub use register::{RegisterOutcome, register};
pub use runtime::Runtime;
pub use workspace::Workspace;
