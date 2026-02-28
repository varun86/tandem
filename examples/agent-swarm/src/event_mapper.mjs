import { BLOCKED_BY, TASK_STATUS, nowMs } from "./swarm_types.mjs";

export function createMapperContext() {
  return {
    requestToTask: new Map(),
    authNotified: new Set(),
  };
}

function matchesTask(task, props) {
  const sid = props.sessionID || props.sessionId || props.id;
  const rid = props.runID || props.run_id;
  if (!sid || task.sessionId !== sid) return false;
  if (!rid) return true;
  return task.runId === rid;
}

export function transitionTask(task, event, context) {
  const props = event.properties || {};
  const part = props.part || {};
  task.lastUpdateMs = nowMs();

  if (event.type === "session.run.started") {
    task.status = TASK_STATUS.RUNNING;
    task.blockedBy = undefined;
    task.statusReason = "run started";
    return { changed: true, actions: [] };
  }

  if (event.type === "message.part.updated") {
    const partType = part.type || part.part_type;
    const state = part.state || "";
    if (partType === "tool" || partType === "tool-invocation" || partType === "tool-result") {
      task.status = TASK_STATUS.RUNNING;
      task.statusReason = state ? `tool:${state}` : "tool activity";
      if (state === "failed") {
        task.statusReason = part.error || "tool failed";
      }
      return { changed: true, actions: [] };
    }
    return { changed: false, actions: [] };
  }

  if (event.type === "permission.asked") {
    const reqId = props.requestID;
    if (reqId) context.requestToTask.set(reqId, task.taskId);
    task.status = TASK_STATUS.BLOCKED;
    task.blockedBy = BLOCKED_BY.APPROVAL;
    task.statusReason = "waiting for approval";
    return { changed: true, actions: [] };
  }

  if (event.type === "permission.replied") {
    const reply = `${props.reply || ""}`.toLowerCase();
    if (["deny", "reject"].includes(reply)) {
      task.status = TASK_STATUS.FAILED;
      task.blockedBy = BLOCKED_BY.ERROR;
      task.statusReason = `approval ${reply}`;
    } else {
      task.status = TASK_STATUS.RUNNING;
      task.blockedBy = undefined;
      task.statusReason = `approval ${reply || "granted"}`;
    }
    return { changed: true, actions: [] };
  }

  if (event.type === "mcp.auth.required") {
    task.status = TASK_STATUS.BLOCKED;
    task.blockedBy = BLOCKED_BY.AUTH;
    task.statusReason = props.message || "mcp auth required";
    const key = `${task.taskId}:${task.runId}`;
    const actions = [];
    if (!context.authNotified.has(key)) {
      context.authNotified.add(key);
      actions.push({ type: "notify_auth_once", taskId: task.taskId, reason: task.statusReason });
    }
    return { changed: true, actions };
  }

  if (event.type === "session.run.finished") {
    const status = `${props.status || ""}`.toLowerCase();
    if (status === "completed") {
      if (task.ownerRole === "worker" || task.ownerRole === "tester") {
        task.status = TASK_STATUS.READY_FOR_REVIEW;
        task.statusReason = `${task.ownerRole} complete`;
        if (task.ownerRole === "tester") {
          task.checksStatus = "completed";
        }
      } else if (task.ownerRole === "reviewer") {
        task.status = TASK_STATUS.COMPLETE;
        task.statusReason = "review complete";
      } else {
        task.status = TASK_STATUS.COMPLETE;
      }
      task.blockedBy = undefined;
    } else {
      task.status = TASK_STATUS.FAILED;
      task.blockedBy = BLOCKED_BY.ERROR;
      task.statusReason = status || "run failed";
    }
    return { changed: true, actions: [] };
  }

  return { changed: false, actions: [] };
}

export function applyEvent(registry, event, context) {
  const actions = [];
  let changed = false;
  if (event.type === "permission.replied") {
    const reqId = event.properties?.requestID;
    const taskId = reqId ? context.requestToTask.get(reqId) : undefined;
    if (taskId && registry.tasks[taskId]) {
      const out = transitionTask(registry.tasks[taskId], event, context);
      return { changed: out.changed, actions: out.actions };
    }
  }
  for (const task of Object.values(registry.tasks || {})) {
    if (!matchesTask(task, event.properties || {})) continue;
    const out = transitionTask(task, event, context);
    changed = changed || out.changed;
    actions.push(...out.actions);
  }
  return { changed, actions };
}
