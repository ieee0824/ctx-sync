//! `bootstrap --apply`: put the collected candidates into `10-project.md`.
//!
//! Only the project document is touched: inferred findings never become
//! decisions.

use serde::Serialize;

use super::Workspace;
use crate::bootstrap::BootstrapReport;
use crate::model::MdDoc;
use crate::store::ContextStore;
use crate::{Error, Result};

const SECTION: &str = "Initial Context";

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapApplyOutcome {
    pub file_name: String,
    pub committed: Option<String>,
}

/// Adds an `## Initial Context` section to the configured project file and commits it
/// (without syncing). An existing section is never overwritten.
pub fn apply_bootstrap(ws: &Workspace, report: &BootstrapReport) -> Result<BootstrapApplyOutcome> {
    ws.store.ensure()?;
    let project_file = &ws.config.context.project;
    let mut doc = match ws.store.read_file(project_file)? {
        Some(text) => MdDoc::parse(&text)?,
        None => MdDoc {
            title: "Project".into(),
            ..Default::default()
        },
    };
    if doc.section(SECTION).is_some() {
        return Err(Error::General(format!(
            "{project_file} already has an {SECTION} section"
        )));
    }
    doc.set_section(SECTION, &report.render_section_body());
    ws.store.write_file(project_file, &doc.render())?;
    let committed = ws.store.commit("ctx-sync: bootstrap")?;
    Ok(BootstrapApplyOutcome {
        file_name: project_file.clone(),
        committed,
    })
}
