import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Activity, Bolt, Loader2, Play, Sparkles } from "lucide-react";
import {
  api,
  opencodeBenchApi,
  type OpencodeBenchHistoryResponse,
  type OpencodeBenchLatestResult,
  type OpencodeBenchScenarioResult,
} from "../api";
import { attachPortalRunStream } from "../utils/portalRunStream";

interface StressRunRecord {
  id: string;
  workerId: number;
  sessionId: string;
  runId: string;
  status: "running" | "completed" | "errored";
  startedAt: number;
  completedAt?: number;
  firstDeltaAt?: number;
  prompt: string;
  events: string[];
  lastStatus?: string;
  error?: string;
}

interface MetricSnapshot {
  timestamp: number;
  completed: number;
  errored: number;
  active: number;
  avgLatency: number;
  avgFirstDelta: number;
}

interface DiagnosticStats {
  count: number;
  commandMs: number;
  getMs: number;
  listMs: number;
  mixedMs: number;
}

interface ServerSoakMetrics {
  completed: number;
  errored: number;
  avgLatency: number;
}

interface ScenarioComparison {
  tandemAvgMs: number;
  tandemP95Ms: number;
  tandemP99Ms: number;
  opencodeAvgMs: number;
  opencodeP95Ms: number;
  opencodeP99Ms: number;
  opencodeErrors: number;
}

const DEFAULT_PROMPT = `Fetch https://tandem.frumu.ai/docs/ via webfetch (markdown mode) and summarize the first 20 tokens of the page in one sentence.`;

export const StressLab: React.FC = () => {
  const [concurrency, setConcurrency] = useState(4);
  const [concurrencyInput, setConcurrencyInput] = useState("4");
  const [cycleDelay, setCycleDelay] = useState(1200);
  const [cycleDelayInput, setCycleDelayInput] = useState("1200");
  const [prompt, setPrompt] = useState("");
  const [scenarioMode, setScenarioMode] = useState<
    "remote" | "file" | "inline" | "shared_edit" | "providerless"
  >("remote");
  const [providerlessProfile, setProviderlessProfile] = useState<
    | "command_only"
    | "get_session_only"
    | "list_sessions_only"
    | "mixed"
    | "diagnostic_sweep"
    | "soak_mixed"
  >("mixed");
  const [providerlessRunner, setProviderlessRunner] = useState<"browser" | "server">("server");
  const [filePath, setFilePath] = useState("/srv/tandem/docs/overview.md");
  const [inlineBody, setInlineBody] = useState("# Summary\n- Highlight");
  const sharedEditFixture = useMemo(
    () =>
      Array.from(
        { length: 200 },
        (_, idx) =>
          `- Line ${idx + 1}: Tandem benchmark fixture sentence ${idx + 1} about reliability, latency, and observability.`
      ).join("\n"),
    []
  );
  const [providerlessCommand, setProviderlessCommand] = useState("pwd");
  const [soakSecondsInput, setSoakSecondsInput] = useState("60");
  const [isRunning, setIsRunning] = useState(false);
  const [runRecords, setRunRecords] = useState<StressRunRecord[]>([]);
  const [eventLog, setEventLog] = useState<string[]>([]);
  const [metricHistory, setMetricHistory] = useState<MetricSnapshot[]>([]);
  const [diagnosticStats, setDiagnosticStats] = useState<DiagnosticStats>({
    count: 0,
    commandMs: 0,
    getMs: 0,
    listMs: 0,
    mixedMs: 0,
  });
  const [soakReport, setSoakReport] = useState("");
  const [serverSoakMetrics, setServerSoakMetrics] = useState<ServerSoakMetrics>({
    completed: 0,
    errored: 0,
    avgLatency: 0,
  });
  const [opencodeLatest, setOpencodeLatest] = useState<OpencodeBenchLatestResult | null>(null);
  const [opencodeHistory, setOpencodeHistory] = useState<OpencodeBenchHistoryResponse | null>(null);
  const [opencodeHealth, setOpencodeHealth] = useState<string>("unknown");
  const [comparisonError, setComparisonError] = useState<string | null>(null);
  const [comparisonLoading, setComparisonLoading] = useState(false);
  const [showSharedFixture, setShowSharedFixture] = useState(false);
  const eventLogRef = useRef<HTMLDivElement | null>(null);

  const stressActiveRef = useRef(false);
  const cycleDelayRef = useRef(cycleDelay);
  const promptRef = useRef("");
  const scenarioRef = useRef(scenarioMode);
  const providerlessCommandRef = useRef(providerlessCommand);
  const providerlessProfileRef = useRef(providerlessProfile);
  const providerlessRunnerRef = useRef(providerlessRunner);
  const soakSecondsRef = useRef(60);
  const activeWorkersRef = useRef(0);
  const soakSamplesRef = useRef<number[]>([]);
  const soakErrorsRef = useRef(0);
  const soakStartedAtRef = useRef<number>(0);
  const soakStopAtRef = useRef<number>(0);
  const workerSessions = useRef<Record<number, string>>({});
  const streamRefs = useRef<Record<string, { current: EventSource | null }>>({});
  const serverSoakStreamRef = useRef<EventSource | null>(null);

  useEffect(() => {
    cycleDelayRef.current = cycleDelay;
  }, [cycleDelay]);

  useEffect(() => {
    setConcurrencyInput(String(concurrency));
  }, [concurrency]);

  useEffect(() => {
    setCycleDelayInput(String(cycleDelay));
  }, [cycleDelay]);

  const basePrompt = useMemo(() => {
    if (scenarioMode === "providerless") {
      if (providerlessProfile === "command_only") {
        return `Providerless mode (command-only): run command "${providerlessCommand}" only.`;
      }
      if (providerlessProfile === "get_session_only") {
        return "Providerless mode (getSession-only): stress session lookup endpoint only.";
      }
      if (providerlessProfile === "list_sessions_only") {
        return "Providerless mode (listSessions-only): stress session list endpoint only.";
      }
      if (providerlessProfile === "diagnostic_sweep") {
        return `Providerless mode (diagnostic sweep): run command/getSession/listSessions/mixed once and auto-stop.`;
      }
      if (providerlessProfile === "soak_mixed") {
        if (providerlessRunner === "server") {
          return `Providerless soak mode (server): run mixed endpoint load from the portal server for ${soakSecondsInput}s and stream back results.`;
        }
        return `Providerless soak mode (browser): run mixed endpoint load continuously for ${soakSecondsInput}s, then emit percentile report.`;
      }
      return `Providerless mode (mixed): run command "${providerlessCommand}" + getSession + listSessions together.`;
    }
    if (scenarioMode === "file") {
      return `Use the read tool to open ${filePath} and summarize its key sections in one markdown paragraph.`;
    }
    if (scenarioMode === "shared_edit") {
      return (
        "Edit the shared 200-line markdown fixture for clarity. Return improved markdown and " +
        `a 5-item bullet list of key edits.\n\n${sharedEditFixture}`
      );
    }
    if (scenarioMode === "inline") {
      return `Summarize the following markdown blob:\n\n${inlineBody}`;
    }
    return DEFAULT_PROMPT;
  }, [
    scenarioMode,
    filePath,
    inlineBody,
    sharedEditFixture,
    providerlessCommand,
    providerlessProfile,
    providerlessRunner,
    soakSecondsInput,
  ]);

  const currentPrompt = useMemo(() => {
    const suffix = prompt.trim();
    return suffix ? `${basePrompt}\n\n${suffix}` : basePrompt;
  }, [basePrompt, prompt]);

  const basePromptPreview = useMemo(() => {
    if (scenarioMode !== "shared_edit") return basePrompt;
    return showSharedFixture ? basePrompt : `${basePrompt.split("\n").slice(0, 3).join("\n")}\n…`;
  }, [basePrompt, scenarioMode, showSharedFixture]);

  useEffect(() => {
    promptRef.current = currentPrompt;
  }, [currentPrompt]);

  useEffect(() => {
    scenarioRef.current = scenarioMode;
  }, [scenarioMode]);

  useEffect(() => {
    providerlessCommandRef.current = providerlessCommand;
  }, [providerlessCommand]);

  useEffect(() => {
    providerlessProfileRef.current = providerlessProfile;
  }, [providerlessProfile]);

  useEffect(() => {
    providerlessRunnerRef.current = providerlessRunner;
  }, [providerlessRunner]);

  useEffect(() => {
    soakSecondsRef.current = Math.max(5, parsePositiveInt(soakSecondsInput, 60));
  }, [soakSecondsInput]);

  const addEventLog = useCallback((message: string) => {
    setEventLog((prev) => {
      const next = [...prev, `${new Date().toLocaleTimeString()} – ${message}`];
      return next.slice(-80);
    });
  }, []);

  const refreshComparisonData = useCallback(async () => {
    setComparisonLoading(true);
    setComparisonError(null);
    try {
      const [latest, history, health] = await Promise.all([
        opencodeBenchApi.getLatest(),
        opencodeBenchApi.getHistory(30),
        opencodeBenchApi.getHealth(),
      ]);
      setOpencodeLatest(latest);
      setOpencodeHistory(history);
      const healthRow = health as Record<string, unknown>;
      setOpencodeHealth(String(healthRow.ready ?? healthRow.ok ?? "ok"));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setComparisonError(message);
      setOpencodeHealth("unreachable");
    } finally {
      setComparisonLoading(false);
    }
  }, []);

  const parsePositiveInt = (value: string, fallback: number): number => {
    const parsed = Number.parseInt(value.trim(), 10);
    if (!Number.isFinite(parsed) || parsed < 1) return fallback;
    return parsed;
  };

  const percentile = (values: number[], p: number): number => {
    if (values.length === 0) return 0;
    const sorted = [...values].sort((a, b) => a - b);
    const idx = Math.min(sorted.length - 1, Math.max(0, Math.ceil((p / 100) * sorted.length) - 1));
    return sorted[idx];
  };

  const buildSoakReport = useCallback((durationSec: number, samples: number[], errors: number) => {
    const attempts = samples.length + errors;
    const errorRate = attempts > 0 ? (errors / attempts) * 100 : 0;
    if (samples.length === 0) {
      return [
        "Soak Results (providerless mixed)",
        `Duration: ${durationSec}s`,
        "Samples: 0",
        `Attempts: ${attempts}`,
        `Errors: ${errors}`,
        `Provider error rate: ${errorRate.toFixed(1)}%`,
        "No successful samples captured.",
      ].join("\n");
    }
    const avg = samples.reduce((sum, value) => sum + value, 0) / samples.length;
    const p50 = percentile(samples, 50);
    const p95 = percentile(samples, 95);
    const p99 = percentile(samples, 99);
    const min = Math.min(...samples);
    const max = Math.max(...samples);
    return [
      "Soak Results (providerless mixed)",
      `Duration: ${durationSec}s`,
      `Samples: ${samples.length}`,
      `Attempts: ${attempts}`,
      `Errors: ${errors}`,
      `Provider error rate: ${errorRate.toFixed(1)}%`,
      `Avg: ${Math.round(avg)}ms`,
      `P50: ${Math.round(p50)}ms`,
      `P95: ${Math.round(p95)}ms`,
      `P99: ${Math.round(p99)}ms`,
      `Min: ${Math.round(min)}ms`,
      `Max: ${Math.round(max)}ms`,
    ].join("\n");
  }, []);

  const addRunRecord = useCallback((record: StressRunRecord) => {
    setRunRecords((prev) => {
      const filtered = prev.filter((entry) => entry.id !== record.id);
      return [...filtered, record].slice(-120);
    });
  }, []);

  type RunRecordPatch =
    | Partial<StressRunRecord>
    | ((previous: StressRunRecord) => Partial<StressRunRecord> | undefined);
  const updateRunRecord = useCallback((id: string, patch: RunRecordPatch) => {
    setRunRecords((prev) =>
      prev.map((entry) => {
        if (entry.id !== id) return entry;
        const delta = typeof patch === "function" ? patch(entry) : patch;
        if (!delta) return entry;
        return { ...entry, ...delta };
      })
    );
  }, []);

  const attachAndTrack = useCallback(
    async (workerId: number, sessionId: string, runId: string, recordId: string): Promise<void> => {
      return new Promise((resolve) => {
        const holder: { current: EventSource | null } = { current: null };
        streamRefs.current[recordId] = holder;
        attachPortalRunStream(holder, sessionId, runId, {
          addSystemLog: (content) => {
            addEventLog(`[worker ${workerId}] ${content}`);
          },
          addTextDelta: (delta) => {
            if (!delta.trim()) return;
            updateRunRecord(recordId, (prev) => {
              if (!prev.firstDeltaAt) {
                return { ...prev, firstDeltaAt: Date.now(), events: [...prev.events, delta] };
              }
              return { ...prev, events: [...prev.events, delta] };
            });
          },
          onToolStart: ({ tool }) => {
            addEventLog(`[worker ${workerId}] tool ${tool} started`);
          },
          onToolEnd: ({ tool, result }) => {
            addEventLog(`[worker ${workerId}] tool ${tool} ended`);
            updateRunRecord(recordId, (prev) => ({
              ...prev,
              events: [...prev.events, `tool ${tool} result ${result.slice(0, 40)}`],
            }));
          },
          onFinalize: (status) => {
            const completedAt = Date.now();
            const isSuccess = status === "completed";
            updateRunRecord(recordId, (prev) => ({
              ...prev,
              status: isSuccess ? "completed" : "errored",
              lastStatus: status,
              completedAt,
              error: isSuccess ? undefined : status,
            }));
            holder.current?.close();
            delete streamRefs.current[recordId];
            resolve();
          },
        });
      });
    },
    [addEventLog, updateRunRecord]
  );

  const runWorkerLoop = useCallback(
    async (workerId: number) => {
      try {
        const sessionId = await api.createSession(`Stress worker ${workerId}`);
        workerSessions.current[workerId] = sessionId;
        while (stressActiveRef.current) {
          const scenario = scenarioRef.current;
          const runId =
            scenario === "providerless"
              ? `providerless-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
              : (
                  await api.startAsyncRun(
                    sessionId,
                    `${promptRef.current} (worker ${workerId} @ ${new Date().toISOString()})`
                  )
                ).runId;
          const recordId = `${workerId}-${runId}`;
          const record: StressRunRecord = {
            id: recordId,
            workerId,
            sessionId,
            runId,
            prompt:
              scenario === "providerless"
                ? `providerless:${providerlessCommandRef.current}`
                : `${promptRef.current} (worker ${workerId})`,
            startedAt: Date.now(),
            status: "running",
            events: [],
          };
          addRunRecord(record);
          if (scenario === "providerless") {
            try {
              const profile = providerlessProfileRef.current;
              if (profile === "command_only") {
                await api.runSessionCommand(sessionId, providerlessCommandRef.current);
              } else if (profile === "get_session_only") {
                await api.getSession(sessionId);
              } else if (profile === "list_sessions_only") {
                await api.listSessions({ pageSize: 5 });
              } else if (profile === "diagnostic_sweep") {
                const t0 = performance.now();
                await api.runSessionCommand(sessionId, providerlessCommandRef.current);
                const commandMs = performance.now() - t0;

                const t1 = performance.now();
                await api.getSession(sessionId);
                const getMs = performance.now() - t1;

                const t2 = performance.now();
                await api.listSessions({ pageSize: 5 });
                const listMs = performance.now() - t2;

                const t3 = performance.now();
                await Promise.all([
                  api.runSessionCommand(sessionId, providerlessCommandRef.current),
                  api.getSession(sessionId),
                  api.listSessions({ pageSize: 5 }),
                ]);
                const mixedMs = performance.now() - t3;

                setDiagnosticStats((prev) => ({
                  count: prev.count + 1,
                  commandMs: prev.commandMs + commandMs,
                  getMs: prev.getMs + getMs,
                  listMs: prev.listMs + listMs,
                  mixedMs: prev.mixedMs + mixedMs,
                }));

                addEventLog(
                  `[worker ${workerId}] sweep command=${Math.round(commandMs)}ms get=${Math.round(getMs)}ms list=${Math.round(listMs)}ms mixed=${Math.round(mixedMs)}ms`
                );
              } else if (profile === "soak_mixed") {
                const t = performance.now();
                await Promise.all([
                  api.runSessionCommand(sessionId, providerlessCommandRef.current),
                  api.getSession(sessionId),
                  api.listSessions({ pageSize: 5 }),
                ]);
                const mixedMs = performance.now() - t;
                soakSamplesRef.current.push(mixedMs);
                updateRunRecord(recordId, {
                  status: "completed",
                  completedAt: Date.now(),
                  lastStatus: "providerless_completed_soak_mixed",
                  events: [`profile:soak_mixed`, `mixed_ms:${Math.round(mixedMs)}`],
                });
                const remainingMs = Math.max(0, soakStopAtRef.current - Date.now());
                if (remainingMs <= 0) {
                  stressActiveRef.current = false;
                }
              } else {
                await Promise.all([
                  api.runSessionCommand(sessionId, providerlessCommandRef.current),
                  api.getSession(sessionId),
                  api.listSessions({ pageSize: 5 }),
                ]);
              }
              updateRunRecord(recordId, {
                status: "completed",
                completedAt: Date.now(),
                lastStatus: `providerless_completed_${profile}`,
                events: [`command:${providerlessCommandRef.current}`, `profile:${profile}`],
              });
              addEventLog(`[worker ${workerId}] providerless cycle completed`);
            } catch (error) {
              const message = error instanceof Error ? error.message : String(error);
              if (providerlessProfileRef.current === "soak_mixed") {
                soakErrorsRef.current += 1;
              }
              updateRunRecord(recordId, {
                status: "errored",
                completedAt: Date.now(),
                error: message,
                lastStatus: "providerless_error",
              });
              addEventLog(`[worker ${workerId}] providerless error: ${message}`);
            }
            if (
              providerlessProfileRef.current === "soak_mixed" &&
              Date.now() >= soakStopAtRef.current
            ) {
              stressActiveRef.current = false;
            }
          } else {
            await attachAndTrack(workerId, sessionId, runId, recordId);
          }
          if (!stressActiveRef.current || providerlessProfileRef.current === "diagnostic_sweep") {
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, cycleDelayRef.current));
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        addEventLog(`worker ${workerId} failed: ${message}`);
      } finally {
        activeWorkersRef.current = Math.max(0, activeWorkersRef.current - 1);
        if (activeWorkersRef.current === 0) {
          stressActiveRef.current = false;
          setIsRunning(false);
          if (providerlessProfileRef.current === "soak_mixed") {
            const durationSec = Math.max(
              1,
              Math.round((Date.now() - soakStartedAtRef.current) / 1000)
            );
            const report = buildSoakReport(
              durationSec,
              soakSamplesRef.current,
              soakErrorsRef.current
            );
            setSoakReport(report);
          }
          if (scenarioRef.current !== "providerless") {
            void refreshComparisonData();
          }
          addEventLog("Stress lab run completed");
        }
      }
    },
    [
      addEventLog,
      addRunRecord,
      attachAndTrack,
      buildSoakReport,
      refreshComparisonData,
      updateRunRecord,
    ]
  );

  const startStress = useCallback(() => {
    const nextConcurrency = Math.min(64, parsePositiveInt(concurrencyInput, 4));
    const nextCycleDelay = Math.max(0, parsePositiveInt(cycleDelayInput, 1200));
    setConcurrency(nextConcurrency);
    setCycleDelay(nextCycleDelay);
    cycleDelayRef.current = nextCycleDelay;
    setDiagnosticStats({
      count: 0,
      commandMs: 0,
      getMs: 0,
      listMs: 0,
      mixedMs: 0,
    });
    if (providerlessRunnerRef.current === "server") {
      soakSamplesRef.current = [];
      soakErrorsRef.current = 0;
      setSoakReport("");
      setServerSoakMetrics({ completed: 0, errored: 0, avgLatency: 0 });
      soakStartedAtRef.current = Date.now();
      soakStopAtRef.current = soakStartedAtRef.current + soakSecondsRef.current * 1000;
      const streamUrl = api.getServerStressStreamUrl({
        scenario: scenarioRef.current,
        profile:
          providerlessProfileRef.current === "diagnostic_sweep"
            ? "mixed"
            : providerlessProfileRef.current,
        concurrency: nextConcurrency,
        durationSeconds: soakSecondsRef.current,
        cycleDelayMs: nextCycleDelay,
        command: providerlessCommandRef.current,
        prompt: prompt.trim(),
        filePath,
        inlineBody: scenarioRef.current === "shared_edit" ? "" : inlineBody,
      });
      serverSoakStreamRef.current?.close();
      const stream = new EventSource(streamUrl);
      serverSoakStreamRef.current = stream;
      stressActiveRef.current = true;
      setIsRunning(true);
      addEventLog("Stress lab started (server stream)");

      stream.addEventListener("open", () => {
        addEventLog("Server-side stream connected");
      });
      stream.addEventListener("progress", (event) => {
        try {
          const data = JSON.parse((event as MessageEvent).data || "{}") as {
            completed?: number;
            errors?: number;
            lastLatencyMs?: number;
            lastMixedMs?: number;
          };
          const sample =
            typeof data.lastLatencyMs === "number" && Number.isFinite(data.lastLatencyMs)
              ? data.lastLatencyMs
              : data.lastMixedMs;
          if (typeof sample === "number" && Number.isFinite(sample)) {
            soakSamplesRef.current.push(sample);
          }
          const avg =
            soakSamplesRef.current.length > 0
              ? soakSamplesRef.current.reduce((sum, value) => sum + value, 0) /
                soakSamplesRef.current.length
              : 0;
          setServerSoakMetrics({
            completed: data.completed ?? 0,
            errored: data.errors ?? 0,
            avgLatency: avg,
          });
          addEventLog(
            `[server] completed=${data.completed ?? 0} errors=${data.errors ?? 0} latency=${Math.round(
              sample ?? 0
            )}ms`
          );
        } catch {
          addEventLog("[server] progress update");
        }
      });
      stream.addEventListener("log", (event) => {
        try {
          const data = JSON.parse((event as MessageEvent).data || "{}") as {
            level?: string;
            message?: string;
          };
          addEventLog(`[server ${data.level || "info"}] ${data.message || "event"}`);
        } catch {
          addEventLog("[server] log event");
        }
      });
      stream.addEventListener("summary", (event) => {
        try {
          const data = JSON.parse((event as MessageEvent).data || "{}") as {
            report?: string;
            completed?: number;
            errors?: number;
            latency?: { avg?: number };
            mixed?: { avg?: number };
          };
          if (data.report) {
            setSoakReport(data.report);
          }
          setServerSoakMetrics({
            completed: data.completed ?? 0,
            errored: data.errors ?? 0,
            avgLatency: data.latency?.avg ?? data.mixed?.avg ?? 0,
          });
          addEventLog(
            `Server run completed: completed=${data.completed ?? 0} errors=${data.errors ?? 0}`
          );
        } catch {
          addEventLog("Server run completed");
        }
        if (scenarioRef.current !== "providerless") {
          void refreshComparisonData();
        }
        stream.close();
        serverSoakStreamRef.current = null;
        stressActiveRef.current = false;
        setIsRunning(false);
      });
      stream.onerror = () => {
        addEventLog("Server stream disconnected");
        stream.close();
        serverSoakStreamRef.current = null;
        stressActiveRef.current = false;
        setIsRunning(false);
      };
      return;
    }
    activeWorkersRef.current = nextConcurrency;
    stressActiveRef.current = true;
    setIsRunning(true);
    addEventLog("Stress lab started");
    for (let workerId = 1; workerId <= nextConcurrency; workerId += 1) {
      void runWorkerLoop(workerId);
    }
  }, [addEventLog, concurrencyInput, cycleDelayInput, filePath, inlineBody, prompt, runWorkerLoop]);

  const stopStress = useCallback(() => {
    stressActiveRef.current = false;
    setIsRunning(false);
    activeWorkersRef.current = 0;
    Object.values(streamRefs.current).forEach((holder) => holder.current?.close());
    streamRefs.current = {};
    serverSoakStreamRef.current?.close();
    serverSoakStreamRef.current = null;
    addEventLog("Stress lab stopped");
  }, [addEventLog]);

  const toggleStress = () => {
    if (isRunning) {
      stopStress();
    } else {
      startStress();
    }
  };

  const metrics = useMemo(() => {
    if (providerlessRunner === "server") {
      const total = serverSoakMetrics.completed + serverSoakMetrics.errored;
      const errorRatePercent = total > 0 ? (serverSoakMetrics.errored / total) * 100 : 0;
      return {
        completed: serverSoakMetrics.completed,
        errored: serverSoakMetrics.errored,
        active: isRunning ? concurrency : 0,
        total,
        averageLatency: serverSoakMetrics.avgLatency,
        averageFirstDelta: 0,
        errorRatePercent,
      };
    }

    const completed = runRecords.filter((record) => record.status === "completed");
    const errored = runRecords.filter((record) => record.status === "errored");
    const averageLatency = completed.length
      ? completed.reduce(
          (acc, record) => acc + ((record.completedAt || Date.now()) - record.startedAt),
          0
        ) / completed.length
      : 0;

    const firstDeltaTimes = completed
      .map((record) => record.firstDeltaAt && record.firstDeltaAt - record.startedAt)
      .filter((value): value is number => typeof value === "number");
    const averageFirstDelta =
      firstDeltaTimes.length > 0
        ? firstDeltaTimes.reduce((sum, value) => sum + value, 0) / firstDeltaTimes.length
        : 0;

    const active = runRecords.filter((record) => record.status === "running").length;

    return {
      completed: completed.length,
      errored: errored.length,
      active,
      total: runRecords.length,
      averageLatency,
      averageFirstDelta,
      errorRatePercent: runRecords.length > 0 ? (errored.length / runRecords.length) * 100 : 0,
    };
  }, [
    concurrency,
    isRunning,
    providerlessProfile,
    providerlessRunner,
    runRecords,
    scenarioMode,
    serverSoakMetrics,
  ]);

  useEffect(() => {
    const entry: MetricSnapshot = {
      timestamp: Date.now(),
      completed: metrics.completed,
      errored: metrics.errored,
      active: metrics.active,
      avgLatency: metrics.averageLatency,
      avgFirstDelta: metrics.averageFirstDelta,
    };
    setMetricHistory((prev) => {
      const next = [...prev, entry];
      return next.slice(-24);
    });
  }, [metrics]);

  useEffect(() => {
    if (!eventLogRef.current) return;
    eventLogRef.current.scrollTop = eventLogRef.current.scrollHeight;
  }, [eventLog]);

  useEffect(() => {
    void refreshComparisonData();
  }, [refreshComparisonData]);

  const workerSummaries = useMemo(() => {
    const map = new Map<number, StressRunRecord>();
    runRecords.forEach((record) => map.set(record.workerId, record));
    return Array.from(map.entries()).map(([workerId, record]) => ({ workerId, record }));
  }, [runRecords]);

  const latencyValues = useMemo(
    () =>
      runRecords
        .filter((record) => record.completedAt)
        .map((record) => record.completedAt! - record.startedAt),
    [runRecords]
  );

  const desiredOpencodeScenarioName = useMemo(() => {
    if (scenarioMode === "remote") return "remote_prompt";
    if (scenarioMode === "file") return "file_prompt";
    if (scenarioMode === "shared_edit") return "shared_edit_prompt";
    if (scenarioMode === "inline") return "inline_prompt";
    return null;
  }, [scenarioMode]);

  const selectedOpencodeScenarioName = useMemo(() => {
    if (!opencodeLatest || !desiredOpencodeScenarioName) return desiredOpencodeScenarioName;
    const exact = opencodeLatest.scenarios.some(
      (entry) => entry.name === desiredOpencodeScenarioName
    );
    if (exact) return desiredOpencodeScenarioName;
    if (desiredOpencodeScenarioName === "shared_edit_prompt") return "inline_prompt";
    return desiredOpencodeScenarioName;
  }, [desiredOpencodeScenarioName, opencodeLatest]);

  const selectedOpencodeScenario = useMemo<OpencodeBenchScenarioResult | null>(() => {
    if (!opencodeLatest || !selectedOpencodeScenarioName) return null;
    return (
      opencodeLatest.scenarios.find((entry) => entry.name === selectedOpencodeScenarioName) || null
    );
  }, [opencodeLatest, selectedOpencodeScenarioName]);

  const scenarioComparison = useMemo<ScenarioComparison | null>(() => {
    if (!selectedOpencodeScenario) return null;
    const tandemSamples = providerlessRunner === "server" ? soakSamplesRef.current : latencyValues;
    if (tandemSamples.length === 0) return null;
    const tandemAvgMs = tandemSamples.reduce((sum, value) => sum + value, 0) / tandemSamples.length;
    return {
      tandemAvgMs,
      tandemP95Ms: percentile(tandemSamples, 95),
      tandemP99Ms: percentile(tandemSamples, 99),
      opencodeAvgMs: selectedOpencodeScenario.avg_ms,
      opencodeP95Ms: selectedOpencodeScenario.p95_ms,
      opencodeP99Ms: selectedOpencodeScenario.p99_ms,
      opencodeErrors: selectedOpencodeScenario.errors,
    };
  }, [latencyValues, providerlessRunner, selectedOpencodeScenario]);

  const history30dScenarioAvg = useMemo(() => {
    if (!opencodeHistory || !selectedOpencodeScenarioName) return 0;
    const values: number[] = [];
    opencodeHistory.items.forEach((item) => {
      const found = item.scenarios.find((entry) => entry.name === selectedOpencodeScenarioName);
      if (found) values.push(found.avg_ms);
    });
    if (values.length === 0) return 0;
    return values.reduce((sum, value) => sum + value, 0) / values.length;
  }, [opencodeHistory, selectedOpencodeScenarioName]);

  useEffect(() => {
    return () => {
      stressActiveRef.current = false;
      Object.values(streamRefs.current).forEach((holder) => holder.current?.close());
      serverSoakStreamRef.current?.close();
    };
  }, []);

  return (
    <div className="flex h-full flex-col bg-gray-950 text-white">
      <div className="p-6 border-b border-gray-800 bg-gradient-to-r from-pink-600/30 to-purple-700/30">
        <div className="flex flex-col gap-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Bolt className="text-amber-300" /> Tandem Stress Lab
          </h1>
          <p className="text-sm text-gray-200 max-w-2xl">
            Floods the engine with concurrent asynchronous runs that call providers and tools so you
            can watch SSE throughput, latency, and the platform’s ability to keep many live runs
            streaming simultaneously.
          </p>
        </div>
      </div>

      <div className="p-6 space-y-6 overflow-auto flex-1">
        <section className="grid gap-4 md:grid-cols-2">
          <div className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-3">
            <label className="text-xs text-gray-400 uppercase tracking-wide">Concurrency</label>
            <input
              type="text"
              inputMode="numeric"
              value={concurrencyInput}
              onChange={(e) => setConcurrencyInput(e.target.value)}
              disabled={isRunning}
              className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white"
            />
            <label className="text-xs text-gray-400 uppercase tracking-wide">
              Cycle delay (ms)
            </label>
            <input
              type="text"
              inputMode="numeric"
              value={cycleDelayInput}
              onChange={(e) => setCycleDelayInput(e.target.value)}
              disabled={isRunning}
              className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white"
            />
          </div>
          <div className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-2">
            <label className="text-xs text-gray-400 uppercase tracking-wide">Prompt</label>
            <textarea
              rows={4}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white"
              disabled={isRunning}
            />
          </div>
        </section>

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={toggleStress}
            className={`px-6 py-3 font-semibold rounded-md flex items-center gap-2 border transition-colors ${
              isRunning
                ? "bg-rose-600 border-rose-500 hover:bg-rose-500"
                : "bg-emerald-600 border-emerald-500 hover:bg-emerald-500"
            }`}
          >
            {isRunning ? <Loader2 className="animate-spin" size={20} /> : <Play size={20} />}
            {isRunning ? "Stop Test" : "Start Flood"}
          </button>
          <div className="flex flex-col gap-1 text-xs text-gray-400">
            <span>Total runs recorded: {metrics.total}</span>
            <span>Active runs: {metrics.active}</span>
          </div>
        </div>

        <section className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold">Scenario</h2>
            <span className="text-xs text-gray-500">Switch workloads</span>
          </div>
          <div className="flex flex-wrap gap-2">
            {[
              { value: "remote", label: "Remote docs" },
              { value: "file", label: "Local file read" },
              { value: "inline", label: "Inline markdown" },
              { value: "shared_edit", label: "Shared 200-line edit" },
              { value: "providerless", label: "Providerless (Tandem-only smoke)" },
            ].map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() =>
                  setScenarioMode(
                    option.value as "remote" | "file" | "inline" | "shared_edit" | "providerless"
                  )
                }
                disabled={isRunning}
                className={`px-3 py-1 rounded-full text-xs border ${
                  scenarioMode === option.value
                    ? "border-emerald-500 bg-emerald-500/20 text-emerald-200"
                    : "border-gray-700 text-gray-300 hover:border-white hover:text-white"
                }`}
              >
                {option.label}
              </button>
            ))}
          </div>
          <div className="space-y-1">
            <label className="text-[11px] text-gray-400">Execution runner</label>
            <div className="flex flex-wrap gap-2">
              {[
                { value: "server", label: "Server-side stream" },
                { value: "browser", label: "Browser-side" },
              ].map((option) => (
                <button
                  key={option.value}
                  type="button"
                  onClick={() => setProviderlessRunner(option.value as "browser" | "server")}
                  disabled={isRunning}
                  className={`px-3 py-1 rounded-full text-xs border ${
                    providerlessRunner === option.value
                      ? "border-emerald-500 bg-emerald-500/20 text-emerald-200"
                      : "border-gray-700 text-gray-300 hover:border-white hover:text-white"
                  }`}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>
          {(providerlessRunner === "server" ||
            (scenarioMode === "providerless" && providerlessProfile === "soak_mixed")) && (
            <div className="space-y-1">
              <label className="text-[11px] text-gray-400">Test duration (seconds)</label>
              <input
                type="text"
                inputMode="numeric"
                value={soakSecondsInput}
                onChange={(e) => setSoakSecondsInput(e.target.value)}
                className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white text-sm"
                placeholder="60"
                disabled={isRunning}
              />
            </div>
          )}
          {scenarioMode === "file" && (
            <div className="space-y-1">
              <label className="text-[11px] text-gray-400">File path</label>
              <input
                type="text"
                value={filePath}
                onChange={(e) => setFilePath(e.target.value)}
                className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white text-sm"
                placeholder="/srv/tandem/docs/overview.md"
                disabled={isRunning}
              />
            </div>
          )}
          {scenarioMode === "inline" && (
            <div className="space-y-1">
              <label className="text-[11px] text-gray-400">Inline markdown snippet</label>
              <textarea
                rows={3}
                value={inlineBody}
                onChange={(e) => setInlineBody(e.target.value)}
                className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white text-sm"
                disabled={isRunning}
              />
            </div>
          )}
          {scenarioMode === "shared_edit" && (
            <div className="space-y-1">
              <div className="flex items-center justify-between">
                <label className="text-[11px] text-gray-400">
                  Shared markdown fixture (200 lines)
                </label>
                <button
                  type="button"
                  onClick={() => setShowSharedFixture((prev) => !prev)}
                  className="rounded border border-gray-700 px-2 py-1 text-[11px] text-gray-300"
                >
                  {showSharedFixture ? "Collapse fixture" : "Expand fixture"}
                </button>
              </div>
              {showSharedFixture && (
                <textarea
                  rows={8}
                  readOnly
                  value={sharedEditFixture}
                  className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-gray-300 text-xs font-mono"
                />
              )}
            </div>
          )}
          {scenarioMode === "providerless" && (
            <div className="space-y-3">
              <div className="space-y-1">
                <label className="text-[11px] text-gray-400">Providerless profile</label>
                <div className="flex flex-wrap gap-2">
                  {[
                    { value: "command_only", label: "Command only" },
                    { value: "get_session_only", label: "getSession only" },
                    { value: "list_sessions_only", label: "listSessions only" },
                    { value: "mixed", label: "Mixed (all 3)" },
                    { value: "diagnostic_sweep", label: "Diagnostic sweep (one-shot)" },
                    { value: "soak_mixed", label: "Soak mixed (timed)" },
                  ].map((option) => (
                    <button
                      key={option.value}
                      type="button"
                      onClick={() =>
                        setProviderlessProfile(
                          option.value as
                            | "command_only"
                            | "get_session_only"
                            | "list_sessions_only"
                            | "mixed"
                            | "diagnostic_sweep"
                            | "soak_mixed"
                        )
                      }
                      disabled={isRunning}
                      className={`px-3 py-1 rounded-full text-xs border ${
                        providerlessProfile === option.value
                          ? "border-sky-500 bg-sky-500/20 text-sky-200"
                          : "border-gray-700 text-gray-300 hover:border-white hover:text-white"
                      }`}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </div>
              <div className="space-y-1">
                <label className="text-[11px] text-gray-400">Providerless command</label>
                <input
                  type="text"
                  value={providerlessCommand}
                  onChange={(e) => setProviderlessCommand(e.target.value)}
                  className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white text-sm"
                  placeholder="pwd"
                  disabled={
                    isRunning ||
                    (providerlessProfile !== "command_only" &&
                      providerlessProfile !== "mixed" &&
                      providerlessProfile !== "soak_mixed")
                  }
                />
              </div>
            </div>
          )}
          <p className="text-[11px] text-gray-500 whitespace-pre-wrap">{basePromptPreview}</p>
          <p className="text-[11px] text-gray-500">
            Comparison mode includes only shared LLM scenarios (`remote`, `file`, `inline`,
            `shared_edit`). Providerless is a Tandem internal smoke/perf test and is excluded from
            OpenCode deltas.
          </p>
        </section>

        <section className="grid gap-4 md:grid-cols-5">
          <Card label="Completed" value={metrics.completed} icon={<Sparkles />} />
          <Card label="Errored" value={metrics.errored} icon={<Activity />} />
          <Card
            label="Avg latency"
            value={`${Math.round(metrics.averageLatency)} ms`}
            icon={<Bolt />}
          />
          <Card
            label="Provider error rate"
            value={`${metrics.errorRatePercent.toFixed(1)}%`}
            icon={<Activity />}
          />
          <Card
            label="Avg first delta"
            value={`${Math.round(metrics.averageFirstDelta)} ms`}
            icon={<Sparkles />}
          />
        </section>

        <section className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold">Event log</h2>
            <span className="text-[11px] text-gray-500">Live activity stream (auto-scroll)</span>
          </div>
          <div
            ref={eventLogRef}
            className="max-h-44 overflow-y-auto space-y-1 rounded border border-dashed border-gray-800 bg-gray-950/60 p-2 text-[11px] text-gray-300 font-mono"
          >
            {eventLog.length === 0 ? (
              <p className="text-gray-500">Waiting for events...</p>
            ) : (
              eventLog.slice(-64).map((line, index) => (
                <p
                  key={`${line}-${index}`}
                  className={`rounded px-2 py-0.5 ${
                    line.includes("errored")
                      ? "bg-rose-900/40 text-rose-200"
                      : "bg-emerald-900/20 text-emerald-200"
                  }`}
                >
                  {line}
                </p>
              ))
            )}
          </div>
        </section>

        <section className="grid gap-4 lg:grid-cols-2">
          <div className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-2">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-gray-300">Throughput</h2>
              <span className="text-[11px] text-gray-500">
                Completed vs. errored runs over time
              </span>
            </div>
            <LineChart
              series={[
                {
                  data: metricHistory.map((entry) => entry.completed),
                  color: "#34d399",
                  label: "Completed",
                },
                {
                  data: metricHistory.map((entry) => entry.errored),
                  color: "#f87171",
                  label: "Errored",
                },
              ]}
            />
          </div>
          <div className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-2">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-gray-300">Latency Trend</h2>
              <span className="text-[11px] text-gray-500">Avg response + first delta time</span>
            </div>
            <LineChart
              series={[
                {
                  data: metricHistory.map((entry) => entry.avgLatency),
                  color: "#38bdf8",
                  label: "Avg latency",
                },
                {
                  data: metricHistory.map((entry) => entry.avgFirstDelta),
                  color: "#facc15",
                  label: "First delta",
                },
              ]}
            />
          </div>
        </section>

        <section className="rounded-lg border border-sky-800/60 bg-sky-950/30 p-4 space-y-3">
          <div className="flex items-center justify-between gap-3">
            <h2 className="text-sm font-semibold text-sky-200">Tandem vs OpenCode (latest)</h2>
            <button
              type="button"
              onClick={() => void refreshComparisonData()}
              disabled={comparisonLoading}
              className="rounded border border-sky-700 px-2 py-1 text-xs text-sky-100 disabled:opacity-40"
            >
              {comparisonLoading ? "Refreshing..." : "Refresh"}
            </button>
          </div>
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4 text-xs">
            <div className="rounded border border-gray-700 p-2">
              OpenCode health: <span className="font-semibold">{opencodeHealth}</span>
            </div>
            <div className="rounded border border-gray-700 p-2">
              Latest run:{" "}
              <span className="font-semibold">
                {opencodeLatest ? new Date(opencodeLatest.timestamp_utc).toLocaleString() : "n/a"}
              </span>
            </div>
            <div className="rounded border border-gray-700 p-2">
              30d points: <span className="font-semibold">{opencodeHistory?.count ?? 0}</span>
            </div>
            <div className="rounded border border-gray-700 p-2">
              Scenario:{" "}
              <span className="font-semibold">
                {selectedOpencodeScenarioName ?? "n/a"}
                {desiredOpencodeScenarioName === "shared_edit_prompt" &&
                selectedOpencodeScenarioName === "inline_prompt"
                  ? " (fallback)"
                  : ""}
              </span>
            </div>
          </div>
          {comparisonError && (
            <p className="text-xs text-rose-300">Comparison load failed: {comparisonError}</p>
          )}
          {scenarioMode === "providerless" ? (
            <p className="text-xs text-gray-400">
              OpenCode does not support providerless scenario. Switch to remote/file/inline for
              direct comparison.
            </p>
          ) : scenarioComparison ? (
            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4 text-sm">
              <div className="rounded border border-gray-700 p-2">
                Tandem avg/p95: {Math.round(scenarioComparison.tandemAvgMs)} /{" "}
                {Math.round(scenarioComparison.tandemP95Ms)}ms
              </div>
              <div className="rounded border border-gray-700 p-2">
                OpenCode avg/p95: {Math.round(scenarioComparison.opencodeAvgMs)} /{" "}
                {Math.round(scenarioComparison.opencodeP95Ms)}ms
              </div>
              <div className="rounded border border-gray-700 p-2">
                Avg delta (T - O):{" "}
                <span
                  className={
                    scenarioComparison.tandemAvgMs <= scenarioComparison.opencodeAvgMs
                      ? "text-emerald-300"
                      : "text-rose-300"
                  }
                >
                  {Math.round(scenarioComparison.tandemAvgMs - scenarioComparison.opencodeAvgMs)}ms
                </span>
              </div>
              <div className="rounded border border-gray-700 p-2">
                OpenCode errors/latest: {scenarioComparison.opencodeErrors} (30d avg:{" "}
                {Math.round(history30dScenarioAvg)}ms)
              </div>
            </div>
          ) : (
            <p className="text-xs text-gray-400">
              Run a Tandem LLM scenario (`remote` / `file` / `inline`) to compute deltas. Comparison
              data auto-refreshes after each completed test.
            </p>
          )}
        </section>

        {scenarioMode === "providerless" && providerlessProfile === "diagnostic_sweep" && (
          <section className="rounded-lg border border-sky-700/60 bg-sky-900/10 p-4 space-y-2">
            <h2 className="text-sm font-semibold text-sky-200">
              One-Shot Sweep Averages ({diagnosticStats.count} workers)
            </h2>
            {diagnosticStats.count > 0 ? (
              <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4 text-sm">
                <div className="rounded border border-gray-700 p-2">
                  command: {Math.round(diagnosticStats.commandMs / diagnosticStats.count)}ms
                </div>
                <div className="rounded border border-gray-700 p-2">
                  getSession: {Math.round(diagnosticStats.getMs / diagnosticStats.count)}ms
                </div>
                <div className="rounded border border-gray-700 p-2">
                  listSessions: {Math.round(diagnosticStats.listMs / diagnosticStats.count)}ms
                </div>
                <div className="rounded border border-gray-700 p-2">
                  mixed: {Math.round(diagnosticStats.mixedMs / diagnosticStats.count)}ms
                </div>
              </div>
            ) : (
              <p className="text-xs text-sky-200/70">Run the sweep once to populate averages.</p>
            )}
          </section>
        )}

        {(providerlessRunner === "server" ||
          (scenarioMode === "providerless" && providerlessProfile === "soak_mixed")) && (
          <section className="rounded-lg border border-emerald-700/60 bg-emerald-900/10 p-4 space-y-2">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-emerald-200">
                {providerlessRunner === "server"
                  ? "Server Stream Result (Copy/Paste)"
                  : "Soak Result (Copy/Paste)"}
              </h2>
              <button
                type="button"
                onClick={() => {
                  if (!soakReport.trim()) return;
                  void navigator.clipboard.writeText(soakReport);
                }}
                disabled={!soakReport.trim()}
                className="rounded border border-gray-700 px-2 py-1 text-xs text-gray-200 disabled:opacity-40"
              >
                Copy Report
              </button>
            </div>
            <textarea
              value={soakReport}
              readOnly
              rows={10}
              placeholder="Run a timed soak test to generate a report."
              className="w-full rounded border border-gray-700 bg-gray-950 px-3 py-2 text-white text-xs font-mono"
            />
          </section>
        )}

        <section className="grid gap-4 lg:grid-cols-2">
          <div className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-3">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-gray-300">Latency Heatmap</h2>
              <span className="text-[11px] text-gray-500">Bucketed completion times</span>
            </div>
            <LatencyHeatmap latencies={latencyValues} />
          </div>
          <div className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-3">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-gray-300">Worker health</h2>
              <span className="text-[11px] text-gray-500">Latest run per worker</span>
            </div>
            <WorkerMatrix summaries={workerSummaries} />
          </div>
        </section>

        <section className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold">Workers</h2>
            <span className="text-xs text-gray-400">Last run clear so far</span>
          </div>
          <div className="grid gap-4 md:grid-cols-2">
            {workerSummaries.map(({ workerId, record }) => (
              <div
                key={workerId}
                className="rounded border border-gray-800 bg-gray-950/60 p-3 space-y-1"
              >
                <div className="flex items-center justify-between text-sm">
                  <span className="font-semibold">Worker {workerId}</span>
                  <span
                    className={`text-[11px] font-mono px-2 py-0.5 rounded-full text-white ${
                      record.status === "running"
                        ? "bg-emerald-600"
                        : record.status === "errored"
                          ? "bg-rose-500"
                          : "bg-blue-600"
                    }`}
                  >
                    {record.status}
                  </span>
                </div>
                <p className="text-[11px] text-gray-400">
                  {record.runId.substring(0, 8)} @ {new Date(record.startedAt).toLocaleTimeString()}
                </p>
                <p className="text-[11px] text-gray-400">
                  latency:{" "}
                  {record.completedAt ? `${record.completedAt - record.startedAt} ms` : "–"}
                </p>
                {record.firstDeltaAt && (
                  <p className="text-[11px] text-gray-400">
                    delta in {record.firstDeltaAt - record.startedAt} ms
                  </p>
                )}
                {record.status === "errored" && record.error && (
                  <p className="text-[11px] text-rose-300">Error: {record.error}</p>
                )}
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
};

interface CardProps {
  label: string;
  value: React.ReactNode;
  icon: React.ReactNode;
}

const Card: React.FC<CardProps> = ({ label, value, icon }) => (
  <div className="rounded-lg border border-gray-800 bg-gray-900/80 p-4 flex items-center gap-4">
    <div className="text-gray-400">{icon}</div>
    <div className="flex flex-col">
      <span className="text-2xl font-bold leading-none">{value}</span>
      <span className="text-xs uppercase tracking-wide text-gray-500">{label}</span>
    </div>
  </div>
);

interface LineSeries {
  data: number[];
  color: string;
  label: string;
}

const LineChart: React.FC<{ series: LineSeries[]; width?: number; height?: number }> = ({
  series,
  width = 320,
  height = 110,
}) => {
  const flatValues = series.flatMap((entry) => entry.data);
  const computedMax = flatValues.length > 0 ? Math.max(...flatValues) : 0;
  const maxValue = computedMax > 0 ? computedMax : 1;
  const minValue = 0;
  if (series.every((entry) => entry.data.length === 0)) {
    return <p className="text-xs text-gray-500">Waiting for data…</p>;
  }

  const points = (data: number[]) => {
    const len = data.length;
    if (len === 0) return "";
    return data
      .map((value, index) => {
        const x = len === 1 ? width / 2 : (index / (len - 1)) * (width - 16) + 8;
        const normalized = Math.min(Math.max(value - minValue, 0), maxValue) / maxValue;
        const y = height - normalized * (height - 16) - 8;
        return `${x},${y}`;
      })
      .join(" ");
  };

  return (
    <div className="w-full">
      <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-28">
        {series.map((entry) => (
          <polyline
            key={entry.label}
            points={points(entry.data)}
            fill="none"
            stroke={entry.color}
            strokeWidth={2}
            strokeLinecap="round"
            opacity={entry.data.length === 0 ? 0.4 : 1}
          />
        ))}
        <rect x="0" y="0" width={width} height={height} fill="transparent" />
      </svg>
      <div className="flex items-center gap-4 text-[11px] text-gray-400 flex-wrap mt-1">
        {series.map((entry) => (
          <span key={entry.label} className="flex items-center gap-1">
            <span className="h-2 w-2 rounded-full" style={{ backgroundColor: entry.color }} />
            {entry.label}
          </span>
        ))}
      </div>
    </div>
  );
};

const LatencyHeatmap: React.FC<{ latencies: number[] }> = ({ latencies }) => {
  const buckets = [200, 400, 800, 1200, 2000, 4000];
  const labels = ["<200", "200-400", "400-800", "800-1200", "1200-2000", "2000+"];
  const counts = buckets.map((threshold, index) => {
    const min = index === 0 ? 0 : buckets[index - 1];
    const max = threshold;
    return latencies.filter(
      (value) => value >= min && (index === buckets.length - 1 ? true : value < max)
    ).length;
  });
  const maxCount = Math.max(...counts, 1);

  return (
    <div className="grid grid-cols-3 gap-3">
      {counts.map((count, index) => (
        <div key={labels[index]} className="flex flex-col items-center text-[11px] text-gray-300">
          <div
            className="h-14 w-full rounded-md bg-gradient-to-b from-emerald-500/60 to-black"
            style={{ opacity: 0.15 + (count / maxCount) * 0.7 }}
          />
          <span className="mt-2 font-semibold">{count}</span>
          <span className="text-gray-500">{labels[index]}</span>
        </div>
      ))}
    </div>
  );
};

const WorkerMatrix: React.FC<{
  summaries: { workerId: number; record: StressRunRecord }[];
}> = ({ summaries }) => (
  <div className="grid gap-3 sm:grid-cols-2">
    {summaries.map(({ workerId, record }) => (
      <div key={workerId} className="rounded border border-gray-800 bg-gray-950/50 p-3">
        <div className="flex items-center justify-between text-sm">
          <span className="font-semibold">Worker {workerId}</span>
          <span
            className={`text-[11px] font-mono px-2 py-0.5 rounded-full text-white ${
              record.status === "running"
                ? "bg-emerald-600"
                : record.status === "errored"
                  ? "bg-rose-500"
                  : "bg-blue-600"
            }`}
          >
            {record.status}
          </span>
        </div>
        <p className="text-[11px] text-gray-400">
          Last run {record.runId.substring(0, 8)} @{" "}
          {new Date(record.startedAt).toLocaleTimeString()}
        </p>
        <p className="text-[11px] text-gray-400">
          {record.events.slice(-1)[0] || "No events yet."}
        </p>
      </div>
    ))}
  </div>
);
