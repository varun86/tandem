use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};

include!("context_runs_parts/part01.rs");
include!("context_runs_parts/part02.rs");
include!("context_runs_parts/part03.rs");

pub async fn context_run_effective_started_at_ms(
    state: &AppState,
    run_id: &str,
) -> Result<u64, StatusCode> {
    let run = load_context_run_state(state, run_id).await?;
    Ok(run.started_at_ms.unwrap_or(run.created_at_ms))
}
