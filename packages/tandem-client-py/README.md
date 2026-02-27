# tandem-client

Python client for the [Tandem](https://tandem.frumu.ai/) autonomous agent engine HTTP + SSE API.

## Install

```bash
pip install tandem-client
```

Python 3.10+ required.

## Quick start

```python
import asyncio
from tandem_client import TandemClient

async def main():
    async with TandemClient(
        base_url="http://localhost:39731",
        token="your-engine-token",     # from `tandem-engine token generate`
    ) as client:
        # 1. Create a session
        session_id = await client.sessions.create(
            title="My agent",
            directory="/path/to/my/project",
        )

        # 2. Start an async run
        run = await client.sessions.prompt_async(
            session_id, "Summarize the README and list the top 3 TODOs"
        )

        # 3. Stream the response
        async for event in client.stream(session_id, run.run_id):
            if event.type == "session.response":
                print(event.properties.get("delta", ""), end="", flush=True)
            if event.type in ("run.complete", "run.failed"):
                break

asyncio.run(main())
```

## Sync usage (scripts)

```python
from tandem_client import SyncTandemClient

client = SyncTandemClient(base_url="http://localhost:39731", token="...")
session_id = client.sessions.create(title="My agent")
run = client.sessions.prompt_async(session_id, "Analyze this folder")
print(f"Run started: {run.run_id}")
# Note: stream() is async-only; use the async client to receive events
client.close()
```

## API

### `TandemClient(base_url, token, *, timeout=20.0)`

Use as an async context manager or call `await client.aclose()` manually.

| Method | Description |
|--------|-------------|
| `await client.health()` | Check engine readiness |
| `client.stream(session_id, run_id?)` | Async generator of `EngineEvent` |
| `client.global_stream()` | Stream all engine events |
| `await client.list_tool_ids()` | List all registered tool IDs |

---

### `client.sessions`

| Method | Description |
|--------|-------------|
| `create(title?, directory?, provider?, model?)` | Create session, returns `session_id` |
| `list(q?, page?, page_size?, archived?, scope?, workspace?)` | List sessions |
| `get(session_id)` | Get session details |
| `delete(session_id)` | Delete a session |
| `messages(session_id)` | Get message history |
| `active_run(session_id)` | Get active run state |
| `prompt_async(session_id, prompt)` | Start async run, returns `PromptAsyncResult(run_id)` |

### `client.routines`

| Method | Description |
|--------|-------------|
| `list(family?)` | List routines or automations |
| `create(options, family?)` | Create a scheduled routine |
| `delete(routine_id, family?)` | Delete a routine |
| `run_now(routine_id, family?)` | Trigger a routine immediately |
| `list_runs(family?, limit?)` | List recent run records |
| `list_artifacts(run_id, family?)` | List run artifacts |

**Create a cron routine:**
```python
await client.routines.create({
    "name": "Daily digest",
    "schedule": "0 8 * * *",
    "prompt": "Summarize today's activity and write a report to daily-digest.md",
    "allowed_tools": ["read", "write", "websearch"],
})
```

### `client.mcp`

```python
await client.mcp.add("arcade", "https://mcp.arcade.ai/mcp")
await client.mcp.connect("arcade")
tools = await client.mcp.list_tools()
```

| Method | Description |
|--------|-------------|
| `list()` | List registered MCP servers |
| `list_tools()` | List discovered tools |
| `add(name, transport, *, headers?, enabled?)` | Register an MCP server |
| `connect(name)` | Connect and discover tools |
| `disconnect(name)` | Disconnect |
| `refresh(name)` | Re-discover tools |
| `set_enabled(name, enabled)` | Enable/disable |

### `client.channels`

```python
await client.channels.put("telegram", {"token": "bot:xxx", "allowed_users": ["@you"]})
status = await client.channels.status()
print(status.telegram.connected)
```

### `client.permissions`

```python
snapshot = await client.permissions.list()
for req in snapshot.requests:
    await client.permissions.reply(req.id, "allow")
```

### `client.providers`

```python
catalog = await client.providers.catalog()
await client.providers.set_defaults("openrouter", "anthropic/claude-3.7-sonnet")
await client.providers.set_api_key("openrouter", "sk-or-...")
```

---

## Common event types

| `event.type` | Description |
|-------------|-------------|
| `session.response` | Text delta in `event.properties["delta"]` |
| `session.tool_call` | Tool invocation |
| `session.tool_result` | Tool result |
| `run.complete` | Run finished successfully |
| `run.failed` | Run failed |
| `permission.request` | Approval needed |

## License

MIT
