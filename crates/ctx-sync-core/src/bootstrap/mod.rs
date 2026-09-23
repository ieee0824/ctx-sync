//! `bootstrap`: rule-based collection of initial context candidates from an
//! existing repository. No LLM is involved, and nothing collected here is
//! turned into a decision.

mod manifest;
mod report;

pub use manifest::{collect_manifests, infer_from_dependency};
pub use report::{BootstrapReport, Certainty, Finding};
