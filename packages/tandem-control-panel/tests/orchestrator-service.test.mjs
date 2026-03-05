import assert from "node:assert/strict";
import test from "node:test";

import { deriveRunBudget } from "../server/services/orchestratorService.js";

test("deriveRunBudget uses relaxed advisory defaults when explicit run budget is missing", () => {
  const startedAtMs = Date.now() - 6_566_000;
  const budget = deriveRunBudget(
    {
      started_at_ms: startedAtMs,
      updated_at_ms: startedAtMs + 6_566_000,
    },
    [{ type: "task_started" }, { type: "task_failed" }],
    [{ id: "task-1" }]
  );

  assert.equal(budget.limits_enforced, false);
  assert.equal(budget.source, "derived");
  assert.equal(budget.max_iterations, 500);
  assert.equal(budget.max_tokens, 400000);
  assert.equal(budget.max_wall_time_secs, 7 * 24 * 60 * 60);
  assert.equal(budget.max_subagent_runs, 2000);
  assert.equal(budget.exceeded, false);
  assert.equal(budget.exceeded_reason, "");
});

test("deriveRunBudget respects explicit run budget caps and exceeded state", () => {
  const startedAtMs = Date.now() - 7_200_000;
  const budget = deriveRunBudget(
    {
      started_at_ms: startedAtMs,
      updated_at_ms: startedAtMs + 7_200_000,
      budget: {
        max_iterations: 20,
        max_tokens: 1000,
        max_wall_time_secs: 3600,
        max_subagent_runs: 10,
      },
    },
    [{ type: "task_started" }],
    [{ id: "task-1" }]
  );

  assert.equal(budget.limits_enforced, true);
  assert.equal(budget.source, "run");
  assert.equal(budget.max_wall_time_secs, 3600);
  assert.equal(budget.exceeded, true);
  assert.equal(budget.exceeded_reason, "One or more execution limits exceeded.");
});
