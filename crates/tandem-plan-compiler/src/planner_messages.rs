// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use serde::{Deserialize, Serialize};

use crate::planner_types::PlannerInvocationFailure;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeachingLibrarySummary {
    #[serde(default)]
    pub explanations: Vec<String>,
    #[serde(default)]
    pub objections: Vec<String>,
    #[serde(default)]
    pub proof_points: Vec<String>,
}

pub fn planner_teaching_library_summary() -> TeachingLibrarySummary {
    TeachingLibrarySummary {
        explanations: vec![
            "Summarize the workflow in 2-4 sentences.".to_string(),
            "State which steps produce the final deliverable.".to_string(),
        ],
        objections: vec![
            "Call out missing inputs or unclear dependencies.".to_string(),
            "Highlight missing connectors or unavailable capabilities.".to_string(),
        ],
        proof_points: vec![
            "List artifacts or validations that prove success.".to_string(),
            "Reference evidence sources used by the plan.".to_string(),
        ],
    }
}

pub(crate) fn planner_llm_unavailable_hint() -> &'static str {
    "This workflow needs planner model settings before Tandem can revise it. Configure a planner model and try again."
}

pub(crate) fn planner_llm_invalid_response_hint() -> &'static str {
    "The planner could not produce a valid workflow revision. Keep the current plan or try a more specific request."
}

pub(crate) fn planner_failure_clarifier_hint(failure: &PlannerInvocationFailure) -> String {
    let detail = failure
        .detail
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let lower = detail.to_ascii_lowercase();
    if failure.reason == "provider_auth_failed"
        || lower.contains("user not found")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
        || lower.contains("invalid api key")
        || lower.contains("403")
        || lower.contains("401")
    {
        return "The configured planner provider rejected authentication (User not found / unauthorized). Check provider account linkage or API key in Settings, or switch planner model/provider.".to_string();
    }
    if detail.is_empty() {
        planner_llm_invalid_response_hint().to_string()
    } else {
        detail.to_string()
    }
}
