---
description: how to add a new HTTP handler test to tandem-server
---

> [!IMPORTANT]
> Never add tests directly to `http.rs` or `http/tests/mod.rs`.
> Always add to the matching domain file under `http/tests/`.

## Steps

1. Identify which domain your handler belongs to:

| Domain file       | What goes here                                                                  |
| ----------------- | ------------------------------------------------------------------------------- |
| `sessions.rs`     | Session CRUD, prompt_async/sync, run lifecycle, message append, event streaming |
| `packs.rs`        | Pack detect, install, uninstall, list, update                                   |
| `pack_builder.rs` | Pack builder preview, apply, cancel, confirm                                    |
| `presets.rs`      | Presets index, compose, fork, export, overrides                                 |
| `capabilities.rs` | Capability resolve, readiness checks                                            |
| `permissions.rs`  | Permission reply routes, tool approve/deny                                      |
| `context_runs.rs` | Context run create/events/replay, blackboard patches, tasks                     |
| `routines.rs`     | Routines create/patch/run_now, automations, routine event contracts             |
| `missions.rs`     | Mission create, start, cancel, budget exhaustion                                |
| `memory.rs`       | Memory put/search/promote/demote/admin routes                                   |
| `resources.rs`    | Shared resource put/get/list/delete, SSE events                                 |
| `agent_teams.rs`  | Agent team spawn, cancel, policy, capability gates                              |
| `global.rs`       | Health, readiness gate, admin routes, path sanitization                         |
| `providers.rs`    | Provider catalog route, merge_known_provider_defaults                           |
| `channels.rs`     | Channel config + verification routes                                            |

2. Open `crates/tandem-server/src/http/tests/<domain>.rs`

3. Add your test inside the existing body of the file (no wrapper `mod` needed —
   the file is already a submodule of `http::tests`):

```rust
#[tokio::test]          // async tests
// OR
#[test]                 // sync tests
async fn my_new_test() {
    let state = test_state().await;
    let app = app_router(state.clone());
    // ...
}
```

4. Available helpers (from `super::*` which re-exports `mod.rs`):

- `test_state() -> AppState` — fresh isolated app state
- `next_event_of_type(&mut rx, "event.type") -> EngineEvent` — wait for an event
- `write_pack_zip(path, manifest)` — create a test pack zip
- `write_plain_zip_without_marker(path)` — create a zip without the pack marker

// turbo 5. Run your test to verify:

```bash
cargo test -p tandem-server <my_new_test>
```
