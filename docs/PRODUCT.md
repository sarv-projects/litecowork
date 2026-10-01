# Product Shape

This document details the user-facing contract defined by
[ARCHITECTURE.md](../ARCHITECTURE.md) §11.

## Navigation

Keep the primary navigation to:

- **Home** — one composer for questions and tasks, plus recent work.
- **Tasks** — durable outcomes, current Steps/Attempts, blockers, and results.
- **Library** — explicit saved/uploaded/imported/linked resources and generated artifacts.
- **Automations** — triggers that create ordinary Tasks under a stated execution policy.
- **Discover** — agents and LitePSM-backed capabilities, skills, connectors, and plugins.

Do not make Agents, Models, MCP, Plugins, Providers, Environments, Memory, or Workflows
default top-level destinations. Show advanced controls contextually or in Settings and
Inspector surfaces.

## Composer and durable work

The same composer accepts a question or an outcome request. A short answer need not
create a Task. A durable, multi-step outcome materializes a Task under the current
Conversation. The UI should make that transition understandable without requiring a
mode toggle.

The attachment menu supports files, folders, screenshots, Library resources, and
connected apps. Connection setup and per-Task capability grants are separate actions.

## Task and Live Desk

Show goal, status, current observable work, Runtime location, artifact results, and any
decision needed from the user. Use ordinary words such as “This computer”, “Cloud”,
“Waiting for your laptop”, and “Needs you”. Agent identity and protocol details belong in
an optional Inspector.

Visual activity must correspond to persisted Task/Step/Attempt or verifier state. Only
show a worker lane after an Attempt exists; only show a capability as used after a real
invocation; only show an Artifact after its version exists; only show a verified mark
after verification completed. Handoffs show checkpointing and a new Runtime taking
ownership, never a process teleport.

## Library and Workbench

Library is a projection over artifacts and explicit user resources, not a cognitive
memory engine. Saving an Artifact to Library is an explicit action.

Workbench renders the current artifact or a provider's real UI. It may host document,
spreadsheet, slide, PDF, image, code/diff, browser, terminal, or MCP app surfaces as
capabilities are integrated. Do not simulate another product's GUI when no such surface
is actually running.

## Motion and accessibility

Motion should explain a real state transition and remain useful with reduced motion
enabled. Reduced-motion behavior can use state changes and short fades without changing
the underlying information or available actions.
