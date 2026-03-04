use axum::routing::{get, post};
use axum::Router;

use super::context_runs::*;
use crate::AppState;

pub(super) fn apply(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            "/context/runs",
            post(context_run_create).get(context_run_list),
        )
        .route(
            "/context/runs/{run_id}",
            get(context_run_get).put(context_run_put),
        )
        .route(
            "/context/runs/{run_id}/events",
            get(context_run_events).post(context_run_event_append),
        )
        .route(
            "/context/runs/{run_id}/todos/sync",
            post(context_run_todos_sync),
        )
        .route(
            "/context/runs/{run_id}/events/stream",
            get(context_run_events_stream),
        )
        .route(
            "/context/runs/{run_id}/lease/validate",
            post(context_run_lease_validate),
        )
        .route(
            "/context/runs/{run_id}/blackboard",
            get(context_run_blackboard_get),
        )
        .route(
            "/context/runs/{run_id}/blackboard/patches",
            get(context_run_blackboard_patches_get).post(context_run_blackboard_patch),
        )
        .route(
            "/context/runs/{run_id}/tasks",
            post(context_run_tasks_create),
        )
        .route(
            "/context/runs/{run_id}/tasks/claim",
            post(context_run_tasks_claim),
        )
        .route(
            "/context/runs/{run_id}/tasks/{task_id}/transition",
            post(context_run_task_transition),
        )
        .route(
            "/context/runs/{run_id}/checkpoints",
            post(context_run_checkpoint_create),
        )
        .route(
            "/context/runs/{run_id}/checkpoints/latest",
            get(context_run_checkpoint_latest),
        )
        .route("/context/runs/{run_id}/replay", get(context_run_replay))
        .route(
            "/context/runs/{run_id}/driver/next",
            post(context_run_driver_next),
        )
}
