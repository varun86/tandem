use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tandem_plan_compiler::api as compiler_api;

use super::*;

#[allow(unused_imports)]
pub(crate) use compiler_api::planner_model_spec;

#[derive(Debug, Deserialize)]
pub(super) struct WorkflowPlanPreviewRequest {
    pub prompt: String,
    #[serde(default)]
    pub schedule: Option<Value>,
    #[serde(default)]
    pub plan_source: Option<String>,
    #[serde(default)]
    pub allowed_mcp_servers: Vec<String>,
    #[serde(default)]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub operator_preferences: Option<Value>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct WorkflowPlanApplyRequest {
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(default)]
    pub plan: Option<crate::WorkflowPlan>,
    #[serde(default)]
    pub creator_id: Option<String>,
    #[serde(default)]
    pub pack_builder_export: Option<WorkflowPlanPackBuilderExportRequest>,
    #[serde(default)]
    pub overlap_decision: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WorkflowPlanImportRequest {
    pub bundle: compiler_api::PlanPackageImportBundle,
    #[serde(default)]
    pub creator_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub(super) struct WorkflowPlanPackBuilderExportRequest {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub thread_key: Option<String>,
    #[serde(default)]
    pub auto_apply: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WorkflowPlanChatStartRequest {
    pub prompt: String,
    #[serde(default)]
    pub schedule: Option<Value>,
    #[serde(default)]
    pub plan_source: Option<String>,
    #[serde(default)]
    pub allowed_mcp_servers: Vec<String>,
    #[serde(default)]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub operator_preferences: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WorkflowPlanChatMessageRequest {
    pub plan_id: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct WorkflowPlanChatResetRequest {
    pub plan_id: String,
}

fn workflow_plan_import_summary(plan_package: &compiler_api::PlanPackage) -> serde_json::Value {
    json!({
        "plan_id": plan_package.plan_id,
        "plan_revision": plan_package.plan_revision,
        "routine_count": plan_package.routine_graph.len(),
        "context_object_count": plan_package.context_objects.len(),
        "credential_envelope_count": plan_package.credential_envelopes.len(),
    })
}

async fn compile_preview_plan_overlap(
    state: &AppState,
    plan_package: &compiler_api::PlanPackage,
) -> compiler_api::OverlapComparison {
    let prior_plans = state
        .list_automations_v2()
        .await
        .into_iter()
        .filter_map(|automation| {
            automation
                .metadata
                .as_ref()
                .and_then(|metadata| {
                    metadata
                        .get("plan_package")
                        .or_else(|| metadata.get("planPackage"))
                })
                .cloned()
        })
        .filter_map(|value| serde_json::from_value::<compiler_api::PlanPackage>(value).ok())
        .filter(|prior| prior.plan_id != plan_package.plan_id)
        .collect::<Vec<_>>();
    compiler_api::analyze_plan_overlap(plan_package, &prior_plans)
}

fn parse_overlap_decision(
    decision: Option<&str>,
) -> Result<Option<compiler_api::OverlapDecision>, String> {
    let Some(decision) = decision.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    match decision.to_ascii_lowercase().as_str() {
        "reuse" => Ok(Some(compiler_api::OverlapDecision::Reuse)),
        "merge" => Ok(Some(compiler_api::OverlapDecision::Merge)),
        "fork" => Ok(Some(compiler_api::OverlapDecision::Fork)),
        "new" => Ok(Some(compiler_api::OverlapDecision::New)),
        _ => Err(format!("unsupported overlap decision `{decision}`")),
    }
}

fn teaching_library_summary() -> Value {
    serde_json::to_value(compiler_api::planner_teaching_library_summary())
        .unwrap_or_else(|_| json!({}))
}

pub(super) async fn workflow_plan_preview(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanPreviewRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let prompt = input.prompt.trim();
    if prompt.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "prompt is required",
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ));
    }
    if let Some(workspace_root) = input.workspace_root.as_deref() {
        crate::normalize_absolute_workspace_root(workspace_root).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": error,
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            )
        })?;
    }
    let build = workflow_planner_host::build_workflow_plan(
        &state,
        prompt,
        input.schedule.as_ref(),
        input.plan_source.as_deref().unwrap_or("unknown"),
        input.allowed_mcp_servers,
        input.workspace_root.as_deref(),
        input.operator_preferences,
    )
    .await
    .map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let plan = build.plan;
    let planner_diagnostics = build.planner_diagnostics.clone();
    let teaching_library = teaching_library_summary();
    let plan_json = compiler_api::workflow_plan_to_json(&plan).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let plan_package = compiler_api::compile_workflow_plan_preview_package_with_revision(
        &plan_json,
        Some("workflow_planner"),
        1,
    );
    let plan_package_validation = compiler_api::validate_plan_package(&plan_package);
    let overlap_analysis = compile_preview_plan_overlap(&state, &plan_package).await;
    let plan_package_bundle = compiler_api::export_plan_package_bundle(&plan_package);
    let host = workflow_planner_host::WorkflowPlannerHost { state: &state };
    compiler_api::store_preview_draft::<
        crate::routines::types::RoutineMisfirePolicy,
        crate::AutomationFlowInputRef,
        crate::AutomationFlowOutputContract,
        _,
    >(&host, plan.clone(), planner_diagnostics.clone())
    .await
    .map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("{error:?}"),
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    Ok(Json(json!({
        "plan": plan,
        "plan_package": plan_package,
        "plan_package_bundle": plan_package_bundle,
        "plan_package_validation": plan_package_validation,
        "overlap_analysis": overlap_analysis,
        "teaching_library": teaching_library,
        "clarifier": build.clarifier,
        "planner_diagnostics": planner_diagnostics,
        "assistant_message": build.assistant_text.map(|text| json!({
            "role": "assistant",
            "text": text,
        })),
    })))
}

pub(super) async fn workflow_plan_chat_start(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanChatStartRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let prompt = input.prompt.trim();
    if prompt.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "prompt is required",
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ));
    }
    if let Some(workspace_root) = input.workspace_root.as_deref() {
        crate::normalize_absolute_workspace_root(workspace_root).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": error,
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            )
        })?;
    }
    let build = workflow_planner_host::build_workflow_plan(
        &state,
        prompt,
        input.schedule.as_ref(),
        input.plan_source.as_deref().unwrap_or("unknown"),
        input.allowed_mcp_servers,
        input.workspace_root.as_deref(),
        input.operator_preferences,
    )
    .await
    .map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let plan = build.plan;
    let host = workflow_planner_host::WorkflowPlannerHost { state: &state };
    let draft = compiler_api::store_chat_start_draft::<
        crate::routines::types::RoutineMisfirePolicy,
        crate::AutomationFlowInputRef,
        crate::AutomationFlowOutputContract,
        _,
    >(
        &host,
        plan.clone(),
        build.planner_diagnostics.clone(),
        build.assistant_text.clone(),
    )
    .await
    .map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("{error:?}"),
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let plan_json = compiler_api::workflow_plan_to_json(&draft.current_plan).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let plan_package = compiler_api::compile_workflow_plan_preview_package_with_revision(
        &plan_json,
        Some("workflow_planner"),
        draft.plan_revision,
    );
    let plan_package_validation = compiler_api::validate_plan_package(&plan_package);
    let overlap_analysis = compile_preview_plan_overlap(&state, &plan_package).await;
    let plan_package_bundle = compiler_api::export_plan_package_bundle(&plan_package);
    let teaching_library = teaching_library_summary();
    Ok(Json(json!({
        "plan": draft.current_plan,
        "plan_package": plan_package,
        "plan_package_bundle": plan_package_bundle,
        "plan_package_validation": plan_package_validation,
        "overlap_analysis": overlap_analysis,
        "teaching_library": teaching_library,
        "conversation": draft.conversation,
        "planner_diagnostics": draft.planner_diagnostics,
        "clarifier": build.clarifier,
        "assistant_message": build.assistant_text.map(|text| json!({
            "role": "assistant",
            "text": text,
        })),
    })))
}

pub(super) async fn workflow_plan_get(
    State(state): State<AppState>,
    axum::extract::Path(plan_id): axum::extract::Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let host = workflow_planner_host::WorkflowPlannerHost { state: &state };
    let draft = compiler_api::load_workflow_plan_draft::<
        crate::routines::types::RoutineMisfirePolicy,
        crate::AutomationFlowInputRef,
        crate::AutomationFlowOutputContract,
        _,
    >(&host, &plan_id)
    .await
    .map_err(|error| match error {
        compiler_api::PlannerDraftError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(compiler_api::draft_not_found_response(&plan_id)),
        ),
        compiler_api::PlannerDraftError::InvalidState(error)
        | compiler_api::PlannerDraftError::Store(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ),
    })?;
    let plan_json = compiler_api::workflow_plan_to_json(&draft.current_plan).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let plan_package = compiler_api::compile_workflow_plan_preview_package_with_revision(
        &plan_json,
        Some("workflow_planner"),
        draft.plan_revision,
    );
    let plan_package_validation = compiler_api::validate_plan_package(&plan_package);
    let plan_package_bundle = compiler_api::export_plan_package_bundle(&plan_package);
    let overlap_analysis = compile_preview_plan_overlap(&state, &plan_package).await;
    let initial_plan_json =
        compiler_api::workflow_plan_to_json(&draft.initial_plan).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": error,
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            )
        })?;
    let plan_package_replay = compiler_api::compare_workflow_plan_preview_replay_with_revision(
        &plan_json,
        draft.plan_revision,
        &initial_plan_json,
        1,
    );
    let teaching_library = teaching_library_summary();
    Ok(Json(json!({
        "plan": draft.current_plan,
        "plan_package": plan_package,
        "plan_package_bundle": plan_package_bundle,
        "plan_package_validation": plan_package_validation,
        "overlap_analysis": overlap_analysis,
        "plan_package_replay": plan_package_replay,
        "teaching_library": teaching_library,
        "conversation": draft.conversation,
        "planner_diagnostics": draft.planner_diagnostics,
    })))
}

pub(super) async fn workflow_plan_chat_message(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanChatMessageRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let plan_id = input.plan_id.trim();
    let message = input.message.trim();
    if plan_id.is_empty() || message.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "plan_id and message are required",
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ));
    }
    let host = workflow_planner_host::WorkflowPlannerHost { state: &state };
    let revision = compiler_api::revise_workflow_plan_draft::<
        crate::routines::types::RoutineMisfirePolicy,
        crate::AutomationFlowInputRef,
        crate::AutomationFlowOutputContract,
        _,
    >(
        &host,
        plan_id,
        message,
        compiler_api::PlannerLoopConfig {
            session_title: "Workflow Planner Revision".to_string(),
            timeout_ms: super::workflow_planner_policy::planner_revision_timeout_ms(),
            override_env: "TANDEM_WORKFLOW_PLANNER_TEST_REVISION_RESPONSE".to_string(),
        },
        workflow_planner_host::normalize_workflow_step_metadata,
    )
    .await
    .map_err(|error| match error {
        compiler_api::PlannerDraftError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(compiler_api::draft_not_found_response(plan_id)),
        ),
        compiler_api::PlannerDraftError::InvalidState(error)
        | compiler_api::PlannerDraftError::Store(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ),
    })?;
    let plan_json =
        compiler_api::workflow_plan_to_json(&revision.draft.current_plan).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": error,
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            )
        })?;
    let plan_package = compiler_api::compile_workflow_plan_preview_package_with_revision(
        &plan_json,
        Some("workflow_planner"),
        revision.draft.plan_revision,
    );
    let plan_package_validation = compiler_api::validate_plan_package(&plan_package);
    let overlap_analysis = compile_preview_plan_overlap(&state, &plan_package).await;
    let plan_package_bundle = compiler_api::export_plan_package_bundle(&plan_package);
    let initial_plan_json = compiler_api::workflow_plan_to_json(&revision.draft.initial_plan)
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": error,
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            )
        })?;
    let plan_package_replay = compiler_api::compare_workflow_plan_preview_replay_with_revision(
        &plan_json,
        revision.draft.plan_revision,
        &initial_plan_json,
        1,
    );
    let teaching_library = teaching_library_summary();
    Ok(Json(json!({
        "plan": revision.draft.current_plan,
        "plan_package": plan_package,
        "plan_package_bundle": plan_package_bundle,
        "plan_package_validation": plan_package_validation,
        "overlap_analysis": overlap_analysis,
        "plan_package_replay": plan_package_replay,
        "teaching_library": teaching_library,
        "conversation": revision.draft.conversation,
        "assistant_message": {
            "role": "assistant",
            "text": revision.assistant_text,
        },
        "change_summary": revision.change_summary,
        "clarifier": revision.clarifier,
        "planner_diagnostics": revision.draft.planner_diagnostics,
    })))
}

pub(super) async fn workflow_plan_chat_reset(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanChatResetRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let plan_id = input.plan_id.trim();
    if plan_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "plan_id is required",
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ));
    }
    let host = workflow_planner_host::WorkflowPlannerHost { state: &state };
    let draft = compiler_api::reset_workflow_plan_draft::<
        crate::routines::types::RoutineMisfirePolicy,
        crate::AutomationFlowInputRef,
        crate::AutomationFlowOutputContract,
        _,
    >(&host, plan_id)
    .await
    .map_err(|error| match error {
        compiler_api::PlannerDraftError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(compiler_api::draft_not_found_response(plan_id)),
        ),
        compiler_api::PlannerDraftError::InvalidState(error)
        | compiler_api::PlannerDraftError::Store(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ),
    })?;
    let plan_json = compiler_api::workflow_plan_to_json(&draft.current_plan).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let plan_package = compiler_api::compile_workflow_plan_preview_package_with_revision(
        &plan_json,
        Some("workflow_planner"),
        draft.plan_revision,
    );
    let plan_package_validation = compiler_api::validate_plan_package(&plan_package);
    let overlap_analysis = compile_preview_plan_overlap(&state, &plan_package).await;
    let plan_package_bundle = compiler_api::export_plan_package_bundle(&plan_package);
    let initial_plan_json =
        compiler_api::workflow_plan_to_json(&draft.initial_plan).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": error,
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            )
        })?;
    let plan_package_replay = compiler_api::compare_workflow_plan_preview_replay_with_revision(
        &plan_json,
        draft.plan_revision,
        &initial_plan_json,
        1,
    );
    let teaching_library = teaching_library_summary();
    Ok(Json(json!({
        "plan": draft.current_plan,
        "plan_package": plan_package,
        "plan_package_bundle": plan_package_bundle,
        "plan_package_validation": plan_package_validation,
        "overlap_analysis": overlap_analysis,
        "plan_package_replay": plan_package_replay,
        "teaching_library": teaching_library,
        "conversation": draft.conversation,
        "planner_diagnostics": draft.planner_diagnostics,
    })))
}

pub(super) async fn workflow_plan_apply(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanApplyRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let plan_id = input
        .plan_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let plan = match (input.plan, plan_id.as_deref()) {
        (Some(plan), _) => plan,
        (None, Some(plan_id)) => state.get_workflow_plan(plan_id).await.ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "workflow plan not found",
                    "code": "WORKFLOW_PLAN_NOT_FOUND",
                    "plan_id": plan_id,
                })),
            )
        })?,
        (None, None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "plan or plan_id is required",
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            ));
        }
    };
    compiler_api::validate_workflow_plan(&plan).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let draft_context = if let Some(plan_id) = plan_id.as_deref() {
        state.get_workflow_plan_draft(plan_id).await
    } else {
        None
    };
    let apply_revision = draft_context
        .as_ref()
        .map(|draft| draft.plan_revision)
        .unwrap_or(1);
    let planner_diagnostics = draft_context
        .as_ref()
        .and_then(|draft| draft.planner_diagnostics.clone());
    let plan_json = compiler_api::workflow_plan_to_json(&plan).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error,
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        )
    })?;
    let mut plan_package = compiler_api::compile_workflow_plan_preview_package_with_revision(
        &plan_json,
        Some("workflow_planner"),
        apply_revision,
    );
    let plan_package_validation = compiler_api::validate_plan_package(&plan_package);
    let mut overlap_analysis = compile_preview_plan_overlap(&state, &plan_package).await;
    if plan_package_validation.blocker_count > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "plan package validation failed",
                "code": "WORKFLOW_PLAN_INVALID",
                "plan_package": plan_package,
                "plan_package_validation": plan_package_validation,
            })),
        ));
    }
    let requested_overlap_decision = parse_overlap_decision(input.overlap_decision.as_deref())
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": error,
                    "code": "WORKFLOW_PLAN_INVALID",
                })),
            )
        })?;
    if overlap_analysis.requires_user_confirmation && requested_overlap_decision.is_none() {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "overlap confirmation is required before apply",
                "code": "WORKFLOW_PLAN_OVERLAP_CONFIRMATION_REQUIRED",
                "plan_package": plan_package,
                "plan_package_validation": plan_package_validation,
                "overlap_analysis": overlap_analysis,
            })),
        ));
    }
    if overlap_analysis.matched_plan_id.is_none() && requested_overlap_decision.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "overlap_decision was provided but no prior overlap was detected",
                "code": "WORKFLOW_PLAN_INVALID",
            })),
        ));
    }
    if let Some(decision) = requested_overlap_decision {
        overlap_analysis.decision = decision;
        overlap_analysis.requires_user_confirmation = false;
    }
    if let Some(entry) = compiler_api::overlap_log_entry_from_analysis(
        &overlap_analysis,
        input.creator_id.as_deref().unwrap_or("workflow_planner"),
        &chrono::Utc::now().to_rfc3339(),
    ) {
        plan_package
            .overlap_policy
            .get_or_insert_with(Default::default)
            .overlap_log
            .push(entry);
    }

    let mut automation = compile_plan_to_automation_v2(
        &plan,
        Some(&plan_package),
        input.creator_id.as_deref().unwrap_or("workflow_planner"),
    );
    let approved_plan_materialization = compiler_api::approved_plan_materialization(&plan_package);
    let approved_plan_success_memory =
        compiler_api::approved_plan_success_memory_value(&plan_package);
    let plan_package_bundle = compiler_api::export_plan_package_bundle(&plan_package);
    if let Some(metadata) = automation.metadata.as_mut().and_then(Value::as_object_mut) {
        metadata.insert(
            "plan_source".to_string(),
            serde_json::to_value(&plan.plan_source).unwrap_or(Value::Null),
        );
        metadata.insert(
            "plan_package".to_string(),
            serde_json::to_value(&plan_package).unwrap_or(Value::Null),
        );
        metadata.insert(
            "plan_package_bundle".to_string(),
            serde_json::to_value(&plan_package_bundle).unwrap_or(Value::Null),
        );
        metadata.insert(
            "plan_package_validation".to_string(),
            serde_json::to_value(&plan_package_validation).unwrap_or(Value::Null),
        );
        metadata.insert(
            "overlap_analysis".to_string(),
            serde_json::to_value(&overlap_analysis).unwrap_or(Value::Null),
        );
        metadata.insert(
            "approved_plan_materialization".to_string(),
            approved_plan_success_memory.clone(),
        );
        metadata.insert(
            "planner_diagnostics".to_string(),
            planner_diagnostics.clone().unwrap_or(Value::Null),
        );
    } else {
        automation.metadata = Some(json!({
            "plan_package": plan_package,
            "plan_package_bundle": plan_package_bundle.clone(),
            "plan_package_validation": plan_package_validation,
            "overlap_analysis": overlap_analysis,
            "approved_plan_materialization": approved_plan_success_memory.clone(),
            "planner_diagnostics": planner_diagnostics,
        }));
    }
    let stored = state.put_automation_v2(automation).await.map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error.to_string(),
                "code": "WORKFLOW_PLAN_APPLY_FAILED",
            })),
        )
    })?;
    if let Some(plan_id) = plan_id.as_deref() {
        if let Some(mut draft) = state.get_workflow_plan_draft(plan_id).await {
            draft.last_success_materialization = Some(approved_plan_success_memory);
            state.put_workflow_plan_draft(draft).await;
        }
    }
    let pack_builder_export = match input.pack_builder_export {
        Some(export) if export.enabled.unwrap_or(true) => {
            Some(export_workflow_plan_to_pack_builder(&state, &plan, &export).await)
        }
        _ => None,
    };
    Ok(Json(json!({
        "ok": true,
        "plan": plan,
        "plan_package": plan_package,
        "plan_package_bundle": plan_package_bundle,
        "overlap_analysis": overlap_analysis,
        "approved_plan_materialization": approved_plan_materialization,
        "automation": stored,
        "pack_builder_export": pack_builder_export,
    })))
}

async fn workflow_plan_import_inner(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanImportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let report = compiler_api::validate_plan_package_bundle(&input.bundle);
    if !report.compatible {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "plan bundle import validation failed",
                "code": "WORKFLOW_PLAN_INVALID",
                "bundle": input.bundle,
                "import_validation": report,
            })),
        ));
    }
    let workspace_root = state.workspace_index.snapshot().await.root;
    let import_preview = compiler_api::preview_plan_package_import_bundle(
        &input.bundle,
        &workspace_root,
        input.creator_id.as_deref().unwrap_or("workflow_planner"),
    );
    let plan_package_validation = compiler_api::validate_plan_package(&import_preview.plan_package);
    Ok(Json(json!({
        "ok": true,
        "bundle": input.bundle,
        "import_validation": report,
        "plan_package_preview": import_preview.plan_package,
        "plan_package_validation": plan_package_validation,
        "derived_scope_snapshot": import_preview.derived_scope_snapshot,
        "summary": workflow_plan_import_summary(&import_preview.plan_package),
        "import_transform_log": import_preview.import_transform_log,
        "import_source_bundle_digest": import_preview.source_bundle_digest,
    })))
}

pub(super) async fn workflow_plan_import(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanImportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    workflow_plan_import_inner(State(state), Json(input)).await
}

pub(super) async fn workflow_plan_import_preview(
    State(state): State<AppState>,
    Json(input): Json<WorkflowPlanImportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    workflow_plan_import_inner(State(state), Json(input)).await
}

async fn export_workflow_plan_to_pack_builder(
    state: &AppState,
    plan: &crate::WorkflowPlan,
    export: &WorkflowPlanPackBuilderExportRequest,
) -> Value {
    let args = compiler_api::pack_builder_export_args(
        plan,
        &compiler_api::PackBuilderExportOptions {
            session_id: export.session_id.clone(),
            thread_key: export.thread_key.clone(),
            auto_apply: export.auto_apply.unwrap_or(false),
        },
    );
    match super::pack_builder::run_pack_builder_tool(state, args).await {
        Ok(payload) => payload,
        Err(code) => json!({
            "status": "export_failed",
            "error": "pack_builder_export_failed",
            "http_status": code.as_u16(),
        }),
    }
}
