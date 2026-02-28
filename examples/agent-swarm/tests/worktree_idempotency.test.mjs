import test from "node:test";
import assert from "node:assert/strict";
import { seedTasks } from "../src/swarm_orchestrator.mjs";
import { blankRegistry } from "../src/swarm_types.mjs";

test("repeated runs do not create duplicate worktrees for same taskId", async () => {
  const registry = blankRegistry();
  const tasks = [{ taskId: "dup", title: "Task Dup" }];

  let worktreeCalls = 0;
  const createWorktree = async (taskId) => {
    worktreeCalls += 1;
    return { worktreePath: `/repo/.swarm/worktrees/${taskId}`, branch: `swarm/${taskId}` };
  };

  const createSession = async (taskId) => ({ id: `s-${taskId}` });
  const startRun = async (_task, sessionId) => `r-${sessionId}`;

  await seedTasks({ registry, taskDefs: tasks, createWorktree, createSession, startRun });
  await seedTasks({ registry, taskDefs: tasks, createWorktree, createSession, startRun });

  assert.equal(worktreeCalls, 1);
  assert.equal(Object.keys(registry.tasks).length, 1);
  assert.equal(registry.tasks.dup.worktreePath, "/repo/.swarm/worktrees/dup");
});
