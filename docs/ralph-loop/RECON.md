# Ralph Loop Implementation - Codebase Reconnaissance

> **Reference Implementation**: https://raw.githubusercontent.com/Th0rgal/open-ralph-wiggum/refs/heads/master/ralph.ts

This document maps the Ralph Loop requirements to Tandem's existing codebase architecture.

---

## 1. Session Management & Sidecar Communication

### Key Files

- [`src-tauri/src/sidecar.rs`](../../src-tauri/src/sidecar.rs) - SidecarManager implementation
- [`src-tauri/src/state.rs`](../../src-tauri/src/state.rs) - AppState with sidecar reference
- [`src-tauri/src/commands.rs`](../../src-tauri/src/commands.rs) - Tauri commands

### How Sessions Work

**Session Creation**:

- Sessions are created via OpenCode sidecar HTTP API
- [`SidecarManager`](../../src-tauri/src/sidecar.rs:584) maintains connection to sidecar process
- Session ID format: `ses_xxx` (string)

**Sending Messages**:

```rust
// src-tauri/src/sidecar.rs:1155
pub async fn send_message(&self, session_id: &str, request: SendMessageRequest) -> Result<()>
```

- Uses `POST /session/{id}/prompt_async` endpoint
- Returns 204 No Content immediately
- Actual response comes via SSE event stream

**Event Streaming**:

```rust
// src-tauri/src/sidecar.rs:1343
pub async fn subscribe_events(&self) -> Result<impl Stream<Item = Result<StreamEvent>>>
```

- SSE stream at `GET /event`
- Events filtered by session_id in [`send_message_streaming`](../../src-tauri/src/commands.rs:1228)

### Critical Event: SessionIdle

**"Agent run finished" signal**: [`StreamEvent::SessionIdle`](../../src-tauri/src/sidecar.rs:476)

```rust
// src-tauri/src/sidecar.rs:475-477
/// Session is idle (generation complete)
SessionIdle { session_id: String },
```

This is the key event Ralph Loop must watch for to detect iteration completion.

Parsed from SSE events in [`parse_sse_event`](../../src-tauri/src/sidecar.rs:2100):

```rust
if matches!(status, "idle" | "complete" | "completed") {
    return Some(StreamEvent::SessionIdle { session_id });
}
```

**Usage in streaming**:

```rust
// src-tauri/src/commands.rs:1402-1405
if matches!(event, StreamEvent::SessionIdle { .. }) {
    break;
}
```

---

## 2. Tool Permission & Plan Mode Integration

### Key Files

- [`src-tauri/src/tool_proxy.rs`](../../src-tauri/src/tool_proxy.rs) - Operation journal and staging
- [`src/components/plan/ExecutionPlanPanel.tsx`](../../src/components/plan/ExecutionPlanPanel.tsx)

### Permission Events

[`StreamEvent::PermissionAsked`](../../src-tauri/src/sidecar.rs:480-485):

```rust
PermissionAsked {
    session_id: String,
    request_id: String,
    tool: Option<String>,
    args: Option<serde_json::Value>,
},
```

### Plan Mode Integration Points

Ralph Loop **must NOT** auto-execute when Plan Mode is enabled:

- Check if staging operations exist in [`StagingStore`](../../src-tauri/src/tool_proxy.rs)
- If operations are staged, Ralph should pause and wait for user approval
- Current staging state tracked in [`AppState.staging_store`](../../src-tauri/src/state.rs:210)

---

## 3. UI Integration Points

### Bottom Chat Control Bar

**Primary location**: [`src/components/chat/ContextToolbar.tsx`](../../src/components/chat/ContextToolbar.tsx:47)

Current structure:

```tsx
<div className="flex items-center gap-2 px-3 py-2 border-t border-border/50 bg-surface/30">
  <AgentSelector />
  <ToolCategoryPicker />
  {/* Allow All Tools Toggle */}
  <ModelSelector />
  <div className="flex-1" /> {/* Spacer */}
  <span>Enter to send • Shift+Enter for newline</span>
</div>
```

**Where to add Loop toggle**:

- After ModelSelector (around line 105)
- Use same pattern as Allow All Tools button
- Show status chip when loop is active

### Chat Input Component

**File**: [`src/components/chat/ChatInput.tsx`](../../src/components/chat/ChatInput.tsx)

Key props to extend:

```tsx
interface ChatInputProps {
  onSend: (message: string, attachments?: FileAttachment[]) => void;
  // ... existing props

  // Ralph Loop props to add:
  loopEnabled?: boolean;
  onLoopToggle?: (enabled: boolean) => void;
  loopStatus?: RalphLoopStatus;
}
```

---

## 4. Storage Architecture

### Workspace-Local Storage

**Pattern to follow**: Use workspace path from [`AppState.workspace_path`](../../src-tauri/src/state.rs:186)

**Ralph directory**: `.opencode/tandem/ralph/`

**Files**:

- `state.json` - Current loop state
- `history.json` - Iteration history (capped at 50)
- `context.md` - Pending injected context
- `summary.md` - Human-friendly status (optional)

**Similar implementation**: Memory module uses SQLite in workspace:

```rust
// src-tauri/src/memory/db.rs
let db_path = workspace_path.join(".opencode/tandem/memory.db");
```

---

## 5. App State Extension

### Where to Add Ralph State

**File**: [`src-tauri/src/state.rs`](../../src-tauri/src/state.rs:184)

Add to [`AppState`](../../src-tauri/src/state.rs:184):

```rust
pub struct AppState {
    // ... existing fields

    /// Ralph Loop manager for iterative task execution
    pub ralph_manager: Arc<RalphLoopManager>,
}
```

Initialize in [`AppState::new()`](../../src-tauri/src/state.rs:216):

```rust
Self {
    // ... existing fields
    ralph_manager: Arc::new(RalphLoopManager::new()),
}
```

---

## 6. Tauri Commands Registration

### Where to Register Commands

**File**: [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs)

Existing pattern (line 393+):

```rust
.invoke_handler(tauri::generate_handler![
    commands::send_message,
    commands::send_message_streaming,
    commands::cancel_generation,
    // ... more commands
])
```

**Ralph commands to add**:

```rust
commands::ralph_start,
commands::ralph_cancel,
commands::ralph_pause,
commands::ralph_resume,
commands::ralph_add_context,
commands::ralph_status,
commands::ralph_history,
```

---

## 7. Frontend State Management

### Current Pattern

**File**: [`src/hooks/useAppState.ts`](../../src/hooks/useAppState.ts)

Uses Tauri events for real-time updates:

```typescript
listen("sidecar_event", (event) => {
  // Handle streaming events
});
```

**Ralph integration**:

- Poll `ralph_status` every 1s when loop is active, OR
- Emit custom events from Rust (preferred)

---

## 8. Key Symbols for Integration

### Rust Backend

| Symbol                     | Location                        | Purpose                     |
| -------------------------- | ------------------------------- | --------------------------- |
| `SidecarManager`           | `src-tauri/src/sidecar.rs:584`  | Manages OpenCode connection |
| `send_message`             | `src-tauri/src/sidecar.rs:1155` | Send prompt to sidecar      |
| `subscribe_events`         | `src-tauri/src/sidecar.rs:1343` | Get SSE event stream        |
| `StreamEvent::SessionIdle` | `src-tauri/src/sidecar.rs:476`  | Iteration completion signal |
| `AppState`                 | `src-tauri/src/state.rs:184`    | Global app state            |
| `StagingStore`             | `src-tauri/src/tool_proxy.rs`   | Plan mode operations        |

### TypeScript Frontend

| Symbol           | Location                                 | Purpose                   |
| ---------------- | ---------------------------------------- | ------------------------- |
| `ContextToolbar` | `src/components/chat/ContextToolbar.tsx` | Add Loop toggle here      |
| `ChatInput`      | `src/components/chat/ChatInput.tsx`      | Handle loop start on send |
| `invoke`         | `@tauri-apps/api/core`                   | Call Tauri commands       |
| `listen`         | `@tauri-apps/api/event`                  | Subscribe to events       |

---

## 9. Git Integration for File Tracking

### Implementation Pattern

Use `std::process::Command` to run git:

```rust
// Before iteration
let before_status = Command::new("git")
    .args(["status", "--porcelain"])
    .current_dir(workspace_path)
    .output()?;

// After iteration completes
let after_status = Command::new("git")
    .args(["status", "--porcelain"])
    .current_dir(workspace_path)
    .output()?;

// Get diff
let diff = Command::new("git")
    .args(["diff", "--name-only"])
    .current_dir(workspace_path)
    .output()?;
```

**Graceful degradation**: Skip file tracking if not a git repo (no error).

---

## 10. Summary: Integration Checklist

- [x] Located session management (`SidecarManager`)
- [x] Identified message sending (`send_message`)
- [x] Found event streaming (`subscribe_events`)
- [x] Confirmed completion signal (`StreamEvent::SessionIdle`)
- [x] Located permission events (`StreamEvent::PermissionAsked`)
- [x] Found UI location (`ContextToolbar`)
- [x] Identified storage pattern (workspace-local `.opencode/tandem/`)
- [x] Located app state (`AppState`)
- [x] Found command registration pattern
- [x] Identified frontend state pattern (`useAppState`)

---

## Next Steps

1. Create Ralph Loop module structure in `src-tauri/src/ralph/`
2. Implement types (`RalphConfig`, `RalphState`, `IterationRecord`)
3. Implement storage layer for state.json/history.json
4. Implement service layer with tokio task + cancellation
5. Add Tauri commands
6. Create frontend toggle and panel
7. Integrate with sidecar event streaming
8. Add documentation
