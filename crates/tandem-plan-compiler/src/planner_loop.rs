// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tandem_types::ModelSpec;
use tandem_workflows::plan_package::{
    AutomationV2Schedule, WorkflowPlan, WorkflowPlanConversation, WorkflowPlanStep,
};

use crate::host::{PlannerLlmInvocation, PlannerLoopHost};
use crate::planner_invoke::invoke_planner_json;
use crate::planner_messages::{
    planner_failure_clarifier_hint, planner_llm_invalid_response_hint, planner_llm_unavailable_hint,
};
use crate::planner_types::{PlannerClarifier, PlannerInvocationFailure};
use crate::workflow_plan::{
    decode_planner_plan_value, normalize_and_validate_planner_plan,
    planner_llm_provider_unconfigured_hint, planner_model_spec, workflow_schedule_equal,
    workflow_steps_equal, PlannerPlanMode, PlannerPlanNormalizationContext, WorkflowInputRefLike,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannerRevisionAction {
    Revise,
    Clarify,
    Keep,
}

#[derive(Debug, Deserialize)]
pub struct PlannerRevisionPayload {
    pub action: PlannerRevisionAction,
    #[serde(default)]
    pub assistant_text: Option<String>,
    #[serde(default)]
    pub change_summary: Vec<String>,
    #[serde(default)]
    pub clarifier: Option<PlannerClarifier>,
    #[serde(default)]
    #[serde(alias = "workflow_plan")]
    pub plan: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerLoopConfig {
    pub session_title: String,
    pub timeout_ms: u64,
    pub override_env: String,
}

pub async fn revise_workflow_plan_with_planner_loop<M, I, O, H>(
    host: &H,
    current_plan: &WorkflowPlan<AutomationV2Schedule<M>, WorkflowPlanStep<I, O>>,
    conversation: &WorkflowPlanConversation,
    message: &str,
    config: PlannerLoopConfig,
    mut normalize_step: impl FnMut(&mut WorkflowPlanStep<I, O>),
) -> (
    WorkflowPlan<AutomationV2Schedule<M>, WorkflowPlanStep<I, O>>,
    String,
    Vec<String>,
    Value,
)
where
    M: Clone + serde::Serialize + DeserializeOwned,
    I: Clone + Default + WorkflowInputRefLike + serde::Serialize + DeserializeOwned,
    O: Clone + Default + serde::Serialize + DeserializeOwned,
    H: PlannerLoopHost,
{
    let Some(model) = planner_model_spec(current_plan.operator_preferences.as_ref()) else {
        let question = planner_llm_unavailable_hint();
        return (
            current_plan.clone(),
            format!("I kept the current plan. Clarification needed: {question}"),
            Vec::new(),
            json!({
                "field": "general",
                "question": question,
                "options": [],
            }),
        );
    };

    if !host.is_provider_configured(&model.provider_id).await {
        let question = planner_llm_provider_unconfigured_hint(&model.provider_id);
        return (
            current_plan.clone(),
            format!("I kept the current plan. Clarification needed: {question}"),
            Vec::new(),
            json!({
                "field": "general",
                "question": question,
                "options": [],
            }),
        );
    }

    let normalization_ctx = PlannerPlanNormalizationContext {
        mode: PlannerPlanMode::Revise,
        plan_id: &current_plan.plan_id,
        planner_version: &current_plan.planner_version,
        plan_source: &current_plan.plan_source,
        original_prompt: &current_plan.original_prompt,
        normalized_prompt: &current_plan.normalized_prompt,
        resolved_workspace_root: &current_plan.workspace_root,
        explicit_schedule: None,
        request_allowed_mcp_servers: &current_plan.allowed_mcp_servers,
        request_operator_preferences: current_plan.operator_preferences.as_ref(),
    };

    match try_llm_revise_workflow_plan(host, &config, &model, current_plan, conversation, message)
        .await
    {
        Ok(payload) => parse_llm_revision_payload(
            current_plan,
            payload,
            &normalization_ctx,
            &mut normalize_step,
        )
        .unwrap_or_else(|| {
            let question = planner_llm_invalid_response_hint();
            (
                current_plan.clone(),
                format!("I kept the current plan. Clarification needed: {question}"),
                Vec::new(),
                json!({
                    "field": "general",
                    "question": question,
                    "options": [],
                }),
            )
        }),
        Err(failure) => {
            let question = planner_failure_clarifier_hint(&failure);
            (
                current_plan.clone(),
                format!("I kept the current plan. Clarification needed: {question}"),
                Vec::new(),
                json!({
                    "field": "general",
                    "question": question,
                    "options": [],
                }),
            )
        }
    }
}

async fn try_llm_revise_workflow_plan<M, I, O, H>(
    host: &H,
    config: &PlannerLoopConfig,
    model: &ModelSpec,
    current_plan: &WorkflowPlan<AutomationV2Schedule<M>, WorkflowPlanStep<I, O>>,
    conversation: &WorkflowPlanConversation,
    message: &str,
) -> Result<PlannerRevisionPayload, PlannerInvocationFailure>
where
    M: serde::Serialize,
    I: serde::Serialize,
    O: serde::Serialize,
    H: PlannerLoopHost,
{
    let capability_summary = host
        .capability_summary(&current_plan.allowed_mcp_servers)
        .await;
    let prompt = build_llm_workflow_revision_prompt(
        current_plan,
        conversation,
        message,
        &capability_summary,
    );

    invoke_planner_json(
        host,
        PlannerLlmInvocation {
            session_title: config.session_title.clone(),
            workspace_root: current_plan.workspace_root.clone(),
            model: model.clone(),
            prompt,
            run_key: format!("workflow-plan-revision:{}", current_plan.plan_id),
            timeout_ms: config.timeout_ms,
            override_env: config.override_env.clone(),
        },
    )
    .await
}

fn parse_llm_revision_payload<M, I, O>(
    current_plan: &WorkflowPlan<AutomationV2Schedule<M>, WorkflowPlanStep<I, O>>,
    payload: PlannerRevisionPayload,
    ctx: &PlannerPlanNormalizationContext<'_, M>,
    normalize_step: &mut impl FnMut(&mut WorkflowPlanStep<I, O>),
) -> Option<(
    WorkflowPlan<AutomationV2Schedule<M>, WorkflowPlanStep<I, O>>,
    String,
    Vec<String>,
    Value,
)>
where
    M: Clone + serde::Serialize + DeserializeOwned,
    I: Clone + Default + WorkflowInputRefLike + serde::Serialize + DeserializeOwned,
    O: Clone + Default + serde::Serialize + DeserializeOwned,
{
    match payload.action {
        PlannerRevisionAction::Clarify => {
            let clarifier = payload.clarifier?;
            let question = clarifier.question.trim();
            if question.is_empty() {
                return None;
            }
            let assistant_text = payload
                .assistant_text
                .unwrap_or_else(|| question.to_string());
            Some((
                current_plan.clone(),
                assistant_text,
                Vec::new(),
                json!({
                    "field": clarifier.field.unwrap_or_else(|| "general".to_string()),
                    "question": question,
                    "options": clarifier.options,
                }),
            ))
        }
        PlannerRevisionAction::Keep => Some((
            current_plan.clone(),
            payload
                .assistant_text
                .unwrap_or_else(|| "I kept the current workflow plan.".to_string()),
            Vec::new(),
            Value::Null,
        )),
        PlannerRevisionAction::Revise => {
            let candidate = decode_planner_plan_value(payload.plan?)?;
            let revised_plan =
                normalize_and_validate_planner_plan(candidate, ctx, normalize_step).ok()?;
            if workflow_steps_equal(&revised_plan.steps, &current_plan.steps)
                && revised_plan.title == current_plan.title
                && revised_plan.description == current_plan.description
                && workflow_schedule_equal(&revised_plan.schedule, &current_plan.schedule)
                && revised_plan.workspace_root == current_plan.workspace_root
                && revised_plan.allowed_mcp_servers == current_plan.allowed_mcp_servers
                && revised_plan.operator_preferences == current_plan.operator_preferences
            {
                return Some((
                    current_plan.clone(),
                    payload
                        .assistant_text
                        .unwrap_or_else(|| "I kept the current workflow plan.".to_string()),
                    Vec::new(),
                    Value::Null,
                ));
            }
            let change_summary = if payload.change_summary.is_empty() {
                vec!["updated workflow plan".to_string()]
            } else {
                payload.change_summary
            };
            let assistant_text = payload
                .assistant_text
                .unwrap_or_else(|| format!("Updated the plan: {}.", change_summary.join(", ")));
            Some((revised_plan, assistant_text, change_summary, Value::Null))
        }
    }
}

fn build_llm_workflow_revision_prompt<M, I, O>(
    current_plan: &WorkflowPlan<AutomationV2Schedule<M>, WorkflowPlanStep<I, O>>,
    conversation: &WorkflowPlanConversation,
    message: &str,
    capability_summary: &Value,
) -> String
where
    M: serde::Serialize,
    I: serde::Serialize,
    O: serde::Serialize,
{
    let transcript = conversation
        .messages
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|entry| format!("{}: {}", entry.role, entry.text.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    let common_sections = crate::planner_prompts::workflow_plan_common_sections();
    format!(
        concat!(
            "You are revising a Tandem automation workflow plan.\n",
            "Planner intelligence lives in the model. Return JSON only.\n",
            "{}",
            "You may revise title, description, schedule, workspace_root, allowed_mcp_servers, operator_preferences, steps, dependencies, input_refs, and output_contracts.\n",
            "Planner capability summary and runtime MCP inventory (use this instead of inventing tools or hidden capabilities):\n{}\n",
            "Return one of:\n",
            "{{\"action\":\"revise\",\"assistant_text\":\"...\",\"change_summary\":[\"...\"],\"plan\":{{...full WorkflowPlan...}}}}\n",
            "{{\"action\":\"clarify\",\"assistant_text\":\"...\",\"clarifier\":{{\"field\":\"general\",\"question\":\"...\"}}}}\n",
            "{{\"action\":\"keep\",\"assistant_text\":\"...\"}}\n\n",
            "Original prompt:\n{}\n\n",
            "Current plan JSON:\n{}\n\n",
            "Recent planning conversation:\n{}\n\n",
            "User revision request:\n{}\n"
        ),
        common_sections,
        serde_json::to_string_pretty(capability_summary).unwrap_or_else(|_| "{}".to_string()),
        current_plan.original_prompt.trim(),
        serde_json::to_string_pretty(current_plan).unwrap_or_else(|_| "{}".to_string()),
        if transcript.trim().is_empty() {
            "(none yet)".to_string()
        } else {
            transcript
        },
        message.trim(),
    )
}
