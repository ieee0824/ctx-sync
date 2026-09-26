use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use ctx_sync_testutil::{TestRemote, TestWorker};
use serde_json::{Value, json};

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ctx-sync-mcp")).with_file_name("ctx-sync")
}

fn cli(worker: &TestWorker, args: &[&str]) -> String {
    let output = Command::new(cli_bin())
        .args(args)
        .current_dir(worker.project())
        .envs(worker.env())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "ctx-sync {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

struct Mcp {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Mcp {
    fn start(worker: &TestWorker) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ctx-sync-mcp"))
            .args(["--project", worker.project().to_str().unwrap()])
            .envs(worker.env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }

    fn request(&mut self, id: u32, method: &str, params: Value) -> Value {
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        assert!(self.output.read_line(&mut line).unwrap() > 0);
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id);
        response
    }

    fn tool(&mut self, id: u32, name: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({"name": name, "arguments": arguments}),
        )["result"]
            .clone()
    }

    fn finish(mut self) {
        drop(self.input);
        assert!(self.child.wait().unwrap().success());
    }
}

#[test]
fn mcp_handoff_is_visible_to_a_second_cli_worker() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = TestWorker::new(&remote);
    cli(&a, &["attach", "testgist01"]);

    let mut mcp = Mcp::start(&a);
    assert_eq!(
        mcp.request(1, "initialize", json!({}))["result"]["protocolVersion"],
        "2025-06-18"
    );
    assert_eq!(
        mcp.request(2, "tools/list", json!({}))["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let onboarding = mcp.tool(3, "get_onboarding_context", json!({"name": "mcp-a"}));
    assert_eq!(onboarding["structuredContent"]["registered"], true);
    let update = mcp.tool(
        4,
        "update_worker",
        json!({"attention": ["from mcp"], "sync": true}),
    );
    assert_ne!(update["structuredContent"]["sync"], Value::Null);
    let finish = mcp.tool(5, "finish_worker", json!({"summary": "via mcp"}));
    assert!(
        finish["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("# Handoff Complete")
    );
    mcp.finish();

    let b = TestWorker::new(&remote);
    cli(&b, &["attach", "testgist01"]);
    let onboarding = cli(&b, &["agent", "start", "--name", "b"]);
    assert!(onboarding.contains("from mcp"), "{onboarding}");
    assert!(onboarding.contains("via mcp"), "{onboarding}");
}
