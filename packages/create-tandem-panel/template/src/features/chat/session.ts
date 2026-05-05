export const CHAT_SESSION_KEY = "tcp.chat.session";

export type ChatSession = {
  id: string;
  title: string;
  raw?: any;
};

export function sessionIdOf(session: any): string {
  return String(session?.id || session?.session_id || session?.sessionID || "").trim();
}

export function sessionTitleOf(session: any): string {
  const title = String(session?.title || session?.name || "").trim();
  if (title) return title;
  const id = sessionIdOf(session);
  if (!id) return "Session";
  return `Session ${id.slice(0, 8)}`;
}

function toRows(input: any): any[] {
  if (Array.isArray(input)) return input;
  if (Array.isArray(input?.sessions)) return input.sessions;
  return [];
}

function isInternalProviderTestSession(session: any): boolean {
  return String(session?.title || "")
    .trim()
    .toLowerCase()
    .startsWith("__provider_test__");
}

function isChatSession(session: any): boolean {
  const sourceKind = String(session?.sourceKind || session?.source_kind || "")
    .trim()
    .toLowerCase()
    .replace(/-/g, "_");
  return !sourceKind || sourceKind === "chat";
}

export function normalizeSessions(input: any): ChatSession[] {
  return toRows(input)
    .filter((row) => !isInternalProviderTestSession(row))
    .filter(isChatSession)
    .map((row) => {
      const id = sessionIdOf(row);
      return {
        id,
        title: sessionTitleOf(row),
        raw: row,
      };
    })
    .filter((row) => !!row.id);
}

export async function loadSessions(
  client: any,
  api: (path: string, init?: RequestInit) => Promise<any>
): Promise<ChatSession[]> {
  try {
    const direct = await client.sessions.list({ pageSize: 50, source: "chat" });
    const normalized = normalizeSessions(direct);
    if (normalized.length > 0) return normalized;
  } catch {
    // fallback below
  }

  try {
    const raw = await api("/api/engine/session?page_size=50&source=chat", { method: "GET" });
    return normalizeSessions(raw);
  } catch {
    return [];
  }
}

export function loadStoredSessionId(): string {
  try {
    return localStorage.getItem(CHAT_SESSION_KEY) || "";
  } catch {
    return "";
  }
}

export function saveStoredSessionId(sessionId: string) {
  try {
    if (sessionId) localStorage.setItem(CHAT_SESSION_KEY, sessionId);
    else localStorage.removeItem(CHAT_SESSION_KEY);
  } catch {
    // ignore
  }
}
