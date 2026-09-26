//! Use cases behind the CLI commands.

mod agent;
mod agent_instructions;
mod attach;
mod bootstrap;
mod claim;
mod conflict;
mod conflict_resolve;
mod decision;
mod handoff;
mod init;
mod prune;
mod register;
mod runtime;
mod workspace;

pub use agent::{AgentStartOptions, AgentStartOutcome, agent_finish, agent_start};
pub use agent_instructions::{
    AGENT_INSTRUCTIONS_HEADING, InstallOutcome, agent_instructions_section,
    install_agent_instructions, merge_agent_instructions,
};
pub use attach::{AttachOptions, AttachOutcome, attach};
pub use bootstrap::{BootstrapApplyOutcome, apply_bootstrap};
pub use claim::{
    ClaimConflict, ClaimInput, ClaimOutcome, changed_file_conflicts, claim, claim_conflicts,
    conflict_warning,
};
pub use conflict::conflict_show;
pub use conflict_resolve::{Resolution, ResolveOutcome, conflict_resolve};
pub use decision::{DecisionAddOutcome, NewDecision, add_decision, supersede_decision};
pub use handoff::{HandoffInput, HandoffOutcome, apply_handoff, handoff};
pub use init::{InitOptions, InitOutcome, InitRemote, init};
pub use prune::{PruneOptions, PruneOutcome, PruneReason, PrunedWorker, prune_workers};
pub use register::{RegisterOutcome, register};
pub use runtime::Runtime;
pub use workspace::Workspace;
