import { useState, useEffect, useCallback } from "react";
import { client } from "../api";
import { RoutineRecord, RoutineSchedule } from "@frumu/tandem-client";
import {
  Clock,
  Plus,
  Play,
  Trash2,
  RefreshCw,
  ChevronDown,
  ChevronRight,
  CheckCircle,
  XCircle,
  AlertCircle,
  Loader2,
  Calendar,
} from "lucide-react";

interface RoutineItem extends RoutineRecord {
  source: "routines" | "automations";
}

function parseCron(s?: string): string {
  if (!s) return "";
  const parts = s.trim().split(/\s+/);
  if (parts.length < 5) return s;
  const [min, hour, , , day] = parts;
  if (min === "0" && day === "*") return `Daily at ${hour.padStart(2, "0")}:00`;
  if (min !== "*" && hour !== "*") return `At ${hour.padStart(2, "0")}:${min.padStart(2, "0")}`;
  return s;
}

function getScheduleLabel(r: RoutineRecord): string {
  if (!r.schedule) return "Manual";
  if (typeof r.schedule === "string") return r.schedule;
  if (r.schedule.type === "cron") return parseCron(r.schedule.cron);
  if (r.schedule.type === "interval") {
    const interval = r.schedule.intervalMs / 1000;
    if (interval < 120) return `Every ${interval}s`;
    if (interval < 3600) return `Every ${Math.round(interval / 60)}m`;
    return `Every ${Math.round(interval / 3600)}h`;
  }
  if (r.schedule.type === "manual") return "Manual";
  return "Scheduled";
}

/* ─── Create form ─── */
interface CreateFormProps {
  onCreate: () => void;
}
function CreateForm({ onCreate }: CreateFormProps) {
  const [name, setName] = useState("");
  const [prompt, setPrompt] = useState("");
  const [scheduleType, setScheduleType] = useState<"interval" | "cron" | "manual">("interval");
  const [intervalMin, setIntervalMin] = useState("60");
  const [cron, setCron] = useState("0 8 * * *");
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const save = async () => {
    if (!name.trim() || !prompt.trim()) {
      setErr("Name and prompt are required.");
      return;
    }
    setSaving(true);
    setErr(null);
    try {
      const schedule: RoutineSchedule | undefined =
        scheduleType === "interval"
          ? { type: "interval", intervalMs: parseInt(intervalMin) * 60 * 1000 }
          : scheduleType === "cron"
            ? { type: "cron", cron }
            : { type: "manual" };
      await client.routines.create({ name: name.trim(), prompt: prompt.trim(), schedule });
      setName("");
      setPrompt("");
      setIntervalMin("60");
      setCron("0 8 * * *");
      onCreate();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="bg-gray-900/60 border border-gray-800 rounded-2xl p-4 space-y-3">
      <h3 className="text-sm font-semibold text-gray-200 flex items-center gap-2">
        <Plus size={14} />
        New Routine
      </h3>
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Routine name, e.g. Daily Digest"
        className="w-full bg-gray-800 border border-gray-700 rounded-xl px-3 py-2 text-sm text-gray-200 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/40"
      />
      <textarea
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        placeholder="Prompt, e.g. Search the web for AI news today and save a summary to notes/ai-digest.md"
        rows={3}
        className="w-full bg-gray-800 border border-gray-700 rounded-xl px-3 py-2 text-sm text-gray-200 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/40 resize-none"
      />
      <div className="flex gap-2">
        {(["interval", "cron", "manual"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setScheduleType(t)}
            className={`text-xs px-3 py-1.5 rounded-lg border transition-colors ${scheduleType === t ? "border-emerald-500/50 bg-emerald-500/10 text-emerald-300" : "border-gray-700 text-gray-400 hover:text-white hover:bg-gray-800"}`}
          >
            {t === "interval" ? "Interval" : t === "cron" ? "Cron" : "Run manually"}
          </button>
        ))}
      </div>
      {scheduleType === "interval" && (
        <div className="flex items-center gap-2">
          <input
            type="number"
            min="1"
            value={intervalMin}
            onChange={(e) => setIntervalMin(e.target.value)}
            className="w-24 bg-gray-800 border border-gray-700 rounded-xl px-3 py-2 text-sm text-gray-200 focus:outline-none focus:ring-2 focus:ring-emerald-500/40"
          />
          <span className="text-sm text-gray-400">minutes</span>
        </div>
      )}
      {scheduleType === "cron" && (
        <input
          value={cron}
          onChange={(e) => setCron(e.target.value)}
          placeholder="0 8 * * *  (8am daily)"
          className="w-full bg-gray-800 border border-gray-700 rounded-xl px-3 py-2 text-sm font-mono text-gray-200 focus:outline-none focus:ring-2 focus:ring-emerald-500/40"
        />
      )}
      {err && <p className="text-xs text-rose-400">{err}</p>}
      <button
        onClick={() => void save()}
        disabled={saving}
        className="w-full py-2 rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:bg-gray-700 disabled:text-gray-500 text-white text-sm font-medium transition-colors"
      >
        {saving ? "Creating…" : "Create Routine"}
      </button>
    </div>
  );
}

/* ─── Routine card ─── */
function RoutineCard({
  r,
  onDelete,
  onRunNow,
  onRefresh,
}: {
  r: RoutineItem;
  onDelete: () => void;
  onRunNow: () => void;
  onRefresh: () => void;
}) {
  const [running, setRunning] = useState(false);
  const [open, setOpen] = useState(false);

  const run = async () => {
    setRunning(true);
    try {
      await client.routines.run(r.id);
      onRefresh();
    } catch {
      /* ignore */
    } finally {
      setRunning(false);
    }
  };

  const del = async () => {
    try {
      await client.routines.delete(r.id);
      onDelete();
    } catch {
      /* ignore */
    }
  };

  const status = String(r.status || "").toLowerCase();
  const isActive = status === "active" || status === "running" || status === "enabled";
  const isFailed = status === "failed" || status === "error";

  return (
    <div
      className={`border rounded-2xl overflow-hidden transition-all ${open ? "border-emerald-800/50" : "border-gray-800/60"} bg-gray-900/60`}
    >
      <button
        className="w-full flex items-center gap-3 px-4 py-3 hover:bg-gray-800/40 transition-colors text-left"
        onClick={() => setOpen((o) => !o)}
      >
        {open ? (
          <ChevronDown size={14} className="text-gray-500 shrink-0" />
        ) : (
          <ChevronRight size={14} className="text-gray-500 shrink-0" />
        )}
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-gray-100 truncate">
            {r.name || (r.title as string) || r.id}
          </p>
          <p className="text-xs text-gray-500 flex items-center gap-1 mt-0.5">
            <Calendar size={10} />
            {getScheduleLabel(r)}
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {isFailed && <XCircle size={14} className="text-rose-400" />}
          {isActive && <CheckCircle size={14} className="text-emerald-400" />}
          {!isFailed && !isActive && status && (
            <span className="text-[10px] text-gray-500 capitalize">{status}</span>
          )}
        </div>
      </button>
      {open && (
        <div className="border-t border-gray-800 px-4 py-3 space-y-2">
          {typeof r.prompt === "string" && r.prompt.trim().length > 0 && (
            <p className="text-xs text-gray-400 bg-gray-950/50 rounded-lg p-2 font-mono whitespace-pre-wrap">
              {r.prompt}
            </p>
          )}
          {(r.lastRun || r.lastRunAt || r.last_run || r.last_run_at) && (
            <p className="text-xs text-gray-500">
              Last run:{" "}
              <span className="text-gray-400">
                {String(r.lastRun || r.lastRunAt || r.last_run || r.last_run_at)}
              </span>
            </p>
          )}
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={() => {
                void run();
                onRunNow();
              }}
              disabled={running}
              className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg bg-emerald-600/10 border border-emerald-600/30 text-emerald-400 hover:bg-emerald-600/20 transition-colors disabled:opacity-50"
            >
              {running ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
              Run now
            </button>
            <button
              onClick={() => void del()}
              className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg border border-gray-700 text-gray-400 hover:text-rose-400 hover:border-rose-800/50 transition-colors"
            >
              <Trash2 size={12} />
              Delete
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/* ─── Main ─── */
export default function Agents() {
  const [items, setItems] = useState<RoutineItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [r, a] = await Promise.all([client.routines.list(), client.automations.list()]);
      const rItems = ((r.items || r.definitions || []) as RoutineRecord[]).map((x) => ({
        ...x,
        source: "routines" as const,
      }));
      const aItems = ((a.items || a.definitions || []) as RoutineRecord[]).map((x) => ({
        ...x,
        source: "automations" as const,
      }));
      const seen = new Set<string>();
      const merged = [...rItems, ...aItems].filter((x) => {
        if (seen.has(x.id)) return false;
        seen.add(x.id);
        return true;
      });
      setItems(merged);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="h-full overflow-y-auto bg-gray-950">
      <div className="max-w-2xl mx-auto px-4 py-8 space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-white flex items-center gap-2">
              <Clock className="text-emerald-400" size={22} />
              Agents
            </h1>
            <p className="text-sm text-gray-400 mt-1">
              Scheduled routines that run automatically on your engine.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => void load()}
              className="p-2 rounded-lg hover:bg-gray-800 text-gray-400 hover:text-white transition-colors"
              title="Refresh"
            >
              <RefreshCw size={16} className={loading ? "animate-spin" : ""} />
            </button>
            <button
              onClick={() => setShowCreate((s) => !s)}
              className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white text-sm font-medium transition-colors"
            >
              <Plus size={14} />
              {showCreate ? "Cancel" : "New"}
            </button>
          </div>
        </div>

        {/* Create form */}
        {showCreate && (
          <CreateForm
            onCreate={() => {
              setShowCreate(false);
              void load();
            }}
          />
        )}

        {/* Error */}
        {error && (
          <div className="flex items-center gap-2 text-rose-400 bg-rose-900/20 border border-rose-800/40 rounded-xl px-4 py-3 text-sm">
            <AlertCircle size={14} />
            {error}
          </div>
        )}

        {/* Loading */}
        {loading && (
          <div className="flex items-center justify-center py-16 text-gray-600">
            <Loader2 size={24} className="animate-spin" />
          </div>
        )}

        {/* Items */}
        {!loading && items.length === 0 && !error && (
          <div className="text-center py-16 text-gray-600">
            <Clock size={32} className="mx-auto mb-3 opacity-30" />
            <p className="text-sm">No routines yet. Create one to automate tasks on a schedule.</p>
          </div>
        )}

        <div className="space-y-3">
          {items.map((r) => (
            <RoutineCard
              key={r.id}
              r={r}
              onDelete={() => setItems((p) => p.filter((x) => x.id !== r.id))}
              onRunNow={() => {
                /* immediate feedback */
              }}
              onRefresh={() => void load()}
            />
          ))}
        </div>

        {/* Hint */}
        {!loading && items.length > 0 && (
          <p className="text-xs text-center text-gray-600">
            {items.length} routine{items.length !== 1 ? "s" : ""} · Runs are managed by the engine
            scheduler
          </p>
        )}
      </div>
    </div>
  );
}
