use super::*;
use axum::{routing::get, Router};
use tokio::net::TcpListener;

#[tokio::test]
async fn provider_route_returns_known_providers_without_synthetic_default_models() {
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
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let openai = all
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some("openai"))
        .cloned()
        .expect("openai entry");

    assert_eq!(
        openai
            .get("models")
            .and_then(Value::as_object)
            .map(|m| m.len()),
        Some(0)
    );
    assert_eq!(
        openai.get("catalog_source").and_then(Value::as_str),
        Some("empty")
    );
    assert_eq!(
        openai.get("catalog_status").and_then(Value::as_str),
        Some("unavailable")
    );
}

#[tokio::test]
async fn provider_route_marks_config_models_as_config_catalogs() {
    let state = test_state().await;
    state
        .config
        .patch_project(json!({
            "providers": {
                "openai": {
                    "url": "https://api.openai.com/v1",
                    "models": {
                        "gpt-4.1-mini": {
                            "name": "GPT 4.1 Mini",
                            "context_length": 128000
                        }
                    }
                }
            }
        }))
        .await
        .expect("patch project");
    state
        .providers
        .reload(state.config.get().await.into())
        .await;

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
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let openai = all
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some("openai"))
        .cloned()
        .expect("openai entry");

    assert_eq!(
        openai.get("catalog_source").and_then(Value::as_str),
        Some("config")
    );
    assert_eq!(
        openai.get("catalog_status").and_then(Value::as_str),
        Some("ok")
    );
    assert!(
        openai
            .get("models")
            .and_then(Value::as_object)
            .and_then(|models| models.get("gpt-4.1-mini"))
            .is_some(),
        "expected configured model to appear in catalog"
    );
}

#[tokio::test]
async fn provider_route_uses_runtime_auth_for_remote_catalog_fetch() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let app = Router::new().route(
        "/v1/models",
        get(|| async {
            Json(json!({
                "data": [
                    { "id": "gpt-4.1-mini", "name": "GPT 4.1 Mini", "context_length": 128000 }
                ]
            }))
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve test provider");
    });

    let state = test_state().await;
    state
        .config
        .patch_project(json!({
            "providers": {
                "openai": {
                    "url": format!("http://{addr}/v1")
                }
            }
        }))
        .await
        .expect("patch project");
    state
        .auth
        .write()
        .await
        .insert("openai".to_string(), "test-key".to_string());
    state
        .providers
        .reload(state.config.get().await.into())
        .await;

    let app = app_router(state);
    let req = Request::builder()
        .method("GET")
        .uri("/provider")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    server.abort();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
    let payload: Value = serde_json::from_slice(&body).expect("json");
    let all = payload
        .get("all")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let openai = all
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some("openai"))
        .cloned()
        .expect("openai entry");

    assert_eq!(
        openai.get("catalog_source").and_then(Value::as_str),
        Some("remote")
    );
    assert!(
        openai
            .get("models")
            .and_then(Value::as_object)
            .and_then(|models| models.get("gpt-4.1-mini"))
            .is_some(),
        "expected runtime-auth-backed remote catalog"
    );
}
