export async function renderPacks(ctx) {
  const { byId, state, toast, renderIcons, escapeHtml, api } = ctx;
  const trustBadgeClass = (badge) => {
    const value = String(badge || "").toLowerCase();
    if (value === "official") return "tcp-badge-info";
    if (value === "verified") return "tcp-badge-warn";
    return "tcp-badge-err";
  };
  const asArray = (value) => (Array.isArray(value) ? value : []);
  const app = byId("view");
  app.innerHTML = `
    <div class="tcp-card mb-4">
      <h3 class="tcp-title mb-2">Pack Library</h3>
      <p class="tcp-subtle mb-3">Install, inspect, export, and remove Tandem Packs.</p>
      <div class="grid gap-2 md:grid-cols-3">
        <input id="packs-install-url" class="tcp-input" placeholder="Install from URL (https://...zip)" />
        <input id="packs-install-path" class="tcp-input" placeholder="Install from server path (/path/to/pack.zip)" />
        <button id="packs-install-btn" class="tcp-btn-primary"><i data-lucide="download"></i> Install</button>
      </div>
      <div class="mt-3 flex flex-wrap gap-2">
        <button id="packs-refresh-btn" class="tcp-btn"><i data-lucide="refresh-cw"></i> Refresh</button>
        <button id="packs-cap-discovery-btn" class="tcp-btn"><i data-lucide="binary"></i> Capability Discovery</button>
      </div>
      <div id="packs-inspect-summary" class="mt-3 hidden rounded-xl border border-slate-800 bg-slate-950/70 p-3 text-xs text-slate-200"></div>
      <pre id="packs-meta" class="mt-3 rounded-xl border border-slate-800 bg-slate-950/70 p-3 text-xs text-slate-300 hidden"></pre>
    </div>
    <div id="packs-list" class="grid gap-3"></div>
    <div class="tcp-card mt-4">
      <div class="mb-2 flex items-center justify-between gap-2">
        <h3 class="tcp-title">Skill Module Library</h3>
        <button id="presets-refresh-btn" class="tcp-btn"><i data-lucide="refresh-cw"></i> Refresh</button>
      </div>
      <div class="grid gap-2 md:grid-cols-3">
        <input id="presets-filter-text" class="tcp-input" placeholder="Filter by id/tag/layer" />
        <input id="presets-filter-publisher" class="tcp-input" placeholder="Publisher filter" />
        <input id="presets-filter-capability" class="tcp-input" placeholder="Required capability filter" />
      </div>
      <div id="presets-skill-list" class="mt-3 grid gap-2"></div>
    </div>
  `;

  const packsListEl = byId("packs-list");
  const metaEl = byId("packs-meta");
  const summaryEl = byId("packs-inspect-summary");
  const skillsHostEl = byId("presets-skill-list");
  let presetIndex = null;

  const renderSkills = () => {
    const skills = Array.isArray(presetIndex?.skill_modules) ? presetIndex.skill_modules : [];
    const qText = String(byId("presets-filter-text")?.value || "").trim().toLowerCase();
    const qPub = String(byId("presets-filter-publisher")?.value || "")
      .trim()
      .toLowerCase();
    const qCap = String(byId("presets-filter-capability")?.value || "")
      .trim()
      .toLowerCase();
    const filtered = skills.filter((row) => {
      const id = String(row?.id || "").toLowerCase();
      const layer = String(row?.layer || "").toLowerCase();
      const tags = Array.isArray(row?.tags)
        ? row.tags.map((t) => String(t || "").toLowerCase())
        : [];
      const publisher = String(row?.publisher || "").toLowerCase();
      const caps = Array.isArray(row?.required_capabilities)
        ? row.required_capabilities.map((c) => String(c || "").toLowerCase())
        : [];
      const hayText = [id, layer, ...tags].join(" ");
      if (qText && !hayText.includes(qText)) return false;
      if (qPub && !publisher.includes(qPub)) return false;
      if (qCap && !caps.some((c) => c.includes(qCap))) return false;
      return true;
    });
    if (!filtered.length) {
      skillsHostEl.innerHTML = '<p class="tcp-subtle">No skill modules found.</p>';
      return;
    }
    skillsHostEl.innerHTML = filtered
      .map((row) => {
        const caps = Array.isArray(row?.required_capabilities) ? row.required_capabilities : [];
        const tags = Array.isArray(row?.tags) ? row.tags : [];
        return `
          <article class="rounded-xl border border-slate-800 bg-slate-950/60 p-3">
            <div class="flex items-center justify-between gap-2">
              <strong>${escapeHtml(String(row?.id || "unknown"))}</strong>
              <span class="tcp-subtle text-xs">${escapeHtml(String(row?.layer || "unknown"))}</span>
            </div>
            <div class="mt-1 text-xs text-slate-400">publisher: ${escapeHtml(String(row?.publisher || "unknown"))}</div>
            <div class="mt-1 text-xs text-slate-400">required capabilities: ${escapeHtml(String(caps.length))}</div>
            <div class="mt-1 text-xs text-slate-500">${escapeHtml(tags.join(", ") || "no tags")}</div>
          </article>
        `;
      })
      .join("");
  };

  const loadPresetIndex = async () => {
    try {
      const payload = await api("/api/presets/index");
      presetIndex = payload?.index || { skill_modules: [] };
      renderSkills();
    } catch (e) {
      presetIndex = { skill_modules: [] };
      renderSkills();
      toast("err", `Preset index load failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const setInspectSummary = (inspection) => {
    if (!summaryEl) return;
    const pack = inspection?.pack || {};
    const installed = pack?.installed || {};
    const trust = pack?.trust || {};
    const risk = pack?.risk || {};
    const sheet = pack?.permission_sheet || {};
    const badge = String(trust?.verification_badge || "unverified");
    const signature = String(trust?.signature || "unsigned");
    const required = asArray(sheet?.required_capabilities);
    const optional = asArray(sheet?.optional_capabilities);
    const providerSpecific = asArray(sheet?.provider_specific_dependencies);
    const routines = asArray(sheet?.routines_declared);
    const packLabel = `${String(installed?.name || "unknown")}@${String(installed?.version || "unknown")}`;
    summaryEl.classList.remove("hidden");
    summaryEl.innerHTML = `
      <div class="flex items-center justify-between gap-2">
        <strong class="text-sm">${escapeHtml(packLabel)}</strong>
        <span class="${trustBadgeClass(badge)}">${escapeHtml(badge)}</span>
      </div>
      <div class="mt-2 grid gap-1 text-[11px] text-slate-300">
        <div>signature: <span class="font-mono">${escapeHtml(signature)}</span></div>
        <div>risk level: <span class="font-mono">${escapeHtml(String(sheet?.risk_level || "standard"))}</span></div>
        <div>required capabilities: <span class="font-mono">${required.length}</span></div>
        <div>optional capabilities: <span class="font-mono">${optional.length}</span></div>
        <div>provider-specific deps: <span class="font-mono">${providerSpecific.length}</span></div>
        <div>routines declared: <span class="font-mono">${routines.length}</span></div>
        <div>routines enabled: <span class="font-mono">${escapeHtml(String(risk?.routines_enabled ?? false))}</span></div>
      </div>
    `;
  };
  const setMeta = (value) => {
    if (!metaEl) return;
    if (!value) {
      metaEl.classList.add("hidden");
      metaEl.textContent = "";
      summaryEl?.classList.add("hidden");
      return;
    }
    metaEl.classList.remove("hidden");
    metaEl.textContent = JSON.stringify(value, null, 2);
  };

  const loadPacks = async () => {
    try {
      const payload = await state.client.packs.list();
      const packs = Array.isArray(payload?.packs) ? payload.packs : [];
      if (!packs.length) {
        packsListEl.innerHTML = '<div class="tcp-card"><p class="tcp-subtle">No installed packs.</p></div>';
        renderIcons();
        return;
      }
      packsListEl.innerHTML = packs
        .map((pack) => {
          const id = String(pack.pack_id || pack.name || "");
          const name = String(pack.name || "unknown");
          const version = String(pack.version || "unknown");
          const type = String(pack.pack_type || "unknown");
          return `
          <div class="tcp-card">
            <div class="flex items-start justify-between gap-3">
              <div>
                <h4 class="font-semibold">${escapeHtml(name)} <span class="tcp-subtle">(${escapeHtml(version)})</span></h4>
                <p class="tcp-subtle text-xs">pack_id: <span class="font-mono">${escapeHtml(id)}</span> · type: ${escapeHtml(type)}</p>
              </div>
              <div class="flex flex-wrap gap-2">
                <button class="tcp-btn" data-pack-inspect="${escapeHtml(id)}"><i data-lucide="search"></i> Inspect</button>
                <button class="tcp-btn" data-pack-export="${escapeHtml(id)}"><i data-lucide="archive"></i> Export</button>
                <button class="tcp-btn" data-pack-updates="${escapeHtml(id)}"><i data-lucide="badge-check"></i> Updates</button>
                <button class="tcp-btn" data-pack-update="${escapeHtml(id)}"><i data-lucide="arrow-up-circle"></i> Update</button>
                <button class="tcp-btn" data-pack-remove="${escapeHtml(id)}"><i data-lucide="trash-2"></i> Uninstall</button>
              </div>
            </div>
          </div>`;
        })
        .join("");
      renderIcons();

      packsListEl.querySelectorAll("[data-pack-inspect]").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const id = btn.getAttribute("data-pack-inspect");
          if (!id) return;
          try {
            const inspected = await state.client.packs.inspect(id);
            setInspectSummary(inspected);
            setMeta(inspected);
          } catch (e) {
            toast("err", `Inspect failed: ${e instanceof Error ? e.message : String(e)}`);
          }
        });
      });

      packsListEl.querySelectorAll("[data-pack-export]").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const id = btn.getAttribute("data-pack-export");
          if (!id) return;
          try {
            const exported = await state.client.packs.export({ pack_id: id });
            toast("ok", `Exported to ${exported?.exported?.path || "unknown path"}`);
          } catch (e) {
            toast("err", `Export failed: ${e instanceof Error ? e.message : String(e)}`);
          }
        });
      });

      packsListEl.querySelectorAll("[data-pack-updates]").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const id = btn.getAttribute("data-pack-updates");
          if (!id) return;
          try {
            const updates = await state.client.packs.updates(id);
            setMeta(updates);
            const count = Array.isArray(updates?.updates) ? updates.updates.length : 0;
            const needsReapproval = !!updates?.reapproval_required;
            toast(
              needsReapproval ? "warn" : "info",
              needsReapproval
                ? `Updates available: ${count}. Permission scope changed; re-approval required.`
                : `Updates available: ${count}`
            );
          } catch (e) {
            toast("err", `Updates check failed: ${e instanceof Error ? e.message : String(e)}`);
          }
        });
      });

      packsListEl.querySelectorAll("[data-pack-update]").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const id = btn.getAttribute("data-pack-update");
          if (!id) return;
          try {
            const updated = await state.client.packs.update(id, {});
            setMeta(updated);
            const needsReapproval = !!updated?.reapproval_required;
            toast(
              needsReapproval ? "warn" : "info",
              updated?.reason ||
                (updated?.updated
                  ? "Pack updated"
                  : needsReapproval
                    ? "No update applied. Permission re-approval required."
                    : "No update applied")
            );
          } catch (e) {
            toast("err", `Update failed: ${e instanceof Error ? e.message : String(e)}`);
          }
        });
      });

      packsListEl.querySelectorAll("[data-pack-remove]").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const id = btn.getAttribute("data-pack-remove");
          if (!id) return;
          if (!window.confirm(`Uninstall pack ${id}?`)) return;
          try {
            await state.client.packs.uninstall({ pack_id: id });
            toast("ok", `Uninstalled ${id}`);
            await loadPacks();
          } catch (e) {
            toast("err", `Uninstall failed: ${e instanceof Error ? e.message : String(e)}`);
          }
        });
      });
    } catch (e) {
      packsListEl.innerHTML = `<div class="tcp-card"><p class="text-rose-300 text-sm">Failed to load packs: ${escapeHtml(
        e instanceof Error ? e.message : String(e)
      )}</p></div>`;
      renderIcons();
    }
  };

  byId("packs-refresh-btn")?.addEventListener("click", () => void loadPacks());
  byId("presets-refresh-btn")?.addEventListener("click", () => void loadPresetIndex());
  byId("presets-filter-text")?.addEventListener("input", renderSkills);
  byId("presets-filter-publisher")?.addEventListener("input", renderSkills);
  byId("presets-filter-capability")?.addEventListener("input", renderSkills);
  byId("packs-cap-discovery-btn")?.addEventListener("click", async () => {
    try {
      const discovery = await state.client.capabilities.discovery();
      setMeta(discovery);
      const count = Array.isArray(discovery?.tools) ? discovery.tools.length : 0;
      toast("ok", `Discovered ${count} tools`);
    } catch (e) {
      toast("err", `Capability discovery failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  });
  byId("packs-install-btn")?.addEventListener("click", async () => {
    const url = String(byId("packs-install-url")?.value || "").trim();
    const path = String(byId("packs-install-path")?.value || "").trim();
    if (!url && !path) {
      toast("err", "Provide either URL or server path.");
      return;
    }
    try {
      const payload = await state.client.packs.install({
        url: url || undefined,
        path: path || undefined,
        source: { kind: "control_panel" },
      });
      setMeta(payload);
      toast(
        "ok",
        `Installed ${payload?.installed?.name || "pack"} ${payload?.installed?.version || ""}`.trim()
      );
      await loadPacks();
    } catch (e) {
      toast("err", `Install failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  });

  await loadPacks();
  await loadPresetIndex();
}
