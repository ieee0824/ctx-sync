use ctx_sync_core::ops::{self, AgentStartOptions, Runtime};
use ctx_sync_core::view::render_onboard_markdown;
use ctx_sync_core::{Result, clock};

use super::not_implemented;
use crate::cli::{AgentCommand, AgentStartArgs};

pub fn run(command: &AgentCommand) -> Result<()> {
    match command {
        AgentCommand::Start(args) => start(args),
        AgentCommand::Finish(_args) => Err(not_implemented("agent finish")),
    }
}

fn start(args: &AgentStartArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let outcome = ops::agent_start(
        &rt,
        AgentStartOptions {
            cwd: std::env::current_dir()?,
            name: args.name.clone(),
            resume: args.resume,
            now: clock::now()?,
        },
    )?;
    for warning in &outcome.warnings {
        eprintln!("warning: {warning}");
    }
    print!("{}", render_onboard_markdown(&outcome.onboard));
    Ok(())
}
