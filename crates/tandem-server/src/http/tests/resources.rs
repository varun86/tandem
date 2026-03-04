use super::*;

#[tokio::test]
async fn resource_put_patch_get_and_list_roundtrip() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let put_req = Request::builder()
        .method("PUT")
        .uri("/resource/project/demo/board")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "value": {"status":"todo","count":1},
                "updated_by": "agent-1"
            })
            .to_string(),
        ))
        .expect("put request");
    let put_resp = app.clone().oneshot(put_req).await.expect("put response");
    assert_eq!(put_resp.status(), StatusCode::OK);

    let patch_req = Request::builder()
        .method("PATCH")
        .uri("/resource/project/demo/board")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "value": {"count":2},
                "if_match_rev": 1,
                "updated_by": "agent-2"
            })
            .to_string(),
        ))
        .expect("patch request");
    let patch_resp = app
        .clone()
        .oneshot(patch_req)
        .await
        .expect("patch response");
    assert_eq!(patch_resp.status(), StatusCode::OK);

    let get_req = Request::builder()
        .method("GET")
        .uri("/resource/project/demo/board")
        .body(Body::empty())
        .expect("get request");
    let get_resp = app.clone().oneshot(get_req).await.expect("get response");
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body = to_bytes(get_resp.into_body(), usize::MAX)
        .await
        .expect("get body");
    let payload: Value = serde_json::from_slice(&get_body).expect("json");
    assert_eq!(
        payload
            .get("resource")
            .and_then(|r| r.get("rev"))
            .and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        payload
            .get("resource")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str()),
        Some("todo")
    );
    assert_eq!(
        payload
            .get("resource")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64()),
        Some(2)
    );

    let list_req = Request::builder()
        .method("GET")
        .uri("/resource?prefix=project/demo")
        .body(Body::empty())
        .expect("list request");
    let list_resp = app.clone().oneshot(list_req).await.expect("list response");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = to_bytes(list_resp.into_body(), usize::MAX)
        .await
        .expect("list body");
    let list_payload: Value = serde_json::from_slice(&list_body).expect("json");
    assert_eq!(list_payload.get("count").and_then(|v| v.as_u64()), Some(1));
}

#[tokio::test]
async fn resource_put_conflict_returns_409() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let first_req = Request::builder()
        .method("PUT")
        .uri("/resource/mission/demo/card-1")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "value": {"title":"Card 1"},
                "updated_by": "agent-1"
            })
            .to_string(),
        ))
        .expect("first request");
    let first_resp = app
        .clone()
        .oneshot(first_req)
        .await
        .expect("first response");
    assert_eq!(first_resp.status(), StatusCode::OK);

    let conflict_req = Request::builder()
        .method("PUT")
        .uri("/resource/mission/demo/card-1")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "value": {"title":"Card 1 updated"},
                "if_match_rev": 99,
                "updated_by": "agent-2"
            })
            .to_string(),
        ))
        .expect("conflict request");
    let conflict_resp = app
        .clone()
        .oneshot(conflict_req)
        .await
        .expect("conflict response");
    assert_eq!(conflict_resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn resource_updated_event_contract_snapshot() {
    let state = test_state().await;
    let mut rx = state.event_bus.subscribe();
    let app = app_router(state.clone());

    let put_req = Request::builder()
        .method("PUT")
        .uri("/resource/project/demo/board")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "value": {"status":"todo"},
                "updated_by": "agent-1"
            })
            .to_string(),
        ))
        .expect("put request");
    let put_resp = app.clone().oneshot(put_req).await.expect("put response");
    assert_eq!(put_resp.status(), StatusCode::OK);

    let event = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("event");
            if event.event_type == "resource.updated" {
                return event;
            }
        }
    })
    .await
    .expect("resource.updated timeout");

    let mut properties = event
        .properties
        .as_object()
        .cloned()
        .expect("resource.updated properties object");
    let updated_at_ms = properties
        .remove("updatedAtMs")
        .and_then(|v| v.as_u64())
        .expect("updatedAtMs");
    assert!(updated_at_ms > 0);

    let snapshot = json!({
        "type": event.event_type,
        "properties": properties,
    });
    let expected = json!({
        "type": "resource.updated",
        "properties": {
            "key": "project/demo/board",
            "rev": 1,
            "updatedBy": "agent-1"
        }
    });
    assert_eq!(snapshot, expected);
}

#[tokio::test]
async fn resource_deleted_event_contract_snapshot() {
    let state = test_state().await;
    let mut rx = state.event_bus.subscribe();
    let app = app_router(state.clone());

    let put_req = Request::builder()
        .method("PUT")
        .uri("/resource/project/demo/board")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "value": {"status":"todo"},
                "updated_by": "agent-1"
            })
            .to_string(),
        ))
        .expect("put request");
    let put_resp = app.clone().oneshot(put_req).await.expect("put response");
    assert_eq!(put_resp.status(), StatusCode::OK);

    let delete_req = Request::builder()
        .method("DELETE")
        .uri("/resource/project/demo/board")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "if_match_rev": 1,
                "updated_by": "reviewer-1"
            })
            .to_string(),
        ))
        .expect("delete request");
    let delete_resp = app
        .clone()
        .oneshot(delete_req)
        .await
        .expect("delete response");
    assert_eq!(delete_resp.status(), StatusCode::OK);

    let event = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("event");
            if event.event_type == "resource.deleted" {
                return event;
            }
        }
    })
    .await
    .expect("resource.deleted timeout");

    let mut properties = event
        .properties
        .as_object()
        .cloned()
        .expect("resource.deleted properties object");
    let updated_at_ms = properties
        .remove("updatedAtMs")
        .and_then(|v| v.as_u64())
        .expect("updatedAtMs");
    assert!(updated_at_ms > 0);

    let snapshot = json!({
        "type": event.event_type,
        "properties": properties,
    });
    let expected = json!({
        "type": "resource.deleted",
        "properties": {
            "key": "project/demo/board",
            "rev": 1,
            "updatedBy": "reviewer-1"
        }
    });
    assert_eq!(snapshot, expected);
}
