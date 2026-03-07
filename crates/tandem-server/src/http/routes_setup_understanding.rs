use axum::routing::post;
use axum::Router;

use super::*;

pub(super) fn apply(router: Router<AppState>) -> Router<AppState> {
    router.route("/setup/understand", post(setup_understand))
}
