# TUI Test Suite: Deterministic + Agent-Driven (tandem-tui)

## Why this exists
`tandem-tui` has complex state transitions (multi-agent chat, request center, planning wizard, task sync). We need two complementary systems:

1. Deterministic tests for CI gating.
2. AI-driven exploratory runs for bug discovery and repro capture.

The highest-risk area is **Plan mode** request handling and plan-wizard transitions.

---

## Current codebase reality (Feb 26, 2026)

- TUI crate: `crates/tandem-tui`
- Main loop + terminal IO: `crates/tandem-tui/src/main.rs`
- State machine/reducer + keymap: `crates/tandem-tui/src/app.rs`
- Rendering: `crates/tandem-tui/src/ui/mod.rs`
- Existing tests are mostly reducer/keymap tests inside `app.rs` (`#[cfg(test)]`).
- There is no dedicated `crates/tandem-tui/tests/` integration suite yet.
- No explicit `--test-mode` flag exists yet.

---

## Goals

1. Cover all major TUI flows with deterministic tests.
2. Add PTY end-to-end coverage for real key input + real rendered terminal output.
3. Add an AI agent runner that can drive the TUI, detect UX failures, and emit repro artifacts.
4. Convert discovered bugs into deterministic regression tests.

---

## Priority risk map (what to test first)

1. **Plan mode guardrails**
- `PromptTodoUpdated` pending-plan guard (`plan_awaiting_approval`)
- `PlanFeedbackWizard` open/close/edit/submit behavior
- clarification-question loop (`plan_waiting_for_clarification_question`)

2. **Request Center reliability**
- malformed/empty `question` payload handling
- request cursor consistency after resolve/reject
- stale modal close behavior when queue empties

3. **Agent navigation + focus correctness**
- `Tab`/`Shift+Tab`, `Alt+1..9`, `Alt+G`, grid paging (`[` and `]`)
- active agent consistency when events arrive from non-active agents

4. **Tasks/todos projection consistency**
- tool/request todo payloads reflected in both session-level and agent-level task lists
- task preview/fingerprint transitions in plan mode

---

## Test pyramid for this repo

## 1) Reducer tests (CI-gating, highest ROI)

Place in `crates/tandem-tui/src/app.rs` test module initially (or move to split modules later).

Add tests for:

- `Action::PromptTodoUpdated`:
  - opens `ModalState::PlanFeedbackWizard` only when fingerprint changes
  - sets `plan_awaiting_approval` when payload is all-pending in Plan mode
  - ignores duplicate all-pending updates while awaiting approval
- `Action::PlanWizardSubmit`:
  - with all fields empty => clarification question follow-up path
  - with edited fields => feedback markdown path and proper queue/dispatch behavior
- `Action::PromptMalformedQuestion` and malformed `PromptRequest::Question`:
  - rejects request, removes from queue, and issues one retry prompt only
- Request center cursor invariants:
  - resolve/reject never leaves `request_cursor` out of bounds
  - modal closes when request list becomes empty
- Keymap invariants in `handle_key_event`:
  - modal-specific key routing for PlanFeedbackWizard and RequestCenter
  - no accidental action fallthrough when modal is active

## 2) Render tests (stable screen assertions)

Create `crates/tandem-tui/tests/render_views.rs` using `ratatui::backend::TestBackend`.

Snapshot/assert key content for:

- chat view with request panel collapsed/expanded
- Plan Feedback Wizard modal with empty + populated fields
- request center permission card and question card
- status bar mode/activity rendering (`Plan`, request counts, active agent)

Guideline: assert semantic text markers, not fragile column-perfect layout.

## 3) PTY e2e tests (real terminal behavior)

Create:

- `crates/tandem-tui/tests/e2e_pty_basic.rs`
- `crates/tandem-tui/tests/support/pty_harness.rs`

Use:

- `portable-pty` to spawn binary
- `vt100` to parse ANSI output into a screen model

Harness API:

- `send_key(...)`
- `send_text(...)`
- `wait_for_text("...", timeout)`
- `screen_text()`
- `dump_artifacts(path)`

Minimum PTY cases:

- open help modal (`F1`) then close (`Esc`)
- switch agent (`Tab`/`Shift+Tab`) and verify active agent marker changes
- open request center (`Alt+R`) in controlled test scenario and confirm navigation keys work

---

## Test mode requirements (add this first)

Add `TANDEM_TUI_TEST_MODE=1` (or `--test-mode`) so tests can run deterministically.

In test mode:

- disable spinner/throbber animation
- disable startup animation delays
- disable time-dependent status noise where possible
- force `TANDEM_TUI_SYNC_RENDER=off`
- optionally render a compact debug line with:
  - state name
  - active modal
  - active agent id/index
  - request count/cursor
  - plan flags (`awaiting_approval`, `waiting_for_clarification`)

This debug line makes PTY assertions and AI exploration much more reliable.

---

## AI exploratory runner design

Create binary: `crates/tandem-tui/src/bin/tandem-tui-agent-runner.rs`

Responsibilities:

1. Spawn `tandem-tui` in PTY with test mode.
2. Capture frames (ANSI -> vt100 screen text).
3. Execute action sequences from strict JSON.
4. Record evidence artifacts for every run.

### JSON action protocol

```json
{
  "goal": "string",
  "step": 1,
  "actions": [
    { "type": "key", "value": "UP|DOWN|LEFT|RIGHT|ENTER|ESC|TAB|BACKTAB|F1|CHAR:x|CTRL:x|ALT:x" },
    { "type": "text", "value": "literal typed text" },
    { "type": "wait_ms", "value": 120 }
  ],
  "assertions": [
    { "type": "contains", "value": "expected substring" },
    { "type": "not_contains", "value": "unexpected substring" }
  ],
  "notes": "short reasoning",
  "bug": null
}
```

### Artifact bundle (required)

For each run, save:

- `run.jsonl` (step/action log with timestamps)
- `last_frame.txt`
- `frame_history/NNN.txt` (rolling window)
- `assertions.json`
- `bug_report.json` (if bug found)

Bug report schema:

- `title`
- `expected`
- `actual`
- `repro_actions`
- `evidence_frames`

---

## Exploratory scenarios (seed set)

1. Open Help (`F1`) and exit (`Esc`) repeatedly.
2. Cycle agents with `Tab`/`Shift+Tab`; verify active agent label changes.
3. Toggle focus/grid (`Alt+G`) and page grid (`[`/`]`) under multiple agents.
4. Open request center (`Alt+R`), navigate requests/options, close cleanly.
5. In Plan mode (`/mode plan`), verify plan wizard appears after todo updates.
6. In plan wizard, edit fields, submit, and verify follow-up text appears.
7. Trigger malformed question flow and verify one retry guidance message.
8. Ensure request cursor remains valid as requests are resolved/removed.
9. Rapid modal open/close (`Alt+R`, `Esc`, `F1`, `Esc`) without stuck focus.
10. Verify no silent failure indicators after permission/question resolution.

---

## Deterministic regression loop for discovered bugs

1. Agent runner finds bug and writes artifact bundle.
2. Convert repro into reducer test first (preferred).
3. If reducer test cannot represent bug, add PTY test.
4. Keep the exploratory scenario in nightly suite.

Rule: every confirmed planning-mode bug gets a reducer regression test.

---

## Keybindings for automation (from `handle_key_event`)

Global/Chat:

- `F1`: help modal
- `F2`: docs
- `Alt+M`: cycle mode
- `Alt+G`: focus/grid toggle
- `Alt+R`: open request center
- `Tab` / `Shift+Tab`: next/prev agent
- `Alt+1..Alt+9`: jump to agent
- `[` / `]`: grid page prev/next
- `Enter`: submit command
- `Shift+Enter` or `Alt+Enter`: newline
- `Ctrl+C`: cancel run (double-tap quit behavior)
- `Ctrl+X`: quit

Plan Feedback Wizard modal:

- `Tab`/`Down`: next field
- `Shift+Tab`/`Up`: previous field
- text input + `Backspace`
- `Enter`: submit
- `Esc`: close

Request Center modal:

- `Up`/`Down`: request or option navigation
- `Left`/`Right`: option selection
- `1..3` (permission quick choice), `1..9` (question options when valid)
- `Space`: toggle question option
- `Ctrl+E`: expand/collapse panel
- `R`: reject request
- `Enter`: confirm/submit
- `Esc`: close

---

## CI and execution policy

PR-gating (required):

- reducer tests
- render tests
- a small PTY smoke subset

Nightly/manual (non-gating initially):

- full PTY suite
- AI exploratory scenarios

If flakes appear, quarantine only the flaky PTY/AI scenario, never reducer tests.

---

## Implementation checklist

- [ ] Add `TANDEM_TUI_TEST_MODE` behavior in runtime + rendering paths
- [ ] Add plan-mode reducer regression tests for wizard/approval/question edge cases
- [ ] Add request-center reducer invariant tests
- [ ] Add `TestBackend` render tests for key views/modals
- [ ] Add PTY harness (`portable-pty` + `vt100`) and smoke e2e tests
- [ ] Add `tandem-tui-agent-runner` with strict JSON protocol
- [ ] Add nightly scenario pack + artifact retention
- [ ] Document all commands in `docs/TUI_TESTING.md`

---

## Acceptance criteria

1. CI passes reliably on Linux with deterministic suites.
2. Planning mode has targeted regression coverage for known error-prone transitions.
3. Agent runner can produce reproducible bug bundles from real TUI interaction.
4. At least three exploratory-found bugs are converted into deterministic tests.
