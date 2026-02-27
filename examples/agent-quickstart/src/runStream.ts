import { api } from "./api";

export interface RunStreamHandlers {
    onLog: (msg: string) => void;
    onTextDelta: (delta: string) => void;
    onToolStart: (tool: string) => void;
    onToolEnd: (tool: string, result: string) => void;
    onFinalize: (status: "completed" | "cancelled" | "error" | "timeout" | "stream_error" | "inactive" | "inactive_no_events") => void;
}

export function attachRunStream(
    esRef: { current: EventSource | null },
    sessionId: string,
    _runId: string,
    handlers: RunStreamHandlers,
    opts?: { runTimeoutMs?: number }
): void {
    if (esRef.current) { esRef.current.close(); esRef.current = null; }

    let finalized = false;
    let sawEvent = false;
    let streamedText = "";
    const textSnapshots = new Map<string, string>();
    const runTimeoutMs = opts?.runTimeoutMs ?? 180_000;
    let source: EventSource | null = null;
    let reconnectAttempts = 0;
    const maxReconnects = 8;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let runTimeout: ReturnType<typeof setTimeout> | null = null;

    const finalize = (status: RunStreamHandlers["onFinalize"] extends (s: infer S) => void ? S : never) => {
        if (finalized) return;
        finalized = true;
        if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
        if (runTimeout) { clearTimeout(runTimeout); runTimeout = null; }
        clearInterval(pollInterval);
        void (async () => {
            // Best-effort: hydrate final assistant text from history if streaming missed chars.
            try {
                const msgs = await api.getSessionMessages(sessionId);
                let latest = "";
                for (let i = msgs.length - 1; i >= 0; i--) {
                    const m = msgs[i];
                    if (m.info?.role !== "assistant") continue;
                    const t = (m.parts || []).filter(p => p.type === "text" && p.text).map(p => p.text).join("\n").trim();
                    if (t) { latest = t; break; }
                }
                if (latest && !streamedText) {
                    handlers.onTextDelta(latest);
                } else if (latest && latest.startsWith(streamedText) && latest.length > streamedText.length) {
                    handlers.onTextDelta(latest.slice(streamedText.length));
                }
            } catch { /* ignore */ }
            handlers.onFinalize(status);
            source?.close();
            if (esRef.current === source) esRef.current = null;
        })();
    };

    const resetRunTimeout = () => {
        if (runTimeout) clearTimeout(runTimeout);
        runTimeout = setTimeout(async () => {
            if (finalized) return;
            try {
                const s = await api.getActiveRun(sessionId);
                if (s.active) { handlers.onLog(`Run still active past ${runTimeoutMs}ms — extending.`); resetRunTimeout(); return; }
            } catch { /* ignore */ }
            finalize("timeout");
        }, runTimeoutMs);
    };

    const scheduleReconnect = async (reason: string) => {
        if (finalized) return;
        reconnectAttempts++;
        try {
            const s = await api.getActiveRun(sessionId);
            if (!s.active) { finalize("inactive"); return; }
        } catch { /* ignore */ }
        if (reconnectAttempts > maxReconnects) { finalize("stream_error"); return; }
        const delay = Math.min(8000, 1200 * reconnectAttempts);
        handlers.onLog(`Stream disconnected (${reason}). Reconnecting in ${Math.round(delay / 100) / 10}s…`);
        reconnectTimer = setTimeout(() => { reconnectTimer = null; if (!finalized) openStream(); }, delay);
    };

    // Watchdog: if no events arrive in 4s, check if run is already done
    const watchdog = setTimeout(async () => {
        if (finalized || sawEvent) return;
        try {
            const s = await api.getActiveRun(sessionId);
            if (!s.active) finalize("inactive_no_events");
            else handlers.onLog("Run active but no events yet — waiting for provider output…");
        } catch { finalize("inactive_no_events"); }
    }, 4000);

    const pollInterval = setInterval(async () => {
        if (finalized) return;
        try {
            const s = await api.getActiveRun(sessionId);
            if (!s.active) { handlers.onLog("Run became inactive (poll). Finalizing."); finalize("inactive"); }
        } catch { /* ignore */ }
    }, 5000);

    resetRunTimeout();

    const handleMsg = (e: MessageEvent<string>) => {
        try {
            resetRunTimeout();
            const data = JSON.parse(e.data) as { type?: string; properties?: Record<string, unknown> };
            if (data.type && data.type !== "server.connected" && data.type !== "engine.lifecycle.ready") sawEvent = true;

            // Terminal events
            if (data.type === "session.run.finished") {
                const status = (data.properties?.status as string) || "completed";
                handlers.onLog(`Run finished: ${status}`);
                finalize(["completed", "cancelled", "error", "timeout"].includes(status) ? status as "completed" | "cancelled" | "error" | "timeout" : "completed");
                return;
            }

            // Permissions
            if (data.type === "permission.asked") {
                const tool = String(data.properties?.tool || data.properties?.permission || "tool");
                handlers.onLog(`Permission requested for ${tool}.`);
                return;
            }
            if (data.type === "permission.replied") {
                const reply = String(data.properties?.reply || "?");
                handlers.onLog(`Permission ${reply}.`);
                return;
            }

            if (data.type !== "message.part.updated") return;
            const part = data.properties?.part as Record<string, unknown> | undefined;
            if (!part) return;

            // Tool events
            if (part.type === "tool" || part.type === "tool-invocation" || part.type === "tool-result") {
                const rawState = part.state;
                const status = typeof rawState === "string" ? rawState : (rawState as Record<string, string>)?.status;
                if (status === "running" || status === "in_progress" || status === "pending") {
                    handlers.onToolStart(String(part.tool || "tool"));
                    return;
                }
                if (["completed", "failed", "error", "cancelled", "denied"].includes(status || "")) {
                    const rawResult = part.result ?? part.error ?? (rawState as Record<string, unknown>)?.result ?? "";
                    handlers.onToolEnd(String(part.tool || "tool"), typeof rawResult === "string" ? rawResult : JSON.stringify(rawResult || {}));
                }
                return;
            }

            // Text delta
            if (part.type === "text") {
                const delta = data.properties?.delta;
                if (typeof delta === "string" && delta.length > 0) {
                    streamedText += delta;
                    handlers.onTextDelta(delta);
                    return;
                }
                const fullText = typeof part.text === "string" ? part.text : "";
                if (fullText) {
                    const key = String(part.id || part.partID || "text-part");
                    const prev = textSnapshots.get(key) || "";
                    if (fullText.startsWith(prev) && fullText.length > prev.length) {
                        const inferred = fullText.slice(prev.length);
                        streamedText += inferred;
                        handlers.onTextDelta(inferred);
                    } else if (!prev && !streamedText) {
                        streamedText += fullText;
                        handlers.onTextDelta(fullText);
                    }
                    textSnapshots.set(key, fullText);
                }
            }
        } catch {
            handlers.onLog("Failed to parse stream event.");
        }
    };

    const openStream = () => {
        if (finalized) return;
        source?.close();
        const es = new EventSource(api.getEventStreamUrl(sessionId));
        source = es;
        esRef.current = es;
        es.onopen = () => { reconnectAttempts = 0; resetRunTimeout(); };
        es.onmessage = handleMsg;
        es.onerror = () => {
            es.close();
            if (source === es) source = null;
            if (esRef.current === es) esRef.current = null;
            void scheduleReconnect("sse_error");
        };
    };

    openStream();
    void watchdog; // suppress unused warning — it's already scheduled
}
