mod common;

use ctx_sync_core::view::{
    ViewOptions, build_context_view, filter_context_view, relevance, render_context_markdown,
    task_tokens,
};

fn view() -> ctx_sync_core::view::ContextView {
    build_context_view(
        &common::snapshot(),
        &ViewOptions::new(common::time("2026-09-23T19:00:00+09:00")),
    )
}

#[test]
fn tokenization_keeps_unicode_and_deduplicates_words() {
    assert_eq!(
        task_tokens("Fix the ParserResult warnings"),
        ["fix", "parserresult", "warnings"]
    );
    assert_eq!(task_tokens("パーサの修正"), ["パーサの修正"]);
    assert_eq!(task_tokens("THE a parser parser"), ["parser"]);
    assert_eq!(relevance(&task_tokens("parser git"), "Parser and GIT"), 2);
}

#[test]
fn parser_filter_keeps_only_related_worker_and_attention() {
    let mut source = view();
    // The shared fixture's UI blocker mentions ParserResult and would also
    // match the task by the attention-text rule.
    source.attention.retain(|item| item.worker != "ui");
    let filtered = filter_context_view(source, "parser");
    assert_eq!(filtered.task.as_deref(), Some("parser"));
    assert!(filtered.decisions.is_empty());
    assert_eq!(filtered.active_workers.len(), 1);
    assert_eq!(filtered.active_workers[0].name, "parser");
    assert_eq!(filtered.attention.len(), 2);
    let rendered = render_context_markdown(&filtered);
    assert!(
        rendered.contains("Project ID: 6f1c2b7e-0000-4000-8000-000000000001\nTask filter: parser")
    );
    assert!(!rendered.contains("### ui ("));
}

#[test]
fn relevant_attention_from_another_worker_remains_visible() {
    let filtered = filter_context_view(view(), "parser");
    assert_eq!(filtered.attention.len(), 3);
    assert!(filtered.attention.iter().any(|item| item.worker == "ui"));
}

#[test]
fn git_filter_keeps_relevant_decision() {
    let filtered = filter_context_view(view(), "git");
    assert_eq!(filtered.decisions.len(), 1);
    assert_eq!(filtered.decisions[0].id, "20260923-002-use-git-transport");
}

#[test]
fn stopwords_do_not_remove_any_items() {
    let original = view();
    let filtered = filter_context_view(original.clone(), "the a");
    assert_eq!(filtered.task.as_deref(), Some("the a"));
    assert_eq!(filtered.decisions.len(), original.decisions.len());
    assert_eq!(filtered.active_workers.len(), original.active_workers.len());
    assert_eq!(filtered.attention.len(), original.attention.len());
    assert!(render_context_markdown(&view()).contains("Project ID:"));
    assert!(!render_context_markdown(&view()).contains("Task filter:"));
}
