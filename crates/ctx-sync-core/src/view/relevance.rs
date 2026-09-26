//! Simple keyword matching for task-focused context views.

use std::collections::HashSet;

use super::ContextView;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "in", "is", "it", "of", "on",
    "or", "the", "to", "with",
];

pub fn task_tokens(task: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    task.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c.is_ascii())
        .filter(|word| word.chars().count() >= 2 && !STOPWORDS.contains(word))
        .filter(|word| seen.insert((*word).to_string()))
        .map(str::to_string)
        .collect()
}

pub fn relevance(tokens: &[String], text: &str) -> usize {
    let text = text.to_lowercase();
    tokens.iter().filter(|token| text.contains(*token)).count()
}

pub fn filter_context_view(mut view: ContextView, task: &str) -> ContextView {
    view.task = Some(task.into());
    let tokens = task_tokens(task);
    if tokens.is_empty() {
        return view;
    }

    view.decisions.retain(|decision| {
        relevance(
            &tokens,
            &format!("{} {}", decision.title, decision.decision),
        ) > 0
    });
    view.active_workers.retain(|worker| {
        let fields = [
            worker.task.as_str(),
            &worker.working_on.join(" "),
            &worker.claims.join(" "),
            &worker.changed.join(" "),
            &worker.interface_changes.join(" "),
            &worker.attention.join(" "),
            worker.summary.as_str(),
        ];
        relevance(&tokens, &fields.join(" ")) > 0
    });
    view.attention.retain(|item| {
        view.active_workers
            .iter()
            .any(|worker| worker.name == item.worker)
            || relevance(&tokens, &item.text) > 0
    });
    view
}
