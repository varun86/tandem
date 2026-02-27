/* ─── Slim EngineAPI for agent-quickstart ─── */

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

const asStr = (v: unknown): string | null =>
    typeof v === "string" && v.trim().length > 0 ? v : null;

const asEpochMs = (v: unknown): number => {
    if (typeof v !== "number" || !Number.isFinite(v)) return Date.now();
    return v < 1_000_000_000_000 ? Math.trunc(v * 1000) : Math.trunc(v);
};

const DEFAULT_PERMISSION_RULES: JsonObject[] = [
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

const parseRunId = (payload: JsonObject): string => {
    const direct =
        asStr(payload.id) || asStr(payload.runID) || asStr(payload.runId) || asStr(payload.run_id);
    if (direct) return direct;
    const nested = (payload.run || null) as JsonObject | null;
    if (nested) {
        const n =
            asStr(nested.id) || asStr(nested.runID) || asStr(nested.runId) || asStr(nested.run_id);
        if (n) return n;
    }
    throw new Error("Run ID missing in engine response");
};

/* ── Public types ── */
export interface SessionRecord {
    id: string;
    title?: string;
    directory?: string;
    workspaceRoot?: string;
    workspace_root?: string;
    created_at_ms?: number;
    [k: string]: unknown;
}

export interface SessionListResponse {
    sessions: SessionRecord[];
    count: number;
}

export interface EngineMessage {
    info?: { role?: string };
    parts?: Array<{ type?: string; text?: string }>;
    [k: string]: unknown;
}

export interface ActiveRun {
    runID?: string;
    runId?: string;
    run_id?: string;
    [k: string]: unknown;
}

export interface ProviderEntry {
    id: string;
    name?: string;
    models?: Record<string, { name?: string }>;
}

export interface ProviderCatalog {
    all?: ProviderEntry[];
    connected?: string[];
    default?: string;
}

export interface ChannelsStatusResponse {
    telegram?: { connected?: boolean; sessions?: number; error?: string };
    discord?: { connected?: boolean; sessions?: number; error?: string };
    slack?: { connected?: boolean; sessions?: number; error?: string };
    [k: string]: unknown;
}

export interface ChannelsConfigResponse {
    telegram?: JsonObject;
    discord?: JsonObject;
    slack?: JsonObject;
    [k: string]: unknown;
}

export interface RoutineRecord {
    id: string;
    name?: string;
    title?: string;
    schedule?: JsonObject;
    last_run?: string;
    last_run_at?: string;
    status?: string;
    [k: string]: unknown;
}

export interface PermissionRequest {
    id: string;
    tool?: string;
    permission?: string;
    status?: string;
    sessionID?: string;
    sessionId?: string;
    session_id?: string;
}

export class EngineAPI {
    private baseUrl = "/engine";
    private token: string | null;
    private readonly timeoutMs = 20_000;
    private modelCache: { providerID: string; modelID: string } | null = null;
    private modelCacheAt = 0;

    constructor(token: string | null = null) {
        this.token = token;
    }

    setToken(t: string) {
        this.token = t;
    }
    getToken() {
        return this.token;
    }
    get isConfigured() {
        return !!this.token;
    }

    private get headers() {
        return {
            "Content-Type": "application/json",
            ...(this.token ? { Authorization: `Bearer ${this.token}` } : {}),
        };
    }

    private async req<T>(path: string, init: RequestInit = {}): Promise<T> {
        const ctrl = new AbortController();
        const tid = window.setTimeout(() => ctrl.abort(), this.timeoutMs);
        let res: Response;
        try {
            res = await fetch(`${this.baseUrl}${path}`, {
                ...init,
                headers: { ...this.headers, ...(init.headers || {}) },
                signal: ctrl.signal,
            });
        } catch (e) {
            if (e instanceof DOMException && e.name === "AbortError")
                throw new Error(`Request timed out: ${path}`);
            throw e;
        } finally {
            window.clearTimeout(tid);
        }

        if (!res.ok) {
            if (res.status === 401) {
                window.dispatchEvent(new CustomEvent(PORTAL_AUTH_EXPIRED_EVENT));
                throw new Error("Session expired. Please sign in again.");
            }
            const body = await res.text().catch(() => "");
            throw new Error(`HTTP ${res.status}: ${body}`);
        }
        if (res.status === 204) return undefined as T;
        return (await res.json()) as T;
    }

    /* ── URLs ── */
    getEventStreamUrl(sessionId: string): string {
        const p = new URLSearchParams({ sessionID: sessionId, token: this.token || "" });
        return `${this.baseUrl}/event?${p}`;
    }
    getGlobalEventStreamUrl(): string {
        return `${this.baseUrl}/global/event?token=${encodeURIComponent(this.token || "")}`;
    }

    /* ── Provider / model resolution ── */
    async resolveModelSpec(): Promise<{ providerID: string; modelID: string } | null> {
        if (this.modelCache && Date.now() - this.modelCacheAt < 30_000) return this.modelCache;
        try {
            const [cfg, catalog] = await Promise.all([
                this.req<JsonObject>("/config/providers"),
                this.req<ProviderCatalog>("/provider"),
            ]);
            const connected = new Set((catalog.connected || []).filter(Boolean));
            const defaultProv = asStr(cfg["default"] as unknown) || asStr(catalog.default as unknown);
            const provs = (cfg["providers"] as Record<string, JsonObject> | undefined) || {};

            const pick = (prov: string): string | null => asStr(provs[prov]?.["default_model"] as unknown);
            if (defaultProv && connected.has(defaultProv) && pick(defaultProv)) {
                this.modelCache = { providerID: defaultProv, modelID: pick(defaultProv)! };
                this.modelCacheAt = Date.now();
                return this.modelCache;
            }
            for (const prov of connected) {
                const m = pick(prov);
                if (m) {
                    this.modelCache = { providerID: prov, modelID: m };
                    this.modelCacheAt = Date.now();
                    return this.modelCache;
                }
            }
        } catch { /* ignore */ }
        return null;
    }

    clearModelCache() {
        this.modelCache = null;
        this.modelCacheAt = 0;
    }

    /* ── Sessions ── */
    async createSession(title = "Agent Chat"): Promise<string> {
        const model = await this.resolveModelSpec();
        const workspace = getWorkspaceRoot();
        const payload: JsonObject = {
            title,
            directory: workspace || ".",
            permission: DEFAULT_PERMISSION_RULES,
        };
        if (workspace) payload.workspace_root = workspace;
        if (model) { payload.model = model; payload.provider = model.providerID; }
        const data = await this.req<{ id: string }>("/session", {
            method: "POST",
            body: JSON.stringify(payload),
        });
        return data.id;
    }

    async listSessions(opts?: { page?: number; pageSize?: number }): Promise<SessionListResponse> {
        const p = new URLSearchParams();
        if (opts?.page) p.set("page", String(opts.page));
        if (opts?.pageSize) p.set("page_size", String(opts.pageSize));
        const qs = p.toString() ? `?${p}` : "";
        const raw = await this.req<unknown>(`/session${qs}`);
        if (Array.isArray(raw)) {
            const sessions = (raw as JsonObject[]).map((o) => ({
                ...(o as SessionRecord),
                created_at_ms: asEpochMs((o as { created_at_ms?: unknown }).created_at_ms ?? ((o.time as JsonObject)?.created)),
            }));
            return { sessions, count: sessions.length };
        }
        const w = (raw ?? {}) as JsonObject;
        const sessions = ((Array.isArray(w.sessions) ? w.sessions : []) as JsonObject[]).map((o) => ({
            ...(o as SessionRecord),
            created_at_ms: asEpochMs((o as { created_at_ms?: unknown }).created_at_ms ?? ((o.time as JsonObject)?.created)),
        }));
        return { sessions, count: typeof w.count === "number" ? w.count : sessions.length };
    }

    async getSession(id: string): Promise<SessionRecord> {
        return this.req<SessionRecord>(`/session/${encodeURIComponent(id)}`);
    }

    async getSessionMessages(id: string): Promise<EngineMessage[]> {
        return this.req<EngineMessage[]>(`/session/${encodeURIComponent(id)}/message`);
    }

    async deleteSession(id: string): Promise<void> {
        await this.req<void>(`/session/${encodeURIComponent(id)}`, { method: "DELETE" });
    }

    async startAsyncRun(sessionId: string, text?: string): Promise<{ runId: string }> {
        const model = await this.resolveModelSpec();
        const payload: JsonObject = text ? { parts: [{ type: "text", text }] } : {};
        if (model) payload.model = model;

        const ctrl = new AbortController();
        const tid = window.setTimeout(() => ctrl.abort(), this.timeoutMs);
        let res: Response;
        try {
            res = await fetch(`${this.baseUrl}/session/${encodeURIComponent(sessionId)}/prompt_async?return=run`, {
                method: "POST",
                headers: this.headers,
                body: JSON.stringify(payload),
                signal: ctrl.signal,
            });
        } finally {
            window.clearTimeout(tid);
        }

        // Handle conflict (another run active) — attach to it
        if (res.status === 409) {
            const conflict = ((await res.json().catch(() => ({}))) || {}) as JsonObject;
            const code = asStr(conflict.code);
            const activeRun = (conflict.activeRun || {}) as JsonObject;
            const runId = asStr(activeRun.runID) || asStr(activeRun.runId) || asStr(activeRun.run_id);
            if (code === "SESSION_RUN_CONFLICT" && runId) return { runId };
        }

        if (!res.ok) {
            const body = await res.text().catch(() => "");
            throw new Error(`HTTP ${res.status}: ${body}`);
        }

        const data = ((await res.json().catch(() => ({}))) || {}) as JsonObject;
        return { runId: parseRunId(data) };
    }

    async getActiveRun(sessionId: string): Promise<{ active: ActiveRun | null }> {
        return this.req(`/session/${encodeURIComponent(sessionId)}/run`);
    }

    /* ── Permissions ── */
    async listPermissions(): Promise<{ requests: PermissionRequest[]; rules: unknown[] }> {
        return this.req("/permission");
    }
    async replyPermission(id: string, reply: "allow" | "always" | "deny" | "once"): Promise<{ ok: boolean }> {
        return this.req(`/permission/${encodeURIComponent(id)}/reply`, {
            method: "POST",
            body: JSON.stringify({ reply }),
        });
    }

    /* ── Channels ── */
    async getChannelsStatus(): Promise<ChannelsStatusResponse> {
        return this.req("/channels/status");
    }
    async getChannelsConfig(): Promise<ChannelsConfigResponse> {
        return this.req("/channels/config");
    }
    async putChannel(ch: "telegram" | "discord" | "slack", payload: JsonObject): Promise<{ ok: boolean }> {
        return this.req(`/channels/${ch}`, { method: "PUT", body: JSON.stringify(payload) });
    }
    async deleteChannel(ch: "telegram" | "discord" | "slack"): Promise<{ ok: boolean }> {
        return this.req(`/channels/${ch}`, { method: "DELETE" });
    }

    /* ── Routines / Automations ── */
    async listRoutines(): Promise<{ items?: RoutineRecord[]; definitions?: RoutineRecord[] }> {
        return this.req("/routines");
    }
    async listAutomations(): Promise<{ items?: RoutineRecord[]; definitions?: RoutineRecord[] }> {
        return this.req("/automations");
    }
    async createRoutine(payload: JsonObject): Promise<{ id: string; ok?: boolean }> {
        return this.req("/routines", { method: "POST", body: JSON.stringify(payload) });
    }
    async deleteRoutine(id: string): Promise<{ ok: boolean }> {
        return this.req(`/routines/${encodeURIComponent(id)}`, { method: "DELETE" });
    }
    async runNow(id: string): Promise<{ ok?: boolean; run_id?: string }> {
        return this.req(`/routines/${encodeURIComponent(id)}/run`, { method: "POST" });
    }

    /* ── Provider config ── */
    async getProviderCatalog(): Promise<ProviderCatalog> {
        return this.req("/provider");
    }
    async setProviderAuth(providerId: string, apiKey: string): Promise<void> {
        await this.req(`/auth/${encodeURIComponent(providerId)}`, {
            method: "PUT",
            body: JSON.stringify({ apiKey }),
        });
    }
    async setProviderDefaults(providerId: string, modelId: string): Promise<void> {
        await this.req("/config", {
            method: "PATCH",
            body: JSON.stringify({
                default_provider: providerId,
                providers: { [providerId]: { default_model: modelId } },
            }),
        });
        this.clearModelCache();
    }

    /* ── Health ── */
    async getHealth(): Promise<{ status?: string }> {
        return this.req("/global/health");
    }

    /* ── Tools ── */
    async listToolIds(): Promise<string[]> {
        return this.req("/tool/ids");
    }

    /* ── MCP ── */
    async listMcpServers(): Promise<Record<string, unknown>> {
        return this.req("/mcp");
    }
    async connectMcpServer(name: string): Promise<{ ok: boolean }> {
        return this.req(`/mcp/${encodeURIComponent(name)}/connect`, { method: "POST" });
    }
}

export const api = new EngineAPI(null);
