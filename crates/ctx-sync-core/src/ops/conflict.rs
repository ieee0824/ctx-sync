//! Inspect a recorded sync conflict without changing the context repository.

use super::Workspace;
use crate::state::ConflictRecord;
use crate::view::{ConflictFile, ConflictView};
use crate::{Error, Result};

pub fn conflict_show(ws: &Workspace) -> Result<ConflictView> {
    let Some(record) = ConflictRecord::load(ws.store.conflict_path())? else {
        return Ok(ConflictView {
            record: None,
            files: Vec::new(),
        });
    };
    let git = ws.store.git();
    for head in [&record.remote_head, &record.local_head] {
        let commit = format!("{head}^{{commit}}");
        if !git.run(&["cat-file", "-e", &commit])?.success {
            return Err(Error::General(format!(
                "conflict commit {head} not found; run `ctx-sync pull`"
            )));
        }
    }
    let mut files = Vec::with_capacity(record.files.len());
    for file in &record.files {
        let show = |head: &str| {
            let object = format!("{head}:{file}");
            let output = git.run(&["show", &object])?;
            Ok::<_, Error>(output.success.then_some(output.stdout))
        };
        let remote = show(&record.remote_head)?;
        let local = show(&record.local_head)?;
        let diff = git.run_checked(&[
            "diff",
            "--no-color",
            &record.remote_head,
            &record.local_head,
            "--",
            file,
        ])?;
        files.push(ConflictFile {
            file: file.clone(),
            remote,
            local,
            diff,
        });
    }
    Ok(ConflictView {
        record: Some(record),
        files,
    })
}
