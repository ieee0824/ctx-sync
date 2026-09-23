use ctx_sync_core::Result;
use ctx_sync_core::ops::{InstallOutcome, install_agent_instructions};

use crate::project_root::detect_project_root;

pub fn run() -> Result<()> {
    let root = detect_project_root(&std::env::current_dir()?);
    let message = match install_agent_instructions(&root)? {
        InstallOutcome::Created => "Created AGENTS.md",
        InstallOutcome::Appended => "Appended Shared Context section to AGENTS.md",
        InstallOutcome::AlreadyPresent => "AGENTS.md already contains a Shared Context section",
    };
    println!("{message}");
    Ok(())
}
