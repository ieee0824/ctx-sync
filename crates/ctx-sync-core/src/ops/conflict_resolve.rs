//! Apply an explicit resolution to a recorded context conflict.

use std::path::PathBuf;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use super::Workspace;
use crate::state::ConflictRecord;
use crate::store::gist::RebaseStrategy;
use crate::store::{ContextStore, SyncOutcome};
use crate::{Error, Result};

pub enum Resolution {
    KeepLocal,
    KeepRemote,
    /// (context file name, path to the merged contents).
    Merged(Vec<(String, PathBuf)>),
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolveOutcome {
    pub resolution: String,
    pub files: Vec<String>,
    pub sync: SyncOutcome,
}

pub fn conflict_resolve(
    ws: &Workspace,
    resolution: Resolution,
    now: DateTime<FixedOffset>,
) -> Result<ResolveOutcome> {
    let record = ConflictRecord::load(ws.store.conflict_path())?
        .ok_or_else(|| Error::General("no context conflict recorded".into()))?;
    let files = record.files;
    let (kind, strategy, merged) = match resolution {
        Resolution::KeepLocal => ("keep-local", RebaseStrategy::KeepLocal, Vec::new()),
        Resolution::KeepRemote => ("keep-remote", RebaseStrategy::KeepRemote, Vec::new()),
        Resolution::Merged(paths) => {
            let mut contents = Vec::with_capacity(paths.len());
            for (file, path) in paths {
                if !files.contains(&file) {
                    return Err(Error::General(format!("{file} is not in conflict")));
                }
                contents.push((file, std::fs::read_to_string(path)?));
            }
            ("merged", RebaseStrategy::KeepRemote, contents)
        }
    };
    ws.store.rebase_with_strategy(strategy)?;
    if !merged.is_empty() {
        for (file, content) in &merged {
            ws.store.write_file(file, content)?;
        }
        ws.store.commit(&format!(
            "ctx-sync: resolve conflict in {}",
            files.join(", ")
        ))?;
    }
    let sync = ws.store.sync(now)?;
    Ok(ResolveOutcome {
        resolution: kind.into(),
        files,
        sync,
    })
}
