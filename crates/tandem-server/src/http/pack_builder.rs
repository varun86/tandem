use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct PackBuilderPreviewRequest {
    pub goal: Option<String>,
    pub session_id: Option<String>,
    pub thread_key: Option<String>,
    pub context_run_id: Option<String>,
    pub auto_apply: Option<bool>,
    pub selected_connectors: Option<Vec<String>>,
    pub schedule: Option<Value>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct PackBuilderApplyRequest {
    pub plan_id: Option<String>,
    pub session_id: Option<String>,
    pub thread_key: Option<String>,
    pub context_run_id: Option<String>,
    pub selected_connectors: Option<Vec<String>>,
    pub approvals: Option<PackBuilderApprovals>,
    pub secret_refs_confirmed: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub(super) struct PackBuilderApprovals {
    pub approve_connector_registration: Option<bool>,
    pub approve_pack_install: Option<bool>,
    pub approve_enable_routines: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct PackBuilderCancelRequest {
    pub plan_id: Option<String>,
    pub session_id: Option<String>,
    pub thread_key: Option<String>,
    pub context_run_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct PackBuilderPendingQuery {
    pub plan_id: Option<String>,
    pub session_id: Option<String>,
    pub thread_key: Option<String>,
    pub context_run_id: Option<String>,
}

// Internal helpers

pub(super) async fn run_pack_builder_tool(
    state: &AppState,
    args: Value,
) -> Result<Value, StatusCode> {
    let result = state
        .tools
        .execute("pack_builder", args)
        .await
        .map_err(|e| {
            tracing::warn!("pack_builder tool execution failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let mut metadata = result.metadata;
    if let Some(obj) = metadata.as_object_mut() {
        obj.entry("output".to_string())
            .or_insert_with(|| Value::String(result.output));
    }
    Ok(metadata)
}

pub(super) fn pack_builder_task_status_from_payload(
    payload: &Value,
) -> ContextBlackboardTaskStatus {
    let status = payload
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    match status {
        "pending" => ContextBlackboardTaskStatus::Pending,
        "success" => ContextBlackboardTaskStatus::Done,
        "failed" | "error" => ContextBlackboardTaskStatus::Failed,
        _ => ContextBlackboardTaskStatus::Done,
    }
}

pub(super) fn sanitize_context_id(id: Option<&str>) -> Option<String> {
    id.map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

pub(super) fn pack_builder_task_id_for(
    payload: &Value,
    mode: &str,
    session_id: Option<&str>,
) -> String {
    if let Some(plan_id) = payload
        .get("plan_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return format!("pack-builder-plan-{plan_id}");
    }
    if let Some(session) = sanitize_context_id(session_id) {
        return format!("pack-builder-session-{}", session.replace([':', '/'], "_"));
    }
    format!("pack-builder-{mode}-{}", Uuid::new_v4())
}

pub(super) async fn ensure_pack_builder_context_run(
    state: &AppState,
    run_id: &str,
    objective: Option<&str>,
) -> Result<(), StatusCode> {
    if load_context_run_state(state, run_id).await.is_ok() {
        return Ok(());
    }
    let now = crate::now_ms();
    let run = ContextRunState {
        run_id: run_id.to_string(),
        run_type: "pack_builder".to_string(),
        source_client: Some("pack_builder_api".to_string()),
        model_provider: None,
        model_id: None,
        mcp_servers: Vec::new(),
        status: ContextRunStatus::Running,
        objective: objective
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| "Pack Builder coordination".to_string()),
        workspace: ContextWorkspaceLease::default(),
        steps: Vec::new(),
        why_next_step: Some("Track pack builder workflow via blackboard tasks".to_string()),
        revision: 1,
        created_at_ms: now,
        started_at_ms: Some(now),
        ended_at_ms: None,
        last_error: None,
        updated_at_ms: now,
    };
    save_context_run_state(state, &run).await
}

pub(super) async fn pack_builder_emit_blackboard_task(
    state: &AppState,
    run_id: &str,
    mode: &str,
    session_id: Option<&str>,
    payload: &Value,
) -> Result<(), StatusCode> {
    let lock = context_run_lock_for(run_id).await;
    let _guard = lock.lock().await;

    let task_id = pack_builder_task_id_for(payload, mode, session_id);
    let now = crate::now_ms();
    let status = pack_builder_task_status_from_payload(payload);
    let blackboard = load_context_blackboard(state, run_id);
    let existing = blackboard
        .tasks
        .iter()
        .find(|row| row.id == task_id)
        .cloned();

    if existing.is_none() {
        let task = ContextBlackboardTask {
            id: task_id.clone(),
            task_type: format!("pack_builder.{mode}"),
            payload: json!({
                "title": format!("Pack Builder {mode}"),
                "mode": mode,
                "plan_id": payload.get("plan_id").cloned().unwrap_or(Value::Null),
                "status": payload.get("status").cloned().unwrap_or(Value::Null),
            }),
            status: ContextBlackboardTaskStatus::Pending,
            workflow_id: Some("pack_builder".to_string()),
            workflow_node_id: Some(mode.to_string()),
            parent_task_id: None,
            depends_on_task_ids: Vec::new(),
            decision_ids: Vec::new(),
            artifact_ids: Vec::new(),
            assigned_agent: Some("pack_builder".to_string()),
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
        let (patch, _) = append_context_blackboard_patch(
            state,
            run_id,
            ContextBlackboardPatchOp::AddTask,
            serde_json::to_value(&task).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )?;
        let _ = context_run_event_append(
            State(state.clone()),
            Path(run_id.to_string()),
            Json(ContextRunEventAppendInput {
                event_type: "context.task.created".to_string(),
                status: ContextRunStatus::Running,
                step_id: Some(task_id.clone()),
                payload: json!({
                    "task_id": task_id,
                    "task_type": task.task_type,
                    "patch_seq": patch.seq,
                    "task_rev": task.task_rev,
                    "source": "pack_builder",
                }),
            }),
        )
        .await;
    }

    let current = load_context_blackboard(state, run_id)
        .tasks
        .into_iter()
        .find(|row| row.id == task_id);
    let next_rev = current
        .as_ref()
        .map(|row| row.task_rev.saturating_add(1))
        .unwrap_or(1);
    let error_text = payload
        .get("error")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
    let update_payload = json!({
        "task_id": task_id,
        "status": status,
        "assigned_agent": "pack_builder",
        "task_rev": next_rev,
        "error": error_text,
    });
    let (patch, _) = append_context_blackboard_patch(
        state,
        run_id,
        ContextBlackboardPatchOp::UpdateTaskState,
        update_payload,
    )?;
    let _ = context_run_event_append(
        State(state.clone()),
        Path(run_id.to_string()),
        Json(ContextRunEventAppendInput {
            event_type: context_task_status_event_name(&status).to_string(),
            status: ContextRunStatus::Running,
            step_id: Some(task_id.clone()),
            payload: json!({
                "task_id": task_id,
                "status": status,
                "patch_seq": patch.seq,
                "task_rev": next_rev,
                "source": "pack_builder",
                "mode": mode,
                "plan_id": payload.get("plan_id").cloned().unwrap_or(Value::Null),
            }),
        }),
    )
    .await;
    Ok(())
}

// Handlers

pub(super) async fn pack_builder_preview(
    State(state): State<AppState>,
    Json(input): Json<PackBuilderPreviewRequest>,
) -> Result<Json<Value>, StatusCode> {
    let context_run_id = input.context_run_id.clone();
    let session_for_bb = input.session_id.clone();
    let args = json!({
        "mode": "preview",
        "goal": input.goal,
        "__session_id": input.session_id,
        "thread_key": input.thread_key,
        "auto_apply": input.auto_apply.unwrap_or(false),
        "selected_connectors": input.selected_connectors.unwrap_or_default(),
        "schedule": input.schedule,
    });
    let payload = run_pack_builder_tool(&state, args).await?;
    if let Some(run_id) = sanitize_context_id(context_run_id.as_deref()) {
        ensure_pack_builder_context_run(&state, &run_id, Some("Pack Builder Preview")).await?;
        let _ = pack_builder_emit_blackboard_task(
            &state,
            &run_id,
            "preview",
            session_for_bb.as_deref(),
            &payload,
        )
        .await;
    }
    Ok(Json(payload))
}

pub(super) async fn pack_builder_apply(
    State(state): State<AppState>,
    Json(input): Json<PackBuilderApplyRequest>,
) -> Result<Json<Value>, StatusCode> {
    let context_run_id = input.context_run_id.clone();
    let session_for_bb = input.session_id.clone();
    let args = json!({
        "mode": "apply",
        "plan_id": input.plan_id,
        "__session_id": input.session_id,
        "thread_key": input.thread_key,
        "selected_connectors": input.selected_connectors.unwrap_or_default(),
        "approvals": input.approvals.unwrap_or_default(),
        "secret_refs_confirmed": input.secret_refs_confirmed.unwrap_or(json!({})),
    });
    let payload = run_pack_builder_tool(&state, args).await?;
    if let Some(run_id) = sanitize_context_id(context_run_id.as_deref()) {
        ensure_pack_builder_context_run(&state, &run_id, Some("Pack Builder Application")).await?;
        let _ = pack_builder_emit_blackboard_task(
            &state,
            &run_id,
            "apply",
            session_for_bb.as_deref(),
            &payload,
        )
        .await;
    }
    Ok(Json(payload))
}

pub(super) async fn pack_builder_cancel(
    State(state): State<AppState>,
    Json(input): Json<PackBuilderCancelRequest>,
) -> Result<Json<Value>, StatusCode> {
    let context_run_id = input.context_run_id.clone();
    let session_for_bb = input.session_id.clone();
    let args = json!({
        "mode": "cancel",
        "plan_id": input.plan_id,
        "__session_id": input.session_id,
        "thread_key": input.thread_key,
    });
    let payload = run_pack_builder_tool(&state, args).await?;
    if let Some(run_id) = sanitize_context_id(context_run_id.as_deref()) {
        ensure_pack_builder_context_run(&state, &run_id, Some("Pack Builder cancellation")).await?;
        let _ = pack_builder_emit_blackboard_task(
            &state,
            &run_id,
            "cancel",
            session_for_bb.as_deref(),
            &payload,
        )
        .await;
    }
    Ok(Json(payload))
}

pub(super) async fn pack_builder_pending(
    State(state): State<AppState>,
    Query(query): Query<PackBuilderPendingQuery>,
) -> Result<Json<Value>, StatusCode> {
    let context_run_id = query.context_run_id.clone();
    let session_for_bb = query.session_id.clone();
    let args = json!({
        "mode": "pending",
        "plan_id": query.plan_id,
        "__session_id": query.session_id,
        "thread_key": query.thread_key,
    });
    let payload = run_pack_builder_tool(&state, args).await?;
    if let Some(run_id) = sanitize_context_id(context_run_id.as_deref()) {
        ensure_pack_builder_context_run(&state, &run_id, Some("Pack Builder pending")).await?;
        let _ = pack_builder_emit_blackboard_task(
            &state,
            &run_id,
            "pending",
            session_for_bb.as_deref(),
            &payload,
        )
        .await;
    }
    Ok(Json(payload))
}
