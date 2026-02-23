# What's New in v0.3.15

## Update Reliability Hotfixes

- Fixed sidecar version skew handling so stale AppData engine binaries no longer override newer bundled versions.
- Reduced false update-loop prompts where desktop reported very old installed engine versions after app upgrades.
- Normalized version display labels in update surfaces.

## MCP Runtime Compatibility

- Added compatibility fallback for MCP tool discovery when mixed-version engines do not expose `GET /mcp/tools`.
- Extensions MCP view now degrades gracefully by deriving tools from MCP server `tool_cache`.

## Publishing Pipeline Hardening

- Corrected workspace crate publish order and dependency coverage in registry publish automation to avoid dependency-order failures.
