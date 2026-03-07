use crate::http::AppState;
use crate::FailureReporterConfig;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, Default)]
pub(super) struct FailureReporterConfigInput {
    #[serde(default)]
    pub failure_reporter: Option<FailureReporterConfig>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct FailureReporterDraftsQuery {
    pub limit: Option<usize>,
}

pub(super) async fn get_failure_reporter_config(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let config = state.failure_reporter_config().await;
    Json(json!({
        "failure_reporter": config
    }))
}

pub(super) async fn patch_failure_reporter_config(
    State(state): State<AppState>,
    Json(input): Json<FailureReporterConfigInput>,
) -> Response {
    let Some(config) = input.failure_reporter else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "failure_reporter object is required",
                "code": "FAILURE_REPORTER_CONFIG_REQUIRED",
            })),
        )
            .into_response();
    };
    match state.put_failure_reporter_config(config).await {
        Ok(saved) => Json(json!({ "failure_reporter": saved })).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid failure reporter config",
                "code": "FAILURE_REPORTER_CONFIG_INVALID",
                "detail": error.to_string(),
            })),
        )
            .into_response(),
    }
}

pub(super) async fn get_failure_reporter_status(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let status = state.failure_reporter_status().await;
    Json(json!({
        "status": status
    }))
}

pub(super) async fn list_failure_reporter_drafts(
    State(state): State<AppState>,
    Query(query): Query<FailureReporterDraftsQuery>,
) -> Json<serde_json::Value> {
    let drafts = state
        .list_failure_reporter_drafts(query.limit.unwrap_or(50))
        .await;
    Json(json!({
        "drafts": drafts,
        "count": drafts.len(),
    }))
}

pub(super) async fn get_failure_reporter_draft(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let draft = state
        .list_failure_reporter_drafts(200)
        .await
        .into_iter()
        .find(|row| row.draft_id == id);
    match draft {
        Some(draft) => Json(json!({ "draft": draft })).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Failure reporter draft not found",
                "code": "FAILURE_REPORTER_DRAFT_NOT_FOUND",
            })),
        )
            .into_response(),
    }
}
