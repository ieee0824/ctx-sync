//! `init`: create a new ctx-sync project.
//!
//! project id → initial context → Gist → `.ctx-sync.toml` → local state.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use uuid::Uuid;

use super::Runtime;
use crate::config::{CONFIG_FILE, ProjectConfig};
use crate::fs_util::TempDirGuard;
use crate::gh;
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
    /// Follow-up advice for the user (e.g. how to enable pushing).
    pub hints: Vec<String>,
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
        InitRemote::CreateGist { public } => init_new_gist(rt, &opts, &name, *public),
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
    finish(rt, opts, &meta, gist_id, revision, Vec::new())
}

/// Creates the Gist with the initial files using `gh`, then clones it.
fn init_new_gist(
    rt: &Runtime,
    opts: &InitOptions,
    name: &str,
    public: bool,
) -> Result<InitOutcome> {
    if !gh::is_available() {
        return Err(Error::InvalidConfig(
            "gh command not found; create a gist manually and run `ctx-sync init --gist-id <id>`"
                .into(),
        ));
    }
    if !gh::is_authenticated()? {
        return Err(Error::Auth(
            "gh is not authenticated; run `gh auth login`".into(),
        ));
    }
    let meta = Meta::new(name, opts.now);
    let tmp = TempDirGuard::new(rt.state_root.tmp_dir().join(Uuid::new_v4().to_string()));
    let files = write_initial_files(tmp.path(), &meta)?;
    let gist_id = gh::gist_create(&files, public, &format!("ctx-sync: {name}"))?;
    drop(tmp);

    let state = rt.state_root.project(&meta.project_id);
    let store = rt.gist_store(&state, &gist_id, opts.protocol);
    store.ensure()?;
    let revision = store.revision()?.unwrap_or_default();
    let mut hints = Vec::new();
    if opts.protocol == Protocol::Https {
        hints.push("to push over https, run `gh auth setup-git` once (or use --ssh)".to_string());
    }
    finish(rt, opts, &meta, &gist_id, revision, hints)
}

/// Writes `00-meta.json`, `10-project.md` and `20-architecture.md` into
/// `dir` and returns their paths.
fn write_initial_files(dir: &Path, meta: &Meta) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let files = [
        (META_FILE, meta.to_json()),
        (PROJECT_FILE, initial_project_md(&meta.project_name)),
        (ARCHITECTURE_FILE, initial_architecture_md()),
    ];
    let mut paths = Vec::new();
    for (name, content) in files {
        let path = dir.join(name);
        std::fs::write(&path, content)?;
        paths.push(path);
    }
    Ok(paths)
}

/// Saves `.ctx-sync.toml`, `index.json` and `local.json`.
fn finish(
    rt: &Runtime,
    opts: &InitOptions,
    meta: &Meta,
    gist_id: &str,
    revision: String,
    hints: Vec<String>,
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
        hints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::parse_now;
    use crate::model::MdDoc;

    #[test]
    fn initial_files_are_written_to_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let meta = Meta::new(
            "demo",
            parse_now(Some("2026-09-23T18:00:00+09:00")).unwrap(),
        );
        let paths = write_initial_files(&dir.path().join("tmp"), &meta).unwrap();
        let names: Vec<_> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, [META_FILE, PROJECT_FILE, ARCHITECTURE_FILE]);
        let parsed = Meta::parse(&std::fs::read_to_string(&paths[0]).unwrap()).unwrap();
        assert_eq!(parsed, meta);
        let project = MdDoc::parse(&std::fs::read_to_string(&paths[1]).unwrap()).unwrap();
        assert!(project.section_body("Goal").unwrap().contains("demo"));
    }

    /// Creates a real secret gist with `gh`. Run manually with
    /// `CTX_SYNC_GH_TEST=1 cargo test -p ctx-sync-core -- --ignored` and
    /// delete the gist afterwards.
    #[test]
    #[ignore = "creates a real GitHub gist"]
    fn creates_a_gist_with_gh() {
        if std::env::var("CTX_SYNC_GH_TEST").as_deref() != Ok("1") {
            eprintln!("skipped: set CTX_SYNC_GH_TEST=1 to create a real gist");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let rt = Runtime {
            state_root: crate::state::StateRoot::new(dir.path().join("state")),
            remote_url_override: None,
            git_env: Vec::new(),
        };
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let outcome = init(
            &rt,
            InitOptions {
                project_root: project.clone(),
                project_name: "ctx-sync-gh-test".into(),
                remote: InitRemote::CreateGist { public: false },
                protocol: Protocol::Https,
                force: false,
                now: parse_now(None).unwrap(),
            },
        )
        .unwrap();
        eprintln!("created gist {}", outcome.gist_id);
        assert!(project.join(CONFIG_FILE).is_file());
    }
}
