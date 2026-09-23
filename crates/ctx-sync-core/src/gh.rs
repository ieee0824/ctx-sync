//! Thin wrapper around the GitHub CLI (`gh`).
//!
//! `gh` is optional. It is only used to create a new Gist; syncing and
//! attaching to an existing Gist use plain `git`.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::gist_id::parse_gist_id;
use crate::{Error, Result};

fn gh() -> Command {
    let mut cmd = Command::new("gh");
    cmd.stdin(Stdio::null())
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1");
    cmd
}

fn not_found() -> Error {
    Error::InvalidConfig(
        "gh command not found; create a gist manually and run `ctx-sync init --gist-id <id>`"
            .into(),
    )
}

/// Whether `gh` is installed.
pub fn is_available() -> bool {
    gh().arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether `gh` is logged in to github.com. Returns `Ok(false)` when `gh` is missing.
pub fn is_authenticated() -> Result<bool> {
    match gh()
        .args(["auth", "status", "--hostname", "github.com"])
        .output()
    {
        Ok(out) => Ok(out.status.success()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::Io(e)),
    }
}

/// Creates a Gist with `gh gist create` and returns its ID.
pub fn gist_create(files: &[PathBuf], public: bool, description: &str) -> Result<String> {
    let mut cmd = gh();
    cmd.args(["gist", "create", "--desc", description]);
    if public {
        cmd.arg("--public");
    }
    cmd.args(files);
    let out = cmd.output().map_err(|e| match e.kind() {
        ErrorKind::NotFound => not_found(),
        _ => Error::Io(e),
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let message = format!("gh gist create failed: {stderr}");
        return Err(if stderr.to_lowercase().contains("auth") {
            Error::Auth(message)
        } else {
            Error::Remote(message)
        });
    }
    parse_gist_create_output(&String::from_utf8_lossy(&out.stdout))
}

/// Extracts the Gist ID from the output of `gh gist create`.
pub fn parse_gist_create_output(stdout: &str) -> Result<String> {
    let url = stdout
        .lines()
        .map(str::trim)
        .rfind(|l| l.starts_with("https://"))
        .ok_or_else(|| Error::Remote(format!("gist URL not found in gh output: {stdout:?}")))?;
    parse_gist_id(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_url_only_output() {
        assert_eq!(
            parse_gist_create_output("https://gist.github.com/ieee0824/abc123\n").unwrap(),
            "abc123"
        );
    }

    #[test]
    fn parses_output_with_status_lines() {
        let out =
            "- Creating gist...\n✓ Created secret gist\nhttps://gist.github.com/ieee0824/abc123\n";
        assert_eq!(parse_gist_create_output(out).unwrap(), "abc123");
    }

    #[test]
    fn empty_output_is_an_error() {
        let err = parse_gist_create_output("").unwrap_err();
        assert_eq!(err.exit_code(), 5, "{err}");
    }
}
