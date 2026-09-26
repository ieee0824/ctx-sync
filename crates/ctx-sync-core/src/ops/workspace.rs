//! An opened project: its configuration, local state and context store.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::Runtime;
use crate::config::{CONFIG_FILE, ProjectConfig, find_project_root};
use crate::state::{Index, LocalProject, ProjectState, WorkerIdentity};
use crate::store::GistStore;
use crate::{Error, Result};

pub struct Workspace {
    /// Directory containing `.ctx-sync.toml` (the worktree root).
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub project_id: Uuid,
    pub project_state: ProjectState,
    pub local: LocalProject,
    pub store: GistStore,
}

impl Workspace {
    /// Finds `.ctx-sync.toml` from `cwd` and loads this machine's state for
    /// its Gist. Fails with `InvalidConfig` when the machine is not attached.
    pub fn open(rt: &Runtime, cwd: &Path) -> Result<Self> {
        let root = find_project_root(cwd)?;
        let config = ProjectConfig::load(&root.join(CONFIG_FILE))?;
        let gist_id = config.remote.id.clone();
        let not_attached = || {
            Error::InvalidConfig(format!(
                "this machine is not attached to gist {gist_id}; run `ctx-sync attach {gist_id}`"
            ))
        };
        let project_id = Index::load(&rt.state_root)?
            .get(&gist_id)
            .ok_or_else(not_attached)?;
        let project_state = rt.state_root.project(&project_id);
        let local = LocalProject::load(&project_state.local_json())?
            .filter(|local| local.gist_id == gist_id)
            .ok_or_else(not_attached)?;
        let store = rt
            .gist_store(&project_state, &gist_id, local.protocol)
            .with_context_files(config.context.clone());
        Ok(Self {
            root,
            config,
            project_id,
            project_state,
            local,
            store,
        })
    }

    /// Worker identity file of this worktree.
    pub fn identity_path(&self) -> Result<PathBuf> {
        self.project_state.worker_json(&self.root)
    }

    pub fn identity(&self) -> Result<Option<WorkerIdentity>> {
        WorkerIdentity::load(&self.identity_path()?)
    }

    pub fn require_identity(&self) -> Result<WorkerIdentity> {
        self.identity()?.ok_or_else(|| {
            Error::InvalidConfig("worker is not registered; run `ctx-sync register <name>`".into())
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::*;
    use crate::state::{Protocol, StateRoot};
    use crate::store::ContextStore;

    struct Fixture {
        _dir: tempfile::TempDir,
        project: PathBuf,
        rt: Runtime,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(project.join("sub")).unwrap();
        ProjectConfig::new_gist("abc")
            .save(&project.join(CONFIG_FILE))
            .unwrap();
        let rt = Runtime {
            state_root: StateRoot::new(dir.path().join("state")),
            remote_url_override: None,
            git_env: vec![],
        };
        Fixture {
            _dir: dir,
            project,
            rt,
        }
    }

    fn attach(rt: &Runtime, gist_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        let mut index = Index::load(&rt.state_root).unwrap();
        index.insert(gist_id, id);
        index.save(&rt.state_root).unwrap();
        LocalProject {
            gist_id: gist_id.into(),
            protocol: Protocol::Ssh,
            attached_at: DateTime::parse_from_rfc3339("2026-09-23T18:00:00+09:00").unwrap(),
        }
        .save(&rt.state_root.project(&id).local_json())
        .unwrap();
        id
    }

    fn expect_invalid(result: Result<Workspace>, needle: &str) {
        match result {
            Err(e @ Error::InvalidConfig(_)) => {
                assert!(e.to_string().contains(needle), "{e}")
            }
            Err(e) => panic!("unexpected error {e:?}"),
            Ok(_) => panic!("expected an error"),
        }
    }

    #[test]
    fn missing_config_is_invalid() {
        let f = fixture();
        let elsewhere = tempfile::tempdir().unwrap();
        expect_invalid(Workspace::open(&f.rt, elsewhere.path()), "ctx-sync init");
    }

    #[test]
    fn unattached_machine_is_invalid() {
        let f = fixture();
        expect_invalid(Workspace::open(&f.rt, &f.project), "ctx-sync attach abc");
    }

    #[test]
    fn index_without_local_json_is_invalid() {
        let f = fixture();
        let mut index = Index::default();
        index.insert("abc", Uuid::new_v4());
        index.save(&f.rt.state_root).unwrap();
        expect_invalid(Workspace::open(&f.rt, &f.project), "ctx-sync attach abc");
    }

    #[test]
    fn opens_an_attached_project_from_a_subdirectory() {
        let f = fixture();
        let id = attach(&f.rt, "abc");
        let ws = Workspace::open(&f.rt, &f.project.join("sub")).unwrap();
        assert_eq!(ws.root, f.project);
        assert_eq!(ws.project_id, id);
        assert_eq!(ws.config.remote.id, "abc");
        assert_eq!(ws.local.protocol, Protocol::Ssh);
        assert_eq!(ws.store.repo_dir(), ws.project_state.context_repo());
        assert_eq!(ws.store.remote().url(), "git@gist.github.com:abc.git");
    }

    #[test]
    fn identity_is_required_until_registered() {
        let f = fixture();
        attach(&f.rt, "abc");
        let ws = Workspace::open(&f.rt, &f.project).unwrap();
        assert!(ws.identity().unwrap().is_none());
        let err = ws.require_identity().unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
        assert!(err.to_string().contains("ctx-sync register"));

        let now = DateTime::parse_from_rfc3339("2026-09-23T18:00:00+09:00").unwrap();
        let identity = WorkerIdentity::new("parser", &ws.root, now).unwrap();
        identity.save(&ws.identity_path().unwrap()).unwrap();
        assert_eq!(ws.identity().unwrap(), Some(identity.clone()));
        assert_eq!(ws.require_identity().unwrap(), identity);
    }
}
