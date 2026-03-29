// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tandem_workflows::plan_package::{AutomationV2ScheduleType, WorkflowPlanStep};

use crate::plan_validation::validate_plan_package;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanLifecycleState {
    Draft,
    Preview,
    AwaitingApproval,
    Approved,
    Applied,
    Active,
    Degraded,
    Paused,
    Superseded,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutineSemanticKind {
    Research,
    Monitoring,
    Drafting,
    Review,
    Execution,
    Sync,
    Reporting,
    Publication,
    Remediation,
    Triage,
    Orchestration,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    Scheduled,
    Manual,
    EventDriven,
    ApprovalTriggered,
    ReleaseTriggered,
    ArtifactTriggered,
    DependencyTriggered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    InternalOnly,
    DraftOnly,
    ApprovalRequired,
    AutoApproved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyMode {
    Hard,
    Soft,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyResolutionStrategy {
    TopologicalSequential,
    TopologicalParallel,
    StrictSequential,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PartialFailureMode {
    ContinueIndependent,
    PauseDownstreamOnly,
    PauseAll,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReentryPoint {
    FailedStep,
    RoutineStart,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MidRoutineConnectorFailureMode {
    SurfaceAndPause,
    SurfaceAndDegrade,
    SurfaceAndBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CrossRoutineVisibility {
    None,
    DeclaredOutputsOnly,
    PlanOwnerOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissionContextScope {
    GoalOnly,
    GoalAndOwnRoutine,
    GoalAndDependencies,
    FullPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunHistoryVisibility {
    RoutineOnly,
    PlanOwner,
    NamedRoles,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntermediateArtifactVisibility {
    RoutineOnly,
    PlanOwner,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinalArtifactVisibility {
    RoutineOnly,
    DeclaredConsumers,
    PlanOwner,
    Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationModel {
    ArtifactOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerVisibility {
    None,
    GoalOnly,
    DeclaredOutputsOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Fast,
    Mid,
    Strong,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextObjectScope {
    Mission,
    Plan,
    Routine,
    Step,
    Handoff,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextValidationStatus {
    Pending,
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrecedenceSourceTier {
    CompilerDefault,
    UserOverride,
    ApprovedPlanState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanDiffChangeType {
    Add,
    Update,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManualTriggerSource {
    Calendar,
    Mission,
    Routine,
    Api,
    DryRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanOwner {
    pub owner_id: String,
    pub scope: String,
    pub audience: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MissionDefinition {
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SuccessCriteria {
    #[serde(default)]
    pub required_artifacts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_viable_completion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_window_hours: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextObjectProvenance {
    pub plan_id: String,
    pub routine_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextObject {
    pub context_object_id: String,
    pub name: String,
    pub kind: String,
    pub scope: ContextObjectScope,
    pub owner_routine_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_step_id: Option<String>,
    #[serde(default)]
    pub declared_consumers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_ref: Option<String>,
    #[serde(default)]
    pub data_scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_window_hours: Option<u32>,
    pub validation_status: ContextValidationStatus,
    pub provenance: ContextObjectProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrecedenceLogEntry {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiler_default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_override: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_plan_state: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_value: Option<Value>,
    pub source_tier: PrecedenceSourceTier,
    pub conflict_detected: bool,
    pub resolution_rule: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanDiffChangedField {
    pub path: String,
    pub change_type: PlanDiffChangeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_value: Option<Value>,
    pub requires_revalidation: bool,
    pub requires_reapproval: bool,
    pub breaking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PlanDiffSummary {
    #[serde(default)]
    pub changed_count: usize,
    #[serde(default)]
    pub breaking_count: usize,
    #[serde(default)]
    pub revalidation_required: bool,
    #[serde(default)]
    pub reapproval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanDiff {
    pub from_revision: u32,
    pub to_revision: u32,
    #[serde(default)]
    pub changed_fields: Vec<PlanDiffChangedField>,
    pub summary: PlanDiffSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManualTriggerRecord {
    pub trigger_id: String,
    pub plan_id: String,
    pub plan_revision: u32,
    pub routine_id: String,
    pub triggered_by: String,
    pub trigger_source: ManualTriggerSource,
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy_snapshot: Option<ApprovalMatrix>,
    #[serde(default)]
    pub connector_binding_snapshot: Vec<ConnectorBinding>,
    pub triggered_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default)]
    pub artifacts_produced: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerDefinition {
    #[serde(rename = "type")]
    pub trigger_type: TriggerKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutineDependency {
    #[serde(rename = "type")]
    pub dependency_type: String,
    pub routine_id: String,
    pub mode: DependencyMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DependencyResolution {
    pub strategy: DependencyResolutionStrategy,
    pub partial_failure_mode: PartialFailureMode,
    pub reentry_point: ReentryPoint,
    pub mid_routine_connector_failure: MidRoutineConnectorFailureMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RoutineConnectorResolution {
    #[serde(default)]
    pub states: Vec<String>,
    #[serde(default)]
    pub binding_options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataScope {
    #[serde(default)]
    pub readable_paths: Vec<String>,
    #[serde(default)]
    pub writable_paths: Vec<String>,
    #[serde(default)]
    pub denied_paths: Vec<String>,
    pub cross_routine_visibility: CrossRoutineVisibility,
    pub mission_context_scope: MissionContextScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_context_justification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditScope {
    pub run_history_visibility: RunHistoryVisibility,
    #[serde(default)]
    pub named_audit_roles: Vec<String>,
    pub intermediate_artifact_visibility: IntermediateArtifactVisibility,
    pub final_artifact_visibility: FinalArtifactVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorRequirement {
    pub capability: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepModelSelection {
    pub tier: ModelTier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepModelPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<StepModelSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelRoutingEntry {
    pub step_id: String,
    pub tier: ModelTier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub resolved: bool,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelRoutingReport {
    #[serde(default)]
    pub tier_assigned_count: usize,
    #[serde(default)]
    pub provider_unresolved_count: usize,
    #[serde(default)]
    pub entries: Vec<ModelRoutingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuccessCriteriaSubjectKind {
    Plan,
    Routine,
    Step,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuccessCriteriaEvaluationStatus {
    Missing,
    Defined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuccessCriteriaEvaluationEntry {
    pub subject: SuccessCriteriaSubjectKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routine_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default)]
    pub required_artifacts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_viable_completion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_window_hours: Option<u32>,
    #[serde(default)]
    pub declared_fields: Vec<String>,
    pub status: SuccessCriteriaEvaluationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SuccessCriteriaEvaluationReport {
    #[serde(default)]
    pub total_subjects: usize,
    #[serde(default)]
    pub defined_count: usize,
    #[serde(default)]
    pub missing_count: usize,
    #[serde(default)]
    pub entries: Vec<SuccessCriteriaEvaluationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepFailurePolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_missing_connector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_model_failure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepRetryPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepCostRate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_usd_per_token: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_usd_per_token: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepCostProvenance {
    pub step_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_at_execution_time: Option<StepCostRate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computed_cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_run_cost_usd_at_step_end: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_warning_fired: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_limit_reached: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routine_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_provenance: Option<StepCostProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepPackage {
    pub step_id: String,
    pub label: String,
    pub kind: String,
    pub action: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub context_reads: Vec<String>,
    #[serde(default)]
    pub context_writes: Vec<String>,
    #[serde(default)]
    pub connector_requirements: Vec<ConnectorRequirement>,
    #[serde(default)]
    pub model_policy: StepModelPolicy,
    pub approval_policy: ApprovalMode,
    #[serde(default)]
    pub success_criteria: SuccessCriteria,
    #[serde(default)]
    pub failure_policy: StepFailurePolicy,
    #[serde(default)]
    pub retry_policy: StepRetryPolicy,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<StepProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutinePackage {
    pub routine_id: String,
    pub semantic_kind: RoutineSemanticKind,
    pub trigger: TriggerDefinition,
    #[serde(default)]
    pub dependencies: Vec<RoutineDependency>,
    pub dependency_resolution: DependencyResolution,
    #[serde(default)]
    pub connector_resolution: RoutineConnectorResolution,
    pub data_scope: DataScope,
    pub audit_scope: AuditScope,
    #[serde(default)]
    pub success_criteria: SuccessCriteria,
    #[serde(default)]
    pub steps: Vec<StepPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorIntent {
    pub capability: String,
    pub why: String,
    pub required: bool,
    pub degraded_mode_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ConnectorBindingResolutionEntry {
    pub capability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    pub required: bool,
    pub degraded_mode_allowed: bool,
    pub resolved: bool,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowlist_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ConnectorBindingResolutionReport {
    #[serde(default)]
    pub mapped_count: usize,
    #[serde(default)]
    pub unresolved_required_count: usize,
    #[serde(default)]
    pub unresolved_optional_count: usize,
    #[serde(default)]
    pub entries: Vec<ConnectorBindingResolutionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorBinding {
    pub capability: String,
    pub binding_type: String,
    pub binding_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowlist_pattern: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialBindingRef {
    pub capability: String,
    pub binding_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CredentialEnvelope {
    pub routine_id: String,
    #[serde(default)]
    pub entitled_connectors: Vec<CredentialBindingRef>,
    #[serde(default)]
    pub denied_connectors: Vec<CredentialBindingRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_issued_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuing_authority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BudgetPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_per_run_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_daily_cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_weekly_cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_ceiling_per_run: Option<u64>,
    #[serde(default)]
    pub cheap_model_preferred_for: Vec<String>,
    #[serde(default)]
    pub strong_model_reserved_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CostTrackingUnit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default)]
    pub recorded_fields: Vec<String>,
    #[serde(default)]
    pub tracking_scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetWindowEnforcement {
    pub window: String,
    pub on_limit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DailyAndWeeklyEnforcement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily: Option<BudgetWindowEnforcement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekly: Option<BudgetWindowEnforcement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BudgetEnforcement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_tracking_unit: Option<CostTrackingUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_warning_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hard_limit_behavior: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_preservation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_and_weekly_enforcement: Option<DailyAndWeeklyEnforcement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ApprovalMatrix {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_posts: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_replies: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbound_email: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_reports: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_mutations: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive_actions: Option<ApprovalMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterRoutinePolicy {
    pub communication_model: CommunicationModel,
    pub shared_memory_access: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_memory_justification: Option<String>,
    pub peer_visibility: PeerVisibility,
    pub artifact_handoff_validation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TriggerPolicy {
    #[serde(default)]
    pub supported: Vec<TriggerKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OutputRoots {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drafts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PlanValidationState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_connectors_mapped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directories_writable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedules_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models_resolved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies_resolvable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvals_complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_modes_acknowledged: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_scopes_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_scopes_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_context_scopes_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_routine_policy_complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_envelopes_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compartmentalized_activation_ready: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_objects_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_criteria_evaluation: Option<SuccessCriteriaEvaluationReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OverlapIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_hash: Option<String>,
    #[serde(default)]
    pub normalized_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SemanticIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverlapLogEntry {
    pub matched_plan_id: String,
    pub matched_plan_revision: u32,
    pub match_layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity_score: Option<f64>,
    pub decision: String,
    pub decided_by: String,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OverlapPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_identity: Option<OverlapIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_identity: Option<SemanticIdentity>,
    #[serde(default)]
    pub overlap_log: Vec<OverlapLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanPackage {
    pub plan_id: String,
    pub plan_revision: u32,
    pub lifecycle_state: PlanLifecycleState,
    pub owner: PlanOwner,
    pub mission: MissionDefinition,
    #[serde(default)]
    pub success_criteria: SuccessCriteria,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_policy: Option<BudgetPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_enforcement: Option<BudgetEnforcement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ApprovalMatrix>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_routine_policy: Option<InterRoutinePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_policy: Option<TriggerPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_roots: Option<OutputRoots>,
    #[serde(default)]
    pub precedence_log: Vec<PrecedenceLogEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_diff: Option<PlanDiff>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_trigger_record: Option<ManualTriggerRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_state: Option<PlanValidationState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap_policy: Option<OverlapPolicy>,
    #[serde(default)]
    pub routine_graph: Vec<RoutinePackage>,
    #[serde(default)]
    pub connector_intents: Vec<ConnectorIntent>,
    #[serde(default)]
    pub connector_bindings: Vec<ConnectorBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_binding_resolution: Option<ConnectorBindingResolutionReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_routing_resolution: Option<ModelRoutingReport>,
    #[serde(default)]
    pub credential_envelopes: Vec<CredentialEnvelope>,
    #[serde(default)]
    pub context_objects: Vec<ContextObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

pub fn with_manual_trigger_record(
    plan_package: &PlanPackage,
    trigger_id: &str,
    triggered_by: &str,
    trigger_source: ManualTriggerSource,
    dry_run: bool,
    triggered_at: &str,
    run_id: Option<&str>,
    outcome: Option<&str>,
    artifacts_produced: Vec<String>,
    notes: Option<&str>,
) -> Option<PlanPackage> {
    let routine_id = plan_package.routine_graph.first()?.routine_id.clone();
    let mut next = plan_package.clone();
    next.manual_trigger_record = Some(ManualTriggerRecord {
        trigger_id: trigger_id.to_string(),
        plan_id: next.plan_id.clone(),
        plan_revision: next.plan_revision,
        routine_id,
        triggered_by: triggered_by.to_string(),
        trigger_source,
        dry_run,
        approval_policy_snapshot: next.approval_policy.clone(),
        connector_binding_snapshot: next.connector_bindings.clone(),
        triggered_at: triggered_at.to_string(),
        run_id: run_id.map(str::to_string),
        outcome: outcome.map(str::to_string),
        artifacts_produced,
        notes: notes.map(str::to_string),
    });
    Some(next)
}

pub fn allowed_lifecycle_transitions(state: PlanLifecycleState) -> &'static [PlanLifecycleState] {
    match state {
        PlanLifecycleState::Draft => &[PlanLifecycleState::Preview, PlanLifecycleState::Archived],
        PlanLifecycleState::Preview => &[
            PlanLifecycleState::AwaitingApproval,
            PlanLifecycleState::Draft,
            PlanLifecycleState::Archived,
        ],
        PlanLifecycleState::AwaitingApproval => &[
            PlanLifecycleState::Approved,
            PlanLifecycleState::Preview,
            PlanLifecycleState::Draft,
            PlanLifecycleState::Archived,
        ],
        PlanLifecycleState::Approved => &[
            PlanLifecycleState::Applied,
            PlanLifecycleState::Preview,
            PlanLifecycleState::Draft,
            PlanLifecycleState::Superseded,
        ],
        PlanLifecycleState::Applied => &[
            PlanLifecycleState::Active,
            PlanLifecycleState::Paused,
            PlanLifecycleState::Superseded,
            PlanLifecycleState::Archived,
        ],
        PlanLifecycleState::Active => &[
            PlanLifecycleState::Degraded,
            PlanLifecycleState::Paused,
            PlanLifecycleState::Superseded,
            PlanLifecycleState::Archived,
        ],
        PlanLifecycleState::Degraded => &[
            PlanLifecycleState::Active,
            PlanLifecycleState::Paused,
            PlanLifecycleState::Superseded,
            PlanLifecycleState::Archived,
        ],
        PlanLifecycleState::Paused => &[
            PlanLifecycleState::Active,
            PlanLifecycleState::Degraded,
            PlanLifecycleState::Superseded,
            PlanLifecycleState::Archived,
        ],
        PlanLifecycleState::Superseded => {
            &[PlanLifecycleState::Archived, PlanLifecycleState::Draft]
        }
        PlanLifecycleState::Archived => &[PlanLifecycleState::Draft],
    }
}

pub fn can_transition_plan_lifecycle(from: PlanLifecycleState, to: PlanLifecycleState) -> bool {
    allowed_lifecycle_transitions(from).contains(&to)
}

fn default_dependency_resolution() -> DependencyResolution {
    DependencyResolution {
        strategy: DependencyResolutionStrategy::TopologicalSequential,
        partial_failure_mode: PartialFailureMode::PauseDownstreamOnly,
        reentry_point: ReentryPoint::FailedStep,
        mid_routine_connector_failure: MidRoutineConnectorFailureMode::SurfaceAndPause,
    }
}

fn default_connector_resolution() -> RoutineConnectorResolution {
    RoutineConnectorResolution {
        states: vec![
            "unresolved".to_string(),
            "options_ready".to_string(),
            "awaiting_user_choice".to_string(),
            "selected".to_string(),
            "bound".to_string(),
            "linked_to_revision".to_string(),
            "degraded_ready".to_string(),
            "activation_handed_off".to_string(),
            "blocked".to_string(),
            "deferred".to_string(),
        ],
        binding_options: vec![
            "mcp_server".to_string(),
            "native_feature".to_string(),
            "oauth_integration".to_string(),
            "manual_credential".to_string(),
            "http_adapter".to_string(),
        ],
    }
}

fn default_data_scope(workspace_root: &str, routine_id: &str) -> DataScope {
    let scoped_root =
        |kind: &str| format!("{workspace_root}/knowledge/workflows/{kind}/{routine_id}/**");
    DataScope {
        readable_paths: vec![
            "mission.goal".to_string(),
            scoped_root("plan"),
            scoped_root("drafts"),
            scoped_root("proof"),
            scoped_root("run-history"),
        ],
        writable_paths: vec![
            scoped_root("plan"),
            scoped_root("drafts"),
            scoped_root("proof"),
            scoped_root("run-history"),
        ],
        denied_paths: vec!["credentials/**".to_string()],
        cross_routine_visibility: CrossRoutineVisibility::None,
        mission_context_scope: MissionContextScope::GoalAndOwnRoutine,
        mission_context_justification: None,
    }
}

fn default_audit_scope() -> AuditScope {
    AuditScope {
        run_history_visibility: RunHistoryVisibility::PlanOwner,
        named_audit_roles: Vec::new(),
        intermediate_artifact_visibility: IntermediateArtifactVisibility::RoutineOnly,
        final_artifact_visibility: FinalArtifactVisibility::PlanOwner,
    }
}

fn default_budget_policy() -> BudgetPolicy {
    BudgetPolicy {
        max_cost_per_run_usd: Some(4.0),
        max_daily_cost_usd: Some(20.0),
        max_weekly_cost_usd: Some(60.0),
        token_ceiling_per_run: Some(40_000),
        cheap_model_preferred_for: vec![
            "search".to_string(),
            "dedupe".to_string(),
            "clustering".to_string(),
            "bulk extraction".to_string(),
        ],
        strong_model_reserved_for: vec![
            "public copy".to_string(),
            "approval review".to_string(),
            "final synthesis".to_string(),
        ],
    }
}

fn default_budget_enforcement() -> BudgetEnforcement {
    BudgetEnforcement {
        cost_tracking_unit: Some(CostTrackingUnit {
            method: Some("token_count × model_rate_per_token".to_string()),
            recorded_fields: vec![
                "tokens_in".to_string(),
                "tokens_out".to_string(),
                "model_id".to_string(),
                "rate_at_execution_time".to_string(),
                "computed_cost_usd".to_string(),
            ],
            tracking_scope: vec![
                "step".to_string(),
                "routine".to_string(),
                "plan_run".to_string(),
            ],
        }),
        soft_warning_threshold: Some(0.8),
        hard_limit_behavior: Some("pause_before_step".to_string()),
        partial_result_preservation: Some(true),
        daily_and_weekly_enforcement: Some(DailyAndWeeklyEnforcement {
            daily: Some(BudgetWindowEnforcement {
                window: "rolling_24h".to_string(),
                on_limit: "defer_until_next_window".to_string(),
            }),
            weekly: Some(BudgetWindowEnforcement {
                window: "rolling_7d".to_string(),
                on_limit: "block_and_request_review".to_string(),
            }),
        }),
    }
}

fn re_root_path(workspace_root: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        workspace_root.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    )
}

fn default_output_roots(workspace_root: &str) -> OutputRoots {
    OutputRoots {
        plan: Some(re_root_path(workspace_root, "knowledge/workflows/plan/")),
        history: Some(re_root_path(
            workspace_root,
            "knowledge/workflows/run-history/",
        )),
        proof: Some(re_root_path(workspace_root, "knowledge/workflows/proof/")),
        drafts: Some(re_root_path(workspace_root, "knowledge/workflows/drafts/")),
    }
}

fn required_capabilities_for_routine(
    routine: &RoutinePackage,
) -> std::collections::BTreeSet<String> {
    let mut required_capabilities = std::collections::BTreeSet::new();
    for step in &routine.steps {
        for requirement in &step.connector_requirements {
            required_capabilities.insert(requirement.capability.clone());
        }
    }
    required_capabilities
}

pub fn derive_connector_binding_resolution_for_plan(
    plan: &PlanPackage,
) -> ConnectorBindingResolutionReport {
    let mut entries_by_capability =
        std::collections::BTreeMap::<String, ConnectorBindingResolutionEntry>::new();

    for intent in &plan.connector_intents {
        entries_by_capability.insert(
            intent.capability.clone(),
            ConnectorBindingResolutionEntry {
                capability: intent.capability.clone(),
                why: Some(intent.why.clone()),
                required: intent.required,
                degraded_mode_allowed: intent.degraded_mode_allowed,
                resolved: false,
                status: if intent.required {
                    "unresolved_required".to_string()
                } else {
                    "unresolved_optional".to_string()
                },
                binding_type: None,
                binding_id: None,
                allowlist_pattern: None,
            },
        );
    }

    for binding in &plan.connector_bindings {
        let entry = entries_by_capability
            .entry(binding.capability.clone())
            .or_insert_with(|| ConnectorBindingResolutionEntry {
                capability: binding.capability.clone(),
                why: None,
                required: false,
                degraded_mode_allowed: false,
                resolved: false,
                status: "unresolved_optional".to_string(),
                binding_type: None,
                binding_id: None,
                allowlist_pattern: None,
            });
        entry.binding_type = Some(binding.binding_type.clone());
        entry.binding_id = Some(binding.binding_id.clone());
        entry.allowlist_pattern = binding.allowlist_pattern.clone();
        if binding.status == "mapped" {
            entry.resolved = true;
            entry.status = "mapped".to_string();
        } else if entry.required {
            entry.status = "unresolved_required".to_string();
        } else {
            entry.status = "unresolved_optional".to_string();
        }
    }

    let mut entries = entries_by_capability.into_values().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.capability.cmp(&right.capability));

    let mapped_count = entries.iter().filter(|entry| entry.resolved).count();
    let unresolved_required_count = entries
        .iter()
        .filter(|entry| !entry.resolved && entry.required)
        .count();
    let unresolved_optional_count = entries
        .iter()
        .filter(|entry| !entry.resolved && !entry.required)
        .count();

    ConnectorBindingResolutionReport {
        mapped_count,
        unresolved_required_count,
        unresolved_optional_count,
        entries,
    }
}

pub fn derive_model_routing_resolution_for_plan(plan: &PlanPackage) -> ModelRoutingReport {
    let mut entries = plan
        .routine_graph
        .iter()
        .flat_map(|routine| {
            routine.steps.iter().map(|step| {
                let tier = step
                    .model_policy
                    .primary
                    .as_ref()
                    .map(|selection| selection.tier.clone())
                    .unwrap_or(ModelTier::Mid);
                let resolved = step.model_policy.primary.is_some();
                ModelRoutingEntry {
                    step_id: step.step_id.clone(),
                    tier,
                    provider_id: None,
                    model_id: None,
                    resolved,
                    status: if resolved {
                        "tier_assigned".to_string()
                    } else {
                        "unrouted".to_string()
                    },
                    reason: if resolved {
                        Some("step declares a routing tier but provider/model selection is still pending".to_string())
                    } else {
                        Some("step does not declare a model routing tier yet".to_string())
                    },
                }
            })
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| left.step_id.cmp(&right.step_id));

    let tier_assigned_count = entries.iter().filter(|entry| entry.resolved).count();
    let provider_unresolved_count = entries
        .iter()
        .filter(|entry| entry.provider_id.is_none())
        .count();

    ModelRoutingReport {
        tier_assigned_count,
        provider_unresolved_count,
        entries,
    }
}

fn success_criteria_declared_fields(criteria: &SuccessCriteria) -> Vec<String> {
    let mut declared_fields = Vec::new();
    if !criteria.required_artifacts.is_empty() {
        declared_fields.push("required_artifacts".to_string());
    }
    if criteria
        .minimum_viable_completion
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        declared_fields.push("minimum_viable_completion".to_string());
    }
    if criteria
        .minimum_output
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        declared_fields.push("minimum_output".to_string());
    }
    if criteria.freshness_window_hours.is_some() {
        declared_fields.push("freshness_window_hours".to_string());
    }
    declared_fields
}

fn success_criteria_entry(
    subject: SuccessCriteriaSubjectKind,
    routine_id: Option<&str>,
    step_id: Option<&str>,
    criteria: &SuccessCriteria,
) -> SuccessCriteriaEvaluationEntry {
    let declared_fields = success_criteria_declared_fields(criteria);
    let status = if declared_fields.is_empty() {
        SuccessCriteriaEvaluationStatus::Missing
    } else {
        SuccessCriteriaEvaluationStatus::Defined
    };
    SuccessCriteriaEvaluationEntry {
        subject,
        routine_id: routine_id.map(|value| value.to_string()),
        step_id: step_id.map(|value| value.to_string()),
        required_artifacts: criteria.required_artifacts.clone(),
        minimum_viable_completion: criteria.minimum_viable_completion.clone(),
        minimum_output: criteria.minimum_output.clone(),
        freshness_window_hours: criteria.freshness_window_hours,
        declared_fields,
        status,
    }
}

pub fn derive_success_criteria_evaluation_for_plan(
    plan: &PlanPackage,
) -> SuccessCriteriaEvaluationReport {
    let mut entries = Vec::new();
    entries.push(success_criteria_entry(
        SuccessCriteriaSubjectKind::Plan,
        None,
        None,
        &plan.success_criteria,
    ));
    for routine in &plan.routine_graph {
        entries.push(success_criteria_entry(
            SuccessCriteriaSubjectKind::Routine,
            Some(&routine.routine_id),
            None,
            &routine.success_criteria,
        ));
        for step in &routine.steps {
            entries.push(success_criteria_entry(
                SuccessCriteriaSubjectKind::Step,
                Some(&routine.routine_id),
                Some(&step.step_id),
                &step.success_criteria,
            ));
        }
    }
    let defined_count = entries
        .iter()
        .filter(|entry| entry.status == SuccessCriteriaEvaluationStatus::Defined)
        .count();
    let missing_count = entries.len().saturating_sub(defined_count);
    SuccessCriteriaEvaluationReport {
        total_subjects: entries.len(),
        defined_count,
        missing_count,
        entries,
    }
}

pub fn derive_credential_envelopes_for_plan(plan: &PlanPackage) -> Vec<CredentialEnvelope> {
    derive_credential_envelopes(&plan.routine_graph, &plan.connector_bindings)
}

fn derive_credential_envelopes(
    routines: &[RoutinePackage],
    connector_bindings: &[ConnectorBinding],
) -> Vec<CredentialEnvelope> {
    let binding_refs = connector_bindings
        .iter()
        .map(|binding| CredentialBindingRef {
            capability: binding.capability.clone(),
            binding_id: binding.binding_id.clone(),
        })
        .collect::<Vec<_>>();

    routines
        .iter()
        .map(|routine| {
            let required_capabilities = required_capabilities_for_routine(routine);

            let entitled_connectors = binding_refs
                .iter()
                .filter(|binding| required_capabilities.contains(&binding.capability))
                .cloned()
                .collect::<Vec<_>>();
            let denied_connectors = binding_refs
                .iter()
                .filter(|binding| !required_capabilities.contains(&binding.capability))
                .cloned()
                .collect::<Vec<_>>();

            CredentialEnvelope {
                routine_id: routine.routine_id.clone(),
                entitled_connectors,
                denied_connectors,
                envelope_issued_at: None,
                envelope_expires_at: None,
                issuing_authority: Some("engine".to_string()),
            }
        })
        .collect()
}

fn derive_context_objects(
    plan_id: &str,
    mission_goal: &str,
    workspace_root: &str,
    routine: &RoutinePackage,
) -> Vec<ContextObject> {
    let mut context_objects = vec![
        ContextObject {
            context_object_id: mission_goal_context_object_id(&routine.routine_id),
            name: "Mission goal".to_string(),
            kind: "mission_goal".to_string(),
            scope: ContextObjectScope::Mission,
            owner_routine_id: routine.routine_id.clone(),
            producer_step_id: None,
            declared_consumers: vec![routine.routine_id.clone()],
            artifact_ref: None,
            data_scope_refs: vec!["mission.goal".to_string()],
            freshness_window_hours: None,
            validation_status: ContextValidationStatus::Pending,
            provenance: ContextObjectProvenance {
                plan_id: plan_id.to_string(),
                routine_id: routine.routine_id.clone(),
                step_id: None,
            },
            summary: Some(mission_goal.to_string()),
        },
        ContextObject {
            context_object_id: workspace_environment_context_object_id(&routine.routine_id),
            name: "Workspace environment".to_string(),
            kind: "workspace_environment".to_string(),
            scope: ContextObjectScope::Plan,
            owner_routine_id: routine.routine_id.clone(),
            producer_step_id: None,
            declared_consumers: vec![routine.routine_id.clone()],
            artifact_ref: None,
            data_scope_refs: routine
                .data_scope
                .readable_paths
                .iter()
                .filter(|path| path.as_str() != "mission.goal")
                .take(1)
                .cloned()
                .collect(),
            freshness_window_hours: None,
            validation_status: ContextValidationStatus::Pending,
            provenance: ContextObjectProvenance {
                plan_id: plan_id.to_string(),
                routine_id: routine.routine_id.clone(),
                step_id: None,
            },
            summary: Some(workspace_root.to_string()),
        },
    ];

    context_objects.extend(routine.steps.iter().flat_map(|step| {
        step.artifacts.iter().map(|artifact| ContextObject {
            context_object_id: handoff_context_object_id(
                &routine.routine_id,
                &step.step_id,
                artifact,
            ),
            name: format!("{} handoff", step.label),
            kind: "step_output_handoff".to_string(),
            scope: ContextObjectScope::Handoff,
            owner_routine_id: routine.routine_id.clone(),
            producer_step_id: Some(step.step_id.clone()),
            declared_consumers: vec![routine.routine_id.clone()],
            artifact_ref: Some(artifact.clone()),
            data_scope_refs: routine.data_scope.writable_paths.clone(),
            freshness_window_hours: None,
            validation_status: ContextValidationStatus::Pending,
            provenance: ContextObjectProvenance {
                plan_id: plan_id.to_string(),
                routine_id: routine.routine_id.clone(),
                step_id: Some(step.step_id.clone()),
            },
            summary: step.success_criteria.minimum_output.clone(),
        })
    }));

    context_objects
}

fn mission_goal_context_object_id(routine_id: &str) -> String {
    format!("ctx:{routine_id}:mission.goal")
}

fn workspace_environment_context_object_id(routine_id: &str) -> String {
    format!("ctx:{routine_id}:workspace.environment")
}

fn handoff_context_object_id(routine_id: &str, step_id: &str, artifact: &str) -> String {
    format!("ctx:{routine_id}:{step_id}:{artifact}")
}

fn step_context_reads(routine_id: &str) -> Vec<String> {
    vec![
        mission_goal_context_object_id(routine_id),
        workspace_environment_context_object_id(routine_id),
    ]
}

fn step_context_writes(routine_id: &str, step: &WorkflowPlanStep<Value, Value>) -> Vec<String> {
    step_artifacts(step)
        .into_iter()
        .map(|artifact| handoff_context_object_id(routine_id, &step.step_id, &artifact))
        .collect()
}

fn trigger_from_schedule(
    schedule: &crate::contracts::AutomationV2ScheduleJson,
) -> TriggerDefinition {
    let trigger_type = match schedule.schedule_type {
        AutomationV2ScheduleType::Manual => TriggerKind::Manual,
        AutomationV2ScheduleType::Cron | AutomationV2ScheduleType::Interval => {
            TriggerKind::Scheduled
        }
    };

    let schedule_string = match schedule.schedule_type {
        AutomationV2ScheduleType::Cron => schedule.cron_expression.clone(),
        AutomationV2ScheduleType::Interval => schedule
            .interval_seconds
            .map(|seconds| format!("interval:{seconds}")),
        AutomationV2ScheduleType::Manual => None,
    };
    let timezone = match schedule.schedule_type {
        AutomationV2ScheduleType::Cron | AutomationV2ScheduleType::Interval => {
            Some(schedule.timezone.clone())
        }
        AutomationV2ScheduleType::Manual => None,
    };

    TriggerDefinition {
        trigger_type,
        schedule: schedule_string,
        timezone,
    }
}

fn step_label(step: &WorkflowPlanStep<Value, Value>) -> String {
    if !step.objective.trim().is_empty() {
        step.objective.trim().to_string()
    } else {
        step.step_id.replace(['_', '-'], " ")
    }
}

fn input_names(step: &WorkflowPlanStep<Value, Value>) -> Vec<String> {
    step.input_refs
        .iter()
        .filter_map(|input| {
            input
                .get("alias")
                .and_then(Value::as_str)
                .or_else(|| input.get("from_step_id").and_then(Value::as_str))
                .map(|value| value.to_string())
        })
        .collect()
}

fn output_contract_kind(contract: &Value) -> Option<String> {
    contract
        .get("kind")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn step_outputs(step: &WorkflowPlanStep<Value, Value>) -> Vec<String> {
    match step.output_contract.as_ref().and_then(output_contract_kind) {
        Some(kind) => vec![format!("{}:{kind}", step.step_id)],
        None => Vec::new(),
    }
}

fn step_artifacts(step: &WorkflowPlanStep<Value, Value>) -> Vec<String> {
    if step.output_contract.is_some() {
        vec![format!("{}.artifact", step.step_id)]
    } else {
        Vec::new()
    }
}

fn step_connector_requirements(step: &WorkflowPlanStep<Value, Value>) -> Vec<ConnectorRequirement> {
    let mut requirements = Vec::new();

    if step
        .metadata
        .as_ref()
        .and_then(|value| value.get("builder"))
        .and_then(|value| value.get("web_research_expected"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        requirements.push(ConnectorRequirement {
            capability: "websearch".to_string(),
            required: true,
        });
    }

    if let Some(required_tools) = step
        .output_contract
        .as_ref()
        .and_then(|value| value.get("enforcement"))
        .and_then(|value| value.get("required_tools"))
        .and_then(Value::as_array)
    {
        for tool in required_tools.iter().filter_map(Value::as_str) {
            let capability = tool.trim();
            if capability.is_empty()
                || requirements
                    .iter()
                    .any(|existing| existing.capability == capability)
            {
                continue;
            }
            requirements.push(ConnectorRequirement {
                capability: capability.to_string(),
                required: true,
            });
        }
    }

    requirements
}

fn semantic_kind_for_plan(plan: &crate::contracts::WorkflowPlanJson) -> RoutineSemanticKind {
    if plan
        .steps
        .iter()
        .any(|step| step.kind.to_ascii_lowercase().contains("research"))
    {
        RoutineSemanticKind::Research
    } else {
        RoutineSemanticKind::Mixed
    }
}

fn normalize_token(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn canonical_overlap_hash(plan: &crate::contracts::WorkflowPlanJson) -> String {
    let output_kinds = plan
        .steps
        .iter()
        .filter_map(|step| step.output_contract.as_ref().and_then(output_contract_kind))
        .collect::<Vec<_>>()
        .join("|");
    let routine_semantics = plan
        .steps
        .iter()
        .map(|step| {
            format!(
                "{}:{}",
                normalize_token(&step.step_id),
                normalize_token(&step.kind)
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let source_set = {
        let mut values = plan
            .requires_integrations
            .iter()
            .chain(plan.allowed_mcp_servers.iter())
            .map(|value| normalize_token(value))
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        values.join("|")
    };
    let topic = plan
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&plan.title);
    let normalized = [
        format!("goal={}", normalize_token(&plan.normalized_prompt)),
        format!("topic={}", normalize_token(topic)),
        format!("source_set={source_set}"),
        format!("outputs={output_kinds}"),
        format!("routine_semantics={routine_semantics}"),
    ]
    .join("\n");
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}

fn default_overlap_policy(plan: &crate::contracts::WorkflowPlanJson) -> OverlapPolicy {
    OverlapPolicy {
        exact_identity: Some(OverlapIdentity {
            hash_version: Some(1),
            canonical_hash: Some(canonical_overlap_hash(plan)),
            normalized_fields: vec![
                "goal".to_string(),
                "topic".to_string(),
                "source_set".to_string(),
                "outputs".to_string(),
                "routine_semantics".to_string(),
            ],
        }),
        semantic_identity: Some(SemanticIdentity {
            similarity_model: Some("text-embedding-3-large".to_string()),
            semantic_signature: None,
            similarity_threshold: Some(0.85),
        }),
        overlap_log: Vec::new(),
    }
}

pub fn compile_workflow_plan_preview_package(
    plan: &crate::contracts::WorkflowPlanJson,
    owner_id: Option<&str>,
) -> PlanPackage {
    let owner_id = owner_id.unwrap_or("workflow_planner");
    let routine_id = format!("{}_routine", plan.plan_id);
    let steps = plan
        .steps
        .iter()
        .map(|step| StepPackage {
            step_id: step.step_id.clone(),
            label: step_label(step),
            kind: step.kind.clone(),
            action: step.objective.clone(),
            inputs: input_names(step),
            outputs: step_outputs(step),
            dependencies: step.depends_on.clone(),
            context_reads: step_context_reads(&routine_id),
            context_writes: step_context_writes(&routine_id, step),
            connector_requirements: step_connector_requirements(step),
            model_policy: StepModelPolicy::default(),
            approval_policy: ApprovalMode::InternalOnly,
            success_criteria: SuccessCriteria {
                required_artifacts: step_artifacts(step),
                minimum_viable_completion: None,
                minimum_output: step
                    .output_contract
                    .as_ref()
                    .and_then(output_contract_kind)
                    .map(|kind| format!("produce {kind} output")),
                freshness_window_hours: None,
            },
            failure_policy: StepFailurePolicy::default(),
            retry_policy: StepRetryPolicy::default(),
            artifacts: step_artifacts(step),
            provenance: None,
            notes: step.metadata.clone().map(|value| value.to_string()),
        })
        .collect::<Vec<_>>();

    let connector_intents = plan
        .requires_integrations
        .iter()
        .map(|capability| ConnectorIntent {
            capability: capability.clone(),
            why: "Required by workflow plan preview".to_string(),
            required: true,
            degraded_mode_allowed: false,
        })
        .collect::<Vec<_>>();

    let required_artifacts = steps
        .iter()
        .flat_map(|step| step.artifacts.clone())
        .collect::<Vec<_>>();

    let mut package = PlanPackage {
        plan_id: plan.plan_id.clone(),
        plan_revision: 1,
        lifecycle_state: PlanLifecycleState::Preview,
        owner: PlanOwner {
            owner_id: owner_id.to_string(),
            scope: "workspace".to_string(),
            audience: "internal".to_string(),
        },
        mission: MissionDefinition {
            goal: plan.original_prompt.clone(),
            summary: plan
                .description
                .clone()
                .or_else(|| Some(plan.title.clone())),
            domain: Some("workflow".to_string()),
        },
        success_criteria: SuccessCriteria {
            required_artifacts: required_artifacts.clone(),
            minimum_viable_completion: Some(format!(
                "Preview a routine graph with {} step(s)",
                steps.len()
            )),
            minimum_output: None,
            freshness_window_hours: None,
        },
        budget_policy: Some(default_budget_policy()),
        budget_enforcement: Some(default_budget_enforcement()),
        approval_policy: Some(ApprovalMatrix {
            internal_reports: Some(ApprovalMode::AutoApproved),
            public_posts: Some(ApprovalMode::ApprovalRequired),
            public_replies: Some(ApprovalMode::ApprovalRequired),
            outbound_email: Some(ApprovalMode::ApprovalRequired),
            connector_mutations: Some(ApprovalMode::ApprovalRequired),
            destructive_actions: Some(ApprovalMode::ApprovalRequired),
        }),
        inter_routine_policy: Some(InterRoutinePolicy {
            communication_model: CommunicationModel::ArtifactOnly,
            shared_memory_access: false,
            shared_memory_justification: None,
            peer_visibility: PeerVisibility::DeclaredOutputsOnly,
            artifact_handoff_validation: true,
        }),
        trigger_policy: Some(TriggerPolicy {
            supported: vec![
                TriggerKind::Scheduled,
                TriggerKind::Manual,
                TriggerKind::ArtifactTriggered,
                TriggerKind::DependencyTriggered,
            ],
        }),
        output_roots: Some(default_output_roots(&plan.workspace_root)),
        precedence_log: Vec::new(),
        plan_diff: None,
        manual_trigger_record: None,
        validation_state: None,
        overlap_policy: Some(default_overlap_policy(plan)),
        routine_graph: vec![RoutinePackage {
            routine_id: routine_id.clone(),
            semantic_kind: semantic_kind_for_plan(plan),
            trigger: trigger_from_schedule(&plan.schedule),
            dependencies: Vec::new(),
            dependency_resolution: default_dependency_resolution(),
            connector_resolution: default_connector_resolution(),
            data_scope: default_data_scope(&plan.workspace_root, &routine_id),
            audit_scope: default_audit_scope(),
            success_criteria: SuccessCriteria {
                required_artifacts,
                minimum_viable_completion: Some(format!(
                    "At least {} step(s) remain inspectable in preview",
                    steps.len()
                )),
                minimum_output: None,
                freshness_window_hours: None,
            },
            steps,
        }],
        connector_intents,
        connector_bindings: Vec::new(),
        connector_binding_resolution: None,
        model_routing_resolution: None,
        credential_envelopes: Vec::new(),
        context_objects: Vec::new(),
        metadata: Some(serde_json::json!({
            "source": "workflow_plan_preview",
            "planner_version": plan.planner_version,
            "plan_source": plan.plan_source,
            "execution_target": plan.execution_target,
            "allowed_mcp_servers": plan.allowed_mcp_servers,
            "save_options": plan.save_options,
        })),
    };
    package.connector_binding_resolution =
        Some(derive_connector_binding_resolution_for_plan(&package));
    package.model_routing_resolution = Some(derive_model_routing_resolution_for_plan(&package));
    package.credential_envelopes =
        derive_credential_envelopes(&package.routine_graph, &package.connector_bindings);
    package.context_objects = package
        .routine_graph
        .iter()
        .flat_map(|routine| {
            derive_context_objects(
                &package.plan_id,
                &package.mission.goal,
                &plan.workspace_root,
                routine,
            )
        })
        .collect();
    let validation = validate_plan_package(&package);
    package.validation_state = Some(validation.validation_state);
    package
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plan_package_roundtrips_preview_shape() {
        let package = PlanPackage {
            plan_id: "plan_0f6e8c".to_string(),
            plan_revision: 3,
            lifecycle_state: PlanLifecycleState::Preview,
            owner: PlanOwner {
                owner_id: "evan".to_string(),
                scope: "workspace".to_string(),
                audience: "internal".to_string(),
            },
            mission: MissionDefinition {
                goal: "Operationalize a user goal".to_string(),
                summary: Some("Turn one mission into a governed multi-routine plan".to_string()),
                domain: Some("mixed".to_string()),
            },
            success_criteria: SuccessCriteria {
                required_artifacts: vec!["founder_brief.md".to_string()],
                minimum_viable_completion: Some("At least one usable routine graph".to_string()),
                minimum_output: None,
                freshness_window_hours: None,
            },
            budget_policy: Some(BudgetPolicy {
                max_cost_per_run_usd: Some(4.0),
                max_daily_cost_usd: Some(20.0),
                max_weekly_cost_usd: Some(60.0),
                token_ceiling_per_run: Some(40_000),
                cheap_model_preferred_for: vec!["search".to_string()],
                strong_model_reserved_for: vec!["final synthesis".to_string()],
            }),
            budget_enforcement: Some(BudgetEnforcement {
                cost_tracking_unit: Some(CostTrackingUnit {
                    method: Some("token_count × model_rate_per_token".to_string()),
                    recorded_fields: vec!["tokens_in".to_string(), "tokens_out".to_string()],
                    tracking_scope: vec!["step".to_string(), "plan_run".to_string()],
                }),
                soft_warning_threshold: Some(0.8),
                hard_limit_behavior: Some("pause_before_step".to_string()),
                partial_result_preservation: Some(true),
                daily_and_weekly_enforcement: None,
            }),
            approval_policy: Some(ApprovalMatrix {
                public_posts: Some(ApprovalMode::ApprovalRequired),
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
            trigger_policy: Some(TriggerPolicy {
                supported: vec![TriggerKind::Scheduled, TriggerKind::Manual],
            }),
            output_roots: Some(OutputRoots {
                plan: Some("knowledge/workflows/plan/".to_string()),
                history: Some("knowledge/workflows/run-history/".to_string()),
                proof: Some("knowledge/workflows/proof/".to_string()),
                drafts: Some("knowledge/workflows/drafts/".to_string()),
            }),
            precedence_log: vec![PrecedenceLogEntry {
                path: "budget_policy.max_cost_per_run_usd".to_string(),
                compiler_default: Some(json!(2.0)),
                user_override: Some(json!(4.0)),
                approved_plan_state: None,
                resolved_value: Some(json!(4.0)),
                source_tier: PrecedenceSourceTier::UserOverride,
                conflict_detected: true,
                resolution_rule: "approved_plan_state > user_override > compiler_default"
                    .to_string(),
                resolved_at: Some("2026-03-27T09:12:00Z".to_string()),
            }],
            plan_diff: Some(PlanDiff {
                from_revision: 2,
                to_revision: 3,
                changed_fields: vec![PlanDiffChangedField {
                    path: "routine_graph[0].trigger.schedule".to_string(),
                    change_type: PlanDiffChangeType::Update,
                    old_value: Some(json!("0 9 * * *")),
                    new_value: Some(json!("0 10 * * *")),
                    requires_revalidation: true,
                    requires_reapproval: false,
                    breaking: false,
                }],
                summary: PlanDiffSummary {
                    changed_count: 1,
                    breaking_count: 0,
                    revalidation_required: true,
                    reapproval_required: false,
                },
            }),
            manual_trigger_record: Some(ManualTriggerRecord {
                trigger_id: "mt_01HZY".to_string(),
                plan_id: "plan_0f6e8c".to_string(),
                plan_revision: 3,
                routine_id: "founder_brief_daily".to_string(),
                triggered_by: "user_123".to_string(),
                trigger_source: ManualTriggerSource::Calendar,
                dry_run: true,
                approval_policy_snapshot: Some(ApprovalMatrix {
                    internal_reports: Some(ApprovalMode::AutoApproved),
                    public_posts: Some(ApprovalMode::ApprovalRequired),
                    ..ApprovalMatrix::default()
                }),
                connector_binding_snapshot: vec![ConnectorBinding {
                    capability: "gmail".to_string(),
                    binding_type: "oauth_integration".to_string(),
                    binding_id: "gmail-prod".to_string(),
                    allowlist_pattern: Some("gmail.send".to_string()),
                    status: "mapped".to_string(),
                }],
                triggered_at: "2026-03-27T09:15:00Z".to_string(),
                run_id: Some("run_abc123".to_string()),
                outcome: Some("paused_after_validation".to_string()),
                artifacts_produced: vec!["founder_brief_draft.md".to_string()],
                notes: Some("Dry-run from calendar entry".to_string()),
            }),
            validation_state: Some(PlanValidationState {
                required_connectors_mapped: Some(false),
                directories_writable: Some(true),
                schedules_valid: Some(true),
                models_resolved: Some(true),
                dependencies_resolvable: Some(true),
                approvals_complete: Some(true),
                degraded_modes_acknowledged: Some(false),
                data_scopes_valid: Some(true),
                audit_scopes_valid: Some(true),
                mission_context_scopes_valid: Some(true),
                inter_routine_policy_complete: Some(true),
                credential_envelopes_valid: Some(true),
                compartmentalized_activation_ready: Some(true),
                context_objects_valid: Some(true),
                success_criteria_evaluation: None,
            }),
            overlap_policy: Some(OverlapPolicy {
                exact_identity: Some(OverlapIdentity {
                    hash_version: Some(1),
                    canonical_hash: Some("abc123".to_string()),
                    normalized_fields: vec!["goal".to_string(), "outputs".to_string()],
                }),
                semantic_identity: Some(SemanticIdentity {
                    similarity_model: Some("text-embedding-3-large".to_string()),
                    semantic_signature: Some("vec-ref".to_string()),
                    similarity_threshold: Some(0.8),
                }),
                overlap_log: vec![OverlapLogEntry {
                    matched_plan_id: "plan_old".to_string(),
                    matched_plan_revision: 2,
                    match_layer: "semantic".to_string(),
                    similarity_score: Some(0.92),
                    decision: "fork".to_string(),
                    decided_by: "user_confirmed".to_string(),
                    decided_at: "2026-03-27T10:00:00Z".to_string(),
                }],
            }),
            routine_graph: vec![RoutinePackage {
                routine_id: "founder_brief_daily".to_string(),
                semantic_kind: RoutineSemanticKind::Reporting,
                trigger: TriggerDefinition {
                    trigger_type: TriggerKind::Scheduled,
                    schedule: Some("0 10 * * *".to_string()),
                    timezone: Some("UTC".to_string()),
                },
                dependencies: vec![RoutineDependency {
                    dependency_type: "routine".to_string(),
                    routine_id: "market_pain_daily".to_string(),
                    mode: DependencyMode::Hard,
                }],
                dependency_resolution: DependencyResolution {
                    strategy: DependencyResolutionStrategy::TopologicalSequential,
                    partial_failure_mode: PartialFailureMode::PauseDownstreamOnly,
                    reentry_point: ReentryPoint::FailedStep,
                    mid_routine_connector_failure: MidRoutineConnectorFailureMode::SurfaceAndPause,
                },
                connector_resolution: RoutineConnectorResolution {
                    states: vec!["unresolved".to_string(), "bound".to_string()],
                    binding_options: vec!["mcp_server".to_string(), "native_feature".to_string()],
                },
                data_scope: DataScope {
                    readable_paths: vec!["mission.goal".to_string()],
                    writable_paths: vec![
                        "knowledge/workflows/drafts/founder_brief_daily/**".to_string()
                    ],
                    denied_paths: vec!["credentials/**".to_string()],
                    cross_routine_visibility: CrossRoutineVisibility::DeclaredOutputsOnly,
                    mission_context_scope: MissionContextScope::GoalAndDependencies,
                    mission_context_justification: None,
                },
                audit_scope: AuditScope {
                    run_history_visibility: RunHistoryVisibility::PlanOwner,
                    named_audit_roles: Vec::new(),
                    intermediate_artifact_visibility: IntermediateArtifactVisibility::RoutineOnly,
                    final_artifact_visibility: FinalArtifactVisibility::DeclaredConsumers,
                },
                success_criteria: SuccessCriteria {
                    required_artifacts: vec!["founder_brief.md".to_string()],
                    minimum_viable_completion: None,
                    minimum_output: Some("one usable brief draft".to_string()),
                    freshness_window_hours: Some(24),
                },
                steps: vec![StepPackage {
                    step_id: "draft_brief".to_string(),
                    label: "Draft brief".to_string(),
                    kind: "reporting".to_string(),
                    action: "synthesize the daily findings into a founder brief".to_string(),
                    inputs: vec!["clustered_themes".to_string()],
                    outputs: vec!["founder_brief_draft".to_string()],
                    dependencies: vec!["market_pain_daily".to_string()],
                    context_reads: vec![
                        "ctx:founder_brief_daily:mission.goal".to_string(),
                        "ctx:founder_brief_daily:workspace.environment".to_string(),
                    ],
                    context_writes: vec![
                        "ctx:founder_brief_daily:draft_brief:founder_brief_draft.md".to_string(),
                    ],
                    connector_requirements: vec![ConnectorRequirement {
                        capability: "gmail".to_string(),
                        required: true,
                    }],
                    model_policy: StepModelPolicy {
                        primary: Some(StepModelSelection {
                            tier: ModelTier::Mid,
                        }),
                    },
                    approval_policy: ApprovalMode::DraftOnly,
                    success_criteria: SuccessCriteria {
                        minimum_output: Some("one usable brief draft".to_string()),
                        ..SuccessCriteria::default()
                    },
                    failure_policy: StepFailurePolicy {
                        on_model_failure: Some("retry_once_then_pause".to_string()),
                        ..StepFailurePolicy::default()
                    },
                    retry_policy: StepRetryPolicy {
                        max_attempts: Some(2),
                    },
                    artifacts: vec!["founder_brief_draft.md".to_string()],
                    provenance: Some(StepProvenance {
                        plan_id: Some("plan_0f6e8c".to_string()),
                        routine_id: Some("founder_brief_daily".to_string()),
                        step_id: Some("draft_brief".to_string()),
                        cost_provenance: None,
                    }),
                    notes: None,
                }],
            }],
            connector_intents: vec![ConnectorIntent {
                capability: "gmail".to_string(),
                why: "Deliver founder brief".to_string(),
                required: true,
                degraded_mode_allowed: false,
            }],
            connector_bindings: vec![ConnectorBinding {
                capability: "gmail".to_string(),
                binding_type: "oauth_integration".to_string(),
                binding_id: "gmail-prod".to_string(),
                allowlist_pattern: Some("gmail.send".to_string()),
                status: "mapped".to_string(),
            }],
            connector_binding_resolution: None,
            model_routing_resolution: None,
            credential_envelopes: vec![CredentialEnvelope {
                routine_id: "founder_brief_daily".to_string(),
                entitled_connectors: vec![CredentialBindingRef {
                    capability: "gmail".to_string(),
                    binding_id: "gmail-prod".to_string(),
                }],
                denied_connectors: Vec::new(),
                envelope_issued_at: None,
                envelope_expires_at: None,
                issuing_authority: Some("engine".to_string()),
            }],
            context_objects: vec![ContextObject {
                context_object_id: "ctx:founder_brief_daily:draft_brief:founder_brief_draft.md"
                    .to_string(),
                name: "Draft brief handoff".to_string(),
                kind: "step_output_handoff".to_string(),
                scope: ContextObjectScope::Handoff,
                owner_routine_id: "founder_brief_daily".to_string(),
                producer_step_id: Some("draft_brief".to_string()),
                declared_consumers: vec!["founder_brief_daily".to_string()],
                artifact_ref: Some("founder_brief_draft.md".to_string()),
                data_scope_refs: vec![
                    "knowledge/workflows/drafts/founder_brief_daily/**".to_string()
                ],
                freshness_window_hours: Some(24),
                validation_status: ContextValidationStatus::Pending,
                provenance: ContextObjectProvenance {
                    plan_id: "plan_0f6e8c".to_string(),
                    routine_id: "founder_brief_daily".to_string(),
                    step_id: Some("draft_brief".to_string()),
                },
                summary: Some("one usable brief draft".to_string()),
            }],
            metadata: Some(serde_json::json!({
                "source": "preview",
                "schema_version": 1
            })),
        };

        let json = serde_json::to_value(&package).expect("serialize plan package");
        let roundtrip: PlanPackage =
            serde_json::from_value(json).expect("deserialize plan package");

        assert_eq!(roundtrip, package);
        assert_eq!(roundtrip.routine_graph.len(), 1);
        assert_eq!(roundtrip.routine_graph[0].steps.len(), 1);
    }

    #[test]
    fn compile_workflow_plan_preview_package_projects_workflow_plan_json() {
        let plan = crate::contracts::WorkflowPlanJson {
            plan_id: "plan_preview".to_string(),
            planner_version: "v1".to_string(),
            plan_source: "test".to_string(),
            original_prompt: "Research the market and draft a summary.".to_string(),
            normalized_prompt: "research the market and draft a summary".to_string(),
            confidence: "medium".to_string(),
            title: "Research market".to_string(),
            description: Some("Preview plan".to_string()),
            schedule: crate::contracts::default_fallback_schedule_json(),
            execution_target: "automation_v2".to_string(),
            workspace_root: "/repo".to_string(),
            steps: vec![WorkflowPlanStep {
                step_id: "research_sources".to_string(),
                kind: "research".to_string(),
                objective: "Collect source material".to_string(),
                depends_on: Vec::new(),
                agent_role: "worker".to_string(),
                input_refs: vec![json!({"from_step_id":"seed","alias":"seed_input"})],
                output_contract: Some(json!({
                    "kind": "brief",
                    "enforcement": {
                        "required_tools": ["websearch"]
                    }
                })),
                metadata: Some(json!({
                    "builder": {
                        "web_research_expected": true
                    }
                })),
            }],
            requires_integrations: vec!["gmail".to_string()],
            allowed_mcp_servers: vec!["github".to_string()],
            operator_preferences: None,
            save_options: json!({"can_export_pack": true}),
        };

        let package = compile_workflow_plan_preview_package(&plan, Some("evan"));

        assert_eq!(package.plan_id, "plan_preview");
        assert_eq!(package.lifecycle_state, PlanLifecycleState::Preview);
        assert_eq!(package.owner.owner_id, "evan");
        assert_eq!(package.routine_graph.len(), 1);
        assert_eq!(package.routine_graph[0].steps.len(), 1);
        assert_eq!(
            package.routine_graph[0].steps[0].context_reads,
            vec![
                "ctx:plan_preview_routine:mission.goal".to_string(),
                "ctx:plan_preview_routine:workspace.environment".to_string(),
            ]
        );
        assert_eq!(
            package.routine_graph[0].steps[0].context_writes,
            vec!["ctx:plan_preview_routine:research_sources:research_sources.artifact".to_string()]
        );
        assert_eq!(
            package.routine_graph[0].steps[0].connector_requirements[0].capability,
            "websearch"
        );
        assert_eq!(package.connector_intents[0].capability, "gmail");
        assert_eq!(
            package.connector_binding_resolution.as_ref().map(|report| (
                report.mapped_count,
                report.unresolved_required_count,
                report.entries.len()
            )),
            Some((0, 1, 1))
        );
        assert_eq!(
            package
                .output_roots
                .as_ref()
                .and_then(|roots| roots.plan.as_deref()),
            Some("/repo/knowledge/workflows/plan/")
        );
        assert_eq!(
            package.routine_graph[0].trigger.trigger_type,
            TriggerKind::Manual
        );
        assert!(package.routine_graph[0].trigger.schedule.is_none());
        assert!(package.routine_graph[0].trigger.timezone.is_none());
        assert_eq!(
            package
                .budget_policy
                .as_ref()
                .and_then(|policy| policy.max_cost_per_run_usd),
            Some(4.0)
        );
        assert_eq!(
            package
                .budget_enforcement
                .as_ref()
                .and_then(|enforcement| enforcement.soft_warning_threshold),
            Some(0.8)
        );
        assert_eq!(
            package
                .overlap_policy
                .as_ref()
                .and_then(|policy| policy.exact_identity.as_ref())
                .and_then(|identity| identity.hash_version),
            Some(1)
        );
        assert_eq!(
            package
                .overlap_policy
                .as_ref()
                .and_then(|policy| policy.semantic_identity.as_ref())
                .and_then(|identity| identity.similarity_threshold),
            Some(0.85)
        );
        assert_eq!(package.credential_envelopes.len(), 1);
        assert_eq!(
            package.credential_envelopes[0].routine_id,
            package.routine_graph[0].routine_id
        );
        assert!(package.precedence_log.is_empty());
        assert!(package.plan_diff.is_none());
        assert!(package.manual_trigger_record.is_none());
        assert_eq!(package.credential_envelopes[0].entitled_connectors.len(), 0);
        assert_eq!(package.context_objects.len(), 3);
        assert_eq!(package.context_objects[0].kind, "mission_goal");
        assert_eq!(package.context_objects[1].kind, "workspace_environment");
        assert_eq!(
            package.context_objects[2].producer_step_id.as_deref(),
            Some("research_sources")
        );
        assert_eq!(
            package.routine_graph[0].data_scope.writable_paths[0],
            "/repo/knowledge/workflows/plan/plan_preview_routine/**"
        );
        assert_eq!(
            package
                .validation_state
                .as_ref()
                .and_then(|state| state.credential_envelopes_valid),
            Some(true)
        );
    }

    #[test]
    fn with_manual_trigger_record_captures_plan_snapshots() {
        let plan = compile_workflow_plan_preview_package(
            &crate::contracts::WorkflowPlanJson {
                plan_id: "plan_manual_trigger".to_string(),
                planner_version: "v1".to_string(),
                plan_source: "test".to_string(),
                original_prompt: "Draft a brief".to_string(),
                normalized_prompt: "draft a brief".to_string(),
                confidence: "medium".to_string(),
                title: "Draft a brief".to_string(),
                description: Some("Preview plan".to_string()),
                schedule: crate::contracts::default_fallback_schedule_json(),
                execution_target: "automation_v2".to_string(),
                workspace_root: "/repo".to_string(),
                steps: vec![WorkflowPlanStep {
                    step_id: "draft_brief".to_string(),
                    kind: "analysis".to_string(),
                    objective: "Draft a brief".to_string(),
                    depends_on: Vec::new(),
                    agent_role: "writer".to_string(),
                    input_refs: Vec::new(),
                    output_contract: Some(json!({"kind": "report_markdown"})),
                    metadata: None,
                }],
                requires_integrations: vec!["github".to_string()],
                allowed_mcp_servers: vec!["github".to_string()],
                operator_preferences: None,
                save_options: json!({}),
            },
            Some("control-panel"),
        );
        let updated = with_manual_trigger_record(
            &plan,
            "manual-trigger-run_123",
            "control-panel",
            ManualTriggerSource::Api,
            true,
            "2026-03-28T10:15:00Z",
            Some("run_123"),
            Some("queued"),
            vec!["artifact.md".to_string()],
            Some("Triggered from test"),
        )
        .expect("manual trigger record");

        let record = updated
            .manual_trigger_record
            .as_ref()
            .expect("manual trigger record");
        assert_eq!(record.trigger_id, "manual-trigger-run_123");
        assert_eq!(record.plan_id, updated.plan_id);
        assert_eq!(record.plan_revision, updated.plan_revision);
        assert_eq!(record.routine_id, updated.routine_graph[0].routine_id);
        assert_eq!(record.triggered_by, "control-panel");
        assert_eq!(record.trigger_source, ManualTriggerSource::Api);
        assert!(record.dry_run);
        assert_eq!(record.run_id.as_deref(), Some("run_123"));
        assert_eq!(record.outcome.as_deref(), Some("queued"));
        assert_eq!(record.artifacts_produced, vec!["artifact.md".to_string()]);
        assert_eq!(record.notes.as_deref(), Some("Triggered from test"));
        assert_eq!(
            record.connector_binding_snapshot,
            updated.connector_bindings
        );
        assert_eq!(record.approval_policy_snapshot, updated.approval_policy);
    }

    #[test]
    fn lifecycle_transition_table_matches_spec() {
        assert!(can_transition_plan_lifecycle(
            PlanLifecycleState::Preview,
            PlanLifecycleState::AwaitingApproval
        ));
        assert!(can_transition_plan_lifecycle(
            PlanLifecycleState::Approved,
            PlanLifecycleState::Applied
        ));
        assert!(can_transition_plan_lifecycle(
            PlanLifecycleState::Archived,
            PlanLifecycleState::Draft
        ));
        assert!(!can_transition_plan_lifecycle(
            PlanLifecycleState::Draft,
            PlanLifecycleState::Active
        ));
        assert!(!can_transition_plan_lifecycle(
            PlanLifecycleState::Archived,
            PlanLifecycleState::Applied
        ));
    }
}
