//! Command line definition.

use std::path::PathBuf;

use chrono::TimeDelta;
use clap::{Args, Parser, Subcommand, ValueEnum};
use ctx_sync_core::model::parse_duration;

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
    Onboard(OnboardArgs),
    /// Show project, sync and worker status
    Status(StatusArgs),
    /// Update this worker's shared state
    Handoff(HandoffArgs),
    /// Manage decisions
    #[command(subcommand)]
    Decision(DecisionCommand),
    /// Inspect or resolve a recorded context conflict
    #[command(subcommand)]
    Conflict(ConflictCommand),
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
    /// Only include decisions and workers related to this task (keyword match)
    #[arg(long)]
    pub task: Option<String>,
    /// Treat active workers not updated for this long as stale (e.g. 90m, 24h, 3d)
    #[arg(long, default_value = "24h", value_parser = parse_stale_after)]
    pub stale_after: TimeDelta,
}

#[derive(Args)]
pub struct OnboardArgs {
    #[arg(long)]
    pub json: bool,
    /// Treat active workers not updated for this long as stale (e.g. 90m, 24h, 3d)
    #[arg(long, default_value = "24h", value_parser = parse_stale_after)]
    pub stale_after: TimeDelta,
}

#[derive(Args)]
pub struct StatusArgs {
    /// Treat active workers not updated for this long as stale (e.g. 90m, 24h, 3d)
    #[arg(long, default_value = "24h", value_parser = parse_stale_after)]
    pub stale_after: TimeDelta,
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
    /// Replace a decision by appending a new one
    Supersede(DecisionSupersedeArgs),
}

#[derive(Args)]
pub struct DecisionListArgs {
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DecisionSupersedeArgs {
    /// ID of the decision to replace
    pub old: String,
    pub title: String,
    #[arg(long)]
    pub context: Option<String>,
    #[arg(long)]
    pub decision: Option<String>,
    #[arg(long)]
    pub reason: Option<String>,
    #[arg(long)]
    pub consequences: Option<String>,
    #[arg(long, value_enum, default_value = "accepted")]
    pub status: DecisionStatusArg,
    #[arg(long)]
    pub sync: bool,
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

#[derive(Subcommand)]
pub enum ConflictCommand {
    /// Show the recorded context conflict
    Show(ConflictShowArgs),
    /// Resolve the recorded context conflict and sync
    Resolve(ConflictResolveArgs),
}

#[derive(Args)]
pub struct ConflictShowArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
#[command(group(clap::ArgGroup::new("how").required(true).args(["keep_local", "keep_remote", "merged"])))]
pub struct ConflictResolveArgs {
    #[arg(long)]
    pub keep_local: bool,
    #[arg(long)]
    pub keep_remote: bool,
    /// FILE=PATH: use the content of PATH as the resolved FILE
    #[arg(long, value_parser = parse_merged)]
    pub merged: Vec<(String, PathBuf)>,
}

fn parse_merged(value: &str) -> Result<(String, PathBuf), String> {
    let (file, path) = value
        .split_once('=')
        .ok_or_else(|| "expected FILE=PATH".to_string())?;
    if file.is_empty() || path.is_empty() {
        return Err("expected FILE=PATH".into());
    }
    Ok((file.into(), path.into()))
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
    #[arg(long)]
    pub json: bool,
    /// Treat active workers not updated for this long as stale (e.g. 90m, 24h, 3d)
    #[arg(long, default_value = "24h", value_parser = parse_stale_after)]
    pub stale_after: TimeDelta,
}

fn parse_stale_after(s: &str) -> Result<TimeDelta, String> {
    parse_duration(s).map_err(|error| error.to_string())
}

#[derive(Args)]
pub struct AgentFinishArgs {
    #[command(flatten)]
    pub fields: HandoffFields,
}
