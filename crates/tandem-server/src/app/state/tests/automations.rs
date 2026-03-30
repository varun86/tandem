use super::*;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use tandem_types::{MessageRole, PrewriteCoverageMode, Session};

use crate::app::state::automation::collect_automation_attempt_receipt_events;
use crate::app::state::automation::node_output::{
    build_automation_attempt_evidence, build_automation_validator_summary,
    detect_automation_blocker_category, detect_automation_node_failure_kind,
    detect_automation_node_phase, detect_automation_node_status, wrap_automation_node_output,
};
use crate::app::state::automation::receipts::{
    append_automation_attempt_receipt, automation_attempt_receipt_path_for_state_dir,
    AutomationAttemptReceiptDraft, AutomationAttemptReceiptRecord,
};
use crate::capability_resolver;

fn with_legacy_quality_rollback_enabled<T>(enabled: bool, f: impl FnOnce() -> T) -> T {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock");
    let key = "TANDEM_AUTOMATION_QUALITY_LEGACY_ROLLBACK";
    let previous = std::env::var(key).ok();
    std::env::set_var(key, if enabled { "true" } else { "false" });
    let result = f();
    if let Some(previous) = previous {
        std::env::set_var(key, previous);
    } else {
        std::env::remove_var(key);
    }
    result
}

fn routine_dependency_plan_package() -> tandem_plan_compiler::api::PlanPackage {
    use tandem_plan_compiler::api::{
        ApprovalMode, AuditScope, CommunicationModel, CrossRoutineVisibility, DataScope,
        DependencyMode, DependencyResolution, DependencyResolutionStrategy,
        FinalArtifactVisibility, InterRoutinePolicy, IntermediateArtifactVisibility,
        MidRoutineConnectorFailureMode, MissionContextScope, MissionDefinition, PartialFailureMode,
        PeerVisibility, PlanLifecycleState, PlanOwner, PlanPackage, ReentryPoint,
        RoutineConnectorResolution, RoutineDependency, RoutinePackage, RoutineSemanticKind,
        RunHistoryVisibility, StepFailurePolicy, StepModelPolicy, StepPackage, StepRetryPolicy,
        SuccessCriteria, TriggerDefinition, TriggerKind,
    };

    let step_a = StepPackage {
        step_id: "step_a".to_string(),
        label: "Step A".to_string(),
        kind: "execute".to_string(),
        action: "run_a".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        dependencies: Vec::new(),
        context_reads: Vec::new(),
        context_writes: Vec::new(),
        connector_requirements: Vec::new(),
        model_policy: StepModelPolicy::default(),
        approval_policy: ApprovalMode::InternalOnly,
        success_criteria: SuccessCriteria::default(),
        failure_policy: StepFailurePolicy::default(),
        retry_policy: StepRetryPolicy::default(),
        artifacts: Vec::new(),
        provenance: None,
        notes: None,
    };
    let step_b = StepPackage {
        step_id: "step_b".to_string(),
        label: "Step B".to_string(),
        kind: "execute".to_string(),
        action: "run_b".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        dependencies: Vec::new(),
        context_reads: Vec::new(),
        context_writes: Vec::new(),
        connector_requirements: Vec::new(),
        model_policy: StepModelPolicy::default(),
        approval_policy: ApprovalMode::InternalOnly,
        success_criteria: SuccessCriteria::default(),
        failure_policy: StepFailurePolicy::default(),
        retry_policy: StepRetryPolicy::default(),
        artifacts: Vec::new(),
        provenance: None,
        notes: None,
    };

    PlanPackage {
        plan_id: "plan-dependency-test".to_string(),
        plan_revision: 1,
        lifecycle_state: PlanLifecycleState::Preview,
        owner: PlanOwner {
            owner_id: "owner".to_string(),
            scope: "workspace".to_string(),
            audience: "internal".to_string(),
        },
        mission: MissionDefinition {
            goal: "Dependency test".to_string(),
            summary: None,
            domain: None,
        },
        success_criteria: SuccessCriteria::default(),
        budget_policy: None,
        budget_enforcement: None,
        approval_policy: None,
        inter_routine_policy: Some(InterRoutinePolicy {
            communication_model: CommunicationModel::ArtifactOnly,
            shared_memory_access: false,
            shared_memory_justification: None,
            peer_visibility: PeerVisibility::None,
            artifact_handoff_validation: true,
        }),
        trigger_policy: None,
        output_roots: None,
        precedence_log: Vec::new(),
        plan_diff: None,
        manual_trigger_record: None,
        validation_state: None,
        overlap_policy: None,
        routine_graph: vec![
            RoutinePackage {
                routine_id: "routine_a".to_string(),
                semantic_kind: RoutineSemanticKind::Execution,
                trigger: TriggerDefinition {
                    trigger_type: TriggerKind::Manual,
                    schedule: None,
                    timezone: None,
                },
                dependencies: Vec::new(),
                dependency_resolution: DependencyResolution {
                    strategy: DependencyResolutionStrategy::TopologicalSequential,
                    partial_failure_mode: PartialFailureMode::PauseDownstreamOnly,
                    reentry_point: ReentryPoint::FailedStep,
                    mid_routine_connector_failure: MidRoutineConnectorFailureMode::SurfaceAndPause,
                },
                connector_resolution: RoutineConnectorResolution::default(),
                data_scope: DataScope {
                    readable_paths: Vec::new(),
                    writable_paths: Vec::new(),
                    denied_paths: Vec::new(),
                    cross_routine_visibility: CrossRoutineVisibility::None,
                    mission_context_scope: MissionContextScope::GoalOnly,
                    mission_context_justification: None,
                },
                audit_scope: AuditScope {
                    run_history_visibility: RunHistoryVisibility::PlanOwner,
                    named_audit_roles: Vec::new(),
                    intermediate_artifact_visibility: IntermediateArtifactVisibility::RoutineOnly,
                    final_artifact_visibility: FinalArtifactVisibility::PlanOwner,
                },
                success_criteria: SuccessCriteria::default(),
                steps: vec![step_a],
            },
            RoutinePackage {
                routine_id: "routine_b".to_string(),
                semantic_kind: RoutineSemanticKind::Execution,
                trigger: TriggerDefinition {
                    trigger_type: TriggerKind::Manual,
                    schedule: None,
                    timezone: None,
                },
                dependencies: vec![RoutineDependency {
                    dependency_type: "routine".to_string(),
                    routine_id: "routine_a".to_string(),
                    mode: DependencyMode::Hard,
                }],
                dependency_resolution: DependencyResolution {
                    strategy: DependencyResolutionStrategy::TopologicalSequential,
                    partial_failure_mode: PartialFailureMode::PauseDownstreamOnly,
                    reentry_point: ReentryPoint::FailedStep,
                    mid_routine_connector_failure: MidRoutineConnectorFailureMode::SurfaceAndPause,
                },
                connector_resolution: RoutineConnectorResolution::default(),
                data_scope: DataScope {
                    readable_paths: Vec::new(),
                    writable_paths: Vec::new(),
                    denied_paths: Vec::new(),
                    cross_routine_visibility: CrossRoutineVisibility::None,
                    mission_context_scope: MissionContextScope::GoalOnly,
                    mission_context_justification: None,
                },
                audit_scope: AuditScope {
                    run_history_visibility: RunHistoryVisibility::PlanOwner,
                    named_audit_roles: Vec::new(),
                    intermediate_artifact_visibility: IntermediateArtifactVisibility::RoutineOnly,
                    final_artifact_visibility: FinalArtifactVisibility::PlanOwner,
                },
                success_criteria: SuccessCriteria::default(),
                steps: vec![step_b],
            },
        ],
        connector_intents: Vec::new(),
        connector_bindings: Vec::new(),
        connector_binding_resolution: None,
        model_routing_resolution: None,
        credential_envelopes: Vec::new(),
        context_objects: Vec::new(),
        metadata: None,
    }
}

#[tokio::test]
async fn automation_attempt_receipt_append_uses_jsonl_path_and_skips_malformed_lines() {
    let state_dir =
        std::env::temp_dir().join(format!("tandem-receipt-ledger-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&state_dir).expect("state dir");

    let run_id = "run-ledger-test";
    let node_id = "node-alpha";
    let expected_path = automation_attempt_receipt_path_for_state_dir(&state_dir, run_id, node_id);

    if let Some(parent) = expected_path.parent() {
        std::fs::create_dir_all(parent).expect("receipt parent");
    }

    std::fs::write(
        &expected_path,
        concat!(
            "not-json\n",
            "{\"version\":1,\"run_id\":\"run-ledger-test\",\"node_id\":\"node-alpha\",\"attempt\":1,\"session_id\":\"sess-1\",\"seq\":7,\"ts_ms\":10,\"event_type\":\"started\",\"payload\":{\"step\":\"seed\"}}\n"
        ),
    )
    .expect("seed receipts");

    let summary = append_automation_attempt_receipt(
        &state_dir,
        AutomationAttemptReceiptDraft {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            attempt: 2,
            session_id: "sess-2".to_string(),
            event_type: "completed".to_string(),
            payload: serde_json::json!({"ok": true}),
        },
    )
    .await
    .expect("append receipt");

    assert_eq!(summary.path, expected_path);
    assert_eq!(summary.seq, 8);
    assert_eq!(summary.record_count, 2);

    let raw = std::fs::read_to_string(&expected_path).expect("receipt text");
    let mut lines = raw.lines();
    assert_eq!(lines.next(), Some("not-json"));
    let last_line = lines.last().expect("appended line");
    let appended: AutomationAttemptReceiptRecord =
        serde_json::from_str(last_line).expect("parse appended receipt");
    assert_eq!(appended.version, 1);
    assert_eq!(appended.run_id, run_id);
    assert_eq!(appended.node_id, node_id);
    assert_eq!(appended.attempt, 2);
    assert_eq!(appended.session_id, "sess-2");
    assert_eq!(appended.seq, 8);
    assert_eq!(appended.event_type, "completed");
    assert_eq!(appended.payload, serde_json::json!({"ok": true}));

    let _ = std::fs::remove_dir_all(&state_dir);
}

#[tokio::test]
async fn automation_attempt_receipt_collects_tool_and_artifact_events() {
    let automation = test_phase_automation(
        serde_json::json!([{ "phase_id": "phase_1", "title": "Phase 1", "execution_mode": "soft" }]),
        vec![test_automation_node("draft", Vec::new(), "phase_1", 1)],
    );
    let node = automation.flow.nodes[0].clone();
    let mut session = Session::new(Some("receipt test".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "read".to_string(),
            args: serde_json::json!({"path":"README.md"}),
            result: Some(serde_json::json!({"ok": true})),
            error: None,
        }],
    ));
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "bash".to_string(),
            args: serde_json::json!({"cmd":"false"}),
            result: None,
            error: Some("exit 1".to_string()),
        }],
    ));

    let events = collect_automation_attempt_receipt_events(
        &automation,
        "run-1",
        &node,
        2,
        &session.id,
        &session,
        Some(&("out.md".to_string(), "artifact".to_string())),
        Some("out.md"),
        Some(&serde_json::json!({"status":"succeeded"})),
    );

    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        event_types,
        vec![
            "tool_invoked",
            "tool_succeeded",
            "tool_invoked",
            "tool_failed",
            "artifact_write_success",
        ]
    );
}
#[test]
fn automation_blocked_nodes_respects_barrier_open_phase() {
    let automation = test_phase_automation(
        json!([
            { "phase_id": "phase_1", "title": "Phase 1", "execution_mode": "barrier" },
            { "phase_id": "phase_2", "title": "Phase 2", "execution_mode": "soft" }
        ]),
        vec![
            test_automation_node("draft", Vec::new(), "phase_1", 1),
            test_automation_node("publish", Vec::new(), "phase_2", 100),
        ],
    );
    let run = test_phase_run(vec!["draft", "publish"], Vec::new());

    assert_eq!(
        automation_blocked_nodes(&automation, &run),
        vec!["publish".to_string()]
    );
}

#[test]
fn automation_soft_phase_prefers_current_open_phase_before_priority() {
    let automation = test_phase_automation(
        json!([
            { "phase_id": "phase_1", "title": "Phase 1", "execution_mode": "soft" },
            { "phase_id": "phase_2", "title": "Phase 2", "execution_mode": "soft" }
        ]),
        vec![
            test_automation_node("draft", Vec::new(), "phase_1", 1),
            test_automation_node("publish", Vec::new(), "phase_2", 100),
        ],
    );
    let run = test_phase_run(vec!["draft", "publish"], Vec::new());
    let phase_rank = automation_phase_rank_map(&automation);
    let current_open_phase_rank =
        automation_current_open_phase(&automation, &run).map(|(_, rank, _)| rank);
    let draft = automation
        .flow
        .nodes
        .iter()
        .find(|node| node.node_id == "draft")
        .expect("draft node");
    let publish = automation
        .flow
        .nodes
        .iter()
        .find(|node| node.node_id == "publish")
        .expect("publish node");

    assert!(automation_blocked_nodes(&automation, &run).is_empty());
    assert!(
        automation_node_sort_key(draft, &phase_rank, current_open_phase_rank)
            < automation_node_sort_key(publish, &phase_rank, current_open_phase_rank)
    );
}

#[test]
fn automation_soft_phase_limits_runnable_frontier_to_current_open_phase() {
    let automation = test_phase_automation(
        json!([
            { "phase_id": "phase_1", "title": "Phase 1", "execution_mode": "soft" },
            { "phase_id": "phase_2", "title": "Phase 2", "execution_mode": "soft" }
        ]),
        vec![
            test_automation_node("draft", Vec::new(), "phase_1", 1),
            test_automation_node("publish", Vec::new(), "phase_2", 100),
        ],
    );
    let run = test_phase_run(vec!["draft", "publish"], Vec::new());

    let filtered =
        automation_filter_runnable_by_open_phase(&automation, &run, automation.flow.nodes.clone());

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].node_id, "draft");
}

#[test]
fn automation_routine_dependency_filter_blocks_downstream_routine_until_upstream_completes() {
    let mut automation = test_phase_automation(
        json!([
            { "phase_id": "phase_1", "title": "Phase 1", "execution_mode": "soft" }
        ]),
        vec![
            test_automation_node("step_a", Vec::new(), "phase_1", 1),
            test_automation_node("step_b", Vec::new(), "phase_1", 2),
        ],
    );
    let plan_package_value =
        serde_json::to_value(routine_dependency_plan_package()).expect("plan package json");
    let parsed_plan_package: tandem_plan_compiler::api::PlanPackage =
        serde_json::from_value(plan_package_value.clone()).expect("plan package parse");
    assert_eq!(parsed_plan_package.routine_graph.len(), 2);
    assert_eq!(
        parsed_plan_package.routine_graph[0].steps[0].step_id,
        "step_a"
    );
    assert_eq!(
        parsed_plan_package.routine_graph[1].steps[0].step_id,
        "step_b"
    );
    assert_eq!(
        parsed_plan_package.routine_graph[1].dependencies[0].routine_id,
        "routine_a"
    );
    automation.metadata = Some(json!({
        "plan_package": plan_package_value
    }));

    let incomplete_run = test_phase_run(vec!["step_a", "step_b"], Vec::new());
    let filtered = automation_filter_runnable_by_routine_dependencies(
        &automation,
        &incomplete_run,
        automation.flow.nodes.clone(),
    );
    assert_eq!(
        filtered
            .iter()
            .map(|node| node.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["step_a"]
    );
    assert_eq!(
        automation_blocked_nodes(&automation, &incomplete_run),
        vec!["step_b".to_string()]
    );

    let complete_upstream_run = test_phase_run(vec!["step_b"], vec!["step_a"]);
    let filtered = automation_filter_runnable_by_routine_dependencies(
        &automation,
        &complete_upstream_run,
        vec![automation
            .flow
            .nodes
            .iter()
            .find(|node| node.node_id == "step_b")
            .cloned()
            .expect("step_b node")],
    );
    assert_eq!(
        filtered
            .iter()
            .map(|node| node.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["step_b"]
    );
    assert!(automation_blocked_nodes(&automation, &complete_upstream_run).is_empty());
}

#[test]
fn runnable_write_scope_filter_skips_overlapping_code_nodes() {
    let first = AutomationFlowNode {
        node_id: "first".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "First".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": "code_change",
                "write_scope": "src"
            }
        })),
    };
    let overlapping = AutomationFlowNode {
        node_id: "overlap".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Overlap".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": "code_change",
                "write_scope": "src/lib"
            }
        })),
    };
    let disjoint = AutomationFlowNode {
        node_id: "disjoint".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Disjoint".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": "code_change",
                "write_scope": "docs"
            }
        })),
    };

    let filtered = automation_filter_runnable_by_write_scope_conflicts(
        vec![first.clone(), overlapping, disjoint.clone()],
        3,
    );

    let ids = filtered
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["first", "disjoint"]);
}

#[test]
fn runnable_write_scope_filter_allows_non_code_nodes_to_run_in_parallel() {
    let code = AutomationFlowNode {
        node_id: "code".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Code".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": "code_change",
                "write_scope": "src"
            }
        })),
    };
    let brief = AutomationFlowNode {
        node_id: "brief".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": "marketing-brief.md"
            }
        })),
    };

    let filtered =
        automation_filter_runnable_by_write_scope_conflicts(vec![code.clone(), brief.clone()], 2);

    let ids = filtered
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["code", "brief"]);
}

#[test]
fn output_validator_defaults_follow_existing_runtime_heuristics() {
    let code = AutomationFlowNode {
        node_id: "code".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Implement fix".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": "code_change",
                "output_path": "src/lib.rs"
            }
        })),
    };
    let brief = AutomationFlowNode {
        node_id: "brief".to_string(),
        agent_id: "agent-b".to_string(),
        objective: "Draft research brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: None,
    };
    let review = AutomationFlowNode {
        node_id: "review".to_string(),
        agent_id: "agent-c".to_string(),
        objective: "Approve draft".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "review".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Review),
        gate: None,
        metadata: None,
    };

    assert_eq!(
        automation_output_validator_kind(&code),
        crate::AutomationOutputValidatorKind::CodePatch
    );
    assert_eq!(
        automation_output_validator_kind(&brief),
        crate::AutomationOutputValidatorKind::ResearchBrief
    );
    assert_eq!(
        automation_output_validator_kind(&review),
        crate::AutomationOutputValidatorKind::ReviewDecision
    );
}

#[test]
fn output_validator_explicit_override_wins() {
    let node = AutomationFlowNode {
        node_id: "report".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Write report".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: None,
    };

    assert_eq!(
        automation_output_validator_kind(&node),
        crate::AutomationOutputValidatorKind::StructuredJson
    );
}

#[test]
fn enrich_automation_node_output_overwrites_stale_validator_metadata() {
    let node = AutomationFlowNode {
        node_id: "brief".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Draft research brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: None,
    };
    let output = json!({
        "node_id": "brief",
        "status": "blocked",
        "workflow_class": "artifact",
        "phase": "completed",
        "failure_kind": "verification_failed",
        "validator_kind": "generic_artifact",
        "validator_summary": {
            "kind": "generic_artifact",
            "outcome": "passed"
        },
        "artifact_validation": {
            "unmet_requirements": ["concrete_read_required"]
        }
    });

    let enriched = enrich_automation_node_output_for_contract(&node, &output);
    assert_eq!(
        enriched.get("validator_kind").and_then(Value::as_str),
        Some("research_brief")
    );
    assert_eq!(
        enriched.get("workflow_class").and_then(Value::as_str),
        Some("research")
    );
    assert_eq!(
        enriched.get("phase").and_then(Value::as_str),
        Some("research_validation")
    );
    assert_eq!(
        enriched.get("failure_kind").and_then(Value::as_str),
        Some("research_missing_reads")
    );
    assert_eq!(
        enriched
            .get("validator_summary")
            .and_then(|value| value.get("outcome"))
            .and_then(Value::as_str),
        Some("blocked")
    );
}

#[test]
fn placeholder_artifact_text_is_rejected() {
    assert!(placeholder_like_artifact_text(
        "Completed previously in this run; preserving file creation requirement."
    ));
    assert!(placeholder_like_artifact_text(
        "Created/updated to satisfy workflow artifact requirement. See existing workspace research already completed in this run."
    ));
    assert!(placeholder_like_artifact_text(
        "Marketing brief completed and written to marketing-brief.md."
    ));
    assert!(placeholder_like_artifact_text(
        "Marketing brief already written in prior step; no content change."
    ));
    assert!(placeholder_like_artifact_text(
        "# Status\n\nBlocked handoff"
    ));
    assert!(!placeholder_like_artifact_text(
        "# Marketing Brief\n\n## Audience\nReal sourced content with specific product details."
    ));
}

#[test]
fn artifact_validation_rejection_blocks_node_status() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "write", "websearch"],
        "executed_tools": ["glob", "write"],
        "workspace_inspection_used": true,
        "web_research_used": false
    });
    let artifact_validation = json!({
        "accepted_artifact_path": Value::Null,
        "rejected_artifact_reason": "placeholder overwrite rejected",
        "undeclared_files_created": ["_automation_touch.txt"],
        "auto_cleaned": true,
        "execution_policy": {
            "mode": "filesystem_standard"
        }
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done",
            None,
            &tool_telemetry,
            Some(&artifact_validation),
        );

    assert_eq!(status, "blocked");
    assert_eq!(reason.as_deref(), Some("placeholder overwrite rejected"));
    assert_eq!(approved, None);
}

#[test]
fn research_workflow_failure_kind_is_typed_from_unmet_requirements() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let artifact_validation = json!({
        "semantic_block_reason": "research completed without concrete file reads or required source coverage",
        "unmet_requirements": ["no_concrete_reads", "files_reviewed_not_backed_by_read"],
        "verification": {
            "verification_failed": false
        }
    });

    assert_eq!(
        detect_automation_node_failure_kind(
            &node,
            "blocked",
            None,
            Some("research completed without concrete file reads or required source coverage"),
            Some(&artifact_validation),
        )
        .as_deref(),
        Some("research_missing_reads")
    );
    assert_eq!(
        detect_automation_node_phase(&node, "blocked", Some(&artifact_validation)),
        "research_validation"
    );
    let summary = build_automation_validator_summary(
        crate::AutomationOutputValidatorKind::ResearchBrief,
        "blocked",
        Some("research completed without concrete file reads or required source coverage"),
        Some(&artifact_validation),
    );
    assert_eq!(
        summary.kind,
        crate::AutomationOutputValidatorKind::ResearchBrief
    );
    assert_eq!(summary.outcome, "blocked");
    assert_eq!(
        summary.reason.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert_eq!(
        summary.unmet_requirements,
        vec![
            "no_concrete_reads".to_string(),
            "files_reviewed_not_backed_by_read".to_string()
        ]
    );
}

#[test]
fn research_workflow_status_is_needs_repair_before_repair_budget_is_exhausted() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "websearch", "write"],
        "executed_tools": ["glob", "write"],
    });
    let artifact_validation = json!({
        "semantic_block_reason": "research completed without concrete file reads or required source coverage",
        "unmet_requirements": ["no_concrete_reads", "missing_successful_web_research"],
        "repair_exhausted": false,
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done — `marketing-brief.md` was written.",
            Some(&(
                "marketing-brief.md".to_string(),
                "# Marketing Brief".to_string(),
            )),
            &tool_telemetry,
            Some(&artifact_validation),
        );

    assert_eq!(status, "needs_repair");
    assert!(matches!(
        reason.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
            | Some("research completed without required current web research")
    ));
    assert_eq!(approved, None);
    let summary = build_automation_validator_summary(
        crate::AutomationOutputValidatorKind::ResearchBrief,
        &status,
        reason.as_deref(),
        Some(&artifact_validation),
    );
    assert_eq!(summary.outcome, "needs_repair");
}

#[test]
fn research_workflow_status_blocks_after_repair_budget_is_exhausted() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "websearch", "write"],
        "executed_tools": ["glob", "write"],
    });
    let artifact_validation = json!({
        "semantic_block_reason": "research completed without concrete file reads or required source coverage",
        "unmet_requirements": ["no_concrete_reads", "missing_successful_web_research"],
        "repair_exhausted": true,
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done — `marketing-brief.md` was written.",
            Some(&(
                "marketing-brief.md".to_string(),
                "# Marketing Brief".to_string(),
            )),
            &tool_telemetry,
            Some(&artifact_validation),
        );

    assert_eq!(status, "blocked");
    assert_eq!(
        detect_automation_node_failure_kind(
            &node,
            &status,
            approved,
            reason.as_deref(),
            Some(&artifact_validation),
        )
        .as_deref(),
        Some("research_retry_exhausted")
    );
}

#[test]
fn research_workflow_status_ignores_llm_blocked_when_validation_is_repairable() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "websearch", "write"],
        "executed_tools": ["glob", "write"],
    });
    let artifact_validation = json!({
        "semantic_block_reason": "research completed without concrete file reads or required source coverage",
        "unmet_requirements": ["no_concrete_reads", "missing_successful_web_research"],
        "repair_exhausted": false,
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "The brief is blocked.\n\n{\"status\":\"blocked\",\"reason\":\"tools unavailable\"}",
            Some(&(
                "marketing-brief.md".to_string(),
                "# Marketing Brief".to_string(),
            )),
            &tool_telemetry,
            Some(&artifact_validation),
        );

    assert_eq!(status, "needs_repair");
    assert!(matches!(
        reason.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
            | Some("research completed without required current web research")
    ));
    assert_eq!(approved, None);
}

#[test]
fn research_workflow_status_keeps_blocked_when_repair_is_exhausted_even_if_llm_declares_blocked() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "websearch", "write"],
        "executed_tools": ["glob", "write"],
    });
    let artifact_validation = json!({
        "semantic_block_reason": "research completed without concrete file reads or required source coverage",
        "unmet_requirements": ["no_concrete_reads", "missing_successful_web_research"],
        "repair_exhausted": true,
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "The brief is blocked.\n\n{\"status\":\"blocked\",\"reason\":\"tools unavailable\"}",
            Some(&(
                "marketing-brief.md".to_string(),
                "# Marketing Brief".to_string(),
            )),
            &tool_telemetry,
            Some(&artifact_validation),
        );

    assert_eq!(status, "blocked");
    assert_eq!(reason.as_deref(), Some("tools unavailable"));
    assert_eq!(approved, None);
}

#[test]
fn render_automation_repair_brief_summarizes_previous_research_miss() {
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "research".to_string(),
        objective: "Write marketing-brief.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let prior_output = json!({
        "status": "needs_repair",
        "validator_summary": {
            "reason": "research completed without required current web research",
            "unmet_requirements": [
                "missing_successful_web_research",
                "web_sources_reviewed_missing"
            ]
        },
        "tool_telemetry": {
            "requested_tools": ["glob", "read", "websearch", "write"],
            "executed_tools": ["glob", "write"]
        },
        "artifact_validation": {
            "blocking_classification": "tool_available_but_not_used",
            "unreviewed_relevant_paths": ["docs/pricing.md", "docs/customers.md"],
            "repair_attempt": 1,
            "repair_attempts_remaining": 4,
            "validation_basis": {
                "authority": "filesystem_and_receipts",
                "current_attempt_output_materialized": true,
                "current_attempt_has_recorded_activity": true,
                "current_attempt_has_read": false,
                "current_attempt_has_web_research": false,
                "workspace_inspection_satisfied": false
            },
            "required_next_tool_actions": [
                "Use `read` on the remaining relevant workspace files: docs/pricing.md, docs/customers.md.",
                "Use `websearch` successfully and include the resulting sources in `Web sources reviewed`."
            ]
        }
    });

    let brief =
        render_automation_repair_brief(&node, Some(&prior_output), 2, 5).expect("repair brief");

    assert!(brief.contains("needs_repair"));
    assert!(brief.contains("missing_successful_web_research"));
    assert!(brief.contains("tool_available_but_not_used"));
    assert!(brief.contains("authority=filesystem_and_receipts"));
    assert!(brief.contains("output_materialized=true"));
    assert!(brief.contains("Required next tool actions"));
    assert!(brief.contains("Use `read` on the remaining relevant workspace files"));
    assert!(brief.contains("glob, read, websearch, write"));
    assert!(brief.contains("glob, write"));
    assert!(brief.contains("docs/pricing.md, docs/customers.md"));
    assert!(brief.contains("Remaining repair attempts after this run: 3"));
}

#[test]
fn code_patch_repair_brief_mentions_patch_apply_test_loop() {
    let node = AutomationFlowNode {
        node_id: "code_patch".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Patch the code and verify the change.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "code_patch".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::CodePatch),
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
                "output_path": "src/lib.rs",
                "verification_command": "cargo test",
                "write_scope": "repo-scoped edits"
            }
        })),
    };
    let prior_output = json!({
        "status": "needs_repair",
        "validator_summary": {
            "reason": "verification did not run",
            "unmet_requirements": ["verification_missing"]
        },
        "tool_telemetry": {
            "requested_tools": ["glob", "read", "edit", "apply_patch", "write"],
            "executed_tools": ["glob", "read", "write"]
        },
        "artifact_validation": {
            "blocking_classification": "verification_required",
            "repair_attempt": 1,
            "repair_attempts_remaining": 4,
            "required_next_tool_actions": [
                "Patch the code with `edit` or `apply_patch` before any new `write`.",
                "Run `cargo test` after the patch and fix the smallest failing root cause."
            ]
        }
    });

    let brief =
        render_automation_repair_brief(&node, Some(&prior_output), 2, 5).expect("repair brief");

    assert!(brief.contains("Code workflow repair path"));
    assert!(brief.contains("inspect the touched files"));
    assert!(brief.contains("edit` or `apply_patch"));
    assert!(brief.contains("cargo test"));
    assert!(brief.contains("repo-scoped edits"));
}

#[test]
fn automation_output_enforcement_prefers_contract_over_legacy_builder_metadata() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: Some(crate::AutomationOutputEnforcement {
                validation_profile: None,
                required_tools: vec!["read".to_string()],
                required_evidence: vec!["local_source_reads".to_string()],
                required_sections: vec!["files_reviewed".to_string()],
                prewrite_gates: vec!["workspace_inspection".to_string()],
                retry_on_missing: vec!["local_source_reads".to_string()],
                terminal_on: vec!["repair_budget_exhausted".to_string()],
                repair_budget: Some(2),
                session_text_recovery: Some("disabled".to_string()),
            }),
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": "marketing-brief.md",
                "required_tools": ["read", "websearch"],
                "web_research_expected": true
            }
        })),
    };

    let enforcement = automation_node_output_enforcement(&node);
    assert_eq!(enforcement.required_tools, vec!["read"]);
    assert_eq!(enforcement.required_evidence, vec!["local_source_reads"]);
    assert_eq!(
        enforcement.session_text_recovery.as_deref(),
        Some("disabled")
    );
}

#[test]
fn automation_output_enforcement_backfills_research_contract_from_legacy_builder_metadata() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "required_tools": ["read", "websearch"],
                "web_research_expected": true
            }
        })),
    };

    let enforcement = automation_node_output_enforcement(&node);
    assert!(enforcement.required_tools.iter().any(|tool| tool == "read"));
    assert!(enforcement
        .required_tools
        .iter()
        .any(|tool| tool == "websearch"));
    assert!(enforcement
        .required_sections
        .iter()
        .any(|item| item == "web_sources_reviewed"));
    assert_eq!(
        enforcement.session_text_recovery.as_deref(),
        Some("require_prewrite_satisfied")
    );
}

#[test]
fn research_nodes_default_to_five_attempts() {
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "research".to_string(),
        objective: "Write marketing-brief.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };

    assert_eq!(automation_node_max_attempts(&node), 5);
}

#[test]
fn first_attempt_research_prompt_requires_completed_status() {
    let automation = AutomationV2Spec {
        automation_id: "automation-1".to_string(),
        name: "Research Automation".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "research".to_string(),
        objective: "Write marketing-brief.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
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
                "output_path": "marketing-brief.md",
                "source_coverage_required": true,
                "web_research_expected": true
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "research".to_string(),
        template_id: None,
        display_name: "Research".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec![
                "glob".to_string(),
                "read".to_string(),
                "websearch".to_string(),
                "write".to_string(),
            ],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-1",
        &node,
        1,
        &agent,
        &[],
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
        None,
        None,
        None,
    );

    assert!(prompt.contains("`status` set to `completed`"));
    assert!(prompt.contains("Do not declare the output blocked"));
    assert!(!prompt.contains("at least `status` (`completed` or `blocked`)"));
}

#[test]
fn code_patch_prompt_includes_code_agent_contract_instructions() {
    let automation = AutomationV2Spec {
        automation_id: "automation-code-patch".to_string(),
        name: "Code Patch Prompt".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "code_patch".to_string(),
        agent_id: "coder".to_string(),
        objective: "Patch the code and verify the change.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "code_patch".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::CodePatch),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Produce a patch-backed artifact.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": "code_change",
                "output_path": "src/lib.rs",
                "verification_command": "cargo test"
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "coder".to_string(),
        template_id: None,
        display_name: "Coder".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec![
                "read".to_string(),
                "edit".to_string(),
                "apply_patch".to_string(),
                "write".to_string(),
                "bash".to_string(),
            ],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-code",
        &node,
        1,
        &agent,
        &[],
        &[
            "read".to_string(),
            "edit".to_string(),
            "apply_patch".to_string(),
            "write".to_string(),
            "bash".to_string(),
        ],
        None,
        None,
        None,
    );

    assert!(prompt.contains("Code Agent Contract:"));
    assert!(prompt.contains("inspect -> patch -> apply -> test -> repair"));
    assert!(prompt.contains("Do not claim completion until the patch has been applied"));
    assert!(prompt.contains("Run the declared verification command after applying changes"));
}

#[test]
fn automation_node_required_tools_reads_builder_metadata() {
    let node = AutomationFlowNode {
        node_id: "artifact".to_string(),
        agent_id: "writer".to_string(),
        objective: "Write notes.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "artifact".to_string(),
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
                "output_path": "notes.md",
                "required_tools": ["read", "websearch"]
            }
        })),
    };

    assert_eq!(
        automation_node_required_tools(&node),
        vec!["read".to_string(), "websearch".to_string()]
    );
}

#[test]
fn filter_requested_tools_to_available_removes_unconfigured_websearch() {
    let requested_tools = vec![
        "glob".to_string(),
        "read".to_string(),
        "websearch".to_string(),
        "write".to_string(),
    ];
    let available_tool_names = ["glob", "read", "write"]
        .into_iter()
        .map(str::to_string)
        .collect::<HashSet<_>>();

    let filtered = filter_requested_tools_to_available(requested_tools, &available_tool_names);

    assert_eq!(
        filtered,
        vec!["glob".to_string(), "read".to_string(), "write".to_string()]
    );
}

#[test]
fn wildcard_automation_allowlist_expands_to_minimal_research_tools() {
    let node = AutomationFlowNode {
        node_id: "research_sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Research current web sources and collect citations.".to_string(),
        depends_on: Vec::new(),
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "collect_inputs".to_string(),
            alias: "research_brief".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "citations".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
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
                "output_path": ".tandem/artifacts/research-sources.json"
            }
        })),
    };

    let requested = normalize_automation_requested_tools(&node, "/tmp", vec!["*".to_string()]);

    assert_eq!(
        requested,
        vec![
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string()
        ]
    );
}

#[test]
fn wildcard_automation_allowlist_keeps_email_delivery_tools_narrow() {
    let node = AutomationFlowNode {
        node_id: "notify_user".to_string(),
        agent_id: "committer".to_string(),
        objective: "Send the finalized report to the requested email address.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": "evan@frumu.ai",
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    };
    let available_tool_names = [
        "glob".to_string(),
        "read".to_string(),
        "mcp.gmail_alt.gmail_send_draft".to_string(),
        "mcp.gmail_alt.gmail_send_email".to_string(),
        "mcp.github.issues_list".to_string(),
    ]
    .into_iter()
    .collect::<HashSet<_>>();
    let requested = automation_requested_tools_for_node(
        &node,
        "/tmp",
        vec!["*".to_string()],
        &available_tool_names,
    );

    assert!(!requested.iter().any(|tool| tool == "*"));
    assert!(requested
        .iter()
        .any(|tool| tool == "mcp.gmail_alt.gmail_send_email"));
    assert!(requested
        .iter()
        .any(|tool| tool == "mcp.gmail_alt.gmail_send_draft"));
    assert!(!requested
        .iter()
        .any(|tool| tool == "mcp.github.issues_list"));
}

#[test]
fn wildcard_automation_allowlist_recognizes_outlook_reply_and_compose_email_tools() {
    let node = AutomationFlowNode {
        node_id: "notify_user".to_string(),
        agent_id: "committer".to_string(),
        objective: "Send the finalized report to the requested email address.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": "evan@frumu.ai",
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    };
    let available_tool_names = [
        "glob".to_string(),
        "read".to_string(),
        "mcp.outlook.reply_message".to_string(),
        "mcp.outlook.compose_draft".to_string(),
        "mcp.github.issues_list".to_string(),
    ]
    .into_iter()
    .collect::<HashSet<_>>();
    let requested = automation_requested_tools_for_node(
        &node,
        "/tmp",
        vec!["*".to_string()],
        &available_tool_names,
    );

    assert!(requested
        .iter()
        .any(|tool| tool == "mcp.outlook.reply_message"));
    assert!(requested
        .iter()
        .any(|tool| tool == "mcp.outlook.compose_draft"));
    assert!(!requested
        .iter()
        .any(|tool| tool == "mcp.github.issues_list"));
}

#[test]
fn structured_json_prompt_requires_json_only_without_follow_up_questions() {
    let automation = AutomationV2Spec {
        automation_id: "automation-json-only".to_string(),
        name: "JSON Only".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "research-external-research".to_string(),
        agent_id: "research".to_string(),
        objective: "Perform external research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Return structured JSON.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "title": "External Research",
                "role": "watcher",
                "prompt": "Return structured research handoff.",
                "research_stage": "research_external_sources"
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "research".to_string(),
        template_id: None,
        display_name: "Research".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec![
                "glob".to_string(),
                "read".to_string(),
                "webfetch".to_string(),
            ],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-json-only",
        &node,
        2,
        &agent,
        &[],
        &[
            "glob".to_string(),
            "read".to_string(),
            "webfetch".to_string(),
        ],
        None,
        None,
        None,
    );

    assert!(prompt.contains("The final response body should contain JSON only"));
    assert!(prompt.contains("Do not include headings, bullets, markdown fences, prose explanations, or follow-up questions"));
}

#[test]
fn external_research_prompt_handles_missing_websearch_tool() {
    let automation = AutomationV2Spec {
        automation_id: "automation-external-fallback".to_string(),
        name: "External Fallback".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "research-external-research".to_string(),
        agent_id: "research".to_string(),
        objective: "Perform targeted external research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Return structured JSON.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "title": "External Research",
                "role": "watcher",
                "prompt": "Use external tools and return structured findings.",
                "research_stage": "research_external_sources",
                "web_research_expected": true
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "research".to_string(),
        template_id: None,
        display_name: "Research".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec![
                "glob".to_string(),
                "read".to_string(),
                "webfetch".to_string(),
            ],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-external-fallback",
        &node,
        2,
        &agent,
        &[],
        &[
            "glob".to_string(),
            "read".to_string(),
            "webfetch".to_string(),
        ],
        None,
        None,
        None,
    );

    assert!(prompt.contains("`websearch` is not available in this run"));
    assert!(prompt.contains("Use `webfetch` only for concrete URLs already present in local sources or upstream handoffs"));
    assert!(prompt.contains("Do not ask the user for clarification or permission to continue"));
}

#[test]
fn render_prompt_normalizes_upstream_research_paths_from_sources_root() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-upstream-path-normalization-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("tandem-reference/readmes"))
        .expect("create readmes dir");
    std::fs::write(
        workspace_root.join("tandem-reference/SOURCES.md"),
        "# Sources",
    )
    .expect("seed sources");
    std::fs::write(
        workspace_root.join("tandem-reference/readmes/repo-README.md"),
        "# Repo",
    )
    .expect("seed repo readme");

    let automation = AutomationV2Spec {
        automation_id: "automation-upstream-paths".to_string(),
        name: "Upstream Paths".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "research-local-sources".to_string(),
        agent_id: "research".to_string(),
        objective: "Read prioritized local files".to_string(),
        depends_on: vec!["research-discover-sources".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "research-discover-sources".to_string(),
            alias: "source_inventory".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Return structured JSON.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "title": "Read Local Sources",
                "role": "watcher",
                "prompt": "Use the upstream handoff as the file plan.",
                "research_stage": "research_local_sources"
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "research".to_string(),
        template_id: None,
        display_name: "Research".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["read".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };
    let upstream_inputs = vec![json!({
        "alias": "source_inventory",
        "from_step_id": "research-discover-sources",
        "output": {
            "artifact_validation": {
                "read_paths": ["tandem-reference/SOURCES.md"],
                "current_node_read_paths": ["tandem-reference/SOURCES.md"]
            },
            "content": {
                "structured_handoff": {
                    "discovered_paths": ["readmes/repo-README.md"],
                    "priority_paths": ["readmes/repo-README.md"]
                },
                "text": "{\"discovered_paths\":[\"readmes/repo-README.md\"],\"priority_paths\":[\"readmes/repo-README.md\"]}"
            }
        }
    })];

    let prompt = render_automation_v2_prompt(
        &automation,
        workspace_root.to_str().expect("workspace root string"),
        "run-upstream-paths",
        &node,
        1,
        &agent,
        &upstream_inputs,
        &["read".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("tandem-reference/readmes/repo-README.md"));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn generic_required_tools_prewrite_requirements_enable_repair() {
    let node = AutomationFlowNode {
        node_id: "artifact".to_string(),
        agent_id: "writer".to_string(),
        objective: "Write notes.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "artifact".to_string(),
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
                "output_path": "notes.md",
                "web_research_expected": true,
                "required_tools": ["read", "websearch"]
            }
        })),
    };

    let requirements = automation_node_prewrite_requirements(
        &node,
        &[
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    )
    .expect("prewrite requirements");

    assert!(requirements.concrete_read_required);
    assert!(requirements.successful_web_research_required);
    assert!(requirements.repair_on_unmet_requirements);
    assert_eq!(requirements.coverage_mode, PrewriteCoverageMode::None);
}

#[test]
fn research_finalize_prewrite_requirements_skip_same_node_reads_and_websearch() {
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "research".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: Some(crate::AutomationOutputEnforcement {
                validation_profile: Some("research_synthesis".to_string()),
                required_tools: Vec::new(),
                required_evidence: vec![
                    "local_source_reads".to_string(),
                    "external_sources".to_string(),
                ],
                required_sections: vec![
                    "files_reviewed".to_string(),
                    "files_not_reviewed".to_string(),
                    "citations".to_string(),
                    "web_sources_reviewed".to_string(),
                ],
                prewrite_gates: Vec::new(),
                retry_on_missing: Vec::new(),
                terminal_on: Vec::new(),
                repair_budget: Some(5),
                session_text_recovery: None,
            }),
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": "marketing-brief.md",
                "research_stage": "research_finalize"
            }
        })),
    };

    let requirements = automation_node_prewrite_requirements(
        &node,
        &[
            "read".to_string(),
            "write".to_string(),
            "websearch".to_string(),
        ],
    )
    .expect("prewrite requirements");

    assert!(!requirements.workspace_inspection_required);
    assert!(!requirements.web_research_required);
    assert!(!requirements.concrete_read_required);
    assert!(!requirements.successful_web_research_required);
}

#[test]
fn generic_required_tools_validation_needs_repair_when_read_unused() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-required-tools-test-{}", now_ms()));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let node = AutomationFlowNode {
        node_id: "artifact".to_string(),
        agent_id: "writer".to_string(),
        objective: "Write notes.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "artifact".to_string(),
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
                "output_path": "notes.md",
                "required_tools": ["read"]
            }
        })),
    };
    let mut session = Session::new(Some("required tools".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path":"notes.md",
                "content":"# Notes\n\nA short summary written without reading sources.\n"
            }),
            result: Some(json!({"output":"written"})),
            error: None,
        }],
    ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &["read".to_string(), "write".to_string()],
    );
    let (_, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root"),
        "",
        &tool_telemetry,
        None,
        Some((
            "notes.md".to_string(),
            "# Notes\n\nA short summary written without reading sources.\n".to_string(),
        )),
        &std::collections::BTreeSet::new(),
    );

    assert!(rejected.is_some());
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("needs_repair")
    );
    assert_eq!(
        artifact_validation
            .get("blocking_classification")
            .and_then(Value::as_str),
        Some("tool_available_but_not_used")
    );
    assert_eq!(
        artifact_validation
            .get("required_next_tool_actions")
            .and_then(Value::as_array)
            .and_then(|rows| rows.first())
            .and_then(Value::as_str),
        Some("Use `read` on concrete workspace files before finalizing the brief.")
    );

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done — `notes.md` was written.",
            Some(&(
                "notes.md".to_string(),
                "# Notes\n\nA short summary written without reading sources.\n".to_string(),
            )),
            &tool_telemetry,
            Some(&artifact_validation),
        );
    assert_eq!(status, "needs_repair");
    assert_eq!(
        detect_automation_node_failure_kind(
            &node,
            &status,
            approved,
            reason.as_deref(),
            Some(&artifact_validation),
        )
        .as_deref(),
        Some("required_tool_unused_read")
    );

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn generic_required_tools_nodes_default_to_five_attempts() {
    let node = AutomationFlowNode {
        node_id: "artifact".to_string(),
        agent_id: "writer".to_string(),
        objective: "Write notes.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "artifact".to_string(),
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
                "output_path": "notes.md",
                "required_tools": ["read"]
            }
        })),
    };

    assert_eq!(automation_node_max_attempts(&node), 5);
}

#[test]
fn first_attempt_required_tools_prompt_requires_completed_status() {
    let automation = AutomationV2Spec {
        automation_id: "automation-2".to_string(),
        name: "Generic Artifact Automation".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "artifact".to_string(),
        agent_id: "writer".to_string(),
        objective: "Write notes.md".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "artifact".to_string(),
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
                "output_path": "notes.md",
                "required_tools": ["read"]
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "writer".to_string(),
        template_id: None,
        display_name: "Writer".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["read".to_string(), "write".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-2",
        &node,
        1,
        &agent,
        &[],
        &["read".to_string(), "write".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("`status` set to `completed`"));
    assert!(prompt.contains("required workflow tools remain available"));
    assert!(!prompt.contains("at least `status` (`completed` or `blocked`)"));
}

#[test]
fn first_attempt_required_tools_prompt_without_output_path_requires_handoff() {
    let automation = AutomationV2Spec {
        automation_id: "automation-structured".to_string(),
        name: "Structured Handoff Automation".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "discover".to_string(),
        agent_id: "research-discover".to_string(),
        objective: "Enumerate sources".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: Some(crate::AutomationOutputEnforcement {
                validation_profile: Some("local_research".to_string()),
                required_tools: vec!["read".to_string()],
                required_evidence: Vec::new(),
                required_sections: Vec::new(),
                prewrite_gates: vec![
                    "workspace_inspection".to_string(),
                    "concrete_reads".to_string(),
                ],
                retry_on_missing: vec![
                    "workspace_inspection".to_string(),
                    "concrete_reads".to_string(),
                ],
                terminal_on: Vec::new(),
                repair_budget: Some(5),
                session_text_recovery: Some("require_prewrite_satisfied".to_string()),
            }),
            schema: None,
            summary_guidance: Some("Return a structured handoff.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "research_stage": "research_discover",
                "required_tools": ["read"]
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "research-discover".to_string(),
        template_id: None,
        display_name: "Research Discover".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["glob".to_string(), "read".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-structured",
        &node,
        1,
        &agent,
        &[],
        &["glob".to_string(), "read".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("`status` set to `completed`"));
    assert!(prompt.contains("required workflow tools remain available"));
    assert!(prompt.contains(
            "Do not claim success unless the required structured handoff was actually returned in the final response."
        ));
    assert!(!prompt.contains("write tool actually created the output file"));
}

#[test]
fn prompt_includes_inline_metadata_inputs_and_temp_file_warning() {
    let automation = AutomationV2Spec {
        automation_id: "automation-inline-inputs".to_string(),
        name: "Inline Inputs Automation".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "collect_inputs".to_string(),
        agent_id: "planner".to_string(),
        objective: "Capture the report topic, delivery target, and formatting constraints."
            .to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
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
            "inputs": {
                "topic": "autonomous AI agentic workflows",
                "delivery_email": "evan@frumu.ai",
                "email_format": "simple html"
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "planner".to_string(),
        template_id: None,
        display_name: "Planner".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["read".to_string(), "write".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-inline",
        &node,
        1,
        &agent,
        &[],
        &["read".to_string(), "write".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("Node Inputs:"));
    assert!(prompt.contains("autonomous AI agentic workflows"));
    assert!(prompt.contains("Do not search `/tmp`"));
}

#[test]
fn collect_inputs_prompt_requires_reading_before_writing() {
    let automation = AutomationV2Spec {
        automation_id: "automation-collect-inputs".to_string(),
        name: "Collect Inputs".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "collect_inputs".to_string(),
        agent_id: "planner".to_string(),
        objective: "Inspect the workspace and ground the project identity before web research."
            .to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
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
                "output_path": "collect-inputs.json"
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "planner".to_string(),
        template_id: None,
        display_name: "Planner".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["glob".to_string(), "read".to_string(), "write".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-collect-inputs",
        &node,
        1,
        &agent,
        &[],
        &["glob".to_string(), "read".to_string(), "write".to_string()],
        None,
        None,
        None,
    );

    let expected_output_path = crate::app::state::automation::automation_run_scoped_output_path(
        "run-collect-inputs",
        ".tandem/artifacts/collect-inputs.json",
    )
    .expect("scoped output path");
    assert!(prompt.contains("Collect Inputs Contract:"));
    assert!(prompt.contains("do not stop after discovery"));
    assert!(prompt.contains(&format!(
        "Write the grounded result to `{}`",
        expected_output_path
    )));
}

#[test]
fn prompt_includes_email_delivery_metadata_for_notify_user() {
    let automation = AutomationV2Spec {
        automation_id: "automation-email-delivery".to_string(),
        name: "Email Delivery Automation".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "notify_user".to_string(),
        agent_id: "committer".to_string(),
        objective: "Send the finalized report to the requested email address in the email body using simple HTML.".to_string(),
        depends_on: vec!["generate_report".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "generate_report".to_string(),
            alias: "report_body".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": "evan@frumu.ai",
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "committer".to_string(),
        template_id: None,
        display_name: "Committer".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["*".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: vec!["composio-1".to_string()],
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-email",
        &node,
        1,
        &agent,
        &[json!({
            "alias": "report_body",
            "from_step_id": "generate_report",
            "output": {
                "content": {
                    "path": ".tandem/artifacts/generate-report.html",
                    "text": "<h1>Tandem Strategic Analysis</h1><p>Rich upstream report body.</p>"
                }
            }
        })],
        &["*".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("Delivery target:"));
    assert!(prompt.contains("`evan@frumu.ai`"));
    assert!(prompt.contains("Inline body only: `true`"));
    assert!(prompt.contains("Upstream synthesis rules:"));
    assert!(
        prompt.contains("use the compiled upstream report/body as the email body source of truth")
    );
    assert!(prompt.contains("Deterministic Delivery Body:"));
    assert!(prompt.contains("Source artifact: `.tandem/artifacts/generate-report.html`"));
    assert!(prompt.contains("<h1>Tandem Strategic Analysis</h1>"));
    assert!(prompt.contains(
        "Do not mark the node completed unless you actually execute an email draft or send tool."
    ));
}

#[test]
fn prompt_compacts_upstream_outputs_for_downstream_nodes() {
    let automation = AutomationV2Spec {
        automation_id: "automation-compact-upstream".to_string(),
        name: "Compact Upstream Automation".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "analyze_findings".to_string(),
        agent_id: "analyst".to_string(),
        objective: "Synthesize the clustered themes into a concise analysis and recommendations."
            .to_string(),
        depends_on: vec!["cluster_topics".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "cluster_topics".to_string(),
            alias: "topic_clusters".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };
    let agent = AutomationAgentProfile {
        agent_id: "analyst".to_string(),
        template_id: None,
        display_name: "Analyst".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["*".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };
    let upstream_inputs = vec![json!({
        "alias": "topic_clusters",
        "from_step_id": "cluster_topics",
        "output": {
            "status": "completed",
            "phase": "completed",
            "summary": "Clustered pain points into report themes.",
            "contract_kind": "structured_json",
            "artifact_validation": {
                "accepted_artifact_path": ".tandem/artifacts/cluster-topics.json",
                "artifact_candidates": [{"source": "verified_output", "score": 999}],
                "validation_outcome": "passed",
                "warning_count": 0
            },
            "validator_summary": {
                "kind": "structured_json",
                "outcome": "passed",
                "warning_count": 0
            },
            "tool_telemetry": {
                "executed_tools": ["read", "write"],
                "tool_call_counts": {"read": 1, "write": 1}
            },
            "content": {
                "path": ".tandem/artifacts/cluster-topics.json",
                "raw_assistant_text": "very verbose narrative",
                "text": "{\"themes\":[{\"id\":\"T1\",\"summary\":\"alpha\"}],\"cross_cutting_observation\":\"beta\"}"
            }
        }
    })];

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-compact",
        &node,
        1,
        &agent,
        &upstream_inputs,
        &["*".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("\"themes\""));
    assert!(prompt.contains("\"cross_cutting_observation\""));
    assert!(!prompt.contains("artifact_candidates"));
    assert!(!prompt.contains("raw_assistant_text"));
    assert!(!prompt.contains("tool_call_counts"));
}

#[test]
fn standard_workflow_nodes_receive_default_workspace_output_paths() {
    let node = AutomationFlowNode {
        node_id: "research_sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Research sources".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "citations".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };

    assert_eq!(
        automation_node_required_output_path(&node).as_deref(),
        Some(".tandem/artifacts/research-sources.json")
    );
}

#[test]
fn report_markdown_retries_accept_html_sibling_outputs() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-html-sibling-{}",
        uuid::Uuid::new_v4()
    ));
    let artifact_dir = workspace_root.join(".tandem/runs/run-research/artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::write(
        artifact_dir.join("generate-report.html"),
        "<!doctype html><html><body>Report</body></html>",
    )
    .expect("write html artifact");

    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Draft the report in simple HTML suitable for email body delivery.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
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
                "output_path": ".tandem/artifacts/generate-report.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("generate-report-retry".to_string()),
        Some(workspace_root.to_str().expect("workspace utf8").to_string()),
    );
    let expected_output_path = crate::app::state::automation::automation_run_scoped_output_path(
        "run-research",
        ".tandem/artifacts/generate-report.md",
    )
    .expect("scoped output path");
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": expected_output_path.replace("generate-report.md", "generate-report.html"),
                "content": "<!doctype html><html><body>Report</body></html>"
            }),
            result: Some(json!({"output":"written"})),
            error: None,
        }],
    ));

    let resolved = automation_resolve_verified_output_path(
        &session,
        workspace_root.to_str().expect("workspace utf8"),
        "run-research",
        &node,
        ".tandem/artifacts/generate-report.md",
    )
    .expect("resolve verified output")
    .expect("accepted sibling output");

    assert_eq!(
        resolved
            .file_name()
            .and_then(|value| value.to_str())
            .expect("file name"),
        "generate-report.html"
    );

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn automation_resolve_verified_output_path_accepts_file_path_schema_with_dot_segments() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-html-sibling-file-path-{}",
        uuid::Uuid::new_v4()
    ));
    let artifact_path = workspace_root.join(".tandem/runs/run-research/artifacts/report.md");
    std::fs::create_dir_all(
        artifact_path
            .parent()
            .expect("artifact path should have parent"),
    )
    .expect("create artifact dir");
    std::fs::write(&artifact_path, "report body").expect("write artifact");

    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Draft the report in simple HTML suitable for email body delivery.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
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
                "output_path": ".tandem/artifacts/report.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("generate-report-file-path".to_string()),
        Some(workspace_root.to_str().expect("workspace utf8").to_string()),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![tandem_types::MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "filePath": artifact_path
                    .parent()
                    .expect("artifact path should have parent")
                    .join("./report.md")
                    .to_string_lossy(),
                "content": "report body"
            }),
            result: Some(json!({"output":"written"})),
            error: None,
        }],
    ));

    let resolved = automation_resolve_verified_output_path(
        &session,
        workspace_root.to_str().expect("workspace utf8"),
        "run-research",
        &node,
        ".tandem/artifacts/report.md",
    )
    .expect("resolve verified output")
    .expect("accepted normalized output");

    assert_eq!(resolved, artifact_path);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn citations_nodes_do_not_require_files_reviewed_sections_by_default() {
    let node = AutomationFlowNode {
        node_id: "research_sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Research sources".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "citations".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
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
                "output_path": ".tandem/artifacts/research-sources.json",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };

    let enforcement = automation_node_output_enforcement(&node);

    assert!(enforcement
        .required_sections
        .iter()
        .any(|item| item == "citations"));
    assert!(enforcement
        .validation_profile
        .as_deref()
        .is_some_and(|value| value == "external_research"));
    assert!(!enforcement
        .required_sections
        .iter()
        .any(|item| item == "files_reviewed"));
    assert!(!enforcement
        .required_sections
        .iter()
        .any(|item| item == "files_not_reviewed"));
}

#[test]
fn collect_inputs_nodes_write_deterministic_inline_artifacts() {
    let node = AutomationFlowNode {
        node_id: "collect_inputs".to_string(),
        agent_id: "planner".to_string(),
        objective: "Gather workflow inputs".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "inputs": {
                "topic": "autonomous AI agentic workflows",
                "delivery_email": "evan@frumu.ai",
                "email_format": "simple html",
                "attachments_allowed": false
            }
        })),
    };

    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-inline-artifact-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&workspace_root).expect("temp workspace");

    let output_path =
        automation_node_required_output_path(&node).expect("collect_inputs output path");
    let payload = automation_node_inline_artifact_payload(&node).expect("inline payload");
    let (written_path, file_text) = write_automation_inline_artifact(
        workspace_root.to_str().expect("workspace utf8"),
        "run-inline-collect",
        &output_path,
        &payload,
    )
    .expect("inline artifact write");

    assert_eq!(
        written_path,
        ".tandem/runs/run-inline-collect/artifacts/collect-inputs.json"
    );
    assert!(file_text.contains("autonomous AI agentic workflows"));

    let resolved =
        workspace_root.join(".tandem/runs/run-inline-collect/artifacts/collect-inputs.json");
    assert!(resolved.exists());
    let persisted = std::fs::read_to_string(&resolved).expect("read artifact");
    assert!(persisted.contains("\"delivery_email\": \"evan@frumu.ai\""));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn execute_collect_inputs_node_uses_deterministic_shortcut() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-collect-inputs-exec-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("workspace");

    let automation = AutomationV2Spec {
        automation_id: "automation-inline-collect-inputs".to_string(),
        name: "Collect Inputs Shortcut".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: vec![AutomationAgentProfile {
            agent_id: "agent_planner".to_string(),
            template_id: None,
            display_name: "Planner".to_string(),
            avatar_url: None,
            model_policy: Some(json!({
                "default_model": "openrouter/not-a-real-model"
            })),
            skills: Vec::new(),
            tool_policy: AutomationAgentToolPolicy {
                allowlist: vec!["*".to_string()],
                denylist: Vec::new(),
            },
            mcp_policy: AutomationAgentMcpPolicy {
                allowed_servers: Vec::new(),
                allowed_tools: None,
            },
            approval_policy: None,
        }],
        flow: AutomationFlowSpec {
            nodes: vec![AutomationFlowNode {
                node_id: "collect_inputs".to_string(),
                agent_id: "agent_planner".to_string(),
                objective: "Capture the report topic, delivery target, and formatting constraints."
                    .to_string(),
                depends_on: Vec::new(),
                input_refs: Vec::new(),
                output_contract: Some(AutomationFlowOutputContract {
                    kind: "brief".to_string(),
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
                    "inputs": {
                        "topic": "autonomous AI agentic workflows",
                        "delivery_email": "evan@frumu.ai",
                        "email_format": "simple html",
                        "attachments_allowed": false
                    }
                })),
            }],
        },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: crate::now_ms(),
        updated_at_ms: crate::now_ms(),
        creator_id: "test".to_string(),
        workspace_root: Some(workspace_root.to_string_lossy().to_string()),
        metadata: Some(json!({
            "context_materialization": {
                "routines": [
                    {
                        "routine_id": "collect_inputs",
                        "visible_context_objects": [],
                        "step_context_bindings": [
                            {
                                "step_id": "collect_inputs",
                                "context_reads": ["ctx:collect_inputs:mission.goal"],
                                "context_writes": []
                            }
                        ]
                    }
                ]
            }
        })),
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };

    let state = ready_test_state().await;
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    assert_eq!(
        run.runtime_context
            .as_ref()
            .map(|context| context.routines.len()),
        Some(1)
    );
    state
        .update_automation_v2_run(&run.run_id, |row| {
            row.runtime_context = None;
        })
        .await
        .expect("clear runtime context");
    let claimed = state
        .claim_specific_automation_v2_run(&run.run_id)
        .await
        .expect("claim run");
    assert_eq!(
        claimed
            .runtime_context
            .as_ref()
            .map(|context| context.routines.len()),
        Some(1)
    );
    let node = automation.flow.nodes.first().expect("collect_inputs node");
    let agent = automation.agents.first().expect("planner agent");

    let output = execute_automation_v2_node(&state, &claimed.run_id, &automation, node, agent)
        .await
        .expect("execute collect_inputs");

    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .get("artifact_validation")
            .and_then(|value| value.get("deterministic_artifact"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output
            .get("artifact_validation")
            .and_then(|value| value.get("deterministic_source"))
            .and_then(Value::as_str),
        Some("node_metadata_inputs")
    );

    let artifact_path = workspace_root
        .join(".tandem/runs")
        .join(&claimed.run_id)
        .join("artifacts")
        .join("collect-inputs.json");
    assert!(artifact_path.exists());
    let artifact_text = std::fs::read_to_string(&artifact_path).expect("artifact text");
    assert!(artifact_text.contains("autonomous AI agentic workflows"));

    let session_id = output
        .get("content")
        .and_then(|value| value.get("session_id"))
        .and_then(Value::as_str)
        .expect("session id");
    let session = state
        .storage
        .get_session(session_id)
        .await
        .expect("deterministic session");
    assert!(session.messages.iter().all(|message| {
        message
            .parts
            .iter()
            .all(|part| !matches!(part, tandem_types::MessagePart::ToolInvocation { .. }))
    }));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn automation_run_requeue_increments_attempt_counter() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-requeue-attempts-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace_root).expect("workspace");

    let automation = AutomationV2Spec {
        automation_id: "automation-inline-requeue-attempts".to_string(),
        name: "Requeue Attempt Increments".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: vec![AutomationAgentProfile {
            agent_id: "agent_planner".to_string(),
            template_id: None,
            display_name: "Planner".to_string(),
            avatar_url: None,
            model_policy: Some(json!({
                "default_model": "openrouter/not-a-real-model"
            })),
            skills: Vec::new(),
            tool_policy: AutomationAgentToolPolicy {
                allowlist: vec!["*".to_string()],
                denylist: Vec::new(),
            },
            mcp_policy: AutomationAgentMcpPolicy {
                allowed_servers: Vec::new(),
                allowed_tools: None,
            },
            approval_policy: None,
        }],
        flow: AutomationFlowSpec {
            nodes: vec![AutomationFlowNode {
                node_id: "collect_inputs".to_string(),
                agent_id: "agent_planner".to_string(),
                objective: "Capture the report topic, delivery target, and formatting constraints."
                    .to_string(),
                depends_on: Vec::new(),
                input_refs: Vec::new(),
                output_contract: Some(AutomationFlowOutputContract {
                    kind: "brief".to_string(),
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
                    "inputs": {
                        "topic": "autonomous AI agentic workflows",
                        "delivery_email": "evan@frumu.ai",
                        "email_format": "simple html",
                        "attachments_allowed": false
                    }
                })),
            }],
        },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: crate::now_ms(),
        updated_at_ms: crate::now_ms(),
        creator_id: "test".to_string(),
        workspace_root: Some(workspace_root.to_string_lossy().to_string()),
        metadata: Some(json!({
            "context_materialization": {
                "routines": [
                    {
                        "routine_id": "collect_inputs",
                        "visible_context_objects": [],
                        "step_context_bindings": [
                            {
                                "step_id": "collect_inputs",
                                "context_reads": ["ctx:collect_inputs:mission.goal"],
                                "context_writes": []
                            }
                        ]
                    }
                ]
            }
        })),
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };

    let state = ready_test_state().await;
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let run_id = run.run_id.clone();

    crate::automation_v2::executor::run_automation_v2_run(state.clone(), run).await;
    let first = state
        .get_automation_v2_run(&run_id)
        .await
        .expect("first run");
    assert_eq!(
        first.checkpoint.node_attempts.get("collect_inputs"),
        Some(&1)
    );

    state
        .update_automation_v2_run(&run_id, |row| {
            row.status = AutomationRunStatus::Queued;
            row.detail = Some("requeue collect_inputs".to_string());
            row.resume_reason = Some("requeue collect_inputs".to_string());
            row.stop_kind = None;
            row.stop_reason = None;
            row.pause_reason = None;
            row.checkpoint.awaiting_gate = None;
            row.checkpoint.node_outputs.remove("collect_inputs");
            row.checkpoint
                .completed_nodes
                .retain(|node_id| node_id != "collect_inputs");
            row.checkpoint
                .blocked_nodes
                .retain(|node_id| node_id != "collect_inputs");
            if !row
                .checkpoint
                .pending_nodes
                .iter()
                .any(|node_id| node_id == "collect_inputs")
            {
                row.checkpoint
                    .pending_nodes
                    .push("collect_inputs".to_string());
            }
            if row
                .checkpoint
                .last_failure
                .as_ref()
                .is_some_and(|failure| failure.node_id == "collect_inputs")
            {
                row.checkpoint.last_failure = None;
            }
        })
        .await
        .expect("requeue run");

    let rerun = state.get_automation_v2_run(&run_id).await.expect("rerun");
    crate::automation_v2::executor::run_automation_v2_run(state.clone(), rerun).await;
    let second = state
        .get_automation_v2_run(&run_id)
        .await
        .expect("second run");
    assert_eq!(
        second.checkpoint.node_attempts.get("collect_inputs"),
        Some(&2)
    );
    assert!(second
        .checkpoint
        .node_outputs
        .contains_key("collect_inputs"));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn automation_run_requires_stored_runtime_context_partition_at_startup() {
    let automation = AutomationV2Spec {
        automation_id: "auto-runtime-context-test".to_string(),
        name: "Runtime Context Test".to_string(),
        description: None,
        status: AutomationV2Status::Active,
        schedule: AutomationV2Schedule {
            schedule_type: AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 1,
        updated_at_ms: 1,
        creator_id: "test".to_string(),
        workspace_root: Some(".".to_string()),
        metadata: Some(json!({
            "context_materialization": {
                "routines": [
                    {
                        "routine_id": "collect_inputs",
                        "visible_context_objects": [],
                        "step_context_bindings": []
                    }
                ]
            }
        })),
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let state = ready_test_state().await;
    state
        .put_automation_v2(automation.clone())
        .await
        .expect("store automation");
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    state
        .update_automation_v2_run(&run.run_id, |row| {
            row.runtime_context = None;
        })
        .await
        .expect("clear runtime context");
    let stored_before_clear = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("stored run before clear");
    assert!(state
        .automation_v2_runtime_context(&stored_before_clear)
        .is_some());
    let stored_run = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("stored run");
    assert!(state.automation_v2_runtime_context(&stored_run).is_some());

    crate::automation_v2::executor::run_automation_v2_run(state.clone(), stored_run).await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Failed);
    assert_eq!(
        persisted.detail.as_deref(),
        Some("runtime context partition missing for automation run")
    );
}

#[tokio::test]
async fn automation_run_rejects_invalid_activation_validation_snapshot() {
    let automation = AutomationV2Spec {
        automation_id: "auto-activation-validation-test".to_string(),
        name: "Activation Validation Test".to_string(),
        description: None,
        status: AutomationV2Status::Active,
        schedule: AutomationV2Schedule {
            schedule_type: AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 1,
        updated_at_ms: 1,
        creator_id: "test".to_string(),
        workspace_root: Some(".".to_string()),
        metadata: Some(json!({
            "context_materialization": {
                "routines": [
                    {
                        "routine_id": "collect_inputs",
                        "visible_context_objects": [],
                        "step_context_bindings": []
                    }
                ]
            },
            "plan_package_validation": {
                "ready_for_apply": false,
                "ready_for_activation": false,
                "blocker_count": 1,
                "warning_count": 0,
                "validation_state": {},
                "issues": [
                    {
                        "code": "cross_routine_scope_overlap",
                        "severity": "error",
                        "path": "routines[0]",
                        "message": "scope leak",
                        "blocking": true
                    }
                ]
            }
        })),
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let state = ready_test_state().await;
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let run_id = run.run_id.clone();

    crate::automation_v2::executor::run_automation_v2_run(state.clone(), run).await;

    let persisted = state
        .get_automation_v2_run(&run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Failed);
    assert_eq!(
        persisted.detail.as_deref(),
        Some("plan package not ready for activation: scope leak (cross_routine_scope_overlap)")
    );
}

#[tokio::test]
async fn automation_v2_approved_plan_materialization_is_recovered_from_snapshot() {
    let automation = AutomationV2Spec {
        automation_id: "auto-approved-plan-test".to_string(),
        name: "Approved Plan Test".to_string(),
        description: None,
        status: AutomationV2Status::Active,
        schedule: AutomationV2Schedule {
            schedule_type: AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 1,
        updated_at_ms: 1,
        creator_id: "test".to_string(),
        workspace_root: Some(".".to_string()),
        metadata: Some(json!({
            "plan_package_bundle": {
                "scope_snapshot": {
                    "plan_id": "plan-approved-1",
                    "plan_revision": 4,
                    "context_objects": [
                        {
                            "context_object_id": "ctx:plan:goal",
                            "name": "Plan goal",
                            "kind": "mission_goal",
                            "scope": "mission",
                            "owner_routine_id": "routine_a",
                            "declared_consumers": ["routine_a"],
                            "data_scope_refs": ["mission.goal"],
                            "validation_status": "pending",
                            "provenance": {
                                "plan_id": "plan-approved-1",
                                "routine_id": "routine_a"
                            },
                            "summary": "Plan goal"
                        }
                    ],
                    "credential_envelopes": []
                }
            },
            "approved_plan_materialization": {
                "plan_id": "plan-approved-1",
                "plan_revision": 4,
                "lifecycle_state": "approved",
                "routine_count": 1,
                "step_count": 1,
                "context_object_count": 1,
                "routines": [
                    {
                        "routine_id": "routine_a",
                        "step_ids": ["step_a"],
                        "visible_context_object_ids": ["ctx:plan:goal"],
                        "step_context_bindings": [
                            {
                                "step_id": "step_a",
                                "context_reads": ["ctx:plan:goal"],
                                "context_writes": []
                            }
                        ]
                    }
                ]
            }
        })),
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };

    let state = ready_test_state().await;
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let runtime_context = state
        .automation_v2_runtime_context(&run)
        .expect("runtime context from approved plan");
    let snapshot = state
        .automation_v2_approved_plan_materialization(&run)
        .expect("approved plan materialization");
    assert_eq!(snapshot.plan_id, "plan-approved-1");
    assert_eq!(snapshot.plan_revision, 4);
    assert_eq!(snapshot.routine_count, 1);
    assert_eq!(snapshot.step_count, 1);
    assert_eq!(runtime_context.routines.len(), 1);
    assert_eq!(
        runtime_context.routines[0].visible_context_objects[0].context_object_id,
        "ctx:plan:goal"
    );
    assert_eq!(
        runtime_context.routines[0].step_context_bindings[0].step_id,
        "step_a"
    );
    assert_eq!(
        automation
            .approved_plan_materialization()
            .as_ref()
            .map(|materialization| materialization.plan_id.as_str()),
        Some("plan-approved-1")
    );
}

#[test]
fn first_attempt_structured_json_prompt_without_output_path_requires_handoff_even_without_enforcement(
) {
    let automation = AutomationV2Spec {
        automation_id: "automation-structured-defaults".to_string(),
        name: "Structured Handoff Defaults".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: crate::AutomationFlowSpec { nodes: Vec::new() },
        execution: crate::AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "discover".to_string(),
        agent_id: "research-discover".to_string(),
        objective: "Enumerate sources".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "prompt": "Enumerate the workspace and identify source files."
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "research-discover".to_string(),
        template_id: None,
        display_name: "Research Discover".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["glob".to_string(), "read".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-structured-defaults",
        &node,
        1,
        &agent,
        &[],
        &["glob".to_string(), "read".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("Structured Handoff Expectation"));
    assert!(prompt.contains("`status` set to `completed`"));
    assert!(prompt.contains(
            "Do not claim success unless the required structured handoff was actually returned in the final response."
        ));
}

#[test]
fn research_required_next_tool_actions_summarize_missing_reads_and_websearch() {
    let requested_tools = vec![
        json!("glob"),
        json!("read"),
        json!("websearch"),
        json!("write"),
    ];
    let executed_tools = vec![json!("glob"), json!("write")];
    let unmet_requirements = vec![
        "no_concrete_reads".to_string(),
        "missing_successful_web_research".to_string(),
        "web_sources_reviewed_missing".to_string(),
        "relevant_files_not_reviewed_or_skipped".to_string(),
    ];
    let unreviewed_relevant_paths = vec![
        "docs/pricing.md".to_string(),
        "docs/customers.md".to_string(),
    ];

    let actions = research_required_next_tool_actions(
        &requested_tools,
        &executed_tools,
        true,
        &unmet_requirements,
        &unreviewed_relevant_paths,
        None,
    );

    assert!(actions
        .iter()
        .any(|value| value.contains("docs/pricing.md, docs/customers.md")));
    assert!(actions
        .iter()
        .any(|value| value.contains("Use `websearch` successfully")));
    assert!(actions
        .iter()
        .any(|value| value.contains("Files not reviewed")));
}

#[test]
fn research_required_next_tool_actions_surface_websearch_authorization() {
    let requested_tools = vec![
        json!("glob"),
        json!("read"),
        json!("websearch"),
        json!("write"),
    ];
    let executed_tools = vec![json!("glob"), json!("websearch")];
    let unmet_requirements = vec![
        "no_concrete_reads".to_string(),
        "missing_successful_web_research".to_string(),
        "web_sources_reviewed_missing".to_string(),
    ];

    let actions = research_required_next_tool_actions(
        &requested_tools,
        &executed_tools,
        true,
        &unmet_requirements,
        &Vec::new(),
        Some("web research authorization required"),
    );

    assert!(actions
        .iter()
        .any(|value| value.contains("Skip `websearch` for this run")));
}

#[test]
fn research_required_next_tool_actions_surface_generic_websearch_unavailability() {
    let requested_tools = vec![
        json!("glob"),
        json!("read"),
        json!("websearch"),
        json!("write"),
    ];
    let executed_tools = vec![json!("glob"), json!("websearch")];
    let unmet_requirements = vec![
        "no_concrete_reads".to_string(),
        "missing_successful_web_research".to_string(),
        "web_sources_reviewed_missing".to_string(),
    ];

    let actions = research_required_next_tool_actions(
        &requested_tools,
        &executed_tools,
        true,
        &unmet_requirements,
        &Vec::new(),
        Some("web research unavailable"),
    );

    assert!(actions
        .iter()
        .any(|value| value.contains("external research is unavailable")));
}

#[test]
fn summarize_automation_tool_activity_recovers_tools_from_synthetic_summary() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(Some("synthetic summary".to_string()), None);
    session.messages.push(tandem_types::Message::new(
            MessageRole::Assistant,
            vec![MessagePart::Text {
                text: "I completed project analysis steps using tools, but the model returned no final narrative text.\n\nTool result summary:\nTool `glob` result:\n/home/user123/marketing-tandem/marketing-brief.md\nTool `websearch` result:\nAuthorization required for `websearch`.\nThis integration requires authorization before this action can run.\n\nAuthorize here: https://dashboard.exa.ai/api-keys".to_string(),
            }],
        ));

    let telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );

    assert_eq!(
        telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["glob", "websearch"])
    );
    assert_eq!(
        telemetry
            .get("workspace_inspection_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        telemetry.get("web_research_used").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        telemetry
            .get("latest_web_research_failure")
            .and_then(Value::as_str),
        Some("web research authorization required")
    );
}

#[test]
fn summarize_automation_tool_activity_counts_auth_failed_websearch_as_attempted() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(Some("auth failed websearch".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "websearch".to_string(),
            args: json!({"query":"tandem competitors"}),
            result: None,
            error: Some("Authorization required for `websearch`.".to_string()),
        }],
    ));

    let telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );

    assert_eq!(
        telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["websearch"])
    );
    assert_eq!(
        telemetry.get("web_research_used").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        telemetry
            .get("latest_web_research_failure")
            .and_then(Value::as_str),
        Some("web research authorization required")
    );
}

#[test]
fn summarize_automation_tool_activity_treats_backend_unavailable_websearch_as_unavailable() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(Some("backend unavailable websearch".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "websearch".to_string(),
            args: json!({"query":"tandem competitors"}),
            result: Some(json!({
                "output": "Web search is currently unavailable for `websearch`.",
                "metadata": { "error": "backend_unavailable" }
            })),
            error: None,
        }],
    ));

    let telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );

    assert_eq!(
        telemetry
            .get("latest_web_research_failure")
            .and_then(Value::as_str),
        Some("web research unavailable")
    );
    assert_eq!(
        telemetry
            .get("web_research_succeeded")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn summarize_automation_tool_activity_treats_partial_websearch_with_results_as_success() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(Some("partial websearch".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "websearch".to_string(),
            args: json!({"query":"autonomous AI agentic workflows 2025"}),
            result: Some(json!({
                "output": serde_json::to_string(&json!({
                    "query": "autonomous AI agentic workflows 2025",
                    "result_count": 2,
                    "partial": true,
                    "results": [
                        {"title": "One", "url": "https://example.com/1"},
                        {"title": "Two", "url": "https://example.com/2"}
                    ]
                })).expect("json output"),
                "metadata": {
                    "count": 2,
                    "error": "rate_limited",
                    "partial": true
                }
            })),
            error: None,
        }],
    ));

    let telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );

    assert_eq!(
        telemetry
            .get("web_research_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        telemetry
            .get("latest_web_research_failure")
            .and_then(Value::as_str),
        None
    );
}

#[test]
fn summarize_automation_tool_activity_treats_runtime_websearch_string_result_as_success() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(Some("runtime websearch".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "websearch".to_string(),
            args: json!({"query":"autonomous AI agentic workflows 2024 2025"}),
            result: Some(json!(serde_json::to_string_pretty(&json!({
                "attempted_backends": ["brave"],
                "backend": "brave",
                "configured_backend": "brave",
                "partial": false,
                "query": "autonomous AI agentic workflows 2024 2025",
                "result_count": 2,
                "results": [
                    {
                        "title": "AI Agents in 2025: Expectations vs. Reality | IBM",
                        "url": "https://www.ibm.com/think/insights/ai-agents-2025-expectations-vs-reality"
                    },
                    {
                        "title": "Agentic AI strategy | Deloitte Insights",
                        "url": "https://www.deloitte.com/us/en/insights/topics/technology-management/tech-trends/2026/agentic-ai-strategy.html"
                    }
                ]
            }))
            .expect("json output"))),
            error: None,
        }],
    ));

    let telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );

    assert_eq!(
        telemetry.get("web_research_used").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        telemetry
            .get("web_research_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        telemetry
            .get("latest_web_research_failure")
            .and_then(Value::as_str),
        None
    );
}

#[test]
fn automation_prompt_preflight_marks_warning_for_large_prompt() {
    let prompt = "x".repeat(20_000);
    let preflight = crate::app::state::automation::build_automation_prompt_preflight(
        &prompt,
        &["glob".to_string(), "read".to_string(), "write".to_string()],
        &[json!({
            "name": "write",
            "description": "write a file",
            "input_schema": {"type": "object"}
        })],
        "artifact_write",
        &json!({
            "required_capabilities": ["artifact_write"],
            "resolved": {
                "artifact_write": {
                    "status": "resolved",
                    "offered_tools": ["write"],
                    "available_tools": ["write"]
                }
            },
            "missing_capabilities": []
        }),
        "standard",
        false,
    );

    assert_eq!(
        preflight.get("budget_status").and_then(Value::as_str),
        Some("high")
    );
    assert_eq!(
        preflight.get("degraded_prompt").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn build_automation_attempt_evidence_captures_runtime_websearch_success() {
    let node = AutomationFlowNode {
        node_id: "research_sources".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "collect_inputs".to_string(),
            alias: "research_brief".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "citations".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
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
                "output_path": ".tandem/artifacts/research-sources.json"
            }
        })),
    };
    let mut session = Session::new(Some("research".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"file_path": ".tandem/artifacts/collect-inputs.json"}),
                result: Some(json!("{\"topic\":\"autonomous AI agentic workflows\"}")),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "websearch".to_string(),
                args: json!({"query":"autonomous AI agentic workflows 2024 2025"}),
                result: Some(json!(serde_json::to_string_pretty(&json!({
                    "backend": "brave",
                    "result_count": 2,
                    "partial": false,
                    "results": [
                        {"title": "IBM", "url": "https://example.com/ibm"},
                        {"title": "Deloitte", "url": "https://example.com/deloitte"}
                    ]
                }))
                .expect("json output"))),
                error: None,
            },
        ],
    ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );
    let preflight = crate::app::state::automation::build_automation_prompt_preflight(
        "Research prompt",
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
        &[
            json!({"name":"websearch"}),
            json!({"name":"read"}),
            json!({"name":"write"}),
        ],
        "artifact_write",
        &json!({
            "required_capabilities": ["workspace_read", "workspace_discover", "artifact_write", "web_research"],
            "resolved": {},
            "missing_capabilities": []
        }),
        "standard",
        false,
    );
    let attempt_evidence = build_automation_attempt_evidence(
        &node,
        1,
        &session,
        &session.id,
        ".",
        &tool_telemetry,
        &preflight,
        &json!({
            "required_capabilities": ["web_research"],
            "resolved": {},
            "missing_capabilities": []
        }),
        None,
    );

    let web_status = attempt_evidence
        .get("evidence")
        .and_then(Value::as_object)
        .and_then(|value| value.get("web_research"))
        .and_then(Value::as_object)
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str);
    assert_eq!(web_status, Some("succeeded"));

    let succeeded_tools = attempt_evidence
        .get("tool_execution")
        .and_then(Value::as_object)
        .and_then(|value| value.get("succeeded_tools"))
        .and_then(Value::as_array);
    assert_eq!(
        succeeded_tools
            .is_some_and(|rows| rows.iter().any(|value| value.as_str() == Some("websearch"))),
        true
    );
}

#[test]
fn detect_automation_blocker_category_prefers_delivery_category_from_canonical_evidence() {
    let node = AutomationFlowNode {
        node_id: "notify_user".to_string(),
        agent_id: "agent-committer".to_string(),
        objective: "Send the report by email.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": "evan@frumu.ai",
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    };
    let tool_telemetry = json!({
        "executed_tools": ["read"],
        "preflight": {"budget_status": "ok"},
        "attempt_evidence": {
            "capability_resolution": {
                "missing_capabilities": []
            },
            "delivery": {
                "status": "not_attempted"
            }
        }
    });

    assert_eq!(
        detect_automation_blocker_category(
            &node,
            "blocked",
            Some("email delivery to `evan@frumu.ai` was requested but no email draft/send tool executed"),
            &tool_telemetry,
            None,
        )
        .as_deref(),
        Some("delivery_not_executed")
    );
}

#[test]
fn report_generation_objective_does_not_imply_email_delivery_execution() {
    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Draft the report in simple HTML suitable for email body delivery.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
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
                "output_path": ".tandem/artifacts/generate-report.html"
            }
        })),
    };

    assert!(!crate::app::state::automation::automation_node_requires_email_delivery(&node));
}

#[test]
fn execute_goal_objective_with_gmail_draft_or_send_requires_email_delivery() {
    let node = AutomationFlowNode {
        node_id: "execute_goal".to_string(),
        agent_id: "operator".to_string(),
        objective: "Create a Gmail draft or send the final HTML summary email to evan@frumu.ai if mail tools are available.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };

    assert!(crate::app::state::automation::automation_node_requires_email_delivery(&node));
}

#[test]
fn email_delivery_status_uses_recipient_from_objective_when_metadata_missing() {
    let node = AutomationFlowNode {
        node_id: "execute_goal".to_string(),
        agent_id: "operator".to_string(),
        objective: "Create a Gmail draft or send the final HTML summary email to evan@frumu.ai if mail tools are available.".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: None,
    };

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "A Gmail draft has been created.\n\n{\"status\":\"completed\",\"approved\":true}",
            None,
            &json!({
                "requested_tools": ["glob", "read", "mcp_list"],
                "executed_tools": ["read", "glob", "mcp_list"],
                "tool_call_counts": {"read": 1, "glob": 1, "mcp_list": 1},
                "workspace_inspection_used": true,
                "email_delivery_attempted": false,
                "email_delivery_succeeded": false,
                "latest_email_delivery_failure": null,
                "capability_resolution": {
                    "email_tool_diagnostics": {
                        "available_tools": ["mcp.composio_1.gmail_send_email", "mcp.composio_1.gmail_create_email_draft"],
                        "offered_tools": ["mcp.composio_1.gmail_send_email", "mcp.composio_1.gmail_create_email_draft"],
                        "available_send_tools": ["mcp.composio_1.gmail_send_email"],
                        "offered_send_tools": ["mcp.composio_1.gmail_send_email"],
                        "available_draft_tools": ["mcp.composio_1.gmail_create_email_draft"],
                        "offered_draft_tools": ["mcp.composio_1.gmail_create_email_draft"]
                    }
                }
            }),
            None,
        );

    assert_eq!(status, "blocked");
    assert_eq!(
        reason.as_deref(),
        Some(
            "email delivery to `evan@frumu.ai` was requested but no email draft/send tool executed"
        )
    );
    assert_eq!(approved, Some(true));
}

#[test]
fn research_workflow_failure_kind_detects_missing_citations() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let artifact_validation = json!({
        "semantic_block_reason": "research completed without citation-backed claims",
        "unmet_requirements": ["citations_missing", "web_sources_reviewed_missing"],
        "verification": {
            "verification_failed": false
        }
    });

    assert_eq!(
        detect_automation_node_failure_kind(
            &node,
            "blocked",
            None,
            Some("research completed without citation-backed claims"),
            Some(&artifact_validation),
        )
        .as_deref(),
        Some("research_citations_missing")
    );
    assert_eq!(
        detect_automation_node_phase(&node, "blocked", Some(&artifact_validation)),
        "research_validation"
    );
}

#[test]
fn research_workflow_defaults_to_warning_without_strict_source_coverage() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "allow_preexisting_output_reuse": true
            }
        })),
    };
    let artifact_validation = json!({
        "unmet_requirements": ["no_concrete_reads", "citations_missing", "missing_successful_web_research"],
        "verification": {
            "verification_failed": false
        }
    });

    assert_eq!(
        detect_automation_node_failure_kind(
            &node,
            "completed",
            None,
            None,
            Some(&artifact_validation)
        ),
        None
    );
    assert_eq!(
        detect_automation_node_phase(&node, "completed", Some(&artifact_validation)),
        "completed"
    );
}

#[test]
fn validator_summary_reports_repair_attempt_state() {
    let artifact_validation = json!({
        "semantic_block_reason": "research completed without citation-backed claims",
        "unmet_requirements": ["citations_missing"],
        "repair_attempted": true,
        "repair_attempt": 2,
        "repair_attempts_remaining": 0,
        "repair_succeeded": false,
        "repair_exhausted": true,
    });
    let summary = build_automation_validator_summary(
        crate::AutomationOutputValidatorKind::ResearchBrief,
        "blocked",
        Some("research completed without citation-backed claims"),
        Some(&artifact_validation),
    );
    assert!(summary.repair_attempted);
    assert_eq!(summary.repair_attempt, 2);
    assert_eq!(summary.repair_attempts_remaining, 0);
    assert!(!summary.repair_succeeded);
    assert!(summary.repair_exhausted);
}

#[test]
fn artifact_validation_uses_structured_repair_exhaustion_state_from_session_text() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-repair-state-test-{}", now_ms()));
    std::fs::create_dir_all(workspace_root.join("inputs")).expect("create workspace");
    std::fs::write(workspace_root.join("inputs/questions.md"), "Question")
        .expect("seed input file");

    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let mut session = Session::new(Some("research repair exhausted".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path":"marketing-brief.md",
                "content":"# Marketing Brief\n\n## Findings\nBlocked draft without citations.\n"
            }),
            result: Some(json!({"output":"written"})),
            error: None,
        }],
    ));
    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );
    let session_text = r#"TOOL_MODE_REQUIRED_NOT_SATISFIED: PREWRITE_REQUIREMENTS_EXHAUSTED

{"status":"blocked","reason":"prewrite requirements exhausted before final artifact validation","failureCode":"PREWRITE_REQUIREMENTS_EXHAUSTED","repairAttempt":2,"repairAttemptsRemaining":0,"repairExhausted":true,"unmetRequirements":["concrete_read_required","successful_web_research_required"]}"#;
    let (_accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root"),
        session_text,
        &tool_telemetry,
        None,
        Some((
            "marketing-brief.md".to_string(),
            "# Marketing Brief\n\n## Findings\nBlocked draft without citations.\n".to_string(),
        )),
        &std::collections::BTreeSet::new(),
    );
    assert!(rejected.is_some());
    assert_eq!(
        metadata.get("repair_attempt").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        metadata
            .get("repair_attempts_remaining")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        metadata.get("repair_exhausted").and_then(Value::as_bool),
        Some(true)
    );
    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_artifact_validation_requires_citations_and_web_sources_reviewed() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-research-citation-test-{}", now_ms()));
    std::fs::create_dir_all(workspace_root.join("inputs")).expect("create workspace");
    std::fs::write(workspace_root.join("inputs/questions.md"), "Question")
        .expect("seed input file");

    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let mut session = Session::new(Some("research citations".to_string()), None);
    session.messages.push(tandem_types::Message::new(
            MessageRole::Assistant,
            vec![
                MessagePart::ToolInvocation {
                    tool: "read".to_string(),
                    args: json!({"path":"inputs/questions.md"}),
                    result: Some(json!({"output":"Question"})),
                    error: None,
                },
                MessagePart::ToolInvocation {
                    tool: "websearch".to_string(),
                    args: json!({"query":"market trends"}),
                    result: Some(json!({"output":"Search results found"})),
                    error: None,
                },
                MessagePart::ToolInvocation {
                    tool: "write".to_string(),
                    args: json!({
                        "path":"marketing-brief.md",
                        "content":"# Marketing Brief\n\n## Files reviewed\n- inputs/questions.md\n\n## Files not reviewed\n- inputs/references.md: not available in this run.\n\n## Findings\nClaims are summarized here without explicit citations.\n"
                    }),
                    result: Some(json!({"output":"written"})),
                    error: None,
                },
            ],
        ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "read".to_string(),
            "write".to_string(),
            "websearch".to_string(),
        ],
    );
    let (_, artifact_validation, rejected) = validate_automation_artifact_output(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root"),
            "",
            &tool_telemetry,
            None,
            Some((
                "marketing-brief.md".to_string(),
                "# Marketing Brief\n\n## Files reviewed\n- inputs/questions.md\n\n## Findings\nClaims are summarized here without explicit citations.\n".to_string(),
            )),
            &std::collections::BTreeSet::new(),
        );

    assert_eq!(
        rejected.as_deref(),
        Some("research completed without citation-backed claims")
    );
    assert_eq!(
        artifact_validation
            .get("unmet_requirements")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        vec![
            json!("citations_missing"),
            json!("web_sources_reviewed_missing")
        ]
    );
    assert_eq!(
        artifact_validation
            .get("artifact_candidates")
            .and_then(Value::as_array)
            .and_then(|rows| rows.first())
            .and_then(|value| value.get("citation_count"))
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        artifact_validation
            .get("citation_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        artifact_validation
            .get("web_sources_reviewed_present")
            .and_then(Value::as_bool),
        Some(false)
    );

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn research_citations_validation_accepts_external_research_without_files_reviewed_section() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-research-sources-test-{}", now_ms()));
    std::fs::create_dir_all(workspace_root.join("inputs")).expect("create workspace");
    std::fs::write(workspace_root.join("inputs/questions.md"), "Question")
        .expect("seed input file");

    let node = AutomationFlowNode {
        node_id: "research_sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Research current web sources".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "citations".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Return a citation handoff.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": ".tandem/artifacts/research-sources.json",
                "web_research_expected": true,
                "source_coverage_required": true
            }
        })),
    };
    let mut session = Session::new(Some("research sources".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"inputs/questions.md"}),
                result: Some(json!({"output":"Question"})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "websearch".to_string(),
                args: json!({"query":"autonomous AI agentic workflows 2024 2025"}),
                result: Some(json!({"output":"Search results found"})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path":".tandem/artifacts/research-sources.json",
                    "content":"# Research Sources\n\n## Summary\nCurrent external research was gathered successfully.\n\n## Citations\n1. AI Agents in 2025: Expectations vs. Reality | IBM. Source note: https://www.ibm.com/think/insights/ai-agents-2025-expectations-vs-reality\n2. Agentic AI, explained | MIT Sloan. Source note: https://mitsloan.mit.edu/ideas-made-to-matter/agentic-ai-explained\n\n## Web sources reviewed\n- https://www.ibm.com/think/insights/ai-agents-2025-expectations-vs-reality\n- https://mitsloan.mit.edu/ideas-made-to-matter/agentic-ai-explained\n"
                }),
                result: Some(json!({"output":"written"})),
                error: None,
            },
        ],
    ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "read".to_string(),
            "write".to_string(),
            "websearch".to_string(),
        ],
    );
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root"),
        "",
        &tool_telemetry,
        None,
        Some((
            ".tandem/artifacts/research-sources.json".to_string(),
            "# Research Sources\n\n## Summary\nCurrent external research was gathered successfully.\n\n## Citations\n1. AI Agents in 2025: Expectations vs. Reality | IBM. Source note: https://www.ibm.com/think/insights/ai-agents-2025-expectations-vs-reality\n2. Agentic AI, explained | MIT Sloan. Source note: https://mitsloan.mit.edu/ideas-made-to-matter/agentic-ai-explained\n\n## Web sources reviewed\n- https://www.ibm.com/think/insights/ai-agents-2025-expectations-vs-reality\n- https://mitsloan.mit.edu/ideas-made-to-matter/agentic-ai-explained\n".to_string(),
        )),
        &std::collections::BTreeSet::new(),
    );

    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert!(!artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("files_reviewed_missing"))));
    assert!(!artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("files_reviewed_not_backed_by_read"))));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn marketing_template_automation_migrates_to_split_research_flow() {
    let mut automation = AutomationV2Spec {
        automation_id: "automation-v2-test".to_string(),
        name: "Marketing Content Pipeline".to_string(),
        description: None,
        status: AutomationV2Status::Active,
        schedule: AutomationV2Schedule {
            schedule_type: AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: vec![
            AutomationAgentProfile {
                agent_id: "research".to_string(),
                template_id: None,
                display_name: "Research".to_string(),
                avatar_url: None,
                model_policy: None,
                skills: vec!["analysis".to_string()],
                tool_policy: AutomationAgentToolPolicy {
                    allowlist: vec![
                        "read".to_string(),
                        "write".to_string(),
                        "websearch".to_string(),
                        "glob".to_string(),
                    ],
                    denylist: Vec::new(),
                },
                mcp_policy: AutomationAgentMcpPolicy {
                    allowed_servers: Vec::new(),
                    allowed_tools: None,
                },
                approval_policy: None,
            },
            AutomationAgentProfile {
                agent_id: "copywriter".to_string(),
                template_id: None,
                display_name: "Copywriter".to_string(),
                avatar_url: None,
                model_policy: None,
                skills: Vec::new(),
                tool_policy: AutomationAgentToolPolicy {
                    allowlist: vec!["read".to_string(), "write".to_string()],
                    denylist: Vec::new(),
                },
                mcp_policy: AutomationAgentMcpPolicy {
                    allowed_servers: Vec::new(),
                    allowed_tools: None,
                },
                approval_policy: None,
            },
        ],
        flow: AutomationFlowSpec {
            nodes: vec![
                AutomationFlowNode {
                    node_id: "research-brief".to_string(),
                    agent_id: "research".to_string(),
                    objective: "Legacy research".to_string(),
                    depends_on: Vec::new(),
                    input_refs: Vec::new(),
                    output_contract: Some(AutomationFlowOutputContract {
                        kind: "brief".to_string(),
                        validator: None,
                        enforcement: None,
                        schema: None,
                        summary_guidance: Some("Write `marketing-brief.md`.".to_string()),
                    }),
                    retry_policy: None,
                    timeout_ms: None,
                    stage_kind: None,
                    gate: None,
                    metadata: Some(json!({
                        "builder": {
                            "title": "Research Brief",
                            "role": "watcher",
                            "output_path": "marketing-brief.md",
                            "prompt": "Legacy one-shot research prompt"
                        },
                        "studio": {
                            "output_path": "marketing-brief.md"
                        }
                    })),
                },
                AutomationFlowNode {
                    node_id: "draft-copy".to_string(),
                    agent_id: "copywriter".to_string(),
                    objective: "Draft copy".to_string(),
                    depends_on: vec!["research-brief".to_string()],
                    input_refs: vec![AutomationFlowInputRef {
                        from_step_id: "research-brief".to_string(),
                        alias: "marketing_brief".to_string(),
                    }],
                    output_contract: Some(AutomationFlowOutputContract {
                        kind: "draft".to_string(),
                        validator: None,
                        enforcement: None,
                        schema: None,
                        summary_guidance: None,
                    }),
                    retry_policy: None,
                    timeout_ms: None,
                    stage_kind: None,
                    gate: None,
                    metadata: None,
                },
            ],
        },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 1,
        updated_at_ms: 1,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp/workspace".to_string()),
        metadata: Some(json!({
            "studio": {
                "template_id": "marketing-content-pipeline",
                "version": 1,
                "agent_drafts": [{"agentId":"research"}],
                "node_drafts": [{"nodeId":"research-brief"}]
            }
        })),
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };

    assert!(migrate_bundled_studio_research_split_automation(
        &mut automation
    ));
    assert!(automation
        .flow
        .nodes
        .iter()
        .any(|node| node.node_id == "research-discover-sources"));
    assert!(automation
        .flow
        .nodes
        .iter()
        .any(|node| node.node_id == "research-local-sources"));
    assert!(automation
        .flow
        .nodes
        .iter()
        .any(|node| node.node_id == "research-external-research"));
    let discover_node = automation
        .flow
        .nodes
        .iter()
        .find(|node| node.node_id == "research-discover-sources")
        .expect("discover node present");
    let discover_enforcement = discover_node
        .output_contract
        .as_ref()
        .and_then(|contract| contract.enforcement.as_ref())
        .expect("discover enforcement");
    assert!(discover_enforcement
        .required_tools
        .iter()
        .any(|tool| tool == "read"));
    assert!(discover_enforcement
        .prewrite_gates
        .iter()
        .any(|gate| gate == "workspace_inspection"));
    assert!(discover_enforcement
        .prewrite_gates
        .iter()
        .any(|gate| gate == "concrete_reads"));
    let final_node = automation
        .flow
        .nodes
        .iter()
        .find(|node| node.node_id == "research-brief")
        .expect("final node preserved");
    assert_eq!(
        automation_node_research_stage(final_node).as_deref(),
        Some("research_finalize")
    );
    assert_eq!(final_node.depends_on.len(), 3);
    assert!(automation
        .agents
        .iter()
        .any(|agent| agent.agent_id == "research-discover"));
    assert!(automation
        .agents
        .iter()
        .any(|agent| agent.agent_id == "research-local-sources"));
    assert!(automation
        .agents
        .iter()
        .any(|agent| agent.agent_id == "research-external"));
    let studio = automation
        .metadata
        .as_ref()
        .and_then(|value| value.get("studio"))
        .and_then(Value::as_object)
        .expect("studio metadata");
    assert_eq!(studio.get("version").and_then(Value::as_u64), Some(2));
    assert_eq!(
        studio
            .get("workflow_structure_version")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert!(!studio.contains_key("agent_drafts"));
    assert!(!studio.contains_key("node_drafts"));
}

#[test]
fn research_finalize_validation_accepts_upstream_read_evidence() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-research-finalize-test-{}", now_ms()));
    std::fs::create_dir_all(workspace_root.join("inputs")).expect("create workspace");
    std::fs::write(workspace_root.join("inputs/questions.md"), "Question")
        .expect("seed input file");

    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "research".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: vec![
            "research-discover-sources".to_string(),
            "research-local-sources".to_string(),
            "research-external-research".to_string(),
        ],
        input_refs: vec![
            AutomationFlowInputRef {
                from_step_id: "research-discover-sources".to_string(),
                alias: "source_inventory".to_string(),
            },
            AutomationFlowInputRef {
                from_step_id: "research-local-sources".to_string(),
                alias: "local_source_notes".to_string(),
            },
            AutomationFlowInputRef {
                from_step_id: "research-external-research".to_string(),
                alias: "external_research".to_string(),
            },
        ],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: Some(crate::AutomationOutputEnforcement {
                validation_profile: Some("research_synthesis".to_string()),
                required_tools: Vec::new(),
                required_evidence: vec!["local_source_reads".to_string()],
                required_sections: vec![
                    "files_reviewed".to_string(),
                    "files_not_reviewed".to_string(),
                    "citations".to_string(),
                ],
                prewrite_gates: Vec::new(),
                retry_on_missing: vec![
                    "local_source_reads".to_string(),
                    "files_reviewed".to_string(),
                    "files_not_reviewed".to_string(),
                    "citations".to_string(),
                ],
                terminal_on: Vec::new(),
                repair_budget: Some(5),
                session_text_recovery: None,
            }),
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": "marketing-brief.md",
                "research_stage": "research_finalize",
                "title": "Research Brief",
                "role": "watcher"
            }
        })),
    };

    let mut session = Session::new(Some("research finalize".to_string()), None);
    session.messages.push(tandem_types::Message::new(
            MessageRole::Assistant,
            vec![MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path":"marketing-brief.md",
                    "content":"# Marketing Brief\n\n## Files reviewed\n- inputs/questions.md\n\n## Files not reviewed\n- inputs/extra.md: not needed for this test.\n\n## Proof Points With Citations\n1. Supported claim. Source note: https://example.com/reference\n"
                }),
                result: Some(json!({"output":"written"})),
                error: None,
            }],
        ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &["read".to_string(), "write".to_string()],
    );
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec!["inputs/questions.md".to_string()],
        discovered_relevant_paths: vec!["inputs/questions.md".to_string()],
        web_research_attempted: false,
        web_research_succeeded: false,
        citation_count: 0,
        citations: vec![],
    };
    let (accepted_output, artifact_validation, rejected) =
            validate_automation_artifact_output_with_upstream(
                &node,
                &session,
                workspace_root.to_str().expect("workspace root"),
                None,
                "",
                &tool_telemetry,
                None,
                Some((
                    "marketing-brief.md".to_string(),
                    "# Marketing Brief\n\n## Files reviewed\n- inputs/questions.md\n\n## Files not reviewed\n- inputs/extra.md: not needed for this test.\n\n## Proof Points With Citations\n1. Supported claim. Source note: https://example.com/reference\n".to_string(),
                )),
                &std::collections::BTreeSet::new(),
                Some(&upstream_evidence),
            );

    assert!(accepted_output.is_some(), "{artifact_validation:?}");
    assert!(
        rejected.is_none(),
        "rejected={rejected:?} metadata={artifact_validation:?}"
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("upstream_evidence_applied")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        artifact_validation
            .get("read_paths")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        vec![json!("inputs/questions.md")]
    );

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn validator_summary_tracks_verification_and_repair_state() {
    let artifact_validation = json!({
        "accepted_candidate_source": "session_write",
        "repair_attempted": true,
        "repair_succeeded": true,
        "validation_basis": {
            "authority": "filesystem_and_receipts",
            "current_attempt_output_materialized": true
        },
        "verification": {
            "verification_outcome": "passed"
        }
    });

    let summary = build_automation_validator_summary(
        crate::AutomationOutputValidatorKind::CodePatch,
        "done",
        None,
        Some(&artifact_validation),
    );

    assert_eq!(
        summary.kind,
        crate::AutomationOutputValidatorKind::CodePatch
    );
    assert_eq!(summary.outcome, "passed");
    assert_eq!(
        summary.accepted_candidate_source.as_deref(),
        Some("session_write")
    );
    assert_eq!(summary.verification_outcome.as_deref(), Some("passed"));
    assert_eq!(
        summary
            .validation_basis
            .as_ref()
            .and_then(|value| value.get("authority"))
            .and_then(Value::as_str),
        Some("filesystem_and_receipts")
    );
    assert!(summary.repair_attempted);
    assert!(summary.repair_succeeded);
}

#[test]
fn generic_artifact_validation_blocks_weak_report_markdown() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-editorial-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace_root).expect("workspace dir");
    let node = AutomationFlowNode {
        node_id: "draft-report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Draft the final report".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
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
                "output_path": "report.md"
            }
        })),
    };
    let session = Session::new(
        Some("editorial".to_string()),
        Some(workspace_root.to_string_lossy().to_string()),
    );
    let tool_telemetry = json!({
        "requested_tools": ["write"],
        "executed_tools": ["write"],
        "tool_call_counts": {
            "write": 1
        }
    });
    let (_, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root"),
        "",
        &tool_telemetry,
        None,
        Some(("report.md".to_string(), "# Draft\n\nTODO\n".to_string())),
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(
        rejected.as_deref(),
        Some("editorial artifact is missing expected markdown structure")
    );
    assert_eq!(
        artifact_validation
            .get("unmet_requirements")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        vec![
            json!("editorial_substance_missing"),
            json!("markdown_structure_missing")
        ]
    );
    assert_eq!(
        artifact_validation
            .get("heading_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        artifact_validation
            .get("paragraph_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        detect_automation_node_failure_kind(
            &node,
            "blocked",
            None,
            None,
            Some(&artifact_validation),
        ),
        Some("editorial_quality_failed".to_string())
    );
    assert_eq!(
        detect_automation_node_phase(&node, "blocked", Some(&artifact_validation)),
        "editorial_validation"
    );

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn publish_node_blocks_when_upstream_editorial_validation_failed() {
    let publish = AutomationFlowNode {
        node_id: "publish".to_string(),
        agent_id: "publisher".to_string(),
        objective: "Publish final output".to_string(),
        depends_on: vec!["draft".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "draft".to_string(),
            alias: "draft".to_string(),
        }],
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "role": "publisher"
            }
        })),
    };
    let mut run = test_phase_run(vec!["publish"], vec!["draft"]);
    run.checkpoint.node_outputs.insert(
        "draft".to_string(),
        json!({
            "node_id": "draft",
            "failure_kind": "editorial_quality_failed",
            "phase": "editorial_validation",
            "validator_summary": {
                "unmet_requirements": ["editorial_substance_missing", "markdown_structure_missing"]
            }
        }),
    );

    let reason = automation_publish_editorial_block_reason(&run, &publish).expect("publish block");
    assert!(reason.contains("draft"));
    assert!(reason.contains("editorial"));
}

#[test]
fn report_markdown_blocks_when_rich_upstream_evidence_is_reduced_to_generic_summary() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-upstream-synthesis-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot =
        automation_workspace_root_file_snapshot(workspace_root.to_str().expect("workspace root"));
    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Create the final report".to_string(),
        depends_on: vec!["analyze_findings".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "analyze_findings".to_string(),
            alias: "analysis".to_string(),
        }],
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
                "output_path": "report.md"
            }
        })),
    };
    let session = Session::new(
        Some("thin-final-summary".to_string()),
        Some(workspace_root.to_str().expect("workspace root").to_string()),
    );
    let mut session = session;
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": "report.md",
                "content": "# Strategic Summary\n\nTandem is an engineering agent for local execution.\n\n## Positioning\n\nIt connects human intent to repo changes.\n"
            }),
            result: Some(json!("ok")),
            error: None,
        }],
    ));
    let thin_report = "# Strategic Summary\n\nTandem is an engineering agent for local execution.\n\n## Positioning\n\nIt connects human intent to repo changes.\n".to_string();
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec![
            "README.md".to_string(),
            "docs/product-capabilities.md".to_string(),
        ],
        discovered_relevant_paths: vec![
            "README.md".to_string(),
            "docs/product-capabilities.md".to_string(),
        ],
        web_research_attempted: true,
        web_research_succeeded: true,
        citation_count: 3,
        citations: vec![
            "https://example.com/source-1".to_string(),
            "https://example.com/source-2".to_string(),
            "https://example.com/source-3".to_string(),
        ],
    };

    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root"),
            None,
            "Completed the report.",
            &json!({
                "requested_tools": ["write"],
                "executed_tools": ["write"],
                "tool_call_counts": {
                    "write": 1
                }
            }),
            None,
            Some(("report.md".to_string(), thin_report)),
            &snapshot,
            Some(&upstream_evidence),
        );

    assert!(accepted_output.is_some());
    assert_eq!(
        rejected.as_deref(),
        Some("final artifact does not adequately synthesize the available upstream evidence")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("final artifact does not adequately synthesize the available upstream evidence")
    );
    assert!(artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|items| items
            .iter()
            .any(|value| value.as_str() == Some("upstream_evidence_not_synthesized"))));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn report_markdown_accepts_structured_synthesis_without_inline_citations_when_upstream_is_rich() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-upstream-synthesis-pass-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot =
        automation_workspace_root_file_snapshot(workspace_root.to_str().expect("workspace root"));
    let node = AutomationFlowNode {
        node_id: "analyze_findings".to_string(),
        agent_id: "analyst".to_string(),
        objective: "Synthesize findings into a strategy report".to_string(),
        depends_on: vec!["collect_inputs".to_string(), "research_sources".to_string()],
        input_refs: vec![
            AutomationFlowInputRef {
                from_step_id: "collect_inputs".to_string(),
                alias: "local_grounding".to_string(),
            },
            AutomationFlowInputRef {
                from_step_id: "research_sources".to_string(),
                alias: "external_research".to_string(),
            },
        ],
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
                "output_path": "analyze-findings.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("structured-synthesis".to_string()),
        Some(workspace_root.to_str().expect("workspace root").to_string()),
    );
    let report = "# Strategy Analysis Report\n\n## 1. Executive Summary\nThis analysis synthesizes Tandem's core internal product definitions and external research to refine positioning and strategy. Tandem is positioned as a high-autonomy, agentic engineering engine that solves cognitive load and cross-functional orchestration issues, positioning itself firmly against generic code assistants.\n\n## 2. Product Positioning\n*   **Core Identity:** Tandem by Frumu AI\n*   **Market Category:** Agentic Software Development / IDE-Integrated Engineering Tool\n*   **Key Positioning:** Empowering engineers with high-autonomy, context-accurate AI collaboration embedded directly within their development workflow.\n\n## 3. Target Users & Use-Case Wedges\n*   **Primary Users:** Professional engineers and development teams struggling with high cognitive load.\n*   **Use-Case Wedge:** Utilizing workspace-aware code analysis and automated task execution to bridge the gap between documentation and implementation.\n\n## 4. Investor Narrative & Competitive Outlook\n*   **Competitive Standing:** Tandem differentiates itself by being a full-context engineering engine rather than a simple chatbot.\n*   **Narrative Hook:** Stop context-switching and let Tandem handle tooling and documentation synthesis overhead.\n\n## 5. Risks & Proof Gaps\n*   **Market Risk:** Strong competition from well-capitalized code-assistant vendors.\n*   **Proof Gaps:** Need stronger empirical time-saved and throughput metrics.\n\n## 6. Execution Summary\nThe immediate priority is to prove the agentic value proposition with high-utility automation flows such as multi-file updates and refactors.\n\n---\n*Source Verification: Based on `.tandem/artifacts/collect-inputs.json` and `.tandem/artifacts/research-sources.json`.*\n".to_string();
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": "analyze-findings.md",
                "content": report
            }),
            result: Some(json!("ok")),
            error: None,
        }],
    ));
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
        ],
        discovered_relevant_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            "README.md".to_string(),
        ],
        web_research_attempted: true,
        web_research_succeeded: true,
        citation_count: 3,
        citations: vec![
            "https://example.com/1".to_string(),
            "https://example.com/2".to_string(),
            "https://example.com/3".to_string(),
        ],
    };

    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root"),
            None,
            "Completed the report.",
            &json!({
                "requested_tools": ["read", "write"],
                "executed_tools": ["read", "write"],
                "tool_call_counts": {
                    "read": 2,
                    "write": 1
                }
            }),
            None,
            Some(("analyze-findings.md".to_string(), report)),
            &snapshot,
            Some(&upstream_evidence),
        );

    assert!(accepted_output.is_some());
    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        None
    );
    assert!(!artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|items| items
            .iter()
            .any(|value| value.as_str() == Some("upstream_evidence_not_synthesized"))));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn report_markdown_legacy_metadata_is_forced_to_strict_without_emergency_rollback() {
    with_legacy_quality_rollback_enabled(false, || {
        let workspace_root = std::env::temp_dir().join(format!(
            "tandem-report-forced-strict-quality-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_root).expect("create workspace");
        let snapshot = automation_workspace_root_file_snapshot(
            workspace_root.to_str().expect("workspace root"),
        );
        let node = AutomationFlowNode {
            node_id: "generate_report".to_string(),
            agent_id: "writer".to_string(),
            objective: "Create the final report".to_string(),
            depends_on: vec!["analyze_findings".to_string()],
            input_refs: vec![AutomationFlowInputRef {
                from_step_id: "analyze_findings".to_string(),
                alias: "analysis".to_string(),
            }],
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
                "quality_mode": "legacy",
                "builder": {
                    "output_path": "generate-report.md"
                }
            })),
        };
        let mut session = Session::new(
            Some("legacy-quality-mode".to_string()),
            Some(workspace_root.to_str().expect("workspace root").to_string()),
        );
        let generic_report = "# Summary\n\nPlaceholder update.\n".to_string();
        session.messages.push(tandem_types::Message::new(
            MessageRole::Assistant,
            vec![MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "generate-report.md",
                    "content": generic_report
                }),
                result: Some(json!("ok")),
                error: None,
            }],
        ));
        let upstream_evidence = AutomationUpstreamEvidence {
            read_paths: vec![
                ".tandem/artifacts/collect-inputs.json".to_string(),
                ".tandem/artifacts/research-sources.json".to_string(),
            ],
            discovered_relevant_paths: vec![
                ".tandem/artifacts/collect-inputs.json".to_string(),
                ".tandem/artifacts/research-sources.json".to_string(),
            ],
            web_research_attempted: true,
            web_research_succeeded: true,
            citation_count: 3,
            citations: vec![
                "https://example.com/legacy-1".to_string(),
                "https://example.com/legacy-2".to_string(),
                "https://example.com/legacy-3".to_string(),
            ],
        };

        let (_accepted_output, artifact_validation, rejected) =
            validate_automation_artifact_output_with_upstream(
                &node,
                &session,
                workspace_root.to_str().expect("workspace root"),
                None,
                "Completed the report.",
                &json!({
                    "requested_tools": ["read", "write"],
                    "executed_tools": ["read", "write"],
                    "tool_call_counts": {
                        "read": 2,
                        "write": 1
                    }
                }),
                None,
                Some(("generate-report.md".to_string(), generic_report)),
                &snapshot,
                Some(&upstream_evidence),
            );

        assert!(rejected.is_some());
        assert!(artifact_validation
            .get("unmet_requirements")
            .and_then(Value::as_array)
            .is_some_and(|items| items
                .iter()
                .any(|value| value.as_str() == Some("upstream_evidence_not_synthesized"))));
        assert_eq!(
            artifact_validation
                .get("validation_basis")
                .and_then(|value| value.get("quality_mode"))
                .and_then(Value::as_str),
            Some("strict_research_v1")
        );
        assert_eq!(
            artifact_validation
                .get("validation_basis")
                .and_then(|value| value.get("requested_quality_mode"))
                .and_then(Value::as_str),
            Some("legacy")
        );
        assert_eq!(
            artifact_validation
                .get("validation_basis")
                .and_then(|value| value.get("legacy_quality_rollback_enabled"))
                .and_then(Value::as_bool),
            Some(false)
        );

        let _ = std::fs::remove_dir_all(&workspace_root);
    });
}

#[test]
fn report_markdown_legacy_quality_mode_allows_generic_synthesis_with_emergency_rollback() {
    with_legacy_quality_rollback_enabled(true, || {
        let workspace_root = std::env::temp_dir().join(format!(
            "tandem-report-legacy-quality-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_root).expect("create workspace");
        let snapshot = automation_workspace_root_file_snapshot(
            workspace_root.to_str().expect("workspace root"),
        );
        let node = AutomationFlowNode {
            node_id: "generate_report".to_string(),
            agent_id: "writer".to_string(),
            objective: "Create the final report".to_string(),
            depends_on: vec!["analyze_findings".to_string()],
            input_refs: vec![AutomationFlowInputRef {
                from_step_id: "analyze_findings".to_string(),
                alias: "analysis".to_string(),
            }],
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
                "quality_mode": "legacy",
                "builder": {
                    "output_path": "generate-report.md"
                }
            })),
        };
        let mut session = Session::new(
            Some("legacy-quality-mode".to_string()),
            Some(workspace_root.to_str().expect("workspace root").to_string()),
        );
        let generic_report = "# Summary\n\nPlaceholder update.\n".to_string();
        session.messages.push(tandem_types::Message::new(
            MessageRole::Assistant,
            vec![MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "generate-report.md",
                    "content": generic_report
                }),
                result: Some(json!("ok")),
                error: None,
            }],
        ));
        let upstream_evidence = AutomationUpstreamEvidence {
            read_paths: vec![
                ".tandem/artifacts/collect-inputs.json".to_string(),
                ".tandem/artifacts/research-sources.json".to_string(),
            ],
            discovered_relevant_paths: vec![
                ".tandem/artifacts/collect-inputs.json".to_string(),
                ".tandem/artifacts/research-sources.json".to_string(),
            ],
            web_research_attempted: true,
            web_research_succeeded: true,
            citation_count: 3,
            citations: vec![
                "https://example.com/legacy-1".to_string(),
                "https://example.com/legacy-2".to_string(),
                "https://example.com/legacy-3".to_string(),
            ],
        };

        let (accepted_output, artifact_validation, rejected) =
            validate_automation_artifact_output_with_upstream(
                &node,
                &session,
                workspace_root.to_str().expect("workspace root"),
                None,
                "Completed the report.",
                &json!({
                    "requested_tools": ["read", "write"],
                    "executed_tools": ["read", "write"],
                    "tool_call_counts": {
                        "read": 2,
                        "write": 1
                    }
                }),
                None,
                Some(("generate-report.md".to_string(), generic_report)),
                &snapshot,
                Some(&upstream_evidence),
            );

        assert!(accepted_output.is_some());
        assert!(rejected.is_none());
        assert!(artifact_validation
            .get("unmet_requirements")
            .and_then(Value::as_array)
            .is_none_or(|items| !items
                .iter()
                .any(|value| value.as_str() == Some("upstream_evidence_not_synthesized"))));
        assert_eq!(
            artifact_validation
                .get("validation_basis")
                .and_then(|value| value.get("quality_mode"))
                .and_then(Value::as_str),
            Some("legacy")
        );
        assert_eq!(
            artifact_validation
                .get("validation_basis")
                .and_then(|value| value.get("requested_quality_mode"))
                .and_then(Value::as_str),
            Some("legacy")
        );
        assert_eq!(
            artifact_validation
                .get("validation_basis")
                .and_then(|value| value.get("legacy_quality_rollback_enabled"))
                .and_then(Value::as_bool),
            Some(true)
        );

        let _ = std::fs::remove_dir_all(&workspace_root);
    });
}

#[test]
fn report_markdown_rejects_generic_synthesis_without_evidence_anchors_when_upstream_is_rich() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-anchor-synthesis-block-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot =
        automation_workspace_root_file_snapshot(workspace_root.to_str().expect("workspace root"));
    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Create the final report".to_string(),
        depends_on: vec!["analyze_findings".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "analyze_findings".to_string(),
            alias: "analysis".to_string(),
        }],
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
                "output_path": "generate-report.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("anchor-block-report".to_string()),
        Some(workspace_root.to_str().expect("workspace root").to_string()),
    );
    let generic_report = "# Strategic Summary\n\n## Executive Summary\nThis report synthesizes the available upstream evidence into a concise outlook.\n\n## Key Findings\n* Growth vectors were identified across the workflow.\n* Strategic positioning remains promising.\n\n## Critical Risks\n* Competitive pressure remains a factor.\n\n## Recommendations\n* Continue iterating on the workflow.\n\n## Evidence/Sources\n* Internal documentation and external research informed this summary.\n\n## Next Steps\n* Refine the messaging and validate the next cycle.\n"
        .to_string();
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": "generate-report.md",
                "content": generic_report
            }),
            result: Some(json!("ok")),
            error: None,
        }],
    ));
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        discovered_relevant_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        web_research_attempted: true,
        web_research_succeeded: true,
        citation_count: 3,
        citations: vec![
            "https://example.com/1".to_string(),
            "https://example.com/2".to_string(),
            "https://example.com/3".to_string(),
        ],
    };

    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root"),
            None,
            "Completed the report.",
            &json!({
                "requested_tools": ["write"],
                "executed_tools": ["write"],
                "tool_call_counts": {
                    "write": 1
                }
            }),
            None,
            Some(("generate-report.md".to_string(), generic_report)),
            &snapshot,
            Some(&upstream_evidence),
        );

    assert!(accepted_output.is_some());
    assert_eq!(
        rejected.as_deref(),
        Some("final artifact does not adequately synthesize the available upstream evidence")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("final artifact does not adequately synthesize the available upstream evidence")
    );
    assert!(artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|items| items
            .iter()
            .any(|value| value.as_str() == Some("upstream_evidence_not_synthesized"))));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn report_markdown_accepts_rich_html_synthesis_when_upstream_is_rich() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-html-synthesis-pass-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot =
        automation_workspace_root_file_snapshot(workspace_root.to_str().expect("workspace root"));
    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Create the final report".to_string(),
        depends_on: vec!["analyze_findings".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "analyze_findings".to_string(),
            alias: "analysis".to_string(),
        }],
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
                "output_path": "generate-report.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("html-report".to_string()),
        Some(workspace_root.to_str().expect("workspace root").to_string()),
    );
    let html_report = r#"
<html>
  <body>
    <h1>Frumu AI Tandem: Strategic Summary</h1>
    <p>We synthesized the local Tandem docs and the external research into one report.</p>
    <h3>Core Value Proposition</h3>
    <p>Tandem is an engine-backed workflow system for local execution and agentic operations.</p>
    <ul>
      <li>Local workspace reads and patch-based code execution.</li>
      <li>Current web research for externally grounded synthesis.</li>
      <li>Explicit delivery gating for email and other side effects.</li>
    </ul>
    <h3>Strategic Outlook</h3>
    <p>The positioning emphasizes deterministic execution, provenance, and operator control.</p>
    <p>Sources reviewed: <a href=\".tandem/artifacts/analyze-findings.md\">analysis</a> and <a href=\".tandem/artifacts/research-sources.json\">research</a>.</p>
  </body>
</html>
"#
    .trim()
    .to_string();
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": "generate-report.md",
                "content": html_report
            }),
            result: Some(json!("ok")),
            error: None,
        }],
    ));
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        discovered_relevant_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        web_research_attempted: true,
        web_research_succeeded: true,
        citation_count: 3,
        citations: vec![
            "https://example.com/1".to_string(),
            "https://example.com/2".to_string(),
            "https://example.com/3".to_string(),
        ],
    };

    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root"),
            None,
            "Completed the report.",
            &json!({
                "requested_tools": ["write"],
                "executed_tools": ["write"],
                "tool_call_counts": {
                    "write": 1
                }
            }),
            None,
            Some(("generate-report.md".to_string(), html_report.clone())),
            &snapshot,
            Some(&upstream_evidence),
        );

    assert!(accepted_output.is_some());
    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        None
    );
    assert!(!artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|items| items
            .iter()
            .any(|value| value.as_str() == Some("upstream_evidence_not_synthesized"))));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn report_markdown_rejects_generic_html_synthesis_without_evidence_anchors_when_upstream_is_rich() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-html-anchor-synthesis-block-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot =
        automation_workspace_root_file_snapshot(workspace_root.to_str().expect("workspace root"));
    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Create the final report".to_string(),
        depends_on: vec!["analyze_findings".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "analyze_findings".to_string(),
            alias: "analysis".to_string(),
        }],
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
                "output_path": "generate-report.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("html-anchor-block-report".to_string()),
        Some(workspace_root.to_str().expect("workspace root").to_string()),
    );
    let generic_html_report = r#"
<html>
  <body>
    <h1>Investor Summary: Strategic Analysis Report</h1>
    <p>We synthesized refined market data and findings from our research cycles into key growth vectors and strategic positioning for the target project.</p>
    <h3>Key Findings</h3>
    <ul>
      <li>Market growth vectors are present.</li>
      <li>Strategic positioning is available.</li>
    </ul>
    <h3>Critical Risks &amp; Considerations</h3>
    <p>Competitive pressure and entry barriers remain relevant.</p>
    <p>Operational mitigation follows the updated strategy.</p>
  </body>
</html>
"#
    .trim()
    .to_string();
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "path": "generate-report.md",
                "content": generic_html_report
            }),
            result: Some(json!("ok")),
            error: None,
        }],
    ));
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        discovered_relevant_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        web_research_attempted: true,
        web_research_succeeded: true,
        citation_count: 3,
        citations: vec![
            "https://example.com/1".to_string(),
            "https://example.com/2".to_string(),
            "https://example.com/3".to_string(),
        ],
    };

    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root"),
            None,
            "Completed the report.",
            &json!({
                "requested_tools": ["write"],
                "executed_tools": ["write"],
                "tool_call_counts": {
                    "write": 1
                }
            }),
            None,
            Some(("generate-report.md".to_string(), generic_html_report)),
            &snapshot,
            Some(&upstream_evidence),
        );

    assert!(accepted_output.is_some());
    assert_eq!(
        rejected.as_deref(),
        Some("final artifact does not adequately synthesize the available upstream evidence")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("final artifact does not adequately synthesize the available upstream evidence")
    );
    assert!(artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|items| items
            .iter()
            .any(|value| value.as_str() == Some("upstream_evidence_not_synthesized"))));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[test]
fn execution_policy_reports_workflow_class() {
    let research = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md"
            }
        })),
    };
    let code = AutomationFlowNode {
        node_id: "code".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Code".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
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
                "task_kind": "code_change",
                "output_path": "handoff.md"
            }
        })),
    };

    assert_eq!(
        automation_node_execution_policy(&research, ".")
            .get("workflow_class")
            .and_then(Value::as_str),
        Some("research")
    );
    assert_eq!(
        automation_node_execution_policy(&code, ".")
            .get("workflow_class")
            .and_then(Value::as_str),
        Some("code")
    );
}

#[test]
fn workflow_state_events_capture_typed_stability_transitions() {
    let mut run = AutomationV2RunRecord {
        run_id: "run-1".to_string(),
        automation_id: "automation-1".to_string(),
        trigger_type: "manual".to_string(),
        status: AutomationRunStatus::Running,
        created_at_ms: 0,
        updated_at_ms: 0,
        started_at_ms: Some(0),
        finished_at_ms: None,
        latest_session_id: None,
        active_session_ids: Vec::new(),
        active_instance_ids: Vec::new(),
        checkpoint: AutomationRunCheckpoint {
            completed_nodes: Vec::new(),
            pending_nodes: Vec::new(),
            node_outputs: std::collections::HashMap::new(),
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
    };
    let output = json!({
        "status": "blocked",
        "workflow_class": "research",
        "phase": "research_validation",
        "failure_kind": "research_missing_reads",
        "blocked_reason": "research completed without concrete file reads",
        "artifact_validation": {
            "accepted_candidate_source": "session_write_recovery",
            "artifact_candidates": [
                {
                    "source": "session_write",
                    "length": 1200,
                    "substantive": true,
                    "placeholder_like": false,
                    "accepted": false
                }
            ],
            "repair_attempted": true,
            "repair_succeeded": false,
            "unmet_requirements": ["no_concrete_reads"],
            "blocking_classification": "tool_available_but_not_used",
            "required_next_tool_actions": [
                "Use `read` on concrete workspace files before finalizing the brief."
            ],
            "verification": {
                "verification_expected": false,
                "verification_ran": false,
                "verification_failed": false
            }
        }
    });

    record_automation_workflow_state_events(
        &mut run,
        "research-brief",
        &output,
        2,
        Some("session-1"),
        "blocked brief",
        "brief",
    );

    let events = run
        .checkpoint
        .lifecycle_history
        .iter()
        .map(|event| event.event.as_str())
        .collect::<Vec<_>>();
    assert!(events.contains(&"workflow_state_changed"));
    assert!(events.contains(&"artifact_candidate_written"));
    assert!(events.contains(&"artifact_accepted"));
    assert!(events.contains(&"repair_started"));
    assert!(events.contains(&"repair_exhausted"));
    assert!(events.contains(&"research_coverage_failed"));

    let state_event = run
        .checkpoint
        .lifecycle_history
        .iter()
        .find(|event| event.event == "workflow_state_changed")
        .expect("workflow state event");
    assert_eq!(
        state_event
            .metadata
            .as_ref()
            .and_then(|value| value.get("workflow_class"))
            .and_then(Value::as_str),
        Some("research")
    );
    assert_eq!(
        state_event
            .metadata
            .as_ref()
            .and_then(|value| value.get("failure_kind"))
            .and_then(Value::as_str),
        Some("research_missing_reads")
    );
    assert_eq!(
        state_event
            .metadata
            .as_ref()
            .and_then(|value| value.get("blocking_classification"))
            .and_then(Value::as_str),
        Some("tool_available_but_not_used")
    );
    assert_eq!(
        state_event
            .metadata
            .as_ref()
            .and_then(|value| value.get("required_next_tool_actions"))
            .and_then(Value::as_array)
            .and_then(|rows| rows.first())
            .and_then(Value::as_str),
        Some("Use `read` on concrete workspace files before finalizing the brief.")
    );
}

#[test]
fn code_workflow_verification_failure_sets_verify_failed_status() {
    let node = AutomationFlowNode {
        node_id: "implement".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Implement feature".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
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
                "task_kind": "code_change",
                "verification_command": "cargo test"
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "edit", "apply_patch", "write", "bash"],
        "executed_tools": ["read", "apply_patch", "bash"],
        "verification_expected": true,
        "verification_ran": true,
        "verification_failed": true,
        "latest_verification_failure": "verification command failed with exit code 101: cargo test"
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done\n\n{\"status\":\"completed\"}",
            None,
            &tool_telemetry,
            None,
        );

    assert_eq!(status, "verify_failed");
    assert_eq!(
        reason.as_deref(),
        Some("verification command failed with exit code 101: cargo test")
    );
    assert_eq!(approved, None);
}

#[test]
fn code_workflow_without_verification_run_is_blocked() {
    let node = AutomationFlowNode {
        node_id: "implement".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Implement feature".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
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
                "task_kind": "code_change",
                "verification_command": "cargo test"
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "edit", "apply_patch", "write", "bash"],
        "executed_tools": ["read", "apply_patch"],
        "verification_expected": true,
        "verification_ran": false,
        "verification_failed": false
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done\n\n{\"status\":\"completed\"}",
            None,
            &tool_telemetry,
            None,
        );

    assert_eq!(status, "blocked");
    assert_eq!(
        reason.as_deref(),
        Some("coding task completed without running the declared verification command")
    );
    assert_eq!(approved, None);
}

#[test]
fn collect_automation_external_action_receipts_records_bound_publisher_tools() {
    let automation = AutomationV2Spec {
        automation_id: "auto-publish-test".to_string(),
        name: "Publish Test".to_string(),
        description: None,
        status: AutomationV2Status::Active,
        schedule: AutomationV2Schedule {
            schedule_type: AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 1,
        updated_at_ms: 1,
        creator_id: "test".to_string(),
        workspace_root: Some(".".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "publish".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Publish final update".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "role": "publisher"
            }
        })),
    };
    let mut session = Session::new(Some("publisher".to_string()), Some(".".to_string()));
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "workflow_test.slack".to_string(),
                args: json!({
                    "channel": "engineering",
                    "text": "Ship it"
                }),
                result: Some(json!({
                    "output": "posted",
                    "metadata": {
                        "channel": "engineering"
                    }
                })),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "workflow_test.internal".to_string(),
                args: json!({
                    "value": 1
                }),
                result: Some(json!({"output": "ignored"})),
                error: None,
            },
        ],
    ));
    let mut bindings = capability_resolver::CapabilityBindingsFile::default();
    bindings
        .bindings
        .push(capability_resolver::CapabilityBinding {
            capability_id: "slack.post_message".to_string(),
            provider: "custom".to_string(),
            tool_name: "workflow_test.slack".to_string(),
            tool_name_aliases: Vec::new(),
            request_transform: None,
            response_transform: None,
            metadata: json!({}),
        });

    let receipts = collect_automation_external_action_receipts(
        &bindings,
        "run-1",
        &automation,
        &node,
        1,
        "session-1",
        &session,
    );

    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].source_kind.as_deref(), Some("automation_v2"));
    assert_eq!(
        receipts[0].capability_id.as_deref(),
        Some("slack.post_message")
    );
    assert_eq!(
        receipts[0].context_run_id.as_deref(),
        Some("automation-v2-run-1")
    );
    assert_eq!(receipts[0].target.as_deref(), Some("engineering"));
}

#[test]
fn collect_automation_external_action_receipts_ignores_non_outbound_nodes() {
    let automation = AutomationV2Spec {
        automation_id: "auto-draft-test".to_string(),
        name: "Draft Test".to_string(),
        description: None,
        status: AutomationV2Status::Active,
        schedule: AutomationV2Schedule {
            schedule_type: AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 1,
        updated_at_ms: 1,
        creator_id: "test".to_string(),
        workspace_root: Some(".".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "draft".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Draft final update".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "role": "writer"
            }
        })),
    };
    let mut session = Session::new(Some("writer".to_string()), Some(".".to_string()));
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "workflow_test.slack".to_string(),
            args: json!({
                "channel": "engineering",
                "text": "Ship it"
            }),
            result: Some(json!({"output": "posted"})),
            error: None,
        }],
    ));
    let mut bindings = capability_resolver::CapabilityBindingsFile::default();
    bindings
        .bindings
        .push(capability_resolver::CapabilityBinding {
            capability_id: "slack.post_message".to_string(),
            provider: "custom".to_string(),
            tool_name: "workflow_test.slack".to_string(),
            tool_name_aliases: Vec::new(),
            request_transform: None,
            response_transform: None,
            metadata: json!({}),
        });

    let receipts = collect_automation_external_action_receipts(
        &bindings,
        "run-1",
        &automation,
        &node,
        1,
        "session-1",
        &session,
    );

    assert!(receipts.is_empty());
}

#[test]
fn collect_automation_external_action_receipts_stabilize_identity_across_retries() {
    let automation = AutomationV2Spec {
        automation_id: "auto-publish-attempt-test".to_string(),
        name: "Publish Attempt Test".to_string(),
        description: None,
        status: AutomationV2Status::Active,
        schedule: AutomationV2Schedule {
            schedule_type: AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 1,
        updated_at_ms: 1,
        creator_id: "test".to_string(),
        workspace_root: Some(".".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "publish".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Publish final update".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: None,
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "role": "publisher"
            }
        })),
    };
    let mut session = Session::new(Some("publisher".to_string()), Some(".".to_string()));
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "workflow_test.slack".to_string(),
            args: json!({
                "channel": "engineering",
                "text": "Ship it"
            }),
            result: Some(json!({"output": "posted"})),
            error: None,
        }],
    ));
    let mut bindings = capability_resolver::CapabilityBindingsFile::default();
    bindings
        .bindings
        .push(capability_resolver::CapabilityBinding {
            capability_id: "slack.post_message".to_string(),
            provider: "custom".to_string(),
            tool_name: "workflow_test.slack".to_string(),
            tool_name_aliases: Vec::new(),
            request_transform: None,
            response_transform: None,
            metadata: json!({}),
        });

    let first_attempt = collect_automation_external_action_receipts(
        &bindings,
        "run-1",
        &automation,
        &node,
        1,
        "session-1",
        &session,
    );
    let second_attempt = collect_automation_external_action_receipts(
        &bindings,
        "run-1",
        &automation,
        &node,
        2,
        "session-1",
        &session,
    );

    assert_eq!(first_attempt.len(), 1);
    assert_eq!(second_attempt.len(), 1);
    assert_eq!(first_attempt[0].action_id, second_attempt[0].action_id);
    assert_eq!(
        first_attempt[0].idempotency_key,
        second_attempt[0].idempotency_key
    );
    assert_ne!(first_attempt[0].source_id, second_attempt[0].source_id);
}

#[test]
fn code_workflow_with_full_verification_plan_reports_done() {
    let node = AutomationFlowNode {
        node_id: "implement".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Implement feature".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
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
                "task_kind": "code_change",
                "verification_command": "cargo check\ncargo test\ncargo clippy --all-targets"
            }
        })),
    };
    let mut session = Session::new(Some("verification pass".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "bash".to_string(),
                args: json!({"command":"cargo check"}),
                result: Some(json!({"metadata":{"exit_code":0}})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "bash".to_string(),
                args: json!({"command":"cargo test"}),
                result: Some(json!({"metadata":{"exit_code":0}})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "bash".to_string(),
                args: json!({"command":"cargo clippy --all-targets"}),
                result: Some(json!({"metadata":{"exit_code":0}})),
                error: None,
            },
        ],
    ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "edit".to_string(),
            "apply_patch".to_string(),
            "write".to_string(),
            "bash".to_string(),
        ],
    );

    assert_eq!(
        tool_telemetry
            .get("verification_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        tool_telemetry
            .get("verification_total")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        tool_telemetry
            .get("verification_completed")
            .and_then(Value::as_u64),
        Some(3)
    );

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done\n\n{\"status\":\"completed\"}",
            None,
            &tool_telemetry,
            None,
        );

    assert_eq!(status, "done");
    assert_eq!(reason, None);
    assert_eq!(approved, None);
}

#[test]
fn code_workflow_with_partial_verification_is_blocked() {
    let node = AutomationFlowNode {
        node_id: "implement".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Implement feature".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
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
                "task_kind": "code_change",
                "verification_command": "cargo check\ncargo test\ncargo clippy --all-targets"
            }
        })),
    };
    let mut session = Session::new(Some("verification partial".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "bash".to_string(),
                args: json!({"command":"cargo check"}),
                result: Some(json!({"metadata":{"exit_code":0}})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "bash".to_string(),
                args: json!({"command":"cargo test"}),
                result: Some(json!({"metadata":{"exit_code":0}})),
                error: None,
            },
        ],
    ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "edit".to_string(),
            "apply_patch".to_string(),
            "write".to_string(),
            "bash".to_string(),
        ],
    );

    assert_eq!(
        tool_telemetry
            .get("verification_outcome")
            .and_then(Value::as_str),
        Some("partial")
    );

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done\n\n{\"status\":\"completed\"}",
            None,
            &tool_telemetry,
            None,
        );

    assert_eq!(status, "blocked");
    assert_eq!(
        reason.as_deref(),
        Some("coding task completed with only 2 of 3 declared verification commands run")
    );
    assert_eq!(approved, None);
}

#[test]
fn email_delivery_nodes_block_without_email_tool_execution() {
    let node = AutomationFlowNode {
        node_id: "notify_user".to_string(),
        agent_id: "agent-committer".to_string(),
        objective: "Send the finalized report to the requested email address in the email body using simple HTML.".to_string(),
        depends_on: vec!["generate_report".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "generate_report".to_string(),
            alias: "report_body".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": "evan@frumu.ai",
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    };

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "The report is ready.\n\n{\"status\":\"completed\",\"approved\":true}",
            None,
            &json!({
                "requested_tools": ["*"],
                "executed_tools": ["read"],
                "tool_call_counts": {"read": 1},
                "workspace_inspection_used": true,
                "email_delivery_attempted": false,
                "email_delivery_succeeded": false,
                "latest_email_delivery_failure": null
            }),
            None,
        );

    assert_eq!(status, "blocked");
    assert_eq!(
        reason.as_deref(),
        Some(
            "email delivery to `evan@frumu.ai` was requested but no email draft/send tool executed"
        )
    );
    assert_eq!(approved, Some(true));
}

#[test]
fn email_delivery_nodes_without_email_tools_report_tool_unavailable_with_diagnostics() {
    let node = AutomationFlowNode {
        node_id: "notify_user".to_string(),
        agent_id: "agent-committer".to_string(),
        objective: "Send the finalized report to the requested email address in the email body using simple HTML.".to_string(),
        depends_on: vec!["generate_report".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "generate_report".to_string(),
            alias: "report_body".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": "evan@frumu.ai",
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    };

    let tool_telemetry = json!({
        "requested_tools": ["glob", "read"],
        "executed_tools": ["read"],
        "tool_call_counts": {"read": 1},
        "workspace_inspection_used": true,
        "email_delivery_attempted": false,
        "email_delivery_succeeded": false,
        "latest_email_delivery_failure": null,
        "capability_resolution": {
            "required_capabilities": ["workspace_read", "email_send", "email_draft"],
            "missing_capabilities": ["email_send", "email_draft"],
            "email_tool_diagnostics": {
                "available_tools": [],
                "offered_tools": [],
                "available_send_tools": [],
                "offered_send_tools": [],
                "available_draft_tools": [],
                "offered_draft_tools": [],
                "selected_servers": ["composio-1"],
                "remote_tools": ["mcp.composio_1.send_message"],
                "registered_tools": ["mcp.composio_1.send_message"]
            },
            "mcp_tool_diagnostics": {
                "selected_servers": ["composio-1"],
                "servers": [{
                    "name": "composio-1",
                    "connected": true,
                    "remote_tools": ["mcp.composio_1.send_message"],
                    "registered_tools": ["mcp.composio_1.send_message"]
                }],
                "remote_tools": ["mcp.composio_1.send_message"],
                "registered_tools": ["mcp.composio_1.send_message"],
                "remote_email_like_tools": [],
                "registered_email_like_tools": []
            }
        },
        "attempt_evidence": {
            "delivery": {
                "status": "not_attempted"
            }
        }
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "I could not verify that an email was sent in this run.",
            None,
            &tool_telemetry,
            None,
        );

    assert_eq!(status, "blocked");
    assert!(reason
        .as_deref()
        .is_some_and(|value| value.contains("Discovered email-like tools: none")));
    assert!(reason
        .as_deref()
        .is_some_and(|value| value.contains("Selected MCP servers: composio-1")));
    assert!(reason
        .as_deref()
        .is_some_and(|value| value
            .contains("Remote MCP tools on selected servers: mcp.composio_1.send_message")));
    assert!(reason.as_deref().is_some_and(|value| value.contains(
        "Registered tool-registry tools on selected servers: mcp.composio_1.send_message"
    )));
    assert_eq!(approved, None);
    assert_eq!(
        detect_automation_blocker_category(
            &node,
            &status,
            reason.as_deref(),
            &tool_telemetry,
            None,
        )
        .as_deref(),
        Some("tool_unavailable")
    );
}

#[test]
fn email_delivery_nodes_complete_after_email_tool_execution() {
    let node = AutomationFlowNode {
        node_id: "notify_user".to_string(),
        agent_id: "agent-committer".to_string(),
        objective: "Send the finalized report to the requested email address in the email body using simple HTML.".to_string(),
        depends_on: vec!["generate_report".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "generate_report".to_string(),
            alias: "report_body".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: None,
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": "evan@frumu.ai",
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    };

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Sent the report.\n\n{\"status\":\"completed\",\"approved\":true}",
            None,
            &json!({
                "requested_tools": ["*"],
                "executed_tools": ["read", "mcp.composio_1.gmail_send_email"],
                "tool_call_counts": {"read": 1, "mcp.composio_1.gmail_send_email": 1},
                "workspace_inspection_used": true,
                "email_delivery_attempted": true,
                "email_delivery_succeeded": true,
                "latest_email_delivery_failure": null
            }),
            None,
        );

    assert_eq!(status, "completed");
    assert_eq!(reason, None);
    assert_eq!(approved, Some(true));
}

#[test]
fn infer_selected_mcp_servers_uses_enabled_servers_for_wildcard_allowlist() {
    let selected = crate::app::state::automation::automation_infer_selected_mcp_servers(
        &[],
        &["*".to_string()],
        &["gmail-main".to_string(), "slack-main".to_string()],
        false,
    );

    assert_eq!(
        selected,
        vec!["gmail-main".to_string(), "slack-main".to_string()]
    );
}

#[test]
fn infer_selected_mcp_servers_uses_enabled_servers_for_email_delivery_fallback() {
    let selected = crate::app::state::automation::automation_infer_selected_mcp_servers(
        &[],
        &["glob".to_string(), "read".to_string()],
        &["gmail-main".to_string()],
        true,
    );

    assert_eq!(selected, vec!["gmail-main".to_string()]);
}

#[test]
fn infer_selected_mcp_servers_prefers_explicit_selection_when_present() {
    let selected = crate::app::state::automation::automation_infer_selected_mcp_servers(
        &["composio-1".to_string()],
        &["*".to_string()],
        &["gmail-main".to_string(), "composio-1".to_string()],
        true,
    );

    assert_eq!(selected, vec!["composio-1".to_string()]);
}

#[test]
fn session_read_paths_accepts_json_string_tool_args() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-session-read-paths-json-string-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("src")).expect("create workspace");
    std::fs::write(workspace_root.join("src/lib.rs"), "pub fn demo() {}\n").expect("seed file");

    let mut session = Session::new(
        Some("json string read args".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "read".to_string(),
            args: json!("{\"path\":\"src/lib.rs\"}"),
            result: Some(json!({"ok": true})),
            error: None,
        }],
    ));

    let paths = session_read_paths(
        &session,
        workspace_root.to_str().expect("workspace root string"),
    );

    assert_eq!(paths, vec!["src/lib.rs".to_string()]);
}

#[test]
fn session_write_candidates_accepts_json_string_tool_args() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-session-write-candidates-json-string-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let mut session = Session::new(
        Some("json string write args".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!("{\"path\":\"brief.md\",\"content\":\"Draft body\"}"),
            result: Some(json!({"ok": true})),
            error: None,
        }],
    ));

    let candidates = session_write_candidates_for_output(
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "brief.md",
        None,
    );

    assert_eq!(candidates, vec!["Draft body".to_string()]);
}

#[test]
fn session_write_touched_output_detects_target_path_without_content() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-session-write-touched-output-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let mut session = Session::new(
        Some("write touched output".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "write".to_string(),
            args: json!({
                "output_path": "brief.md"
            }),
            result: Some(json!({"ok": true})),
            error: None,
        }],
    ));

    let touched = session_write_touched_output_for_output(
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "brief.md",
        None,
    );

    assert!(
        touched,
        "write invocation should count as touching declared output path"
    );
}

#[test]
fn session_file_mutation_summary_accepts_json_string_tool_args() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-session-mutation-summary-json-string-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("src")).expect("create workspace");

    let mut session = Session::new(
        Some("json string mutation args".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
            MessageRole::Assistant,
            vec![
                MessagePart::ToolInvocation {
                    tool: "write".to_string(),
                    args: json!("{\"path\":\"src/lib.rs\",\"content\":\"pub fn demo() {}\\n\"}"),
                    result: Some(json!({"ok": true})),
                    error: None,
                },
                MessagePart::ToolInvocation {
                    tool: "apply_patch".to_string(),
                    args: json!("{\"patchText\":\"*** Begin Patch\\n*** Update File: src/other.rs\\n@@\\n-old\\n+new\\n*** End Patch\\n\"}"),
                    result: Some(json!({"ok": true})),
                    error: None,
                },
            ],
        ));

    let summary = session_file_mutation_summary(
        &session,
        workspace_root.to_str().expect("workspace root string"),
    );

    assert_eq!(
        summary
            .get("touched_files")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        vec![json!("src/lib.rs"), json!("src/other.rs")]
    );
    assert_eq!(
        summary
            .get("mutation_tool_by_file")
            .and_then(|value| value.get("src/lib.rs"))
            .cloned(),
        Some(json!(["write"]))
    );
    assert_eq!(
        summary
            .get("mutation_tool_by_file")
            .and_then(|value| value.get("src/other.rs"))
            .cloned(),
        Some(json!(["apply_patch"]))
    );
}

#[test]
fn code_workflow_rejects_unsafe_raw_source_rewrites() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-automation-unsafe-write-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("src")).expect("create workspace");
    std::fs::write(workspace_root.join("src/lib.rs"), "pub fn before() {}\n").expect("seed source");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let long_handoff = format!(
        "# Handoff\n\n{}\n",
        "Detailed implementation summary. ".repeat(20)
    );
    let node = AutomationFlowNode {
        node_id: "implement".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Implement feature".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
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
                "task_kind": "code_change",
                "output_path": "handoff.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("unsafe raw write".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "src/lib.rs",
                    "content": "pub fn after() {}\n"
                }),
                result: Some(json!({"ok": true})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "handoff.md",
                    "content": long_handoff
                }),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));

    let (_, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "",
        &json!({
            "requested_tools": ["read", "write"],
            "executed_tools": ["write"]
        }),
        None,
        Some(("handoff.md".to_string(), long_handoff)),
        &snapshot,
    );

    assert_eq!(
        rejected.as_deref(),
        Some("unsafe raw source rewrite rejected: src/lib.rs")
    );
    assert_eq!(
        metadata
            .get("rejected_artifact_reason")
            .and_then(Value::as_str),
        Some("unsafe raw source rewrite rejected: src/lib.rs")
    );

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_finalize_prompt_includes_upstream_coverage_summary() {
    let automation = AutomationV2Spec {
        automation_id: "automation-research-summary".to_string(),
        name: "Research Summary".to_string(),
        description: None,
        status: crate::AutomationV2Status::Active,
        schedule: crate::AutomationV2Schedule {
            schedule_type: crate::AutomationV2ScheduleType::Manual,
            cron_expression: None,
            interval_seconds: None,
            timezone: "UTC".to_string(),
            misfire_policy: crate::RoutineMisfirePolicy::RunOnce,
        },
        agents: Vec::new(),
        flow: AutomationFlowSpec { nodes: Vec::new() },
        execution: AutomationExecutionPolicy {
            max_parallel_agents: Some(1),
            max_total_runtime_ms: None,
            max_total_tool_calls: None,
            max_total_tokens: None,
            max_total_cost_usd: None,
        },
        output_targets: Vec::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        creator_id: "test".to_string(),
        workspace_root: Some("/tmp".to_string()),
        metadata: None,
        next_fire_at_ms: None,
        last_fired_at_ms: None,
    };
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "research".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: vec![
            "research-discover-sources".to_string(),
            "research-local-sources".to_string(),
            "research-external-research".to_string(),
        ],
        input_refs: vec![
            AutomationFlowInputRef {
                from_step_id: "research-discover-sources".to_string(),
                alias: "source_inventory".to_string(),
            },
            AutomationFlowInputRef {
                from_step_id: "research-local-sources".to_string(),
                alias: "local_source_notes".to_string(),
            },
            AutomationFlowInputRef {
                from_step_id: "research-external-research".to_string(),
                alias: "external_research".to_string(),
            },
        ],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Write `marketing-brief.md`.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "title": "Research Brief",
                "role": "watcher",
                "output_path": "marketing-brief.md",
                "research_stage": "research_finalize",
                "prompt": "Finalize the brief."
            }
        })),
    };
    let agent = AutomationAgentProfile {
        agent_id: "research".to_string(),
        template_id: None,
        display_name: "Research".to_string(),
        avatar_url: None,
        model_policy: None,
        skills: Vec::new(),
        tool_policy: crate::AutomationAgentToolPolicy {
            allowlist: vec!["glob".to_string(), "read".to_string(), "write".to_string()],
            denylist: Vec::new(),
        },
        mcp_policy: crate::AutomationAgentMcpPolicy {
            allowed_servers: Vec::new(),
            allowed_tools: None,
        },
        approval_policy: None,
    };
    let upstream_inputs = vec![
        json!({
            "alias": "source_inventory",
            "from_step_id": "research-discover-sources",
            "output": {
                "content": {
                    "structured_handoff": {
                        "discovered_paths": [
                            {"path": "tandem-reference/SOURCES.md", "type": "file"},
                            {"path": "tandem/implementation_plan.md", "type": "file"}
                        ],
                        "priority_paths": [
                            {"path": "tandem-reference/SOURCES.md", "priority": 1},
                            {"path": "tandem/implementation_plan.md", "priority": 2}
                        ]
                    }
                }
            }
        }),
        json!({
            "alias": "local_source_notes",
            "from_step_id": "research-local-sources",
            "output": {
                "content": {
                    "structured_handoff": {
                        "files_reviewed": ["tandem-reference/SOURCES.md"],
                        "files_not_reviewed": [
                            {"path": "tandem/implementation_plan.md", "reason": "deferred"}
                        ]
                    }
                }
            }
        }),
        json!({
            "alias": "external_research",
            "from_step_id": "research-external-research",
            "output": {
                "content": {
                    "structured_handoff": {
                        "sources_reviewed": [
                            {"url": "https://example.com/reference"}
                        ]
                    }
                }
            }
        }),
    ];

    let prompt = render_automation_v2_prompt(
        &automation,
        "/tmp",
        "run-research-summary",
        &node,
        1,
        &agent,
        &upstream_inputs,
        &["glob".to_string(), "read".to_string(), "write".to_string()],
        None,
        None,
        None,
    );

    assert!(prompt.contains("Research Coverage Summary:"));
    assert!(prompt.contains("`tandem-reference/SOURCES.md`"));
    assert!(prompt.contains("`tandem/implementation_plan.md`"));
    assert!(prompt.contains("`Files reviewed` or `Files not reviewed`"));
    assert!(prompt.contains("citation-backed"));
}

#[test]
fn artifact_validation_restores_substantive_session_write_over_short_completion_note() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-automation-restore-write-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let substantive = format!(
        "# Marketing Brief\n\n## Workspace source audit\n{}\n",
        "Real sourced marketing brief content. ".repeat(40)
    );
    std::fs::write(
        workspace_root.join("marketing-brief.md"),
        "Marketing brief completed and written to marketing-brief.md.\n",
    )
    .expect("seed placeholder");
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let mut session = Session::new(
        Some("restore substantive write".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "marketing-brief.md",
                    "content": substantive
                }),
                result: Some(json!({"ok": true})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "marketing-brief.md",
                    "content": "Marketing brief completed and written to marketing-brief.md."
                }),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));

    let (accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done — `marketing-brief.md` was written in the workspace.\n\n{\"status\":\"completed\",\"approved\":true}",
        &json!({
            "requested_tools": ["glob", "read", "websearch", "write"],
            "executed_tools": ["glob", "websearch", "write"],
            "workspace_inspection_used": true,
            "web_research_used": true
        }),
        None,
        Some((
            "marketing-brief.md".to_string(),
            "Marketing brief completed and written to marketing-brief.md.".to_string(),
        )),
        &snapshot,
    );

    assert!(matches!(
        rejected.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
            | Some("research completed without required current web research")
    ));
    assert_eq!(
        metadata
            .get("recovered_from_session_write")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        metadata
            .get("validation_basis")
            .and_then(Value::as_object)
            .and_then(|value| value.get("authority"))
            .and_then(Value::as_str),
        Some("filesystem_and_receipts")
    );
    assert_eq!(
        metadata
            .get("validation_basis")
            .and_then(Value::as_object)
            .and_then(|value| value.get("current_attempt_output_materialized"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        accepted_output.as_ref().map(|(_, text)| text.as_str()),
        Some("Marketing brief completed and written to marketing-brief.md.")
    );
    let disk_text = std::fs::read_to_string(workspace_root.join("marketing-brief.md"))
        .expect("read restored file");
    assert_eq!(
        disk_text.trim(),
        "Marketing brief completed and written to marketing-brief.md."
    );
    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
        &node,
        "Done — `marketing-brief.md` was written in the workspace.\n\n{\"status\":\"completed\",\"approved\":true}",
        accepted_output.as_ref(),
        &json!({
            "requested_tools": ["glob", "read", "websearch", "write"],
            "executed_tools": ["glob", "websearch", "write"],
            "workspace_inspection_used": true,
            "web_research_used": true
        }),
        Some(&metadata),
    );
    assert_eq!(status, "needs_repair");
    assert!(matches!(
        reason.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
            | Some("research completed without required current web research")
    ));
    assert_eq!(approved, Some(true));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn artifact_validation_blocks_session_text_recovery_until_prewrite_is_satisfied() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-automation-block-session-recovery-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let placeholder = "Marketing brief completed and written to marketing-brief.md.\n";
    let substantive = format!(
        "# Marketing Brief\n\n## Workspace source audit\n{}\n\n## Files reviewed\n- docs/source.md\n\n## Web sources reviewed\n- https://example.com\n",
        "Unsafely recovered brief content. ".repeat(30)
    );
    std::fs::write(workspace_root.join("marketing-brief.md"), placeholder)
        .expect("seed placeholder");
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let session = Session::new(
        Some("blocked recovery".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );

    let (accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        &substantive,
        &json!({
            "requested_tools": ["glob", "read", "websearch", "write"],
            "executed_tools": [],
            "workspace_inspection_used": false,
            "web_research_used": false,
            "web_research_succeeded": false
        }),
        Some(&substantive),
        Some(("marketing-brief.md".to_string(), placeholder.to_string())),
        &snapshot,
    );

    assert_eq!(
        accepted_output.as_ref().map(|(_, text)| text.as_str()),
        None
    );
    assert_eq!(
        rejected.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert_eq!(
        metadata
            .get("recovered_from_session_write")
            .and_then(Value::as_bool),
        Some(false)
    );
    let disk_text = std::fs::read_to_string(workspace_root.join("marketing-brief.md"))
        .expect("read placeholder");
    assert_eq!(disk_text, placeholder);

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_validation_does_not_accept_preexisting_output_without_current_attempt_activity() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-automation-preexisting-research-block-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let stale_preexisting = format!(
        "# Marketing Brief\n\n## Workspace source audit\n{}\n\n## Campaign Goal\nCarry over stale content.\n\n## Files Reviewed\nNone\n\n## Files Not Reviewed\nAll\n\n## Web Sources Reviewed\nNone\n",
        "Stale brief content from an earlier failed run. ".repeat(30)
    );
    let current_disk_output = "# Marketing Brief\n\nAttempt wrote nothing new.\n".to_string();
    std::fs::write(
        workspace_root.join("marketing-brief.md"),
        &current_disk_output,
    )
    .expect("seed output");
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let session = Session::new(
        Some("empty attempt".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );

    let (accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "I completed project analysis steps using tools, but the model returned no final narrative text.",
        &json!({
            "requested_tools": ["glob", "read", "websearch", "write"],
            "executed_tools": [],
            "workspace_inspection_used": false,
            "web_research_used": false,
            "web_research_succeeded": false
        }),
        Some(&stale_preexisting),
        Some((
            "marketing-brief.md".to_string(),
            current_disk_output.clone(),
        )),
        &snapshot,
    );

    assert!(accepted_output.is_none());
    assert_eq!(
        metadata
            .get("accepted_candidate_source")
            .and_then(Value::as_str),
        Some("current_attempt_missing_output_write")
    );
    assert_eq!(
        rejected.as_deref(),
        Some("required output `marketing-brief.md` was not created in the current attempt")
    );
    assert_eq!(
        metadata
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("required output was not created in the current attempt")
    );

    let disk_text = std::fs::read_to_string(workspace_root.join("marketing-brief.md"))
        .expect("read unchanged output");
    assert_eq!(disk_text, current_disk_output);

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn generic_artifact_validation_rejects_stale_preexisting_output_without_current_session_write() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-stale-generic-artifact-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let stale_preexisting =
        "# Report\n\n## Summary\n\nOld generic content.\n\nParagraph two.\n".to_string();
    std::fs::write(workspace_root.join("report.md"), &stale_preexisting)
        .expect("seed stale output");
    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Generate the final report".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
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
                "output_path": "report.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("generate-report-stale".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "read".to_string(),
            args: json!({
                "path": "input.md"
            }),
            result: Some(json!("source material")),
            error: None,
        }],
    ));

    let (accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Completed the report.",
        &json!({
            "requested_tools": ["read", "write"],
            "executed_tools": ["read"],
            "tool_call_counts": {
                "read": 1
            }
        }),
        Some(&stale_preexisting),
        Some(("report.md".to_string(), stale_preexisting.clone())),
        &snapshot,
    );

    assert!(accepted_output.is_none());
    assert_eq!(
        artifact_validation
            .get("accepted_candidate_source")
            .and_then(Value::as_str),
        Some("current_attempt_missing_output_write")
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("blocked")
    );
    assert_eq!(
        rejected.as_deref(),
        Some("required output `report.md` was not created in the current attempt")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("required output was not created in the current attempt")
    );

    let disk_text =
        std::fs::read_to_string(workspace_root.join("report.md")).expect("read stale output");
    assert_eq!(disk_text, stale_preexisting);

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn report_markdown_validation_accepts_updated_verified_output_without_session_write_telemetry() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-report-updated-without-session-write-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let stale_preexisting = "# Strategic Summary\n\nOld report content.\n".to_string();
    let updated_report = r#"
<html>
  <body>
    <h1>Frumu AI Tandem: Strategic Summary</h1>
    <p>We synthesized the upstream research into one report.</p>
    <h3>Core Value Proposition</h3>
    <p>Tandem is an engine-backed workflow system for local execution and agentic operations.</p>
    <ul>
      <li>Local workspace reads and patch-based code execution.</li>
      <li>Current web research for externally grounded synthesis.</li>
      <li>Explicit delivery gating for email and other side effects.</li>
    </ul>
    <h3>Strategic Outlook</h3>
    <p>The positioning emphasizes deterministic execution, provenance, and operator control.</p>
    <p>Sources reviewed: <a href=".tandem/artifacts/analyze-findings.md">analysis</a> and <a href=".tandem/artifacts/research-sources.json">research</a>.</p>
  </body>
</html>
"#
    .trim()
    .to_string();
    std::fs::write(
        workspace_root.join("generate-report.md"),
        &stale_preexisting,
    )
    .expect("seed stale report");
    let node = AutomationFlowNode {
        node_id: "generate_report".to_string(),
        agent_id: "writer".to_string(),
        objective: "Create the final report".to_string(),
        depends_on: vec!["analyze_findings".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "analyze_findings".to_string(),
            alias: "analysis".to_string(),
        }],
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
                "output_path": "generate-report.md"
            }
        })),
    };
    let mut session = Session::new(
        Some("generate-report-updated".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "read".to_string(),
            args: json!({"path":"analyze_findings.md"}),
            result: Some(json!({"ok": true})),
            error: None,
        }],
    ));
    std::fs::write(workspace_root.join("generate-report.md"), &updated_report)
        .expect("write updated report");
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        discovered_relevant_paths: vec![
            ".tandem/artifacts/collect-inputs.json".to_string(),
            ".tandem/artifacts/research-sources.json".to_string(),
            ".tandem/artifacts/analyze-findings.md".to_string(),
        ],
        web_research_attempted: true,
        web_research_succeeded: true,
        citation_count: 3,
        citations: vec![
            "https://example.com/1".to_string(),
            "https://example.com/2".to_string(),
            "https://example.com/3".to_string(),
        ],
    };

    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root"),
            None,
            "Completed the report.",
            &json!({}),
            Some(&stale_preexisting),
            Some(("generate-report.md".to_string(), updated_report.clone())),
            &snapshot,
            Some(&upstream_evidence),
        );

    assert!(accepted_output.is_some(), "{artifact_validation:?}");
    assert!(rejected.is_none(), "{artifact_validation:?}");
    assert_eq!(
        artifact_validation
            .get("accepted_candidate_source")
            .and_then(Value::as_str),
        Some("verified_output")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        None
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("validation_basis")
            .and_then(Value::as_object)
            .and_then(|value| value.get("authority"))
            .and_then(Value::as_str),
        Some("filesystem_and_receipts")
    );
    assert_eq!(
        artifact_validation
            .get("validation_basis")
            .and_then(Value::as_object)
            .and_then(|value| value.get("verified_output_materialized"))
            .and_then(Value::as_bool),
        Some(true)
    );

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_validation_removes_blocked_handoff_artifact_without_preexisting_output() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-automation-blocked-handoff-remove-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let blocked_text = "# Marketing Brief\n\nStatus: blocked pending required source reads and web research in this run.\n\nThis file cannot be finalized from the current toolset available in this session because the required discovery and external research tools referenced by the task (`read`, `glob`, `websearch`) are not available to me here.\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &blocked_text)
        .expect("seed blocked handoff");
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let session = Session::new(
        Some("blocked handoff".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );

    let (accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        &blocked_text,
        &json!({
            "requested_tools": ["glob", "read", "websearch", "write"],
            "executed_tools": ["glob", "websearch", "write"],
            "workspace_inspection_used": true,
            "web_research_used": true,
            "web_research_succeeded": false,
            "latest_web_research_failure": "web research authorization required"
        }),
        None,
        Some(("marketing-brief.md".to_string(), blocked_text.clone())),
        &snapshot,
    );

    assert!(accepted_output.is_none());
    assert_eq!(
        metadata
            .get("blocked_handoff_cleanup_action")
            .and_then(Value::as_str),
        Some("removed_blocked_output")
    );
    assert_eq!(
        rejected.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert!(!workspace_root.join("marketing-brief.md").exists());

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_validation_restores_preexisting_output_without_accepting_blocked_handoff() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-automation-blocked-handoff-restore-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let previous = "# Marketing Brief\n\n## Workspace source audit\nPrepared from earlier sourced work.\n\n## Files reviewed\n- docs/source.md\n\n## Web sources reviewed\n- https://example.com\n".to_string();
    let blocked_text = "# Marketing Brief\n\nStatus: blocked pending required source reads and web research in this run.\n\nThis file cannot be finalized from the current toolset available in this session because the required discovery and external research tools referenced by the task (`read`, `glob`, `websearch`) are not available to me here.\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &blocked_text)
        .expect("seed blocked handoff");
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let session = Session::new(
        Some("blocked handoff restore".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );

    let (accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        &blocked_text,
        &json!({
            "requested_tools": ["glob", "read", "websearch", "write"],
            "executed_tools": ["glob", "websearch", "write"],
            "workspace_inspection_used": true,
            "web_research_used": true,
            "web_research_succeeded": false,
            "latest_web_research_failure": "web research authorization required"
        }),
        Some(&previous),
        Some(("marketing-brief.md".to_string(), blocked_text.clone())),
        &snapshot,
    );

    assert!(accepted_output.is_none());
    assert_eq!(
        metadata
            .get("blocked_handoff_cleanup_action")
            .and_then(Value::as_str),
        Some("restored_preexisting_output")
    );
    assert_eq!(
        rejected.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
    );
    let disk_text = std::fs::read_to_string(workspace_root.join("marketing-brief.md"))
        .expect("read restored artifact");
    assert_eq!(disk_text, previous);

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn artifact_validation_prefers_structurally_stronger_candidate_without_phrase_match() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-automation-stronger-candidate-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let substantive = format!(
        "# Marketing Brief\n\n## Workspace source audit\n{}\n\n## Files reviewed\n- docs/source.md\n\n## Files not reviewed\n- docs/extra.md (out of scope)\n",
        "Detailed sourced content. ".repeat(50)
    );
    let weak_final = "# Marketing Brief\n\nShort wrap-up.\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &weak_final)
        .expect("seed final weak artifact");
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": false
            }
        })),
    };
    let mut session = Session::new(
        Some("stronger candidate".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"docs/source.md"}),
                result: Some(json!({"ok": true})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "marketing-brief.md",
                    "content": substantive
                }),
                result: Some(json!({"ok": true})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "marketing-brief.md",
                    "content": weak_final
                }),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));

    let (accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done",
        &json!({
            "requested_tools": ["glob", "read", "write"],
            "executed_tools": ["read", "write"]
        }),
        None,
        Some((
            "marketing-brief.md".to_string(),
            "# Marketing Brief\n\nShort wrap-up.\n".to_string(),
        )),
        &snapshot,
    );

    assert_eq!(
        rejected.as_deref(),
        Some("research completed without citation-backed claims")
    );
    assert_eq!(
        metadata
            .get("accepted_candidate_source")
            .and_then(Value::as_str),
        Some("session_write")
    );
    assert!(accepted_output
        .as_ref()
        .is_some_and(|(_, text)| text.contains("## Workspace source audit")));
    let disk_text = std::fs::read_to_string(workspace_root.join("marketing-brief.md"))
        .expect("read selected artifact");
    assert!(disk_text.contains("## Workspace source audit"));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn completed_brief_without_read_is_blocked_even_if_it_looks_confident() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let tool_telemetry = json!({
        "requested_tools": ["glob", "read", "websearch", "write"],
        "executed_tools": ["glob", "websearch", "write"],
        "workspace_inspection_used": true,
        "web_research_used": true
    });

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done — `marketing-brief.md` was written in the workspace.\n\n{\"status\":\"completed\",\"approved\":true}",
            Some(&(
                "marketing-brief.md".to_string(),
                "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n## Files reviewed\n- tandem-reference/readmes/repo-README.md\n- tandem-reference/readmes/engine-README.md\n".to_string(),
            )),
            &tool_telemetry,
            None,
        );

    assert_eq!(status, "completed");
    assert_eq!(reason.as_deref(), None);
    assert_eq!(approved, Some(true));
}

#[test]
fn brief_with_timed_out_websearch_is_blocked_when_web_research_is_required() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-websearch-timeout-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace_root).expect("create workspace root");
    let snapshot = std::collections::BTreeSet::new();

    let brief_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n## Files reviewed\n- tandem-reference/readmes/repo-README.md\n\n## Web sources reviewed\n- websearch attempt timed out.\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &brief_text).expect("seed artifact");

    let mut session = Session::new(
        Some("session-timeout".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"tandem-reference/readmes/repo-README.md"}),
                result: Some(json!({"ok": true})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "websearch".to_string(),
                args: json!({"query":"ai coding agents market"}),
                result: Some(json!({
                    "output": "Search timed out. No results received.",
                    "metadata": { "error": "timeout" }
                })),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({
                    "path": "marketing-brief.md",
                    "content": brief_text
                }),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));

    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    );
    assert_eq!(
        tool_telemetry
            .get("web_research_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        tool_telemetry
            .get("web_research_succeeded")
            .and_then(Value::as_bool),
        Some(false)
    );

    let (accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done — `marketing-brief.md` was written in the workspace.\n\n{\"status\":\"completed\",\"approved\":true}",
        &tool_telemetry,
        None,
        Some(("marketing-brief.md".to_string(), brief_text.clone())),
        &snapshot,
    );

    assert!(accepted_output.is_some());
    assert_eq!(
        metadata
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("research completed without citation-backed claims")
    );
    assert_eq!(
        rejected.as_deref(),
        Some("research completed without citation-backed claims")
    );
    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
        &node,
        "Done — `marketing-brief.md` was written in the workspace.\n\n{\"status\":\"completed\",\"approved\":true}",
        accepted_output.as_ref(),
        &tool_telemetry,
        Some(&metadata),
    );
    assert_eq!(status, "needs_repair");
    assert_eq!(
        reason.as_deref(),
        Some("research completed without citation-backed claims")
    );
    assert_eq!(approved, Some(true));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn brief_prewrite_requirements_enable_repair_and_coverage_mode() {
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let requirements = automation_node_prewrite_requirements(
        &node,
        &[
            "glob".to_string(),
            "read".to_string(),
            "websearch".to_string(),
            "write".to_string(),
        ],
    )
    .expect("prewrite requirements");
    assert!(requirements.workspace_inspection_required);
    assert!(requirements.web_research_required);
    assert!(requirements.concrete_read_required);
    assert!(requirements.successful_web_research_required);
    assert!(requirements.repair_on_unmet_requirements);
    assert_eq!(
        requirements.coverage_mode,
        PrewriteCoverageMode::ResearchCorpus
    );
}

#[test]
fn brief_with_unreviewed_discovered_files_is_blocked_with_structured_metadata() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-brief-coverage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(workspace_root.join("docs")).expect("create workspace");
    std::fs::write(
        workspace_root.join("docs/one.md"),
        "# One\nsource content\n",
    )
    .expect("write one");
    std::fs::write(
        workspace_root.join("docs/two.md"),
        "# Two\nsource content\n",
    )
    .expect("write two");
    let snapshot = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let node = AutomationFlowNode {
        node_id: "research".to_string(),
        agent_id: "agent-a".to_string(),
        objective: "Research".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": false
            }
        })),
    };
    let brief_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n## Files reviewed\n- docs/one.md\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &brief_text).expect("seed brief");
    let mut session = Session::new(
        Some("coverage mismatch".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "glob".to_string(),
                args: json!({"pattern":"docs/**/*.md"}),
                result: Some(json!({"output": format!(
                    "{}\n{}",
                    workspace_root.join("docs/one.md").display(),
                    workspace_root.join("docs/two.md").display()
                )})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"docs/one.md"}),
                result: Some(json!({"ok": true})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({"path":"marketing-brief.md","content":brief_text}),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));
    let tool_telemetry = summarize_automation_tool_activity(
        &node,
        &session,
        &["glob".to_string(), "read".to_string(), "write".to_string()],
    );
    let (_accepted_output, metadata, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done\n\n{\"status\":\"completed\"}",
        &tool_telemetry,
        None,
        Some(("marketing-brief.md".to_string(), brief_text)),
        &snapshot,
    );
    assert_eq!(
        rejected.as_deref(),
        Some(
            "research completed without covering or explicitly skipping relevant discovered files"
        )
    );
    assert_eq!(
        metadata
            .get("unreviewed_relevant_paths")
            .and_then(Value::as_array)
            .map(|values| values.len()),
        Some(1)
    );
    assert!(metadata
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("relevant_files_not_reviewed_or_skipped"))));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_brief_without_source_coverage_flag_gets_semantic_block_reason_and_needs_repair() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-research-no-coverage-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let brief_text =
        "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n"
            .to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &brief_text).expect("seed brief");
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let mut session = Session::new(
        Some("research-no-coverage".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "glob".to_string(),
                args: json!({"pattern":"docs/**/*.md"}),
                result: Some(json!({"output": ""})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({"path":"marketing-brief.md","content":brief_text}),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));
    let requested_tools = vec![
        "glob".to_string(),
        "read".to_string(),
        "websearch".to_string(),
        "write".to_string(),
    ];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done\n\n{\"status\":\"completed\"}",
        &tool_telemetry,
        None,
        Some(("marketing-brief.md".to_string(), brief_text.clone())),
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(
        rejected.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("needs_repair")
    );

    let (status, reason, approved): (String, Option<String>, Option<bool>) =
        detect_automation_node_status(
            &node,
            "Done — `marketing-brief.md` was written.",
            Some(&("marketing-brief.md".to_string(), brief_text)),
            &tool_telemetry,
            Some(&artifact_validation),
        );

    assert_eq!(status, "needs_repair");
    assert_eq!(
        reason.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert_eq!(approved, None);

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_brief_full_pipeline_overrides_llm_blocked_to_needs_repair_without_source_coverage_flag()
{
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-research-full-pipeline-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let brief_text =
        "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n"
            .to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &brief_text).expect("seed brief");
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let mut session = Session::new(
        Some("research-full-pipeline".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "glob".to_string(),
                args: json!({"pattern":"docs/**/*.md"}),
                result: Some(json!({"output": ""})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({"path":"marketing-brief.md","content":brief_text}),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));
    let requested_tools = vec![
        "glob".to_string(),
        "read".to_string(),
        "websearch".to_string(),
        "write".to_string(),
    ];
    let session_text =
        "The brief is blocked.\n\n{\"status\":\"blocked\",\"reason\":\"tools unavailable\"}";
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        session_text,
        &tool_telemetry,
        None,
        Some(("marketing-brief.md".to_string(), brief_text.clone())),
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(
        rejected.as_deref(),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        Some("research completed without concrete file reads or required source coverage")
    );

    let output: Value = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        "sess-research-full-pipeline",
        Some("run-research-full-pipeline"),
        session_text,
        accepted_output,
        Some(artifact_validation),
    );

    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("needs_repair")
    );
    assert_eq!(
        output.get("blocked_reason").and_then(Value::as_str),
        Some("research completed without concrete file reads or required source coverage")
    );
    assert!(!automation_output_is_blocked(&output));
    assert!(automation_output_needs_repair(&output));
    assert!(!automation_output_repair_exhausted(&output));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_brief_passes_when_websearch_is_auth_blocked_but_local_evidence_is_complete() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-research-web-failure-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let brief_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n## Campaign goal\nClarify positioning.\n\n## Target audience\n- Operators.\n\n## Core pain points\n- Coordination overhead.\n\n## Positioning angle\nTandem centralizes orchestration.\n\n## Competitor context\nLocal-only comparison for this run.\n\n## Proof points with citations\n1. Supported from docs/source.md. Source note: https://example.com/reference\n\n## Likely objections\n- Proof depth.\n\n## Channel considerations\n- Landing page.\n\n## Recommended message hierarchy\n1. Problem\n2. Promise\n\n## Files reviewed\n- docs/source.md\n\n## Files not reviewed\n- docs/extra.md: not needed for this first pass.\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &brief_text).expect("seed brief");
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let mut session = Session::new(
        Some("research-web-failure".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"docs/source.md"}),
                result: Some(json!({"output":"source"})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "websearch".to_string(),
                args: json!({"query":"tandem competitor landscape"}),
                result: Some(json!({
                    "output": "Authorization required for `websearch`.",
                    "metadata": { "error": "authorization required" }
                })),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({"path":"marketing-brief.md","content":brief_text}),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));
    let requested_tools = vec![
        "glob".to_string(),
        "read".to_string(),
        "websearch".to_string(),
        "write".to_string(),
    ];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done\n\n{\"status\":\"completed\"}",
        &tool_telemetry,
        None,
        Some(("marketing-brief.md".to_string(), brief_text.clone())),
        &std::collections::BTreeSet::new(),
    );

    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        None
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("external_research_mode")
            .and_then(Value::as_str),
        Some("waived_unavailable")
    );
    assert!(!artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| { value.as_str() == Some("missing_successful_web_research") })));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_brief_passes_local_only_when_websearch_is_not_offered() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-research-local-only-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let brief_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n## Campaign goal\nClarify positioning.\n\n## Target audience\n- Operators.\n\n## Core pain points\n- Coordination overhead.\n\n## Positioning angle\nTandem centralizes orchestration.\n\n## Competitor context\nLocal-only comparison for this run.\n\n## Proof points with citations\n1. Supported from docs/source.md. Source note: https://example.com/reference\n\n## Likely objections\n- Proof depth.\n\n## Channel considerations\n- Landing page.\n\n## Recommended message hierarchy\n1. Problem\n2. Promise\n\n## Files reviewed\n- docs/source.md\n\n## Files not reviewed\n- docs/extra.md: not needed for this first pass.\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &brief_text).expect("seed brief");
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let mut session = Session::new(
        Some("research-local-only".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"docs/source.md"}),
                result: Some(json!({"output":"source"})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({"path":"marketing-brief.md","content":brief_text}),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));
    let requested_tools = vec!["glob".to_string(), "read".to_string(), "write".to_string()];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done\n\n{\"status\":\"completed\"}",
        &tool_telemetry,
        None,
        Some(("marketing-brief.md".to_string(), brief_text.clone())),
        &std::collections::BTreeSet::new(),
    );

    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("external_research_mode")
            .and_then(Value::as_str),
        Some("waived_unavailable")
    );

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn research_brief_passes_when_source_audit_uses_markdown_tables() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-research-table-audit-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("docs")).expect("create workspace");
    let brief_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n### Files Reviewed\n| Local Path | Evidence Summary |\n|---|---|\n| `docs/source.md` | Core source reviewed |\n\n### Files Not Reviewed\n| Local Path | Reason |\n|---|---|\n| `docs/extra.md` | Out of scope for this run |\n\n### Web Sources Reviewed\n| URL | Status | Notes |\n|---|---|---|\n| https://example.com | Fetched | Confirmed live |\n\n## Campaign goal\nClarify positioning.\n\n## Target audience\n- Operators.\n\n## Core pain points\n- Coordination overhead.\n\n## Positioning angle\nTandem centralizes orchestration.\n\n## Competitor context\nLocal-only comparison for this run.\n\n## Proof points with citations\n1. Supported from docs/source.md. Source note: https://example.com/reference\n\n## Likely objections\n- Proof depth.\n\n## Channel considerations\n- Landing page.\n\n## Recommended message hierarchy\n1. Problem\n2. Promise\n".to_string();
    std::fs::write(workspace_root.join("marketing-brief.md"), &brief_text).expect("seed brief");
    let node = AutomationFlowNode {
        node_id: "research-brief".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Write marketing brief".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: None,
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
                "output_path": "marketing-brief.md",
                "web_research_expected": true
            }
        })),
    };
    let mut session = Session::new(
        Some("research-table-audit".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"docs/source.md"}),
                result: Some(json!({"output":"source"})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "websearch".to_string(),
                args: json!({"query":"tandem competitor landscape"}),
                result: Some(json!({
                    "output": "Authorization required for `websearch`.",
                    "metadata": { "error": "authorization required" }
                })),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "write".to_string(),
                args: json!({"path":"marketing-brief.md","content":brief_text}),
                result: Some(json!({"ok": true})),
                error: None,
            },
        ],
    ));
    let requested_tools = vec![
        "glob".to_string(),
        "read".to_string(),
        "websearch".to_string(),
        "write".to_string(),
    ];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "Done\n\n{\"status\":\"completed\"}",
        &tool_telemetry,
        None,
        Some(("marketing-brief.md".to_string(), brief_text.clone())),
        &std::collections::BTreeSet::new(),
    );

    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        None
    );
    assert_eq!(
        artifact_validation
            .get("web_sources_reviewed_present")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(artifact_validation
        .get("reviewed_paths_backed_by_read")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("docs/source.md"))));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn structured_handoff_nodes_fail_when_only_fallback_tool_summary_is_returned() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-structured-handoff-fallback-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let node = AutomationFlowNode {
        node_id: "research-discover-sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Discover source corpus".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: Some(crate::AutomationOutputEnforcement {
                validation_profile: Some("local_research".to_string()),
                required_tools: vec!["read".to_string()],
                required_evidence: vec!["local_source_reads".to_string()],
                required_sections: Vec::new(),
                prewrite_gates: vec![
                    "workspace_inspection".to_string(),
                    "concrete_reads".to_string(),
                ],
                retry_on_missing: vec![
                    "local_source_reads".to_string(),
                    "workspace_inspection".to_string(),
                    "concrete_reads".to_string(),
                ],
                terminal_on: Vec::new(),
                repair_budget: Some(5),
                session_text_recovery: Some("require_prewrite_satisfied".to_string()),
            }),
            schema: None,
            summary_guidance: Some("Return a structured handoff.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(
        Some("structured-handoff-fallback".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "read".to_string(),
            args: json!({"path":"tandem-reference/SOURCES.md"}),
            result: Some(json!({"output":"# Sources"})),
            error: None,
        }],
    ));
    let requested_tools = vec!["glob".to_string(), "read".to_string()];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "I completed project analysis steps using tools, but the model returned no final narrative text.\n\nTool result summary:\nTool `read` result:\n# Sources",
        &tool_telemetry,
        None,
        None,
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(
        rejected.as_deref(),
        Some("structured handoff was not returned in the final response")
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("needs_repair")
    );
    assert!(artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("structured_handoff_missing"))));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn structured_handoff_missing_is_repairable_even_without_enforcement_metadata() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-structured-handoff-defaults-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let node = AutomationFlowNode {
        node_id: "research-discover-sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Discover source corpus".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(
        Some("structured-handoff-defaults".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![
            MessagePart::ToolInvocation {
                tool: "glob".to_string(),
                args: json!({"pattern":"**/*.md"}),
                result: Some(json!({"output":"README.md"})),
                error: None,
            },
            MessagePart::ToolInvocation {
                tool: "read".to_string(),
                args: json!({"path":"tandem-reference/SOURCES.md"}),
                result: Some(json!({"output":"# Sources"})),
                error: None,
            },
        ],
    ));
    let requested_tools = vec!["glob".to_string(), "read".to_string()];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "I completed project analysis steps using tools, but the model returned no final narrative text.\n\nTool result summary:\nTool `read` result:\n# Sources",
        &tool_telemetry,
        None,
        None,
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(
        rejected.as_deref(),
        Some("structured handoff was not returned in the final response")
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("needs_repair")
    );
    assert_eq!(
        artifact_validation
            .get("blocking_classification")
            .and_then(Value::as_str),
        Some("handoff_missing")
    );
    assert!(artifact_validation
        .get("required_next_tool_actions")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value
            .as_str()
            .is_some_and(|text| text.contains("structured JSON handoff")))));

    let output: Value = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        "sess-structured-handoff-defaults",
        Some("run-structured-handoff-defaults"),
        "I completed project analysis steps using tools, but the model returned no final narrative text.\n\nTool result summary:\nTool `read` result:\n# Sources",
        None,
        Some(artifact_validation),
    );
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("needs_repair")
    );
    assert_eq!(
        output.get("failure_kind").and_then(Value::as_str),
        Some("structured_handoff_missing")
    );

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn structured_handoff_nodes_require_concrete_reads_without_output_path() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-structured-handoff-reads-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");
    let node = AutomationFlowNode {
        node_id: "research-local-sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Read prioritized sources".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: Some(crate::AutomationOutputEnforcement {
                validation_profile: Some("local_research".to_string()),
                required_tools: vec!["read".to_string()],
                required_evidence: vec!["local_source_reads".to_string()],
                required_sections: Vec::new(),
                prewrite_gates: vec!["concrete_reads".to_string()],
                retry_on_missing: vec![
                    "local_source_reads".to_string(),
                    "concrete_reads".to_string(),
                ],
                terminal_on: Vec::new(),
                repair_budget: Some(5),
                session_text_recovery: Some("require_prewrite_satisfied".to_string()),
            }),
            schema: None,
            summary_guidance: Some("Return a structured handoff.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: None,
    };
    let session = Session::new(
        Some("structured-handoff-missing-read".to_string()),
        Some(
            workspace_root
                .to_str()
                .expect("workspace root string")
                .to_string(),
        ),
    );
    let requested_tools = vec!["read".to_string()];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let (_accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        "{\"read_paths\":[\"tandem-reference/readmes/repo-README.md\"],\"reviewed_facts\":[\"Tandem is an engine-owned workflow runtime.\"],\"files_reviewed\":[\"tandem-reference/readmes/repo-README.md\"],\"files_not_reviewed\":[],\"citations_local\":[\"tandem-reference/readmes/repo-README.md\"]}\n\n{\"status\":\"completed\"}",
        &tool_telemetry,
        None,
        None,
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(
        rejected.as_deref(),
        Some("structured handoff completed without required concrete file reads")
    );
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("needs_repair")
    );
    assert!(artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("no_concrete_reads"))));
    assert!(artifact_validation
        .get("unmet_requirements")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("concrete_read_required"))));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn wrap_automation_node_output_includes_parsed_structured_handoff() {
    let node = AutomationFlowNode {
        node_id: "research-discover-sources".to_string(),
        agent_id: "researcher".to_string(),
        objective: "Discover source corpus".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Return a structured handoff.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: None,
    };
    let mut session = Session::new(Some("structured-handoff-wrap".to_string()), None);
    session.messages.push(tandem_types::Message::new(
        MessageRole::Assistant,
        vec![MessagePart::ToolInvocation {
            tool: "read".to_string(),
            args: json!({"path":"tandem-reference/SOURCES.md"}),
            result: Some(json!({"output":"# Sources"})),
            error: None,
        }],
    ));

    let output: Value = wrap_automation_node_output(
        &node,
        &session,
        &["read".to_string()],
        "sess-structured-handoff-wrap",
        Some("run-structured-handoff-wrap"),
        "Structured handoff ready.\n\n```json\n{\"workspace_inventory_summary\":\"Marketing source bundle found\",\"priority_paths\":[\"tandem-reference/SOURCES.md\"],\"discovered_paths\":[\"tandem-reference/SOURCES.md\"],\"skipped_paths_initial\":[]}\n```\n\n{\"status\":\"completed\"}",
        None,
        Some(json!({})),
    );

    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .get("content")
            .and_then(|value| value.get("structured_handoff"))
            .and_then(|value| value.get("workspace_inventory_summary"))
            .and_then(Value::as_str),
        Some("Marketing source bundle found")
    );
    assert_eq!(
        output
            .get("provenance")
            .and_then(|value| value.get("run_id"))
            .and_then(Value::as_str),
        Some("run-structured-handoff-wrap")
    );
    assert!(output
        .get("content")
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str)
        .is_some_and(|text| text.contains("\"priority_paths\"")));
}
