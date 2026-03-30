use super::*;
use crate::automation_v2::types::{AutomationFlowInputRef, AutomationFlowNode};
use serde_json::json;

// ---------------------------------------------------------------------------
// Phase-0 smoke tests — regression safety net for module extraction.
// Covers the 4 highest-traffic pure functions identified in
// AUTOMATION_MODULARIZATION_PLAN.md §Pre-Extraction Test Safety Net.
//
// These tests verify observable behaviour before any code moves happen, so
// that a broken import or wrong re-export is caught immediately by `cargo test`.
// ---------------------------------------------------------------------------

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn bare_node() -> AutomationFlowNode {
    AutomationFlowNode {
        node_id: "n1".to_string(),
        agent_id: "a1".to_string(),
        objective: "do something".to_string(),
        depends_on: vec![],
        input_refs: vec![],
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    }
}

fn node_with_input_ref() -> AutomationFlowNode {
    let mut node = bare_node();
    node.input_refs = vec![AutomationFlowInputRef {
        from_step_id: "prev".to_string(),
        alias: "research".to_string(),
    }];
    node
}

fn code_workflow_node() -> AutomationFlowNode {
    // automation_node_is_code_workflow checks metadata.builder.task_kind first.
    let mut node = bare_node();
    node.metadata = Some(json!({
        "builder": { "task_kind": "code_change" }
    }));
    node
}

fn code_patch_contract_node() -> AutomationFlowNode {
    let mut node = bare_node();
    node.node_id = "code_patch".to_string();
    node.objective = "Patch the code and verify the change.".to_string();
    node.output_contract = Some(AutomationFlowOutputContract {
        kind: "code_patch".to_string(),
        validator: None,
        enforcement: None,
        schema: None,
        summary_guidance: None,
    });
    node.metadata = Some(json!({
        "builder": {
            "output_path": "src/lib.rs",
            "verification_command": "cargo test"
        }
    }));
    node
}

fn email_delivery_node() -> AutomationFlowNode {
    let mut node = bare_node();
    node.objective = "Send the finalized report to the requested email address.".to_string();
    node.metadata = Some(json!({
        "delivery": {
            "method": "email",
            "to": "evan@frumu.ai",
            "content_type": "text/html",
            "inline_body_only": true,
            "attachments": false
        }
    }));
    node
}

fn report_markdown_node() -> AutomationFlowNode {
    let mut node = node_with_input_ref();
    node.output_contract = Some(AutomationFlowOutputContract {
        kind: "report_markdown".to_string(),
        validator: Some(crate::AutomationOutputValidatorKind::GenericArtifact),
        enforcement: None,
        schema: None,
        summary_guidance: None,
    });
    node
}

fn local_citations_contract_node() -> AutomationFlowNode {
    let mut node = bare_node();
    node.node_id = "research_sources".to_string();
    node.objective =
        "Inspect the local workspace and cite the most relevant project-authored sources."
            .to_string();
    node.output_contract = Some(AutomationFlowOutputContract {
        kind: "citations".to_string(),
        validator: None,
        enforcement: None,
        schema: None,
        summary_guidance: None,
    });
    node.metadata = Some(json!({
        "builder": {
            "web_research_expected": false
        }
    }));
    node
}

#[test]
fn automation_quality_mode_defaults_to_strict_and_requires_rollback_for_legacy_metadata() {
    let strict_mode =
        super::enforcement::automation_quality_mode_resolution_from_metadata(None, true, false);
    assert_eq!(
        strict_mode.effective,
        super::enforcement::AutomationQualityMode::StrictResearchV1
    );
    assert_eq!(strict_mode.requested, None);
    assert!(!strict_mode.legacy_rollback_enabled);

    let legacy_metadata = serde_json::json!({
        "quality_mode": "legacy"
    });
    let legacy_object = legacy_metadata.as_object().cloned().expect("object");
    let forced_strict = super::enforcement::automation_quality_mode_resolution_from_metadata(
        Some(&legacy_object),
        true,
        false,
    );
    assert_eq!(
        forced_strict.requested,
        Some(super::enforcement::AutomationQualityMode::Legacy)
    );
    assert_eq!(
        forced_strict.effective,
        super::enforcement::AutomationQualityMode::StrictResearchV1
    );

    let legacy_mode = super::enforcement::automation_quality_mode_resolution_from_metadata(
        Some(&legacy_object),
        true,
        true,
    );
    assert_eq!(
        legacy_mode.requested,
        Some(super::enforcement::AutomationQualityMode::Legacy)
    );
    assert_eq!(
        legacy_mode.effective,
        super::enforcement::AutomationQualityMode::Legacy
    );
}

// -----------------------------------------------------------------------
// automation_infer_selected_mcp_servers
// -----------------------------------------------------------------------

#[test]
fn mcp_servers_empty_inputs_returns_empty() {
    let result = automation_infer_selected_mcp_servers(&[], &[], &[], false);
    assert!(result.is_empty());
}

#[test]
fn mcp_servers_explicit_allowed_list_returned_directly() {
    let result = automation_infer_selected_mcp_servers(
        &["gmail".to_string()],
        &[],
        &["gmail".to_string(), "slack".to_string()],
        false,
    );
    assert_eq!(result, vec!["gmail"]);
}

#[test]
fn mcp_servers_allowlist_wildcard_returns_all_enabled() {
    let enabled = vec!["gmail".to_string(), "slack".to_string()];
    let result = automation_infer_selected_mcp_servers(&[], &["*".to_string()], &enabled, false);
    assert_eq!(result, enabled);
}

#[test]
fn mcp_servers_requires_email_delivery_returns_all_enabled() {
    let enabled = vec!["gmail".to_string(), "hubspot".to_string()];
    let result = automation_infer_selected_mcp_servers(&[], &[], &enabled, true);
    assert_eq!(result, enabled);
}

#[test]
fn report_markdown_preserves_full_upstream_inputs() {
    let node = report_markdown_node();
    assert!(automation_node_preserves_full_upstream_inputs(&node));

    let mut text_summary = bare_node();
    text_summary.output_contract = Some(AutomationFlowOutputContract {
        kind: "text_summary".to_string(),
        validator: Some(crate::AutomationOutputValidatorKind::GenericArtifact),
        enforcement: None,
        schema: None,
        summary_guidance: None,
    });
    text_summary.input_refs = vec![AutomationFlowInputRef {
        from_step_id: "prev".to_string(),
        alias: "input".to_string(),
    }];
    assert!(automation_node_preserves_full_upstream_inputs(
        &text_summary
    ));
}

#[test]
fn mcp_servers_allowlist_namespace_pattern_matches_server() {
    // "mcp.my_server.*" should match server named "my-server" (dashes → underscores)
    let enabled = vec!["my-server".to_string(), "other".to_string()];
    let result = automation_infer_selected_mcp_servers(
        &[],
        &["mcp.my_server.*".to_string()],
        &enabled,
        false,
    );
    assert_eq!(result, vec!["my-server"]);
}

#[test]
fn mcp_servers_deduplicates_when_allowed_and_allowlist_overlap() {
    let enabled = vec!["gmail".to_string()];
    let result = automation_infer_selected_mcp_servers(
        &["gmail".to_string()],
        &["mcp.gmail.*".to_string()],
        &enabled,
        false,
    );
    assert_eq!(result, vec!["gmail"]);
}

#[test]
fn email_send_detection_recognizes_compact_sendemail_names() {
    assert!(automation_tool_name_is_email_send(
        "mcp.composio_1.gmail_sendemail"
    ));
    assert!(automation_tool_name_is_email_send("Gmail_SendEmail"));
    assert!(automation_tool_name_is_email_draft(
        "mcp.composio_1.gmail_draftemail"
    ));
}

#[test]
fn step_cost_provenance_marks_budget_limit_and_cost_deltas() {
    let provenance = automation_step_cost_provenance(
        "step_1",
        Some("gpt-5.1".to_string()),
        120,
        80,
        2.75,
        9.50,
        true,
    );

    assert_eq!(
        provenance.get("step_id").and_then(Value::as_str),
        Some("step_1")
    );
    assert_eq!(
        provenance.get("model_id").and_then(Value::as_str),
        Some("gpt-5.1")
    );
    assert_eq!(
        provenance.get("tokens_in").and_then(Value::as_u64),
        Some(120)
    );
    assert_eq!(
        provenance.get("tokens_out").and_then(Value::as_u64),
        Some(80)
    );
    assert_eq!(
        provenance.get("computed_cost_usd").and_then(Value::as_f64),
        Some(2.75)
    );
    assert_eq!(
        provenance
            .get("cumulative_run_cost_usd_at_step_end")
            .and_then(Value::as_f64),
        Some(9.50)
    );
    assert_eq!(
        provenance
            .get("budget_limit_reached")
            .and_then(Value::as_bool),
        Some(true)
    );
}

// -----------------------------------------------------------------------
// automation_tool_capability_ids
// -----------------------------------------------------------------------

#[test]
fn capability_ids_bare_node_empty() {
    let node = bare_node();
    let caps = automation_tool_capability_ids(&node, "research");
    assert!(
        caps.is_empty(),
        "bare node should yield no capabilities, got: {caps:?}"
    );
}

#[test]
fn capability_ids_node_with_input_ref_includes_workspace_read() {
    let node = node_with_input_ref();
    let caps = automation_tool_capability_ids(&node, "research");
    assert!(caps.contains(&"workspace_read".to_string()));
}

#[test]
fn capability_ids_code_workflow_git_patch_includes_verify_command() {
    let caps = automation_tool_capability_ids(&code_workflow_node(), "git_patch");
    assert!(
        caps.contains(&"verify_command".to_string()),
        "git_patch code node should require verify_command, got: {caps:?}"
    );
}

#[test]
fn capability_ids_code_workflow_research_mode_excludes_verify_command() {
    let caps = automation_tool_capability_ids(&code_workflow_node(), "research");
    assert!(
        !caps.contains(&"verify_command".to_string()),
        "research mode should not include verify_command, got: {caps:?}"
    );
}

#[test]
fn code_patch_contract_is_treated_as_a_code_workflow() {
    let node = code_patch_contract_node();
    assert_eq!(
        automation_output_validator_kind(&node),
        crate::AutomationOutputValidatorKind::CodePatch
    );
    assert!(automation_node_is_code_workflow(&node));
    assert_eq!(
        automation_node_execution_policy(&node, ".")
            .get("workflow_class")
            .and_then(Value::as_str),
        Some("code")
    );
}

#[test]
fn code_patch_contract_includes_verification_command_capability() {
    let caps = automation_tool_capability_ids(&code_patch_contract_node(), "git_patch");
    assert!(
        caps.contains(&"verify_command".to_string()),
        "code_patch contract should require verify_command in patch mode, got: {caps:?}"
    );
}

#[test]
fn code_patch_contract_enforcement_defaults_require_reads_and_prewrite_gates() {
    let enforcement = automation_node_output_enforcement(&code_patch_contract_node());
    assert_eq!(
        enforcement.validation_profile.as_deref(),
        Some("code_change")
    );
    assert!(enforcement.required_tools.iter().any(|tool| tool == "read"));
    assert!(enforcement
        .required_evidence
        .iter()
        .any(|value| value == "local_source_reads"));
    assert!(enforcement
        .prewrite_gates
        .iter()
        .any(|gate| gate == "workspace_inspection"));
    assert!(enforcement
        .prewrite_gates
        .iter()
        .any(|gate| gate == "concrete_reads"));
}

#[test]
fn code_patch_contract_requires_verification_before_completion() {
    let node = code_patch_contract_node();
    let tool_telemetry = json!({
        "verification_expected": true,
        "verification_ran": false
    });
    assert_eq!(
        detect_automation_node_failure_kind(&node, "blocked", None, None, None).as_deref(),
        None
    );
    assert_eq!(
        detect_automation_node_failure_kind(
            &node,
            "blocked",
            Some(false),
            None,
            Some(&json!({"verification_expected": true, "verification_ran": false}))
        )
        .as_deref(),
        Some("verification_missing")
    );
    assert_eq!(
        detect_automation_blocker_category(&node, "blocked", None, &tool_telemetry, None,),
        Some("verification_required".to_string())
    );
}

#[test]
fn local_citations_contract_defaults_to_local_research_not_external_research() {
    let enforcement = automation_node_output_enforcement(&local_citations_contract_node());
    assert_eq!(
        enforcement.validation_profile.as_deref(),
        Some("local_research")
    );
    assert!(enforcement.required_tools.iter().any(|tool| tool == "glob"));
    assert!(enforcement.required_tools.iter().any(|tool| tool == "read"));
    assert!(enforcement
        .required_evidence
        .iter()
        .any(|value| value == "local_source_reads"));
    assert!(enforcement
        .prewrite_gates
        .iter()
        .any(|gate| gate == "workspace_inspection"));
}

#[test]
fn auto_cleaned_marker_file_rejection_is_downgraded_when_output_is_valid() {
    assert!(super::should_downgrade_auto_cleaned_marker_rejection(
        Some("undeclared marker files created: .tandem_ack"),
        true,
        None,
        true
    ));
    assert!(!super::should_downgrade_auto_cleaned_marker_rejection(
        Some("undeclared marker files created: .tandem_ack"),
        false,
        None,
        true
    ));
    assert!(!super::should_downgrade_auto_cleaned_marker_rejection(
        Some("undeclared marker files created: .tandem_ack"),
        true,
        Some("no_concrete_reads"),
        true
    ));
    assert!(!super::should_downgrade_auto_cleaned_marker_rejection(
        Some("other rejection"),
        true,
        None,
        true
    ));
}

#[test]
fn capability_ids_output_is_sorted_and_deduplicated() {
    let node = node_with_input_ref();
    let caps = automation_tool_capability_ids(&node, "research");
    let mut sorted = caps.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        caps, sorted,
        "capability ids must be sorted and deduplicated"
    );
}

#[test]
fn capability_resolution_expands_wildcard_offered_email_tools() {
    let node = email_delivery_node();
    let available_tool_names = [
        "read".to_string(),
        "glob".to_string(),
        "mcp.composio_1.gmail_send_email".to_string(),
        "mcp.composio_1.gmail_create_email_draft".to_string(),
    ]
    .into_iter()
    .collect::<std::collections::HashSet<_>>();
    let resolution = automation_resolve_capabilities(
        &node,
        "artifact_write",
        &["mcp.composio_1.*".to_string()],
        &available_tool_names,
    );

    let offered_send_tools = resolution
        .get("email_tool_diagnostics")
        .and_then(|value| value.get("offered_send_tools"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let offered_draft_tools = resolution
        .get("email_tool_diagnostics")
        .and_then(|value| value.get("offered_draft_tools"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    assert!(offered_send_tools
        .iter()
        .any(|value| { value.as_str() == Some("mcp.composio_1.gmail_send_email") }));
    assert!(offered_draft_tools
        .iter()
        .any(|value| { value.as_str() == Some("mcp.composio_1.gmail_create_email_draft") }));
}

// -----------------------------------------------------------------------
// normalize_upstream_research_output_paths
// -----------------------------------------------------------------------

#[test]
fn normalize_upstream_paths_passthrough_when_no_content_key() {
    let output = json!({ "summary": "hello" });
    let result = normalize_upstream_research_output_paths("/workspace", None, &output);
    assert_eq!(
        result, output,
        "output with no 'content' key should be returned unchanged"
    );
}

#[test]
fn normalize_upstream_paths_survives_empty_handoff() {
    let output = json!({
        "content": {
            "text": "some text",
            "structured_handoff": {}
        }
    });
    let result = normalize_upstream_research_output_paths("/workspace", None, &output);
    assert!(result.is_object(), "result should still be a JSON object");
}

#[test]
fn normalize_upstream_paths_scopes_tandem_artifacts_for_run() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-upstream-run-scoped-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join(".tandem/runs/run-123/artifacts"))
        .expect("create artifacts");
    std::fs::write(
        workspace_root.join(".tandem/runs/run-123/artifacts/report.md"),
        "report",
    )
    .expect("write artifact");
    let output = json!({
        "content": {
            "structured_handoff": {
                "files_reviewed": [".tandem/artifacts/report.md"]
            }
        }
    });
    let result = normalize_upstream_research_output_paths(
        workspace_root.to_str().expect("workspace"),
        Some("run-123"),
        &output,
    );
    assert_eq!(
        result.pointer("/content/structured_handoff/files_reviewed/0"),
        Some(&json!(".tandem/runs/run-123/artifacts/report.md"))
    );
    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn required_output_path_scopes_shared_artifacts_for_run() {
    let mut node = bare_node();
    node.node_id = "generate_report".to_string();
    node.output_contract = Some(AutomationFlowOutputContract {
        kind: "report_markdown".to_string(),
        validator: Some(crate::AutomationOutputValidatorKind::GenericArtifact),
        enforcement: None,
        schema: None,
        summary_guidance: None,
    });
    node.metadata = Some(json!({
        "builder": {
            "output_path": ".tandem/artifacts/generate-report.md"
        }
    }));

    assert_eq!(
        automation_node_required_output_path_for_run(&node, Some("run-iso")),
        Some(".tandem/runs/run-iso/artifacts/generate-report.md".to_string())
    );
    assert_eq!(
        automation_node_required_output_path_for_run(&node, None),
        Some(".tandem/artifacts/generate-report.md".to_string())
    );
}

#[test]
fn session_write_materialized_output_detects_run_scoped_artifact_files() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-current-attempt-output-{}",
        uuid::Uuid::new_v4()
    ));
    let run_id = "run-123";
    let artifact_path = workspace_root.join(".tandem/runs/run-123/artifacts/report.md");
    std::fs::create_dir_all(
        artifact_path
            .parent()
            .expect("artifact path should have parent"),
    )
    .expect("create artifacts dir");
    std::fs::write(&artifact_path, "report body").expect("write artifact");

    let mut session = Session::new(Some("write evidence".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        tandem_types::MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": ".tandem/artifacts/report.md",
                "content": "report body"
            }),
            result: Some(json!({"output": "written"})),
            error: None,
        }],
    ));

    assert!(session_write_materialized_output_for_output(
        &session,
        workspace_root.to_str().expect("workspace root"),
        ".tandem/artifacts/report.md",
        Some(run_id),
    ));

    std::fs::remove_file(&artifact_path).expect("remove artifact");

    assert!(!session_write_materialized_output_for_output(
        &session,
        workspace_root.to_str().expect("workspace root"),
        ".tandem/artifacts/report.md",
        Some(run_id),
    ));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn reconcile_verified_output_path_waits_for_late_file_visibility() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-reconcile-verified-output-{}",
        uuid::Uuid::new_v4()
    ));
    let run_id = "run-reconcile";
    let output_path = ".tandem/artifacts/report.md";
    let resolved_path = workspace_root.join(".tandem/runs/run-reconcile/artifacts/report.md");
    std::fs::create_dir_all(
        resolved_path
            .parent()
            .expect("artifact path should have parent"),
    )
    .expect("create artifacts dir");

    let mut session = Session::new(Some("reconcile visibility".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        tandem_types::MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": output_path,
                "content": "report body"
            }),
            result: Some(json!({"output": "written"})),
            error: None,
        }],
    ));

    let writer_root = workspace_root.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(60));
        std::fs::write(
            writer_root.join(".tandem/runs/run-reconcile/artifacts/report.md"),
            "report body",
        )
        .expect("write delayed artifact");
    });

    let resolved = super::reconcile_automation_resolve_verified_output_path(
        &session,
        workspace_root.to_str().expect("workspace root"),
        run_id,
        &AutomationFlowNode {
            node_id: "generate_report".to_string(),
            agent_id: "writer".to_string(),
            objective: "Generate report".to_string(),
            depends_on: vec![],
            input_refs: vec![],
            output_contract: Some(AutomationFlowOutputContract {
                kind: "report_markdown".to_string(),
                validator: Some(crate::AutomationOutputValidatorKind::GenericArtifact),
                enforcement: None,
                schema: None,
                summary_guidance: None,
            }),
            retry_policy: None,
            timeout_ms: None,
            stage_kind: None,
            gate: None,
            metadata: Some(json!({
                "builder": {
                    "output_path": output_path
                }
            })),
        },
        output_path,
        300,
        25,
    )
    .await
    .expect("resolve after delay");

    writer.join().expect("writer thread");
    assert_eq!(resolved, Some(resolved_path));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn reconcile_verified_output_path_times_out_when_file_never_appears() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-reconcile-verified-output-timeout-{}",
        uuid::Uuid::new_v4()
    ));
    let run_id = "run-timeout";
    let output_path = ".tandem/artifacts/report.md";
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let session = Session::new(Some("reconcile timeout".to_string()), None);
    let resolved = super::reconcile_automation_resolve_verified_output_path(
        &session,
        workspace_root.to_str().expect("workspace root"),
        run_id,
        &AutomationFlowNode {
            node_id: "generate_report".to_string(),
            agent_id: "writer".to_string(),
            objective: "Generate report".to_string(),
            depends_on: vec![],
            input_refs: vec![],
            output_contract: Some(AutomationFlowOutputContract {
                kind: "report_markdown".to_string(),
                validator: Some(crate::AutomationOutputValidatorKind::GenericArtifact),
                enforcement: None,
                schema: None,
                summary_guidance: None,
            }),
            retry_policy: None,
            timeout_ms: None,
            stage_kind: None,
            gate: None,
            metadata: Some(json!({
                "builder": {
                    "output_path": output_path
                }
            })),
        },
        output_path,
        50,
        10,
    )
    .await
    .expect("resolve timeout");

    assert!(resolved.is_none());

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn reconcile_verified_output_path_recovers_json_artifact_from_session_text() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-reconcile-session-text-json-{}",
        uuid::Uuid::new_v4()
    ));
    let run_id = "run-session-json";
    let output_path = ".tandem/artifacts/research-sources.json";
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let mut session = Session::new(Some("session text recovery".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        tandem_types::MessageRole::Assistant,
        vec![tandem_types::MessagePart::Text {
            text: "{\n  \"sources\": [\n    {\n      \"path\": \"README.md\",\n      \"reason\": \"project overview\"\n    }\n  ],\n  \"summary\": \"Primary local sources identified.\"\n}\n{\"status\":\"completed\"}".to_string(),
        }],
    ));

    let resolved = super::reconcile_automation_resolve_verified_output_path(
        &session,
        workspace_root.to_str().expect("workspace root"),
        run_id,
        &AutomationFlowNode {
            node_id: "research_sources".to_string(),
            agent_id: "researcher".to_string(),
            objective: "Find and record local sources".to_string(),
            depends_on: vec![],
            input_refs: vec![],
            output_contract: Some(AutomationFlowOutputContract {
                kind: "citations".to_string(),
                validator: Some(crate::AutomationOutputValidatorKind::GenericArtifact),
                enforcement: None,
                schema: None,
                summary_guidance: None,
            }),
            retry_policy: None,
            timeout_ms: None,
            stage_kind: None,
            gate: None,
            metadata: Some(json!({
                "builder": {
                    "output_path": output_path
                }
            })),
        },
        output_path,
        50,
        10,
    )
    .await
    .expect("recover from session text");

    let expected =
        workspace_root.join(".tandem/runs/run-session-json/artifacts/research-sources.json");
    assert_eq!(resolved, Some(expected.clone()));
    let written = std::fs::read_to_string(expected).expect("read recovered artifact");
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("parse recovered json");
    assert_eq!(parsed["sources"][0]["path"], "README.md");
    assert_eq!(parsed["summary"], "Primary local sources identified.");

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn session_write_candidates_accepts_file_path_schema_with_normalized_run_scoped_paths() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-write-candidate-file-path-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let run_id = "run-123";
    let artifact_path_with_dot_segments = workspace_root
        .join(".tandem/runs/run-123/artifacts")
        .join("./report.md");

    let mut session = Session::new(Some("file path candidate".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        tandem_types::MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "filePath": artifact_path_with_dot_segments.to_string_lossy(),
                "body": "report body"
            }),
            result: Some(json!({"output":"written"})),
            error: None,
        }],
    ));

    let candidates = session_write_candidates_for_output(
        &session,
        workspace_root.to_str().expect("workspace root"),
        ".tandem/artifacts/report.md",
        Some(run_id),
    );

    assert_eq!(candidates, vec!["report body".to_string()]);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn session_write_materialized_output_accepts_absolute_legacy_artifact_paths() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-current-attempt-output-abs-{}",
        uuid::Uuid::new_v4()
    ));
    let run_id = "run-abs";
    let legacy_abs_path = workspace_root
        .join(".tandem/artifacts/report.md")
        .to_string_lossy()
        .to_string();
    let run_scoped_path = workspace_root.join(".tandem/runs/run-abs/artifacts/report.md");
    std::fs::create_dir_all(
        run_scoped_path
            .parent()
            .expect("artifact path should have parent"),
    )
    .expect("create run artifacts dir");
    std::fs::write(&run_scoped_path, "report body").expect("write run-scoped artifact");

    let mut session = Session::new(Some("absolute write evidence".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        tandem_types::MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": legacy_abs_path,
                "content": "report body"
            }),
            result: Some(json!({"output":"ok"})),
            error: None,
        }],
    ));

    assert!(session_write_materialized_output_for_output(
        &session,
        workspace_root.to_str().expect("workspace root"),
        ".tandem/artifacts/report.md",
        Some(run_id),
    ));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn session_write_materialized_output_accepts_file_path_schema_with_normalized_run_scoped_paths() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-current-attempt-output-file-path-{}",
        uuid::Uuid::new_v4()
    ));
    let run_id = "run-file-path";
    let artifact_path = workspace_root.join(".tandem/runs/run-file-path/artifacts/report.md");
    let artifact_path_with_dot_segments = workspace_root
        .join(".tandem/runs/run-file-path/artifacts")
        .join("./report.md");
    std::fs::create_dir_all(
        artifact_path
            .parent()
            .expect("artifact path should have parent"),
    )
    .expect("create artifacts dir");
    std::fs::write(&artifact_path, "report body").expect("write artifact");

    let mut session = Session::new(Some("file path write evidence".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        tandem_types::MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "filePath": artifact_path_with_dot_segments.to_string_lossy(),
                "content": "report body"
            }),
            result: Some(json!({"output":"written"})),
            error: None,
        }],
    ));

    assert!(session_write_materialized_output_for_output(
        &session,
        workspace_root.to_str().expect("workspace root"),
        ".tandem/artifacts/report.md",
        Some(run_id),
    ));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn session_write_candidates_supports_variant_path_and_content_keys() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-write-candidate-variants-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let mut session = Session::new(Some("candidate variants".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        tandem_types::MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "output_path": ".tandem/artifacts/report.md",
                "contents": "variant payload"
            }),
            result: Some(json!({"output":"ok"})),
            error: None,
        }],
    ));

    let candidates = session_write_candidates_for_output(
        &session,
        workspace_root.to_str().expect("workspace root"),
        ".tandem/artifacts/report.md",
        Some("run-variants"),
    );
    assert_eq!(candidates, vec!["variant payload".to_string()]);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn resolve_automation_output_path_rejects_parent_escape_segments() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-output-path-escape-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let resolved = resolve_automation_output_path(
        workspace_root.to_str().expect("workspace root"),
        "../outside.md",
    );
    assert!(
        resolved.is_err(),
        "expected parent escape path to be rejected, got {resolved:?}"
    );

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn resolve_automation_output_path_normalizes_dot_segments_inside_workspace() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-output-path-normalize-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("nested")).expect("create workspace");

    let resolved = resolve_automation_output_path(
        workspace_root.to_str().expect("workspace root"),
        "nested/../report.md",
    )
    .expect("resolve normalized path");

    assert_eq!(resolved, workspace_root.join("report.md"));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

// -----------------------------------------------------------------------
// assess_artifact_candidate — score ordering invariants
// -----------------------------------------------------------------------

#[test]
fn assess_empty_text_has_negative_score() {
    let assessment =
        assess_artifact_candidate(&bare_node(), "/workspace", "tool", "", &[], &[], &[], &[]);
    assert!(
        assessment.score < 0,
        "empty text should produce a negative score, got {}",
        assessment.score
    );
}

#[test]
fn assess_substantive_text_scores_higher_than_empty() {
    let rich = "## Summary\n\nDetailed analysis.\n\n## Files reviewed\n\n- /workspace/foo.rs\n\n## Approved\n\nYes.";
    let rich_score =
        assess_artifact_candidate(&bare_node(), "/workspace", "tool", rich, &[], &[], &[], &[])
            .score;
    let empty_score =
        assess_artifact_candidate(&bare_node(), "/workspace", "tool", "", &[], &[], &[], &[]).score;
    assert!(
        rich_score > empty_score,
        "substantive text ({rich_score}) should score higher than empty ({empty_score})"
    );
}

#[test]
fn assess_source_field_preserved() {
    let assessment = assess_artifact_candidate(
        &bare_node(),
        "/workspace",
        "my_source",
        "hello",
        &[],
        &[],
        &[],
        &[],
    );
    assert_eq!(assessment.source, "my_source");
}

#[test]
fn assess_evidence_anchors_count_upstream_path_and_url_mentions() {
    let assessment = assess_artifact_candidate(
        &bare_node(),
        "/workspace",
        "tool",
        "See /workspace/docs/product-capabilities.md and https://example.com/source-1 for details.",
        &[],
        &[],
        &[
            "/workspace/docs/product-capabilities.md".to_string(),
            "/workspace/README.md".to_string(),
        ],
        &["https://example.com/source-1".to_string()],
    );
    assert!(
        assessment.evidence_anchor_count >= 2,
        "expected to match at least two upstream evidence anchors, got {}",
        assessment.evidence_anchor_count
    );
}
