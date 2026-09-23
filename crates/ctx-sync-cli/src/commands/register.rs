use ctx_sync_core::ops::{self, Runtime, Workspace};
use ctx_sync_core::{Result, clock};

use crate::cli::RegisterArgs;

pub fn run(args: &RegisterArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let outcome = ops::register(&ws, &args.name, args.force, clock::now()?)?;
    println!(
        "{}",
        if outcome.created {
            "Registered worker"
        } else {
            "Already registered"
        }
    );
    println!();
    println!("id: {}", outcome.identity.short_id());
    println!("name: {}", outcome.identity.name);
    Ok(())
}
