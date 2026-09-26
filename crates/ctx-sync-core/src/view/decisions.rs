//! Compact list of decisions and their supersession relationships.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;

use crate::model::{ContextSnapshot, DecisionStatus};

#[derive(Debug, Clone, Serialize)]
pub struct DecisionListItem {
    pub id: String,
    pub title: String,
    pub status: DecisionStatus,
    pub date: NaiveDate,
    /// Accepted and not superseded by another decision.
    pub effective: bool,
    /// IDs of decisions that supersede this one, in ascending order.
    pub superseded_by: Vec<String>,
    pub supersedes: Vec<String>,
    pub author: Option<String>,
}

/// Build a list in ascending ID order. By default, include effective decisions only.
pub fn build_decision_list(snapshot: &ContextSnapshot, include_all: bool) -> Vec<DecisionListItem> {
    let mut superseded_by: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for decision in &snapshot.decisions {
        for old in &decision.supersedes {
            superseded_by
                .entry(old.as_str())
                .or_default()
                .push(decision.id.as_str().to_string());
        }
    }
    for ids in superseded_by.values_mut() {
        ids.sort();
    }

    let mut items: Vec<_> = snapshot
        .decisions
        .iter()
        .filter_map(|decision| {
            let superseded_by = superseded_by
                .get(decision.id.as_str())
                .cloned()
                .unwrap_or_default();
            let effective = decision.status == DecisionStatus::Accepted && superseded_by.is_empty();
            if !include_all && !effective {
                return None;
            }
            Some(DecisionListItem {
                id: decision.id.as_str().to_string(),
                title: decision.title.clone(),
                status: decision.status,
                date: decision.date,
                effective,
                superseded_by,
                supersedes: decision
                    .supersedes
                    .iter()
                    .map(|id| id.as_str().to_string())
                    .collect(),
                author: decision.author.clone(),
            })
        })
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

/// Render one decision per line, or `No decisions.` when empty.
pub fn render_decision_list(items: &[DecisionListItem]) -> String {
    if items.is_empty() {
        return "No decisions.\n".to_string();
    }
    let mut output = String::new();
    for item in items {
        let status = if item.superseded_by.is_empty() {
            item.status.to_string()
        } else {
            format!("superseded by {}", item.superseded_by.join(", "))
        };
        output.push_str(&format!("{}  {status}  {}\n", item.id, item.title));
    }
    output
}
