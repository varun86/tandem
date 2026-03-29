// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use std::collections::BTreeMap;

use crate::plan_package::{
    AuditScope, ConnectorBindingResolutionReport, ContextObject, CredentialEnvelope, DataScope,
    DependencyResolution, InterRoutinePolicy, ModelRoutingReport, OutputRoots, PlanLifecycleState,
    PlanOwner, PlanPackage, RoutineConnectorResolution, RoutinePackage, RoutineSemanticKind,
    StepProvenance, SuccessCriteria, TriggerDefinition,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutineScopeSnapshot {
    pub routine_id: String,
    pub semantic_kind: RoutineSemanticKind,
    pub trigger: TriggerDefinition,
    pub dependency_resolution: DependencyResolution,
    pub connector_resolution: RoutineConnectorResolution,
    pub data_scope: DataScope,
    pub audit_scope: AuditScope,
    pub success_criteria: SuccessCriteria,
    #[serde(default)]
    pub step_ids: Vec<String>,
    #[serde(default)]
    pub context_object_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanScopeSnapshot {
    pub plan_id: String,
    pub plan_revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_roots: Option<OutputRoots>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_routine_policy: Option<InterRoutinePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_enforcement: Option<crate::plan_package::BudgetEnforcement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_binding_resolution: Option<ConnectorBindingResolutionReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_routing_resolution: Option<ModelRoutingReport>,
    #[serde(default)]
    pub credential_envelopes: Vec<CredentialEnvelope>,
    #[serde(default)]
    pub context_objects: Vec<ContextObject>,
    #[serde(default)]
    pub routine_scopes: Vec<RoutineScopeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanPackageExportBundle {
    pub bundle_version: String,
    pub plan: PlanPackage,
    pub scope_snapshot: PlanScopeSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanPackageImportBundle {
    pub bundle_version: String,
    pub plan: PlanPackage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_snapshot: Option<PlanScopeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanPackageImportPreview {
    pub plan_package: PlanPackage,
    pub derived_scope_snapshot: PlanScopeSnapshot,
    pub source_bundle_digest: String,
    #[serde(default)]
    pub import_transform_log: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanReplayIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanReplayDiffEntry {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_value: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_value: Option<serde_json::Value>,
    pub blocking: bool,
    pub preserved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanReplayReport {
    pub compatible: bool,
    pub scope_metadata_preserved: bool,
    pub handoff_rules_preserved: bool,
    pub credential_isolation_preserved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_plan_revision: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_plan_revision: Option<u32>,
    #[serde(default)]
    pub diff_summary: Vec<PlanReplayDiffEntry>,
    #[serde(default)]
    pub issues: Vec<PlanReplayIssue>,
}

pub fn export_plan_package_bundle(plan: &PlanPackage) -> PlanPackageExportBundle {
    PlanPackageExportBundle {
        bundle_version: "1".to_string(),
        scope_snapshot: plan_scope_snapshot(plan),
        plan: plan.clone(),
    }
}

pub fn validate_plan_package_bundle(bundle: &PlanPackageImportBundle) -> PlanReplayReport {
    let mut issues = Vec::new();
    let derived_snapshot = plan_scope_snapshot(&bundle.plan);
    let Some(provided_snapshot) = bundle.scope_snapshot.clone() else {
        issues.push(PlanReplayIssue {
            code: "missing_scope_snapshot".to_string(),
            path: "scope_snapshot".to_string(),
            message: "Imported bundles must include an explicit scope snapshot.".to_string(),
            blocking: true,
        });
        return PlanReplayReport {
            compatible: false,
            scope_metadata_preserved: false,
            handoff_rules_preserved: false,
            credential_isolation_preserved: false,
            previous_plan_id: None,
            next_plan_id: None,
            previous_plan_revision: None,
            next_plan_revision: None,
            diff_summary: Vec::new(),
            issues,
        };
    };

    if bundle.bundle_version.trim() != "1" {
        issues.push(PlanReplayIssue {
            code: "unsupported_bundle_version".to_string(),
            path: "bundle_version".to_string(),
            message: "Only bundle version `1` is supported for structural imports.".to_string(),
            blocking: true,
        });
    }

    if !matches!(
        bundle.plan.lifecycle_state,
        PlanLifecycleState::Preview | PlanLifecycleState::Draft
    ) {
        issues.push(PlanReplayIssue {
            code: "import_requires_preview_lifecycle".to_string(),
            path: "plan.lifecycle_state".to_string(),
            message:
                "Imported bundles must represent a non-runnable draft or preview plan package."
                    .to_string(),
            blocking: true,
        });
    }

    if provided_snapshot != derived_snapshot {
        issues.push(PlanReplayIssue {
            code: "scope_snapshot_mismatch".to_string(),
            path: "scope_snapshot".to_string(),
            message:
                "Imported bundle scope metadata must match the plan package's derived scope snapshot."
                    .to_string(),
            blocking: true,
        });
    }
    PlanReplayReport {
        compatible: issues.is_empty(),
        scope_metadata_preserved: issues.is_empty(),
        handoff_rules_preserved: issues.is_empty(),
        credential_isolation_preserved: issues.is_empty(),
        previous_plan_id: None,
        next_plan_id: None,
        previous_plan_revision: None,
        next_plan_revision: None,
        diff_summary: Vec::new(),
        issues,
    }
}

pub fn preview_plan_package_import_bundle(
    bundle: &PlanPackageImportBundle,
    workspace_root: &str,
    creator_id: &str,
) -> PlanPackageImportPreview {
    let source_bundle_digest = source_bundle_digest(bundle);
    let mut plan_package = bundle.plan.clone();
    let original_plan_id = plan_package.plan_id.clone();
    let sanitized_plan_id = format!(
        "imported-{}",
        source_bundle_digest.chars().take(12).collect::<String>()
    );

    plan_package.plan_id = sanitized_plan_id.clone();
    plan_package.plan_revision = 1;
    plan_package.lifecycle_state = PlanLifecycleState::Preview;
    plan_package.owner = PlanOwner {
        owner_id: creator_id.to_string(),
        scope: "workspace".to_string(),
        audience: "internal".to_string(),
    };
    plan_package.output_roots = Some(OutputRoots {
        plan: Some(re_root_path(workspace_root, "knowledge/workflows/plan")),
        history: Some(re_root_path(
            workspace_root,
            "knowledge/workflows/run-history",
        )),
        proof: Some(re_root_path(workspace_root, "knowledge/workflows/proof")),
        drafts: Some(re_root_path(workspace_root, "knowledge/workflows/drafts")),
    });
    plan_package.validation_state = None;
    plan_package.connector_bindings.clear();
    plan_package.connector_binding_resolution =
        Some(crate::plan_package::derive_connector_binding_resolution_for_plan(&plan_package));
    plan_package.model_routing_resolution =
        Some(crate::plan_package::derive_model_routing_resolution_for_plan(&plan_package));
    plan_package.credential_envelopes =
        crate::plan_package::derive_credential_envelopes_for_plan(&plan_package);

    for routine in &mut plan_package.routine_graph {
        for step in &mut routine.steps {
            step.provenance = match step.provenance.take() {
                Some(mut provenance) => {
                    provenance.plan_id = Some(sanitized_plan_id.clone());
                    provenance.routine_id = Some(routine.routine_id.clone());
                    provenance.step_id = Some(step.step_id.clone());
                    Some(provenance)
                }
                None => Some(StepProvenance {
                    plan_id: Some(sanitized_plan_id.clone()),
                    routine_id: Some(routine.routine_id.clone()),
                    step_id: Some(step.step_id.clone()),
                    cost_provenance: None,
                }),
            };
        }
    }

    for context_object in &mut plan_package.context_objects {
        context_object.provenance.plan_id = sanitized_plan_id.clone();
    }

    let transform_log = vec![
        format!("source plan id `{original_plan_id}` re-rooted to `{sanitized_plan_id}`"),
        format!("owner re-rooted to `{creator_id}`"),
        format!("output roots re-rooted to `{workspace_root}`"),
        "connector bindings cleared for local preview".to_string(),
        "credential envelopes recomputed from the sanitized preview".to_string(),
    ];

    let mut metadata = plan_package.metadata.take().unwrap_or_else(|| json!({}));
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "import".to_string(),
            json!({
                "source_bundle_digest": source_bundle_digest,
                "source_plan_id": original_plan_id,
                "source_bundle_version": bundle.bundle_version,
                "sanitized_plan_id": sanitized_plan_id,
                "workspace_root": workspace_root,
                "creator_id": creator_id,
                "mode": "sanitized_local_preview"
            }),
        );
    } else {
        metadata = json!({
            "import": {
                "source_bundle_digest": source_bundle_digest,
                "source_plan_id": original_plan_id,
                "source_bundle_version": bundle.bundle_version,
                "sanitized_plan_id": sanitized_plan_id,
                "workspace_root": workspace_root,
                "creator_id": creator_id,
                "mode": "sanitized_local_preview"
            }
        });
    }
    plan_package.metadata = Some(metadata);

    let derived_scope_snapshot = plan_scope_snapshot(&plan_package);
    PlanPackageImportPreview {
        plan_package,
        derived_scope_snapshot,
        source_bundle_digest,
        import_transform_log: transform_log,
    }
}

fn source_bundle_digest(bundle: &PlanPackageImportBundle) -> String {
    let bytes = serde_json::to_vec(bundle).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn re_root_path(workspace_root: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        workspace_root.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    )
}

pub fn compare_plan_package_replay(previous: &PlanPackage, next: &PlanPackage) -> PlanReplayReport {
    let previous_snapshot = plan_scope_snapshot(previous);
    let next_snapshot = plan_scope_snapshot(next);
    let mut issues = Vec::new();

    let mut scope_metadata_preserved = true;
    let mut handoff_rules_preserved = true;
    let mut credential_isolation_preserved = true;
    let mut diff_summary = Vec::new();

    if previous.plan_id != next.plan_id {
        issues.push(PlanReplayIssue {
            code: "plan_id_changed".to_string(),
            path: "plan_id".to_string(),
            message: "Revisions must keep the same plan id to preserve replay lineage.".to_string(),
            blocking: true,
        });
        scope_metadata_preserved = false;
        handoff_rules_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "plan_id".to_string(),
            previous_value: Some(json!(previous.plan_id)),
            next_value: Some(json!(next.plan_id)),
            blocking: true,
            preserved: false,
        });
    }

    if previous.plan_revision != next.plan_revision {
        diff_summary.push(PlanReplayDiffEntry {
            path: "plan_revision".to_string(),
            previous_value: Some(json!(previous.plan_revision)),
            next_value: Some(json!(next.plan_revision)),
            blocking: false,
            preserved: false,
        });
    }

    if previous.output_roots != next.output_roots {
        issues.push(PlanReplayIssue {
            code: "output_roots_changed".to_string(),
            path: "output_roots".to_string(),
            message: "Replay requires preserved output roots so stored artifacts remain reachable."
                .to_string(),
            blocking: true,
        });
        scope_metadata_preserved = false;
        handoff_rules_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "output_roots".to_string(),
            previous_value: Some(json!(previous.output_roots)),
            next_value: Some(json!(next.output_roots)),
            blocking: true,
            preserved: false,
        });
    }

    if previous.inter_routine_policy != next.inter_routine_policy {
        issues.push(PlanReplayIssue {
            code: "inter_routine_policy_changed".to_string(),
            path: "inter_routine_policy".to_string(),
            message: "Replay requires the inter-routine handoff policy to remain stable."
                .to_string(),
            blocking: true,
        });
        handoff_rules_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "inter_routine_policy".to_string(),
            previous_value: Some(json!(previous.inter_routine_policy)),
            next_value: Some(json!(next.inter_routine_policy)),
            blocking: true,
            preserved: false,
        });
    }

    if previous.approval_policy != next.approval_policy {
        issues.push(PlanReplayIssue {
            code: "approval_policy_changed".to_string(),
            path: "approval_policy".to_string(),
            message:
                "Replay requires the approval policy matrix to remain stable across revisions."
                    .to_string(),
            blocking: true,
        });
        scope_metadata_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "approval_policy".to_string(),
            previous_value: Some(json!(previous.approval_policy)),
            next_value: Some(json!(next.approval_policy)),
            blocking: true,
            preserved: false,
        });
    }

    if previous.connector_bindings != next.connector_bindings {
        issues.push(PlanReplayIssue {
            code: "connector_bindings_changed".to_string(),
            path: "connector_bindings".to_string(),
            message: "Replay requires connector bindings to remain stable across plan revisions."
                .to_string(),
            blocking: true,
        });
        scope_metadata_preserved = false;
        credential_isolation_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "connector_bindings".to_string(),
            previous_value: Some(json!(previous.connector_bindings)),
            next_value: Some(json!(next.connector_bindings)),
            blocking: true,
            preserved: false,
        });
    }

    if previous_snapshot.model_routing_resolution != next_snapshot.model_routing_resolution {
        issues.push(PlanReplayIssue {
            code: "model_routing_resolution_changed".to_string(),
            path: "model_routing_resolution".to_string(),
            message: "Replay requires step-level model routing to remain stable across revisions."
                .to_string(),
            blocking: true,
        });
        scope_metadata_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "model_routing_resolution".to_string(),
            previous_value: Some(json!(previous_snapshot.model_routing_resolution)),
            next_value: Some(json!(next_snapshot.model_routing_resolution)),
            blocking: true,
            preserved: false,
        });
    }

    let previous_routines = routine_scope_map(&previous_snapshot.routine_scopes);
    let next_routines = routine_scope_map(&next_snapshot.routine_scopes);

    for (routine_id, previous_routine) in &previous_routines {
        let Some(next_routine) = next_routines.get(routine_id) else {
            issues.push(PlanReplayIssue {
                code: "routine_missing_in_replay".to_string(),
                path: format!("routine_scopes.{routine_id}"),
                message: format!(
                    "Routine `{}` is missing from the replayed plan revision.",
                    routine_id
                ),
                blocking: true,
            });
            scope_metadata_preserved = false;
            handoff_rules_preserved = false;
            continue;
        };

        if previous_routine != next_routine {
            issues.push(PlanReplayIssue {
                code: "routine_scope_changed".to_string(),
                path: format!("routine_scopes.{routine_id}"),
                message: format!(
                    "Routine `{}` changed its scope, trigger, dependency, connector, or step contract.",
                    routine_id
                ),
                blocking: true,
            });
            scope_metadata_preserved = false;
            handoff_rules_preserved = false;
            diff_summary.push(PlanReplayDiffEntry {
                path: format!("routine_scopes.{routine_id}"),
                previous_value: Some(json!(previous_routine)),
                next_value: Some(json!(next_routine)),
                blocking: true,
                preserved: false,
            });
        }
    }

    for (routine_id, next_routine) in &next_routines {
        if !previous_routines.contains_key(routine_id) {
            issues.push(PlanReplayIssue {
                code: "new_routine_added".to_string(),
                path: format!("routine_scopes.{routine_id}"),
                message: format!(
                    "Routine `{}` was added in the replayed plan revision and must be reviewed.",
                    routine_id
                ),
                blocking: true,
            });
            scope_metadata_preserved = false;
            handoff_rules_preserved = false;
            diff_summary.push(PlanReplayDiffEntry {
                path: format!("routine_scopes.{routine_id}"),
                previous_value: None,
                next_value: Some(json!(next_routine)),
                blocking: true,
                preserved: false,
            });
        }

        if next_routine
            .data_scope
            .writable_paths
            .iter()
            .any(|path| path.trim().is_empty())
        {
            issues.push(PlanReplayIssue {
                code: "invalid_writable_scope".to_string(),
                path: format!("routine_scopes.{routine_id}.data_scope.writable_paths"),
                message: format!(
                    "Routine `{}` contains empty writable scope entries after replay.",
                    routine_id
                ),
                blocking: true,
            });
            scope_metadata_preserved = false;
            diff_summary.push(PlanReplayDiffEntry {
                path: format!("routine_scopes.{routine_id}.data_scope.writable_paths"),
                previous_value: Some(json!(previous_routines
                    .get(routine_id)
                    .map(|routine| { routine.data_scope.writable_paths.clone() }))),
                next_value: Some(json!(next_routine.data_scope.writable_paths.clone())),
                blocking: true,
                preserved: false,
            });
        }
    }

    if previous_snapshot.credential_envelopes != next_snapshot.credential_envelopes {
        issues.push(PlanReplayIssue {
            code: "credential_envelope_changed".to_string(),
            path: "credential_envelopes".to_string(),
            message: "Replay requires credential envelopes to remain stable across plan revisions."
                .to_string(),
            blocking: true,
        });
        credential_isolation_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "credential_envelopes".to_string(),
            previous_value: Some(json!(previous_snapshot.credential_envelopes)),
            next_value: Some(json!(next_snapshot.credential_envelopes)),
            blocking: true,
            preserved: false,
        });
    }

    if previous_snapshot.context_objects != next_snapshot.context_objects {
        issues.push(PlanReplayIssue {
            code: "context_object_changed".to_string(),
            path: "context_objects".to_string(),
            message:
                "Replay requires context objects and handoff artifacts to remain stable across revisions."
                    .to_string(),
            blocking: true,
        });
        scope_metadata_preserved = false;
        handoff_rules_preserved = false;
        diff_summary.push(PlanReplayDiffEntry {
            path: "context_objects".to_string(),
            previous_value: Some(json!(previous_snapshot.context_objects)),
            next_value: Some(json!(next_snapshot.context_objects)),
            blocking: true,
            preserved: false,
        });
    }

    PlanReplayReport {
        compatible: issues.is_empty(),
        scope_metadata_preserved,
        handoff_rules_preserved,
        credential_isolation_preserved,
        previous_plan_id: Some(previous.plan_id.clone()),
        next_plan_id: Some(next.plan_id.clone()),
        previous_plan_revision: Some(previous.plan_revision),
        next_plan_revision: Some(next.plan_revision),
        diff_summary,
        issues,
    }
}

pub fn plan_scope_snapshot(plan: &PlanPackage) -> PlanScopeSnapshot {
    let context_objects = {
        let mut objects = plan.context_objects.clone();
        objects.sort_by(|left, right| left.context_object_id.cmp(&right.context_object_id));
        objects
    };
    let mut routine_scopes = plan
        .routine_graph
        .iter()
        .map(|routine| routine_scope_snapshot(routine, &context_objects))
        .collect::<Vec<_>>();
    routine_scopes.sort_by(|left, right| left.routine_id.cmp(&right.routine_id));

    let mut credential_envelopes = plan.credential_envelopes.clone();
    credential_envelopes.sort_by(|left, right| left.routine_id.cmp(&right.routine_id));

    let mut context_objects = plan.context_objects.clone();
    context_objects.sort_by(|left, right| left.context_object_id.cmp(&right.context_object_id));

    PlanScopeSnapshot {
        plan_id: plan.plan_id.clone(),
        plan_revision: plan.plan_revision,
        output_roots: plan.output_roots.clone(),
        inter_routine_policy: plan.inter_routine_policy.clone(),
        budget_enforcement: plan.budget_enforcement.clone(),
        connector_binding_resolution: plan.connector_binding_resolution.clone(),
        model_routing_resolution: plan.model_routing_resolution.clone(),
        credential_envelopes,
        context_objects,
        routine_scopes,
    }
}

fn routine_scope_snapshot(
    routine: &RoutinePackage,
    context_objects: &[ContextObject],
) -> RoutineScopeSnapshot {
    let mut step_ids = routine
        .steps
        .iter()
        .map(|step| step.step_id.clone())
        .collect::<Vec<_>>();
    step_ids.sort();

    let mut context_object_ids = context_objects
        .iter()
        .filter(|context| context.owner_routine_id == routine.routine_id)
        .map(|context| context.context_object_id.clone())
        .collect::<Vec<_>>();
    context_object_ids.sort();
    context_object_ids.dedup();

    RoutineScopeSnapshot {
        routine_id: routine.routine_id.clone(),
        semantic_kind: routine.semantic_kind.clone(),
        trigger: routine.trigger.clone(),
        dependency_resolution: routine.dependency_resolution.clone(),
        connector_resolution: routine.connector_resolution.clone(),
        data_scope: routine.data_scope.clone(),
        audit_scope: routine.audit_scope.clone(),
        success_criteria: routine.success_criteria.clone(),
        step_ids,
        context_object_ids,
    }
}

fn routine_scope_map(snapshots: &[RoutineScopeSnapshot]) -> BTreeMap<String, RoutineScopeSnapshot> {
    snapshots
        .iter()
        .cloned()
        .map(|snapshot| (snapshot.routine_id.clone(), snapshot))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_package::{
        ApprovalMatrix, ApprovalMode, AuditScope, BudgetEnforcement, CommunicationModel,
        ContextObject, ContextObjectProvenance, ContextObjectScope, ContextValidationStatus,
        CredentialBindingRef, CredentialEnvelope, CrossRoutineVisibility, DataScope,
        DependencyResolution, DependencyResolutionStrategy, FinalArtifactVisibility,
        InterRoutinePolicy, IntermediateArtifactVisibility, MissionContextScope, MissionDefinition,
        OutputRoots, OverlapIdentity, OverlapPolicy, PeerVisibility, PlanLifecycleState, PlanOwner,
        PlanPackage, RoutineConnectorResolution, RoutinePackage, RoutineSemanticKind,
        RunHistoryVisibility, StepModelPolicy, StepPackage, SuccessCriteria, TriggerDefinition,
        TriggerKind,
    };

    fn sample_plan() -> PlanPackage {
        PlanPackage {
            plan_id: "plan_001".to_string(),
            plan_revision: 3,
            lifecycle_state: PlanLifecycleState::Preview,
            owner: PlanOwner {
                owner_id: "evan".to_string(),
                scope: "workspace".to_string(),
                audience: "internal".to_string(),
            },
            mission: MissionDefinition {
                goal: "test goal".to_string(),
                summary: None,
                domain: Some("workflow".to_string()),
            },
            success_criteria: SuccessCriteria::default(),
            budget_policy: None,
            budget_enforcement: Some(BudgetEnforcement {
                hard_limit_behavior: Some("pause_before_step".to_string()),
                partial_result_preservation: Some(true),
                ..BudgetEnforcement::default()
            }),
            approval_policy: Some(ApprovalMatrix {
                internal_reports: Some(ApprovalMode::AutoApproved),
                ..ApprovalMatrix::default()
            }),
            inter_routine_policy: Some(InterRoutinePolicy {
                communication_model: CommunicationModel::ArtifactOnly,
                shared_memory_access: false,
                shared_memory_justification: None,
                peer_visibility: PeerVisibility::DeclaredOutputsOnly,
                artifact_handoff_validation: true,
            }),
            trigger_policy: None,
            output_roots: Some(OutputRoots {
                plan: Some("knowledge/workflows/plan/".to_string()),
                history: Some("knowledge/workflows/run-history/".to_string()),
                proof: Some("knowledge/workflows/proof/".to_string()),
                drafts: Some("knowledge/workflows/drafts/".to_string()),
            }),
            precedence_log: Vec::new(),
            plan_diff: None,
            manual_trigger_record: None,
            validation_state: None,
            overlap_policy: Some(OverlapPolicy {
                exact_identity: Some(OverlapIdentity {
                    hash_version: Some(1),
                    canonical_hash: Some("hash".to_string()),
                    normalized_fields: vec!["goal".to_string()],
                }),
                semantic_identity: None,
                overlap_log: Vec::new(),
            }),
            routine_graph: vec![RoutinePackage {
                routine_id: "routine_a".to_string(),
                semantic_kind: RoutineSemanticKind::Research,
                trigger: TriggerDefinition {
                    trigger_type: TriggerKind::Scheduled,
                    schedule: Some("0 9 * * *".to_string()),
                    timezone: Some("UTC".to_string()),
                },
                dependencies: Vec::new(),
                dependency_resolution: DependencyResolution {
                    strategy: DependencyResolutionStrategy::TopologicalSequential,
                    partial_failure_mode: crate::plan_package::PartialFailureMode::PauseAll,
                    reentry_point: crate::plan_package::ReentryPoint::FailedStep,
                    mid_routine_connector_failure:
                        crate::plan_package::MidRoutineConnectorFailureMode::SurfaceAndPause,
                },
                connector_resolution: RoutineConnectorResolution::default(),
                data_scope: DataScope {
                    readable_paths: vec!["mission.goal".to_string()],
                    writable_paths: vec!["knowledge/workflows/plan/routine_a/**".to_string()],
                    denied_paths: vec!["credentials/**".to_string()],
                    cross_routine_visibility: CrossRoutineVisibility::DeclaredOutputsOnly,
                    mission_context_scope: MissionContextScope::GoalAndOwnRoutine,
                    mission_context_justification: None,
                },
                audit_scope: AuditScope {
                    run_history_visibility: RunHistoryVisibility::PlanOwner,
                    named_audit_roles: Vec::new(),
                    intermediate_artifact_visibility: IntermediateArtifactVisibility::RoutineOnly,
                    final_artifact_visibility: FinalArtifactVisibility::DeclaredConsumers,
                },
                success_criteria: SuccessCriteria {
                    required_artifacts: vec!["routine_a.md".to_string()],
                    minimum_viable_completion: None,
                    minimum_output: Some("one draft".to_string()),
                    freshness_window_hours: Some(24),
                },
                steps: vec![StepPackage {
                    step_id: "step_a".to_string(),
                    label: "Step A".to_string(),
                    kind: "research".to_string(),
                    action: "do the work".to_string(),
                    inputs: vec!["mission.goal".to_string()],
                    outputs: vec!["step_a:brief".to_string()],
                    dependencies: Vec::new(),
                    context_reads: Vec::new(),
                    context_writes: Vec::new(),
                    connector_requirements: Vec::new(),
                    model_policy: StepModelPolicy::default(),
                    approval_policy: ApprovalMode::InternalOnly,
                    success_criteria: SuccessCriteria {
                        required_artifacts: vec!["step_a.artifact".to_string()],
                        minimum_viable_completion: None,
                        minimum_output: None,
                        freshness_window_hours: None,
                    },
                    failure_policy: Default::default(),
                    retry_policy: Default::default(),
                    artifacts: vec!["step_a.artifact".to_string()],
                    provenance: None,
                    notes: None,
                }],
            }],
            connector_intents: Vec::new(),
            connector_bindings: Vec::new(),
            connector_binding_resolution: None,
            model_routing_resolution: None,
            credential_envelopes: vec![CredentialEnvelope {
                routine_id: "routine_a".to_string(),
                entitled_connectors: vec![CredentialBindingRef {
                    capability: "search".to_string(),
                    binding_id: "binding_search".to_string(),
                }],
                denied_connectors: Vec::new(),
                envelope_issued_at: None,
                envelope_expires_at: None,
                issuing_authority: Some("engine".to_string()),
            }],
            context_objects: vec![ContextObject {
                context_object_id: "ctx:routine_a:mission.goal".to_string(),
                name: "Mission goal".to_string(),
                kind: "mission_goal".to_string(),
                scope: ContextObjectScope::Mission,
                owner_routine_id: "routine_a".to_string(),
                producer_step_id: None,
                declared_consumers: vec!["routine_a".to_string()],
                artifact_ref: None,
                data_scope_refs: vec!["mission.goal".to_string()],
                freshness_window_hours: None,
                validation_status: ContextValidationStatus::Pending,
                provenance: ContextObjectProvenance {
                    plan_id: "plan_001".to_string(),
                    routine_id: "routine_a".to_string(),
                    step_id: None,
                },
                summary: Some("test goal".to_string()),
            }],
            metadata: None,
        }
    }

    #[test]
    fn export_bundle_preserves_scope_snapshot() {
        let plan = sample_plan();
        let bundle = export_plan_package_bundle(&plan);
        assert_eq!(bundle.bundle_version, "1");
        assert_eq!(bundle.plan, plan);
        assert_eq!(bundle.scope_snapshot.plan_id, "plan_001");
        assert_eq!(bundle.scope_snapshot.routine_scopes.len(), 1);
        assert_eq!(bundle.scope_snapshot.credential_envelopes.len(), 1);
        assert_eq!(bundle.scope_snapshot.context_objects.len(), 1);
        assert_eq!(
            bundle.scope_snapshot.connector_binding_resolution,
            bundle.plan.connector_binding_resolution
        );
        assert_eq!(
            bundle.scope_snapshot.model_routing_resolution,
            bundle.plan.model_routing_resolution
        );
        assert_eq!(
            bundle.scope_snapshot.budget_enforcement,
            bundle.plan.budget_enforcement
        );
        assert_eq!(
            bundle.scope_snapshot.routine_scopes[0].data_scope,
            bundle.plan.routine_graph[0].data_scope
        );
    }

    #[test]
    fn export_bundle_preserves_connector_binding_resolution() {
        let mut plan = sample_plan();
        plan.connector_intents = vec![crate::plan_package::ConnectorIntent {
            capability: "github".to_string(),
            why: "Needed for source control lookups".to_string(),
            required: true,
            degraded_mode_allowed: false,
        }];
        plan.connector_bindings = vec![crate::plan_package::ConnectorBinding {
            capability: "github".to_string(),
            binding_type: "mcp_server".to_string(),
            binding_id: "binding_github".to_string(),
            allowlist_pattern: Some("github.*".to_string()),
            status: "mapped".to_string(),
        }];
        plan.connector_binding_resolution =
            Some(crate::plan_package::derive_connector_binding_resolution_for_plan(&plan));

        let bundle = export_plan_package_bundle(&plan);

        let report = bundle
            .scope_snapshot
            .connector_binding_resolution
            .as_ref()
            .expect("connector binding resolution");
        assert_eq!(report.mapped_count, 1);
        assert_eq!(report.unresolved_required_count, 0);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].capability, "github");
        assert!(bundle
            .scope_snapshot
            .connector_binding_resolution
            .as_ref()
            .is_some());
    }

    #[test]
    fn export_bundle_preserves_model_routing_resolution() {
        let mut plan = sample_plan();
        plan.routine_graph[0].steps[0].model_policy.primary =
            Some(crate::plan_package::StepModelSelection {
                tier: crate::plan_package::ModelTier::Strong,
            });
        plan.model_routing_resolution =
            Some(crate::plan_package::derive_model_routing_resolution_for_plan(&plan));

        let bundle = export_plan_package_bundle(&plan);
        let report = bundle
            .scope_snapshot
            .model_routing_resolution
            .as_ref()
            .expect("model routing resolution");
        assert_eq!(report.tier_assigned_count, 1);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].step_id, "step_a");
    }

    #[test]
    fn import_bundle_validation_is_structural() {
        let plan = sample_plan();
        let bundle = PlanPackageImportBundle {
            bundle_version: "1".to_string(),
            scope_snapshot: Some(plan_scope_snapshot(&plan)),
            plan,
        };
        let report = validate_plan_package_bundle(&bundle);
        assert!(report.compatible);
        assert!(report.issues.is_empty());
        assert!(report.scope_metadata_preserved);
        assert!(report.handoff_rules_preserved);
        assert!(report.credential_isolation_preserved);
    }

    #[test]
    fn import_bundle_requires_scope_snapshot() {
        let bundle = PlanPackageImportBundle {
            bundle_version: "1".to_string(),
            scope_snapshot: None,
            plan: sample_plan(),
        };
        let report = validate_plan_package_bundle(&bundle);
        assert!(!report.compatible);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "missing_scope_snapshot"));
    }

    #[test]
    fn import_bundle_rejects_runnable_lifecycle_states() {
        let mut plan = sample_plan();
        plan.lifecycle_state = PlanLifecycleState::Applied;
        let bundle = PlanPackageImportBundle {
            bundle_version: "1".to_string(),
            scope_snapshot: Some(plan_scope_snapshot(&plan)),
            plan,
        };
        let report = validate_plan_package_bundle(&bundle);
        assert!(!report.compatible);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "import_requires_preview_lifecycle"));
    }

    #[test]
    fn preview_import_bundle_re_roots_lineage_and_recomputes_handoffs() {
        let plan = sample_plan();
        let bundle = PlanPackageImportBundle {
            bundle_version: "1".to_string(),
            scope_snapshot: Some(plan_scope_snapshot(&plan)),
            plan,
        };

        let preview = preview_plan_package_import_bundle(&bundle, "/workspace", "operator_1");

        assert_ne!(preview.plan_package.plan_id, bundle.plan.plan_id);
        assert_eq!(
            preview.plan_package.lifecycle_state,
            PlanLifecycleState::Preview
        );
        assert_eq!(preview.plan_package.owner.owner_id, "operator_1");
        assert_eq!(
            preview
                .plan_package
                .output_roots
                .as_ref()
                .and_then(|roots| roots.plan.as_deref()),
            Some("/workspace/knowledge/workflows/plan")
        );
        assert!(preview
            .plan_package
            .credential_envelopes
            .iter()
            .all(|envelope| envelope.routine_id == "routine_a"));
        assert!(preview
            .plan_package
            .context_objects
            .iter()
            .all(|context| context.provenance.plan_id == preview.plan_package.plan_id));
        assert_eq!(
            preview.derived_scope_snapshot.plan_id,
            preview.plan_package.plan_id
        );
        assert!(preview
            .import_transform_log
            .iter()
            .any(|entry| entry.contains("re-rooted")));
        assert!(preview
            .plan_package
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("import"))
            .and_then(|import| import.get("mode"))
            .and_then(|mode| mode.as_str())
            .is_some_and(|mode| mode == "sanitized_local_preview"));
    }

    #[test]
    fn replay_checks_flag_scope_and_handoff_changes() {
        let previous = sample_plan();
        let mut next = sample_plan();
        next.routine_graph[0].data_scope.writable_paths =
            vec!["knowledge/workflows/plan/routine_a/private/**".to_string()];
        next.context_objects[0].data_scope_refs =
            vec!["knowledge/workflows/plan/changed".to_string()];
        let report = compare_plan_package_replay(&previous, &next);
        assert!(!report.compatible);
        assert!(!report.scope_metadata_preserved);
        assert!(!report.handoff_rules_preserved);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "routine_scope_changed"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "context_object_changed"));
        assert!(report
            .previous_plan_id
            .as_deref()
            .is_some_and(|value| value == previous.plan_id));
        assert!(report
            .next_plan_id
            .as_deref()
            .is_some_and(|value| value == next.plan_id));
        assert_eq!(report.previous_plan_revision, Some(previous.plan_revision));
        assert_eq!(report.next_plan_revision, Some(next.plan_revision));
        assert!(report
            .diff_summary
            .iter()
            .any(|entry| entry.path == "context_objects"));
    }

    #[test]
    fn replay_checks_flag_connector_binding_changes() {
        let previous = sample_plan();
        let mut next = sample_plan();
        next.connector_bindings
            .push(crate::plan_package::ConnectorBinding {
                capability: "search".to_string(),
                binding_type: "mcp_server".to_string(),
                binding_id: "search-binding-2".to_string(),
                allowlist_pattern: Some("search.*".to_string()),
                status: "mapped".to_string(),
            });

        let report = compare_plan_package_replay(&previous, &next);

        assert!(!report.compatible);
        assert!(!report.credential_isolation_preserved);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "connector_bindings_changed"));
        assert!(report
            .diff_summary
            .iter()
            .any(|entry| entry.path == "connector_bindings"));
    }

    #[test]
    fn replay_checks_flag_approval_policy_changes() {
        let previous = sample_plan();
        let mut next = sample_plan();
        next.approval_policy = Some(ApprovalMatrix {
            public_posts: Some(ApprovalMode::ApprovalRequired),
            ..ApprovalMatrix::default()
        });

        let report = compare_plan_package_replay(&previous, &next);

        assert!(!report.compatible);
        assert!(!report.scope_metadata_preserved);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "approval_policy_changed"));
        assert!(report
            .diff_summary
            .iter()
            .any(|entry| entry.path == "approval_policy"));
    }

    #[test]
    fn replay_checks_flag_model_routing_changes() {
        let previous = sample_plan();
        let mut next = sample_plan();
        next.routine_graph[0].steps[0].model_policy.primary =
            Some(crate::plan_package::StepModelSelection {
                tier: crate::plan_package::ModelTier::Strong,
            });
        next.model_routing_resolution =
            Some(crate::plan_package::derive_model_routing_resolution_for_plan(&next));

        let report = compare_plan_package_replay(&previous, &next);

        assert!(!report.compatible);
        assert!(!report.scope_metadata_preserved);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "model_routing_resolution_changed"));
        assert!(report
            .diff_summary
            .iter()
            .any(|entry| entry.path == "model_routing_resolution"));
    }
}
