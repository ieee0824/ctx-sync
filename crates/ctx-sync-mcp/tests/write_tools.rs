use ctx_sync_core::clock::parse_now;
use ctx_sync_core::model::Worker;
use ctx_sync_core::ops::{self, AttachOptions, Runtime, Workspace};
use ctx_sync_core::state::{Protocol, StateRoot};
use ctx_sync_core::store::ContextStore;
use ctx_sync_mcp::server::Server;
use ctx_sync_testutil::{FIXED_NOW, TestRemote, TestWorker};
use serde_json::{Value, json};

fn setup(remote: &TestRemote, worker: &TestWorker, register: bool) -> (Runtime, Server) {
    let rt = Runtime {
        state_root: StateRoot::new(worker.state_home()),
        remote_url_override: Some(remote.url()),
        git_env: remote.git_env(),
    };
    let now = parse_now(Some(FIXED_NOW)).unwrap();
    ops::attach(
        &rt,
        AttachOptions {
            project_root: worker.project().to_path_buf(),
            gist: "testgist01".into(),
            protocol: Protocol::Https,
            now,
        },
    )
    .unwrap();
    if register {
        let ws = Workspace::open(&rt, worker.project()).unwrap();
        ops::register(&ws, "mcp", false, now).unwrap();
    }
    let mut server = Server::new(worker.project().to_path_buf());
    server.runtime = Some(rt.clone());
    (rt, server)
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
fn update_decision_and_finish_write_through_core() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    let (rt, mut server) = setup(&remote, &worker, true);
    let ws = Workspace::open(&rt, worker.project()).unwrap();
    let file = Worker::file_name_for(&ws.require_identity().unwrap().id);

    let update = call(
        &mut server,
        "update_worker",
        json!({"task": "Parser", "attention": ["API changed"], "sync": true}),
    );
    assert_eq!(update["isError"], Value::Null);
    assert!(
        update["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("Updated worker")
    );
    let published = remote.read_file(&file).unwrap();
    assert!(published.contains("Parser"));
    assert!(published.contains("API changed"));

    let decision = call(&mut server, "add_decision", json!({"title": "Use MCP"}));
    let decision_file = decision["structuredContent"]["file_name"].as_str().unwrap();
    assert!(decision_file.starts_with("30-decision-"));
    assert!(ws.store.read_file(decision_file).unwrap().is_some());
    assert!(
        decision["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("Added decision")
    );

    let finish = call(&mut server, "finish_worker", json!({"summary": "done"}));
    assert!(
        finish["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("# Handoff Complete")
    );
    assert!(remote.read_file(&file).unwrap().contains("done"));
}

#[test]
fn unregistered_worker_cannot_update() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    let (_rt, mut server) = setup(&remote, &worker, false);
    let result = call(&mut server, "update_worker", json!({"task": "Parser"}));
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["exit_code"], 4);
}
