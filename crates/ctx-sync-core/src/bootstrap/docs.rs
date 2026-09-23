//! Findings from documentation: README, agent instructions, docs/**.

use std::path::{Path, PathBuf};

use super::Finding;

const MAX_SUMMARY_CHARS: usize = 200;
const MAX_HEADINGS: usize = 10;
const MAX_DOC_FILES: usize = 20;
const MAX_DOC_DEPTH: usize = 3;

pub fn collect_docs(root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    match read(&root.join("README.md")) {
        Some(text) => findings.extend(readme(&text)),
        None => findings.push(Finding::unknown(
            "Purpose",
            "No README found; the project purpose is unknown.",
        )),
    }
    for file in ["AGENTS.md", "CLAUDE.md", "CONTRIBUTING.md"] {
        if let Some(text) = read(&root.join(file)) {
            let headings: Vec<&str> = headings(&text, "## ").take(MAX_HEADINGS).collect();
            let text = if headings.is_empty() {
                file.to_string()
            } else {
                format!("{file} (headings: {})", headings.join(", "))
            };
            findings.push(Finding::confirmed("Document", &text, Some(file)));
        }
    }
    let mut docs = Vec::new();
    markdown_files(&root.join("docs"), 1, &mut docs);
    docs.sort();
    for path in docs.into_iter().take(MAX_DOC_FILES) {
        let Some(text) = read(&path) else { continue };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = match headings(&text, "# ").next() {
            Some(title) => format!("{relative} — {title}"),
            None => relative.clone(),
        };
        findings.push(Finding::confirmed("Docs", &text, Some(&relative)));
    }
    findings
}

/// Title and first paragraph of the README.
fn readme(text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let Some(title_index) = lines.iter().position(|l| l.starts_with("# ")) else {
        return findings;
    };
    let title = lines[title_index][2..].trim();
    findings.push(Finding::confirmed("Title", title, Some("README.md")));

    let paragraph: Vec<&str> = lines[title_index + 1..]
        .iter()
        .map(|l| l.trim())
        .skip_while(|l| l.is_empty())
        .take_while(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if !paragraph.is_empty() {
        let summary = shorten(&paragraph.join(" "));
        findings.push(Finding::confirmed(
            "README summary",
            &summary,
            Some("README.md"),
        ));
    }
    findings
}

/// Headings with the given prefix (`"# "`, `"## "`), outside code fences.
fn headings<'a>(text: &'a str, prefix: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    let mut in_fence = false;
    text.lines().filter_map(move |line| {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            return None;
        }
        if in_fence {
            return None;
        }
        line.strip_prefix(prefix).map(str::trim)
    })
}

fn markdown_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DOC_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        if hidden {
            continue;
        }
        if path.is_dir() {
            markdown_files(&path, depth + 1, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Unreadable or non-UTF-8 files are skipped.
fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_SUMMARY_CHARS {
        text.to_string()
    } else {
        let mut short: String = text.chars().take(MAX_SUMMARY_CHARS).collect();
        short.push('…');
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::Certainty;

    fn write(root: &Path, file: &str, text: &str) {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn lines(findings: &[Finding]) -> Vec<String> {
        findings
            .iter()
            .map(|f| format!("{:?} {}: {}", f.certainty, f.topic, f.text))
            .collect()
    }

    #[test]
    fn readme_title_and_summary() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "README.md",
            "# ctx-sync\n\nSync shared context\nbetween agents.\n\n## Install\n\ncargo install\n",
        );
        assert_eq!(
            lines(&collect_docs(dir.path())),
            [
                "Confirmed Title: ctx-sync",
                "Confirmed README summary: Sync shared context between agents.",
            ]
        );
    }

    #[test]
    fn long_summary_is_shortened() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "README.md",
            &format!("# T\n\n{}\n", "word ".repeat(60)),
        );
        let findings = collect_docs(dir.path());
        let summary = &findings[1].text;
        assert_eq!(summary.chars().count(), 201);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn agent_documents_and_docs_tree() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "README.md", "# T\n");
        write(
            dir.path(),
            "AGENTS.md",
            "# Agents\n\n## Setup\n\n```\n## not a heading\n```\n\n## Testing\n",
        );
        write(dir.path(), "docs/a.md", "# Architecture\n");
        write(dir.path(), "docs/sub/b.md", "no heading\n");
        write(dir.path(), "docs/sub/c.txt", "ignored\n");
        let findings = collect_docs(dir.path());
        assert_eq!(
            lines(&findings),
            [
                "Confirmed Title: T",
                "Confirmed Document: AGENTS.md (headings: Setup, Testing)",
                "Confirmed Docs: docs/a.md — Architecture",
                "Confirmed Docs: docs/sub/b.md",
            ]
        );
        assert_eq!(findings[3].source.as_deref(), Some("docs/sub/b.md"));
    }

    #[test]
    fn missing_readme_is_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let findings = collect_docs(dir.path());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].certainty, Certainty::Unknown);
        assert_eq!(findings[0].topic, "Purpose");
    }

    #[test]
    fn non_utf8_files_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "README.md", "# T\n");
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/bad.md"), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(lines(&collect_docs(dir.path())), ["Confirmed Title: T"]);
    }
}
