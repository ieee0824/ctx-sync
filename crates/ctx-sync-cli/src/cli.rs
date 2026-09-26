//! Command line definition.

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "ctx-sync",
    version,
    about = "Sync shared project context between AI coding agents"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new ctx-sync project
    Init(InitArgs),
    /// Attach this project to an existing ctx-sync gist
    Attach(AttachArgs),
    /// Collect initial context candidates from an existing repository
    Bootstrap(BootstrapArgs),
    /// Register this sandbox as a worker
    Register(RegisterArgs),
    /// Fetch the latest shared context
    Pull,
    /// Commit, rebase and push the shared context
    Sync,
    /// Print the full shared context
    Context(ContextArgs),
    /// Print onboarding context for a newly joining agent
    Onboard,
    /// Show project, sync and worker status
    Status,
    /// Update this worker's shared state
    Handoff(HandoffArgs),
    /// Manage decisions
    #[command(subcommand)]
    Decision(DecisionCommand),
    /// Mark this worker as done
    Done(DoneArgs),
    /// High level commands for AI agents
    #[command(subcommand)]
    Agent(AgentCommand),
    /// Add ctx-sync instructions to AGENTS.md
    InstallAgentInstructions,
}

#[derive(Args)]
pub struct InitArgs {
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, conflicts_with = "gist_id")]
    pub create_gist: bool,
    #[arg(long)]
    pub gist_id: Option<String>,
    #[arg(long, conflicts_with = "public")]
    pub secret: bool,
    #[arg(long)]
    pub public: bool,
    #[arg(long)]
    pub ssh: bool,
    #[arg(long)]
    pub no_input: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct AttachArgs {
    /// Gist ID or URL
    pub gist: String,
    #[arg(long)]
    pub ssh: bool,
}

#[derive(Args)]
pub struct BootstrapArgs {
    #[arg(long)]
    pub apply: bool,
}

#[derive(Args)]
pub struct RegisterArgs {
    pub name: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct ContextArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Default)]
pub struct HandoffFields {
    #[arg(long)]
    pub task: Option<String>,
    #[arg(long)]
    pub summary: Option<String>,
    #[arg(long, value_enum)]
    pub status: Option<WorkerStatusArg>,
    #[arg(long = "working-on")]
    pub working_on: Vec<String>,
    #[arg(long)]
    pub changed: Vec<String>,
    #[arg(long = "interface-change")]
    pub interface_change: Vec<String>,
    #[arg(long)]
    pub attention: Vec<String>,
    #[arg(long = "blocked-by")]
    pub blocked_by: Vec<String>,
    /// Append list values instead of replacing them
    #[arg(long)]
    pub append: bool,
}

#[derive(Args)]
pub struct HandoffArgs {
    #[command(flatten)]
    pub fields: HandoffFields,
    /// Sync after updating
    #[arg(long)]
    pub sync: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum WorkerStatusArg {
    Working,
    Blocked,
    Done,
    Abandoned,
}

#[derive(Subcommand)]
pub enum DecisionCommand {
    /// Add a new decision
    Add(DecisionAddArgs),
    /// List effective decisions, or all decisions with --all
    List(DecisionListArgs),
}

#[derive(Args)]
pub struct DecisionListArgs {
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DecisionAddArgs {
    pub title: String,
    #[arg(long)]
    pub context: Option<String>,
    #[arg(long)]
    pub decision: Option<String>,
    #[arg(long)]
    pub reason: Option<String>,
    #[arg(long)]
    pub consequences: Option<String>,
    #[arg(long)]
    pub supersedes: Vec<String>,
    #[arg(long, value_enum, default_value = "accepted")]
    pub status: DecisionStatusArg,
    #[arg(long)]
    pub sync: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DecisionStatusArg {
    Proposed,
    Accepted,
    Rejected,
}

#[derive(Args)]
pub struct DoneArgs {
    #[arg(long)]
    pub summary: Option<String>,
}

#[derive(Subcommand)]
pub enum AgentCommand {
    /// Prepare this agent and print onboarding context
    Start(AgentStartArgs),
    /// Publish this agent's handoff and sync
    Finish(AgentFinishArgs),
}

#[derive(Args)]
pub struct AgentStartArgs {
    /// Register with this name if this worktree is not registered yet
    #[arg(long)]
    pub name: Option<String>,
    /// Set status back to working if it is done/abandoned
    #[arg(long)]
    pub resume: bool,
}

#[derive(Args)]
pub struct AgentFinishArgs {
    #[command(flatten)]
    pub fields: HandoffFields,
}
