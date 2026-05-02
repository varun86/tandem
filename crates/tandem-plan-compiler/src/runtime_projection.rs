// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use serde_json::json;
use tandem_workflows::plan_package::{WorkflowPlan, WorkflowPlanStep};

use crate::automation_projection::{
    ProjectedAutomationAgentProfile, ProjectedAutomationDraft, ProjectedAutomationExecutionPolicy,
    ProjectedAutomationNode,
};
use crate::materialization::{
    materialization_seed_from_projection, ProjectedAutomationMaterializationSeed,
};
use crate::workflow_plan::{
    agent_id_for_role, compile_operator_model_policy, compile_workflow_agent_tool_allowlist,
    display_name_for_role, infer_explicit_output_targets, plan_max_parallel_agents,
    workflow_plan_agent_roles,
};

pub fn compile_workflow_runtime_projection<S, I, O>(
    plan: &WorkflowPlan<S, WorkflowPlanStep<I, O>>,
    normalize_allowed_tools: impl Fn(Vec<String>) -> Vec<String>,
) -> ProjectedAutomationDraft<I, O>
where
    I: Clone,
    O: Clone,
{
    let model_policy = compile_operator_model_policy(plan.operator_preferences.as_ref());
    let tool_allowlist = compile_workflow_agent_tool_allowlist(
        &plan.allowed_mcp_servers,
        plan.operator_preferences.as_ref(),
        normalize_allowed_tools,
    );
    let agents = workflow_plan_agent_roles(&plan.steps, |step| step.agent_role.as_str())
        .into_iter()
        .map(|agent_role| ProjectedAutomationAgentProfile {
            agent_id: agent_id_for_role(&agent_role),
            template_id: None,
            display_name: display_name_for_role(&agent_role),
            model_policy: model_policy.clone(),
            tool_allowlist: tool_allowlist.clone(),
            allowed_mcp_servers: plan.allowed_mcp_servers.clone(),
        })
        .collect::<Vec<_>>();

    let nodes = plan
        .steps
        .iter()
        .map(|step| ProjectedAutomationNode {
            node_id: step.step_id.clone(),
            agent_id: agent_id_for_role(&step.agent_role),
            objective: step.objective.clone(),
            depends_on: step.depends_on.clone(),
            input_refs: step.input_refs.clone(),
            output_contract: step.output_contract.clone(),
            retry_policy: Some(json!({
                "max_attempts": 3
            })),
            timeout_ms: workflow_runtime_step_timeout_ms(step),
            stage_kind: None,
            gate: None,
            metadata: step.metadata.clone(),
        })
        .collect::<Vec<_>>();

    ProjectedAutomationDraft {
        name: plan.title.clone(),
        description: plan.description.clone(),
        workspace_root: Some(plan.workspace_root.clone()),
        output_targets: infer_explicit_output_targets(&plan.original_prompt),
        agents,
        nodes,
        execution: ProjectedAutomationExecutionPolicy {
            max_parallel_agents: Some(plan_max_parallel_agents(plan.operator_preferences.as_ref())),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        context: None,
        metadata: json!({
            "workflow_plan_id": plan.plan_id,
            "workflow_plan_source": plan.plan_source,
            "workflow_plan_version": plan.planner_version,
        }),
    }
}

fn workflow_runtime_step_timeout_ms<I, O>(step: &WorkflowPlanStep<I, O>) -> Option<u64> {
    let step_id = step.step_id.trim().to_ascii_lowercase();
    let kind = step.kind.trim().to_ascii_lowercase();
    if step_id == "execute_goal" || kind == "execute" {
        Some(1_800_000)
    } else {
        None
    }
}

pub fn project_workflow_runtime_materialization_seed<S, I, O>(
    plan: &WorkflowPlan<S, WorkflowPlanStep<I, O>>,
    normalize_allowed_tools: impl Fn(Vec<String>) -> Vec<String>,
) -> ProjectedAutomationMaterializationSeed<I, O>
where
    I: Clone,
    O: Clone,
{
    materialization_seed_from_projection(compile_workflow_runtime_projection(
        plan,
        normalize_allowed_tools,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use tandem_workflows::plan_package::{
        AutomationV2Schedule, AutomationV2ScheduleType, WorkflowPlan,
    };

    fn test_plan() -> WorkflowPlan<AutomationV2Schedule<Value>, WorkflowPlanStep<Value, Value>> {
        WorkflowPlan {
            plan_id: "wfplan-test".to_string(),
            planner_version: "v1".to_string(),
            plan_source: "unit_test".to_string(),
            original_prompt: "test prompt".to_string(),
            normalized_prompt: "test prompt".to_string(),
            confidence: "medium".to_string(),
            title: "Runtime Test".to_string(),
            description: Some("desc".to_string()),
            schedule: AutomationV2Schedule {
                schedule_type: AutomationV2ScheduleType::Manual,
                cron_expression: None,
                interval_seconds: None,
                timezone: "UTC".to_string(),
                misfire_policy: Value::String("run_once".to_string()),
            },
            execution_target: "automation_v2".to_string(),
            workspace_root: "/tmp/project".to_string(),
            steps: vec![WorkflowPlanStep {
                step_id: "execute_goal".to_string(),
                kind: "execute".to_string(),
                objective: "Do the thing".to_string(),
                depends_on: Vec::new(),
                agent_role: "worker".to_string(),
                input_refs: Vec::new(),
                output_contract: Some(json!({"kind": "structured_json"})),
                metadata: Some(json!({"phase": "main"})),
            }],
            requires_integrations: Vec::new(),
            allowed_mcp_servers: vec!["github".to_string()],
            operator_preferences: Some(json!({
                "model_provider": "test-provider",
                "model_id": "test-model"
            })),
            save_options: json!({}),
        }
    }

    #[test]
    fn compile_workflow_runtime_projection_shapes_agents_and_nodes() {
        let projection = compile_workflow_runtime_projection(&test_plan(), |allowlist| allowlist);

        assert_eq!(projection.agents.len(), 1);
        assert_eq!(projection.agents[0].agent_id, "agent_worker");
        assert_eq!(projection.nodes.len(), 1);
        assert_eq!(projection.nodes[0].node_id, "execute_goal");
        assert_eq!(projection.nodes[0].timeout_ms, Some(1_800_000));
        assert_eq!(projection.execution.max_parallel_agents, Some(1));
        assert_eq!(projection.name, "Runtime Test");
        assert_eq!(
            projection
                .metadata
                .get("workflow_plan_id")
                .and_then(Value::as_str),
            Some("wfplan-test")
        );
        assert!(projection.output_targets.is_empty());
    }
}
