use serde::{Deserialize, Serialize};
use serde_json::Value;

// NOTE: This module is intentionally permissive and generic.
// The host runtime (server, desktop, etc.) can specialize the type aliases to its
// concrete schedule / step schema types without forcing this crate to depend on
// host-only types.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutomationV2ScheduleType {
    Cron,
    Interval,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutomationV2Schedule<MisfirePolicy = Value> {
    #[serde(rename = "type")]
    pub schedule_type: AutomationV2ScheduleType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron_expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    pub timezone: String,
    pub misfire_policy: MisfirePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowPlanStep<InputRef = Value, OutputContract = Value> {
    pub step_id: String,
    pub kind: String,
    pub objective: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub agent_role: String,
    #[serde(default)]
    pub input_refs: Vec<InputRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlan<Schedule = Value, Step = Value> {
    pub plan_id: String,
    pub planner_version: String,
    pub plan_source: String,
    pub original_prompt: String,
    pub normalized_prompt: String,
    pub confidence: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub schedule: Schedule,
    pub execution_target: String,
    pub workspace_root: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub requires_integrations: Vec<String>,
    #[serde(default)]
    pub allowed_mcp_servers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_preferences: Option<Value>,
    pub save_options: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlanChatMessage {
    pub role: String,
    pub text: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlanConversation {
    pub conversation_id: String,
    pub plan_id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub messages: Vec<WorkflowPlanChatMessage>,
}

fn default_workflow_plan_draft_revision() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlanDraftRecord<Plan = Value> {
    pub initial_plan: Plan,
    pub current_plan: Plan,
    #[serde(default = "default_workflow_plan_draft_revision")]
    pub plan_revision: u32,
    pub conversation: WorkflowPlanConversation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_diagnostics: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_materialization: Option<Value>,
}
