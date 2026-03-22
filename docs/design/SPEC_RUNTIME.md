# Spec Runtime

## Status

Proposed design direction.

This document defines how Tandem should add spec-driven authoring discipline without turning documents, branches, or chat transcripts into the runtime.

## Summary

Tandem should borrow the best parts of spec-driven workflows:

- explicit user stories
- measurable success criteria
- structured ambiguity capture
- traceable task derivation
- planning discipline
- checklist- or constitution-based review

Tandem should not borrow the parts that make documents or agents the runtime:

- markdown files as source of truth
- branch folders as execution substrate
- slash commands as the core coordination model
- chat transcripts as runtime memory
- agents reading docs and improvising without canonical engine state

Tandem's stance remains:

- engine-owned runtime state is canonical
- chat is an interface, not the runtime
- markdown is a projection and review surface, not the source of truth
- specs compile into runtime state
- validations and gates are runtime-owned
- ambiguity is explicit state, not hidden prose
- completion requires runtime evidence, not agent self-report

The architecture in this document is intentionally split into two layers:

1. A domain-extensible core model that can support future domains.
2. A first supported execution profile focused on software delivery work:
   - feature implementation
   - bug repair

That keeps the core reusable without pretending v1 is a full spec platform for every kind of work.

## Problem

Tandem already has strong execution primitives:

- engine-owned run state
- blackboards and workboards
- explicit task claiming
- role-aware routing
- reviewer and tester gates
- checkpoints, replay, and lineage

What Tandem does not yet have is a first-class engine-native model for turning intent into structured execution input.

Today there is still a gap between:

- conversational planning
- markdown planning artifacts
- runtime work graphs and validations

That gap creates drift:

- plans can exist without canonical runtime structure
- acceptance criteria can remain prose instead of validators or gates
- assumptions can be made implicitly and lost
- tasks can be inferred from chat instead of derived from explicit intent
- planning surfaces can become agent-specific instead of runtime-native

Tandem needs a spec layer, but it must compile into the existing runtime spine rather than create a second orchestration system beside it.

## What Tandem Borrows From Spec-Driven Workflows

Tandem should borrow authoring discipline, not runtime architecture.

Spec-driven workflows are useful because they push teams toward:

- clearer statements of user intent
- independent user-story slices
- measurable success criteria
- explicit edge cases and ambiguity capture
- better planning before execution
- traceable mapping from intent to implementation work

These are good inputs to Tandem's engine.

## What Tandem Changes Because It Is Engine-Native

Tandem is not a document runner.

Because the engine is canonical:

- specs are stored as engine artifacts, not only as files
- open questions and assumptions are explicit state objects
- compilation produces `MissionSpec`, `WorkItem`s, and validation linkage
- completion is determined by runtime evidence, not planner prose
- markdown remains an export and review surface, not the execution substrate
- deterministic runtime mapping is preferred over agent interpretation

The key shift is:

- not `spec.md -> agent reads and figures it out`
- but `spec artifact -> compiler -> mission/work graph -> engine-coordinated execution`

## Goals

- Add a first-class engine-native spec model with revisioning and stable IDs.
- Preserve Tandem's canonical runtime and workboard architecture.
- Make ambiguity explicit through open questions and recorded assumptions.
- Compile specs into traceable runtime work graphs.
- Link success criteria to executable validators, reviewer gates, or evidence requirements.
- Support a narrow first shipped profile for software delivery work.
- Keep the core model extensible for later domain reuse.

## Non-Goals

- Replacing the orchestrator or workboard with a document workflow.
- Making markdown files or git branches the runtime source of truth.
- Binding the design to one agent, CLI, IDE, or slash-command system.
- Shipping a full multi-domain spec platform in the first version.
- Making plan variants or markdown sync required to deliver initial value.

## Design Principles

### 1. Engine-owned state stays canonical

Authoring may happen through chat, UI forms, TUI flows, APIs, or imported documents. All of those surfaces must resolve to canonical engine objects.

### 2. Specs compile into runtime state

The engine should execute compiled runtime objects, not raw prose.

### 3. Ambiguity is explicit state

Unknowns must be represented as open questions or assumptions with status, ownership, and lineage.

### 4. Validation is runtime-owned

Acceptance criteria must map to runtime validators, reviewer gates, manual evidence requirements, or unresolved criteria that block completion.

### 5. Deterministic mapping beats agent improvisation

Where possible, the compiler should produce deterministic mappings from spec fields to runtime objects. Heuristic derivations are acceptable only when they remain explicit, inspectable, and revisioned.

### 6. Reusable core, narrow first profile

The canonical model should be domain-extensible. The first supported compilation and execution profile should be software implementation and bug repair only.

## Architecture Overview

This design is split into:

### 1. Domain-Extensible Core Model

Canonical engine objects that are general enough to support future domains.

### 2. First Supported Execution Profile

A narrower compilation profile that turns the core model into runnable software-delivery work:

- feature implementation
- bug repair

Future domains such as research, writing, publishing, and routines should reuse the same core substrate later. They are not part of the first shipped profile.

## Domain-Extensible Core Model

### `SpecArtifact`

Canonical intent artifact stored by the engine.

`SpecArtifact` should stay intentionally small in v1. It is the durable source for authored intent, not a container for every downstream planning detail.

Suggested v1 fields:

- `spec_id`
- `title`
- `status`
- `domain`
- `objective`
- `stories`
- `requirements`
- `success_criteria`
- `constraints`
- `edge_cases`
- `open_question_ids`
- `assumption_ids`
- `source_refs`
- `revision`
- `created_at`
- `updated_at`

Notes:

- `domain` keeps the model extensible.
- `stories`, `requirements`, and `success_criteria` should have stable child IDs.
- v1 should avoid making `SpecArtifact` absorb full architecture plans, contracts, and execution details.

### `OpenQuestion`

Explicit unresolved ambiguity tracked by the engine.

Suggested v1 fields:

- `question_id`
- `spec_id`
- `prompt`
- `status` (`open`, `resolved`, `waived`)
- `blocking`
- `scope_refs`
- `resolution`
- `owner`
- `created_at`
- `resolved_at`

`scope_refs` should identify which stories, requirements, or validation scenarios are affected.

### `Assumption`

Recorded assumption used when compilation proceeds without a fully resolved answer.

Suggested v1 fields:

- `assumption_id`
- `spec_id`
- `statement`
- `source`
- `derived_from_question_id`
- `scope_refs`
- `created_at`

Tandem should not pretend assumptions do not exist. It should record them explicitly and preserve their lineage.

### `ValidationScenario`

Canonical representation of completion expectations linked to runtime evidence.

Suggested v1 fields:

- `validation_id`
- `spec_id`
- `scope_refs`
- `description`
- `kind`
- `blocking`
- `expected_evidence`
- `status`

`kind` should distinguish:

- `executable_validator`
- `reviewer_gate`
- `manual_evidence`
- `unresolved_criterion`

This avoids flattening all success criteria into one validation shape.

### `CompilationRecord`

Lineage object that records how a spec revision became runtime objects.

Suggested v1 fields:

- `compilation_id`
- `spec_id`
- `spec_revision`
- `profile`
- `status`
- `warnings`
- `blocking_question_ids`
- `assumption_ids_used`
- `generated_mission_id`
- `generated_work_item_ids`
- `generated_validation_ids`
- `created_at`

This is the audit trail for why a work graph exists and which inputs produced it.

### Optional `PlanVariant`

`PlanVariant` may remain in the long-term design, but it is not required for early value.

If introduced in v1 at all, it should stay lightweight and optional.

Suggested early role:

- a normalized execution plan attached to a spec revision
- or a small number of explicitly named alternatives

V1 should not depend on full multi-variant planning in order to compile and execute software work.

## First Supported Execution Profile

The first fully supported compilation and execution profile is software delivery work:

- feature implementation
- bug repair

This profile uses the domain-extensible core model but narrows compilation behavior, validator mapping, and operator workflow to something buildable.

### Profile Inputs

For the software profile, the compiler should expect:

- one `SpecArtifact`
- zero or more `OpenQuestion`s
- zero or more `Assumption`s
- zero or more `ValidationScenario`s
- an optional lightweight normalized execution plan

### Profile Outputs

Compilation should produce:

- `MissionSpec`
- `WorkItem` graph
- work-item lineage fields back to the spec
- validation linkage for tasks and mission completion

### Typical Software Work Types

- implement a new feature
- repair a bug with reproduction and verification steps
- deliver a scoped refactor when tied to explicit success criteria

The first shipped profile should not try to model every non-coding workflow.

## Compilation Contract

Compilation is the boundary between authored intent and executable runtime state.

It must be explicit.

### Required Before Compilation

The compiler needs enough structure to create a meaningful work graph.

Minimum required inputs:

- `SpecArtifact.objective`
- at least one scoped story or requirement
- at least one completion expectation in `success_criteria` or `ValidationScenario`

Compilation may also require additional fields for the software profile, such as reproduction context for bug repair or target scope for feature work.

### Blocking Conditions

Compilation must block for affected areas when:

- a blocking `OpenQuestion` has not been resolved or waived
- a requirement is too ambiguous to derive a safe work item
- a completion criterion has no valid validation path and cannot be downgraded to manual evidence
- the requested scope cannot be mapped to the first supported execution profile

The compiler should return blocking reasons explicitly, not bury them in prose.

### Warnings and Partial Progress

Compilation may proceed with warnings when:

- a non-blocking `OpenQuestion` is converted into a recorded `Assumption`
- some work-item descriptions require heuristic derivation
- some criteria map only to reviewer gates or manual evidence rather than executable validators

Warnings must be persisted in `CompilationRecord` and surfaced to the operator before execution.

### Deterministic vs Heuristic Mapping

Preferred deterministic mappings:

- story and requirement IDs -> work-item lineage fields
- success criterion with explicit test shape -> validation linkage
- blocking question -> blocked compilation scope
- explicit gate requirement -> reviewer or tester gate metadata

Allowed heuristic mappings, if explicit and inspectable:

- decomposing a high-level requirement into multiple implementation tasks
- inferring validation grouping from multiple related criteria
- generating likely file-scope hints from repository inspection

Heuristic output must still be stored as structured runtime state. The runtime must never depend on undocumented agent interpretation of markdown or chat.

## Runtime Mapping for the Software Profile

For the first supported profile, compilation should map core spec objects into runtime state as follows:

- `SpecArtifact.objective` -> `MissionSpec.objective`
- stories and requirements -> candidate `WorkItem`s
- `OpenQuestion.blocking=true` -> blocked compilation for affected scope
- resolved non-blocking questions -> `Assumption`s if needed
- `ValidationScenario.kind=executable_validator` -> validator linkage
- `ValidationScenario.kind=reviewer_gate` -> reviewer gate linkage
- `ValidationScenario.kind=manual_evidence` -> evidence requirement linkage
- `ValidationScenario.kind=unresolved_criterion` -> blocked completion or blocked compilation depending on scope

This keeps the mapping explicit and engine-native.

## Validation and Completion Model

Not every criterion becomes the same kind of runtime object.

### Executable Validator

Use when the criterion can be checked by code, tests, contracts, schemas, or benchmark logic.

Examples:

- failing test reproduced
- API contract snapshot matches expected shape
- benchmark stays under a defined threshold

### Reviewer Gate

Use when human review is required before promotion or completion.

Examples:

- architecture review
- UX acceptance review
- risk-sensitive change review

### Manual Evidence Requirement

Use when completion requires evidence but not a fully executable validator.

Examples:

- attach reproduction notes
- attach screenshots or logs
- confirm rollout steps were performed

### Unresolved Criterion

Use when a criterion still needs clarification before it can be safely mapped.

This should not silently disappear into planner prose.

### Completion Rule

Work is complete only when linked validators, gates, and evidence requirements are satisfied in runtime state.

Agent claims of completion are not sufficient.

## Workboard Integration

Compiled tasks should land on the workboard as first-class `WorkItem`s, not as loose checklist items copied from a document.

Each generated `WorkItem` should carry lineage fields such as:

- `source_spec_id`
- `source_story_ids`
- `source_requirement_ids`
- `source_validation_ids`
- `assumption_ids`
- `assigned_role`
- `gate`
- `file_scope`

This lets the engine answer:

- why the task exists
- which intent it serves
- which assumption it depends on
- which evidence closes it

## Authoring and Clarification Flow

### Stage 1: Capture

The operator creates or updates a `SpecArtifact` from Desktop, TUI, API, or other authoring surfaces.

### Stage 2: Clarify

Tandem extracts and records `OpenQuestion`s instead of hiding ambiguity in prose.

### Stage 3: Normalize

Where safe, Tandem records explicit `Assumption`s and normalizes validation scenarios for the first supported execution profile.

### Stage 4: Compile

The compiler generates `MissionSpec`, `WorkItem`s, and validation linkage with a `CompilationRecord`.

### Stage 5: Execute

The existing orchestrator, workboard, gates, and runtime evidence model execute the compiled output.

This is still one engine and one orchestration system.

## Markdown and Git Surfaces

Markdown support remains useful, but it is not part of the critical path for v1.

In early phases, markdown should be treated as:

- a projection for review
- an export surface
- a future import surface

Markdown should not be treated as:

- canonical runtime truth
- a required execution substrate
- the object the engine actually runs

Broad markdown sync, round-trip editing, and import/export parity should be deferred until the canonical engine model is stable.

## Relationship to Existing Planning Modes

The current planning mode uses visible markdown plans and gating around those plans.

This proposal keeps visual planning, but changes what is canonical:

- the engine owns the spec state
- the engine owns the compiled work graph
- approvals should ultimately bind to engine revisions and staged operations
- markdown plans become a surface, not the runtime record

This should evolve the existing planning UX, not replace the engine with a doc workflow.

## API and UI Direction

### Engine API

Early APIs should focus on:

- create/get/update/list `SpecArtifact`
- create/resolve/list `OpenQuestion`
- create/list `Assumption`
- compile spec revision into runtime state
- inspect `CompilationRecord`

V1 does not need full markdown import/export APIs or advanced plan-variant management.

### Desktop and TUI UX

The first operator flow should support:

- create or edit a spec
- review stable IDs for stories and requirements
- resolve or waive open questions
- inspect compilation blockers and warnings
- preview generated work graph
- execute against the existing runtime
- inspect linked validation and evidence state

That is enough to prove the architecture without building a full spec authoring suite in one pass.

## Phased Rollout

### Phase 1: Canonical `SpecArtifact`

- Add engine-side `SpecArtifact` persistence with revisioning and stable IDs.
- Support stories, requirements, success criteria, and constraints at a practical v1 depth.
- Expose basic create/get/update/list flows in engine, Desktop, and TUI.

### Phase 2: Open Questions and Clarification

- Add `OpenQuestion` persistence and blocking semantics.
- Allow resolve and waive workflows.
- Add explicit `Assumption` recording for non-blocking ambiguity.

### Phase 3: Compilation to Runtime Work Graph

- Compile a spec revision into `MissionSpec` and `WorkItem`s for the software profile.
- Persist `CompilationRecord`.
- Surface deterministic outputs, heuristic outputs, warnings, and blockers explicitly.

### Phase 4: Validation Linkage

- Map criteria to executable validators, reviewer gates, or manual evidence requirements.
- Require runtime evidence for completion.
- Surface validation linkage in runtime views and events.

### Phase 5: Expansion

- Add optional lightweight `PlanVariant` support where useful.
- Add markdown export and later import surfaces.
- Add additional domain profiles on top of the same core model.

Research, writing, publishing, and routines belong here, after the software profile is stable.

## Initial Acceptance Criteria

The first milestone should prove the engine-native architecture, not the whole future platform.

- A user can create, revise, and retrieve a durable `SpecArtifact` through engine-native flows.
- Stories, requirements, and success criteria have stable IDs and revisioned storage.
- `OpenQuestion`s are first-class engine objects with blocking semantics.
- Non-blocking ambiguity can be converted into explicit, traceable `Assumption`s.
- A spec revision can compile into `MissionSpec` and a `WorkItem` graph for feature implementation or bug repair.
- Generated `WorkItem`s preserve traceability back to spec IDs, validation IDs, and assumptions where applicable.
- Validation linkage distinguishes executable validators, reviewer gates, manual evidence requirements, and unresolved criteria.
- Completion depends on runtime-owned evidence and gate state, not agent self-report.
- Markdown is not required as the canonical execution substrate.

## Why This Fits Tandem

This design strengthens Tandem where it is currently weakest while preserving what is already strong.

It adds disciplined spec authoring and compilation without weakening:

- engine-owned state
- workboard execution
- approval-aware coordination
- validator and gate semantics
- replayable lineage

That is the right shape for Tandem: spec-driven inputs, engine-native runtime.
