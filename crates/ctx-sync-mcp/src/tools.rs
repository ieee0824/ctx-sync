//! MCP tool declarations and shared argument validation.

use std::path::Path;

use ctx_sync_core::Error;
use serde_json::{Value, json};

fn property(kind: &str, description: &str) -> Value {
    json!({"type": kind, "description": description})
}

fn string_array(description: &str) -> Value {
    json!({"type": "array", "items": {"type": "string"}, "description": description})
}

fn definition(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": true
        }
    })
}

fn worker_properties(with_sync: bool) -> Value {
    let mut properties = json!({
        "task": property("string", "Current task"),
        "summary": property("string", "Progress summary"),
        "status": {"type": "string", "enum": ["working", "blocked", "done", "abandoned"]},
        "working_on": string_array("Current work items"),
        "changed": string_array("Changed files or areas"),
        "interface_changes": string_array("Interface changes"),
        "attention": string_array("Items needing attention"),
        "blocked_by": string_array("Blockers"),
        "append": {"type": "boolean", "default": false}
    });
    if with_sync {
        properties["sync"] = json!({"type": "boolean", "default": false});
    }
    properties
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        definition(
            "get_context",
            "Read project context, optionally filtered to a task",
            json!({
                "task": property("string", "Task to filter decisions and workers"),
                "stale_after": property("string", "Stale worker threshold, such as 24h"),
                "pull": {"type": "boolean", "default": true}
            }),
            &[],
        ),
        definition(
            "get_onboarding_context",
            "Read onboarding context and register or resume this worker",
            json!({
                "name": property("string", "Name to register if this worker is new"),
                "resume": {"type": "boolean", "default": false}
            }),
            &[],
        ),
        definition(
            "list_workers",
            "List workers in the shared context",
            json!({"include_inactive": {"type": "boolean", "default": false}}),
            &[],
        ),
        definition(
            "update_worker",
            "Update this worker's shared state",
            worker_properties(true),
            &[],
        ),
        definition(
            "add_decision",
            "Add a project decision",
            json!({
                "title": property("string", "Decision title"),
                "context": property("string", "Background"),
                "decision": property("string", "Chosen approach"),
                "reason": property("string", "Reason for the decision"),
                "consequences": property("string", "Consequences"),
                "supersedes": string_array("IDs of replaced decisions"),
                "status": {"type": "string", "enum": ["proposed", "accepted", "rejected"], "default": "accepted"},
                "sync": {"type": "boolean", "default": false}
            }),
            &["title"],
        ),
        definition(
            "finish_worker",
            "Publish this worker's final handoff and sync",
            worker_properties(false),
            &[],
        ),
    ]
}

pub fn error_result(error: &Error) -> Value {
    let message = error.to_string();
    json!({
        "content": [{"type": "text", "text": format!("error: {message}")}],
        "structuredContent": {"exit_code": error.exit_code(), "message": message},
        "isError": true
    })
}

fn validate(name: &str, arguments: &Value) -> Result<(), Error> {
    let object = arguments
        .as_object()
        .ok_or_else(|| Error::InvalidConfig("tool arguments must be an object".into()))?;
    let definition = tool_definitions()
        .into_iter()
        .find(|tool| tool["name"] == name)
        .expect("known tool");
    let schema = &definition["inputSchema"];
    for required in schema["required"].as_array().expect("required array") {
        let key = required.as_str().expect("required string");
        if !object.contains_key(key) {
            return Err(Error::InvalidConfig(format!(
                "missing required argument: {key}"
            )));
        }
    }
    for (key, value) in object {
        let Some(property) = schema["properties"].get(key) else {
            continue;
        };
        let valid = match property["type"].as_str().expect("property type") {
            "string" => {
                value.is_string()
                    && property["enum"]
                        .as_array()
                        .is_none_or(|variants| variants.contains(value))
            }
            "boolean" => value.is_boolean(),
            "array" => value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string)),
            _ => false,
        };
        if !valid {
            return Err(Error::InvalidConfig(format!("invalid argument: {key}")));
        }
    }
    Ok(())
}

/// Returns None only for an unknown tool name.
pub fn call_tool(_project_dir: &Path, name: &str, arguments: Value) -> Option<Value> {
    if !tool_definitions().iter().any(|tool| tool["name"] == name) {
        return None;
    }
    if let Err(error) = validate(name, &arguments) {
        return Some(error_result(&error));
    }
    Some(error_result(&Error::General(format!(
        "not implemented: {name}"
    ))))
}
