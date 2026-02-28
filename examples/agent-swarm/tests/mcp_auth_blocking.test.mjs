import test from "node:test";
import assert from "node:assert/strict";
import { applyEvent, createMapperContext } from "../src/event_mapper.mjs";

function registry() {
  return {
    version: 1,
    updatedAtMs: 0,
    tasks: {
      t1: {
        taskId: "t1",
        title: "T1",
        ownerRole: "worker",
        status: "running",
        sessionId: "s1",
        runId: "r1",
        worktreePath: "/tmp/w1",
        branch: "swarm/t1",
        lastUpdateMs: 0,
        notifyOnComplete: true,
      },
    },
  };
}

test("repeated mcp.auth.required blocks task and does not loop notifications", () => {
  const state = registry();
  const ctx = createMapperContext();
  const evt = {
    type: "mcp.auth.required",
    properties: { sessionID: "s1", runID: "r1", message: "OAuth required" },
  };

  const first = applyEvent(state, evt, ctx);
  const second = applyEvent(state, evt, ctx);

  assert.equal(state.tasks.t1.status, "blocked");
  assert.equal(state.tasks.t1.blockedBy, "auth");
  assert.equal(first.actions.length, 1);
  assert.equal(second.actions.length, 0);
});
