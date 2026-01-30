# AI Agent Prompt: Add “Ralph Loop” Mode to Tandem (Rust + Tauri + React)

You are a senior engineer working directly inside the Tandem codebase. You have full access to the repo and can modify any files. Your task is to implement a Tandem-native **Ralph Loop**: an iterative run mode that repeatedly calls the OpenCode sidecar until a completion promise is detected, with pause/cancel and context injection, and tight integration with Tandem’s existing Plan Mode + permission staging.

## UX Decision (MANDATORY)

Implement Ralph Loop as a **toggle button in the lower chat control bar** where model selection exists (bottom area of the chat input).

- Do NOT implement Ralph as a separate agent persona “mode” like ask/plan.
- Ralph is an **orchestration/run-policy toggle**, not a reasoning style.
- The toggle should be near model selection (and near Plan Mode toggle if present).

UI Requirements:

- Add a toggle labeled: **“Loop”** (tooltip: “Ralph loop: iterate until complete”).
- When enabled, sending a message should **start the loop** (or continue it if already running).
- Show an inline status chip in the same control area when loop is active:
  - Example: `Loop • Running • Iter 3` / `Loop • Paused • Iter 3`
  - Clicking the chip opens a Ralph panel (or side panel / modal) with details and controls.
- Add panel controls:
  - Pause / Resume
  - Cancel
  - Add Context (text input that injects into next iteration)
  - View History (open history file(s) in existing file viewer if possible)

## Reference Implementation Link (MANDATORY)

In documentation and code comments include this reference:
https://raw.githubusercontent.com/Th0rgal/open-ralph-wiggum/refs/heads/master/ralph.ts

Add it to:

1. `docs/ralph-loop/README.md` under “Inspiration / Reference”
2. A header comment in the Rust module implementing the loop

---

## Goal (What we’re building)

A new feature: **Ralph Loop Mode** per session.

Ralph Loop:

- Takes a user prompt (task)
- Iteratively runs the OpenCode sidecar with an iteration-aware prompt builder
- Stops only when:
  - `<promise>COMPLETE</promise>` detected (promise configurable), AND
  - `min_iterations` reached, AND
  - optional verifier passes (v1: no verifier, but design for it)
- Tracks iteration history:
  - iteration number
  - duration
  - tools used counts (best-effort)
  - file changes list (best-effort)
  - errors summary (best-effort)
- Allows user to inject context mid-loop for next iteration
- Can be cancelled gracefully anytime
- Integrates safely with Tandem’s existing Plan Mode / staging:
  - If Plan Mode is enabled, Ralph Loop must NEVER auto-execute staged operations.
  - Ralph Loop can iterate planning, but execution remains user-gated via existing UI.

---

## Non-goals (v1)

- No third-party plugin systems
- No Bun requirement
- No external installs
- Don’t try to match Th0rgal’s CLI perfectly; implement the core value

---

## Step 0: Repo Recon (MANDATORY FIRST STEP)

Before coding:

1. Locate how Tandem currently:
   - Starts/identifies an OpenCode session (session id)
   - Sends a user message to the sidecar
   - Receives streaming events / message completion signals
   - Intercepts tool permissions (`permission_asked`) and stages them in plan mode
2. Identify where to persist workspace-local files.
3. Find existing bottom chat control bar component (where model selection exists) and how toggles are implemented.

Deliverable for Step 0:

- Create `docs/ralph-loop/RECON.md` with:
  - key file paths and symbols you’ll integrate with
  - which event(s) represent “agent run finished”
  - where to add the toggle and status chip in the UI

Do not proceed until this doc is created.

---

## Architecture Constraints

### Storage: Workspace-local directory

Create a workspace-local directory:
`.opencode/tandem/ralph/`

Files:

- `state.json` (current loop state)
- `context.md` (pending injected context)
- `history.json` (iteration history)
  Optionally:
- `summary.md` (human-friendly summary of latest status)

Rules:

- Keep it per workspace, not in home directory.
- Don’t let history grow unbounded (cap to last 50 iterations in file).

### Concurrency & Cancel Safety

Use tokio tasks + cancellation token (or equivalent). No orphan tasks.

- Cancel should stop promptly.
- Pause should stop between iterations (does not start a new iteration).

---

## Implementation Tasks (You MUST complete these)

### 1) Backend: Ralph loop module (Rust)

Create a module structure like:

- `src-tauri/src/ralph/mod.rs`
- `src-tauri/src/ralph/service.rs`
- `src-tauri/src/ralph/storage.rs`
- `src-tauri/src/ralph/types.rs`

Types:

- `RalphConfig { min_iterations, max_iterations, completion_promise, allow_all_permissions, plan_mode_guard }`
- `RalphRunStatus = Idle | Running | Paused | Completed | Cancelled | Error`
- `RalphState { run_id, session_id, active, status, iteration, started_at, prompt, config }`
- `IterationRecord { iteration, started_at, ended_at, duration_ms, completion_detected, tools_used: map<string,int>, files_modified: string[], errors: string[] }`

### 2) Backend: Tauri commands

Expose commands:

- `ralph_start(session_id, prompt, config) -> { run_id }`
- `ralph_cancel(run_id) -> void`
- `ralph_pause(run_id) -> void`
- `ralph_resume(run_id) -> void`
- `ralph_add_context(run_id, text) -> void`
- `ralph_status(run_id) -> RalphStateSnapshot`
- `ralph_history(run_id, limit) -> IterationRecord[]`

Store active run state in app state (e.g., `AppState`) keyed by session id.

### 3) Sidecar integration (DO NOT SPAWN EXTERNAL CLI)

Run the agent by reusing existing sidecar call path:

- Use the same mechanisms Tandem uses today to send messages and read streaming output.
- Buffer assistant output for the iteration to detect completion promise.
- Capture tool usage counts if you can from existing event stream parsing; otherwise implement “best effort”:
  - If tool events are visible, count them.
  - If not, omit tool counts cleanly.

### 4) Completion detection (strict)

Detect completion by regex:
`(?i)<promise>\s*{completion_promise}\s*</promise>`

Rules:

- Do NOT stop if `iteration < min_iterations`.
- If completion detected but verifier fails (future), continue and add failure summary to next prompt.

### 5) Progress detection (best-effort file change tracking)

If the workspace is a git repo:

- Use `git status --porcelain` before and after each iteration.
- Use `git diff --name-only` to list modified files.
  If not a git repo:
- Skip file tracking without error.

Also add basic struggle detection:

- If no modified files for 3 consecutive iterations OR repeated error lines 2+ times,
  mark `struggle=true` and inject “struggle hint” section into next prompt.

### 6) Prompt builder (core)

Implement:
`build_prompt(iteration, base_prompt, context_md, struggle_summary, config, plan_mode_enabled) -> String`

It must include:

- Iteration header
- The user task prompt
- “Additional Context” section if present
- “Last iteration errors summary” section if present
- Critical rules:
  - Only output the promise when genuinely complete
  - Don’t lie to exit the loop
- The exact required completion token:
  - `When complete, output: <promise>{completion_promise}</promise>`

Plan Mode behavior:

- If Plan Mode is enabled, include explicit instruction:
  - “Do not execute changes directly; stage operations and update plan markdown; wait for user approval to execute.”
- Ralph Loop must not bypass staging.

### 7) Storage implementation

Write and read:

- `state.json`
- `history.json`
- `context.md`

Rules:

- Only clear context if it existed at iteration start (preserve mid-iteration additions).
- Append iteration records; cap to last 50.
- If a loop crashes, persist `status=Error` and expose error to UI.

### 8) Frontend UI: bottom bar toggle + status chip + panel

Implement in the chat input area (same place as model selection):

- Toggle: `Loop`
- Status chip shown only when loop state for this session is active:
  - Running / Paused / Completed / Error
  - iteration count

Panel:

- Minimal panel (drawer/modal/sidebar is fine):
  - Current status
  - Iteration number
  - Last iteration duration
  - Modified files count/list (if available)
  - Errors summary
  - Buttons: Pause/Resume, Cancel
  - Add Context input and submit
  - View History (opens `history.json` or `summary.md` in your file viewer if possible)

Frontend ↔ backend:

- Use `invoke(...)` calls to new Tauri commands.
- Subscribe to loop status updates:
  - Either poll `ralph_status` every ~1s when active,
  - or emit events from Rust (preferred if existing event bus is used).

---

## Safety / UX Rules (must obey)

- Cancel stops promptly and marks loop inactive in `state.json`.
- Pause stops after current iteration finishes; doesn’t start new iteration.
- Never auto-approve filesystem edits in Plan Mode.
- If sidecar fails mid-run, surface error and stop loop cleanly.
- The loop must not wedge the UI; operations run in background.

---

## Deliverables

1. Working v1 implementation of Loop toggle + backend orchestrator
2. `docs/ralph-loop/README.md` documenting:
   - what Loop is
   - where files are stored
   - how it works with Plan Mode
   - how to add context / pause / cancel
   - include the reference link:
     https://raw.githubusercontent.com/Th0rgal/open-ralph-wiggum/refs/heads/master/ralph.ts
3. `docs/ralph-loop/RECON.md` from Step 0

---

## Acceptance Criteria (must pass)

- Toggle appears in bottom chat control bar near model selection.
- Starting a loop creates `.opencode/tandem/ralph/state.json`, `history.json`, and optionally `context.md`.
- Loop iterates until completion promise detected (respecting min iterations).
- Add Context affects next iteration prompt.
- Pause/Resume works (iteration-boundary pause).
- Cancel stops loop and updates state correctly.
- Works when Plan Mode is ON (no auto execution) and OFF.

---

## Execution Plan

Start now:

1. Complete Step 0 and write `docs/ralph-loop/RECON.md`.
2. Implement backend loop service and Tauri commands.
3. Implement UI toggle + status chip + panel.
4. Add docs + manual test checklist.

Proceed in that order.
