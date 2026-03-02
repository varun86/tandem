use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::resolve_shared_paths;

const PROVIDER_AUTH_SERVICE: &str = "ai.frumu.tandem";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAuthBackend {
    Keychain,
    File,
}

fn provider_auth_security_dir() -> PathBuf {
    if let Ok(paths) = resolve_shared_paths() {
        return paths.canonical_root.join("security");
    }
    if let Some(data_dir) = dirs::data_dir() {
        return data_dir.join("tandem").join("security");
    }
    dirs::home_dir()
        .map(|home| home.join(".tandem").join("security"))
        .unwrap_or_else(|| PathBuf::from("security"))
}

fn provider_auth_index_path() -> PathBuf {
    provider_auth_security_dir().join("provider_auth_index.json")
}

fn provider_auth_fallback_path() -> PathBuf {
    provider_auth_security_dir().join("provider_auth_fallback.json")
}

fn normalize_provider_id(id: &str) -> String {
    id.trim().to_ascii_lowercase()
}

fn provider_auth_account(provider_id: &str) -> String {
    format!("provider_api_key::{}", normalize_provider_id(provider_id))
}

fn keyring_entry(provider_id: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(PROVIDER_AUTH_SERVICE, &provider_auth_account(provider_id)).ok()
}

fn write_secure_json(path: &PathBuf, value: &Value) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(value)?;

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(raw.as_bytes())?;
        file.flush()?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, raw)?;
    }

    Ok(())
}

fn read_json(path: &PathBuf) -> anyhow::Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({})))
}

fn load_provider_index() -> HashSet<String> {
    let path = provider_auth_index_path();
    let json = read_json(&path).unwrap_or_else(|_| json!({}));
    let mut out = HashSet::new();
    if let Some(arr) = json.get("providers").and_then(Value::as_array) {
        for entry in arr {
            if let Some(id) = entry.as_str() {
                let normalized = normalize_provider_id(id);
                if !normalized.is_empty() {
                    out.insert(normalized);
                }
            }
        }
    }
    out
}

fn save_provider_index(ids: &HashSet<String>) -> anyhow::Result<()> {
    let mut sorted = ids
        .iter()
        .filter(|id| !id.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    sorted.sort();
    let path = provider_auth_index_path();
    write_secure_json(&path, &json!({ "providers": sorted }))
}

fn load_fallback_map() -> HashMap<String, String> {
    let path = provider_auth_fallback_path();
    let json = read_json(&path).unwrap_or_else(|_| json!({}));
    let mut out = HashMap::new();
    if let Some(obj) = json.as_object() {
        for (id, value) in obj {
            let provider_id = normalize_provider_id(id);
            if provider_id.is_empty() {
                continue;
            }
            let token = value
                .as_str()
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            if !token.is_empty() {
                out.insert(provider_id, token);
            }
        }
    }
    out
}

fn save_fallback_map(map: &HashMap<String, String>) -> anyhow::Result<()> {
    let path = provider_auth_fallback_path();
    let mut root = serde_json::Map::new();
    let mut pairs = map
        .iter()
        .filter_map(|(id, token)| {
            let provider_id = normalize_provider_id(id);
            let key = token.trim();
            if provider_id.is_empty() || key.is_empty() {
                None
            } else {
                Some((provider_id, key.to_string()))
            }
        })
        .collect::<Vec<_>>();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, token) in pairs {
        root.insert(id, Value::String(token));
    }
    write_secure_json(&path, &Value::Object(root))
}

pub fn load_provider_auth() -> HashMap<String, String> {
    let fallback = load_fallback_map();
    let mut known = load_provider_index();
    known.extend(fallback.keys().cloned());
    let mut out = HashMap::new();

    for provider_id in known {
        if let Some(entry) = keyring_entry(&provider_id) {
            if let Ok(secret) = entry.get_password() {
                let trimmed = secret.trim();
                if !trimmed.is_empty() {
                    out.insert(provider_id.clone(), trimmed.to_string());
                    continue;
                }
            }
        }
        if let Some(secret) = fallback.get(&provider_id) {
            let trimmed = secret.trim();
            if !trimmed.is_empty() {
                out.insert(provider_id.clone(), trimmed.to_string());
            }
        }
    }

    out
}

pub fn set_provider_auth(provider_id: &str, token: &str) -> anyhow::Result<ProviderAuthBackend> {
    let id = normalize_provider_id(provider_id);
    let secret = token.trim().to_string();
    if id.is_empty() {
        anyhow::bail!("provider id cannot be empty");
    }
    if secret.is_empty() {
        anyhow::bail!("provider token cannot be empty");
    }

    let mut known = load_provider_index();
    known.insert(id.clone());

    if let Some(entry) = keyring_entry(&id) {
        if entry.set_password(&secret).is_ok() {
            let mut fallback = load_fallback_map();
            fallback.remove(&id);
            let _ = save_fallback_map(&fallback);
            save_provider_index(&known)?;
            return Ok(ProviderAuthBackend::Keychain);
        }
    }

    let mut fallback = load_fallback_map();
    fallback.insert(id.clone(), secret);
    save_fallback_map(&fallback)?;
    save_provider_index(&known)?;
    Ok(ProviderAuthBackend::File)
}

pub fn delete_provider_auth(provider_id: &str) -> anyhow::Result<bool> {
    let id = normalize_provider_id(provider_id);
    if id.is_empty() {
        return Ok(false);
    }

    let mut removed = false;

    if let Some(entry) = keyring_entry(&id) {
        // Ignore unsupported backend errors; we still clear file fallback/index below.
        if entry.delete_password().is_ok() {
            removed = true;
        }
    }

    let mut fallback = load_fallback_map();
    if fallback.remove(&id).is_some() {
        removed = true;
    }
    save_fallback_map(&fallback)?;

    let mut known = load_provider_index();
    if known.remove(&id) {
        removed = true;
    }
    save_provider_index(&known)?;

    Ok(removed)
}
