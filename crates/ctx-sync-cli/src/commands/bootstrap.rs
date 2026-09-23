use ctx_sync_core::Result;
use ctx_sync_core::bootstrap;
use ctx_sync_core::ops::{Runtime, Workspace, apply_bootstrap};

use crate::cli::BootstrapArgs;
use crate::project_root::detect_project_root;

/// Without `--apply`, only prints the candidates; `.ctx-sync.toml` is not
/// needed. With `--apply`, adds them to `10-project.md` (no sync).
pub fn run(args: &BootstrapArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    if !args.apply {
        let root = detect_project_root(&cwd);
        print!("{}", bootstrap::collect(&root, &[]).render_markdown());
        return Ok(());
    }
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &cwd)?;
    let report = bootstrap::collect(&ws.root, &[]);
    let outcome = apply_bootstrap(&ws, &report)?;
    println!("Applied initial context to {}", outcome.file_name);
    match &outcome.committed {
        Some(commit) => println!("committed: {commit}"),
        None => println!("committed: no changes"),
    }
    println!(
        "next: review {}, then run `ctx-sync sync`",
        outcome.file_name
    );
    Ok(())
}
