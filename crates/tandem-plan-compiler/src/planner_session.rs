// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use crate::host::PlannerSessionStore;
use crate::planner_types::PlannerInvocationFailure;
use crate::workflow_plan::truncate_text;

pub async fn begin_planner_session<S: PlannerSessionStore>(
    store: &S,
    session_title: &str,
    workspace_root: &str,
    prompt: &str,
) -> Result<String, PlannerInvocationFailure> {
    let session_id = store
        .create_planner_session(session_title, workspace_root)
        .await
        .map_err(storage_failure)?;
    store
        .append_planner_user_prompt(&session_id, prompt)
        .await
        .map_err(storage_failure)?;
    Ok(session_id)
}

pub async fn finish_planner_session<S: PlannerSessionStore>(
    store: &S,
    session_id: &str,
    response: &str,
) -> Result<(), PlannerInvocationFailure> {
    store
        .append_planner_assistant_response(session_id, response)
        .await
        .map_err(storage_failure)
}

fn storage_failure(error: String) -> PlannerInvocationFailure {
    PlannerInvocationFailure {
        reason: "storage_error".to_string(),
        detail: Some(truncate_text(&error, 500)),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;

    struct TestSessionStore {
        sessions: Mutex<Vec<(String, String)>>,
        user_messages: Mutex<Vec<(String, String)>>,
        assistant_messages: Mutex<Vec<(String, String)>>,
    }

    impl TestSessionStore {
        fn new() -> Self {
            Self {
                sessions: Mutex::new(Vec::new()),
                user_messages: Mutex::new(Vec::new()),
                assistant_messages: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl PlannerSessionStore for TestSessionStore {
        async fn create_planner_session(
            &self,
            title: &str,
            workspace_root: &str,
        ) -> Result<String, String> {
            self.sessions
                .lock()
                .unwrap()
                .push((title.to_string(), workspace_root.to_string()));
            Ok("session_1".to_string())
        }

        async fn append_planner_user_prompt(
            &self,
            session_id: &str,
            prompt: &str,
        ) -> Result<(), String> {
            self.user_messages
                .lock()
                .unwrap()
                .push((session_id.to_string(), prompt.to_string()));
            Ok(())
        }

        async fn append_planner_assistant_response(
            &self,
            session_id: &str,
            response: &str,
        ) -> Result<(), String> {
            self.assistant_messages
                .lock()
                .unwrap()
                .push((session_id.to_string(), response.to_string()));
            Ok(())
        }
    }

    #[tokio::test]
    async fn planner_session_helpers_record_prompt_and_response() {
        let store = TestSessionStore::new();
        let session_id = begin_planner_session(&store, "Planner", "/repo", "Build a workflow.")
            .await
            .unwrap();
        finish_planner_session(&store, &session_id, "{\"action\":\"keep\"}")
            .await
            .unwrap();

        assert_eq!(session_id, "session_1");
        assert_eq!(store.sessions.lock().unwrap().len(), 1);
        assert_eq!(store.user_messages.lock().unwrap().len(), 1);
        assert_eq!(store.assistant_messages.lock().unwrap().len(), 1);
    }
}
