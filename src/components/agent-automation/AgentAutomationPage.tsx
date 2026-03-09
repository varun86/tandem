import { useEffect, useMemo, useState } from "react";
import { Button, Input } from "@/components/ui";
import { ProjectSwitcher } from "@/components/sidebar";
import {
  automationsV2Delete,
  automationsV2Get,
  automationsV2List,
  automationsV2Pause,
  automationsV2Resume,
  automationsV2RunCancel,
  automationsV2RunGet,
  automationsV2RunNow,
  automationsV2RunPause,
  automationsV2RunResume,
  automationsV2Runs,
  automationsV2Update,
  getSessionMessages,
  listProvidersFromSidecar,
  mcpListServers,
  onSidecarEventV2,
  routinesList,
  type AutomationV2RunRecord,
  type AutomationV2Spec,
  type McpServerRecord,
  type ProviderInfo,
  type RoutineSpec,
  type SessionMessage,
  type UserProject,
  workflowPlansApply,
  workflowPlansChatMessage,
  workflowPlansChatReset,
  workflowPlansChatStart,
  type WorkflowPlan,
  type WorkflowPlanConversation,
} from "@/lib/tauri";

type PageTab = "create" | "automations" | "runs";
type WizardStep = 1 | 2 | 3 | 4;
type ScheduleKind = "manual" | "interval" | "cron";
type ExecutionMode = "team" | "swarm";

interface WizardState {
  prompt: string;
  workspaceRoot: string;
  scheduleKind: ScheduleKind;
  intervalSeconds: string;
  cronExpression: string;
  executionMode: ExecutionMode;
  maxParallelAgents: string;
  modelProvider: string;
  modelId: string;
  plannerModelProvider: string;
  plannerModelId: string;
  selectedMcpServers: string[];
  exportPackDraft: boolean;
}

interface WorkflowEditDraft {
  automationId: string;
  name: string;
  description: string;
  workspaceRoot: string;
  scheduleKind: ScheduleKind;
  intervalSeconds: string;
  cronExpression: string;
  executionMode: ExecutionMode;
  maxParallelAgents: string;
  modelProvider: string;
  modelId: string;
  plannerModelProvider: string;
  plannerModelId: string;
  selectedMcpServers: string[];
}

interface AgentAutomationPageProps {
  userProjects: UserProject[];
  activeProject: UserProject | null;
  onSwitchProject: (projectId: string) => void;
  onAddProject: () => void;
  onManageProjects: () => void;
  projectSwitcherLoading?: boolean;
  onOpenMcpExtensions?: () => void;
}

function buildDefaultWizard(
  activeProject: UserProject | null,
  providers: ProviderInfo[],
  current?: Partial<WizardState>
): WizardState {
  const preferredProvider =
    String(current?.modelProvider || "").trim() ||
    providers.find((provider) => provider.configured && provider.models.length > 0)?.id ||
    providers.find((provider) => provider.models.length > 0)?.id ||
    "";
  const preferredModels =
    providers.find((provider) => provider.id === preferredProvider)?.models ?? [];
  return {
    prompt: String(current?.prompt || "").trim(),
    workspaceRoot: String(current?.workspaceRoot || activeProject?.path || "").trim(),
    scheduleKind: current?.scheduleKind ?? "interval",
    intervalSeconds: String(current?.intervalSeconds || "86400"),
    cronExpression: String(current?.cronExpression || ""),
    executionMode: current?.executionMode ?? "team",
    maxParallelAgents: String(current?.maxParallelAgents || "4"),
    modelProvider: preferredProvider,
    modelId: String(current?.modelId || preferredModels[0] || ""),
    plannerModelProvider: String(current?.plannerModelProvider || ""),
    plannerModelId: String(current?.plannerModelId || ""),
    selectedMcpServers: current?.selectedMcpServers ?? [],
    exportPackDraft: current?.exportPackDraft ?? false,
  };
}

function validateWorkspaceRootInput(value: string) {
  const trimmed = String(value || "").trim();
  if (!trimmed) return "Workspace root is required.";
  if (!trimmed.startsWith("/")) return "Workspace root must be an absolute path.";
  return "";
}

function validateModelInput(provider: string, model: string) {
  const providerValue = String(provider || "").trim();
  const modelValue = String(model || "").trim();
  if (!providerValue && !modelValue) return "";
  if (!providerValue) return "Model provider is required when a model is set.";
  if (!modelValue) return "Model is required when a provider is set.";
  return "";
}

function validatePlannerModelInput(provider: string, model: string) {
  const providerValue = String(provider || "").trim();
  const modelValue = String(model || "").trim();
  if (!providerValue && !modelValue) return "";
  if (!providerValue) return "Planner model provider is required when a planner model is set.";
  if (!modelValue) return "Planner model is required when a planner provider is set.";
  return "";
}

function formatDateTime(raw: unknown) {
  const value = Number(raw || 0);
  if (!Number.isFinite(value) || value <= 0) return "n/a";
  return new Date(value < 1_000_000_000_000 ? value * 1000 : value).toLocaleString();
}

function formatSchedule(
  scheduleKind: ScheduleKind,
  intervalSeconds: string,
  cronExpression: string
) {
  if (scheduleKind === "manual") return "Manual";
  if (scheduleKind === "cron") return `Cron: ${String(cronExpression || "").trim() || "unset"}`;
  const seconds = Math.max(1, Number.parseInt(String(intervalSeconds || "3600"), 10) || 3600);
  if (seconds % 86400 === 0) return `Every ${seconds / 86400} day(s)`;
  if (seconds % 3600 === 0) return `Every ${seconds / 3600} hour(s)`;
  if (seconds % 60 === 0) return `Every ${seconds / 60} minute(s)`;
  return `Every ${seconds} second(s)`;
}

function workflowEditToSchedule(
  draft: WorkflowEditDraft | WizardState
): AutomationV2Spec["schedule"] {
  if (draft.scheduleKind === "manual") {
    return { type: "manual", timezone: "UTC", misfire_policy: "run_once" };
  }
  if (draft.scheduleKind === "cron") {
    return {
      type: "cron",
      cron_expression: String(draft.cronExpression || "").trim(),
      timezone: "UTC",
      misfire_policy: "run_once",
    };
  }
  return {
    type: "interval",
    interval_seconds: Math.max(
      1,
      Number.parseInt(String(draft.intervalSeconds || "3600"), 10) || 3600
    ),
    timezone: "UTC",
    misfire_policy: "run_once",
  };
}

function workflowEditToOperatorPreferences(draft: WorkflowEditDraft | WizardState) {
  const prefs: Record<string, unknown> = {
    execution_mode: draft.executionMode,
    max_parallel_agents:
      draft.executionMode === "swarm"
        ? Math.max(
            1,
            Math.min(16, Number.parseInt(String(draft.maxParallelAgents || "4"), 10) || 4)
          )
        : 1,
  };
  const modelProvider = String(draft.modelProvider || "").trim();
  const modelId = String(draft.modelId || "").trim();
  const plannerProvider = String(draft.plannerModelProvider || "").trim();
  const plannerModelId = String(draft.plannerModelId || "").trim();
  if (modelProvider) prefs.model_provider = modelProvider;
  if (modelId) prefs.model_id = modelId;
  if (plannerProvider && plannerModelId) {
    prefs.role_models = {
      planner: {
        provider_id: plannerProvider,
        model_id: plannerModelId,
      },
    };
  }
  return prefs;
}

function compileWorkflowModelPolicy(operatorPreferences: Record<string, unknown>) {
  const payload: Record<string, unknown> = {};
  const provider = String(operatorPreferences.model_provider || "").trim();
  const model = String(operatorPreferences.model_id || "").trim();
  const roleModels = operatorPreferences.role_models;
  if (provider && model) {
    payload.default_model = {
      provider_id: provider,
      model_id: model,
    };
  }
  if (roleModels && typeof roleModels === "object") {
    payload.role_models = roleModels;
  }
  return Object.keys(payload).length > 0 ? payload : null;
}

function normalizeMcpServerNamespace(raw: string) {
  let out = "";
  let previousUnderscore = false;
  for (const ch of String(raw || "").trim()) {
    if (/^[a-z0-9]$/i.test(ch)) {
      out += ch.toLowerCase();
      previousUnderscore = false;
    } else if (!previousUnderscore) {
      out += "_";
      previousUnderscore = true;
    }
  }
  return out.replace(/^_+|_+$/g, "") || "mcp";
}

function compileWorkflowToolAllowlist(selectedMcpServers: string[]) {
  return Array.from(
    new Set([
      "read",
      "websearch",
      "webfetch",
      "webfetch_html",
      ...selectedMcpServers.map((server) => `mcp.${normalizeMcpServerNamespace(server)}.*`),
    ])
  );
}

function extractAutomationOperatorPreferences(automation: AutomationV2Spec) {
  const metadataPrefs =
    automation?.metadata?.operator_preferences || automation?.metadata?.operatorPreferences;
  if (metadataPrefs && typeof metadataPrefs === "object") {
    return metadataPrefs as Record<string, unknown>;
  }
  const firstAgent = Array.isArray(automation?.agents) ? automation.agents[0] : null;
  const defaultModel =
    firstAgent?.model_policy?.default_model || firstAgent?.model_policy?.defaultModel || null;
  const roleModels =
    firstAgent?.model_policy?.role_models || firstAgent?.model_policy?.roleModels || null;
  const prefs: Record<string, unknown> = {};
  if (defaultModel && typeof defaultModel === "object") {
    const provider = String(
      (defaultModel as Record<string, unknown>).provider_id ||
        (defaultModel as Record<string, unknown>).providerId ||
        ""
    ).trim();
    const model = String(
      (defaultModel as Record<string, unknown>).model_id ||
        (defaultModel as Record<string, unknown>).modelId ||
        ""
    ).trim();
    if (provider) prefs.model_provider = provider;
    if (model) prefs.model_id = model;
  }
  if (roleModels && typeof roleModels === "object") {
    prefs.role_models = roleModels;
  }
  return prefs;
}

function scheduleToEditor(schedule: AutomationV2Spec["schedule"]) {
  const type = String(schedule?.type || "")
    .trim()
    .toLowerCase();
  const cronExpression = String(schedule?.cron_expression || "").trim();
  const intervalSeconds = String(
    Math.max(1, Number.parseInt(String(schedule?.interval_seconds || 3600), 10) || 3600)
  );
  return {
    scheduleKind:
      type === "manual"
        ? ("manual" as const)
        : type === "cron"
          ? ("cron" as const)
          : ("interval" as const),
    cronExpression,
    intervalSeconds,
  };
}

function workflowAutomationToEditDraft(automation: AutomationV2Spec): WorkflowEditDraft {
  const prefs = extractAutomationOperatorPreferences(automation);
  const plannerRoleModel =
    (prefs.role_models as Record<string, Record<string, unknown>> | undefined)?.planner || {};
  const schedule = scheduleToEditor(automation.schedule);
  const maxParallelRaw = Number(
    (automation.execution?.max_parallel_agents as number | undefined) || 1
  );
  const selectedMcpServers = Array.isArray(automation?.metadata?.allowed_mcp_servers)
    ? (automation.metadata.allowed_mcp_servers as string[])
    : Array.isArray(automation?.agents?.[0]?.mcp_policy?.allowed_servers)
      ? (automation.agents[0].mcp_policy?.allowed_servers as string[])
      : [];
  return {
    automationId: String(automation.automation_id || "").trim(),
    name: String(automation.name || "").trim(),
    description: String(automation.description || "").trim(),
    workspaceRoot: String(
      automation.workspace_root || automation.metadata?.workspace_root || ""
    ).trim(),
    scheduleKind: schedule.scheduleKind,
    intervalSeconds: schedule.intervalSeconds,
    cronExpression: schedule.cronExpression,
    executionMode: maxParallelRaw > 1 ? "swarm" : "team",
    maxParallelAgents: String(maxParallelRaw > 0 ? maxParallelRaw : 1),
    modelProvider: String(prefs.model_provider || "").trim(),
    modelId: String(prefs.model_id || "").trim(),
    plannerModelProvider: String(
      plannerRoleModel.provider_id || plannerRoleModel.providerId || ""
    ).trim(),
    plannerModelId: String(plannerRoleModel.model_id || plannerRoleModel.modelId || "").trim(),
    selectedMcpServers: selectedMcpServers.map((row) => String(row || "").trim()).filter(Boolean),
  };
}

function eventLooksRelevant(event: {
  source: string;
  payload: { type?: string; event_type?: string };
}) {
  const type = String(event.payload?.type || event.payload?.event_type || "").toLowerCase();
  if (event.source === "system") return false;
  return (
    type.includes("run") ||
    type.includes("automation") ||
    type.includes("workflow") ||
    type.includes("routine")
  );
}

function modelLabel(provider: string, model: string) {
  const providerValue = String(provider || "").trim();
  const modelValue = String(model || "").trim();
  if (!providerValue || !modelValue) return "Engine default";
  return `${providerValue}/${modelValue}`;
}

function runSummary(run: AutomationV2RunRecord) {
  return String(
    run?.checkpoint?.summary ||
      run?.checkpoint?.error ||
      run?.checkpoint?.status_detail ||
      run?.checkpoint?.statusDetail ||
      ""
  ).trim();
}

function shortText(raw: unknown, max = 160) {
  const text = String(raw || "")
    .replace(/\s+/g, " ")
    .trim();
  if (!text) return "";
  return text.length > max ? `${text.slice(0, max - 1).trimEnd()}...` : text;
}

function runDisplayTitle(run: AutomationV2RunRecord | null) {
  const explicitName = String((run as Record<string, unknown> | null)?.name || "").trim();
  if (explicitName) return explicitName;
  const checkpoint = (run?.checkpoint as Record<string, unknown> | undefined) || {};
  const objective = String(
    checkpoint.objective ||
      checkpoint.title ||
      checkpoint.summary ||
      checkpoint.status_detail ||
      checkpoint.statusDetail ||
      ""
  ).trim();
  if (objective) return shortText(objective, 96);
  const automationId = String(run?.automation_id || "").trim();
  return automationId || "Run";
}

function extractSessionIdsFromRun(run: AutomationV2RunRecord | null) {
  const direct = Array.isArray(run?.active_session_ids) ? run.active_session_ids : [];
  const checkpoint = (run?.checkpoint as Record<string, unknown> | undefined) || {};
  const latest = [
    String((run as Record<string, unknown> | null)?.latest_session_id || "").trim(),
    String((run as Record<string, unknown> | null)?.latestSessionId || "").trim(),
    String(checkpoint.latest_session_id || checkpoint.latestSessionId || "").trim(),
  ].filter(Boolean);
  const nodeOutputs =
    (checkpoint.node_outputs as Record<string, Record<string, unknown>>) ||
    (checkpoint.nodeOutputs as Record<string, Record<string, unknown>>) ||
    {};
  const nodeSessionIds = Object.values(nodeOutputs)
    .map((entry) => {
      const content = (entry?.content as Record<string, unknown> | undefined) || {};
      return String(content.session_id || content.sessionId || "").trim();
    })
    .filter(Boolean);
  return Array.from(
    new Set([...latest, ...direct.map((row) => String(row || "").trim()), ...nodeSessionIds])
  );
}

function extractRunNodeOutputs(run: AutomationV2RunRecord | null) {
  const checkpoint = (run?.checkpoint as Record<string, unknown> | undefined) || {};
  const outputs =
    (checkpoint.node_outputs as Record<string, Record<string, unknown>>) ||
    (checkpoint.nodeOutputs as Record<string, Record<string, unknown>>) ||
    {};
  return Object.entries(outputs).map(([nodeId, value]) => ({
    nodeId,
    value,
  }));
}

function nodeOutputText(value: Record<string, unknown>) {
  const summary = String(value?.summary || "").trim();
  const content = (value?.content as Record<string, unknown> | undefined) || {};
  const text = String(content.text || content.raw_text || "").trim();
  return [summary, text].filter(Boolean).join("\n").trim();
}

function sessionMessageText(message: SessionMessage) {
  const parts = Array.isArray(message?.parts) ? message.parts : [];
  const rows = parts
    .map((part) => {
      const row = (part as Record<string, unknown>) || {};
      const type = String(row.type || "").trim();
      if (type === "text" || type === "reasoning") return String(row.text || "").trim();
      if (type === "tool") {
        const tool = String(row.tool || "tool").trim();
        const error = String(row.error || "").trim();
        const result = row.result ? JSON.stringify(row.result, null, 2) : "";
        return [`tool: ${tool}`, error ? `error: ${error}` : "", result].filter(Boolean).join("\n");
      }
      return String(row.text || "").trim();
    })
    .filter(Boolean);
  return rows.join("\n\n").trim();
}

function sessionMessageVariant(message: SessionMessage) {
  const role = String(message?.info?.role || "")
    .trim()
    .toLowerCase();
  if (role === "user") return "user";
  if (role === "assistant") return "assistant";
  const body = sessionMessageText(message).toLowerCase();
  if (body.includes("error")) return "error";
  return "system";
}

function buildRunBlockers(run: AutomationV2RunRecord | null) {
  const blockers: Array<{ key: string; title: string; reason: string }> = [];
  const push = (key: string, title: string, reason: string) => {
    if (!reason.trim()) return;
    if (blockers.some((item) => item.key === key)) return;
    blockers.push({ key, title, reason });
  };
  const checkpoint = (run?.checkpoint as Record<string, unknown> | undefined) || {};
  const detail = String(
    checkpoint.error ||
      checkpoint.status_detail ||
      checkpoint.statusDetail ||
      checkpoint.summary ||
      ""
  ).trim();
  if (String(run?.status || "").trim() === "failed") {
    push("run-failed", "Run failed", detail || "Run finished with failed status.");
  }
  if (String(run?.status || "").trim() === "paused") {
    push("run-paused", "Run paused", detail || "Run was paused before completion.");
  }
  if (!extractSessionIdsFromRun(run).length) {
    push(
      "missing-session",
      "No linked session transcript",
      "This run does not currently expose a linked session transcript."
    );
  }
  for (const output of extractRunNodeOutputs(run)) {
    const body = nodeOutputText(output.value);
    const lower = body.toLowerCase();
    if (
      lower.includes("failed") ||
      lower.includes("error") ||
      lower.includes("blocked") ||
      lower.includes("timed out")
    ) {
      push(`node-${output.nodeId}`, `Node issue: ${output.nodeId}`, shortText(body, 320));
    }
  }
  return blockers;
}

function canPauseRun(run: AutomationV2RunRecord) {
  const status = String(run.status || "")
    .trim()
    .toLowerCase();
  return ["queued", "running", "pausing"].includes(status);
}

function canResumeRun(run: AutomationV2RunRecord) {
  const status = String(run.status || "")
    .trim()
    .toLowerCase();
  return status === "paused";
}

function canCancelRun(run: AutomationV2RunRecord) {
  const status = String(run.status || "")
    .trim()
    .toLowerCase();
  return ["queued", "running", "pausing", "paused"].includes(status);
}

function SectionCard({
  title,
  subtitle,
  children,
  actions,
}: {
  title: string;
  subtitle?: string;
  children: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <section className="rounded-xl border border-border bg-surface p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-semibold text-text">{title}</h3>
          {subtitle ? <p className="mt-1 text-xs text-text-muted">{subtitle}</p> : null}
        </div>
        {actions}
      </div>
      <div className="mt-4">{children}</div>
    </section>
  );
}

export function AgentAutomationPage({
  userProjects,
  activeProject,
  onSwitchProject,
  onAddProject,
  onManageProjects,
  projectSwitcherLoading = false,
  onOpenMcpExtensions,
}: AgentAutomationPageProps) {
  const [tab, setTab] = useState<PageTab>("create");
  const [wizardStep, setWizardStep] = useState<WizardStep>(1);
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [mcpServers, setMcpServers] = useState<McpServerRecord[]>([]);
  const [workflowAutomations, setWorkflowAutomations] = useState<AutomationV2Spec[]>([]);
  const [legacyRoutines, setLegacyRoutines] = useState<RoutineSpec[]>([]);
  const [workflowRuns, setWorkflowRuns] = useState<AutomationV2RunRecord[]>([]);
  const [wizard, setWizard] = useState<WizardState>(() => buildDefaultWizard(activeProject, []));
  const [planPreview, setPlanPreview] = useState<WorkflowPlan | null>(null);
  const [planningConversation, setPlanningConversation] = useState<WorkflowPlanConversation | null>(
    null
  );
  const [planningChangeSummary, setPlanningChangeSummary] = useState<string[]>([]);
  const [plannerDiagnostics, setPlannerDiagnostics] = useState<Record<string, unknown> | null>(
    null
  );
  const [planningMessage, setPlanningMessage] = useState("");
  const [editDraft, setEditDraft] = useState<WorkflowEditDraft | null>(null);
  const [selectedRunId, setSelectedRunId] = useState("");
  const [selectedRunDetail, setSelectedRunDetail] = useState<AutomationV2RunRecord | null>(null);
  const [selectedRunMessages, setSelectedRunMessages] = useState<SessionMessage[]>([]);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [loadingState, setLoadingState] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const providerModelMap = useMemo(
    () =>
      new Map(
        providers
          .map((provider) => [provider.id, provider.models] as const)
          .sort((a, b) => a[0].localeCompare(b[0]))
      ),
    [providers]
  );

  const wizardWorkspaceError = validateWorkspaceRootInput(wizard.workspaceRoot);
  const wizardModelError = validateModelInput(wizard.modelProvider, wizard.modelId);
  const wizardPlannerModelError = validatePlannerModelInput(
    wizard.plannerModelProvider,
    wizard.plannerModelId
  );

  const loadCatalog = async () => {
    const [providerRows, mcpRows] = await Promise.all([
      listProvidersFromSidecar(),
      mcpListServers(),
    ]);
    setProviders(providerRows);
    setMcpServers(mcpRows);
    setWizard((current) => buildDefaultWizard(activeProject, providerRows, current));
  };

  const loadAutomationState = async () => {
    const [workflowResponse, routineRows] = await Promise.all([
      automationsV2List(),
      routinesList(),
    ]);
    const automations = Array.isArray(workflowResponse?.automations)
      ? workflowResponse.automations
      : [];
    const runsPerAutomation = await Promise.all(
      automations.map(async (automation) => {
        const automationId = String(automation?.automation_id || "").trim();
        if (!automationId) return [];
        try {
          const response = await automationsV2Runs(automationId, 10);
          return Array.isArray(response?.runs) ? response.runs : [];
        } catch {
          return [];
        }
      })
    );
    setWorkflowAutomations(automations);
    setLegacyRoutines(routineRows);
    setWorkflowRuns(
      runsPerAutomation
        .flat()
        .sort(
          (a, b) =>
            Number(b?.updated_at_ms || b?.created_at_ms || 0) -
            Number(a?.updated_at_ms || a?.created_at_ms || 0)
        )
    );
  };

  const loadSelectedRunDetail = async (runId: string) => {
    const trimmed = String(runId || "").trim();
    if (!trimmed) {
      setSelectedRunDetail(null);
      setSelectedRunMessages([]);
      return;
    }
    setBusyKey(`inspect:${trimmed}`);
    try {
      const response = await automationsV2RunGet(trimmed);
      const run = response?.run || null;
      setSelectedRunDetail(run);
      const sessionIds = extractSessionIdsFromRun(run);
      if (sessionIds.length > 0) {
        const messages = await getSessionMessages(sessionIds[0]).catch(() => []);
        setSelectedRunMessages(messages);
      } else {
        setSelectedRunMessages([]);
      }
    } catch (inspectError) {
      setError(inspectError instanceof Error ? inspectError.message : String(inspectError));
    } finally {
      setBusyKey((current) => (current === `inspect:${trimmed}` ? null : current));
    }
  };

  const refreshAll = async () => {
    setLoadingState(true);
    try {
      await Promise.all([loadCatalog(), loadAutomationState()]);
      setError(null);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoadingState(false);
    }
  };

  useEffect(() => {
    void refreshAll();
  }, [activeProject?.id]);

  useEffect(() => {
    let refreshTimeout: ReturnType<typeof setTimeout> | null = null;
    let disposed = false;
    const start = async () => {
      const unlisten = await onSidecarEventV2((event) => {
        if (!eventLooksRelevant(event) || disposed) return;
        if (refreshTimeout) clearTimeout(refreshTimeout);
        refreshTimeout = setTimeout(() => {
          void loadAutomationState().catch(() => undefined);
          if (selectedRunId) {
            void loadSelectedRunDetail(selectedRunId).catch(() => undefined);
          }
        }, 500);
      });
      return unlisten;
    };
    let unlistenRef: (() => void) | null = null;
    void start().then((unlisten) => {
      unlistenRef = unlisten;
    });
    return () => {
      disposed = true;
      if (refreshTimeout) clearTimeout(refreshTimeout);
      if (unlistenRef) void unlistenRef();
    };
  }, [selectedRunId]);

  useEffect(() => {
    if (!selectedRunId) {
      setSelectedRunDetail(null);
      setSelectedRunMessages([]);
      return;
    }
    void loadSelectedRunDetail(selectedRunId);
  }, [selectedRunId]);

  const updateWizard = (patch: Partial<WizardState>) => {
    setWizard((current) => ({ ...current, ...patch }));
    if (planPreview) {
      setPlanPreview(null);
      setPlanningConversation(null);
      setPlanningChangeSummary([]);
      setPlannerDiagnostics(null);
    }
  };

  const generatePlan = async () => {
    setBusyKey("generate-plan");
    setError(null);
    try {
      const response = await workflowPlansChatStart({
        prompt: wizard.prompt,
        schedule: workflowEditToSchedule(wizard),
        plan_source: "desktop_automation_page",
        allowed_mcp_servers: wizard.selectedMcpServers,
        workspace_root: wizard.workspaceRoot,
        operator_preferences: workflowEditToOperatorPreferences(wizard),
      });
      setPlanPreview(response.plan || null);
      setPlanningConversation(response.conversation || null);
      setPlanningChangeSummary([]);
      setPlannerDiagnostics(
        (response.planner_diagnostics as Record<string, unknown> | null) || null
      );
    } catch (planError) {
      setError(planError instanceof Error ? planError.message : String(planError));
    } finally {
      setBusyKey(null);
    }
  };

  useEffect(() => {
    if (wizardStep !== 4 || planPreview || busyKey === "generate-plan") return;
    if (
      !wizard.prompt.trim() ||
      wizardWorkspaceError ||
      wizardModelError ||
      wizardPlannerModelError
    ) {
      return;
    }
    void generatePlan();
  }, [wizardStep]);

  const sendPlanningMessage = async () => {
    const nextMessage = String(planningMessage || "").trim();
    if (!planPreview?.plan_id || !nextMessage) return;
    setBusyKey("planning-message");
    setError(null);
    try {
      const response = await workflowPlansChatMessage({
        plan_id: planPreview.plan_id,
        message: nextMessage,
      });
      setPlanPreview(response.plan || null);
      setPlanningConversation(response.conversation || null);
      setPlanningChangeSummary(
        Array.isArray(response.change_summary) ? response.change_summary : []
      );
      setPlannerDiagnostics(
        (response.planner_diagnostics as Record<string, unknown> | null) || null
      );
      setPlanningMessage("");
    } catch (messageError) {
      setError(messageError instanceof Error ? messageError.message : String(messageError));
    } finally {
      setBusyKey(null);
    }
  };

  const resetPlanningChat = async () => {
    if (!planPreview?.plan_id) return;
    setBusyKey("planning-reset");
    setError(null);
    try {
      const response = await workflowPlansChatReset({
        plan_id: planPreview.plan_id,
      });
      setPlanPreview(response.plan || null);
      setPlanningConversation(response.conversation || null);
      setPlanningChangeSummary([]);
      setPlannerDiagnostics(
        (response.planner_diagnostics as Record<string, unknown> | null) || null
      );
      setPlanningMessage("");
    } catch (resetError) {
      setError(resetError instanceof Error ? resetError.message : String(resetError));
    } finally {
      setBusyKey(null);
    }
  };

  const createAutomation = async () => {
    if (!planPreview) {
      await generatePlan();
      return;
    }
    setBusyKey("apply-plan");
    setError(null);
    try {
      await workflowPlansApply({
        plan: planPreview,
        creator_id: "desktop",
        ...(wizard.exportPackDraft
          ? { pack_builder_export: { enabled: true, auto_apply: false } }
          : {}),
      });
      const nextWizard = buildDefaultWizard(activeProject, providers);
      setWizard(nextWizard);
      setWizardStep(1);
      setPlanPreview(null);
      setPlanningConversation(null);
      setPlanningChangeSummary([]);
      setPlannerDiagnostics(null);
      setPlanningMessage("");
      setTab("automations");
      await loadAutomationState();
    } catch (applyError) {
      setError(applyError instanceof Error ? applyError.message : String(applyError));
    } finally {
      setBusyKey(null);
    }
  };

  const openEditDraft = async (automationId: string) => {
    setBusyKey(`edit:${automationId}`);
    setError(null);
    try {
      const response = await automationsV2Get(automationId);
      if (!response.automation) throw new Error("Automation record not found.");
      setEditDraft(workflowAutomationToEditDraft(response.automation));
    } catch (editError) {
      setError(editError instanceof Error ? editError.message : String(editError));
    } finally {
      setBusyKey(null);
    }
  };

  const saveEditDraft = async () => {
    if (!editDraft) return;
    const workspaceError = validateWorkspaceRootInput(editDraft.workspaceRoot);
    const modelError = validateModelInput(editDraft.modelProvider, editDraft.modelId);
    const plannerError = validatePlannerModelInput(
      editDraft.plannerModelProvider,
      editDraft.plannerModelId
    );
    if (!editDraft.name.trim()) {
      setError("Automation name is required.");
      return;
    }
    if (workspaceError || modelError || plannerError) {
      setError(workspaceError || modelError || plannerError);
      return;
    }
    setBusyKey(`save:${editDraft.automationId}`);
    setError(null);
    try {
      const existingResponse = await automationsV2Get(editDraft.automationId);
      const existing = existingResponse.automation;
      const operatorPreferences = workflowEditToOperatorPreferences(editDraft);
      const modelPolicy = compileWorkflowModelPolicy(operatorPreferences);
      const selectedMcpServers = editDraft.selectedMcpServers
        .map((row) => String(row || "").trim())
        .filter(Boolean);
      const toolAllowlist = compileWorkflowToolAllowlist(selectedMcpServers);
      const agents = Array.isArray(existing?.agents)
        ? existing.agents.map((agent) => ({
            ...agent,
            model_policy: modelPolicy ?? agent.model_policy,
            tool_policy: {
              ...(agent.tool_policy || {}),
              allowlist: toolAllowlist,
              denylist: Array.isArray(agent.tool_policy?.denylist)
                ? agent.tool_policy.denylist
                : [],
            },
            mcp_policy: {
              ...(agent.mcp_policy || {}),
              allowed_servers: selectedMcpServers,
              allowed_tools: null,
            },
          }))
        : [];
      await automationsV2Update(editDraft.automationId, {
        name: editDraft.name.trim(),
        description: editDraft.description.trim() || null,
        workspace_root: editDraft.workspaceRoot.trim(),
        schedule: workflowEditToSchedule(editDraft),
        execution: {
          ...(existing.execution || {}),
          max_parallel_agents:
            editDraft.executionMode === "swarm"
              ? Math.max(
                  1,
                  Math.min(16, Number.parseInt(String(editDraft.maxParallelAgents || "4"), 10) || 4)
                )
              : 1,
        },
        agents,
        metadata: {
          ...(existing.metadata || {}),
          workspace_root: editDraft.workspaceRoot.trim(),
          operator_preferences: operatorPreferences,
          allowed_mcp_servers: selectedMcpServers,
        },
      });
      setEditDraft(null);
      await loadAutomationState();
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : String(saveError));
    } finally {
      setBusyKey(null);
    }
  };

  const toggleAutomationState = async (automation: AutomationV2Spec) => {
    const automationId = String(automation.automation_id || "").trim();
    if (!automationId) return;
    setBusyKey(`toggle:${automationId}`);
    setError(null);
    try {
      if (String(automation.status || "").trim() === "paused") {
        await automationsV2Resume(automationId);
      } else {
        await automationsV2Pause(automationId, "Paused from desktop automation page");
      }
      await loadAutomationState();
    } catch (toggleError) {
      setError(toggleError instanceof Error ? toggleError.message : String(toggleError));
    } finally {
      setBusyKey(null);
    }
  };

  const triggerAutomationRun = async (automationId: string) => {
    setBusyKey(`run:${automationId}`);
    setError(null);
    try {
      await automationsV2RunNow(automationId);
      setTab("runs");
      await loadAutomationState();
    } catch (runError) {
      setError(runError instanceof Error ? runError.message : String(runError));
    } finally {
      setBusyKey(null);
    }
  };

  const deleteAutomation = async (automationId: string, name: string) => {
    if (!window.confirm(`Delete workflow automation '${name}'?`)) return;
    setBusyKey(`delete:${automationId}`);
    setError(null);
    try {
      await automationsV2Delete(automationId);
      await loadAutomationState();
    } catch (deleteError) {
      setError(deleteError instanceof Error ? deleteError.message : String(deleteError));
    } finally {
      setBusyKey(null);
    }
  };

  const handleRunAction = async (runId: string, action: "pause" | "resume" | "cancel") => {
    setBusyKey(`${action}:${runId}`);
    setError(null);
    try {
      if (action === "pause") {
        await automationsV2RunPause(runId, "Paused from desktop automation page");
      } else if (action === "resume") {
        await automationsV2RunResume(runId, "Resumed from desktop automation page");
      } else {
        await automationsV2RunCancel(runId, "Cancelled from desktop automation page");
      }
      await loadAutomationState();
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : String(actionError));
    } finally {
      setBusyKey(null);
    }
  };

  const stepCanContinue =
    wizardStep === 1
      ? wizard.prompt.trim().length >= 8
      : wizardStep === 2
        ? wizard.scheduleKind === "manual"
          ? true
          : wizard.scheduleKind === "cron"
            ? wizard.cronExpression.trim().length > 0
            : Number.parseInt(String(wizard.intervalSeconds || "0"), 10) > 0
        : wizardStep === 3
          ? !wizardWorkspaceError && !wizardModelError && !wizardPlannerModelError
          : true;

  const configuredWorkflowCount = workflowAutomations.length;
  const activeWorkflowCount = workflowAutomations.filter(
    (automation) => String(automation.status || "").trim() === "active"
  ).length;
  const runningWorkflowCount = workflowRuns.filter((run) =>
    ["queued", "running", "pausing"].includes(String(run.status || "").trim())
  ).length;
  const failedWorkflowCount = workflowRuns.filter(
    (run) => String(run.status || "").trim() === "failed"
  ).length;
  const selectedRunBlockers = useMemo(
    () => buildRunBlockers(selectedRunDetail),
    [selectedRunDetail]
  );
  const selectedRunNodeOutputs = useMemo(
    () => extractRunNodeOutputs(selectedRunDetail),
    [selectedRunDetail]
  );
  const selectedRunSessionIds = useMemo(
    () => extractSessionIdsFromRun(selectedRunDetail),
    [selectedRunDetail]
  );

  return (
    <div className="h-full overflow-y-auto p-4">
      <div className="mx-auto max-w-[1480px] space-y-4">
        <SectionCard
          title="Desktop Automation"
          subtitle="Workflow-first desktop automation with shared engine state."
          actions={
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                variant={tab === "create" ? "primary" : "secondary"}
                onClick={() => setTab("create")}
              >
                Create
              </Button>
              <Button
                size="sm"
                variant={tab === "automations" ? "primary" : "secondary"}
                onClick={() => setTab("automations")}
              >
                My Automations
              </Button>
              <Button
                size="sm"
                variant={tab === "runs" ? "primary" : "secondary"}
                onClick={() => setTab("runs")}
              >
                Live Tasks
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void refreshAll()}>
                Refresh
              </Button>
            </div>
          }
        >
          <div className="grid gap-3 sm:grid-cols-4">
            <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
              <div className="text-[10px] uppercase tracking-wide text-text-subtle">
                Workflow Automations
              </div>
              <div className="mt-1 text-2xl font-semibold text-text">{configuredWorkflowCount}</div>
            </div>
            <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
              <div className="text-[10px] uppercase tracking-wide text-text-subtle">Active</div>
              <div className="mt-1 text-2xl font-semibold text-text">{activeWorkflowCount}</div>
            </div>
            <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
              <div className="text-[10px] uppercase tracking-wide text-text-subtle">Running</div>
              <div className="mt-1 text-2xl font-semibold text-text">{runningWorkflowCount}</div>
            </div>
            <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
              <div className="text-[10px] uppercase tracking-wide text-text-subtle">Failed</div>
              <div className="mt-1 text-2xl font-semibold text-text">{failedWorkflowCount}</div>
            </div>
          </div>
        </SectionCard>

        <SectionCard
          title="Project Context"
          subtitle="Workspace selection is shared with desktop projects."
        >
          <ProjectSwitcher
            projects={userProjects}
            activeProject={activeProject}
            onSwitchProject={onSwitchProject}
            onAddProject={onAddProject}
            onManageProjects={onManageProjects}
            isLoading={projectSwitcherLoading}
          />
        </SectionCard>

        {error ? (
          <div className="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-200">
            {error}
          </div>
        ) : null}

        {loadingState ? (
          <div className="rounded-lg border border-border bg-surface px-4 py-8 text-center text-sm text-text-muted">
            Loading automation state...
          </div>
        ) : null}

        {!loadingState && tab === "create" ? (
          <>
            <SectionCard
              title="Create Workflow Automation"
              subtitle="Desktop-native flow backed by workflow-plans and automations v2."
              actions={
                <div className="flex items-center gap-2 text-xs text-text-muted">
                  <span>Step {wizardStep} of 4</span>
                </div>
              }
            >
              <div className="grid gap-2 sm:grid-cols-4">
                {[
                  { step: 1 as WizardStep, label: "What" },
                  { step: 2 as WizardStep, label: "When" },
                  { step: 3 as WizardStep, label: "How" },
                  { step: 4 as WizardStep, label: "Review" },
                ].map((entry) => (
                  <button
                    key={entry.step}
                    type="button"
                    className={`rounded-lg border px-3 py-2 text-left text-sm ${
                      wizardStep === entry.step
                        ? "border-primary bg-primary/10 text-text"
                        : wizardStep > entry.step
                          ? "border-border bg-surface-elevated/40 text-text"
                          : "border-border bg-surface text-text-muted"
                    }`}
                    onClick={() => {
                      if (entry.step <= wizardStep) setWizardStep(entry.step);
                    }}
                  >
                    <div className="text-[10px] uppercase tracking-wide text-text-subtle">
                      Step {entry.step}
                    </div>
                    <div className="mt-1 font-medium">{entry.label}</div>
                  </button>
                ))}
              </div>

              {wizardStep === 1 ? (
                <div className="mt-4 space-y-3">
                  <label className="block text-sm font-medium text-text">Objective</label>
                  <textarea
                    value={wizard.prompt}
                    onChange={(event) => updateWizard({ prompt: event.target.value })}
                    className="min-h-[160px] w-full rounded-lg border border-border bg-surface px-3 py-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                    placeholder="Describe what the automation should accomplish, what signals it should use, and what output you want."
                  />
                </div>
              ) : null}

              {wizardStep === 2 ? (
                <div className="mt-4 space-y-4">
                  <div className="grid gap-2 sm:grid-cols-3">
                    {[
                      { kind: "manual" as ScheduleKind, label: "Manual" },
                      { kind: "interval" as ScheduleKind, label: "Interval" },
                      { kind: "cron" as ScheduleKind, label: "Cron" },
                    ].map((entry) => (
                      <button
                        key={entry.kind}
                        type="button"
                        className={`rounded-lg border px-3 py-3 text-left ${
                          wizard.scheduleKind === entry.kind
                            ? "border-primary bg-primary/10"
                            : "border-border bg-surface-elevated/40"
                        }`}
                        onClick={() => updateWizard({ scheduleKind: entry.kind })}
                      >
                        <div className="text-sm font-medium text-text">{entry.label}</div>
                        <div className="mt-1 text-xs text-text-muted">
                          {entry.kind === "manual"
                            ? "Run only when triggered."
                            : entry.kind === "interval"
                              ? "Repeat on a fixed cadence."
                              : "Use an explicit cron expression."}
                        </div>
                      </button>
                    ))}
                  </div>
                  {wizard.scheduleKind === "interval" ? (
                    <Input
                      label="Interval Seconds"
                      type="number"
                      min={1}
                      value={wizard.intervalSeconds}
                      onChange={(event) => updateWizard({ intervalSeconds: event.target.value })}
                    />
                  ) : null}
                  {wizard.scheduleKind === "cron" ? (
                    <Input
                      label="Cron Expression"
                      value={wizard.cronExpression}
                      onChange={(event) => updateWizard({ cronExpression: event.target.value })}
                    />
                  ) : null}
                  <div className="rounded-lg border border-border bg-surface-elevated/40 px-3 py-2 text-sm text-text-muted">
                    Schedule preview:{" "}
                    <span className="font-medium text-text">
                      {formatSchedule(
                        wizard.scheduleKind,
                        wizard.intervalSeconds,
                        wizard.cronExpression
                      )}
                    </span>
                  </div>
                </div>
              ) : null}

              {wizardStep === 3 ? (
                <div className="mt-4 grid gap-4 lg:grid-cols-2">
                  <div className="space-y-3">
                    <Input
                      label="Workspace Root"
                      value={wizard.workspaceRoot}
                      error={wizardWorkspaceError || undefined}
                      onChange={(event) => updateWizard({ workspaceRoot: event.target.value })}
                    />
                    <div className="grid gap-3 sm:grid-cols-2">
                      <label className="block text-sm font-medium text-text">
                        Execution Mode
                        <select
                          value={wizard.executionMode}
                          onChange={(event) =>
                            updateWizard({
                              executionMode: event.target.value === "swarm" ? "swarm" : "team",
                            })
                          }
                          className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                        >
                          <option value="team">Team</option>
                          <option value="swarm">Swarm</option>
                        </select>
                      </label>
                      <Input
                        label="Max Parallel Agents"
                        type="number"
                        min={1}
                        max={16}
                        value={wizard.maxParallelAgents}
                        onChange={(event) =>
                          updateWizard({ maxParallelAgents: event.target.value })
                        }
                      />
                    </div>
                    <div className="grid gap-3 sm:grid-cols-2">
                      <label className="block text-sm font-medium text-text">
                        Workflow Provider
                        <select
                          value={wizard.modelProvider}
                          onChange={(event) => {
                            const nextProvider = event.target.value;
                            updateWizard({
                              modelProvider: nextProvider,
                              modelId: providerModelMap.get(nextProvider)?.[0] ?? "",
                            });
                          }}
                          className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                        >
                          <option value="">Engine default</option>
                          {providers.map((provider) => (
                            <option key={provider.id} value={provider.id}>
                              {provider.id}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="block text-sm font-medium text-text">
                        Workflow Model
                        <select
                          value={wizard.modelId}
                          onChange={(event) => updateWizard({ modelId: event.target.value })}
                          className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                        >
                          <option value="">Engine default</option>
                          {(providerModelMap.get(wizard.modelProvider) ?? []).map((modelId) => (
                            <option key={modelId} value={modelId}>
                              {modelId}
                            </option>
                          ))}
                        </select>
                      </label>
                    </div>
                    {wizardModelError ? (
                      <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
                        {wizardModelError}
                      </div>
                    ) : null}
                    <div className="grid gap-3 sm:grid-cols-2">
                      <label className="block text-sm font-medium text-text">
                        Planner Provider
                        <select
                          value={wizard.plannerModelProvider}
                          onChange={(event) => {
                            const nextProvider = event.target.value;
                            updateWizard({
                              plannerModelProvider: nextProvider,
                              plannerModelId: providerModelMap.get(nextProvider)?.[0] ?? "",
                            });
                          }}
                          className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                        >
                          <option value="">Fallback to workflow model</option>
                          {providers.map((provider) => (
                            <option key={provider.id} value={provider.id}>
                              {provider.id}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="block text-sm font-medium text-text">
                        Planner Model
                        <select
                          value={wizard.plannerModelId}
                          onChange={(event) => updateWizard({ plannerModelId: event.target.value })}
                          className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                        >
                          <option value="">Fallback to workflow model</option>
                          {(providerModelMap.get(wizard.plannerModelProvider) ?? []).map(
                            (modelId) => (
                              <option key={modelId} value={modelId}>
                                {modelId}
                              </option>
                            )
                          )}
                        </select>
                      </label>
                    </div>
                    {wizardPlannerModelError ? (
                      <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
                        {wizardPlannerModelError}
                      </div>
                    ) : null}
                  </div>
                  <div className="space-y-3">
                    <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
                      <div className="flex items-center justify-between gap-2">
                        <div>
                          <div className="text-sm font-medium text-text">Allowed MCP Servers</div>
                          <div className="text-xs text-text-muted">
                            Selected servers are written into operator preferences and agent MCP
                            policy.
                          </div>
                        </div>
                        {onOpenMcpExtensions ? (
                          <Button size="sm" variant="secondary" onClick={onOpenMcpExtensions}>
                            Manage MCP
                          </Button>
                        ) : null}
                      </div>
                      <div className="mt-3 space-y-2">
                        {mcpServers.map((server) => {
                          const checked = wizard.selectedMcpServers.includes(server.name);
                          return (
                            <label
                              key={server.name}
                              className="flex items-start gap-2 rounded-lg border border-border bg-surface px-3 py-2 text-sm text-text"
                            >
                              <input
                                type="checkbox"
                                checked={checked}
                                onChange={() =>
                                  updateWizard({
                                    selectedMcpServers: checked
                                      ? wizard.selectedMcpServers.filter(
                                          (row) => row !== server.name
                                        )
                                      : [...wizard.selectedMcpServers, server.name],
                                  })
                                }
                              />
                              <span className="min-w-0">
                                <span className="block font-medium">{server.name}</span>
                                <span className="block text-xs text-text-muted">
                                  {server.connected ? "connected" : "disconnected"} |{" "}
                                  {server.enabled ? "enabled" : "disabled"}
                                </span>
                              </span>
                            </label>
                          );
                        })}
                        {mcpServers.length === 0 ? (
                          <div className="rounded-lg border border-border bg-surface px-3 py-2 text-sm text-text-muted">
                            No MCP servers configured.
                          </div>
                        ) : null}
                      </div>
                    </div>
                  </div>
                </div>
              ) : null}

              {wizardStep === 4 ? (
                <div className="mt-4 space-y-4">
                  <div className="grid gap-3 lg:grid-cols-[1.2fr_0.8fr]">
                    <div className="space-y-3 rounded-lg border border-border bg-surface-elevated/40 p-3">
                      <div className="flex items-center justify-between gap-2">
                        <div>
                          <div className="text-sm font-medium text-text">Plan Preview</div>
                          <div className="text-xs text-text-muted">
                            Review and revise before creating the automation.
                          </div>
                        </div>
                        <Button
                          size="sm"
                          variant="secondary"
                          loading={busyKey === "generate-plan"}
                          onClick={() => void generatePlan()}
                        >
                          Regenerate
                        </Button>
                      </div>
                      {planPreview ? (
                        <div className="space-y-3">
                          <div>
                            <div className="text-xs uppercase tracking-wide text-text-subtle">
                              Title
                            </div>
                            <div className="mt-1 text-sm font-medium text-text">
                              {planPreview.title || "Untitled plan"}
                            </div>
                          </div>
                          {planPreview.description ? (
                            <div className="text-sm text-text-muted">{planPreview.description}</div>
                          ) : null}
                          <div className="rounded-lg border border-border bg-surface px-3 py-2 text-sm text-text-muted">
                            Workspace:{" "}
                            <span className="font-medium text-text">
                              {planPreview.workspace_root || wizard.workspaceRoot}
                            </span>
                          </div>
                          <div className="space-y-2">
                            {planPreview.steps.map((step, index) => (
                              <div
                                key={step.step_id || `${step.kind}-${index}`}
                                className="rounded-lg border border-border bg-surface px-3 py-2"
                              >
                                <div className="text-xs uppercase tracking-wide text-text-subtle">
                                  {step.kind}
                                </div>
                                <div className="mt-1 text-sm font-medium text-text">
                                  {step.objective}
                                </div>
                                {step.depends_on?.length ? (
                                  <div className="mt-1 text-xs text-text-muted">
                                    Depends on: {step.depends_on.join(", ")}
                                  </div>
                                ) : null}
                              </div>
                            ))}
                          </div>
                        </div>
                      ) : (
                        <div className="rounded-lg border border-border bg-surface px-3 py-6 text-center text-sm text-text-muted">
                          {busyKey === "generate-plan"
                            ? "Generating workflow plan..."
                            : "No plan yet."}
                        </div>
                      )}
                    </div>
                    <div className="space-y-3 rounded-lg border border-border bg-surface-elevated/40 p-3">
                      <div className="text-sm font-medium text-text">Planning Chat</div>
                      <div className="max-h-[320px] space-y-2 overflow-y-auto rounded-lg border border-border bg-surface px-3 py-3">
                        {(planningConversation?.messages || []).map((message, index) => (
                          <div key={`${message.created_at_ms || index}-${index}`}>
                            <div className="text-[10px] uppercase tracking-wide text-text-subtle">
                              {message.role}
                            </div>
                            <div className="mt-1 whitespace-pre-wrap text-sm text-text">
                              {message.text}
                            </div>
                          </div>
                        ))}
                        {!planningConversation?.messages?.length ? (
                          <div className="text-sm text-text-muted">
                            The workflow planning conversation will appear here.
                          </div>
                        ) : null}
                      </div>
                      <textarea
                        value={planningMessage}
                        onChange={(event) => setPlanningMessage(event.target.value)}
                        className="min-h-[88px] w-full rounded-lg border border-border bg-surface px-3 py-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                        placeholder="Ask for revisions, clarify the plan, or tighten the workflow."
                      />
                      <div className="flex flex-wrap gap-2">
                        <Button
                          size="sm"
                          variant="secondary"
                          loading={busyKey === "planning-message"}
                          onClick={() => void sendPlanningMessage()}
                        >
                          Send revision
                        </Button>
                        <Button
                          size="sm"
                          variant="secondary"
                          loading={busyKey === "planning-reset"}
                          onClick={() => void resetPlanningChat()}
                        >
                          Reset chat
                        </Button>
                      </div>
                      {planningChangeSummary.length ? (
                        <div className="rounded-lg border border-border bg-surface px-3 py-2">
                          <div className="text-[10px] uppercase tracking-wide text-text-subtle">
                            Change Summary
                          </div>
                          <ul className="mt-2 space-y-1 text-sm text-text">
                            {planningChangeSummary.map((item) => (
                              <li key={item}>- {item}</li>
                            ))}
                          </ul>
                        </div>
                      ) : null}
                      {plannerDiagnostics ? (
                        <div className="rounded-lg border border-border bg-surface px-3 py-2">
                          <div className="text-[10px] uppercase tracking-wide text-text-subtle">
                            Planner Diagnostics
                          </div>
                          <pre className="mt-2 overflow-x-auto whitespace-pre-wrap text-xs text-text-muted">
                            {JSON.stringify(plannerDiagnostics, null, 2)}
                          </pre>
                        </div>
                      ) : null}
                      <label className="inline-flex items-center gap-2 text-sm text-text-muted">
                        <input
                          type="checkbox"
                          checked={wizard.exportPackDraft}
                          onChange={(event) =>
                            updateWizard({ exportPackDraft: event.target.checked })
                          }
                        />
                        Also export Pack Builder draft
                      </label>
                    </div>
                  </div>
                </div>
              ) : null}

              <div className="mt-4 flex items-center justify-between gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={wizardStep === 1}
                  onClick={() => setWizardStep((current) => Math.max(1, current - 1) as WizardStep)}
                >
                  Back
                </Button>
                {wizardStep < 4 ? (
                  <Button
                    size="sm"
                    variant="primary"
                    disabled={!stepCanContinue}
                    onClick={() =>
                      setWizardStep((current) => Math.min(4, current + 1) as WizardStep)
                    }
                  >
                    Next
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="primary"
                    loading={busyKey === "apply-plan"}
                    onClick={() => void createAutomation()}
                  >
                    Create Automation
                  </Button>
                )}
              </div>
            </SectionCard>
          </>
        ) : null}

        {!loadingState && tab === "automations" ? (
          <>
            <SectionCard
              title="My Automations"
              subtitle="Workflow automations backed by shared engine state."
            >
              <div className="space-y-3">
                {workflowAutomations.map((automation) => {
                  const automationId = String(automation.automation_id || "").trim();
                  const draft = extractAutomationOperatorPreferences(automation);
                  const plannerRoleModel =
                    (draft.role_models as Record<string, Record<string, unknown>> | undefined)
                      ?.planner || {};
                  const runCount = workflowRuns.filter(
                    (run) => run.automation_id === automationId
                  ).length;
                  return (
                    <div
                      key={automationId}
                      className="rounded-lg border border-border bg-surface-elevated/40 p-3"
                    >
                      <div className="flex flex-wrap items-start justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <div className="text-sm font-semibold text-text">
                              {automation.name || automationId}
                            </div>
                            <span className="rounded border border-border bg-surface px-2 py-0.5 text-[10px] uppercase tracking-wide text-text-subtle">
                              {automation.status || "active"}
                            </span>
                          </div>
                          {automation.description ? (
                            <div className="mt-1 text-sm text-text-muted">
                              {automation.description}
                            </div>
                          ) : null}
                          <div className="mt-2 grid gap-2 text-xs text-text-muted sm:grid-cols-2 lg:grid-cols-4">
                            <div>Workspace: {automation.workspace_root || "n/a"}</div>
                            <div>
                              Schedule:{" "}
                              {formatSchedule(
                                scheduleToEditor(automation.schedule).scheduleKind,
                                scheduleToEditor(automation.schedule).intervalSeconds,
                                scheduleToEditor(automation.schedule).cronExpression
                              )}
                            </div>
                            <div>
                              Model:{" "}
                              {modelLabel(
                                String(draft.model_provider || ""),
                                String(draft.model_id || "")
                              )}
                            </div>
                            <div>
                              Planner:{" "}
                              {modelLabel(
                                String(
                                  plannerRoleModel.provider_id || plannerRoleModel.providerId || ""
                                ),
                                String(plannerRoleModel.model_id || plannerRoleModel.modelId || "")
                              )}
                            </div>
                            <div>
                              MCP:{" "}
                              {Array.isArray(automation.metadata?.allowed_mcp_servers)
                                ? (automation.metadata?.allowed_mcp_servers as string[]).join(
                                    ", "
                                  ) || "none"
                                : "none"}
                            </div>
                            <div>Runs loaded: {runCount}</div>
                          </div>
                        </div>
                        <div className="flex flex-wrap gap-2">
                          <Button
                            size="sm"
                            variant="secondary"
                            loading={busyKey === `run:${automationId}`}
                            onClick={() => void triggerAutomationRun(automationId)}
                          >
                            Run now
                          </Button>
                          <Button
                            size="sm"
                            variant="secondary"
                            loading={busyKey === `edit:${automationId}`}
                            onClick={() => void openEditDraft(automationId)}
                          >
                            Edit
                          </Button>
                          <Button
                            size="sm"
                            variant="secondary"
                            disabled={
                              !workflowRuns.some(
                                (run) => String(run.automation_id || "").trim() === automationId
                              )
                            }
                            onClick={() => {
                              const latestRun = workflowRuns.find(
                                (run) => String(run.automation_id || "").trim() === automationId
                              );
                              if (!latestRun?.run_id) return;
                              setSelectedRunId(latestRun.run_id);
                              setTab("runs");
                            }}
                          >
                            Inspect latest
                          </Button>
                          <Button
                            size="sm"
                            variant="secondary"
                            loading={busyKey === `toggle:${automationId}`}
                            onClick={() => void toggleAutomationState(automation)}
                          >
                            {String(automation.status || "").trim() === "paused"
                              ? "Resume"
                              : "Pause"}
                          </Button>
                          <Button
                            size="sm"
                            variant="danger"
                            loading={busyKey === `delete:${automationId}`}
                            onClick={() => void deleteAutomation(automationId, automation.name)}
                          >
                            Delete
                          </Button>
                        </div>
                      </div>
                    </div>
                  );
                })}
                {workflowAutomations.length === 0 ? (
                  <div className="rounded-lg border border-border bg-surface px-3 py-6 text-center text-sm text-text-muted">
                    No workflow automations found yet.
                  </div>
                ) : null}
              </div>
            </SectionCard>

            <SectionCard
              title="Legacy Compatibility"
              subtitle="Existing routine/legacy automation records remain visible during migration."
            >
              <div className="space-y-2">
                {legacyRoutines.map((routine) => (
                  <div
                    key={routine.routine_id}
                    className="rounded-lg border border-border bg-surface-elevated/40 px-3 py-2"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div>
                        <div className="text-sm font-medium text-text">{routine.name}</div>
                        <div className="text-xs text-text-muted">
                          {routine.routine_id} | {routine.status} | {routine.entrypoint}
                        </div>
                      </div>
                      <div className="text-xs text-text-subtle">
                        {routine.output_targets.length} output target(s)
                      </div>
                    </div>
                  </div>
                ))}
                {legacyRoutines.length === 0 ? (
                  <div className="rounded-lg border border-border bg-surface px-3 py-6 text-center text-sm text-text-muted">
                    No legacy routines detected.
                  </div>
                ) : null}
              </div>
            </SectionCard>
          </>
        ) : null}

        {!loadingState && tab === "runs" ? (
          <SectionCard
            title="Live Tasks"
            subtitle="Recent workflow runs with desktop pause, resume, and cancel controls."
          >
            <div className="space-y-3">
              {workflowRuns.map((run) => (
                <div
                  key={run.run_id}
                  className="rounded-lg border border-border bg-surface-elevated/40 p-3"
                >
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <div className="text-sm font-semibold text-text">
                          {runDisplayTitle(run)}
                        </div>
                        <span className="rounded border border-border bg-surface px-2 py-0.5 text-[10px] uppercase tracking-wide text-text-subtle">
                          {run.status}
                        </span>
                      </div>
                      <div className="mt-1 text-xs text-text-muted">
                        Run ID: {run.run_id} | Automation: {run.automation_id}
                      </div>
                      <div className="mt-1 text-xs text-text-muted">
                        Updated:{" "}
                        {formatDateTime(
                          (run as Record<string, unknown>).updated_at_ms ||
                            (run as Record<string, unknown>).created_at_ms
                        )}
                      </div>
                      {runSummary(run) ? (
                        <div className="mt-2 rounded-lg border border-border bg-surface px-3 py-2 text-sm text-text-muted">
                          {runSummary(run)}
                        </div>
                      ) : null}
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={!canPauseRun(run)}
                        loading={busyKey === `pause:${run.run_id}`}
                        onClick={() => void handleRunAction(run.run_id, "pause")}
                      >
                        Pause
                      </Button>
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={!canResumeRun(run)}
                        loading={busyKey === `resume:${run.run_id}`}
                        onClick={() => void handleRunAction(run.run_id, "resume")}
                      >
                        Resume
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        disabled={!canCancelRun(run)}
                        loading={busyKey === `cancel:${run.run_id}`}
                        onClick={() => void handleRunAction(run.run_id, "cancel")}
                      >
                        Cancel
                      </Button>
                      <Button
                        size="sm"
                        variant={selectedRunId === run.run_id ? "primary" : "secondary"}
                        onClick={() => setSelectedRunId(run.run_id)}
                      >
                        Inspect
                      </Button>
                    </div>
                  </div>
                </div>
              ))}
              {workflowRuns.length === 0 ? (
                <div className="rounded-lg border border-border bg-surface px-3 py-6 text-center text-sm text-text-muted">
                  No workflow runs found yet.
                </div>
              ) : null}
            </div>
          </SectionCard>
        ) : null}

        {!loadingState && tab === "runs" && selectedRunDetail ? (
          <SectionCard
            title={`Run Inspector: ${runDisplayTitle(selectedRunDetail)}`}
            subtitle="Reopen completed or failed workflow runs and inspect node outputs plus linked session transcripts."
            actions={
              <div className="flex gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  loading={busyKey === `inspect:${selectedRunId}`}
                  onClick={() => void loadSelectedRunDetail(selectedRunId)}
                >
                  Refresh Run
                </Button>
                <Button size="sm" variant="secondary" onClick={() => setSelectedRunId("")}>
                  Close
                </Button>
              </div>
            }
          >
            <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
              <div className="space-y-4">
                <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
                  <div className="grid gap-2 text-sm text-text-muted">
                    <div>
                      <span className="font-medium text-text">Run ID:</span>{" "}
                      {selectedRunDetail.run_id}
                    </div>
                    <div>
                      <span className="font-medium text-text">Automation:</span>{" "}
                      {selectedRunDetail.automation_id}
                    </div>
                    <div>
                      <span className="font-medium text-text">Status:</span>{" "}
                      {selectedRunDetail.status}
                    </div>
                    <div>
                      <span className="font-medium text-text">Created:</span>{" "}
                      {formatDateTime((selectedRunDetail as Record<string, unknown>).created_at_ms)}
                    </div>
                    <div>
                      <span className="font-medium text-text">Updated:</span>{" "}
                      {formatDateTime((selectedRunDetail as Record<string, unknown>).updated_at_ms)}
                    </div>
                    <div>
                      <span className="font-medium text-text">Linked Sessions:</span>{" "}
                      {selectedRunSessionIds.join(", ") || "none"}
                    </div>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={!canPauseRun(selectedRunDetail)}
                      loading={busyKey === `pause:${selectedRunDetail.run_id}`}
                      onClick={() => void handleRunAction(selectedRunDetail.run_id, "pause")}
                    >
                      Pause
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={!canResumeRun(selectedRunDetail)}
                      loading={busyKey === `resume:${selectedRunDetail.run_id}`}
                      onClick={() => void handleRunAction(selectedRunDetail.run_id, "resume")}
                    >
                      Resume
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      disabled={!canCancelRun(selectedRunDetail)}
                      loading={busyKey === `cancel:${selectedRunDetail.run_id}`}
                      onClick={() => void handleRunAction(selectedRunDetail.run_id, "cancel")}
                    >
                      Cancel
                    </Button>
                  </div>
                  {runSummary(selectedRunDetail) ? (
                    <div className="mt-3 rounded-lg border border-border bg-surface px-3 py-2 text-sm text-text-muted">
                      {runSummary(selectedRunDetail)}
                    </div>
                  ) : null}
                </div>

                <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
                  <div className="text-sm font-medium text-text">Blockers / Errors</div>
                  <div className="mt-3 space-y-2">
                    {selectedRunBlockers.map((blocker) => (
                      <div
                        key={blocker.key}
                        className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2"
                      >
                        <div className="text-sm font-medium text-red-100">{blocker.title}</div>
                        <div className="mt-1 text-sm text-red-200">{blocker.reason}</div>
                      </div>
                    ))}
                    {selectedRunBlockers.length === 0 ? (
                      <div className="rounded-lg border border-border bg-surface px-3 py-2 text-sm text-text-muted">
                        No explicit blockers extracted from the current run payload.
                      </div>
                    ) : null}
                  </div>
                </div>

                <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
                  <div className="text-sm font-medium text-text">Node Outputs</div>
                  <div className="mt-3 space-y-2">
                    {selectedRunNodeOutputs.map((output) => (
                      <div
                        key={output.nodeId}
                        className="rounded-lg border border-border bg-surface px-3 py-2"
                      >
                        <div className="text-xs uppercase tracking-wide text-text-subtle">
                          {output.nodeId}
                        </div>
                        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap text-xs text-text-muted">
                          {nodeOutputText(output.value) || JSON.stringify(output.value, null, 2)}
                        </pre>
                      </div>
                    ))}
                    {selectedRunNodeOutputs.length === 0 ? (
                      <div className="rounded-lg border border-border bg-surface px-3 py-2 text-sm text-text-muted">
                        No node outputs were found on this run checkpoint.
                      </div>
                    ) : null}
                  </div>
                </div>
              </div>

              <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
                <div className="flex items-center justify-between gap-2">
                  <div className="text-sm font-medium text-text">Linked Session Transcript</div>
                  <div className="text-xs text-text-muted">
                    {selectedRunMessages.length} message(s)
                  </div>
                </div>
                <div className="mt-3 max-h-[720px] space-y-3 overflow-y-auto rounded-lg border border-border bg-surface px-3 py-3">
                  {selectedRunMessages.map((message, index) => {
                    const variant = sessionMessageVariant(message);
                    return (
                      <div
                        key={`${message.info?.id || index}-${index}`}
                        className={`rounded-lg border px-3 py-2 ${
                          variant === "user"
                            ? "border-primary/30 bg-primary/10"
                            : variant === "assistant"
                              ? "border-border bg-surface-elevated/60"
                              : variant === "error"
                                ? "border-red-500/30 bg-red-500/10"
                                : "border-border bg-surface"
                        }`}
                      >
                        <div className="text-[10px] uppercase tracking-wide text-text-subtle">
                          {message.info?.role || "session"} |{" "}
                          {formatDateTime(message.info?.time?.created)}
                        </div>
                        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap text-xs text-text">
                          {sessionMessageText(message) || "(empty message)"}
                        </pre>
                      </div>
                    );
                  })}
                  {selectedRunMessages.length === 0 ? (
                    <div className="rounded-lg border border-border bg-surface px-3 py-4 text-sm text-text-muted">
                      No linked session messages were found for this run.
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          </SectionCard>
        ) : null}

        {editDraft ? (
          <SectionCard
            title={`Edit ${editDraft.name || editDraft.automationId}`}
            subtitle="Workflow edit parity for name, schedule, workspace, execution, models, and MCP."
            actions={
              <Button size="sm" variant="secondary" onClick={() => setEditDraft(null)}>
                Close
              </Button>
            }
          >
            <div className="grid gap-4 lg:grid-cols-2">
              <div className="space-y-3">
                <Input
                  label="Name"
                  value={editDraft.name}
                  onChange={(event) =>
                    setEditDraft((current) =>
                      current ? { ...current, name: event.target.value } : current
                    )
                  }
                />
                <label className="block text-sm font-medium text-text">
                  Description
                  <textarea
                    value={editDraft.description}
                    onChange={(event) =>
                      setEditDraft((current) =>
                        current ? { ...current, description: event.target.value } : current
                      )
                    }
                    className="mt-2 min-h-[120px] w-full rounded-lg border border-border bg-surface px-3 py-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                  />
                </label>
                <Input
                  label="Workspace Root"
                  value={editDraft.workspaceRoot}
                  onChange={(event) =>
                    setEditDraft((current) =>
                      current ? { ...current, workspaceRoot: event.target.value } : current
                    )
                  }
                />
                <div className="grid gap-3 sm:grid-cols-3">
                  <label className="block text-sm font-medium text-text">
                    Schedule Kind
                    <select
                      value={editDraft.scheduleKind}
                      onChange={(event) =>
                        setEditDraft((current) =>
                          current
                            ? {
                                ...current,
                                scheduleKind:
                                  event.target.value === "manual"
                                    ? "manual"
                                    : event.target.value === "cron"
                                      ? "cron"
                                      : "interval",
                              }
                            : current
                        )
                      }
                      className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                    >
                      <option value="manual">Manual</option>
                      <option value="interval">Interval</option>
                      <option value="cron">Cron</option>
                    </select>
                  </label>
                  <Input
                    label="Interval Seconds"
                    type="number"
                    min={1}
                    value={editDraft.intervalSeconds}
                    onChange={(event) =>
                      setEditDraft((current) =>
                        current ? { ...current, intervalSeconds: event.target.value } : current
                      )
                    }
                  />
                  <Input
                    label="Cron"
                    value={editDraft.cronExpression}
                    onChange={(event) =>
                      setEditDraft((current) =>
                        current ? { ...current, cronExpression: event.target.value } : current
                      )
                    }
                  />
                </div>
              </div>
              <div className="space-y-3">
                <div className="grid gap-3 sm:grid-cols-2">
                  <label className="block text-sm font-medium text-text">
                    Execution Mode
                    <select
                      value={editDraft.executionMode}
                      onChange={(event) =>
                        setEditDraft((current) =>
                          current
                            ? {
                                ...current,
                                executionMode: event.target.value === "swarm" ? "swarm" : "team",
                              }
                            : current
                        )
                      }
                      className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                    >
                      <option value="team">Team</option>
                      <option value="swarm">Swarm</option>
                    </select>
                  </label>
                  <Input
                    label="Max Parallel Agents"
                    type="number"
                    min={1}
                    max={16}
                    value={editDraft.maxParallelAgents}
                    onChange={(event) =>
                      setEditDraft((current) =>
                        current ? { ...current, maxParallelAgents: event.target.value } : current
                      )
                    }
                  />
                </div>
                <div className="grid gap-3 sm:grid-cols-2">
                  <label className="block text-sm font-medium text-text">
                    Workflow Provider
                    <select
                      value={editDraft.modelProvider}
                      onChange={(event) =>
                        setEditDraft((current) =>
                          current
                            ? {
                                ...current,
                                modelProvider: event.target.value,
                                modelId: providerModelMap.get(event.target.value)?.[0] ?? "",
                              }
                            : current
                        )
                      }
                      className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                    >
                      <option value="">Engine default</option>
                      {providers.map((provider) => (
                        <option key={provider.id} value={provider.id}>
                          {provider.id}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="block text-sm font-medium text-text">
                    Workflow Model
                    <select
                      value={editDraft.modelId}
                      onChange={(event) =>
                        setEditDraft((current) =>
                          current ? { ...current, modelId: event.target.value } : current
                        )
                      }
                      className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                    >
                      <option value="">Engine default</option>
                      {(providerModelMap.get(editDraft.modelProvider) ?? []).map((modelId) => (
                        <option key={modelId} value={modelId}>
                          {modelId}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <div className="grid gap-3 sm:grid-cols-2">
                  <label className="block text-sm font-medium text-text">
                    Planner Provider
                    <select
                      value={editDraft.plannerModelProvider}
                      onChange={(event) =>
                        setEditDraft((current) =>
                          current
                            ? {
                                ...current,
                                plannerModelProvider: event.target.value,
                                plannerModelId: providerModelMap.get(event.target.value)?.[0] ?? "",
                              }
                            : current
                        )
                      }
                      className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                    >
                      <option value="">Fallback to workflow model</option>
                      {providers.map((provider) => (
                        <option key={provider.id} value={provider.id}>
                          {provider.id}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="block text-sm font-medium text-text">
                    Planner Model
                    <select
                      value={editDraft.plannerModelId}
                      onChange={(event) =>
                        setEditDraft((current) =>
                          current ? { ...current, plannerModelId: event.target.value } : current
                        )
                      }
                      className="mt-2 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-text outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                    >
                      <option value="">Fallback to workflow model</option>
                      {(providerModelMap.get(editDraft.plannerModelProvider) ?? []).map(
                        (modelId) => (
                          <option key={modelId} value={modelId}>
                            {modelId}
                          </option>
                        )
                      )}
                    </select>
                  </label>
                </div>
                <div className="rounded-lg border border-border bg-surface-elevated/40 p-3">
                  <div className="text-sm font-medium text-text">Allowed MCP Servers</div>
                  <div className="mt-3 space-y-2">
                    {mcpServers.map((server) => {
                      const checked = editDraft.selectedMcpServers.includes(server.name);
                      return (
                        <label
                          key={server.name}
                          className="flex items-start gap-2 text-sm text-text"
                        >
                          <input
                            type="checkbox"
                            checked={checked}
                            onChange={() =>
                              setEditDraft((current) =>
                                current
                                  ? {
                                      ...current,
                                      selectedMcpServers: checked
                                        ? current.selectedMcpServers.filter(
                                            (row) => row !== server.name
                                          )
                                        : [...current.selectedMcpServers, server.name],
                                    }
                                  : current
                              )
                            }
                          />
                          <span>
                            <span className="block font-medium">{server.name}</span>
                            <span className="block text-xs text-text-muted">
                              {server.connected ? "connected" : "disconnected"} |{" "}
                              {server.enabled ? "enabled" : "disabled"}
                            </span>
                          </span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              </div>
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <Button size="sm" variant="secondary" onClick={() => setEditDraft(null)}>
                Cancel
              </Button>
              <Button
                size="sm"
                variant="primary"
                loading={busyKey === `save:${editDraft.automationId}`}
                onClick={() => void saveEditDraft()}
              >
                Save Changes
              </Button>
            </div>
          </SectionCard>
        ) : null}
      </div>
    </div>
  );
}
