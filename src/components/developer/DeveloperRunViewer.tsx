import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  Brain,
  Database,
  ExternalLink,
  GitBranch,
  PanelsTopLeft,
  RefreshCw,
  Search,
  SquareCheckBig,
  SquareX,
  Workflow,
} from "lucide-react";
import {
  approveCoderRun,
  cancelCoderRun,
  getCoderMemoryHits,
  getCoderRun,
  listCoderArtifacts,
  listCoderMemoryCandidates,
  listCoderRuns,
  readFileText,
  type CoderArtifactRecord,
  type CoderMemoryCandidateRecord,
  type CoderRunRecord,
} from "@/lib/tauri";
import { DiffViewer } from "@/components/plan/DiffViewer";
import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/Card";
import { cn } from "@/lib/utils";

type DeveloperRunViewerProps = {
  repoSlug?: string | null;
  onOpenMcpSettings?: () => void;
};

type RunTaskRecord = Record<string, unknown>;
type BlackboardRow = Record<string, unknown>;
type BlackboardTimelineItem = {
  id: string;
  kind: "decision" | "question";
  text: string;
  tsMs: number | null;
  stepId: string | null;
  sourceEventId: string | null;
};

const TASK_COLUMNS = [
  { key: "runnable", label: "Runnable" },
  { key: "in_progress", label: "In Progress" },
  { key: "blocked", label: "Blocked" },
  { key: "done", label: "Done" },
  { key: "failed", label: "Failed" },
] as const;

function formatTimestamp(value?: number | null): string {
  if (!value) return "Unknown";
  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(value);
  } catch {
    return String(value);
  }
}

function asArray<T>(value: T[] | undefined | null): T[] {
  return Array.isArray(value) ? value : [];
}

function statusTone(status?: string | null): string {
  switch ((status ?? "").toLowerCase()) {
    case "completed":
      return "border-emerald-500/40 bg-emerald-500/10 text-emerald-200";
    case "failed":
    case "blocked":
      return "border-rose-500/40 bg-rose-500/10 text-rose-200";
    case "awaiting_approval":
      return "border-amber-500/40 bg-amber-500/10 text-amber-100";
    case "running":
    case "planning":
      return "border-sky-500/40 bg-sky-500/10 text-sky-200";
    case "cancelled":
      return "border-zinc-500/40 bg-zinc-500/10 text-zinc-300";
    default:
      return "border-border bg-surface-elevated text-text";
  }
}

function renderValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value, null, 2) ?? "";
}

function pickText(value: unknown): string {
  if (typeof value === "string" && value.trim().length > 0) return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return "";
}

function blackboardRowText(row: BlackboardRow): string {
  return (
    pickText(row.text) ||
    pickText(row.summary) ||
    pickText(row.label) ||
    pickText(row.title) ||
    pickText(row.reason) ||
    renderValue(row)
  );
}

function blackboardRowTimestamp(row: BlackboardRow): number | null {
  const value = row.ts_ms;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function blackboardRowStepId(row: BlackboardRow): string | null {
  const value = row.step_id;
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

function blackboardRowSourceEventId(row: BlackboardRow): string | null {
  const value = row.source_event_id;
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

export function DeveloperRunViewer({ repoSlug, onOpenMcpSettings }: DeveloperRunViewerProps) {
  const [runs, setRuns] = useState<CoderRunRecord[]>([]);
  const [runQuery, setRunQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [workflowFilter, setWorkflowFilter] = useState<string>("all");
  const [detailTab, setDetailTab] = useState<"overview" | "artifacts" | "memory">("overview");
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [selectedRun, setSelectedRun] = useState<CoderRunRecord | null>(null);
  const [runState, setRunState] = useState<Record<string, unknown> | null>(null);
  const [artifacts, setArtifacts] = useState<CoderArtifactRecord[]>([]);
  const [memoryHits, setMemoryHits] = useState<Record<string, unknown>[]>([]);
  const [memoryCandidates, setMemoryCandidates] = useState<CoderMemoryCandidateRecord[]>([]);
  const [selectedArtifactPath, setSelectedArtifactPath] = useState<string | null>(null);
  const [selectedArtifactContent, setSelectedArtifactContent] = useState<string>("");
  const [loadingArtifact, setLoadingArtifact] = useState(false);
  const [loadingRuns, setLoadingRuns] = useState(false);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [acting, setActing] = useState<"approve" | "cancel" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadRuns = useCallback(async () => {
    setLoadingRuns(true);
    try {
      const payload = await listCoderRuns({ limit: 40, repoSlug: repoSlug ?? undefined });
      setRuns(payload.runs);
      setSelectedRunId((current) => current ?? payload.runs[0]?.coder_run_id ?? null);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingRuns(false);
    }
  }, [repoSlug]);

  const loadRunDetail = useCallback(async (runId: string) => {
    setLoadingDetail(true);
    try {
      const [runPayload, artifactsPayload, memoryHitsPayload, memoryCandidatesPayload] =
        await Promise.all([
          getCoderRun(runId),
          listCoderArtifacts(runId),
          getCoderMemoryHits(runId, { limit: 8 }),
          listCoderMemoryCandidates(runId),
        ]);
      setSelectedRun(runPayload.coder_run);
      setRunState(runPayload.run);
      setArtifacts(asArray(artifactsPayload.artifacts));
      setMemoryHits(asArray(memoryHitsPayload.hits));
      setMemoryCandidates(asArray(memoryCandidatesPayload.candidates));
      setSelectedArtifactPath((current) => current ?? artifactsPayload.artifacts[0]?.path ?? null);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingDetail(false);
    }
  }, []);

  useEffect(() => {
    void loadRuns();
    const interval = globalThis.setInterval(() => {
      void loadRuns();
    }, 8000);
    return () => globalThis.clearInterval(interval);
  }, [loadRuns]);

  useEffect(() => {
    if (!selectedRunId) {
      setSelectedRun(null);
      setRunState(null);
      setArtifacts([]);
      setMemoryHits([]);
      setMemoryCandidates([]);
      setSelectedArtifactPath(null);
      setSelectedArtifactContent("");
      return;
    }
    void loadRunDetail(selectedRunId);
  }, [loadRunDetail, selectedRunId]);

  useEffect(() => {
    if (!selectedArtifactPath) {
      setSelectedArtifactContent("");
      return;
    }
    let cancelled = false;
    const loadArtifact = async () => {
      setLoadingArtifact(true);
      try {
        const content = await readFileText(selectedArtifactPath, 512 * 1024, 120_000);
        if (!cancelled) {
          setSelectedArtifactContent(content);
        }
      } catch (err) {
        if (!cancelled) {
          setSelectedArtifactContent(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setLoadingArtifact(false);
        }
      }
    };
    void loadArtifact();
    return () => {
      cancelled = true;
    };
  }, [selectedArtifactPath]);

  const selectedTaskRows = useMemo(() => {
    const tasks = runState?.tasks;
    return (Array.isArray(tasks) ? tasks : []) as RunTaskRecord[];
  }, [runState]);

  const selectedBlackboard = useMemo(() => {
    const blackboard = runState?.blackboard;
    return blackboard && typeof blackboard === "object"
      ? (blackboard as Record<string, unknown>)
      : null;
  }, [runState]);

  const decisions = useMemo(() => {
    const rows = selectedBlackboard?.decisions;
    return (Array.isArray(rows) ? rows : []) as BlackboardRow[];
  }, [selectedBlackboard]);

  const openQuestions = useMemo(() => {
    const rows = selectedBlackboard?.open_questions;
    return (Array.isArray(rows) ? rows : []) as BlackboardRow[];
  }, [selectedBlackboard]);

  const latestDecision = useMemo(() => {
    if (decisions.length === 0) return null;
    return (
      [...decisions].sort((left, right) => {
        return (blackboardRowTimestamp(right) ?? 0) - (blackboardRowTimestamp(left) ?? 0);
      })[0] ?? null
    );
  }, [decisions]);

  const blackboardTimeline = useMemo<BlackboardTimelineItem[]>(() => {
    const items = [
      ...decisions.map((row, index) => ({
        id: String(row.id ?? row.source_event_id ?? `decision-${index}`),
        kind: "decision" as const,
        text: blackboardRowText(row),
        tsMs: blackboardRowTimestamp(row),
        stepId: blackboardRowStepId(row),
        sourceEventId: blackboardRowSourceEventId(row),
      })),
      ...openQuestions.map((row, index) => ({
        id: String(row.id ?? row.source_event_id ?? `question-${index}`),
        kind: "question" as const,
        text: blackboardRowText(row),
        tsMs: blackboardRowTimestamp(row),
        stepId: blackboardRowStepId(row),
        sourceEventId: blackboardRowSourceEventId(row),
      })),
    ];
    return items
      .sort((left, right) => {
        const tsDelta = (right.tsMs ?? 0) - (left.tsMs ?? 0);
        if (tsDelta !== 0) return tsDelta;
        if (left.kind !== right.kind) return left.kind === "decision" ? -1 : 1;
        return left.id.localeCompare(right.id);
      })
      .slice(0, 10);
  }, [decisions, openQuestions]);

  const readinessHint = useMemo(() => {
    if (!error) return null;
    const normalized = error.toLowerCase();
    if (normalized.includes("readiness") || normalized.includes("mcp")) {
      return "Coder readiness is blocked. Open MCP settings to inspect server connectivity.";
    }
    return null;
  }, [error]);

  const filteredRuns = useMemo(() => {
    const query = runQuery.trim().toLowerCase();
    return runs.filter((run) => {
      if (statusFilter !== "all" && (run.status ?? "unknown") !== statusFilter) return false;
      if (workflowFilter !== "all" && run.workflow_mode !== workflowFilter) return false;
      if (!query) return true;
      return [
        run.coder_run_id,
        run.repo_binding?.repo_slug,
        run.workflow_mode,
        run.phase,
        run.status,
        run.github_ref ? `${run.github_ref.kind} ${run.github_ref.number}` : "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(query);
    });
  }, [runQuery, runs, statusFilter, workflowFilter]);

  const runStatuses = useMemo(() => {
    return ["all", ...new Set(runs.map((run) => run.status ?? "unknown"))];
  }, [runs]);

  const workflowModes = useMemo(() => {
    return ["all", ...new Set(runs.map((run) => run.workflow_mode))];
  }, [runs]);

  const taskColumns = useMemo(() => {
    const grouped = new Map<string, RunTaskRecord[]>();
    for (const column of TASK_COLUMNS) grouped.set(column.key, []);
    for (const task of selectedTaskRows) {
      const status = String(task.status ?? "pending").toLowerCase();
      const key =
        status === "pending"
          ? "runnable"
          : TASK_COLUMNS.some((column) => column.key === status)
            ? status
            : "runnable";
      grouped.get(key)?.push(task);
    }
    return TASK_COLUMNS.map((column) => ({
      ...column,
      tasks: grouped.get(column.key) ?? [],
    }));
  }, [selectedTaskRows]);

  const artifactHighlights = useMemo(() => {
    return artifacts.filter((artifact) => {
      const type = artifact.artifact_type.toLowerCase();
      const path = artifact.path.toLowerCase();
      return (
        type.includes("memory") ||
        type.includes("triage") ||
        type.includes("validation") ||
        path.includes("duplicate") ||
        path.includes("diff")
      );
    });
  }, [artifacts]);

  const artifactPreview = useMemo(() => {
    if (!selectedArtifactContent) return null;
    try {
      const parsed = JSON.parse(selectedArtifactContent) as Record<string, unknown>;
      const oldValue =
        typeof parsed.old === "string"
          ? parsed.old
          : typeof parsed.before === "string"
            ? parsed.before
            : null;
      const newValue =
        typeof parsed.new === "string"
          ? parsed.new
          : typeof parsed.after === "string"
            ? parsed.after
            : null;
      if (oldValue !== null && newValue !== null) {
        return { kind: "diff" as const, oldValue, newValue };
      }
      return { kind: "raw" as const, value: selectedArtifactContent };
    } catch {
      return { kind: "raw" as const, value: selectedArtifactContent };
    }
  }, [selectedArtifactContent]);

  const handleAction = useCallback(
    async (action: "approve" | "cancel") => {
      if (!selectedRunId) return;
      setActing(action);
      try {
        if (action === "approve") {
          await approveCoderRun(selectedRunId, "approved from desktop developer mode");
        } else {
          await cancelCoderRun(selectedRunId, "cancelled from desktop developer mode");
        }
        await Promise.all([loadRuns(), loadRunDetail(selectedRunId)]);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setActing(null);
      }
    },
    [loadRunDetail, loadRuns, selectedRunId]
  );

  return (
    <div className="h-full w-full overflow-hidden app-background">
      <div className="grid h-full grid-cols-[340px_minmax(0,1fr)] gap-4 p-4">
        <Card className="flex min-h-0 flex-col p-0">
          <CardHeader className="border-b border-border px-5 py-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <CardTitle className="text-base">Developer</CardTitle>
                <CardDescription>
                  Coder runs, artifacts, memory hits, and operator controls.
                </CardDescription>
              </div>
              <Button
                variant="ghost"
                size="sm"
                loading={loadingRuns}
                onClick={() => void loadRuns()}
                aria-label="Refresh coder runs"
              >
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>
          </CardHeader>
          <CardContent className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
            <div className="mb-3 space-y-2">
              <div className="flex items-center gap-2 rounded-2xl border border-border bg-surface-elevated/40 px-3 py-2">
                <Search className="h-4 w-4 text-text-muted" />
                <input
                  value={runQuery}
                  onChange={(event) => setRunQuery(event.target.value)}
                  placeholder="Filter runs by id, repo, mode, or ref"
                  className="w-full bg-transparent text-sm text-text outline-none placeholder:text-text-subtle"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <select
                  value={statusFilter}
                  onChange={(event) => setStatusFilter(event.target.value)}
                  className="rounded-xl border border-border bg-surface px-3 py-2 text-sm text-text outline-none"
                >
                  {runStatuses.map((status) => (
                    <option key={status} value={status}>
                      {status === "all" ? "All statuses" : status}
                    </option>
                  ))}
                </select>
                <select
                  value={workflowFilter}
                  onChange={(event) => setWorkflowFilter(event.target.value)}
                  className="rounded-xl border border-border bg-surface px-3 py-2 text-sm text-text outline-none"
                >
                  {workflowModes.map((mode) => (
                    <option key={mode} value={mode}>
                      {mode === "all" ? "All workflows" : mode}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {filteredRuns.length === 0 ? (
              <div className="flex h-full flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-border bg-surface-elevated/40 p-6 text-center">
                <Workflow className="h-6 w-6 text-text-muted" />
                <div>
                  <p className="text-sm font-medium text-text">No matching coder runs.</p>
                  <p className="text-xs text-text-muted">
                    Adjust the filters or wait for the engine to create a run.
                  </p>
                </div>
              </div>
            ) : (
              <div className="space-y-2">
                {filteredRuns.map((run) => (
                  <button
                    key={run.coder_run_id}
                    type="button"
                    onClick={() => setSelectedRunId(run.coder_run_id)}
                    className={cn(
                      "w-full rounded-2xl border px-4 py-3 text-left transition-colors",
                      selectedRunId === run.coder_run_id
                        ? "border-primary/40 bg-primary/10"
                        : "border-border bg-surface hover:bg-surface-elevated"
                    )}
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <p className="truncate text-sm font-semibold text-text">
                          {run.coder_run_id}
                        </p>
                        <p className="truncate text-xs text-text-muted">
                          {run.repo_binding?.repo_slug} • {run.workflow_mode}
                        </p>
                      </div>
                      <span
                        className={cn(
                          "rounded-full border px-2 py-1 text-[10px] font-medium uppercase tracking-[0.2em]",
                          statusTone(run.status)
                        )}
                      >
                        {run.status ?? "unknown"}
                      </span>
                    </div>
                    <div className="mt-3 flex items-center justify-between gap-3 text-[11px] text-text-muted">
                      <span>{run.phase ?? "analysis"}</span>
                      <span>{formatTimestamp(run.updated_at_ms)}</span>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <div className="min-h-0 overflow-y-auto">
          {!selectedRun ? (
            <Card>
              <CardContent className="flex min-h-[280px] flex-col items-center justify-center gap-3 text-center">
                <Workflow className="h-8 w-8 text-text-muted" />
                <div>
                  <p className="text-sm font-medium text-text">Select a coder run.</p>
                  <p className="text-xs text-text-muted">
                    Run detail, artifacts, memory hits, and candidates appear here.
                  </p>
                </div>
              </CardContent>
            </Card>
          ) : (
            <div className="space-y-4">
              <Card>
                <CardHeader>
                  <div className="flex flex-wrap items-start justify-between gap-4">
                    <div>
                      <CardTitle className="text-xl">{selectedRun.coder_run_id}</CardTitle>
                      <CardDescription>
                        {selectedRun.repo_binding.repo_slug} • {selectedRun.workflow_mode} •{" "}
                        {selectedRun.github_ref
                          ? `${selectedRun.github_ref.kind} #${selectedRun.github_ref.number}`
                          : "no GitHub ref"}
                      </CardDescription>
                    </div>
                    <div className="flex items-center gap-2">
                      <Button
                        variant="secondary"
                        size="sm"
                        loading={loadingDetail}
                        onClick={() => selectedRunId && void loadRunDetail(selectedRunId)}
                      >
                        <RefreshCw className="h-4 w-4" />
                        Refresh
                      </Button>
                      <Button
                        variant="primary"
                        size="sm"
                        loading={acting === "approve"}
                        onClick={() => void handleAction("approve")}
                      >
                        <SquareCheckBig className="h-4 w-4" />
                        Approve
                      </Button>
                      <Button
                        variant="danger"
                        size="sm"
                        loading={acting === "cancel"}
                        onClick={() => void handleAction("cancel")}
                      >
                        <SquareX className="h-4 w-4" />
                        Cancel
                      </Button>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="grid gap-3 md:grid-cols-4">
                  {[
                    ["Status", selectedRun.status ?? "unknown"],
                    ["Phase", selectedRun.phase ?? "analysis"],
                    ["Context Run", selectedRun.linked_context_run_id],
                    ["Updated", formatTimestamp(selectedRun.updated_at_ms)],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      className="rounded-2xl border border-border bg-surface-elevated/50 p-3"
                    >
                      <p className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
                        {label}
                      </p>
                      <p className="mt-1 text-sm font-medium text-text">{value}</p>
                    </div>
                  ))}
                </CardContent>
              </Card>

              {error ? (
                <Card className="border-rose-500/40 bg-rose-500/10">
                  <CardContent className="flex items-center gap-3 py-4">
                    <AlertCircle className="h-5 w-5 text-rose-200" />
                    <div className="min-w-0 flex-1">
                      <p className="text-sm text-rose-100">{error}</p>
                      {readinessHint ? (
                        <p className="mt-1 text-xs text-rose-200/90">{readinessHint}</p>
                      ) : null}
                    </div>
                    {readinessHint && onOpenMcpSettings ? (
                      <Button variant="secondary" size="sm" onClick={onOpenMcpSettings}>
                        <ExternalLink className="h-4 w-4" />
                        Open MCP
                      </Button>
                    ) : null}
                  </CardContent>
                </Card>
              ) : null}

              <div className="flex flex-wrap gap-2">
                {[
                  ["overview", "Overview"],
                  ["artifacts", "Artifacts"],
                  ["memory", "Memory"],
                ].map(([key, label]) => (
                  <button
                    key={key}
                    type="button"
                    onClick={() => setDetailTab(key as typeof detailTab)}
                    className={cn(
                      "rounded-full border px-3 py-1.5 text-xs font-medium transition-colors",
                      detailTab === key
                        ? "border-primary/40 bg-primary/10 text-primary"
                        : "border-border bg-surface text-text-muted hover:text-text"
                    )}
                  >
                    {label}
                  </button>
                ))}
              </div>

              {detailTab === "overview" ? (
                <>
                  <Card>
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2 text-base">
                        <PanelsTopLeft className="h-4 w-4" />
                        Coder Kanban
                      </CardTitle>
                      <CardDescription>Projected directly from engine task state.</CardDescription>
                    </CardHeader>
                    <CardContent className="overflow-x-auto">
                      <div className="grid min-w-[980px] grid-cols-5 gap-3">
                        {taskColumns.map((column) => (
                          <div
                            key={column.key}
                            className="rounded-2xl border border-border bg-surface-elevated/30 p-3"
                          >
                            <div className="mb-3 flex items-center justify-between gap-2">
                              <p className="text-xs font-semibold uppercase tracking-[0.2em] text-text-muted">
                                {column.label}
                              </p>
                              <span className="rounded-full border border-border px-2 py-0.5 text-[10px] text-text-muted">
                                {column.tasks.length}
                              </span>
                            </div>
                            <div className="space-y-2">
                              {column.tasks.length === 0 ? (
                                <div className="rounded-xl border border-dashed border-border px-3 py-4 text-center text-xs text-text-muted">
                                  No tasks
                                </div>
                              ) : (
                                column.tasks.map((task, index) => (
                                  <div
                                    key={String(
                                      task.id ?? task.command_id ?? `${column.key}-${index}`
                                    )}
                                    className="rounded-xl border border-border bg-surface p-3"
                                  >
                                    <p className="text-sm font-medium text-text">
                                      {String(
                                        task.title ??
                                          task.workflow_node_id ??
                                          task.task_type ??
                                          task.id ??
                                          "task"
                                      )}
                                    </p>
                                    <p className="mt-1 text-[11px] text-text-muted">
                                      {String(task.workflow_node_id ?? task.task_type ?? "")}
                                    </p>
                                  </div>
                                ))
                              )}
                            </div>
                          </div>
                        ))}
                      </div>
                    </CardContent>
                  </Card>

                  <div className="grid gap-4 xl:grid-cols-2">
                    <Card>
                      <CardHeader>
                        <CardTitle className="flex items-center gap-2 text-base">
                          <GitBranch className="h-4 w-4" />
                          Blackboard And Decisions
                        </CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-4">
                        {selectedBlackboard ? (
                          <>
                            <div className="grid gap-3 md:grid-cols-3">
                              {[
                                ["Decisions", decisions.length],
                                ["Open questions", openQuestions.length],
                                [
                                  "Artifacts",
                                  Array.isArray(selectedBlackboard.artifacts)
                                    ? selectedBlackboard.artifacts.length
                                    : 0,
                                ],
                              ].map(([label, value]) => (
                                <div
                                  key={label}
                                  className="rounded-2xl border border-border bg-surface-elevated/40 p-3"
                                >
                                  <p className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
                                    {label}
                                  </p>
                                  <p className="mt-1 text-lg font-semibold text-text">
                                    {String(value)}
                                  </p>
                                </div>
                              ))}
                            </div>
                            {latestDecision ? (
                              <div className="rounded-3xl border border-primary/20 bg-primary/5 p-4">
                                <div className="flex flex-wrap items-start justify-between gap-3">
                                  <div>
                                    <p className="text-[11px] uppercase tracking-[0.24em] text-primary/80">
                                      Current decision
                                    </p>
                                    <p className="mt-2 text-sm font-medium text-text">
                                      {blackboardRowText(latestDecision)}
                                    </p>
                                  </div>
                                  <p className="text-xs text-text-muted">
                                    {formatTimestamp(blackboardRowTimestamp(latestDecision))}
                                  </p>
                                </div>
                                <div className="mt-3 flex flex-wrap gap-2">
                                  {blackboardRowStepId(latestDecision) ? (
                                    <span className="rounded-full border border-primary/20 bg-surface px-2.5 py-1 text-[10px] uppercase tracking-[0.2em] text-text-muted">
                                      Step {blackboardRowStepId(latestDecision)}
                                    </span>
                                  ) : null}
                                  {blackboardRowSourceEventId(latestDecision) ? (
                                    <span className="rounded-full border border-primary/20 bg-surface px-2.5 py-1 text-[10px] uppercase tracking-[0.2em] text-text-muted">
                                      Event {blackboardRowSourceEventId(latestDecision)}
                                    </span>
                                  ) : null}
                                </div>
                              </div>
                            ) : (
                              <p className="text-sm text-text-muted">
                                No blackboard decisions yet.
                              </p>
                            )}
                            {blackboardTimeline.length > 0 ? (
                              <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_260px]">
                                <div className="rounded-3xl border border-border bg-surface-elevated/30 p-4">
                                  <div className="mb-4 flex items-center justify-between gap-3">
                                    <div>
                                      <p className="text-sm font-medium text-text">
                                        Decision lineage
                                      </p>
                                      <p className="text-xs text-text-muted">
                                        Chronological blackboard activity from the current run.
                                      </p>
                                    </div>
                                    <span className="rounded-full border border-border px-2.5 py-1 text-[10px] uppercase tracking-[0.2em] text-text-muted">
                                      {blackboardTimeline.length} items
                                    </span>
                                  </div>
                                  <div className="space-y-3">
                                    {blackboardTimeline.map((item, index) => (
                                      <div key={item.id} className="flex gap-3">
                                        <div className="flex w-10 flex-col items-center">
                                          <span
                                            className={cn(
                                              "mt-1 flex h-8 w-8 items-center justify-center rounded-full border text-[10px] font-semibold uppercase tracking-[0.16em]",
                                              item.kind === "decision"
                                                ? "border-primary/30 bg-primary/10 text-primary"
                                                : "border-amber-500/30 bg-amber-500/10 text-amber-100"
                                            )}
                                          >
                                            {item.kind === "decision" ? "D" : "Q"}
                                          </span>
                                          {index < blackboardTimeline.length - 1 ? (
                                            <div className="mt-2 h-full min-h-10 w-px bg-border" />
                                          ) : null}
                                        </div>
                                        <div className="min-w-0 flex-1 rounded-2xl border border-border bg-surface p-3">
                                          <div className="flex flex-wrap items-center gap-2">
                                            <span
                                              className={cn(
                                                "rounded-full border px-2 py-0.5 text-[10px] uppercase tracking-[0.18em]",
                                                item.kind === "decision"
                                                  ? "border-primary/20 bg-primary/10 text-primary"
                                                  : "border-amber-500/20 bg-amber-500/10 text-amber-100"
                                              )}
                                            >
                                              {item.kind === "decision"
                                                ? "Decision"
                                                : "Open question"}
                                            </span>
                                            <span className="text-[11px] text-text-muted">
                                              {formatTimestamp(item.tsMs)}
                                            </span>
                                          </div>
                                          <p className="mt-2 whitespace-pre-wrap break-words text-sm text-text">
                                            {item.text}
                                          </p>
                                          <div className="mt-3 flex flex-wrap gap-2">
                                            {item.stepId ? (
                                              <span className="rounded-full border border-border bg-surface-elevated/50 px-2 py-1 text-[10px] uppercase tracking-[0.18em] text-text-muted">
                                                Step {item.stepId}
                                              </span>
                                            ) : null}
                                            {item.sourceEventId ? (
                                              <span className="rounded-full border border-border bg-surface-elevated/50 px-2 py-1 text-[10px] uppercase tracking-[0.18em] text-text-muted">
                                                Source {item.sourceEventId}
                                              </span>
                                            ) : null}
                                          </div>
                                        </div>
                                      </div>
                                    ))}
                                  </div>
                                </div>

                                <div className="space-y-3">
                                  <div className="rounded-3xl border border-border bg-surface-elevated/30 p-4">
                                    <p className="text-sm font-medium text-text">Open questions</p>
                                    <p className="mt-1 text-xs text-text-muted">
                                      Outstanding uncertainty captured on the blackboard.
                                    </p>
                                    <div className="mt-3 space-y-2">
                                      {openQuestions.length > 0 ? (
                                        openQuestions.slice(0, 4).map((question, index) => (
                                          <div
                                            key={String(
                                              question.id ??
                                                question.source_event_id ??
                                                `open-question-${index}`
                                            )}
                                            className="rounded-2xl border border-border bg-surface p-3"
                                          >
                                            <p className="text-sm text-text">
                                              {blackboardRowText(question)}
                                            </p>
                                            <div className="mt-2 flex flex-wrap gap-2 text-[11px] text-text-muted">
                                              <span>
                                                {formatTimestamp(blackboardRowTimestamp(question))}
                                              </span>
                                              {blackboardRowStepId(question) ? (
                                                <span>step {blackboardRowStepId(question)}</span>
                                              ) : null}
                                            </div>
                                          </div>
                                        ))
                                      ) : (
                                        <p className="text-sm text-text-muted">
                                          No open questions recorded.
                                        </p>
                                      )}
                                    </div>
                                  </div>

                                  <div className="rounded-3xl border border-border bg-surface-elevated/30 p-4">
                                    <p className="text-sm font-medium text-text">Lineage notes</p>
                                    <div className="mt-3 space-y-2 text-xs text-text-muted">
                                      <p>
                                        Decisions and questions are ordered by their recorded
                                        blackboard timestamp.
                                      </p>
                                      <p>
                                        Step and source ids come directly from blackboard rows so
                                        you can correlate them with run tasks and artifacts.
                                      </p>
                                    </div>
                                  </div>
                                </div>
                              </div>
                            ) : null}
                          </>
                        ) : (
                          <p className="text-sm text-text-muted">
                            No blackboard payload returned for this run.
                          </p>
                        )}
                      </CardContent>
                    </Card>

                    <Card>
                      <CardHeader>
                        <CardTitle className="flex items-center gap-2 text-base">
                          <Brain className="h-4 w-4" />
                          Memory Snapshot
                        </CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="grid gap-3 md:grid-cols-2">
                          <div className="rounded-2xl border border-border bg-surface-elevated/40 p-3">
                            <p className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
                              Hits
                            </p>
                            <p className="mt-1 text-lg font-semibold text-text">
                              {memoryHits.length}
                            </p>
                          </div>
                          <div className="rounded-2xl border border-border bg-surface-elevated/40 p-3">
                            <p className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
                              Candidates
                            </p>
                            <p className="mt-1 text-lg font-semibold text-text">
                              {memoryCandidates.length}
                            </p>
                          </div>
                        </div>
                        <div className="space-y-2">
                          {memoryHits.slice(0, 3).map((hit, index) => (
                            <div
                              key={String(hit.candidate_id ?? hit.memory_id ?? index)}
                              className="rounded-2xl border border-border bg-surface-elevated/40 p-3"
                            >
                              <pre className="whitespace-pre-wrap break-words text-[11px] text-text-muted">
                                {renderValue(hit.summary ?? hit.content ?? hit.payload ?? hit)}
                              </pre>
                            </div>
                          ))}
                        </div>
                      </CardContent>
                    </Card>
                  </div>
                </>
              ) : null}

              {detailTab === "artifacts" ? (
                <div className="grid gap-4 xl:grid-cols-2">
                  <Card>
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2 text-base">
                        <Database className="h-4 w-4" />
                        Artifact Feed
                      </CardTitle>
                      <CardDescription>
                        Includes duplicate and memory-backed history from engine artifacts.
                      </CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-2">
                      {artifacts.length === 0 ? (
                        <p className="text-sm text-text-muted">No artifacts yet.</p>
                      ) : (
                        (artifactHighlights.length > 0 ? artifactHighlights : artifacts).map(
                          (artifact) => (
                            <button
                              key={artifact.id}
                              type="button"
                              onClick={() => setSelectedArtifactPath(artifact.path)}
                              className={cn(
                                "w-full rounded-2xl border p-3 text-left",
                                selectedArtifactPath === artifact.path
                                  ? "border-primary/40 bg-primary/10"
                                  : "border-border bg-surface-elevated/40"
                              )}
                            >
                              <div className="flex items-center justify-between gap-3">
                                <p className="text-sm font-medium text-text">
                                  {artifact.artifact_type}
                                </p>
                                <span className="text-xs text-text-muted">
                                  {formatTimestamp(artifact.ts_ms)}
                                </span>
                              </div>
                              <p className="mt-2 break-all font-mono text-[11px] text-text-muted">
                                {artifact.path}
                              </p>
                            </button>
                          )
                        )
                      )}
                    </CardContent>
                  </Card>

                  <Card>
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2 text-base">
                        <Database className="h-4 w-4" />
                        Artifact Inspector
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-3">
                      {selectedArtifactPath ? (
                        <>
                          <div className="rounded-2xl border border-border bg-surface-elevated/40 p-3">
                            <p className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
                              Selected
                            </p>
                            <p className="mt-2 break-all font-mono text-[11px] text-text-muted">
                              {selectedArtifactPath}
                            </p>
                          </div>
                          {loadingArtifact ? (
                            <p className="text-sm text-text-muted">Loading artifact preview…</p>
                          ) : artifactPreview?.kind === "diff" ? (
                            <DiffViewer
                              oldValue={artifactPreview.oldValue}
                              newValue={artifactPreview.newValue}
                              oldTitle="Before"
                              newTitle="After"
                            />
                          ) : (
                            <pre className="max-h-[420px] overflow-auto rounded-2xl border border-border bg-surface-elevated/40 p-3 text-[11px] text-text-muted">
                              {artifactPreview?.value ?? "No artifact preview available."}
                            </pre>
                          )}
                        </>
                      ) : (
                        <p className="text-sm text-text-muted">
                          Select an artifact to inspect its contents.
                        </p>
                      )}
                    </CardContent>
                  </Card>
                </div>
              ) : null}

              {detailTab === "memory" ? (
                <div className="grid gap-4 xl:grid-cols-2">
                  <Card>
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2 text-base">
                        <Brain className="h-4 w-4" />
                        Memory Hits
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-2">
                      {memoryHits.length === 0 ? (
                        <p className="text-sm text-text-muted">No memory hits returned.</p>
                      ) : (
                        memoryHits.map((hit, index) => (
                          <div
                            key={String(hit.candidate_id ?? hit.memory_id ?? index)}
                            className="rounded-2xl border border-border bg-surface-elevated/40 p-3"
                          >
                            <div className="flex items-center justify-between gap-3">
                              <p className="text-sm font-medium text-text">
                                {renderValue(hit.kind ?? hit.source ?? "memory_hit")}
                              </p>
                              <span className="text-xs text-text-muted">
                                {typeof hit.score === "number" ? hit.score.toFixed(2) : ""}
                              </span>
                            </div>
                            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-words text-[11px] text-text-muted">
                              {renderValue(hit.summary ?? hit.content ?? hit.payload ?? hit)}
                            </pre>
                          </div>
                        ))
                      )}
                    </CardContent>
                  </Card>

                  <Card>
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2 text-base">
                        <Brain className="h-4 w-4" />
                        Memory Candidates
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-2">
                      {memoryCandidates.length === 0 ? (
                        <p className="text-sm text-text-muted">No memory candidates recorded.</p>
                      ) : (
                        memoryCandidates.map((candidate) => (
                          <div
                            key={candidate.candidate_id}
                            className="rounded-2xl border border-border bg-surface-elevated/40 p-3"
                          >
                            <div className="flex items-center justify-between gap-3">
                              <p className="text-sm font-medium text-text">{candidate.kind}</p>
                              <span className="text-xs text-text-muted">
                                {formatTimestamp(candidate.created_at_ms)}
                              </span>
                            </div>
                            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-words text-[11px] text-text-muted">
                              {renderValue(candidate.summary ?? candidate.payload)}
                            </pre>
                          </div>
                        ))
                      )}
                    </CardContent>
                  </Card>
                </div>
              ) : null}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
