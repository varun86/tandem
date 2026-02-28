import { blankRegistry, nowMs, resourceKeyCandidates } from "./swarm_types.mjs";

let activeResourceKey = resourceKeyCandidates()[0];

export async function loadRegistry(api) {
  for (const key of resourceKeyCandidates()) {
    try {
      const rec = await api.get(`/resource/${encodeURIComponent(key)}`);
      if (!rec || typeof rec !== "object") continue;
      activeResourceKey = key;
      return rec.value && typeof rec.value === "object" ? rec.value : blankRegistry();
    } catch {
      // try next key
    }
  }
  return blankRegistry();
}

export async function saveRegistry(api, registry, updatedBy = "examples.agent-swarm") {
  registry.updatedAtMs = nowMs();
  const keys = [activeResourceKey, ...resourceKeyCandidates()].filter(
    (key, idx, arr) => key && arr.indexOf(key) === idx
  );
  let lastError = null;
  for (const key of keys) {
    try {
      await api.put(`/resource/${encodeURIComponent(key)}`, {
        value: registry,
        updated_by: updatedBy,
      });
      activeResourceKey = key;
      return;
    } catch (err) {
      lastError = err;
      const text = String(err || "");
      if (!text.includes("INVALID_RESOURCE_KEY")) {
        throw err;
      }
    }
  }
  if (lastError) throw lastError;
}

export function upsertTask(registry, task) {
  registry.tasks[task.taskId] = {
    ...(registry.tasks[task.taskId] || {}),
    ...task,
    lastUpdateMs: nowMs(),
    notifyOnComplete: true,
  };
  registry.updatedAtMs = nowMs();
  return registry.tasks[task.taskId];
}

export function currentResourceKey() {
  return activeResourceKey;
}
