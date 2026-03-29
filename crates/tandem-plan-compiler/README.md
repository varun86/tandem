# tandem-plan-compiler

Mission and plan compiler boundary for Tandem.

This crate is the extracted compiler layer that turns high-level Tandem goals
into governed plan packages, runtime projections, and draft lifecycle behavior.

## Boundary

Consumers should import from `tandem_plan_compiler::api` only.

- `api` is the curated embedding surface for hosts like `tandem-server`
- the rest of the crate layout is implementation detail and may change during
  extraction work

## What stays outside this crate

This crate does not own HTTP, storage engines, provider transport, MCP server
registries, or runtime-side `AutomationV2Spec` persistence. Those remain host
concerns and are supplied through traits or thin adapter modules.

## What this crate owns

- mission and workflow planning
- planner revision flow
- draft lifecycle logic
- runtime projection IR
- shared output-contract seeds and policy defaults
- compiler-facing host traits
