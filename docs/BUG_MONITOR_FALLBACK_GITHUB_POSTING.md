# Bug Monitor Fallback GitHub Posting Policy

## Purpose

When Bug Monitor triage does not produce a rendered issue body, the fallback path must still create a useful GitHub post. This keeps operators from receiving near-empty issues when triage is still running, has timed out, or failed.

The fallback path is intentionally conservative:

- surface the strongest inline evidence that is already available in `BugMonitorDraftRecord` and `BugMonitorIncidentRecord`
- avoid dumping unbounded payloads into GitHub issues
- keep full evidence available in artifacts for deep debugging

## Evidence sources used by fallback rendering

`build_issue_body` reads from:

- `BugMonitorDraftRecord`:
  - `detail`, `title`
  - `triage_run_id`, `evidence_refs`, `confidence`, `risk_level`, `expected_destination`
  - `quality_gate` and `github_status`
  - `last_post_error` (for triage timeout detail)
- `BugMonitorIncidentRecord`:
  - `incident_id`, `event_type`, `workspace_root`, `excerpt`, `run_id`, `session_id`, `correlation_id`
  - `component`, `level`, `occurrence_count`, `last_seen_at_ms`
  - `evidence_refs`
  - `event_payload` only for deep artifact inspection (not rendered inline in the fallback post)

## Bounded body policy (current implementation)

The fallback post is rendered with bounded sections:

- **Logs**: fenced block using up to 30 incident excerpts or 12 detail lines, truncated to 4,000 chars
- **Evidence**: deduplicated union of draft+incident `evidence_refs`, max 15
- **Diagnostic metadata**: run/session/correlation/component/level and high-signal timing fields when present
- **Triage signal**: confidence, risk, destination, and blocked quality details when `quality_gate` is not passed
- **Triage status marker**:
  - `triage_timed_out`
  - `triage_pending`
  - `github_post_failed`

These markers are intentionally explicit so responders can quickly see why the rendered body is fallback-only.

## Truncation and total-size control

All fallback segments are truncated before the final body assembly:

- per-line truncation for long tokens in logs and metadata fields
- overall post cap (about 12 KB) before hidden markers are appended
- dedicated record caps for nested tool evidence rows and quality details

This keeps issues readable and avoids GitHub API edge cases from oversized posts.

## Operational guidance

If fallback output still looks too weak:

1. verify that the incident or draft has non-empty fields (`incident.excerpt` or `draft.detail`, evidence refs, run/session IDs)
2. verify the triage path and why it timed out (`triage_status` marker in body)
3. inspect linked incident/draft/post artifacts for full payload and run logs
4. prefer improving triage payload quality over increasing fallback caps

Do not use fallback output as a substitute for complete triage; it is a safeguard for actionability, not a replacement for full investigation.
