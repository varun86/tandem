# TypeScript SDK Migration Guide (v0.1.0)

## Overview
The @frumu/tandem-client library has been significantly hardened to ensure canonical field casing, discriminated union types, and runtime validation boundaries using Zod.

## Breaking Changes

### Canonical property names
Loose and redundant field names like `runID` or `run_id` have been strictly normalized to canonical camelCase variables:
- `session.sessionID` -> `session.sessionId`
- `run.runID` -> `run.runId`
- `createdAtMs` and `updatedAtMs` properties are consistently used.

The SDK client handles all JSON and boundary validation automatically, guaranteeing that objects returned from methods conform strictly to standard TypeScript fields.

### `EngineEvent` Type
The `streamSse()` output type `EngineEvent` is now a Discriminated Union of 10 core event types, including `RunStartedEvent`, `ToolCalledEvent`, etc.
This enables structural typing based on `event.type` properties.

We introduced two new stream helpers:
- `on(eventType, callback)`
- `filterByType(eventType)`

#### Example migration:
```typescript
// Old
for await (const event of client.streamSse(sessionId)) {
    if (event.type === 'run.completed') {
        console.log(event.properties.duration); // No Type safety
    }
}

// New
const stream = await client.streamSse(sessionId);
for await (const event of filterByType(stream, 'run.completed')) {
    // event is tightly typed as RunCompletedEvent
}
```

### Errors
The client boundary strictly verifies payloads using `z.safeParse`. Validation issues or invalid shape returns will throw `TandemValidationError` with the expected Zod issues.
