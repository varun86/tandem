fn bug_monitor_triage_manual_schedule() -> crate::AutomationV2Schedule {
    crate::AutomationV2Schedule {
        schedule_type: crate::AutomationV2ScheduleType::Manual,
        cron_expression: None,
        interval_seconds: None,
        timezone: "UTC".to_string(),
        misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
    }
}

fn bug_monitor_triage_output_contract(
    artifact_type: &str,
    summary_guidance: &str,
) -> crate::AutomationFlowOutputContract {
    crate::AutomationFlowOutputContract {
        kind: "structured_json".to_string(),
        validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
        enforcement: None,
        schema: None,
        summary_guidance: Some(summary_guidance.to_string()),
    }
}

fn bug_monitor_triage_node(
    node_id: &str,
    agent_id: &str,
    objective: &str,
    depends_on: Vec<String>,
    timeout_ms: u64,
    artifact_path: &str,
    artifact_type: &str,
    payload: serde_json::Value,
) -> crate::AutomationFlowNode {
    crate::AutomationFlowNode {
        node_id: node_id.to_string(),
        agent_id: agent_id.to_string(),
        objective: objective.to_string(),
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        depends_on,
        input_refs: Vec::new(),
        output_contract: Some(bug_monitor_triage_output_contract(
            artifact_type,
            "Write a completed JSON artifact with concrete evidence, file references where available, uncertainty, and bounded next steps.",
        )),
        retry_policy: Some(json!({
            "max_attempts": 2,
            "backoff_ms": 10_000,
        })),
        timeout_ms: Some(timeout_ms),
        max_tool_calls: Some(24),
        stage_kind: Some(crate::AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": artifact_type,
                "title": objective,
                "output_path": artifact_path,
                "knowledge": {
                    "subject": objective,
                    "payload": payload,
                },
            },
            "bug_monitor": {
                "artifact_type": artifact_type,
            },
        })),
    }
}

pub(crate) fn bug_monitor_triage_spec(
    draft: &BugMonitorDraftRecord,
    workspace_root: Option<String>,
    model_policy: Option<serde_json::Value>,
    mcp_servers: Vec<String>,
    inspection_payload: serde_json::Value,
    research_payload: serde_json::Value,
    validation_payload: serde_json::Value,
    fix_payload: serde_json::Value,
) -> crate::AutomationV2Spec {
    let now = crate::now_ms();
    let automation_id = format!("automation-v2-bug-monitor-triage-{}", draft.draft_id);
    let model_policy = model_policy.or_else(|| {
        Some(json!({
            "default_model": "system_default",
        }))
    });
    crate::AutomationV2Spec {
        automation_id,
        name: format!(
            "Bug Monitor triage: {}",
            draft.title.as_deref().unwrap_or("runtime failure").trim()
        ),
        description: Some(format!(
            "Inspect, research, validate, and propose a fix for Bug Monitor draft {}.",
            draft.draft_id
        )),
        status: crate::AutomationV2Status::Active,
        schedule: bug_monitor_triage_manual_schedule(),
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        agents: vec![crate::AutomationAgentProfile {
            agent_id: "bug_monitor_triage_agent".to_string(),
            template_id: None,
            display_name: "Bug Monitor Triage".to_string(),
            avatar_url: None,
            model_policy,
            skills: Vec::new(),
            tool_policy: crate::AutomationAgentToolPolicy {
                allowlist: vec![
                    "read".to_string(),
                    "grep".to_string(),
                    "glob".to_string(),
                    "codesearch".to_string(),
                    "mcp_list".to_string(),
                    "write".to_string(),
                    "web_search".to_string(),
                ],
                denylist: vec!["edit".to_string(), "apply_patch".to_string()],
            },
            mcp_policy: crate::AutomationAgentMcpPolicy {
                allowed_servers: mcp_servers,
                allowed_tools: None,
            },
            approval_policy: None,
        }],
        flow: crate::AutomationFlowSpec {
            nodes: vec![
                bug_monitor_triage_node(
                    "inspect_failure_report",
                    "bug_monitor_triage_agent",
                    "Inspect failure report and affected area",
                    Vec::new(),
                    240_000,
                    ".tandem/artifacts/bug_monitor.inspection.json",
                    "bug_monitor_inspection",
                    inspection_payload,
                ),
                bug_monitor_triage_node(
                    "research_likely_root_cause",
                    "bug_monitor_triage_agent",
                    "Research likely root cause and related failures",
                    vec!["inspect_failure_report".to_string()],
                    600_000,
                    ".tandem/artifacts/bug_monitor.research.json",
                    "bug_monitor_research",
                    research_payload,
                ),
                bug_monitor_triage_node(
                    "validate_failure_scope",
                    "bug_monitor_triage_agent",
                    "Validate or reproduce failure scope",
                    vec!["research_likely_root_cause".to_string()],
                    240_000,
                    ".tandem/artifacts/bug_monitor.validation.json",
                    "bug_monitor_validation",
                    validation_payload,
                ),
                bug_monitor_triage_node(
                    "propose_fix_and_verification",
                    "bug_monitor_triage_agent",
                    "Propose fix and verification plan",
                    vec!["validate_failure_scope".to_string()],
                    360_000,
                    ".tandem/artifacts/bug_monitor.fix_proposal.json",
                    "bug_monitor_fix_proposal",
                    fix_payload,
                ),
            ],
        },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: Some(1_800_000),
            max_total_tool_calls: Some(96),
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: now,
        updated_at_ms: now,
        creator_id: "bug_monitor".to_string(),
        workspace_root,
        metadata: Some(json!({
            "source": "bug_monitor",
            "draft_id": draft.draft_id,
            "repo": draft.repo,
            "fingerprint": draft.fingerprint,
        })),
        next_fire_at_ms: None,
        last_fired_at_ms: None,
        scope_policy: None,
        watch_conditions: Vec::new(),
        handoff_config: None,
    }
}

pub(crate) fn bug_monitor_automation_run_id_from_triage_run_id(
    triage_run_id: &str,
) -> Option<String> {
    triage_run_id
        .strip_prefix("automation-v2-automation-v2-run-")
        .map(|suffix| format!("automation-v2-run-{suffix}"))
}

pub(crate) fn bug_monitor_triage_context_run_id(run_id: &str) -> String {
    super::context_runs::automation_v2_context_run_id(run_id)
}

pub(crate) async fn bug_monitor_triage_effective_started_at_ms(
    state: &AppState,
    triage_run_id: &str,
) -> Option<u64> {
    if let Some(run_id) = bug_monitor_automation_run_id_from_triage_run_id(triage_run_id) {
        return state
            .get_automation_v2_run(&run_id)
            .await
            .map(|run| run.started_at_ms.unwrap_or(run.created_at_ms));
    }
    super::context_runs::context_run_effective_started_at_ms(state, triage_run_id)
        .await
        .ok()
}

/// True if the triage run has reached a terminal status. Missing or
/// corrupt run state is treated as non-terminal so the timeout recovery
/// can mark the draft and publish the evidence-rich fallback body.
pub(crate) async fn bug_monitor_triage_run_is_terminal(state: &AppState, run_id: &str) -> bool {
    if let Some(automation_run_id) = bug_monitor_automation_run_id_from_triage_run_id(run_id) {
        return state
            .get_automation_v2_run(&automation_run_id)
            .await
            .map(|run| {
                matches!(
                    run.status,
                    crate::AutomationRunStatus::Completed
                        | crate::AutomationRunStatus::Failed
                        | crate::AutomationRunStatus::Cancelled
                )
            })
            .unwrap_or(false);
    }
    match load_context_run_state(state, run_id).await {
        Ok(run) => super::context_runs::context_run_is_terminal(&run.status),
        Err(_) => false,
    }
}

pub(crate) async fn bug_monitor_triage_timeout_diagnostics(
    state: &AppState,
    run_id: &str,
    timeout_ms: u64,
) -> Option<serde_json::Value> {
    if let Some(automation_run_id) = bug_monitor_automation_run_id_from_triage_run_id(run_id) {
        return bug_monitor_automation_triage_timeout_diagnostics(
            state,
            &automation_run_id,
            timeout_ms,
        )
        .await;
    }
    super::context_runs::bug_monitor_triage_timeout_diagnostics(state, run_id, timeout_ms).await
}

async fn bug_monitor_automation_triage_timeout_diagnostics(
    state: &AppState,
    run_id: &str,
    timeout_ms: u64,
) -> Option<serde_json::Value> {
    let run = state.get_automation_v2_run(run_id).await?;
    let now = crate::now_ms();
    let started_at_ms = run.started_at_ms.unwrap_or(run.created_at_ms);
    let elapsed_ms = now.saturating_sub(started_at_ms);
    let stale_ms = now.saturating_sub(run.updated_at_ms);
    let status = serde_json::to_value(&run.status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string());
    let total_steps = run
        .automation_snapshot
        .as_ref()
        .map(|automation| automation.flow.nodes.len())
        .unwrap_or_else(|| {
            run.checkpoint
                .completed_nodes
                .len()
                .saturating_add(run.checkpoint.pending_nodes.len())
                .saturating_add(run.checkpoint.blocked_nodes.len())
        });
    let completed_steps = run.checkpoint.completed_nodes.len();
    let failed_steps = usize::from(run.checkpoint.last_failure.is_some());
    let active_step = run
        .checkpoint
        .pending_nodes
        .first()
        .or_else(|| run.checkpoint.blocked_nodes.first())
        .map(|node_id| {
            json!({
                "step_id": node_id,
                "title": node_id,
                "status": if run.checkpoint.blocked_nodes.contains(node_id) {
                    "blocked"
                } else {
                    "pending"
                },
            })
        });
    Some(json!({
        "run_id": run.run_id,
        "context_run_id": bug_monitor_triage_context_run_id(&run.run_id),
        "run_status": status,
        "timeout_ms": timeout_ms,
        "elapsed_ms": elapsed_ms,
        "stale_ms": stale_ms,
        "last_event_seq": Value::Null,
        "step_count": total_steps,
        "completed_steps": completed_steps,
        "failed_steps": failed_steps,
        "active_step": active_step,
        "last_failure": run.checkpoint.last_failure,
    }))
}

#[cfg(test)]
mod bug_monitor_triage_spec_tests {
    use super::*;

    #[test]
    fn triage_spec_has_four_nodes_with_correct_dependencies() {
        let draft = BugMonitorDraftRecord {
            draft_id: "draft-1".to_string(),
            repo: "owner/repo".to_string(),
            title: Some("Failure".to_string()),
            detail: Some("detail".to_string()),
            fingerprint: "fp".to_string(),
            ..Default::default()
        };
        let spec = bug_monitor_triage_spec(
            &draft,
            Some("/tmp/workspace".to_string()),
            None,
            Vec::new(),
            json!({}),
            json!({}),
            json!({}),
            json!({}),
        );

        assert_eq!(spec.flow.nodes.len(), 4);
        assert_eq!(spec.flow.nodes[0].node_id, "inspect_failure_report");
        assert!(spec.flow.nodes[0].depends_on.is_empty());
        assert_eq!(
            spec.flow.nodes[1].depends_on,
            vec!["inspect_failure_report".to_string()]
        );
        assert_eq!(
            spec.flow.nodes[2].depends_on,
            vec!["research_likely_root_cause".to_string()]
        );
        assert_eq!(
            spec.flow.nodes[3].depends_on,
            vec!["validate_failure_scope".to_string()]
        );
        assert_eq!(spec.flow.nodes[1].timeout_ms, Some(600_000));
        assert!(spec.flow.nodes.iter().all(|node| {
            node.output_contract
                .as_ref()
                .and_then(|contract| contract.validator)
                == Some(crate::AutomationOutputValidatorKind::StructuredJson)
        }));
    }
}
