use super::*;

#[tokio::test]
async fn channels_config_returns_non_secret_shape() {
    let state = test_state().await;
    let _ = state
        .config
        .patch_project(json!({
            "channels": {
                "telegram": {
                    "bot_token": "tg-secret",
                    "allowed_users": ["@alice", "@bob"],
                    "mention_only": true
                },
                "discord": {
                    "bot_token": "dc-secret",
                    "allowed_users": ["*"],
                    "mention_only": false,
                    "guild_id": "1234"
                },
                "slack": {
                    "bot_token": "sl-secret",
                    "channel_id": "C123",
                    "allowed_users": ["U1"]
                }
            }
        }))
        .await
        .expect("patch project");
    let app = app_router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/channels/config")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("response body");
    let payload: Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(
        payload
            .get("telegram")
            .and_then(|v| v.get("has_token"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(payload
        .get("telegram")
        .and_then(Value::as_object)
        .is_some_and(|obj| !obj.contains_key("bot_token")));
    assert!(payload
        .get("discord")
        .and_then(Value::as_object)
        .is_some_and(|obj| !obj.contains_key("bot_token")));
    assert!(payload
        .get("slack")
        .and_then(Value::as_object)
        .is_some_and(|obj| !obj.contains_key("bot_token")));
}

#[tokio::test]
async fn channels_verify_discord_without_token_returns_setup_hint() {
    let state = test_state().await;
    let app = app_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/channels/discord/verify")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("response body");
    let payload: Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(
        payload.get("ok").and_then(Value::as_bool),
        Some(false),
        "verify should fail without token"
    );
    assert_eq!(
        payload.get("channel").and_then(Value::as_str),
        Some("discord")
    );
    assert!(
        payload
            .get("hints")
            .and_then(Value::as_array)
            .is_some_and(|arr| !arr.is_empty()),
        "verify should include setup hints"
    );
}
