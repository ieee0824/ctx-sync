//! `bootstrap`: rule-based collection of initial context candidates from an
//! existing repository. No LLM is involved, and nothing collected here is
//! turned into a decision.

mod report;

pub use report::{BootstrapReport, Certainty, Finding};
