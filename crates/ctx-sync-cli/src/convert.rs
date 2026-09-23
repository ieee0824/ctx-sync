//! Conversion of CLI arguments into core types.

use ctx_sync_core::model::WorkerStatus;
use ctx_sync_core::ops::HandoffInput;

use crate::cli::{HandoffFields, WorkerStatusArg};

pub fn worker_status(arg: WorkerStatusArg) -> WorkerStatus {
    match arg {
        WorkerStatusArg::Working => WorkerStatus::Working,
        WorkerStatusArg::Blocked => WorkerStatus::Blocked,
        WorkerStatusArg::Done => WorkerStatus::Done,
        WorkerStatusArg::Abandoned => WorkerStatus::Abandoned,
    }
}

/// An empty list argument means "not given".
fn list(values: &[String]) -> Option<Vec<String>> {
    (!values.is_empty()).then(|| values.to_vec())
}

pub fn handoff_input(fields: &HandoffFields) -> HandoffInput {
    HandoffInput {
        task: fields.task.clone(),
        summary: fields.summary.clone(),
        status: fields.status.map(worker_status),
        working_on: list(&fields.working_on),
        changed: list(&fields.changed),
        interface_changes: list(&fields.interface_change),
        attention: list(&fields.attention),
        blocked_by: list(&fields.blocked_by),
        append: fields.append,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_lists_are_not_given() {
        let input = handoff_input(&HandoffFields {
            attention: vec!["a".into()],
            status: Some(WorkerStatusArg::Blocked),
            ..Default::default()
        });
        assert_eq!(input.attention, Some(vec!["a".to_string()]));
        assert_eq!(input.changed, None);
        assert_eq!(input.status, Some(WorkerStatus::Blocked));
        assert!(!input.append);
    }
}
