import { useState, useEffect, useRef, useCallback } from "react";
import { api } from "../api";
import { Radio, RefreshCw, Trash2, Filter } from "lucide-react";

interface LiveEvent {
    id: string;
    ts: number;
    type: string;
    rawType: string;
    payload: unknown;
    sessionId?: string;
}

const TYPE_COLOR: Record<string, string> = {
    "message.part.updated": "text-violet-400",
    "session.run.started": "text-emerald-400",
    "session.run.finished": "text-emerald-300",
    "permission.asked": "text-amber-400",
    "permission.replied": "text-amber-300",
    "tool.loop_guard.triggered": "text-rose-400",
    "server.connected": "text-gray-500",
};

function typeColor(t: string): string {
    return TYPE_COLOR[t] || (t.includes("error") ? "text-rose-400" : t.includes("tool") ? "text-sky-400" : "text-gray-300");
}

function EventRow({ ev }: { ev: LiveEvent }) {
    const [open, setOpen] = useState(false);
    const pay = ev.payload as Record<string, unknown> | null;
    const sid = ev.sessionId;
    const props = pay?.properties as Record<string, unknown> | undefined;
    const delta = props?.delta;
    const summary = (() => {
        if (ev.type === "message.part.updated" && typeof delta === "string") return delta.slice(0, 80).replace(/\n/g, " ");
        if (ev.type === "session.run.started") return `run ${String(props?.runID || "").slice(0, 8)}`;
        if (ev.type === "session.run.finished") return `run ${String(props?.runID || "").slice(0, 8)} → ${String(props?.status || "?")}`;
        if (ev.type === "permission.asked") return `${String(props?.tool || props?.permission || "?")}`;
        if (ev.type === "permission.replied") return `${String(props?.reply || "?")} → ${String(props?.tool || "?")}`;
        return "";
    })();

    return (
        <div
            className={`border-b border-gray-800/60 px-3 py-2 cursor-pointer hover:bg-gray-800/30 transition-colors ${open ? "bg-gray-800/20" : ""}`}
            onClick={() => setOpen((o) => !o)}
        >
            <div className="flex items-center gap-2 min-w-0">
                <span className="text-[10px] text-gray-600 font-mono shrink-0">
                    {new Date(ev.ts).toLocaleTimeString([], { hour12: false })}
                </span>
                <span className={`text-[11px] font-mono shrink-0 ${typeColor(ev.type)}`}>{ev.rawType}</span>
                {sid && <span className="text-[10px] text-gray-600 font-mono shrink-0">[{sid.slice(0, 8)}]</span>}
                {summary && <span className="text-[11px] text-gray-400 truncate">{summary}</span>}
            </div>
            {open && (
                <div className="mt-2 bg-gray-950 rounded-lg p-3 max-h-48 overflow-y-auto">
                    <pre className="text-[11px] text-gray-300 font-mono whitespace-pre-wrap break-all leading-5">
                        {JSON.stringify(ev.payload, null, 2)}
                    </pre>
                </div>
            )}
        </div>
    );
}

export default function LiveFeed() {
    const [events, setEvents] = useState<LiveEvent[]>([]);
    const [filter, setFilter] = useState("");
    const [paused, setPaused] = useState(false);
    const esRef = useRef<EventSource | null>(null);
    const pausedRef = useRef(false);
    const bottomRef = useRef<HTMLDivElement>(null);
    const [autoscroll, setAutoscroll] = useState(true);

    useEffect(() => { pausedRef.current = paused; }, [paused]);
    useEffect(() => {
        if (autoscroll && !paused) bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [events, autoscroll, paused]);

    const connect = useCallback(() => {
        esRef.current?.close();
        const url = api.getGlobalEventStreamUrl();
        const es = new EventSource(url);
        esRef.current = es;
        es.onmessage = (e: MessageEvent<string>) => {
            if (pausedRef.current) return;
            try {
                const data = JSON.parse(e.data) as Record<string, unknown>;
                const type = String(data.type || "unknown");
                if (type === "server.connected" || type === "engine.lifecycle.ready") return;
                const props = data.properties as Record<string, unknown> | undefined;
                const sid = String(
                    props?.sessionID || props?.sessionId || props?.session_id || data.sessionID || data.sessionId || ""
                ).trim() || undefined;
                setEvents((prev) => {
                    const next = [
                        ...prev,
                        { id: Math.random().toString(36), ts: Date.now(), type, rawType: type, payload: data, sessionId: sid },
                    ].slice(-300);
                    return next;
                });
            } catch { /* ignore */ }
        };
        es.onerror = () => {
            es.close();
            if (esRef.current === es) esRef.current = null;
            // Reconnect after 2s
            setTimeout(() => { if (!pausedRef.current) connect(); }, 2000);
        };
    }, []); // eslint-disable-line react-hooks/exhaustive-deps

    useEffect(() => {
        connect();
        return () => { esRef.current?.close(); esRef.current = null; };
    }, [connect]);

    const filtered = filter.trim()
        ? events.filter((e) => e.type.includes(filter.trim()) || e.sessionId?.includes(filter.trim()))
        : events;

    return (
        <div className="flex flex-col h-full bg-gray-950">
            {/* Header */}
            <div className="shrink-0 border-b border-gray-800 px-4 py-3 bg-gray-900/80 backdrop-blur">
                <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-2">
                        <Radio size={18} className={`${esRef.current ? "text-emerald-400 animate-pulse" : "text-gray-600"}`} />
                        <h1 className="text-sm font-semibold text-gray-100">Live Feed</h1>
                        <span className="text-[11px] text-gray-500 bg-gray-800 rounded-full px-2 py-0.5">{events.length}</span>
                    </div>
                    <div className="flex items-center gap-2">
                        <div className="relative">
                            <Filter size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-500" />
                            <input
                                value={filter}
                                onChange={(e) => setFilter(e.target.value)}
                                placeholder="Filter by type or session…"
                                className="pl-7 pr-3 py-1.5 text-xs bg-gray-800 border border-gray-700 rounded-lg text-gray-200 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/30 w-48"
                            />
                        </div>
                        <button
                            onClick={() => setPaused((p) => !p)}
                            className={`text-xs px-3 py-1.5 rounded-lg border transition-colors ${paused ? "border-amber-500/40 bg-amber-500/10 text-amber-300" : "border-gray-700 text-gray-400 hover:text-white hover:bg-gray-800"}`}
                        >
                            {paused ? "Paused" : "Live"}
                        </button>
                        <button onClick={() => { setEvents([]); }} className="p-1.5 rounded-lg hover:bg-gray-800 text-gray-500 hover:text-white transition-colors" title="Clear">
                            <Trash2 size={14} />
                        </button>
                        <button onClick={() => setAutoscroll((a) => !a)} className={`p-1.5 rounded-lg hover:bg-gray-800 transition-colors ${autoscroll ? "text-emerald-400" : "text-gray-600"}`} title="Auto-scroll">
                            <RefreshCw size={14} />
                        </button>
                    </div>
                </div>
            </div>

            {/* Events */}
            <div className="flex-1 overflow-y-auto">
                {events.length === 0 && (
                    <div className="flex flex-col items-center justify-center h-full gap-3 text-gray-600">
                        <Radio size={32} className="opacity-30" />
                        <p className="text-sm">Waiting for engine events…</p>
                        <p className="text-xs">Start a chat or trigger a routine to see live events here.</p>
                    </div>
                )}
                {filtered.map((ev) => <EventRow key={ev.id} ev={ev} />)}
                {!paused && <div ref={bottomRef} />}
            </div>

            {/* Footer hint */}
            <div className="shrink-0 border-t border-gray-800 px-4 py-2 flex items-center justify-between">
                <p className="text-[11px] text-gray-600">Global SSE stream · <span className="font-mono">/global/event</span></p>
                <p className="text-[11px] text-gray-600">Click any event to expand · max 300 events</p>
            </div>
        </div>
    );
}
