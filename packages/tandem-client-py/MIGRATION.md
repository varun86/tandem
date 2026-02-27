# Python SDK Migration Guide (v0.1.0)

## Overview
The `tandem_client` Python SDK has been significantly hardened using Pydantic `AliasChoices` and Discriminator fields to normalize API variance into completely canonical pythonic structure.

## Breaking Changes

### Canonical Property Names (snake_case)
Models no longer return undefined variance of properties like `sessionID` or `runID`. The SDK parses and maps all properties efficiently into their standard Python snake_case equivalents under strict Pydantic parsing rules.

- `session.sessionID` or `sessionId` -> `session.session_id`
- `run.runID` or `runId` -> `run.run_id`
- `createdAtMs` -> `created_at_ms`
- `updatedAtMs` -> `updated_at_ms`
- `requiresApproval` -> `requires_approval`

You no longer need to check `if event.run_id or event.runID`. Simply use `.run_id`.

### `EngineEvent` Discriminated Union
The SSE stream no longer yields a flat `EngineEvent` object allowing arbitrary properties. `EngineEvent` is now a strongly typed Pydantic V2 Union with the delimiter `type`. 

### Validation
Invalid responses from the Tandem engine will throw a descriptive `TandemValidationError` highlighting the required properties missed, stopping bad downstream state immediately.
