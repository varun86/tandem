use axum::routing::{get, post, put};
use axum::Router;

use super::*;

pub(super) fn apply(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            "/capabilities/bindings",
            get(capabilities_bindings_get).put(capabilities_bindings_put),
        )
        .route("/capabilities/discovery", get(capabilities_discovery))
        .route("/capabilities/resolve", post(capabilities_resolve))
        .route("/capabilities/readiness", post(capabilities_readiness))
}
