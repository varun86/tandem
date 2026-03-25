import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useQueryClient } from "@tanstack/react-query";
import { Badge, PanelCard } from "../ui/index.tsx";
import { EmptyState } from "./ui";
import { ProviderModelSelector } from "../components/ProviderModelSelector";
import { renderMarkdownSafe } from "../lib/markdown";

type ProviderOption = {
  id: string;
  models: string[];
  configured?: boolean;
};

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

const DRAFT_PREFIX = "tcp.coding.task-planning.v1";

function safeString(value: unknown) {
  return String(value || "").trim();
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

function storageKey(projectSlug: string) {
  return `${DRAFT_PREFIX}:${safeString(projectSlug) || "unbound"}`;
}

function loadDraft(projectSlug: string): PlanningDraft | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = localStorage.getItem(storageKey(projectSlug));
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return null;
    return {
      goal: safeString(parsed.goal),
      workspaceRoot: safeString(parsed.workspaceRoot),
      notes: safeString(parsed.notes),
      plannerProvider: safeString(parsed.plannerProvider || parsed.planner_provider),
      plannerModel: safeString(parsed.plannerModel || parsed.planner_model),
      plan: parsed.plan ?? null,
      conversation: parsed.conversation ?? null,
      changeSummary: Array.isArray(parsed.changeSummary)
        ? parsed.changeSummary.map((row: any) => safeString(row)).filter(Boolean)
        : [],
      plannerError: safeString(parsed.plannerError),
      plannerDiagnostics: parsed.plannerDiagnostics ?? null,
      publishedAtMs: Number(parsed.publishedAtMs || 0) || null,
      publishedTasks: Array.isArray(parsed.publishedTasks) ? parsed.publishedTasks : [],
    };
  } catch {
    return null;
  }
}

function saveDraft(projectSlug: string, draft: PlanningDraft) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(
      storageKey(projectSlug),
      JSON.stringify({
        ...draft,
        updatedAtMs: Date.now(),
      })
    );
  } catch {
    // ignore
  }
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
  const queryClient = useQueryClient();
  const [goal, setGoal] = useState("");
  const [workspaceRoot, setWorkspaceRoot] = useState("");
  const [notes, setNotes] = useState("");
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
  const [planningState, setPlanningState] = useState<"idle" | "generating" | "revising">("idle");
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
  const localDraftKey = useMemo(() => storageKey(selectedProjectSlug), [selectedProjectSlug]);
  const isGitHubProject = String(taskSourceType || "").trim() === "github_project";
  const providerOptions = useMemo<ProviderOption[]>(() => {
    const rows = Array.isArray(providersCatalogQuery.data?.all)
      ? providersCatalogQuery.data.all
      : [];
    const configuredProviders = ((providersConfigQuery.data?.providers as
      | Record<string, any>
      | undefined) || {}) as Record<string, any>;
    const mapped = rows
      .map((provider: any) => ({
        id: String(provider?.id || "").trim(),
        models: Object.keys(provider?.models || {}),
        configured: !!configuredProviders[String(provider?.id || "").trim()],
      }))
      .filter((provider: ProviderOption) => !!provider.id)
      .sort((a: ProviderOption, b: ProviderOption) => a.id.localeCompare(b.id));
    const defaultProvider = safeString(providerStatus.defaultProvider);
    const defaultModel = safeString(providerStatus.defaultModel);
    if (defaultProvider && !mapped.some((row) => row.id === defaultProvider)) {
      mapped.unshift({
        id: defaultProvider,
        models: defaultModel ? [defaultModel] : [],
        configured: true,
      });
    }
    return mapped;
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
  const isRevisingPlan = planningState === "revising";
  const isPlanning = planningState !== "idle";
  const plannerFallbackReason = safeString(
    plannerDiagnostics?.fallback_reason || plannerDiagnostics?.fallbackReason
  );
  const clarificationNeeded = plannerFallbackReason === "clarification_needed";
  const clarificationQuestion =
    (clarificationNeeded && safeString(plannerError)) ||
    (clarificationNeeded &&
      safeString(
        Array.isArray(planningConversation?.messages)
          ? planningConversation.messages[planningConversation.messages.length - 1]?.text
          : ""
      )) ||
    "";

  useEffect(() => {
    const draft = loadDraft(selectedProjectSlug);
    setLoadingDraft(true);
    if (draft) {
      setGoal(draft.goal);
      setWorkspaceRoot(draft.workspaceRoot || resolvedWorkspaceRoot || workspaceRootSeed || "");
      setNotes(draft.notes);
      setPlannerProvider(draft.plannerProvider || "");
      setPlannerModel(draft.plannerModel || "");
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
    } else {
      setGoal("");
      setWorkspaceRoot(resolvedWorkspaceRoot || workspaceRootSeed || "");
      setNotes("");
      setPlannerProvider(providerStatus.defaultProvider || "");
      setPlannerModel(providerStatus.defaultModel || "");
      setPlanPreview(null);
      setPlanningConversation(null);
      setPlanningChangeSummary([]);
      setPlannerError("");
      setPlannerDiagnostics(null);
      setPublishStatus("");
      setLastSavedAtMs(null);
      setPublishedTasks([]);
    }
    setLoadingDraft(false);
  }, [
    localDraftKey,
    providerStatus.defaultModel,
    providerStatus.defaultProvider,
    resolvedWorkspaceRoot,
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
    if (loadingDraft) return;
    saveDraft(selectedProjectSlug, {
      goal,
      workspaceRoot,
      notes,
      plannerProvider,
      plannerModel,
      plan: planPreview,
      conversation: planningConversation,
      changeSummary: planningChangeSummary,
      plannerError,
      plannerDiagnostics,
      publishedAtMs: lastSavedAtMs,
      publishedTasks,
    });
  }, [
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
    plannerModel,
    plannerProvider,
    workspaceRoot,
  ]);

  const previewMutation = async () => {
    const trimmedGoal = safeString(goal);
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
    if (!client?.workflowPlans?.chatStart) {
      toast("err", "This control panel build is missing workflow planner support.");
      return;
    }
    setSaveStatus("");
    setPlannerError("");
    setPlannerDiagnostics(null);
    setPlanningState("generating");
    try {
      const response = await client.workflowPlans.chatStart({
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
      setPlannerError(
        typeof response?.clarifier?.question === "string" ? String(response.clarifier.question) : ""
      );
      setLastSavedAtMs(Date.now());
      setPublishedTasks([]);
      setSaveStatus("Plan saved locally.");
      toast(
        "ok",
        typeof response?.clarifier?.question === "string"
          ? "Planner needs clarification before it can draft richer tasks."
          : "Planner generated a task draft."
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setPlannerError(message);
      setSaveStatus(
        message.includes("Request timed out after")
          ? "The panel timed out before the planner finished. The engine may still be working, so wait a moment before retrying."
          : ""
      );
      toast("err", message);
    } finally {
      setPlanningState("idle");
    }
  };

  const reviseMutation = async () => {
    if (!client?.workflowPlans?.chatMessage || !planPreview?.plan_id) {
      toast("warn", "Generate a plan first before revising it.");
      return;
    }
    const trimmedNotes = safeString(notes);
    if (!trimmedNotes) {
      toast("warn", "Add a note or revision request first.");
      return;
    }
    setSaveStatus("");
    setPlannerError("");
    setPlannerDiagnostics(null);
    setPlanningState("revising");
    try {
      const response = await client.workflowPlans.chatMessage({
        plan_id: planPreview.plan_id,
        message: [
          `Revise the workflow plan for repo ${safeString(workspaceRoot)}.`,
          `Goal: ${safeString(goal)}`,
          selectedProjectSlug ? `Selected project: ${selectedProjectSlug}` : "",
          "Keep the workflow valid and machine-parseable.",
          "Prefer concrete repo-aware implementation steps.",
          "If the request is ambiguous, return a clarification question.",
          "",
          "Revision notes:",
          trimmedNotes,
        ].join("\n"),
      });
      setPlanPreview(response?.plan || null);
      setPlanningConversation(response?.conversation || null);
      setPlanningChangeSummary(
        Array.isArray(response?.change_summary)
          ? response.change_summary.map((row: any) => safeString(row)).filter(Boolean)
          : []
      );
      setPlannerDiagnostics(response?.planner_diagnostics || response?.plannerDiagnostics || null);
      setPlannerError(
        typeof response?.clarifier?.question === "string" ? String(response.clarifier.question) : ""
      );
      setLastSavedAtMs(Date.now());
      setPublishedTasks([]);
      setSaveStatus("Revision saved locally.");
      toast("ok", "Plan revised.");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setPlannerError(message);
      setSaveStatus(
        message.includes("Request timed out after")
          ? "The panel timed out before the planner revision finished. The engine may still be working, so wait a moment before retrying."
          : ""
      );
      toast("err", message);
    } finally {
      setPlanningState("idle");
    }
  };

  const resetMutation = async () => {
    if (!planPreview?.plan_id || !client?.workflowPlans?.chatReset) {
      setGoal("");
      setNotes("");
      setPlanPreview(null);
      setPlanningConversation(null);
      setPlanningChangeSummary([]);
      setPlannerError("");
      setPlannerDiagnostics(null);
      setPublishStatus("");
      setSaveStatus("Draft reset locally.");
      saveDraft(selectedProjectSlug, {
        goal: "",
        workspaceRoot,
        notes: "",
        plannerProvider,
        plannerModel,
        plan: null,
        conversation: null,
        changeSummary: [],
        plannerError: "",
        plannerDiagnostics: null,
        publishedAtMs: null,
        publishedTasks: [],
      });
      return;
    }
    setResetting(true);
    try {
      const response = await client.workflowPlans.chatReset({
        plan_id: planPreview.plan_id,
      });
      setPlanPreview(response?.plan || null);
      setPlanningConversation(response?.conversation || null);
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
        saveDraft(selectedProjectSlug, {
          goal,
          workspaceRoot,
          notes,
          plannerProvider,
          plannerModel,
          plan: planPreview,
          conversation: planningConversation,
          changeSummary: planningChangeSummary,
          plannerError,
          plannerDiagnostics,
          publishedAtMs: Date.now(),
          publishedTasks,
        });
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
        saveDraft(selectedProjectSlug, {
          goal,
          workspaceRoot,
          notes,
          plannerProvider,
          plannerModel,
          plan: planPreview,
          conversation: planningConversation,
          changeSummary: planningChangeSummary,
          plannerError,
          plannerDiagnostics,
          publishedAtMs: Date.now(),
          publishedTasks: tasks.map((task) => ({
            title: task.title,
            publishedAtMs: Date.now(),
          })),
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

  return (
    <div className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(340px,0.9fr)]">
      <PanelCard
        title={plannerTitle}
        subtitle="Use the built-in scrum-master planner to turn a goal into reviewable implementation tasks."
      >
        <div className="grid gap-4">
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
              <span className="text-xs uppercase tracking-wide text-slate-500">Goal</span>
              <textarea
                className="tcp-input min-h-[112px] text-sm"
                value={goal}
                onInput={(event) => setGoal((event.target as HTMLTextAreaElement).value)}
                placeholder="Explain the feature or outcome you want the planner to break down."
                disabled={isPlanning}
              />
            </label>
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
              <span className="text-xs uppercase tracking-wide text-slate-500">Revision notes</span>
              <textarea
                className="tcp-input min-h-[96px] text-sm"
                value={notes}
                onInput={(event) => setNotes((event.target as HTMLTextAreaElement).value)}
                placeholder={
                  clarificationNeeded
                    ? "Answer the planner's question here, then click Answer question."
                    : "Tell the planner what to fix, narrow, or rework in the next pass."
                }
                disabled={isPlanning}
              />
            </label>
          </div>

          {isPlanning ? (
            <div className="rounded-2xl border border-sky-500/30 bg-sky-950/20 p-3 text-sm text-sky-100">
              <div className="flex items-center gap-2 font-medium">
                <i
                  data-lucide="loader-circle"
                  className="h-4 w-4 animate-spin"
                  aria-hidden="true"
                ></i>
                {isGeneratingPlan ? "Generating task plan…" : "Revising task plan…"}
              </div>
              <div className="mt-1 text-xs text-sky-200/80">
                The request is running against the engine now. Repo-aware planning can take a bit
                longer while the model inspects files and drafts the task breakdown.
              </div>
            </div>
          ) : null}

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className="tcp-btn-primary inline-flex items-center gap-2"
              onClick={() => void previewMutation()}
              disabled={isPlanning}
            >
              {isGeneratingPlan ? (
                <i
                  data-lucide="loader-circle"
                  className="h-4 w-4 animate-spin"
                  aria-hidden="true"
                ></i>
              ) : null}
              {isGeneratingPlan
                ? "Generating plan…"
                : planPreview
                  ? "Regenerate plan"
                  : "Generate plan"}
            </button>
            <button
              type="button"
              className="tcp-btn inline-flex items-center gap-2"
              onClick={() => void reviseMutation()}
              disabled={isPlanning || !planPreview?.plan_id || !notes.trim()}
            >
              {isRevisingPlan ? (
                <i
                  data-lucide="loader-circle"
                  className="h-4 w-4 animate-spin"
                  aria-hidden="true"
                ></i>
              ) : null}
              {isRevisingPlan
                ? "Updating…"
                : clarificationNeeded
                  ? "Answer question"
                  : "Revise with notes"}
            </button>
            <button
              type="button"
              className="tcp-btn"
              disabled={resetting || isPlanning}
              onClick={() => void resetMutation()}
            >
              {resetting ? "Resetting…" : "Reset plan"}
            </button>
            <button
              type="button"
              className="tcp-btn"
              onClick={() => {
                setNotes("");
                setPlannerError("");
                setPlannerDiagnostics(null);
                setSaveStatus("");
              }}
              disabled={isPlanning || (!notes && !plannerError && !plannerDiagnostics)}
            >
              Clear notes and warnings
            </button>
            <button
              type="button"
              className="tcp-btn"
              onClick={() => {
                saveDraft(selectedProjectSlug, {
                  goal,
                  workspaceRoot,
                  notes,
                  plannerProvider,
                  plannerModel,
                  plan: planPreview,
                  conversation: planningConversation,
                  changeSummary: planningChangeSummary,
                  plannerError,
                  plannerDiagnostics,
                  publishedAtMs: lastSavedAtMs,
                  publishedTasks,
                });
                setSaveStatus("Draft saved locally.");
                toast("ok", "Task planning draft saved locally.");
              }}
              disabled={isPlanning}
            >
              Save draft
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
              Copy markdown
            </button>
          </div>

          {plannerError ? (
            <div
              className={`rounded-2xl p-3 text-sm ${
                clarificationNeeded
                  ? "border border-amber-500/40 bg-amber-950/30 text-amber-100"
                  : "border border-red-500/40 bg-red-950/30 text-red-200"
              }`}
            >
              {clarificationNeeded ? (
                <div className="mb-1 text-xs uppercase tracking-wide text-amber-300">
                  Planner question
                </div>
              ) : null}
              {plannerError}
            </div>
          ) : null}

          {plannerDiagnostics ? (
            <div className="rounded-2xl border border-white/10 bg-black/20 p-3">
              <div className="text-xs uppercase tracking-wide text-slate-500">
                Planner diagnostics
              </div>
              {plannerFallbackReason ? (
                <div className="mt-2 text-sm text-slate-300">
                  {plannerFallbackReason === "no_planner_model"
                    ? "The planner fell back because no usable planner model reached the backend for this generated plan."
                    : plannerFallbackReason === "clarification_needed"
                      ? "The planner needs one more answer before it can generate a richer repo-aware plan."
                      : `Fallback reason: ${safeString(
                          plannerDiagnostics?.fallback_reason || plannerDiagnostics?.fallbackReason
                        )}`}
                </div>
              ) : null}
              <pre className="mt-2 max-h-48 overflow-auto text-xs text-slate-300">
                {JSON.stringify(plannerDiagnostics, null, 2)}
              </pre>
            </div>
          ) : null}

          {planningChangeSummary.length ? (
            <div className="rounded-2xl border border-emerald-500/30 bg-emerald-950/20 p-3">
              <div className="text-xs uppercase tracking-wide text-emerald-300">
                Latest plan changes
              </div>
              <div className="mt-2 flex flex-wrap gap-2">
                {planningChangeSummary.map((item, index) => (
                  <span key={`${item}-${index}`} className="tcp-badge-ok">
                    {item}
                  </span>
                ))}
              </div>
            </div>
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

      <div className="grid gap-4">
        <PanelCard title="Planned tasks" subtitle="Review the generated backlog before publishing">
          {plannedTasks.length ? (
            <div className="grid gap-3">
              {plannedTasks.map((task, index) => (
                <div
                  key={`${task.id}-${index}`}
                  className="rounded-2xl border border-white/10 bg-black/20 p-4"
                >
                  <div className="flex flex-wrap items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="text-sm font-semibold text-slate-100">{task.title}</div>
                      <div className="mt-1 text-xs text-slate-500">
                        {task.kind || "task"}
                        {task.dependsOn.length ? ` · depends on ${task.dependsOn.join(", ")}` : ""}
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
            <EmptyState text="The planner returned a plan, but no step list was available." />
          ) : (
            <EmptyState text="Generate a plan to see task drafts here." />
          )}
        </PanelCard>

        <PanelCard
          title="Plan details"
          subtitle="The planner's markdown and conversation revisions stay visible here"
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
              <div className="rounded-2xl border border-white/10 bg-black/20 p-3">
                <div className="text-xs uppercase tracking-wide text-slate-500">Conversation</div>
                <div className="mt-2 max-h-72 overflow-auto space-y-3">
                  {Array.isArray(planningConversation?.messages) &&
                  planningConversation.messages.length ? (
                    planningConversation.messages.map((message: any, index: number) => (
                      <div
                        key={`${message?.created_at_ms || index}-${index}`}
                        className="grid gap-1"
                      >
                        <span className="text-[11px] uppercase tracking-wide text-slate-500">
                          {safeString(message?.role || "assistant")}
                        </span>
                        <div className="text-sm text-slate-200">
                          {safeString(message?.text || "") || " "}
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="text-sm text-slate-400">
                      The planner conversation will appear here after the first generation.
                    </div>
                  )}
                </div>
              </div>
              <div className="grid gap-2">
                <button
                  type="button"
                  className="tcp-btn-primary"
                  disabled={
                    publishing ||
                    isPlanning ||
                    !goal.trim() ||
                    !workspaceRoot.trim() ||
                    clarificationNeeded
                  }
                  onClick={() => void publishTasks()}
                >
                  {publishing
                    ? "Publishing…"
                    : isGitHubProject && canPublishToGitHub
                      ? "Approve and publish to GitHub Project"
                      : "Save local task bundle"}
                </button>
                <div className="text-xs text-slate-500">
                  {clarificationNeeded
                    ? "Answer the planner's question before approving or publishing tasks."
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
      </div>
    </div>
  );
}
