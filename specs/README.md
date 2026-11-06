
# Feature specification packages

This directory contains the feature-level Software Design Document packages for LiteCowork V1. IDs are stable and must not be renamed after acceptance. The normative global architecture, product, domain and machine contracts remain under docs/.

## Package lifecycle

DRAFT → IN_REVIEW → ACCEPTED/LOCKED → IMPLEMENTING → VERIFIED. A package validator checks required files and structure; it does not accept design or establish implementation/qualification.

| ID | Feature | Status | Package |
|---|---|---|---|
| 001 | Conversation Session Supervisor and Native Turn Execution | IN_REVIEW — core decisions B-01–B-10 locked; API/UI/error subcontracts remain DRAFT | [001-conversation-session-supervisor](001-conversation-session-supervisor/README.md) |
| 002 | Native agent installation, authentication and binding | DRAFT | [002-native-agent-install-auth-binding](002-native-agent-install-auth-binding/README.md) |
| 003 | Workspace roots and context ingestion | DRAFT | [003-workspace-roots-context-ingestion](003-workspace-roots-context-ingestion/README.md) |
| 004 | Agent relay | DRAFT | [004-agent-relay](004-agent-relay/README.md) |
| 005 | Safe Task path | DRAFT | [005-safe-task-path](005-safe-task-path/README.md) |
| 006 | Golden Connection and one capability write | DRAFT | [006-golden-connection-capability-write](006-golden-connection-capability-write/README.md) |
| 007 | Professional Artifacts, one format | DRAFT | [007-professional-artifacts](007-professional-artifacts/README.md) |
| 008 | Workbench browser, terminal and computer | DRAFT | [008-workbench-browser-terminal-computer](008-workbench-browser-terminal-computer/README.md) |
| 009 | Coworkers and scoped memory | DRAFT | [009-coworkers-scoped-memory](009-coworkers-scoped-memory/README.md) |
| 010 | Automations and recurring host | DRAFT | [010-automations-recurring-host](010-automations-recurring-host/README.md) |
| 011 | Desktop shell, IPC, settings and Runtime | DRAFT | [011-desktop-shell-ipc-settings-runtime](011-desktop-shell-ipc-settings-runtime/README.md) |

## Package 001 manifest

| Path | Role |
|---|---|
| [README.md](001-conversation-session-supervisor/README.md) | Scope, owners, lifecycle and current evidence |
| [SPEC.md](001-conversation-session-supervisor/SPEC.md) | Problem, goals, HLD/LLD, locked/flexible decisions and acceptance |
| [PLAN.md](001-conversation-session-supervisor/PLAN.md) | Phases, dependencies, risks and gates |
| [TASKS.md](001-conversation-session-supervisor/TASKS.md) | Executable implementation and qualification tasks |
| [DATA-MODEL.md](001-conversation-session-supervisor/DATA-MODEL.md) | Records, identity and persistence boundary |
| [RESEARCH.md](001-conversation-session-supervisor/RESEARCH.md) | Pinned evidence and alternatives |
| [WIRING.yaml](001-conversation-session-supervisor/WIRING.yaml) | Component/owner/transaction graph |
| [contracts/README.md](001-conversation-session-supervisor/contracts/README.md) | Contract index |
| [contracts/api-events.md](001-conversation-session-supervisor/contracts/api-events.md) | OpenAPI/event mapping |
| [contracts/errors-recovery.md](001-conversation-session-supervisor/contracts/errors-recovery.md) | Typed errors, crash windows and recovery |
| [contracts/ui-behavior.md](001-conversation-session-supervisor/contracts/ui-behavior.md) | UI flows, states, commands and tests |

## Global engineering links

- [Specification standard](../docs/SPECIFICATION-STANDARD.md)
- [V1 feature register](../docs/V1-FEATURE-SDD-REGISTER.md)
- [Feature and UI-core coverage](../docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md)
- [Typed UI-Core contracts](../docs/UI-CORE-TYPED-CONTRACTS.md)
- [Build tasks](../docs/LITECOWORK-BUILD-TASKS.md)
- [Spec/task traceability](../docs/SPEC-TASK-TRACEABILITY.md)
- [Implementation status](../docs/IMPLEMENTATION-STATUS.md)
- [Acceptance and E2E plan](../docs/ACCEPTANCE-AND-E2E-PLAN.md)

Every new package's RESEARCH.md must record pinned repository SHAs, current vendor documentation links, a copy/prototype/reject decision against LiteCowork invariants and unresolved questions with accountable owners. Tests must name executable commands and evidence outputs. UI contracts must cite the relevant UIV1 IDs and feature acceptance IDs.
