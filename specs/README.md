
# Feature specification packages

This directory contains the feature-level Software Design Document packages for LiteCowork V1. IDs are stable and must not be renamed after acceptance. The normative global architecture, product, domain and machine contracts remain under docs/.

## Package lifecycle

DRAFT → IN_REVIEW → ACCEPTED/LOCKED → IMPLEMENTING → VERIFIED. A package validator checks required files and structure; it does not accept design or establish implementation/qualification.

Owner acceptance was recorded on 2026-11-09 in [OWNER-ACCEPTANCE-REVIEW-PACKET.md](../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md). Packages 001-012 have accepted baselines; Feature 009 revision 1.0 remains accepted while its revision 1.1 amendment is IN_REVIEW under the SPEC's change-control rule. Packages 013-020 were added from the V1 gap analysis and are IN_REVIEW until the whole SPEC of each is accepted. On 2026-11-10 the owner accepted B-38 to B-41, D-01 and B-45 as proposed, deferred all of 013, every Windows row and the release-dependent rows (B-43, B-44), and left B-42 and the B-19 follow-up open (register A-62). A later provider check added the separate A-82 GitHub permission mismatch; both 006 owner decisions remain open. Code status per package is in [../CURRENT-RUN.md](../CURRENT-RUN.md) "Handoff status by package" and each package README. Every package carries a V1 TASKS.md that ends in a "Package VERIFIED for V1" task; the release go/no-go is 019-T23. Delivery waves are in [LITECOWORK-BUILD-TASKS.md](../docs/LITECOWORK-BUILD-TASKS.md). Lifecycle labels describe design acceptance; they do not establish implementation completion or qualification.

On the same 2026-11-09 review the owner also approved folding the proposed tasks P-05 and P-11..P-19 (with the P-00..P-09 rendering and shell set) into their owning packages, including the LOCKED ones, under the ACP native-agent transport direction (register [A-54](../docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md)). The native-agent adapter (Feature 002 002-T17, consumed by Feature 001) is now defined as an Agent Client Protocol (ACP) client with a per-provider launch catalog, keeping the OpenCode v2 HTTP path as one non-ACP adapter until its ACP route qualifies. The folded tasks pushed several packages to new final ids (001 to T28, 002 to T19, 003 to T18, 008 to T21, 011 to T23, 013 to T21, 015 to T21, 016 to T19, 018 to T19); always read the current TASKS.md for the exact ids. The capability-to-task coverage map is [V1-PRODUCT-FEATURE-COVERAGE.md](../docs/V1-PRODUCT-FEATURE-COVERAGE.md).

| ID | Feature | Status | Package |
|---|---|---|---|
| 001 | Conversation Session Supervisor and Native Turn Execution | ACCEPTED/LOCKED 2026-11-09 (B-01..B-10; B-02 rider: OpenCode v2.0.32 first, Windows x64 first row) | [001-conversation-session-supervisor](001-conversation-session-supervisor/README.md) |
| 002 | Native agent installation, authentication and binding | ACCEPTED/LOCKED 2026-11-09 (B-11, B-12) | [002-native-agent-install-auth-binding](002-native-agent-install-auth-binding/README.md) |
| 003 | Workspace roots and context ingestion | ACCEPTED/LOCKED 2026-11-09 (B-13, B-14 with Windows scanner rider) | [003-workspace-roots-context-ingestion](003-workspace-roots-context-ingestion/README.md) |
| 004 | Agent relay | ACCEPTED/LOCKED 2026-11-09 (B-15, B-16) | [004-agent-relay](004-agent-relay/README.md) |
| 005 | Safe Task path | ACCEPTED/LOCKED 2026-11-09 (B-17 Linux-first rider, B-18); B-35 Windows provider PROPOSED | [005-safe-task-path](005-safe-task-path/README.md) |
| 006 | Golden Connection and one capability write | ACCEPTED/LOCKED 2026-11-09 (B-19, B-20); T01/T02 BLOCKED on the client-secret and effective-permission decisions (A-82); Task-write platform order OPEN (A-80) | [006-golden-connection-capability-write](006-golden-connection-capability-write/README.md) |
| 007 | Professional Artifacts, one format | ACCEPTED/LOCKED 2026-11-09 (B-21, B-30, B-31); T01/T06 BLOCKED on receipt ordering and local-write restart recovery review (A-81); destination-class decision is closed | [007-professional-artifacts](007-professional-artifacts/README.md) |
| 008 | Workbench browser, terminal and computer | Revision 1.0 ACCEPTED/LOCKED 2026-11-09 (B-22, B-23, B-32 deferred); revision 1.1 IN_REVIEW/TARGET with A-78/A-83 gates | [008-workbench-browser-terminal-computer](008-workbench-browser-terminal-computer/README.md) |
| 009 | Coworkers and scoped memory | Revision 1.0 ACCEPTED/LOCKED 2026-11-09 (B-24, B-25); revision 1.1 IN_REVIEW (A-91 manual-memory representation remains open) | [009-coworkers-scoped-memory](009-coworkers-scoped-memory/README.md) |
| 010 | Automations and recurring host | ACCEPTED/LOCKED 2026-11-09 (B-26, B-27); CONNECTOR_EVENT over LOCAL_POLL confirmed 2026-11-10 | [010-automations-recurring-host](010-automations-recurring-host/README.md) |
| 011 | Desktop shell, IPC, settings and Runtime | ACCEPTED/LOCKED 2026-11-09 (B-28 Windows-first rider, B-29, B-33) | [011-desktop-shell-ipc-settings-runtime](011-desktop-shell-ipc-settings-runtime/README.md) |
| 012 | Conversation co-edit and take-control | ACCEPTED/LOCKED 2026-11-09 (B-34 model; implementation waits on the 001 Stop fix) | [012-conversation-coedit-take-control](012-conversation-coedit-take-control/README.md) |
| 013 | LiteSPM package lifecycle bridge | IN_REVIEW (new 2026-11-09); B-36, B-37 DEFERRED by the owner 2026-11-10 (all LiteSPM parts; nothing implemented) | [013-litespm-package-lifecycle-bridge](013-litespm-package-lifecycle-bridge/README.md) |
| 014 | Conversation history management | IN_REVIEW (new); B-38 ACCEPTED as proposed 2026-11-10; storage layer v31 only | [014-conversation-history-management](014-conversation-history-management/README.md) |
| 015 | Task control and heterogeneous delegation | IN_REVIEW (new); B-39 ACCEPTED as proposed 2026-11-10 | [015-task-control-delegation](015-task-control-delegation/README.md) |
| 016 | Rich presentation, Intelligent UI and generated media | IN_REVIEW; baseline AC-001..AC-019 acceptance pending at T01; extension AC-020..AC-027 separately gated at T25 (A-84); A-85 blocks MCP proposal exposure | [016-rich-presentation-intelligent-ui](016-rich-presentation-intelligent-ui/README.md) |
| 017 | Data portability, backup and restore | IN_REVIEW (new); B-41 ACCEPTED as proposed 2026-11-10 | [017-data-portability-backup-restore](017-data-portability-backup-restore/README.md) |
| 018 | Usage, diagnostics and observability | IN_REVIEW (new); B-42 PROPOSED and open after the 2026-11-10 review | [018-usage-diagnostics-observability](018-usage-diagnostics-observability/README.md) |
| 019 | Release engineering and V1 qualification | IN_REVIEW (new); B-43, B-44 DEFERRED 2026-11-10 (release dependent); CI and release machinery skeleton in code | [019-release-engineering-qualification](019-release-engineering-qualification/README.md) |
| 020 | Proactivity and quiet learning | IN_REVIEW (new); D-01, B-45 ACCEPTED as proposed 2026-11-10 | [020-proactivity-quiet-learning](020-proactivity-quiet-learning/README.md) |

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

Every new package's RESEARCH.md must record pinned repository SHAs, current vendor documentation links, a copy/prototype/reject decision against LiteCowork invariants and unresolved questions with accountable owners. Tests must name executable commands and evidence outputs. UI contracts must cite the live flow IDs in [FLOWS.md](../docs/FLOWS.md) (F00–F143) and the feature EARS acceptance IDs. The legacy `UIV1-xxx` namespace is retired as an active acceptance authority (register [A-39](../docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md)); treat any UIV1 citation as a provenance pointer to the archived catalog only.
