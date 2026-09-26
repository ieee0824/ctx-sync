mod common;

use ctx_sync_core::view::{ViewOptions, build_onboard_view, render_onboard_markdown};
use uuid::Uuid;

fn parser() -> Option<Uuid> {
    Some(Uuid::parse_str(common::PARSER_ID).unwrap())
}

fn opts() -> ViewOptions {
    ViewOptions::new(common::time("2026-09-23T19:00:00+09:00"))
}

const EXPECTED: &str = "# Project Onboarding

## Project

demo

### Goal

Share the development context.

### Non Goals

- AI chat

## You

parser (11111111) — working
Task: Parser implementation

## Current Architecture

### Components

- core
- cli

## Important Decisions

- 20260923-002-use-git-transport: Use Git transport — Treat the Gist as a Git repository.

## Active Workers

### ui (33333333) — blocked

Task:
UI

## Needs Attention

- [parser] API changed
- [parser] interface: ParserResult gained warnings
- [ui] blocked by: waiting for ParserResult

## Recent Relevant Changes

- 2026-09-23T18:20:00+09:00 parser: Worker file format added.
- 2026-09-23T18:10:00+09:00 gist: Gist backend implemented.
";

#[test]
fn renders_the_expected_markdown() {
    let view = build_onboard_view(&common::snapshot(), parser(), 5, &opts());
    assert_eq!(render_onboard_markdown(&view), EXPECTED);
}

#[test]
fn you_are_not_listed_among_active_workers() {
    let view = build_onboard_view(&common::snapshot(), parser(), 5, &opts());
    assert_eq!(view.you.as_ref().unwrap().name, "parser");
    let names: Vec<_> = view
        .active_workers
        .iter()
        .map(|w| w.name.as_str())
        .collect();
    assert_eq!(names, ["ui"]);

    let anonymous = build_onboard_view(&common::snapshot(), None, 5, &opts());
    assert!(anonymous.you.is_none());
    assert!(!render_onboard_markdown(&anonymous).contains("## You"));
    let names: Vec<_> = anonymous
        .active_workers
        .iter()
        .map(|w| w.name.as_str())
        .collect();
    assert_eq!(names, ["parser", "ui"]);
}

#[test]
fn files_are_limited_to_ten() {
    let mut snapshot = common::snapshot();
    let ui = snapshot
        .workers
        .iter_mut()
        .find(|w| w.name == "ui")
        .unwrap();
    ui.changed = (0..12).map(|i| format!("src/ui/{i:02}.rs")).collect();
    let view = build_onboard_view(&snapshot, parser(), 5, &opts());
    let files = &view.active_workers[0].files;
    assert_eq!(files.len(), 11);
    assert_eq!(files[9], "src/ui/09.rs");
    assert_eq!(files[10], "... and 2 more");
}

#[test]
fn recent_changes_are_newest_first_and_limited() {
    let view = build_onboard_view(&common::snapshot(), parser(), 1, &opts());
    assert_eq!(view.recent_changes.len(), 1);
    assert_eq!(view.recent_changes[0].worker, "parser");
}

#[test]
fn long_decision_summaries_are_shortened() {
    let mut snapshot = common::snapshot();
    let decision = snapshot
        .decisions
        .iter_mut()
        .find(|d| d.id.as_str() == "20260923-002-use-git-transport")
        .unwrap();
    decision.decision = format!("\n{}\nsecond line", "あ".repeat(130));
    let view = build_onboard_view(&snapshot, parser(), 5, &opts());
    let summary = &view.important_decisions[0].summary;
    assert_eq!(summary.chars().count(), 121);
    assert!(summary.ends_with('…'));
}

#[test]
fn only_effective_decisions_are_listed() {
    let view = build_onboard_view(&common::snapshot(), parser(), 5, &opts());
    let ids: Vec<_> = view
        .important_decisions
        .iter()
        .map(|d| d.id.as_str())
        .collect();
    assert_eq!(ids, ["20260923-002-use-git-transport"]);
}

#[test]
fn stale_workers_are_marked_in_onboarding_and_json() {
    let view = build_onboard_view(
        &common::snapshot(),
        None,
        5,
        &ViewOptions::new(common::time("2026-09-26T19:00:00+09:00")),
    );
    let markdown = render_onboard_markdown(&view);
    assert!(markdown.contains("### parser (11111111) — working, stale (3d)"));
    assert!(markdown.contains("### ui (33333333) — blocked, stale (3d)"));
    assert!(!markdown.contains("### gist (22222222)"));
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["active_workers"][0]["stale"], true);
    assert_eq!(json["active_workers"][0]["age"], "3d");
    assert_eq!(json["active_workers"][1]["stale"], true);
}
