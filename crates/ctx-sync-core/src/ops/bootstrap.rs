//! `bootstrap --apply`: put the collected candidates into `10-project.md`.
//!
//! Only the project document is touched: inferred findings never become
//! decisions.

use serde::Serialize;

use super::Workspace;
use crate::bootstrap::BootstrapReport;
use crate::model::{MdDoc, PROJECT_FILE};
use crate::store::ContextStore;
use crate::{Error, Result};

const SECTION: &str = "Initial Context";

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapApplyOutcome {
    pub file_name: String,
    pub committed: Option<String>,
}

/// Adds an `## Initial Context` section to `10-project.md` and commits it
/// (without syncing). An existing section is never overwritten.
pub fn apply_bootstrap(ws: &Workspace, report: &BootstrapReport) -> Result<BootstrapApplyOutcome> {
    ws.store.ensure()?;
    let mut doc = match ws.store.read_file(PROJECT_FILE)? {
        Some(text) => MdDoc::parse(&text)?,
        None => MdDoc {
            title: "Project".into(),
            ..Default::default()
        },
    };
    if doc.section(SECTION).is_some() {
        return Err(Error::General(format!(
            "{PROJECT_FILE} already has an {SECTION} section"
        )));
    }
    doc.set_section(SECTION, &report.render_section_body());
    ws.store.write_file(PROJECT_FILE, &doc.render())?;
    let committed = ws.store.commit("ctx-sync: bootstrap")?;
    Ok(BootstrapApplyOutcome {
        file_name: PROJECT_FILE.to_string(),
        committed,
    })
}
