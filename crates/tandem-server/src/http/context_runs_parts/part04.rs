async fn sync_bug_monitor_automation_node_artifact(
    state: &AppState,
    context_run_id: &str,
    automation: &crate::AutomationV2Spec,
    run: &crate::AutomationV2RunRecord,
    node: &crate::AutomationFlowNode,
    output: Option<&Value>,
) -> Result<(), StatusCode> {
    let is_bug_monitor_triage = automation
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("source"))
        .and_then(Value::as_str)
        == Some("bug_monitor");
    if !is_bug_monitor_triage {
        return Ok(());
    }
    let Some(output) = output else {
        return Ok(());
    };
    let status = output
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !status.eq_ignore_ascii_case("completed") {
        return Ok(());
    }
    let Some(artifact_type) = node
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("bug_monitor"))
        .and_then(|metadata| metadata.get("artifact_type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let artifact_id = format!("bug-monitor-v2-{}-{}", run.run_id, node.node_id);
    let blackboard = load_context_blackboard(state, context_run_id);
    if blackboard.artifacts.iter().any(|row| row.id == artifact_id) {
        return Ok(());
    }
    let payload = output
        .get("content")
        .cloned()
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| output.clone());
    let relative_path = match artifact_type {
        "bug_monitor_inspection" => "artifacts/bug_monitor.inspection.json",
        "bug_monitor_research" => "artifacts/bug_monitor.research.json",
        "bug_monitor_validation" => "artifacts/bug_monitor.validation.json",
        "bug_monitor_fix_proposal" => "artifacts/bug_monitor.fix_proposal.json",
        _ => return Ok(()),
    };
    append_json_artifact_to_context_run(
        state,
        context_run_id,
        &artifact_id,
        artifact_type,
        relative_path,
        &payload,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if artifact_type == "bug_monitor_fix_proposal" {
        if let Some(draft_id) = automation
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("draft_id"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            if let Err(error) =
                crate::http::bug_monitor::finalize_completed_bug_monitor_triage(state, draft_id)
                    .await
            {
                tracing::warn!(
                    draft_id = %draft_id,
                    run_id = %run.run_id,
                    error = %error,
                    "failed to finalize completed Bug Monitor triage after artifact sync",
                );
            }
        }
    }
    Ok(())
}
