mod common;

use ctx_sync_core::view::{build_decision_list, render_decision_list};

#[test]
fn default_list_contains_only_the_effective_decision() {
    let items = build_decision_list(&common::snapshot(), false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "20260923-002-use-git-transport");
    assert!(items[0].effective);
    assert_eq!(
        render_decision_list(&items),
        "20260923-002-use-git-transport  accepted  Use Git transport\n"
    );
}

#[test]
fn all_decisions_include_supersession_and_other_statuses() {
    let items = build_decision_list(&common::snapshot(), true);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].id, "20260923-001-use-gist");
    assert_eq!(items[0].superseded_by, ["20260923-002-use-git-transport"]);
    assert!(!items[0].effective);
    assert_eq!(
        render_decision_list(&items),
        "20260923-001-use-gist  superseded by 20260923-002-use-git-transport  Use GitHub Gist\n\
         20260923-002-use-git-transport  accepted  Use Git transport\n\
         20260923-003-maybe-s3  proposed  Maybe S3\n"
    );
}

#[test]
fn empty_list_has_a_short_message() {
    let mut snapshot = common::snapshot();
    snapshot.decisions.clear();
    assert_eq!(
        render_decision_list(&build_decision_list(&snapshot, true)),
        "No decisions.\n"
    );
}
