//! Test helpers for ctx-sync.
//!
//! A local bare repository ([`TestRemote`]) stands in for the GitHub Gist, and
//! each [`TestWorker`] is a sandbox with its own state home and project repo.
//!
//! Every git command runs with an isolated git configuration (see
//! [`git_env`]) so that the developer's global settings — for example a
//! pre-push hook that forbids pushing to `main`, or `commit.gpgsign` — cannot
//! change test results.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Fixed time passed to ctx-sync as `CTX_SYNC_NOW`.
pub const FIXED_NOW: &str = "2026-09-23T18:00:00+09:00";

/// Project ID written by [`TestRemote::seed_context`].
pub const SEED_PROJECT_ID: &str = "6f1c2b7e-0000-4000-8000-000000000001";

const GITCONFIG: &str = "\
[user]
    name = ctx-sync-test
    email = test@example.com
[init]
    defaultBranch = main
[protocol \"file\"]
    allow = always
";

/// Environment variables that detach git from the user's global and system
/// configuration: `GIT_CONFIG_GLOBAL=<config_dir>/gitconfig` and
/// `GIT_CONFIG_NOSYSTEM=1`.
///
/// Writes `<config_dir>/gitconfig` if it does not exist yet.
pub fn git_env(config_dir: &Path) -> Vec<(String, String)> {
    let path = config_dir.join("gitconfig");
    if !path.exists() {
        std::fs::create_dir_all(config_dir).expect("create git config dir");
        std::fs::write(&path, GITCONFIG).expect("write gitconfig");
    }
    vec![
        (
            "GIT_CONFIG_GLOBAL".into(),
            path.to_string_lossy().into_owned(),
        ),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
    ]
}

fn git_command(cwd: &Path, env: &[(String, String)], args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        // Do not inherit a repository from the environment (e.g. inside a git hook).
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .envs(env.iter().map(|(k, v)| (k, v)));
    cmd
}

/// Runs git with `env` and returns stdout. Panics with stderr on failure.
pub fn git(cwd: &Path, env: &[(String, String)], args: &[&str]) -> String {
    let out = git_command(cwd, env, args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed in {}:\n{}",
        cwd.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Like [`git`], but returns `None` instead of panicking on failure.
fn try_git(cwd: &Path, env: &[(String, String)], args: &[&str]) -> Option<String> {
    let out = git_command(cwd, env, args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn write_files(dir: &Path, files: &[(&str, &str)]) {
    for (name, content) in files {
        std::fs::write(dir.join(name), content).unwrap_or_else(|e| panic!("write {name}: {e}"));
    }
}

/// A bare repository standing in for the ctx-sync Gist.
pub struct TestRemote {
    dir: TempDir,
    bare: PathBuf,
    env: Vec<(String, String)>,
}

impl TestRemote {
    /// Creates a bare repository (HEAD = `main`) with an initial commit that
    /// only contains `README.md`, like a Gist created by hand (a Gist cannot
    /// be empty).
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        let env = git_env(&dir.path().join("config"));
        let bare = dir.path().join("remote.git");
        git(
            dir.path(),
            &env,
            &[
                "init",
                "-q",
                "--bare",
                "--initial-branch=main",
                "remote.git",
            ],
        );
        let remote = Self { dir, bare, env };
        remote.push_files(&[("README.md", "# test gist\n")], "initial commit");
        remote
    }

    /// Absolute path of the bare repository, for `CTX_SYNC_REMOTE_URL_OVERRIDE`.
    pub fn url(&self) -> String {
        self.bare.to_string_lossy().into_owned()
    }

    /// Commits `name` in a separate clone and pushes it to `main`, simulating
    /// a change made by another worker.
    pub fn push_file(&self, name: &str, content: &str, message: &str) {
        self.push_files(&[(name, content)], message);
    }

    fn push_files(&self, files: &[(&str, &str)], message: &str) {
        let work = tempfile::Builder::new()
            .prefix("clone-")
            .tempdir_in(self.dir.path())
            .expect("create clone dir");
        let work = work.path();
        if self.log().is_empty() {
            git(work, &self.env, &["init", "-q", "--initial-branch=main"]);
            git(work, &self.env, &["remote", "add", "origin", &self.url()]);
        } else {
            git(work, &self.env, &["clone", "-q", &self.url(), "."]);
        }
        write_files(work, files);
        git(work, &self.env, &["add", "-A"]);
        git(work, &self.env, &["commit", "-q", "-m", message]);
        git(work, &self.env, &["push", "-q", "origin", "HEAD:main"]);
    }

    /// Content of `name` on `main`, or `None` when it does not exist.
    pub fn read_file(&self, name: &str) -> Option<String> {
        let spec = format!("main:{name}");
        try_git(&self.bare, &self.env, &["show", &spec])
    }

    /// Commit subjects on `main`, newest first.
    pub fn log(&self) -> Vec<String> {
        try_git(&self.bare, &self.env, &["log", "--format=%s", "main"])
            .map(|out| out.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    pub fn git_env(&self) -> Vec<(String, String)> {
        self.env.clone()
    }

    /// Pushes `00-meta.json`, `10-project.md` and `20-architecture.md` and
    /// returns [`SEED_PROJECT_ID`].
    ///
    /// Lets phases before `init` (Phase 6) test attach, register and so on.
    pub fn seed_context(&self, project_name: &str) -> String {
        let meta = format!(
            "{{\"schema_version\":1,\"project_id\":\"{SEED_PROJECT_ID}\",\
             \"project_name\":\"{project_name}\",\"created_at\":\"{FIXED_NOW}\",\
             \"ctx_sync_version\":\"0.1.0\"}}\n"
        );
        self.push_files(
            &[
                ("00-meta.json", meta.as_str()),
                ("10-project.md", "# Project\n\n## Goal\n\nTest project.\n"),
                (
                    "20-architecture.md",
                    "# Architecture\n\n## Components\n\n- core\n- cli\n",
                ),
            ],
            "seed ctx-sync context",
        );
        SEED_PROJECT_ID.to_string()
    }
}

impl Default for TestRemote {
    fn default() -> Self {
        Self::new()
    }
}

/// One sandbox: an empty ctx-sync state home and a project repository.
pub struct TestWorker {
    _dir: TempDir,
    home: PathBuf,
    project: PathBuf,
    remote_url: String,
    env: Vec<(String, String)>,
}

impl TestWorker {
    /// Creates an empty state home and a project repository (branch `main`)
    /// with one commit containing `README.md`.
    pub fn new(remote: &TestRemote) -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        let env = git_env(&dir.path().join("config"));
        let home = dir.path().join("state");
        let project = dir.path().join("project");
        std::fs::create_dir_all(&home).expect("create state home");
        std::fs::create_dir_all(&project).expect("create project dir");
        git(&project, &env, &["init", "-q", "--initial-branch=main"]);
        write_files(&project, &[("README.md", "# test project\n")]);
        git(&project, &env, &["add", "-A"]);
        git(&project, &env, &["commit", "-q", "-m", "initial commit"]);
        Self {
            _dir: dir,
            home,
            project,
            remote_url: remote.url(),
            env,
        }
    }

    pub fn project(&self) -> &Path {
        &self.project
    }

    /// Directory to pass as `CTX_SYNC_HOME`.
    pub fn state_home(&self) -> PathBuf {
        self.home.clone()
    }

    /// Environment for a ctx-sync child process: `CTX_SYNC_HOME`,
    /// `CTX_SYNC_REMOTE_URL_OVERRIDE`, `CTX_SYNC_NOW` and [`git_env`].
    pub fn env(&self) -> Vec<(String, String)> {
        let mut env = vec![
            (
                "CTX_SYNC_HOME".to_string(),
                self.home.to_string_lossy().into_owned(),
            ),
            (
                "CTX_SYNC_REMOTE_URL_OVERRIDE".to_string(),
                self.remote_url.clone(),
            ),
            ("CTX_SYNC_NOW".to_string(), FIXED_NOW.to_string()),
        ];
        env.extend(self.env.iter().cloned());
        env
    }

    pub fn git_env(&self) -> Vec<(String, String)> {
        self.env.clone()
    }

    /// Runs git in the project repository.
    pub fn git(&self, args: &[&str]) -> String {
        git(&self.project, &self.env, args)
    }
}
