use ctx_sync_core::ops::{self, AgentStartOptions, Runtime, Workspace};
use ctx_sync_core::view::render_onboard_markdown;
use ctx_sync_core::{Error, Result, clock};

use crate::cli::{AgentCommand, AgentFinishArgs, AgentStartArgs};
use crate::convert::handoff_input;

pub fn run(command: &AgentCommand) -> Result<()> {
    match command {
        AgentCommand::Start(args) => start(args),
        AgentCommand::Finish(args) => finish(args),
    }
}

fn finish(args: &AgentFinishArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let outcome = ops::agent_finish(&ws, handoff_input(&args.fields), clock::now()?)?;
    for warning in outcome
        .warnings
        .iter()
        .chain(outcome.sync.iter().flat_map(|sync| &sync.warnings))
    {
        eprintln!("warning: {warning}");
    }
    let revision = outcome
        .sync
        .as_ref()
        .expect("agent_finish always synchronizes")
        .revision
        .as_str();
    println!("# Handoff Complete\n");
    println!("worker: {} ({})", outcome.worker_name, outcome.short_id);
    println!("status: {}", outcome.status);
    println!("file: {}", outcome.file_name);
    println!("revision: {revision}");
    Ok(())
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
            stale_after: args.stale_after,
        },
    )?;
    for warning in &outcome.warnings {
        eprintln!("warning: {warning}");
    }
    if args.json {
        let json = serde_json::to_string_pretty(&outcome)
            .map_err(|e| Error::General(format!("cannot serialize agent start: {e}")))?;
        println!("{json}");
    } else {
        print!("{}", render_onboard_markdown(&outcome.onboard));
    }
    Ok(())
}
