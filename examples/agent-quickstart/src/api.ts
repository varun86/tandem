import { TandemClient } from "@frumu/tandem-client";

export type JsonObject = Record<string, unknown>;

const PORTAL_WORKSPACE_ROOT_KEY = "tandem_aq_workspace_root";
export const PORTAL_AUTH_EXPIRED_EVENT = "tandem_portal_auth_expired";

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
 * Singleton instance of the official TandemClient SDK.
 * It is configured to route calls through the Vite/Express proxy at `/engine`.
 */
export const client = new TandemClient({
  baseUrl: "/engine",
  token: null, // Will be injected by the AuthContext later
});

export const asEpochMs = (v: unknown): number => {
  if (typeof v !== "number" || !Number.isFinite(v)) return Date.now();
  return v < 1_000_000_000_000 ? Math.trunc(v * 1000) : Math.trunc(v);
};
