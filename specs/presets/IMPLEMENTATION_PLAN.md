# Preset and Pack Implementation Plan

## Scope

Rust-first implementation plan for PackManager + PresetRegistry + CapabilityRuntime.

## Rust Module Breakdown

## New crate: `crates/tandem-packs`

- `lib.rs`
- `manifest.rs`
- `detector.rs`
- `safe_zip.rs`
- `installer.rs`
- `locks.rs`
- `index.rs`
- `export.rs`
- `trust.rs`
- `risk.rs`

## New crate: `crates/tandem-presets`

- `lib.rs`
- `types.rs`
- `registry.rs`
- `loader.rs`
- `compose.rs`
- `overrides.rs`
- `diff.rs`
- `errors.rs`

## New crate: `crates/tandem-capabilities`

- `lib.rs`
- `types.rs`
- `catalog.rs`
- `bindings.rs`
- `resolver.rs`
- `canonical_schema.rs`

## Server integration (`crates/tandem-server`)

- add `/packs/*`, `/presets/*`, `/capabilities/*`, `/attachments/*` routes
- emit lifecycle events on existing event bus
- add chat/attachment ingestion bridge

## Data Structures

- `PackManifest`
- `InstalledPack`
- `PackInstallRecord`
- `PackIndex`
- `PackRiskSheet`
- `SkillModule`
- `AgentPreset`
- `AutomationPreset`
- `PresetOverride`
- `ResolvedPreset`
- `CapabilityBinding`
- `CapabilityResolution`

## Storage and Atomicity

## Pack storage

- `TANDEM_HOME/packs/<name>/<version>/`
- `TANDEM_HOME/packs/index.json`
- `TANDEM_HOME/packs/<name>/current`
- `TANDEM_HOME/packs/_staging/<install_id>/`

## Atomic install

1. stage zip and validation output
2. safe extract into staging
3. validate manifest + contents completeness
4. acquire per-pack lock
5. atomic move staging -> final version dir
6. atomic index write (tmp + rename)
7. update `current` pointer
8. release lock

## Effective Preset Resolution

1. locate all layers for requested id
2. select winning source by precedence
3. if override tracks upstream, include upstream metadata
4. compose prompt deterministically
5. merge capabilities/policies
6. compute `composition_hash`

## Capability Runtime Integration

1. provider discovery via `list_tools()` with schemas
2. bindings loaded from data files
3. resolver applies precedence:

- user override
- org policy
- pack preference
- system default

4. runtime executes resolved provider tool while preserving capability-level contracts

## Frontend Integration

## Desktop (Tauri)

- add commands wrapping new endpoints
- add Pack Library panel
- extend Agent/Automation builders to use PresetRegistry

## Control Panel

- add pack view + actions in `views/agents.js` (or dedicated `views/packs.js`)
- add module/agent/automation preset browsing and resolve preview
- add fork/edit/update UI flows

## Test Plan

## Unit Tests

- deterministic prompt composition hash stability
- capability merge rules
- policy least-privilege merge
- immutable source write rejection
- manifest parser and contents completeness checks

## Integration Tests

- install from attachment id path
- install from URL/file path
- concurrent install lock behavior
- index and pointer atomic update behavior
- fork/override lifecycle
- update diff + scope-increase approval gate

## Regression Tests

- traversal and zip bomb rejection
- secret scanner rejection path
- routine default-disabled post-install
- non-portable dependency flagging

## UI Smoke Checklist

- both UIs list same packs and preset counts
- both UIs inspect same resolved prompt/capability summary
- both UIs can fork immutable preset and edit local copy
- both UIs render chat install card on `pack.detected`
- both UIs show trust/signature status

## Worked Example Coverage in Tests

- 3 modules + 1 agent preset + 1 automation preset fixture
- assert computed capability summary equals expected unions
- assert composed prompt exactly matches fixture snapshot
