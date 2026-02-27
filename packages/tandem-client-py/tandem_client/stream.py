"""SSE streaming for the Tandem engine — uses httpx-sse."""
from __future__ import annotations

import json
from typing import AsyncGenerator, Optional

import httpx
from httpx_sse import aconnect_sse
from pydantic import TypeAdapter

from .types import EngineEvent

_engine_event_adapter = TypeAdapter(EngineEvent)


async def stream_sse(
    url: str,
    token: str,
    *,
    client: httpx.AsyncClient,
    timeout: float = 300.0,
) -> AsyncGenerator[EngineEvent, None]:
    """
    Async generator that yields :class:`EngineEvent` objects from a Tandem SSE endpoint.

    Example::

        async for event in stream_sse(url, token, client=http_client):
            if event.type == "session.response":
                print(event.properties.get("delta", ""), end="", flush=True)
            if event.type in ("run.complete", "run.failed"):
                break
    """
    headers = {
        "Accept": "text/event-stream",
        "Authorization": f"Bearer {token}",
        "Cache-Control": "no-cache",
    }
    async with aconnect_sse(client, "GET", url, headers=headers, timeout=timeout) as event_source:
        async for sse in event_source.aiter_sse():
            data = sse.data
            if not data or data.startswith(":"):
                continue
            try:
                payload = json.loads(data)
            except json.JSONDecodeError:
                continue
            if not isinstance(payload, dict):
                continue
            event_type: str = payload.get("type", "unknown")  # type: ignore[assignment]
            if not isinstance(event_type, str):
                payload["type"] = "unknown"
            
            try:
                yield _engine_event_adapter.validate_python(payload)
            except Exception:
                pass
