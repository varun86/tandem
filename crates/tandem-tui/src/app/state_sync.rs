use super::{ActivitySummary, AgentPane, App, AppState};

impl App {
    pub(super) fn active_agent_clone(&self) -> Option<super::AgentPane> {
        if let AppState::Chat {
            agents,
            active_agent_index,
            ..
        } = &self.state
        {
            return agents.get(*active_agent_index).cloned();
        }
        None
    }

    pub(super) fn sync_chat_from_active_agent(&mut self) {
        if let AppState::Chat {
            session_id,
            command_input,
            messages,
            scroll_from_bottom,
            tasks,
            active_task_id,
            agents,
            active_agent_index,
            ..
        } = &mut self.state
        {
            if let Some(agent) = agents.get(*active_agent_index) {
                *session_id = agent.session_id.clone();
                *command_input = agent.draft.clone();
                *messages = agent.messages.clone();
                *scroll_from_bottom = agent.scroll_from_bottom;
                *tasks = agent.tasks.clone();
                *active_task_id = agent.active_task_id.clone();
            }
        }
    }

    pub(super) fn sync_active_agent_from_chat(&mut self) {
        if let AppState::Chat {
            session_id,
            command_input,
            messages,
            scroll_from_bottom,
            tasks,
            active_task_id,
            agents,
            active_agent_index,
            ..
        } = &mut self.state
        {
            if let Some(agent) = agents.get_mut(*active_agent_index) {
                agent.session_id = session_id.clone();
                agent.draft = command_input.clone();
                agent.messages = messages.clone();
                agent.scroll_from_bottom = *scroll_from_bottom;
                agent.tasks = tasks.clone();
                agent.active_task_id = active_task_id.clone();
                Self::prune_agent_paste_registry(agent);
            }
        }
    }

    pub fn active_activity_summary(&self) -> Option<ActivitySummary> {
        if let AppState::Chat {
            agents,
            active_agent_index,
            pending_requests,
            plan_awaiting_approval,
            plan_waiting_for_clarification_question,
            ..
        } = &self.state
        {
            let agent = agents.get(*active_agent_index)?;
            return Some(crate::activity::summarize_active_agent(
                agent,
                pending_requests,
                *plan_waiting_for_clarification_question,
                *plan_awaiting_approval,
            ));
        }
        None
    }

    pub fn agent_status_label(&self, agent: &AgentPane, spinner: &str) -> String {
        crate::activity::agent_status_label(agent, spinner)
    }

    pub(crate) fn pending_request_counts(&self) -> (usize, usize) {
        if let AppState::Chat {
            agents,
            active_agent_index,
            pending_requests,
            ..
        } = &self.state
        {
            if let Some(agent) = agents.get(*active_agent_index) {
                let active_count = pending_requests
                    .iter()
                    .filter(|r| r.session_id == agent.session_id && r.agent_id == agent.agent_id)
                    .count();
                let background_count = pending_requests.len().saturating_sub(active_count);
                return (active_count, background_count);
            }
            return (0, pending_requests.len());
        }
        (0, 0)
    }
}
