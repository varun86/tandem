use super::{App, AppState, ModalState};

impl App {
    pub(super) fn queue_plan_agent_prompt(&mut self, count: usize) {
        if let AppState::Chat {
            plan_multi_agent_prompt,
            modal,
            agents,
            ..
        } = &mut self.state
        {
            if agents.len() >= count {
                return;
            }
            *plan_multi_agent_prompt = Some(count);
            if modal.is_none() {
                *modal = Some(ModalState::StartPlanAgents { count });
            }
        }
    }

    pub(super) fn open_queued_plan_agent_prompt(&mut self) {
        if let AppState::Chat {
            plan_multi_agent_prompt,
            modal,
            agents,
            ..
        } = &mut self.state
        {
            if modal.is_some() {
                return;
            }
            if let Some(count) = plan_multi_agent_prompt.take() {
                if agents.len() < count {
                    *modal = Some(ModalState::StartPlanAgents { count });
                }
            }
        }
    }

    pub(super) async fn ensure_agent_count(&mut self, count: usize) -> usize {
        let (current_count, fallback_session, active_idx) = if let AppState::Chat {
            agents,
            active_agent_index,
            ..
        } = &self.state
        {
            let fallback = agents
                .get(*active_agent_index)
                .map(|a| a.session_id.clone())
                .unwrap_or_default();
            (agents.len(), fallback, *active_agent_index)
        } else {
            (0, String::new(), 0)
        };

        if current_count >= count {
            return 0;
        }

        let mut new_panes = Vec::new();
        for idx in current_count..count {
            let agent_id = format!("A{}", idx + 1);
            let mut session_id = fallback_session.clone();
            if let Some(client) = &self.client {
                if let Ok(session) = client
                    .create_session(Some(format!("{} session", agent_id)))
                    .await
                {
                    session_id = session.id;
                }
            }
            new_panes.push(Self::make_agent_pane(agent_id, session_id));
        }

        let created = new_panes.len();
        if let AppState::Chat {
            agents,
            active_agent_index,
            grid_page,
            ..
        } = &mut self.state
        {
            agents.extend(new_panes);
            let max_page = agents.len().saturating_sub(1) / 4;
            if *grid_page > max_page {
                *grid_page = max_page;
            }
            *active_agent_index = active_idx.min(agents.len().saturating_sub(1));
        }
        created
    }
}
