# Bug Monitor External Log Intake Demo

This fixture exercises the external-project intake path without requiring a workflow run.

## Fixture

- Example log: `docs/fixtures/bug-monitor-external-log-intake/service.log.jsonl`
- Source format: JSON-lines
- Expected result: one Bug Monitor incident/draft for `external_service_crash`

## Example Config

Use a workspace-local copy of this repository when testing path validation.

```json
{
  "bug_monitor": {
    "enabled": true,
    "repo": "frumu-ai/tandem",
    "monitored_projects": [
      {
        "project_id": "external-demo",
        "name": "External demo service",
        "enabled": true,
        "repo": "frumu-ai/tandem",
        "workspace_root": "/home/evan/tandem",
        "log_sources": [
          {
            "source_id": "service-jsonl",
            "path": "docs/fixtures/bug-monitor-external-log-intake/service.log.jsonl",
            "format": "json",
            "minimum_level": "error",
            "start_position": "beginning",
            "watch_interval_seconds": 5
          }
        ]
      }
    ]
  }
}
```

## Smoke Path

1. Save the config through Settings -> Bug Monitor.
2. Confirm the external project panel shows one enabled project and one enabled source.
3. Wait for the watcher to poll.
4. Confirm the source health reports a candidate/submission count.
5. Confirm Bug Monitor incidents include the fixture failure with a `tandem://bug-monitor/...` evidence ref.

For live testing, append a new JSON line with a distinct `fingerprint` or error message so dedupe cooldown does not suppress the candidate.
