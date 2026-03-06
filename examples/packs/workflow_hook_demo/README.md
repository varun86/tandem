# Workflow Hook Demo

This pack demonstrates Tandem workflow hooks without modifying core code.

Included behavior:

- `task_created -> capability:kanban.update`
- `task_completed -> capability:slack.notify`

Files:

- `workflows/build_feature.yaml`
- `hooks/notifications.yaml`
