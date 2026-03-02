use serde_json::Value;
use std::sync::OnceLock;

mod generated {
    include!("mcp_catalog_generated.rs");
}

fn normalize_slug(raw: &str) -> String {
    raw.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn index() -> Option<&'static Value> {
    static INDEX: OnceLock<Option<Value>> = OnceLock::new();
    INDEX
        .get_or_init(|| serde_json::from_str::<Value>(generated::INDEX_JSON).ok())
        .as_ref()
}

pub fn toml_for_slug(slug: &str) -> Option<&'static str> {
    let normalized = normalize_slug(slug);
    if normalized.is_empty() {
        return None;
    }
    generated::SERVERS
        .iter()
        .find(|(entry_slug, _)| *entry_slug == normalized)
        .map(|(_, toml)| *toml)
}
