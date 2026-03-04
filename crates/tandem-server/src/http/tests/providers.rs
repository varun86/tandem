use super::*;

#[tokio::test]
async fn provider_route_returns_catalog_shape() {
    let state = test_state().await;
    let app = app_router(state);
    let req = Request::builder()
        .method("GET")
        .uri("/provider")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
    let payload: Value = serde_json::from_slice(&body).expect("json");
    let all = payload
        .get("all")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(!all.is_empty());
    let first = all.first().cloned().unwrap_or_else(|| json!({}));
    assert!(first.get("id").and_then(|v| v.as_str()).is_some());
}

#[test]
fn merge_known_provider_defaults_does_not_mark_all_connected() {
    let mut wire = WireProviderCatalog {
        all: Vec::new(),
        connected: Vec::new(),
    };
    merge_known_provider_defaults(&mut wire);

    assert!(wire.all.iter().any(|p| p.id == "openrouter"));
    assert!(wire.connected.is_empty());
}
