# Strict Write Failure Debug Report

## Scope

This document summarizes the current strict blackboard failure we have been chasing, the fixes already attempted, what changed after each fix, and the current working diagnosis.

Primary failure signature:

```text
TOOL_MODE_REQUIRED_NOT_SATISFIED: WRITE_ARGS_EMPTY_FROM_PROVIDER
```

Common surface symptoms:

- strict swarm tasks fail after repeated retries
- traces show `write · err=WRITE_ARGS_EMPTY_FROM_PROVIDER`
- OpenRouter/OpenAI dashboards show large output token counts with `finish = tool_calls`
- Tandem session storage still shows `write.args = {}`

## What We Observed

The failing runs are not simply "empty responses".

Repeated evidence from failing sessions:

- a full task prompt is stored
- an initial read-only tool call like `glob` is often accepted
- one or more `write` tool calls are recorded
- those `write` calls are persisted with `args: {}`
- the assistant message after that is synthetic failure text, not the original model output

Representative failing sessions we inspected directly:

- `cde70a97-b25d-4f98-aa3d-5e65fd814ad7`
- `76bd93c9-b253-4297-8975-63f265d5a787`

Relevant local persistence paths:

- [`session_meta.json`](/home/user123/.local/share/tandem/storage/session_meta.json)
- context-run `events.jsonl` under `/home/user123/.local/share/tandem/context_runs/`

Observed persisted shape:

```json
{
  "role": "user",
  "parts": [
    { "type": "text", "text": "full strict swarm task prompt..." },
    {
      "type": "tool_invocation",
      "tool": "glob",
      "args": {
        "__effective_cwd": "/home/user123/game",
        "__session_id": "..."
      },
      "result": "...",
      "error": null
    },
    {
      "type": "tool_invocation",
      "tool": "write",
      "args": {},
      "result": null,
      "error": "WRITE_ARGS_EMPTY_FROM_PROVIDER"
    }
  ]
}
```

Important implication:

- the failure is not only in the UI or swarm verification summary
- by the time the session is persisted, Tandem already believes the `write` args are empty

## Root Problems Identified So Far

### 1. Original path-recovery bug in strict mode

We confirmed an early real bug in strict write recovery:

- the prompt already contained a valid required output target such as `game.html`
- strict recovery logic in [`engine_loop.rs`](/home/user123/tandem/crates/tandem-core/src/engine_loop.rs) rejected `.html` and similar artifact paths too aggressively
- this caused `write {}` to degrade into `FILE_PATH_MISSING` instead of recovering the required target

Status:

- fixed
- this was real, but it was not the whole problem

### 2. Task contract mismatch for non-writing tasks

We also confirmed a workflow-level problem:

- tasks like "Inspect workspace and decide target single-file game artifact path and naming" were still being treated as `implementation`
- that incorrectly pushed them into `write_required=true`
- strict retries then forced repeated write attempts for a task whose natural output was a decision, not a file mutation

Status:

- partially addressed by moving task contract handling toward backend ownership
- still worth treating as a separate category from the empty-write failure

### 3. Streamed tool-call args appear to be lost before persistence

This is the current primary suspect for the remaining `WRITE_ARGS_EMPTY_FROM_PROVIDER` failures.

What we know:

- external provider dashboards show large output token counts and `finish = tool_calls`
- Tandem persistence shows empty `write.args`
- direct session inspection shows no large `content` payload stored with the `write`

The likely loss point is the streamed tool-call reconstruction/persistence path.

## Fixes Already Attempted

### A. Strict output-target recovery changes

Implemented in [`engine_loop.rs`](/home/user123/tandem/crates/tandem-core/src/engine_loop.rs):

- split explicit `output_target.path` sanitization from free-form path guessing
- allowed structured required targets like `game.html`
- limited strict write recovery to authoritative targets
- preserved more specific malformed-write failure reasons

Result:

- failures moved from generic `FILE_PATH_MISSING` toward `WRITE_ARGS_EMPTY_FROM_PROVIDER`
- this confirmed the path-recovery bug existed, but did not solve the end-to-end failure

### B. Multi-attempt strict write retries

Implemented across core/control-panel/Tauri paths:

- default retry budget increased to 3 total attempts
- retries now trigger on malformed write failures, not only on "no assistant/no tool" cases
- traces now record attempt counts and retry state

Result:

- retries now happen
- failures still end in `WRITE_ARGS_EMPTY_FROM_PROVIDER`
- this proved the system was retrying, but retrying the same broken persisted write state

### C. Non-writing task handling

We introduced separation between non-writing and writing tasks so read-only inspection tasks are not forced into `write_required=true`.

Result:

- good architectural direction
- not enough by itself because some tasks still arrived at execution as `Task Kind: implementation`
- this did not explain the core empty-write persistence issue

### D. Backend ownership of task contract

We started moving strict task classification and execution-mode ownership into the backend so the UI/control-panel does not invent execution policy locally.

Result:

- improved architecture
- reduced UI-side orchestration ambiguity
- did not yet eliminate the empty-write failure itself

## Current Primary Hypothesis

The current primary issue is likely that streamed tool-call args are being computed transiently but not persisted correctly.

Critical code path that was identified:

### Engine streaming path

In [`engine_loop.rs`](/home/user123/tandem/crates/tandem-core/src/engine_loop.rs):

- streamed tool-call deltas build reconstructed previews via `parse_streamed_tool_args(...)`
- earlier behavior published the user-visible/persisted tool part as `args: {}`
- the reconstructed args only lived in the side-channel event payload such as `toolCallDelta.parsedArgsPreview`

### Server persistence path

In [`lib.rs`](/home/user123/tandem/crates/tandem-server/src/lib.rs):

- the session part persister stores the `part`
- if `part.args` is already `{}`, the persisted session remains empty unless we explicitly copy the streamed preview into persistence

### Session merge path

In [`storage.rs`](/home/user123/tandem/crates/tandem-core/src/storage.rs):

- merge logic operates on persisted tool parts
- it cannot recover streamed preview args that were never copied into `part.args`

## Instrumentation / Persistence Fixes Added

To improve debuggability and stop dropping streamed args, we added two changes.

### 1. Publish parsed streamed args into the tool part

Updated [`engine_loop.rs`](/home/user123/tandem/crates/tandem-core/src/engine_loop.rs):

- when a streamed tool delta has a reconstructed args preview, publish that preview into `WireMessagePart::tool_invocation(..., args, ...)`
- do not keep emitting `args: {}`

Intent:

- if the provider streamed usable partial JSON, the persisted session should now carry it directly in `part.args`

### 2. Server-side persistence fallback

Updated [`lib.rs`](/home/user123/tandem/crates/tandem-server/src/lib.rs):

- if `part.args` is empty, the persister now falls back to `toolCallDelta.parsedArgsPreview` when available

Intent:

- even if another path still emits an empty `part.args`, persistence should retain the streamed preview instead of dropping it

### 3. Regression coverage

Added a tandem-server regression test in [`sessions.rs`](/home/user123/tandem/crates/tandem-server/src/http/tests/sessions.rs):

- verifies persistence prefers `toolCallDelta.parsedArgsPreview` when `part.args` is empty

## What We Learned After Rebuilds

After rebuilding and running fresh sessions, we still observed new failures with persisted `write.args = {}`.

This leaves two realistic explanations:

### Option 1. The rebuilt binary was not the one serving the local engine port

This would explain why the new persistence behavior was not visible in subsequent session storage.

### Option 2. There is another loss point after the patched code path

If the running binary did include the patch, then some later step is still overwriting or dropping the write args before session persistence completes.

At the moment, the evidence supports "args are still empty in persisted storage", but does not yet prove which of those two explanations is true.

## What Did Not Pan Out

These changes improved behavior or architecture but did not fix the core failure:

- simply increasing retries
- treating the failure as purely a provider issue
- only fixing `.html` output-target recovery
- only moving task-kind classification out of the UI

Those changes were still useful because they narrowed the failure:

- the problem is no longer just path recovery
- the problem is no longer hidden by generic strict-write failure summaries
- the problem is now specifically tied to empty persisted `write.args`

## Current Working Diagnosis

Current best diagnosis:

- the provider is very likely returning tool-call output
- Tandem is still persisting the `write` invocation as `args: {}`
- therefore the most likely bug is in our streamed tool-call parsing, event projection, or session persistence pipeline

More precise label we should use internally:

- `STREAMED_TOOL_ARGS_NOT_PERSISTED`

The current externally surfaced label:

- `WRITE_ARGS_EMPTY_FROM_PROVIDER`

is useful operationally, but misleading for debugging because it suggests the upstream model emitted nothing. Our current evidence does not support that as the main explanation.

## Recommended Next Debug Step

The next step should be direct logging at the persistence boundary.

Add targeted logging for each streamed `write` tool part:

- raw incoming tool-call delta chunk
- `argsDelta`
- `parsedArgsPreview`
- `part.args` at emit time
- `part.args` at persist time
- final merged stored `args`

Also verify the running local engine binary/version explicitly when reproducing, so we can rule out serving an old build.

Success condition for the next repro:

- a new failing session should either show non-empty `write.args` in persisted storage, or
- clearly prove that `parsedArgsPreview` was already empty before persistence

That will tell us whether the real bug is:

- stream reconstruction
- persistence projection
- post-persist merge/overwrite
- or binary/version mismatch during local testing

## Short Timeline

1. We initially suspected strict path recovery and confirmed a real `.html` target bug.
2. We fixed output-target recovery and got more specific malformed-write failures instead of generic path-missing failures.
3. We added multi-attempt retries; retries worked but kept failing the same way.
4. We separated non-writing tasks from writing tasks and started moving task contract ownership into the backend.
5. We directly inspected stored sessions and confirmed `write.args = {}` is already present in persisted data.
6. We patched the streaming/persistence path to preserve `parsedArgsPreview`.
7. After rebuilds, fresh failures still showed `write.args = {}`, so the remaining work is now highly focused on the stream-to-persistence pipeline or binary mismatch.

## Current Bottom Line

We are no longer guessing broadly.

The current debugging target is narrow:

- why streamed `write` args are still ending up as `{}` in persisted session storage

Everything else should be treated as secondary until that path is proven correct.
