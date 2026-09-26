use ctx_sync_core::ops::{Resolution, Runtime, Workspace, conflict_resolve, conflict_show};
use ctx_sync_core::view::render_conflict_text;
use ctx_sync_core::{Result, clock};

use crate::cli::ConflictCommand;

pub fn run(command: &ConflictCommand) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    match command {
        ConflictCommand::Show(args) => {
            let view = conflict_show(&ws)?;
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&view).expect("serialize conflict view")
                );
            } else {
                print!("{}", render_conflict_text(&view));
            }
        }
        ConflictCommand::Resolve(args) => {
            let resolution = if args.keep_local {
                Resolution::KeepLocal
            } else if args.keep_remote {
                Resolution::KeepRemote
            } else {
                Resolution::Merged(args.merged.clone())
            };
            let outcome = conflict_resolve(&ws, resolution, clock::now()?)?;
            println!("Resolved context conflict ({})", outcome.resolution);
            println!("files: {}", outcome.files.join(", "));
            println!("synced: {}", outcome.sync.revision);
            for warning in &outcome.sync.warnings {
                eprintln!("warning: {warning}");
            }
        }
    }
    Ok(())
}
