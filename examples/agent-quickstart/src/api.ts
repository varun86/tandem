import { TandemClient } from "@frumu/tandem-client";

export type JsonObject = Record<string, unknown>;

const PORTAL_WORKSPACE_ROOT_KEY = "tandem_aq_workspace_root";
export const PORTAL_AUTH_EXPIRED_EVENT = "tandem_portal_auth_expired";
let currentToken = "";

export const getWorkspaceRoot = (): string | null => {
  const raw = window.localStorage.getItem(PORTAL_WORKSPACE_ROOT_KEY);
  return raw?.trim() || null;
};

export const setWorkspaceRoot = (v: string | null) => {
  if (!v?.trim()) window.localStorage.removeItem(PORTAL_WORKSPACE_ROOT_KEY);
  else window.localStorage.setItem(PORTAL_WORKSPACE_ROOT_KEY, v.trim());
};

export const DEFAULT_PERMISSION_RULES: JsonObject[] = [
  { permission: "ls", pattern: "*", action: "allow" },
  { permission: "list", pattern: "*", action: "allow" },
  { permission: "glob", pattern: "*", action: "allow" },
  { permission: "search", pattern: "*", action: "allow" },
  { permission: "grep", pattern: "*", action: "allow" },
  { permission: "read", pattern: "*", action: "allow" },
  { permission: "memory_store", pattern: "*", action: "allow" },
  { permission: "memory_search", pattern: "*", action: "allow" },
  { permission: "memory_list", pattern: "*", action: "allow" },
  { permission: "websearch", pattern: "*", action: "allow" },
  { permission: "webfetch", pattern: "*", action: "allow" },
  { permission: "webfetch_html", pattern: "*", action: "allow" },
  { permission: "bash", pattern: "*", action: "allow" },
  { permission: "todowrite", pattern: "*", action: "allow" },
  { permission: "todo_write", pattern: "*", action: "allow" },
];

/**
 * Live SDK instance (ES module live-binding).
 * It is configured to route calls through the Vite/Express proxy at `/engine`.
 */
const createClient = (token: string) =>
  new TandemClient({
    baseUrl: "/engine",
    token,
  });

export let client = createClient("");

export const setClientToken = (token: string) => {
  currentToken = token;
  client = createClient(token);
};

export const clearClientToken = () => {
  currentToken = "";
  client = createClient("");
};

export const verifyToken = async (token: string): Promise<boolean> => {
  const probe = createClient(token);
  try {
    await probe.health();
    return true;
  } catch {
    return false;
  }
};

const engineRequest = async <T>(path: string, init: RequestInit = {}): Promise<T> => {
  const headers = new Headers(init.headers || {});
  if (currentToken) headers.set("Authorization", `Bearer ${currentToken}`);
  if (init.body && !headers.has("Content-Type")) headers.set("Content-Type", "application/json");

  const response = await fetch(`/engine${path}`, {
    ...init,
    headers,
  });

  if (response.status === 401) {
    window.dispatchEvent(new Event(PORTAL_AUTH_EXPIRED_EVENT));
    throw new Error("Unauthorized");
  }

  if (!response.ok) {
    const raw = await response.text();
    throw new Error(raw || `Request failed: ${response.status}`);
  }

  return (await response.json()) as T;
};

export interface McpServerRecord {
  name: string;
  transport: string;
  enabled: boolean;
  connected: boolean;
  last_error?: string;
  headers?: Record<string, string>;
}

export const listMcpServers = async (): Promise<Record<string, McpServerRecord>> =>
  engineRequest<Record<string, McpServerRecord>>("/mcp");

export const addMcpServer = async (payload: {
  name: string;
  transport: string;
  headers?: Record<string, string>;
  enabled?: boolean;
}) => engineRequest<{ ok: boolean }>("/mcp", { method: "POST", body: JSON.stringify(payload) });

export const connectMcpServer = async (name: string) =>
  engineRequest<{ ok: boolean }>(`/mcp/${encodeURIComponent(name)}/connect`, { method: "POST" });

export const disconnectMcpServer = async (name: string) =>
  engineRequest<{ ok: boolean }>(`/mcp/${encodeURIComponent(name)}/disconnect`, { method: "POST" });

export const refreshMcpServer = async (name: string) =>
  engineRequest<{ ok: boolean; count?: number; error?: string }>(
    `/mcp/${encodeURIComponent(name)}/refresh`,
    { method: "POST" }
  );

export const setMcpServerEnabled = async (name: string, enabled: boolean) =>
  engineRequest<{ ok: boolean }>(`/mcp/${encodeURIComponent(name)}`, {
    method: "PATCH",
    body: JSON.stringify({ enabled }),
  });

export const listMcpTools = async (): Promise<unknown[]> => engineRequest<unknown[]>("/mcp/tools");

export const deleteMcpServer = async (name: string) =>
  engineRequest<{ ok: boolean }>(`/mcp/${encodeURIComponent(name)}`, { method: "DELETE" });

export const promptAsyncWithModel = async (
  sessionId: string,
  prompt: string,
  model: { provider: string; model: string }
): Promise<{ runId: string }> => {
  const payload = {
    parts: [{ type: "text", text: prompt }],
    model: {
      providerID: model.provider,
      modelID: model.model,
    },
  };
  const res = await engineRequest<Record<string, unknown>>(
    `/session/${encodeURIComponent(sessionId)}/prompt_async?return=run`,
    { method: "POST", body: JSON.stringify(payload) }
  );
  const id = [res.runID, res.runId, res.run_id].find(
    (v): v is string => typeof v === "string" && v.trim().length > 0
  );
  if (!id) throw new Error("Run ID missing from prompt_async response");
  return { runId: id };
};

export const asEpochMs = (v: unknown): number => {
  if (typeof v !== "number" || !Number.isFinite(v)) return Date.now();
  return v < 1_000_000_000_000 ? Math.trunc(v * 1000) : Math.trunc(v);
};

export interface SharedResourceRecord<T = unknown> {
  key: string;
  value: T;
  rev?: string;
  updated_at_ms?: number;
  updated_by?: string;
}

export interface SwarmTaskRecord {
  taskId: string;
  title: string;
  ownerRole: string;
  status: string;
  statusReason?: string;
  sessionId: string;
  runId: string;
  worktreePath: string;
  branch: string;
  prUrl?: string;
  prNumber?: number;
  checksStatus?: string;
  lastUpdateMs: number;
  blockedBy?: "approval" | "auth" | "error";
  notifyOnComplete: true;
}

export interface SwarmRegistry {
  version: number;
  updatedAtMs: number;
  tasks: Record<string, SwarmTaskRecord>;
}

export const getSharedResource = async <T = unknown>(
  key: string
): Promise<SharedResourceRecord<T> | null> => {
  try {
    return await engineRequest<SharedResourceRecord<T>>(`/resource/${encodeURIComponent(key)}`);
  } catch {
    return null;
  }
};

export const putSharedResource = async <T = unknown>(
  key: string,
  value: T,
  updatedBy = "agent-quickstart.swarm-ui"
): Promise<SharedResourceRecord<T>> =>
  engineRequest<SharedResourceRecord<T>>(`/resource/${encodeURIComponent(key)}`, {
    method: "PUT",
    body: JSON.stringify({
      value,
      updated_by: updatedBy,
    }),
  });

export const listRoutinesRaw = async (): Promise<unknown[]> =>
  engineRequest<unknown[]>("/routines");

export const upsertRoutineRaw = async (payload: Record<string, unknown>) =>
  engineRequest<Record<string, unknown>>("/routines", {
    method: "POST",
    body: JSON.stringify(payload),
  });

export const runRoutineNowRaw = async (routineId: string) =>
  engineRequest<Record<string, unknown>>(`/routines/${encodeURIComponent(routineId)}/run_now`, {
    method: "POST",
    body: JSON.stringify({}),
  });

export const launchSwarmManager = async (
  objective: string,
  workspaceRoot: string
): Promise<{ ok: boolean; stdout?: string; stderr?: string; sessionId: string }> => {
  const session = await engineRequest<{ id: string }>("/session", {
    method: "POST",
    body: JSON.stringify({
      title: "Swarm UI Launcher",
      directory: workspaceRoot,
      workspace_root: workspaceRoot,
    }),
  });

  const command = await engineRequest<{ ok: boolean; stdout?: string; stderr?: string }>(
    `/session/${encodeURIComponent(session.id)}/command`,
    {
      method: "POST",
      body: JSON.stringify({
        command: "node",
        args: ["examples/agent-swarm/src/manager.mjs", objective],
        cwd: workspaceRoot,
      }),
    }
  );

  return { ...command, sessionId: session.id };
};
