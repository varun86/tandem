# TUI Testing

## Deterministic reducer tests

Run existing + new reducer tests:

```bash
cargo test -p tandem-tui app::tests
```

Planning-mode regressions added in `crates/tandem-tui/src/app.rs`:
- `plan_mode_prompt_todo_updated_opens_wizard_and_sets_approval_guard`
- `plan_mode_duplicate_all_pending_todo_update_is_ignored_while_awaiting_approval`
- `malformed_question_retry_prompt_is_dispatched_once_per_request_id`

## PTY smoke test scaffold

Smoke test file:
- `crates/tandem-tui/tests/e2e_pty_smoke.rs`

Run (explicitly opt in because it is ignored by default):

```bash
cargo test -p tandem-tui --test e2e_pty_smoke -- --ignored --nocapture
```

Artifacts are written to:
- `.tmp/tui-pty-smoke`

## AI exploratory runner

Binary:
- `crates/tandem-tui/src/bin/tandem-tui-agent-runner.rs`
- Optional override env: `TANDEM_TUI_BIN=/abs/path/to/tandem-tui`

Run:

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner
```

Run with scenario file flag:

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner -- --scenario-file crates/tandem-tui/tests/agent_scenarios.jsonl
```

Optional custom artifact directory:

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner -- --scenario-file crates/tandem-tui/tests/agent_scenarios.jsonl --artifact-dir .tmp/tui-agent-runs/manual
```

Record canonical step lines while executing:

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner -- --scenario-file crates/tandem-tui/tests/agent_scenarios.jsonl --record-scenario-out .tmp/tui-agent-runs/recorded_scenario.jsonl
```

Replay a prior failed run log:

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner -- --replay-run .tmp/tui-agent-runs/<run-id>/run.jsonl
```

Limit steps (for quick smoke):

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner -- --scenario-file crates/tandem-tui/tests/agent_scenarios.jsonl --max-steps 3
```

Tune startup wait if engine/bootstrap is slow:

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner -- --scenario-file crates/tandem-tui/tests/agent_scenarios.jsonl --startup-wait-ms 1200
```

Protocol:
- Runner prints JSON observation lines.
- Send one JSON step per stdin line.
- Each step can include `actions`, `assertions`, and optional `bug` report.

Example input line:

```json
{"goal":"open help","step":1,"actions":[{"type":"key","value":"F1"},{"type":"wait_ms","value":120}],"assertions":[{"type":"contains","value":"Modal"}],"notes":"smoke"}
```

Artifacts are written to:
- `.tmp/tui-agent-runs/<timestamp>/`
- includes `run.jsonl`, `last_frame.txt`, `frame_history/`, and optional `bug_report.json`.
- `run.jsonl` now embeds `step_input`, so failed runs can be replayed directly with `--replay-run`.

`stdin` piping still works:

```bash
cargo run -p tandem-tui --bin tandem-tui-agent-runner < crates/tandem-tui/tests/agent_scenarios.jsonl
```

## Test mode

Set deterministic mode for automation:

```bash
TANDEM_TUI_TEST_MODE=1 TANDEM_TUI_SYNC_RENDER=off cargo run -p tandem-tui
```

Skip engine bootstrap entirely for UI automation:

```bash
TANDEM_TUI_TEST_MODE=1 TANDEM_TUI_TEST_SKIP_ENGINE=1 TANDEM_TUI_SYNC_RENDER=off cargo run -p tandem-tui
```

Effects:
- synchronized rendering disabled
- startup gating reduced for automation
- animated spinner surfaces frozen
- status line includes compact `TEST ...` debug suffix for assertions

## Crates/network requirements

The PTY harness and runner require crates:
- `portable-pty`
- `vt100`

If dependency resolution fails, ensure network access to:
- `index.crates.io`
- `crates.io`
- `static.crates.io`

Then run:

```bash
cargo fetch --locked
cargo test -p tandem-tui --no-run
```

For offline CI, pre-warm and cache cargo registry/git after a successful fetch.

Common failure:
- `no matching package named portable-pty found` while using `--offline`
Fix:
1. Run one online fetch (`cargo fetch --locked`)
2. Re-run offline checks/tests after cache is populated
