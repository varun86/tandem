use crate::agent_teams::{emit_spawn_approved, emit_spawn_denied, emit_spawn_requested};
use crate::http::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tandem_orchestrator::{
    AgentInstanceStatus, DefaultMissionReducer, MissionEvent, MissionReducer, MissionSpec,
    NoopMissionReducer, SpawnRequest, SpawnSource, WorkItem, WorkItemStatus,
};
use tandem_types::EngineEvent;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub(super) struct AgentTeamToolApprovalOutput {
    #[serde(rename = "approvalID")]
    pub approval_id: String,
    #[serde(rename = "sessionID", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "toolCallID")]
    pub tool_call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct MissionCreateInput {
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub work_items: Vec<MissionCreateWorkItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MissionCreateWorkItem {
    #[serde(default)]
    pub work_item_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub assigned_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MissionEventInput {
    pub event: MissionEvent,
}

#[derive(Debug, Deserialize)]
pub(super) struct AgentTeamSpawnInput {
    #[serde(rename = "missionID")]
    pub mission_id: Option<String>,
    #[serde(rename = "parentInstanceID")]
    pub parent_instance_id: Option<String>,
    #[serde(rename = "templateID")]
    pub template_id: Option<String>,
    pub role: tandem_orchestrator::AgentRole,
    pub source: Option<SpawnSource>,
    pub justification: String,
    #[serde(default)]
    pub budget_override: Option<tandem_orchestrator::BudgetLimit>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct AgentTeamInstancesQuery {
    #[serde(rename = "missionID")]
    pub mission_id: Option<String>,
    #[serde(rename = "parentInstanceID")]
    pub parent_instance_id: Option<String>,
    pub status: Option<AgentInstanceStatus>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct AgentTeamCancelInput {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AgentTeamTemplateCreateInput {
    pub template: tandem_orchestrator::AgentTemplate,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct AgentTeamTemplatePatchInput {
    pub role: Option<tandem_orchestrator::AgentRole>,
    pub system_prompt: Option<String>,
    pub skills: Option<Vec<tandem_orchestrator::SkillRef>>,
    pub default_budget: Option<tandem_orchestrator::BudgetLimit>,
    pub capabilities: Option<tandem_orchestrator::CapabilitySpec>,
}

pub(super) fn mission_event_id(event: &MissionEvent) -> &str {
    match event {
        MissionEvent::MissionStarted { mission_id }
        | MissionEvent::MissionPaused { mission_id, .. }
        | MissionEvent::MissionResumed { mission_id }
        | MissionEvent::MissionCanceled { mission_id, .. }
        | MissionEvent::RunStarted { mission_id, .. }
        | MissionEvent::RunFinished { mission_id, .. }
        | MissionEvent::ToolObserved { mission_id, .. }
        | MissionEvent::ApprovalGranted { mission_id, .. }
        | MissionEvent::ApprovalDenied { mission_id, .. }
        | MissionEvent::TimerFired { mission_id, .. }
        | MissionEvent::ResourceChanged { mission_id, .. } => mission_id,
    }
}

pub(super) async fn mission_create(
    State(state): State<AppState>,
    Json(input): Json<MissionCreateInput>,
) -> Json<Value> {
    let spec = MissionSpec::new(input.title, input.goal);
    let mission_id = spec.mission_id.clone();
    let mut mission = NoopMissionReducer::init(spec);
    mission.work_items = input
        .work_items
        .into_iter()
        .map(|item| WorkItem {
            work_item_id: item
                .work_item_id
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            title: item.title,
            detail: item.detail,
            status: WorkItemStatus::Todo,
            depends_on: Vec::new(),
            assigned_agent: item.assigned_agent,
            run_id: None,
            artifact_refs: Vec::new(),
            metadata: None,
        })
        .collect();

    state
        .missions
        .write()
        .await
        .insert(mission_id.clone(), mission.clone());
    state.event_bus.publish(EngineEvent::new(
        "mission.created",
        json!({
            "missionID": mission_id,
            "workItemCount": mission.work_items.len(),
        }),
    ));

    Json(json!({
        "mission": mission,
    }))
}

pub(super) async fn mission_list(State(state): State<AppState>) -> Json<Value> {
    let mut missions = state
        .missions
        .read()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    missions.sort_by(|a, b| a.mission_id.cmp(&b.mission_id));
    Json(json!({
        "missions": missions,
        "count": missions.len(),
    }))
}

pub(super) async fn mission_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mission = state
        .missions
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Mission not found",
                    "code": "MISSION_NOT_FOUND",
                    "missionID": id,
                })),
            )
        })?;
    Ok(Json(json!({
        "mission": mission,
    })))
}

pub(super) async fn mission_apply_event(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<MissionEventInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let event = input.event;
    let event_for_runtime = event.clone();
    if mission_event_id(&event) != id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Mission event mission_id mismatch",
                "code": "MISSION_EVENT_MISMATCH",
                "missionID": id,
            })),
        ));
    }

    let current = state
        .missions
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Mission not found",
                    "code": "MISSION_NOT_FOUND",
                    "missionID": id,
                })),
            )
        })?;

    let (next, commands) = DefaultMissionReducer::reduce(&current, event);
    let next_revision = next.revision;
    let next_status = next.status.clone();
    state
        .missions
        .write()
        .await
        .insert(id.clone(), next.clone());

    state.event_bus.publish(EngineEvent::new(
        "mission.updated",
        json!({
            "missionID": id,
            "revision": next_revision,
            "status": next_status,
            "commandCount": commands.len(),
        }),
    ));
    let orchestrator_spawns =
        run_orchestrator_runtime_spawns(&state, &next, &event_for_runtime).await;
    let orchestrator_cancellations =
        run_orchestrator_runtime_cancellations(&state, &next, &event_for_runtime).await;

    Ok(Json(json!({
        "mission": next,
        "commands": commands,
        "orchestratorSpawns": orchestrator_spawns,
        "orchestratorCancellations": orchestrator_cancellations,
    })))
}

async fn run_orchestrator_runtime_spawns(
    state: &AppState,
    mission: &tandem_orchestrator::MissionState,
    event: &MissionEvent,
) -> Vec<Value> {
    let MissionEvent::MissionStarted { mission_id } = event else {
        return Vec::new();
    };
    if mission_id != &mission.mission_id {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for item in &mission.work_items {
        let Some(agent_name) = item.assigned_agent.as_deref() else {
            continue;
        };
        let Some(role) = parse_agent_role(agent_name) else {
            rows.push(json!({
                "workItemID": item.work_item_id,
                "agent": agent_name,
                "ok": false,
                "code": "UNSUPPORTED_ASSIGNED_AGENT",
                "error": "assigned_agent does not map to an agent-team role"
            }));
            continue;
        };
        let req = SpawnRequest {
            mission_id: Some(mission.mission_id.clone()),
            parent_instance_id: None,
            source: SpawnSource::OrchestratorRuntime,
            parent_role: Some(tandem_orchestrator::AgentRole::Orchestrator),
            role,
            template_id: None,
            justification: format!("mission work item {}", item.work_item_id),
            budget_override: None,
        };
        emit_spawn_requested(state, &req);
        let result = state.agent_teams.spawn(state, req.clone()).await;
        if !result.decision.allowed || result.instance.is_none() {
            emit_spawn_denied(state, &req, &result.decision);
            rows.push(json!({
                "workItemID": item.work_item_id,
                "agent": agent_name,
                "ok": false,
                "code": result.decision.code,
                "error": result.decision.reason,
            }));
            continue;
        }
        let instance = result.instance.expect("checked is_some");
        emit_spawn_approved(state, &req, &instance);
        rows.push(json!({
            "workItemID": item.work_item_id,
            "agent": agent_name,
            "ok": true,
            "instanceID": instance.instance_id,
            "sessionID": instance.session_id,
            "status": instance.status,
        }));
    }
    rows
}

fn parse_agent_role(agent_name: &str) -> Option<tandem_orchestrator::AgentRole> {
    match agent_name.trim().to_ascii_lowercase().as_str() {
        "orchestrator" => Some(tandem_orchestrator::AgentRole::Orchestrator),
        "delegator" => Some(tandem_orchestrator::AgentRole::Delegator),
        "worker" => Some(tandem_orchestrator::AgentRole::Worker),
        "watcher" => Some(tandem_orchestrator::AgentRole::Watcher),
        "reviewer" => Some(tandem_orchestrator::AgentRole::Reviewer),
        "tester" => Some(tandem_orchestrator::AgentRole::Tester),
        "committer" => Some(tandem_orchestrator::AgentRole::Committer),
        _ => None,
    }
}

async fn run_orchestrator_runtime_cancellations(
    state: &AppState,
    mission: &tandem_orchestrator::MissionState,
    event: &MissionEvent,
) -> Value {
    let MissionEvent::MissionCanceled { mission_id, reason } = event else {
        return json!({
            "triggered": false,
            "cancelledInstances": 0u64
        });
    };
    if mission_id != &mission.mission_id {
        return json!({
            "triggered": false,
            "cancelledInstances": 0u64
        });
    }
    let cancelled = state
        .agent_teams
        .cancel_mission(state, &mission.mission_id, reason)
        .await;
    json!({
        "triggered": true,
        "reason": reason,
        "cancelledInstances": cancelled,
    })
}

pub(super) async fn agent_team_templates(State(state): State<AppState>) -> Json<Value> {
    let templates = state.agent_teams.list_templates().await;
    Json(json!({
        "templates": templates,
        "count": templates.len(),
    }))
}

pub(super) async fn agent_team_template_create(
    State(state): State<AppState>,
    Json(input): Json<AgentTeamTemplateCreateInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if input.template.template_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "code": "INVALID_TEMPLATE_ID",
                "error": "template_id is required"
            })),
        ));
    }
    let workspace_root = state.workspace_index.snapshot().await.root;
    let template = state
        .agent_teams
        .upsert_template(&workspace_root, input.template)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "ok": false,
                    "code": "TEMPLATE_PERSIST_FAILED",
                    "error": error.to_string(),
                })),
            )
        })?;
    Ok(Json(json!({
        "ok": true,
        "template": template,
    })))
}

pub(super) async fn agent_team_template_patch(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AgentTeamTemplatePatchInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = state
        .agent_teams
        .list_templates()
        .await
        .into_iter()
        .find(|template| template.template_id == id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "ok": false,
                    "code": "TEMPLATE_NOT_FOUND",
                    "error": "template not found",
                    "templateID": id,
                })),
            )
        })?;
    let mut updated = existing;
    if let Some(role) = input.role {
        updated.role = role;
    }
    if let Some(system_prompt) = input.system_prompt {
        updated.system_prompt = Some(system_prompt);
    }
    if let Some(skills) = input.skills {
        updated.skills = skills;
    }
    if let Some(default_budget) = input.default_budget {
        updated.default_budget = default_budget;
    }
    if let Some(capabilities) = input.capabilities {
        updated.capabilities = capabilities;
    }

    let workspace_root = state.workspace_index.snapshot().await.root;
    let template = state
        .agent_teams
        .upsert_template(&workspace_root, updated)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "ok": false,
                    "code": "TEMPLATE_PERSIST_FAILED",
                    "error": error.to_string(),
                })),
            )
        })?;
    Ok(Json(json!({
        "ok": true,
        "template": template,
    })))
}

pub(super) async fn agent_team_template_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let workspace_root = state.workspace_index.snapshot().await.root;
    let deleted = state
        .agent_teams
        .delete_template(&workspace_root, &id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "ok": false,
                    "code": "TEMPLATE_DELETE_FAILED",
                    "error": error.to_string(),
                })),
            )
        })?;
    if !deleted {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "ok": false,
                "code": "TEMPLATE_NOT_FOUND",
                "error": "template not found",
                "templateID": id,
            })),
        ));
    }
    Ok(Json(json!({
        "ok": true,
        "deleted": true,
        "templateID": id,
    })))
}

pub(super) async fn agent_team_instances(
    State(state): State<AppState>,
    Query(query): Query<AgentTeamInstancesQuery>,
) -> Json<Value> {
    let instances = state
        .agent_teams
        .list_instances(
            query.mission_id.as_deref(),
            query.parent_instance_id.as_deref(),
            query.status,
        )
        .await;
    Json(json!({
        "instances": instances,
        "count": instances.len(),
    }))
}

pub(super) async fn agent_team_missions(State(state): State<AppState>) -> Json<Value> {
    let missions = state.agent_teams.list_mission_summaries().await;
    Json(json!({
        "missions": missions,
        "count": missions.len(),
    }))
}

pub(super) async fn agent_team_approvals(State(state): State<AppState>) -> Json<Value> {
    let spawn = state.agent_teams.list_spawn_approvals().await;
    let session_ids = state
        .agent_teams
        .list_instances(None, None, None)
        .await
        .into_iter()
        .map(|instance| instance.session_id)
        .collect::<std::collections::HashSet<_>>();
    let permissions = state
        .permissions
        .list()
        .await
        .into_iter()
        .filter(|req| {
            req.session_id
                .as_ref()
                .map(|sid| session_ids.contains(sid))
                .unwrap_or(false)
        })
        .map(|req| AgentTeamToolApprovalOutput {
            approval_id: req.id.clone(),
            session_id: req.session_id.clone(),
            tool_call_id: req.id,
            tool: req.tool,
            args: req.args,
            status: req.status,
        })
        .collect::<Vec<_>>();
    Json(json!({
        "spawnApprovals": spawn,
        "toolApprovals": permissions,
        "count": spawn.len() + permissions.len(),
    }))
}

pub(super) async fn agent_team_spawn(
    State(state): State<AppState>,
    Json(input): Json<AgentTeamSpawnInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let req = SpawnRequest {
        mission_id: input.mission_id.clone(),
        parent_instance_id: input.parent_instance_id.clone(),
        source: input.source.unwrap_or(SpawnSource::UiAction),
        parent_role: None,
        role: input.role,
        template_id: input.template_id.clone(),
        justification: input.justification.clone(),
        budget_override: input.budget_override,
    };
    emit_spawn_requested(&state, &req);
    let result = state.agent_teams.spawn(&state, req.clone()).await;
    if !result.decision.allowed || result.instance.is_none() {
        emit_spawn_denied(&state, &req, &result.decision);
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "ok": false,
                "code": result.decision.code,
                "error": result.decision.reason,
                "requiresUserApproval": result.decision.requires_user_approval,
            })),
        ));
    }
    let instance = result.instance.expect("checked is_some");
    emit_spawn_approved(&state, &req, &instance);
    Ok(Json(json!({
        "ok": true,
        "missionID": instance.mission_id,
        "instanceID": instance.instance_id,
        "sessionID": instance.session_id,
        "runID": instance.run_id,
        "status": instance.status,
        "skillHash": instance.skill_hash,
    })))
}

pub(super) async fn agent_team_approve_spawn(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AgentTeamCancelInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let reason = input
        .reason
        .unwrap_or_else(|| "approved by user".to_string());
    let Some(result) = state
        .agent_teams
        .approve_spawn_approval(&state, &id, Some(reason.as_str()))
        .await
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "ok": false,
                "code": "APPROVAL_NOT_FOUND",
                "error": "Spawn approval not found",
                "approvalID": id,
            })),
        ));
    };
    if !result.decision.allowed || result.instance.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "ok": false,
                "code": result.decision.code,
                "error": result.decision.reason,
                "approvalID": id,
            })),
        ));
    }
    let instance = result.instance.expect("checked is_some");
    Ok(Json(json!({
        "ok": true,
        "approvalID": id,
        "decision": "approved",
        "instanceID": instance.instance_id,
        "sessionID": instance.session_id,
        "missionID": instance.mission_id,
        "status": instance.status,
    })))
}

pub(super) async fn agent_team_deny_spawn(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AgentTeamCancelInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let reason = input.reason.unwrap_or_else(|| "denied by user".to_string());
    let Some(approval) = state
        .agent_teams
        .deny_spawn_approval(&id, Some(reason.as_str()))
        .await
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "ok": false,
                "code": "APPROVAL_NOT_FOUND",
                "error": "Spawn approval not found",
                "approvalID": id,
            })),
        ));
    };
    let denied_decision = tandem_orchestrator::SpawnDecision {
        allowed: false,
        code: approval.decision_code,
        reason: Some(reason.clone()),
        requires_user_approval: false,
    };
    emit_spawn_denied(&state, &approval.request, &denied_decision);
    Ok(Json(json!({
        "ok": true,
        "approvalID": id,
        "decision": "denied",
        "reason": reason,
    })))
}

pub(super) async fn agent_team_cancel_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AgentTeamCancelInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let reason = input
        .reason
        .unwrap_or_else(|| "cancelled by user".to_string());
    let Some(instance) = state
        .agent_teams
        .cancel_instance(&state, &id, &reason)
        .await
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "ok": false,
                "code": "INSTANCE_NOT_FOUND",
                "error": "Agent instance not found",
                "instanceID": id,
            })),
        ));
    };
    Ok(Json(json!({
        "ok": true,
        "instanceID": instance.instance_id,
        "sessionID": instance.session_id,
        "status": instance.status,
    })))
}

pub(super) async fn agent_team_cancel_mission(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AgentTeamCancelInput>,
) -> Json<Value> {
    let reason = input
        .reason
        .unwrap_or_else(|| "mission cancelled by user".to_string());
    let cancelled = state.agent_teams.cancel_mission(&state, &id, &reason).await;
    Json(json!({
        "ok": true,
        "missionID": id,
        "cancelledInstances": cancelled,
    }))
}
