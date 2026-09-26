use ctx_sync_core::clock::parse_now;
use ctx_sync_core::model::WorkerStatus;
use ctx_sync_core::ops::{self, AttachOptions, HandoffInput, Runtime, Workspace};
use ctx_sync_core::state::{Protocol, StateRoot};
use ctx_sync_mcp::server::Server;
use ctx_sync_testutil::{FIXED_NOW, TestRemote, TestWorker};
use serde_json::{Value, json};

fn runtime(remote: &TestRemote, worker: &TestWorker) -> Runtime {
    Runtime {
        state_root: StateRoot::new(worker.state_home()),
        remote_url_override: Some(remote.url()),
        git_env: remote.git_env(),
    }
}

fn attach(remote: &TestRemote, worker: &TestWorker) -> Runtime {
    let rt = runtime(remote, worker);
    ops::attach(
        &rt,
        AttachOptions {
            project_root: worker.project().to_path_buf(),
            gist: "testgist01".into(),
            protocol: Protocol::Https,
            now: parse_now(Some(FIXED_NOW)).unwrap(),
        },
    )
    .unwrap();
    rt
}

fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    server
        .handle(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }))
        .unwrap()["result"]
        .clone()
}

#[test]
fn context_onboarding_and_worker_listing_use_the_attached_project() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = TestWorker::new(&remote);
    let a_rt = attach(&remote, &a);
    let a_ws = Workspace::open(&a_rt, a.project()).unwrap();
    let now = parse_now(Some(FIXED_NOW)).unwrap();
    ops::register(&a_ws, "parser", false, now).unwrap();
    ops::handoff(
        &a_ws,
        HandoffInput {
            task: Some("Parser implementation".into()),
            ..Default::default()
        },
        true,
        now,
    )
    .unwrap();

    let mut a_server = Server::new(a.project().to_path_buf());
    a_server.runtime = Some(a_rt);
    let context = call(&mut a_server, "get_context", json!({"task": "parser"}));
    assert_eq!(context["structuredContent"]["project_name"], "demo");
    assert_eq!(context["structuredContent"]["task"], "parser");
    assert_eq!(
        context["structuredContent"]["active_workers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let b = TestWorker::new(&remote);
    let b_rt = attach(&remote, &b);
    let mut b_server = Server::new(b.project().to_path_buf());
    b_server.runtime = Some(b_rt.clone());
    let onboarding = call(
        &mut b_server,
        "get_onboarding_context",
        json!({"name": "mcp"}),
    );
    assert_eq!(onboarding["structuredContent"]["registered"], true);
    assert!(
        onboarding["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("# Project Onboarding")
    );

    let b_ws = Workspace::open(&b_rt, b.project()).unwrap();
    ops::handoff(
        &b_ws,
        HandoffInput {
            status: Some(WorkerStatus::Done),
            ..Default::default()
        },
        false,
        now,
    )
    .unwrap();
    let active = call(&mut b_server, "list_workers", json!({}));
    assert_eq!(
        active["structuredContent"]["workers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let all = call(
        &mut b_server,
        "list_workers",
        json!({"include_inactive": true}),
    );
    assert_eq!(
        all["structuredContent"]["workers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn unattached_project_returns_configuration_error() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    let mut server = Server::new(worker.project().to_path_buf());
    server.runtime = Some(runtime(&remote, &worker));
    let result = call(&mut server, "get_context", json!({}));
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["exit_code"], 4);
}
