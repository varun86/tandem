use crate::automation_v2::types::AutomationFlowNode;
use serde_json::Value;

pub(crate) fn render_automation_repair_brief(
    node: &AutomationFlowNode,
    prior_output: Option<&Value>,
    attempt: u32,
    max_attempts: u32,
) -> Option<String> {
    if attempt <= 1 {
        return None;
    }
    let prior_output = prior_output?;
    if !automation_output_needs_repair(prior_output) {
        return None;
    }

    let validator_summary = prior_output.get("validator_summary");
    let artifact_validation = prior_output.get("artifact_validation");
    let tool_telemetry = prior_output.get("tool_telemetry");
    let validator_outcome = validator_summary
        .and_then(|value| value.get("outcome"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let unmet_requirements_from_summary = validator_summary
        .and_then(|value| value.get("unmet_requirements"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let is_upstream_passed = validator_outcome
        .is_some_and(|outcome| outcome.eq_ignore_ascii_case("passed"))
        && unmet_requirements_from_summary.is_empty();
    if is_upstream_passed {
        return None;
    }
    let reason = validator_summary
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            artifact_validation
                .and_then(|value| value.get("semantic_block_reason"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("the previous attempt did not satisfy the runtime validator");
    let unmet_requirements = unmet_requirements_from_summary;
    let blocking_classification = artifact_validation
        .and_then(|value| value.get("blocking_classification"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unspecified");
    let required_next_tool_actions = artifact_validation
        .and_then(|value| value.get("required_next_tool_actions"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let validation_basis = artifact_validation
        .and_then(|value| value.get("validation_basis"))
        .and_then(Value::as_object);
    let validation_basis_line = validation_basis
        .map(|basis| {
            let authority = basis
                .get("authority")
                .and_then(Value::as_str)
                .unwrap_or("unspecified");
            let current_attempt_output_materialized = basis
                .get("current_attempt_output_materialized")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let current_attempt_has_recorded_activity = basis
                .get("current_attempt_has_recorded_activity")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let current_attempt_has_read = basis
                .get("current_attempt_has_read")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let current_attempt_has_web_research = basis
                .get("current_attempt_has_web_research")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let workspace_inspection_satisfied = basis
                .get("workspace_inspection_satisfied")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            format!(
                "authority={}, output_materialized={}, recorded_activity={}, read={}, web_research={}, workspace_inspection={}",
                authority,
                current_attempt_output_materialized,
                current_attempt_has_recorded_activity,
                current_attempt_has_read,
                current_attempt_has_web_research,
                workspace_inspection_satisfied
            )
        })
        .unwrap_or_else(|| "none recorded".to_string());
    let tools_offered = tool_telemetry
        .and_then(|value| value.get("requested_tools"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tools_executed = tool_telemetry
        .and_then(|value| value.get("executed_tools"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let unreviewed_relevant_paths = artifact_validation
        .and_then(|value| value.get("unreviewed_relevant_paths"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let repair_attempt = artifact_validation
        .and_then(|value| value.get("repair_attempt"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(attempt.saturating_sub(1));
    let repair_attempts_remaining = artifact_validation
        .and_then(|value| value.get("repair_attempts_remaining"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_else(|| max_attempts.saturating_sub(attempt.saturating_sub(1)));

    let unmet_line = if unmet_requirements.is_empty() {
        "none recorded".to_string()
    } else {
        unmet_requirements.join(", ")
    };
    let tools_offered_line = if tools_offered.is_empty() {
        "none recorded".to_string()
    } else {
        tools_offered.join(", ")
    };
    let tools_executed_line = if tools_executed.is_empty() {
        "none recorded".to_string()
    } else {
        tools_executed.join(", ")
    };
    let unreviewed_line = if unreviewed_relevant_paths.is_empty() {
        "none recorded".to_string()
    } else {
        unreviewed_relevant_paths.join(", ")
    };
    let next_actions_line = if required_next_tool_actions.is_empty() {
        "none recorded".to_string()
    } else {
        required_next_tool_actions.join(" | ")
    };

    Some(format!(
        "Repair Brief:\n- Node `{}` is being retried because the previous attempt ended in `needs_repair`.\n- Previous validation reason: {}.\n- Validation basis: {}.\n- Unmet requirements: {}.\n- Blocking classification: {}.\n- Required next tool actions: {}.\n- Tools offered last attempt: {}.\n- Tools executed last attempt: {}.\n- Relevant files still unread or explicitly unreviewed: {}.\n- Previous repair attempt count: {}.\n- Remaining repair attempts after this run: {}.\n- For this retry, satisfy the unmet requirements before finalizing the artifact.\n- Do not write a blocked handoff unless the required tools were actually attempted and remained unavailable or failed.",
        node.node_id,
        reason,
        validation_basis_line,
        unmet_line,
        blocking_classification,
        next_actions_line,
        tools_offered_line,
        tools_executed_line,
        unreviewed_line,
        repair_attempt,
        repair_attempts_remaining.saturating_sub(1),
    ))
}

fn parsed_status_u32(status: Option<&Value>, key: &str) -> Option<u32> {
    status
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn infer_artifact_repair_state(
    parsed_status: Option<&Value>,
    repair_attempted: bool,
    repair_succeeded: bool,
    semantic_block_reason: Option<&str>,
    tool_telemetry: &Value,
) -> (u32, u32, bool) {
    let default_budget = tandem_core::prewrite_repair_retry_max_attempts() as u32;
    let inferred_attempt = tool_telemetry
        .get("tool_call_counts")
        .and_then(|value| value.get("write"))
        .and_then(Value::as_u64)
        .and_then(|count| count.checked_sub(1))
        .map(|count| count.min(default_budget as u64) as u32)
        .unwrap_or(0);
    let repair_attempt = parsed_status_u32(parsed_status, "repairAttempt").unwrap_or_else(|| {
        if repair_attempted {
            inferred_attempt.max(1)
        } else {
            0
        }
    });
    let repair_attempts_remaining = parsed_status_u32(parsed_status, "repairAttemptsRemaining")
        .unwrap_or_else(|| default_budget.saturating_sub(repair_attempt.min(default_budget)));
    let repair_exhausted = parsed_status
        .and_then(|value| value.get("repairExhausted"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            repair_attempted
                && !repair_succeeded
                && semantic_block_reason.is_some()
                && repair_attempt >= default_budget
        });
    (repair_attempt, repair_attempts_remaining, repair_exhausted)
}

pub(crate) fn automation_output_needs_repair(output: &Value) -> bool {
    output
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("needs_repair"))
}

pub(crate) fn automation_output_repair_exhausted(output: &Value) -> bool {
    output
        .get("artifact_validation")
        .and_then(|value| value.get("repair_exhausted"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn automation_output_blocked_reason(output: &Value) -> Option<String> {
    output
        .get("blocked_reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
