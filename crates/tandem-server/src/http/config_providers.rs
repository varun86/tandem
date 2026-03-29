use crate::http::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;
use tandem_wire::{
    WireProviderCatalog, WireProviderEntry, WireProviderModel, WireProviderModelLimit,
};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub(super) struct AuthInput {
    #[serde(alias = "apiKey", alias = "api_key")]
    pub token: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct ApiTokenInput {
    #[serde(alias = "apiToken", alias = "api_token")]
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct LegacyProviderInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
    pub configured: bool,
}

fn is_internal_secret_id(raw: &str) -> bool {
    let normalized = raw.trim().to_ascii_lowercase();
    normalized.starts_with("mcp_header::") || normalized.starts_with("channel::")
}

pub(super) async fn get_config(State(state): State<AppState>) -> Json<Value> {
    let effective = normalize_effective_config_with_identity(redacted(
        state.config.get_effective_value().await,
    ));
    let layers = normalize_layers_with_identity(redacted(state.config.get_layers_value().await));
    Json(json!({
        "effective": effective,
        "layers": layers
    }))
}

pub(super) async fn patch_config(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Response {
    if contains_secret_config_fields(&input) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
            "error": "Secret provider keys are not accepted in config patches.",
            "code": "CONFIG_SECRET_REJECTED",
            "hint": "Use PUT /auth/{provider} or environment variables."
            })),
        )
            .into_response();
    }
    let effective = match state
        .config
        .patch_project(normalize_config_patch_input(input))
        .await
    {
        Ok(effective) => effective,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    state
        .providers
        .reload(state.config.get().await.into())
        .await;
    Json(json!({ "effective": normalize_effective_config_with_identity(redacted(effective)) }))
        .into_response()
}

pub(super) async fn global_config(State(state): State<AppState>) -> Json<Value> {
    let global =
        normalize_effective_config_with_identity(redacted(state.config.get_global_value().await));
    let effective = normalize_effective_config_with_identity(redacted(
        state.config.get_effective_value().await,
    ));
    Json(json!({
        "global": global,
        "effective": effective
    }))
}

pub(super) async fn global_config_patch(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Response {
    if contains_secret_config_fields(&input) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
            "error": "Secret provider keys are not accepted in global config patches.",
            "code": "CONFIG_SECRET_REJECTED",
            "hint": "Use PUT /auth/{provider} or environment variables."
            })),
        )
            .into_response();
    }
    let effective = match state
        .config
        .patch_global(normalize_config_patch_input(input))
        .await
    {
        Ok(effective) => effective,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    state
        .providers
        .reload(state.config.get().await.into())
        .await;
    Json(json!({ "effective": normalize_effective_config_with_identity(redacted(effective)) }))
        .into_response()
}

pub(super) async fn get_config_identity(State(state): State<AppState>) -> Json<Value> {
    let effective = normalize_effective_config_with_identity(redacted(
        state.config.get_effective_value().await,
    ));
    let identity = effective
        .get("identity")
        .cloned()
        .unwrap_or_else(identity_default_value);
    Json(json!({
        "identity": identity,
        "presets": personality_presets_catalog()
    }))
}

pub(super) async fn patch_config_identity(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Response {
    if contains_secret_config_fields(&input) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
            "error": "Secret provider keys are not accepted in config patches.",
            "code": "CONFIG_SECRET_REJECTED",
            "hint": "Use PUT /auth/{provider} or environment variables."
            })),
        )
            .into_response();
    }

    let patch = if input.get("identity").is_some() {
        normalize_config_patch_input(input)
    } else {
        normalize_config_patch_input(json!({ "identity": input }))
    };
    let effective = match state.config.patch_project(patch).await {
        Ok(effective) => effective,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    state
        .providers
        .reload(state.config.get().await.into())
        .await;
    Json(json!({
        "identity": normalize_effective_config_with_identity(redacted(effective))
            .get("identity")
            .cloned()
            .unwrap_or_else(identity_default_value),
        "presets": personality_presets_catalog()
    }))
    .into_response()
}

pub(super) async fn config_providers(State(state): State<AppState>) -> Json<Value> {
    let cfg = state.config.get_effective_value().await;
    let providers = redacted(cfg.get("providers").cloned().unwrap_or_else(|| json!({})));
    let default_provider = cfg.get("default_provider").cloned().unwrap_or(Value::Null);
    Json(json!({
        "providers": providers,
        "default": default_provider
    }))
}

pub(super) async fn list_providers(State(state): State<AppState>) -> Json<Value> {
    let cfg = state.config.get().await;
    let default = cfg.default_provider.unwrap_or_else(|| "local".to_string());
    let connected = state
        .providers
        .list()
        .await
        .into_iter()
        .map(|p| p.id)
        .collect::<Vec<_>>();
    let all = state.providers.list().await;
    let mut wire = WireProviderCatalog::from_providers(all, connected);
    let effective_cfg = state.config.get_effective_value().await;
    let persisted_auth = tandem_core::load_provider_auth();
    let runtime_auth = state.auth.read().await.clone();
    let config_model_provider_set = merge_provider_models_from_config(&mut wire, &effective_cfg)
        .into_iter()
        .collect::<std::collections::HashSet<_>>();

    for entry in &mut wire.all {
        entry.models.clear();
        entry.catalog_source = None;
        entry.catalog_status = None;
        entry.catalog_message = None;
    }
    wire.all.retain(|entry| entry.id != "local");
    ensure_known_provider_entries(&mut wire);

    for entry in &mut wire.all {
        match fetch_remote_provider_models(
            &entry.id,
            &effective_cfg,
            &runtime_auth,
            &persisted_auth,
        )
        .await
        {
            ProviderCatalogFetchResult::Remote { models } => {
                entry.models = models;
                entry.catalog_source = Some("remote".to_string());
                entry.catalog_status = Some("ok".to_string());
            }
            ProviderCatalogFetchResult::Unavailable { message } => {
                entry.catalog_source = Some("empty".to_string());
                entry.catalog_status = Some("unavailable".to_string());
                entry.catalog_message = Some(message);
            }
            ProviderCatalogFetchResult::Error { message } => {
                entry.catalog_source = Some("empty".to_string());
                entry.catalog_status = Some("error".to_string());
                entry.catalog_message = Some(message);
            }
        }
    }

    merge_provider_models_from_config(&mut wire, &effective_cfg);

    for entry in &mut wire.all {
        if config_model_provider_set.contains(&entry.id)
            && entry.catalog_source.as_deref() != Some("remote")
        {
            entry.catalog_source = Some("config".to_string());
            entry.catalog_status = Some("ok".to_string());
            entry.catalog_message = None;
            continue;
        }
        if entry.models.is_empty() && entry.catalog_status.is_none() {
            entry.catalog_source = Some("empty".to_string());
            entry.catalog_status = Some("unavailable".to_string());
            entry.catalog_message =
                Some("Live model discovery is unavailable. Enter a model ID manually.".to_string());
        } else if entry.catalog_source.is_none() {
            entry.catalog_source = Some("config".to_string());
            entry.catalog_status = Some("ok".to_string());
            entry.catalog_message = None;
        }
    }

    Json(json!({
        "all": wire.all,
        "connected": wire.connected,
        "default": default
    }))
}

pub(super) async fn list_providers_legacy(
    State(state): State<AppState>,
) -> Json<Vec<LegacyProviderInfo>> {
    let connected_ids = state
        .providers
        .list()
        .await
        .into_iter()
        .map(|p| p.id)
        .collect::<std::collections::HashSet<_>>();
    let providers = state
        .providers
        .list()
        .await
        .into_iter()
        .map(|p| LegacyProviderInfo {
            id: p.id.clone(),
            name: p.name,
            models: p.models.into_iter().map(|m| m.id).collect(),
            configured: connected_ids.contains(&p.id),
        })
        .collect::<Vec<_>>();
    Json(providers)
}

pub(super) async fn provider_auth(State(state): State<AppState>) -> Json<Value> {
    let cfg = state.config.get_effective_value().await;
    let providers_cfg = cfg
        .get("providers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let persisted = tandem_core::load_provider_auth();
    let persisted_ids = persisted
        .keys()
        .filter(|id| !is_internal_secret_id(id))
        .map(|id| id.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();
    let runtime_auth = state.auth.read().await.clone();
    let connected = state
        .providers
        .list()
        .await
        .into_iter()
        .map(|provider| provider.id.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();
    let mut known_ids = std::collections::HashSet::new();

    if let Some(default_id) = cfg.get("default_provider").and_then(Value::as_str) {
        let normalized = canonical_provider_id(default_id);
        if !normalized.is_empty() {
            known_ids.insert(normalized);
        }
    }
    known_ids.extend(providers_cfg.keys().map(|id| canonical_provider_id(id)));
    known_ids.extend(connected.iter().map(|id| canonical_provider_id(id)));
    known_ids.extend(runtime_auth.keys().map(|id| canonical_provider_id(id)));
    known_ids.extend(persisted_ids.iter().map(|id| canonical_provider_id(id)));

    let mut ids = known_ids.into_iter().collect::<Vec<_>>();
    ids.sort();

    let mut providers = serde_json::Map::new();
    for provider_id in ids {
        let has_runtime_key = provider_id_aliases(&provider_id).iter().any(|alias| {
            runtime_auth
                .get(*alias)
                .map(|token| !token.trim().is_empty())
                .unwrap_or(false)
        });
        let has_config_key = provider_config_value(&cfg, &provider_id)
            .and_then(Value::as_object)
            .map(|entry| {
                entry
                    .get("api_key")
                    .or_else(|| entry.get("apiKey"))
                    .and_then(Value::as_str)
                    .map(|token| !token.trim().is_empty())
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        let has_env_key = provider_has_env_secret(&provider_id);
        let has_persisted_key = provider_id_aliases(&provider_id)
            .iter()
            .any(|alias| persisted_ids.contains(*alias));
        let has_key = has_env_key || has_runtime_key || has_config_key || has_persisted_key;
        let source = if has_env_key {
            "env"
        } else if has_persisted_key {
            "persisted"
        } else if has_runtime_key || has_config_key {
            "runtime"
        } else {
            "none"
        };
        let configured = connected.contains(&provider_id);
        providers.insert(
            provider_id,
            json!({
                "has_key": has_key,
                "configured": configured,
                "connected": configured,
                "source": source,
            }),
        );
    }

    Json(json!({ "providers": providers }))
}

pub(super) async fn provider_oauth_authorize() -> Json<Value> {
    Json(json!({"authorizationUrl": null}))
}

pub(super) async fn provider_oauth_callback() -> Json<Value> {
    Json(json!({"ok": true}))
}

pub(super) async fn set_auth(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AuthInput>,
) -> Json<Value> {
    let normalized_id = id.trim().to_ascii_lowercase();
    if normalized_id.is_empty() {
        return Json(json!({"ok": false, "error": "provider id cannot be empty"}));
    }
    let token = input.token.unwrap_or_default().trim().to_string();
    if token.is_empty() {
        return Json(json!({"ok": false, "error": "token cannot be empty"}));
    }

    let backend = match tandem_core::set_provider_auth(&normalized_id, &token) {
        Ok(tandem_core::ProviderAuthBackend::Keychain) => "keychain",
        Ok(tandem_core::ProviderAuthBackend::File) => "file",
        Err(err) => {
            return Json(json!({
                "ok": false,
                "id": normalized_id,
                "error": format!("failed to persist provider auth: {err}")
            }));
        }
    };

    // Keep legacy in-memory auth map for compatibility while runtime config
    // becomes the canonical provider-key source.
    state
        .auth
        .write()
        .await
        .insert(normalized_id.clone(), token.clone());

    let patch = json!({
        "providers": {
            normalized_id.clone(): {
                "api_key": token
            }
        }
    });
    let ok = state.config.patch_runtime(patch).await.is_ok();
    if ok {
        state
            .providers
            .reload(state.config.get().await.into())
            .await;
    }
    Json(json!({"ok": ok, "id": normalized_id, "backend": backend}))
}

pub(super) async fn delete_auth(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    let normalized_id = id.trim().to_ascii_lowercase();
    if normalized_id.is_empty() {
        return Json(json!({"ok": false, "error": "provider id cannot be empty"}));
    }
    let removed = state.auth.write().await.remove(&normalized_id).is_some();
    let persisted_removed = tandem_core::delete_provider_auth(&normalized_id)
        .map(|ok| ok)
        .unwrap_or(false);
    let runtime_removed = state
        .config
        .delete_runtime_provider_key(&normalized_id)
        .await
        .is_ok();
    if runtime_removed {
        state
            .providers
            .reload(state.config.get().await.into())
            .await;
    }
    Json(json!({"ok": removed || runtime_removed || persisted_removed}))
}

pub(super) async fn set_api_token(
    State(state): State<AppState>,
    Json(input): Json<ApiTokenInput>,
) -> Json<Value> {
    let token = input.token.unwrap_or_default().trim().to_string();
    if token.is_empty() {
        return Json(json!({
            "ok": false,
            "error": "token cannot be empty"
        }));
    }
    state.set_api_token(Some(token)).await;
    Json(json!({"ok": true}))
}

pub(super) async fn clear_api_token(State(state): State<AppState>) -> Json<Value> {
    state.set_api_token(None).await;
    Json(json!({"ok": true}))
}

pub(super) async fn generate_api_token(State(state): State<AppState>) -> Json<Value> {
    let token = format!("tk_{}", Uuid::new_v4().simple());
    state.set_api_token(Some(token.clone())).await;
    Json(json!({
        "ok": true,
        "token": token
    }))
}

#[derive(Debug)]
enum ProviderCatalogFetchResult {
    Remote {
        models: HashMap<String, WireProviderModel>,
    },
    Unavailable {
        message: String,
    },
    Error {
        message: String,
    },
}

fn ensure_provider_entry<'a>(
    wire: &'a mut WireProviderCatalog,
    provider_id: &str,
    provider_name: Option<&str>,
) -> &'a mut WireProviderEntry {
    if let Some(idx) = wire.all.iter().position(|entry| entry.id == provider_id) {
        return &mut wire.all[idx];
    }

    wire.all.push(WireProviderEntry {
        id: provider_id.to_string(),
        name: provider_name.map(|s| s.to_string()),
        models: HashMap::new(),
        catalog_source: None,
        catalog_status: None,
        catalog_message: None,
    });
    wire.all.last_mut().expect("provider entry just inserted")
}

fn canonical_provider_id(provider_id: &str) -> String {
    match provider_id.trim().to_ascii_lowercase().as_str() {
        "llama.cpp" => "llama_cpp".to_string(),
        other => other.to_string(),
    }
}

fn provider_id_aliases(provider_id: &str) -> &'static [&'static str] {
    match canonical_provider_id(provider_id).as_str() {
        "llama_cpp" => &["llama_cpp", "llama.cpp"],
        _ => &[],
    }
}

fn known_provider_name(provider_id: &str) -> Option<&'static str> {
    match canonical_provider_id(provider_id).as_str() {
        "openrouter" => Some("OpenRouter"),
        "openai" => Some("OpenAI"),
        "anthropic" => Some("Anthropic"),
        "ollama" => Some("Ollama"),
        "llama_cpp" => Some("llama.cpp"),
        "groq" => Some("Groq"),
        "mistral" => Some("Mistral"),
        "together" => Some("Together"),
        "cohere" => Some("Cohere"),
        "azure" => Some("Azure OpenAI-Compatible"),
        "bedrock" => Some("Bedrock-Compatible"),
        "vertex" => Some("Vertex-Compatible"),
        "copilot" => Some("GitHub Copilot-Compatible"),
        _ => None,
    }
}

fn ensure_known_provider_entries(wire: &mut WireProviderCatalog) {
    for provider_id in [
        "openrouter",
        "openai",
        "anthropic",
        "ollama",
        "llama_cpp",
        "groq",
        "mistral",
        "together",
        "cohere",
        "azure",
        "bedrock",
        "vertex",
        "copilot",
    ] {
        ensure_provider_entry(wire, provider_id, known_provider_name(provider_id));
    }
}

fn merge_provider_model_map(
    wire: &mut WireProviderCatalog,
    provider_id: &str,
    provider_name: Option<&str>,
    models: HashMap<String, WireProviderModel>,
) {
    let entry = ensure_provider_entry(wire, provider_id, provider_name);
    for (model_id, model) in models {
        entry.models.insert(model_id, model);
    }
}

fn config_provider_root<'a>(cfg: &'a Value) -> Option<&'a serde_json::Map<String, Value>> {
    cfg.get("providers")
        .and_then(Value::as_object)
        .or_else(|| cfg.get("provider").and_then(Value::as_object))
}

fn provider_config_value<'a>(cfg: &'a Value, provider_id: &str) -> Option<&'a Value> {
    let root = config_provider_root(cfg)?;
    root.get(provider_id).or_else(|| {
        provider_id_aliases(provider_id)
            .iter()
            .find_map(|alias| root.get(*alias))
    })
}

fn merge_provider_models_from_config(wire: &mut WireProviderCatalog, cfg: &Value) -> Vec<String> {
    let Some(provider_root) = config_provider_root(cfg) else {
        return Vec::new();
    };

    let mut merged = Vec::new();
    for (provider_id, provider_value) in provider_root {
        let provider_id = canonical_provider_id(provider_id);
        let provider_name = provider_value
            .get("name")
            .and_then(|v| v.as_str())
            .or(known_provider_name(&provider_id))
            .or(Some(provider_id.as_str()));

        let mut model_map: HashMap<String, WireProviderModel> = HashMap::new();
        if let Some(models_obj) = provider_value.get("models").and_then(|v| v.as_object()) {
            for (model_id, model_value) in models_obj {
                let display_name = model_value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| Some(model_id.to_string()));
                let context = model_value
                    .get("limit")
                    .and_then(|v| v.get("context"))
                    .and_then(|v| v.as_u64())
                    .or_else(|| model_value.get("context_length").and_then(|v| v.as_u64()))
                    .map(|v| v as u32);

                model_map.insert(
                    model_id.to_string(),
                    WireProviderModel {
                        name: display_name,
                        limit: context.map(|ctx| WireProviderModelLimit { context: Some(ctx) }),
                    },
                );
            }
        }

        if !model_map.is_empty() {
            merge_provider_model_map(wire, &provider_id, provider_name, model_map);
            merged.push(provider_id);
        }
    }

    merged
}

fn provider_default_url(provider_id: &str) -> Option<&'static str> {
    match canonical_provider_id(provider_id).as_str() {
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "openai" => Some("https://api.openai.com/v1"),
        "llama_cpp" => Some("http://127.0.0.1:8080/v1"),
        "groq" => Some("https://api.groq.com/openai/v1"),
        "mistral" => Some("https://api.mistral.ai/v1"),
        "together" => Some("https://api.together.xyz/v1"),
        _ => None,
    }
}

fn remote_catalog_support_message(provider_id: &str) -> String {
    match provider_id {
        "azure" | "vertex" | "bedrock" | "copilot" | "ollama" => {
            "Live model discovery is unavailable for this provider. Enter a model ID manually."
                .to_string()
        }
        "anthropic" | "cohere" => {
            "This provider does not currently expose a reliable live model catalog here. Enter a model ID manually."
                .to_string()
        }
        _ => "Live model discovery is unavailable. Enter a model ID manually.".to_string(),
    }
}

fn provider_config_api_key(
    cfg: &Value,
    provider_id: &str,
    runtime_auth: &HashMap<String, String>,
    persisted_auth: &HashMap<String, String>,
) -> Option<String> {
    provider_config_value(cfg, provider_id)
        .and_then(|entry| entry.get("api_key").or_else(|| entry.get("apiKey")))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "x")
        .map(str::to_string)
        .or_else(|| {
            runtime_auth
                .get(&provider_id.to_ascii_lowercase())
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            persisted_auth
                .get(&provider_id.to_ascii_lowercase())
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            provider_env_candidates(provider_id)
                .into_iter()
                .find_map(|key| std::env::var(&key).ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn provider_base_url(cfg: &Value, provider_id: &str) -> Option<String> {
    provider_config_value(cfg, provider_id)
        .and_then(|entry| entry.get("url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| provider_default_url(provider_id).map(str::to_string))
}

fn normalize_openai_catalog_base(input: &str) -> String {
    let mut base = input.trim().trim_end_matches('/').to_string();
    for suffix in ["/chat/completions", "/completions", "/models"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped.trim_end_matches('/').to_string();
            break;
        }
    }
    while let Some(prefix) = base.strip_suffix("/v1") {
        if prefix.ends_with("/v1") {
            base = prefix.to_string();
            continue;
        }
        break;
    }
    if base.ends_with("/v1") {
        base
    } else {
        format!("{}/v1", base.trim_end_matches('/'))
    }
}

fn parse_openrouter_model_payload(body: &Value) -> Option<HashMap<String, WireProviderModel>> {
    let data = body.get("data").and_then(|v| v.as_array())?;
    let mut out = HashMap::new();
    for item in data {
        let Some(model_id) = item.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some(model_id.to_string()));
        let context = item
            .get("context_length")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                item.get("top_provider")
                    .and_then(|v| v.get("context_length"))
                    .and_then(|v| v.as_u64())
            })
            .map(|v| v as u32);

        out.insert(
            model_id.to_string(),
            WireProviderModel {
                name,
                limit: context.map(|ctx| WireProviderModelLimit { context: Some(ctx) }),
            },
        );
    }
    (!out.is_empty()).then_some(out)
}

fn parse_openai_compatible_model_payload(
    body: &Value,
) -> Option<HashMap<String, WireProviderModel>> {
    let data = body.get("data").and_then(|v| v.as_array())?;
    let mut out = HashMap::new();
    for item in data {
        let Some(model_id) = item.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some(model_id.to_string()));
        let context = item
            .get("context_window")
            .and_then(|v| v.as_u64())
            .or_else(|| item.get("context_length").and_then(|v| v.as_u64()))
            .map(|v| v as u32);
        out.insert(
            model_id.to_string(),
            WireProviderModel {
                name,
                limit: context.map(|ctx| WireProviderModelLimit { context: Some(ctx) }),
            },
        );
    }
    (!out.is_empty()).then_some(out)
}

async fn fetch_openrouter_models(
    cfg: &Value,
    runtime_auth: &HashMap<String, String>,
    persisted_auth: &HashMap<String, String>,
) -> Result<HashMap<String, WireProviderModel>, String> {
    let api_key = provider_config_api_key(cfg, "openrouter", runtime_auth, persisted_auth)
        .or_else(|| std::env::var("OPENCODE_OPENROUTER_API_KEY").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let Some(api_key) = api_key else {
        return Err(
            "OpenRouter requires an API key before live model discovery is available.".to_string(),
        );
    };

    let client = reqwest::Client::new();
    let mut req = client
        .get("https://openrouter.ai/api/v1/models")
        .timeout(Duration::from_secs(20));
    req = req.bearer_auth(api_key);
    let resp = req
        .send()
        .await
        .map_err(|err| format!("Failed to fetch OpenRouter models: {err}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "OpenRouter model catalog request failed with status {}",
            resp.status()
        ));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|err| format!("Failed to decode OpenRouter models: {err}"))?;
    parse_openrouter_model_payload(&body)
        .ok_or_else(|| "OpenRouter returned an empty or invalid model catalog.".to_string())
}

async fn fetch_openai_compatible_models(
    provider_id: &str,
    cfg: &Value,
    runtime_auth: &HashMap<String, String>,
    persisted_auth: &HashMap<String, String>,
) -> Result<HashMap<String, WireProviderModel>, String> {
    let api_key = provider_config_api_key(cfg, provider_id, runtime_auth, persisted_auth);
    if api_key.is_none() && !provider_supports_optional_auth(provider_id) {
        return Err(format!(
            "{} requires an API key before live model discovery is available.",
            known_provider_name(provider_id).unwrap_or(provider_id)
        ));
    }
    let Some(base_url) = provider_base_url(cfg, provider_id) else {
        return Err("No provider base URL is configured for live model discovery.".to_string());
    };
    let url = format!("{}/models", normalize_openai_catalog_base(&base_url));
    let client = reqwest::Client::new();
    let request = client.get(&url).timeout(Duration::from_secs(20));
    let request = if let Some(api_key) = api_key {
        request.bearer_auth(api_key)
    } else {
        request
    };
    let resp = request
        .send()
        .await
        .map_err(|err| format!("Failed to fetch model catalog from {provider_id}: {err}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "{} model catalog request failed with status {}",
            provider_id,
            resp.status()
        ));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|err| format!("Failed to decode model catalog from {provider_id}: {err}"))?;
    parse_openai_compatible_model_payload(&body).ok_or_else(|| {
        format!("{provider_id} returned an empty or invalid OpenAI-compatible model catalog.")
    })
}

fn normalize_cohere_catalog_base(input: &str) -> String {
    let mut base = input.trim().trim_end_matches('/').to_string();
    for suffix in ["/v1", "/v2", "/models"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped.trim_end_matches('/').to_string();
            break;
        }
    }
    format!("{}/v1", base.trim_end_matches('/'))
}

fn parse_anthropic_model_payload(body: &Value) -> Option<HashMap<String, WireProviderModel>> {
    let data = body.get("data").and_then(|v| v.as_array())?;
    let mut out = HashMap::new();
    for item in data {
        let Some(model_id) = item.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = item
            .get("display_name")
            .or_else(|| item.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some(model_id.to_string()));
        out.insert(
            model_id.to_string(),
            WireProviderModel { name, limit: None },
        );
    }
    (!out.is_empty()).then_some(out)
}

async fn fetch_anthropic_models(
    cfg: &Value,
    runtime_auth: &HashMap<String, String>,
    persisted_auth: &HashMap<String, String>,
) -> Result<HashMap<String, WireProviderModel>, String> {
    let Some(api_key) = provider_config_api_key(cfg, "anthropic", runtime_auth, persisted_auth)
    else {
        return Err(
            "Anthropic requires an API key before live model discovery is available.".to_string(),
        );
    };
    let resp = reqwest::Client::new()
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|err| format!("Failed to fetch Anthropic models: {err}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Anthropic model catalog request failed with status {}",
            resp.status()
        ));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|err| format!("Failed to decode Anthropic models: {err}"))?;
    parse_anthropic_model_payload(&body)
        .ok_or_else(|| "Anthropic returned an empty or invalid model catalog.".to_string())
}

fn parse_cohere_model_payload(body: &Value) -> Option<HashMap<String, WireProviderModel>> {
    let data = body
        .get("models")
        .and_then(|v| v.as_array())
        .or_else(|| body.get("data").and_then(|v| v.as_array()))?;
    let mut out = HashMap::new();
    for item in data {
        let Some(model_id) = item
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| item.get("id").and_then(|v| v.as_str()))
        else {
            continue;
        };
        let context = item
            .get("context_length")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        out.insert(
            model_id.to_string(),
            WireProviderModel {
                name: Some(model_id.to_string()),
                limit: context.map(|ctx| WireProviderModelLimit { context: Some(ctx) }),
            },
        );
    }
    (!out.is_empty()).then_some(out)
}

async fn fetch_cohere_models(
    cfg: &Value,
    runtime_auth: &HashMap<String, String>,
    persisted_auth: &HashMap<String, String>,
) -> Result<HashMap<String, WireProviderModel>, String> {
    let Some(api_key) = provider_config_api_key(cfg, "cohere", runtime_auth, persisted_auth) else {
        return Err(
            "Cohere requires an API key before live model discovery is available.".to_string(),
        );
    };
    let base_url = provider_config_value(cfg, "cohere")
        .and_then(|entry| entry.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("https://api.cohere.com/v2");
    let url = format!("{}/models", normalize_cohere_catalog_base(base_url));
    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(api_key)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|err| format!("Failed to fetch Cohere models: {err}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Cohere model catalog request failed with status {}",
            resp.status()
        ));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|err| format!("Failed to decode Cohere models: {err}"))?;
    parse_cohere_model_payload(&body)
        .ok_or_else(|| "Cohere returned an empty or invalid model catalog.".to_string())
}

async fn fetch_remote_provider_models(
    provider_id: &str,
    cfg: &Value,
    runtime_auth: &HashMap<String, String>,
    persisted_auth: &HashMap<String, String>,
) -> ProviderCatalogFetchResult {
    match provider_id {
        "openrouter" => match fetch_openrouter_models(cfg, runtime_auth, persisted_auth).await {
            Ok(models) => ProviderCatalogFetchResult::Remote { models },
            Err(message) => {
                if message.contains("requires an API key") {
                    ProviderCatalogFetchResult::Unavailable { message }
                } else {
                    tracing::debug!("openrouter catalog discovery failed: {message}");
                    ProviderCatalogFetchResult::Error { message }
                }
            }
        },
        "openai" | "llama_cpp" | "groq" | "mistral" | "together" => {
            match fetch_openai_compatible_models(provider_id, cfg, runtime_auth, persisted_auth)
                .await
            {
                Ok(models) => ProviderCatalogFetchResult::Remote { models },
                Err(message) => {
                    if message.contains("requires an API key") {
                        ProviderCatalogFetchResult::Unavailable { message }
                    } else {
                        tracing::warn!("{provider_id} catalog discovery failed: {message}");
                        ProviderCatalogFetchResult::Error { message }
                    }
                }
            }
        }
        "anthropic" => match fetch_anthropic_models(cfg, runtime_auth, persisted_auth).await {
            Ok(models) => ProviderCatalogFetchResult::Remote { models },
            Err(message) => {
                if message.contains("requires an API key") {
                    ProviderCatalogFetchResult::Unavailable { message }
                } else {
                    tracing::warn!("anthropic catalog discovery failed: {message}");
                    ProviderCatalogFetchResult::Error { message }
                }
            }
        },
        "cohere" => match fetch_cohere_models(cfg, runtime_auth, persisted_auth).await {
            Ok(models) => ProviderCatalogFetchResult::Remote { models },
            Err(message) => {
                if message.contains("requires an API key") {
                    ProviderCatalogFetchResult::Unavailable { message }
                } else {
                    tracing::warn!("cohere catalog discovery failed: {message}");
                    ProviderCatalogFetchResult::Error { message }
                }
            }
        },
        "azure" | "vertex" | "bedrock" | "copilot" | "ollama" => {
            ProviderCatalogFetchResult::Unavailable {
                message: remote_catalog_support_message(provider_id),
            }
        }
        _ => ProviderCatalogFetchResult::Unavailable {
            message: "Live model discovery is unavailable. Enter a model ID manually.".to_string(),
        },
    }
}

fn provider_supports_optional_auth(provider_id: &str) -> bool {
    matches!(canonical_provider_id(provider_id).as_str(), "llama_cpp")
}

fn provider_env_candidates(provider_id: &str) -> Vec<String> {
    let normalized = provider_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_ascii_uppercase();

    let mut out = vec![format!("{}_API_KEY", normalized)];
    match provider_id.to_ascii_lowercase().as_str() {
        "openai" => out.push("OPENAI_API_KEY".to_string()),
        "openrouter" => out.push("OPENROUTER_API_KEY".to_string()),
        "anthropic" => out.push("ANTHROPIC_API_KEY".to_string()),
        "groq" => out.push("GROQ_API_KEY".to_string()),
        "mistral" => out.push("MISTRAL_API_KEY".to_string()),
        "together" => out.push("TOGETHER_API_KEY".to_string()),
        "azure" => out.push("AZURE_OPENAI_API_KEY".to_string()),
        "vertex" => out.push("VERTEX_API_KEY".to_string()),
        "bedrock" => out.push("BEDROCK_API_KEY".to_string()),
        "copilot" => out.push("GITHUB_TOKEN".to_string()),
        "cohere" => out.push("COHERE_API_KEY".to_string()),
        _ => {}
    }
    out.sort();
    out.dedup();
    out
}

fn provider_has_env_secret(provider_id: &str) -> bool {
    provider_env_candidates(provider_id).into_iter().any(|key| {
        std::env::var(&key)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    })
}

fn redact_secret_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, field) in map.iter_mut() {
                if key.eq_ignore_ascii_case("api_key")
                    || key.eq_ignore_ascii_case("apikey")
                    || key.eq_ignore_ascii_case("bot_token")
                    || key.eq_ignore_ascii_case("botToken")
                {
                    *field = Value::String("[REDACTED]".to_string());
                } else {
                    redact_secret_fields(field);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_secret_fields(item);
            }
        }
        _ => {}
    }
}

fn redacted(mut value: Value) -> Value {
    redact_secret_fields(&mut value);
    value
}

fn contains_secret_config_fields(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, field)| {
            key.eq_ignore_ascii_case("api_key")
                || key.eq_ignore_ascii_case("apikey")
                || key.eq_ignore_ascii_case("bot_token")
                || key.eq_ignore_ascii_case("botToken")
                || contains_secret_config_fields(field)
        }),
        Value::Array(items) => items.iter().any(contains_secret_config_fields),
        _ => false,
    }
}

fn merge_json(base: &mut Value, overlay: &Value) {
    if overlay.is_null() {
        return;
    }
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                if value.is_null() {
                    continue;
                }
                match base_map.get_mut(key) {
                    Some(existing) => merge_json(existing, value),
                    None => {
                        base_map.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base_value, overlay_value) => {
            *base_value = overlay_value.clone();
        }
    }
}

fn identity_default_value() -> Value {
    json!({
        "bot": {
            "canonical_name": "Tandem",
            "avatar_url": null,
            "aliases": {
                "desktop": "Tandem",
                "tui": "Tandem TUI",
                "portal": "Tandem Portal",
                "control_panel": "Tandem Control Panel",
                "channels": "Tandem",
                "protocol": "Tandem",
                "cli": "Tandem"
            }
        },
        "personality": {
            "default": {
                "preset": "balanced",
                "custom_instructions": null
            },
            "per_agent": {}
        }
    })
}

fn personality_presets_catalog() -> Value {
    json!([
        {
            "id": "balanced",
            "label": "Balanced",
            "description": "Pragmatic, direct, and neutral tone."
        },
        {
            "id": "concise",
            "label": "Concise",
            "description": "Short, high-signal responses focused on outcomes."
        },
        {
            "id": "friendly",
            "label": "Friendly",
            "description": "Warm, approachable style while staying practical."
        },
        {
            "id": "mentor",
            "label": "Mentor",
            "description": "Guides with context and explicit reasoning."
        },
        {
            "id": "critical",
            "label": "Critical",
            "description": "Skeptical, risk-first framing with clear tradeoffs."
        }
    ])
}

fn normalize_effective_config_with_identity(mut value: Value) -> Value {
    let legacy_bot_name = value
        .get("bot_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string);
    let legacy_persona = value
        .get("persona")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string);

    if !value.is_object() {
        return value;
    }

    let root = value.as_object_mut().expect("checked object");
    if !root.contains_key("identity") || !root.get("identity").is_some_and(Value::is_object) {
        root.insert("identity".to_string(), identity_default_value());
    }
    if let Some(identity) = root.get_mut("identity") {
        let mut normalized = identity_default_value();
        merge_json(&mut normalized, identity);
        *identity = normalized;
    }

    if let Some(legacy_bot_name) = legacy_bot_name {
        let canonical_name = root
            .get("identity")
            .and_then(Value::as_object)
            .and_then(|identity| identity.get("bot"))
            .and_then(Value::as_object)
            .and_then(|bot| bot.get("canonical_name"))
            .and_then(Value::as_str);
        let should_fill = canonical_name
            .map(|value| value.trim().is_empty())
            .unwrap_or(true);
        if should_fill {
            root["identity"]["bot"]["canonical_name"] = Value::String(legacy_bot_name);
        }
    }

    if let Some(legacy_persona) = legacy_persona {
        let has_custom = root
            .get("identity")
            .and_then(|identity| identity.get("personality"))
            .and_then(|personality| personality.get("default"))
            .and_then(|default| default.get("custom_instructions"))
            .and_then(Value::as_str)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if !has_custom {
            root["identity"]["personality"]["default"]["custom_instructions"] =
                Value::String(legacy_persona);
        }
    }

    let canonical_name = root
        .get("identity")
        .and_then(|identity| identity.get("bot"))
        .and_then(|bot| bot.get("canonical_name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("Tandem")
        .to_string();
    if let Some(aliases) = root
        .get_mut("identity")
        .and_then(Value::as_object_mut)
        .and_then(|identity| identity.get_mut("bot"))
        .and_then(Value::as_object_mut)
        .and_then(|bot| bot.get_mut("aliases"))
        .and_then(Value::as_object_mut)
    {
        for alias in [
            "desktop",
            "portal",
            "channels",
            "protocol",
            "cli",
            "control_panel",
        ] {
            let needs_fill = aliases
                .get(alias)
                .and_then(Value::as_str)
                .map(|v| v.trim().is_empty())
                .unwrap_or(true);
            if needs_fill {
                aliases.insert(alias.to_string(), Value::String(canonical_name.clone()));
            }
        }
        let tui_needs_fill = aliases
            .get("tui")
            .and_then(Value::as_str)
            .map(|v| v.trim().is_empty())
            .unwrap_or(true);
        if tui_needs_fill {
            aliases.insert(
                "tui".to_string(),
                Value::String(format!("{canonical_name} TUI")),
            );
        }
    }

    let default_preset_empty = root
        .get("identity")
        .and_then(|identity| identity.get("personality"))
        .and_then(|personality| personality.get("default"))
        .and_then(|default| default.get("preset"))
        .and_then(Value::as_str)
        .map(|v| v.trim().is_empty())
        .unwrap_or(true);
    if default_preset_empty {
        root["identity"]["personality"]["default"]["preset"] =
            Value::String("balanced".to_string());
    }

    root.insert("bot_name".to_string(), Value::String(canonical_name));
    let compat_persona = root
        .get("identity")
        .and_then(|identity| identity.get("personality"))
        .and_then(|personality| personality.get("default"))
        .and_then(|default| default.get("custom_instructions"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
    root.insert(
        "persona".to_string(),
        compat_persona.map_or(Value::Null, Value::String),
    );

    value
}

fn normalize_layers_with_identity(mut value: Value) -> Value {
    let Some(root) = value.as_object_mut() else {
        return value;
    };
    for layer in ["global", "project", "managed", "env", "runtime", "cli"] {
        if let Some(entry) = root.get_mut(layer) {
            let normalized = normalize_effective_config_with_identity(entry.clone());
            *entry = normalized;
        }
    }
    value
}

fn normalize_config_patch_input(mut input: Value) -> Value {
    let Some(root) = input.as_object_mut() else {
        return input;
    };
    let legacy_bot_name = root
        .get("bot_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string);
    let legacy_persona = root
        .get("persona")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string);

    if let Some(legacy_bot_name) = legacy_bot_name {
        root.entry("identity".to_string())
            .or_insert_with(|| json!({}));
        root["identity"]["bot"]["canonical_name"] = Value::String(legacy_bot_name);
    }
    if let Some(legacy_persona) = legacy_persona {
        root.entry("identity".to_string())
            .or_insert_with(|| json!({}));
        root["identity"]["personality"]["default"]["custom_instructions"] =
            Value::String(legacy_persona);
        if root["identity"]["personality"]["default"]
            .get("preset")
            .and_then(Value::as_str)
            .map(|v| v.trim().is_empty())
            .unwrap_or(true)
        {
            root["identity"]["personality"]["default"]["preset"] =
                Value::String("balanced".to_string());
        }
    }

    root.remove("bot_name");
    root.remove("persona");
    normalize_identity_avatar_patch(root);
    input
}

fn normalize_identity_avatar_patch(root: &mut serde_json::Map<String, Value>) {
    let avatar_slot = root
        .get_mut("identity")
        .and_then(Value::as_object_mut)
        .and_then(|identity| identity.get_mut("bot"))
        .and_then(Value::as_object_mut)
        .and_then(|bot| bot.get_mut("avatar_url"));

    let Some(slot) = avatar_slot else {
        return;
    };
    let Some(raw) = slot.as_str() else {
        return;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        *slot = Value::Null;
        return;
    }
    if let Some(normalized) = normalize_avatar_data_url(trimmed) {
        *slot = Value::String(normalized);
    }
}

fn normalize_avatar_data_url(input: &str) -> Option<String> {
    if !input.starts_with("data:image/") {
        return Some(input.to_string());
    }

    let (meta, payload) = input.split_once(',')?;
    if !meta.contains(";base64") {
        return None;
    }
    // Safety guard against very large inline payloads.
    if payload.len() > 24 * 1024 * 1024 {
        return None;
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.as_bytes())
        .ok()?;
    if bytes.len() > 16 * 1024 * 1024 {
        return None;
    }

    let mut image = image::load_from_memory(&bytes).ok()?;
    if image.width() > 512 || image.height() > 512 {
        image = image.thumbnail(512, 512);
    }

    // Re-encode to PNG for consistent, browser-safe storage.
    let mut out = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(out);
    Some(format!("data:image/png;base64,{encoded}"))
}
