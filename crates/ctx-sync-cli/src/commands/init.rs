use ctx_sync_core::config::ContextFiles;
use ctx_sync_core::ops::{self, InitOptions, InitRemote, Runtime};
use ctx_sync_core::state::Protocol;
use ctx_sync_core::{Error, Result, clock};

use crate::cli::InitArgs;
use crate::project_root::detect_project_root;
use crate::prompt;

pub fn run(args: &InitArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let root = detect_project_root(&std::env::current_dir()?);
    let interactive = !args.no_input && prompt::is_interactive();

    let dir_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project_name = match &args.project {
        Some(name) => name.clone(),
        None if interactive => prompt::ask("Project name", Some(&dir_name))?,
        None => dir_name,
    };

    let remote = if let Some(gist_id) = &args.gist_id {
        InitRemote::ExistingGist {
            gist_id: gist_id.clone(),
        }
    } else if args.create_gist {
        InitRemote::CreateGist {
            public: public(args, interactive)?,
        }
    } else if interactive {
        if prompt::confirm("Create GitHub Gist?", true)? {
            InitRemote::CreateGist {
                public: public(args, interactive)?,
            }
        } else {
            InitRemote::ExistingGist {
                gist_id: prompt::ask("Existing gist ID or URL", None)?,
            }
        }
    } else {
        return Err(Error::InvalidConfig(
            "either --create-gist or --gist-id is required".into(),
        ));
    };

    let outcome = ops::init(
        &rt,
        InitOptions {
            project_root: root,
            project_name,
            remote,
            protocol: if args.ssh {
                Protocol::Ssh
            } else {
                Protocol::Https
            },
            force: args.force,
            context_files: ContextFiles {
                project: args
                    .project_file
                    .clone()
                    .unwrap_or_else(|| ContextFiles::default().project),
                architecture: args
                    .architecture_file
                    .clone()
                    .unwrap_or_else(|| ContextFiles::default().architecture),
            },
            now: clock::now()?,
        },
    )?;
    for hint in &outcome.hints {
        eprintln!("hint: {hint}");
    }
    println!("Initialized ctx-sync project");
    println!();
    println!("project: {}", outcome.project_name);
    println!("project id: {}", outcome.project_id);
    println!("gist: {}", outcome.gist_id);
    println!("config: .ctx-sync.toml");
    println!();
    println!("Next:");
    println!("  git add .ctx-sync.toml && git commit");
    println!("  ctx-sync register <name>");
    Ok(())
}

/// Visibility of a new Gist: secret unless asked otherwise.
fn public(args: &InitArgs, interactive: bool) -> Result<bool> {
    if args.public {
        Ok(true)
    } else if args.secret || !interactive {
        Ok(false)
    } else {
        Ok(prompt::choose("Visibility", &["secret", "public"], 0)? == 1)
    }
}
