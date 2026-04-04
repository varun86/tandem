use futures::future::join_all;
use futures::FutureExt;
use serde_json::{json, Value};
use std::panic::AssertUnwindSafe;

use crate::app::state::AppState;
use crate::automation_v2::types::{AutomationRunStatus, AutomationStopKind};
use crate::util::time::now_ms;

#[derive(Debug, Clone, PartialEq, Eq)]
enum DerivedTerminalRunState {
    Completed,
    Blocked {
        blocked_nodes: Vec<String>,
        detail: String,
    },
    Failed {
        failed_nodes: Vec<String>,
        blocked_nodes: Vec<String>,
        detail: String,
    },
}

fn node_output_status(value: &Value) -> String {
    value
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn node_output_failure_kind(value: &Value) -> String {
    value
        .get("failure_kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn blocked_failure_kind(kind: &str) -> bool {
    matches!(
        kind,
        "research_retry_exhausted"
            | "editorial_quality_failed"
            | "semantic_blocked"
            | "review_not_approved"
            | "upstream_not_approved"
            | "artifact_rejected"
            | "unsafe_raw_write_rejected"
            | "placeholder_overwrite_rejected"
    )
}

/// Returns `true` when a node should be skipped entirely because an upstream
/// node that is marked as a triage gate found no work.
///
/// The triage node signals this by having `metadata.triage_gate == true` in
/// the automation spec, and outputting `{"content": {"has_work": false}}`.
/// When skipped, downstream nodes are also unconditionally skipped via the
/// same check (`should_skip_due_to_triage_gate` is called for every pending
/// node each loop iteration after the triage output lands).
fn should_skip_due_to_triage_gate(
    node: &crate::automation_v2::types::AutomationFlowNode,
    node_outputs: &std::collections::HashMap<String, serde_json::Value>,
    flow_nodes: &[crate::automation_v2::types::AutomationFlowNode],
) -> bool {
    for dep_id in &node.depends_on {
        // Only apply the skip when the dependency is itself a triage gate node.
        let dep_is_triage = flow_nodes
            .iter()
            .find(|n| &n.node_id == dep_id)
            .and_then(|n| n.metadata.as_ref())
            .and_then(|m| m.get("triage_gate"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !dep_is_triage {
            // Propagate: if the *dependency* was itself skipped due to triage,
            // its skip output also carries triage_skipped:true so this node
            // picks it up in the next iteration.
            let dep_triage_skipped = node_outputs
                .get(dep_id)
                .and_then(|o| o.get("triage_skipped"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if dep_triage_skipped {
                return true;
            }
            continue;
        }
        let has_work = node_outputs
            .get(dep_id)
            .and_then(|o| o.get("content"))
            .and_then(|c| c.get("has_work"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true); // default: proceed (don't skip) if field is absent
        if !has_work {
            return true;
        }
    }
    false
}

fn automation_activation_validation_failure(
    automation: &crate::automation_v2::types::AutomationV2Spec,
) -> Option<String> {
    let report = automation.plan_package_validation_report()?;
    if report.ready_for_activation {
        return None;
    }
    report
        .issues
        .iter()
        .find(|issue| issue.blocking)
        .map(|issue| {
            format!(
                "plan package not ready for activation: {} ({})",
                issue.message, issue.code
            )
        })
        .or_else(|| Some("plan package not ready for activation".to_string()))
}

fn derive_terminal_run_state(
    automation: &crate::automation_v2::types::AutomationV2Spec,
    run: &crate::automation_v2::types::AutomationV2RunRecord,
    deadlock: bool,
) -> DerivedTerminalRunState {
    let mut blocked_nodes = run.checkpoint.blocked_nodes.clone();
    blocked_nodes.extend(crate::app::state::automation_blocked_nodes(automation, run));
    let mut failed_nodes = Vec::new();
    let pending_nodes = run
        .checkpoint
        .pending_nodes
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for node in &automation.flow.nodes {
        let attempts = run
            .checkpoint
            .node_attempts
            .get(&node.node_id)
            .copied()
            .unwrap_or(0);
        let max_attempts = crate::app::state::automation_node_max_attempts(node);
        if pending_nodes.contains(&node.node_id) && attempts >= max_attempts {
            failed_nodes.push(node.node_id.clone());
        }
    }
    for (node_id, output) in &run.checkpoint.node_outputs {
        let status = node_output_status(output);
        let failure_kind = node_output_failure_kind(output);
        if matches!(status.as_str(), "failed" | "verify_failed")
            || failure_kind == "verification_failed"
            || failure_kind == "run_failed"
        {
            failed_nodes.push(node_id.clone());
            continue;
        }
        if status == "blocked" || blocked_failure_kind(&failure_kind) {
            blocked_nodes.push(node_id.clone());
        }
    }
    blocked_nodes.sort();
    blocked_nodes.dedup();
    failed_nodes.sort();
    failed_nodes.dedup();
    if !failed_nodes.is_empty() {
        let detail = format!(
            "automation run failed from node outcomes: {}",
            failed_nodes.join(", ")
        );
        return DerivedTerminalRunState::Failed {
            failed_nodes,
            blocked_nodes,
            detail,
        };
    }
    if !blocked_nodes.is_empty() {
        return DerivedTerminalRunState::Blocked {
            blocked_nodes,
            detail: if deadlock {
                "automation run blocked: no runnable nodes remain".to_string()
            } else {
                "automation run blocked by upstream node outcome".to_string()
            },
        };
    }
    if deadlock {
        DerivedTerminalRunState::Failed {
            failed_nodes: Vec::new(),
            blocked_nodes: Vec::new(),
            detail: "flow deadlock: no runnable nodes".to_string(),
        }
    } else {
        DerivedTerminalRunState::Completed
    }
}

pub async fn run_automation_v2_run(
    state: AppState,
    run: crate::automation_v2::types::AutomationV2RunRecord,
) {
    let run_id = run.run_id.clone();
    let Some(_runtime_context) = run.runtime_context.as_ref() else {
        let detail = "runtime context partition missing for automation run".to_string();
        let _ = state
            .update_automation_v2_run(&run.run_id, |row| {
                row.status = AutomationRunStatus::Failed;
                row.detail = Some(detail.clone());
            })
            .await;
        return;
    };
    let automation = state
        .get_automation_v2(&run.automation_id)
        .await
        .or_else(|| run.automation_snapshot.clone());
    let Some(automation) = automation else {
        let _ = state
            .update_automation_v2_run(&run.run_id, |row| {
                row.status = AutomationRunStatus::Failed;
                row.detail = Some("automation not found".to_string());
            })
            .await;
        return;
    };
    if let Some(detail) = automation_activation_validation_failure(&automation) {
        let _ = state
            .update_automation_v2_run(&run.run_id, |row| {
                row.status = AutomationRunStatus::Failed;
                row.detail = Some(detail.clone());
                row.stop_kind = Some(AutomationStopKind::GuardrailStopped);
                row.stop_reason = Some(detail.clone());
                crate::app::state::automation::lifecycle::record_automation_lifecycle_event(
                    row,
                    "run_failed_activation_validation",
                    Some(detail.clone()),
                    Some(AutomationStopKind::GuardrailStopped),
                );
            })
            .await;
        return;
    }
    if let Err(error) =
        crate::app::state::clear_automation_declared_outputs(&state, &automation, &run.run_id).await
    {
        let _ = state
            .update_automation_v2_run(&run.run_id, |row| {
                row.status = AutomationRunStatus::Failed;
                row.detail = Some(error.to_string());
            })
            .await;
        return;
    }
    let max_parallel = automation
        .execution
        .max_parallel_agents
        .unwrap_or(1)
        .clamp(1, 16) as usize;

    loop {
        let Some(latest) = state.get_automation_v2_run(&run_id).await else {
            break;
        };
        if latest.checkpoint.awaiting_gate.is_none() {
            let blocked_nodes = crate::app::state::automation_blocked_nodes(&automation, &latest);
            let _ = state
                .update_automation_v2_run(&run_id, |row| {
                    row.checkpoint.blocked_nodes = blocked_nodes.clone();
                    crate::app::state::record_automation_open_phase_event(&automation, row);
                })
                .await;
        }
        if let Some(detail) = crate::app::state::automation_guardrail_failure(&automation, &latest)
        {
            let session_ids = latest.active_session_ids.clone();
            for session_id in &session_ids {
                let _ = state.cancellations.cancel(&session_id).await;
            }
            state.forget_automation_v2_sessions(&session_ids).await;
            let instance_ids = latest.active_instance_ids.clone();
            for instance_id in instance_ids {
                let _ = state
                    .agent_teams
                    .cancel_instance(&state, &instance_id, "paused by budget guardrail")
                    .await;
            }
            let _ = state
                .update_automation_v2_run(&run_id, |row| {
                    row.status = AutomationRunStatus::Paused;
                    row.detail = Some(detail.clone());
                    row.pause_reason = Some(detail.clone());
                    row.stop_kind = Some(AutomationStopKind::GuardrailStopped);
                    row.stop_reason = Some(detail.clone());
                    row.active_session_ids.clear();
                    row.active_instance_ids.clear();
                    crate::app::state::automation::lifecycle::record_automation_lifecycle_event(
                        row,
                        "run_paused",
                        Some(detail.clone()),
                        Some(AutomationStopKind::GuardrailStopped),
                    );
                })
                .await;
            break;
        }
        if matches!(
            latest.status,
            AutomationRunStatus::Paused
                | AutomationRunStatus::Pausing
                | AutomationRunStatus::AwaitingApproval
                | AutomationRunStatus::Cancelled
                | AutomationRunStatus::Blocked
                | AutomationRunStatus::Failed
                | AutomationRunStatus::Completed
        ) {
            break;
        }
        if latest.checkpoint.pending_nodes.is_empty() {
            let terminal_state = derive_terminal_run_state(&automation, &latest, false);
            let _ = state
                .update_automation_v2_run(&run_id, |row| match &terminal_state {
                    DerivedTerminalRunState::Completed => {
                        row.status = AutomationRunStatus::Completed;
                        row.detail = Some("automation run completed".to_string());
                    }
                    DerivedTerminalRunState::Blocked {
                        blocked_nodes,
                        detail,
                    } => {
                        row.checkpoint.blocked_nodes = blocked_nodes.clone();
                        row.status = AutomationRunStatus::Blocked;
                        row.detail = Some(detail.clone());
                    }
                    DerivedTerminalRunState::Failed {
                        failed_nodes,
                        blocked_nodes,
                        detail,
                    } => {
                        row.checkpoint.blocked_nodes = blocked_nodes.clone();
                        if let Some(node_id) = failed_nodes.first() {
                            row.checkpoint.last_failure =
                                Some(crate::automation_v2::types::AutomationFailureRecord {
                                    node_id: node_id.clone(),
                                    reason: detail.clone(),
                                    failed_at_ms: now_ms(),
                                });
                        }
                        row.status = AutomationRunStatus::Failed;
                        row.detail = Some(detail.clone());
                    }
                })
                .await;
            break;
        }

        let completed = latest
            .checkpoint
            .completed_nodes
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let pending = latest.checkpoint.pending_nodes.clone();
        let mut triage_skipped_node_ids: Vec<String> = Vec::new();
        let mut runnable = pending
            .iter()
            .filter_map(|node_id| {
                let node = automation
                    .flow
                    .nodes
                    .iter()
                    .find(|n| n.node_id == *node_id)?;
                let attempts = latest
                    .checkpoint
                    .node_attempts
                    .get(node_id)
                    .copied()
                    .unwrap_or(0);
                let max_attempts = crate::app::state::automation_node_max_attempts(node);
                if attempts >= max_attempts {
                    return None;
                }
                // Dependency check: all deps must be completed.
                if !node.depends_on.iter().all(|dep| completed.contains(dep)) {
                    return None;
                }
                // Triage gate: skip if an upstream triage node found no work.
                if should_skip_due_to_triage_gate(
                    node,
                    &latest.checkpoint.node_outputs,
                    &automation.flow.nodes,
                ) {
                    triage_skipped_node_ids.push(node_id.clone());
                    return None;
                }
                Some(node.clone())
            })
            .collect::<Vec<_>>();
        // Apply triage skips before proceeding to phase/routine filtering.
        if !triage_skipped_node_ids.is_empty() {
            let _ = state
                .update_automation_v2_run(&run_id, |row| {
                    for node_id in &triage_skipped_node_ids {
                        row.checkpoint.pending_nodes.retain(|id| id != node_id);
                        if !row.checkpoint.completed_nodes.iter().any(|id| id == node_id) {
                            row.checkpoint.completed_nodes.push(node_id.clone());
                        }
                        row.checkpoint.node_outputs.insert(
                            node_id.clone(),
                            json!({
                                "status": "skipped",
                                "summary": "Skipped: upstream triage found no work.",
                                "triage_skipped": true,
                                "contract_kind": "text_summary",
                            }),
                        );
                        crate::app::state::automation::lifecycle::record_automation_lifecycle_event_with_metadata(
                            row,
                            "node_skipped_no_work",
                            Some(format!("node `{node_id}` skipped: upstream triage found no work")),
                            None,
                            Some(json!({ "node_id": node_id, "triage_skipped": true })),
                        );
                    }
                })
                .await;
            // Re-enter the loop so newly-depending nodes can be evaluated for skip.
            continue;
        }
        runnable = crate::app::state::automation_filter_runnable_by_open_phase(
            &automation,
            &latest,
            runnable,
        );
        runnable = crate::app::state::automation_filter_runnable_by_routine_dependencies(
            &automation,
            &latest,
            runnable,
        );
        let phase_rank = crate::app::state::automation_phase_rank_map(&automation);
        let current_open_phase_rank =
            crate::app::state::automation_current_open_phase(&automation, &latest)
                .map(|(_, rank, _)| rank);
        runnable.sort_by(|a, b| {
            crate::app::state::automation_node_sort_key(a, &phase_rank, current_open_phase_rank)
                .cmp(&crate::app::state::automation_node_sort_key(
                    b,
                    &phase_rank,
                    current_open_phase_rank,
                ))
        });
        let runnable = crate::app::state::automation_filter_runnable_by_write_scope_conflicts(
            runnable,
            max_parallel,
        );

        if runnable.is_empty() {
            let terminal_state = derive_terminal_run_state(&automation, &latest, true);
            let _ = state
                .update_automation_v2_run(&run_id, |row| match &terminal_state {
                    DerivedTerminalRunState::Completed => {
                        row.status = AutomationRunStatus::Completed;
                        row.detail = Some("automation run completed".to_string());
                    }
                    DerivedTerminalRunState::Blocked {
                        blocked_nodes,
                        detail,
                    } => {
                        row.checkpoint.blocked_nodes = blocked_nodes.clone();
                        row.status = AutomationRunStatus::Blocked;
                        row.detail = Some(detail.clone());
                    }
                    DerivedTerminalRunState::Failed {
                        failed_nodes,
                        blocked_nodes,
                        detail,
                    } => {
                        row.checkpoint.blocked_nodes = blocked_nodes.clone();
                        if let Some(node_id) = failed_nodes.first() {
                            row.checkpoint.last_failure =
                                Some(crate::automation_v2::types::AutomationFailureRecord {
                                    node_id: node_id.clone(),
                                    reason: detail.clone(),
                                    failed_at_ms: now_ms(),
                                });
                        }
                        row.status = AutomationRunStatus::Failed;
                        row.detail = Some(detail.clone());
                    }
                })
                .await;
            break;
        }

        let executable = runnable
            .iter()
            .filter(|node| !crate::app::state::is_automation_approval_node(node))
            .cloned()
            .collect::<Vec<_>>();
        if executable.is_empty() {
            if let Some(gate_node) = runnable
                .iter()
                .find(|node| crate::app::state::is_automation_approval_node(node))
            {
                let blocked_nodes = crate::app::state::collect_automation_descendants(
                    &automation,
                    &std::iter::once(gate_node.node_id.clone()).collect(),
                )
                .into_iter()
                .filter(|node_id| node_id != &gate_node.node_id)
                .collect::<Vec<_>>();
                let Some(gate) = crate::app::state::build_automation_pending_gate(gate_node) else {
                    let _ = state
                        .update_automation_v2_run(&run_id, |row| {
                            row.status = AutomationRunStatus::Failed;
                            row.detail = Some("approval node missing gate config".to_string());
                        })
                        .await;
                    break;
                };
                let _ = state
                    .update_automation_v2_run(&run_id, |row| {
                        row.status = AutomationRunStatus::AwaitingApproval;
                        row.detail = Some(format!("awaiting approval for gate `{}`", gate.node_id));
                        row.checkpoint.awaiting_gate = Some(gate.clone());
                        row.checkpoint.blocked_nodes = blocked_nodes.clone();
                    })
                    .await;
            }
            break;
        }

        let runnable_node_ids = executable
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<Vec<_>>();
        let _ = state
            .update_automation_v2_run(&run_id, |row| {
                for node_id in &runnable_node_ids {
                    let attempts = row.checkpoint.node_attempts.entry(node_id.clone()).or_insert(0);
                    *attempts += 1;
                }
                for node in &executable {
                    let attempt = row
                        .checkpoint
                        .node_attempts
                        .get(&node.node_id)
                        .copied()
                        .unwrap_or(0);
                    crate::app::state::automation::lifecycle::record_automation_lifecycle_event_with_metadata(
                        row,
                        "node_started",
                        Some(format!("node `{}` started", node.node_id)),
                        None,
                        Some(json!({
                            "node_id": node.node_id,
                            "agent_id": node.agent_id,
                            "objective": node.objective,
                            "attempt": attempt,
                        })),
                    );
                }
            })
            .await;

        let tasks = executable
            .iter()
            .map(|node| {
                let Some(agent) = automation
                    .agents
                    .iter()
                    .find(|a| a.agent_id == node.agent_id)
                    .cloned()
                else {
                    return futures::future::ready((
                        node.node_id.clone(),
                        Err(anyhow::anyhow!("agent not found")),
                    ))
                    .boxed();
                };
                let state = state.clone();
                let run_id = run_id.clone();
                let automation = automation.clone();
                let node = node.clone();
                async move {
                    let result = AssertUnwindSafe(crate::app::state::execute_automation_v2_node(
                        &state,
                        &run_id,
                        &automation,
                        &node,
                        &agent,
                    ))
                    .catch_unwind()
                    .await
                    .map_err(|panic_payload| {
                        let detail = if let Some(message) = panic_payload.downcast_ref::<&str>() {
                            (*message).to_string()
                        } else if let Some(message) = panic_payload.downcast_ref::<String>() {
                            message.clone()
                        } else {
                            "unknown panic".to_string()
                        };
                        anyhow::anyhow!("node execution panicked: {}", detail)
                    })
                    .and_then(|result| result);
                    (node.node_id, result)
                }
                .boxed()
            })
            .collect::<Vec<_>>();
        let outcomes = join_all(tasks).await;

        let mut terminal_failure = None::<String>;
        let latest_attempts = state
            .get_automation_v2_run(&run_id)
            .await
            .map(|row| row.checkpoint.node_attempts)
            .unwrap_or_default();
        for (node_id, result) in outcomes {
            match result {
                Ok(output) => {
                    let mut output = output;
                    if let Some((path, content_digest)) =
                        crate::app::state::automation::node_output::automation_output_validated_artifact(
                            &output,
                        )
                    {
                        let mut scheduler = state.automation_scheduler.write().await;
                        scheduler.preexisting_registry.register_validated(
                            &run_id,
                            &node_id,
                            crate::app::state::automation::scheduler::ValidatedArtifact {
                                path,
                                content_digest,
                            },
                        );
                    }
                    let can_accept = state
                        .get_automation_v2_run(&run_id)
                        .await
                        .map(|row| {
                            matches!(
                                row.status,
                                AutomationRunStatus::Running | AutomationRunStatus::Queued
                            )
                        })
                        .unwrap_or(false);
                    if !can_accept {
                        continue;
                    }
                    let session_id = crate::app::state::automation_output_session_id(&output);
                    let summary = output
                        .get("summary")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default()
                        .to_string();
                    let contract_kind = output
                        .get("contract_kind")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default()
                        .to_string();
                    let blocked = crate::app::state::automation_output_is_blocked(&output);
                    let verify_failed =
                        crate::app::state::automation_output_is_verify_failed(&output);
                    let needs_repair = crate::app::state::automation_output_needs_repair(&output);
                    let has_warnings = crate::app::state::automation_output_has_warnings(&output);
                    let repair_exhausted =
                        crate::app::state::automation_output_repair_exhausted(&output);
                    let mut blocked_reason =
                        crate::app::state::automation_output_blocked_reason(&output);
                    let mut failure_reason =
                        crate::app::state::automation_output_failure_reason(&output);
                    let attempt = latest_attempts.get(&node_id).copied().unwrap_or(1);
                    let max_attempts = automation
                        .flow
                        .nodes
                        .iter()
                        .find(|row| row.node_id == node_id)
                        .map(crate::app::state::automation_node_max_attempts)
                        .unwrap_or(1);
                    let terminal_repair_block =
                        needs_repair && (repair_exhausted || attempt >= max_attempts);
                    if terminal_repair_block {
                        if let Some(object) = output.as_object_mut() {
                            object.insert("status".to_string(), json!("blocked"));
                            if object
                                .get("blocked_reason")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .unwrap_or_default()
                                .is_empty()
                            {
                                object.insert(
                                    "blocked_reason".to_string(),
                                    json!(failure_reason.clone().unwrap_or_else(|| {
                                        format!(
                                            "node `{}` exhausted repair attempts without satisfying validation",
                                            node_id
                                        )
                                    })),
                                );
                            }
                        }
                        blocked_reason =
                            crate::app::state::automation_output_blocked_reason(&output);
                        failure_reason =
                            crate::app::state::automation_output_failure_reason(&output);
                    }
                    let blocked = blocked || terminal_repair_block;
                    let is_approval_rejected = blocked
                        && !verify_failed
                        && output.get("approved").and_then(serde_json::Value::as_bool)
                            == Some(false);
                    let _ = state
                        .update_automation_v2_run(&run_id, |row| {
                            if is_approval_rejected {
                                let ancestors =
                                    crate::app::state::collect_automation_ancestors(&automation, &node_id);
                                let resettable_ancestors: Vec<String> = ancestors
                                    .iter()
                                    .filter(|anc_id| row.checkpoint.completed_nodes.contains(*anc_id))
                                    .filter(|anc_id| {
                                        let used =
                                            row.checkpoint.node_attempts.get(*anc_id).copied().unwrap_or(0);
                                        let anc_max = automation
                                            .flow
                                            .nodes
                                            .iter()
                                            .find(|n| n.node_id == **anc_id)
                                            .map(crate::app::state::automation_node_max_attempts)
                                            .unwrap_or(1);
                                        used < anc_max
                                    })
                                    .cloned()
                                    .collect();
                                if !resettable_ancestors.is_empty() {
                                    let reset_roots: std::collections::HashSet<String> =
                                        resettable_ancestors.iter().cloned().collect();
                                    let nodes_to_reset =
                                        crate::app::state::collect_automation_descendants(
                                            &automation,
                                            &reset_roots,
                                        );
                                    let mut reset_list =
                                        nodes_to_reset.iter().cloned().collect::<Vec<_>>();
                                    reset_list.sort();
                                    for reset_id in &nodes_to_reset {
                                        row.checkpoint.node_outputs.remove(reset_id);
                                        row.checkpoint.completed_nodes.retain(|id| id != reset_id);
                                        row.checkpoint.blocked_nodes.retain(|id| id != reset_id);
                                        if !row.checkpoint.pending_nodes.iter().any(|id| id == reset_id) {
                                            row.checkpoint.pending_nodes.push(reset_id.clone());
                                        }
                                    }
                                    row.status = AutomationRunStatus::Queued;
                                    row.checkpoint.last_failure = None;
                                    crate::app::state::automation::lifecycle::record_automation_lifecycle_event_with_metadata(
                                        row,
                                        "node_approval_rollback",
                                        Some(format!(
                                            "node `{}` rejected upstream output; rolling back and re-queuing: {}",
                                            node_id,
                                            reset_list.join(", ")
                                        )),
                                        None,
                                        Some(json!({
                                            "node_id": node_id,
                                            "attempt": attempt,
                                            "session_id": session_id,
                                            "reset_nodes": reset_list,
                                            "blocked_reason": blocked_reason,
                                        })),
                                    );
                                    crate::app::state::refresh_automation_runtime_state(&automation, row);
                                    return;
                                }
                            }
                            let blocked_descendants = if blocked || verify_failed {
                                crate::app::state::collect_automation_descendants(
                                    &automation,
                                    &std::iter::once(node_id.clone()).collect(),
                                )
                            } else {
                                std::collections::HashSet::new()
                            };
                            row.checkpoint.pending_nodes.retain(|id| {
                                if id == &node_id {
                                    needs_repair && !terminal_repair_block
                                } else {
                                    !blocked_descendants.contains(id)
                                }
                            });
                            if needs_repair && !terminal_repair_block {
                                if !crate::app::state::automation_node_has_passing_artifact(
                                    &node_id,
                                    &row.checkpoint,
                                ) {
                                    if !row.checkpoint.pending_nodes.iter().any(|id| id == &node_id) {
                                        row.checkpoint.pending_nodes.push(node_id.clone());
                                    }
                                }
                            }
                            if !blocked
                                && !needs_repair
                                && !verify_failed
                                && !row.checkpoint.completed_nodes.iter().any(|id| id == &node_id)
                            {
                                row.checkpoint.completed_nodes.push(node_id.clone());
                            }
                            if blocked {
                                if !row.checkpoint.blocked_nodes.iter().any(|id| id == &node_id) {
                                    row.checkpoint.blocked_nodes.push(node_id.clone());
                                }
                                for blocked_node in &blocked_descendants {
                                    if !row.checkpoint.blocked_nodes.iter().any(|id| id == blocked_node) {
                                        row.checkpoint.blocked_nodes.push(blocked_node.clone());
                                    }
                                }
                            }
                            row.checkpoint.node_outputs.insert(node_id.clone(), output.clone());
                            if !verify_failed
                                && row
                                    .checkpoint
                                    .last_failure
                                    .as_ref()
                                    .is_some_and(|failure| failure.node_id == node_id)
                            {
                                row.checkpoint.last_failure = None;
                            }
                            if verify_failed {
                                row.checkpoint.last_failure = Some(
                                    crate::automation_v2::types::AutomationFailureRecord {
                                        node_id: node_id.clone(),
                                        reason: failure_reason.clone().unwrap_or_else(|| {
                                            "verification failed".to_string()
                                        }),
                                        failed_at_ms: now_ms(),
                                    },
                                );
                            }
                            crate::app::state::automation::lifecycle::record_automation_workflow_state_events(
                                row,
                                &node_id,
                                &output,
                                attempt,
                                session_id.as_deref(),
                                &summary,
                                &contract_kind,
                            );
                            crate::app::state::automation::lifecycle::record_automation_lifecycle_event_with_metadata(
                                row,
                                if verify_failed {
                                    "node_verify_failed"
                                } else if needs_repair && !terminal_repair_block {
                                    "node_repair_requested"
                                } else if has_warnings {
                                    "node_completed_with_warnings"
                                } else if blocked {
                                    "node_blocked"
                                } else {
                                    "node_completed"
                                },
                                Some(if verify_failed {
                                    format!("node `{}` failed verification", node_id)
                                } else if needs_repair && !terminal_repair_block {
                                    format!("node `{}` requested another repair attempt", node_id)
                                } else if has_warnings {
                                    format!("node `{}` completed with warnings", node_id)
                                } else if blocked {
                                    format!("node `{}` blocked downstream execution", node_id)
                                } else {
                                    format!("node `{}` completed", node_id)
                                }),
                                None,
                                Some(json!({
                                    "node_id": node_id,
                                    "attempt": attempt,
                                    "session_id": session_id,
                                    "summary": summary,
                                    "contract_kind": contract_kind,
                                    "status": if verify_failed {
                                        "verify_failed"
                                    } else if needs_repair && !terminal_repair_block {
                                        "needs_repair"
                                    } else if has_warnings {
                                        "completed_with_warnings"
                                    } else if blocked {
                                        "blocked"
                                    } else {
                                        "completed"
                                    },
                                    "warning_count": output
                                        .get("artifact_validation")
                                        .and_then(|value| value.get("warning_count"))
                                        .and_then(Value::as_u64),
                                    "max_attempts": max_attempts,
                                    "blocked_reason": blocked_reason,
                                    "failure_reason": failure_reason,
                                    "blocked_descendants": blocked_descendants,
                                })),
                            );
                            if !blocked && !needs_repair && !verify_failed {
                                crate::app::state::record_milestone_promotions(&automation, row, &node_id);
                            }
                            crate::app::state::refresh_automation_runtime_state(&automation, row);
                        })
                        .await;
                    if verify_failed {
                        terminal_failure =
                            Some(failure_reason.unwrap_or_else(|| {
                                format!("node `{}` failed verification", node_id)
                            }));
                        let _ = state
                            .update_automation_v2_run(&run_id, |row| {
                                row.status = AutomationRunStatus::Failed;
                                row.detail = terminal_failure.clone();
                            })
                            .await;
                        break;
                    }
                }
                Err(error) => {
                    let should_ignore = state
                        .get_automation_v2_run(&run_id)
                        .await
                        .map(|row| {
                            matches!(
                                row.status,
                                AutomationRunStatus::Paused
                                    | AutomationRunStatus::Pausing
                                    | AutomationRunStatus::AwaitingApproval
                                    | AutomationRunStatus::Cancelled
                                    | AutomationRunStatus::Blocked
                                    | AutomationRunStatus::Failed
                                    | AutomationRunStatus::Completed
                            )
                        })
                        .unwrap_or(false);
                    if should_ignore {
                        continue;
                    }
                    let detail = crate::app::state::truncate_text(&error.to_string(), 500);
                    let attempts = latest_attempts.get(&node_id).copied().unwrap_or(1);
                    let max_attempts = automation
                        .flow
                        .nodes
                        .iter()
                        .find(|row| row.node_id == node_id)
                        .map(crate::app::state::automation_node_max_attempts)
                        .unwrap_or(1);
                    let terminal = attempts >= max_attempts;
                    let _ = state
                        .update_automation_v2_run(&run_id, |row| {
                            crate::app::state::automation::lifecycle::record_automation_lifecycle_event_with_metadata(
                                row,
                                "node_failed",
                                Some(format!("node `{}` failed", node_id)),
                                None,
                                Some(json!({
                                    "node_id": node_id,
                                    "attempt": attempts,
                                    "max_attempts": max_attempts,
                                    "reason": detail,
                                    "terminal": terminal,
                                })),
                            );
                        })
                        .await;
                    if terminal {
                        terminal_failure = Some(format!(
                            "node `{}` failed after {}/{} attempts: {}",
                            node_id, attempts, max_attempts, detail
                        ));
                        let _ = state
                            .update_automation_v2_run(&run_id, |row| {
                                row.checkpoint.last_failure =
                                    Some(crate::automation_v2::types::AutomationFailureRecord {
                                        node_id: node_id.clone(),
                                        reason: detail.clone(),
                                        failed_at_ms: now_ms(),
                                    });
                            })
                            .await;
                        break;
                    }
                    let _ = state
                        .update_automation_v2_run(&run_id, |row| {
                            row.detail = Some(format!(
                                "retrying node `{}` after attempt {}/{} failed: {}",
                                node_id, attempts, max_attempts, detail
                            ));
                        })
                        .await;
                }
            }
        }
        if let Some(detail) = terminal_failure {
            let _ = state
                .update_automation_v2_run(&run_id, |row| {
                    row.status = AutomationRunStatus::Failed;
                    row.detail = Some(detail);
                })
                .await;
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_automation() -> crate::automation_v2::types::AutomationV2Spec {
        crate::automation_v2::types::AutomationV2Spec {
            knowledge: tandem_orchestrator::KnowledgeBinding::default(),
            automation_id: "automation-test".to_string(),
            name: "test".to_string(),
            description: None,
            status: crate::automation_v2::types::AutomationV2Status::Active,
            schedule: crate::automation_v2::types::AutomationV2Schedule {
                schedule_type: crate::automation_v2::types::AutomationV2ScheduleType::Manual,
                cron_expression: None,
                interval_seconds: None,
                timezone: "UTC".to_string(),
                misfire_policy: crate::RoutineMisfirePolicy::Skip,
            },
            agents: Vec::new(),
            flow: crate::automation_v2::types::AutomationFlowSpec {
                nodes: vec![crate::automation_v2::types::AutomationFlowNode {
                    knowledge: tandem_orchestrator::KnowledgeBinding::default(),
                    node_id: "research-brief".to_string(),
                    agent_id: "research".to_string(),
                    objective: "Research".to_string(),
                    depends_on: Vec::new(),
                    input_refs: Vec::new(),
                    output_contract: None,
                    retry_policy: None,
                    timeout_ms: None,
                    stage_kind: None,
                    gate: None,
                    metadata: None,
                }],
            },
            execution: crate::automation_v2::types::AutomationExecutionPolicy {
                max_parallel_agents: None,
                max_total_runtime_ms: None,
                max_total_tool_calls: None,
                max_total_tokens: None,
                max_total_cost_usd: None,
            },
            output_targets: Vec::new(),
            created_at_ms: 0,
            updated_at_ms: 0,
            creator_id: "tests".to_string(),
            workspace_root: None,
            metadata: None,
            next_fire_at_ms: None,
            last_fired_at_ms: None,
        }
    }

    fn test_run_with_output(output: Value) -> crate::automation_v2::types::AutomationV2RunRecord {
        crate::automation_v2::types::AutomationV2RunRecord {
            run_id: "run-test".to_string(),
            automation_id: "automation-test".to_string(),
            trigger_type: "manual".to_string(),
            status: crate::automation_v2::types::AutomationRunStatus::Running,
            created_at_ms: 0,
            updated_at_ms: 0,
            started_at_ms: Some(0),
            finished_at_ms: None,
            active_session_ids: Vec::new(),
            latest_session_id: None,
            active_instance_ids: Vec::new(),
            checkpoint: crate::automation_v2::types::AutomationRunCheckpoint {
                completed_nodes: Vec::new(),
                pending_nodes: Vec::new(),
                node_outputs: std::collections::HashMap::from([(
                    "research-brief".to_string(),
                    output,
                )]),
                node_attempts: std::collections::HashMap::new(),
                blocked_nodes: Vec::new(),
                awaiting_gate: None,
                gate_history: Vec::new(),
                lifecycle_history: Vec::new(),
                last_failure: None,
            },
            runtime_context: None,
            automation_snapshot: None,
            pause_reason: None,
            resume_reason: None,
            detail: None,
            stop_kind: None,
            stop_reason: None,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            estimated_cost_usd: 0.0,
            scheduler: None,
        }
    }

    #[test]
    fn derive_terminal_run_state_marks_blocked_outputs_as_blocked() {
        let automation = test_automation();
        let run = test_run_with_output(json!({
            "status": "blocked",
            "failure_kind": "research_citations_missing",
        }));
        assert_eq!(
            derive_terminal_run_state(&automation, &run, false),
            DerivedTerminalRunState::Blocked {
                blocked_nodes: vec!["research-brief".to_string()],
                detail: "automation run blocked by upstream node outcome".to_string(),
            }
        );
    }

    #[test]
    fn derive_terminal_run_state_marks_verify_failed_outputs_as_failed() {
        let automation = test_automation();
        let run = test_run_with_output(json!({
            "status": "verify_failed",
            "failure_kind": "verification_failed",
        }));
        assert_eq!(
            derive_terminal_run_state(&automation, &run, false),
            DerivedTerminalRunState::Failed {
                failed_nodes: vec!["research-brief".to_string()],
                blocked_nodes: Vec::new(),
                detail: "automation run failed from node outcomes: research-brief".to_string(),
            }
        );
    }

    #[test]
    fn derive_terminal_run_state_does_not_block_repairable_research_outputs() {
        let automation = test_automation();
        let run = test_run_with_output(json!({
            "status": "needs_repair",
            "failure_kind": "research_missing_reads",
            "artifact_validation": {
                "repair_exhausted": false
            }
        }));
        assert_eq!(
            derive_terminal_run_state(&automation, &run, false),
            DerivedTerminalRunState::Completed
        );
    }

    #[test]
    fn derive_terminal_run_state_fails_pending_nodes_that_exhausted_attempts() {
        let automation = test_automation();
        let mut run = test_run_with_output(json!({
            "status": "completed",
        }));
        run.checkpoint.node_outputs.clear();
        run.checkpoint.pending_nodes = vec!["research-brief".to_string()];
        run.checkpoint
            .node_attempts
            .insert("research-brief".to_string(), 3);

        assert_eq!(
            derive_terminal_run_state(&automation, &run, true),
            DerivedTerminalRunState::Failed {
                failed_nodes: vec!["research-brief".to_string()],
                blocked_nodes: Vec::new(),
                detail: "automation run failed from node outcomes: research-brief".to_string(),
            }
        );
    }
}
