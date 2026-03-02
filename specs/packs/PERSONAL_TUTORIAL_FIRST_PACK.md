# Personal Tutorial: Build Your First Tandem Pack

This is a practical, short tutorial to help you learn by doing.

## Goal

Create, zip, import, and run one workflow pack end-to-end.

Use this template first:

- `examples/packs/daily_github_pr_reviewer/`

## 1) Inspect the pack structure

A Tandem pack is installable only when `tandempack.yaml` exists at zip root.

Required files in this tutorial pack:

- `tandempack.yaml`
- `agents/github_reviewer.md`
- `missions/daily_pr_review.yaml`
- `routines/daily_pr_review.yaml`
- `README.md`

## 2) Zip the pack

Run from repo root:

```bash
cd examples/packs/daily_github_pr_reviewer
zip -r daily_github_pr_reviewer.zip tandempack.yaml README.md agents missions routines
```

## 3) Import the zip

In Control Panel:

- Open `Settings` -> `Packs`
- Install from file/path
- Select `daily_github_pr_reviewer.zip`

Expected behavior:

- Pack is detected
- Pack installs under deterministic path in `TANDEM_HOME/packs/<name>/<version>/`
- Mission and routine are registered

## 4) Validate capabilities

This template requires:

- `github.list_pull_requests`
- `github.create_issue`

Optional:

- `slack.post_message`

If required capabilities are missing, the run should return a structured missing-capability error.

## 5) Run and observe

Run the workflow manually once and verify:

- Mission executes
- Required capability resolution succeeds
- Routine remains disabled by default after install

## 6) Learn by editing

Try these edits and re-zip/re-import:

- Change mission steps
- Add/remove optional capability
- Rename agent prompt style

## Starter packs included

- `examples/packs/daily_github_pr_reviewer/` (workflow)
- `examples/packs/slack_release_notes_writer/` (workflow)
- `examples/packs/customer_support_drafter/` (skill)
