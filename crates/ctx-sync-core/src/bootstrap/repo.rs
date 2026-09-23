//! Findings from git history and the directory structure, plus the questions
//! bootstrap cannot answer.

use std::path::Path;

use super::Finding;
use crate::git::Git;

const MAX_COMMITS: usize = 10;
const IGNORED_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "build",
    "vendor",
    "__pycache__",
];

/// Subjects of the latest commits. Empty outside git or without commits.
pub fn collect_git_log(root: &Path, git_env: &[(String, String)]) -> Vec<Finding> {
    let git = Git::new(root).with_env(git_env);
    let Ok(out) = git.run(&["log", "-n", "30", "--pretty=format:%s"]) else {
        return Vec::new();
    };
    if !out.success {
        return Vec::new();
    }
    let subjects: Vec<&str> = out
        .stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(MAX_COMMITS)
        .collect();
    if subjects.is_empty() {
        return Vec::new();
    }
    vec![Finding::confirmed(
        "Recent commits",
        &subjects.join("; "),
        Some("git log"),
    )]
}

/// Top-level directories and their subdirectories, e.g.
/// `src/, docs/, crates/ (a, b)`.
pub fn collect_tree(root: &Path) -> Vec<Finding> {
    let entries: Vec<String> = subdirs(root)
        .into_iter()
        .map(|dir| {
            let children = subdirs(&root.join(&dir));
            if children.is_empty() {
                format!("{dir}/")
            } else {
                format!("{dir}/ ({})", children.join(", "))
            }
        })
        .collect();
    if entries.is_empty() {
        return Vec::new();
    }
    vec![Finding::confirmed(
        "Top-level structure",
        &entries.join(", "),
        Some("repository structure"),
    )]
}

/// What rules cannot tell; always the same list.
pub fn unknown_questions() -> Vec<Finding> {
    vec![
        Finding::unknown("Goal", "The project goal has not been confirmed."),
        Finding::unknown("Non Goals", "Non goals have not been defined."),
        Finding::unknown(
            "Architecture",
            "Key architectural constraints have not been confirmed.",
        ),
        Finding::unknown(
            "Roadmap",
            "Whether other backends, platforms or integrations are planned is unknown.",
        ),
    ]
}

/// Visible subdirectory names, sorted.
fn subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|name| !name.starts_with('.') && !IGNORED_DIRS.contains(&name.as_str()))
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::Certainty;

    #[test]
    fn tree_lists_visible_directories() {
        let dir = tempfile::tempdir().unwrap();
        for sub in [
            "src",
            "target/debug",
            "crates/b",
            "crates/a",
            ".git",
            "node_modules/x",
        ] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        std::fs::write(dir.path().join("README.md"), "x").unwrap();
        let findings = collect_tree(dir.path());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].text, "crates/ (a, b), src/");
        assert_eq!(findings[0].certainty, Certainty::Confirmed);
    }

    #[test]
    fn empty_tree_has_no_finding() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect_tree(dir.path()).is_empty());
    }

    #[test]
    fn unknown_questions_are_all_unknown() {
        let questions = unknown_questions();
        assert_eq!(questions.len(), 4);
        assert!(questions.iter().all(|q| q.certainty == Certainty::Unknown));
    }

    #[test]
    fn git_log_outside_a_repository_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let env = ctx_sync_testutil::git_env(&dir.path().join("config"));
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        assert!(collect_git_log(&project, &env).is_empty());
    }

    #[test]
    fn git_log_lists_recent_subjects() {
        let remote = ctx_sync_testutil::TestRemote::new();
        let worker = ctx_sync_testutil::TestWorker::new(&remote);
        for i in 0..12 {
            std::fs::write(worker.project().join(format!("f{i}")), "x").unwrap();
            worker.git(&["add", "-A"]);
            worker.git(&["commit", "-q", "-m", &format!("change {i}")]);
        }
        let findings = collect_git_log(worker.project(), &worker.git_env());
        assert_eq!(findings.len(), 1);
        let subjects: Vec<&str> = findings[0].text.split("; ").collect();
        assert_eq!(subjects.len(), 10);
        assert_eq!(subjects[0], "change 11");
        assert_eq!(findings[0].source.as_deref(), Some("git log"));
    }
}
