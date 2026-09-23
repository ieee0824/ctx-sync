//! One module per command. Each command is filled in by its own issue.

use ctx_sync_core::{Error, Result};

use crate::cli::Command;

mod agent;
mod attach;
mod bootstrap;
mod context;
mod decision;
mod done;
mod handoff;
mod init;
mod install_agent_instructions;
mod onboard;
mod pull;
mod register;
mod status;
mod sync;

pub fn dispatch(command: Command) -> Result<()> {
    match command {
        Command::Init(args) => init::run(&args),
        Command::Attach(args) => attach::run(&args),
        Command::Bootstrap(args) => bootstrap::run(&args),
        Command::Register(args) => register::run(&args),
        Command::Pull => pull::run(),
        Command::Sync => sync::run(),
        Command::Context(args) => context::run(&args),
        Command::Onboard => onboard::run(),
        Command::Status => status::run(),
        Command::Handoff(args) => handoff::run(&args),
        Command::Decision(command) => decision::run(&command),
        Command::Done(args) => done::run(&args),
        Command::Agent(command) => agent::run(&command),
        Command::InstallAgentInstructions => install_agent_instructions::run(),
    }
}

fn not_implemented(command: &str) -> Error {
    Error::General(format!("not implemented: {command}"))
}
