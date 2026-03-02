# Prompt Composition

## Scope

Defines deterministic assembly of final runtime prompt for agent execution from base persona + skill modules.

## Deterministic Ordering

Composition order is fixed:

1. core
2. domain
3. style
4. safety

Within each section:

- sort by `order` ascending
- tie-break by `module_id` lexical

A stable separator is inserted between fragments:
`\n\n--- module:<module_id> section:<section> ---\n\n`

## Params Resolution

Merge precedence:

1. invocation params
2. preset defaults
3. module defaults

Validation:

- each module and agent preset can define `params_schema`
- merged params must validate before composition
- invalid params produce `PRESET_PARAMS_INVALID`

## Capability Merge Rules

For an `AgentPreset` with modules:

- required = union(all required)
- optional = union(all optional) minus required
- non_portable = union(all non_portable)

For `AutomationPreset`:

- required = union(task-bound effective agent required) union mission/routine required
- optional = union(task-bound optional) union mission/routine optional minus required

## Policy Merge Rules

- deny overrides allow
- narrower scope overrides broader scope
- final policy is least-privilege merged profile

## Worked Example

Inputs:

- Base persona fragment
- 3 modules:
  - `tandem.skill.github.core`
  - `tandem.skill.github.pr`
  - `tandem.skill.communication.concise`

Fragments:

- base:
  - "You are a GitHub PR worker focused on safe, auditable automation."
- github.core (core):
  - "Gather issue context before proposing code changes."
- github.pr (domain):
  - "Create a branch, implement scoped changes, and open a PR with rationale."
- communication.concise (style):
  - "Respond concisely with bullet points and action summary."
- safety module (safety):
  - "Never execute unapproved side effects outside declared capabilities."

Final composed prompt:

```text
You are a GitHub PR worker focused on safe, auditable automation.

--- module:tandem.skill.github.core section:core ---

Gather issue context before proposing code changes.

--- module:tandem.skill.github.pr section:domain ---

Create a branch, implement scoped changes, and open a PR with rationale.

--- module:tandem.skill.communication.concise section:style ---

Respond concisely with bullet points and action summary.

--- module:tandem.skill.safety.default section:safety ---

Never execute unapproved side effects outside declared capabilities.
```

Computed capabilities:

- required:
  - `github.list_issues`
  - `github.create_branch`
  - `github.create_pull_request`
- optional:
  - `github.add_pull_request_comment`
  - `slack.post_message`

## Reproducibility Contract

For fixed:

- preset id/version
- module refs/versions
- params
- policy profile versions

The output prompt and capability summary must produce a stable `composition_hash`.
