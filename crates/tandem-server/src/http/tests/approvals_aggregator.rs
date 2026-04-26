use super::global::create_test_automation_v2;
use super::*;

use axum::body::{to_bytes, Body};
use axum::http::Request;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn approvals_pending_endpoint_surfaces_automation_v2_awaiting_gate() {
    let state = test_state().await;
    let app = app_router(state.clone());
    let automation = create_test_automation_v2(&state, "auto-v2-approvals-aggregator").await;
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("run");

    state
        .update_automation_v2_run(&run.run_id, |row| {
            row.status = crate::AutomationRunStatus::AwaitingApproval;
            row.checkpoint.awaiting_gate = Some(crate::AutomationPendingGate {
                node_id: "publish".to_string(),
                title: "Publish approval".to_string(),
                instructions: Some("approve final publish step".to_string()),
                decisions: vec![
                    "approve".to_string(),
                    "rework".to_string(),
                    "cancel".to_string(),
                ],
                rework_targets: vec!["draft".to_string()],
                requested_at_ms: crate::now_ms(),
                upstream_node_ids: vec!["draft".to_string()],
            });
        })
        .await
        .expect("updated run");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/approvals/pending")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), 200);

    let body = to_bytes(resp.into_body(), 1_000_000)
        .await
        .expect("body bytes");
    let payload: Value = serde_json::from_slice(&body).expect("json body");

    let approvals = payload
        .get("approvals")
        .and_then(Value::as_array)
        .expect("approvals array");
    assert!(!approvals.is_empty(), "expected at least one approval");

    let first = &approvals[0];
    assert_eq!(
        first.get("source").and_then(Value::as_str),
        Some("automation_v2")
    );
    assert_eq!(
        first.get("run_id").and_then(Value::as_str),
        Some(run.run_id.as_str())
    );
    assert_eq!(
        first.get("node_id").and_then(Value::as_str),
        Some("publish")
    );
    let request_id = first
        .get("request_id")
        .and_then(Value::as_str)
        .expect("request_id");
    assert!(
        request_id.starts_with("automation_v2:"),
        "request_id should be namespaced: {request_id}",
    );
    let decisions = first
        .get("decisions")
        .and_then(Value::as_array)
        .expect("decisions array");
    assert_eq!(decisions.len(), 3);

    let surface = first
        .get("surface_payload")
        .expect("surface_payload object");
    assert_eq!(
        surface.get("decide_endpoint").and_then(Value::as_str),
        Some(format!("/automations/v2/runs/{}/gate_decide", run.run_id).as_str())
    );

    let count = payload.get("count").and_then(Value::as_u64).unwrap_or(0);
    assert!(count >= 1);
}

#[tokio::test]
async fn approvals_pending_endpoint_returns_empty_when_no_gates_pending() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/approvals/pending")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), 200);

    let body = to_bytes(resp.into_body(), 1_000_000)
        .await
        .expect("body bytes");
    let payload: Value = serde_json::from_slice(&body).expect("json body");
    let approvals = payload
        .get("approvals")
        .and_then(Value::as_array)
        .expect("approvals array");
    assert!(approvals.is_empty());
    assert_eq!(payload.get("count").and_then(Value::as_u64), Some(0));
}

#[tokio::test]
async fn approvals_pending_endpoint_filters_by_source_unknown_returns_empty() {
    let state = test_state().await;
    let app = app_router(state.clone());
    let automation = create_test_automation_v2(&state, "auto-v2-approvals-source-filter").await;
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("run");

    state
        .update_automation_v2_run(&run.run_id, |row| {
            row.status = crate::AutomationRunStatus::AwaitingApproval;
            row.checkpoint.awaiting_gate = Some(crate::AutomationPendingGate {
                node_id: "publish".to_string(),
                title: "Publish approval".to_string(),
                instructions: None,
                decisions: vec!["approve".to_string()],
                rework_targets: vec![],
                requested_at_ms: crate::now_ms(),
                upstream_node_ids: vec![],
            });
        })
        .await
        .expect("updated run");

    // Filter by `coder` — automation_v2 records should be excluded.
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/approvals/pending?source=coder")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(resp.into_body(), 1_000_000).await.expect("body");
    let payload: Value = serde_json::from_slice(&body).expect("json");
    let approvals = payload
        .get("approvals")
        .and_then(Value::as_array)
        .expect("approvals array");
    assert!(approvals.is_empty());
}
