//! Handle one line-delimited MCP JSON-RPC request at a time.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::protocol;
use crate::tools;

pub struct Server {
    pub project_dir: PathBuf,
}

impl Server {
    pub fn new(project_dir: PathBuf) -> Self {
        Self { project_dir }
    }

    /// Notifications have no response; requests return exactly one response.
    pub fn handle(&mut self, message: Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(protocol::error(Value::Null, -32600, "Invalid Request"));
        };
        let id = object.get("id").cloned();
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || !object.get("method").is_some_and(Value::is_string)
        {
            return Some(protocol::error(
                id.unwrap_or(Value::Null),
                -32600,
                "Invalid Request",
            ));
        }
        let id = id?;
        let method = object["method"].as_str().expect("validated method");
        let response = match method {
            "initialize" => protocol::success(
                id,
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "ctx-sync", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Shared development context for multiple agents. Call get_onboarding_context before starting work and finish_worker when done."
                }),
            ),
            "ping" => protocol::success(id, json!({})),
            "tools/list" => protocol::success(id, json!({"tools": tools::tool_definitions()})),
            "tools/call" => {
                let name = object
                    .get("params")
                    .and_then(|params| params.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = object
                    .get("params")
                    .and_then(|params| params.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match tools::call_tool(&self.project_dir, name, arguments) {
                    Some(result) => protocol::success(id, result),
                    None => protocol::error(id, -32602, &format!("Unknown tool: {name}")),
                }
            }
            _ => protocol::error(id, -32601, "Method not found"),
        };
        Some(response)
    }

    /// Invalid JSON returns a parse error with a null request ID.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let response = match serde_json::from_str(line) {
            Ok(value) => self.handle(value),
            Err(_) => Some(protocol::error(Value::Null, -32700, "Parse error")),
        }?;
        Some(response.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(server: &mut Server, input: &str) -> Value {
        serde_json::from_str(&server.handle_line(input).unwrap()).unwrap()
    }

    #[test]
    fn initializes_and_responds_to_ping() {
        let mut server = Server::new(PathBuf::from("."));
        let initialized = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        );
        assert_eq!(initialized["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(initialized["result"]["serverInfo"]["name"], "ctx-sync");
        assert_eq!(initialized["result"]["capabilities"]["tools"], json!({}));
        assert_eq!(
            response(&mut server, r#"{"jsonrpc":"2.0","id":"p","method":"ping"}"#)["result"],
            json!({})
        );
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
    }

    #[test]
    fn rejects_unknown_or_malformed_requests() {
        let mut server = Server::new(PathBuf::from("."));
        assert_eq!(
            response(&mut server, r#"{"jsonrpc":"2.0","id":1,"method":"other"}"#)["error"]["code"],
            -32601
        );
        assert_eq!(response(&mut server, "{")["error"]["code"], -32700);
        assert_eq!(response(&mut server, "{")["id"], Value::Null);
        assert_eq!(
            response(&mut server, r#"{"jsonrpc":"1.0","id":2,"method":"ping"}"#)["error"]["code"],
            -32600
        );
        assert_eq!(
            response(
                &mut server,
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"missing"}}"#
            )["error"]["message"],
            "Unknown tool: missing"
        );
    }

    #[test]
    fn lists_six_tools_and_validates_calls() {
        let mut server = Server::new(PathBuf::from("."));
        let list = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        );
        let tools = list["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 6);
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "get_context",
                "get_onboarding_context",
                "list_workers",
                "update_worker",
                "add_decision",
                "finish_worker"
            ]
        );
        let decision = tools
            .iter()
            .find(|tool| tool["name"] == "add_decision")
            .unwrap();
        assert_eq!(decision["inputSchema"]["required"], json!(["title"]));
        assert_eq!(decision["inputSchema"]["type"], "object");
        let invalid = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"update_worker","arguments":{"append":"yes"}}}"#,
        );
        assert_eq!(invalid["result"]["isError"], true);
        assert_eq!(invalid["result"]["structuredContent"]["exit_code"], 4);
        let missing_title = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"add_decision","arguments":{}}}"#,
        );
        assert_eq!(missing_title["result"]["structuredContent"]["exit_code"], 4);
        let placeholder = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_context","arguments":{"unknown":1}}}"#,
        );
        assert_eq!(placeholder["result"]["isError"], true);
        assert_eq!(placeholder["result"]["structuredContent"]["exit_code"], 1);
    }
}
