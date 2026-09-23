//! `bootstrap`: rule-based collection of initial context candidates from an
//! existing repository. No LLM is involved, and nothing collected here is
//! turned into a decision.

use std::path::Path;

mod docs;
mod manifest;
mod repo;
mod report;

pub use docs::collect_docs;
pub use manifest::{collect_manifests, infer_from_dependency};
pub use repo::{collect_git_log, collect_tree, unknown_questions};
pub use report::{BootstrapReport, Certainty, Finding};

/// Every collector, in order: manifests, docs, tree, git log, unknowns.
pub fn collect(root: &Path, git_env: &[(String, String)]) -> BootstrapReport {
    let mut report = BootstrapReport::default();
    report.extend(collect_manifests(root));
    report.extend(collect_docs(root));
    report.extend(collect_tree(root));
    report.extend(collect_git_log(root, git_env));
    report.extend(unknown_questions());
    report
}
