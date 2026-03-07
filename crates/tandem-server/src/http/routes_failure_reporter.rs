use super::failure_reporter::*;
use crate::http::AppState;
use axum::routing::get;
use axum::Router;

pub(super) fn apply(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            "/config/failure-reporter",
            get(get_failure_reporter_config).patch(patch_failure_reporter_config),
        )
        .route("/failure-reporter/status", get(get_failure_reporter_status))
        .route(
            "/failure-reporter/drafts",
            get(list_failure_reporter_drafts),
        )
        .route(
            "/failure-reporter/drafts/{id}",
            get(get_failure_reporter_draft),
        )
}
