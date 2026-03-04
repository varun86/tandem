use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use futures::Stream;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::http::context_types::*;
use crate::AppState;

pub(super) fn context_runs_root(state: &AppState) -> PathBuf {
    state
        .shared_resources_path
        .parent()
        .map(|parent| parent.join("context_runs"))
        .unwrap_or_else(|| PathBuf::from(".tandem").join("context_runs"))
}

pub(super) fn context_run_dir(state: &AppState, run_id: &str) -> PathBuf {
    context_runs_root(state).join(run_id)
}

pub(super) fn context_run_state_path(state: &AppState, run_id: &str) -> PathBuf {
    context_run_dir(state, run_id).join("run_state.json")
}

pub(super) fn context_run_events_path(state: &AppState, run_id: &str) -> PathBuf {
    context_run_dir(state, run_id).join("events.jsonl")
}

pub(super) fn context_run_blackboard_path(state: &AppState, run_id: &str) -> PathBuf {
    context_run_dir(state, run_id).join("blackboard.json")
}

pub(super) fn context_run_blackboard_patches_path(state: &AppState, run_id: &str) -> PathBuf {
    context_run_dir(state, run_id).join("blackboard_patches.jsonl")
}

pub(super) fn context_run_checkpoints_dir(state: &AppState, run_id: &str) -> PathBuf {
    context_run_dir(state, run_id).join("checkpoints")
}

pub(super) async fn ensure_context_run_dir(
    state: &AppState,
    run_id: &str,
) -> Result<(), StatusCode> {
    let run_dir = context_run_dir(state, run_id);
    tokio::fs::create_dir_all(&run_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub(super) async fn load_context_run_state(
    state: &AppState,
    run_id: &str,
) -> Result<ContextRunState, StatusCode> {
    let path = context_run_state_path(state, run_id);
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    serde_json::from_str::<ContextRunState>(&raw).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) async fn save_context_run_state(
    state: &AppState,
    run: &ContextRunState,
) -> Result<(), StatusCode> {
    ensure_context_run_dir(state, &run.run_id).await?;
    let path = context_run_state_path(state, &run.run_id);
    let payload =
        serde_json::to_string_pretty(run).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tokio::fs::write(path, payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) fn load_context_run_events_jsonl(
    path: &FsPath,
    since_seq: Option<u64>,
    tail: Option<usize>,
) -> Vec<ContextRunEventRecord> {
    let content = match std::fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let mut rows: Vec<ContextRunEventRecord> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<ContextRunEventRecord>(line).ok())
        .filter(|row| {
            if let Some(since) = since_seq {
                return row.seq > since;
            }
            true
        })
        .collect();
    rows.sort_by_key(|row| row.seq);
    if let Some(tail_count) = tail {
        if rows.len() > tail_count {
            rows = rows.split_off(rows.len().saturating_sub(tail_count));
        }
    }
    rows
}

pub(super) fn latest_context_run_event_seq(path: &FsPath) -> u64 {
    load_context_run_events_jsonl(path, None, None)
        .last()
        .map(|row| row.seq)
        .unwrap_or(0)
}

pub(super) fn append_jsonl_line(path: &FsPath, value: &Value) -> Result<(), StatusCode> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let line = serde_json::to_string(value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    writeln!(file, "{}", line).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) fn load_context_blackboard(state: &AppState, run_id: &str) -> ContextBlackboardState {
    let path = context_run_blackboard_path(state, run_id);
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str::<ContextBlackboardState>(&raw).unwrap_or_default(),
        Err(_) => ContextBlackboardState::default(),
    }
}

pub(super) fn save_context_blackboard(
    state: &AppState,
    run_id: &str,
    blackboard: &ContextBlackboardState,
) -> Result<(), StatusCode> {
    let path = context_run_blackboard_path(state, run_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let payload =
        serde_json::to_string_pretty(blackboard).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(path, payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) fn apply_context_blackboard_patch(
    blackboard: &mut ContextBlackboardState,
    patch: &ContextBlackboardPatchRecord,
) -> Result<(), StatusCode> {
    let mut task_idx: HashMap<String, usize> = HashMap::new();
    for (idx, task) in blackboard.tasks.iter().enumerate() {
        task_idx.insert(task.id.clone(), idx);
    }
    match patch.op {
        ContextBlackboardPatchOp::AddFact => {
            let row = serde_json::from_value::<ContextBlackboardItem>(patch.payload.clone())
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            blackboard.facts.push(row);
        }
        ContextBlackboardPatchOp::AddDecision => {
            let row = serde_json::from_value::<ContextBlackboardItem>(patch.payload.clone())
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            blackboard.decisions.push(row);
        }
        ContextBlackboardPatchOp::AddOpenQuestion => {
            let row = serde_json::from_value::<ContextBlackboardItem>(patch.payload.clone())
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            blackboard.open_questions.push(row);
        }
        ContextBlackboardPatchOp::AddArtifact => {
            let row = serde_json::from_value::<ContextBlackboardArtifact>(patch.payload.clone())
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            blackboard.artifacts.push(row);
        }
        ContextBlackboardPatchOp::SetRollingSummary => {
            let value = patch
                .payload
                .as_str()
                .ok_or(StatusCode::BAD_REQUEST)?
                .to_string();
            blackboard.summaries.rolling = value;
        }
        ContextBlackboardPatchOp::SetLatestContextPack => {
            let value = patch
                .payload
                .as_str()
                .ok_or(StatusCode::BAD_REQUEST)?
                .to_string();
            blackboard.summaries.latest_context_pack = value;
        }
        ContextBlackboardPatchOp::AddTask => {
            let task = serde_json::from_value::<ContextBlackboardTask>(patch.payload.clone())
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            if let Some(idx) = task_idx.get(&task.id).copied() {
                blackboard.tasks[idx] = task;
            } else {
                blackboard.tasks.push(task);
            }
        }
        ContextBlackboardPatchOp::UpdateTaskLease => {
            let task_id = patch
                .payload
                .get("task_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or(StatusCode::BAD_REQUEST)?
                .to_string();
            let Some(idx) = task_idx.get(&task_id).copied() else {
                return Err(StatusCode::NOT_FOUND);
            };
            let task = blackboard.tasks.get_mut(idx).ok_or(StatusCode::NOT_FOUND)?;
            task.lease_owner = patch
                .payload
                .get("lease_owner")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            task.lease_token = patch
                .payload
                .get("lease_token")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            task.lease_expires_at_ms = patch
                .payload
                .get("lease_expires_at_ms")
                .and_then(Value::as_u64);
            if let Some(agent_id) = patch
                .payload
                .get("assigned_agent")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                task.assigned_agent = Some(agent_id.to_string());
            }
            if let Some(next_rev) = patch.payload.get("task_rev").and_then(Value::as_u64) {
                task.task_rev = next_rev;
            } else {
                task.task_rev = task.task_rev.saturating_add(1);
            }
            task.updated_ts = patch.ts_ms;
        }
        ContextBlackboardPatchOp::UpdateTaskState => {
            let task_id = patch
                .payload
                .get("task_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or(StatusCode::BAD_REQUEST)?
                .to_string();
            let Some(idx) = task_idx.get(&task_id).copied() else {
                return Err(StatusCode::NOT_FOUND);
            };
            let task = blackboard.tasks.get_mut(idx).ok_or(StatusCode::NOT_FOUND)?;
            let status = patch
                .payload
                .get("status")
                .cloned()
                .ok_or(StatusCode::BAD_REQUEST)
                .and_then(|row| {
                    serde_json::from_value::<ContextBlackboardTaskStatus>(row)
                        .map_err(|_| StatusCode::BAD_REQUEST)
                })?;
            task.status = status;
            if let Some(lease_owner) = patch
                .payload
                .get("lease_owner")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                task.lease_owner = Some(lease_owner.to_string());
            }
            if patch.payload.get("lease_owner").is_some()
                && patch
                    .payload
                    .get("lease_owner")
                    .and_then(Value::as_str)
                    .is_none()
            {
                task.lease_owner = None;
            }
            if let Some(lease_token) = patch
                .payload
                .get("lease_token")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                task.lease_token = Some(lease_token.to_string());
            }
            if patch.payload.get("lease_token").is_some()
                && patch
                    .payload
                    .get("lease_token")
                    .and_then(Value::as_str)
                    .is_none()
            {
                task.lease_token = None;
            }
            task.lease_expires_at_ms = patch
                .payload
                .get("lease_expires_at_ms")
                .and_then(Value::as_u64);
            if let Some(agent_id) = patch
                .payload
                .get("assigned_agent")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                task.assigned_agent = Some(agent_id.to_string());
            }
            if let Some(err_text) = patch
                .payload
                .get("error")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                task.last_error = Some(err_text.to_string());
            }
            if let Some(next_retry_at_ms) = patch
                .payload
                .get("next_retry_at_ms")
                .and_then(Value::as_u64)
            {
                task.next_retry_at_ms = Some(next_retry_at_ms);
            }
            if let Some(next_attempt) = patch.payload.get("attempt").and_then(Value::as_u64) {
                task.attempt = next_attempt as u32;
            }
            if let Some(next_rev) = patch.payload.get("task_rev").and_then(Value::as_u64) {
                task.task_rev = next_rev;
            } else {
                task.task_rev = task.task_rev.saturating_add(1);
            }
            task.updated_ts = patch.ts_ms;
        }
    }
    blackboard.revision = patch.seq;
    Ok(())
}

pub(super) fn load_context_blackboard_patches(
    state: &AppState,
    run_id: &str,
    since_seq: Option<u64>,
    tail: Option<usize>,
) -> Vec<ContextBlackboardPatchRecord> {
    let path = context_run_blackboard_patches_path(state, run_id);
    let rows = std::fs::read_to_string(path).unwrap_or_default();
    let mut parsed = rows
        .lines()
        .filter_map(|line| serde_json::from_str::<ContextBlackboardPatchRecord>(line).ok())
        .filter(|row| {
            if let Some(min_seq) = since_seq {
                return row.seq > min_seq;
            }
            true
        })
        .collect::<Vec<_>>();
    parsed.sort_by_key(|row| row.seq);
    if let Some(tail_count) = tail {
        if parsed.len() > tail_count {
            parsed = parsed.split_off(parsed.len().saturating_sub(tail_count));
        }
    }
    parsed
}

pub(super) fn next_context_blackboard_patch_seq(state: &AppState, run_id: &str) -> u64 {
    load_context_blackboard_patches(state, run_id, None, Some(1))
        .last()
        .map(|row| row.seq)
        .unwrap_or(0)
        .saturating_add(1)
}

pub(super) fn append_context_blackboard_patch(
    state: &AppState,
    run_id: &str,
    op: ContextBlackboardPatchOp,
    payload: Value,
) -> Result<(ContextBlackboardPatchRecord, ContextBlackboardState), StatusCode> {
    let patch = ContextBlackboardPatchRecord {
        patch_id: format!("bbp-{}", Uuid::new_v4()),
        run_id: run_id.to_string(),
        seq: next_context_blackboard_patch_seq(state, run_id),
        ts_ms: crate::now_ms(),
        op,
        payload,
    };
    append_jsonl_line(
        &context_run_blackboard_patches_path(state, run_id),
        &serde_json::to_value(&patch).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    )?;
    let mut blackboard = load_context_blackboard(state, run_id);
    apply_context_blackboard_patch(&mut blackboard, &patch)?;
    save_context_blackboard(state, run_id, &blackboard)?;
    Ok((patch, blackboard))
}

pub(super) fn context_task_status_event_name(status: &ContextBlackboardTaskStatus) -> &'static str {
    match status {
        ContextBlackboardTaskStatus::InProgress => "context.task.started",
        ContextBlackboardTaskStatus::Done => "context.task.completed",
        ContextBlackboardTaskStatus::Failed => "context.task.failed",
        ContextBlackboardTaskStatus::Blocked => "context.task.blocked",
        ContextBlackboardTaskStatus::Runnable | ContextBlackboardTaskStatus::Pending => {
            "context.task.requeued"
        }
    }
}

pub(super) fn context_run_lock_map(
) -> &'static std::sync::OnceLock<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>
{
    static LOCKS: std::sync::OnceLock<
        tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    > = std::sync::OnceLock::new();
    &LOCKS
}

pub(super) async fn context_run_lock_for(run_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    let map = context_run_lock_map().get_or_init(|| tokio::sync::Mutex::new(HashMap::new()));
    let mut guard = map.lock().await;
    guard
        .entry(run_id.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

pub(super) async fn context_run_create(
    State(state): State<AppState>,
    Json(input): Json<ContextRunCreateInput>,
) -> Result<Json<Value>, StatusCode> {
    let run_id = input
        .run_id
        .unwrap_or_else(|| format!("run-{}", Uuid::new_v4()));
    ensure_context_run_dir(&state, &run_id).await?;
    let run_path = context_run_state_path(&state, &run_id);
    if run_path.exists() {
        return Ok(Json(json!({
            "error": "run already exists",
            "code": "CONTEXT_RUN_EXISTS",
            "run_id": run_id
        })));
    }
    let now = crate::now_ms();
    let run = ContextRunState {
        run_id: run_id.clone(),
        run_type: input.run_type.unwrap_or_else(|| "interactive".to_string()),
        source_client: input
            .source_client
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        model_provider: input
            .model_provider
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        model_id: input
            .model_id
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        mcp_servers: input
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>(),
        status: ContextRunStatus::Queued,
        objective: input.objective,
        workspace: input.workspace.unwrap_or_default(),
        steps: Vec::new(),
        why_next_step: None,
        revision: 1,
        created_at_ms: now,
        started_at_ms: None,
        ended_at_ms: None,
        last_error: None,
        updated_at_ms: now,
    };
    save_context_run_state(&state, &run).await?;
    Ok(Json(json!({ "ok": true, "run": run })))
}

pub(super) async fn context_run_list(
    State(state): State<AppState>,
    Query(query): Query<ContextRunListQuery>,
) -> Result<Json<Value>, StatusCode> {
    let root = context_runs_root(&state);
    if !root.exists() {
        return Ok(Json(json!({ "runs": [] })));
    }
    let workspace_filter = query
        .workspace
        .as_deref()
        .and_then(tandem_core::normalize_workspace_path);
    let run_type_filter = query
        .run_type
        .as_ref()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty());
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let mut rows = Vec::<ContextRunState>::new();
    let mut dir = tokio::fs::read_dir(root)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    while let Ok(Some(entry)) = dir.next_entry().await {
        if !entry
            .file_type()
            .await
            .map(|kind| kind.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let run_id = entry.file_name().to_string_lossy().to_string();
        if let Ok(run) = load_context_run_state(&state, &run_id).await {
            if let Some(workspace) = workspace_filter.as_deref() {
                if run.workspace.canonical_path.trim() != workspace {
                    continue;
                }
            }
            if let Some(run_type) = run_type_filter.as_deref() {
                if run.run_type.trim().to_ascii_lowercase() != run_type {
                    continue;
                }
            }
            rows.push(run);
        }
    }
    rows.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    rows.truncate(limit);
    Ok(Json(json!({ "runs": rows })))
}

pub(super) async fn context_run_get(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let run = load_context_run_state(&state, &run_id).await?;
    Ok(Json(json!({ "run": run })))
}

pub(super) async fn context_run_put(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(mut run): Json<ContextRunState>,
) -> Result<Json<Value>, StatusCode> {
    if run.run_id != run_id {
        return Err(StatusCode::BAD_REQUEST);
    }
    let now = crate::now_ms();
    run.revision = run.revision.saturating_add(1);
    run.updated_at_ms = now;
    save_context_run_state(&state, &run).await?;
    Ok(Json(json!({ "ok": true, "run": run })))
}

pub(super) fn context_run_is_terminal(status: &ContextRunStatus) -> bool {
    matches!(
        status,
        ContextRunStatus::Failed | ContextRunStatus::Completed | ContextRunStatus::Cancelled
    )
}

pub(super) fn context_step_status_from_todo(status: Option<&str>) -> ContextStepStatus {
    let normalized = status
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "pending".to_string());
    match normalized.as_str() {
        "in_progress" | "in-progress" | "working" | "doing" | "active" => {
            ContextStepStatus::InProgress
        }
        "runnable" | "ready" => ContextStepStatus::Runnable,
        "done" | "completed" | "complete" => ContextStepStatus::Done,
        "blocked" => ContextStepStatus::Blocked,
        "failed" | "error" => ContextStepStatus::Failed,
        _ => ContextStepStatus::Pending,
    }
}

pub(super) fn normalize_context_todo_step_id(raw: Option<&str>, idx: usize) -> String {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("todo-{}", idx.saturating_add(1)))
}

pub(super) async fn context_run_todos_sync(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextTodoSyncInput>,
) -> Result<Json<Value>, StatusCode> {
    let mut run = load_context_run_state(&state, &run_id).await?;
    let replace = input.replace.unwrap_or(true);

    let mapped_steps = input
        .todos
        .iter()
        .enumerate()
        .map(|(idx, todo)| ContextRunStep {
            step_id: normalize_context_todo_step_id(todo.id.as_deref(), idx),
            title: todo.content.trim().to_string(),
            status: context_step_status_from_todo(todo.status.as_deref()),
        })
        .filter(|step| !step.title.is_empty())
        .collect::<Vec<_>>();

    if replace {
        run.steps = mapped_steps;
    } else {
        for step in mapped_steps {
            if let Some(existing) = run.steps.iter_mut().find(|row| row.step_id == step.step_id) {
                existing.title = step.title;
                existing.status = step.status;
            } else {
                run.steps.push(step);
            }
        }
    }

    if context_run_is_terminal(&run.status) {
        // keep terminal status unchanged
    } else if run
        .steps
        .iter()
        .any(|step| matches!(step.status, ContextStepStatus::InProgress))
    {
        run.status = ContextRunStatus::Running;
    } else if run
        .steps
        .iter()
        .any(|step| matches!(step.status, ContextStepStatus::Runnable))
    {
        run.status = ContextRunStatus::Planning;
    } else if run
        .steps
        .iter()
        .all(|step| matches!(step.status, ContextStepStatus::Done))
        && !run.steps.is_empty()
    {
        run.status = ContextRunStatus::Completed;
    } else if !run.steps.is_empty() {
        run.status = ContextRunStatus::Planning;
    }

    run.why_next_step = run
        .steps
        .iter()
        .find(|step| {
            matches!(
                step.status,
                ContextStepStatus::InProgress
                    | ContextStepStatus::Runnable
                    | ContextStepStatus::Pending
            )
        })
        .map(|step| format!("continue task `{}` from synced todo list", step.step_id));

    run.revision = run.revision.saturating_add(1);
    run.updated_at_ms = crate::now_ms();
    save_context_run_state(&state, &run).await?;

    let _ = context_run_event_append(
        State(state.clone()),
        Path(run_id.clone()),
        Json(ContextRunEventAppendInput {
            event_type: "todo_synced".to_string(),
            status: run.status.clone(),
            step_id: None,
            payload: json!({
                "count": run.steps.len(),
                "replace": replace,
                "source_session_id": input.source_session_id,
                "source_run_id": input.source_run_id,
                "todos": input.todos,
                "why_next_step": run.why_next_step.clone(),
            }),
        }),
    )
    .await;

    Ok(Json(json!({
        "ok": true,
        "run": run
    })))
}

pub(super) fn context_driver_select_next_step(
    run: &ContextRunState,
) -> (Option<usize>, String, ContextRunStatus) {
    if context_run_is_terminal(&run.status) {
        return (
            None,
            format!(
                "run is terminal (`{}`); no next step can be selected",
                serde_json::to_string(&run.status).unwrap_or_else(|_| "\"terminal\"".to_string())
            ),
            run.status.clone(),
        );
    }
    if let Some(step) = run
        .steps
        .iter()
        .find(|step| matches!(step.status, ContextStepStatus::InProgress))
    {
        return (
            None,
            format!(
                "step `{}` is already in_progress; keep current execution focus",
                step.step_id
            ),
            ContextRunStatus::Running,
        );
    }
    if let Some((idx, step)) = run
        .steps
        .iter()
        .enumerate()
        .find(|(_, step)| matches!(step.status, ContextStepStatus::Runnable))
    {
        return (
            Some(idx),
            format!(
                "selected runnable step `{}` as next execution target",
                step.step_id
            ),
            ContextRunStatus::Running,
        );
    }
    if let Some((idx, step)) = run
        .steps
        .iter()
        .enumerate()
        .find(|(_, step)| matches!(step.status, ContextStepStatus::Pending))
    {
        return (
            Some(idx),
            format!(
                "no runnable step available; promoted pending step `{}` for execution",
                step.step_id
            ),
            ContextRunStatus::Running,
        );
    }
    if !run.steps.is_empty()
        && run
            .steps
            .iter()
            .all(|step| matches!(step.status, ContextStepStatus::Done))
    {
        return (
            None,
            "all steps are done; marking run completed".to_string(),
            ContextRunStatus::Completed,
        );
    }
    if run
        .steps
        .iter()
        .any(|step| matches!(step.status, ContextStepStatus::Failed))
    {
        return (
            None,
            "one or more steps failed and no runnable work remains; run is blocked".to_string(),
            ContextRunStatus::Blocked,
        );
    }
    (
        None,
        "no actionable steps found; run remains blocked".to_string(),
        ContextRunStatus::Blocked,
    )
}

pub(super) async fn context_run_driver_next(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextDriverNextInput>,
) -> Result<Json<Value>, StatusCode> {
    let mut run = load_context_run_state(&state, &run_id).await?;
    let dry_run = input.dry_run.unwrap_or(false);
    let (selected_idx, why_next_step, target_status) = context_driver_select_next_step(&run);

    let selected_step_id = selected_idx.map(|idx| run.steps[idx].step_id.clone());
    let selected_step_status = selected_idx.map(|idx| {
        if matches!(run.steps[idx].status, ContextStepStatus::Pending) {
            "pending"
        } else {
            "runnable"
        }
    });

    if !dry_run {
        if let Some(idx) = selected_idx {
            if matches!(run.steps[idx].status, ContextStepStatus::Pending) {
                run.steps[idx].status = ContextStepStatus::Runnable;
            }
            run.steps[idx].status = ContextStepStatus::InProgress;
        }
        run.status = target_status.clone();
        run.why_next_step = Some(why_next_step.clone());
        run.revision = run.revision.saturating_add(1);
        run.updated_at_ms = crate::now_ms();
        save_context_run_state(&state, &run).await?;

        let _ = context_run_event_append(
            State(state.clone()),
            Path(run_id.clone()),
            Json(ContextRunEventAppendInput {
                event_type: "meta_next_step_selected".to_string(),
                status: target_status.clone(),
                step_id: selected_step_id.clone(),
                payload: json!({
                    "why_next_step": why_next_step,
                    "selected_step_id": selected_step_id,
                    "selected_step_previous_status": selected_step_status,
                    "driver": "context_driver_v1"
                }),
            }),
        )
        .await;
    }

    Ok(Json(json!({
        "ok": true,
        "dry_run": dry_run,
        "run_id": run_id,
        "selected_step_id": selected_step_id,
        "target_status": target_status,
        "why_next_step": why_next_step,
        "run": if dry_run { serde_json::to_value(&run).unwrap_or_else(|_| json!(null)) } else { serde_json::to_value(&run).unwrap_or_else(|_| json!(null)) }
    })))
}

pub(super) fn context_event_payload_text(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

pub(super) fn context_event_step_status(payload: &Value) -> Option<ContextStepStatus> {
    let text = context_event_payload_text(payload, "step_status")
        .or_else(|| context_event_payload_text(payload, "stepStatus"))
        .or_else(|| context_event_payload_text(payload, "step-state"))?;
    parse_context_step_status_text(&text)
}

pub(super) fn materialize_plan_steps_from_objective(objective: &str) -> Vec<ContextRunStep> {
    let mut steps = objective
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let normalized = line
                .trim_start_matches(|c: char| {
                    matches!(c, '-' | '*' | '#' | '0'..='9' | '.' | ')' | '[' | ']')
                })
                .trim();
            if normalized.len() < 8 {
                None
            } else {
                Some(normalized.to_string())
            }
        })
        .take(6)
        .enumerate()
        .map(|(idx, title)| ContextRunStep {
            step_id: format!("step-{}", idx.saturating_add(1)),
            title,
            status: ContextStepStatus::Pending,
        })
        .collect::<Vec<_>>();

    if steps.is_empty() {
        steps = vec![
            ContextRunStep {
                step_id: "step-1".to_string(),
                title: "Plan implementation approach".to_string(),
                status: ContextStepStatus::Pending,
            },
            ContextRunStep {
                step_id: "step-2".to_string(),
                title: "Execute implementation and produce artifacts".to_string(),
                status: ContextStepStatus::Pending,
            },
            ContextRunStep {
                step_id: "step-3".to_string(),
                title: "Validate and finalize output".to_string(),
                status: ContextStepStatus::Pending,
            },
        ];
    }
    steps
}

pub(super) fn apply_context_event_transition(
    run: &mut ContextRunState,
    input: &ContextRunEventAppendInput,
) {
    let event_type = input.event_type.trim().to_ascii_lowercase();
    let event_step_id = input
        .step_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
    match event_type.as_str() {
        "planning_started" => {
            if run.steps.is_empty() {
                run.steps = materialize_plan_steps_from_objective(&run.objective);
            }
            run.status = ContextRunStatus::AwaitingApproval;
            run.why_next_step = Some("plan generated and awaiting approval".to_string());
            run.started_at_ms = run.started_at_ms.or(Some(crate::now_ms()));
            run.ended_at_ms = None;
            run.last_error = None;
        }
        "plan_approved" => {
            run.status = ContextRunStatus::Running;
            run.started_at_ms = run.started_at_ms.or(Some(crate::now_ms()));
            run.ended_at_ms = None;
            run.last_error = None;
            if let Some(step) = run.steps.iter_mut().find(|step| {
                matches!(
                    step.status,
                    ContextStepStatus::Pending | ContextStepStatus::Runnable
                )
            }) {
                step.status = ContextStepStatus::InProgress;
                run.why_next_step =
                    Some(format!("executing step `{}` after approval", step.step_id));
            }
        }
        "run_paused" => {
            run.status = ContextRunStatus::Paused;
            run.why_next_step = Some("run paused by operator".to_string());
        }
        "run_resumed" => {
            run.status = ContextRunStatus::Running;
            run.why_next_step = Some("run resumed by operator".to_string());
            run.ended_at_ms = None;
        }
        "run_cancelled" => {
            run.status = ContextRunStatus::Cancelled;
            run.why_next_step = Some("run cancelled by operator".to_string());
            run.ended_at_ms = Some(crate::now_ms());
        }
        "task_retry_requested" => {
            if let Some(step_id) = input
                .step_id
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                if let Some(step) = run.steps.iter_mut().find(|step| step.step_id == step_id) {
                    step.status = ContextStepStatus::Runnable;
                }
                run.why_next_step = Some(format!("retry requested for step `{step_id}`"));
            }
            run.status = ContextRunStatus::Running;
            run.ended_at_ms = None;
            run.last_error = None;
        }
        "revision_requested" => {
            run.status = ContextRunStatus::Blocked;
            let feedback =
                context_event_payload_text(&input.payload, "feedback").unwrap_or_default();
            run.why_next_step = Some(if feedback.is_empty() {
                "revision requested by operator".to_string()
            } else {
                format!("revision requested: {feedback}")
            });
        }
        "step_started" | "task_started" => {
            run.status = input.status.clone();
            if let Some(step_id) = event_step_id.clone() {
                if let Some(step) = run.steps.iter_mut().find(|step| step.step_id == step_id) {
                    step.status = context_event_step_status(&input.payload)
                        .unwrap_or(ContextStepStatus::InProgress);
                }
            }
            if let Some(why) = context_event_payload_text(&input.payload, "why_next_step") {
                run.why_next_step = Some(why);
            }
        }
        "step_completed" | "task_completed" | "step_done" => {
            run.status = input.status.clone();
            if let Some(step_id) = event_step_id.clone() {
                if let Some(step) = run.steps.iter_mut().find(|step| step.step_id == step_id) {
                    step.status = context_event_step_status(&input.payload)
                        .unwrap_or(ContextStepStatus::Done);
                }
            }
            if let Some(why) = context_event_payload_text(&input.payload, "why_next_step") {
                run.why_next_step = Some(why);
            }
        }
        "step_failed" | "task_failed" => {
            run.status = input.status.clone();
            if let Some(step_id) = event_step_id.clone() {
                if let Some(step) = run.steps.iter_mut().find(|step| step.step_id == step_id) {
                    step.status = context_event_step_status(&input.payload)
                        .unwrap_or(ContextStepStatus::Failed);
                }
            }
            if let Some(err_text) = context_event_payload_text(&input.payload, "error") {
                run.last_error = Some(err_text);
            }
            if let Some(why) = context_event_payload_text(&input.payload, "why_next_step") {
                run.why_next_step = Some(why);
            }
        }
        _ => {
            run.status = input.status.clone();
            if let Some(step_id) = event_step_id.clone() {
                if let Some(step_status) = context_event_step_status(&input.payload) {
                    if let Some(step) = run.steps.iter_mut().find(|step| step.step_id == step_id) {
                        step.status = step_status;
                    }
                }
            }
            if let Some(why) = context_event_payload_text(&input.payload, "why_next_step") {
                run.why_next_step = Some(why);
            }
            if let Some(err_text) = context_event_payload_text(&input.payload, "error") {
                run.last_error = Some(err_text);
            }
        }
    }

    if matches!(
        run.status,
        ContextRunStatus::Completed | ContextRunStatus::Failed | ContextRunStatus::Cancelled
    ) {
        run.ended_at_ms = Some(crate::now_ms());
    }
}

pub(super) async fn context_run_event_append(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextRunEventAppendInput>,
) -> Result<Json<Value>, StatusCode> {
    let events_path = context_run_events_path(&state, &run_id);
    let seq = latest_context_run_event_seq(&events_path).saturating_add(1);
    let record = ContextRunEventRecord {
        event_id: format!("evt-{}", Uuid::new_v4()),
        run_id: run_id.clone(),
        seq,
        ts_ms: crate::now_ms(),
        event_type: input.event_type.clone(),
        status: input.status.clone(),
        step_id: input.step_id.clone(),
        payload: input.payload.clone(),
    };
    append_jsonl_line(
        &events_path,
        &serde_json::to_value(&record).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    )?;

    if let Ok(mut run) = load_context_run_state(&state, &run_id).await {
        apply_context_event_transition(&mut run, &input);
        run.revision = run.revision.saturating_add(1);
        run.updated_at_ms = crate::now_ms();
        save_context_run_state(&state, &run).await?;
    }

    Ok(Json(json!({ "ok": true, "event": record })))
}

pub(super) async fn context_run_events(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<super::RunEventsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let rows = load_context_run_events_jsonl(
        &context_run_events_path(&state, &run_id),
        query.since_seq,
        query.tail,
    );
    Ok(Json(json!({ "events": rows })))
}

pub(super) fn context_run_events_sse_stream(
    state: AppState,
    run_id: String,
    query: super::RunEventsQuery,
) -> impl Stream<Item = Result<Event, std::convert::Infallible>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(256);
    tokio::spawn(async move {
        let events_path = context_run_events_path(&state, &run_id);
        let ready = serde_json::to_string(&json!({
            "status":"ready",
            "stream":"context_run_events",
            "runID": run_id,
            "timestamp_ms": crate::now_ms(),
            "path": events_path.to_string_lossy().to_string(),
        }))
        .unwrap_or_default();
        if tx.send(ready).await.is_err() {
            return;
        }

        let initial = load_context_run_events_jsonl(&events_path, query.since_seq, query.tail);
        let mut last_seq = query.since_seq.unwrap_or(0);
        for row in initial {
            last_seq = last_seq.max(row.seq);
            let payload = serde_json::to_string(&json!({
                "type":"run.event",
                "properties": row
            }))
            .unwrap_or_default();
            if tx.send(payload).await.is_err() {
                return;
            }
        }

        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let updates = load_context_run_events_jsonl(&events_path, Some(last_seq), None);
            for row in updates {
                last_seq = last_seq.max(row.seq);
                let payload = serde_json::to_string(&json!({
                    "type":"run.event",
                    "properties": row
                }))
                .unwrap_or_default();
                if tx.send(payload).await.is_err() {
                    return;
                }
            }
        }
    });
    ReceiverStream::new(rx).map(|payload| Ok(Event::default().data(payload)))
}

pub(super) async fn context_run_events_stream(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<super::RunEventsQuery>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    Sse::new(context_run_events_sse_stream(state, run_id, query))
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(10)))
}

pub(super) async fn context_run_lease_validate(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextLeaseValidateInput>,
) -> Result<Json<Value>, StatusCode> {
    let mut run = load_context_run_state(&state, &run_id).await?;
    if run.workspace.canonical_path.trim().is_empty() {
        run.workspace.canonical_path = input.current_path.clone();
        if run.workspace.lease_epoch == 0 {
            run.workspace.lease_epoch = 1;
        }
        run.revision = run.revision.saturating_add(1);
        run.updated_at_ms = crate::now_ms();
        save_context_run_state(&state, &run).await?;
        return Ok(Json(json!({ "ok": true, "mismatch": false })));
    }

    if run.workspace.canonical_path != input.current_path {
        run.status = ContextRunStatus::Paused;
        run.revision = run.revision.saturating_add(1);
        run.updated_at_ms = crate::now_ms();
        save_context_run_state(&state, &run).await?;
        let _ = context_run_event_append(
            State(state.clone()),
            Path(run_id.clone()),
            Json(ContextRunEventAppendInput {
                event_type: "workspace_mismatch".to_string(),
                status: ContextRunStatus::Paused,
                step_id: input.step_id.clone(),
                payload: json!({
                    "phase": input.phase,
                    "expected_path": run.workspace.canonical_path,
                    "actual_path": input.current_path,
                }),
            }),
        )
        .await;
        return Ok(Json(json!({
            "ok": false,
            "mismatch": true,
            "status": "paused"
        })));
    }

    Ok(Json(json!({ "ok": true, "mismatch": false })))
}

pub(super) async fn context_run_blackboard_get(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let blackboard = load_context_blackboard(&state, &run_id);
    Ok(Json(json!({ "blackboard": blackboard })))
}

pub(super) fn context_blackboard_has_command_id(
    state: &AppState,
    run_id: &str,
    command_id: &str,
) -> bool {
    if command_id.trim().is_empty() {
        return false;
    }
    load_context_blackboard_patches(state, run_id, None, Some(500))
        .iter()
        .any(|patch| {
            patch
                .payload
                .get("command_id")
                .and_then(Value::as_str)
                .map(|value| value == command_id)
                .unwrap_or(false)
        })
}

pub(super) async fn context_run_blackboard_patches_get(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<ContextBlackboardPatchesQuery>,
) -> Result<Json<Value>, StatusCode> {
    let patches = load_context_blackboard_patches(&state, &run_id, query.since_seq, query.tail);
    Ok(Json(json!({ "patches": patches })))
}

pub(super) async fn context_run_blackboard_patch(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextBlackboardPatchInput>,
) -> Result<Json<Value>, StatusCode> {
    let lock = context_run_lock_for(&run_id).await;
    let _guard = lock.lock().await;
    let (patch, blackboard) =
        append_context_blackboard_patch(&state, &run_id, input.op, input.payload)?;
    Ok(Json(
        json!({ "ok": true, "patch": patch, "blackboard": blackboard }),
    ))
}

pub(super) async fn context_run_tasks_create(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextTaskCreateBatchInput>,
) -> Result<Json<Value>, StatusCode> {
    let lock = context_run_lock_for(&run_id).await;
    let _guard = lock.lock().await;
    if input.tasks.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let run_status = load_context_run_state(&state, &run_id)
        .await
        .ok()
        .map(|run| run.status)
        .unwrap_or(ContextRunStatus::Planning);
    let mut created = Vec::<ContextBlackboardTask>::new();
    let mut patches = Vec::<ContextBlackboardPatchRecord>::new();

    for row in input.tasks {
        if row.task_type.trim().is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }
        if let Some(command_id) = row.command_id.as_deref() {
            if context_blackboard_has_command_id(&state, &run_id, command_id) {
                continue;
            }
        }
        let now = crate::now_ms();
        let task = ContextBlackboardTask {
            id: row
                .id
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| format!("task-{}", Uuid::new_v4())),
            task_type: row.task_type.trim().to_string(),
            payload: row.payload,
            status: row.status.unwrap_or(ContextBlackboardTaskStatus::Pending),
            workflow_id: row
                .workflow_id
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            workflow_node_id: row
                .workflow_node_id
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            parent_task_id: row
                .parent_task_id
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            depends_on_task_ids: row
                .depends_on_task_ids
                .into_iter()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>(),
            decision_ids: row.decision_ids,
            artifact_ids: row.artifact_ids,
            assigned_agent: None,
            priority: row.priority.unwrap_or(0),
            attempt: 0,
            max_attempts: row.max_attempts.unwrap_or(3),
            last_error: None,
            next_retry_at_ms: None,
            lease_owner: None,
            lease_token: None,
            lease_expires_at_ms: None,
            task_rev: 1,
            created_ts: now,
            updated_ts: now,
        };
        let mut payload =
            serde_json::to_value(&task).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if let Some(command_id) = row.command_id {
            payload["command_id"] = json!(command_id);
        }
        let (patch, _) = append_context_blackboard_patch(
            &state,
            &run_id,
            ContextBlackboardPatchOp::AddTask,
            payload,
        )?;
        let _ = context_run_event_append(
            State(state.clone()),
            Path(run_id.clone()),
            Json(ContextRunEventAppendInput {
                event_type: "context.task.created".to_string(),
                status: run_status.clone(),
                step_id: Some(task.id.clone()),
                payload: json!({
                    "task_id": task.id,
                    "task_type": task.task_type,
                    "patch_seq": patch.seq,
                    "task_rev": task.task_rev,
                    "workflow_id": task.workflow_id,
                }),
            }),
        )
        .await;
        patches.push(patch);
        created.push(task);
    }

    let blackboard = load_context_blackboard(&state, &run_id);
    Ok(Json(
        json!({ "ok": true, "tasks": created, "patches": patches, "blackboard": blackboard }),
    ))
}

pub(super) async fn context_run_tasks_claim(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextTaskClaimInput>,
) -> Result<Json<Value>, StatusCode> {
    let lock = context_run_lock_for(&run_id).await;
    let _guard = lock.lock().await;
    if input.agent_id.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(command_id) = input.command_id.as_deref() {
        if context_blackboard_has_command_id(&state, &run_id, command_id) {
            return Ok(Json(json!({
                "ok": true,
                "deduped": true,
                "task": null,
                "blackboard": load_context_blackboard(&state, &run_id),
            })));
        }
    }
    let run_status = load_context_run_state(&state, &run_id)
        .await
        .ok()
        .map(|run| run.status)
        .unwrap_or(ContextRunStatus::Running);
    let blackboard = load_context_blackboard(&state, &run_id);
    let now = crate::now_ms();
    let mut task_idx = blackboard
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| {
            if let Some(task_type) = input.task_type.as_deref() {
                if task.task_type != task_type {
                    return false;
                }
            }
            if let Some(workflow_id) = input.workflow_id.as_deref() {
                if task.workflow_id.as_deref() != Some(workflow_id) {
                    return false;
                }
            }
            matches!(
                task.status,
                ContextBlackboardTaskStatus::Runnable | ContextBlackboardTaskStatus::Pending
            )
        })
        .filter(|(_, task)| {
            task.depends_on_task_ids.iter().all(|dep| {
                blackboard
                    .tasks
                    .iter()
                    .find(|row| row.id == *dep)
                    .map(|row| matches!(row.status, ContextBlackboardTaskStatus::Done))
                    .unwrap_or(false)
            })
        })
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    task_idx.sort_by(|a, b| {
        let left = &blackboard.tasks[*a];
        let right = &blackboard.tasks[*b];
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.created_ts.cmp(&right.created_ts))
    });
    let Some(selected_idx) = task_idx.first().copied() else {
        return Ok(Json(json!({
            "ok": true,
            "task": null,
            "blackboard": blackboard,
        })));
    };
    let selected = blackboard.tasks[selected_idx].clone();
    let lease_ms = input.lease_ms.unwrap_or(30_000).clamp(5_000, 300_000);
    let lease_token = format!("lease-{}", Uuid::new_v4());
    let next_rev = selected.task_rev.saturating_add(1);
    let mut payload = json!({
        "task_id": selected.id,
        "status": ContextBlackboardTaskStatus::InProgress,
        "assigned_agent": input.agent_id.trim(),
        "lease_owner": input.agent_id.trim(),
        "lease_token": lease_token,
        "lease_expires_at_ms": now.saturating_add(lease_ms),
        "task_rev": next_rev,
    });
    if let Some(command_id) = input.command_id {
        payload["command_id"] = json!(command_id);
    }
    let (patch, mut blackboard) = append_context_blackboard_patch(
        &state,
        &run_id,
        ContextBlackboardPatchOp::UpdateTaskState,
        payload,
    )?;
    let claimed = blackboard
        .tasks
        .iter()
        .find(|task| task.id == selected.id)
        .cloned();
    let _ = context_run_event_append(
        State(state.clone()),
        Path(run_id.clone()),
        Json(ContextRunEventAppendInput {
            event_type: "context.task.claimed".to_string(),
            status: run_status.clone(),
            step_id: Some(selected.id.clone()),
            payload: json!({
                "task_id": selected.id,
                "agent_id": input.agent_id.trim(),
                "patch_seq": patch.seq,
                "task_rev": next_rev,
                "workflow_id": selected.workflow_id,
            }),
        }),
    )
    .await;
    if let Some(task) = claimed.as_ref() {
        let _ = context_run_event_append(
            State(state.clone()),
            Path(run_id.clone()),
            Json(ContextRunEventAppendInput {
                event_type: "context.task.started".to_string(),
                status: run_status,
                step_id: Some(task.id.clone()),
                payload: json!({
                    "task_id": task.id,
                    "patch_seq": patch.seq,
                    "task_rev": task.task_rev,
                }),
            }),
        )
        .await;
    }
    blackboard = load_context_blackboard(&state, &run_id);
    Ok(Json(
        json!({ "ok": true, "task": claimed, "patch": patch, "blackboard": blackboard }),
    ))
}

pub(super) async fn context_run_task_transition(
    State(state): State<AppState>,
    Path((run_id, task_id)): Path<(String, String)>,
    Json(input): Json<ContextTaskTransitionInput>,
) -> Result<Json<Value>, StatusCode> {
    let lock = context_run_lock_for(&run_id).await;
    let _guard = lock.lock().await;
    if let Some(command_id) = input.command_id.as_deref() {
        if context_blackboard_has_command_id(&state, &run_id, command_id) {
            return Ok(Json(json!({
                "ok": true,
                "deduped": true,
                "task": load_context_blackboard(&state, &run_id)
                    .tasks
                    .into_iter()
                    .find(|row| row.id == task_id),
                "blackboard": load_context_blackboard(&state, &run_id),
            })));
        }
    }
    let run_status = load_context_run_state(&state, &run_id)
        .await
        .ok()
        .map(|run| run.status)
        .unwrap_or(ContextRunStatus::Running);
    let blackboard = load_context_blackboard(&state, &run_id);
    let Some(current) = blackboard
        .tasks
        .iter()
        .find(|row| row.id == task_id)
        .cloned()
    else {
        return Err(StatusCode::NOT_FOUND);
    };
    if let Some(expected_rev) = input.expected_task_rev {
        if expected_rev != current.task_rev {
            return Ok(Json(json!({
                "ok": false,
                "error": "task revision mismatch",
                "code": "TASK_REV_MISMATCH",
                "task_rev": current.task_rev
            })));
        }
    }
    if let Some(token) = input.lease_token.as_deref() {
        if current.lease_token.as_deref() != Some(token) {
            return Ok(Json(json!({
                "ok": false,
                "error": "invalid lease token",
                "code": "TASK_LEASE_INVALID"
            })));
        }
    }
    let action = input.action.trim().to_ascii_lowercase();
    let next_rev = current.task_rev.saturating_add(1);
    let now = crate::now_ms();
    let (op, mut payload) = match action.as_str() {
        "heartbeat" => (
            ContextBlackboardPatchOp::UpdateTaskLease,
            json!({
                "task_id": task_id,
                "lease_owner": input.agent_id.clone().or(current.lease_owner.clone()),
                "assigned_agent": input.agent_id.clone().or(current.assigned_agent.clone()),
                "lease_token": input.lease_token.clone().or(current.lease_token.clone()),
                "lease_expires_at_ms": now.saturating_add(input.lease_ms.unwrap_or(30_000).clamp(5_000, 300_000)),
                "task_rev": next_rev,
            }),
        ),
        "status" => (
            ContextBlackboardPatchOp::UpdateTaskState,
            json!({
                "task_id": task_id,
                "status": input.status.clone().ok_or(StatusCode::BAD_REQUEST)?,
                "lease_owner": current.lease_owner,
                "lease_token": current.lease_token,
                "lease_expires_at_ms": current.lease_expires_at_ms,
                "assigned_agent": input.agent_id.clone().or(current.assigned_agent),
                "task_rev": next_rev,
                "error": input.error.clone(),
                "attempt": current.attempt,
            }),
        ),
        "complete" => (
            ContextBlackboardPatchOp::UpdateTaskState,
            json!({
                "task_id": task_id,
                "status": ContextBlackboardTaskStatus::Done,
                "lease_owner": Value::Null,
                "lease_token": Value::Null,
                "lease_expires_at_ms": Value::Null,
                "assigned_agent": input.agent_id.clone().or(current.assigned_agent),
                "task_rev": next_rev,
                "attempt": current.attempt,
            }),
        ),
        "fail" => (
            ContextBlackboardPatchOp::UpdateTaskState,
            json!({
                "task_id": task_id,
                "status": ContextBlackboardTaskStatus::Failed,
                "lease_owner": Value::Null,
                "lease_token": Value::Null,
                "lease_expires_at_ms": Value::Null,
                "assigned_agent": input.agent_id.clone().or(current.assigned_agent),
                "task_rev": next_rev,
                "error": input.error.clone().or(current.last_error),
                "attempt": current.attempt.saturating_add(1),
            }),
        ),
        "release" => (
            ContextBlackboardPatchOp::UpdateTaskState,
            json!({
                "task_id": task_id,
                "status": ContextBlackboardTaskStatus::Runnable,
                "lease_owner": Value::Null,
                "lease_token": Value::Null,
                "lease_expires_at_ms": Value::Null,
                "assigned_agent": current.assigned_agent,
                "task_rev": next_rev,
                "attempt": current.attempt,
            }),
        ),
        "retry" => (
            ContextBlackboardPatchOp::UpdateTaskState,
            json!({
                "task_id": task_id,
                "status": ContextBlackboardTaskStatus::Runnable,
                "lease_owner": Value::Null,
                "lease_token": Value::Null,
                "lease_expires_at_ms": Value::Null,
                "assigned_agent": current.assigned_agent,
                "task_rev": next_rev,
                "error": Value::Null,
                "attempt": current.attempt.saturating_add(1),
                "next_retry_at_ms": now,
            }),
        ),
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    if let Some(command_id) = input.command_id {
        payload["command_id"] = json!(command_id);
    }
    let (patch, blackboard) = append_context_blackboard_patch(&state, &run_id, op, payload)?;
    let task = blackboard
        .tasks
        .iter()
        .find(|row| row.id == task_id)
        .cloned();
    let (event_type, event_payload) = if action == "heartbeat" {
        (
            "context.task.heartbeat".to_string(),
            json!({
                "task_id": task_id,
                "patch_seq": patch.seq,
                "task_rev": task.as_ref().map(|row| row.task_rev),
            }),
        )
    } else {
        let status = task
            .as_ref()
            .map(|row| row.status.clone())
            .unwrap_or(ContextBlackboardTaskStatus::Blocked);
        (
            context_task_status_event_name(&status).to_string(),
            json!({
                "task_id": task_id,
                "patch_seq": patch.seq,
                "task_rev": task.as_ref().map(|row| row.task_rev),
                "status": status,
                "error": input.error,
            }),
        )
    };
    let _ = context_run_event_append(
        State(state.clone()),
        Path(run_id.clone()),
        Json(ContextRunEventAppendInput {
            event_type,
            status: run_status,
            step_id: Some(task_id.clone()),
            payload: event_payload,
        }),
    )
    .await;
    Ok(Json(
        json!({ "ok": true, "task": task, "patch": patch, "blackboard": blackboard }),
    ))
}

pub(super) async fn context_run_checkpoint_create(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(input): Json<ContextCheckpointCreateInput>,
) -> Result<Json<Value>, StatusCode> {
    let run_state = load_context_run_state(&state, &run_id).await?;
    let blackboard = load_context_blackboard(&state, &run_id);
    let events_path = context_run_events_path(&state, &run_id);
    let seq = latest_context_run_event_seq(&events_path);
    let checkpoint = ContextCheckpointRecord {
        checkpoint_id: format!("cp-{}", Uuid::new_v4()),
        run_id: run_id.clone(),
        seq,
        ts_ms: crate::now_ms(),
        reason: input.reason,
        run_state,
        blackboard,
    };
    let dir = context_run_checkpoints_dir(&state, &run_id);
    std::fs::create_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let path = dir.join(format!("{:020}.json", seq));
    let payload =
        serde_json::to_string_pretty(&checkpoint).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(path, payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "checkpoint": checkpoint })))
}

pub(super) async fn context_run_checkpoint_latest(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let dir = context_run_checkpoints_dir(&state, &run_id);
    if !dir.exists() {
        return Ok(Json(json!({ "checkpoint": null })));
    }
    let mut latest_name: Option<String> = None;
    let mut entries = std::fs::read_dir(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    while let Some(Ok(entry)) = entries.next() {
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".json") {
            continue;
        }
        let should_replace = match latest_name.as_ref() {
            Some(current) => name > *current,
            None => true,
        };
        if should_replace {
            latest_name = Some(name);
        }
    }
    let Some(file_name) = latest_name else {
        return Ok(Json(json!({ "checkpoint": null })));
    };
    let raw = std::fs::read_to_string(dir.join(file_name))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let checkpoint = serde_json::from_str::<ContextCheckpointRecord>(&raw)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "checkpoint": checkpoint })))
}

pub(super) fn parse_context_step_status_text(value: &str) -> Option<ContextStepStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pending" => Some(ContextStepStatus::Pending),
        "runnable" => Some(ContextStepStatus::Runnable),
        "in_progress" | "in-progress" | "working" | "active" => Some(ContextStepStatus::InProgress),
        "blocked" => Some(ContextStepStatus::Blocked),
        "done" | "completed" | "complete" => Some(ContextStepStatus::Done),
        "failed" | "error" => Some(ContextStepStatus::Failed),
        _ => None,
    }
}

pub(super) fn replay_step_status_from_event(
    event: &ContextRunEventRecord,
) -> Option<ContextStepStatus> {
    if let Some(status_text) = event.payload.get("step_status").and_then(Value::as_str) {
        if let Some(status) = parse_context_step_status_text(status_text) {
            return Some(status);
        }
    }
    let ty = event.event_type.to_ascii_lowercase();
    if ty.contains("step_started") || ty.contains("step_running") {
        return Some(ContextStepStatus::InProgress);
    }
    if ty.contains("step_blocked") {
        return Some(ContextStepStatus::Blocked);
    }
    if ty.contains("step_failed") {
        return Some(ContextStepStatus::Failed);
    }
    if ty.contains("step_done") || ty.contains("step_completed") {
        return Some(ContextStepStatus::Done);
    }
    None
}

pub(super) fn latest_context_checkpoint_record(
    state: &AppState,
    run_id: &str,
) -> Option<ContextCheckpointRecord> {
    let dir = context_run_checkpoints_dir(state, run_id);
    if !dir.exists() {
        return None;
    }
    let mut latest_name: Option<String> = None;
    let mut entries = std::fs::read_dir(&dir).ok()?;
    while let Some(Ok(entry)) = entries.next() {
        if !entry
            .file_type()
            .ok()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".json") {
            continue;
        }
        let should_replace = match latest_name.as_ref() {
            Some(current) => name > *current,
            None => true,
        };
        if should_replace {
            latest_name = Some(name);
        }
    }
    let file_name = latest_name?;
    let raw = std::fs::read_to_string(dir.join(file_name)).ok()?;
    serde_json::from_str::<ContextCheckpointRecord>(&raw).ok()
}

pub(super) fn context_run_replay_materialize(
    run_id: &str,
    persisted: &ContextRunState,
    checkpoint: Option<&ContextCheckpointRecord>,
    events: &[ContextRunEventRecord],
) -> ContextRunState {
    let mut replay = if let Some(cp) = checkpoint {
        cp.run_state.clone()
    } else {
        let mut base = persisted.clone();
        base.status = ContextRunStatus::Queued;
        base.why_next_step = None;
        base.revision = 1;
        base.updated_at_ms = base.created_at_ms;
        for step in &mut base.steps {
            step.status = ContextStepStatus::Pending;
        }
        base
    };
    replay.run_id = run_id.to_string();
    let mut step_index: HashMap<String, usize> = replay
        .steps
        .iter()
        .enumerate()
        .map(|(idx, step)| (step.step_id.clone(), idx))
        .collect();

    for event in events {
        replay.status = event.status.clone();
        replay.updated_at_ms = replay.updated_at_ms.max(event.ts_ms);
        if let Some(why) = event.payload.get("why_next_step").and_then(Value::as_str) {
            if !why.trim().is_empty() {
                replay.why_next_step = Some(why.to_string());
            }
        }
        if let Some(items) = event.payload.get("steps").and_then(Value::as_array) {
            for item in items {
                let Some(step_id) = item.get("step_id").and_then(Value::as_str) else {
                    continue;
                };
                let step_title = item
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(step_id)
                    .to_string();
                let step_status = item
                    .get("status")
                    .and_then(Value::as_str)
                    .and_then(parse_context_step_status_text)
                    .unwrap_or(ContextStepStatus::Pending);
                let index = if let Some(existing) = step_index.get(step_id).copied() {
                    existing
                } else {
                    replay.steps.push(ContextRunStep {
                        step_id: step_id.to_string(),
                        title: step_title.clone(),
                        status: ContextStepStatus::Pending,
                    });
                    let idx = replay.steps.len().saturating_sub(1);
                    step_index.insert(step_id.to_string(), idx);
                    idx
                };
                replay.steps[index].title = step_title;
                replay.steps[index].status = step_status;
            }
        }
        if let Some(step_id) = &event.step_id {
            let index = if let Some(existing) = step_index.get(step_id).copied() {
                existing
            } else {
                replay.steps.push(ContextRunStep {
                    step_id: step_id.clone(),
                    title: event
                        .payload
                        .get("step_title")
                        .and_then(Value::as_str)
                        .unwrap_or(step_id.as_str())
                        .to_string(),
                    status: ContextStepStatus::Pending,
                });
                let idx = replay.steps.len().saturating_sub(1);
                step_index.insert(step_id.clone(), idx);
                idx
            };
            if let Some(step_status) = replay_step_status_from_event(event) {
                replay.steps[index].status = step_status;
            }
        }
    }

    let checkpoint_revision = checkpoint.map(|cp| cp.run_state.revision).unwrap_or(1);
    replay.revision = checkpoint_revision.saturating_add(events.len() as u64);
    replay
}

pub(super) fn context_blackboard_replay_materialize(
    persisted: &ContextBlackboardState,
    checkpoint: Option<&ContextCheckpointRecord>,
    patches: &[ContextBlackboardPatchRecord],
) -> ContextBlackboardState {
    let mut replay = checkpoint
        .map(|cp| cp.blackboard.clone())
        .unwrap_or_else(ContextBlackboardState::default);
    if checkpoint.is_none() && replay.revision == 0 && persisted.revision > 0 {
        replay.revision = 0;
    }
    for patch in patches {
        let _ = apply_context_blackboard_patch(&mut replay, patch);
    }
    replay
}

pub(super) async fn context_run_replay(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<ContextRunReplayQuery>,
) -> Result<Json<Value>, StatusCode> {
    let persisted = load_context_run_state(&state, &run_id).await?;
    let from_checkpoint = query.from_checkpoint.unwrap_or(true);
    let checkpoint = if from_checkpoint {
        latest_context_checkpoint_record(&state, &run_id)
    } else {
        None
    };
    let start_seq = checkpoint.as_ref().map(|cp| cp.seq).unwrap_or(0);
    let mut events = load_context_run_events_jsonl(
        &context_run_events_path(&state, &run_id),
        Some(start_seq),
        None,
    );
    if let Some(upto_seq) = query.upto_seq {
        events.retain(|row| row.seq <= upto_seq);
    }
    let replay = context_run_replay_materialize(&run_id, &persisted, checkpoint.as_ref(), &events);
    let persisted_blackboard = load_context_blackboard(&state, &run_id);
    let mut patches = load_context_blackboard_patches(&state, &run_id, None, None);
    if let Some(cp) = checkpoint.as_ref() {
        patches.retain(|patch| patch.ts_ms > cp.ts_ms);
    }
    let replay_blackboard =
        context_blackboard_replay_materialize(&persisted_blackboard, checkpoint.as_ref(), &patches);
    let blackboard_revision_mismatch = replay_blackboard.revision != persisted_blackboard.revision;
    let blackboard_task_count_mismatch =
        replay_blackboard.tasks.len() != persisted_blackboard.tasks.len();
    let mut replay_task_status = HashMap::<String, ContextBlackboardTaskStatus>::new();
    for task in &replay_blackboard.tasks {
        replay_task_status.insert(task.id.clone(), task.status.clone());
    }
    let blackboard_task_status_mismatch = persisted_blackboard.tasks.iter().any(|task| {
        replay_task_status
            .get(&task.id)
            .map(|status| status != &task.status)
            .unwrap_or(true)
    });
    let status_mismatch = replay.status != persisted.status;
    let why_next_step_mismatch =
        persisted.why_next_step.is_some() && replay.why_next_step != persisted.why_next_step;
    let step_count_mismatch =
        !persisted.steps.is_empty() && replay.steps.len() != persisted.steps.len();
    let drift = ContextReplayDrift {
        mismatch: status_mismatch
            || why_next_step_mismatch
            || step_count_mismatch
            || blackboard_revision_mismatch
            || blackboard_task_count_mismatch
            || blackboard_task_status_mismatch,
        status_mismatch,
        why_next_step_mismatch,
        step_count_mismatch,
        blackboard_revision_mismatch,
        blackboard_task_count_mismatch,
        blackboard_task_status_mismatch,
    };
    Ok(Json(json!({
        "ok": true,
        "run_id": run_id,
        "from_checkpoint": checkpoint.is_some(),
        "checkpoint_seq": checkpoint.as_ref().map(|cp| cp.seq),
        "events_applied": events.len(),
        "blackboard_patches_applied": patches.len(),
        "replay": replay,
        "replay_blackboard": replay_blackboard,
        "persisted": persisted,
        "persisted_blackboard": persisted_blackboard,
        "drift": drift
    })))
}

pub(super) async fn sync_automation_v2_run_blackboard(
    state: &AppState,
    automation: &crate::AutomationV2Spec,
    run: &crate::AutomationV2RunRecord,
) -> Result<(), StatusCode> {
    let run_id = automation_v2_context_run_id(&run.run_id);
    let lock = context_run_lock_for(&run_id).await;
    let _guard = lock.lock().await;

    if load_context_run_state(state, &run_id).await.is_err() {
        let now = crate::now_ms();
        let context_run = ContextRunState {
            run_id: run_id.clone(),
            run_type: "automation_v2".to_string(),
            source_client: Some("automation_v2_scheduler".to_string()),
            model_provider: None,
            model_id: None,
            mcp_servers: Vec::new(),
            status: automation_run_status_to_context(&run.status),
            objective: automation
                .description
                .clone()
                .unwrap_or_else(|| automation.name.clone()),
            workspace: ContextWorkspaceLease::default(),
            steps: automation
                .flow
                .nodes
                .iter()
                .map(|node| ContextRunStep {
                    step_id: node.node_id.clone(),
                    title: node.objective.clone(),
                    status: ContextStepStatus::Pending,
                })
                .collect(),
            why_next_step: Some("Track automation v2 flow via blackboard tasks".to_string()),
            revision: 1,
            created_at_ms: now,
            started_at_ms: Some(now),
            ended_at_ms: if matches!(
                run.status,
                crate::AutomationRunStatus::Completed
                    | crate::AutomationRunStatus::Failed
                    | crate::AutomationRunStatus::Cancelled
            ) {
                Some(now)
            } else {
                None
            },
            last_error: run.detail.clone(),
            updated_at_ms: now,
        };
        save_context_run_state(state, &context_run).await?;
    }

    let blackboard = load_context_blackboard(state, &run_id);
    let now = crate::now_ms();

    for node in &automation.flow.nodes {
        let task_id = format!("node-{}", node.node_id);
        let depends_on = node
            .depends_on
            .iter()
            .map(|dep| format!("node-{dep}"))
            .collect::<Vec<_>>();

        let status = automation_node_task_status(run, &node.node_id, &depends_on);

        let existing = blackboard.tasks.iter().find(|t| t.id == task_id);
        if let Some(task) = existing {
            if task.status != status {
                let payload = json!({
                    "task_id": task_id,
                    "status": status,
                    "task_rev": task.task_rev.saturating_add(1),
                });
                let _ = append_context_blackboard_patch(
                    state,
                    &run_id,
                    ContextBlackboardPatchOp::UpdateTaskState,
                    payload,
                )?;
            }
        } else {
            let task = ContextBlackboardTask {
                id: task_id.clone(),
                task_type: "automation_node".to_string(),
                payload: json!({
                    "node_id": node.node_id,
                    "name": node.objective,
                    "agent_id": node.agent_id,
                }),
                status,
                workflow_id: Some(automation.automation_id.clone()),
                workflow_node_id: Some(node.node_id.clone()),
                parent_task_id: None,
                depends_on_task_ids: depends_on,
                decision_ids: Vec::new(),
                artifact_ids: Vec::new(),
                assigned_agent: Some(node.agent_id.clone()),
                priority: 0,
                attempt: 0,
                max_attempts: 3,
                last_error: None,
                next_retry_at_ms: None,
                lease_owner: None,
                lease_token: None,
                lease_expires_at_ms: None,
                task_rev: 1,
                created_ts: now,
                updated_ts: now,
            };
            let _ = append_context_blackboard_patch(
                state,
                &run_id,
                ContextBlackboardPatchOp::AddTask,
                serde_json::to_value(&task).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            )?;
        }
    }

    Ok(())
}

pub(super) fn automation_v2_context_run_id(run_id: &str) -> String {
    format!("automation-v2-{run_id}")
}

pub(super) fn automation_run_status_to_context(
    status: &crate::AutomationRunStatus,
) -> ContextRunStatus {
    match status {
        crate::AutomationRunStatus::Queued => ContextRunStatus::Queued,
        crate::AutomationRunStatus::Running
        | crate::AutomationRunStatus::Pausing
        | crate::AutomationRunStatus::Paused => ContextRunStatus::Running,
        crate::AutomationRunStatus::Completed => ContextRunStatus::Completed,
        crate::AutomationRunStatus::Failed => ContextRunStatus::Failed,
        crate::AutomationRunStatus::Cancelled => ContextRunStatus::Cancelled,
    }
}

pub(super) fn automation_node_task_status(
    run: &crate::AutomationV2RunRecord,
    node_id: &str,
    depends_on: &[String],
) -> ContextBlackboardTaskStatus {
    let completed = run
        .checkpoint
        .completed_nodes
        .iter()
        .any(|row| row == node_id);
    if completed {
        return ContextBlackboardTaskStatus::Done;
    }
    if matches!(
        run.status,
        crate::AutomationRunStatus::Cancelled | crate::AutomationRunStatus::Failed
    ) {
        return ContextBlackboardTaskStatus::Failed;
    }
    let deps_done = depends_on.iter().all(|dep_task_id| {
        let dep_node_id = dep_task_id.strip_prefix("node-").unwrap_or(dep_task_id);
        run.checkpoint
            .completed_nodes
            .iter()
            .any(|row| row == dep_node_id)
    });
    if !deps_done {
        return ContextBlackboardTaskStatus::Blocked;
    }
    if matches!(
        run.status,
        crate::AutomationRunStatus::Paused | crate::AutomationRunStatus::Pausing
    ) {
        return ContextBlackboardTaskStatus::Blocked;
    }
    if run
        .checkpoint
        .pending_nodes
        .iter()
        .any(|row| row == node_id)
    {
        return ContextBlackboardTaskStatus::Runnable;
    }
    ContextBlackboardTaskStatus::Pending
}
