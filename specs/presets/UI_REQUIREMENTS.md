# UI Requirements

## Scope

Defines required preset and pack UI capabilities for:

- Tandem Tauri Desktop
- `packages/tandem-control-panel`

Both must use the same backend registry and contracts.

## A) Skill Library

Required features:

- browse/filter `SkillModule` by domain, tags, publisher, verification tier, required capabilities
- inspect module details:
  - description
  - version
  - required/optional capabilities
  - included prompt sections/fragments
  - policy profile reference
  - portability/non-portable status

## B) Agent Preset Library + Builder

Required features:

- create agent from existing `AgentPreset`
- compose agent from selected modules
- prompt preview (deterministic ordered output)
- capability summary and risk sheet
- fork/edit/save for immutable sources
- version pin visibility and update-available indicator

## C) Automation Preset Library + Builder

Required features:

- browse/install prebuilt `AutomationPreset`
- add/remove/edit mission steps
- bind each task to `AgentPreset` or composed agent
- routine controls: enable/disable, schedule edit
- capability summary at automation level
- fork/edit/save for immutable sources

Hard safety behavior:

- routines from installed packs shown disabled by default
- enable action requires explicit user confirmation

## D) Pack Library (First-Class Surface)

Required in both UIs:

- list installed packs
- install pack (upload/drag-drop/file/URL/attachment source)
- inspect manifest, contents, trust status, signature status, risk sheet
- uninstall pack
- export selected presets/agents/automations as pack
- update check and apply (when available)

## E) Chat Attachment Install UX

Required in both chat surfaces:

- on `pack.detected`, show inline card with:
  - Inspect
  - Install
  - Trust/signature status
- install action routes to PackManager install-from-attachment
- respect auto-install policy only for trusted source/publisher

## F) Search/Tagging/Versioning

- global search across modules, agent presets, automation presets, packs
- tag filters
- show source layer (`builtin`, `pack`, `project`, future `org`)
- show pinned version and update target
