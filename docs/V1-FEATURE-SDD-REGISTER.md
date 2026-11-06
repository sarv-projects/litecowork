
# V1 feature specification register

This register tracks package lifecycle, ownership and V1 release-area mapping. Package status is a design/approval state; it is not implementation status. Current code, route, integration and qualification evidence is recorded in IMPLEMENTATION-STATUS.md.

## Lifecycle rule

DRAFT → IN_REVIEW → ACCEPTED/LOCKED → IMPLEMENTING → VERIFIED. BLOCKED and SUPERSEDED are explicit. A feature is not ACCEPTED/LOCKED until its SPEC and required supporting contracts have owner approval. A passing structural validator establishes only package shape.

## Register

| ID | Feature | V1 area | Primary owner domain | Package state | Current source evidence |
|---|---|---|---|---|---|
| 001 | Conversation session supervisor and native turn execution | 1 Universal Chat | Conversation, Agent Runtime, Storage, Operator, UI, Trust | IN_REVIEW. Core decisions B-01–B-10 locked; API/UI/error recovery subcontracts remain DRAFT | Cursor-paginated Conversation UI/API and OpenCode dispatch code path exist; Send remains visibly enabled despite target qualification gate; no strict single-flight/outbox/restart recovery; Stop ineffective for OpenCode, retry UI absent, composer affordances inert and empty state is Antigravity placeholder; other providers not turn-capable |
| 002 | Native agent installation, authentication and binding | 2 BYO Agent | Agent Runtime, Security/Trust, UI | DRAFT | 15 curated candidates/ACP metadata and bounded version observations; auth UNKNOWN/readiness NOT_PROBED. Codex/OpenCode profile probes only; OpenCode offer incompatible, Codex not dispatched by Conversation supervisor. Install/update/auth-begin/native-config are absent/target; descriptor and credential routes 503; binding enable lacks fresh AC-004 descriptor gate. No Usage projection or qualified model/effort picker |
| 003 | Workspace roots and context ingestion | 2, 4, 8 | Workspace, Resources, Context, Storage | DRAFT | Root list/status/scan/observation routes exist; private root add works only Linux/macOS and Windows returns 503; scan is Linux-only metadata census and non-Linux returns 501; root scan does not ingest content. Resource upload/index/lexical retrieval exist separately. ZIP originals remain opaque; source preview worker is development-feature-only and absent from release sidecars. No ContextAttachment/Context Used UI or Conversation retrieval wiring |
| 004 | Agent relay | 1, 2, 6 | Agent Fabric, Conversation, Delegation | DRAFT | Per-Conversation binding switch is served and per-turn producer identity is persisted/projected, but UI type/parser drops producer/session IDs; picker filters Workspace/enabled/lead eligibility but not dispatchable adapter or descriptor/auth freshness. Only OpenCode dispatches, its current offer cannot be newly bound, Codex has no Conversation dispatch branch, and no A→B→A pair exists. Resource materialization is unsupported; B-15/B-16 remain open |
| 005 | Safe Task path | 6 Tasks | Task Runtime, Environment, Trust, Storage | DRAFT | Task record/spec revision/readiness exists; accepted-plan dispatch and Attempt/Effect execution unavailable. Workspace DelegationProfile CRUD/history/performance and SQLite child-admission policy primitives exist, but no daemon/Operator caller or delegated worker execution path is connected |
| 006 | Golden Connection and capability write | 5 Capabilities | LiteSPM, Capability Fabric, Connections, Trust | DRAFT | Discover projections and truthful fail-closed package connect/test; Approval list/get/resolve real; narrow Task Resource-read grant and read-only Invocation history. No account Connection/OAuth routes, provider read/write dispatch, Trust/Effect admission or reconciliation; connect-or-install 503 and test UNKNOWN |
| 007 | Professional Artifacts | 9 Library | Artifact, Presentation, Resource | DRAFT | Artifact list/version/content, text/plain append, promote/archive, safe preview and exact managed-blob local Save As exist. No served Artifact create route, no create UI, no professional format engine/round-trip and no durable ExportReceipt; Task outputs are read-only and have no live producer |
| 008 | Workbench browser, terminal and computer | 4 Workbench | Desktop Shell, Environment, Browser, Trust | DRAFT | Files is Resource projection/preview; Browser is iframe preview; Terminal fabricates command success/ConPTY/Job Object claims without process; Diffs are synthetic; Activity union has no UI branch; Computer-use provider unavailable. No BrowserSession, PTY or Computer routes/providers |
| 009 | Coworkers and scoped memory | 3, 8 | Coworker, Context, Learning, Responsibility | DRAFT | Coworker CRUD/revisions/status/primary persist; editor saves lead/delegation/profile/interaction/failover/context/notification policy with incomplete consumers; pause does not fence already-enabled Automation occurrences and Task composer rejects PAUSED Coworkers for owner Tasks; Quick Create does not open chat; Conversation has no Coworker origin; ContextDocument create rejects Coworker/Goal owner kinds; CoworkerContextPolicy is not enforced by retrieval; no learning service/storage/UI; presence returns 503 |
| 010 | Automations and recurring host | 7 Automations | Automation, Routine, TriggerHost, Task, Notifications | DRAFT | Local 30-second TriggerHost settles supported schedules and materializes READY Tasks only. retry_policy/wake_policy have no consumer; occurrence history UI drops next_cursor. Notifications creates PENDING rows only with no sender/status consumer; preferred_channels ignored; ON_CONDITION maps only to DEPENDENCY_BLOCKED; quiet hours not editable. Pause success copy and module header are stale. External WorkflowBackendAdapter remains target-only |
| 011 | Desktop shell, IPC, settings and Runtime | 10 Desktop | Runtime Lifecycle, IPC, Storage, Desktop UI | DRAFT | Daemon/Tauri shell and OS-authenticated Unix/Windows IPC source exist; T02 source implemented but packaged peer/OS qualification remains open. Mounted SettingsView has four categories only; target 16-category IA/187 controls are not shipped; appearance selectors and shortcut labels are inert/display-only. Windows/macOS native service-manager integration and full Settings error/recovery UI remain open |

## Required package contents

Every package under specs/ contains README.md, SPEC.md, PLAN.md, TASKS.md, DATA-MODEL.md, RESEARCH.md, WIRING.yaml and contracts/README.md, contracts/api-events.md, contracts/errors-recovery.md and contracts/ui-behavior.md.

Each feature contract must cover:

- User outcome and explicit non-goals.
- HLD, LLD, ownership and trusted boundaries.
- Domain identities, lifecycle, data, revision and transaction rules.
- Exact API operation IDs and event/schema mapping.
- Authentication, authorization, leases, idempotency and effects.
- Errors, crash windows, timeout-after-effect and recovery.
- UI screens, exact controls, eligibility, loading/empty/stale/denied/error states and recovery actions.
- Tests by layer: unit, storage, Operator, provider, desktop UI, OS/system and owner acceptance.
- Pinned research and copy/prototype/reject decisions.

## Implementation and qualification

Do not equate a package lifecycle with code status. Use the evidence scale in IMPLEMENTATION-STATUS.md. An implementation task must update the global owner, machine schema, UI action/screen contract, wiring, traceability and tests in the same reviewed change. Feature completion requires a real journey rather than schema counts or component presence.

Validation command: python .specify/scripts/validate_feature_package.py

## Reference comparison and scope note

OpenBot's pinned source review is recorded in [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md). It informs compact Task, model-picker and Usage UX, while its native WorkflowGraph engine remains an external architecture reference. Feature 010 continues to own Automation/Occurrence/Routine-to-Task behavior; workflow graphs stay with connected providers under CONNECTORS-WORKFLOWS.md.
