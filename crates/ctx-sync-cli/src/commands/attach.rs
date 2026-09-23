use ctx_sync_core::ops::{self, AttachOptions, Runtime};
use ctx_sync_core::state::Protocol;
use ctx_sync_core::{Result, clock};

use crate::cli::AttachArgs;
use crate::project_root::detect_project_root;

pub fn run(args: &AttachArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let outcome = ops::attach(
        &rt,
        AttachOptions {
            project_root: detect_project_root(&std::env::current_dir()?),
            gist: args.gist.clone(),
            protocol: if args.ssh {
                Protocol::Ssh
            } else {
                Protocol::Https
            },
            now: clock::now()?,
        },
    )?;
    let config = if outcome.config_written {
        "created"
    } else {
        "unchanged"
    };
    println!("Attached to ctx-sync project");
    println!();
    println!("project: {}", outcome.project_name);
    println!("project id: {}", outcome.project_id);
    println!("gist: {}", outcome.gist_id);
    println!("config: .ctx-sync.toml ({config})");
    Ok(())
}
