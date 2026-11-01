# Feature Specifications

Canonical index of feature-level Software Design Document packages.

New material features require a complete package conforming to [the Specification Standard](../docs/SPECIFICATION-STANDARD.md), using templates under [.specify](../.specify/README.md). Feature IDs are stable; do not rename a folder after acceptance.

Planned V1 packages **002–011** are indexed in [V1 Feature SDD register](../docs/V1-FEATURE-SDD-REGISTER.md). Engineering checklist: [Feature and UI–core contract coverage](../docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md). Master build tasks: [LITECOWORK-BUILD-TASKS.md](../docs/LITECOWORK-BUILD-TASKS.md). Traceability: [SPEC-TASK-TRACEABILITY.md](../docs/SPEC-TASK-TRACEABILITY.md). Full file manifest: [COMPLETE-ACTIVE-CORPUS-INVENTORY.md](../docs/COMPLETE-ACTIVE-CORPUS-INVENTORY.md).

| ID | Feature | Status | Package |
|---|---|---|---|
| 001 | Conversation Session Supervisor and Native Turn Execution | DRAFT | [001-conversation-session-supervisor](001-conversation-session-supervisor/README.md) |
| 002 | Native agent install, auth and binding | DRAFT | [002-native-agent-install-auth-binding](002-native-agent-install-auth-binding/README.md) |
| 003 | Workspace roots and context ingestion | DRAFT | [003-workspace-roots-context-ingestion](003-workspace-roots-context-ingestion/README.md) |
| 004 | Agent relay | DRAFT | [004-agent-relay](004-agent-relay/README.md) |
| 005 | Safe Task path | DRAFT | [005-safe-task-path](005-safe-task-path/README.md) |
| 006 | Golden Connection and one capability write | DRAFT | [006-golden-connection-capability-write](006-golden-connection-capability-write/README.md) |
| 007 | Professional artifacts, one format | DRAFT | [007-professional-artifacts](007-professional-artifacts/README.md) |
| 008 | Workbench browser, terminal and computer | DRAFT | [008-workbench-browser-terminal-computer](008-workbench-browser-terminal-computer/README.md) |
| 009 | Coworkers and scoped memory | DRAFT | [009-coworkers-scoped-memory](009-coworkers-scoped-memory/README.md) |
| 010 | Automations and recurring host | DRAFT | [010-automations-recurring-host](010-automations-recurring-host/README.md) |
| 011 | Desktop shell, IPC, settings and runtime health | DRAFT | [011-desktop-shell-ipc-settings-runtime](011-desktop-shell-ipc-settings-runtime/README.md) |

**Validate all:** `python3 .specify/scripts/validate_feature_package.py`

## Package 001 — file manifest (all required for structural validation)

| Path | Role |
|---|---|
| [001-conversation-session-supervisor/README.md](001-conversation-session-supervisor/README.md) | Scope, owners, navigation |
| [001-conversation-session-supervisor/SPEC.md](001-conversation-session-supervisor/SPEC.md) | Problem, EARS, HLD/LLD, blockers |
| [001-conversation-session-supervisor/PLAN.md](001-conversation-session-supervisor/PLAN.md) | Phases, dependency DAG |
| [001-conversation-session-supervisor/TASKS.md](001-conversation-session-supervisor/TASKS.md) | 001-T01–T09 executable tasks |
| [001-conversation-session-supervisor/DATA-MODEL.md](001-conversation-session-supervisor/DATA-MODEL.md) | Records, persistence boundary |
| [001-conversation-session-supervisor/RESEARCH.md](001-conversation-session-supervisor/RESEARCH.md) | Evidence and rivals |
| [001-conversation-session-supervisor/WIRING.yaml](001-conversation-session-supervisor/WIRING.yaml) | Components, transactions |
| [001-conversation-session-supervisor/contracts/README.md](001-conversation-session-supervisor/contracts/README.md) | Contract index |
| [001-conversation-session-supervisor/contracts/api-events.md](001-conversation-session-supervisor/contracts/api-events.md) | OpenAPI/event binding |
| [001-conversation-session-supervisor/contracts/errors-recovery.md](001-conversation-session-supervisor/contracts/errors-recovery.md) | Typed failures |
| [001-conversation-session-supervisor/contracts/ui-behavior.md](001-conversation-session-supervisor/contracts/ui-behavior.md) | UI flows, states, commands |

**Validate:** `python3 .specify/scripts/validate_feature_package.py specs/001-conversation-session-supervisor`

Historical `docs/superpowers/plans/` files remain working records and are not equivalent to a complete accepted feature package.
