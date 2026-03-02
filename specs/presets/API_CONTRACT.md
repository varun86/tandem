# Preset and Pack API Contract

## Scope

Shared backend contract for Desktop and Control Panel.

## PackManager APIs

## `GET /packs`

Returns installed pack list.

Response:

```json
{
  "packs": [
    {
      "pack_id": "tpk_01...",
      "name": "github-pr-workflow",
      "version": "1.0.0",
      "source": "marketplace",
      "publisher": {
        "publisher_id": "pub_tandem_official",
        "display_name": "Tandem",
        "verification_tier": "official"
      },
      "trust_status": "trusted",
      "signature_status": "valid",
      "installed_at_ms": 1772400000000
    }
  ]
}
```

## `GET /packs/{id}`

Returns manifest + validated contents + risk sheet.

## `POST /packs/install`

Install from one source:

- uploaded file path
- URL
- `attachment_id`

Request:

```json
{
  "source": {
    "type": "attachment",
    "attachment_id": "att_123"
  },
  "confirm": true
}
```

## `POST /packs/uninstall`

```json
{ "pack_id": "tpk_01...", "version": "1.0.0" }
```

## `POST /packs/export`

Export selected entities into pack zip.

```json
{
  "selection": {
    "skill_modules": ["tandem.skill.github.core@1.0.0"],
    "agent_presets": ["tandem.agent.github.pr_worker@1.0.0"],
    "automation_presets": ["tandem.automation.github.pr_triage@1.0.0"]
  },
  "output_path": "/tmp/github-pr-pack.zip"
}
```

## `GET /packs/{id}/updates`

Return available updates (future-compatible).

## `POST /packs/{id}/update`

Apply selected update.

## Preset APIs

## `GET /presets/skill-modules`

## `GET /presets/agent-presets`

## `GET /presets/automation-presets`

Supports filters: source layer, publisher, tags, capabilities.

## `GET /presets/{kind}/{id}/resolve`

Returns effective resolved object:

- prompt composition
- merged capabilities
- merged policy
- composition hash

## `POST /presets/fork`

```json
{
  "kind": "agent",
  "id": "tandem.agent.github.pr_worker",
  "version": "1.0.0",
  "tracking": true,
  "target_project": "current"
}
```

## `POST /presets/overrides`

Create project override preset.

## `PATCH /presets/overrides/{id}`

Update project override with optimistic revision control.

## `DELETE /presets/overrides/{id}`

Delete project override.

## Attachment/Detection APIs

## `POST /attachments/ingest`

- download/stage attachment
- detect pack marker
- emit `pack.detected` when matched

## `POST /attachments/{id}/install-pack`

Trigger PackManager install flow from staged attachment.

## SSE/Event Bus Events

- `attachment.received`
- `pack.detected`
- `pack.install.started`
- `pack.install.succeeded`
- `pack.install.failed`
- `pack.uninstall.succeeded`
- `pack.export.succeeded`
- `preset.forked`
- `preset.override.updated`
- `preset.composition.changed`

## Error Codes

- `PACK_MARKER_MISSING`
- `PACK_ARCHIVE_UNSAFE`
- `PACK_SIGNATURE_INVALID`
- `PRESET_IMMUTABLE_SOURCE`
- `PRESET_SCHEMA_INVALID`
- `PRESET_CONFLICT`
- `PRESET_CAPABILITY_SCOPE_INCREASE_REQUIRES_APPROVAL`
