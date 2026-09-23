//! Git information of the *project* repository (not the context repository),
//! recorded in the worker file by `handoff`.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::Result;
use crate::git::Git;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RepoInfo {
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    /// Short hash of HEAD, `None` without commits.
    pub commit: Option<String>,
    /// Uncommitted files plus files changed since the merge base with
    /// `origin/HEAD`, sorted and deduplicated.
    pub changed_files: Vec<String>,
}

/// Returns `RepoInfo::default()` when `project_root` is not a git repository.
pub fn collect(project_root: &Path, git_env: &[(String, String)]) -> Result<RepoInfo> {
    let git = Git::new(project_root).with_env(git_env);
    let inside = git.run(&["rev-parse", "--is-inside-work-tree"])?;
    if !inside.success || inside.stdout.trim() != "true" {
        return Ok(RepoInfo::default());
    }

    let branch = git.run(&["branch", "--show-current"])?;
    let branch = Some(branch.stdout.trim().to_string()).filter(|b| branch.success && !b.is_empty());
    let head = git.run(&["rev-parse", "--short", "HEAD"])?;
    let commit = head.success.then(|| head.stdout.trim().to_string());

    let mut changed = BTreeSet::new();
    let status = git.run_checked(&[
        "-c",
        "core.quotepath=false",
        "status",
        "--porcelain",
        "--untracked-files=all",
    ])?;
    for line in status.lines() {
        let Some(path) = line.get(3..) else { continue };
        let path = path.rsplit(" -> ").next().unwrap_or(path);
        changed.insert(unquote(path));
    }

    let has_upstream = git
        .run(&[
            "rev-parse",
            "--verify",
            "--quiet",
            "refs/remotes/origin/HEAD",
        ])?
        .success;
    if has_upstream && commit.is_some() {
        let base = git.run(&["merge-base", "HEAD", "origin/HEAD"])?;
        if base.success {
            let range = format!("{}...HEAD", base.stdout.trim());
            let diff =
                git.run_checked(&["-c", "core.quotepath=false", "diff", "--name-only", &range])?;
            changed.extend(diff.lines().filter(|l| !l.is_empty()).map(unquote));
        }
    }

    Ok(RepoInfo {
        branch,
        commit,
        changed_files: changed.into_iter().collect(),
    })
}

/// Git quotes paths with special characters: `"a b.txt"`.
fn unquote(path: &str) -> String {
    let path = path.trim();
    path.strip_prefix('"')
        .and_then(|p| p.strip_suffix('"'))
        .unwrap_or(path)
        .to_string()
}
