use super::*;

#[tokio::test]
async fn routines_create_run_now_and_history_roundtrip() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-1",
                "name": "Daily digest",
                "schedule": { "interval_seconds": { "seconds": 60 } },
                "entrypoint": "mission.default",
                "creator_type": "user",
                "creator_id": "u-1"
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/routines/routine-1/run_now")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "run_count": 2,
                "reason": "manual smoke check"
            })
            .to_string(),
        ))
        .expect("run_now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("run_now response");
    assert_eq!(run_now_resp.status(), StatusCode::OK);

    let history_req = Request::builder()
        .method("GET")
        .uri("/routines/routine-1/history?limit=10")
        .body(Body::empty())
        .expect("history request");
    let history_resp = app
        .clone()
        .oneshot(history_req)
        .await
        .expect("history response");
    assert_eq!(history_resp.status(), StatusCode::OK);
    let history_body = to_bytes(history_resp.into_body(), usize::MAX)
        .await
        .expect("history body");
    let history_payload: Value = serde_json::from_slice(&history_body).expect("history json");
    assert_eq!(
        history_payload.get("count").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        history_payload
            .get("events")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("run_count"))
            .and_then(|v| v.as_u64()),
        Some(2)
    );
}

#[tokio::test]
async fn routines_patch_can_pause_routine() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-2",
                "name": "Research routine",
                "schedule": { "interval_seconds": { "seconds": 120 } },
                "entrypoint": "mission.default"
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let patch_req = Request::builder()
        .method("PATCH")
        .uri("/routines/routine-2")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "status": "paused"
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
    let patch_body = to_bytes(patch_resp.into_body(), usize::MAX)
        .await
        .expect("patch body");
    let patch_payload: Value = serde_json::from_slice(&patch_body).expect("patch json");
    assert_eq!(
        patch_payload
            .get("routine")
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str()),
        Some("paused")
    );
}

#[tokio::test]
async fn routines_allowlist_is_persisted_and_copied_to_runs() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-tools",
                "name": "Tool-scoped routine",
                "schedule": { "interval_seconds": { "seconds": 90 } },
                "entrypoint": "mission.default",
                "allowed_tools": ["  mcp.arcade.search  ", "read", "read", ""],
                "output_targets": ["  s3://reports/daily.json  ", "s3://reports/daily.json", ""]
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .expect("create body");
    let create_payload: Value = serde_json::from_slice(&create_body).expect("create json");
    assert_eq!(
        create_payload
            .get("routine")
            .and_then(|v| v.get("allowed_tools"))
            .and_then(|v| v.as_array())
            .map(|rows| rows
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()),
        Some(vec!["mcp.arcade.search".to_string(), "read".to_string()])
    );
    assert_eq!(
        create_payload
            .get("routine")
            .and_then(|v| v.get("output_targets"))
            .and_then(|v| v.as_array())
            .map(|rows| rows
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()),
        Some(vec!["s3://reports/daily.json".to_string()])
    );

    let patch_req = Request::builder()
        .method("PATCH")
        .uri("/routines/routine-tools")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "allowed_tools": ["mcp.arcade.send_email", "bash"],
                "output_targets": ["https://storage.example/run/output.md"]
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
    let patch_body = to_bytes(patch_resp.into_body(), usize::MAX)
        .await
        .expect("patch body");
    let patch_payload: Value = serde_json::from_slice(&patch_body).expect("patch json");
    assert_eq!(
        patch_payload
            .get("routine")
            .and_then(|v| v.get("allowed_tools"))
            .and_then(|v| v.as_array())
            .map(|rows| rows
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()),
        Some(vec![
            "mcp.arcade.send_email".to_string(),
            "bash".to_string()
        ])
    );
    assert_eq!(
        patch_payload
            .get("routine")
            .and_then(|v| v.get("output_targets"))
            .and_then(|v| v.as_array())
            .map(|rows| rows
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()),
        Some(vec!["https://storage.example/run/output.md".to_string()])
    );

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/routines/routine-tools/run_now")
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .expect("run_now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("run_now response");
    assert_eq!(run_now_resp.status(), StatusCode::OK);
    let run_now_body = to_bytes(run_now_resp.into_body(), usize::MAX)
        .await
        .expect("run_now body");
    let run_now_payload: Value = serde_json::from_slice(&run_now_body).expect("run_now json");
    let run_id = run_now_payload
        .get("runID")
        .and_then(|v| v.as_str())
        .expect("runID");

    let run_get_req = Request::builder()
        .method("GET")
        .uri(format!("/routines/runs/{run_id}"))
        .body(Body::empty())
        .expect("run get request");
    let run_get_resp = app
        .clone()
        .oneshot(run_get_req)
        .await
        .expect("run get response");
    assert_eq!(run_get_resp.status(), StatusCode::OK);
    let run_get_body = to_bytes(run_get_resp.into_body(), usize::MAX)
        .await
        .expect("run get body");
    let run_get_payload: Value = serde_json::from_slice(&run_get_body).expect("run get json");
    assert_eq!(
        run_get_payload
            .get("run")
            .and_then(|v| v.get("allowed_tools"))
            .and_then(|v| v.as_array())
            .map(|rows| rows
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()),
        Some(vec![
            "mcp.arcade.send_email".to_string(),
            "bash".to_string()
        ])
    );
    assert_eq!(
        run_get_payload
            .get("run")
            .and_then(|v| v.get("output_targets"))
            .and_then(|v| v.as_array())
            .map(|rows| rows
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()),
        Some(vec!["https://storage.example/run/output.md".to_string()])
    );
}

#[tokio::test]
async fn routines_runs_all_can_filter_by_routine() {
    let state = test_state().await;
    let app = app_router(state.clone());

    for routine_id in ["routine-run-a", "routine-run-b"] {
        let create_req = Request::builder()
            .method("POST")
            .uri("/routines")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "routine_id": routine_id,
                    "name": format!("Routine {routine_id}"),
                    "schedule": { "interval_seconds": { "seconds": 60 } },
                    "entrypoint": "mission.default",
                })
                .to_string(),
            ))
            .expect("create request");
        let create_resp = app
            .clone()
            .oneshot(create_req)
            .await
            .expect("create response");
        assert_eq!(create_resp.status(), StatusCode::OK);

        let run_now_req = Request::builder()
            .method("POST")
            .uri(format!("/routines/{routine_id}/run_now"))
            .header("content-type", "application/json")
            .body(Body::from(json!({}).to_string()))
            .expect("run_now request");
        let run_now_resp = app
            .clone()
            .oneshot(run_now_req)
            .await
            .expect("run_now response");
        assert_eq!(run_now_resp.status(), StatusCode::OK);
    }

    let all_req = Request::builder()
        .method("GET")
        .uri("/routines/runs?limit=10")
        .body(Body::empty())
        .expect("runs all request");
    let all_resp = app
        .clone()
        .oneshot(all_req)
        .await
        .expect("runs all response");
    assert_eq!(all_resp.status(), StatusCode::OK);
    let all_body = to_bytes(all_resp.into_body(), usize::MAX)
        .await
        .expect("runs all body");
    let all_payload: Value = serde_json::from_slice(&all_body).expect("runs all json");
    assert!(all_payload
        .get("count")
        .and_then(|v| v.as_u64())
        .is_some_and(|count| count >= 2));

    let filtered_req = Request::builder()
        .method("GET")
        .uri("/routines/runs?routine_id=routine-run-b&limit=10")
        .body(Body::empty())
        .expect("runs filtered request");
    let filtered_resp = app
        .clone()
        .oneshot(filtered_req)
        .await
        .expect("runs filtered response");
    assert_eq!(filtered_resp.status(), StatusCode::OK);
    let filtered_body = to_bytes(filtered_resp.into_body(), usize::MAX)
        .await
        .expect("runs filtered body");
    let filtered_payload: Value =
        serde_json::from_slice(&filtered_body).expect("runs filtered json");
    assert!(filtered_payload
        .get("count")
        .and_then(|v| v.as_u64())
        .is_some_and(|count| count >= 1));
    let all_match_routine = filtered_payload
        .get("runs")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter().all(|row| {
                row.get("routine_id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == "routine-run-b")
            })
        })
        .unwrap_or(false);
    assert!(all_match_routine);
}

#[tokio::test]
async fn automations_create_requires_mission_objective() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/automations")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "automation_id": "auto-empty-objective",
                "name": "Automation without objective",
                "schedule": { "interval_seconds": { "seconds": 300 } },
                "mission": {
                    "objective": "   "
                }
            })
            .to_string(),
        ))
        .expect("automation create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("automation create response");
    assert_eq!(create_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn automations_create_rejects_invalid_mode() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/automations")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "automation_id": "auto-invalid-mode",
                "name": "Automation invalid mode",
                "schedule": { "interval_seconds": { "seconds": 300 } },
                "mode": "swarm-ish",
                "mission": {
                    "objective": "Execute a mission with invalid mode."
                }
            })
            .to_string(),
        ))
        .expect("automation create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("automation create response");
    assert_eq!(create_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn automations_create_and_run_now_roundtrip() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/automations")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "automation_id": "auto-digest",
                "name": "Daily Digest Automation",
                "schedule": { "interval_seconds": { "seconds": 600 } },
                "mission": {
                    "objective": "Generate a daily digest with clear sources.",
                    "success_criteria": ["Contains source URLs", "Writes one artifact"]
                },
                "policy": {
                    "tool": {
                        "run_allowlist": ["read", "websearch", "webfetch", "write"],
                        "external_integrations_allowed": true
                    },
                    "approval": {
                        "requires_approval": true
                    }
                }
            })
            .to_string(),
        ))
        .expect("automation create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("automation create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/automations/auto-digest/run_now")
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .expect("automation run_now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("automation run_now response");
    assert_eq!(run_now_resp.status(), StatusCode::OK);
    let run_now_body = to_bytes(run_now_resp.into_body(), usize::MAX)
        .await
        .expect("automation run_now body");
    let run_now_payload: Value =
        serde_json::from_slice(&run_now_body).expect("automation run_now json");
    assert_eq!(
        run_now_payload
            .get("run")
            .and_then(|v| v.get("automation_id"))
            .and_then(|v| v.as_str()),
        Some("auto-digest")
    );
    assert_eq!(
        run_now_payload
            .get("run")
            .and_then(|v| v.get("mission_snapshot"))
            .and_then(|v| v.get("objective"))
            .and_then(|v| v.as_str()),
        Some("Generate a daily digest with clear sources.")
    );
    let run_id = run_now_payload
        .get("run")
        .and_then(|v| v.get("run_id"))
        .and_then(|v| v.as_str())
        .expect("automation run_id in run_now response")
        .to_string();

    let history_req = Request::builder()
        .method("GET")
        .uri("/automations/auto-digest/history?limit=5")
        .body(Body::empty())
        .expect("automation history request");
    let history_resp = app
        .clone()
        .oneshot(history_req)
        .await
        .expect("automation history response");
    assert_eq!(history_resp.status(), StatusCode::OK);
    let history_body = to_bytes(history_resp.into_body(), usize::MAX)
        .await
        .expect("automation history body");
    let history_payload: Value =
        serde_json::from_slice(&history_body).expect("automation history json");
    assert_eq!(
        history_payload.get("automationID").and_then(|v| v.as_str()),
        Some("auto-digest")
    );

    let add_artifact_req = Request::builder()
        .method("POST")
        .uri(format!("/automations/runs/{run_id}/artifacts"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "uri": "file://reports/daily-digest.md",
                "kind": "report",
                "label": "Daily Digest",
            })
            .to_string(),
        ))
        .expect("automation add artifact request");
    let add_artifact_resp = app
        .clone()
        .oneshot(add_artifact_req)
        .await
        .expect("automation add artifact response");
    assert_eq!(add_artifact_resp.status(), StatusCode::OK);

    let list_artifacts_req = Request::builder()
        .method("GET")
        .uri(format!("/automations/runs/{run_id}/artifacts"))
        .body(Body::empty())
        .expect("automation list artifacts request");
    let list_artifacts_resp = app
        .clone()
        .oneshot(list_artifacts_req)
        .await
        .expect("automation list artifacts response");
    assert_eq!(list_artifacts_resp.status(), StatusCode::OK);
    let list_artifacts_body = to_bytes(list_artifacts_resp.into_body(), usize::MAX)
        .await
        .expect("automation list artifacts body");
    let list_artifacts_payload: Value =
        serde_json::from_slice(&list_artifacts_body).expect("automation list artifacts json");
    assert_eq!(
        list_artifacts_payload
            .get("automationRunID")
            .and_then(|v| v.as_str()),
        Some(run_id.as_str())
    );
    assert!(list_artifacts_payload
        .get("count")
        .and_then(|v| v.as_u64())
        .is_some_and(|count| count >= 1));

    let patch_req = Request::builder()
        .method("PATCH")
        .uri("/automations/auto-digest")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "mode": "ORCHESTRATED"
            })
            .to_string(),
        ))
        .expect("automation patch request");
    let patch_resp = app
        .clone()
        .oneshot(patch_req)
        .await
        .expect("automation patch response");
    assert_eq!(patch_resp.status(), StatusCode::OK);
    let patch_body = to_bytes(patch_resp.into_body(), usize::MAX)
        .await
        .expect("automation patch body");
    let patch_payload: Value = serde_json::from_slice(&patch_body).expect("automation patch json");
    assert_eq!(
        patch_payload
            .get("automation")
            .and_then(|v| v.get("mode"))
            .and_then(|v| v.as_str()),
        Some("orchestrated")
    );
}

#[tokio::test]
async fn routines_run_now_blocks_external_side_effects_by_default() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-ext-blocked",
                "name": "External email sender",
                "schedule": { "interval_seconds": { "seconds": 300 } },
                "entrypoint": "connector.email.reply",
                "requires_approval": true,
                "external_integrations_allowed": false
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/routines/routine-ext-blocked/run_now")
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .expect("run_now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("run_now response");
    assert_eq!(run_now_resp.status(), StatusCode::FORBIDDEN);

    let history_req = Request::builder()
        .method("GET")
        .uri("/routines/routine-ext-blocked/history?limit=5")
        .body(Body::empty())
        .expect("history request");
    let history_resp = app
        .clone()
        .oneshot(history_req)
        .await
        .expect("history response");
    assert_eq!(history_resp.status(), StatusCode::OK);
    let history_body = to_bytes(history_resp.into_body(), usize::MAX)
        .await
        .expect("history body");
    let history_payload: Value = serde_json::from_slice(&history_body).expect("history json");
    assert_eq!(
        history_payload
            .get("events")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str()),
        Some("blocked_policy")
    );
}

#[tokio::test]
async fn routines_run_now_requires_approval_for_external_side_effects_when_enabled() {
    let state = test_state().await;
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-ext-approval",
                "name": "External draft workflow",
                "schedule": { "interval_seconds": { "seconds": 300 } },
                "entrypoint": "connector.email.reply",
                "requires_approval": true,
                "external_integrations_allowed": true
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/routines/routine-ext-approval/run_now")
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .expect("run_now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("run_now response");
    assert_eq!(run_now_resp.status(), StatusCode::OK);
    let run_now_body = to_bytes(run_now_resp.into_body(), usize::MAX)
        .await
        .expect("run_now body");
    let run_now_payload: Value = serde_json::from_slice(&run_now_body).expect("run_now json");
    assert_eq!(
        run_now_payload.get("status").and_then(|v| v.as_str()),
        Some("pending_approval")
    );

    let history_req = Request::builder()
        .method("GET")
        .uri("/routines/routine-ext-approval/history?limit=5")
        .body(Body::empty())
        .expect("history request");
    let history_resp = app
        .clone()
        .oneshot(history_req)
        .await
        .expect("history response");
    assert_eq!(history_resp.status(), StatusCode::OK);
    let history_body = to_bytes(history_resp.into_body(), usize::MAX)
        .await
        .expect("history body");
    let history_payload: Value = serde_json::from_slice(&history_body).expect("history json");
    assert_eq!(
        history_payload
            .get("events")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str()),
        Some("pending_approval")
    );
}

#[tokio::test]
async fn routine_fired_event_contract_snapshot() {
    let state = test_state().await;
    let mut rx = state.event_bus.subscribe();
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-fired-contract",
                "name": "Routine fired contract",
                "schedule": { "interval_seconds": { "seconds": 300 } },
                "entrypoint": "mission.default"
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/routines/routine-fired-contract/run_now")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "run_count": 2 }).to_string()))
        .expect("run now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("run now response");
    assert_eq!(run_now_resp.status(), StatusCode::OK);

    let event = next_event_of_type(&mut rx, "routine.fired").await;
    let mut properties = event
        .properties
        .as_object()
        .cloned()
        .expect("properties object");
    let fired_at_ms = properties
        .remove("firedAtMs")
        .and_then(|v| v.as_u64())
        .expect("firedAtMs");
    assert!(fired_at_ms > 0);

    let snapshot = json!({
        "type": event.event_type,
        "properties": properties,
    });
    let expected = json!({
        "type": "routine.fired",
        "properties": {
            "routineID": "routine-fired-contract",
            "runCount": 2,
            "triggerType": "manual"
        }
    });
    assert_eq!(snapshot, expected);
}

#[tokio::test]
async fn routine_approval_required_event_contract_snapshot() {
    let state = test_state().await;
    let mut rx = state.event_bus.subscribe();
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-approval-contract",
                "name": "Routine approval contract",
                "schedule": { "interval_seconds": { "seconds": 300 } },
                "entrypoint": "connector.email.reply",
                "requires_approval": true,
                "external_integrations_allowed": true
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/routines/routine-approval-contract/run_now")
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .expect("run now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("run now response");
    assert_eq!(run_now_resp.status(), StatusCode::OK);

    let event = next_event_of_type(&mut rx, "routine.approval_required").await;
    let snapshot = json!({
        "type": event.event_type,
        "properties": event.properties,
    });
    let expected = json!({
        "type": "routine.approval_required",
        "properties": {
            "routineID": "routine-approval-contract",
            "runCount": 1,
            "triggerType": "manual",
            "reason": "manual approval required before external side effects (manual)"
        }
    });
    assert_eq!(snapshot, expected);
}

#[tokio::test]
async fn routine_blocked_event_contract_snapshot() {
    let state = test_state().await;
    let mut rx = state.event_bus.subscribe();
    let app = app_router(state.clone());

    let create_req = Request::builder()
        .method("POST")
        .uri("/routines")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "routine_id": "routine-blocked-contract",
                "name": "Routine blocked contract",
                "schedule": { "interval_seconds": { "seconds": 300 } },
                "entrypoint": "connector.email.reply",
                "requires_approval": true,
                "external_integrations_allowed": false
            })
            .to_string(),
        ))
        .expect("create request");
    let create_resp = app
        .clone()
        .oneshot(create_req)
        .await
        .expect("create response");
    assert_eq!(create_resp.status(), StatusCode::OK);

    let run_now_req = Request::builder()
        .method("POST")
        .uri("/routines/routine-blocked-contract/run_now")
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .expect("run now request");
    let run_now_resp = app
        .clone()
        .oneshot(run_now_req)
        .await
        .expect("run now response");
    assert_eq!(run_now_resp.status(), StatusCode::FORBIDDEN);

    let event = next_event_of_type(&mut rx, "routine.blocked").await;
    let snapshot = json!({
        "type": event.event_type,
        "properties": event.properties,
    });
    let expected = json!({
        "type": "routine.blocked",
        "properties": {
            "routineID": "routine-blocked-contract",
            "runCount": 1,
            "triggerType": "manual",
            "reason": "external integrations are disabled by policy"
        }
    });
    assert_eq!(snapshot, expected);
}

#[tokio::test]
async fn routine_tool_policy_hook_denies_disallowed_tool_for_session_scope() {
    let state = test_state().await;
    let session = Session::new(Some("routine-session".to_string()), Some(".".to_string()));
    let session_id = session.id.clone();
    state
        .storage
        .save_session(session)
        .await
        .expect("save session");

    state
        .set_routine_session_policy(
            session_id.clone(),
            "run-routine-hook-1".to_string(),
            "routine-hook-1".to_string(),
            vec!["read".to_string(), "mcp.arcade.search".to_string()],
        )
        .await;

    let hook = crate::agent_teams::ServerToolPolicyHook::new(state.clone());
    let decision = hook
        .evaluate_tool(ToolPolicyContext {
            session_id,
            message_id: "msg-1".to_string(),
            tool: "bash".to_string(),
            args: json!({"command":"echo hi"}),
        })
        .await
        .expect("policy decision");

    assert!(!decision.allowed);
    assert!(decision
        .reason
        .as_deref()
        .unwrap_or_default()
        .contains("not allowed for routine"));
}
