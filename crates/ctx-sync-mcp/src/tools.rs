//! MCP tool declarations and shared argument validation.

use std::path::Path;

use ctx_sync_core::Error;
use ctx_sync_core::clock;
use ctx_sync_core::model::{DecisionStatus, WorkerStatus, default_stale_after, parse_duration};
use ctx_sync_core::ops::{
    self, AgentStartOptions, HandoffInput, HandoffOutcome, NewDecision, Runtime, Workspace,
};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::view::{
    ViewOptions, WorkerSummary, build_context_view, filter_context_view, render_context_markdown,
    render_onboard_markdown,
};
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
pub fn call_tool(project_dir: &Path, name: &str, arguments: Value) -> Option<Value> {
    call_tool_with_runtime(project_dir, None, name, arguments)
}

pub fn call_tool_with_runtime(
    project_dir: &Path,
    runtime: Option<&Runtime>,
    name: &str,
    arguments: Value,
) -> Option<Value> {
    if !tool_definitions().iter().any(|tool| tool["name"] == name) {
        return None;
    }
    if let Err(error) = validate(name, &arguments) {
        return Some(error_result(&error));
    }
    let runtime = match runtime.cloned().map(Ok).unwrap_or_else(Runtime::from_env) {
        Ok(runtime) => runtime,
        Err(error) => return Some(error_result(&error)),
    };
    let result = match name {
        "get_context" => get_context(project_dir, &runtime, &arguments),
        "get_onboarding_context" => get_onboarding_context(project_dir, &runtime, &arguments),
        "list_workers" => list_workers(project_dir, &runtime, &arguments),
        "update_worker" => update_worker(project_dir, &runtime, &arguments, false),
        "add_decision" => add_decision(project_dir, &runtime, &arguments),
        "finish_worker" => update_worker(project_dir, &runtime, &arguments, true),
        _ => Err(Error::General(format!("not implemented: {name}"))),
    };
    Some(result.unwrap_or_else(|error| {
        let mut result = error_result(&error);
        if matches!(error, Error::ContextConflict { .. }) {
            let text = result["content"][0]["text"].as_str().unwrap_or_default();
            result["content"][0]["text"] =
                json!(format!("{text}\nrun ctx-sync status for details"));
        }
        result
    }))
}

fn success_result(text: String, structured: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": structured
    })
}

fn get_context(project_dir: &Path, rt: &Runtime, args: &Value) -> Result<Value, Error> {
    let ws = Workspace::open(rt, project_dir)?;
    let mut warnings = Vec::new();
    if args["pull"].as_bool().unwrap_or(true)
        && let ctx_sync_core::store::PullOutcome::Diverged { ahead, behind } = ws.store.pull()?
    {
        warnings.push(format!(
            "context diverged ({ahead} ahead, {behind} behind); run `ctx-sync sync`"
        ));
    }
    let now = clock::now()?;
    let stale_after = args["stale_after"]
        .as_str()
        .map(parse_duration)
        .transpose()?
        .unwrap_or_else(default_stale_after);
    let snapshot = ws.store.snapshot()?;
    let mut view = build_context_view(&snapshot, &ViewOptions { now, stale_after });
    view.warnings.extend(warnings);
    if let Some(task) = args["task"].as_str() {
        view = filter_context_view(view, task);
    }
    let text = render_context_markdown(&view);
    Ok(success_result(
        text,
        serde_json::to_value(view).expect("serializable context view"),
    ))
}

fn get_onboarding_context(project_dir: &Path, rt: &Runtime, args: &Value) -> Result<Value, Error> {
    let outcome = ops::agent_start(
        rt,
        AgentStartOptions {
            cwd: project_dir.to_path_buf(),
            name: args["name"].as_str().map(str::to_string),
            resume: args["resume"].as_bool().unwrap_or(false),
            now: clock::now()?,
            stale_after: default_stale_after(),
        },
    )?;
    let warnings = outcome
        .warnings
        .iter()
        .map(|warning| format!("> warning: {warning}\n"))
        .collect::<String>();
    let text = format!("{warnings}{}", render_onboard_markdown(&outcome.onboard));
    Ok(success_result(
        text,
        serde_json::to_value(outcome).expect("serializable onboarding outcome"),
    ))
}

fn list_workers(project_dir: &Path, rt: &Runtime, args: &Value) -> Result<Value, Error> {
    let ws = Workspace::open(rt, project_dir)?;
    let snapshot = ws.store.snapshot()?;
    let opts = ViewOptions::new(clock::now()?);
    let workers: Vec<_> = snapshot
        .workers
        .iter()
        .filter(|worker| {
            args["include_inactive"].as_bool().unwrap_or(false) || worker.status.is_active()
        })
        .map(|worker| WorkerSummary::new(worker, &opts))
        .collect();
    let text = if workers.is_empty() {
        "No workers.".into()
    } else {
        workers
            .iter()
            .map(|worker| {
                format!(
                    "{} ({}) {}: {}",
                    worker.name, worker.short_id, worker.status, worker.task
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(success_result(text, json!({"workers": workers})))
}

fn strings(args: &Value, key: &str) -> Vec<String> {
    args[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_strings(args: &Value, key: &str) -> Option<Vec<String>> {
    let items = strings(args, key);
    (!items.is_empty()).then_some(items)
}

fn handoff_input(args: &Value) -> HandoffInput {
    let status = match args["status"].as_str() {
        Some("working") => Some(WorkerStatus::Working),
        Some("blocked") => Some(WorkerStatus::Blocked),
        Some("done") => Some(WorkerStatus::Done),
        Some("abandoned") => Some(WorkerStatus::Abandoned),
        _ => None,
    };
    HandoffInput {
        task: args["task"].as_str().map(str::to_string),
        summary: args["summary"].as_str().map(str::to_string),
        status,
        working_on: optional_strings(args, "working_on"),
        changed: optional_strings(args, "changed"),
        interface_changes: optional_strings(args, "interface_changes"),
        attention: optional_strings(args, "attention"),
        blocked_by: optional_strings(args, "blocked_by"),
        append: args["append"].as_bool().unwrap_or(false),
    }
}

fn warnings_text(warnings: &[String], sync: Option<&ctx_sync_core::store::SyncOutcome>) -> String {
    warnings
        .iter()
        .chain(sync.into_iter().flat_map(|outcome| outcome.warnings.iter()))
        .map(|warning| format!("warning: {warning}\n"))
        .collect()
}

fn update_worker(
    project_dir: &Path,
    rt: &Runtime,
    args: &Value,
    finish: bool,
) -> Result<Value, Error> {
    let ws = Workspace::open(rt, project_dir)?;
    let input = handoff_input(args);
    let now = clock::now()?;
    let outcome = if finish {
        ops::agent_finish(&ws, input, now)?
    } else {
        ops::handoff(&ws, input, args["sync"].as_bool().unwrap_or(false), now)?
    };
    let text = if finish {
        format!(
            "# Handoff Complete\n\nworker: {} ({})\nstatus: {}\nfile: {}\nrevision: {}\n",
            outcome.worker_name,
            outcome.short_id,
            outcome.status,
            outcome.file_name,
            outcome.sync.as_ref().expect("finish always syncs").revision
        )
    } else {
        handoff_text(&outcome)
    };
    let text = format!(
        "{text}{}",
        warnings_text(&outcome.warnings, outcome.sync.as_ref())
    );
    Ok(success_result(
        text,
        serde_json::to_value(outcome).expect("serializable handoff outcome"),
    ))
}

fn handoff_text(outcome: &HandoffOutcome) -> String {
    let committed = outcome.committed.as_deref().unwrap_or("no changes");
    let synced = outcome
        .sync
        .as_ref()
        .map(|sync| sync.revision.as_str())
        .unwrap_or("no (run `ctx-sync sync`)");
    format!(
        "Updated worker {} ({})\n\nstatus: {}\nfile: {}\ncommitted: {committed}\nsynced: {synced}\n",
        outcome.worker_name, outcome.short_id, outcome.status, outcome.file_name
    )
}

fn add_decision(project_dir: &Path, rt: &Runtime, args: &Value) -> Result<Value, Error> {
    let ws = Workspace::open(rt, project_dir)?;
    let status = match args["status"].as_str().unwrap_or("accepted") {
        "proposed" => DecisionStatus::Proposed,
        "rejected" => DecisionStatus::Rejected,
        _ => DecisionStatus::Accepted,
    };
    let input = NewDecision {
        title: args["title"].as_str().expect("validated title").into(),
        context: args["context"].as_str().map(str::to_string),
        decision: args["decision"].as_str().map(str::to_string),
        reason: args["reason"].as_str().map(str::to_string),
        consequences: args["consequences"].as_str().map(str::to_string),
        supersedes: strings(args, "supersedes"),
        status,
    };
    let outcome = ops::add_decision(
        &ws,
        input,
        args["sync"].as_bool().unwrap_or(false),
        clock::now()?,
    )?;
    let committed = outcome.committed.as_deref().unwrap_or("no changes");
    let synced = outcome
        .sync
        .as_ref()
        .map(|sync| sync.revision.as_str())
        .unwrap_or("no (run `ctx-sync sync`)");
    let text = format!(
        "Added decision\n\nid: {}\nfile: {}\ncommitted: {committed}\nsynced: {synced}\n{}",
        outcome.id,
        outcome.file_name,
        warnings_text(&outcome.warnings, outcome.sync.as_ref())
    );
    Ok(success_result(
        text,
        serde_json::to_value(outcome).expect("serializable decision outcome"),
    ))
}
