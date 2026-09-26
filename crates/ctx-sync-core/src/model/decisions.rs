//! Calculations over the set of decisions.
//!
//! Supersession is derived here instead of rewriting the old decision's
//! `Status`, because decision files are append-only.

use std::collections::{BTreeMap, HashMap, HashSet};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renumber {
    pub from: DecisionId,
    pub to: DecisionId,
}

/// Plan new IDs for local-only decisions whose date and sequence collide.
pub fn plan_renumber(decisions: &[Decision], local_only: &HashSet<DecisionId>) -> Vec<Renumber> {
    let mut maximums: BTreeMap<&str, u32> = BTreeMap::new();
    for decision in decisions {
        let maximum = maximums.entry(decision.id.date_str()).or_default();
        *maximum = (*maximum).max(decision.id.seq());
    }

    let mut plan = Vec::new();
    for group in duplicate_decision_seqs(decisions) {
        let has_remote = group.ids.iter().any(|id| !local_only.contains(id));
        for (index, from) in group.ids.iter().enumerate() {
            if !local_only.contains(from) || (!has_remote && index == 0) {
                continue;
            }
            let maximum = maximums
                .get_mut(group.date.as_str())
                .expect("duplicate group has an existing date");
            *maximum += 1;
            let slug = from
                .as_str()
                .splitn(3, '-')
                .nth(2)
                .expect("decision ID has a slug");
            let to = DecisionId::parse(&format!("{}-{:03}-{slug}", group.date, *maximum))
                .expect("renumbered decision ID is valid");
            plan.push(Renumber {
                from: from.clone(),
                to,
            });
        }
    }
    plan.sort_by(|a, b| a.from.cmp(&b.from));
    plan
}

/// Return a copy with its new ID and superseded IDs rewritten through `map`.
pub fn renumber_decision(
    decision: &Decision,
    to: &DecisionId,
    map: &HashMap<DecisionId, DecisionId>,
) -> Decision {
    let mut changed = decision.clone();
    changed.id = to.clone();
    changed.supersedes = decision
        .supersedes
        .iter()
        .map(|id| map.get(id).unwrap_or(id).clone())
        .collect();
    changed
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

    #[test]
    fn plans_remote_collisions_after_the_largest_used_sequence() {
        let decisions = [
            decision("20260923-001-a", DecisionStatus::Accepted, &[]),
            decision("20260923-001-b", DecisionStatus::Accepted, &[]),
            decision("20260923-002-c", DecisionStatus::Accepted, &[]),
        ];
        let local = HashSet::from([decisions[1].id.clone(), decisions[2].id.clone()]);
        assert_eq!(
            plan_renumber(&decisions, &local),
            vec![Renumber {
                from: decisions[1].id.clone(),
                to: DecisionId::parse("20260923-003-b").unwrap(),
            }]
        );
    }

    #[test]
    fn retains_the_first_local_decision_in_an_all_local_group() {
        let decisions = [
            decision("20260923-001-b", DecisionStatus::Accepted, &[]),
            decision("20260923-001-a", DecisionStatus::Accepted, &[]),
        ];
        let local = decisions.iter().map(|d| d.id.clone()).collect();
        assert_eq!(
            plan_renumber(&decisions, &local),
            vec![Renumber {
                from: DecisionId::parse("20260923-001-b").unwrap(),
                to: DecisionId::parse("20260923-002-b").unwrap(),
            }]
        );
    }

    #[test]
    fn planning_is_empty_without_duplicates_and_dates_are_independent() {
        let decisions = [
            decision("20260923-001-a", DecisionStatus::Accepted, &[]),
            decision("20260923-001-b", DecisionStatus::Accepted, &[]),
            decision("20260924-001-c", DecisionStatus::Accepted, &[]),
            decision("20260924-001-d", DecisionStatus::Accepted, &[]),
        ];
        let local = HashSet::from([decisions[1].id.clone(), decisions[3].id.clone()]);
        assert!(plan_renumber(&decisions[..1], &local).is_empty());
        let plan = plan_renumber(&decisions, &local);
        assert_eq!(plan[0].to.as_str(), "20260923-002-b");
        assert_eq!(plan[1].to.as_str(), "20260924-002-d");
        let reversed = decisions.iter().rev().cloned().collect::<Vec<_>>();
        assert_eq!(plan_renumber(&reversed, &local), plan);
    }

    #[test]
    fn renumbering_rewrites_superseded_ids() {
        let decision = decision(
            "20260923-002-c",
            DecisionStatus::Accepted,
            &["20260923-001-b"],
        );
        let from = DecisionId::parse("20260923-001-b").unwrap();
        let to = DecisionId::parse("20260923-003-b").unwrap();
        let map = HashMap::from([(from, to.clone())]);
        let new_id = DecisionId::parse("20260923-004-c").unwrap();
        let changed = renumber_decision(&decision, &new_id, &map);
        assert_eq!(changed.id, new_id);
        assert_eq!(changed.supersedes, vec![to]);
        assert_eq!(decision.id.as_str(), "20260923-002-c");
        assert_eq!(decision.supersedes[0].as_str(), "20260923-001-b");
    }
}
