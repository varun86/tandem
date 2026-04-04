use super::plan_helpers;
use super::{
    Action, AgentStatus, App, AppState, ChatMessage, ContentBlock, LiveToolCall, MessageRole,
    ModalState, PendingPermissionRequest, PendingQuestionRequest, PendingRequest,
    PendingRequestKind, PlanFeedbackWizardState, QuestionDraft, TandemMode, ToolCallInfo,
};
use tandem_wire::WireSessionMessage;

impl App {
    pub(super) fn active_chat_identity(&self) -> Option<(String, String)> {
        if let AppState::Chat {
            agents,
            active_agent_index,
            ..
        } = &self.state
        {
            let agent = agents.get(*active_agent_index)?;
            return Some((agent.session_id.clone(), agent.agent_id.clone()));
        }
        None
    }

    fn request_matches_active(&self, session_id: &str, agent_id: &str) -> bool {
        self.active_chat_identity()
            .map(|(active_session, active_agent)| {
                active_session == session_id && active_agent == agent_id
            })
            .unwrap_or(false)
    }

    pub(super) fn is_agent_busy(status: &AgentStatus) -> bool {
        matches!(
            status,
            AgentStatus::Running | AgentStatus::Streaming | AgentStatus::Cancelling
        )
    }

    pub(super) fn open_request_center_if_needed(&mut self) {
        if let AppState::Chat {
            pending_requests,
            modal,
            request_cursor,
            ..
        } = &mut self.state
        {
            Self::purge_invalid_question_requests(pending_requests);
            if pending_requests.is_empty() {
                *modal = None;
                *request_cursor = 0;
                return;
            }
            if *request_cursor >= pending_requests.len() {
                *request_cursor = pending_requests.len().saturating_sub(1);
            }
            if modal.is_none() {
                *modal = Some(ModalState::RequestCenter);
            }
        }
    }

    fn purge_invalid_question_requests(pending_requests: &mut Vec<PendingRequest>) -> usize {
        let before = pending_requests.len();
        pending_requests.retain(|request| match &request.kind {
            PendingRequestKind::Permission(_) => true,
            PendingRequestKind::Question(question) => {
                !question.questions.is_empty()
                    && question
                        .questions
                        .iter()
                        .any(|q| !q.question.trim().is_empty())
            }
        });
        before.saturating_sub(pending_requests.len())
    }

    fn extract_assistant_message(messages: &[WireSessionMessage]) -> Option<Vec<ContentBlock>> {
        let message = messages
            .iter()
            .rev()
            .find(|msg| msg.info.role.eq_ignore_ascii_case("assistant"))?;

        let mut blocks = Vec::new();
        for part in &message.parts {
            let type_str = part.get("type").and_then(|v| v.as_str());
            match type_str {
                Some("text") | Some("output_text") | Some("message_text") => {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        blocks.push(ContentBlock::Text(text.to_string()));
                    } else if let Some(value) = part.get("value").and_then(|v| v.as_str()) {
                        blocks.push(ContentBlock::Text(value.to_string()));
                    } else if let Some(content) = part.get("content").and_then(|v| v.as_str()) {
                        blocks.push(ContentBlock::Text(content.to_string()));
                    }
                }
                Some("tool_use") | Some("tool_call") | Some("tool") => {
                    let id = part
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = part
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = part
                        .get("input")
                        .map(|v| v.to_string())
                        .unwrap_or("{}".to_string());
                    blocks.push(ContentBlock::ToolCall(ToolCallInfo {
                        id,
                        name,
                        args: input,
                    }));
                }
                Some(_) | None => {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        if !text.is_empty() {
                            blocks.push(ContentBlock::Text(text.to_string()));
                        }
                    }
                }
            }
        }

        if blocks.is_empty() {
            None
        } else {
            Some(blocks)
        }
    }

    fn merge_prompt_success_messages(target: &mut Vec<ChatMessage>, new_messages: &[ChatMessage]) {
        if new_messages.is_empty() {
            return;
        }
        if new_messages.len() == 1 {
            if let Some(new_text) = Self::assistant_text_of(&new_messages[0]) {
                if let Some(last) = target.last_mut() {
                    if let Some(last_text) = Self::assistant_text_of(last) {
                        let last_trimmed = last_text.trim();
                        let new_trimmed = new_text.trim();
                        if new_trimmed.is_empty() {
                            return;
                        }
                        if last_trimmed.is_empty()
                            || new_trimmed == last_trimmed
                            || new_trimmed.starts_with(last_trimmed)
                        {
                            *last = new_messages[0].clone();
                            return;
                        }
                        if last_trimmed.starts_with(new_trimmed) {
                            return;
                        }
                    }
                }
            }
        }
        target.extend_from_slice(new_messages);
    }

    fn append_assistant_delta(target: &mut Vec<ChatMessage>, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(ChatMessage {
            role: MessageRole::Assistant,
            content,
        }) = target.last_mut()
        {
            if let Some(ContentBlock::Text(existing)) = content.first_mut() {
                existing.push_str(delta);
                return;
            }
            content.push(ContentBlock::Text(delta.to_string()));
            return;
        }
        target.push(ChatMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text(delta.to_string())],
        });
    }

    fn assistant_text_of(message: &ChatMessage) -> Option<String> {
        if !matches!(message.role, MessageRole::Assistant) {
            return None;
        }
        let text = message
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub(super) fn maybe_dispatch_queued_for_agent(&mut self, session_id: &str, agent_id: &str) {
        let next_msg = if let AppState::Chat { agents, .. } = &mut self.state {
            if let Some(agent) = agents
                .iter_mut()
                .find(|a| a.session_id == session_id && a.agent_id == agent_id)
            {
                if Self::is_agent_busy(&agent.status) {
                    return;
                }
                if let Some(steering) = agent.steering_message.take() {
                    agent.follow_up_queue.clear();
                    Some(steering)
                } else {
                    agent.follow_up_queue.pop_front()
                }
            } else {
                None
            }
        } else {
            None
        };
        if let Some(next_msg) = next_msg {
            self.dispatch_prompt_for_agent(session_id.to_string(), agent_id.to_string(), next_msg);
        }
    }

    pub(super) fn dispatch_prompt_for_agent(
        &mut self,
        session_id: String,
        agent_id: String,
        msg: String,
    ) {
        let is_active_target = self.request_matches_active(&session_id, &agent_id);
        if let AppState::Chat {
            agents,
            active_agent_index,
            messages,
            scroll_from_bottom,
            ..
        } = &mut self.state
        {
            if let Some(agent) = agents
                .iter_mut()
                .find(|a| a.agent_id == agent_id && a.session_id == session_id)
            {
                agent.messages.push(ChatMessage {
                    role: MessageRole::User,
                    content: vec![ContentBlock::Text(msg.clone())],
                });
                agent.scroll_from_bottom = 0;
            }
            if is_active_target && *active_agent_index < agents.len() {
                *scroll_from_bottom = 0;
                messages.push(ChatMessage {
                    role: MessageRole::User,
                    content: vec![ContentBlock::Text(msg.clone())],
                });
            }
        }
        self.sync_active_agent_from_chat();

        if let Some(client) = &self.client {
            if let Some(tx) = &self.action_tx {
                let client = client.clone();
                let tx = tx.clone();
                let bypass_plan_wrapping = self.is_delegated_worker_agent(&session_id, &agent_id)
                    || Self::is_agent_team_assignment_prompt(&msg);
                let prompt_msg = if bypass_plan_wrapping {
                    msg.clone()
                } else {
                    self.prepare_prompt_text(&msg)
                };
                let agent = Some(self.current_mode.as_agent().to_string());
                let model = self.current_model_spec();
                let run_session_id = session_id.clone();
                let run_agent_id = agent_id.clone();
                let saw_stream_error =
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

                tokio::spawn(async move {
                    let saw_stream_error_cb = saw_stream_error.clone();
                    match client
                        .send_prompt_with_stream_events(
                            &run_session_id,
                            &prompt_msg,
                            agent.as_deref(),
                            Some(&run_agent_id),
                            model,
                            |event| {
                                if let Some(err) =
                                    crate::net::client::extract_stream_error(&event.payload)
                                {
                                    if !saw_stream_error_cb
                                        .swap(true, std::sync::atomic::Ordering::Relaxed)
                                    {
                                        let _ = tx.send(Action::PromptFailure {
                                            session_id: run_session_id.clone(),
                                            agent_id: run_agent_id.clone(),
                                            error: err,
                                        });
                                    }
                                }
                                if event.event_type == "session.run.started" {
                                    let _ = tx.send(Action::PromptRunStarted {
                                        session_id: run_session_id.clone(),
                                        agent_id: run_agent_id.clone(),
                                        run_id: event.run_id.clone(),
                                    });
                                }
                                if let Some(delta) =
                                    crate::net::client::extract_delta_text(&event.payload)
                                {
                                    let _ = tx.send(Action::PromptDelta {
                                        session_id: run_session_id.clone(),
                                        agent_id: run_agent_id.clone(),
                                        delta,
                                    });
                                }
                                if let Some(tool_delta) =
                                    crate::net::client::extract_stream_tool_delta(&event.payload)
                                {
                                    let _ = tx.send(Action::PromptToolDelta {
                                        session_id: run_session_id.clone(),
                                        agent_id: run_agent_id.clone(),
                                        tool_call_id: tool_delta.tool_call_id,
                                        tool_name: tool_delta.tool_name,
                                        args_delta: tool_delta.args_delta,
                                        args_preview: tool_delta.args_preview,
                                    });
                                }
                                if let Some(message) =
                                    crate::net::client::extract_stream_activity(&event.payload)
                                {
                                    let _ = tx.send(Action::PromptInfo {
                                        session_id: run_session_id.clone(),
                                        agent_id: run_agent_id.clone(),
                                        message,
                                    });
                                }
                                if let Some(request_event) =
                                    crate::net::client::extract_stream_request(&event.payload)
                                {
                                    let action = Self::stream_request_to_action(
                                        run_session_id.clone(),
                                        run_agent_id.clone(),
                                        request_event,
                                    );
                                    let _ = tx.send(action);
                                }
                                if let Some((event_session_id, todos)) =
                                    crate::net::client::extract_stream_todo_update(&event.payload)
                                {
                                    let _ = tx.send(Action::PromptTodoUpdated {
                                        session_id: event_session_id,
                                        todos,
                                    });
                                }
                                if let Some(agent_team_event) =
                                    crate::net::client::extract_stream_agent_team_event(
                                        &event.payload,
                                    )
                                {
                                    let _ = tx.send(Action::PromptAgentTeamEvent {
                                        session_id: run_session_id.clone(),
                                        agent_id: run_agent_id.clone(),
                                        event: agent_team_event,
                                    });
                                }
                            },
                        )
                        .await
                    {
                        Ok(run) => {
                            if saw_stream_error.load(std::sync::atomic::Ordering::Relaxed) {
                                return;
                            }
                            if let Some(response) = Self::extract_assistant_message(&run.messages) {
                                let _ = tx.send(Action::PromptSuccess {
                                    session_id: run_session_id.clone(),
                                    agent_id: run_agent_id.clone(),
                                    messages: vec![ChatMessage {
                                        role: MessageRole::Assistant,
                                        content: response,
                                    }],
                                });
                            } else if !run.streamed {
                                let _ = tx.send(Action::PromptFailure {
                                    session_id: run_session_id.clone(),
                                    agent_id: run_agent_id.clone(),
                                    error: "No assistant response received. Check provider key/config with /keys, /provider, /model."
                                        .to_string(),
                                });
                            } else {
                                let _ = tx.send(Action::PromptSuccess {
                                    session_id: run_session_id.clone(),
                                    agent_id: run_agent_id.clone(),
                                    messages: vec![],
                                });
                            }
                        }
                        Err(err) => {
                            if !saw_stream_error.load(std::sync::atomic::Ordering::Relaxed) {
                                let _ = tx.send(Action::PromptFailure {
                                    session_id: run_session_id.clone(),
                                    agent_id: run_agent_id.clone(),
                                    error: err.to_string(),
                                });
                            }
                        }
                    }
                });
                return;
            }
        }

        if let AppState::Chat { messages, .. } = &mut self.state {
            messages.push(ChatMessage {
                role: MessageRole::System,
                content: vec![ContentBlock::Text(
                    "Error: Async channel not initialized. Cannot send prompt.".to_string(),
                )],
            });
        }
    }

    pub(super) fn handle_prompt_run_started(
        &mut self,
        event_session_id: String,
        agent_id: String,
        run_id: Option<String>,
    ) {
        if let AppState::Chat {
            agents,
            active_agent_index,
            session_id,
            ..
        } = &mut self.state
        {
            if let Some(agent_idx) = agents
                .iter()
                .position(|a| a.agent_id == agent_id && a.session_id == event_session_id)
            {
                let agent = &mut agents[agent_idx];
                agent.status = AgentStatus::Running;
                agent.active_run_id = run_id;
                agent.stream_collector =
                    Some(crate::ui::markdown_stream::MarkdownStreamCollector::new());
                agent.live_tool_calls.clear();
                agent.exploration_batch = None;
                if *active_agent_index == agent_idx {
                    *session_id = agent.session_id.clone();
                }
            }
        }
    }

    pub(super) fn handle_prompt_success(
        &mut self,
        event_session_id: String,
        agent_id: String,
        new_messages: Vec<ChatMessage>,
    ) {
        let dispatch_session_id = event_session_id.clone();
        let dispatch_agent_id = agent_id.clone();
        let mut clarification_follow_up: Option<(String, String)> = None;
        let mut finalized_tail: Option<String> = None;
        let mut exploration_summary: Option<ChatMessage> = None;
        if let AppState::Chat {
            agents,
            active_agent_index,
            messages,
            scroll_from_bottom,
            pending_requests,
            plan_waiting_for_clarification_question,
            ..
        } = &mut self.state
        {
            if let Some(agent) = agents
                .iter_mut()
                .find(|a| a.agent_id == agent_id && a.session_id == event_session_id)
            {
                if let Some(collector) = &mut agent.stream_collector {
                    let tail = collector.finalize();
                    finalized_tail = Some(tail.clone());
                    Self::append_assistant_delta(&mut agent.messages, &tail);
                }
                agent.stream_collector = None;
                exploration_summary = crate::activity::take_exploration_completion_message(
                    &mut agent.exploration_batch,
                )
                .or_else(|| crate::activity::exploration_completion_message(agent));
                Self::merge_prompt_success_messages(&mut agent.messages, &new_messages);
                if let Some(summary) = &exploration_summary {
                    agent.messages.push(summary.clone());
                }
                agent.status = AgentStatus::Done;
                agent.active_run_id = None;
                agent.live_tool_calls.clear();
                agent.live_activity_message = None;
                agent.scroll_from_bottom = 0;
            }
            if *active_agent_index < agents.len()
                && agents[*active_agent_index].agent_id == agent_id
                && agents[*active_agent_index].session_id == event_session_id
            {
                if let Some(tail) = finalized_tail {
                    Self::append_assistant_delta(messages, &tail);
                } else if let Some(agent) = agents.get_mut(*active_agent_index) {
                    if let Some(collector) = &mut agent.stream_collector {
                        let tail = collector.finalize();
                        Self::append_assistant_delta(messages, &tail);
                    }
                    agent.stream_collector = None;
                }
                Self::merge_prompt_success_messages(messages, &new_messages);
                if let Some(summary) = exploration_summary {
                    messages.push(summary);
                }
                *scroll_from_bottom = 0;
            }
            if matches!(self.current_mode, TandemMode::Plan)
                && *plan_waiting_for_clarification_question
            {
                let has_question_pending = pending_requests.iter().any(|request| {
                    request.session_id == event_session_id
                        && request.agent_id == agent_id
                        && matches!(request.kind, PendingRequestKind::Question(_))
                });
                if !has_question_pending {
                    clarification_follow_up = Some((event_session_id.clone(), agent_id.clone()));
                }
                *plan_waiting_for_clarification_question = false;
            }
        }
        self.sync_active_agent_from_chat();
        self.maybe_dispatch_queued_for_agent(&dispatch_session_id, &dispatch_agent_id);
        if let Some((session_id, agent_id)) = clarification_follow_up {
            self.dispatch_prompt_for_agent(
                session_id,
                agent_id,
                "Before finalizing this plan, use the `question` tool to ask exactly one concise clarification or approval question with 2-3 choices, then wait for the user's answer.".to_string(),
            );
        }
    }

    pub(super) async fn handle_prompt_todo_updated(
        &mut self,
        event_session_id: String,
        todos: Vec<serde_json::Value>,
    ) {
        let payload = serde_json::json!({ "todos": todos });
        let mut todo_sync_jobs: Vec<(
            String,
            Vec<crate::net::client::ContextTodoSyncItem>,
            Option<String>,
            Option<String>,
        )> = Vec::new();
        let should_guard_pending = matches!(self.current_mode, TandemMode::Plan)
            && plan_helpers::task_payload_all_pending(Some(&payload));
        if let AppState::Chat {
            session_id,
            messages,
            tasks,
            active_task_id,
            agents,
            modal,
            plan_wizard,
            last_plan_task_fingerprint,
            plan_awaiting_approval,
            ..
        } = &mut self.state
        {
            if should_guard_pending && *plan_awaiting_approval {
                return;
            }

            let fingerprint = plan_helpers::plan_fingerprint_from_args(Some(&payload));
            let preview = plan_helpers::plan_preview_from_args(Some(&payload));
            let should_open_wizard = matches!(self.current_mode, TandemMode::Plan)
                && !fingerprint.is_empty()
                && *last_plan_task_fingerprint != fingerprint;

            if *session_id == event_session_id {
                plan_helpers::apply_task_payload(
                    tasks,
                    active_task_id,
                    "todo_write",
                    Some(&payload),
                );
            }
            for agent in agents.iter_mut() {
                if agent.session_id == event_session_id {
                    plan_helpers::apply_task_payload(
                        &mut agent.tasks,
                        &mut agent.active_task_id,
                        "todo_write",
                        Some(&payload),
                    );
                    if let Some(bound_run_id) = agent.bound_context_run_id.clone() {
                        todo_sync_jobs.push((
                            bound_run_id,
                            plan_helpers::context_todo_items_from_tasks(&agent.tasks),
                            Some(agent.session_id.clone()),
                            agent.active_run_id.clone(),
                        ));
                    }
                }
            }
            if !fingerprint.is_empty() {
                *last_plan_task_fingerprint = fingerprint;
            }
            if should_guard_pending {
                *plan_awaiting_approval = true;
            }
            if should_open_wizard {
                *modal = Some(ModalState::PlanFeedbackWizard);
                *plan_wizard = PlanFeedbackWizardState {
                    plan_name: String::new(),
                    scope: String::new(),
                    constraints: String::new(),
                    priorities: String::new(),
                    notes: String::new(),
                    cursor_step: 0,
                    source_request_id: None,
                    task_preview: preview,
                };
                messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: vec![ContentBlock::Text(
                        "Plan tasks updated. Review and refine in the Plan Feedback wizard."
                            .to_string(),
                    )],
                });
            }
        }
        if let Some(client) = &self.client {
            for (run_id, todo_items, source_session_id, source_run_id) in todo_sync_jobs {
                if let Err(err) = client
                    .context_run_sync_todos(
                        &run_id,
                        todo_items,
                        true,
                        source_session_id,
                        source_run_id,
                    )
                    .await
                {
                    self.connection_status =
                        format!("Context todo sync failed for {}: {}", run_id, err);
                }
            }
        }
        self.sync_chat_from_active_agent();
    }

    pub(super) async fn handle_prompt_agent_team_event(
        &mut self,
        event_session_id: String,
        agent_id: String,
        event: crate::net::client::StreamAgentTeamEvent,
    ) {
        let mut route_target: Option<(String, String, String)> = None;
        let mut info_line: Option<String> = None;

        if event.event_type == "agent_team.mailbox.enqueued" {
            if let (Some(team_name), Some(recipient)) =
                (event.team_name.as_deref(), event.recipient.as_deref())
            {
                if recipient != "*" {
                    let mut target = if let Some(bound_session_id) =
                        Self::load_agent_team_member_session_binding(team_name, recipient).await
                    {
                        if let AppState::Chat { agents, .. } = &self.state {
                            Self::resolve_agent_target_for_bound_session(
                                agents,
                                recipient,
                                &bound_session_id,
                            )
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if target.is_none() {
                        target = if let AppState::Chat { agents, .. } = &self.state {
                            Self::resolve_agent_target_for_recipient(agents, recipient)
                        } else {
                            None
                        };
                    }
                    if target.is_none() {
                        if let Some(required_agents) = Self::recipient_agent_number(recipient) {
                            let _ = self.ensure_agent_count(required_agents).await;
                            target = if let AppState::Chat { agents, .. } = &self.state {
                                Self::resolve_agent_target_for_recipient(agents, recipient)
                            } else {
                                None
                            };
                        }
                    }
                    if let Some((target_session, target_agent)) = target {
                        if event.message_type.as_deref() == Some("task_prompt") {
                            if let AppState::Chat { agents, .. } = &mut self.state {
                                if let Some(agent) = agents.iter_mut().find(|a| {
                                    a.session_id == target_session && a.agent_id == target_agent
                                }) {
                                    agent.delegated_worker = true;
                                    agent.delegated_team_name = Some(team_name.to_string());
                                }
                            }
                        }
                        if let Some(prompt) =
                            Self::load_agent_team_mailbox_prompt(team_name, recipient).await
                        {
                            let _ = Self::persist_agent_team_member_session_binding(
                                team_name,
                                recipient,
                                &target_session,
                            )
                            .await;
                            let _ = Self::persist_agent_team_session_context(
                                team_name,
                                &target_session,
                            )
                            .await;
                            info_line = Some(format!(
                                "Agent-team routed {} to {} ({})",
                                event.message_type.as_deref().unwrap_or("message"),
                                recipient,
                                team_name
                            ));
                            route_target = Some((target_session, target_agent, prompt));
                        } else {
                            info_line = Some(format!(
                                "Agent-team queued for {} in {}, but mailbox payload could not be loaded.",
                                recipient, team_name
                            ));
                        }
                    }
                }
            }
        }

        if info_line.is_none() {
            info_line = Some(format!(
                "Agent-team event: {}",
                event.event_type.replace("agent_team.", "")
            ));
        }

        if let AppState::Chat { messages, .. } = &mut self.state {
            messages.push(ChatMessage {
                role: MessageRole::System,
                content: vec![ContentBlock::Text(
                    info_line.unwrap_or_else(|| "Agent-team event received.".to_string()),
                )],
            });
        }
        self.sync_active_agent_from_chat();
        if let Some((target_session, target_agent, prompt)) = route_target {
            self.dispatch_prompt_for_agent(target_session, target_agent, prompt);
        } else {
            self.maybe_dispatch_queued_for_agent(&event_session_id, &agent_id);
        }
    }

    pub(super) fn handle_prompt_delta(
        &mut self,
        event_session_id: String,
        agent_id: String,
        delta: String,
    ) {
        if let AppState::Chat {
            agents,
            active_agent_index,
            messages,
            scroll_from_bottom,
            ..
        } = &mut self.state
        {
            let mut committed = String::new();
            if let Some(agent) = agents
                .iter_mut()
                .find(|a| a.agent_id == agent_id && a.session_id == event_session_id)
            {
                agent.status = AgentStatus::Streaming;
                agent.scroll_from_bottom = 0;
                let collector = agent
                    .stream_collector
                    .get_or_insert_with(crate::ui::markdown_stream::MarkdownStreamCollector::new);
                committed = collector.push_delta_commit_complete(&delta);
                Self::append_assistant_delta(&mut agent.messages, &committed);
            }
            if *active_agent_index < agents.len()
                && agents[*active_agent_index].agent_id == agent_id
                && agents[*active_agent_index].session_id == event_session_id
            {
                *scroll_from_bottom = 0;
                Self::append_assistant_delta(messages, &committed);
            }
        }
    }

    pub(super) fn handle_prompt_info(
        &mut self,
        event_session_id: String,
        agent_id: String,
        message: String,
    ) {
        if let AppState::Chat { agents, .. } = &mut self.state {
            if let Some(agent) = agents
                .iter_mut()
                .find(|a| a.agent_id == agent_id && a.session_id == event_session_id)
            {
                if !matches!(agent.status, AgentStatus::Streaming) {
                    agent.status = AgentStatus::Running;
                }
                agent.live_activity_message = Some(message);
            }
        }
        self.sync_active_agent_from_chat();
    }

    pub(super) fn handle_prompt_tool_delta(
        &mut self,
        event_session_id: String,
        agent_id: String,
        tool_call_id: String,
        tool_name: String,
        args_preview: String,
    ) {
        if let AppState::Chat {
            agents,
            active_agent_index,
            messages,
            scroll_from_bottom,
            ..
        } = &mut self.state
        {
            if let Some(agent_idx) = agents
                .iter()
                .position(|a| a.agent_id == agent_id && a.session_id == event_session_id)
            {
                let is_active = *active_agent_index == agent_idx;
                let agent = &mut agents[agent_idx];
                if matches!(agent.status, AgentStatus::Idle | AgentStatus::Done) {
                    agent.status = AgentStatus::Streaming;
                }
                let exploration_summary = crate::activity::record_tool_call(
                    &mut agent.exploration_batch,
                    &tool_name,
                    &args_preview,
                );
                if let Some(summary) = exploration_summary {
                    agent.messages.push(summary.clone());
                    if is_active {
                        messages.push(summary);
                        *scroll_from_bottom = 0;
                    }
                }
                agent.live_tool_calls.insert(
                    tool_call_id,
                    LiveToolCall {
                        tool_name,
                        args_preview,
                    },
                );
                agent.live_activity_message = None;
            }
        }
        self.sync_active_agent_from_chat();
    }

    pub(super) async fn handle_prompt_malformed_question(
        &mut self,
        event_session_id: String,
        agent_id: String,
        request_id: String,
    ) {
        let retry_key = format!("{}:{}", event_session_id, request_id);
        let should_retry = self.malformed_question_retries.insert(retry_key);
        if let Some(client) = &self.client {
            let _ = client.reject_question(&request_id).await;
        }
        if let AppState::Chat {
            pending_requests,
            request_cursor,
            modal,
            messages,
            ..
        } = &mut self.state
        {
            pending_requests.retain(|entry| match &entry.kind {
                PendingRequestKind::Permission(permission) => permission.id != request_id,
                PendingRequestKind::Question(question) => question.id != request_id,
            });
            if pending_requests.is_empty() {
                *request_cursor = 0;
                if matches!(modal, Some(ModalState::RequestCenter)) {
                    *modal = None;
                }
            } else if *request_cursor >= pending_requests.len() {
                *request_cursor = pending_requests.len().saturating_sub(1);
            }
            messages.push(ChatMessage {
                role: MessageRole::System,
                content: vec![ContentBlock::Text(
                    "Dismissed malformed question request and asked the agent to retry with a structured question payload.".to_string(),
                )],
            });
        }
        if should_retry {
            self.dispatch_prompt_for_agent(
                event_session_id,
                agent_id,
                "Your last `question` tool call had invalid or empty arguments. Retry once using this exact shape: {\"questions\":[{\"header\":\"Question\",\"question\":\"...\",\"options\":[{\"label\":\"...\",\"description\":\"...\"},{\"label\":\"...\",\"description\":\"...\"}],\"multiple\":false,\"custom\":true}]}. After calling the tool, stop and wait for the user's answer.".to_string(),
            );
        }
        self.sync_active_agent_from_chat();
    }

    pub(super) async fn handle_prompt_request(
        &mut self,
        event_session_id: String,
        agent_id: String,
        request: PendingRequestKind,
    ) {
        if let PendingRequestKind::Question(question) = &request {
            if question.questions.is_empty() {
                let retry_key = format!("{}:{}", event_session_id, question.id);
                let should_retry = self.malformed_question_retries.insert(retry_key);
                if let Some(client) = &self.client {
                    let _ = client.reject_question(&question.id).await;
                }
                if let AppState::Chat { messages, .. } = &mut self.state {
                    messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: vec![ContentBlock::Text(
                            "Ignored malformed question request with no prompts.".to_string(),
                        )],
                    });
                }
                if should_retry {
                    self.dispatch_prompt_for_agent(
                        event_session_id,
                        agent_id,
                        "Retry the `question` tool with a valid non-empty `questions` array and wait for user input.".to_string(),
                    );
                }
                self.sync_active_agent_from_chat();
                return;
            }
        }
        if self.is_delegated_worker_agent(&event_session_id, &agent_id) {
            match &request {
                PendingRequestKind::Permission(permission) => {
                    if let Some(client) = &self.client {
                        let _ = client.reply_permission(&permission.id, "once").await;
                    }
                    if let AppState::Chat { messages, .. } = &mut self.state {
                        messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: vec![ContentBlock::Text(format!(
                                "Auto-approved delegated permission request {} for {}.",
                                permission.id, agent_id
                            ))],
                        });
                    }
                    self.sync_active_agent_from_chat();
                    return;
                }
                PendingRequestKind::Question(question) => {
                    if let Some(client) = &self.client {
                        let _ = client.reject_question(&question.id).await;
                    }
                    if let AppState::Chat { messages, .. } = &mut self.state {
                        messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: vec![ContentBlock::Text(format!(
                                "Auto-rejected delegated question request {} for {} to keep execution flowing.",
                                question.id, agent_id
                            ))],
                        });
                    }
                    self.dispatch_prompt_for_agent(
                        event_session_id,
                        agent_id,
                        "Continue execution without asking clarification questions unless absolutely blocked. Make a reasonable assumption and proceed.".to_string(),
                    );
                    self.sync_active_agent_from_chat();
                    return;
                }
            }
        }
        if matches!(self.current_mode, TandemMode::Plan) {
            if let PendingRequestKind::Permission(permission) = &request {
                if plan_helpers::is_todo_write_tool_name(&permission.tool) {
                    if let Some(client) = &self.client {
                        let _ = client.reply_permission(&permission.id, "once").await;
                    }
                    let fingerprint =
                        plan_helpers::plan_fingerprint_from_args(permission.args.as_ref());
                    let preview = plan_helpers::plan_preview_from_args(permission.args.as_ref());
                    let should_open_wizard = if let AppState::Chat {
                        last_plan_task_fingerprint,
                        ..
                    } = &self.state
                    {
                        !fingerprint.is_empty() && *last_plan_task_fingerprint != fingerprint
                    } else {
                        false
                    };
                    if let AppState::Chat {
                        tasks,
                        active_task_id,
                        plan_wizard,
                        modal,
                        last_plan_task_fingerprint,
                        ..
                    } = &mut self.state
                    {
                        plan_helpers::apply_task_payload(
                            tasks,
                            active_task_id,
                            &permission.tool,
                            permission.args.as_ref(),
                        );
                        if !fingerprint.is_empty() {
                            *last_plan_task_fingerprint = fingerprint;
                        }
                        if should_open_wizard {
                            *modal = Some(ModalState::PlanFeedbackWizard);
                            *plan_wizard = PlanFeedbackWizardState {
                                plan_name: String::new(),
                                scope: String::new(),
                                constraints: String::new(),
                                priorities: String::new(),
                                notes: String::new(),
                                cursor_step: 0,
                                source_request_id: Some(permission.id.clone()),
                                task_preview: preview,
                            };
                        }
                    }
                    self.queue_plan_agent_prompt(4);
                    self.sync_active_agent_from_chat();
                    return;
                }
            }
        }
        if let AppState::Chat {
            pending_requests,
            request_cursor,
            modal,
            agents,
            active_agent_index,
            plan_waiting_for_clarification_question,
            ..
        } = &mut self.state
        {
            Self::purge_invalid_question_requests(pending_requests);
            let is_question_request = matches!(&request, PendingRequestKind::Question(_));
            let request_id = match &request {
                PendingRequestKind::Permission(permission) => permission.id.clone(),
                PendingRequestKind::Question(question) => question.id.clone(),
            };
            let exists = pending_requests.iter().any(|entry| match &entry.kind {
                PendingRequestKind::Permission(permission) => permission.id == request_id,
                PendingRequestKind::Question(question) => question.id == request_id,
            });
            if !exists {
                pending_requests.push(PendingRequest {
                    session_id: event_session_id.clone(),
                    agent_id: agent_id.clone(),
                    kind: request,
                });
            }

            let active_matches = *active_agent_index < agents.len()
                && agents[*active_agent_index].agent_id == agent_id
                && agents[*active_agent_index].session_id == event_session_id;
            if active_matches {
                if let Some(idx) = pending_requests.iter().position(|entry| match &entry.kind {
                    PendingRequestKind::Permission(permission) => permission.id == request_id,
                    PendingRequestKind::Question(question) => question.id == request_id,
                }) {
                    *request_cursor = idx;
                } else {
                    *request_cursor = pending_requests.len().saturating_sub(1);
                }
                *modal = Some(ModalState::RequestCenter);
            }
            if is_question_request {
                *plan_waiting_for_clarification_question = false;
            }
        }
    }

    pub(super) fn handle_prompt_request_resolved(&mut self, request_id: String) {
        if let AppState::Chat {
            pending_requests,
            request_cursor,
            modal,
            ..
        } = &mut self.state
        {
            pending_requests.retain(|entry| match &entry.kind {
                PendingRequestKind::Permission(permission) => permission.id != request_id,
                PendingRequestKind::Question(question) => question.id != request_id,
            });
            if pending_requests.is_empty() {
                *request_cursor = 0;
                if matches!(modal, Some(ModalState::RequestCenter)) {
                    *modal = None;
                }
            } else if *request_cursor >= pending_requests.len() {
                *request_cursor = pending_requests.len().saturating_sub(1);
            }
        }
    }

    pub(super) fn handle_prompt_failure(
        &mut self,
        event_session_id: String,
        agent_id: String,
        error: String,
    ) {
        let dispatch_session_id = event_session_id.clone();
        let dispatch_agent_id = agent_id.clone();
        let mut finalized_tail: Option<String> = None;
        if let AppState::Chat {
            agents,
            active_agent_index,
            messages,
            scroll_from_bottom,
            ..
        } = &mut self.state
        {
            if let Some(agent) = agents
                .iter_mut()
                .find(|a| a.agent_id == agent_id && a.session_id == event_session_id)
            {
                if let Some(collector) = &mut agent.stream_collector {
                    let tail = collector.finalize();
                    finalized_tail = Some(tail.clone());
                    Self::append_assistant_delta(&mut agent.messages, &tail);
                }
                agent.stream_collector = None;
                agent.status = AgentStatus::Error;
                agent.active_run_id = None;
                agent.live_tool_calls.clear();
                agent.exploration_batch = None;
                agent.scroll_from_bottom = 0;
                agent.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: vec![ContentBlock::Text(format!("Prompt failed: {}", error))],
                });
            }
            if *active_agent_index < agents.len()
                && agents[*active_agent_index].agent_id == agent_id
                && agents[*active_agent_index].session_id == event_session_id
            {
                if let Some(tail) = finalized_tail {
                    Self::append_assistant_delta(messages, &tail);
                } else if let Some(agent) = agents.get_mut(*active_agent_index) {
                    if let Some(collector) = &mut agent.stream_collector {
                        let tail = collector.finalize();
                        Self::append_assistant_delta(messages, &tail);
                    }
                    agent.stream_collector = None;
                }
                *scroll_from_bottom = 0;
                messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: vec![ContentBlock::Text(format!("Prompt failed: {}", error))],
                });
            }
        }
        self.sync_active_agent_from_chat();
        self.maybe_dispatch_queued_for_agent(&dispatch_session_id, &dispatch_agent_id);
    }

    pub(super) fn stream_request_to_action(
        session_id: String,
        agent_id: String,
        event: crate::net::client::StreamRequestEvent,
    ) -> Action {
        match event {
            crate::net::client::StreamRequestEvent::PermissionAsked(request) => {
                if request
                    .tool
                    .as_deref()
                    .map(plan_helpers::is_question_tool_name)
                    .unwrap_or(false)
                {
                    let questions = plan_helpers::question_drafts_from_permission_args(
                        request.args.as_ref(),
                        request.query.as_deref(),
                    );
                    if !questions.is_empty() {
                        return Action::PromptRequest {
                            session_id,
                            agent_id,
                            request: PendingRequestKind::Question(PendingQuestionRequest {
                                id: request.id.clone(),
                                questions,
                                question_index: 0,
                                permission_request_id: Some(request.id),
                            }),
                        };
                    }
                }
                Action::PromptRequest {
                    session_id,
                    agent_id,
                    request: PendingRequestKind::Permission(PendingPermissionRequest {
                        id: request.id,
                        tool: request.tool.unwrap_or_else(|| "tool".to_string()),
                        args: request.args,
                        args_source: request.args_source,
                        args_integrity: request.args_integrity,
                        query: request.query,
                        status: request.status,
                    }),
                }
            }
            crate::net::client::StreamRequestEvent::PermissionReplied { request_id, reply } => {
                Action::PromptRequestResolved {
                    session_id,
                    agent_id,
                    request_id,
                    reply,
                }
            }
            crate::net::client::StreamRequestEvent::QuestionAsked(request) => {
                let questions = request
                    .questions
                    .into_iter()
                    .map(|q| {
                        let has_options = !q.options.is_empty();
                        QuestionDraft {
                            header: q.header,
                            question: q.question,
                            options: q.options,
                            multiple: q.multiple.unwrap_or(false),
                            custom: q.custom.unwrap_or(!has_options),
                            selected_options: Vec::new(),
                            custom_input: String::new(),
                            option_cursor: 0,
                        }
                    })
                    .collect::<Vec<_>>();
                if questions.is_empty() {
                    return Action::PromptMalformedQuestion {
                        session_id,
                        agent_id,
                        request_id: request.id,
                    };
                }
                Action::PromptRequest {
                    session_id,
                    agent_id,
                    request: PendingRequestKind::Question(PendingQuestionRequest {
                        id: request.id,
                        questions,
                        question_index: 0,
                        permission_request_id: None,
                    }),
                }
            }
        }
    }
}
