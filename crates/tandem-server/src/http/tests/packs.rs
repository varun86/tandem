use super::*;

#[tokio::test]
async fn packs_detect_requires_root_marker() {
    let state = test_state().await;
    let mut rx = state.event_bus.subscribe();
    let root = std::env::temp_dir().join(format!("tandem-pack-detect-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let plain_zip = root.join("plain.zip");
    write_pack_zip(
        &plain_zip,
        "name: detect-test\nversion: 1.0.0\ntype: skill\npack_id: detect-test\n",
    );
    let app = app_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/packs/detect")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "path": plain_zip.to_string_lossy()
            })
            .to_string(),
        ))
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
    let payload: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.get("is_pack").and_then(|v| v.as_bool()), Some(true));
    let event = next_event_of_type(&mut rx, "pack.detected").await;
    assert_eq!(
        event.properties.get("marker").and_then(|v| v.as_str()),
        Some("tandempack.yaml")
    );
    assert_eq!(
        event.properties.get("path").and_then(|v| v.as_str()),
        Some(plain_zip.to_string_lossy().as_ref())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn packs_detect_returns_false_without_marker() {
    let state = test_state().await;
    let root = std::env::temp_dir().join(format!("tandem-pack-detect-none-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let plain_zip = root.join("plain.zip");
    write_plain_zip_without_marker(&plain_zip);
    let app = app_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/packs/detect")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "path": plain_zip.to_string_lossy()
            })
            .to_string(),
        ))
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
    let payload: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(
        payload.get("is_pack").and_then(|v| v.as_bool()),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn packs_install_list_and_uninstall_roundtrip() {
    let state = test_state().await;
    let mut rx = state.event_bus.subscribe();
    let root = std::env::temp_dir().join(format!("tandem-pack-install-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let pack_zip = root.join("pack.zip");
    write_pack_zip(
        &pack_zip,
        "name: roundtrip-pack\nversion: 1.2.3\ntype: workflow\npack_id: roundtrip-pack\n",
    );
    let app = app_router(state.clone());
    let install_req = Request::builder()
        .method("POST")
        .uri("/packs/install")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "path": pack_zip.to_string_lossy(),
                "source": {"kind":"test"}
            })
            .to_string(),
        ))
        .expect("request");
    let install_resp = app.clone().oneshot(install_req).await.expect("response");
    assert_eq!(install_resp.status(), StatusCode::OK);
    let install_body = to_bytes(install_resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let install_payload: Value = serde_json::from_slice(&install_body).expect("json");
    let install_path = install_payload
        .get("installed")
        .and_then(|v| v.get("install_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(install_path.ends_with("/roundtrip-pack/1.2.3"));
    let current_pointer = std::path::PathBuf::from(install_path)
        .parent()
        .expect("parent")
        .join("current");
    let current_version = std::fs::read_to_string(&current_pointer).expect("read current");
    assert_eq!(current_version.trim(), "1.2.3");
    let started = next_event_of_type(&mut rx, "pack.install.started").await;
    assert_eq!(
        started.properties.get("path").and_then(|v| v.as_str()),
        Some(pack_zip.to_string_lossy().as_ref())
    );
    let succeeded = next_event_of_type(&mut rx, "pack.install.succeeded").await;
    assert_eq!(
        succeeded.properties.get("pack_id").and_then(|v| v.as_str()),
        Some("roundtrip-pack")
    );
    let registry = next_event_of_type(&mut rx, "registry.updated").await;
    assert_eq!(
        registry.properties.get("entity").and_then(|v| v.as_str()),
        Some("packs")
    );

    let list_req = Request::builder()
        .method("GET")
        .uri("/packs")
        .body(Body::empty())
        .expect("request");
    let list_resp = app.clone().oneshot(list_req).await.expect("response");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = to_bytes(list_resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let list_payload: Value = serde_json::from_slice(&list_body).expect("json");
    let packs = list_payload
        .get("packs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(packs.iter().any(|p| {
        p.get("pack_id").and_then(|v| v.as_str()) == Some("roundtrip-pack")
            && p.get("version").and_then(|v| v.as_str()) == Some("1.2.3")
    }));

    let uninstall_req = Request::builder()
        .method("POST")
        .uri("/packs/uninstall")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "pack_id": "roundtrip-pack",
                "version": "1.2.3"
            })
            .to_string(),
        ))
        .expect("request");
    let uninstall_resp = app.clone().oneshot(uninstall_req).await.expect("response");
    assert_eq!(uninstall_resp.status(), StatusCode::OK);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn packs_updates_endpoints_return_stub_payload() {
    let state = test_state().await;
    let root = std::env::temp_dir().join(format!("tandem-pack-updates-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let pack_zip = root.join("pack.zip");
    write_pack_zip(
        &pack_zip,
        "name: update-pack\nversion: 1.0.0\ntype: workflow\npack_id: update-pack\n",
    );
    let app = app_router(state.clone());
    let install_req = Request::builder()
        .method("POST")
        .uri("/packs/install")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "path": pack_zip.to_string_lossy(),
                "source": {"kind":"test"}
            })
            .to_string(),
        ))
        .expect("request");
    let install_resp = app.clone().oneshot(install_req).await.expect("response");
    assert_eq!(install_resp.status(), StatusCode::OK);

    let updates_req = Request::builder()
        .method("GET")
        .uri("/packs/update-pack/updates")
        .body(Body::empty())
        .expect("request");
    let updates_resp = app.clone().oneshot(updates_req).await.expect("response");
    assert_eq!(updates_resp.status(), StatusCode::OK);
    let updates_body = to_bytes(updates_resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let updates_payload: Value = serde_json::from_slice(&updates_body).expect("json");
    assert_eq!(
        updates_payload
            .get("current_version")
            .and_then(|v| v.as_str()),
        Some("1.0.0")
    );
    assert_eq!(
        updates_payload
            .get("reapproval_required")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert!(updates_payload
        .get("permissions_diff")
        .and_then(|v| v.as_object())
        .is_some());

    let apply_req = Request::builder()
        .method("POST")
        .uri("/packs/update-pack/update")
        .header("content-type", "application/json")
        .body(Body::from(json!({"target_version":"1.1.0"}).to_string()))
        .expect("request");
    let apply_resp = app.clone().oneshot(apply_req).await.expect("response");
    assert_eq!(apply_resp.status(), StatusCode::OK);
    let apply_body = to_bytes(apply_resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let apply_payload: Value = serde_json::from_slice(&apply_body).expect("json");
    assert_eq!(
        apply_payload.get("reason").and_then(|v| v.as_str()),
        Some("updates_not_implemented")
    );
    assert_eq!(
        apply_payload
            .get("reapproval_required")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(root);
}
