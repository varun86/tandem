---
title: Memory Internals
description: The storage-level model behind Tandem memory, retrieval, replay, and knowledge reuse.
---

Tandem keeps four different things separate:

- the raw transcript
- the retrieval memory
- the run replay state
- the artifacts and checkpoints

If you are answering "how does Tandem remember things?", this page is the storage-level version of that answer.

## The short version

- **History** is what happened in a session.
- **Memory** is reusable recall stored in `memory.sqlite`.
- **Replay state** is how a run can be reconstructed later.
- **Artifacts** are the files and outputs created by a run.
- **Knowledge reuse** is promoted memory that later runs can preflight and reuse.

The engine uses these layers so it can stay bounded in context without pretending that the whole transcript is the prompt forever.

## Storage surfaces

| Surface                 | Backing store              | What it holds                                                                | Scope                    |
| ----------------------- | -------------------------- | ---------------------------------------------------------------------------- | ------------------------ |
| Working memory          | Prompt context only        | The current turn, bounded memory context, and runtime state                  | Per turn                 |
| Session history         | Session storage            | Messages, turns, and the source-of-truth conversation record                 | Per session              |
| Retrieval memory        | `memory.sqlite`            | Session/project/global memory chunks, retrieval state, and knowledge records | Cross-session, scoped    |
| Governed memory records | `memory.sqlite` FTS tables | User-keyed notes, facts, and solution capsules written through memory tools  | User / project partition |
| Knowledge reuse         | `memory.sqlite`            | Reusable project or global knowledge that workflows can preflight            | Project / global         |
| Artifact memory         | Files and uploads          | Output files, handoffs, attachments, blackboard artifacts                    | Per run or workspace     |
| Execution memory        | `context_runs/<run_id>/`   | Run state, events, patches, and checkpoints                                  | Per run                  |
| Cache / derived memory  | Cache files                | Response caches, embedding caches, and other acceleration layers             | Derived                  |

## Memory layers

### Working memory

Working memory is the prompt-time context the model sees right now.

It includes:

- the current conversation window
- the bounded memory context block
- temporary runtime state

This is not durable storage. If it is not written into a store, it disappears when the turn ends.

### Session history

Session history is the raw transcript.

It answers:

- what the user said
- what the agent replied
- what tool calls happened
- what the run status was

Session history is the source of truth for the conversation, but it is not the same thing as reusable memory.

### Retrieval memory

Retrieval memory lives in `memory.sqlite`.

It is tiered:

- `session` memory is ephemeral and session-scoped
- `project` memory is reusable inside a project
- `global` memory is cross-session and cross-project

This is the memory that gets searched when Tandem wants reusable facts without replaying the whole transcript.

Under the hood, imported/session/project/global retrieval memory is stored as chunks plus vectors:

- `session_memory_chunks` + `session_memory_vectors`
- `project_memory_chunks` + `project_memory_vectors`
- `global_memory_chunks` + `global_memory_vectors`

The vector tables use sqlite-vec. Tandem embeds each chunk, then retrieves nearest chunks for the current query. This path is for semantic recall across the `session`, `project`, and `global` tiers.

### Governed memory records

Tandem also stores governed memory records in `memory.sqlite`.

These are different from the vector chunk tables. They are written by governed memory APIs such as `memory.put`, `memory.promote`, and automatic event ingestion. They live in `memory_records` and are indexed by `memory_records_fts`.

The FTS index exists for the tool-facing memory path:

- it keys records by user identity and run provenance
- it supports fast keyword/BM25 search for notes, facts, and solution capsules
- it can filter by project, channel, host, visibility, expiration, and demotion state
- it gives governed memory tools a deterministic retrieval path even when semantic embeddings are disabled or inappropriate for a policy surface

The search flow is:

1. Tandem normalizes the query into quoted FTS tokens joined with `OR`. For example, `rust workspace bug` becomes `"rust" OR "workspace" OR "bug"`.
2. SQLite FTS5 searches `memory_records_fts.content`.
3. Matching FTS rows are joined back to `memory_records` so Tandem can enforce metadata filters.
4. The query only returns records for the current `user_id`.
5. Demoted records and expired records are skipped.
6. Optional `project_tag`, `channel_tag`, and `host_tag` filters narrow the search.
7. Results are ordered by `bm25(memory_records_fts)` ascending, where lower BM25 rank is more relevant.
8. Tandem converts the BM25 rank into a simple tool-facing score with `1.0 / (1.0 + rank.max(0.0))`.
9. If FTS returns no hits, Tandem falls back to a substring `LIKE` search over `memory_records.content`, ordered by newest record first, with a low fixed score.

The BM25 path is intentionally lexical rather than semantic. That makes it useful for governed memory because exact task names, customer names, error strings, ticket IDs, project tags, and source terms matter. It also means `memory_search` can be predictable: the agent can search for the words it expects to find in a prior note or solution capsule.

In short: vector tables power semantic chunk recall; FTS powers governed, user-keyed memory record search.

### Knowledge reuse

Tandem also keeps structured reusable knowledge in the same memory store.

That layer exists so later runs can:

- preflight prior knowledge before recomputing
- reuse validated facts instead of starting over
- keep workflow memory project-scoped instead of flat and global

Think of this as promoted memory, not raw chat history.

### Artifact memory

Artifacts are durable outputs, not the same thing as memory.

Examples include:

- file outputs and handoffs
- channel uploads
- blackboard artifacts
- run-generated docs and reports

Artifacts are important because they are inspectable and durable, but they do not become semantic memory unless Tandem explicitly stores or promotes them.

### Execution memory / resumability

Execution memory is the run-state layer.

It lets Tandem reconstruct or replay work using:

- `run_state.json`
- `events.jsonl`
- `blackboard.json`
- `blackboard_patches.jsonl`
- `checkpoints/`

This is different from knowledge memory. It answers "how did the run evolve?" rather than "what should the model remember?"

### Cache / derived memory

Cache layers make memory faster, but they are not the source of truth.

Examples include:

- response caches
- embedding caches
- derived indexes

If a cache is lost, Tandem should still be able to rebuild the real memory from the underlying stores.

## How memory gets written

### Session writes

Each new message is appended to the session history.

That keeps the conversation record intact even when the prompt window is truncated later.

### Retrieval memory writes

Tandem stores retrieval memory when something is worth reusing later.

Typical write paths include:

- `memory.put` for a new governed memory record
- `memory.import` to index existing markdown/text directories or OpenClaw memory exports
- `memory.promote` to move something into a more reusable tier
- `memory.demote` to reduce visibility or scope
- `contextDistill` to extract durable memories from a session conversation

### Agent memory tools

Agents do not need direct database access to use memory. Give them memory tools in their `allowed_tools` list.

Common agent-facing tools:

- `memory_search`: search stored memory for prior notes, facts, solution capsules, or relevant context.
- `memory_store`: persist useful content into memory with scope and metadata.
- `memory_list`: inspect stored memory records in the allowed scope.
- `memory_delete`: delete a memory record or chunk in the allowed scope.

Recommended default:

```json
["memory_search", "memory_store", "memory_list"]
```

Only give `memory_delete` to trusted agents or admin/operator flows. Deletion changes the durable memory store and should normally be a narrower capability than read/search/store.

Typical tool arguments:

- `memory_search`: `query`, plus optional scope controls such as `tier`, `session_id`, `project_id`, `limit`, and `allow_global`.
- `memory_store`: `content`, plus optional `tier`, `session_id`, `project_id`, `source`, `metadata`, and `allow_global`.
- `memory_list`: optional scope controls such as `tier`, `session_id`, `project_id`, `limit`, and `allow_global`.
- `memory_delete`: `chunk_id` or `id`, plus optional scope controls such as `tier`, `session_id`, `project_id`, and `allow_global`.

For normal recall, let the agent call `memory_search` without explicit IDs so Tandem can use the current session/project context. Pass explicit IDs only when narrowing the scope deliberately.

### File and OpenClaw imports

Path-based imports are the first-class way to seed retrieval memory from existing docs.

Use this when you want Tandem agents to retrieve existing project docs, support policies, SOPs, handoffs, run artifacts, or OpenClaw exports without pasting them into a chat session.

Import is available through:

- Control Panel Files: select a folder or file location, then choose **Import to Memory**
- Control Panel Memory: choose **Import Knowledge**
- HTTP: `POST /memory/import`
- SDKs: `client.memory.importPath(...)` and `client.memory.import_path(...)`
- CLI: `tandem-engine memory import ...`

The HTTP/SDK import path uses the same internal importer as the CLI. It does not shell out to `tandem-engine`.

Supported formats:

- `directory`: markdown/text directory import
- `openclaw`: OpenClaw memory export import

Supported tiers:

- `global`: cross-project memory
- `project`: project-scoped memory, requires `project_id`
- `session`: session-scoped memory, requires `session_id`

CLI examples:

```bash
tandem-engine memory import --path ./notes --format directory --tier global
tandem-engine memory import --path ./docs --format directory --tier project --project-id repo-123 --sync-deletes
tandem-engine memory import --path ./handoff --format directory --tier session --session-id sess-123
tandem-engine memory import --path ~/.openclaw --format openclaw --tier global
```

The CLI import command does not currently accept a `--user-id` or `--subject` flag. `--tier global` means "store these imported chunks in the global retrieval tier", not "write governed user-scoped `memory_records` rows". User-scoped governed memory is created through the memory APIs/tools, where the capability subject becomes the record `user_id`.

HTTP request:

```json
{
  "source": {
    "kind": "path",
    "path": "/srv/tandem/imports/company-docs"
  },
  "format": "directory",
  "tier": "project",
  "project_id": "company-brain-demo",
  "session_id": null,
  "sync_deletes": true
}
```

Response:

```json
{
  "ok": true,
  "source": {
    "kind": "path",
    "path": "/srv/tandem/imports/company-docs"
  },
  "format": "directory",
  "tier": "project",
  "project_id": "company-brain-demo",
  "session_id": null,
  "sync_deletes": true,
  "discovered_files": 42,
  "files_processed": 42,
  "indexed_files": 39,
  "skipped_files": 3,
  "deleted_files": 0,
  "chunks_created": 312,
  "errors": 0
}
```

Validation rules:

- `source.kind` currently supports `path` only
- `source.path` must be non-empty, readable, and exist on the engine host
- `tier: "project"` requires `project_id`
- `tier: "session"` requires `session_id`
- invalid requests return `400`
- importer failures return `500`

### Channel archival writes

Channel integrations keep raw transcripts and retrieval memory separate.

After a successful channel reply, Tandem archives the completed user + assistant exchange into global retrieval memory, so future channel sessions can recall it without loading the full transcript every time.

### Run-state writes

During execution, Tandem writes run state atomically and appends event and patch records.

That is what makes replay possible later.

## How memory gets read back

### Prompt injection

When Tandem builds context for a run, it injects bounded memory into a structured block such as:

```text
<memory_context>
  <current_session>...</current_session>
  <relevant_history>...</relevant_history>
  <project_facts>...</project_facts>
</memory_context>
```

This keeps the model focused on reusable facts rather than forcing it to reread the full transcript.

### Search

The memory APIs let you search stored memory by query and scope.

Typical uses:

- search prior work by project or user
- find reusable facts from earlier runs
- inspect what has already been promoted

### Replay

The run replay path reconstructs execution from the persisted run state plus later events and blackboard patches.

This is useful when you want to answer:

- what happened
- what changed
- why a run ended the way it did

## What to teach agents

Use these distinctions consistently:

- **History** tells you what happened.
- **Memory** tells you what should be reusable.
- **Artifacts** tell you what was produced.
- **Replay state** tells you how the execution unfolded.

Do not flatten them into one "memory" bucket.

The most common mistakes are:

- treating session history as long-term memory
- treating artifacts as semantic recall
- treating cache as durable knowledge
- treating replay state as if it were user knowledge

## Public APIs worth knowing

- `client.memory.put`
- `client.memory.importPath` / `client.memory.import_path`
- `client.memory.search`
- `client.memory.list`
- `client.memory.promote`
- `client.memory.demote`
- `client.memory.delete`
- `client.memory.audit`
- `client.memory.contextResolveUri`
- `client.memory.contextTree`
- `client.memory.contextGenerateLayers`
- `client.memory.contextDistill`

## Related docs

- [How Tandem Works Under the Hood](https://docs.tandem.ac/how-tandem-works/)
- [Agents & Sessions](https://docs.tandem.ac/agents-and-sessions/)
- [Channel Integrations](https://docs.tandem.ac/channel-integrations/)
- [Headless Service](https://docs.tandem.ac/headless-service/)
- [TypeScript SDK](https://docs.tandem.ac/sdk/typescript/)
- [Python SDK](https://docs.tandem.ac/sdk/python/)
