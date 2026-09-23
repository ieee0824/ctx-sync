//! `attach`: connect this project and machine to an existing ctx-sync Gist.
//!
//! Works without `gh`: only `git` is used.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use uuid::Uuid;

use super::Runtime;
use crate::config::{CONFIG_FILE, ProjectConfig};
use crate::gist_id::parse_gist_id;
use crate::model::{META_FILE, Meta};
use crate::state::{Index, LocalProject, Protocol, StateRoot};
use crate::store::ContextStore;
use crate::{Error, Result};

pub struct AttachOptions {
    pub project_root: PathBuf,
    /// Gist ID or URL.
    pub gist: String,
    pub protocol: Protocol,
    pub now: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttachOutcome {
    pub project_id: Uuid,
    pub project_name: String,
    pub gist_id: String,
    /// Whether `.ctx-sync.toml` was newly written.
    pub config_written: bool,
    pub revision: String,
}

pub fn attach(rt: &Runtime, opts: AttachOptions) -> Result<AttachOutcome> {
    let gist_id = parse_gist_id(&opts.gist)?;
    let config_path = opts.project_root.join(CONFIG_FILE);
    let has_config = config_path.is_file();
    if has_config {
        let config = ProjectConfig::load(&config_path)?;
        if config.remote.id != gist_id {
            return Err(Error::InvalidConfig(format!(
                "this project is already attached to gist {}",
                config.remote.id
            )));
        }
    }

    // The project id is only known after reading 00-meta.json, so clone
    // into a temporary location first.
    let tmp = TempDir::new(rt.state_root.tmp_dir().join(Uuid::new_v4().to_string()));
    let tmp_state = StateRoot::new(tmp.path()).project(&Uuid::nil());
    let tmp_store = rt.gist_store(&tmp_state, &gist_id, opts.protocol);
    tmp_store.ensure()?;
    let meta = match tmp_store.read_file(META_FILE)? {
        Some(text) => Meta::parse(&text)?,
        None => {
            return Err(Error::InvalidConfig(format!(
                "gist {gist_id} is not a ctx-sync context ({META_FILE} not found)"
            )));
        }
    };

    let state = rt.state_root.project(&meta.project_id);
    let store = rt.gist_store(&state, &gist_id, opts.protocol);
    if state.context_repo().join(".git").exists() {
        // Already cloned on this machine (e.g. another worktree): reuse it,
        // pointing it at the requested URL.
        store
            .git()
            .run_checked(&["remote", "set-url", "origin", &store.remote().url()])?;
        store.pull()?;
    } else {
        std::fs::create_dir_all(state.dir())?;
        std::fs::rename(tmp_state.context_repo(), state.context_repo())?;
    }
    drop(tmp);

    if !has_config {
        ProjectConfig::new_gist(&gist_id).save(&config_path)?;
    }
    let mut index = Index::load(&rt.state_root)?;
    index.insert(&gist_id, meta.project_id);
    index.save(&rt.state_root)?;
    LocalProject {
        gist_id: gist_id.clone(),
        protocol: opts.protocol,
        attached_at: opts.now,
    }
    .save(&state.local_json())?;

    Ok(AttachOutcome {
        project_id: meta.project_id,
        project_name: meta.project_name,
        gist_id,
        config_written: !has_config,
        revision: store.revision()?.unwrap_or_default(),
    })
}

/// Directory removed when dropped, on success and on every error path.
struct TempDir(PathBuf);

impl TempDir {
    fn new(path: PathBuf) -> Self {
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use ctx_sync_testutil::{FIXED_NOW, SEED_PROJECT_ID, TestRemote};

    use super::*;
    use crate::clock::parse_now;

    fn runtime(remote: &TestRemote, home: &Path) -> Runtime {
        Runtime {
            state_root: StateRoot::new(home),
            remote_url_override: Some(remote.url()),
            git_env: remote.git_env(),
        }
    }

    fn options(project: &Path, gist: &str) -> AttachOptions {
        AttachOptions {
            project_root: project.to_path_buf(),
            gist: gist.into(),
            protocol: Protocol::Https,
            now: parse_now(Some(FIXED_NOW)).unwrap(),
        }
    }

    #[test]
    fn attaches_and_records_local_state() {
        let remote = TestRemote::new();
        remote.seed_context("demo");
        let dir = tempfile::tempdir().unwrap();
        let rt = runtime(&remote, &dir.path().join("state"));
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();

        let outcome = attach(&rt, options(&project, "testgist01")).unwrap();
        assert_eq!(outcome.project_id.to_string(), SEED_PROJECT_ID);
        assert_eq!(outcome.project_name, "demo");
        assert!(outcome.config_written);
        assert!(!outcome.revision.is_empty());

        let state = rt.state_root.project(&outcome.project_id);
        assert!(state.context_repo().join(META_FILE).is_file());
        assert_eq!(
            Index::load(&rt.state_root).unwrap().get("testgist01"),
            Some(outcome.project_id)
        );
        assert!(LocalProject::load(&state.local_json()).unwrap().is_some());
        // Nothing is left in the temporary directory.
        let leftovers = std::fs::read_dir(rt.state_root.tmp_dir()).map_or(0, |d| d.count());
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn missing_meta_cleans_up_and_is_invalid_config() {
        let remote = TestRemote::new();
        let dir = tempfile::tempdir().unwrap();
        let rt = runtime(&remote, &dir.path().join("state"));
        let err = attach(&rt, options(dir.path(), "testgist01")).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
        assert!(err.to_string().contains("not a ctx-sync context"));
        let leftovers = std::fs::read_dir(rt.state_root.tmp_dir()).map_or(0, |d| d.count());
        assert_eq!(leftovers, 0);
        assert!(!dir.path().join(CONFIG_FILE).exists());
    }
}
