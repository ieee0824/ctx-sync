//! The recorded sides and diff of a context conflict.

use serde::Serialize;

use crate::state::ConflictRecord;

#[derive(Debug, Clone, Serialize)]
pub struct ConflictFile {
    pub file: String,
    /// Content at the remote head, if that file exists there.
    pub remote: Option<String>,
    /// Content at the local head, if that file exists there.
    pub local: Option<String>,
    /// `git diff --no-color <remote_head> <local_head> -- <file>`.
    pub diff: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictView {
    pub record: Option<ConflictRecord>,
    pub files: Vec<ConflictFile>,
}

pub fn render_conflict_text(view: &ConflictView) -> String {
    let Some(record) = &view.record else {
        return "No context conflict recorded.\n".to_string();
    };
    let mut sections = vec![
        format!(
            "Context conflict (detected at {})",
            record.detected_at.to_rfc3339()
        ),
        format!(
            "remote: {}  local: {}",
            record.remote_head.chars().take(7).collect::<String>(),
            record.local_head.chars().take(7).collect::<String>()
        ),
    ];
    for file in &view.files {
        sections.push(format!("## {}\n\n{}", file.file, file.diff.trim_end()));
    }
    let mut rendered = sections.join("\n\n");
    rendered.push('\n');
    rendered
}
