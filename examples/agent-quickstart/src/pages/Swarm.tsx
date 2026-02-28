import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  asEpochMs,
  client,
  getSharedResource,
  getWorkspaceRoot,
  launchSwarmManager,
  listRoutinesRaw,
  putSharedResource,
  runRoutineNowRaw,
  setWorkspaceRoot,
  SwarmRegistry,
  SwarmTaskRecord,
  upsertRoutineRaw,
} from "../api";
import {
  Activity,
  Bot,
  CheckCircle2,
  Loader2,
  Network,
  Play,
  RefreshCw,
  Save,
  Settings,
  Sparkles,
  Workflow,
  Wrench,
} from "lucide-react";

const SWARM_KEYS = ["swarm.active_tasks", "project/swarm.active_tasks"] as const;
const HEALTH_ROUTINE_ID = "swarm-health-check";

const HEALTH_ROUTINE_PAYLOAD: Record<string, unknown> = {
  routine_id: HEALTH_ROUTINE_ID,
  name: "Swarm Health Check",
  schedule: {
    cron: {
      expression: "*/10 * * * *",
    },
  },
  timezone: "UTC",
  misfire_policy: "run_once",
  entrypoint: "mission.default",
  args: {
    mode: "standalone",
    uses_external_integrations: true,
    prompt:
      "Run: bash examples/agent-swarm/scripts/check_swarm_health.sh. Summarize stuck tasks and GitHub check results from swarm.active_tasks.",
  },
  allowed_tools: ["bash", "read", "mcp.arcade.*"],
  requires_approval: true,
  external_integrations_allowed: true,
  output_targets: ["file://reports/swarm-health/latest.txt"],
};

interface WirePulse {
  edge: string;
  atMs: number;
}

interface GraphNode {
  id: string;
  label: string;
  x: number;
  y: number;
  role: string;
  active: boolean;
}

const statusColor = (status: string): string => {
  const normalized = status.toLowerCase();
  if (normalized.includes("fail")) return "text-rose-300 bg-rose-500/15 border-rose-500/40";
  if (normalized.includes("block")) return "text-amber-300 bg-amber-500/15 border-amber-500/40";
  if (normalized.includes("ready")) return "text-sky-300 bg-sky-500/15 border-sky-500/40";
  if (normalized.includes("complete"))
    return "text-emerald-300 bg-emerald-500/15 border-emerald-500/40";
  return "text-indigo-300 bg-indigo-500/15 border-indigo-500/40";
};

function useNow(tickMs: number) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), tickMs);
    return () => window.clearInterval(id);
  }, [tickMs]);
  return now;
}

function WireGraph({ tasks, pulses }: { tasks: SwarmTaskRecord[]; pulses: WirePulse[] }) {
  const workerTasks = tasks.slice(0, 5);
  const nodes: GraphNode[] = [
    { id: "manager", label: "Manager", x: 120, y: 100, role: "manager", active: true },
    {
      id: "tester",
      label: "Tester",
      x: 560,
      y: 52,
      role: "tester",
      active: tasks.some((t) => t.ownerRole === "tester"),
    },
    {
      id: "reviewer",
      label: "Reviewer",
      x: 560,
      y: 148,
      role: "reviewer",
      active: tasks.some((t) => t.ownerRole === "reviewer"),
    },
    ...workerTasks.map((task, idx) => ({
      id: `worker-${idx}`,
      label: task.taskId,
      x: 340,
      y: 44 + idx * 44,
      role: "worker",
      active: ["running", "blocked", "ready_for_review"].includes(task.status),
    })),
  ];

  const edgeKeys = new Set(pulses.filter((p) => Date.now() - p.atMs < 5000).map((p) => p.edge));

  return (
    <div className="relative rounded-2xl border border-gray-800 bg-[#111827]/70 overflow-hidden">
      <svg viewBox="0 0 680 240" className="w-full h-[280px]">
        <defs>
          <linearGradient id="wire-grad" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0%" stopColor="#22d3ee" stopOpacity="0.15" />
            <stop offset="50%" stopColor="#60a5fa" stopOpacity="0.95" />
            <stop offset="100%" stopColor="#34d399" stopOpacity="0.15" />
          </linearGradient>
          <filter id="wire-glow" x="-40%" y="-40%" width="180%" height="180%">
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {workerTasks.map((_, idx) => {
          const y = 44 + idx * 44;
          const edgeKey = `manager:worker-${idx}`;
          const active = edgeKeys.has(edgeKey);
          return (
            <line
              key={edgeKey}
              x1={156}
              y1={100}
              x2={304}
              y2={y}
              className={active ? "wire-line wire-line-active" : "wire-line"}
            />
          );
        })}

        {workerTasks.map((task, idx) => {
          const y = 44 + idx * 44;
          const toTester =
            task.ownerRole === "tester" ||
            task.ownerRole === "reviewer" ||
            task.status === "ready_for_review";
          const toReviewer = task.ownerRole === "reviewer" || task.status === "ready_for_review";
          return (
            <g key={`fanout-${task.taskId}`}>
              <line
                x1={376}
                y1={y}
                x2={526}
                y2={52}
                className={toTester ? "wire-line wire-line-active" : "wire-line"}
              />
              <line
                x1={376}
                y1={y}
                x2={526}
                y2={148}
                className={toReviewer ? "wire-line wire-line-active" : "wire-line"}
              />
            </g>
          );
        })}

        {nodes.map((node) => (
          <g key={node.id}>
            <circle
              cx={node.x}
              cy={node.y}
              r={node.active ? 20 : 16}
              className={node.active ? "node node-active" : "node"}
            />
            <text
              x={node.x}
              y={node.y + 34}
              textAnchor="middle"
              className="fill-gray-300 text-[11px] font-medium"
            >
              {node.label}
            </text>
          </g>
        ))}
      </svg>
      <div className="absolute top-3 left-3 text-xs text-cyan-300/90 flex items-center gap-1">
        <Sparkles size={12} />
        Live communication wires
      </div>
      <style>{`
        .wire-line { stroke: rgba(148,163,184,0.25); stroke-width: 1.6; fill: none; }
        .wire-line-active { stroke: url(#wire-grad); stroke-width: 2.4; stroke-dasharray: 10 8; animation: wireFlow 1.1s linear infinite; filter: url(#wire-glow); }
        .node { fill: rgba(30,41,59,0.95); stroke: rgba(100,116,139,0.55); stroke-width: 1.5; }
        .node-active { fill: rgba(14,116,144,0.35); stroke: rgba(34,211,238,0.95); animation: nodePulse 1.5s ease-in-out infinite; }
        @keyframes wireFlow { from { stroke-dashoffset: 0; } to { stroke-dashoffset: -18; } }
        @keyframes nodePulse { 0% { filter: drop-shadow(0 0 0 rgba(34,211,238,0.0)); } 50% { filter: drop-shadow(0 0 8px rgba(34,211,238,0.7)); } 100% { filter: drop-shadow(0 0 0 rgba(34,211,238,0.0)); } }
      `}</style>
    </div>
  );
}

export default function Swarm() {
  const now = useNow(1000);
  const [workspaceRoot, setWorkspaceRootInput] = useState(
    getWorkspaceRoot() || "/home/evan/tandem"
  );
  const [objective, setObjective] = useState(
    "Implement a focused feature, run tests, open PRs, and prepare review notes"
  );
  const [registry, setRegistry] = useState<SwarmRegistry | null>(null);
  const [draftRegistry, setDraftRegistry] = useState<SwarmRegistry | null>(null);
  const [pulses, setPulses] = useState<WirePulse[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [routineInstalled, setRoutineInstalled] = useState(false);
  const [activeSwarmKey, setActiveSwarmKey] = useState<string>(SWARM_KEYS[0]);
  const mountedRef = useRef(true);

  const tasks = useMemo(() => {
    const taskMap = (draftRegistry || registry)?.tasks || {};
    return Object.values(taskMap).sort((a, b) => (b.lastUpdateMs || 0) - (a.lastUpdateMs || 0));
  }, [draftRegistry, registry]);

  const pushPulse = useCallback((edge: string) => {
    setPulses((prev) => [...prev.slice(-40), { edge, atMs: Date.now() }]);
  }, []);

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [resource, routines] = await Promise.all([
        (async () => {
          for (const key of SWARM_KEYS) {
            const rec = await getSharedResource<SwarmRegistry>(key);
            if (rec?.value) {
              setActiveSwarmKey(key);
              return rec;
            }
          }
          setActiveSwarmKey(SWARM_KEYS[0]);
          return null;
        })(),
        listRoutinesRaw(),
      ]);
      const base =
        resource?.value ||
        ({
          version: 1,
          updatedAtMs: Date.now(),
          tasks: {},
        } as SwarmRegistry);
      setRegistry(base);
      setDraftRegistry(JSON.parse(JSON.stringify(base)) as SwarmRegistry);
      setRoutineInstalled(
        Array.isArray(routines) &&
          routines.some(
            (r) =>
              (r as { routineID?: string; routine_id?: string }).routineID === HEALTH_ROUTINE_ID ||
              (r as { routine_id?: string }).routine_id === HEALTH_ROUTINE_ID
          )
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void loadData();
    return () => {
      mountedRef.current = false;
    };
  }, [loadData]);

  useEffect(() => {
    const controller = new AbortController();
    const run = async () => {
      try {
        const stream = client.globalStream({ signal: controller.signal });
        for await (const ev of stream) {
          const type = ev.type;
          if (
            type !== "session.run.started" &&
            type !== "message.part.updated" &&
            type !== "session.run.finished"
          ) {
            continue;
          }
          const sid = (ev.properties?.sessionID as string | undefined) || ev.sessionId;
          if (!sid) continue;
          const index = tasks.findIndex((t) => t.sessionId === sid);
          if (index < 0) continue;
          pushPulse(`manager:worker-${Math.min(index, 4)}`);
          if (type === "session.run.finished") {
            void loadData();
          }
        }
      } catch {
        // ignore stream reconnect; page polling refresh handles consistency
      }
    };
    void run();
    return () => controller.abort();
  }, [loadData, pushPulse, tasks]);

  const saveWorkspace = () => {
    setWorkspaceRoot(workspaceRoot.trim() || null);
    setMessage("Workspace saved for swarm launch commands.");
  };

  const withBusy = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const launchManager = async () => {
    await withBusy(async () => {
      const output = await launchSwarmManager(objective, workspaceRoot);
      setMessage(
        output.ok
          ? `Manager run completed. Session: ${output.sessionId}`
          : `Manager failed. Session: ${output.sessionId}`
      );
      await loadData();
    });
  };

  const installRoutine = async () => {
    await withBusy(async () => {
      await upsertRoutineRaw(HEALTH_ROUTINE_PAYLOAD);
      setRoutineInstalled(true);
      setMessage("Swarm health routine installed.");
    });
  };

  const runRoutine = async () => {
    await withBusy(async () => {
      await runRoutineNowRaw(HEALTH_ROUTINE_ID);
      setMessage("Swarm health routine triggered.");
    });
  };

  const saveRegistry = async () => {
    if (!draftRegistry) return;
    await withBusy(async () => {
      const saved = await putSharedResource(activeSwarmKey, {
        ...draftRegistry,
        updatedAtMs: Date.now(),
      }).catch(async () => {
        const fallback = SWARM_KEYS.find((k) => k !== activeSwarmKey) || SWARM_KEYS[1];
        const rec = await putSharedResource(fallback, {
          ...draftRegistry,
          updatedAtMs: Date.now(),
        });
        setActiveSwarmKey(fallback);
        return rec;
      });
      const next = (saved.value as SwarmRegistry) || draftRegistry;
      setRegistry(next);
      setDraftRegistry(JSON.parse(JSON.stringify(next)) as SwarmRegistry);
      setMessage("Swarm registry saved.");
    });
  };

  const updateTaskField = <K extends keyof SwarmTaskRecord>(
    taskId: string,
    key: K,
    value: SwarmTaskRecord[K]
  ) => {
    setDraftRegistry((prev) => {
      if (!prev) return prev;
      const task = prev.tasks[taskId];
      if (!task) return prev;
      return {
        ...prev,
        updatedAtMs: Date.now(),
        tasks: {
          ...prev.tasks,
          [taskId]: {
            ...task,
            [key]: value,
            lastUpdateMs: Date.now(),
          },
        },
      };
    });
  };

  const taskCount = tasks.length;
  const runningCount = tasks.filter((t) => t.status === "running").length;
  const blockedCount = tasks.filter((t) => t.status === "blocked").length;
  const readyCount = tasks.filter((t) => t.status === "ready_for_review").length;

  return (
    <div className="h-full overflow-y-auto bg-[#0b1020]">
      <div className="max-w-7xl mx-auto px-4 py-8 space-y-6">
        <div className="rounded-2xl border border-cyan-500/20 bg-gradient-to-r from-cyan-500/10 via-blue-500/10 to-emerald-500/10 p-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h1 className="text-2xl font-semibold text-white flex items-center gap-2">
                <Network className="text-cyan-300" size={24} />
                Swarm Control Room
              </h1>
              <p className="text-sm text-cyan-100/80 mt-1">
                Monitor live agent communications, edit task registry state, and operate swarm
                setup.
              </p>
            </div>
            <button
              onClick={() => void loadData()}
              disabled={busy || loading}
              className="px-3 py-2 rounded-xl bg-white/10 hover:bg-white/20 text-cyan-100 text-sm inline-flex items-center gap-2 disabled:opacity-50"
            >
              <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
              Refresh
            </button>
          </div>
        </div>

        {(error || message) && (
          <div
            className={`rounded-xl border px-4 py-3 text-sm ${
              error
                ? "bg-rose-900/30 border-rose-700/50 text-rose-200"
                : "bg-emerald-900/30 border-emerald-700/50 text-emerald-200"
            }`}
          >
            {error || message}
          </div>
        )}

        <div className="grid xl:grid-cols-[1.5fr_1fr] gap-6">
          <div className="space-y-4">
            <WireGraph tasks={tasks} pulses={pulses} />

            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
              <div className="rounded-xl border border-gray-800 bg-gray-900/70 p-3">
                <p className="text-xs text-gray-400">Tasks</p>
                <p className="text-xl font-semibold text-white mt-1">{taskCount}</p>
              </div>
              <div className="rounded-xl border border-gray-800 bg-gray-900/70 p-3">
                <p className="text-xs text-gray-400">Running</p>
                <p className="text-xl font-semibold text-cyan-300 mt-1">{runningCount}</p>
              </div>
              <div className="rounded-xl border border-gray-800 bg-gray-900/70 p-3">
                <p className="text-xs text-gray-400">Blocked</p>
                <p className="text-xl font-semibold text-amber-300 mt-1">{blockedCount}</p>
              </div>
              <div className="rounded-xl border border-gray-800 bg-gray-900/70 p-3">
                <p className="text-xs text-gray-400">Ready</p>
                <p className="text-xl font-semibold text-emerald-300 mt-1">{readyCount}</p>
              </div>
            </div>
          </div>

          <div className="space-y-4">
            <div className="rounded-2xl border border-gray-800 bg-gray-900/75 p-4 space-y-3">
              <h2 className="text-sm font-semibold text-white flex items-center gap-2">
                <Settings size={16} className="text-cyan-300" />
                Swarm Setup
              </h2>
              <label className="text-xs text-gray-400">Workspace root</label>
              <input
                value={workspaceRoot}
                onChange={(e) => setWorkspaceRootInput(e.target.value)}
                className="w-full bg-gray-800 border border-gray-700 rounded-xl px-3 py-2 text-sm text-gray-200"
              />
              <button
                onClick={saveWorkspace}
                className="w-full rounded-xl border border-gray-700 hover:border-cyan-500/50 px-3 py-2 text-sm text-gray-200"
              >
                Save workspace
              </button>

              <label className="text-xs text-gray-400">Manager objective</label>
              <textarea
                value={objective}
                onChange={(e) => setObjective(e.target.value)}
                className="w-full min-h-24 bg-gray-800 border border-gray-700 rounded-xl px-3 py-2 text-sm text-gray-200"
              />
              <button
                onClick={() => void launchManager()}
                disabled={busy || !objective.trim()}
                className="w-full rounded-xl bg-cyan-600 hover:bg-cyan-500 disabled:opacity-60 px-3 py-2 text-sm text-white font-medium inline-flex items-center justify-center gap-2"
              >
                {busy ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />}
                Launch swarm manager (Node)
              </button>
            </div>

            <div className="rounded-2xl border border-gray-800 bg-gray-900/75 p-4 space-y-3">
              <h2 className="text-sm font-semibold text-white flex items-center gap-2">
                <Wrench size={16} className="text-emerald-300" />
                Routine + Health
              </h2>
              <div className="text-xs text-gray-300 flex items-center gap-2">
                {routineInstalled ? (
                  <>
                    <CheckCircle2 size={14} className="text-emerald-300" /> Installed:{" "}
                    {HEALTH_ROUTINE_ID}
                  </>
                ) : (
                  <>
                    <Activity size={14} className="text-amber-300" /> Routine not detected
                  </>
                )}
              </div>
              <button
                onClick={() => void installRoutine()}
                disabled={busy}
                className="w-full rounded-xl border border-gray-700 hover:border-emerald-500/50 px-3 py-2 text-sm text-gray-200"
              >
                Install / Upsert health routine
              </button>
              <button
                onClick={() => void runRoutine()}
                disabled={busy}
                className="w-full rounded-xl bg-emerald-600/80 hover:bg-emerald-500 disabled:opacity-60 px-3 py-2 text-sm text-white"
              >
                Run health check now
              </button>
              <p className="text-[11px] text-gray-500">
                Telegram delivery depends on engine env: <code>TANDEM_TELEGRAM_BOT_TOKEN</code> and{" "}
                <code>TANDEM_SWARM_TELEGRAM_CHAT_ID</code>.
              </p>
            </div>
          </div>
        </div>

        <div className="rounded-2xl border border-gray-800 bg-gray-900/70 overflow-hidden">
          <div className="px-4 py-3 border-b border-gray-800 flex items-center justify-between gap-3">
            <h2 className="text-sm font-semibold text-white flex items-center gap-2">
              <Workflow size={16} className="text-indigo-300" />
              Swarm Task Registry Editor
            </h2>
            <button
              onClick={() => void saveRegistry()}
              disabled={busy || !draftRegistry}
              className="px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 disabled:opacity-60 text-xs text-white inline-flex items-center gap-2"
            >
              <Save size={12} /> Save registry
            </button>
          </div>

          {loading ? (
            <div className="p-8 flex justify-center text-gray-500">
              <Loader2 size={20} className="animate-spin" />
            </div>
          ) : tasks.length === 0 ? (
            <div className="p-6 text-sm text-gray-500 flex items-center gap-2">
              <Bot size={14} /> No tasks found in <code>{activeSwarmKey}</code>.
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full min-w-[1200px]">
                <thead className="bg-gray-950/60">
                  <tr className="text-left text-xs text-gray-400">
                    <th className="px-3 py-2">Task</th>
                    <th className="px-3 py-2">Owner</th>
                    <th className="px-3 py-2">Status</th>
                    <th className="px-3 py-2">Reason</th>
                    <th className="px-3 py-2">Branch</th>
                    <th className="px-3 py-2">PR</th>
                    <th className="px-3 py-2">Checks</th>
                    <th className="px-3 py-2">Updated</th>
                  </tr>
                </thead>
                <tbody>
                  {tasks.map((task) => (
                    <tr key={task.taskId} className="border-t border-gray-800 text-sm align-top">
                      <td className="px-3 py-2 text-gray-100">
                        <div className="font-medium">{task.title}</div>
                        <div className="text-xs text-gray-500 mt-1">{task.taskId}</div>
                      </td>
                      <td className="px-3 py-2">
                        <select
                          value={task.ownerRole}
                          onChange={(e) =>
                            updateTaskField(task.taskId, "ownerRole", e.target.value)
                          }
                          className="bg-gray-800 border border-gray-700 rounded px-2 py-1 text-gray-100"
                        >
                          <option>worker</option>
                          <option>tester</option>
                          <option>reviewer</option>
                          <option>manager</option>
                        </select>
                      </td>
                      <td className="px-3 py-2">
                        <select
                          value={task.status}
                          onChange={(e) => updateTaskField(task.taskId, "status", e.target.value)}
                          className={`border rounded px-2 py-1 ${statusColor(task.status)}`}
                        >
                          <option>pending</option>
                          <option>running</option>
                          <option>blocked</option>
                          <option>ready_for_review</option>
                          <option>complete</option>
                          <option>failed</option>
                        </select>
                      </td>
                      <td className="px-3 py-2">
                        <input
                          value={task.statusReason || ""}
                          onChange={(e) =>
                            updateTaskField(task.taskId, "statusReason", e.target.value)
                          }
                          className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1 text-gray-100"
                        />
                      </td>
                      <td className="px-3 py-2 text-gray-300 font-mono text-xs">{task.branch}</td>
                      <td className="px-3 py-2">
                        {task.prUrl ? (
                          <a
                            href={task.prUrl}
                            target="_blank"
                            rel="noreferrer"
                            className="text-cyan-300 hover:text-cyan-200 underline decoration-dotted"
                          >
                            #{task.prNumber || "PR"}
                          </a>
                        ) : (
                          <span className="text-gray-600">none</span>
                        )}
                      </td>
                      <td className="px-3 py-2">
                        <input
                          value={task.checksStatus || ""}
                          onChange={(e) =>
                            updateTaskField(task.taskId, "checksStatus", e.target.value)
                          }
                          className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1 text-gray-100"
                        />
                      </td>
                      <td className="px-3 py-2 text-xs text-gray-500">
                        {Math.max(0, Math.floor((now - asEpochMs(task.lastUpdateMs)) / 1000))}s ago
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
        <div className="text-xs text-gray-500">
          Active swarm registry key: <code>{activeSwarmKey}</code>
        </div>
      </div>
    </div>
  );
}
