import React, { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { Loader2, Users } from "lucide-react";
import { handleCommonRunEvent } from "../utils/liveEventDebug";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize from "rehype-sanitize";

interface AgentResponse {
  persona: string;
  response: string;
  loading: boolean;
  error: string | null;
  logs: string[];
}

const personas = [
  {
    name: "The Critic",
    prompt:
      "You are a rigorous critic. Identify failure modes, blind spots, and hidden costs. End with a risk score (1-10): ",
  },
  {
    name: "The Optimist",
    prompt:
      "You are an ambitious optimist. Identify upside, leverage points, and breakout potential. End with an upside score (1-10): ",
  },
  {
    name: "The Engineer",
    prompt:
      "You are a pragmatic software engineer. Evaluate technical feasibility, architecture choices, and operational risks. End with a buildability score (1-10): ",
  },
];

const SWARM_SESSION_KEY = "tandem_portal_swarm_sessions";

export const SwarmDashboard: React.FC = () => {
  const [query, setQuery] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const logContainerRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const [agents, setAgents] = useState<AgentResponse[]>(
    personas.map((p) => ({
      persona: p.name,
      response: "",
      loading: false,
      error: null,
      logs: ["Idle. Waiting for run."],
    }))
  );
  const appendAgentLog = (index: number, message: string) => {
    const stamp = new Date().toLocaleTimeString();
    setAgents((prev) => {
      const updated = [...prev];
      const current = updated[index];
      if (!current) return prev;
      updated[index] = {
        ...current,
        logs: [...current.logs, `${stamp} | ${message}`].slice(-40),
      };
      return updated;
    });
  };

  const loadAgentResponse = async (personaName: string, sessionId: string) => {
    try {
      const messages = await api.getSessionMessages(sessionId);
      const lastAssistant = [...messages].reverse().find((m) => m.info?.role === "assistant");
      const finalText = (lastAssistant?.parts || [])
        .filter((p) => p.type === "text" && p.text)
        .map((p) => p.text)
        .join("\n")
        .trim();
      if (finalText) {
        setAgents((prev) => {
          const updated = [...prev];
          const idx = personas.findIndex((p) => p.name === personaName);
          if (idx >= 0) {
            updated[idx].response = finalText;
            updated[idx].loading = false;
            updated[idx].error = null;
            updated[idx].logs = [
              ...updated[idx].logs,
              `${new Date().toLocaleTimeString()} | Loaded final response (${finalText.length} chars)`,
            ].slice(-40);
          }
          return updated;
        });
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setAgents((prev) => {
        const updated = [...prev];
        const idx = personas.findIndex((p) => p.name === personaName);
        if (idx >= 0) {
          updated[idx].loading = false;
          updated[idx].error = errorMessage || "Failed to load saved session";
        }
        return updated;
      });
    }
  };

  useEffect(() => {
    const raw = localStorage.getItem(SWARM_SESSION_KEY);
    if (!raw) return;
    try {
      const parsed = JSON.parse(raw) as Record<string, string>;
      void Promise.all(
        Object.entries(parsed).map(([personaName, sid]) => loadAgentResponse(personaName, sid))
      );
    } catch (err) {
      console.error("Failed to parse swarm session map", err);
    }
  }, []);

  useEffect(() => {
    const id = window.requestAnimationFrame(() => {
      agents.forEach((agent) => {
        const el = logContainerRefs.current[agent.persona];
        if (!el) return;
        el.scrollTop = el.scrollHeight;
      });
    });
    return () => window.cancelAnimationFrame(id);
  }, [agents]);

  const handleStart = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim() || isRunning) return;

    setIsRunning(true);

    // Reset state
    setAgents(
      personas.map((p) => ({
        persona: p.name,
        response: "",
        loading: true,
        error: null,
        logs: [`${new Date().toLocaleTimeString()} | Run queued`],
      }))
    );

    // Fan out requests to 3 distinct agent sessions in parallel
    await Promise.all(
      personas.map(async (persona, index) => {
        try {
          // 1. Create a dedicated session for this persona
          const sessionId = await api.createSession(`Swarm: ${persona.name}`);
          appendAgentLog(index, `Session created: ${sessionId.slice(0, 8)}...`);
          const raw = localStorage.getItem(SWARM_SESSION_KEY);
          const currentMap = raw ? (JSON.parse(raw) as Record<string, string>) : {};
          currentMap[persona.name] = sessionId;
          localStorage.setItem(SWARM_SESSION_KEY, JSON.stringify(currentMap));

          // 2. Start the run
          const fullPrompt = `${persona.prompt}\n\n${query}`;
          const { runId } = await api.startAsyncRun(sessionId, fullPrompt);
          appendAgentLog(index, `Run started: ${runId.slice(0, 8)}...`);

          // 3. Listen to the event stream
          const eventSource = new EventSource(api.getEventStreamUrl(sessionId, runId));
          let finalized = false;
          let sawRunEvent = false;
          let sawFirstDelta = false;
          const startedAt = Date.now();
          const watchdog = window.setTimeout(async () => {
            if (finalized || sawRunEvent) return;
            try {
              const runState = await api.getActiveRun(sessionId);
              if (!runState?.active) {
                appendAgentLog(index, "Run inactive before live events arrived.");
                setAgents((prev) => {
                  const updated = [...prev];
                  updated[index].loading = false;
                  if (!updated[index].response) {
                    updated[index].error =
                      "Run ended before live events arrived. Check provider key/model and logs.";
                  }
                  return updated;
                });
                void finalizeAgent();
                return;
              }
              setAgents((prev) => {
                const updated = [...prev];
                if (!updated[index].response) {
                  updated[index].response = "[Run active, waiting for live deltas...]";
                }
                return updated;
              });
              appendAgentLog(index, "Run active, waiting for first delta.");
            } catch {
              appendAgentLog(index, "No live events yet; run state poll failed.");
              setAgents((prev) => {
                const updated = [...prev];
                updated[index].error = "No live events yet and failed to query run state.";
                return updated;
              });
            }
          }, 4000);
          const runStatePoll = window.setInterval(async () => {
            if (finalized) return;
            try {
              const runState = await api.getActiveRun(sessionId);
              if (!runState?.active) {
                appendAgentLog(index, "Run became inactive during poll.");
                setAgents((prev) => {
                  const updated = [...prev];
                  updated[index].loading = false;
                  if (!updated[index].response) {
                    updated[index].error = "Run became inactive before a terminal stream event.";
                  }
                  return updated;
                });
                void finalizeAgent();
              }
            } catch {
              // Non-fatal polling failure while stream remains attached.
            }
          }, 5000);

          const finalizeAgent = async () => {
            if (finalized) return;
            finalized = true;
            window.clearTimeout(watchdog);
            window.clearInterval(runStatePoll);
            appendAgentLog(index, `Finalizing run (${Date.now() - startedAt}ms elapsed)`);
            await loadAgentResponse(persona.name, sessionId);
            eventSource.close();
          };

          eventSource.onopen = () => {
            appendAgentLog(index, "SSE stream connected.");
          };

          eventSource.onmessage = (evt) => {
            try {
              const data = JSON.parse(evt.data);
              if (data.type !== "server.connected" && data.type !== "engine.lifecycle.ready") {
                sawRunEvent = true;
              }

              const handledCommon = handleCommonRunEvent(
                data,
                ({ content }) => {
                  appendAgentLog(index, `System: ${content}`);
                  if (/engine error/i.test(content)) {
                    setAgents((prev) => {
                      const updated = [...prev];
                      updated[index].loading = false;
                      updated[index].error = content;
                      return updated;
                    });
                  }
                },
                (status) => {
                  appendAgentLog(index, `Run status: ${status}`);
                  if (
                    status === "completed" ||
                    status === "failed" ||
                    status === "error" ||
                    status === "cancelled" ||
                    status === "canceled" ||
                    status === "timeout" ||
                    status === "timed_out"
                  ) {
                    void finalizeAgent();
                  }
                }
              );
              if (handledCommon) return;

              if (
                data.type === "message.part.updated" &&
                data.properties?.part?.type === "text" &&
                data.properties?.delta
              ) {
                if (!sawFirstDelta) {
                  sawFirstDelta = true;
                  appendAgentLog(index, `First delta received (${Date.now() - startedAt}ms).`);
                }
                setAgents((prev) => {
                  const updated = [...prev];
                  updated[index].response += data.properties.delta;
                  return updated;
                });
              } else if (
                data.type === "run.status.updated" &&
                (data.properties?.status === "completed" || data.properties?.status === "failed")
              ) {
                appendAgentLog(index, `run.status.updated: ${String(data.properties?.status)}`);
                void finalizeAgent();
              } else if (
                data.type === "session.run.finished" &&
                (data.properties?.status === "completed" || data.properties?.status === "failed")
              ) {
                appendAgentLog(index, `session.run.finished: ${String(data.properties?.status)}`);
                void finalizeAgent();
              } else if (data.type === "session.error") {
                appendAgentLog(
                  index,
                  `session.error: ${String(data.properties?.error?.message || "unknown error")}`
                );
                setAgents((prev) => {
                  const updated = [...prev];
                  updated[index].loading = false;
                  updated[index].error =
                    data.properties?.error?.message || "Engine error during swarm run.";
                  return updated;
                });
                void finalizeAgent();
              }
            } catch {
              appendAgentLog(index, "Failed to parse stream event payload.");
              setAgents((prev) => {
                const updated = [...prev];
                updated[index].loading = false;
                updated[index].error = "Failed to parse stream event payload.";
                return updated;
              });
              void finalizeAgent();
            }
          };

          eventSource.onerror = () => {
            window.clearTimeout(watchdog);
            window.clearInterval(runStatePoll);
            appendAgentLog(index, "SSE stream disconnected.");
            setAgents((prev) => {
              const updated = [...prev];
              updated[index].loading = false;
              updated[index].error = "Stream disconnected";
              return updated;
            });
            eventSource.close();
          };
        } catch (err) {
          const errorMessage = err instanceof Error ? err.message : String(err);
          appendAgentLog(index, `Failed to start agent: ${errorMessage || "unknown error"}`);
          setAgents((prev) => {
            const updated = [...prev];
            updated[index].loading = false;
            updated[index].error = errorMessage || "Failed to start agent";
            return updated;
          });
        }
      })
    );

    setIsRunning(false);
  };

  return (
    <div className="flex flex-col h-full bg-gray-950 p-3 sm:p-4 lg:p-6">
      <div className="mb-6">
        <h2 className="text-2xl font-bold text-white flex items-center gap-2">
          <Users className="text-purple-500" />
          Parallel Agent Swarm
        </h2>
        <p className="text-gray-400 mt-1">
          Submit an idea. Watch three distinct AI personas evaluate it concurrently.
        </p>
      </div>

      <form onSubmit={handleStart} className="flex flex-col gap-3 sm:flex-row sm:gap-4 mb-6">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="E.g., Build an AI incident co-pilot for on-call teams that reads logs, proposes fixes, and drafts status updates."
          className="flex-1 bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 text-white focus:outline-none focus:ring-2 focus:ring-purple-500"
          disabled={isRunning}
        />
        <button
          type="submit"
          disabled={isRunning || !query.trim()}
          className="bg-purple-600 hover:bg-purple-700 disabled:opacity-50 text-white px-6 py-3 rounded-lg font-medium flex items-center justify-center gap-2 transition-colors sm:w-auto"
        >
          {isRunning ? <Loader2 className="animate-spin" size={20} /> : <Users size={20} />}
          {isRunning ? "Deploying Swarm..." : "Run Swarm Review"}
        </button>
      </form>

      <div className="flex-1 grid grid-cols-1 md:grid-cols-3 gap-6">
        {agents.map((agent, i) => (
          <div
            key={i}
            className="bg-gray-900 border border-gray-800 rounded-lg flex flex-col shadow-inner"
          >
            <div className="bg-gray-800/50 border-b border-gray-800 px-4 py-3 flex items-center justify-between">
              <span className="font-semibold text-gray-200">{agent.persona}</span>
              {agent.loading && <Loader2 size={16} className="text-purple-500 animate-spin" />}
            </div>
            <div className="border-b border-gray-800 bg-gray-950/70 px-4 py-2">
              <div className="text-[10px] uppercase tracking-wide text-gray-500 mb-1">Live log</div>
              <div
                ref={(el) => {
                  logContainerRefs.current[agent.persona] = el;
                }}
                className="max-h-24 overflow-y-auto rounded border border-gray-800 bg-gray-950 px-2 py-1 text-[11px] font-mono text-gray-400"
              >
                {agent.logs.length > 0 ? (
                  agent.logs.map((line, idx) => (
                    <div key={`${agent.persona}-log-${idx}`} className="truncate">
                      {line}
                    </div>
                  ))
                ) : (
                  <div>Waiting for logs...</div>
                )}
              </div>
            </div>
            <div className="flex-1 p-4 overflow-y-auto text-sm text-gray-300 leading-relaxed">
              {agent.response ? (
                <div className="prose prose-invert prose-sm max-w-none prose-pre:bg-gray-900/70 prose-pre:border prose-pre:border-gray-700 prose-a:text-blue-400 hover:prose-a:text-blue-300">
                  <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>
                    {agent.response}
                  </ReactMarkdown>
                </div>
              ) : agent.loading ? (
                <span className="text-gray-600 italic">Thinking...</span>
              ) : (
                <span className="text-gray-600 italic">Waiting for input.</span>
              )}
              {agent.error && <p className="text-red-400 mt-2">{agent.error}</p>}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
