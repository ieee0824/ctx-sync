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
    Error::General(format!(
        "git {} failed: {}",
        args.join(" "),
        out.stderr.trim()
    ))
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
    fn missing_cwd_is_reported() {
        let err = Git::new("/nonexistent/ctx-sync-test")
            .run(&["status"])
            .unwrap_err();
        assert!(err.to_string().contains("working directory does not exist"));
    }
}
