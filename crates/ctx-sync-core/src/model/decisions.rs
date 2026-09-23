//! Calculations over the set of decisions.
//!
//! Supersession is derived here instead of rewriting the old decision's
//! `Status`, because decision files are append-only.

use std::collections::{BTreeMap, HashSet};

use chrono::NaiveDate;

use super::{Decision, DecisionId, DecisionStatus};

/// Accepted decisions that no other decision supersedes, sorted by id.
pub fn effective_decisions(decisions: &[Decision]) -> Vec<&Decision> {
    let superseded: HashSet<&DecisionId> =
        decisions.iter().flat_map(|d| d.supersedes.iter()).collect();
    let mut effective: Vec<&Decision> = decisions
        .iter()
        .filter(|d| d.status == DecisionStatus::Accepted && !superseded.contains(&d.id))
        .collect();
    effective.sort_by(|a, b| a.id.cmp(&b.id));
    effective
}

/// Next sequence number for `date`: the largest existing one plus 1 (1 when
/// there is none).
///
/// Looking at every existing ID of the date means a new ID never collides
/// with one that is already known locally. Duplicates created concurrently
/// by other workers are reported by [`duplicate_decision_seqs`].
pub fn next_decision_seq(decisions: &[Decision], date: NaiveDate) -> u32 {
    let date = date.format("%Y%m%d").to_string();
    decisions
        .iter()
        .filter(|d| d.id.date_str() == date)
        .map(|d| d.id.seq())
        .max()
        .map_or(1, |max| max + 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateSeq {
    pub date: String,
    pub seq: u32,
    pub ids: Vec<DecisionId>,
}

/// Groups of decisions sharing the same (date, seq).
///
/// This happens when two workers add a decision on the same day at the same
/// time. v0.1 only warns about it.
pub fn duplicate_decision_seqs(decisions: &[Decision]) -> Vec<DuplicateSeq> {
    let mut groups: BTreeMap<(String, u32), Vec<DecisionId>> = BTreeMap::new();
    for d in decisions {
        groups
            .entry((d.id.date_str().to_string(), d.id.seq()))
            .or_default()
            .push(d.id.clone());
    }
    groups
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|((date, seq), mut ids)| {
            ids.sort();
            DuplicateSeq { date, seq, ids }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, d).unwrap()
    }

    fn decision(id: &str, status: DecisionStatus, supersedes: &[&str]) -> Decision {
        let id = DecisionId::parse(id).unwrap();
        Decision {
            date: NaiveDate::parse_from_str(id.date_str(), "%Y%m%d").unwrap(),
            id,
            title: "t".into(),
            status,
            author: None,
            supersedes: supersedes
                .iter()
                .map(|s| DecisionId::parse(s).unwrap())
                .collect(),
            context: String::new(),
            decision: String::new(),
            reason: String::new(),
            consequences: String::new(),
            extra_sections: vec![],
        }
    }

    fn ids(decisions: Vec<&Decision>) -> Vec<&str> {
        decisions.into_iter().map(|d| d.id.as_str()).collect()
    }

    #[test]
    fn superseded_decisions_are_not_effective() {
        let decisions = [
            decision(
                "20260923-002-b",
                DecisionStatus::Accepted,
                &["20260923-001-a"],
            ),
            decision("20260923-001-a", DecisionStatus::Accepted, &[]),
        ];
        assert_eq!(ids(effective_decisions(&decisions)), ["20260923-002-b"]);
    }

    #[test]
    fn only_accepted_decisions_are_effective() {
        let decisions = [
            decision("20260923-001-a", DecisionStatus::Proposed, &[]),
            decision("20260923-002-b", DecisionStatus::Rejected, &[]),
            decision("20260923-003-c", DecisionStatus::Superseded, &[]),
            decision("20260923-005-e", DecisionStatus::Accepted, &[]),
            decision("20260923-004-d", DecisionStatus::Accepted, &[]),
        ];
        assert_eq!(
            ids(effective_decisions(&decisions)),
            ["20260923-004-d", "20260923-005-e"]
        );
    }

    #[test]
    fn next_seq_follows_the_largest_of_the_day() {
        let decisions = [
            decision("20260923-001-a", DecisionStatus::Accepted, &[]),
            decision("20260923-002-b", DecisionStatus::Accepted, &[]),
            decision("20260922-007-c", DecisionStatus::Accepted, &[]),
        ];
        assert_eq!(next_decision_seq(&decisions, day(23)), 3);
        assert_eq!(next_decision_seq(&decisions, day(24)), 1);
        assert_eq!(next_decision_seq(&[], day(23)), 1);
    }

    #[test]
    fn next_seq_does_not_fill_gaps() {
        let decisions = [
            decision("20260923-001-a", DecisionStatus::Accepted, &[]),
            decision("20260923-003-c", DecisionStatus::Accepted, &[]),
        ];
        assert_eq!(next_decision_seq(&decisions, day(23)), 4);
    }

    #[test]
    fn detects_duplicate_sequence_numbers() {
        let decisions = [
            decision("20260923-001-b", DecisionStatus::Accepted, &[]),
            decision("20260923-001-a", DecisionStatus::Accepted, &[]),
            decision("20260923-002-c", DecisionStatus::Accepted, &[]),
        ];
        assert_eq!(
            duplicate_decision_seqs(&decisions),
            vec![DuplicateSeq {
                date: "20260923".into(),
                seq: 1,
                ids: vec![
                    DecisionId::parse("20260923-001-a").unwrap(),
                    DecisionId::parse("20260923-001-b").unwrap(),
                ],
            }]
        );
        assert!(duplicate_decision_seqs(&decisions[1..]).is_empty());
    }
}
