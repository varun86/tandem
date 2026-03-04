use super::*;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub(super) struct PackSelectorPath {
    pub selector: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct PackDetectInput {
    pub path: String,
    #[serde(default)]
    pub attachment_id: Option<String>,
    #[serde(default)]
    pub connector: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub sender_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PackInstallFromAttachmentInput {
    pub attachment_id: String,
    pub path: String,
    #[serde(default)]
    pub connector: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub sender_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct PackUpdateApplyInput {
    #[serde(default)]
    pub target_version: Option<String>,
}

pub(super) async fn packs_list(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let packs = state.pack_manager.list().await.map_err(|err| {
        tracing::warn!("packs list failed: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "packs": packs })))
}

pub(super) async fn packs_get(
    State(state): State<AppState>,
    Path(PackSelectorPath { selector }): Path<PackSelectorPath>,
) -> Result<Json<Value>, StatusCode> {
    let inspection = state.pack_manager.inspect(&selector).await.map_err(|err| {
        if err.to_string().contains("not found") {
            StatusCode::NOT_FOUND
        } else {
            tracing::warn!("pack inspect failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;
    Ok(Json(json!({
        "pack": inspection,
    })))
}

pub(super) async fn packs_install(
    State(state): State<AppState>,
    Json(input): Json<PackInstallRequest>,
) -> Result<Json<Value>, StatusCode> {
    state.event_bus.publish(EngineEvent::new(
        "pack.install.started",
        json!({
            "source": input.source,
            "path": input.path,
            "url": input.url,
        }),
    ));
    let result = state.pack_manager.install(input).await;
    match result {
        Ok(installed) => {
            state.event_bus.publish(EngineEvent::new(
                "pack.install.succeeded",
                json!({
                    "pack_id": installed.pack_id,
                    "name": installed.name,
                    "version": installed.version,
                }),
            ));
            state.event_bus.publish(EngineEvent::new(
                "registry.updated",
                json!({ "entity": "packs" }),
            ));
            Ok(Json(json!({ "installed": installed })))
        }
        Err(err) => {
            state.event_bus.publish(EngineEvent::new(
                "pack.install.failed",
                json!({
                    "error": err.to_string(),
                    "code": "pack_install_failed",
                }),
            ));
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

pub(super) async fn packs_install_from_attachment(
    State(state): State<AppState>,
    Json(input): Json<PackInstallFromAttachmentInput>,
) -> Result<Json<Value>, StatusCode> {
    let source = json!({
        "kind": "attachment",
        "attachment_id": input.attachment_id,
        "connector": input.connector,
        "channel_id": input.channel_id,
        "sender_id": input.sender_id,
    });
    packs_install(
        State(state),
        Json(PackInstallRequest {
            path: Some(input.path),
            url: None,
            source,
        }),
    )
    .await
}

pub(super) async fn packs_uninstall(
    State(state): State<AppState>,
    Json(input): Json<PackUninstallRequest>,
) -> Result<Json<Value>, StatusCode> {
    let removed = state.pack_manager.uninstall(input).await.map_err(|err| {
        if err.to_string().contains("not found") {
            StatusCode::NOT_FOUND
        } else {
            tracing::warn!("pack uninstall failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;
    state.event_bus.publish(EngineEvent::new(
        "registry.updated",
        json!({ "entity": "packs" }),
    ));
    Ok(Json(json!({ "removed": removed })))
}

pub(super) async fn packs_export(
    State(state): State<AppState>,
    Json(input): Json<PackExportRequest>,
) -> Result<Json<Value>, StatusCode> {
    let exported = state.pack_manager.export(input).await.map_err(|err| {
        tracing::warn!("pack export failed: {}", err);
        StatusCode::BAD_REQUEST
    })?;
    Ok(Json(json!({ "exported": exported })))
}

pub(super) async fn packs_updates_get(
    State(state): State<AppState>,
    Path(PackSelectorPath { selector }): Path<PackSelectorPath>,
) -> Result<Json<Value>, StatusCode> {
    let inspection = state.pack_manager.inspect(&selector).await.map_err(|err| {
        if err.to_string().contains("not found") {
            StatusCode::NOT_FOUND
        } else {
            tracing::warn!("pack updates check failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;
    Ok(Json(json!({
        "pack_id": inspection.installed.pack_id,
        "name": inspection.installed.name,
        "current_version": inspection.installed.version,
        "updates": [],
        "permissions_diff": {
            "added_required_capabilities": [],
            "removed_required_capabilities": [],
            "added_provider_specific_dependencies": [],
            "removed_provider_specific_dependencies": [],
            "routine_scope_changed": false
        },
        "reapproval_required": false
    })))
}

pub(super) async fn packs_update_post(
    State(state): State<AppState>,
    Path(PackSelectorPath { selector }): Path<PackSelectorPath>,
    Json(input): Json<PackUpdateApplyInput>,
) -> Result<Json<Value>, StatusCode> {
    let inspection = state.pack_manager.inspect(&selector).await.map_err(|err| {
        if err.to_string().contains("not found") {
            StatusCode::NOT_FOUND
        } else {
            tracing::warn!("pack update apply failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;
    state.event_bus.publish(EngineEvent::new(
        "pack.update.not_available",
        json!({
            "pack_id": inspection.installed.pack_id,
            "name": inspection.installed.name,
            "current_version": inspection.installed.version,
            "target_version": input.target_version,
            "reason": "updates_not_implemented",
        }),
    ));
    Ok(Json(json!({
        "updated": false,
        "pack_id": inspection.installed.pack_id,
        "name": inspection.installed.name,
        "current_version": inspection.installed.version,
        "target_version": input.target_version,
        "reason": "updates_not_implemented",
        "permissions_diff": {
            "added_required_capabilities": [],
            "removed_required_capabilities": [],
            "added_provider_specific_dependencies": [],
            "removed_provider_specific_dependencies": [],
            "routine_scope_changed": false
        },
        "reapproval_required": false
    })))
}

pub(super) async fn packs_detect(
    State(state): State<AppState>,
    Json(input): Json<PackDetectInput>,
) -> Result<Json<Value>, StatusCode> {
    let path = PathBuf::from(&input.path);
    let is_pack = state.pack_manager.detect(&path).await.map_err(|err| {
        tracing::warn!("pack detect failed: {}", err);
        StatusCode::BAD_REQUEST
    })?;
    if is_pack {
        state.event_bus.publish(EngineEvent::new(
            "pack.detected",
            json!({
                "path": input.path,
                "attachment_id": input.attachment_id,
                "connector": input.connector,
                "channel_id": input.channel_id,
                "sender_id": input.sender_id,
                "marker": "tandempack.yaml",
            }),
        ));
    }
    Ok(Json(json!({
        "is_pack": is_pack,
        "marker": "tandempack.yaml",
    })))
}
