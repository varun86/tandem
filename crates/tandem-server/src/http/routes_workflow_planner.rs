use axum::routing::{get, post};
use axum::Router;

use super::workflow_planner::*;
use crate::AppState;

pub(super) fn apply(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/workflow-plans/preview", post(workflow_plan_preview))
        .route("/workflow-plans/apply", post(workflow_plan_apply))
        .route("/workflow-plans/chat/start", post(workflow_plan_chat_start))
        .route(
            "/workflow-plans/chat/message",
            post(workflow_plan_chat_message),
        )
        .route("/workflow-plans/chat/reset", post(workflow_plan_chat_reset))
        .route(
            "/workflow-plans/import/preview",
            post(workflow_plan_import_preview),
        )
        .route("/workflow-plans/import", post(workflow_plan_import))
        .route("/workflow-plans/{plan_id}", get(workflow_plan_get))
}
