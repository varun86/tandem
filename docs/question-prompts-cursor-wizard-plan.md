# Fix “question” prompts: interactive, one-at-a-time UI + correct backend wiring

## Summary

We’ll make Tandem properly handle LLM “question” prompts by:

1. parsing OpenCode’s real `question.asked` event shape (multi-question requests),
2. rendering an interactive “cursor-style” wizard (one question at a time, supports multi-select + custom answer),
3. replying via OpenCode’s `/question/{requestID}/reply`,
4. optionally fetching already-pending questions so you can see the fix immediately in the current session even if the SSE event was previously missed.

---

## What’s broken today (root cause)

- Backend event parsing in `src-tauri/src/sidecar.rs` expects the old shape: `questionID`, single `question`, `options: [{id,label}]`.
- OpenCode actually emits `question.asked` with a `QuestionRequest` payload:
  - `id` (requestID), `sessionID`, and `questions: QuestionInfo[]`
  - `QuestionInfo.options` is `{label, description}` and supports `multiple` + `custom`.
- Because parsing fails, the frontend never receives `type: "question_asked"` → the UI never opens (you only see the raw tool call card in chat).

---

## Decisions (defaults we’ll implement)

- **UI surface:** modal overlay wizard (reuses existing `QuestionDialog` concept).
- **Flow:** wizard / “cursor” style (one question at a time).
- **Visibility:** on session open, fetch pending questions from `/question` so existing prompts show up.

(If you’d rather have inline-in-chat cards instead of a modal, we can swap that later without changing backend contracts.)

---

## Public interface changes (types + commands)

### Frontend (`src/lib/tauri.ts`)

- Replace the current single-question event type with a **request-based** type:
  - `QuestionRequestEvent { request_id, session_id, questions: QuestionInfo[] }`
  - `QuestionInfo { header, question, options, multiple?, custom? }`
  - `QuestionOption { label, description }`
- Update `StreamEvent` union: `question_asked` payload becomes request-based.

### Tauri commands (Rust → TS)

- Add:
  - `list_questions` → returns pending `QuestionRequest[]`
  - `reply_question(request_id, answers: string[][])` → POST `/question/{requestID}/reply`
  - `reject_question(request_id)` → POST `/question/{requestID}/reject`
- Update frontend wrapper functions accordingly.
- We can keep the old `answer_question` name as a thin wrapper if it’s referenced elsewhere, but `Chat.tsx` will move to the new API.

---

## Implementation steps (decision-complete)

### 1) Backend: parse OpenCode `question.asked` correctly

**File:** `src-tauri/src/sidecar.rs`

- Update `StreamEvent::QuestionAsked` to carry:
  - `session_id: String`
  - `request_id: String`
  - `questions: Vec<QuestionInfo>`
- Add Rust structs matching OpenCode schema:
  - `QuestionInfo { header: String, question: String, options: Vec<QuestionOption>, multiple: Option<bool>, custom: Option<bool> }`
  - `QuestionOption { label: String, description: String }`
- Update `convert_opencode_event()`:
  - For `"question.asked"`:
    - read `props["id"]` as `request_id`
    - read `props["sessionID"]`
    - parse `props["questions"][]` into `QuestionInfo`
  - (Optional but tidy) handle `"question.replied"` / `"question.rejected"` as `Raw` or a small “cleared” event; frontend can also just close UI on submit.

### 2) Backend: implement list/reply/reject question endpoints

**File:** `src-tauri/src/sidecar.rs`

- Add methods:
  - `list_questions(&self) -> Result<Vec<QuestionRequest>>` using `GET {base}/question`
  - `reply_question(&self, request_id: &str, answers: Vec<Vec<String>>) -> Result<()>` using `POST {base}/question/{id}/reply` with JSON `{ answers: [...] }`
  - `reject_question(&self, request_id: &str) -> Result<()>` using `POST {base}/question/{id}/reject`
- Ensure errors propagate with helpful messages (match existing error patterns).

**File:** `src-tauri/src/commands.rs`

- Expose new `#[tauri::command]` functions:
  - `list_questions()`
  - `reply_question(request_id, answers)`
  - `reject_question(request_id)`
- Wire them into `state.sidecar.*`.

### 3) Frontend: add a queued, cursor-style question wizard

**Files:**

- `src/components/chat/Chat.tsx`
- `src/components/chat/QuestionDialog.tsx`
- `src/lib/tauri.ts`

**Chat state changes (`Chat.tsx`):**

- Replace `pendingQuestion: QuestionEvent | null` with:
  - `pendingQuestionRequests: QuestionRequestEvent[]` (queue)
  - `activeQuestionIndex: number` (index within current request)
  - `draftAnswers: string[][]` (answers collected so far for the active request)
  - `handledQuestionRequestIdsRef: Set<string>` to avoid duplicates
- Event handling:
  - On `StreamEvent.type === "question_asked"`:
    - if `event.session_id !== currentSessionId`: ignore
    - if request already handled/queued: ignore
    - push into queue; if no current modal open, open it
- Session entry / “show fix in current session”:
  - On session selection or on mount (when `currentSessionId` changes):
    - call `list_questions()`, filter by `sessionID === currentSessionId`
    - enqueue any not already handled
  - Also optionally trigger `list_questions()` after a `tool_end` for tool `"question"` to pick up requests created slightly before the event arrives.

**QuestionDialog UI changes (`QuestionDialog.tsx`):**

- Fix missing React import (`useState`) and redesign props:
  - `request: QuestionRequestEvent | null`
  - `questionIndex: number`
  - `onSubmitRequest(answers: string[][])`
  - `onRejectRequest()`
  - `onUpdateDraftAnswer(index, answer: string[])` (or keep local state and return per-step)
- Render one question at a time:
  - Header: request-level + question header + progress indicator (e.g. `2 / 6`)
  - Options:
    - if `multiple`: checkbox-like toggles (store `string[]` of selected labels)
    - else: radio-like selection (store `[label]`)
  - Show option `description` under the label (small text).
  - Custom answer input:
    - enabled if `custom !== false` (treat undefined as allowed)
    - for single-choice: typing custom clears selected; selecting clears custom
    - for multi-choice: custom becomes an additional entry if non-empty (we’ll send it as another string in the answer array)
  - Buttons:
    - `Next` (until last question)
    - `Submit` on last question
    - `Cancel` → calls reject
    - (Optional) `Back` if you want; default is no-back to keep it simple unless you request it.

**Reply behavior:**

- When user completes the wizard:
  - `reply_question(request_id, answers)` via `src/lib/tauri.ts`
  - close modal
  - mark request id as handled
  - optionally refresh `list_questions()` once to ensure UI stays in sync.

### 4) Update TS bindings

**File:** `src/lib/tauri.ts`

- Add new interfaces mirroring the Rust event payload and API calls.
- Replace `answerQuestion(sessionId, questionId, answer)` usage in `Chat.tsx` with:
  - `replyQuestion(requestId, answers: string[][])`
- Add:
  - `listQuestions(): Promise<QuestionRequestEvent[]>`
  - `rejectQuestion(requestId): Promise<void>`

### 5) Changelog

**File:** `CHANGELOG.md`

- Add an “Unreleased” entry: interactive question prompts (wizard UI), multi-select + custom answers, and correct handling of OpenCode question events.

---

## Tests / validation

### Rust unit tests

**File:** `src-tauri/src/sidecar.rs` (existing `#[cfg(test)] mod tests`)

- Add a test that feeds `parse_sse_event()` a `question.asked` SSE payload shaped like OpenCode:
  - ensures we emit `StreamEvent::QuestionAsked { request_id, session_id, questions[..] }`
  - asserts `multiple/custom/options.description` parse correctly.

### App-level sanity checks

- Run `cargo check --manifest-path src-tauri/Cargo.toml`
- Run `npm run build`
- Manual flow:
  1. Trigger a skill/tool that emits `question`
  2. Confirm modal opens and shows Q1
  3. Select options + custom
  4. Submit and confirm assistant continues
  5. Reload app mid-prompt and confirm pending question is rediscovered via `list_questions()`.

---

## Assumptions

- OpenCode sidecar supports the OpenAPI contract in `openapi_temp.json` (`/question`, `/question/{id}/reply`, SSE event `question.asked`).
- Answer format accepts custom answers as strings in the `answers[i]` array (same shape as selected labels).
- We only display prompts for the currently-selected session (`sessionID` match).
