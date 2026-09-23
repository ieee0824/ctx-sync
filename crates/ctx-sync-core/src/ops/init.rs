//! `init`: create a new ctx-sync project.
//!
//! project id → initial context → Gist → `.ctx-sync.toml` → local state.

use std::path::PathBuf;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use uuid::Uuid;

use super::Runtime;
use crate::config::{CONFIG_FILE, ProjectConfig};
use crate::gist_id::parse_gist_id;
use crate::model::docs::{initial_architecture_md, initial_project_md};
use crate::model::{ARCHITECTURE_FILE, META_FILE, Meta, PROJECT_FILE};
use crate::state::{Index, LocalProject, Protocol};
use crate::store::ContextStore;
use crate::{Error, Result};

pub struct InitOptions {
    pub project_root: PathBuf,
    pub project_name: String,
    pub remote: InitRemote,
    pub protocol: Protocol,
    /// Overwrite an existing `.ctx-sync.toml`.
    pub force: bool,
    pub now: DateTime<FixedOffset>,
}

pub enum InitRemote {
    /// A Gist created by hand (it must not contain a ctx-sync context yet).
    ExistingGist { gist_id: String },
    /// Create a new Gist with `gh gist create`.
    CreateGist { public: bool },
}

#[derive(Debug, Clone, Serialize)]
pub struct InitOutcome {
    pub project_id: Uuid,
    pub project_name: String,
    pub gist_id: String,
    pub config_path: PathBuf,
    pub revision: String,
}

pub fn init(rt: &Runtime, opts: InitOptions) -> Result<InitOutcome> {
    let name = opts.project_name.trim().to_string();
    if name.is_empty() {
        return Err(Error::InvalidConfig("project name is empty".into()));
    }
    let config_path = opts.project_root.join(CONFIG_FILE);
    if config_path.exists() && !opts.force {
        return Err(Error::InvalidConfig(format!(
            "{CONFIG_FILE} already exists; use --force to overwrite"
        )));
    }
    match &opts.remote {
        InitRemote::ExistingGist { gist_id } => {
            init_existing_gist(rt, &opts, &name, &parse_gist_id(gist_id)?)
        }
        InitRemote::CreateGist { .. } => Err(Error::General("not implemented: create gist".into())),
    }
}

fn init_existing_gist(
    rt: &Runtime,
    opts: &InitOptions,
    name: &str,
    gist_id: &str,
) -> Result<InitOutcome> {
    let meta = Meta::new(name, opts.now);
    let state = rt.state_root.project(&meta.project_id);
    let store = rt.gist_store(&state, gist_id, opts.protocol);
    store.ensure()?;
    if store.read_file(META_FILE)?.is_some() {
        // The project id is new, so the whole state directory is ours.
        let _ = std::fs::remove_dir_all(state.dir());
        return Err(Error::InvalidConfig(format!(
            "gist {gist_id} already contains a ctx-sync context; use `ctx-sync attach {gist_id}`"
        )));
    }
    store.write_file(META_FILE, &meta.to_json())?;
    store.write_file(PROJECT_FILE, &initial_project_md(name))?;
    store.write_file(ARCHITECTURE_FILE, &initial_architecture_md())?;
    store.commit(&format!("ctx-sync: init {name}"))?;
    let revision = store.sync(opts.now)?.revision;
    finish(rt, opts, &meta, gist_id, revision)
}

/// Saves `.ctx-sync.toml`, `index.json` and `local.json`.
fn finish(
    rt: &Runtime,
    opts: &InitOptions,
    meta: &Meta,
    gist_id: &str,
    revision: String,
) -> Result<InitOutcome> {
    let config_path = opts.project_root.join(CONFIG_FILE);
    ProjectConfig::new_gist(gist_id).save(&config_path)?;
    let mut index = Index::load(&rt.state_root)?;
    index.insert(gist_id, meta.project_id);
    index.save(&rt.state_root)?;
    LocalProject {
        gist_id: gist_id.to_string(),
        protocol: opts.protocol,
        attached_at: opts.now,
    }
    .save(&rt.state_root.project(&meta.project_id).local_json())?;
    Ok(InitOutcome {
        project_id: meta.project_id,
        project_name: meta.project_name.clone(),
        gist_id: gist_id.to_string(),
        config_path,
        revision,
    })
}
