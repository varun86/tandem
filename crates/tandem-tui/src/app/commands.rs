use serde_json::{json, Value};
use tandem_core::engine_api_token_file_path;
use tokio::time::sleep;

use super::plan_helpers;
use crate::app::{
    Action, AgentStatus, App, AppState, ContentBlock, EngineConnectionSource, EngineStalePolicy,
    MessageRole, ModalState, SetupStep, TandemMode, TaskStatus, UiMode,
};
use crate::command_catalog::HELP_TEXT;

pub(super) async fn try_execute_basic_command(
    app: &mut App,
    cmd_name: &str,
    args: &[&str],
) -> Option<String> {
    match cmd_name {
        "help" => Some(HELP_TEXT.to_string()),
        "diff" => Some(app.open_diff_overlay().await),
        "files" => {
            let query = if args.is_empty() {
                None
            } else {
                Some(args.join(" "))
            };
            app.open_file_search_modal(query.as_deref());
            Some(if let Some(q) = query {
                format!("Opened file search for query: {}", q)
            } else {
                "Opened file search overlay.".to_string()
            })
        }
        "edit" => Some(app.open_external_editor_for_active_input().await),
        "workspace" => Some(match args.first().copied() {
            Some("show") | None => {
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "<unknown>".to_string());
                format!("Current workspace directory:\n  {}", cwd)
            }
            Some("use") => {
                let raw_path = args
                    .get(1..)
                    .map(|items| items.join(" "))
                    .unwrap_or_default();
                if raw_path.trim().is_empty() {
                    return Some("Usage: /workspace use <path>".to_string());
                }
                let target = match App::resolve_workspace_path(raw_path.trim()) {
                    Ok(path) => path,
                    Err(err) => return Some(err),
                };
                let previous = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "<unknown>".to_string());
                if let Err(err) = std::env::set_current_dir(&target) {
                    return Some(format!(
                        "Failed to switch workspace to {}: {}",
                        target.display(),
                        err
                    ));
                }
                let current = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| target.display().to_string());
                format!(
                    "Workspace switched.\n  From: {}\n  To:   {}",
                    previous, current
                )
            }
            _ => "Usage: /workspace [show|use <path>]".to_string(),
        }),
        "engine" => Some(match args.first().copied() {
            Some("status") => {
                if let Some(client) = &app.client {
                    match client.get_engine_status().await {
                        Ok(status) => {
                            let required = App::desired_engine_version()
                                .map(App::format_semver_triplet)
                                .unwrap_or_else(|| "unknown".to_string());
                            let stale_policy = EngineStalePolicy::from_env();
                            format!(
                                "Engine Status:\n  Healthy: {}\n  Version: {}\n  Required: {}\n  Mode: {}\n  Endpoint: {}\n  Source: {}\n  Stale policy: {}",
                                if status.healthy { "Yes" } else { "No" },
                                status.version,
                                required,
                                status.mode,
                                client.base_url(),
                                app.engine_connection_source.as_str(),
                                stale_policy.as_str()
                            )
                        }
                        Err(e) => format!("Failed to get engine status: {}", e),
                    }
                } else {
                    "Engine: Not connected".to_string()
                }
            }
            Some("restart") => {
                app.connection_status = "Restarting engine...".to_string();
                app.release_engine_lease().await;
                app.stop_engine_process().await;
                app.client = None;
                app.engine_base_url_override = None;
                app.engine_connection_source = EngineConnectionSource::Unknown;
                app.engine_spawned_at = None;
                app.provider_catalog = None;
                sleep(std::time::Duration::from_millis(300)).await;
                app.state = AppState::Connecting;
                "Engine restart requested.".to_string()
            }
            Some("token") => {
                let show_full = args.get(1).map(|s| s.eq_ignore_ascii_case("show")) == Some(true);
                let Some(token) = app.engine_api_token.as_deref().map(str::trim) else {
                    return Some("Engine token is not configured.".to_string());
                };
                if token.is_empty() {
                    return Some("Engine token is not configured.".to_string());
                }
                let value = if show_full {
                    token.to_string()
                } else {
                    App::masked_engine_api_token(token)
                };
                let path = engine_api_token_file_path().to_string_lossy().to_string();
                let backend = app
                    .engine_api_token_backend
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                if show_full {
                    format!(
                        "Engine API token:\n  {}\nStorage: {}\nPath:\n  {}",
                        value, backend, path
                    )
                } else {
                    format!(
                        "Engine API token (masked):\n  {}\nStorage: {}\nUse `/engine token show` to reveal.\nPath:\n  {}",
                        value, backend, path
                    )
                }
            }
            _ => "Usage: /engine status | restart | token [show]".to_string(),
        }),
        "browser" => Some(match args.first().copied() {
            Some("status") | Some("doctor") => {
                if let Some(client) = &app.client {
                    match client.get_browser_status().await {
                        Ok(status) => {
                            let mut lines = vec![
                                "Browser Status:".to_string(),
                                format!("  Enabled: {}", if status.enabled { "Yes" } else { "No" }),
                                format!(
                                    "  Runnable: {}",
                                    if status.runnable { "Yes" } else { "No" }
                                ),
                                format!(
                                    "  Sidecar: {}",
                                    status
                                        .sidecar
                                        .path
                                        .clone()
                                        .unwrap_or_else(|| "<not found>".to_string())
                                ),
                                format!(
                                    "  Browser: {}",
                                    status
                                        .browser
                                        .path
                                        .clone()
                                        .unwrap_or_else(|| "<not found>".to_string())
                                ),
                            ];
                            if let Some(version) = status.browser.version.as_deref() {
                                lines.push(format!("  Browser version: {}", version));
                            }
                            if !status.blocking_issues.is_empty() {
                                lines.push("Blocking issues:".to_string());
                                for issue in status.blocking_issues {
                                    lines.push(format!("  - {}: {}", issue.code, issue.message));
                                }
                            }
                            if !status.recommendations.is_empty() {
                                lines.push("Recommendations:".to_string());
                                for row in status.recommendations {
                                    lines.push(format!("  - {}", row));
                                }
                            }
                            if !status.install_hints.is_empty() {
                                lines.push("Install hints:".to_string());
                                for row in status.install_hints {
                                    lines.push(format!("  - {}", row));
                                }
                            }
                            lines.join("\n")
                        }
                        Err(e) => format!("Failed to get browser status: {}", e),
                    }
                } else {
                    "Engine: Not connected".to_string()
                }
            }
            _ => "Usage: /browser status | doctor".to_string(),
        }),
        "agent" => Some(match args.first().copied() {
            Some("new") => {
                app.sync_active_agent_from_chat();
                let next_agent_id = if let AppState::Chat { agents, .. } = &app.state {
                    format!("A{}", agents.len() + 1)
                } else {
                    "A1".to_string()
                };
                let mut new_session_id: Option<String> = None;
                if let Some(client) = &app.client {
                    if let Ok(session) = client
                        .create_session(Some(format!("{} session", next_agent_id)))
                        .await
                    {
                        new_session_id = Some(session.id);
                    }
                }
                if let AppState::Chat {
                    agents,
                    active_agent_index,
                    ..
                } = &mut app.state
                {
                    let fallback_session = agents
                        .get(*active_agent_index)
                        .map(|a| a.session_id.clone())
                        .unwrap_or_default();
                    let pane = App::make_agent_pane(
                        next_agent_id,
                        new_session_id.unwrap_or(fallback_session),
                    );
                    agents.push(pane);
                    *active_agent_index = agents.len().saturating_sub(1);
                }
                app.sync_chat_from_active_agent();
                "Created new agent.".to_string()
            }
            Some("list") => {
                if let AppState::Chat {
                    agents,
                    active_agent_index,
                    ..
                } = &app.state
                {
                    let mut out = Vec::new();
                    for (i, a) in agents.iter().enumerate() {
                        let marker = if i == *active_agent_index { ">" } else { " " };
                        out.push(format!(
                            "{} {} [{}] {}",
                            marker,
                            a.agent_id,
                            a.session_id,
                            format!("{:?}", a.status)
                        ));
                    }
                    format!("Agents:\n{}", out.join("\n"))
                } else {
                    "Not in chat.".to_string()
                }
            }
            Some("use") => {
                if let Some(agent_id) = args.get(1) {
                    app.sync_active_agent_from_chat();
                    if let AppState::Chat {
                        agents,
                        active_agent_index,
                        ..
                    } = &mut app.state
                    {
                        if let Some(idx) = agents.iter().position(|a| &a.agent_id == agent_id) {
                            *active_agent_index = idx;
                            app.sync_chat_from_active_agent();
                            return Some(format!("Switched to {}.", agent_id));
                        }
                    }
                    format!("Agent not found: {}", agent_id)
                } else {
                    "Usage: /agent use <A#>".to_string()
                }
            }
            Some("close") => {
                app.sync_active_agent_from_chat();
                let active_idx = if let AppState::Chat {
                    active_agent_index, ..
                } = &app.state
                {
                    *active_agent_index
                } else {
                    0
                };
                app.cancel_agent_if_running(active_idx).await;
                if let AppState::Chat {
                    agents,
                    active_agent_index,
                    grid_page,
                    ..
                } = &mut app.state
                {
                    if agents.len() <= 1 {
                        return Some("Cannot close last agent.".to_string());
                    }
                    agents.remove(active_idx);
                    if *active_agent_index >= agents.len() {
                        *active_agent_index = agents.len().saturating_sub(1);
                    }
                    let max_page = agents.len().saturating_sub(1) / 4;
                    if *grid_page > max_page {
                        *grid_page = max_page;
                    }
                }
                app.sync_chat_from_active_agent();
                "Closed active agent.".to_string()
            }
            Some("fanout") => {
                let mode_switched = if matches!(app.current_mode, TandemMode::Plan) {
                    app.current_mode = TandemMode::Orchestrate;
                    true
                } else {
                    false
                };
                let mode_note = if mode_switched {
                    " Mode auto-switched from plan -> orchestrate."
                } else {
                    ""
                };
                let (target, goal_start_idx) = match args.get(1) {
                    Some(raw) => match raw.parse::<usize>() {
                        Ok(n) => (n.clamp(2, 9), 2),
                        Err(_) => (4, 1),
                    },
                    None => (4, 1),
                };
                let goal = args
                    .iter()
                    .skip(goal_start_idx)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();
                let created = app.ensure_agent_count(target).await;
                if let AppState::Chat {
                    ui_mode, grid_page, ..
                } = &mut app.state
                {
                    *ui_mode = UiMode::Grid;
                    *grid_page = 0;
                }
                app.sync_chat_from_active_agent();
                if !goal.is_empty() {
                    let agents = if let AppState::Chat { agents, .. } = &app.state {
                        agents.iter().take(target).cloned().collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    if let Some(lead) = agents.first() {
                        let team_name = format!(
                            "fanout-{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0)
                        );
                        let create_team_args = serde_json::json!({
                            "team_name": team_name,
                            "description": format!("Fanout run for goal: {}", goal),
                            "agent_type": "lead"
                        });
                        let mut lead_commands =
                            vec![format!("/tool TeamCreate {}", create_team_args)];
                        for agent in agents.iter().skip(1) {
                            let task_prompt = format!(
                                "You are {} in a coordinated fanout run for team `{}`.\n\
                                 Goal: {}.\n\
                                 Own one concrete workstream end-to-end, execute it, and report concise outcomes and blockers.\n\
                                 Do not ask clarification questions unless absolutely blocked.\n\
                                 Do not wait for plan approvals; make reasonable assumptions and proceed.",
                                agent.agent_id, team_name, goal
                            );
                            let task_args = serde_json::json!({
                                "description": format!("{} workstream for {}", agent.agent_id, goal),
                                "prompt": task_prompt,
                                "subagent_type": "generalist",
                                "team_name": team_name,
                                "name": agent.agent_id
                            });
                            lead_commands.push(format!("/tool task {}", task_args));
                        }
                        let lead_kickoff = format!(
                            "You are the lead coordinator for team `{}`. Goal: {}.\n\
                             Use TaskList/TaskUpdate to track delegated progress and keep execution moving until completion.",
                            team_name, goal
                        );
                        lead_commands.push(lead_kickoff);
                        if let AppState::Chat { agents, .. } = &mut app.state {
                            if let Some(lead_agent) = agents.iter_mut().find(|a| {
                                a.agent_id == lead.agent_id && a.session_id == lead.session_id
                            }) {
                                for cmd in lead_commands {
                                    lead_agent.follow_up_queue.push_back(cmd);
                                }
                            }
                        }
                        app.maybe_dispatch_queued_for_agent(&lead.session_id, &lead.agent_id);
                        return Some(format!(
                            "Started coordinated fanout: {} total agents (created {}). Team `{}` bootstrapped and assignments dispatched.{}",
                            target, created, team_name, mode_note
                        ));
                    }
                    return Some(format!(
                        "Started coordinated fanout: {} total agents (created {}). Goal dispatched.{}",
                        target, created, mode_note
                    ));
                }
                if created > 0 {
                    format!(
                        "Started fanout: {} total agents (created {}). Grid view enabled.{}",
                        target, created, mode_note
                    )
                } else {
                    format!(
                        "Fanout ready: already at {}+ agents. Grid view enabled.{}",
                        target, mode_note
                    )
                }
            }
            _ => "Usage: /agent new|list|use <A#>|close|fanout [n] [goal]".to_string(),
        }),
        "sessions" => Some(if app.sessions.is_empty() {
            "No sessions found.".to_string()
        } else {
            let lines: Vec<String> = app
                .sessions
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let marker = if i == app.selected_session_index {
                        "→ "
                    } else {
                        "  "
                    };
                    format!("{}{} (ID: {})", marker, s.title, s.id)
                })
                .collect();
            format!("Sessions:\n{}", lines.join("\n"))
        }),
        "new" => Some({
            let title = if args.is_empty() {
                None
            } else {
                Some(args.join(" ").trim().to_string())
            };
            let title_for_display = title.clone().unwrap_or_else(|| "New Session".to_string());
            if let Some(client) = &app.client {
                match client.create_session(title).await {
                    Ok(session) => {
                        app.sessions.push(session.clone());
                        app.selected_session_index = app.sessions.len() - 1;
                        format!(
                            "Created session: {} (ID: {})",
                            title_for_display, session.id
                        )
                    }
                    Err(e) => format!("Failed to create session: {}", e),
                }
            } else {
                "Not connected to engine".to_string()
            }
        }),
        "recent" => Some(match args.first().copied() {
            Some("run") => {
                let Some(raw_index) = args.get(1) else {
                    return Some("Usage: /recent run <index>".to_string());
                };
                let Ok(index) = raw_index.parse::<usize>() else {
                    return Some(format!("Invalid recent-command index: {}", raw_index));
                };
                if index == 0 {
                    return Some("Recent-command index is 1-based.".to_string());
                }
                let commands = app.recent_commands_snapshot();
                let Some(command) = commands.get(index - 1).cloned() else {
                    return Some(format!(
                        "Recent-command index {} is out of range ({} stored).",
                        index,
                        commands.len()
                    ));
                };
                let result = Box::pin(app.execute_command(&command)).await;
                format!(
                    "Replayed recent command #{}: {}\n\n{}",
                    index, command, result
                )
            }
            Some("clear") => {
                let cleared = app.clear_recent_commands();
                format!("Cleared {} recent command(s).", cleared)
            }
            Some("list") | None => {
                let commands = app.recent_commands_snapshot();
                if commands.is_empty() {
                    "No recent slash commands yet.".to_string()
                } else {
                    format!(
                        "Recent commands:\n{}\n\nNext\n  /recent run <index>\n  /recent clear",
                        commands
                            .iter()
                            .enumerate()
                            .map(|(idx, command)| format!("  {}. {}", idx + 1, command))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                }
            }
            _ => "Usage: /recent [list|run <index>|clear]".to_string(),
        }),
        "use" => Some({
            let Some(target_id) = args.first().copied() else {
                return Some("Usage: /use <session_id>".to_string());
            };
            if let Some(idx) = app.sessions.iter().position(|s| s.id == target_id) {
                app.selected_session_index = idx;
                let loaded_messages = app.load_chat_history(target_id).await;
                let (recalled_tasks, recalled_active_task_id) =
                    plan_helpers::rebuild_tasks_from_messages(&loaded_messages);
                if let AppState::Chat {
                    session_id,
                    messages,
                    scroll_from_bottom,
                    tasks,
                    active_task_id,
                    agents,
                    active_agent_index,
                    ..
                } = &mut app.state
                {
                    *session_id = target_id.to_string();
                    *messages = loaded_messages.clone();
                    *scroll_from_bottom = 0;
                    *tasks = recalled_tasks.clone();
                    *active_task_id = recalled_active_task_id.clone();
                    if let Some(agent) = agents.get_mut(*active_agent_index) {
                        agent.session_id = target_id.to_string();
                        agent.messages = loaded_messages;
                        agent.scroll_from_bottom = 0;
                        agent.tasks = recalled_tasks;
                        agent.active_task_id = recalled_active_task_id;
                    }
                }
                format!("Switched to session: {}", target_id)
            } else {
                format!("Session not found: {}", target_id)
            }
        }),
        "keys" => Some(if let Some(keystore) = &app.keystore {
            let mut provider_ids: Vec<String> = keystore
                .list_keys()
                .into_iter()
                .map(|k| App::normalize_provider_id_from_keystore_key(&k))
                .collect();
            provider_ids.sort();
            provider_ids.dedup();
            if provider_ids.is_empty() {
                "No provider keys configured.".to_string()
            } else {
                format!(
                    "Configured providers:\n{}",
                    provider_ids
                        .iter()
                        .map(|p| format!("  {} - configured", p))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        } else {
            "Keystore not unlocked. Enter PIN to access keys.".to_string()
        }),
        "key" => Some(match args.first().copied() {
            Some("set") => {
                let provider_id = args
                    .get(1)
                    .map(|s| s.to_string())
                    .or_else(|| app.current_provider.clone());
                let Some(provider_id) = provider_id else {
                    return Some(
                        "Usage: /key set <provider_id> (or set /provider first)".to_string(),
                    );
                };
                if app.open_key_wizard_for_provider(&provider_id) {
                    format!("Opening key setup wizard for {}...", provider_id)
                } else {
                    format!("Provider not found: {}", provider_id)
                }
            }
            Some("remove") => {
                let Some(provider_id) = args.get(1).copied() else {
                    return Some("Usage: /key remove <provider_id>".to_string());
                };
                format!("Key removal not implemented. Provider: {}", provider_id)
            }
            Some("test") => {
                let Some(provider_id) = args.get(1).copied() else {
                    return Some("Usage: /key test <provider_id>".to_string());
                };
                if let Some(client) = &app.client {
                    if let Ok(catalog) = client.list_providers().await {
                        let catalog = App::sanitize_provider_catalog(catalog);
                        let is_connected = catalog.connected.contains(&provider_id.to_string());
                        if catalog.all.iter().any(|p| p.id == provider_id) {
                            if is_connected {
                                return Some(format!(
                                    "Provider {}: Connected and working!",
                                    provider_id
                                ));
                            }
                            return Some(format!(
                                "Provider {}: Not connected. Use /key set to add credentials.",
                                provider_id
                            ));
                        }
                    }
                }
                format!("Provider {}: Not connected or not available.", provider_id)
            }
            _ => "Usage: /key set|remove|test <provider_id>".to_string(),
        }),
        "cancel" => Some({
            let active_idx = if let AppState::Chat {
                active_agent_index, ..
            } = &app.state
            {
                *active_agent_index
            } else {
                0
            };
            app.cancel_agent_if_running(active_idx).await;
            if let AppState::Chat { agents, .. } = &mut app.state {
                if let Some(agent) = agents.get_mut(active_idx) {
                    agent.status = AgentStatus::Idle;
                    agent.active_run_id = None;
                }
            }
            app.sync_chat_from_active_agent();
            "Cancel requested for active agent.".to_string()
        }),
        "steer" => Some({
            if args.is_empty() {
                return Some("Usage: /steer <message>".to_string());
            }
            let msg = args.join(" ");
            if let AppState::Chat { command_input, .. } = &mut app.state {
                command_input.set_text(msg);
            }
            if let Some(tx) = &app.action_tx {
                let _ = tx.send(Action::QueueSteeringFromComposer);
            }
            "Steering message queued.".to_string()
        }),
        "followup" => Some({
            if args.is_empty() {
                return Some("Usage: /followup <message>".to_string());
            }
            let msg = args.join(" ");
            let mut queued_len = 0usize;
            if let AppState::Chat {
                agents,
                active_agent_index,
                ..
            } = &mut app.state
            {
                if let Some(agent) = agents.get_mut(*active_agent_index) {
                    let merged_into_existing = !agent.follow_up_queue.is_empty();
                    if merged_into_existing {
                        if let Some(last) = agent.follow_up_queue.back_mut() {
                            if !last.is_empty() {
                                last.push('\n');
                            }
                            last.push_str(&msg);
                        }
                    } else {
                        agent.follow_up_queue.push_back(msg);
                    }
                    queued_len = agent.follow_up_queue.len();
                }
            }
            format!("Queued follow-up message (#{}).", queued_len)
        }),
        "queue" => Some({
            if matches!(args.first().map(|s| s.to_ascii_lowercase()), Some(cmd) if cmd == "clear") {
                if let AppState::Chat {
                    agents,
                    active_agent_index,
                    ..
                } = &mut app.state
                {
                    if let Some(agent) = agents.get_mut(*active_agent_index) {
                        agent.follow_up_queue.clear();
                        agent.steering_message = None;
                    }
                }
                return Some("Cleared queued steering and follow-up messages.".to_string());
            }
            if let AppState::Chat {
                agents,
                active_agent_index,
                ..
            } = &app.state
            {
                if let Some(agent) = agents.get(*active_agent_index) {
                    let steering = if agent.steering_message.is_some() {
                        "yes"
                    } else {
                        "no"
                    };
                    let next_followup = agent
                        .follow_up_queue
                        .front()
                        .map(|m| {
                            if m.chars().count() > 80 {
                                format!("{}...", m.chars().take(80).collect::<String>())
                            } else {
                                m.clone()
                            }
                        })
                        .unwrap_or_else(|| "(none)".to_string());
                    return Some(format!(
                        "Queue status:\n  steering: {}\n  follow-ups: {}\n  next: {}",
                        steering,
                        agent.follow_up_queue.len(),
                        next_followup
                    ));
                }
            }
            "Queue unavailable in current state.".to_string()
        }),
        "messages" => Some({
            let limit = args.first().and_then(|s| s.parse().ok()).unwrap_or(10);
            format!("Message history not implemented yet. (limit: {})", limit)
        }),
        "last_error" => Some(if let AppState::Chat { messages, .. } = &app.state {
            let maybe_error = messages.iter().rev().find_map(|m| {
                if m.role != MessageRole::System {
                    return None;
                }
                let text = m
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.to_lowercase().contains("failed") || text.to_lowercase().contains("error") {
                    Some(text)
                } else {
                    None
                }
            });
            maybe_error.unwrap_or_else(|| "No recent error found.".to_string())
        } else {
            "Not in a chat session.".to_string()
        }),
        "task" => Some(if let AppState::Chat { tasks, .. } = &mut app.state {
            match args.first().copied() {
                Some("add") => {
                    if args.len() < 2 {
                        return Some("Usage: /task add <description>".to_string());
                    }
                    let description = args[1..].join(" ");
                    let id = format!("task-{}", tasks.len() + 1);
                    tasks.push(crate::app::Task {
                        id: id.clone(),
                        description: description.clone(),
                        status: TaskStatus::Pending,
                        pinned: false,
                    });
                    format!("Task added: {} (ID: {})", description, id)
                }
                Some("done") | Some("fail") | Some("work") | Some("pending") => {
                    if args.len() < 2 {
                        return Some("Usage: /task <status> <id>".to_string());
                    }
                    let id = args[1];
                    if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
                        match args[0] {
                            "done" => task.status = TaskStatus::Done,
                            "fail" => task.status = TaskStatus::Failed,
                            "work" => task.status = TaskStatus::Working,
                            "pending" => task.status = TaskStatus::Pending,
                            _ => {}
                        }
                        format!("Task {} marked as {}", id, args[0])
                    } else {
                        format!("Task not found: {}", id)
                    }
                }
                Some("pin") => {
                    if args.len() < 2 {
                        return Some("Usage: /task pin <id>".to_string());
                    }
                    let id = args[1];
                    if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
                        task.pinned = !task.pinned;
                        format!("Task {} pinned: {}", id, task.pinned)
                    } else {
                        format!("Task not found: {}", id)
                    }
                }
                Some("list") => {
                    if tasks.is_empty() {
                        "No tasks.".to_string()
                    } else {
                        let lines: Vec<String> = tasks
                            .iter()
                            .map(|t| {
                                format!(
                                    "[{}] {} ({:?}) - Pinned: {}",
                                    t.id, t.description, t.status, t.pinned
                                )
                            })
                            .collect();
                        format!("Tasks:\n{}", lines.join("\n"))
                    }
                }
                _ => "Usage: /task add|done|fail|work|pin|list ...".to_string(),
            }
        } else {
            "Not in a chat session.".to_string()
        }),
        "prompt" => Some({
            let text = args.join(" ");
            if text.is_empty() {
                return Some("Usage: /prompt <text...>".to_string());
            }
            let (session_id, active_agent_id) = if let AppState::Chat {
                session_id,
                agents,
                active_agent_index,
                ..
            } = &mut app.state
            {
                let agent_id = agents
                    .get(*active_agent_index)
                    .map(|a| a.agent_id.clone())
                    .unwrap_or_else(|| "A1".to_string());
                (session_id.clone(), agent_id)
            } else {
                (String::new(), "A1".to_string())
            };

            if session_id.is_empty() {
                return Some("Not in a chat session. Use /use <session_id> first.".to_string());
            }
            app.dispatch_prompt_for_agent(session_id, active_agent_id, text);
            "Prompt sent.".to_string()
        }),
        "title" => Some({
            let new_title = args.join(" ");
            if new_title.is_empty() {
                return Some("Usage: /title <new title...>".to_string());
            }
            if let AppState::Chat { session_id, .. } = &mut app.state {
                if let Some(client) = &app.client {
                    let req = crate::net::client::UpdateSessionRequest {
                        title: Some(new_title.clone()),
                        ..Default::default()
                    };
                    if let Ok(_session) = client.update_session(session_id, req).await {
                        if let Some(s) = app.sessions.iter_mut().find(|s| &s.id == session_id) {
                            s.title = new_title.clone();
                        }
                        return Some(format!("Session renamed to: {}", new_title));
                    }
                }
                "Failed to rename session.".to_string()
            } else {
                "Not in a chat session.".to_string()
            }
        }),
        "missions" => Some({
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            match client.mission_list().await {
                Ok(missions) => {
                    if missions.is_empty() {
                        return Some("No missions found.".to_string());
                    }
                    let lines = missions
                        .into_iter()
                        .map(|mission| {
                            format!(
                                "- {} [{}] {} (work_items={})",
                                mission.mission_id,
                                format!("{:?}", mission.status).to_lowercase(),
                                mission.spec.title,
                                mission.work_items.len()
                            )
                        })
                        .collect::<Vec<_>>();
                    format!("Missions:\n{}", lines.join("\n"))
                }
                Err(err) => format!("Failed to list missions: {}", err),
            }
        }),
        "mission_create" => Some({
            if args.is_empty() {
                return Some(
                    "Usage: /mission_create <title> :: <goal> [:: work_item_title]".to_string(),
                );
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let raw = args.join(" ");
            let segments = raw
                .split("::")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if segments.len() < 2 {
                return Some(
                    "Usage: /mission_create <title> :: <goal> [:: work_item_title]".to_string(),
                );
            }
            let work_items = if let Some(work_item_title) = segments.get(2) {
                vec![crate::net::client::MissionCreateWorkItem {
                    work_item_id: None,
                    title: (*work_item_title).to_string(),
                    detail: None,
                    assigned_agent: None,
                }]
            } else {
                vec![crate::net::client::MissionCreateWorkItem {
                    work_item_id: None,
                    title: "Initial implementation".to_string(),
                    detail: Some("Auto-seeded work item".to_string()),
                    assigned_agent: None,
                }]
            };
            let request = crate::net::client::MissionCreateRequest {
                title: segments[0].to_string(),
                goal: segments[1].to_string(),
                work_items,
            };
            match client.mission_create(request).await {
                Ok(mission) => format!(
                    "Created mission {}: {}",
                    mission.mission_id, mission.spec.title
                ),
                Err(err) => format!("Failed to create mission: {}", err),
            }
        }),
        "mission_get" => Some({
            if args.len() != 1 {
                return Some("Usage: /mission_get <mission_id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            match client.mission_get(args[0]).await {
                Ok(mission) => {
                    let item_lines = mission
                        .work_items
                        .iter()
                        .map(|item| {
                            format!(
                                "- {} [{}]",
                                item.title,
                                format!("{:?}", item.status).to_lowercase()
                            )
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "Mission {} [{}]\nTitle: {}\nGoal: {}\nWork Items:\n{}",
                        mission.mission_id,
                        format!("{:?}", mission.status).to_lowercase(),
                        mission.spec.title,
                        mission.spec.goal,
                        if item_lines.is_empty() {
                            "- (none)".to_string()
                        } else {
                            item_lines.join("\n")
                        }
                    )
                }
                Err(err) => format!("Failed to get mission: {}", err),
            }
        }),
        "mission_event" => Some({
            if args.len() < 2 {
                return Some("Usage: /mission_event <mission_id> <event_json>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let mission_id = args[0];
            let raw_json = args[1..].join(" ");
            let event = match serde_json::from_str::<Value>(&raw_json) {
                Ok(value) => value,
                Err(err) => return Some(format!("Invalid event JSON: {}", err)),
            };
            match client.mission_apply_event(mission_id, event).await {
                Ok(result) => format!(
                    "Applied event to mission {} (revision={}, commands={})",
                    result.mission.mission_id,
                    result.mission.revision,
                    result.commands.len()
                ),
                Err(err) => format!("Failed to apply mission event: {}", err),
            }
        }),
        "mission_start" => Some({
            if args.len() != 1 {
                return Some("Usage: /mission_start <mission_id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let mission_id = args[0];
            let event = serde_json::json!({
                "type": "mission_started",
                "mission_id": mission_id
            });
            match client.mission_apply_event(mission_id, event).await {
                Ok(result) => format!(
                    "Mission started {} (revision={})",
                    result.mission.mission_id, result.mission.revision
                ),
                Err(err) => format!("Failed to start mission: {}", err),
            }
        }),
        "mission_review_ok" => Some({
            if args.len() < 2 {
                return Some(
                    "Usage: /mission_review_ok <mission_id> <work_item_id> [approval_id]"
                        .to_string(),
                );
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let mission_id = args[0];
            let work_item_id = args[1];
            let approval_id = args.get(2).copied().unwrap_or("review-1");
            let event = serde_json::json!({
                "type": "approval_granted",
                "mission_id": mission_id,
                "work_item_id": work_item_id,
                "approval_id": approval_id
            });
            match client.mission_apply_event(mission_id, event).await {
                Ok(result) => format!(
                    "Review approved for {}:{} (revision={})",
                    mission_id, work_item_id, result.mission.revision
                ),
                Err(err) => format!("Failed to approve review: {}", err),
            }
        }),
        "mission_test_ok" => Some({
            if args.len() < 2 {
                return Some(
                    "Usage: /mission_test_ok <mission_id> <work_item_id> [approval_id]".to_string(),
                );
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let mission_id = args[0];
            let work_item_id = args[1];
            let approval_id = args.get(2).copied().unwrap_or("test-1");
            let event = serde_json::json!({
                "type": "approval_granted",
                "mission_id": mission_id,
                "work_item_id": work_item_id,
                "approval_id": approval_id
            });
            match client.mission_apply_event(mission_id, event).await {
                Ok(result) => format!(
                    "Test approved for {}:{} (revision={})",
                    mission_id, work_item_id, result.mission.revision
                ),
                Err(err) => format!("Failed to approve test: {}", err),
            }
        }),
        "mission_review_no" => Some({
            if args.len() < 2 {
                return Some(
                    "Usage: /mission_review_no <mission_id> <work_item_id> [reason]".to_string(),
                );
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let mission_id = args[0];
            let work_item_id = args[1];
            let reason = if args.len() > 2 {
                args[2..].join(" ")
            } else {
                "needs_revision".to_string()
            };
            let event = serde_json::json!({
                "type": "approval_denied",
                "mission_id": mission_id,
                "work_item_id": work_item_id,
                "approval_id": "review-1",
                "reason": reason
            });
            match client.mission_apply_event(mission_id, event).await {
                Ok(result) => format!(
                    "Review denied for {}:{} (revision={})",
                    mission_id, work_item_id, result.mission.revision
                ),
                Err(err) => format!("Failed to deny review: {}", err),
            }
        }),
        "agent-team" | "agent_team" => Some({
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let sub = args.first().copied().unwrap_or("summary");
            match sub {
                "summary" => {
                    let missions = client.agent_team_missions().await;
                    let instances = client.agent_team_instances(None).await;
                    let approvals = client.agent_team_approvals().await;
                    match (missions, instances, approvals) {
                        (Ok(missions), Ok(instances), Ok(approvals)) => format!(
                            "Agent-Team Summary:\n  Missions: {}\n  Instances: {}\n  Spawn approvals: {}\n  Tool approvals: {}",
                            missions.len(),
                            instances.len(),
                            approvals.spawn_approvals.len(),
                            approvals.tool_approvals.len()
                        ),
                        _ => "Failed to load agent-team summary.".to_string(),
                    }
                }
                "missions" => match client.agent_team_missions().await {
                    Ok(missions) => {
                        if missions.is_empty() {
                            return Some("No agent-team missions found.".to_string());
                        }
                        let lines = missions
                            .into_iter()
                            .map(|mission| {
                                format!(
                                    "- {} total={} running={} done={} failed={} cancelled={}",
                                    mission.mission_id,
                                    mission.instance_count,
                                    mission.running_count,
                                    mission.completed_count,
                                    mission.failed_count,
                                    mission.cancelled_count
                                )
                            })
                            .collect::<Vec<_>>();
                        format!("Agent-Team Missions:\n{}", lines.join("\n"))
                    }
                    Err(err) => format!("Failed to list agent-team missions: {}", err),
                },
                "instances" => {
                    let mission_id = args.get(1).copied();
                    match client.agent_team_instances(mission_id).await {
                        Ok(instances) => {
                            if instances.is_empty() {
                                return Some("No agent-team instances found.".to_string());
                            }
                            let lines = instances
                                .into_iter()
                                .map(|instance| {
                                    format!(
                                        "- {} role={} mission={} status={} parent={}",
                                        instance.instance_id,
                                        instance.role,
                                        instance.mission_id,
                                        instance.status,
                                        instance
                                            .parent_instance_id
                                            .unwrap_or_else(|| "-".to_string())
                                    )
                                })
                                .collect::<Vec<_>>();
                            format!("Agent-Team Instances:\n{}", lines.join("\n"))
                        }
                        Err(err) => format!("Failed to list agent-team instances: {}", err),
                    }
                }
                "approvals" => match client.agent_team_approvals().await {
                    Ok(approvals) => {
                        let mut lines = Vec::new();
                        for spawn in approvals.spawn_approvals {
                            lines.push(format!("- spawn approval {}", spawn.approval_id));
                        }
                        for tool in approvals.tool_approvals {
                            lines.push(format!(
                                "- tool approval {} ({})",
                                tool.approval_id,
                                tool.tool.unwrap_or_else(|| "tool".to_string())
                            ));
                        }
                        if lines.is_empty() {
                            "No agent-team approvals pending.".to_string()
                        } else {
                            format!("Agent-Team Approvals:\n{}", lines.join("\n"))
                        }
                    }
                    Err(err) => format!("Failed to list agent-team approvals: {}", err),
                },
                "bindings" => {
                    let team_filter = args.get(1).copied();
                    App::format_local_agent_team_bindings(team_filter)
                }
                "approve" => {
                    if args.len() < 3 {
                        return Some(
                            "Usage: /agent-team approve <spawn|tool> <id> [reason]".to_string(),
                        );
                    }
                    let target = args[1];
                    let id = args[2];
                    let reason = if args.len() > 3 {
                        args[3..].join(" ")
                    } else {
                        "approved in TUI".to_string()
                    };
                    match target {
                        "spawn" => match client.agent_team_approve_spawn(id, &reason).await {
                            Ok(true) => format!("Approved spawn approval {}.", id),
                            Ok(false) => format!("Spawn approval not found or denied: {}", id),
                            Err(err) => format!("Failed to approve spawn approval: {}", err),
                        },
                        "tool" => match client.reply_permission(id, "allow").await {
                            Ok(true) => format!("Approved tool request {}.", id),
                            Ok(false) => format!("Tool request not found: {}", id),
                            Err(err) => format!("Failed to approve tool request: {}", err),
                        },
                        _ => "Usage: /agent-team approve <spawn|tool> <id> [reason]".to_string(),
                    }
                }
                "deny" => {
                    if args.len() < 3 {
                        return Some(
                            "Usage: /agent-team deny <spawn|tool> <id> [reason]".to_string(),
                        );
                    }
                    let target = args[1];
                    let id = args[2];
                    let reason = if args.len() > 3 {
                        args[3..].join(" ")
                    } else {
                        "denied in TUI".to_string()
                    };
                    match target {
                        "spawn" => match client.agent_team_deny_spawn(id, &reason).await {
                            Ok(true) => format!("Denied spawn approval {}.", id),
                            Ok(false) => {
                                format!("Spawn approval not found or already resolved: {}", id)
                            }
                            Err(err) => format!("Failed to deny spawn approval: {}", err),
                        },
                        "tool" => match client.reply_permission(id, "deny").await {
                            Ok(true) => format!("Denied tool request {}.", id),
                            Ok(false) => format!("Tool request not found: {}", id),
                            Err(err) => format!("Failed to deny tool request: {}", err),
                        },
                        _ => "Usage: /agent-team deny <spawn|tool> <id> [reason]".to_string(),
                    }
                }
                _ => {
                    "Usage: /agent-team [summary|missions|instances [mission_id]|approvals|bindings [team]|approve <spawn|tool> <id> [reason]|deny <spawn|tool> <id> [reason]]".to_string()
                }
            }
        }),
        "preset" | "presets" => Some({
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let sub = args.first().copied().unwrap_or("help").to_ascii_lowercase();
            match sub.as_str() {
                "index" => match client.presets_index().await {
                    Ok(index) => format!(
                        "Preset index:\n  skill_modules: {}\n  agent_presets: {}\n  automation_presets: {}\n  pack_presets: {}\n  generated_at_ms: {}",
                        index.skill_modules.len(),
                        index.agent_presets.len(),
                        index.automation_presets.len(),
                        index.pack_presets.len(),
                        index.generated_at_ms
                    ),
                    Err(err) => format!("Failed to load preset index: {}", err),
                },
                "agent" => {
                    let action = args.get(1).copied().unwrap_or("help").to_ascii_lowercase();
                    match action.as_str() {
                        "compose" => {
                            let tail = args.get(2..).unwrap_or(&[]).join(" ");
                            let mut pieces = tail.splitn(2, "::");
                            let base_prompt = pieces.next().unwrap_or("").trim();
                            let fragments_raw = pieces.next().unwrap_or("").trim();
                            if base_prompt.is_empty() || fragments_raw.is_empty() {
                                return Some(
                                    "Usage: /preset agent compose <base_prompt> :: <fragments_json>"
                                        .to_string(),
                                );
                            }
                            let fragments_json =
                                match serde_json::from_str::<Value>(fragments_raw) {
                                    Ok(value) if value.is_array() => value,
                                    Ok(_) => {
                                        return Some(
                                            "fragments_json must be a JSON array of {id,phase,content}"
                                                .to_string(),
                                        );
                                    }
                                    Err(err) => return Some(format!("Invalid fragments_json: {}", err)),
                                };
                            let request = json!({
                                "base_prompt": base_prompt,
                                "fragments": fragments_json,
                            });
                            match client.presets_compose_preview(request).await {
                                Ok(payload) => {
                                    let composition =
                                        payload.get("composition").cloned().unwrap_or(payload);
                                    format!(
                                        "Agent compose preview:\n{}",
                                        serde_json::to_string_pretty(&composition)
                                            .unwrap_or_else(|_| "{}".to_string())
                                    )
                                }
                                Err(err) => format!("Compose preview failed: {}", err),
                            }
                        }
                        "summary" => {
                            let tail = args.get(2..).unwrap_or(&[]).join(" ");
                            let (required, optional) =
                                App::parse_required_optional_segments(&tail);
                            let request = json!({
                                "agent": {
                                    "required": required,
                                    "optional": optional,
                                },
                                "tasks": [],
                            });
                            match client.presets_capability_summary(request).await {
                                Ok(payload) => {
                                    let summary = payload.get("summary").cloned().unwrap_or(payload);
                                    format!(
                                        "Agent capability summary:\n{}",
                                        serde_json::to_string_pretty(&summary)
                                            .unwrap_or_else(|_| "{}".to_string())
                                    )
                                }
                                Err(err) => format!("Capability summary failed: {}", err),
                            }
                        }
                        "fork" => {
                            if args.len() < 3 {
                                return Some(
                                    "Usage: /preset agent fork <source_path> [target_id]".to_string(),
                                );
                            }
                            let source_path = args[2];
                            let target_id = args.get(3).copied();
                            let request = json!({
                                "kind": "agent_preset",
                                "source_path": source_path,
                                "target_id": target_id,
                            });
                            match client.presets_fork(request).await {
                                Ok(payload) => format!(
                                    "Forked agent preset override:\n{}",
                                    serde_json::to_string_pretty(&payload)
                                        .unwrap_or_else(|_| "{}".to_string())
                                ),
                                Err(err) => format!("Agent preset fork failed: {}", err),
                            }
                        }
                        _ => "Usage: /preset agent <compose|summary|fork> ...".to_string(),
                    }
                }
                "automation" => {
                    let action = args.get(1).copied().unwrap_or("help").to_ascii_lowercase();
                    match action.as_str() {
                        "summary" => {
                            let tail = args.get(2..).unwrap_or(&[]).join(" ");
                            let segments = tail
                                .split("::")
                                .map(str::trim)
                                .filter(|part| !part.is_empty())
                                .collect::<Vec<_>>();
                            if segments.is_empty() {
                                return Some("Usage: /preset automation summary <tasks_json> [:: required=<csv> :: optional=<csv>]".to_string());
                            }
                            let tasks_json = match serde_json::from_str::<Value>(segments[0]) {
                                Ok(value) => value,
                                Err(err) => return Some(format!("Invalid tasks_json: {}", err)),
                            };
                            let tasks = match App::normalize_automation_tasks(&tasks_json) {
                                Ok(items) => items,
                                Err(err) => return Some(err),
                            };
                            let (required, optional) = if segments.len() > 1 {
                                App::parse_required_optional_segments(&segments[1..].join(" :: "))
                            } else {
                                (Vec::new(), Vec::new())
                            };
                            let capability_tasks = tasks
                                .iter()
                                .map(|task| {
                                    json!({
                                        "required": task.get("required").cloned().unwrap_or_else(|| json!([])),
                                        "optional": task.get("optional").cloned().unwrap_or_else(|| json!([])),
                                    })
                                })
                                .collect::<Vec<_>>();
                            let request = json!({
                                "agent": {
                                    "required": required,
                                    "optional": optional,
                                },
                                "tasks": capability_tasks,
                            });
                            match client.presets_capability_summary(request).await {
                                Ok(payload) => {
                                    let summary = payload.get("summary").cloned().unwrap_or(payload);
                                    format!(
                                        "Automation capability summary ({} tasks):\n{}",
                                        tasks.len(),
                                        serde_json::to_string_pretty(&summary)
                                            .unwrap_or_else(|_| "{}".to_string())
                                    )
                                }
                                Err(err) => format!("Automation summary failed: {}", err),
                            }
                        }
                        "save" => {
                            let tail = args.get(2..).unwrap_or(&[]).join(" ");
                            let segments = tail
                                .split("::")
                                .map(str::trim)
                                .filter(|part| !part.is_empty())
                                .collect::<Vec<_>>();
                            if segments.len() < 2 {
                                return Some("Usage: /preset automation save <id> :: <tasks_json> [:: required=<csv> :: optional=<csv>]".to_string());
                            }
                            let id = segments[0];
                            if id.is_empty() {
                                return Some("Automation preset id is required.".to_string());
                            }
                            let tasks_json = match serde_json::from_str::<Value>(segments[1]) {
                                Ok(value) => value,
                                Err(err) => return Some(format!("Invalid tasks_json: {}", err)),
                            };
                            let tasks = match App::normalize_automation_tasks(&tasks_json) {
                                Ok(items) => items,
                                Err(err) => return Some(err),
                            };
                            let (required, optional) = if segments.len() > 2 {
                                App::parse_required_optional_segments(&segments[2..].join(" :: "))
                            } else {
                                (Vec::new(), Vec::new())
                            };
                            let capability_tasks = tasks
                                .iter()
                                .map(|task| {
                                    json!({
                                        "required": task.get("required").cloned().unwrap_or_else(|| json!([])),
                                        "optional": task.get("optional").cloned().unwrap_or_else(|| json!([])),
                                    })
                                })
                                .collect::<Vec<_>>();
                            let summary_request = json!({
                                "agent": {
                                    "required": required,
                                    "optional": optional,
                                },
                                "tasks": capability_tasks,
                            });
                            let summary_payload =
                                match client.presets_capability_summary(summary_request).await {
                                    Ok(payload) => payload,
                                    Err(err) => {
                                        return Some(format!("Automation summary failed: {}", err));
                                    }
                                };
                            let summary = summary_payload
                                .get("summary")
                                .cloned()
                                .unwrap_or_else(|| json!({}));
                            let yaml = App::automation_override_yaml(id, &tasks, &summary);
                            match client
                                .presets_override_put("automation_preset", id, &yaml)
                                .await
                            {
                                Ok(payload) => format!(
                                    "Saved automation preset override `{}` with {} task(s).\n{}",
                                    id,
                                    tasks.len(),
                                    serde_json::to_string_pretty(&payload)
                                        .unwrap_or_else(|_| "{}".to_string())
                                ),
                                Err(err) => format!("Automation override save failed: {}", err),
                            }
                        }
                        _ => "Usage: /preset automation <summary|save> ...".to_string(),
                    }
                }
                _ => "Usage: /preset <index|agent|automation> ...".to_string(),
            }
        }),
        "context_runs" => Some({
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let limit = args
                .first()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(20);
            match client.context_runs_list().await {
                Ok(mut runs) => {
                    if runs.is_empty() {
                        return Some("No context runs found.".to_string());
                    }
                    runs.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
                    let lines = runs
                        .into_iter()
                        .take(limit)
                        .map(|run| {
                            format!(
                                "- {} [{}] type={} steps={} updated_at={}\n  objective: {}",
                                run.run_id,
                                format!("{:?}", run.status).to_lowercase(),
                                run.run_type,
                                run.steps.len(),
                                run.updated_at_ms,
                                run.objective
                            )
                        })
                        .collect::<Vec<_>>();
                    format!("Context runs:\n{}", lines.join("\n"))
                }
                Err(err) => format!("Failed to list context runs: {}", err),
            }
        }),
        "context_run_create" => Some({
            if args.is_empty() {
                return Some("Usage: /context_run_create <objective...>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let objective = args.join(" ");
            match client
                .context_run_create(None, objective, Some("interactive".to_string()), None)
                .await
            {
                Ok(run) => format!("Created context run {} [{}].", run.run_id, run.run_type),
                Err(err) => format!("Failed to create context run: {}", err),
            }
        }),
        "context_run_get" => Some({
            if args.len() != 1 {
                return Some("Usage: /context_run_get <run_id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            match client.context_run_get(run_id).await {
                Ok(detail) => {
                    let run = detail.run;
                    let rollback_preview_steps = detail
                        .rollback_preview_summary
                        .get("step_count")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0);
                    let rollback_history_entries = detail
                        .rollback_history_summary
                        .get("entry_count")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0);
                    let rollback_policy_eligible = detail
                        .rollback_policy
                        .get("eligible")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false);
                    let rollback_required_ack = detail
                        .rollback_policy
                        .get("required_policy_ack")
                        .and_then(|value| value.as_str())
                        .unwrap_or("<none>");
                    let last_rollback_outcome = detail
                        .last_rollback_outcome
                        .get("outcome")
                        .and_then(|value| value.as_str())
                        .unwrap_or("<none>");
                    let last_rollback_reason = detail
                        .last_rollback_outcome
                        .get("reason")
                        .and_then(|value| value.as_str())
                        .unwrap_or("<none>");
                    format!(
                        "Context run {}\n  status: {}\n  type: {}\n  revision: {}\n  workspace: {}\n  steps: {}\n  why_next_step: {}\n  objective: {}\n\nRollback\n  preview_steps: {}\n  history_entries: {}\n  policy: {}\n  required_ack: {}\n  last_outcome: {}\n  last_reason: {}\n\nNext\n  /context_run_rollback_preview {}\n  /context_run_rollback_history {}",
                        run.run_id,
                        format!("{:?}", run.status).to_lowercase(),
                        run.run_type,
                        run.revision,
                        run.workspace.canonical_path,
                        run.steps.len(),
                        run.why_next_step.unwrap_or_else(|| "<none>".to_string()),
                        run.objective,
                        rollback_preview_steps,
                        rollback_history_entries,
                        if rollback_policy_eligible {
                            "eligible"
                        } else {
                            "blocked"
                        },
                        rollback_required_ack,
                        last_rollback_outcome,
                        last_rollback_reason,
                        run.run_id,
                        run.run_id
                    )
                }
                Err(err) => format!("Failed to load context run: {}", err),
            }
        }),
        "context_run_rollback_preview" => Some({
            if args.len() != 1 {
                return Some("Usage: /context_run_rollback_preview <run_id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            match client.context_run_rollback_preview(run_id).await {
                Ok(preview) => {
                    if preview.steps.is_empty() {
                        return Some(format!(
                            "No rollback preview steps for context run {}.",
                            run_id
                        ));
                    }
                    let lines = preview
                        .steps
                        .iter()
                        .take(12)
                        .map(|step| {
                            format!(
                                "  - [{}] seq={} ops={} tool={} event={}",
                                if step.executable { "exec" } else { "info" },
                                step.seq,
                                step.operation_count,
                                step.tool.as_deref().unwrap_or("<unknown>"),
                                step.event_id
                            )
                        })
                        .collect::<Vec<_>>();
                    let executable_ids = preview
                        .steps
                        .iter()
                        .filter(|step| step.executable)
                        .map(|step| step.event_id.clone())
                        .collect::<Vec<_>>();
                    let executable_id_lines = if executable_ids.is_empty() {
                        "  <none>".to_string()
                    } else {
                        executable_ids
                            .iter()
                            .map(|event_id| format!("  {}", event_id))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    let next = if executable_ids.is_empty() {
                        "  No executable rollback steps are available yet.".to_string()
                    } else {
                        format!(
                            "  /context_run_rollback_execute {} --ack {}\n  /context_run_rollback_execute_all {} --ack",
                            run_id,
                            executable_ids.join(" "),
                            run_id
                        )
                    };
                    format!(
                        "Rollback preview ({})\n  step_count: {}\n  executable_steps: {}\n  advisory_steps: {}\n  fully_executable: {}\n\nExecutable ids\n{}\n\nSteps\n{}\n\nNext\n{}",
                        run_id,
                        preview.step_count,
                        preview.executable_step_count,
                        preview.advisory_step_count,
                        preview.executable,
                        executable_id_lines,
                        lines.join("\n"),
                        next
                    )
                }
                Err(err) => format!("Failed to load rollback preview: {}", err),
            }
        }),
        "context_run_rollback_execute" => Some({
            if args.len() < 3 || args[1] != "--ack" {
                return Some(
                    "Usage: /context_run_rollback_execute <run_id> --ack <event_id...>".to_string(),
                );
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let event_ids = args[2..]
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            if event_ids.is_empty() {
                return Some("Provide at least one rollback preview event id.".to_string());
            }
            match client
                .context_run_rollback_execute(
                    run_id,
                    event_ids.clone(),
                    Some("allow_rollback_execution".to_string()),
                )
                .await
            {
                Ok(result) => {
                    let missing = result
                        .missing_event_ids
                        .as_ref()
                        .filter(|rows| !rows.is_empty())
                        .map(|rows| rows.join(", "))
                        .unwrap_or_else(|| "<none>".to_string());
                    format!(
                        "Rollback execute ({})\n  applied: {}\n  selected: {}\n  applied_steps: {}\n  applied_operations: {}\n  missing: {}\n  reason: {}\n\nNext\n  /context_run_rollback_history {}\n  /context_run_rollback_preview {}",
                        run_id,
                        result.applied,
                        if result.selected_event_ids.is_empty() {
                            event_ids.join(", ")
                        } else {
                            result.selected_event_ids.join(", ")
                        },
                        result.applied_step_count.unwrap_or(0),
                        result.applied_operation_count.unwrap_or(0),
                        missing,
                        result.reason.unwrap_or_else(|| "<none>".to_string()),
                        run_id,
                        run_id
                    )
                }
                Err(err) => format!("Failed to execute rollback: {}", err),
            }
        }),
        "context_run_rollback_execute_all" => Some({
            if args.len() != 2 || args[1] != "--ack" {
                return Some("Usage: /context_run_rollback_execute_all <run_id> --ack".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let preview = match client.context_run_rollback_preview(run_id).await {
                Ok(preview) => preview,
                Err(err) => return Some(format!("Failed to load rollback preview: {}", err)),
            };
            let event_ids = preview
                .steps
                .iter()
                .filter(|step| step.executable)
                .map(|step| step.event_id.clone())
                .collect::<Vec<_>>();
            if event_ids.is_empty() {
                return Some(format!(
                    "No executable rollback preview steps for context run {}.",
                    run_id
                ));
            }
            match client
                .context_run_rollback_execute(
                    run_id,
                    event_ids.clone(),
                    Some("allow_rollback_execution".to_string()),
                )
                .await
            {
                Ok(result) => {
                    let missing = result
                        .missing_event_ids
                        .as_ref()
                        .filter(|rows| !rows.is_empty())
                        .map(|rows| rows.join(", "))
                        .unwrap_or_else(|| "<none>".to_string());
                    let selected = if result.selected_event_ids.is_empty() {
                        event_ids.join(", ")
                    } else {
                        result.selected_event_ids.join(", ")
                    };
                    format!(
                        "Rollback execute all ({})\n  applied: {}\n  selected: {}\n  applied_steps: {}\n  applied_operations: {}\n  missing: {}\n  reason: {}\n\nNext\n  /context_run_rollback_history {}\n  /context_run_rollback_preview {}",
                        run_id,
                        result.applied,
                        selected,
                        result.applied_step_count.unwrap_or(0),
                        result.applied_operation_count.unwrap_or(0),
                        missing,
                        result.reason.unwrap_or_else(|| "<none>".to_string()),
                        run_id,
                        run_id
                    )
                }
                Err(err) => format!("Failed to execute rollback: {}", err),
            }
        }),
        "context_run_rollback_history" => Some({
            if args.len() != 1 {
                return Some("Usage: /context_run_rollback_history <run_id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            match client.context_run_rollback_history(run_id).await {
                Ok(history) => {
                    if history.entries.is_empty() {
                        return Some(format!("No rollback receipts for context run {}.", run_id));
                    }
                    let entry_count = history.entries.len();
                    let applied_count = history
                        .entries
                        .iter()
                        .filter(|entry| entry.outcome == "applied")
                        .count();
                    let blocked_count = history
                        .entries
                        .iter()
                        .filter(|entry| entry.outcome != "applied")
                        .count();
                    let lines = history
                        .entries
                        .iter()
                        .rev()
                        .take(6)
                        .map(|entry| {
                            let selected = if entry.selected_event_ids.is_empty() {
                                "<none>".to_string()
                            } else {
                                entry.selected_event_ids.join(", ")
                            };
                            let missing = entry
                                .missing_event_ids
                                .as_ref()
                                .filter(|rows| !rows.is_empty())
                                .map(|rows| rows.join(", "))
                                .unwrap_or_else(|| "<none>".to_string());
                            let actions = entry
                                .applied_by_action
                                .as_ref()
                                .filter(|counts| !counts.is_empty())
                                .map(|counts| {
                                    let mut rows = counts
                                        .iter()
                                        .map(|(action, count)| format!("{}={}", action, count))
                                        .collect::<Vec<_>>();
                                    rows.sort();
                                    rows.join(", ")
                                })
                                .unwrap_or_else(|| "<none>".to_string());
                            format!(
                                "  - seq={} outcome={} ts={}\n    selected: {}\n    missing: {}\n    steps: {}\n    operations: {}\n    actions: {}\n    reason: {}",
                                entry.seq,
                                entry.outcome,
                                entry.ts_ms,
                                selected,
                                missing,
                                entry.applied_step_count.unwrap_or(0),
                                entry.applied_operation_count.unwrap_or(0),
                                actions,
                                entry.reason.as_deref().unwrap_or("<none>")
                            )
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "Rollback receipts ({})\n  entries: {}\n  applied: {}\n  blocked: {}\n\nRecent receipts\n{}",
                        run_id,
                        entry_count,
                        applied_count,
                        blocked_count,
                        lines.join("\n")
                    )
                }
                Err(err) => format!("Failed to load rollback receipts: {}", err),
            }
        }),
        "context_run_events" => Some({
            if args.is_empty() {
                return Some("Usage: /context_run_events <run_id> [tail]".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let tail = if args.len() > 1 {
                match args[1].parse::<usize>() {
                    Ok(value) if value > 0 => Some(value),
                    _ => return Some("tail must be a positive integer.".to_string()),
                }
            } else {
                Some(20)
            };
            match client.context_run_events(run_id, None, tail).await {
                Ok(events) => {
                    if events.is_empty() {
                        return Some(format!("No events for context run {}.", run_id));
                    }
                    let lines = events
                        .iter()
                        .map(|event| {
                            format!(
                                "- #{} {} status={} step={} ts={}",
                                event.seq,
                                event.event_type,
                                format!("{:?}", event.status).to_lowercase(),
                                event.step_id.as_deref().unwrap_or("-"),
                                event.ts_ms
                            )
                        })
                        .collect::<Vec<_>>();
                    format!("Context run events ({}):\n{}", run_id, lines.join("\n"))
                }
                Err(err) => format!("Failed to load context run events: {}", err),
            }
        }),
        "context_run_pause" | "context_run_resume" | "context_run_cancel" => Some({
            if args.len() != 1 {
                return Some(format!("Usage: /{} <run_id>", cmd_name));
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let (event_type, status, label) = match cmd_name {
                "context_run_pause" => (
                    "run_paused",
                    crate::net::client::ContextRunStatus::Paused,
                    "paused",
                ),
                "context_run_resume" => (
                    "run_resumed",
                    crate::net::client::ContextRunStatus::Running,
                    "running",
                ),
                _ => (
                    "run_cancelled",
                    crate::net::client::ContextRunStatus::Cancelled,
                    "cancelled",
                ),
            };
            match client
                .context_run_append_event(
                    run_id,
                    event_type,
                    status,
                    None,
                    json!({ "source": "tui" }),
                )
                .await
            {
                Ok(event) => format!(
                    "Context run {} {} (seq={} event={}).",
                    run_id, label, event.seq, event.event_id
                ),
                Err(err) => format!("Failed to update context run status: {}", err),
            }
        }),
        "context_run_blackboard" => Some({
            if args.len() != 1 {
                return Some("Usage: /context_run_blackboard <run_id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            match client.context_run_blackboard(run_id).await {
                Ok(blackboard) => format!(
                    "Context blackboard {}\n  revision: {}\n  facts: {}\n  decisions: {}\n  open_questions: {}\n  artifacts: {}\n  rolling_summary: {}\n  latest_context_pack: {}",
                    run_id,
                    blackboard.revision,
                    blackboard.facts.len(),
                    blackboard.decisions.len(),
                    blackboard.open_questions.len(),
                    blackboard.artifacts.len(),
                    if blackboard.summaries.rolling.is_empty() { "<empty>" } else { "<present>" },
                    if blackboard.summaries.latest_context_pack.is_empty() { "<empty>" } else { "<present>" }
                ),
                Err(err) => format!("Failed to load context run blackboard: {}", err),
            }
        }),
        "context_run_next" => Some({
            if args.is_empty() {
                return Some("Usage: /context_run_next <run_id> [dry_run]".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let dry_run = args
                .get(1)
                .map(|value| {
                    matches!(
                        value.to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "dry"
                    )
                })
                .unwrap_or(false);
            match client.context_run_driver_next(run_id, dry_run).await {
                Ok(next) => format!(
                    "ContextDriver next ({})\n  run: {}\n  dry_run: {}\n  target_status: {}\n  selected_step: {}\n  why_next_step: {}",
                    if dry_run { "preview" } else { "applied" },
                    next.run_id,
                    next.dry_run,
                    format!("{:?}", next.target_status).to_lowercase(),
                    next.selected_step_id.unwrap_or_else(|| "<none>".to_string()),
                    next.why_next_step
                ),
                Err(err) => format!("Failed to run ContextDriver next-step selection: {}", err),
            }
        }),
        "context_run_replay" => Some({
            if args.is_empty() {
                return Some("Usage: /context_run_replay <run_id> [upto_seq]".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let upto_seq = if args.len() > 1 {
                match args[1].parse::<u64>() {
                    Ok(value) if value > 0 => Some(value),
                    _ => return Some("upto_seq must be a positive integer.".to_string()),
                }
            } else {
                None
            };
            match client.context_run_replay(run_id, upto_seq, Some(true)).await {
                Ok(replay) => format!(
                    "Context replay {}\n  from_checkpoint: {} (seq={})\n  events_applied: {}\n  replay_status: {}\n  persisted_status: {}\n  drift: {} (status={}, why={}, steps={})",
                    replay.run_id,
                    replay.from_checkpoint,
                    replay
                        .checkpoint_seq
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    replay.events_applied,
                    format!("{:?}", replay.replay.status).to_lowercase(),
                    format!("{:?}", replay.persisted.status).to_lowercase(),
                    replay.drift.mismatch,
                    replay.drift.status_mismatch,
                    replay.drift.why_next_step_mismatch,
                    replay.drift.step_count_mismatch
                ),
                Err(err) => format!("Failed to replay context run: {}", err),
            }
        }),
        "context_run_lineage" => Some({
            if args.is_empty() {
                return Some("Usage: /context_run_lineage <run_id> [tail]".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let tail = if args.len() > 1 {
                match args[1].parse::<usize>() {
                    Ok(value) if value > 0 => Some(value),
                    _ => return Some("tail must be a positive integer.".to_string()),
                }
            } else {
                Some(100)
            };
            match client.context_run_events(run_id, None, tail).await {
                Ok(events) => {
                    let decisions = events
                        .iter()
                        .filter(|event| event.event_type == "meta_next_step_selected")
                        .collect::<Vec<_>>();
                    if decisions.is_empty() {
                        return Some(format!(
                            "No decision lineage events for context run {}.",
                            run_id
                        ));
                    }
                    let lines = decisions
                        .iter()
                        .map(|event| {
                            let why = event
                                .payload
                                .get("why_next_step")
                                .and_then(Value::as_str)
                                .unwrap_or("<missing>");
                            let selected = event
                                .payload
                                .get("selected_step_id")
                                .and_then(Value::as_str)
                                .or_else(|| event.step_id.as_deref())
                                .unwrap_or("-");
                            format!(
                                "- #{} ts={} status={} step={} why={}",
                                event.seq,
                                event.ts_ms,
                                format!("{:?}", event.status).to_lowercase(),
                                selected,
                                why
                            )
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "Context decision lineage ({}):\n{}",
                        run_id,
                        lines.join("\n")
                    )
                }
                Err(err) => format!("Failed to load context run lineage: {}", err),
            }
        }),
        "context_run_sync_tasks" => Some({
            if args.len() != 1 {
                return Some("Usage: /context_run_sync_tasks <run_id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let run_id = args[0];
            let (source_session_id, source_run_id, todos) = match &app.state {
                AppState::Chat {
                    session_id,
                    agents,
                    active_agent_index,
                    tasks,
                    ..
                } => {
                    let mapped = plan_helpers::context_todo_items_from_tasks(tasks);
                    let run_ref = agents
                        .get(*active_agent_index)
                        .and_then(|agent| agent.active_run_id.clone());
                    (Some(session_id.clone()), run_ref, mapped)
                }
                _ => (None, None, Vec::new()),
            };
            if todos.is_empty() {
                return Some("No tasks available to sync.".to_string());
            }
            match client
                .context_run_sync_todos(run_id, todos, true, source_session_id, source_run_id)
                .await
            {
                Ok(run) => format!(
                    "Synced tasks into context run {}.\n  steps: {}\n  status: {}\n  why_next_step: {}",
                    run.run_id,
                    run.steps.len(),
                    format!("{:?}", run.status).to_lowercase(),
                    run.why_next_step.unwrap_or_else(|| "<none>".to_string())
                ),
                Err(err) => format!("Failed to sync tasks into context run: {}", err),
            }
        }),
        "context_run_bind" => Some({
            if args.len() != 1 {
                return Some("Usage: /context_run_bind <run_id|off>".to_string());
            }
            let target = args[0];
            if let AppState::Chat {
                agents,
                active_agent_index,
                ..
            } = &mut app.state
            {
                let Some(agent) = agents.get_mut(*active_agent_index) else {
                    return Some("No active agent.".to_string());
                };
                if target.eq_ignore_ascii_case("off") || target == "-" {
                    agent.bound_context_run_id = None;
                    return Some(format!(
                        "Cleared context-run binding for {}.",
                        agent.agent_id
                    ));
                }
                agent.bound_context_run_id = Some(target.to_string());
                format!(
                    "Bound {} todowrite updates to context run {}.",
                    agent.agent_id, target
                )
            } else {
                "Context-run binding is available in chat mode only.".to_string()
            }
        }),
        "routines" => Some({
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            match client.routines_list().await {
                Ok(routines) => {
                    if routines.is_empty() {
                        return Some("No routines configured.".to_string());
                    }
                    let lines = routines
                        .into_iter()
                        .map(|routine| {
                            let schedule = match routine.schedule {
                                crate::net::client::RoutineSchedule::IntervalSeconds {
                                    seconds,
                                } => format!("interval:{}s", seconds),
                                crate::net::client::RoutineSchedule::Cron { expression } => {
                                    format!("cron:{expression}")
                                }
                            };
                            format!(
                                "- {} [{}] {} ({})",
                                routine.routine_id, routine.name, schedule, routine.entrypoint
                            )
                        })
                        .collect::<Vec<_>>();
                    format!("Routines:\n{}", lines.join("\n"))
                }
                Err(err) => format!("Failed to list routines: {}", err),
            }
        }),
        "routine_create" => Some({
            if args.len() < 3 {
                return Some(
                    "Usage: /routine_create <id> <interval_seconds> <entrypoint>".to_string(),
                );
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let routine_id = args[0].to_string();
            let interval_seconds = match args[1].parse::<u64>() {
                Ok(seconds) if seconds > 0 => seconds,
                _ => return Some("interval_seconds must be a positive integer.".to_string()),
            };
            let entrypoint = args[2..].join(" ");
            let request = crate::net::client::RoutineCreateRequest {
                routine_id: Some(routine_id.clone()),
                name: routine_id.clone(),
                schedule: crate::net::client::RoutineSchedule::IntervalSeconds {
                    seconds: interval_seconds,
                },
                timezone: None,
                misfire_policy: Some(crate::net::client::RoutineMisfirePolicy::RunOnce),
                entrypoint: entrypoint.clone(),
                args: Some(serde_json::json!({})),
                allowed_tools: None,
                output_targets: None,
                creator_type: Some("user".to_string()),
                creator_id: Some("tui".to_string()),
                requires_approval: Some(true),
                external_integrations_allowed: Some(false),
                next_fire_at_ms: None,
            };
            match client.routines_create(request).await {
                Ok(routine) => format!(
                    "Created routine {} ({}s -> {}).",
                    routine.routine_id, interval_seconds, routine.entrypoint
                ),
                Err(err) => format!("Failed to create routine: {}", err),
            }
        }),
        "routine_edit" => Some({
            if args.len() != 2 {
                return Some("Usage: /routine_edit <id> <interval_seconds>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let routine_id = args[0];
            let interval_seconds = match args[1].parse::<u64>() {
                Ok(seconds) if seconds > 0 => seconds,
                _ => return Some("interval_seconds must be a positive integer.".to_string()),
            };
            let request = crate::net::client::RoutinePatchRequest {
                schedule: Some(crate::net::client::RoutineSchedule::IntervalSeconds {
                    seconds: interval_seconds,
                }),
                ..Default::default()
            };
            match client.routines_patch(routine_id, request).await {
                Ok(_) => format!(
                    "Updated routine {} schedule to every {}s.",
                    routine_id, interval_seconds
                ),
                Err(err) => format!("Failed to edit routine: {}", err),
            }
        }),
        "routine_pause" => Some({
            if args.len() != 1 {
                return Some("Usage: /routine_pause <id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let routine_id = args[0];
            let request = crate::net::client::RoutinePatchRequest {
                status: Some(crate::net::client::RoutineStatus::Paused),
                ..Default::default()
            };
            match client.routines_patch(routine_id, request).await {
                Ok(_) => format!("Paused routine {}.", routine_id),
                Err(err) => format!("Failed to pause routine: {}", err),
            }
        }),
        "routine_resume" => Some({
            if args.len() != 1 {
                return Some("Usage: /routine_resume <id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let routine_id = args[0];
            let request = crate::net::client::RoutinePatchRequest {
                status: Some(crate::net::client::RoutineStatus::Active),
                ..Default::default()
            };
            match client.routines_patch(routine_id, request).await {
                Ok(_) => format!("Resumed routine {}.", routine_id),
                Err(err) => format!("Failed to resume routine: {}", err),
            }
        }),
        "routine_run_now" => Some({
            if args.is_empty() {
                return Some("Usage: /routine_run_now <id> [run_count]".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let routine_id = args[0];
            let run_count = if args.len() > 1 {
                match args[1].parse::<u32>() {
                    Ok(count) if count > 0 => Some(count),
                    _ => return Some("run_count must be a positive integer.".to_string()),
                }
            } else {
                None
            };
            let request = crate::net::client::RoutineRunNowRequest {
                run_count,
                reason: Some("manual_tui".to_string()),
            };
            match client.routines_run_now(routine_id, request).await {
                Ok(resp) => format!(
                    "Triggered routine {} (run_count={}).",
                    resp.routine_id, resp.run_count
                ),
                Err(err) => format!("Failed to trigger routine: {}", err),
            }
        }),
        "routine_delete" => Some({
            if args.len() != 1 {
                return Some("Usage: /routine_delete <id>".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let routine_id = args[0];
            match client.routines_delete(routine_id).await {
                Ok(true) => format!("Deleted routine {}.", routine_id),
                Ok(false) => format!("Routine not found: {}", routine_id),
                Err(err) => format!("Failed to delete routine: {}", err),
            }
        }),
        "routine_history" => Some({
            if args.is_empty() {
                return Some("Usage: /routine_history <id> [limit]".to_string());
            }
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let routine_id = args[0];
            let limit = if args.len() > 1 {
                match args[1].parse::<usize>() {
                    Ok(value) => Some(value),
                    Err(_) => return Some("limit must be a positive integer.".to_string()),
                }
            } else {
                Some(10)
            };
            match client.routines_history(routine_id, limit).await {
                Ok(events) => {
                    if events.is_empty() {
                        return Some(format!("No history for routine {}.", routine_id));
                    }
                    let lines = events
                        .iter()
                        .map(|event| {
                            format!(
                                "- {} run_count={} status={} at={}",
                                event.trigger_type,
                                event.run_count,
                                event.status,
                                event.fired_at_ms
                            )
                        })
                        .collect::<Vec<_>>();
                    format!("Routine history ({}):\n{}", routine_id, lines.join("\n"))
                }
                Err(err) => format!("Failed to load routine history: {}", err),
            }
        }),
        "config" => Some({
            let lines = vec![
                format!(
                    "Engine URL: {}",
                    app.client
                        .as_ref()
                        .map(|c| c.base_url())
                        .unwrap_or(&"not connected")
                ),
                format!("Sessions: {}", app.sessions.len()),
                format!("Current Mode: {:?}", app.current_mode),
                format!(
                    "Current Provider: {}",
                    app.current_provider.as_deref().unwrap_or("none")
                ),
                format!(
                    "Current Model: {}",
                    app.current_model.as_deref().unwrap_or("none")
                ),
            ];
            format!("Configuration:\n{}", lines.join("\n"))
        }),
        "requests" => Some({
            if let AppState::Chat {
                pending_requests,
                modal,
                request_cursor,
                ..
            } = &mut app.state
            {
                if pending_requests.is_empty() {
                    "No pending requests.".to_string()
                } else {
                    if *request_cursor >= pending_requests.len() {
                        *request_cursor = pending_requests.len().saturating_sub(1);
                    }
                    *modal = Some(ModalState::RequestCenter);
                    format!(
                        "Opened request center ({} pending).",
                        pending_requests.len()
                    )
                }
            } else {
                "Requests are only available in chat mode.".to_string()
            }
        }),
        "copy" => Some({
            if let AppState::Chat { messages, .. } = &app.state {
                match app.copy_latest_assistant_to_clipboard(messages) {
                    Ok(len) => format!("Copied {} characters to clipboard.", len),
                    Err(err) => format!("Clipboard copy failed: {}", err),
                }
            } else {
                "Clipboard copy works in chat screens only.".to_string()
            }
        }),
        "approve" | "deny" | "answer" => Some({
            let Some(client) = &app.client else {
                return Some("Engine client not connected.".to_string());
            };
            let session_id = if let AppState::Chat { session_id, .. } = &app.state {
                Some(session_id.clone())
            } else {
                None
            };

            match cmd_name {
                "approve" => {
                    if args
                        .first()
                        .map(|s| s.eq_ignore_ascii_case("all"))
                        .unwrap_or(false)
                        || args.is_empty()
                    {
                        let Ok(snapshot) = client.list_permissions().await else {
                            return Some("Failed to load pending permissions.".to_string());
                        };
                        let pending: Vec<String> = snapshot
                            .requests
                            .iter()
                            .filter(|r| r.status.as_deref() == Some("pending"))
                            .filter(|r| {
                                if let Some(sid) = &session_id {
                                    r.session_id.as_deref() == Some(sid.as_str())
                                } else {
                                    true
                                }
                            })
                            .map(|r| r.id.clone())
                            .collect();
                        if pending.is_empty() {
                            return Some("No pending permissions.".to_string());
                        }
                        let mut approved = 0usize;
                        for id in pending {
                            if client.reply_permission(&id, "allow").await.unwrap_or(false) {
                                approved += 1;
                            }
                        }
                        format!("Approved {} pending permission request(s).", approved)
                    } else {
                        let id = args[0];
                        let reply = if args
                            .get(1)
                            .map(|s| s.eq_ignore_ascii_case("always"))
                            .unwrap_or(false)
                        {
                            "always"
                        } else {
                            "allow"
                        };
                        if client.reply_permission(id, reply).await.unwrap_or(false) {
                            format!("Approved permission request {}.", id)
                        } else {
                            format!("Permission request not found: {}", id)
                        }
                    }
                }
                "deny" => {
                    if args.is_empty() {
                        return Some("Usage: /deny <id>".to_string());
                    }
                    let id = args[0];
                    if client.reply_permission(id, "deny").await.unwrap_or(false) {
                        format!("Denied permission request {}.", id)
                    } else {
                        format!("Permission request not found: {}", id)
                    }
                }
                "answer" => {
                    if args.is_empty() {
                        return Some("Usage: /answer <id> <text>".to_string());
                    }
                    let id = args[0];
                    let reply = if args.len() > 1 {
                        args[1..].join(" ")
                    } else {
                        "allow".to_string()
                    };
                    if client
                        .reply_permission(id, reply.as_str())
                        .await
                        .unwrap_or(false)
                    {
                        format!("Replied to permission request {}.", id)
                    } else {
                        format!("Permission request not found: {}", id)
                    }
                }
                _ => "Unsupported permission command.".to_string(),
            }
        }),
        "mode" => Some(if args.is_empty() {
            let agent = app.current_mode.as_agent();
            format!("Current mode: {:?} (agent: {})", app.current_mode, agent)
        } else {
            let mode_name = args[0];
            if let Some(mode) = TandemMode::from_str(mode_name) {
                app.current_mode = mode;
                format!("Mode set to: {:?}", mode)
            } else {
                format!(
                    "Unknown mode: {}. Use /modes to see available modes.",
                    mode_name
                )
            }
        }),
        "modes" => Some({
            let lines: Vec<String> = TandemMode::all_modes()
                .iter()
                .map(|(name, desc)| format!("  {} - {}", name, desc))
                .collect();
            format!("Available modes:\n{}", lines.join("\n"))
        }),
        "providers" => Some(if let Some(catalog) = &app.provider_catalog {
            let lines: Vec<String> = catalog
                .all
                .iter()
                .map(|p| {
                    let status = if catalog.connected.contains(&p.id) {
                        "connected"
                    } else {
                        "not configured"
                    };
                    format!("  {} - {}", p.id, status)
                })
                .collect();
            if lines.is_empty() {
                "No providers available.".to_string()
            } else {
                format!("Available providers:\n{}", lines.join("\n"))
            }
        } else {
            "Loading providers... (use /providers to refresh)".to_string()
        }),
        "provider" => Some({
            let mut step = SetupStep::SelectProvider;
            let mut selected_provider_index = 0;
            let filter_model = String::new();

            if !args.is_empty() {
                let provider_id = args[0];
                if let Some(catalog) = &app.provider_catalog {
                    if let Some(idx) = catalog.all.iter().position(|p| p.id == provider_id) {
                        selected_provider_index = idx;
                        step = if catalog.connected.contains(&provider_id.to_string()) {
                            SetupStep::SelectModel
                        } else {
                            SetupStep::EnterApiKey
                        };
                    }
                }
            } else if let Some(current) = &app.current_provider {
                if let Some(catalog) = &app.provider_catalog {
                    if let Some(idx) = catalog.all.iter().position(|p| &p.id == current) {
                        selected_provider_index = idx;
                        step = if catalog.connected.contains(current) {
                            SetupStep::SelectModel
                        } else {
                            SetupStep::EnterApiKey
                        };
                    }
                }
            }

            app.state = AppState::SetupWizard {
                step,
                provider_catalog: app.provider_catalog.clone(),
                selected_provider_index,
                selected_model_index: 0,
                api_key_input: String::new(),
                model_input: filter_model,
            };
            "Opening provider selection...".to_string()
        }),
        "models" => Some({
            let provider_id = args
                .first()
                .map(|s| s.to_string())
                .or_else(|| app.current_provider.clone());
            if let Some(catalog) = &app.provider_catalog {
                if let Some(pid) = &provider_id {
                    if let Some(provider) = catalog.all.iter().find(|p| p.id == *pid) {
                        let model_ids: Vec<String> = provider.models.keys().cloned().collect();
                        if model_ids.is_empty() {
                            format!("No models available for provider: {}", pid)
                        } else {
                            format!(
                                "Models for {}:\n{}",
                                pid,
                                model_ids
                                    .iter()
                                    .map(|m| format!("  {}", m))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            )
                        }
                    } else {
                        format!("Provider not found: {}", pid)
                    }
                } else {
                    "No provider selected. Use /provider <id> first.".to_string()
                }
            } else {
                "Loading providers... (use /providers to refresh)".to_string()
            }
        }),
        "model" => Some(if args.is_empty() {
            let mut selected_provider_index = 0;
            if let Some(current) = &app.current_provider {
                if let Some(catalog) = &app.provider_catalog {
                    if let Some(idx) = catalog.all.iter().position(|p| &p.id == current) {
                        selected_provider_index = idx;
                    }
                }
            }
            app.state = AppState::SetupWizard {
                step: SetupStep::SelectModel,
                provider_catalog: app.provider_catalog.clone(),
                selected_provider_index,
                selected_model_index: 0,
                api_key_input: String::new(),
                model_input: String::new(),
            };
            "Opening model selection...".to_string()
        } else {
            let model_id = args.join(" ");
            app.current_model = Some(model_id.clone());
            app.pending_model_provider = None;
            if let Some(provider_id) = app.current_provider.clone() {
                app.persist_provider_defaults(&provider_id, Some(&model_id), None)
                    .await;
            }
            format!("Model set to: {}", model_id)
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        ChatMessage, ComposerInputState, ModalState, PendingPermissionRequest, PendingRequest,
        PendingRequestKind, PlanFeedbackWizardState, Task, UiMode,
    };
    use crate::crypto::keystore::SecureKeyStore;
    use crate::net::client::EngineClient;
    use crate::net::client::{ProviderCatalog, Session, SessionTime};
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;
    use tandem_wire::{WireProviderEntry, WireProviderModel};

    #[tokio::test]
    async fn rollback_commands_render_engine_responses() {
        let server = MockServer::start(HashMap::from([
            (
                "/context/runs/run-1/checkpoints/mutations/rollback-preview".to_string(),
                json_response(
                    r#"{"steps":[{"seq":3,"event_id":"evt-1","tool":"edit_file","executable":true,"operation_count":2},{"seq":4,"event_id":"evt-2","tool":"read_file","executable":false,"operation_count":1}],"step_count":2,"executable_step_count":1,"advisory_step_count":1,"executable":false}"#,
                ),
            ),
            (
                "/context/runs/run-1/checkpoints/mutations/rollback-history".to_string(),
                json_response(
                    r#"{"entries":[{"seq":7,"ts_ms":200,"event_id":"evt-rollback-2","outcome":"blocked","selected_event_ids":["evt-1"],"applied_step_count":0,"applied_operation_count":0,"reason":"approval required"},{"seq":6,"ts_ms":100,"event_id":"evt-rollback-1","outcome":"applied","selected_event_ids":["evt-1"],"applied_step_count":1,"applied_operation_count":2,"applied_by_action":{"rewrite_file":2}}],"summary":{"entry_count":2,"by_outcome":{"applied":1,"blocked":1}}}"#,
                ),
            ),
            (
                "/context/runs/run-1/checkpoints/mutations/rollback-execute".to_string(),
                json_response(
                    r#"{"applied":true,"selected_event_ids":["evt-1"],"applied_step_count":1,"applied_operation_count":2,"missing_event_ids":[],"reason":null}"#,
                ),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let preview = app
            .execute_command("/context_run_rollback_preview run-1")
            .await;
        assert!(preview.contains("Rollback preview (run-1)"));
        assert!(preview.contains("evt-1"));

        let history = app
            .execute_command("/context_run_rollback_history run-1")
            .await;
        assert!(history.contains("Rollback receipts (run-1)"));
        assert!(history.contains("outcome=applied"));
        assert!(history.contains("outcome=blocked"));

        let execute = app
            .execute_command("/context_run_rollback_execute run-1 --ack evt-1")
            .await;
        assert!(execute.contains("Rollback execute (run-1)"));
        assert!(execute.contains("selected: evt-1"));
    }

    #[tokio::test]
    async fn recent_command_helper_lists_replays_and_clears() {
        let mut app = App::new();

        let mode = app.execute_command("/mode coder").await;
        assert!(mode.contains("Mode set to: Coder"));

        let workspace = app.execute_command("/workspace show").await;
        assert!(workspace.contains("Current workspace directory:"));

        let recent = app.execute_command("/recent").await;
        assert!(recent.contains("1. /workspace show"));
        assert!(recent.contains("2. /mode coder"));

        let replay = app.execute_command("/recent run 2").await;
        assert!(replay.contains("Replayed recent command #2: /mode coder"));
        assert!(replay.contains("Mode set to: Coder"));

        let cleared = app.execute_command("/recent clear").await;
        assert_eq!(cleared, "Cleared 2 recent command(s).");
        assert_eq!(
            app.execute_command("/recent").await,
            "No recent slash commands yet."
        );
    }

    #[tokio::test]
    async fn session_commands_list_and_switch_sessions() {
        let mut app = App::new();
        app.sessions = vec![
            Session {
                id: "s-1".to_string(),
                title: "First".to_string(),
                directory: None,
                workspace_root: None,
                time: Some(SessionTime {
                    created: Some(1),
                    updated: Some(2),
                }),
            },
            Session {
                id: "s-2".to_string(),
                title: "Second".to_string(),
                directory: None,
                workspace_root: None,
                time: Some(SessionTime {
                    created: Some(3),
                    updated: Some(4),
                }),
            },
        ];
        app.selected_session_index = 1;
        app.state = chat_state("s-1");

        let sessions = app.execute_command("/sessions").await;
        assert!(sessions.contains("→ Second (ID: s-2)"));
        assert!(sessions.contains("  First (ID: s-1)"));

        let switched = app.execute_command("/use s-2").await;
        assert_eq!(switched, "Switched to session: s-2");
        assert_eq!(app.selected_session_index, 1);
        match &app.state {
            AppState::Chat {
                session_id,
                active_agent_index,
                agents,
                ..
            } => {
                assert_eq!(session_id, "s-2");
                assert_eq!(agents[*active_agent_index].session_id, "s-2");
            }
            _ => panic!("expected chat state"),
        }
    }

    #[tokio::test]
    async fn key_commands_list_keys_and_open_wizard() {
        let mut app = App::new();
        let path =
            std::env::temp_dir().join(format!("tandem-tui-keystore-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut keystore = SecureKeyStore::load(&path, vec![7; 32]).expect("keystore");
        keystore
            .set("openai_api_key", "secret".to_string())
            .expect("set key");
        app.keystore = Some(keystore);
        app.current_provider = Some("openai".to_string());
        app.provider_catalog = Some(ProviderCatalog {
            all: vec![WireProviderEntry {
                id: "openai".to_string(),
                name: Some("OpenAI".to_string()),
                models: HashMap::<String, WireProviderModel>::new(),
                catalog_source: None,
                catalog_status: None,
                catalog_message: None,
            }],
            connected: vec!["openai".to_string()],
            default: Some("openai".to_string()),
        });

        let keys = app.execute_command("/keys").await;
        assert!(keys.contains("Configured providers:"));
        assert!(keys.contains("openai - configured"));

        let wizard = app.execute_command("/key set").await;
        assert_eq!(wizard, "Opening key setup wizard for openai...");
        match &app.state {
            AppState::SetupWizard { step, .. } => assert_eq!(*step, SetupStep::EnterApiKey),
            _ => panic!("expected setup wizard"),
        }

        let _ = std::fs::remove_file(PathBuf::from(path));
    }

    #[tokio::test]
    async fn queue_commands_manage_followups_and_errors() {
        let mut app = App::new();
        app.state = chat_state("s-1");
        if let AppState::Chat {
            agents, messages, ..
        } = &mut app.state
        {
            agents[0]
                .follow_up_queue
                .push_back("first follow-up".to_string());
            agents[0].steering_message = Some("steer".to_string());
            messages.push(ChatMessage {
                role: MessageRole::System,
                content: vec![ContentBlock::Text("Something failed badly".to_string())],
            });
        }

        let queue = app.execute_command("/queue").await;
        assert!(queue.contains("steering: yes"));
        assert!(queue.contains("follow-ups: 1"));
        assert!(queue.contains("first follow-up"));

        let error = app.execute_command("/last_error").await;
        assert_eq!(error, "Something failed badly");

        let cleared = app.execute_command("/queue clear").await;
        assert_eq!(cleared, "Cleared queued steering and follow-up messages.");

        let messages = app.execute_command("/messages 25").await;
        assert_eq!(messages, "Message history not implemented yet. (limit: 25)");
    }

    #[tokio::test]
    async fn steer_followup_and_cancel_commands_update_active_agent_state() {
        let mut app = App::new();
        app.state = chat_state("s-1");
        if let AppState::Chat { agents, .. } = &mut app.state {
            agents[0].status = AgentStatus::Running;
            agents[0].active_run_id = Some("run-1".to_string());
        }

        let steer = app.execute_command("/steer check logs").await;
        assert_eq!(steer, "Steering message queued.");
        match &app.state {
            AppState::Chat { command_input, .. } => assert_eq!(command_input.text(), "check logs"),
            _ => panic!("expected chat state"),
        }

        let followup = app.execute_command("/followup inspect rollback").await;
        assert_eq!(followup, "Queued follow-up message (#1).");
        match &app.state {
            AppState::Chat { agents, .. } => {
                assert_eq!(
                    agents[0].follow_up_queue.front().map(String::as_str),
                    Some("inspect rollback")
                );
            }
            _ => panic!("expected chat state"),
        }

        let cancel = app.execute_command("/cancel").await;
        assert_eq!(cancel, "Cancel requested for active agent.");
        match &app.state {
            AppState::Chat { agents, .. } => {
                assert_eq!(agents[0].status, AgentStatus::Idle);
                assert_eq!(agents[0].active_run_id, None);
            }
            _ => panic!("expected chat state"),
        }
    }

    #[tokio::test]
    async fn task_and_prompt_commands_update_chat_state() {
        let mut app = App::new();
        app.state = chat_state("s-1");

        let added = app.execute_command("/task add investigate rollback").await;
        assert_eq!(added, "Task added: investigate rollback (ID: task-1)");

        let pinned = app.execute_command("/task pin task-1").await;
        assert_eq!(pinned, "Task task-1 pinned: true");

        let worked = app.execute_command("/task work task-1").await;
        assert_eq!(worked, "Task task-1 marked as work");

        let listed = app.execute_command("/task list").await;
        assert!(listed.contains("[task-1] investigate rollback (Working) - Pinned: true"));

        let prompt = app.execute_command("/prompt review status").await;
        assert_eq!(prompt, "Prompt sent.");
        match &app.state {
            AppState::Chat {
                messages, agents, ..
            } => {
                assert!(messages
                    .iter()
                    .any(|m| m.content.iter().any(|block| match block {
                        ContentBlock::Text(text) => text == "review status",
                        _ => false,
                    })));
                assert!(agents[0].messages.iter().any(|m| m.content.iter().any(
                    |block| match block {
                        ContentBlock::Text(text) => text == "review status",
                        _ => false,
                    }
                )));
            }
            _ => panic!("expected chat state"),
        }
    }

    #[tokio::test]
    async fn title_command_renames_current_session() {
        let server = MockServer::start(HashMap::from([(
            "/session/s-1".to_string(),
            json_response(
                r#"{"id":"s-1","title":"Renamed Session","directory":null,"workspaceRoot":null,"time":{"created":1,"updated":2}}"#,
            ),
        )]))
        .expect("mock server");
        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));
        app.sessions = vec![Session {
            id: "s-1".to_string(),
            title: "Original Session".to_string(),
            directory: None,
            workspace_root: None,
            time: Some(SessionTime {
                created: Some(1),
                updated: Some(2),
            }),
        }];
        app.state = chat_state("s-1");

        let renamed = app.execute_command("/title Renamed Session").await;
        assert_eq!(renamed, "Session renamed to: Renamed Session");
        assert_eq!(app.sessions[0].title, "Renamed Session");
    }

    #[tokio::test]
    async fn mission_commands_render_list_detail_and_create_views() {
        let mission =
            mission_state_json("m-1", "running", "Stabilize rollback", "Ship safer undo", 3);
        let created = mission_state_json("m-2", "draft", "Fresh mission", "Start clean", 1);
        let server = MockServer::start(HashMap::from([
            (
                "/mission".to_string(),
                json_response(
                    &serde_json::json!({
                        "missions": [serde_json::from_str::<serde_json::Value>(&mission).expect("mission")]
                    })
                    .to_string(),
                ),
            ),
            (
                "/mission/m-1".to_string(),
                json_response(
                    &serde_json::json!({
                        "mission": serde_json::from_str::<serde_json::Value>(&mission).expect("mission detail")
                    })
                    .to_string(),
                ),
            ),
        ]))
        .expect("mock server");
        let create_server = MockServer::start(HashMap::from([(
            "/mission".to_string(),
            json_response(
                &serde_json::json!({
                    "mission": serde_json::from_str::<serde_json::Value>(&created).expect("created mission")
                })
                .to_string(),
            ),
        )]))
        .expect("create mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let list = app.execute_command("/missions").await;
        assert!(list.contains("Missions:"));
        assert!(list.contains("m-1 [running] Stabilize rollback (work_items=1)"));

        let detail = app.execute_command("/mission_get m-1").await;
        assert!(detail.contains("Mission m-1 [running]"));
        assert!(detail.contains("Title: Stabilize rollback"));
        assert!(detail.contains("Goal: Ship safer undo"));
        assert!(detail.contains("- Verify rollback [review]"));

        app.client = Some(EngineClient::new(create_server.base_url()));
        let created = app
            .execute_command("/mission_create Fresh mission :: Start clean :: Draft task")
            .await;
        assert_eq!(created, "Created mission m-2: Fresh mission");
    }

    #[tokio::test]
    async fn mission_event_commands_apply_expected_engine_events() {
        let mission =
            mission_state_json("m-1", "running", "Stabilize rollback", "Ship safer undo", 5);
        let server = MockServer::start(HashMap::from([(
            "/mission/m-1/event".to_string(),
            json_response(
                &serde_json::json!({
                    "mission": serde_json::from_str::<serde_json::Value>(&mission).expect("mission event result"),
                    "commands": [{ "type": "notify" }]
                })
                .to_string(),
            ),
        )]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let invalid = app.execute_command("/mission_event m-1 nope").await;
        assert!(invalid.starts_with("Invalid event JSON:"));

        let applied = app
            .execute_command(r#"/mission_event m-1 {"type":"custom","state":"ok"}"#)
            .await;
        assert_eq!(
            applied,
            "Applied event to mission m-1 (revision=5, commands=1)"
        );

        let started = app.execute_command("/mission_start m-1").await;
        assert_eq!(started, "Mission started m-1 (revision=5)");

        let review_ok = app
            .execute_command("/mission_review_ok m-1 w-1 gate-7")
            .await;
        assert_eq!(review_ok, "Review approved for m-1:w-1 (revision=5)");

        let test_ok = app.execute_command("/mission_test_ok m-1 w-1").await;
        assert_eq!(test_ok, "Test approved for m-1:w-1 (revision=5)");

        let review_no = app
            .execute_command("/mission_review_no m-1 w-1 needs more logs")
            .await;
        assert_eq!(review_no, "Review denied for m-1:w-1 (revision=5)");
    }

    #[tokio::test]
    async fn agent_team_commands_render_summary_and_list_views() {
        let server = MockServer::start(HashMap::from([
            (
                "/agent-team/missions".to_string(),
                json_response(
                    r#"{"missions":[{"missionID":"mission-1","instanceCount":3,"runningCount":1,"completedCount":1,"failedCount":0,"cancelledCount":1}]}"#,
                ),
            ),
            (
                "/agent-team/instances".to_string(),
                json_response(
                    r#"{"instances":[{"instanceID":"agent-1","role":"reviewer","missionID":"mission-1","sessionID":"s-1","status":"running","parentInstanceID":"lead-1"}]}"#,
                ),
            ),
            (
                "/agent-team/approvals".to_string(),
                json_response(
                    r#"{"spawnApprovals":[{"approvalID":"spawn-1","createdAtMs":1,"request":{"missionID":"mission-1","reason":"Need helper"}}],"toolApprovals":[{"approvalID":"tool-1","sessionID":"s-1","toolCallID":"call-1","tool":"shell","status":"pending"}]}"#,
                ),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let summary = app.execute_command("/agent-team").await;
        assert!(summary.contains("Agent-Team Summary:"));
        assert!(summary.contains("Missions: 1"));
        assert!(summary.contains("Instances: 1"));
        assert!(summary.contains("Spawn approvals: 1"));
        assert!(summary.contains("Tool approvals: 1"));

        let missions = app.execute_command("/agent-team missions").await;
        assert!(missions.contains("Agent-Team Missions:"));
        assert!(missions.contains("mission-1 total=3 running=1 done=1 failed=0 cancelled=1"));

        let instances = app.execute_command("/agent-team instances mission-1").await;
        assert!(instances.contains("Agent-Team Instances:"));
        assert!(instances
            .contains("agent-1 role=reviewer mission=mission-1 status=running parent=lead-1"));

        let approvals = app.execute_command("/agent-team approvals").await;
        assert!(approvals.contains("Agent-Team Approvals:"));
        assert!(approvals.contains("spawn approval spawn-1"));
        assert!(approvals.contains("tool approval tool-1 (shell)"));
    }

    #[tokio::test]
    async fn agent_team_commands_handle_bindings_and_permission_replies() {
        let server = MockServer::start(HashMap::from([
            (
                "/agent-team/approvals/spawn/spawn-1/approve".to_string(),
                json_response(r#"{"ok":true}"#),
            ),
            (
                "/agent-team/approvals/spawn/spawn-1/deny".to_string(),
                json_response(r#"{"ok":true}"#),
            ),
            (
                "/permission/tool-1/reply".to_string(),
                json_response(r#"{"ok":true}"#),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let bindings = app.execute_command("/agent-team bindings").await;
        assert!(
            bindings == "No local agent-team state found."
                || bindings == "No local agent-team bindings found."
        );

        let approve_spawn = app
            .execute_command("/agent-team approve spawn spawn-1 looks good")
            .await;
        assert_eq!(approve_spawn, "Approved spawn approval spawn-1.");

        let approve_tool = app.execute_command("/agent-team approve tool tool-1").await;
        assert_eq!(approve_tool, "Approved tool request tool-1.");

        let deny_spawn = app.execute_command("/agent-team deny spawn spawn-1").await;
        assert_eq!(deny_spawn, "Denied spawn approval spawn-1.");

        let deny_tool = app.execute_command("/agent-team deny tool tool-1").await;
        assert_eq!(deny_tool, "Denied tool request tool-1.");
    }

    #[tokio::test]
    async fn agent_commands_manage_agent_panes() {
        let server = MockServer::start(HashMap::from([(
            "/api/session".to_string(),
            json_response(
                r#"{"id":"s-2","title":"A2 session","directory":null,"workspaceRoot":null,"time":{"created":1,"updated":2}}"#,
            ),
        )]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));
        app.state = chat_state("s-1");

        let created = app.execute_command("/agent new").await;
        assert_eq!(created, "Created new agent.");

        let listed = app.execute_command("/agent list").await;
        assert!(listed.contains("Agents:"));
        assert!(listed.contains("> A2 [s-2] Idle"));
        assert!(listed.contains("  A1 [s-1] Idle"));

        let switched = app.execute_command("/agent use A1").await;
        assert_eq!(switched, "Switched to A1.");

        let closed = app.execute_command("/agent close").await;
        assert_eq!(closed, "Closed active agent.");
        match &app.state {
            AppState::Chat {
                agents,
                active_agent_index,
                ..
            } => {
                assert_eq!(agents.len(), 1);
                assert_eq!(*active_agent_index, 0);
                assert_eq!(agents[0].agent_id, "A2");
            }
            _ => panic!("expected chat state"),
        }
    }

    #[tokio::test]
    async fn agent_fanout_creates_grid_and_switches_mode() {
        let mut app = App::new();
        app.state = chat_state("s-1");
        app.current_mode = TandemMode::Plan;

        let result = app.execute_command("/agent fanout 3").await;
        assert_eq!(
            result,
            "Started fanout: 3 total agents (created 2). Grid view enabled. Mode auto-switched from plan -> orchestrate."
        );
        assert_eq!(app.current_mode, TandemMode::Orchestrate);
        match &app.state {
            AppState::Chat {
                agents,
                ui_mode,
                grid_page,
                ..
            } => {
                assert_eq!(agents.len(), 3);
                assert_eq!(*ui_mode, UiMode::Grid);
                assert_eq!(*grid_page, 0);
                assert_eq!(agents[1].agent_id, "A2");
                assert_eq!(agents[2].agent_id, "A3");
            }
            _ => panic!("expected chat state"),
        }
    }

    #[tokio::test]
    async fn preset_commands_render_index_and_agent_views() {
        let server = MockServer::start(HashMap::from([
            (
                "/presets/index".to_string(),
                json_response(
                    r#"{"index":{"skill_modules":[{"id":"skill.a","version":"1","kind":"skill_module","layer":"base","path":"skills/a","tags":[],"publisher":null,"required_capabilities":[]}],"agent_presets":[{"id":"agent.main","version":"1","kind":"agent_preset","layer":"base","path":"agents/main","tags":[],"publisher":null,"required_capabilities":[]}],"automation_presets":[],"pack_presets":[],"generated_at_ms":42}}"#,
                ),
            ),
            (
                "/presets/compose/preview".to_string(),
                json_response(r#"{"composition":{"prompt":"merged preset prompt"}}"#),
            ),
            (
                "/presets/capability_summary".to_string(),
                json_response(r#"{"summary":{"required":["shell"],"optional":["git"]}}"#),
            ),
            (
                "/presets/fork".to_string(),
                json_response(r#"{"id":"agent-copy","kind":"agent_preset","layer":"override"}"#),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let index = app.execute_command("/preset index").await;
        assert!(index.contains("Preset index:"));
        assert!(index.contains("skill_modules: 1"));
        assert!(index.contains("agent_presets: 1"));
        assert!(index.contains("generated_at_ms: 42"));

        let compose = app
            .execute_command(r#"/preset agent compose Base prompt :: [{"id":"frag-1","phase":"plan","content":"think"}]"#)
            .await;
        assert!(compose.contains("Agent compose preview:"));
        assert!(compose.contains("merged preset prompt"));

        let summary = app
            .execute_command("/preset agent summary required=shell optional=git")
            .await;
        assert!(summary.contains("Agent capability summary:"));
        assert!(summary.contains("\"required\""));

        let fork = app
            .execute_command("/preset agent fork presets/base.yaml agent-copy")
            .await;
        assert!(fork.contains("Forked agent preset override:"));
        assert!(fork.contains("agent-copy"));
    }

    #[tokio::test]
    async fn preset_automation_commands_validate_and_save() {
        let server = MockServer::start(HashMap::from([
            (
                "/presets/capability_summary".to_string(),
                json_response(r#"{"summary":{"score":"ok","required":["shell"]}}"#),
            ),
            (
                "/presets/overrides/automation_preset/nightly".to_string(),
                json_response(r#"{"ok":true,"path":"automation_preset/nightly"}"#),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let invalid = app
            .execute_command("/preset agent compose Base prompt :: {\"bad\":true}")
            .await;
        assert_eq!(
            invalid,
            "fragments_json must be a JSON array of {id,phase,content}"
        );

        let summary = app
            .execute_command(
                r#"/preset automation summary [{"required":["shell"],"optional":["git"]}] :: required=python :: optional=gh"#,
            )
            .await;
        assert!(summary.contains("Automation capability summary (1 tasks):"));
        assert!(summary.contains("\"score\""));

        let saved = app
            .execute_command(
                r#"/preset automation save nightly :: [{"required":["shell"],"optional":["git"]}] :: required=python :: optional=gh"#,
            )
            .await;
        assert!(saved.contains("Saved automation preset override `nightly` with 1 task(s)."));
        assert!(saved.contains("automation_preset/nightly"));
    }

    #[tokio::test]
    async fn routine_commands_validate_usage_and_engine_requirements() {
        let mut app = App::new();

        assert_eq!(
            app.execute_command("/routines").await,
            "Engine client not connected."
        );
        assert_eq!(
            app.execute_command("/routine_create").await,
            "Usage: /routine_create <id> <interval_seconds> <entrypoint>"
        );
        assert_eq!(
            app.execute_command("/routine_edit nightly").await,
            "Usage: /routine_edit <id> <interval_seconds>"
        );
        assert_eq!(
            app.execute_command("/routine_run_now").await,
            "Usage: /routine_run_now <id> [run_count]"
        );
        assert_eq!(
            app.execute_command("/routine_history").await,
            "Usage: /routine_history <id> [limit]"
        );
        assert_eq!(
            app.execute_command("/routine_create nightly 60 plan nightly")
                .await,
            "Engine client not connected."
        );
        assert_eq!(
            app.execute_command("/routine_delete nightly").await,
            "Engine client not connected."
        );

        app.client = Some(EngineClient::new("http://127.0.0.1:1".to_string()));

        assert_eq!(
            app.execute_command("/routine_create nightly nope plan nightly")
                .await,
            "interval_seconds must be a positive integer."
        );
        assert_eq!(
            app.execute_command("/routine_edit nightly nope").await,
            "interval_seconds must be a positive integer."
        );
        assert_eq!(
            app.execute_command("/routine_run_now nightly nope").await,
            "run_count must be a positive integer."
        );
        assert_eq!(
            app.execute_command("/routine_history nightly nope").await,
            "limit must be a positive integer."
        );
    }

    #[tokio::test]
    async fn config_requests_and_copy_commands_use_expected_state() {
        let mut app = App::new();
        app.current_provider = Some("openai".to_string());
        app.current_model = Some("gpt-4.1".to_string());

        let config = app.execute_command("/config").await;
        assert!(config.contains("Configuration:"));
        assert!(config.contains("Current Provider: openai"));
        assert!(config.contains("Current Model: gpt-4.1"));

        let copy = app.execute_command("/copy").await;
        assert_eq!(copy, "Clipboard copy works in chat screens only.");

        app.state = chat_state("s-1");
        if let AppState::Chat {
            pending_requests,
            request_cursor,
            ..
        } = &mut app.state
        {
            pending_requests.push(PendingRequest {
                session_id: "s-1".to_string(),
                agent_id: "A1".to_string(),
                kind: PendingRequestKind::Permission(PendingPermissionRequest {
                    id: "perm-1".to_string(),
                    tool: "shell".to_string(),
                    args: None,
                    args_source: None,
                    args_integrity: None,
                    query: Some("ls".to_string()),
                    status: Some("pending".to_string()),
                }),
            });
            *request_cursor = 99;
        }

        let requests = app.execute_command("/requests").await;
        assert_eq!(requests, "Opened request center (1 pending).");
        match &app.state {
            AppState::Chat {
                modal,
                request_cursor,
                ..
            } => {
                assert_eq!(modal, &Some(ModalState::RequestCenter));
                assert_eq!(*request_cursor, 0);
            }
            _ => panic!("expected chat state"),
        }
    }

    #[tokio::test]
    async fn permission_commands_reply_and_filter_pending_requests() {
        let server = MockServer::start(HashMap::from([
            (
                "/permission".to_string(),
                json_response(
                    r#"{"requests":[{"id":"perm-1","sessionID":"s-1","status":"pending"},{"id":"perm-2","sessionID":"s-2","status":"pending"},{"id":"perm-3","sessionID":"s-1","status":"approved"}],"rules":[]}"#,
                ),
            ),
            (
                "/permission/perm-1/reply".to_string(),
                json_response(r#"{"ok":true}"#),
            ),
            (
                "/permission/perm-9/reply".to_string(),
                json_response(r#"{"ok":true}"#),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));
        app.state = chat_state("s-1");

        let approve_all = app.execute_command("/approve all").await;
        assert_eq!(approve_all, "Approved 1 pending permission request(s).");

        let approve_one = app.execute_command("/approve perm-9 always").await;
        assert_eq!(approve_one, "Approved permission request perm-9.");

        let deny = app.execute_command("/deny perm-9").await;
        assert_eq!(deny, "Denied permission request perm-9.");

        let answer = app.execute_command("/answer perm-9 once").await;
        assert_eq!(answer, "Replied to permission request perm-9.");
    }

    #[tokio::test]
    async fn context_run_commands_render_list_detail_and_driver_views() {
        let run_one = context_run_state_json("run-1", "running", "Investigate rollback", 200);
        let run_two = context_run_state_json("run-2", "paused", "Review logs", 100);
        let replay_run = context_run_state_json("run-1", "running", "Investigate rollback", 200);
        let persisted_run = context_run_state_json("run-1", "paused", "Investigate rollback", 210);
        let next_run = context_run_state_json("run-1", "running", "Investigate rollback", 220);
        let server = MockServer::start(HashMap::from([
            (
                "/context/runs".to_string(),
                json_response(
                    &serde_json::json!({
                        "runs": [
                            serde_json::from_str::<serde_json::Value>(&run_two).expect("run two"),
                            serde_json::from_str::<serde_json::Value>(&run_one).expect("run one")
                        ]
                    })
                    .to_string(),
                ),
            ),
            (
                "/context/runs/run-1".to_string(),
                json_response(
                    &serde_json::json!({
                        "run": serde_json::from_str::<serde_json::Value>(&run_one).expect("detail run"),
                        "rollback_preview_summary": { "step_count": 2 },
                        "rollback_history_summary": { "entry_count": 1 },
                        "last_rollback_outcome": { "outcome": "applied", "reason": "manual" },
                        "rollback_policy": { "eligible": true, "required_policy_ack": "allow_rollback_execution" }
                    })
                    .to_string(),
                ),
            ),
            (
                "/context/runs/run-1/events".to_string(),
                json_response(
                    &serde_json::json!({
                        "events": [
                            {
                                "event_id": "evt-2",
                                "run_id": "run-1",
                                "seq": 2,
                                "ts_ms": 220,
                                "type": "meta_next_step_selected",
                                "status": "running",
                                "step_id": "step-2",
                                "payload": {
                                    "why_next_step": "Need edit verification",
                                    "selected_step_id": "step-2"
                                }
                            },
                            {
                                "event_id": "evt-1",
                                "run_id": "run-1",
                                "seq": 1,
                                "ts_ms": 200,
                                "type": "tool_completed",
                                "status": "running",
                                "step_id": "step-1",
                                "payload": {}
                            }
                        ]
                    })
                    .to_string(),
                ),
            ),
            (
                "/context/runs/run-1/blackboard".to_string(),
                json_response(
                    &serde_json::json!({
                        "blackboard": {
                            "facts": [{ "id": "fact-1", "ts_ms": 1, "text": "Rollback ready" }],
                            "decisions": [{ "id": "decision-1", "ts_ms": 2, "text": "Pause before execute" }],
                            "open_questions": [{ "id": "question-1", "ts_ms": 3, "text": "Need approval?" }],
                            "artifacts": [{ "id": "artifact-1", "ts_ms": 4, "path": "/tmp/plan.md", "artifact_type": "note" }],
                            "summaries": { "rolling": "summary", "latest_context_pack": "pack" },
                            "revision": 9
                        }
                    })
                    .to_string(),
                ),
            ),
            (
                "/context/runs/run-1/replay".to_string(),
                json_response(
                    &serde_json::json!({
                        "ok": true,
                        "run_id": "run-1",
                        "from_checkpoint": true,
                        "checkpoint_seq": 3,
                        "events_applied": 4,
                        "replay": serde_json::from_str::<serde_json::Value>(&replay_run).expect("replay"),
                        "persisted": serde_json::from_str::<serde_json::Value>(&persisted_run).expect("persisted"),
                        "drift": {
                            "mismatch": true,
                            "status_mismatch": true,
                            "why_next_step_mismatch": false,
                            "step_count_mismatch": true
                        }
                    })
                    .to_string(),
                ),
            ),
            (
                "/context/runs/run-1/driver/next".to_string(),
                json_response(
                    &serde_json::json!({
                        "ok": true,
                        "dry_run": true,
                        "run_id": "run-1",
                        "selected_step_id": "step-2",
                        "target_status": "running",
                        "why_next_step": "Need edit verification",
                        "run": serde_json::from_str::<serde_json::Value>(&next_run).expect("next")
                    })
                    .to_string(),
                ),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let list = app.execute_command("/context_runs 1").await;
        assert!(list.contains("Context runs:"));
        assert!(list.contains("run-1 [running]"));
        assert!(!list.contains("run-2 [paused]"));

        let detail = app.execute_command("/context_run_get run-1").await;
        assert!(detail.contains("Context run run-1"));
        assert!(detail.contains("preview_steps: 2"));
        assert!(detail.contains("required_ack: allow_rollback_execution"));

        let events = app.execute_command("/context_run_events run-1 10").await;
        assert!(events.contains("Context run events (run-1):"));
        assert!(events.contains("meta_next_step_selected"));

        let blackboard = app.execute_command("/context_run_blackboard run-1").await;
        assert!(blackboard.contains("Context blackboard run-1"));
        assert!(blackboard.contains("facts: 1"));
        assert!(blackboard.contains("latest_context_pack: <present>"));

        let next = app.execute_command("/context_run_next run-1 dry").await;
        assert!(next.contains("ContextDriver next (preview)"));
        assert!(next.contains("selected_step: step-2"));

        let replay = app.execute_command("/context_run_replay run-1 3").await;
        assert!(replay.contains("Context replay run-1"));
        assert!(replay.contains("drift: true"));

        let lineage = app.execute_command("/context_run_lineage run-1 10").await;
        assert!(lineage.contains("Context decision lineage (run-1):"));
        assert!(lineage.contains("why=Need edit verification"));
    }

    #[tokio::test]
    async fn context_run_create_and_lifecycle_commands_render_engine_responses() {
        let created_run = context_run_state_json("run-1", "planning", "Investigate rollback", 50);
        let server = MockServer::start(HashMap::from([
            (
                "/context/runs".to_string(),
                json_response(
                    &serde_json::json!({
                        "run": serde_json::from_str::<serde_json::Value>(&created_run).expect("created run")
                    })
                    .to_string(),
                ),
            ),
            (
                "/context/runs/run-1/events".to_string(),
                json_response(
                    &serde_json::json!({
                        "event": {
                            "event_id": "evt-lifecycle",
                            "run_id": "run-1",
                            "seq": 7,
                            "ts_ms": 500,
                            "type": "run_updated",
                            "status": "running",
                            "step_id": null,
                            "payload": { "source": "tui" }
                        }
                    })
                    .to_string(),
                ),
            ),
        ]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));

        let created = app
            .execute_command("/context_run_create Investigate rollback")
            .await;
        assert_eq!(created, "Created context run run-1 [interactive].");

        let paused = app.execute_command("/context_run_pause run-1").await;
        assert_eq!(
            paused,
            "Context run run-1 paused (seq=7 event=evt-lifecycle)."
        );

        let resumed = app.execute_command("/context_run_resume run-1").await;
        assert_eq!(
            resumed,
            "Context run run-1 running (seq=7 event=evt-lifecycle)."
        );

        let cancelled = app.execute_command("/context_run_cancel run-1").await;
        assert_eq!(
            cancelled,
            "Context run run-1 cancelled (seq=7 event=evt-lifecycle)."
        );
    }

    #[tokio::test]
    async fn context_run_bind_and_sync_tasks_update_chat_state() {
        let synced_run = context_run_state_json("run-1", "running", "Investigate rollback", 300);
        let server = MockServer::start(HashMap::from([(
            "/context/runs/run-1/todos/sync".to_string(),
            json_response(
                &serde_json::json!({
                    "run": serde_json::from_str::<serde_json::Value>(&synced_run).expect("synced run")
                })
                .to_string(),
            ),
        )]))
        .expect("mock server");

        let mut app = App::new();
        app.client = Some(EngineClient::new(server.base_url()));
        app.state = chat_state("s-1");
        if let AppState::Chat { tasks, agents, .. } = &mut app.state {
            tasks.push(Task {
                id: "task-1".to_string(),
                description: "Investigate rollback".to_string(),
                status: TaskStatus::Working,
                pinned: true,
            });
            agents[0].active_run_id = Some("source-run".to_string());
        }

        let bound = app.execute_command("/context_run_bind run-1").await;
        assert_eq!(bound, "Bound A1 todowrite updates to context run run-1.");
        match &app.state {
            AppState::Chat { agents, .. } => {
                assert_eq!(agents[0].bound_context_run_id.as_deref(), Some("run-1"));
            }
            _ => panic!("expected chat state"),
        }

        let synced = app.execute_command("/context_run_sync_tasks run-1").await;
        assert!(synced.contains("Synced tasks into context run run-1."));
        assert!(synced.contains("status: running"));
        assert!(synced.contains("why_next_step: Need edit verification"));

        let cleared = app.execute_command("/context_run_bind off").await;
        assert_eq!(cleared, "Cleared context-run binding for A1.");
    }

    fn chat_state(session_id: &str) -> AppState {
        let agent = App::make_agent_pane("A1".to_string(), session_id.to_string());
        AppState::Chat {
            session_id: session_id.to_string(),
            command_input: ComposerInputState::new(),
            messages: Vec::new(),
            scroll_from_bottom: 0,
            tasks: Vec::<Task>::new(),
            active_task_id: None,
            agents: vec![agent],
            active_agent_index: 0,
            ui_mode: UiMode::Focus,
            grid_page: 0,
            modal: Option::<ModalState>::None,
            pending_requests: Vec::<PendingRequest>::new(),
            request_cursor: 0,
            permission_choice: 0,
            plan_wizard: PlanFeedbackWizardState::default(),
            last_plan_task_fingerprint: Vec::new(),
            plan_awaiting_approval: false,
            plan_multi_agent_prompt: None,
            plan_waiting_for_clarification_question: false,
            request_panel_expanded: false,
        }
    }

    fn context_run_state_json(
        run_id: &str,
        status: &str,
        objective: &str,
        updated_at_ms: u64,
    ) -> String {
        serde_json::json!({
            "run_id": run_id,
            "run_type": "interactive",
            "status": status,
            "objective": objective,
            "workspace": {
                "workspace_id": "ws-1",
                "canonical_path": "/tmp/workspace",
                "lease_epoch": 1
            },
            "steps": [
                { "step_id": "step-1", "title": "Inspect", "status": "done" },
                { "step_id": "step-2", "title": "Verify", "status": "runnable" }
            ],
            "why_next_step": "Need edit verification",
            "revision": 4,
            "created_at_ms": 10,
            "updated_at_ms": updated_at_ms
        })
        .to_string()
    }

    fn mission_state_json(
        mission_id: &str,
        status: &str,
        title: &str,
        goal: &str,
        revision: u64,
    ) -> String {
        serde_json::json!({
            "mission_id": mission_id,
            "status": status,
            "spec": {
                "mission_id": mission_id,
                "title": title,
                "goal": goal,
                "success_criteria": [],
                "entrypoint": null,
                "budgets": {},
                "capabilities": {},
                "metadata": null
            },
            "work_items": [
                {
                    "work_item_id": "w-1",
                    "title": "Verify rollback",
                    "detail": null,
                    "status": "review",
                    "depends_on": [],
                    "assigned_agent": null,
                    "run_id": null,
                    "artifact_refs": [],
                    "metadata": null
                }
            ],
            "revision": revision,
            "updated_at_ms": 100
        })
        .to_string()
    }

    struct MockServer {
        addr: std::net::SocketAddr,
        running: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }

    impl MockServer {
        fn start(routes: HashMap<String, String>) -> anyhow::Result<Self> {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            listener.set_nonblocking(true)?;
            let addr = listener.local_addr()?;
            let running = Arc::new(AtomicBool::new(true));
            let worker_running = Arc::clone(&running);
            let worker = std::thread::spawn(move || {
                while worker_running.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = handle_request(stream, &routes);
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });
            Ok(Self {
                addr,
                running,
                worker: Some(worker),
            })
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.addr)
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            self.running.store(false, Ordering::SeqCst);
            let _ = TcpStream::connect(self.addr);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    fn json_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    fn handle_request(
        mut stream: TcpStream,
        routes: &HashMap<String, String>,
    ) -> anyhow::Result<()> {
        stream.set_read_timeout(Some(Duration::from_millis(250)))?;
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or_default();
        let raw_path = first_line.split_whitespace().nth(1).unwrap_or("/");
        let path = raw_path.split('?').next().unwrap_or(raw_path);
        let response = routes.get(path).cloned().unwrap_or_else(|| {
            json_response(r#"{"error":"not found"}"#).replacen("200 OK", "404 Not Found", 1)
        });
        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        Ok(())
    }
}
