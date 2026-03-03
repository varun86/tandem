function pickStatusClass(status) {
  const normalized = String(status || "").toLowerCase();
  if (normalized.includes("fail") || normalized.includes("error")) return "tcp-badge-err";
  if (normalized.includes("wait") || normalized.includes("queue") || normalized.includes("new") || normalized.includes("block")) return "tcp-badge-warn";
  return "tcp-badge-ok";
}

function reasonText(reason) {
  if (!reason) return "";
  if (reason.kind === "task_transition") {
    return `${reason.taskId}: ${reason.from} -> ${reason.to} (${reason.reason || "status changed"})`;
  }
  if (reason.kind === "task_reason") return `${reason.taskId}: ${reason.reason || "updated"}`;
  return reason.reason || JSON.stringify(reason);
}

function normalizeMcpServers(raw) {
  if (Array.isArray(raw)) {
    return raw
      .map((entry) => {
        if (!entry || typeof entry !== "object") return null;
        const name = String(entry.name || "").trim();
        if (!name) return null;
        return {
          name,
          connected: !!entry.connected,
          enabled: entry.enabled !== false,
        };
      })
      .filter(Boolean)
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  if (!raw || typeof raw !== "object") return [];
  if (Array.isArray(raw.servers)) return normalizeMcpServers(raw.servers);

  return Object.entries(raw)
    .map(([name, row]) => ({
      name: String(name || "").trim(),
      connected: !!row?.connected,
      enabled: row?.enabled !== false,
    }))
    .filter((row) => row.name)
    .sort((a, b) => a.name.localeCompare(b.name));
}

function swarmFormHasFocus() {
  const active = document.activeElement;
  if (!active) return false;
  if (!(active instanceof HTMLElement)) return false;
  if (!active.closest("[data-swarm-form]")) return false;
  const tag = String(active.tagName || "").toLowerCase();
  if (tag === "textarea") return true;
  if (tag === "select") return true;
  if (tag !== "input") return false;
  const type = String(active.getAttribute("type") || "text").toLowerCase();
  return !["button", "submit", "reset", "checkbox", "radio", "range"].includes(type);
}

function swarmRefreshLocked(state) {
  return Number(state?.__swarmUiLockUntil || 0) > Date.now();
}

function setSwarmRefreshLock(state, msFromNow) {
  const until = Date.now() + Math.max(0, Number(msFromNow || 0));
  state.__swarmUiLockUntil = Math.max(Number(state.__swarmUiLockUntil || 0), until);
}

function ageText(ts) {
  const ms = Number(ts || 0);
  if (!ms) return "unknown";
  const delta = Math.max(0, Date.now() - ms);
  if (delta < 1000) return "just now";
  const sec = Math.floor(delta / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  return `${day}d ago`;
}

function boardColumns() {
  return [
    { key: "pending", label: "Pending" },
    { key: "running", label: "Running" },
    { key: "blocked", label: "Blocked" },
    { key: "ready_for_review", label: "Review" },
    { key: "complete", label: "Complete" },
    { key: "failed", label: "Failed" },
  ];
}

function normalizeTaskStatus(raw) {
  const status = String(raw || "pending").trim().toLowerCase();
  if (["pending", "running", "blocked", "ready_for_review", "complete", "failed"].includes(status)) {
    return status;
  }
  if (status.includes("fail") || status.includes("error")) return "failed";
  if (status.includes("block") || status.includes("wait")) return "blocked";
  if (status.includes("review")) return "ready_for_review";
  if (status.includes("run") || status.includes("active")) return "running";
  if (status.includes("done") || status.includes("complete")) return "complete";
  return "pending";
}

function copyText(text) {
  const value = String(text || "");
  if (!value) return Promise.resolve();
  if (navigator?.clipboard?.writeText) return navigator.clipboard.writeText(value);
  const area = document.createElement("textarea");
  area.value = value;
  area.style.position = "fixed";
  area.style.left = "-10000px";
  document.body.appendChild(area);
  area.select();
  document.execCommand("copy");
  area.remove();
  return Promise.resolve();
}

function buildTaskCard(task, escapeHtml) {
  const taskId = String(task.taskId || "task");
  const title = String(task.title || task.taskId || "Untitled task");
  const owner = String(task.ownerRole || "unknown");
  const status = String(task.status || "unknown");
  const statusReason = String(task.statusReason || "");
  const sessionId = String(task.sessionId || "");
  const runId = String(task.runId || "");
  const branch = String(task.branch || "");
  const worktree = String(task.worktreePath || "");
  const prUrl = String(task.prUrl || "");
  const prNumber = task.prNumber != null ? String(task.prNumber) : "";
  const checksStatus = String(task.checksStatus || "");
  const updated = Number(task.lastUpdateMs || 0);

  const copyPayload = [
    `taskId=${taskId}`,
    sessionId ? `sessionId=${sessionId}` : "",
    runId ? `runId=${runId}` : "",
    branch ? `branch=${branch}` : "",
  ]
    .filter(Boolean)
    .join("\n");

  return `<article class="tcp-list-item border border-slate-700/70 bg-slate-950/35">
    <div class="mb-2 flex items-start justify-between gap-2">
      <div class="min-w-0">
        <div class="truncate font-semibold text-slate-100" title="${escapeHtml(title)}">${escapeHtml(title)}</div>
        <div class="truncate text-xs text-slate-400">${escapeHtml(taskId)}</div>
      </div>
      <span class="${pickStatusClass(status)}">${escapeHtml(status)}</span>
    </div>
    <div class="grid gap-1 text-xs text-slate-300">
      <div><span class="text-slate-400">Owner:</span> ${escapeHtml(owner)}</div>
      <div><span class="text-slate-400">Reason:</span> ${escapeHtml(statusReason || "-")}</div>
      <div><span class="text-slate-400">Updated:</span> ${escapeHtml(ageText(updated))}</div>
      <div><span class="text-slate-400">Session:</span> ${escapeHtml(sessionId || "-")}</div>
      <div><span class="text-slate-400">Run:</span> ${escapeHtml(runId || "-")}</div>
      <div><span class="text-slate-400">Branch:</span> ${escapeHtml(branch || "-")}</div>
      <div class="truncate" title="${escapeHtml(worktree || "-")}"><span class="text-slate-400">Worktree:</span> ${escapeHtml(worktree || "-")}</div>
      ${prUrl ? `<div><span class="text-slate-400">PR:</span> <a class="underline" href="${escapeHtml(prUrl)}" target="_blank" rel="noreferrer">#${escapeHtml(prNumber || "link")}</a></div>` : ""}
      ${checksStatus ? `<div><span class="text-slate-400">Checks:</span> ${escapeHtml(checksStatus)}</div>` : ""}
    </div>
    <div class="mt-3 flex flex-wrap gap-2">
      <button class="tcp-btn h-7 px-2 text-xs" data-swarm-copy="${escapeHtml(copyPayload)}">Copy IDs</button>
      <button class="tcp-btn h-7 px-2 text-xs" data-swarm-focus-task="${escapeHtml(taskId)}">Focus Logs</button>
      ${sessionId ? `<button class="tcp-btn h-7 px-2 text-xs" data-swarm-open-session="${escapeHtml(sessionId)}">Open Session</button>` : ""}
    </div>
  </article>`;
}

export async function renderSwarm(ctx, options = {}) {
  const { api, byId, escapeHtml, toast, state, addCleanup, setRoute } = ctx;
  if (state.route !== "swarm") return;
  const force = options?.force === true;
  if (!force && state.__swarmRenderedOnce && (swarmFormHasFocus() || swarmRefreshLocked(state))) return;
  if (state.__swarmRenderInFlight) return;
  state.__swarmRenderInFlight = true;
  try {
  const renderRouteSnapshot = state.route;
  if (state.__swarmLiveCleanup && Array.isArray(state.__swarmLiveCleanup)) {
    for (const fn of state.__swarmLiveCleanup) {
      try {
        fn();
      } catch {
        // ignore cleanup failure
      }
    }
  }
  state.__swarmLiveCleanup = [];

  const [status, snapshot, providerCatalog, providerConfig, mcpRaw] = await Promise.all([
    api("/api/swarm/status").catch(() => ({ status: "error" })),
    api("/api/swarm/snapshot").catch(() => ({ registry: { value: { tasks: {} } }, logs: [], reasons: [] })),
    state.client?.providers?.catalog?.().catch(() => ({ all: [] })),
    state.client?.providers?.config?.().catch(() => ({ default: "", providers: {} })),
    state.client?.mcp?.list?.().catch(() => ({})),
  ]);
  if (state.route !== renderRouteSnapshot) return;

  const tasks = Object.values(snapshot.registry?.value?.tasks || {});
  const reasons = (snapshot.reasons || []).slice().reverse();
  const providers = Array.isArray(providerCatalog?.all)
    ? providerCatalog.all
        .map((row) => ({
          id: String(row?.id || "").trim(),
          models: Object.keys(row?.models || {}).filter(Boolean),
        }))
        .filter((row) => row.id)
        .sort((a, b) => a.id.localeCompare(b.id))
    : [];
  const connectedMcp = normalizeMcpServers(mcpRaw).filter((row) => row.connected && row.enabled);

  if (!state.__swarmDraft || typeof state.__swarmDraft !== "object") state.__swarmDraft = {};
  const draft = state.__swarmDraft;
  if (!draft.workspaceRoot) draft.workspaceRoot = String(status.workspaceRoot || "").trim();
  if (!draft.objective) draft.objective = String(status.objective || "Ship a small feature end-to-end");
  if (!draft.maxTasks) draft.maxTasks = String(status.maxTasks || 3);
  if (typeof draft.allowInitNonEmpty !== "boolean") draft.allowInitNonEmpty = false;
  if (!draft.modelProvider) draft.modelProvider = String(status.modelProvider || providerConfig?.default || "").trim();
  const modelsForProvider = providers.find((row) => row.id === draft.modelProvider)?.models || [];
  if (!draft.modelId) {
    const configuredDefault = String(providerConfig?.providers?.[draft.modelProvider]?.default_model || "").trim();
    draft.modelId = String(status.modelId || configuredDefault || modelsForProvider[0] || "").trim();
  }
  if (!Array.isArray(draft.mcpServers)) {
    draft.mcpServers = Array.isArray(status.mcpServers)
      ? status.mcpServers.map((v) => String(v).trim()).filter(Boolean)
      : [];
  }

  const selectedMcp = new Set(
    draft.mcpServers.map((v) => String(v).trim().toLowerCase()).filter(Boolean)
  );

  const groupedTasks = {
    pending: [],
    running: [],
    blocked: [],
    ready_for_review: [],
    complete: [],
    failed: [],
  };
  for (const task of tasks) {
    groupedTasks[normalizeTaskStatus(task.status)].push(task);
  }
  for (const key of Object.keys(groupedTasks)) {
    groupedTasks[key].sort((a, b) => (b.lastUpdateMs || 0) - (a.lastUpdateMs || 0));
  }

  const preflight = status.preflight || {};
  const preflightCode = String(preflight.code || "").trim();
  const gitMissing = preflight.gitAvailable === false;
  const preflightProblem = preflight.repoReady === false && String(preflight.reason || "").trim();
  const startDisabled = !status.localEngine || gitMissing;
  const startTitle = !status.localEngine
    ? "Swarm orchestration is disabled on remote engine URLs."
    : gitMissing
      ? `${String(preflight.reason || "Git executable not found")}. ${String(preflight.guidance || "")}`.trim()
      : "Start swarm";

  const viewEl = byId("view");
  viewEl.innerHTML = `
    <div class="tcp-card" data-swarm-form="1">
      <div class="mb-3 flex items-center justify-between gap-3">
        <h3 class="tcp-title flex items-center gap-2"><i data-lucide="cpu"></i> Node Swarm Orchestrator</h3>
        <span class="${pickStatusClass(status.status)}">${escapeHtml(status.status || "idle")}</span>
      </div>
      <p class="mb-3 rounded-xl border border-slate-700/60 bg-slate-900/25 px-3 py-2 text-xs text-slate-300">
        Swarm uses real task state from <code>swarm.active_tasks</code>. The board below reflects actual task transitions.
      </p>
      ${gitMissing ? `<p class="mb-3 rounded-xl border border-rose-700/60 bg-rose-950/25 px-3 py-2 text-sm text-rose-300">${escapeHtml(preflight.reason || "Git executable not found")}. ${escapeHtml(preflight.guidance || "Install Git and restart.")}</p>` : ""}
      ${
        !gitMissing && preflightProblem
          ? `<div class="mb-3 rounded-xl border border-amber-700/60 bg-amber-950/20 px-3 py-2 text-sm text-amber-300">
               <div>${escapeHtml(preflight.reason)}</div>
               ${
                 preflightCode === "not_repo_non_empty"
                   ? '<div class="mt-2 flex gap-2"><button id="swarm-init-nonempty" class="tcp-btn h-8 px-3 text-xs">Initialize This Directory As Git Repo</button></div>'
                   : ""
               }
             </div>`
          : ""
      }
      ${status.repoRoot ? `<p class="mb-3 rounded-xl border border-slate-700/60 bg-slate-900/20 px-3 py-2 text-xs text-slate-300"><strong>Repo root:</strong> ${escapeHtml(status.repoRoot)}</p>` : ""}
      <div class="grid gap-3 md:grid-cols-[1fr_160px_auto]">
        <input id="swarm-root" class="tcp-input" value="${escapeHtml(draft.workspaceRoot || "")}" placeholder="workspace root" />
        <input id="swarm-max" class="tcp-input" type="number" min="1" value="${escapeHtml(String(draft.maxTasks || 3))}" />
        <div class="flex gap-2">
          <button id="swarm-start" class="tcp-btn-primary" ${startDisabled ? "disabled" : ""} title="${escapeHtml(startTitle)}"><i data-lucide="play"></i> Start</button>
          <button id="swarm-stop" class="tcp-btn-danger"><i data-lucide="square"></i> Stop</button>
        </div>
      </div>
      <div class="mt-3 grid gap-3 lg:grid-cols-2">
        <div class="grid gap-2">
          <label for="swarm-model-provider" class="text-xs uppercase tracking-wide text-slate-400">Model Provider</label>
          <select id="swarm-model-provider" class="tcp-select">
            <option value="">Default provider/model</option>
            ${providers
              .map(
                (row) =>
                  `<option value="${escapeHtml(row.id)}" ${row.id === draft.modelProvider ? "selected" : ""}>${escapeHtml(row.id)}</option>`
              )
              .join("")}
          </select>
        </div>
        <div class="grid gap-2">
          <label for="swarm-model-id" class="text-xs uppercase tracking-wide text-slate-400">Model ID</label>
          <select id="swarm-model-id" class="tcp-select" ${draft.modelProvider ? "" : "disabled"}>
            ${
              draft.modelProvider
                ? (providers.find((row) => row.id === draft.modelProvider)?.models || [])
                    .map(
                      (modelId) =>
                        `<option value="${escapeHtml(modelId)}" ${modelId === draft.modelId ? "selected" : ""}>${escapeHtml(modelId)}</option>`
                    )
                    .join("")
                : '<option value="">Uses provider default</option>'
            }
          </select>
        </div>
      </div>
      <div class="mt-3 grid gap-2">
        <label for="swarm-objective" class="text-xs uppercase tracking-wide text-slate-400">Objective (Markdown)</label>
        <textarea id="swarm-objective" class="tcp-input min-h-[180px] resize-y leading-relaxed" placeholder="Describe the swarm objective in markdown...">${escapeHtml(draft.objective || "")}</textarea>
      </div>
      <div class="mt-3 grid gap-2">
        <div class="text-xs uppercase tracking-wide text-slate-400">MCP Servers</div>
        <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
          ${
            connectedMcp.length
              ? connectedMcp
                  .map(
                    (row) => `<label class="tcp-list-item flex items-center gap-2 text-sm">
            <input type="checkbox" data-swarm-mcp-option="${escapeHtml(row.name)}" ${selectedMcp.has(row.name.toLowerCase()) ? "checked" : ""} />
            <span>${escapeHtml(row.name)}</span>
          </label>`
                  )
                  .join("")
              : '<p class="tcp-subtle">No connected MCP servers found. Connect them in Settings > MCP.</p>'
          }
        </div>
      </div>
      ${status.localEngine ? "" : '<p class="mt-3 rounded-xl border border-amber-700/60 bg-amber-950/20 px-3 py-2 text-sm text-amber-300">Swarm orchestration is disabled on remote engine URLs. Monitoring remains available.</p>'}
    </div>

    <div class="grid gap-4 xl:grid-cols-[minmax(0,2fr)_minmax(340px,1fr)]">
      <div class="tcp-card">
        <div class="mb-3 flex items-center justify-between gap-2">
          <h3 class="tcp-title">Swarm Kanban</h3>
          <span class="tcp-subtle text-xs">${tasks.length} tasks</span>
        </div>
        <div class="grid gap-3 xl:grid-cols-3 2xl:grid-cols-6">
          ${boardColumns()
            .map((col) => {
              const entries = groupedTasks[col.key] || [];
              return `<section class="rounded-xl border border-slate-700/70 bg-slate-950/30 p-2">
                <div class="mb-2 flex items-center justify-between gap-2">
                  <h4 class="text-xs font-semibold uppercase tracking-wide text-slate-300">${escapeHtml(col.label)}</h4>
                  <span class="tcp-badge-info">${entries.length}</span>
                </div>
                <div class="grid max-h-[520px] gap-2 overflow-auto">
                  ${entries.map((task) => buildTaskCard(task, escapeHtml)).join("") || '<p class="px-2 py-1 text-xs text-slate-500">No tasks</p>'}
                </div>
              </section>`;
            })
            .join("")}
        </div>
      </div>

      <aside class="grid gap-4">
        <div class="tcp-card">
          <div class="mb-3 flex items-center justify-between gap-2">
            <h3 class="tcp-title">Swarm Why Timeline</h3>
            <div class="flex gap-2">
              <select id="swarm-reason-kind" class="tcp-select !w-auto !py-1.5 text-xs">
                <option value="">All kinds</option>
                <option value="task_transition">task_transition</option>
                <option value="task_reason">task_reason</option>
              </select>
              <input id="swarm-reason-task" class="tcp-input !w-44 !py-1.5 text-xs" placeholder="Filter by task id" />
            </div>
          </div>
          <div id="swarm-reasons" class="grid max-h-[360px] gap-2 overflow-auto"></div>
        </div>

        <div class="tcp-card">
          <h3 class="tcp-title mb-3">Swarm Logs</h3>
          <pre id="swarm-logs" class="tcp-code max-h-[360px] overflow-auto"></pre>
        </div>
      </aside>
    </div>
  `;

  function renderReasons() {
    const kind = byId("swarm-reason-kind")?.value?.trim() || "";
    const taskFilter = byId("swarm-reason-task")?.value?.trim()?.toLowerCase() || "";
    const filtered = reasons.filter((r) => {
      if (kind && r.kind !== kind) return false;
      if (taskFilter && !String(r.taskId || "").toLowerCase().includes(taskFilter)) return false;
      return true;
    });

    const reasonsEl = byId("swarm-reasons");
    if (!reasonsEl) return;
    reasonsEl.innerHTML =
      filtered
        .map(
          (r) => `
        <div class="tcp-list-item">
          <div class="flex items-center justify-between gap-2"><span class="text-xs text-slate-400">${new Date(r.at).toLocaleTimeString()}</span><span class="${pickStatusClass(r.to || r.from)}">${escapeHtml(r.kind || "reason")}</span></div>
          <div class="mt-1"><strong>${escapeHtml(r.taskId || "swarm")}</strong> <span class="tcp-subtle">${escapeHtml(r.role || "")}</span></div>
          <div class="mt-1 text-sm text-slate-300">${escapeHtml(reasonText(r))}</div>
        </div>
      `
        )
        .join("") || '<p class="tcp-subtle">No timeline reasons yet.</p>';
  }

  byId("swarm-reason-kind")?.addEventListener("change", renderReasons);
  byId("swarm-reason-task")?.addEventListener("input", renderReasons);
  renderReasons();

  byId("swarm-logs").textContent = (snapshot.logs || [])
    .slice(-200)
    .map((l) => `[${new Date(l.at).toLocaleTimeString()}] ${l.stream}: ${l.line}`)
    .join("\n");

  const setDraftValue = (key, value) => {
    draft[key] = value;
  };
  const formRoot = viewEl.querySelector("[data-swarm-form]");
  formRoot?.addEventListener("pointerdown", () => setSwarmRefreshLock(state, 1500));
  formRoot?.addEventListener("focusin", () => setSwarmRefreshLock(state, 60_000));
  formRoot?.addEventListener("focusout", () => setSwarmRefreshLock(state, 1200));
  const collectCurrentFormState = () => {
    const workspaceRoot = String(byId("swarm-root")?.value ?? draft.workspaceRoot ?? "").trim();
    const objective = String(byId("swarm-objective")?.value ?? draft.objective ?? "").trim();
    const maxTasks = Number.parseInt(
      String(byId("swarm-max")?.value ?? draft.maxTasks ?? "3"),
      10
    ) || 3;
    const modelProvider = String(
      byId("swarm-model-provider")?.value ?? draft.modelProvider ?? ""
    ).trim();
    const modelId = String(byId("swarm-model-id")?.value ?? draft.modelId ?? "").trim();
    const mcpServers = [...viewEl.querySelectorAll("[data-swarm-mcp-option]:checked")]
      .map((node) => String(node.getAttribute("data-swarm-mcp-option") || "").trim())
      .filter(Boolean);
    draft.workspaceRoot = workspaceRoot;
    draft.objective = objective;
    draft.maxTasks = String(maxTasks);
    draft.modelProvider = modelProvider;
    draft.modelId = modelId;
    draft.mcpServers = mcpServers;
    return {
      workspaceRoot,
      objective,
      maxTasks,
      modelProvider,
      modelId,
      mcpServers,
    };
  };

  byId("swarm-root")?.addEventListener("input", (event) =>
    setDraftValue("workspaceRoot", String(event.target?.value || ""))
  );
  byId("swarm-objective")?.addEventListener("input", (event) =>
    setDraftValue("objective", String(event.target?.value || ""))
  );
  byId("swarm-max")?.addEventListener("input", (event) =>
    setDraftValue("maxTasks", String(event.target?.value || ""))
  );
  byId("swarm-model-provider")?.addEventListener("change", (event) => {
    const providerId = String(event.target?.value || "").trim();
    setDraftValue("modelProvider", providerId);
    const modelCandidates = providers.find((row) => row.id === providerId)?.models || [];
    const configuredDefault = String(
      providerConfig?.providers?.[providerId]?.default_model || ""
    ).trim();
    setDraftValue(
      "modelId",
      modelCandidates.includes(configuredDefault) ? configuredDefault : modelCandidates[0] || ""
    );
    renderSwarm(ctx);
  });
  byId("swarm-model-id")?.addEventListener("change", (event) =>
    setDraftValue("modelId", String(event.target?.value || "").trim())
  );

  viewEl.querySelectorAll("[data-swarm-mcp-option]").forEach((el) =>
    el.addEventListener("change", () => {
      const picked = [...viewEl.querySelectorAll("[data-swarm-mcp-option]:checked")]
        .map((node) => String(node.getAttribute("data-swarm-mcp-option") || "").trim())
        .filter(Boolean);
      setDraftValue("mcpServers", picked);
    })
  );

  viewEl.querySelectorAll("[data-swarm-copy]").forEach((button) =>
    button.addEventListener("click", async () => {
      const value = String(button.getAttribute("data-swarm-copy") || "");
      try {
        await copyText(value);
        toast("ok", "Task identifiers copied.");
      } catch {
        toast("err", "Failed to copy task identifiers.");
      }
    })
  );

  viewEl.querySelectorAll("[data-swarm-focus-task]").forEach((button) =>
    button.addEventListener("click", () => {
      const taskId = String(button.getAttribute("data-swarm-focus-task") || "").trim();
      const input = byId("swarm-reason-task");
      if (input) input.value = taskId;
      renderReasons();
      byId("swarm-logs")?.scrollIntoView({ behavior: "smooth", block: "start" });
    })
  );

  viewEl.querySelectorAll("[data-swarm-open-session]").forEach((button) =>
    button.addEventListener("click", () => {
      const sessionId = String(button.getAttribute("data-swarm-open-session") || "").trim();
      if (!sessionId) return;
      state.currentSessionId = sessionId;
      if (typeof setRoute === "function") {
        setRoute("chat");
      }
    })
  );

  byId("swarm-start")?.addEventListener("click", async () => {
    try {
      const current = collectCurrentFormState();
      await api("/api/swarm/start", {
        method: "POST",
        body: JSON.stringify({
          workspaceRoot: current.workspaceRoot,
          objective: current.objective,
          maxTasks: current.maxTasks,
          modelProvider: current.modelProvider,
          modelId: current.modelId,
          mcpServers: current.mcpServers,
          allowInitNonEmpty: !!draft.allowInitNonEmpty,
        }),
      });
      toast("ok", "Swarm started.");
      renderSwarm(ctx, { force: true });
    } catch (e) {
      toast("err", e instanceof Error ? e.message : String(e));
    }
  });

  byId("swarm-init-nonempty")?.addEventListener("click", async () => {
    draft.allowInitNonEmpty = true;
    try {
      const current = collectCurrentFormState();
      await api("/api/swarm/start", {
        method: "POST",
        body: JSON.stringify({
          workspaceRoot: current.workspaceRoot,
          objective: current.objective,
          maxTasks: current.maxTasks,
          modelProvider: current.modelProvider,
          modelId: current.modelId,
          mcpServers: current.mcpServers,
          allowInitNonEmpty: true,
        }),
      });
      toast("ok", "Directory initialized as a Git repo. Swarm started.");
      draft.allowInitNonEmpty = false;
      renderSwarm(ctx, { force: true });
    } catch (e) {
      toast("err", e instanceof Error ? e.message : String(e));
    }
  });

  byId("swarm-stop")?.addEventListener("click", async () => {
    try {
      await api("/api/swarm/stop", { method: "POST" });
      toast("ok", "Swarm stop requested.");
      renderSwarm(ctx, { force: true });
    } catch (e) {
      toast("err", e instanceof Error ? e.message : String(e));
    }
  });

  const poll = setInterval(() => {
    if (state.route !== "swarm") return;
    if (swarmFormHasFocus()) return;
    if (swarmRefreshLocked(state)) return;
    renderSwarm(ctx);
  }, 4000);
  const stopPoll = () => clearInterval(poll);
  state.__swarmLiveCleanup.push(stopPoll);
  addCleanup(stopPoll);

  try {
    const evt = new EventSource("/api/swarm/events", { withCredentials: true });
    evt.onmessage = () => {
      if (state.route !== "swarm") return;
      if (swarmFormHasFocus()) return;
      if (swarmRefreshLocked(state)) return;
      renderSwarm(ctx);
    };
    evt.onerror = () => evt.close();
    const stopEvt = () => evt.close();
    state.__swarmLiveCleanup.push(stopEvt);
    addCleanup(stopEvt);
  } catch {
    // ignore
  }
  } finally {
    state.__swarmRenderInFlight = false;
    state.__swarmRenderedOnce = true;
  }
}
