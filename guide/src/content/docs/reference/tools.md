---
title: Tools Reference
---

The Tandem Engine tool registry currently exposes the following tools.

## File Operations

- **`read`**: Read file contents.
  - Input: `path` (string)
- **`write`**: Write file contents (overwrites).
  - Input: `path` (string), `content` (string)
- **`edit`**: String replacement in a file.
  - Input: `path` (string), `old` (string), `new` (string)
- **`glob`**: Find files by pattern.
  - Input: `pattern` (string, e.g., `src/**/*.rs`)

## Search

- **`grep`**: Regex search in files.
  - Input: `pattern` (string), `path` (string, root directory)
- **`websearch`**: Search the web (powered by Exa.ai).
  - Input: `query` (string), `limit` (integer)
- **`codesearch`**: Semantic code search (if configured).
- **`memory_list`**: List persisted memory entries for a scope/tier.
  - Input: optional scope + filter arguments (e.g., `session_id`, `project_id`, `tier`, `limit`)
- **`memory_search`**: Search persisted memory by query and scope.
  - Input: `query` plus one or more scopes (e.g., session/workspace).
- **`memory_store`**: Persist memory content for session/project/global retrieval.
  - Input: `content` plus scope/tier arguments (e.g., `session_id`, `project_id`, `tier`)

## Web

- **`webfetch`**: Fetch URL and return structured Markdown/JSON output.
  - Input: `url` (string), optional `mode`, `return`, `max_bytes`, `timeout_ms`, `max_redirects`
- **`webfetch_html`**: Fetch URL and return raw HTML text.
  - Input: `url` (string), optional `max_bytes`, `timeout_ms`, `max_redirects`

## System

- **`bash`**: Run shell commands (PowerShell on Windows, Bash on Linux/Mac).
  - Input: `command` (string)
- **`mcp_debug`**: Call an MCP tool directly.
- **`todo_write`**: Update the Todo/task list.
  - Aliases: `todowrite`, `update_todo_list`
- **`task`**: Update the current task status.
- **`question`**: Ask a structured question to the user and wait for input.
- **`spawn_agent`**: Spawn an agent-team worker instance (runtime/policy gated).
  - Input: mission/spawn payload (e.g., `missionID`, `role`, `templateID`, `source`)
- **`teamcreate`**: Create/register an agent-team context for coordinated teammate tasks.
  - Input: team metadata (e.g., `team_name`, `description`, `agent_type`)
- **`taskcreate`**: Create teammate task records in a team context.
  - Input: task payload (e.g., `team_name`, `name`, `description`)
- **`taskupdate`**: Update teammate task status/notes/progress in a team context.
  - Input: task update payload (e.g., `team_name`, `task_id`, `status`, `notes`)
- **`tasklist`**: List tasks for a team context.
  - Input: optional filters (e.g., `team_name`, status filter)
- **`sendmessage`**: Send mailbox-style message/task prompt to one or more teammates.
  - Input: message payload (e.g., `team_name`, `to`, `content`, `summary`)

## Specialized

- **`skill`**: Execute a skill.
- **`apply_patch`**: Apply a unified diff patch.
- **`batch`**: Execute multiple tools in a batch.
- **`lsp`**: Interact with the Language Server Protocol.
