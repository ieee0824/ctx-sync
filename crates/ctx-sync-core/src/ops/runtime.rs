//! Execution environment of a command.
//!
//! Environment variables are read here, in one place. Everything else in the
//! core receives these values as arguments, which keeps it testable.

use crate::Result;
use crate::state::{ProjectState, Protocol, StateRoot};
use crate::store::{GistStore, REMOTE_URL_OVERRIDE_ENV, RemoteSpec};

#[derive(Debug, Clone)]
pub struct Runtime {
    pub state_root: StateRoot,
    /// `CTX_SYNC_REMOTE_URL_OVERRIDE`: replaces every Gist URL (tests).
    pub remote_url_override: Option<String>,
    /// Extra environment for git child processes (tests).
    pub git_env: Vec<(String, String)>,
}

impl Runtime {
    /// Reads `CTX_SYNC_HOME` and `CTX_SYNC_REMOTE_URL_OVERRIDE`.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            state_root: StateRoot::resolve()?,
            remote_url_override: std::env::var(REMOTE_URL_OVERRIDE_ENV)
                .ok()
                .filter(|v| !v.is_empty()),
            git_env: Vec::new(),
        })
    }

    pub fn remote_spec(&self, gist_id: &str, protocol: Protocol) -> RemoteSpec {
        RemoteSpec::new(gist_id, protocol).with_url_override(self.remote_url_override.clone())
    }

    pub fn gist_store(&self, state: &ProjectState, gist_id: &str, protocol: Protocol) -> GistStore {
        GistStore::new(state, self.remote_spec(gist_id, protocol))
            .with_git_env(self.git_env.clone())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::store::ContextStore;

    fn runtime(url_override: Option<&str>) -> Runtime {
        Runtime {
            state_root: StateRoot::new("/state"),
            remote_url_override: url_override.map(str::to_string),
            git_env: vec![("K".into(), "V".into())],
        }
    }

    #[test]
    fn remote_spec_applies_the_override() {
        assert_eq!(
            runtime(None).remote_spec("abc", Protocol::Https).url(),
            "https://gist.github.com/abc.git"
        );
        assert_eq!(
            runtime(Some("/tmp/remote.git"))
                .remote_spec("abc", Protocol::Https)
                .url(),
            "/tmp/remote.git"
        );
    }

    #[test]
    fn gist_store_uses_the_project_state() {
        let rt = runtime(Some("/tmp/remote.git"));
        let state = rt.state_root.project(&Uuid::nil());
        let store = rt.gist_store(&state, "abc", Protocol::Ssh);
        assert_eq!(store.repo_dir(), state.context_repo());
        assert_eq!(store.remote().url(), "/tmp/remote.git");
        assert_eq!(store.remote().gist_id, "abc");
    }

    #[test]
    fn from_env_succeeds() {
        assert!(Runtime::from_env().is_ok());
    }
}
