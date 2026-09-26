//! Fixed snapshot shared by the view tests.
#![allow(dead_code)]

use chrono::{DateTime, FixedOffset, NaiveDate};
use ctx_sync_core::model::{
    ContextSnapshot, Decision, DecisionId, DecisionStatus, MdDoc, Meta, Worker, WorkerStatus,
};
use uuid::Uuid;

pub const PROJECT_ID: &str = "6f1c2b7e-0000-4000-8000-000000000001";
pub const PARSER_ID: &str = "11111111-0000-4000-8000-000000000001";
pub const GIST_ID: &str = "22222222-0000-4000-8000-000000000002";
pub const UI_ID: &str = "33333333-0000-4000-8000-000000000003";

pub fn time(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s).unwrap()
}

fn decision(
    id: &str,
    title: &str,
    status: DecisionStatus,
    supersedes: &[&str],
    body: &str,
) -> Decision {
    Decision {
        id: DecisionId::parse(id).unwrap(),
        title: title.into(),
        status,
        date: NaiveDate::from_ymd_opt(2026, 9, 23).unwrap(),
        author: None,
        supersedes: supersedes
            .iter()
            .map(|s| DecisionId::parse(s).unwrap())
            .collect(),
        context: String::new(),
        decision: body.into(),
        reason: String::new(),
        consequences: String::new(),
        extra_sections: vec![],
    }
}

fn worker(id: &str, name: &str, status: WorkerStatus, updated: &str) -> Worker {
    let mut w = Worker::new(Uuid::parse_str(id).unwrap(), name, time(updated));
    w.status = status;
    w
}

/// - decisions: 001 (superseded by 002), 002 (accepted), 003 (proposed)
/// - workers: gist (done), parser (working), ui (blocked)
pub fn snapshot() -> ContextSnapshot {
    let mut parser = worker(
        PARSER_ID,
        "parser",
        WorkerStatus::Working,
        "2026-09-23T18:20:00+09:00",
    );
    parser.branch = Some("feat/parser".into());
    parser.commit = Some("abc1234".into());
    parser.task = "Parser implementation".into();
    parser.working_on = vec!["parser state machine".into()];
    parser.changed = vec!["src/parser.rs".into(), "src/error.rs".into()];
    parser.interface_changes = vec!["ParserResult gained warnings".into()];
    parser.attention = vec!["API changed".into()];
    parser.summary = "Worker file format added.".into();

    let mut gist = worker(
        GIST_ID,
        "gist",
        WorkerStatus::Done,
        "2026-09-23T18:10:00+09:00",
    );
    gist.branch = Some("feat/gist".into());
    gist.task = "Gist backend".into();
    gist.attention = vec!["old note".into()];
    gist.summary = "Gist backend implemented.".into();

    let mut ui = worker(
        UI_ID,
        "ui",
        WorkerStatus::Blocked,
        "2026-09-23T18:00:00+09:00",
    );
    ui.task = "UI".into();
    ui.blocked_by = vec!["waiting for ParserResult".into()];

    ContextSnapshot {
        meta: Meta {
            schema_version: 1,
            project_id: Uuid::parse_str(PROJECT_ID).unwrap(),
            project_name: "demo".into(),
            created_at: time("2026-09-23T18:00:00+09:00"),
            ctx_sync_version: Some("0.1.0".into()),
            project_file: None,
            architecture_file: None,
        },
        project: MdDoc::parse(
            "# Project\n\n## Goal\n\nShare the development context.\n\n## Non Goals\n\n- AI chat\n",
        )
        .unwrap(),
        architecture: MdDoc::parse("# Architecture\n\n## Components\n\n- core\n- cli\n").unwrap(),
        decisions: vec![
            decision(
                "20260923-001-use-gist",
                "Use GitHub Gist",
                DecisionStatus::Accepted,
                &[],
                "Store the context in a Gist.",
            ),
            decision(
                "20260923-002-use-git-transport",
                "Use Git transport",
                DecisionStatus::Accepted,
                &["20260923-001-use-gist"],
                "Treat the Gist as a Git repository.",
            ),
            decision(
                "20260923-003-maybe-s3",
                "Maybe S3",
                DecisionStatus::Proposed,
                &[],
                "Consider S3.",
            ),
        ],
        workers: vec![gist, parser, ui],
        warnings: vec![],
    }
}
