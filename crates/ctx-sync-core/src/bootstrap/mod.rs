//! `bootstrap`: rule-based collection of initial context candidates from an
//! existing repository. No LLM is involved, and nothing collected here is
//! turned into a decision.

mod docs;
mod manifest;
mod repo;
mod report;

pub use docs::collect_docs;
pub use manifest::{collect_manifests, infer_from_dependency};
pub use repo::{collect_git_log, collect_tree, unknown_questions};
pub use report::{BootstrapReport, Certainty, Finding};
