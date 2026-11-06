# LiteCowork

**One workspace for the AI agents you already use.**

LiteCowork is a local-first, chat-first desktop AI workspace built around native agent harnesses. It brings Conversations, persistent Coworkers, durable Tasks, local resources, capabilities, Automations, Artifacts and evidence into one coherent desktop experience without replacing the selected native agent's reasoning engine.

**Release posture: pre-release development. V1 is not release-qualified.** The application and daemon source are present in this repository. The current implementation boundary, test results and blockers are documented in docs/IMPLEMENTATION-STATUS.md.

## Product principles

- **BYOA first:** preserve native agent behavior, authentication, models, prompts, tools, private sessions and agent-owned controls.
- **Conversation first:** ordinary chat does not require a Task or Coworker.
- **Durable work:** outcome-oriented work may become an explicit Task whose state survives replaceable workers and daemon restarts.
- **Honest completion:** agent self-report is not verification. Effects, artifacts and acceptance evidence determine whether work is complete.
- **Local ownership:** V1 execution is local to the user's desktop unless the user explicitly connects a provider. Laptop-off cloud continuation is later scope.
- **Shared capabilities:** LiteSPM is the package lifecycle authority across compatible agent hosts; LiteCowork separately governs assignment, grants and mediated use.
- **One workspace:** chat, files, work, Coworkers, Routines, Automations and deliverables share one Workspace and history model.
- **Progressive disclosure:** plain-language outcomes are primary; agent options, policy, process and evidence are inspectable when needed.

## Main product objects

| Object | Meaning |
|---|---|
| Workspace | Ownership, resource and authorization boundary |
| Conversation | Human-facing chat history and turns; may use different native agents over time |
| Coworker | Persistent assistant identity, instructions, scoped context and approved responsibilities; not a resident process |
| Task | Durable outcome-oriented work with accepted plan, steps, attempts, effects, artifacts and evidence |
| AgentModule / AgentBinding | Native harness integration and Workspace-specific selection |
| Capability / Connection | A package or external service; installation, authentication, assignment and invocation remain separate |
| Resource / Artifact | Pinned source material or committed output with revision and provenance |
| Automation / Routine | Triggered or reusable work definition; local trigger hosting does not imply Task execution |

## Architecture

- Rust workspace and local daemon: apps/litecoworkd
- Tauri v2 / React desktop application: apps/litecowork-ui
- Domain, storage, runtime and IPC crates: crates/
- Normative product and domain contracts: docs/
- Feature SDD packages: specs/
- Specification templates and validators: .specify/

The desktop Operator is not the Runtime. The daemon owns durable operations and lifecycle; the native agent owns its own reasoning and private state; LiteSPM owns package lifecycle. A route, package listing, probe or UI affordance is not proof of a qualified product flow.

## Product and engineering documentation

Read in this order:

1. [Product contract](docs/PRODUCT.md)
2. [V1 product release contract](docs/V1-PRODUCT-RELEASE-CONTRACT.md)
3. [System architecture](ARCHITECTURE.md)
4. [Current implementation and release status](docs/IMPLEMENTATION-STATUS.md)
5. [V1 desktop screen and interaction contract](docs/V1-UI-SCREEN-CONTRACT.md)
6. [Domain owner contracts and machine schemas](docs/README.md)
7. [Feature SDD register](docs/V1-FEATURE-SDD-REGISTER.md) and [specifications](specs/README.md)
8. [Acceptance and E2E plan](docs/ACCEPTANCE-AND-E2E-PLAN.md)

Useful engineering indexes:

- [V1 design index](docs/V1-DESIGN-INDEX.md)
- [Contradiction and alignment register](docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md)
- [Semantic served-handler to UI map](docs/SEMANTIC-SERVED-UI-MAP.md)
- [Feature and UI-core coverage](docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md)
- [Repository documentation audit](docs/REPOSITORY-DOCUMENTATION-AUDIT.md)
- [Active corpus inventory](docs/COMPLETE-ACTIVE-CORPUS-INVENTORY.md)
- [Served-route inventory](docs/schemas/served-route-inventory.json)

## Development

The Rust workspace is rooted at Cargo.toml. The desktop package uses pnpm and is under apps/litecowork-ui. Use the package scripts and Rust workspace configuration in the repository; do not infer the supported build matrix from a developer machine alone.

Before claiming a feature works, trace its screen/action to the typed Tauri command, authenticated Operator route, service owner, storage/effect transaction, provider integration, stream/projection and recovery behavior. Record CODE, SERVED, INTEGRATED and QUALIFIED separately.

## Current delivery truth

Conversation CRUD and an OpenCode-only code-level dispatch path exist, but the visible Send control violates the disabled-until-qualified gate, Stop is ineffective for OpenCode, failed-turn Retry has no UI affordance, and no packaged provider USER journey is qualified. The local 30-second Automation TriggerHost settles supported schedules and materializes READY Tasks but does not execute them. Notification outcomes can create PENDING delivery records, but no outbound sender advances them to delivered states. Task planning/Attempt admission, a qualified mediated Effect path, LiteSPM lifecycle integration, a real process-backed terminal, a controlled browser, Computer use and the full Coworker/memory experience are not release-qualified. The recorded Windows Rust workspace suite passed 359 daemon tests (0 failed, 1 ignored) and exited 0 overall; frontend production build and focused Conversation/responsive tests passed, while the full serial frontend suite remains incomplete and packaged desktop/provider E2E is unqualified. The precise evidence and ordered blockers are maintained in [IMPLEMENTATION-STATUS.md](docs/IMPLEMENTATION-STATUS.md).
