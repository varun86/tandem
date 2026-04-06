import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useQueryClient } from "@tanstack/react-query";
import { renderIcons } from "../app/icons.js";
import { Badge, PanelCard } from "../ui/index.tsx";
import { EmptyState } from "./ui";
import { ProviderModelSelector } from "../components/ProviderModelSelector";
import { ChatInterfacePanel } from "../components/ChatInterfacePanel";
import { renderMarkdownSafe } from "../lib/markdown";
import { PlannerDiagnosticsPanel } from "../features/planner/PlannerDiagnosticsPanel";
import {
  buildPlannerProviderOptions,
  buildDefaultKnowledgeOperatorPreferences,
  buildKnowledgeRolloutGuidance,
  normalizePlannerConversationMessages,
  type PlannerProviderOption,
} from "../features/planner/plannerShared";

type TaskPlanningPanelProps = {
  client: any;
  api: (path: string, init?: RequestInit) => Promise<any>;
  toast: (kind: "ok" | "info" | "warn" | "err", text: string) => void;
  selectedProjectSlug: string;
  selectedProject: any | null;
  githubProjectBoardSnapshot: any | null;
  taskSourceType: string;
  workspaceRootSeed: string;
  connectedMcpServers: string[];
  engineHealthy: boolean;
  providerStatus: {
    defaultProvider: string;
    defaultModel: string;
  };
};

type PlanningDraft = {
  goal: string;
  workspaceRoot: string;
  notes: string;
  plannerProvider: string;
  plannerModel: string;
  plan: any | null;
  conversation: any | null;
  changeSummary: string[];
  plannerError: string;
  plannerDiagnostics: any | null;
  publishedAtMs: number | null;
  updatedAtMs?: number | null;
  publishedTasks: Array<{
    title: string;
    issueNumber?: number | null;
    issueUrl?: string;
    publishedAtMs: number;
  }>;
};

type PlannedTask = {
  id: string;
  title: string;
  objective: string;
  kind: string;
  dependsOn: string[];
  inputRefs: Array<{ fromStepId: string; alias: string }>;
  outputContract: string;
};

type ClarifierOption = {
  id: string;
  label: string;
};

type ClarificationState =
  | { status: "none" }
  | {
      status: "waiting";
      question: string;
      options: ClarifierOption[];
    };

type PlannerSessionRow = {
  id: string;
  title: string;
  updatedAtMs: number;
};

const PLANNER_HANDOFF_KEY = "tandem.intent-planner.codingTaskHandoff.v1";

function safeString(value: unknown) {
  return String(value || "").trim();
}

function isMissingWorkflowPlanError(message: string) {
  const text = safeString(message).toLowerCase();
  return text.includes("workflow_plan_not_found") || text.includes("workflow plan not found");
}

function inferGitHubWorkspaceRoot(selectedProject: any | null) {
  const repoNameFromTaskSource = safeString(selectedProject?.taskSource?.repo);
  if (repoNameFromTaskSource) {
    const repoName = repoNameFromTaskSource.split("/").pop() || repoNameFromTaskSource;
    return `/workspace/repos/${repoName}`;
  }
  const repoUrl = safeString(selectedProject?.repoUrl);
  if (!repoUrl) return "";
  try {
    const parsed = new URL(repoUrl);
    const parts = parsed.pathname.split("/").filter(Boolean);
    const repoName = (parts[parts.length - 1] || "").replace(/\.git$/, "");
    return repoName ? `/workspace/repos/${repoName}` : "";
  } catch {
    const match = repoUrl.match(/\/([^/]+?)(?:\.git)?(?:\?.*)?$/);
    return match?.[1] ? `/workspace/repos/${match[1]}` : "";
  }
}

function plannerSessionTitle(input: {
  goal?: string;
  plan?: any;
  fallbackTime?: number;
  existingTitle?: string;
}) {
  const planTitle = safeString(input.plan?.title);
  if (planTitle) return planTitle;
  const goal = safeString(input.goal);
  if (goal) return goal.length > 48 ? `${goal.slice(0, 47).trimEnd()}…` : goal;
  if (input.existingTitle) return input.existingTitle;
  return `Plan ${new Date(input.fallbackTime || Date.now()).toLocaleTimeString()}`;
}

function normalizePlanSteps(plan: any): PlannedTask[] {
  const steps = Array.isArray(plan?.steps) ? plan.steps : [];
  return steps
    .map((step: any, index: number) => {
      const id = safeString(step?.step_id || step?.stepId || `step-${index + 1}`);
      const objective = safeString(step?.objective);
      const kind = safeString(step?.kind || "task");
      const dependsOn = Array.isArray(step?.depends_on || step?.dependsOn)
        ? (step?.depends_on || step?.dependsOn).map((row: any) => safeString(row)).filter(Boolean)
        : [];
      const inputRefs = Array.isArray(step?.input_refs || step?.inputRefs)
        ? (step?.input_refs || step?.inputRefs).map((row: any) => ({
            fromStepId: safeString(row?.from_step_id || row?.fromStepId),
            alias: safeString(row?.alias),
          }))
        : [];
      const outputContract = safeString(
        step?.output_contract?.kind || step?.outputContract?.kind || ""
      );
      return {
        id,
        title: objective || `${kind} ${index + 1}`,
        objective: objective || `${kind} ${index + 1}`,
        kind,
        dependsOn,
        inputRefs,
        outputContract,
      };
    })
    .filter((step: PlannedTask) => step.title || step.objective);
}

function findIssueNumber(result: any): number | null {
  const candidates: unknown[] = [result?.output];
  const metadataResult = result?.metadata?.result;
  if (metadataResult) {
    candidates.push(metadataResult);
    const content = metadataResult?.content;
    if (Array.isArray(content)) {
      for (const entry of content) {
        if (entry && typeof entry === "object") {
          candidates.push(entry?.text);
          candidates.push(entry);
        }
      }
    }
  }
  for (const candidate of candidates) {
    if (candidate == null) continue;
    if (typeof candidate === "number" && Number.isFinite(candidate)) return Number(candidate);
    if (typeof candidate === "object") {
      const obj = candidate as Record<string, any>;
      const direct =
        obj.issue_number ?? obj.issueNumber ?? obj.number ?? obj.issue?.number ?? obj.data?.number;
      if (typeof direct === "number" && Number.isFinite(direct)) return Number(direct);
      if (typeof direct === "string" && direct.trim()) {
        const parsed = Number.parseInt(direct, 10);
        if (Number.isFinite(parsed)) return parsed;
      }
      continue;
    }
    const text = String(candidate || "").trim();
    if (!text) continue;
    try {
      const parsed = JSON.parse(text);
      if (parsed && typeof parsed === "object") {
        const direct =
          parsed.issue_number ?? parsed.issueNumber ?? parsed.number ?? parsed.data?.number;
        if (typeof direct === "number" && Number.isFinite(direct)) return Number(direct);
        if (typeof direct === "string" && direct.trim()) {
          const parsedNumber = Number.parseInt(direct, 10);
          if (Number.isFinite(parsedNumber)) return parsedNumber;
        }
      }
    } catch {
      const match =
        text.match(/"issue_number"\s*:\s*(\d+)/i) ||
        text.match(/"issueNumber"\s*:\s*(\d+)/i) ||
        text.match(/"number"\s*:\s*(\d+)/i) ||
        text.match(/#(\d+)/);
      if (match) {
        const parsed = Number.parseInt(match[1], 10);
        if (Number.isFinite(parsed)) return parsed;
      }
    }
  }
  return null;
}

function findProjectItemId(result: any): number | null {
  const candidates: unknown[] = [result?.output];
  const metadataResult = result?.metadata?.result;
  if (metadataResult) {
    candidates.push(metadataResult);
    const content = metadataResult?.content;
    if (Array.isArray(content)) {
      for (const entry of content) {
        if (entry && typeof entry === "object") {
          candidates.push(entry?.text);
          candidates.push(entry);
        }
      }
    }
  }
  for (const candidate of candidates) {
    if (candidate == null) continue;
    if (typeof candidate === "number" && Number.isFinite(candidate)) return Number(candidate);
    if (typeof candidate === "object") {
      const obj = candidate as Record<string, any>;
      const direct =
        obj.project_item_id ?? obj.projectItemId ?? obj.item_id ?? obj.itemId ?? obj.id;
      if (typeof direct === "number" && Number.isFinite(direct)) return Number(direct);
      if (typeof direct === "string" && direct.trim()) {
        const parsed = Number.parseInt(direct, 10);
        if (Number.isFinite(parsed)) return parsed;
      }
      continue;
    }
    const text = String(candidate || "").trim();
    if (!text) continue;
    try {
      const parsed = JSON.parse(text);
      if (parsed && typeof parsed === "object") {
        const direct =
          parsed.project_item_id ??
          parsed.projectItemId ??
          parsed.item_id ??
          parsed.itemId ??
          parsed.id;
        if (typeof direct === "number" && Number.isFinite(direct)) return Number(direct);
        if (typeof direct === "string" && direct.trim()) {
          const parsedNumber = Number.parseInt(direct, 10);
          if (Number.isFinite(parsedNumber)) return parsedNumber;
        }
      }
    } catch {
      const match =
        text.match(/"project_item_id"\s*:\s*(\d+)/i) ||
        text.match(/"projectItemId"\s*:\s*(\d+)/i) ||
        text.match(/"item_id"\s*:\s*(\d+)/i) ||
        text.match(/"itemId"\s*:\s*(\d+)/i) ||
        text.match(/"id"\s*:\s*(\d+)/i);
      if (match) {
        const parsed = Number.parseInt(match[1], 10);
        if (Number.isFinite(parsed)) return parsed;
      }
    }
  }
  return null;
}

function buildTaskMarkdown(
  plan: any,
  task: PlannedTask,
  index: number,
  context: {
    goal: string;
    workspaceRoot: string;
    projectSlug: string;
    taskSourceType: string;
    selectedProject: any | null;
    notes: string;
    plannerProvider: string;
    plannerModel: string;
  }
) {
  const planDescription = safeString(plan?.description);
  const refs = task.inputRefs.length
    ? task.inputRefs.map((row) => `- ${row.alias || "ref"} <- ${row.fromStepId}`).join("\n")
    : "- None";
  const dependsOn = task.dependsOn.length
    ? task.dependsOn.map((row) => `- ${row}`).join("\n")
    : "- None";
  const repoUrl = safeString(context.selectedProject?.repoUrl);
  const taskSource = context.selectedProject?.taskSource || {};
  const sourceSummary =
    context.taskSourceType === "github_project"
      ? `GitHub Project ${safeString(taskSource.owner)}/${safeString(taskSource.repo)} #${safeString(taskSource.project)}`
      : context.taskSourceType === "kanban_board" || context.taskSourceType === "local_backlog"
        ? safeString(taskSource.path) || "local task source"
        : "manual";

  return [
    `# ${task.title}`,
    "",
    "## Summary",
    task.objective || "Planned repository task.",
    "",
    "## Context",
    `- Project: ${context.projectSlug || "unbound"}`,
    `- Task source: ${sourceSummary}`,
    `- Workspace root: ${context.workspaceRoot || "engine workspace root"}`,
    repoUrl ? `- Repo URL: ${repoUrl}` : "",
    `- Planner persona: scrum-master`,
    `- Planner provider: ${context.plannerProvider || "workspace default"}`,
    `- Planner model: ${context.plannerModel || "workspace default"}`,
    `- Step kind: ${task.kind || "task"}`,
    "",
    "## Expected Results",
    planDescription
      ? planDescription
      : "Follow the goal and preserve existing behavior unless the plan says otherwise.",
    "",
    "## Implementation Notes",
    `- Depends on:\n${dependsOn}`,
    `- Input refs:\n${refs}`,
    task.outputContract
      ? `- Output contract: ${task.outputContract}`
      : "- Output contract: not specified",
    "",
    "## Verification",
    "- Validate the touched files in the workspace.",
    "- Run the smallest relevant build or test command for the repo area.",
    "- Confirm the user-visible behavior described by the plan.",
    "",
    "## Rework Notes",
    context.notes ? context.notes : "None",
    "",
    "## Planner Snapshot",
    "```json",
    JSON.stringify(
      {
        task_number: index + 1,
        task_title: task.title,
        task_kind: task.kind,
        step_id: task.id,
        workspace_root: context.workspaceRoot,
        project_slug: context.projectSlug,
        planner_provider: context.plannerProvider,
        planner_model: context.plannerModel,
      },
      null,
      2
    ),
    "```",
  ]
    .filter(Boolean)
    .join("\n");
}

function buildExportMarkdown(
  plan: any,
  tasks: PlannedTask[],
  context: {
    goal: string;
    workspaceRoot: string;
    projectSlug: string;
    notes: string;
    plannerProvider: string;
    plannerModel: string;
  }
) {
  const body = [
    `# Task Plan`,
    "",
    `- Project: ${context.projectSlug || "unbound"}`,
    `- Workspace root: ${context.workspaceRoot || "engine workspace root"}`,
    context.goal ? `- Goal: ${context.goal}` : "- Goal: (none)",
    `- Planner provider: ${context.plannerProvider || "workspace default"}`,
    `- Planner model: ${context.plannerModel || "workspace default"}`,
    context.notes ? `- Rework notes: ${context.notes}` : "",
    plan?.title ? `- Plan title: ${plan.title}` : "",
    "",
    "## Tasks",
    ...tasks.flatMap((task, index) => [
      `### ${index + 1}. ${task.title}`,
      "",
      task.objective || "",
      "",
      task.outputContract ? `- Expected result: ${task.outputContract}` : "",
      task.dependsOn.length ? `- Depends on: ${task.dependsOn.join(", ")}` : "",
      "",
    ]),
    "",
    "## Raw Plan",
    "```json",
    JSON.stringify(plan || {}, null, 2),
    "```",
  ]
    .filter(Boolean)
    .join("\n");
  return body;
}

function plannerOperatorPreferences(options: {
  plannerProvider: string;
  plannerModel: string;
  defaultProvider: string;
  defaultModel: string;
  selectedProjectSlug: string;
  taskSourceType: string;
  selectedProject: any | null;
  goal: string;
  notes: string;
}) {
  const baseProvider = safeString(options.defaultProvider);
  const baseModel = safeString(options.defaultModel);
  const selectedPlannerProvider = safeString(options.plannerProvider);
  const selectedPlannerModel = safeString(options.plannerModel);
  const payload: Record<string, any> = {};

  if (baseProvider && baseModel) {
    payload.model_provider = baseProvider;
    payload.model_id = baseModel;
  }

  if (selectedPlannerProvider && selectedPlannerModel) {
    payload.role_models = {
      planner: {
        provider_id: selectedPlannerProvider,
        model_id: selectedPlannerModel,
      },
    };
  }

  Object.assign(payload, buildDefaultKnowledgeOperatorPreferences(options.goal));
  Object.assign(payload, buildKnowledgeRolloutGuidance(options.goal));

  return payload;
}

export function TaskPlanningPanel({
  client,
  api,
  toast,
  selectedProjectSlug,
  selectedProject,
  githubProjectBoardSnapshot,
  taskSourceType,
  workspaceRootSeed,
  connectedMcpServers,
  engineHealthy,
  providerStatus,
}: TaskPlanningPanelProps) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const queryClient = useQueryClient();
  const [plannerSessions, setPlannerSessions] = useState<PlannerSessionRow[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState("");
  const [sessionsOpen, setSessionsOpen] = useState(false);
  const [goal, setGoal] = useState("");
  const [workspaceRoot, setWorkspaceRoot] = useState("");
  const [notes, setNotes] = useState("");
  const [plannerInput, setPlannerInput] = useState("");
  const [plannerProvider, setPlannerProvider] = useState(providerStatus.defaultProvider || "");
  const [plannerModel, setPlannerModel] = useState(providerStatus.defaultModel || "");
  const [planPreview, setPlanPreview] = useState<any>(null);
  const [planningConversation, setPlanningConversation] = useState<any>(null);
  const [planningChangeSummary, setPlanningChangeSummary] = useState<string[]>([]);
  const [plannerError, setPlannerError] = useState("");
  const [plannerDiagnostics, setPlannerDiagnostics] = useState<any>(null);
  const [publishing, setPublishing] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [loadingDraft, setLoadingDraft] = useState(false);
  const [planningState, setPlanningState] = useState<
    "idle" | "generating" | "clarifying" | "revising"
  >("idle");
  const [clarification, setClarification] = useState<ClarificationState>({ status: "none" });
  const [saveStatus, setSaveStatus] = useState("");
  const [publishStatus, setPublishStatus] = useState("");
  const [lastSavedAtMs, setLastSavedAtMs] = useState<number | null>(null);
  const [publishedTasks, setPublishedTasks] = useState<PlanningDraft["publishedTasks"]>([]);
  const controlPanelConfigQuery = useQuery({
    queryKey: ["coding-workflows", "control-panel-config", selectedProjectSlug],
    queryFn: () => api("/api/control-panel/config"),
  });
  const providersCatalogQuery = useQuery({
    queryKey: ["coding-workflows", "providers", "catalog"],
    queryFn: () => client.providers.catalog().catch(() => ({ all: [] })),
    refetchInterval: 30000,
  });
  const providersConfigQuery = useQuery({
    queryKey: ["coding-workflows", "providers", "config"],
    queryFn: () => client.providers.config().catch(() => ({})),
    refetchInterval: 30000,
  });

  const plannedTasks = useMemo(() => normalizePlanSteps(planPreview), [planPreview]);
  const isGitHubProject = String(taskSourceType || "").trim() === "github_project";
  const providerOptions = useMemo<PlannerProviderOption[]>(() => {
    return buildPlannerProviderOptions({
      providerCatalog: providersCatalogQuery.data,
      providerConfig: providersConfigQuery.data,
      defaultProvider: providerStatus.defaultProvider,
      defaultModel: providerStatus.defaultModel,
    });
  }, [
    providerStatus.defaultModel,
    providerStatus.defaultProvider,
    providersCatalogQuery.data,
    providersConfigQuery.data,
  ]);
  const controlPanelRepoPath = safeString(
    controlPanelConfigQuery.data?.config?.repository?.path ||
      controlPanelConfigQuery.data?.config?.repository?.worktree_root ||
      ""
  );
  const resolvedWorkspaceRoot = useMemo(() => {
    if (isGitHubProject) {
      return (
        controlPanelRepoPath || inferGitHubWorkspaceRoot(selectedProject) || workspaceRootSeed || ""
      );
    }
    if (
      String(taskSourceType || "").trim() === "kanban_board" ||
      String(taskSourceType || "").trim() === "local_backlog"
    ) {
      return safeString(selectedProject?.taskSource?.path || workspaceRootSeed || "");
    }
    return workspaceRootSeed || "";
  }, [controlPanelRepoPath, isGitHubProject, selectedProject, taskSourceType, workspaceRootSeed]);
  const canPublishToGitHub =
    isGitHubProject &&
    !!safeString(selectedProject?.taskSource?.owner) &&
    !!safeString(selectedProject?.taskSource?.repo) &&
    Number.isFinite(Number(selectedProject?.taskSource?.project || 0));
  const hasBasePlannerModel = !!(
    safeString(providerStatus.defaultProvider) && safeString(providerStatus.defaultModel)
  );
  const hasExplicitPlannerOverride = !!(safeString(plannerProvider) && safeString(plannerModel));
  const plannerSelectionMatchesWorkspaceDefault =
    safeString(plannerProvider) === safeString(providerStatus.defaultProvider) &&
    safeString(plannerModel) === safeString(providerStatus.defaultModel);
  const plannerCanUseLlm = hasBasePlannerModel || hasExplicitPlannerOverride;
  const isGeneratingPlan = planningState === "generating";
  const isClarifying = planningState === "clarifying";
  const isPlanning = planningState !== "idle";
  const plannerFallbackReason = safeString(
    plannerDiagnostics?.fallback_reason || plannerDiagnostics?.fallbackReason
  );
  const clarificationNeeded = plannerFallbackReason === "clarification_needed";
  const latestAssistantMessage = safeString(
    Array.isArray(planningConversation?.messages)
      ? [...planningConversation.messages]
          .reverse()
          .find((message: any) => safeString(message?.role).toLowerCase() === "assistant")?.text
      : ""
  );
  const plannerTimedOut =
    /timed out before completion/i.test(safeString(plannerError)) ||
    /timed out before completion/i.test(latestAssistantMessage);
  const plannerChatMessages = useMemo(
    () => normalizePlannerConversationMessages(planningConversation, false),
    [planningConversation]
  );
  const plannerStatusTitle = isPlanning
    ? isGeneratingPlan
      ? "Generating task plan…"
      : isClarifying
        ? "Waiting for planner…"
        : "Revising task plan…"
    : "";
  const plannerStatusDetail = isClarifying
    ? "The planner is thinking. You'll be able to answer its question shortly."
    : "The request is running against the engine now. Repo-aware planning can take a bit longer while the model inspects files and drafts the task breakdown.";
  const plannerInputPlaceholder = planPreview
    ? "Send a follow-up message or revision request to the planner. (Enter to send, Shift+Enter newline)"
    : "Describe the outcome you want and send to planner. (Enter to send, Shift+Enter newline)";
  const planIsFallbackOnly =
    plannedTasks.length === 1 &&
    safeString(plannedTasks[0]?.id) === "execute_goal" &&
    (!!plannerFallbackReason || /fallback draft/i.test(safeString(planPreview?.description)));
  const displayTasks = planIsFallbackOnly ? [] : plannedTasks;

  const hydrateDraft = (draft: PlanningDraft | null) => {
    if (draft) {
      setGoal(draft.goal);
      setPlannerInput(draft.notes || draft.goal || "");
      setWorkspaceRoot(draft.workspaceRoot || resolvedWorkspaceRoot || workspaceRootSeed || "");
      setNotes(draft.notes);
      setPlannerProvider(draft.plannerProvider || providerStatus.defaultProvider || "");
      setPlannerModel(draft.plannerModel || providerStatus.defaultModel || "");
      setPlanPreview(draft.plan);
      setPlanningConversation(draft.conversation);
      setPlanningChangeSummary(draft.changeSummary || []);
      setPlannerError(draft.plannerError || "");
      setPlannerDiagnostics(draft.plannerDiagnostics || null);
      setPublishStatus(
        draft.publishedAtMs
          ? `Last published ${new Date(draft.publishedAtMs).toLocaleString()}`
          : ""
      );
      setLastSavedAtMs(draft.publishedAtMs || null);
      setPublishedTasks(draft.publishedTasks || []);
      setClarification({ status: "none" });
      return;
    }
    setGoal("");
    setPlannerInput("");
    setWorkspaceRoot(resolvedWorkspaceRoot || workspaceRootSeed || "");
    setNotes("");
    setPlannerProvider(providerStatus.defaultProvider || "");
    setPlannerModel(providerStatus.defaultModel || "");
    setPlanPreview(null);
    setPlanningConversation(null);
    setPlanningChangeSummary([]);
    setPlannerError("");
    setPlannerDiagnostics(null);
    setClarification({ status: "none" });
    setPublishStatus("");
    setLastSavedAtMs(null);
    setPublishedTasks([]);
  };

  const planningDraftFromSession = useCallback(
    (session: any | null): PlanningDraft | null => {
      if (!session) return null;
      const draft = session?.draft || null;
      const plan = draft?.current_plan || draft?.currentPlan || null;
      return {
        goal: safeString(session?.goal || plan?.title || ""),
        workspaceRoot: safeString(session?.workspace_root || session?.workspaceRoot || ""),
        notes: safeString(session?.notes || ""),
        plannerProvider: safeString(session?.planner_provider || session?.plannerProvider || ""),
        plannerModel: safeString(session?.planner_model || session?.plannerModel || ""),
        plan,
        conversation: draft?.conversation || null,
        changeSummary: [],
        plannerError: "",
        plannerDiagnostics: draft?.planner_diagnostics || draft?.plannerDiagnostics || null,
        publishedAtMs: Number(session?.published_at_ms || session?.publishedAtMs || 0) || null,
        updatedAtMs: Number(session?.updated_at_ms || session?.updatedAtMs || 0) || Date.now(),
        publishedTasks: Array.isArray(session?.published_tasks || session?.publishedTasks)
          ? session?.published_tasks || session?.publishedTasks
          : [],
      };
    },
    [
      resolvedWorkspaceRoot,
      providerStatus.defaultModel,
      providerStatus.defaultProvider,
      workspaceRootSeed,
    ]
  );

  const refreshPlannerSessions = useCallback(async () => {
    if (!client?.workflowPlannerSessions?.list) return [];
    const response = await client.workflowPlannerSessions.list({
      project_slug: selectedProjectSlug,
    });
    const rows = Array.isArray(response?.sessions) ? response.sessions : [];
    setPlannerSessions(
      rows
        .map((row: any) => ({
          id: safeString(row?.session_id || row?.sessionId),
          title: safeString(row?.title),
          updatedAtMs: Number(row?.updated_at_ms || row?.updatedAtMs || 0) || 0,
        }))
        .filter((row: PlannerSessionRow) => row.id)
        .sort(
          (left: PlannerSessionRow, right: PlannerSessionRow) =>
            right.updatedAtMs - left.updatedAtMs
        )
    );
    return rows;
  }, [client?.workflowPlannerSessions?.list, selectedProjectSlug]);

  const loadPlannerSession = useCallback(
    async (sessionId: string) => {
      if (!client?.workflowPlannerSessions?.get || !sessionId) return;
      const response = await client.workflowPlannerSessions.get(sessionId);
      const session = response?.session || null;
      if (!session) return;
      hydrateDraft(planningDraftFromSession(session));
      setPublishStatus(
        session.published_at_ms
          ? `Last published ${new Date(session.published_at_ms).toLocaleString()}`
          : ""
      );
      setLastSavedAtMs(Number(session.updated_at_ms || 0) || null);
      setPlannerSessions((current) => {
        const next = current.filter((row) => row.id !== session.session_id);
        next.unshift({
          id: session.session_id,
          title: safeString(session.title) || "Untitled plan",
          updatedAtMs: Number(session.updated_at_ms || 0) || Date.now(),
        });
        return next.sort((left, right) => right.updatedAtMs - left.updatedAtMs);
      });
    },
    [client?.workflowPlannerSessions?.get, planningDraftFromSession]
  );

  const patchActivePlannerSession = useCallback(
    async (patch: Record<string, unknown>) => {
      if (!selectedSessionId || !client?.workflowPlannerSessions?.patch) return;
      try {
        const response = await client.workflowPlannerSessions.patch(
          selectedSessionId,
          patch as any
        );
        const session = response?.session;
        if (session?.session_id) {
          setPlannerSessions((current) => {
            const next = current.filter((row) => row.id !== session.session_id);
            next.unshift({
              id: session.session_id,
              title: safeString(session.title) || "Untitled plan",
              updatedAtMs: Number(session.updated_at_ms || 0) || Date.now(),
            });
            return next.sort((left, right) => right.updatedAtMs - left.updatedAtMs);
          });
        }
      } catch {
        // ignore transient patch failures while the user is typing
      }
    },
    [client?.workflowPlannerSessions?.patch, selectedSessionId]
  );

  useEffect(() => {
    let cancelled = false;
    setLoadingDraft(true);
    (async () => {
      try {
        let rows = await refreshPlannerSessions();
        if (!rows.length && client?.workflowPlannerSessions?.create) {
          const created = await client.workflowPlannerSessions.create({
            project_slug: selectedProjectSlug,
            title: plannerSessionTitle({ goal: "", fallbackTime: Date.now() }),
            workspace_root: resolvedWorkspaceRoot || workspaceRootSeed || "",
            goal: "",
            notes: "",
            planner_provider: providerStatus.defaultProvider || "",
            planner_model: providerStatus.defaultModel || "",
            plan_source: "coding_task_planning",
            allowed_mcp_servers: connectedMcpServers,
            operator_preferences: plannerOperatorPreferences({
              plannerProvider,
              plannerModel,
              defaultProvider: providerStatus.defaultProvider,
              defaultModel: providerStatus.defaultModel,
              selectedProjectSlug,
              taskSourceType,
              selectedProject,
              goal: "",
              notes: "",
            }),
          });
          const createdSession = created?.session;
          if (createdSession?.session_id) {
            rows = [createdSession, ...rows];
            setPlannerSessions(
              rows
                .map((row: any) => ({
                  id: safeString(row?.session_id || row?.sessionId),
                  title: safeString(row?.title),
                  updatedAtMs: Number(row?.updated_at_ms || row?.updatedAtMs || 0) || 0,
                }))
                .filter((row: PlannerSessionRow) => row.id)
                .sort(
                  (left: PlannerSessionRow, right: PlannerSessionRow) =>
                    right.updatedAtMs - left.updatedAtMs
                )
            );
          }
        }
        const nextSessionId = safeString(rows[0]?.session_id || rows[0]?.sessionId || "");
        if (nextSessionId) {
          setSelectedSessionId(nextSessionId);
        }
      } catch {
        // ignore load failures; the page will remain usable for a new session
      } finally {
        if (!cancelled) setLoadingDraft(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    client?.workflowPlannerSessions,
    connectedMcpServers,
    plannerModel,
    plannerProvider,
    providerStatus.defaultModel,
    providerStatus.defaultProvider,
    refreshPlannerSessions,
    resolvedWorkspaceRoot,
    selectedProject,
    selectedProjectSlug,
    taskSourceType,
    workspaceRootSeed,
  ]);

  useEffect(() => {
    if (!selectedSessionId) return;
    let cancelled = false;
    setLoadingDraft(true);
    (async () => {
      try {
        await loadPlannerSession(selectedSessionId);
      } catch {
        hydrateDraft(null);
      } finally {
        if (!cancelled) setLoadingDraft(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    selectedSessionId,
    loadPlannerSession,
    resolvedWorkspaceRoot,
    workspaceRootSeed,
    providerStatus.defaultProvider,
    providerStatus.defaultModel,
  ]);

  useEffect(() => {
    if (typeof sessionStorage === "undefined") return;
    try {
      const raw = sessionStorage.getItem(PLANNER_HANDOFF_KEY);
      if (!raw) return;
      sessionStorage.removeItem(PLANNER_HANDOFF_KEY);
      const handoff = JSON.parse(raw);
      if (!client?.workflowPlannerSessions?.create) return;
      const loadHandoff = async () => {
        const response = await client.workflowPlannerSessions.create({
          project_slug: selectedProjectSlug,
          title: plannerSessionTitle({
            goal: handoff?.goal,
            plan: handoff?.plan,
            fallbackTime: Date.now(),
          }),
          workspace_root: safeString(
            handoff?.workspaceRoot || handoff?.workspace_root || workspaceRootSeed
          ),
          goal: safeString(handoff?.goal),
          notes: safeString(handoff?.notes),
          planner_provider: safeString(handoff?.plannerProvider || handoff?.planner_provider),
          planner_model: safeString(handoff?.plannerModel || handoff?.planner_model),
          plan_source: "coding_task_planning",
          plan: handoff?.plan ?? null,
          conversation: handoff?.conversation ?? null,
          planner_diagnostics: handoff?.plannerDiagnostics ?? null,
          plan_revision: handoff?.plan?.plan_revision ?? 1,
          last_success_materialization: handoff?.lastSuccessMaterialization ?? null,
          allowed_mcp_servers: Array.isArray(handoff?.allowedMcpServers)
            ? handoff.allowedMcpServers
            : connectedMcpServers,
          operator_preferences: handoff?.operatorPreferences ?? null,
        });
        const session = response?.session;
        if (session?.session_id) {
          setPlannerSessions((current) =>
            [
              {
                id: session.session_id,
                title: safeString(session.title) || "Untitled plan",
                updatedAtMs: Number(session.updated_at_ms || 0) || Date.now(),
              },
              ...current.filter((row) => row.id !== session.session_id),
            ].sort((left, right) => right.updatedAtMs - left.updatedAtMs)
          );
          setSelectedSessionId(session.session_id);
          hydrateDraft(planningDraftFromSession(session));
          setPublishStatus("Planner handoff loaded from Planner page.");
        }
      };
      void loadHandoff();
    } catch {
      // ignore
    }
  }, [
    client?.workflowPlannerSessions?.create,
    connectedMcpServers,
    planningDraftFromSession,
    selectedProjectSlug,
    workspaceRootSeed,
  ]);

  useEffect(() => {
    if (!workspaceRoot && resolvedWorkspaceRoot) {
      setWorkspaceRoot(resolvedWorkspaceRoot);
    }
  }, [resolvedWorkspaceRoot, workspaceRoot]);

  useEffect(() => {
    setPlannerError("");
    setPlannerDiagnostics(null);
    setSaveStatus("");
  }, [plannerProvider, plannerModel]);

  useEffect(() => {
    if (loadingDraft || !selectedSessionId) return;
    const plan = planPreview || null;
    const currentPlanId = safeString(plan?.plan_id || "");
    const draft =
      plan && plan.plan_id
        ? {
            initial_plan: plan,
            current_plan: plan,
            plan_revision: Number(plan.plan_revision || 1) || 1,
            conversation: planningConversation || {
              conversation_id: `wfchat-${selectedSessionId}`,
              plan_id: plan.plan_id,
              created_at_ms: Date.now(),
              updated_at_ms: Date.now(),
              messages: [],
            },
            planner_diagnostics: plannerDiagnostics,
            last_success_materialization: null,
          }
        : undefined;
    void patchActivePlannerSession({
      title: plannerSessionTitle({
        goal,
        plan,
        fallbackTime: lastSavedAtMs || Date.now(),
      }),
      workspace_root: workspaceRoot,
      goal,
      notes,
      planner_provider: plannerProvider,
      planner_model: plannerModel,
      plan_source: "coding_task_planning",
      allowed_mcp_servers: connectedMcpServers,
      operator_preferences: plannerOperatorPreferences({
        plannerProvider,
        plannerModel,
        defaultProvider: providerStatus.defaultProvider,
        defaultModel: providerStatus.defaultModel,
        selectedProjectSlug,
        taskSourceType,
        selectedProject,
        goal,
        notes,
      }),
      current_plan_id: currentPlanId || undefined,
      draft,
      published_at_ms: lastSavedAtMs || undefined,
      published_tasks: publishedTasks.length ? publishedTasks : undefined,
    });
  }, [
    connectedMcpServers,
    goal,
    lastSavedAtMs,
    loadingDraft,
    notes,
    planPreview,
    plannerDiagnostics,
    plannerError,
    planningChangeSummary,
    planningConversation,
    publishedTasks,
    selectedProjectSlug,
    selectedSessionId,
    taskSourceType,
    plannerModel,
    plannerProvider,
    patchActivePlannerSession,
    workspaceRoot,
    providerStatus.defaultProvider,
    providerStatus.defaultModel,
    selectedProject,
  ]);

  const createNewPlannerSession = async () => {
    if (!client?.workflowPlannerSessions?.create) return;
    try {
      const response = await client.workflowPlannerSessions.create({
        project_slug: selectedProjectSlug,
        title: plannerSessionTitle({
          goal: "",
          fallbackTime: Date.now(),
        }),
        workspace_root: resolvedWorkspaceRoot || workspaceRootSeed || workspaceRoot || "",
        goal: "",
        notes: "",
        planner_provider: providerStatus.defaultProvider || "",
        planner_model: providerStatus.defaultModel || "",
        plan_source: "coding_task_planning",
        allowed_mcp_servers: connectedMcpServers,
        operator_preferences: plannerOperatorPreferences({
          plannerProvider: providerStatus.defaultProvider || "",
          plannerModel: providerStatus.defaultModel || "",
          defaultProvider: providerStatus.defaultProvider,
          defaultModel: providerStatus.defaultModel,
          selectedProjectSlug,
          taskSourceType,
          selectedProject,
          goal: "",
          notes: "",
        }),
      });
      const session = response?.session;
      if (session?.session_id) {
        setPlannerSessions((current) =>
          [
            {
              id: session.session_id,
              title: safeString(session.title) || "Untitled plan",
              updatedAtMs: Number(session.updated_at_ms || 0) || Date.now(),
            },
            ...current.filter((row) => row.id !== session.session_id),
          ].sort((left, right) => right.updatedAtMs - left.updatedAtMs)
        );
        setSelectedSessionId(session.session_id);
        setSessionsOpen(false);
        hydrateDraft(planningDraftFromSession(session));
        toast("ok", "Started a new planner session.");
      }
    } catch (error) {
      toast("err", error instanceof Error ? error.message : String(error));
    }
  };

  const renamePlannerSession = async (sessionId: string) => {
    const session = plannerSessions.find((row) => row.id === sessionId);
    if (!session || !client?.workflowPlannerSessions?.patch) return;
    const nextTitle = window.prompt("Rename planner session", session.title || "Untitled plan");
    if (nextTitle === null) return;
    const title = safeString(nextTitle);
    if (!title) return;
    try {
      const response = await client.workflowPlannerSessions.patch(sessionId, { title });
      const updated = response?.session;
      if (updated?.session_id) {
        setPlannerSessions((current) =>
          current
            .filter((row) => row.id !== updated.session_id)
            .concat({
              id: updated.session_id,
              title: safeString(updated.title) || title,
              updatedAtMs: Number(updated.updated_at_ms || 0) || Date.now(),
            })
            .sort((left, right) => right.updatedAtMs - left.updatedAtMs)
        );
        if (selectedSessionId === sessionId) {
          setSelectedSessionId(updated.session_id);
        }
      }
      toast("ok", "Planner session renamed.");
    } catch (error) {
      toast("err", error instanceof Error ? error.message : String(error));
    }
  };

  const duplicatePlannerSession = async (sessionId: string) => {
    const session = plannerSessions.find((row) => row.id === sessionId);
    if (!session || !client?.workflowPlannerSessions?.duplicate) return;
    try {
      const response = await client.workflowPlannerSessions.duplicate(sessionId, {
        title: `Copy of ${session.title || "Untitled plan"}`,
      });
      const copied = response?.session;
      if (copied?.session_id) {
        setPlannerSessions((current) =>
          current
            .filter((row) => row.id !== copied.session_id)
            .concat({
              id: copied.session_id,
              title: safeString(copied.title) || `Copy of ${session.title || "Untitled plan"}`,
              updatedAtMs: Number(copied.updated_at_ms || 0) || Date.now(),
            })
            .sort((left, right) => right.updatedAtMs - left.updatedAtMs)
        );
        setSelectedSessionId(copied.session_id);
        setSessionsOpen(false);
      }
      toast("ok", "Planner session duplicated.");
    } catch (error) {
      toast("err", error instanceof Error ? error.message : String(error));
    }
  };

  const deletePlannerSession = async (sessionId: string) => {
    const session = plannerSessions.find((row) => row.id === sessionId);
    if (!session) return;
    if (
      typeof window !== "undefined" &&
      !window.confirm(`Delete planner session "${session.title || "Untitled plan"}"?`)
    ) {
      return;
    }
    try {
      await client.workflowPlannerSessions.delete(sessionId);
      const remaining = plannerSessions.filter((row) => row.id !== sessionId);
      setPlannerSessions(remaining);
      if (selectedSessionId === sessionId) {
        const nextSessionId = remaining[0]?.id || "";
        if (nextSessionId) {
          setSelectedSessionId(nextSessionId);
        } else {
          void createNewPlannerSession();
        }
      }
      toast("ok", "Planner session deleted.");
    } catch (error) {
      toast("err", error instanceof Error ? error.message : String(error));
    }
  };

  const previewMutation = async (messageOverride?: string) => {
    const trimmedGoal = safeString(messageOverride ?? plannerInput ?? goal);
    const trimmedWorkspaceRoot = safeString(workspaceRoot);
    if (!trimmedGoal) {
      toast("warn", "Describe the goal you want to turn into tasks.");
      return;
    }
    if (!trimmedWorkspaceRoot) {
      toast("warn", "Workspace root is required so the planner can inspect the repo.");
      return;
    }
    if (!trimmedWorkspaceRoot.startsWith("/")) {
      toast("warn", "Workspace root must be an absolute path.");
      return;
    }
    if (!client?.workflowPlannerSessions?.start || !selectedSessionId) {
      toast("err", "This control panel build is missing workflow planner support.");
      return;
    }
    setSaveStatus("");
    setPlannerError("");
    setPlannerDiagnostics(null);
    setGoal(trimmedGoal);
    let nextState: "idle" | "clarifying" = "idle";
    setPlanningState("generating");
    try {
      const response = await client.workflowPlannerSessions.start(selectedSessionId, {
        prompt: [
          `Create a Tandem workflow plan for the repo at ${trimmedWorkspaceRoot} to implement this goal: ${trimmedGoal}`,
          "Use small ordered implementation steps.",
          "Prefer step objectives that mention likely files, modules, or repo areas to inspect or change.",
          "If the target or desired outcome is ambiguous, return a clarification question instead of guessing.",
          safeString(selectedProject?.repoUrl)
            ? `Repo URL: ${safeString(selectedProject?.repoUrl)}`
            : "",
          selectedProjectSlug ? `Selected project: ${selectedProjectSlug}` : "",
          notes ? `User feedback / revision notes:\n${notes}` : "",
        ]
          .filter(Boolean)
          .join("\n"),
        plan_source: "coding_task_planning",
        workspace_root: trimmedWorkspaceRoot,
        allowed_mcp_servers: connectedMcpServers,
        operator_preferences: plannerOperatorPreferences({
          plannerProvider,
          plannerModel,
          defaultProvider: providerStatus.defaultProvider,
          defaultModel: providerStatus.defaultModel,
          selectedProjectSlug,
          taskSourceType,
          selectedProject,
          goal: trimmedGoal,
          notes,
        }),
      });
      setPlanPreview(response?.plan || null);
      setPlanningConversation(response?.conversation || null);
      setPlanningChangeSummary([]);
      setPlannerDiagnostics(response?.planner_diagnostics || response?.plannerDiagnostics || null);
      const clarQuestion =
        typeof response?.clarifier?.question === "string"
          ? String(response.clarifier.question)
          : "";
      const clarOptions: ClarifierOption[] = Array.isArray(response?.clarifier?.options)
        ? response.clarifier.options
            .map((o: any) => ({ id: String(o?.id || ""), label: String(o?.label || "") }))
            .filter((o: ClarifierOption) => o.id && o.label)
        : [];
      setPlannerError(clarQuestion);
      if (clarQuestion && clarOptions.length > 0) {
        setClarification({
          status: "waiting",
          question: clarQuestion,
          options: clarOptions,
        });
        nextState = "clarifying";
      } else {
        setClarification({ status: "none" });
      }
      setLastSavedAtMs(Date.now());
      setPublishedTasks([]);
      setSaveStatus("Plan synced to session.");
      setPlannerInput("");
      toast(
        "ok",
        typeof response?.clarifier?.question === "string"
          ? "Planner needs clarification before it can draft richer tasks."
          : "Planner generated a task draft."
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (isMissingWorkflowPlanError(message)) {
        setPlanPreview(null);
        setPlanningConversation(null);
        setPlanningChangeSummary([]);
        setPlannerError("");
        setPlannerDiagnostics(null);
        setClarification({ status: "none" });
        toast("info", "The previous planner session expired. Starting a fresh planning chat.");
        await previewMutation(trimmedGoal);
        return;
      }
      setPlannerError(message);
      setClarification({ status: "none" });
      setSaveStatus(
        message.includes("Request timed out after")
          ? "The panel timed out before the planner finished. The engine may still be working, so wait a moment before retrying."
          : ""
      );
      toast("err", message);
    } finally {
      setPlanningState(nextState);
    }
  };

  const reviseMutation = async (messageOverride?: string) => {
    if (!client?.workflowPlannerSessions?.message || !selectedSessionId) {
      toast("warn", "Generate a plan first before revising it.");
      return;
    }
    const trimmedNotes = safeString(messageOverride ?? plannerInput ?? notes);
    if (!trimmedNotes) {
      toast("warn", "Add a message for the planner first.");
      return;
    }
    setSaveStatus("");
    setPlannerError("");
    setPlannerDiagnostics(null);
    setNotes(trimmedNotes);
    let nextState: "idle" | "clarifying" = "idle";
    setPlanningState("revising");
    try {
      const response = await client.workflowPlannerSessions.message(selectedSessionId, {
        message: clarificationNeeded
          ? trimmedNotes
          : [
              `Revise the workflow plan for repo ${safeString(workspaceRoot)}.`,
              `Goal: ${safeString(goal)}`,
              selectedProjectSlug ? `Selected project: ${selectedProjectSlug}` : "",
              "Keep the workflow valid and machine-parseable.",
              "Prefer concrete repo-aware implementation steps.",
              "If the request is ambiguous, return a clarification question.",
              "",
              "Revision notes:",
              trimmedNotes,
            ]
              .filter(Boolean)
              .join("\n"),
      });
      setPlanPreview(response?.plan || null);
      setPlanningConversation(response?.conversation || null);
      setPlanningChangeSummary(
        Array.isArray(response?.change_summary)
          ? response.change_summary.map((row: any) => safeString(row)).filter(Boolean)
          : []
      );
      setPlannerDiagnostics(response?.planner_diagnostics || response?.plannerDiagnostics || null);
      const clarQuestion =
        typeof response?.clarifier?.question === "string"
          ? String(response.clarifier.question)
          : "";
      const clarOptions: ClarifierOption[] = Array.isArray(response?.clarifier?.options)
        ? response.clarifier.options
            .map((o: any) => ({ id: String(o?.id || ""), label: String(o?.label || "") }))
            .filter((o: ClarifierOption) => o.id && o.label)
        : [];
      setPlannerError(clarQuestion);
      if (clarQuestion && clarOptions.length > 0) {
        setClarification({
          status: "waiting",
          question: clarQuestion,
          options: clarOptions,
        });
        nextState = "clarifying";
      } else {
        setClarification({ status: "none" });
      }
      setLastSavedAtMs(Date.now());
      setPublishedTasks([]);
      setSaveStatus("Revision synced to session.");
      setPlannerInput("");
      toast("ok", "Plan revised.");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (isMissingWorkflowPlanError(message)) {
        setPlanPreview(null);
        setPlanningConversation(null);
        setPlanningChangeSummary([]);
        setPlannerError("");
        setPlannerDiagnostics(null);
        setClarification({ status: "none" });
        toast("info", "The previous planner session expired. Starting a fresh planning chat.");
        await previewMutation(trimmedNotes);
        return;
      }
      setPlannerError(message);
      setClarification({ status: "none" });
      setSaveStatus(
        message.includes("Request timed out after")
          ? "The panel timed out before the planner revision finished. The engine may still be working, so wait a moment before retrying."
          : ""
      );
      toast("err", message);
    } finally {
      setPlanningState(nextState);
    }
  };

  const resetMutation = async () => {
    if (!client?.workflowPlannerSessions?.reset || !selectedSessionId) {
      setGoal("");
      setPlannerInput("");
      setNotes("");
      setPlanPreview(null);
      setPlanningConversation(null);
      setPlanningChangeSummary([]);
      setPlannerError("");
      setPlannerDiagnostics(null);
      setClarification({ status: "none" });
      setPublishStatus("");
      setSaveStatus("Draft cleared.");
      return;
    }
    setResetting(true);
    try {
      const response = await client.workflowPlannerSessions.reset(selectedSessionId);
      setPlanPreview(response?.plan || null);
      setPlanningConversation(response?.conversation || null);
      setPlannerInput("");
      setPlanningChangeSummary([]);
      setPlannerError("");
      setPlannerDiagnostics(response?.planner_diagnostics || response?.plannerDiagnostics || null);
      setPublishedTasks([]);
      setPublishStatus("");
      setSaveStatus("Plan reset.");
      setLastSavedAtMs(Date.now());
      toast("ok", "Plan reset.");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (isMissingWorkflowPlanError(message)) {
        setGoal("");
        setPlannerInput("");
        setNotes("");
        setPlanPreview(null);
        setPlanningConversation(null);
        setPlanningChangeSummary([]);
        setPlannerError("");
        setPlannerDiagnostics(null);
        setClarification({ status: "none" });
        setPublishStatus("");
        setSaveStatus("The expired planner session was cleared locally.");
        toast("info", "The previous planner session had expired, so the local draft was reset.");
        return;
      }
      setPlannerError(message);
      toast("err", message);
    } finally {
      setResetting(false);
    }
  };

  const publishTasks = async () => {
    const tasks = plannedTasks.length
      ? plannedTasks
      : goal.trim()
        ? [
            {
              id: "task-1",
              title: goal.trim(),
              objective: goal.trim(),
              kind: "task",
              dependsOn: [],
              inputRefs: [],
              outputContract: "",
            },
          ]
        : [];
    if (!tasks.length) {
      toast("warn", "Generate a plan before publishing tasks.");
      return;
    }
    setPublishing(true);
    setPublishStatus("");
    try {
      if (isGitHubProject && canPublishToGitHub) {
        const source = selectedProject?.taskSource || {};
        const owner = safeString(source.owner);
        const repo = safeString(source.repo);
        const projectNumber = Number(source.project);
        let projectBoardSnapshot = githubProjectBoardSnapshot;
        const readyStatusFieldId = Number(
          projectBoardSnapshot?.status_field_id || projectBoardSnapshot?.statusFieldId || 0
        );
        const readyOptionId = safeString(
          projectBoardSnapshot?.status_option_map?.ready ||
            projectBoardSnapshot?.statusOptionMap?.ready ||
            projectBoardSnapshot?.status_option_map?.Ready ||
            projectBoardSnapshot?.statusOptionMap?.Ready ||
            ""
        );
        if ((!readyStatusFieldId || !readyOptionId) && safeString(selectedProjectSlug)) {
          try {
            projectBoardSnapshot = await api(
              `/api/aca/projects/${encodeURIComponent(selectedProjectSlug)}/board?refresh=true`
            );
          } catch {
            // Fall back to the cached snapshot if a live refresh fails.
          }
        }
        const publishedTasks: PlanningDraft["publishedTasks"] = [];
        for (let index = 0; index < tasks.length; index += 1) {
          const task = tasks[index];
          const body = buildTaskMarkdown(planPreview, task, index, {
            goal,
            workspaceRoot,
            projectSlug: selectedProjectSlug,
            taskSourceType,
            selectedProject,
            notes,
            plannerProvider,
            plannerModel,
          });
          const issueResult = await client.executeTool("mcp.github.issue_write", {
            method: "create",
            owner,
            repo,
            title: task.title,
            body,
            labels: ["tandem", "planned-task", "ready"],
          });
          const issueNumber = findIssueNumber(issueResult);
          if (!issueNumber) {
            throw new Error(
              `Created GitHub issue for "${task.title}" but could not determine the issue number from the engine response.`
            );
          }
          const addProjectItemResult = await client.executeTool("mcp.github.projects_write", {
            method: "add_project_item",
            owner,
            project_number: projectNumber,
            item_type: "issue",
            item_owner: owner,
            item_repo: repo,
            issue_number: issueNumber,
          });
          const projectItemId = findProjectItemId(addProjectItemResult);
          if (projectItemId && readyStatusFieldId && readyOptionId) {
            await client.executeTool("mcp.github.projects_write", {
              method: "update_project_item",
              owner,
              project_number: projectNumber,
              item_id: projectItemId,
              updated_field: {
                id: readyStatusFieldId,
                value: readyOptionId,
              },
            });
          }
          publishedTasks.push({
            title: task.title,
            issueNumber,
            issueUrl: `https://github.com/${owner}/${repo}/issues/${issueNumber}`,
            publishedAtMs: Date.now(),
          });
        }
        setPublishedTasks(publishedTasks);
        setPublishStatus(`Published ${publishedTasks.length} GitHub tasks into Ready.`);
        setLastSavedAtMs(Date.now());
        await Promise.all([
          queryClient.invalidateQueries({
            queryKey: ["coding-workflows", "aca-project-board", selectedProjectSlug],
          }),
          queryClient.invalidateQueries({
            queryKey: ["coding-workflows", "aca-project-tasks", selectedProjectSlug],
          }),
          queryClient.invalidateQueries({
            queryKey: ["coding-workflows", "aca-projects"],
          }),
        ]);
        toast("ok", `Published ${publishedTasks.length} task(s) to the GitHub Project.`);
      } else {
        const exportMarkdown = buildExportMarkdown(planPreview, tasks, {
          goal,
          workspaceRoot,
          projectSlug: selectedProjectSlug,
          notes,
          plannerProvider,
          plannerModel,
        });
        setPublishedTasks(
          tasks.map((task) => ({
            title: task.title,
            publishedAtMs: Date.now(),
          }))
        );
        setPublishStatus(
          "Saved a local task bundle. Use the exported markdown to update your kanban file."
        );
        if (navigator.clipboard?.writeText) {
          await navigator.clipboard.writeText(exportMarkdown);
        }
        toast(
          "ok",
          "Saved a local task bundle. The plan markdown has been copied to your clipboard."
        );
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setPublishStatus(message);
      toast("err", message);
    } finally {
      setPublishing(false);
    }
  };

  const exportMarkdown = useMemo(
    () =>
      buildExportMarkdown(planPreview, plannedTasks, {
        goal,
        workspaceRoot,
        projectSlug: selectedProjectSlug,
        notes,
        plannerProvider,
        plannerModel,
      }),
    [
      goal,
      notes,
      planPreview,
      plannedTasks,
      plannerModel,
      plannerProvider,
      selectedProjectSlug,
      workspaceRoot,
    ]
  );

  const plannerTitle = isGitHubProject
    ? "GitHub Project task planning"
    : "Local kanban task planning";
  const hasPlannerResponse =
    !!planPreview ||
    !!plannerDiagnostics ||
    planningChangeSummary.length > 0 ||
    (Array.isArray(planningConversation?.messages) && planningConversation.messages.length > 0);

  useEffect(() => {
    if (rootRef.current) renderIcons(rootRef.current);
  }, [
    clarification.status,
    displayTasks.length,
    hasPlannerResponse,
    isPlanning,
    plannerError,
    planPreview,
    planningChangeSummary.length,
    planningConversation,
    publishStatus,
    saveStatus,
  ]);

  return (
    <div ref={rootRef} className="grid gap-4">
      <PanelCard
        title={plannerTitle}
        subtitle="Use the built-in scrum-master planner to turn a goal into reviewable implementation tasks."
      >
        <div className="grid gap-4">
          <div className="rounded-2xl border border-white/10 bg-black/20 p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <div className="text-xs uppercase tracking-wide text-slate-500">
                  Planner sessions
                </div>
                <div className="tcp-subtle text-xs">
                  Keep multiple coding plans per project instead of one long planner thread.
                </div>
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  className="tcp-btn"
                  onClick={() => setSessionsOpen((open) => !open)}
                >
                  <i data-lucide="history"></i>
                  {sessionsOpen ? "Hide sessions" : "Show sessions"}
                </button>
                <button type="button" className="tcp-btn" onClick={createNewPlannerSession}>
                  <i data-lucide="plus"></i>
                  New plan
                </button>
              </div>
            </div>
            {sessionsOpen ? (
              <div className="mt-3 grid gap-2">
                {plannerSessions.map((session) => (
                  <div
                    key={session.id}
                    className="flex items-center gap-2 rounded-xl border border-white/10 bg-black/20 p-2"
                  >
                    <button
                      type="button"
                      className={`tcp-btn flex-1 justify-start text-left ${
                        session.id === selectedSessionId ? "tcp-btn-primary" : ""
                      }`}
                      onClick={() => {
                        setSelectedSessionId(session.id);
                        setSessionsOpen(false);
                      }}
                      title={session.title}
                    >
                      <span className="truncate">{session.title || "Untitled plan"}</span>
                    </button>
                    <button
                      type="button"
                      className="tcp-btn"
                      title="Rename session"
                      onClick={() => void renamePlannerSession(session.id)}
                    >
                      <i data-lucide="pencil"></i>
                    </button>
                    <button
                      type="button"
                      className="tcp-btn"
                      title="Duplicate session"
                      onClick={() => void duplicatePlannerSession(session.id)}
                    >
                      <i data-lucide="copy"></i>
                    </button>
                    <div className="tcp-subtle hidden text-xs sm:block">
                      {session.updatedAtMs
                        ? new Date(session.updatedAtMs).toLocaleString()
                        : "Saved locally"}
                    </div>
                    <button
                      type="button"
                      className="tcp-btn"
                      title="Delete session"
                      onClick={() => deletePlannerSession(session.id)}
                    >
                      <i data-lucide="trash-2"></i>
                    </button>
                  </div>
                ))}
              </div>
            ) : null}
          </div>

          <div className="rounded-2xl border border-emerald-500/20 bg-emerald-500/5 p-4">
            <div className="flex flex-wrap items-center gap-2">
              <Badge tone="ok">scrum-master</Badge>
              <Badge tone={isGitHubProject ? "info" : "warn"}>
                {taskSourceType || "unknown task source"}
              </Badge>
              <Badge tone={engineHealthy ? "ok" : "warn"}>
                {engineHealthy ? "Engine ready" : "Engine unavailable"}
              </Badge>
            </div>
            <p className="tcp-subtle mt-3 text-sm">
              Describe the work once, let the planner break it into repo-aware tasks, then revise it
              with comments before publishing. GitHub Project approval creates issues and moves them
              into the board. Local kanban approval saves a durable export bundle.
            </p>
          </div>

          <div className="grid gap-3 xl:grid-cols-2">
            <div className="rounded-2xl border border-white/10 bg-black/20 p-4 xl:col-span-2">
              <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                <div>
                  <div className="text-xs uppercase tracking-wide text-slate-500">
                    Planner model
                  </div>
                  <div className="tcp-subtle text-xs">
                    Leave this on workspace default to use the base provider and model, or pick an
                    explicit planner override for richer revisions.
                  </div>
                </div>
                <Badge tone={plannerCanUseLlm ? "ok" : "warn"}>
                  {hasExplicitPlannerOverride
                    ? "Planner override active"
                    : hasBasePlannerModel
                      ? "Workspace default active"
                      : "No planner model configured"}
                </Badge>
              </div>
              <ProviderModelSelector
                providerLabel="Planner provider"
                modelLabel="Planner model"
                draft={{ provider: plannerProvider, model: plannerModel }}
                providers={providerOptions}
                onChange={({ provider, model }) => {
                  setPlannerProvider(provider);
                  setPlannerModel(model);
                }}
                inheritLabel="Workspace default"
                disabled={isPlanning}
              />
              <div className="mt-3 flex flex-wrap gap-2">
                <button
                  type="button"
                  className="tcp-btn"
                  onClick={() => {
                    setPlannerProvider("");
                    setPlannerModel("");
                  }}
                  disabled={isPlanning || (!plannerProvider && !plannerModel)}
                >
                  Use workspace default
                </button>
                <button
                  type="button"
                  className="tcp-btn"
                  onClick={() => {
                    setPlannerProvider(providerStatus.defaultProvider || "");
                    setPlannerModel(providerStatus.defaultModel || "");
                  }}
                  disabled={
                    isPlanning || !hasBasePlannerModel || plannerSelectionMatchesWorkspaceDefault
                  }
                >
                  Restore workspace model
                </button>
                <button
                  type="button"
                  className="tcp-btn"
                  onClick={() => {
                    setPlannerError("");
                    setPlannerDiagnostics(null);
                    setSaveStatus("");
                  }}
                  disabled={isPlanning || (!plannerError && !plannerDiagnostics)}
                >
                  Clear diagnostics
                </button>
              </div>
              <div className="mt-2 text-xs text-slate-500">
                Base model:{" "}
                {hasBasePlannerModel
                  ? `${safeString(providerStatus.defaultProvider)} / ${safeString(providerStatus.defaultModel)}`
                  : "not configured"}
              </div>
            </div>
            <label className="grid gap-2 xl:col-span-2">
              <span className="text-xs uppercase tracking-wide text-slate-500">Workspace root</span>
              <input
                className="tcp-input text-sm"
                value={workspaceRoot}
                onInput={(event) => setWorkspaceRoot((event.target as HTMLInputElement).value)}
                placeholder="/absolute/path/to/the/repo/checkout"
                disabled={isPlanning}
              />
              {resolvedWorkspaceRoot ? (
                <span className="tcp-subtle text-xs">
                  Resolved from the selected project: {resolvedWorkspaceRoot}
                </span>
              ) : null}
              <span className="tcp-subtle text-xs">
                The planner uses this checkout to inspect files and glob directories for the
                selected project.
              </span>
            </label>
            <label className="grid gap-2 xl:col-span-2">
              <span className="text-xs uppercase tracking-wide text-slate-500">Planner chat</span>
            </label>
          </div>

          <ChatInterfacePanel
            messages={plannerChatMessages}
            emptyText="Send your first planner message to start the chat flow."
            inputValue={plannerInput}
            inputPlaceholder={plannerInputPlaceholder}
            sendLabel={planPreview ? "Send to planner" : "Generate plan"}
            onInputChange={setPlannerInput}
            onSend={() =>
              void (planPreview ? reviseMutation(plannerInput) : previewMutation(plannerInput))
            }
            sendDisabled={isPlanning || !safeString(plannerInput)}
            inputDisabled={isPlanning}
            statusTitle={plannerStatusTitle}
            statusDetail={isPlanning ? plannerStatusDetail : ""}
            questionTitle="Planner question"
            questionText={clarification.status === "waiting" ? clarification.question : ""}
            quickReplies={clarification.status === "waiting" ? clarification.options : []}
            onQuickReply={(option) => void reviseMutation(option.label)}
            questionHint="Reply in the planner chat box below or choose a suggested answer."
          />

          <div className="flex flex-wrap gap-2">
            {planPreview ? (
              <button
                type="button"
                className="tcp-btn"
                onClick={() => void previewMutation(plannerInput || goal)}
                disabled={isPlanning}
              >
                <i data-lucide="refresh-cw"></i>
                Regenerate plan
              </button>
            ) : null}
            <button
              type="button"
              className="tcp-btn"
              disabled={resetting || isPlanning}
              onClick={() => void resetMutation()}
            >
              <i data-lucide="rotate-ccw"></i>
              {resetting ? "Resetting…" : "Reset plan"}
            </button>
            <button
              type="button"
              className="tcp-btn"
              onClick={() => {
                setPlannerInput("");
                setNotes("");
                setPlannerError("");
                setPlannerDiagnostics(null);
                setClarification({ status: "none" });
                setSaveStatus("");
              }}
              disabled={
                isPlanning || (!plannerInput && !notes && !plannerError && !plannerDiagnostics)
              }
            >
              <i data-lucide="eraser"></i>
              Clear composer and warnings
            </button>
            <button
              type="button"
              className="tcp-btn"
              onClick={() => {
                void patchActivePlannerSession({
                  title: plannerSessionTitle({
                    goal,
                    plan: planPreview,
                    fallbackTime: lastSavedAtMs || Date.now(),
                  }),
                  workspace_root: workspaceRoot,
                  goal,
                  notes,
                  planner_provider: plannerProvider,
                  planner_model: plannerModel,
                  plan_source: "coding_task_planning",
                  allowed_mcp_servers: connectedMcpServers,
                  operator_preferences: plannerOperatorPreferences({
                    plannerProvider,
                    plannerModel,
                    defaultProvider: providerStatus.defaultProvider,
                    defaultModel: providerStatus.defaultModel,
                    selectedProjectSlug,
                    taskSourceType,
                    selectedProject,
                    goal,
                    notes,
                  }),
                  current_plan_id: planPreview?.plan_id,
                  draft: planPreview
                    ? {
                        initial_plan: planPreview,
                        current_plan: planPreview,
                        plan_revision: Number(planPreview.plan_revision || 1) || 1,
                        conversation: planningConversation || {
                          conversation_id: `wfchat-${selectedSessionId}`,
                          plan_id: planPreview.plan_id,
                          created_at_ms: Date.now(),
                          updated_at_ms: Date.now(),
                          messages: [],
                        },
                        planner_diagnostics: plannerDiagnostics,
                        last_success_materialization: null,
                      }
                    : undefined,
                  published_at_ms: lastSavedAtMs || undefined,
                  published_tasks: publishedTasks.length ? publishedTasks : undefined,
                });
                setSaveStatus("Planner session synced.");
                toast("ok", "Task planning session synced.");
              }}
              disabled={isPlanning}
            >
              <i data-lucide="save"></i>
              Sync session
            </button>
            <button
              type="button"
              className="tcp-btn"
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(exportMarkdown);
                  toast("ok", "Task draft markdown copied to clipboard.");
                } catch {
                  toast("warn", "Could not copy the markdown export to the clipboard.");
                }
              }}
              disabled={isPlanning}
            >
              <i data-lucide="copy"></i>
              Copy markdown
            </button>
          </div>

          {plannerError && !clarificationNeeded ? (
            <div
              className={`rounded-2xl p-3 text-sm ${
                plannerTimedOut
                  ? "border border-amber-500/40 bg-amber-950/30 text-amber-100"
                  : clarificationNeeded
                    ? "border border-amber-500/40 bg-amber-950/30 text-amber-100"
                    : "border border-red-500/40 bg-red-950/30 text-red-200"
              }`}
            >
              {plannerTimedOut ? (
                <div className="mb-1 text-xs uppercase tracking-wide text-amber-300">
                  Planner timed out
                </div>
              ) : clarificationNeeded ? (
                <div className="mb-1 text-xs uppercase tracking-wide text-amber-300">
                  Planner question
                </div>
              ) : null}
              {plannerError}
            </div>
          ) : null}

          {plannerDiagnostics || planningChangeSummary.length ? (
            <PlannerDiagnosticsPanel
              plannerDiagnostics={{
                ...plannerDiagnostics,
                summary:
                  plannerDiagnostics?.summary ||
                  plannerDiagnostics?.detail ||
                  (plannerFallbackReason === "no_planner_model"
                    ? "The planner fell back because no usable planner model reached the backend for this generated plan."
                    : plannerFallbackReason === "clarification_needed"
                      ? "The planner needs one more answer before it can generate a richer repo-aware plan."
                      : ""),
              }}
              teachingLibrary={null}
              planningChangeSummary={planningChangeSummary}
            />
          ) : null}

          {saveStatus || publishStatus ? (
            <div className="rounded-2xl border border-white/10 bg-black/20 p-3 text-sm text-slate-300">
              {saveStatus ? <div>{saveStatus}</div> : null}
              {publishStatus ? <div className="mt-1">{publishStatus}</div> : null}
              {lastSavedAtMs ? (
                <div className="mt-1 text-xs text-slate-500">
                  Last saved {new Date(lastSavedAtMs).toLocaleString()}
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
      </PanelCard>

      {hasPlannerResponse ? (
        <div className="grid gap-4">
          <PanelCard
            title="Plan details"
            subtitle="Planner metadata and markdown stay visible here"
          >
            {planPreview ? (
              <div className="grid gap-3">
                <div className="grid gap-2 text-sm text-slate-300">
                  <div>
                    <span className="tcp-subtle">Title:</span>{" "}
                    {safeString(planPreview?.title) || "Untitled plan"}
                  </div>
                  <div>
                    <span className="tcp-subtle">Confidence:</span>{" "}
                    {safeString(planPreview?.confidence) || "unknown"}
                  </div>
                  <div>
                    <span className="tcp-subtle">Plan source:</span>{" "}
                    {safeString(planPreview?.plan_source || planPreview?.planSource) ||
                      "coding_task_planning"}
                  </div>
                </div>
                {typeof planPreview?.description === "string" && planPreview.description.trim() ? (
                  <div className="rounded-2xl border border-white/10 bg-black/20 p-3">
                    <div className="text-xs uppercase tracking-wide text-slate-500">
                      Planner markdown
                    </div>
                    <div
                      className="tcp-markdown tcp-markdown-ai mt-2 text-sm"
                      dangerouslySetInnerHTML={{
                        __html: renderMarkdownSafe(String(planPreview.description || "")),
                      }}
                    />
                  </div>
                ) : null}
                <div className="grid gap-2">
                  <button
                    type="button"
                    className="tcp-btn-primary"
                    disabled={
                      publishing ||
                      isPlanning ||
                      !goal.trim() ||
                      !workspaceRoot.trim() ||
                      clarificationNeeded ||
                      plannerTimedOut ||
                      planIsFallbackOnly
                    }
                    onClick={() => void publishTasks()}
                  >
                    <i
                      data-lucide={
                        isGitHubProject && canPublishToGitHub ? "badge-check" : "arrow-up-circle"
                      }
                    ></i>
                    {publishing
                      ? "Publishing…"
                      : isGitHubProject && canPublishToGitHub
                        ? "Approve and publish to GitHub Project"
                        : "Save local task bundle"}
                  </button>
                  <div className="text-xs text-slate-500">
                    {clarificationNeeded
                      ? "Answer the planner's question before approving or publishing tasks."
                      : plannerTimedOut
                        ? "Retry the planner revision or switch models before approving tasks."
                        : planIsFallbackOnly
                          ? "Wait for a real task breakdown before approving or publishing tasks."
                          : isGitHubProject && canPublishToGitHub
                            ? "This will create GitHub issues, add each issue to the selected project board, and move it into Ready when the board metadata is available."
                            : "Local kanban mode saves the plan locally so you can apply it to the board file or keep it as a durable draft."}
                  </div>
                </div>
              </div>
            ) : (
              <EmptyState text="No plan has been generated yet." />
            )}
          </PanelCard>

          <PanelCard
            title="Planned tasks"
            subtitle="Review the generated backlog before publishing"
          >
            {displayTasks.length ? (
              <div className="grid gap-3">
                {displayTasks.map((task, index) => (
                  <div
                    key={`${task.id}-${index}`}
                    className="rounded-2xl border border-white/10 bg-black/20 p-4"
                  >
                    <div className="flex flex-wrap items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="text-sm font-semibold text-slate-100">{task.title}</div>
                        <div className="mt-1 text-xs text-slate-500">
                          {task.kind || "task"}
                          {task.dependsOn.length
                            ? ` · depends on ${task.dependsOn.join(", ")}`
                            : ""}
                        </div>
                      </div>
                      <Badge tone="info">Step {index + 1}</Badge>
                    </div>
                    <div className="mt-3 grid gap-2 text-sm text-slate-300">
                      <div>
                        <span className="tcp-subtle">Summary:</span> {task.objective}
                      </div>
                      {task.outputContract ? (
                        <div>
                          <span className="tcp-subtle">Expected result:</span> {task.outputContract}
                        </div>
                      ) : null}
                      {task.inputRefs.length ? (
                        <div className="text-xs text-slate-500">
                          <span className="tcp-subtle">Inputs:</span>{" "}
                          {task.inputRefs
                            .map((row) => `${row.alias} <- ${row.fromStepId}`)
                            .join(", ")}
                        </div>
                      ) : null}
                    </div>
                  </div>
                ))}
              </div>
            ) : planPreview ? (
              <EmptyState
                text={
                  clarificationNeeded
                    ? "Answer the planner's question to generate a real task breakdown."
                    : plannerTimedOut
                      ? "The last revision timed out, so the planner kept the current fallback draft."
                      : "The planner returned a plan, but no usable step list was available."
                }
              />
            ) : (
              <EmptyState text="Generate a plan to see task drafts here." />
            )}
          </PanelCard>
        </div>
      ) : null}
    </div>
  );
}
