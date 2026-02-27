# @frumu/tandem-client

TypeScript / Node.js client for the [Tandem](https://tandem.frumu.ai/) autonomous agent engine HTTP + SSE API.

## Install

```bash
npm install @frumu/tandem-client
```

Requires **Node 18+** (uses built-in `fetch` and `ReadableStream`).

## Quick start

```typescript
import { TandemClient } from "@frumu/tandem-client";

const client = new TandemClient({
  baseUrl: "http://localhost:39731",   // engine URL
  token: "your-engine-token",          // from `tandem-engine token generate`
});

// 1. Create a session
const sessionId = await client.sessions.create({
  title: "My agent session",
  directory: "/path/to/my/project",
});

// 2. Start an async run
const { runId } = await client.sessions.promptAsync(
  sessionId,
  "Summarize the README and list the top 3 TODOs"
);

// 3. Stream the response
for await (const event of client.stream(sessionId, runId)) {
  if (event.type === "session.response") {
    process.stdout.write(String(event.properties.delta ?? ""));
  }
  if (event.type === "run.complete" || event.type === "run.failed") break;
}
```

## API

### `new TandemClient(options)`

| Option | Type | Description |
|--------|------|-------------|
| `baseUrl` | `string` | Engine base URL (e.g. `http://localhost:39731`) |
| `token` | `string` | Engine API token |
| `timeoutMs` | `number` | Request timeout in ms (default 20000) |

### `client.health()` → `SystemHealth`

Check engine readiness.

### `client.stream(sessionId, runId?, options?)` → `AsyncGenerator<EngineEvent>`

Stream events from a session run. Yields typed `EngineEvent` objects.

### `client.globalStream(options?)` → `AsyncGenerator<EngineEvent>`

Stream all engine events across all sessions.

---

### `client.sessions`

| Method | Description |
|--------|-------------|
| `create(options?)` | Create a session, returns `sessionId` |
| `list(options?)` | List sessions |
| `get(sessionId)` | Get session details |
| `delete(sessionId)` | Delete a session |
| `messages(sessionId)` | Get message history |
| `activeRun(sessionId)` | Get the currently active run |
| `promptAsync(sessionId, prompt)` | Start an async run, returns `{ runId }` |

### `client.routines`

| Method | Description |
|--------|-------------|
| `list(family?)` | List routines or automations |
| `create(options, family?)` | Create a scheduled routine |
| `delete(id, family?)` | Delete a routine |
| `runNow(id, family?)` | Trigger a routine immediately |
| `listRuns(family?, limit?)` | List recent run records |
| `listArtifacts(runId, family?)` | List artifacts from a run |

**Create a scheduled routine:**
```typescript
await client.routines.create({
  name: "Daily digest",
  schedule: "0 8 * * *",  // cron expression
  prompt: "Summarize today's activity and write a report",
  allowed_tools: ["read", "websearch", "webfetch"],
});
```

### `client.mcp`

| Method | Description |
|--------|-------------|
| `list()` | List registered MCP servers |
| `listTools()` | List all discovered tools |
| `add(options)` | Register an MCP server |
| `connect(name)` | Connect and discover tools |
| `disconnect(name)` | Disconnect |
| `refresh(name)` | Re-discover tools |
| `setEnabled(name, enabled)` | Enable/disable |

```typescript
await client.mcp.add({ name: "arcade", transport: "https://mcp.arcade.ai/mcp" });
await client.mcp.connect("arcade");
const tools = await client.mcp.listTools();
```

### `client.channels`

| Method | Description |
|--------|-------------|
| `config()` | Get channel configuration |
| `status()` | Get live connection status |
| `put(channel, payload)` | Configure a channel |
| `delete(channel)` | Remove a channel configuration |

### `client.permissions`

| Method | Description |
|--------|-------------|
| `list()` | List pending requests and rules |
| `reply(requestId, reply)` | Approve/deny a permission request |

### `client.providers`

| Method | Description |
|--------|-------------|
| `catalog()` | List available providers |
| `config()` | Get current provider configuration |
| `setDefaults(providerId, modelId)` | Set default provider and model |
| `setApiKey(providerId, apiKey)` | Store an API key |

---

## Engine events reference

Common `event.type` values:

| Type | Description |
|------|-------------|
| `session.response` | Streaming text delta in `event.properties.delta` |
| `session.tool_call` | Tool invocation in `event.properties` |
| `session.tool_result` | Tool result |
| `run.complete` | Run finished successfully |
| `run.failed` | Run failed |
| `permission.request` | Approval needed — use `client.permissions.reply()` |

## License

MIT
