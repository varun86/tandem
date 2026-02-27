# Tandem Core HTTP Contracts

This document establishes the expected payloads and canonical behavior for the core HTTP API surface across all SDKs. Every SDK implemented must parse these exact shapes correctly.

## 1. Global Health (`/global/health`)
- **Method:** `GET`
- **Wire Response:** `{"ready": true, "phase": "startup"}`
- **SDK Normalized Response:** `SystemHealth`

## 2. Session List (`/session`)
- **Method:** `GET`
- **Wire Response:** `{"sessions": [{"id": "s_123", "title": "Example", "createdAtMs": 1700000000, "workspaceRoot": "/app"}], "count": 1}`
- **SDK Normalized Response:** `SessionListResponse` containing `[SessionRecord]`

## 3. Session Run Trigger (`/session/:id/prompt_async`)
- **Method:** `POST`
- **Input:** `{"parts": [{"type": "text", "text": "Prompt"}]}`
- **Wire Response:** `{"runID": "r_123"}` 
- **Conflict Response (409):** `{"activeRun": {"runId": "r_123"}}`
- **SDK Normalized Response:** Parses canonical `runId` explicitly.

## 4. Key-Value Resources (`/resource`)
- **Method:** `GET`
- **Wire Response:** `{"items": [{"key": "status", "value": "active", "updatedAtMs": 1700000000}], "count": 1}`
- **SDK Normalized Response:** Canonical fields (`key`, `value`, `updatedAtMs` (TS) / `updated_at_ms` (Py)).

## 5. Semantic Memory (`/memory` / `memory/search` / `memory/put`)
- **Method:** `POST /memory/search`
- **Input:** `{"query": "database details", "limit": 5}`
- **Wire Response:** `{"results": [{"id": "m_1", "text": "Uses Postgres", "sessionID": "s_123"}], "count": 1}`
- **SDK Normalized Response:** Canonical fields (`sessionId` (TS) / `session_id` (Py)).

## 6. Definitions (`/routines`)
- **Method:** `GET`
- **Wire Response:** `{"routines": [{"id": "rt_1", "status": "enabled", "requiresApproval": true}]}`
- **SDK Normalized Response:** Canonical fields (`requiresApproval` (TS) / `requires_approval` (Py)).
