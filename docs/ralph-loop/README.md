# Ralph Loop

An iterative run mode for Tandem that repeatedly calls the OpenCode sidecar until a completion promise is detected.

> **Inspiration / Reference**: https://raw.githubusercontent.com/Th0rgal/open-ralph-wiggum/refs/heads/master/ralph.ts

---

## What is Ralph Loop?

Ralph Loop is an orchestration mode that automatically iterates on a task until it's complete. Instead of manually prompting the AI multiple times, Ralph Loop:

1. Takes your initial task description
2. Runs the AI agent
3. Detects when the task is complete via a special completion token
4. Automatically continues if more work is needed
5. Stops when the task is genuinely finished

---

## How to Use

### Starting a Loop

1. Enable the **Loop** toggle in the chat control bar (next to model selection)
2. Type your task description
3. Send the message
4. Ralph Loop will automatically iterate until complete

### During a Loop

When a loop is active, you'll see a status chip showing:

- Current status (Running / Paused / Completed / Error)
- Current iteration number

Click the status chip to open the Ralph Panel with:

- **Pause/Resume**: Control iteration flow
- **Cancel**: Stop the loop immediately
- **Add Context**: Inject additional instructions for the next iteration
- **View History**: See past iterations and their results

### Completion Detection

Ralph Loop stops when:

1. The AI outputs `<promise>COMPLETE</promise>` in its response
2. At least `min_iterations` have completed (default: 1)
3. No struggle is detected

The AI is instructed to only output this token when the task is genuinely complete.

---

## Storage

Ralph Loop stores its state in your workspace:

```
.opencode/tandem/ralph/
├── state.json    # Current loop state
├── history.json  # Iteration history (last 50 iterations)
├── context.md    # Pending context to inject
└── summary.md    # Human-readable summary (optional)
```

These files are workspace-local and not synced.

---

## Plan Mode Integration

Ralph Loop works seamlessly with Tandem's Plan Mode:

- When Plan Mode is **enabled**, Ralph Loop will:
  - Stage operations rather than executing them
  - Wait for your approval before making changes
  - Continue iterating on the plan until complete

- When Plan Mode is **disabled**, Ralph Loop will:
  - Execute operations directly
  - Iterate until the task is finished

**Important**: Ralph Loop never bypasses Plan Mode staging. If operations are staged, you'll need to approve them before they execute.

---

## Struggle Detection

Ralph Loop includes basic struggle detection:

- If no files are modified for 3 consecutive iterations
- If the same error appears 2+ times in a row

When struggle is detected:

- The loop continues but marks `struggle=true`
- A hint is injected into the next prompt suggesting alternative approaches

---

## Configuration

Default configuration (not yet user-configurable):

- `min_iterations`: 1 - Minimum iterations before stopping
- `max_iterations`: 50 - Safety limit to prevent infinite loops
- `completion_promise`: "COMPLETE" - The token to detect completion

---

## Safety Features

1. **Cancellation**: You can cancel the loop at any time
2. **Pause/Resume**: Pause between iterations to review progress
3. **Max Iterations**: Hard limit of 50 iterations
4. **Error Handling**: Errors are captured and surfaced in the UI
5. **Plan Mode Respect**: Never auto-executes staged operations

---

## Troubleshooting

### Loop not stopping

- Check that the AI is outputting `<promise>COMPLETE</promise>`
- Verify `min_iterations` has been reached
- Check if struggle detection is preventing completion

### No file changes detected

- Ensure your workspace is a git repository
- Ralph uses `git status` and `git diff` to track changes

### Context not being injected

- Context is only injected at the start of an iteration
- Context is cleared after being used once

---

## Technical Details

Ralph Loop is implemented as:

- **Backend**: Rust module in `src-tauri/src/ralph/`
- **Frontend**: React components in `src/components/ralph/`
- **Storage**: JSON files in workspace `.opencode/tandem/ralph/`

The loop integrates with Tandem's existing:

- Sidecar communication (HTTP + SSE events)
- Event streaming infrastructure
- Plan Mode / staging system
