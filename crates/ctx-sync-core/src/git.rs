//! Thin wrapper around the `git` command.
//!
//! ctx-sync runs `git` as a subprocess instead of linking libgit2 so that the
//! user's credentials, SSH configuration and rebase logic are used as-is.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{Error, Result};

/// Runs `git` in a fixed working directory.
#[derive(Debug, Clone)]
pub struct Git {
    cwd: PathBuf,
    env: Vec<(String, String)>,
}

/// Captured result of a `git` invocation.
#[derive(Debug, Clone)]
pub struct GitOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Git {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            env: Vec::new(),
        }
    }

    /// Adds environment variables passed to every `git` child process.
    pub fn with_env(mut self, vars: &[(String, String)]) -> Self {
        self.env.extend(vars.iter().cloned());
        self
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Runs `git`. Returns `Ok` even when git exits with a non-zero status.
    pub fn run(&self, args: &[&str]) -> Result<GitOutput> {
        if !self.cwd.is_dir() {
            return Err(Error::General(format!(
                "working directory does not exist: {}",
                self.cwd.display()
            )));
        }
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            // stderr is classified by its text, so keep it in English.
            .env("LC_ALL", "C")
            .env("LANG", "C")
            // Never block an AI agent on an interactive credential prompt.
            .env("GIT_TERMINAL_PROMPT", "0")
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    Error::General("git command not found; please install git".into())
                }
                _ => Error::Io(e),
            })?;
        Ok(GitOutput {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Runs `git` and returns its stdout. A non-zero exit status is an error.
    pub fn run_checked(&self, args: &[&str]) -> Result<String> {
        let out = self.run(args)?;
        if out.success {
            Ok(out.stdout)
        } else {
            Err(error_from_output(args, &out))
        }
    }

    /// Like [`Git::run_checked`], with surrounding whitespace trimmed.
    pub fn run_line(&self, args: &[&str]) -> Result<String> {
        Ok(self.run_checked(args)?.trim().to_string())
    }
}

fn error_from_output(args: &[&str], out: &GitOutput) -> Error {
    failure_to_error(
        classify(&out.stderr),
        format!("git {} failed: {}", args.join(" "), out.stderr.trim()),
    )
}

/// Kind of a `git` failure, judged from its stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFailure {
    Auth,
    NonFastForward,
    Conflict,
    NotFound,
    Network,
    Other,
}

/// Patterns in lowercase, evaluated from top to bottom.
const PATTERNS: &[(GitFailure, &[&str])] = &[
    (
        GitFailure::Auth,
        &[
            "authentication failed",
            "permission denied (publickey)",
            "could not read username",
            "could not read password",
            "terminal prompts disabled",
            "invalid username or password",
            "returned error: 403",
        ],
    ),
    (
        GitFailure::NonFastForward,
        &[
            "non-fast-forward",
            "(fetch first)",
            "updates were rejected because",
        ],
    ),
    (
        GitFailure::Conflict,
        &[
            "conflict (",
            "could not apply",
            "merge conflict",
            "resolve all conflicts",
        ],
    ),
    (
        GitFailure::NotFound,
        &[
            "repository not found",
            "does not appear to be a git repository",
            "returned error: 404",
        ],
    ),
    (
        GitFailure::Network,
        &[
            "could not resolve host",
            "connection timed out",
            "failed to connect",
            "connection refused",
            "network is unreachable",
        ],
    ),
];

/// Classifies a git failure from its stderr (case-insensitive).
///
/// Requires the English messages produced under `LC_ALL=C`.
pub fn classify(stderr: &str) -> GitFailure {
    let stderr = stderr.to_lowercase();
    PATTERNS
        .iter()
        .find(|(_, patterns)| patterns.iter().any(|p| stderr.contains(p)))
        .map_or(GitFailure::Other, |(kind, _)| *kind)
}

/// Converts a classified failure into the error that carries its exit code.
pub fn failure_to_error(kind: GitFailure, message: String) -> Error {
    match kind {
        GitFailure::Auth => Error::Auth(message),
        GitFailure::NonFastForward | GitFailure::NotFound | GitFailure::Network => {
            Error::Remote(message)
        }
        GitFailure::Conflict => Error::ContextConflict { files: vec![] },
        GitFailure::Other => Error::General(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo() -> (tempfile::TempDir, Git) {
        let dir = tempfile::tempdir().unwrap();
        let git = Git::new(dir.path());
        git.run_checked(&["init", "-q"]).unwrap();
        (dir, git)
    }

    #[test]
    fn runs_git_in_cwd() {
        let (_dir, git) = init_repo();
        assert_eq!(
            git.run_line(&["rev-parse", "--is-inside-work-tree"])
                .unwrap(),
            "true"
        );
    }

    #[test]
    fn run_returns_ok_on_failure() {
        let (_dir, git) = init_repo();
        let out = git.run(&["no-such-subcommand"]).unwrap();
        assert!(!out.success);
        assert!(!out.stderr.is_empty());
    }

    #[test]
    fn run_checked_returns_err_on_failure() {
        let (_dir, git) = init_repo();
        let err = git.run_checked(&["no-such-subcommand"]).unwrap_err();
        assert!(matches!(err, Error::General(_)), "{err:?}");
        assert!(err.to_string().contains("git no-such-subcommand failed"));
    }

    #[test]
    fn with_env_is_passed_to_child() {
        let (_dir, git) = init_repo();
        let git = git.with_env(&[("GIT_DIR".into(), "/nonexistent".into())]);
        let out = git.run(&["rev-parse", "--git-dir"]).unwrap();
        assert!(!out.success, "{out:?}");
    }

    #[test]
    fn stderr_is_in_english() {
        let (_dir, git) = init_repo();
        let out = git.run(&["rev-parse", "HEAD"]).unwrap();
        assert!(!out.success);
        assert!(
            out.stderr.contains("unknown revision") || out.stderr.contains("ambiguous argument"),
            "{}",
            out.stderr
        );
    }

    #[test]
    fn classifies_stderr() {
        let cases = [
            (
                "remote: Invalid username or password.\nfatal: Authentication failed for 'https://gist.github.com/abc.git/'",
                GitFailure::Auth,
            ),
            (
                "git@gist.github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.",
                GitFailure::Auth,
            ),
            (
                "fatal: could not read Username for 'https://gist.github.com': terminal prompts disabled",
                GitFailure::Auth,
            ),
            (
                " ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs to 'x'\nhint: Updates were rejected because the remote contains work that you do not have locally.",
                GitFailure::NonFastForward,
            ),
            (
                " ! [rejected]        main -> main (non-fast-forward)",
                GitFailure::NonFastForward,
            ),
            (
                "CONFLICT (content): Merge conflict in 20-architecture.md\nerror: could not apply 1234567... edit",
                GitFailure::Conflict,
            ),
            (
                "remote: Repository not found.\nfatal: repository 'https://gist.github.com/x.git/' not found",
                GitFailure::NotFound,
            ),
            (
                "fatal: '/tmp/nope' does not appear to be a git repository",
                GitFailure::NotFound,
            ),
            (
                "fatal: unable to access 'https://gist.github.com/x.git/': Could not resolve host: gist.github.com",
                GitFailure::Network,
            ),
            ("fatal: something else", GitFailure::Other),
        ];
        for (stderr, expected) in cases {
            assert_eq!(classify(stderr), expected, "{stderr}");
        }
    }

    #[test]
    fn failure_to_error_maps_exit_codes() {
        let msg = || "m".to_string();
        assert_eq!(failure_to_error(GitFailure::Auth, msg()).exit_code(), 3);
        assert_eq!(
            failure_to_error(GitFailure::NonFastForward, msg()).exit_code(),
            5
        );
        assert_eq!(failure_to_error(GitFailure::NotFound, msg()).exit_code(), 5);
        assert_eq!(failure_to_error(GitFailure::Network, msg()).exit_code(), 5);
        assert_eq!(failure_to_error(GitFailure::Conflict, msg()).exit_code(), 2);
        assert_eq!(failure_to_error(GitFailure::Other, msg()).exit_code(), 1);
    }

    #[test]
    fn run_checked_classifies_missing_remote() {
        let (_dir, git) = init_repo();
        let err = git
            .run_checked(&["ls-remote", "/nonexistent/ctx-sync-remote"])
            .unwrap_err();
        assert_eq!(err.exit_code(), 5, "{err}");
    }

    #[test]
    fn missing_cwd_is_reported() {
        let err = Git::new("/nonexistent/ctx-sync-test")
            .run(&["status"])
            .unwrap_err();
        assert!(err.to_string().contains("working directory does not exist"));
    }
}
