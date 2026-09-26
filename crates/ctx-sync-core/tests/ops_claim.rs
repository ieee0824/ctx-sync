use chrono::{DateTime, FixedOffset};
use ctx_sync_core::config::ProjectConfig;
use ctx_sync_core::model::{Worker, WorkerStatus};
use ctx_sync_core::ops::{
    ClaimInput, HandoffInput, Workspace, changed_file_conflicts, claim, claim_conflicts,
    conflict_warning, handoff,
};
use ctx_sync_core::state::{LocalProject, Protocol, StateRoot, WorkerIdentity};
use ctx_sync_core::store::{ContextStore, GistStore, RemoteSpec};
use ctx_sync_testutil::{TestRemote, TestWorker};
use uuid::Uuid;

fn now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-09-23T18:30:00+09:00").unwrap()
}

struct Fixture {
    worker: TestWorker,
    ws: Workspace,
}

fn fixture(remote: &TestRemote, name: &str) -> Fixture {
    let worker = TestWorker::new(remote);
    let project_state = StateRoot::new(worker.state_home()).project(&Uuid::nil());
    let store = GistStore::new(
        &project_state,
        RemoteSpec::new("x", Protocol::Https).with_url_override(Some(remote.url())),
    )
    .with_git_env(remote.git_env());
    store.ensure().unwrap();
    WorkerIdentity::new(name, worker.project(), now())
        .unwrap()
        .save(&project_state.worker_json(worker.project()).unwrap())
        .unwrap();
    let ws = Workspace {
        root: worker.project().to_path_buf(),
        config: ProjectConfig::new_gist("x"),
        project_id: Uuid::nil(),
        project_state,
        local: LocalProject {
            gist_id: "x".into(),
            protocol: Protocol::Https,
            attached_at: now(),
        },
        store,
    };
    Fixture { worker, ws }
}

#[test]
fn pure_conflict_checks_ignore_self_and_inactive_workers() {
    let me = Uuid::new_v4();
    let mut other = Worker::new(Uuid::new_v4(), "parser", now());
    other.claims = vec!["src/parser/**".into()];
    let mut done = other.clone();
    done.id = Uuid::new_v4();
    done.status = WorkerStatus::Done;
    let mut myself = other.clone();
    myself.id = me;
    let workers = [other.clone(), done, myself];

    let claims = claim_conflicts(&["src/**".into()], me, &workers);
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].worker, "parser");
    assert_eq!(claims[0].mine, "src/**");
    assert_eq!(claims[0].theirs, "src/parser/**");
    assert_eq!(
        conflict_warning(&claims[0]),
        format!(
            "src/** overlaps with parser ({})'s claim src/parser/**",
            claims[0].short_id
        )
    );
    assert!(claim_conflicts(&["docs/**".into()], me, &workers).is_empty());

    let files = changed_file_conflicts(&["src/parser/lexer.rs".into()], me, &workers);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].mine, "src/parser/lexer.rs");
    assert!(changed_file_conflicts(&["docs/readme.md".into()], me, &workers).is_empty());
}

#[test]
fn claims_warn_on_overlap_and_can_be_released() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = fixture(&remote, "alice");
    let b = fixture(&remote, "bob");
    claim(
        &a.ws,
        ClaimInput {
            patterns: vec!["src/parser/**".into()],
            release: false,
        },
        true,
        now(),
    )
    .unwrap();
    let result = claim(
        &b.ws,
        ClaimInput {
            patterns: vec!["src/**".into(), "src/**".into(), "docs/**".into()],
            release: false,
        },
        false,
        now(),
    )
    .unwrap();
    assert_eq!(result.claims, ["src/**", "docs/**"]);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].worker, "alice");
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("src/** overlaps"))
    );

    let released = claim(
        &b.ws,
        ClaimInput {
            patterns: vec!["src/**".into()],
            release: true,
        },
        false,
        now(),
    )
    .unwrap();
    assert_eq!(released.claims, ["docs/**"]);
    assert!(released.conflicts.is_empty());
    let all = claim(
        &b.ws,
        ClaimInput {
            patterns: vec![],
            release: true,
        },
        false,
        now(),
    )
    .unwrap();
    assert!(all.claims.is_empty());
    assert_eq!(
        claim(
            &b.ws,
            ClaimInput {
                patterns: vec!["../private".into()],
                release: false,
            },
            false,
            now(),
        )
        .unwrap_err()
        .exit_code(),
        4
    );
}

#[test]
fn handoff_warns_when_changed_file_matches_another_claim() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = fixture(&remote, "alice");
    let b = fixture(&remote, "bob");
    claim(
        &a.ws,
        ClaimInput {
            patterns: vec!["src/parser/**".into()],
            release: false,
        },
        true,
        now(),
    )
    .unwrap();
    b.ws.store.pull().unwrap();
    std::fs::create_dir_all(b.worker.project().join("src/parser")).unwrap();
    std::fs::write(b.worker.project().join("src/parser/lexer.rs"), "lexer").unwrap();
    let outcome = handoff(
        &b.ws,
        HandoffInput {
            changed: Some(vec!["src/parser/lexer.rs".into()]),
            ..Default::default()
        },
        false,
        now(),
    )
    .unwrap();
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("alice") && warning.contains("src/parser/lexer.rs"))
    );
}
