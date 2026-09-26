mod common;

use ctx_sync_core::view::{ViewOptions, build_context_view, render_context_markdown};

fn opts() -> ViewOptions {
    ViewOptions::new(common::time("2026-09-23T19:00:00+09:00"))
}

const EXPECTED: &str = "# Shared Project Context

Project: demo
Project ID: 6f1c2b7e-0000-4000-8000-000000000001

## Project

### Goal

Share the development context.

### Non Goals

- AI chat

## Architecture

### Components

- core
- cli

## Accepted Decisions

### 20260923-002-use-git-transport: Use Git transport

Supersedes: 20260923-001-use-gist

Treat the Gist as a Git repository.

## Active Workers

### parser (11111111)

Status: working
Branch: feat/parser
Task: Parser implementation
Working On:
- parser state machine
Changed:
- src/parser.rs
- src/error.rs
Interface Changes:
- ParserResult gained warnings
Attention:
- API changed
Summary: Worker file format added.

### ui (33333333)

Status: blocked
Task: UI
Blocked By:
- waiting for ParserResult

## Attention

- [parser] API changed
- [parser] interface: ParserResult gained warnings
- [ui] blocked by: waiting for ParserResult
";

#[test]
fn renders_the_expected_markdown() {
    let view = build_context_view(&common::snapshot(), &opts());
    assert_eq!(render_context_markdown(&view), EXPECTED);
}

#[test]
fn only_effective_decisions_are_included() {
    let view = build_context_view(&common::snapshot(), &opts());
    let ids: Vec<_> = view.decisions.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["20260923-002-use-git-transport"]);
}

#[test]
fn done_workers_are_left_out() {
    let view = build_context_view(&common::snapshot(), &opts());
    let names: Vec<_> = view
        .active_workers
        .iter()
        .map(|w| w.name.as_str())
        .collect();
    assert_eq!(names, ["parser", "ui"]);
    let markdown = render_context_markdown(&view);
    assert!(!markdown.contains("old note"));
    assert!(!markdown.contains("### gist"));
}

#[test]
fn embedded_headings_are_demoted() {
    let markdown = render_context_markdown(&build_context_view(&common::snapshot(), &opts()));
    assert!(markdown.contains("\n### Goal\n"));
    assert!(!markdown.contains("\n## Goal\n"));
}

#[test]
fn empty_sections_say_none_and_warnings_are_listed() {
    let mut snapshot = common::snapshot();
    snapshot.decisions.clear();
    snapshot.workers.clear();
    snapshot.architecture = ctx_sync_core::model::MdDoc::parse("# Architecture\n").unwrap();
    snapshot.warnings = vec!["skipped 40-worker-x.md: missing ID".into()];
    let markdown = render_context_markdown(&build_context_view(&snapshot, &opts()));
    assert!(markdown.contains("## Architecture\n\n_None_\n"));
    assert!(markdown.contains("## Accepted Decisions\n\n_None_\n"));
    assert!(markdown.contains("## Active Workers\n\n_None_\n"));
    assert!(markdown.contains("## Attention\n\n_None_\n"));
    assert!(markdown.ends_with("## Warnings\n\n- skipped 40-worker-x.md: missing ID\n"));
}

#[test]
fn view_serializes_to_json() {
    let view = build_context_view(&common::snapshot(), &opts());
    let json: serde_json::Value = serde_json::to_value(&view).unwrap();
    assert_eq!(json["decisions"].as_array().unwrap().len(), 1);
    assert_eq!(json["active_workers"][0]["status"], "working");
    assert_eq!(json["attention"][1]["kind"], "interface_change");
}

#[test]
fn stale_workers_are_visible_in_text_and_json() {
    let view = build_context_view(
        &common::snapshot(),
        &ViewOptions::new(common::time("2026-09-26T19:00:00+09:00")),
    );
    let markdown = render_context_markdown(&view);
    assert!(markdown.contains("Status: working (stale: last updated 3d ago)"));
    assert!(markdown.contains("Status: blocked (stale: last updated 3d ago)"));
    assert!(!markdown.contains("### gist ("));
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["active_workers"][0]["stale"], true);
    assert_eq!(json["active_workers"][0]["age"], "3d");
    assert_eq!(json["active_workers"][1]["stale"], true);
}

#[test]
fn active_claims_appear_in_context() {
    let mut snapshot = common::snapshot();
    snapshot.workers[1].claims = vec!["src/parser/**".into()];
    snapshot.workers[0].claims = vec!["private/**".into()];
    let view = build_context_view(&snapshot, &opts());
    assert_eq!(view.active_workers[0].claims, ["src/parser/**"]);
    let markdown = render_context_markdown(&view);
    assert!(markdown.contains("Claims:\n- src/parser/**"));
    assert!(!markdown.contains("private/**"));
}
