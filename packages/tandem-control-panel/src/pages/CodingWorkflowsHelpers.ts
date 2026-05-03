import type { PlannerProviderOption } from "../features/planner/plannerShared";
export type CodingTab = "overview" | "board" | "planning" | "manual" | "integrations";
export type TaskSourceType = "manual" | "kanban_board" | "github_project" | "local_backlog";

export type GithubRepoRef = {
  owner: string;
  repo: string;
  slug: string;
};

export function toArray(input: any, key: string) {
  if (Array.isArray(input)) return input;
  if (Array.isArray(input?.[key])) return input[key];
  return [];
}

export function normalizeServers(raw: any) {
  const rows = Array.isArray(raw)
    ? raw
    : Array.isArray(raw?.servers)
      ? raw.servers
      : raw && typeof raw === "object"
        ? Object.entries(raw).map(([name, row]) => ({ name, ...(row as any) }))
        : [];
  return rows
    .map((row: any) => ({
      name: String(row?.name || "").trim(),
      connected: !!row?.connected,
      enabled: row?.enabled !== false,
      transport: String(row?.transport || "").trim(),
      lastError: String(row?.last_error || row?.lastError || "").trim(),
    }))
    .filter((row: any) => row.name)
    .sort((a: any, b: any) => a.name.localeCompare(b.name));
}

export function normalizeTools(raw: any) {
  const rows = Array.isArray(raw) ? raw : Array.isArray(raw?.tools) ? raw.tools : [];
  return rows
    .map((tool: any) => {
      if (typeof tool === "string") return tool;
      return String(tool?.namespaced_name || tool?.namespacedName || tool?.id || "").trim();
    })
    .filter(Boolean);
}

export function normalizeProjects(raw: any) {
  const rows = Array.isArray(raw)
    ? raw
    : raw && typeof raw === "object"
      ? Object.entries(raw).map(([slug, record]) => ({
          slug,
          ...(record as any),
        }))
      : [];
  const bySignature = new Map<string, any>();
  for (const row of rows) {
    const taskSource = row?.task_source || row?.taskSource || {};
    const taskType = String(taskSource?.type || "").trim();
    const repo = row?.repo || {};
    const repoUrl = String(
      row?.repo_url || row?.repoUrl || repo?.clone_url || repo?.cloneUrl || ""
    ).trim();
    const signature =
      taskType === "github_project" &&
      String(taskSource?.owner || "").trim() &&
      String(taskSource?.repo || "").trim() &&
      String(taskSource?.project || "").trim()
        ? `github:${String(taskSource.owner).trim().toLowerCase()}/${String(taskSource.repo)
            .trim()
            .toLowerCase()}#${String(taskSource.project).trim()}`
        : repoUrl
          ? `repo:${repoUrl.toLowerCase()}`
          : `slug:${row.slug}`;
    const current = bySignature.get(signature);
    if (!current) {
      bySignature.set(signature, row);
      continue;
    }
    const currentRepo = current?.repo || {};
    const currentRepoUrl = String(
      current?.repo_url || current?.repoUrl || currentRepo?.clone_url || currentRepo?.cloneUrl || ""
    ).trim();
    const currentScore = (current.implicit ? 0 : 10) + (currentRepoUrl ? 1 : 0);
    const nextScore = (row.implicit ? 0 : 10) + (repoUrl ? 1 : 0);
    if (nextScore > currentScore) {
      bySignature.set(signature, row);
    }
  }

  return Array.from(bySignature.values())
    .map((row: any) => {
      const repo = row?.repo || {};
      const taskSource = row?.task_source || row?.taskSource || {};
      return {
        slug: String(row?.slug || "").trim(),
        name: String(row?.name || row?.display_name || row?.displayName || row?.slug || "").trim(),
        repoUrl: String(
          row?.repo_url || row?.repoUrl || repo?.clone_url || repo?.cloneUrl || ""
        ).trim(),
        repoPath: String(repo?.path || row?.repo_path || row?.repoPath || "").trim(),
        worktreeRoot: String(
          repo?.worktree_root || row?.worktree_root || row?.worktreeRoot || ""
        ).trim(),
        defaultBranch: String(
          repo?.default_branch || row?.default_branch || row?.defaultBranch || "main"
        ).trim(),
        remoteName: String(
          repo?.remote_name || row?.remote_name || row?.remoteName || "origin"
        ).trim(),
        credentialFile: String(
          repo?.credential_file || row?.credential_file || row?.credentialFile || ""
        ).trim(),
        taskSource,
        implicit: row?.implicit === true,
      };
    })
    .filter((row: any) => row.slug)
    .sort((a: any, b: any) => {
      if (a.implicit !== b.implicit) return a.implicit ? 1 : -1;
      return a.slug.localeCompare(b.slug);
    });
}

export function normalizeGithubBoard(raw: any) {
  const items = Array.isArray(raw?.items)
    ? raw.items
        .map((item: any, index: number) => ({
          id: String(item?.id || item?.project_item_id || `item-${index}`).trim(),
          projectItemId: String(item?.project_item_id || item?.projectItemId || "").trim(),
          title: String(item?.title || "Untitled item").trim(),
          statusKey: String(item?.status_key || item?.statusKey || "unknown").trim(),
          statusName: String(item?.status_name || item?.statusName || "Unknown").trim(),
          issueNumber: item?.issue_number || item?.issueNumber || null,
          issueUrl: String(item?.issue_url || item?.issueUrl || "").trim(),
          repoName: String(item?.repo_name || item?.repoName || "").trim(),
          actionable: item?.actionable === true,
          selector: String(
            item?.project_item_id ||
              item?.projectItemId ||
              item?.issue_number ||
              item?.issueNumber ||
              item?.id ||
              ""
          ).trim(),
        }))
        .filter((item: any) => item.id)
    : [];
  return {
    items,
    source: String(raw?.source || "").trim(),
    warning: String(raw?.warning || "").trim(),
    isStale: raw?.is_stale === true,
    lastSyncedAtMs: Number(raw?.last_synced_at_ms || raw?.lastSyncedAtMs || 0),
  };
}

export function runId(run: any, index: number) {
  return String(run?.run_id || run?.runId || run?.id || `run-${index}`).trim();
}

export function runTitle(run: any) {
  return String(run?.title || run?.summary || run?.run_id || run?.runId || "Untitled run").trim();
}

export function runUpdatedAt(run: any) {
  const value = Number(
    run?.updated_at_ms ||
      run?.created_at_ms ||
      run?.snapshot?.updated_at_ms ||
      run?.snapshot?.created_at_ms ||
      0
  );
  return Number.isFinite(value) ? value : 0;
}

export function runStatus(run: any) {
  return String(run?.status || run?.snapshot?.status || run?.status?.run?.status || "unknown")
    .trim()
    .toLowerCase();
}

export function runPhase(run: any) {
  return String(run?.phase?.name || run?.snapshot?.phase?.name || run?.status?.phase?.name || "")
    .trim()
    .toLowerCase();
}

export function formatStatus(status: string) {
  return String(status || "unknown")
    .replace(/_/g, " ")
    .replace(/\b\w/g, (match) => match.toUpperCase());
}

export const ACTIVE_RUN_STALE_AFTER_MS = 30 * 60 * 1000;
export const GITHUB_ITEM_LAUNCH_LOCK_MS = 15 * 1000;

export function runHasLiveSession(run: any) {
  return run?.is_running === true || run?.snapshot?.is_running === true;
}

export function runIsActive(run: any) {
  const status = runStatus(run);
  if (["completed", "done", "failed", "cancelled", "blocked", "archived"].includes(status)) {
    return false;
  }
  if (runHasLiveSession(run)) {
    return true;
  }
  const updatedAt = runUpdatedAt(run);
  if (!updatedAt) {
    return false;
  }
  return Date.now() - updatedAt < ACTIVE_RUN_STALE_AFTER_MS;
}

export function runTaskIdentity(run: any, index: number) {
  const task = run?.blackboard?.task || run?.snapshot?.blackboard?.task || {};
  const source = task?.source || {};
  const repo = task?.repo || {};
  return String(
    source?.issue_url ||
      source?.url ||
      source?.item_url ||
      source?.project_item_id ||
      source?.card_id ||
      task?.task_id ||
      `${repo?.slug || run?.project_slug || "project"}:${runTitle(run)}:${index}`
  ).trim();
}

export function githubBoardItemIdentity(item: any) {
  return String(item?.issueUrl || item?.projectItemId || item?.id || "").trim();
}

export function githubBoardItemCanRun(item: any) {
  const statusKey = String(item?.statusKey || "")
    .trim()
    .toLowerCase();
  if (!item?.selector) return false;
  if (statusKey === "in_review" || statusKey === "done") return false;
  return item?.actionable === true || ["ready", "backlog", "todo"].includes(statusKey);
}

export function dedupeRuns(runs: any[]) {
  const latestByIdentity = new Map<string, any>();
  runs.forEach((run, index) => {
    const identity = runTaskIdentity(run, index);
    const existing = latestByIdentity.get(identity);
    if (!existing || runUpdatedAt(run) >= runUpdatedAt(existing)) {
      latestByIdentity.set(identity, run);
    }
  });
  return Array.from(latestByIdentity.values()).sort((a, b) => runUpdatedAt(b) - runUpdatedAt(a));
}

export function parseGithubRepoRef(raw: string): GithubRepoRef | null {
  const input = String(raw || "").trim();
  if (!input) return null;

  const cleanPath = (path: string) => {
    const parts = path
      .replace(/^\/+/, "")
      .replace(/\.git$/i, "")
      .split("/")
      .map((part) => part.trim())
      .filter(Boolean);
    if (parts.length < 2) return null;
    const [owner, repo] = parts;
    if (!owner || !repo) return null;
    return { owner, repo, slug: `${owner}/${repo}` };
  };

  const sshMatch = input.match(/^git@github\.com:([^/]+)\/(.+?)(?:\.git)?$/i);
  if (sshMatch?.[1] && sshMatch?.[2]) {
    return cleanPath(`${sshMatch[1]}/${sshMatch[2]}`);
  }

  try {
    const url = new URL(input);
    if (url.hostname.toLowerCase() !== "github.com") return null;
    return cleanPath(url.pathname);
  } catch {
    return cleanPath(input);
  }
}

export function buildTaskSourcePayload(
  taskSourceType: TaskSourceType,
  {
    prompt,
    path,
    repoRef,
    projectNumber,
  }: {
    prompt: string;
    path: string;
    repoRef: GithubRepoRef | null;
    projectNumber: string;
  }
) {
  if (taskSourceType === "manual") {
    return { type: "manual", prompt: prompt.trim() };
  }
  if (taskSourceType === "kanban_board") {
    return { type: "kanban_board", path: path.trim() };
  }
  if (taskSourceType === "local_backlog") {
    return { type: "local_backlog", path: path.trim() };
  }
  return {
    type: "github_project",
    owner: repoRef?.owner || "",
    repo: repoRef?.repo || "",
    project: projectNumber.trim(),
  };
}

export function isSafeManagedPath(raw: string) {
  const text = String(raw || "")
    .trim()
    .replace(/\\/g, "/");
  if (!text) return true;
  if (text.startsWith("/") || /^[A-Za-z]:/.test(text)) return false;
  const parts = text.split("/").filter(Boolean);
  return parts.length > 0 && !parts.some((part) => part === "." || part === "..");
}

export function parseSseEnvelope(data: string) {
  try {
    const parsed = JSON.parse(String(data || "{}"));
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}
