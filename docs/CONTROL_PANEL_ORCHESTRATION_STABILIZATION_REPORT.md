# Control Panel Orchestration Stabilization Report

Date: March 6, 2026

## Purpose

This report summarizes the stabilization and standardization work completed on Tandem's control-panel orchestration path.

The target design is:

```text
Run State (source of truth)
  -> Blackboard Projection
  -> UI Panels
```

The main goal of this work was to stop treating the blackboard as the primary mutable task store and move the web control panel toward a deterministic, run-scoped, multi-run-safe orchestration model.

## Starting Problems

Before this work:

- the control panel used a mix of global server state and per-run state
- the UI rebuilt task state from multiple sources
- `blackboard.tasks` behaved like a writable task authority
- `run_state.json` and blackboard task state could diverge
- task transitions were too permissive
- the backend had duplicate context-run implementations
- event append and run mutation ordering were not consistently serialized

## What Was Accomplished

### 1. Control-panel runtime was made run-scoped

The control-panel server no longer relies primarily on one global active swarm state for all orchestration runs.

Completed changes:

- added per-run controller state in `packages/tandem-control-panel/bin/setup.js`
- updated swarm/control routes to resolve and persist state by `runId`
- fixed executor mode, verification mode, workflow id, and workspace resolution to be run-local
- reduced cross-run leakage in status, retry, continue/resume, and verification behavior

Result:

- the control panel is substantially safer for concurrent runs
- one run's executor/verification state is much less likely to bleed into another run

### 2. The web UI now prefers one canonical task model

The control panel now renders orchestration tasks through a single projection layer instead of ad hoc task normalization scattered through the page.

Completed changes:

- added `packages/tandem-control-panel/src/features/orchestrator/blackboardProjection.ts`
- updated `OrchestratorPage.tsx` to use the projection module
- expanded UI task states to include `created`, `assigned`, and `validated`
- updated `TaskBoard.tsx` to render the stricter lifecycle model
- server-side swarm snapshot logic now prefers `run.tasks` before `blackboard.tasks`

Result:

- the web control panel is much closer to `Run -> Projection -> UI`
- frontend task rendering is more deterministic
- the browser is no longer acting like a second task authority

### 3. `run.tasks` became the effective backend task authority

The `/context/runs` backend now treats `run.tasks` as the canonical task store.

Completed changes:

- `ContextRunState` now includes first-class `tasks`
- task create/claim/transition handlers read authority from `run.tasks`
- integrations like pack-builder and skills-memory now update `run.tasks` first
- automation-v2 context-run projection now maintains task authority in `run.tasks`

Result:

- task state is now anchored in the run snapshot instead of blackboard task rows
- the system is materially closer to a canonical run-record architecture

### 4. Blackboard task authority was cut off

The blackboard is no longer the authoritative task persistence surface.

Completed changes:

- generic `POST /context/runs/{run_id}/blackboard/patches` now rejects task mutation ops
- persisted `blackboard.json` no longer stores task rows
- blackboard read responses project tasks from `run.tasks`
- replay/drift logic now compares against projected run-owned task state instead of raw persisted blackboard task rows

Result:

- blackboard is now functioning as a projection/compatibility layer rather than the main task store
- old blackboard payloads remain readable for backward compatibility

### 5. Task lifecycle protections were tightened

Task mutation is now more constrained.

Completed changes:

- invalid task lifecycle jumps are rejected
- active task mutations require a valid lease token
- task revision mismatch handling remains enforced
- task endpoints now reject unsafe transitions instead of silently accepting them

Result:

- the task system is less prone to random state jumps
- agents and callers have less ability to mutate task state arbitrarily

### 6. Duplicate backend implementations were reduced

There were effectively two overlapping context-run/task orchestration implementations in `crates/tandem-server`.

Completed changes:

- removed dead duplicate pack-builder context-run helper logic from `crates/tandem-server/src/http.rs`
- removed the dead duplicate local context-run storage/blackboard helper block from `crates/tandem-server/src/http.rs`
- updated the remaining test reference to point at the real `http/context_runs.rs` helper path

Result:

- there is now one live `/context/runs` backend implementation
- the codebase has less drift risk between "real" and "shadow" orchestration code

### 7. Event and mutation ordering improved

The event path and task mutation path are now closer to one serialized flow.

Completed changes:

- event append seq generation is guarded by a per-run event lock
- the public `/context/runs/{run_id}/events` endpoint now also respects the per-run run lock
- task create/claim/transition now use an internal event append helper instead of round-tripping through the public route handler
- added a shared `append_context_run_task_commit(...)` helper to centralize:
  - run task write
  - compatibility blackboard patch append
  - task event append

Result:

- fewer race windows between run mutation and task event emission
- less handler-specific drift in task mutation behavior

### 8. Regression tests were added for the new authority model

Completed coverage additions:

- blackboard task patch ops are rejected through the generic patch endpoint
- persisted `blackboard.json` omits task rows after task creation
- read-time blackboard projection still exposes tasks from `run.tasks`
- task event payloads continue to include `patch_seq` after the commit-helper refactor

Result:

- the new run-owned task model is now explicitly tested instead of being only an implementation detail

## Current Architecture After Stabilization

The current live control-panel path now behaves approximately like this:

```text
Control Panel UI
  -> control-panel server adapters
  -> tandem-server /context/runs/*
  -> run_state.json (run + tasks)
  -> events.jsonl
  -> blackboard_patches.jsonl
  -> blackboard.json (projection-compatible snapshot without task authority)
  -> projected blackboard/task view
  -> UI panels
```

More specifically:

- `run.tasks` is the effective task authority
- `blackboard.tasks` is a projected compatibility view
- the control panel prefers `run.tasks`
- multi-run state is much more run-scoped in the web layer

## Files Most Affected

### Control panel

- `packages/tandem-control-panel/bin/setup.js`
- `packages/tandem-control-panel/server/routes/swarm.js`
- `packages/tandem-control-panel/src/features/orchestrator/blackboardProjection.ts`
- `packages/tandem-control-panel/src/pages/OrchestratorPage.tsx`
- `packages/tandem-control-panel/src/features/orchestration/TaskBoard.tsx`
- `packages/tandem-control-panel/src/features/orchestration/types.ts`

### Backend

- `crates/tandem-server/src/http/context_runs.rs`
- `crates/tandem-server/src/http/context_types.rs`
- `crates/tandem-server/src/http/pack_builder.rs`
- `crates/tandem-server/src/http/skills_memory.rs`
- `crates/tandem-server/src/http.rs`
- `crates/tandem-server/src/http/tests/context_runs.rs`

## What Is Better Now

The system is not fully finished, but these improvements are real:

- the web control panel is much less likely to show cross-run contamination
- the UI is substantially more deterministic
- blackboard is no longer the main task authority
- the backend has one live context-run implementation instead of shadow duplicates
- task writes now follow a more consistent run-first path
- task event emission is more closely aligned with run mutation
- the backend is much closer to a stable multi-run orchestration runtime

## Phase 2 Run Engine

The backend now has an explicit Run Engine commit path.

```text
Run State snapshot
  <- recovered from events.jsonl when needed
ContextRunEngine
  -> events.jsonl (authoritative ordered mutation history)
  -> run_state.json (current snapshot cache)
  -> blackboard_patches.jsonl (compatibility projection stream)
  -> blackboard.json (compatibility projection snapshot)
  -> UI panels
```

Key changes in Phase 2:

- `ContextRunEngine` is now the intended mutation boundary for task commits, meta run events, and blackboard compatibility patches
- task create/claim/transition handlers use engine commits instead of directly mutating files
- pack-builder, skills-memory, automation-v2 sync, and workflow sync now route task/projection updates through the engine commit path
- authoritative event records now carry revision metadata and optional `task_id` / `command_id`
- `run_state.json` and `blackboard.json` writes now use temp-file replacement instead of direct overwrite
- the public `POST /context/runs/{run_id}/events` endpoint rejects `context.task.*` events so task authority only flows through task mutations

This does not make multi-file persistence magically transactional at the filesystem level, but it does establish one ordered per-run commit flow and a journal-first recovery path for run snapshots.

### Blackboard role after Phase 2

The blackboard remains projection-only.

- `run.tasks` is still the task authority
- `events.jsonl` is the authoritative mutation history for committed run changes
- `blackboard_patches.jsonl` and `blackboard.json` exist for compatibility, replay, and UI projection
- task state must not be sourced from UI or blackboard payloads

## What Still Remains

The major authority and commit-order problems are addressed, but cleanup remains:

- remove or fold the unused legacy helper functions and old lock maps still present in `crates/tandem-server/src/http/context_runs.rs`
- tighten visibility so the new engine surface does not emit Rust `private_interfaces` warnings
- extend crash-recovery tests from targeted regression coverage to broader replay/repair matrix coverage

## Bottom Line

The control-panel orchestration path is materially more stable than it was at the start of this work.

The biggest architectural inversion is now in place:

```text
run.tasks -> blackboard projection -> control-panel UI
```

That is the core correction required to make Tandem's web control panel behave more like an autonomous orchestration surface and less like a collection of loosely coupled mutable views.

The remaining work is mostly about making persistence more atomic and removing the last compatibility/legacy burden, not about rediscovering the source of truth.
