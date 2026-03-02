# Preset Concepts

## Scope

Defines Tandem's modular preset entities for enterprise-scale reuse across Desktop and Control Panel.

## Entity Definitions

## SkillModule

Reusable building block for agent behavior.
Contains prompt fragments, capability requirements, optional policy profile reference, and optional params schema.

Required fields:

- `id`
- `version`
- `publisher`
- `description`
- `tags`
- `capabilities.required|optional`
- `prompt_fragments` references

## AgentPreset

Composed persona built from one or more `SkillModule` references plus base persona and policy profile.

Required fields:

- `id`
- `version`
- `publisher`
- `description`
- `tags`
- `base_persona`
- `skill_modules[]`
- `capabilities.required|optional`
- `policy_profile`

## AutomationPreset

Reusable automation definition containing mission DAG + routine triggers + task-to-agent bindings.

Required fields:

- `id`
- `version`
- `publisher`
- `description`
- `tags`
- `mission` (steps/edges)
- `routines`
- `task_agent_bindings`
- `capabilities.required|optional`

## Stable IDs and Versioning

- ID format: `<namespace>.<type>.<slug>`
- Types: `skill`, `agent`, `automation`
- IDs are immutable.
- Versions use semver and are immutable artifacts.

Examples:

- `tandem.skill.github.core`
- `tandem.agent.github.pr_worker`
- `tandem.automation.github.pr_triage`

## Portability Rules

- Presets and modules must express behavior via capability IDs.
- Provider-specific tools are only allowed in explicit `non_portable` declarations.
- Runtime resolution maps capabilities to active connector tools.

## Canonical File Layout

```text
skill_modules/<id>.yaml
agent_presets/<id>.yaml
automation_presets/<id>.yaml
prompts/<fragment>.md
```

## Suggested Schemas

## `skill_modules/<id>.yaml`

```yaml
id: tandem.skill.github.core
version: 1.0.0
publisher:
  publisher_id: pub_tandem_official
  display_name: Tandem
  verification_tier: official
description: Core GitHub operations module
tags: [github, engineering, core]
capabilities:
  required: [github.list_issues]
  optional: [github.add_pull_request_comment]
  non_portable: []
policy_profile: policy.github.standard
params_schema:
  type: object
  properties:
    default_repo:
      type: string
prompt_fragments:
  core:
    - prompts/github-core.md
```

## `agent_presets/<id>.yaml`

```yaml
id: tandem.agent.github.pr_worker
version: 1.0.0
publisher:
  publisher_id: pub_tandem_official
  display_name: Tandem
  verification_tier: official
description: PR worker agent preset
tags: [github, pull-request, worker]
base_persona:
  name: GitHub PR Worker
  tone: precise
skill_modules:
  - tandem.skill.github.core@1.0.0
  - tandem.skill.github.pr@1.0.0
  - tandem.skill.communication.concise@1.0.0
capabilities:
  required: []
  optional: []
policy_profile: policy.github.readwrite
params_schema:
  type: object
  properties:
    repo:
      type: string
```

## `automation_presets/<id>.yaml`

```yaml
id: tandem.automation.github.pr_triage
version: 1.0.0
publisher:
  publisher_id: pub_tandem_official
  display_name: Tandem
  verification_tier: official
description: Issue triage to PR automation
tags: [github, automation, triage]
mission:
  steps:
    - id: collect_issues
      action: github.list_issues
    - id: branch_and_pr
      action: github.create_pull_request
  edges:
    - from: collect_issues
      to: branch_and_pr
routines:
  - id: daily_sync
    trigger:
      type: cron
      expression: "0 13 * * *"
    enabled_by_default: false
task_agent_bindings:
  collect_issues: tandem.agent.github.pr_worker@1.0.0
  branch_and_pr: tandem.agent.github.pr_worker@1.0.0
capabilities:
  required: []
  optional: []
```

## Worked Example

## Skill Modules (3)

1. `tandem.skill.github.core@1.0.0`

- required: `github.list_issues`

2. `tandem.skill.github.pr@1.0.0`

- required: `github.create_branch`, `github.create_pull_request`
- optional: `github.add_pull_request_comment`

3. `tandem.skill.communication.concise@1.0.0`

- optional: `slack.post_message`

## Agent Preset

`tandem.agent.github.pr_worker@1.0.0` references all three modules.

Computed agent capability summary:

- required:
  - `github.list_issues`
  - `github.create_branch`
  - `github.create_pull_request`
- optional:
  - `github.add_pull_request_comment`
  - `slack.post_message`

## Automation Preset

`tandem.automation.github.pr_triage@1.0.0`

- task `collect_issues` bound to `tandem.agent.github.pr_worker@1.0.0`
- task `branch_and_pr` bound to `tandem.agent.github.pr_worker@1.0.0`

Computed automation capability summary:

- required:
  - `github.list_issues`
  - `github.create_branch`
  - `github.create_pull_request`
- optional:
  - `github.add_pull_request_comment`
  - `slack.post_message`
