use super::*;

fn parse_allowed_users(value: Option<&Value>) -> Vec<String> {
    let mut users = value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if users.is_empty() {
        users.push("*".to_string());
    }
    users
}

fn mask_saved_token(has_token: bool) -> Option<&'static str> {
    if has_token {
        Some("****")
    } else {
        None
    }
}

pub(super) async fn channels_config(State(state): State<AppState>) -> Json<Value> {
    let effective = state.config.get_effective_value().await;
    let channels = effective.get("channels").and_then(Value::as_object);

    let telegram = channels
        .and_then(|obj| obj.get("telegram"))
        .and_then(Value::as_object);
    let discord = channels
        .and_then(|obj| obj.get("discord"))
        .and_then(Value::as_object);
    let slack = channels
        .and_then(|obj| obj.get("slack"))
        .and_then(Value::as_object);

    let telegram_has_token = telegram
        .and_then(|cfg| cfg.get("bot_token"))
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let discord_has_token = discord
        .and_then(|cfg| cfg.get("bot_token"))
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let slack_has_token = slack
        .and_then(|cfg| cfg.get("bot_token"))
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    Json(json!({
        "telegram": {
            "has_token": telegram_has_token,
            "token_masked": mask_saved_token(telegram_has_token),
            "allowed_users": parse_allowed_users(telegram.and_then(|cfg| cfg.get("allowed_users"))),
            "mention_only": telegram
                .and_then(|cfg| cfg.get("mention_only"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "style_profile": telegram
                .and_then(|cfg| cfg.get("style_profile"))
                .and_then(Value::as_str)
                .unwrap_or("default"),
            "security_profile": telegram
                .and_then(|cfg| cfg.get("security_profile"))
                .and_then(Value::as_str)
                .unwrap_or("operator"),
        },
        "discord": {
            "has_token": discord_has_token,
            "token_masked": mask_saved_token(discord_has_token),
            "allowed_users": parse_allowed_users(discord.and_then(|cfg| cfg.get("allowed_users"))),
            "mention_only": discord
                .and_then(|cfg| cfg.get("mention_only"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            "guild_id": discord
                .and_then(|cfg| cfg.get("guild_id"))
                .and_then(Value::as_str),
            "security_profile": discord
                .and_then(|cfg| cfg.get("security_profile"))
                .and_then(Value::as_str)
                .unwrap_or("operator"),
        },
        "slack": {
            "has_token": slack_has_token,
            "token_masked": mask_saved_token(slack_has_token),
            "allowed_users": parse_allowed_users(slack.and_then(|cfg| cfg.get("allowed_users"))),
            "mention_only": slack
                .and_then(|cfg| cfg.get("mention_only"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "channel_id": slack
                .and_then(|cfg| cfg.get("channel_id"))
                .and_then(Value::as_str),
            "security_profile": slack
                .and_then(|cfg| cfg.get("security_profile"))
                .and_then(Value::as_str)
                .unwrap_or("operator"),
        }
    }))
}

pub(super) async fn channels_status(State(state): State<AppState>) -> Json<Value> {
    let status = state.channel_statuses().await;
    Json(json!({
        "telegram": status.get("telegram").cloned().unwrap_or_else(|| ChannelStatus {
            enabled: false,
            connected: false,
            last_error: None,
            active_sessions: 0,
            meta: json!({}),
        }),
        "discord": status.get("discord").cloned().unwrap_or_else(|| ChannelStatus {
            enabled: false,
            connected: false,
            last_error: None,
            active_sessions: 0,
            meta: json!({}),
        }),
        "slack": status.get("slack").cloned().unwrap_or_else(|| ChannelStatus {
            enabled: false,
            connected: false,
            last_error: None,
            active_sessions: 0,
            meta: json!({}),
        }),
    }))
}

pub(super) async fn channels_verify(
    State(state): State<AppState>,
    Path(name): Path<String>,
    input: Option<Json<Value>>,
) -> Result<Json<Value>, StatusCode> {
    let normalized = name.to_ascii_lowercase();
    let payload = input.map(|Json(v)| v).unwrap_or_else(|| json!({}));

    match normalized.as_str() {
        "discord" => Ok(Json(discord_channel_verify(&state, &payload).await)),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

const DISCORD_FLAG_GATEWAY_PRESENCE: u64 = 1 << 12;
const DISCORD_FLAG_GATEWAY_PRESENCE_LIMITED: u64 = 1 << 13;
const DISCORD_FLAG_GATEWAY_MEMBERS: u64 = 1 << 14;
const DISCORD_FLAG_GATEWAY_MEMBERS_LIMITED: u64 = 1 << 15;
const DISCORD_FLAG_GATEWAY_MESSAGE_CONTENT: u64 = 1 << 18;
const DISCORD_FLAG_GATEWAY_MESSAGE_CONTENT_LIMITED: u64 = 1 << 19;

async fn discord_channel_verify(state: &AppState, payload: &Value) -> Value {
    let provided_token = payload
        .get("bot_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let effective = state.config.get_effective_value().await;
    let saved_token = effective
        .get("channels")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("discord"))
        .and_then(Value::as_object)
        .and_then(|cfg| cfg.get("bot_token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let token = provided_token.or(saved_token).unwrap_or_default();
    let has_token = !token.is_empty();
    let mut hints: Vec<String> = Vec::new();
    if !has_token {
        hints.push("Add your Discord bot token, then click Save or Verify again.".to_string());
        return json!({
            "ok": false,
            "channel": "discord",
            "checks": {
                "has_token": false,
                "token_auth_ok": false,
                "gateway_ok": false,
                "message_content_intent_ok": false
            },
            "status_codes": {
                "users_me": null,
                "gateway_bot": null,
                "application_me": null
            },
            "hints": hints,
            "details": {}
        });
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return json!({
                "ok": false,
                "channel": "discord",
                "checks": {
                    "has_token": true,
                    "token_auth_ok": false,
                    "gateway_ok": false,
                    "message_content_intent_ok": false
                },
                "status_codes": {
                    "users_me": null,
                    "gateway_bot": null,
                    "application_me": null
                },
                "hints": ["Local HTTP client setup failed. Restart Tandem and retry verification."],
                "details": {
                    "error": e.to_string()
                }
            });
        }
    };
    let auth_header = format!("Bot {token}");

    let users_resp = client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", auth_header.clone())
        .send()
        .await;
    let gateway_resp = client
        .get("https://discord.com/api/v10/gateway/bot")
        .header("Authorization", auth_header.clone())
        .send()
        .await;
    let app_resp = client
        .get("https://discord.com/api/v10/applications/@me")
        .header("Authorization", auth_header)
        .send()
        .await;

    let users_status = users_resp.as_ref().ok().map(|r| r.status().as_u16());
    let gateway_status = gateway_resp.as_ref().ok().map(|r| r.status().as_u16());
    let app_status = app_resp.as_ref().ok().map(|r| r.status().as_u16());

    let token_auth_ok = users_status == Some(200);
    let gateway_ok = gateway_status == Some(200);

    let mut bot_username: Option<String> = None;
    let mut bot_id: Option<String> = None;
    if let Ok(resp) = users_resp {
        if resp.status().is_success() {
            if let Ok(v) = resp.json::<Value>().await {
                bot_username = v
                    .get("username")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                bot_id = v.get("id").and_then(Value::as_str).map(ToString::to_string);
            }
        }
    }

    let mut app_flags: Option<u64> = None;
    if let Ok(resp) = app_resp {
        if resp.status().is_success() {
            if let Ok(v) = resp.json::<Value>().await {
                app_flags = v.get("flags").and_then(Value::as_u64);
            }
        }
    }

    let message_content_intent_ok = app_flags.is_some_and(|flags| {
        flags
            & (DISCORD_FLAG_GATEWAY_MESSAGE_CONTENT | DISCORD_FLAG_GATEWAY_MESSAGE_CONTENT_LIMITED)
            != 0
    });
    let presence_intent_enabled = app_flags.is_some_and(|flags| {
        flags & (DISCORD_FLAG_GATEWAY_PRESENCE | DISCORD_FLAG_GATEWAY_PRESENCE_LIMITED) != 0
    });
    let server_members_intent_enabled = app_flags.is_some_and(|flags| {
        flags & (DISCORD_FLAG_GATEWAY_MEMBERS | DISCORD_FLAG_GATEWAY_MEMBERS_LIMITED) != 0
    });

    if !token_auth_ok {
        if users_status == Some(401) {
            hints.push("Discord rejected this token (401). Regenerate bot token in Developer Portal -> Bot and update Tandem.".to_string());
        } else {
            hints.push("Could not authenticate bot token with Discord `/users/@me`.".to_string());
        }
    }
    if !gateway_ok {
        if gateway_status == Some(429) {
            hints.push("Discord gateway verification is rate-limited right now. Wait a few seconds and verify again.".to_string());
        } else {
            hints.push("Discord `/gateway/bot` check failed. Verify outbound network access to discord.com.".to_string());
        }
    }
    if token_auth_ok && gateway_ok && !message_content_intent_ok {
        hints.push("Enable `Message Content Intent` in Discord Developer Portal -> Bot -> Privileged Gateway Intents.".to_string());
    }
    if hints.is_empty() {
        hints.push("Discord checks passed. If replies are still missing, verify channel/thread permissions: View Channel, Send Messages, Read Message History, Send Messages in Threads.".to_string());
    }

    let ok = token_auth_ok && gateway_ok && message_content_intent_ok;
    json!({
        "ok": ok,
        "channel": "discord",
        "checks": {
            "has_token": has_token,
            "token_auth_ok": token_auth_ok,
            "gateway_ok": gateway_ok,
            "message_content_intent_ok": message_content_intent_ok,
            "presence_intent_enabled": presence_intent_enabled,
            "server_members_intent_enabled": server_members_intent_enabled
        },
        "status_codes": {
            "users_me": users_status,
            "gateway_bot": gateway_status,
            "application_me": app_status
        },
        "hints": hints,
        "details": {
            "bot_username": bot_username,
            "bot_id": bot_id,
            "application_flags": app_flags
        }
    })
}

pub(super) async fn channels_put(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let normalized = name.to_ascii_lowercase();
    let effective = state.config.get_effective_value().await;
    let existing_channel_cfg = |channel: &str| -> Option<&serde_json::Map<String, Value>> {
        effective
            .get("channels")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get(channel))
            .and_then(Value::as_object)
    };
    let existing_bot_token = |channel: &str| -> Option<String> {
        existing_channel_cfg(channel)
            .and_then(|cfg| cfg.get("bot_token"))
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let existing_channel_id = |channel: &str| -> Option<String> {
        existing_channel_cfg(channel)
            .and_then(|cfg| cfg.get("channel_id"))
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let mut project = state.config.get_project_value().await;
    let Some(root) = project.as_object_mut() else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let channels = root
        .entry("channels".to_string())
        .or_insert_with(|| json!({}));
    let Some(channels_obj) = channels.as_object_mut() else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    match normalized.as_str() {
        "telegram" => {
            let mut cfg: TelegramConfigFile =
                serde_json::from_value(input).map_err(|_| StatusCode::BAD_REQUEST)?;
            cfg.allowed_users = crate::normalize_allowed_users_or_wildcard(cfg.allowed_users);
            if cfg.bot_token.trim().is_empty() {
                cfg.bot_token = existing_bot_token("telegram").unwrap_or_default();
            }
            if cfg.bot_token.trim().is_empty() {
                return Err(StatusCode::BAD_REQUEST);
            }
            channels_obj.insert("telegram".to_string(), json!(cfg));
        }
        "discord" => {
            let mut cfg: DiscordConfigFile =
                serde_json::from_value(input).map_err(|_| StatusCode::BAD_REQUEST)?;
            cfg.allowed_users = crate::normalize_allowed_users_or_wildcard(cfg.allowed_users);
            if cfg.bot_token.trim().is_empty() {
                cfg.bot_token = existing_bot_token("discord").unwrap_or_default();
            }
            if cfg.bot_token.trim().is_empty() {
                return Err(StatusCode::BAD_REQUEST);
            }
            channels_obj.insert("discord".to_string(), json!(cfg));
        }
        "slack" => {
            let mut cfg: SlackConfigFile =
                serde_json::from_value(input).map_err(|_| StatusCode::BAD_REQUEST)?;
            cfg.allowed_users = crate::normalize_allowed_users_or_wildcard(cfg.allowed_users);
            if cfg.bot_token.trim().is_empty() {
                cfg.bot_token = existing_bot_token("slack").unwrap_or_default();
            }
            if cfg.channel_id.trim().is_empty() {
                cfg.channel_id = existing_channel_id("slack").unwrap_or_default();
            }
            if cfg.bot_token.trim().is_empty() || cfg.channel_id.trim().is_empty() {
                return Err(StatusCode::BAD_REQUEST);
            }
            channels_obj.insert("slack".to_string(), json!(cfg));
        }
        _ => return Err(StatusCode::NOT_FOUND),
    }
    state
        .config
        .replace_project_value(project)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .restart_channel_listeners()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({"ok": true})))
}

pub(super) async fn channels_delete(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(secret_id) = tandem_core::channel_secret_store_id(&name) {
        let _ = tandem_core::delete_provider_auth(&secret_id);
    }
    let mut project = state.config.get_project_value().await;
    let Some(root) = project.as_object_mut() else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let channels = root
        .entry("channels".to_string())
        .or_insert_with(|| json!({}));
    let Some(channels_obj) = channels.as_object_mut() else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    channels_obj.remove(&name.to_ascii_lowercase());
    state
        .config
        .replace_project_value(project)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .restart_channel_listeners()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({"ok": true})))
}

pub(super) async fn admin_reload_config(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    state
        .providers
        .reload(state.config.get().await.into())
        .await;
    state
        .restart_channel_listeners()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct ChannelToolPreferences {
    #[serde(default)]
    pub enabled_tools: Vec<String>,
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    #[serde(default)]
    pub enabled_mcp_servers: Vec<String>,
}

const PUBLIC_DEMO_ALLOWED_TOOLS: &[&str] = &[
    "websearch",
    "webfetch",
    "webfetch_html",
    "memory_search",
    "memory_store",
    "memory_list",
];

fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn parse_channel_security_profile(
    raw: Option<&str>,
) -> tandem_channels::config::ChannelSecurityProfile {
    match raw.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if value == "trusted_team" || value == "trusted-team" => {
            tandem_channels::config::ChannelSecurityProfile::TrustedTeam
        }
        Some(value) if value == "public_demo" || value == "public-demo" => {
            tandem_channels::config::ChannelSecurityProfile::PublicDemo
        }
        _ => tandem_channels::config::ChannelSecurityProfile::Operator,
    }
}

fn channel_security_profile_from_config(
    effective: &Value,
    channel: &str,
) -> tandem_channels::config::ChannelSecurityProfile {
    let raw = effective
        .get("channels")
        .and_then(Value::as_object)
        .and_then(|channels| channels.get(channel))
        .and_then(Value::as_object)
        .and_then(|cfg| cfg.get("security_profile"))
        .and_then(Value::as_str);
    parse_channel_security_profile(raw)
}

fn sanitize_tool_preferences_for_security_profile(
    prefs: ChannelToolPreferences,
    security_profile: tandem_channels::config::ChannelSecurityProfile,
) -> ChannelToolPreferences {
    let enabled_tools = unique_strings(prefs.enabled_tools);
    let disabled_tools = unique_strings(prefs.disabled_tools);
    let enabled_mcp_servers = unique_strings(prefs.enabled_mcp_servers);

    if security_profile != tandem_channels::config::ChannelSecurityProfile::PublicDemo {
        return ChannelToolPreferences {
            enabled_tools,
            disabled_tools,
            enabled_mcp_servers,
        };
    }

    ChannelToolPreferences {
        enabled_tools: enabled_tools
            .into_iter()
            .filter(|tool| {
                PUBLIC_DEMO_ALLOWED_TOOLS
                    .iter()
                    .any(|allowed| allowed == tool)
            })
            .collect(),
        disabled_tools,
        enabled_mcp_servers: Vec::new(),
    }
}

fn tool_preferences_path() -> PathBuf {
    let base = std::env::var("TANDEM_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if let Some(data_dir) = dirs::data_dir() {
                return data_dir.join("tandem").join("data");
            }
            dirs::home_dir()
                .map(|home| home.join(".tandem").join("data"))
                .unwrap_or_else(|| PathBuf::from(".tandem"))
        });
    base.join("channel_tool_preferences.json")
}

type ToolPreferencesMap = std::collections::HashMap<String, ChannelToolPreferences>;

async fn load_tool_preferences_map() -> std::collections::HashMap<String, ChannelToolPreferences> {
    let path = tool_preferences_path();
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return std::collections::HashMap::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

async fn save_tool_preferences_map(map: &ToolPreferencesMap) {
    let path = tool_preferences_path();
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(json) = serde_json::to_vec_pretty(map) {
        let _ = tokio::fs::write(&path, json).await;
    }
}

pub(super) async fn channel_tool_preferences_get(
    State(state): State<AppState>,
    Path(channel): Path<String>,
) -> Result<Json<ChannelToolPreferences>, StatusCode> {
    let key = channel.to_string();
    let mut map = load_tool_preferences_map().await;
    let prefs = map.get(&key).cloned().unwrap_or_default();
    let effective = state.config.get_effective_value().await;
    let security_profile = channel_security_profile_from_config(&effective, &key);
    let sanitized = sanitize_tool_preferences_for_security_profile(prefs.clone(), security_profile);
    if sanitized != prefs {
        map.insert(key, sanitized.clone());
        save_tool_preferences_map(&map).await;
    }
    Ok(Json(sanitized))
}

#[derive(Debug, serde::Deserialize)]
pub struct ChannelToolPreferencesInput {
    pub enabled_tools: Option<Vec<String>>,
    pub disabled_tools: Option<Vec<String>>,
    pub enabled_mcp_servers: Option<Vec<String>>,
    pub reset: Option<bool>,
}

pub(super) async fn channel_tool_preferences_put(
    State(state): State<AppState>,
    Path(channel): Path<String>,
    Json(input): Json<ChannelToolPreferencesInput>,
) -> Result<Json<ChannelToolPreferences>, StatusCode> {
    let mut map = load_tool_preferences_map().await;
    let key = channel.to_string();
    let effective = state.config.get_effective_value().await;
    let security_profile = channel_security_profile_from_config(&effective, &key);

    let new_prefs = if input.reset.unwrap_or(false) {
        ChannelToolPreferences::default()
    } else {
        let existing = map.get(&key).cloned().unwrap_or_default();
        ChannelToolPreferences {
            enabled_tools: input.enabled_tools.unwrap_or(existing.enabled_tools),
            disabled_tools: input.disabled_tools.unwrap_or(existing.disabled_tools),
            enabled_mcp_servers: input
                .enabled_mcp_servers
                .unwrap_or(existing.enabled_mcp_servers),
        }
    };
    let new_prefs = sanitize_tool_preferences_for_security_profile(new_prefs, security_profile);

    map.insert(key, new_prefs.clone());
    save_tool_preferences_map(&map).await;
    Ok(Json(new_prefs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_demo_sanitizes_enabled_tools_and_mcp_servers() {
        let prefs = ChannelToolPreferences {
            enabled_tools: vec![
                "websearch".to_string(),
                "bash".to_string(),
                "webfetch_html".to_string(),
                "bash".to_string(),
            ],
            disabled_tools: vec!["read".to_string(), "read".to_string()],
            enabled_mcp_servers: vec!["github".to_string(), "slack".to_string()],
        };

        let sanitized = sanitize_tool_preferences_for_security_profile(
            prefs,
            tandem_channels::config::ChannelSecurityProfile::PublicDemo,
        );

        assert_eq!(
            sanitized.enabled_tools,
            vec!["websearch".to_string(), "webfetch_html".to_string()]
        );
        assert_eq!(sanitized.disabled_tools, vec!["read".to_string()]);
        assert!(sanitized.enabled_mcp_servers.is_empty());
    }

    #[test]
    fn operator_keeps_existing_tool_preferences() {
        let prefs = ChannelToolPreferences {
            enabled_tools: vec!["bash".to_string(), "bash".to_string()],
            disabled_tools: vec!["read".to_string(), "".to_string()],
            enabled_mcp_servers: vec!["github".to_string(), "github".to_string()],
        };

        let sanitized = sanitize_tool_preferences_for_security_profile(
            prefs,
            tandem_channels::config::ChannelSecurityProfile::Operator,
        );

        assert_eq!(sanitized.enabled_tools, vec!["bash".to_string()]);
        assert_eq!(sanitized.disabled_tools, vec!["read".to_string()]);
        assert_eq!(sanitized.enabled_mcp_servers, vec!["github".to_string()]);
    }
}
