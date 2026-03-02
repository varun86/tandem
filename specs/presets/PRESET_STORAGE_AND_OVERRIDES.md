# Preset Storage and Overrides

## Scope

Defines storage layers, fork semantics, update behavior, rollback, and conflict handling.

## Storage Layers

## Built-ins (read-only)

- Source: runtime-shipped templates/presets
- Editable: no

## Installed packs (read-only)

- Source: `TANDEM_HOME/packs/<name>/<version>/...`
- Editable: no

## Project overrides (editable)

- Source: `<workspace>/.tandem/presets/...`
- Editable: yes

## Org shared layer (future)

- Source: `TANDEM_HOME/org-presets/...`
- Editable: policy-controlled

## Registry Precedence

Effective view resolution order:

1. project overrides
2. org shared
3. installed packs
4. built-ins

## Fork Semantics

## Use

- Keeps upstream reference pinned to exact version.
- No local copy.

## Fork

- Creates local editable copy in project overrides.
- Source remains immutable.

Fork metadata:

```yaml
fork_of:
  source_layer: pack
  source_ref: "pack:tpk_01...@1.0.0"
  upstream_id: tandem.agent.github.pr_worker
  upstream_version: 1.0.0
  tracking: true
```

## Update (for tracking forks)

- Compare local fork against upstream new version.
- Produce structured diff:
  - prompt fragments
  - capabilities
  - policies
  - routines/triggers
- If permission scope increases, re-approval required.

## Detach

- Tracking can be disabled.
- Further upstream updates are informational only.

## Rollback Rules

- Project overrides keep revision snapshots.
- Rollback target selected by revision id.
- Rollback never mutates upstream pack content.

## Conflict Rules

- Optimistic concurrency via `base_revision`.
- Save fails with conflict when latest revision differs.
- Client can inspect diff and retry merge/save.

## Immutability Enforcement

- Any write request targeting built-in/pack path returns `PRESET_IMMUTABLE_SOURCE`.
- Edit action in UI must route to fork flow automatically.

## Atomicity Requirements

- Override writes: temp file + atomic rename.
- Registry index updates: temp + rename.
- Per-preset lock key for concurrent edit operations.
