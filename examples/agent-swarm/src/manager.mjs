import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { execFileSync } from "node:child_process";
import { createApi, loadDotEnv } from "./tandem_api.mjs";
import { applyEvent, createMapperContext } from "./event_mapper.mjs";
import { currentResourceKey, loadRegistry, saveRegistry } from "./swarm_registry.mjs";
import { TASK_STATUS } from "./swarm_types.mjs";
import { seedTasks } from "./swarm_orchestrator.mjs";

const here = path.dirname(new URL(import.meta.url).pathname);
const root = path.resolve(here, "..");
loadDotEnv(path.join(root, ".env"));

const BASE_URL = process.env.TANDEM_BASE_URL || "http://127.0.0.1:39731";
const API_TOKEN = process.env.TANDEM_API_TOKEN || "";
const MAX_TASKS = Number(process.env.SWARM_MAX_TASKS || "3");
const MODEL_PROVIDER = String(process.env.SWARM_MODEL_PROVIDER || "").trim();
const MODEL_ID = String(process.env.SWARM_MODEL_ID || "").trim();
const MCP_SERVERS = String(process.env.SWARM_MCP_SERVERS || "")
  .split(",")
  .map((v) => v.trim())
  .filter(Boolean);
const OBJECTIVE =
  process.argv.slice(2).join(" ") ||
  process.env.SWARM_OBJECTIVE ||
  "Ship a small feature end-to-end";

const api = createApi(BASE_URL, API_TOKEN);

function readPrompt(name) {
  return fs.readFileSync(path.join(root, "agents", name), "utf8");
}

function parseTasksFromText(text) {
  try {
    return JSON.parse(text)?.tasks || [];
  } catch {
    const m = text.match(/\{[\s\S]*\}/);
    if (m) {
      try {
        return JSON.parse(m[0])?.tasks || [];
      } catch {
        return [];
      }
    }
    return [];
  }
}

function extractAssistantTextFromSession(sessionWire) {
  const messages = sessionWire?.messages || [];
  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    const role = msg?.info?.role;
    if (role !== "assistant") continue;
    const parts = msg.parts || [];
    const text = parts
      .map((p) => p.text)
      .filter(Boolean)
      .join("\n");
    if (text) return text;
  }
  return "";
}

function runCreateWorktree(taskId) {
  const script = path.join(root, "scripts", "create_worktree.sh");
  const out = execFileSync(script, [taskId], { encoding: "utf8" });
  const rows = {};
  for (const rawLine of out.trim().split(/\r?\n/)) {
    const line = String(rawLine || "").trim();
    if (!line) continue;
    const idx = line.indexOf("=");
    if (idx <= 0) continue;
    const key = line.slice(0, idx).trim();
    const value = line.slice(idx + 1).trim();
    if (!key) continue;
    rows[key] = value;
  }
  const worktreePath = String(rows.worktreePath || rows.WORKTREE_PATH || "").trim();
  const branch = String(rows.branch || rows.BRANCH || "").trim();
  if (!worktreePath || !branch) {
    throw new Error(`create_worktree.sh returned invalid output. Missing worktreePath/branch for task ${taskId}. Raw output:\n${out}`);
  }
  return { worktreePath, branch };
}

function mcpSlug(input) {
  const cleaned = String(input || "")
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return cleaned || "mcp";
}

function buildPermissionRules(mcpServers = []) {
  const rules = [
    "ls",
    "list",
    "glob",
    "search",
    "grep",
    "read",
    "write",
    "edit",
    "bash",
    "todowrite",
    "todo_write",
    "websearch",
    "webfetch",
    "webfetch_html",
    "memory_store",
    "memory_search",
    "memory_list",
  ].map((permission) => ({ permission, pattern: "*", action: "allow" }));

  for (const server of mcpServers) {
    rules.push({
      permission: `mcp.${mcpSlug(server)}.*`,
      pattern: "*",
      action: "allow",
    });
  }

  return rules;
}

function modelPayload(modelProvider, modelId) {
  if (!modelProvider || !modelId) return {};
  return {
    provider: modelProvider,
    model: { providerID: modelProvider, modelID: modelId },
  };
}

async function createTaskSession(title, worktreePath, runtimeConfig) {
  return api.post("/session", {
    title,
    directory: worktreePath,
    workspace_root: worktreePath,
    permission: runtimeConfig.permissionRules,
    ...modelPayload(runtimeConfig.modelProvider, runtimeConfig.modelId),
  });
}

async function startRun(sessionId, prompt, runtimeConfig) {
  const payload = {
    parts: [{ type: "text", text: prompt }],
  };
  if (runtimeConfig.modelProvider && runtimeConfig.modelId) {
    payload.model = {
      providerID: runtimeConfig.modelProvider,
      modelID: runtimeConfig.modelId,
    };
  }
  const res = await api.post(`/session/${sessionId}/prompt_async?return=run`, payload);
  return res.runID || res.runId || res.run_id;
}

async function sendManagerDecompose(objective, runtimeConfig) {
  const managerSession = await api.post("/session", {
    title: "Agent Swarm Manager",
    directory: process.cwd(),
    workspace_root: process.cwd(),
    permission: runtimeConfig.permissionRules,
    ...modelPayload(runtimeConfig.modelProvider, runtimeConfig.modelId),
  });
  const managerPrompt = readPrompt("manager.md");
  const mcpHint = runtimeConfig.mcpServers.length
    ? `\nUse MCP servers when relevant: ${runtimeConfig.mcpServers.join(", ")}.`
    : "";
  const req = `${managerPrompt}\n\nObjective:\n${objective}\n\nReturn 1-${MAX_TASKS} tasks.${mcpHint}`;
  const response = await api.post(`/session/${managerSession.id}/prompt_sync`, {
    parts: [{ type: "text", text: req }],
  });
  const text = extractAssistantTextFromSession(response);
  const tasks = parseTasksFromText(text).slice(0, MAX_TASKS);
  if (tasks.length > 0) return tasks;
  return [
    {
      taskId: "task-1",
      title: objective,
      ownerRole: "worker",
      description: objective,
      acceptanceCriteria: [],
    },
  ];
}

function buildWorkerPrompt(task, worktreePath, branch) {
  return `${readPrompt("worker.md")}\n\nTask: ${task.title}\nDescription: ${task.description || task.title}\nBranch: ${branch}\nWorktree: ${worktreePath}`;
}

function buildTesterPrompt(task, worktreePath) {
  return `${readPrompt("tester.md")}\n\nTask: ${task.title}\nWorktree: ${worktreePath}`;
}

function buildReviewerPrompt(task, worktreePath, prUrl) {
  return `${readPrompt("reviewer.md")}\n\nTask: ${task.title}\nWorktree: ${worktreePath}\nPR: ${prUrl || "unknown"}`;
}

function extractPr(text) {
  const match = text.match(/https?:\/\/github\.com\/[^\s]+\/pull\/(\d+)/i);
  if (!match) return {};
  return { prUrl: match[0], prNumber: Number(match[1]) };
}

async function updatePrFromSession(task) {
  try {
    const messages = await api.get(`/session/${task.sessionId}/message`);
    const text = JSON.stringify(messages);
    const pr = extractPr(text);
    if (pr.prUrl) {
      task.prUrl = pr.prUrl;
      task.prNumber = pr.prNumber;
    }
  } catch {
    // ignore parse failure
  }
}

async function main() {
  const runtimeConfig = {
    modelProvider: MODEL_PROVIDER,
    modelId: MODEL_ID,
    mcpServers: MCP_SERVERS,
    permissionRules: buildPermissionRules(MCP_SERVERS),
  };
  const taskDefs = await sendManagerDecompose(OBJECTIVE, runtimeConfig);
  const registry = await loadRegistry(api);
  await seedTasks({
    registry,
    taskDefs,
    createWorktree: async (taskId) => runCreateWorktree(taskId),
    createSession: async (taskId, worktreePath) => createTaskSession(`Swarm Worker ${taskId}`, worktreePath, runtimeConfig),
    startRun: async (task, sessionId, worktreePath, branch) =>
      startRun(sessionId, buildWorkerPrompt(task, worktreePath, branch), runtimeConfig),
  });

  await saveRegistry(api, registry);
  console.log(`Using swarm registry key: ${currentResourceKey()}`);

  const ctx = createMapperContext();
  const abortController = new AbortController();
  const done = new Promise((resolve) => setTimeout(resolve, 5 * 60 * 1000));

  const eventLoop = api
    .streamEvents(
      async (event) => {
        const out = applyEvent(registry, event, ctx);
        if (!out.changed) return;

        for (const action of out.actions) {
          if (action.type === "notify_auth_once") {
            console.log(`[auth-required] ${action.taskId}: ${action.reason}`);
          }
        }

        for (const task of Object.values(registry.tasks)) {
          if (
            task.status === TASK_STATUS.READY_FOR_REVIEW &&
            task.ownerRole === "worker" &&
            !task._testerStarted
          ) {
            await updatePrFromSession(task);
            const s = await createTaskSession(`Swarm Tester ${task.taskId}`, task.worktreePath, runtimeConfig);
            const r = await startRun(s.id, buildTesterPrompt(task, task.worktreePath), runtimeConfig);
            Object.assign(task, {
              ownerRole: "tester",
              status: TASK_STATUS.RUNNING,
              sessionId: s.id,
              runId: r,
              _testerStarted: true,
            });
          } else if (
            task.status === TASK_STATUS.READY_FOR_REVIEW &&
            task.ownerRole === "tester" &&
            !task._reviewerStarted
          ) {
            const s = await createTaskSession(`Swarm Reviewer ${task.taskId}`, task.worktreePath, runtimeConfig);
            const r = await startRun(
              s.id,
              buildReviewerPrompt(task, task.worktreePath, task.prUrl),
              runtimeConfig
            );
            Object.assign(task, {
              ownerRole: "reviewer",
              status: TASK_STATUS.RUNNING,
              sessionId: s.id,
              runId: r,
              _reviewerStarted: true,
            });
          }
        }

        await saveRegistry(api, registry);

        const allTerminal = Object.values(registry.tasks).every((t) =>
          [TASK_STATUS.COMPLETE, TASK_STATUS.FAILED].includes(t.status)
        );
        if (allTerminal) abortController.abort();
      },
      { signal: abortController.signal }
    )
    .catch((err) => {
      if (!String(err).includes("AbortError")) {
        console.error(err);
      }
    });

  await Promise.race([done, eventLoop]);
  abortController.abort();
  await saveRegistry(api, registry);

  console.log("Swarm final status:");
  for (const task of Object.values(registry.tasks)) {
    console.log(`- ${task.taskId}: ${task.status} (${task.ownerRole}) ${task.prUrl || ""}`);
  }
  console.log("Merge gate: explicit user approval required. No automatic merge performed.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
