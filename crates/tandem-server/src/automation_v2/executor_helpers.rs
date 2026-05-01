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

fn output_only_failed_for_missing_materialized_artifact(value: &Value) -> bool {
    let unmet_requirements = value
        .pointer("/artifact_validation/unmet_requirements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let unmet_is_missing_output_only = unmet_requirements.is_empty()
        || unmet_requirements.iter().all(|item| {
            matches!(
                item.as_str(),
                Some("current_attempt_output_missing") | Some("structured_handoff_missing")
            )
        });
    if !unmet_is_missing_output_only {
        return false;
    }
    let blocked_reason = value
        .get("blocked_reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let rejected_reason = value
        .pointer("/artifact_validation/rejected_artifact_reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    blocked_reason.contains("explicit status or validated output")
        || blocked_reason.contains("required output `")
        || rejected_reason.contains("required output `")
}

fn run_node_is_settled_completed(
    run: &crate::automation_v2::types::AutomationV2RunRecord,
    node_id: &str,
) -> bool {
    run.checkpoint
        .completed_nodes
        .iter()
        .any(|id| id == node_id)
        || crate::app::state::automation_node_has_passing_artifact(node_id, &run.checkpoint)
}

fn automation_failure_is_provider_stream_related(detail: &str) -> bool {
    let lowered = detail.to_ascii_lowercase();
    lowered.contains("provider stream chunk error")
        || lowered.contains("stream chunk error")
        || lowered.contains("error decoding response body")
        || lowered.contains("unexpected eof")
        || lowered.contains("incomplete streamed response")
}

fn lifecycle_missing_workspace_paths(metadata: &Value) -> Vec<String> {
    metadata
        .get("must_write_file_statuses")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|item| {
                    item.get("materialized_by_current_attempt")
                        .and_then(Value::as_bool)
                        != Some(true)
                })
                .filter_map(|item| item.get("path").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn recent_node_attempt_evidence(
    run: &crate::automation_v2::types::AutomationV2RunRecord,
    node_id: Option<&str>,
) -> Vec<Value> {
    let Some(node_id) = node_id else {
        return Vec::new();
    };
    let mut evidence = Vec::new();
    for record in run.checkpoint.lifecycle_history.iter().rev() {
        let Some(metadata) = record.metadata.as_ref() else {
            continue;
        };
        if metadata.get("node_id").and_then(Value::as_str) != Some(node_id) {
            continue;
        }
        let unmet_requirements = metadata
            .get("unmet_requirements")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let missing_workspace_files = lifecycle_missing_workspace_paths(metadata);
        let required_next_tool_actions = metadata
            .get("required_next_tool_actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let rejected_artifact_reason = metadata
            .get("rejected_artifact_reason")
            .and_then(Value::as_str);
        let useful = !unmet_requirements.is_empty()
            || !missing_workspace_files.is_empty()
            || !required_next_tool_actions.is_empty()
            || rejected_artifact_reason.is_some();
        if !useful {
            continue;
        }
        evidence.push(json!({
            "event": record.event,
            "recorded_at_ms": record.recorded_at_ms,
            "reason": record.reason,
            "attempt": metadata.get("attempt").cloned().unwrap_or(Value::Null),
            "unmet_requirements": unmet_requirements,
            "missing_workspace_files": missing_workspace_files,
            "required_next_tool_actions": required_next_tool_actions,
            "rejected_artifact_reason": rejected_artifact_reason,
            "summary": metadata.get("summary").cloned().unwrap_or(Value::Null),
        }));
        if evidence.len() >= 5 {
            break;
        }
    }
    evidence.reverse();
    evidence
}

fn validation_errors_with_prior_evidence(current: Value, evidence: &[Value]) -> Value {
    let mut rows = current.as_array().cloned().unwrap_or_default();
    for item in evidence {
        if let Some(unmet) = item.get("unmet_requirements").and_then(Value::as_array) {
            rows.extend(unmet.iter().cloned());
        }
        if let Some(paths) = item
            .get("missing_workspace_files")
            .and_then(Value::as_array)
        {
            for path in paths.iter().filter_map(Value::as_str) {
                rows.push(json!(format!(
                    "required workspace file `{}` was not written in a prior attempt",
                    path
                )));
            }
        }
    }
    rows.sort_by(|left, right| left.to_string().cmp(&right.to_string()));
    rows.dedup();
    Value::Array(rows)
}
