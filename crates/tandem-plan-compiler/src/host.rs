// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use async_trait::async_trait;
use serde_json::Value;
use tandem_types::ModelSpec;

use crate::planner_types::PlannerInvocationFailure;

#[derive(Debug, Clone)]
pub struct PlannerLlmInvocation {
    pub session_title: String,
    pub workspace_root: String,
    pub model: ModelSpec,
    pub prompt: String,
    pub run_key: String,
    pub timeout_ms: u64,
    pub override_env: String,
}

#[async_trait]
pub trait WorkspaceResolver: Send + Sync {
    async fn resolve_workspace_root(&self, requested: Option<&str>) -> Result<String, String>;
}

#[async_trait]
pub trait PlanStore: Send + Sync {
    async fn get_draft(&self, _plan_id: &str) -> Result<Option<Value>, String> {
        Ok(None)
    }

    async fn put_draft(&self, _plan_id: &str, _draft: Value) -> Result<(), String> {
        Ok(())
    }
}

#[async_trait]
pub trait PlannerSessionStore: Send + Sync {
    async fn create_planner_session(
        &self,
        title: &str,
        workspace_root: &str,
    ) -> Result<String, String>;

    async fn append_planner_user_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Result<(), String>;

    async fn append_planner_assistant_response(
        &self,
        session_id: &str,
        response: &str,
    ) -> Result<(), String>;
}

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

#[async_trait]
pub trait PlannerModelRegistry: Send + Sync {
    async fn is_provider_configured(&self, provider_id: &str) -> bool;
}

#[async_trait]
pub trait McpToolCatalog: Send + Sync {
    async fn capability_summary(&self, allowed_mcp_servers: &[String]) -> Value;
}

#[async_trait]
pub trait PlannerLlmInvoker: Send + Sync {
    async fn invoke_planner_llm(
        &self,
        invocation: PlannerLlmInvocation,
    ) -> Result<Value, PlannerInvocationFailure>;
}

pub trait TelemetrySink: Send + Sync {
    fn warn(&self, _message: &str) {}
}

pub trait PlannerLoopHost:
    PlannerModelRegistry + McpToolCatalog + PlannerLlmInvoker + TelemetrySink
{
}

impl<T> PlannerLoopHost for T where
    T: PlannerModelRegistry + McpToolCatalog + PlannerLlmInvoker + TelemetrySink
{
}
