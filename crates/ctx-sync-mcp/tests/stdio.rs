use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn exchanges_initialize_and_ping_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ctx-sync-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let input = child.stdin.as_mut().unwrap();
        writeln!(
            input,
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}}"
        )
        .unwrap();
        writeln!(
            input,
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}}"
        )
        .unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines: Vec<serde_json::Value> = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(lines[1]["result"], serde_json::json!({}));
}
