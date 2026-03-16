use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{HostRuntimeContext, Message, ModelSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTime {
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub slug: Option<String>,
    pub version: Option<String>,
    pub project_id: Option<String>,
    pub title: String,
    pub directory: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_workspace_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_from_workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_to_workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach_timestamp_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach_reason: Option<String>,
    pub time: SessionTime,
    pub model: Option<ModelSpec>,
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<HostRuntimeContext>,
    #[serde(default)]
    pub messages: Vec<Message>,
}

impl Session {
    pub fn new(title: Option<String>, directory: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            slug: None,
            version: Some("v1".to_string()),
            project_id: None,
            title: title.unwrap_or_else(|| "New session".to_string()),
            directory: directory.unwrap_or_else(|| ".".to_string()),
            workspace_root: None,
            origin_workspace_root: None,
            attached_from_workspace: None,
            attached_to_workspace: None,
            attach_timestamp_ms: None,
            attach_reason: None,
            time: SessionTime {
                created: now,
                updated: now,
            },
            model: None,
            provider: None,
            environment: None,
            messages: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub parent_id: Option<String>,
    pub title: Option<String>,
    pub directory: Option<String>,
    pub workspace_root: Option<String>,
    pub model: Option<ModelSpec>,
    pub provider: Option<String>,
    pub permission: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    #[serde(default)]
    pub parts: Vec<crate::MessagePartInput>,
    pub model: Option<ModelSpec>,
    pub agent: Option<String>,
    #[serde(default, alias = "toolMode", alias = "tool_mode")]
    pub tool_mode: Option<ToolMode>,
    #[serde(default, alias = "toolAllowlist", alias = "tool_allowlist")]
    pub tool_allowlist: Option<Vec<String>>,
    #[serde(default, alias = "contextMode", alias = "context_mode")]
    pub context_mode: Option<ContextMode>,
    #[serde(default, alias = "writeRequired", alias = "write_required")]
    pub write_required: Option<bool>,
    #[serde(
        default,
        alias = "prewriteRequirements",
        alias = "prewrite_requirements"
    )]
    pub prewrite_requirements: Option<PrewriteRequirements>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrewriteRequirements {
    #[serde(
        default,
        alias = "workspaceInspectionRequired",
        alias = "workspace_inspection_required"
    )]
    pub workspace_inspection_required: bool,
    #[serde(
        default,
        alias = "webResearchRequired",
        alias = "web_research_required"
    )]
    pub web_research_required: bool,
    #[serde(
        default,
        alias = "concreteReadRequired",
        alias = "concrete_read_required"
    )]
    pub concrete_read_required: bool,
    #[serde(
        default,
        alias = "successfulWebResearchRequired",
        alias = "successful_web_research_required"
    )]
    pub successful_web_research_required: bool,
    #[serde(
        default,
        alias = "repairOnUnmetRequirements",
        alias = "repair_on_unmet_requirements"
    )]
    pub repair_on_unmet_requirements: bool,
    #[serde(default, alias = "coverageMode", alias = "coverage_mode")]
    pub coverage_mode: PrewriteCoverageMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrewriteCoverageMode {
    #[default]
    None,
    FilesReviewedBacked,
    ResearchCorpus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    Auto,
    None,
    Required,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextMode {
    Auto,
    Compact,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: String,
}
