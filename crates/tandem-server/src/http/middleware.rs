use axum::extract::{Request, State};
use axum::http::header;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::{AppState, StartupStatus};

use super::ErrorEnvelope;

pub(super) async fn auth_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }
    let path = request.uri().path();
    if state.web_ui_enabled() && request.uri().path().starts_with(&state.web_ui_prefix()) {
        return next.run(request).await;
    }
    if path == "/global/health" {
        return next.run(request).await;
    }

    let required = state.api_token().await;
    let Some(expected) = required else {
        return next.run(request).await;
    };

    let provided = extract_request_token(request.headers());
    if provided.as_deref() == Some(expected.as_str()) {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorEnvelope {
            error: "Unauthorized: missing or invalid API token".to_string(),
            code: Some("AUTH_REQUIRED".to_string()),
        }),
    )
        .into_response()
}

fn extract_request_token(headers: &HeaderMap) -> Option<String> {
    if let Some(token) = headers
        .get("x-agent-token")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Some(token.to_string());
    }
    if let Some(token) = headers
        .get("x-tandem-token")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Some(token.to_string());
    }

    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())?;
    let trimmed = auth.trim();
    let bearer = trimmed
        .strip_prefix("Bearer ")
        .or_else(|| trimmed.strip_prefix("bearer "))?;
    let token = bearer.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

pub(super) async fn startup_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }
    if request.uri().path() == "/global/health" {
        return next.run(request).await;
    }
    if state.is_ready() {
        return next.run(request).await;
    }

    let snapshot = state.startup_snapshot().await;
    let status_text = match snapshot.status {
        StartupStatus::Starting => "starting",
        StartupStatus::Ready => "ready",
        StartupStatus::Failed => "failed",
    };
    let code = match snapshot.status {
        StartupStatus::Failed => "ENGINE_STARTUP_FAILED",
        _ => "ENGINE_STARTING",
    };
    let error = format!(
        "Engine {}: phase={} attempt_id={} elapsed_ms={}{}",
        status_text,
        snapshot.phase,
        snapshot.attempt_id,
        snapshot.elapsed_ms,
        snapshot
            .last_error
            .as_ref()
            .map(|e| format!(" error={}", e))
            .unwrap_or_default()
    );
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorEnvelope {
            error,
            code: Some(code.to_string()),
        }),
    )
        .into_response()
}
