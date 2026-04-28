use super::*;

impl EngineLoop {
    pub(super) async fn execute_tool_with_timeout(
        &self,
        tool: &str,
        args: Value,
        cancel: CancellationToken,
        progress: Option<SharedToolProgressSink>,
    ) -> anyhow::Result<tandem_types::ToolResult> {
        let timeout_ms = tool_exec_timeout_ms() as u64;
        match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            self.tools
                .execute_with_cancel_and_progress(tool, args, cancel, progress),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => anyhow::bail!("TOOL_EXEC_TIMEOUT_MS_EXCEEDED({timeout_ms})"),
        }
    }

    pub(super) async fn find_recent_matching_user_message_id(
        &self,
        session_id: &str,
        text: &str,
    ) -> Option<String> {
        let session = self.storage.get_session(session_id).await?;
        let last = session.messages.last()?;
        if !matches!(last.role, MessageRole::User) {
            return None;
        }
        let age_ms = (Utc::now() - last.created_at).num_milliseconds().max(0) as u64;
        if age_ms > 10_000 {
            return None;
        }
        let last_text = last
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if last_text == text {
            return Some(last.id.clone());
        }
        None
    }

    pub(super) async fn auto_rename_session_from_user_text(
        &self,
        session_id: &str,
        fallback_text: &str,
    ) {
        let Some(mut session) = self.storage.get_session(session_id).await else {
            return;
        };
        if !title_needs_repair(&session.title) {
            return;
        }

        let first_user_text = session.messages.iter().find_map(|message| {
            if !matches!(message.role, MessageRole::User) {
                return None;
            }
            message.parts.iter().find_map(|part| match part {
                MessagePart::Text { text } if !text.trim().is_empty() => Some(text.clone()),
                _ => None,
            })
        });

        let source = first_user_text.unwrap_or_else(|| fallback_text.to_string());
        let Some(title) = derive_session_title_from_prompt(&source, 60) else {
            return;
        };

        session.title = title;
        session.time.updated = Utc::now();
        let _ = self.storage.save_session(session).await;
    }

    pub(super) async fn workspace_sandbox_violation(
        &self,
        session_id: &str,
        tool: &str,
        args: &Value,
    ) -> Option<String> {
        if self.workspace_override_active(session_id).await {
            return None;
        }
        if is_mcp_tool_name(tool) {
            if let Some(server) = mcp_server_from_tool_name(tool) {
                if is_mcp_sandbox_exempt_server(server) {
                    return None;
                }
            }
            let candidate_paths = extract_tool_candidate_paths(tool, args);
            if candidate_paths.is_empty() {
                return None;
            }
            let session = self.storage.get_session(session_id).await?;
            let workspace = session
                .workspace_root
                .or_else(|| crate::normalize_workspace_path(&session.directory))?;
            let workspace_path = PathBuf::from(&workspace);
            if let Some(sensitive) = candidate_paths.iter().find(|path| {
                let raw = Path::new(path);
                let resolved = if raw.is_absolute() {
                    raw.to_path_buf()
                } else {
                    workspace_path.join(raw)
                };
                is_sensitive_path_candidate(&resolved)
            }) {
                return Some(format!(
                    "Sandbox blocked MCP tool `{tool}` path `{sensitive}` (sensitive path policy)."
                ));
            }
            let outside = candidate_paths.iter().find(|path| {
                let raw = Path::new(path);
                let resolved = if raw.is_absolute() {
                    raw.to_path_buf()
                } else {
                    workspace_path.join(raw)
                };
                !crate::is_within_workspace_root(&resolved, &workspace_path)
            })?;
            return Some(format!(
                "Sandbox blocked MCP tool `{tool}` path `{outside}` (workspace root: `{workspace}`)"
            ));
        }
        let session = self.storage.get_session(session_id).await?;
        let workspace = session
            .workspace_root
            .or_else(|| crate::normalize_workspace_path(&session.directory))?;
        let workspace_path = PathBuf::from(&workspace);
        let candidate_paths = extract_tool_candidate_paths(tool, args);
        if candidate_paths.is_empty() {
            if is_shell_tool_name(tool) {
                if let Some(command) = extract_shell_command(args) {
                    if shell_command_targets_sensitive_path(&command) {
                        return Some(format!(
                            "Sandbox blocked `{tool}` command targeting sensitive paths."
                        ));
                    }
                }
            }
            return None;
        }
        if let Some(sensitive) = candidate_paths.iter().find(|path| {
            let raw = Path::new(path);
            let resolved = if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                workspace_path.join(raw)
            };
            is_sensitive_path_candidate(&resolved)
        }) {
            return Some(format!(
                "Sandbox blocked `{tool}` path `{sensitive}` (sensitive path policy)."
            ));
        }

        let outside = candidate_paths.iter().find(|path| {
            let raw = Path::new(path);
            let resolved = if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                workspace_path.join(raw)
            };
            !crate::is_within_workspace_root(&resolved, &workspace_path)
        })?;
        Some(format!(
            "Sandbox blocked `{tool}` path `{outside}` (workspace root: `{workspace}`)"
        ))
    }

    pub(super) async fn session_write_policy_violation(
        &self,
        session_id: &str,
        tool: &str,
        args: &Value,
    ) -> Option<String> {
        let policy = self.get_session_write_policy(session_id).await?;
        if matches!(policy.mode, SessionWritePolicyMode::RepoEdit) {
            return None;
        }

        let targets = extract_session_write_target_paths(tool, args);
        if targets.is_empty() {
            if tool_requires_concrete_write_target(tool, args) {
                return Some(format!(
                    "Write policy blocked `{tool}` because this session only allows declared output targets."
                ));
            }
            return None;
        }

        let session = self.storage.get_session(session_id).await?;
        let workspace = session
            .workspace_root
            .or_else(|| crate::normalize_workspace_path(&session.directory))?;
        let workspace_path = normalize_path_lexical(Path::new(&workspace));
        let effective_cwd = string_field(args, "__effective_cwd")
            .map(PathBuf::from)
            .or_else(|| {
                let directory = session.directory.trim();
                if directory.is_empty() || directory == "." {
                    None
                } else {
                    Some(PathBuf::from(directory))
                }
            })
            .unwrap_or_else(|| workspace_path.clone());
        let allowed_paths = policy
            .allowed_paths
            .iter()
            .map(|path| resolve_policy_path(path, &workspace_path, &workspace_path))
            .collect::<HashSet<_>>();
        if allowed_paths.is_empty() {
            return Some(format!(
                "Write policy blocked `{tool}` because no declared output targets are available for this session."
            ));
        }

        let outside = targets.iter().find(|target| {
            let resolved = resolve_policy_path(target, &effective_cwd, &workspace_path);
            !allowed_paths.contains(&resolved)
        });
        outside.map(|target| {
            format!(
                "Write policy blocked `{tool}` target `{target}`. This automation session may only write declared output targets."
            )
        })
    }

    pub(super) async fn resolve_tool_execution_context(
        &self,
        session_id: &str,
    ) -> Option<(String, String, Option<String>)> {
        let session = self.storage.get_session(session_id).await?;
        let workspace_root = session
            .workspace_root
            .or_else(|| crate::normalize_workspace_path(&session.directory))?;
        let effective_cwd = if session.directory.trim().is_empty()
            || session.directory.trim() == "."
        {
            workspace_root.clone()
        } else {
            crate::normalize_workspace_path(&session.directory).unwrap_or(workspace_root.clone())
        };
        let project_id = session
            .project_id
            .clone()
            .or_else(|| crate::workspace_project_id(&workspace_root));
        Some((workspace_root, effective_cwd, project_id))
    }

    pub(super) async fn mark_session_run_failed(&self, session_id: &str, error: &str) {
        let detail = truncate_text(error, 1_000);
        self.event_bus.publish(EngineEvent::new(
            "session.updated",
            json!({
                "sessionID": session_id,
                "status": "failed",
                "error": detail,
            }),
        ));
        self.event_bus.publish(EngineEvent::new(
            "session.status",
            json!({
                "sessionID": session_id,
                "status": "failed",
                "error": detail,
            }),
        ));
        self.cancellations.remove(session_id).await;
    }

    pub(super) async fn workspace_override_active(&self, session_id: &str) -> bool {
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        let mut overrides = self.workspace_overrides.write().await;
        let expired: Vec<String> = overrides
            .iter()
            .filter_map(|(id, &exp)| if exp <= now { Some(id.clone()) } else { None })
            .collect();
        overrides.retain(|_, expires_at| *expires_at > now);
        drop(overrides);
        for expired_id in expired {
            self.event_bus.publish(EngineEvent::new(
                "workspace.override.expired",
                json!({ "sessionID": expired_id }),
            ));
        }
        self.workspace_overrides
            .read()
            .await
            .get(session_id)
            .map(|expires_at| *expires_at > now)
            .unwrap_or(false)
    }

    pub(super) async fn generate_final_narrative_without_tools(
        &self,
        session_id: &str,
        active_agent: &AgentDefinition,
        provider_hint: Option<&str>,
        model_id: Option<&str>,
        cancel: CancellationToken,
        tool_outputs: &[String],
    ) -> Option<String> {
        if cancel.is_cancelled() {
            return None;
        }
        let mut messages = load_chat_history(
            self.storage.clone(),
            session_id,
            ChatHistoryProfile::Standard,
        )
        .await;
        let mut system_parts = vec![tandem_runtime_system_prompt(
            &self.host_runtime_context,
            &[],
        )];
        if let Some(system) = active_agent.system_prompt.as_ref() {
            system_parts.push(system.clone());
        }
        messages.insert(
            0,
            ChatMessage {
                role: "system".to_string(),
                content: system_parts.join("\n\n"),
                attachments: Vec::new(),
            },
        );
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: build_post_tool_final_narrative_prompt(tool_outputs),
            attachments: Vec::new(),
        });
        let stream = self
            .providers
            .stream_for_provider(
                provider_hint,
                model_id,
                messages,
                ToolMode::None,
                None,
                cancel.clone(),
            )
            .await
            .ok()?;
        tokio::pin!(stream);
        let mut completion = String::new();
        while let Some(chunk) = stream.next().await {
            if cancel.is_cancelled() {
                return None;
            }
            match chunk {
                Ok(StreamChunk::TextDelta(delta)) => {
                    let delta = strip_model_control_markers(&delta);
                    if !delta.trim().is_empty() {
                        completion.push_str(&delta);
                    }
                }
                Ok(StreamChunk::Done { .. }) => break,
                Ok(_) => {}
                Err(_) => return None,
            }
        }
        let completion = truncate_text(&strip_model_control_markers(&completion), 16_000);
        if completion.trim().is_empty() {
            None
        } else {
            Some(completion)
        }
    }
}

fn resolve_policy_path(path: &str, effective_cwd: &Path, workspace_root: &Path) -> PathBuf {
    let raw = Path::new(path);
    let resolved = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        let base = if effective_cwd.is_absolute() {
            effective_cwd.to_path_buf()
        } else {
            workspace_root.join(effective_cwd)
        };
        base.join(raw)
    };
    normalize_path_lexical(&resolved)
}

fn normalize_path_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            std::path::Component::Normal(value) => normalized.push(value),
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                normalized.push(component.as_os_str())
            }
        }
    }
    normalized
}

fn extract_session_write_target_paths(tool: &str, args: &Value) -> Vec<String> {
    let tool = normalize_tool_name(tool);
    let mut paths = match tool.as_str() {
        "write" | "edit" | "delete" | "delete_file" => string_fields(
            args,
            &[
                "path",
                "file_path",
                "filePath",
                "filepath",
                "target_path",
                "output_path",
                "file",
            ],
        ),
        "apply_patch" => args
            .get("patchText")
            .or_else(|| args.get("patch"))
            .and_then(Value::as_str)
            .map(extract_apply_patch_write_paths)
            .unwrap_or_default(),
        "bash" | "shell" => extract_bash_write_targets(
            &args
                .get("command")
                .or_else(|| args.get("cmd"))
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ),
        _ => extract_tool_candidate_paths(&tool, args),
    };
    paths.sort();
    paths.dedup();
    paths
}

fn string_field(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_fields(args: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| string_field(args, key))
        .collect::<Vec<_>>()
}

fn extract_apply_patch_write_paths(patch: &str) -> Vec<String> {
    let mut paths = HashSet::new();
    for line in patch.lines() {
        let trimmed = line.trim();
        let marker = trimmed
            .strip_prefix("*** Add File: ")
            .or_else(|| trimmed.strip_prefix("*** Update File: "))
            .or_else(|| trimmed.strip_prefix("*** Delete File: "));
        if let Some(path) = marker.map(str::trim).filter(|value| !value.is_empty()) {
            paths.insert(path.to_string());
        }
    }
    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort();
    paths
}

fn extract_bash_write_targets(command: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for part in command.split(">>").flat_map(|value| value.split('>')) {
        let candidate = part.trim().split_whitespace().next().unwrap_or("").trim();
        if candidate.starts_with('/')
            || candidate.starts_with("./")
            || candidate.starts_with("../")
            || candidate.starts_with("~/")
            || candidate.starts_with(".tandem/")
        {
            targets.push(candidate.to_string());
        }
    }
    targets.sort();
    targets.dedup();
    targets
}

fn tool_requires_concrete_write_target(tool: &str, args: &Value) -> bool {
    let tool = normalize_tool_name(tool);
    match tool.as_str() {
        "write" | "edit" | "delete" | "delete_file" | "apply_patch" => true,
        "bash" | "shell" => args
            .get("command")
            .or_else(|| args.get("cmd"))
            .and_then(Value::as_str)
            .is_some_and(shell_command_appears_mutating),
        _ => false,
    }
}

fn shell_command_appears_mutating(command: &str) -> bool {
    let lowered = command.to_ascii_lowercase();
    lowered.contains(" >")
        || lowered.contains(">>")
        || lowered.contains(" tee ")
        || lowered.starts_with("tee ")
        || lowered.contains(" sed -i")
        || lowered.starts_with("sed -i")
        || lowered.contains(" perl -pi")
        || lowered.starts_with("perl -pi")
        || lowered.contains(" rm ")
        || lowered.starts_with("rm ")
        || lowered.contains(" mv ")
        || lowered.starts_with("mv ")
        || lowered.contains(" cp ")
        || lowered.starts_with("cp ")
}

#[cfg(test)]
mod session_write_policy_tests {
    use super::*;

    #[test]
    fn write_policy_extracts_apply_patch_targets() {
        let args = json!({
            "patchText": "*** Begin Patch\n*** Update File: packages/app/src/main.ts\n@@\n old\n*** End Patch\n"
        });
        assert_eq!(
            extract_session_write_target_paths("apply_patch", &args),
            vec!["packages/app/src/main.ts".to_string()]
        );
    }

    #[test]
    fn write_policy_normalizes_equivalent_paths() {
        let workspace = Path::new("/workspace/project");
        let resolved = resolve_policy_path(
            "./.tandem/runs/run-1/../run-1/artifacts/out.md",
            workspace,
            workspace,
        );
        assert_eq!(
            resolved,
            PathBuf::from("/workspace/project/.tandem/runs/run-1/artifacts/out.md")
        );
    }

    #[test]
    fn write_policy_blocks_opaque_mutating_shell_commands() {
        assert!(tool_requires_concrete_write_target(
            "bash",
            &json!({"command": "cat <<'EOF' > packages/app/src/main.ts\nbroken\nEOF"})
        ));
        assert!(!tool_requires_concrete_write_target(
            "bash",
            &json!({"command": "rg \"needle\" packages/app/src"})
        ));
    }
}
