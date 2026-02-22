# What's New in v0.3.11

## Provider + Model Routing

- Fixed custom provider routing so local OpenAI-compatible gateways (for example llama-swap) are used correctly.
- Provider settings now use live model catalogs from the running engine when available.
- Custom provider `selected_model` is persisted and restored correctly.

## Agent Automation + MCP

- Continued Phase 2 automation work for scheduled routines and mission wiring.
- Added/expanded headless automation examples, including mission cold-start benchmark scripts.
- Improved docs for MCP automated agents and setup guidance.

## Release UX + Reliability

- Release notes and update metadata handling improved in Settings.
- Better compatibility handling around update/release metadata sources.
