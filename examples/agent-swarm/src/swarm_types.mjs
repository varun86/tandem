export const PRIMARY_SWARM_RESOURCE_KEY = "swarm.active_tasks";
export const FALLBACK_SWARM_RESOURCE_KEY = "project/swarm.active_tasks";

export function resourceKeyCandidates() {
  const envKey = (process.env.SWARM_RESOURCE_KEY || "").trim();
  const keys = [
    envKey || PRIMARY_SWARM_RESOURCE_KEY,
    PRIMARY_SWARM_RESOURCE_KEY,
    FALLBACK_SWARM_RESOURCE_KEY,
  ];
  return [...new Set(keys.filter(Boolean))];
}

export const TASK_STATUS = {
  PENDING: "pending",
  RUNNING: "running",
  BLOCKED: "blocked",
  READY_FOR_REVIEW: "ready_for_review",
  COMPLETE: "complete",
  FAILED: "failed",
};

export const BLOCKED_BY = {
  APPROVAL: "approval",
  AUTH: "auth",
  ERROR: "error",
};

export function nowMs() {
  return Date.now();
}

export function blankRegistry() {
  return { version: 1, updatedAtMs: nowMs(), tasks: {} };
}
