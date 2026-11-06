# Feature and UI–Core contract coverage

This document maps V1 product areas to normative domain owners, actual implementation, UI, feature SDD and missing qualification evidence. It does not authorize implementation and does not create domain or machine authority.

## 1. Authority

| Question | Authority |
|---|---|
| Product and release boundary | PRODUCT.md, V1-PRODUCT-RELEASE-CONTRACT.md |
| Global invariants and ownership | ../ARCHITECTURE.md, DATA-MODEL.md, STATE-MACHINES.md |
| Intended UX | V1-UI-SCREEN-CONTRACT.md, V1-UI-ACTION-STATE-LEDGER.md, DESKTOP-SHELL-LLD.md |
| Current implementation | IMPLEMENTATION-STATUS.md and active source |
| Actual route registrations | schemas/served-route-inventory.json |
| Feature design/lifecycle | ../specs/README.md and V1-FEATURE-SDD-REGISTER.md |
| Release proof | ACCEPTANCE-AND-E2E-PLAN.md, TESTING.md |

The composed Operator has 173 OpenAPI route registrations, one authenticated local-only operation outside OpenAPI and 56 OpenAPI operations not mounted. These are registration counts, not counts of integrated or qualified product capabilities.

## 2. Ten product areas

| # | V1 product area | Normative owners | Actual source boundary | UI surfaces | SDD | Required release evidence |
|---|---|---|---|---|---|---|
| 1 | Universal Chat | EXPERIENCE.md, API.md, STATE-MACHINES.md, AGENT-CONTROL.md, NATIVE-COMPOSER-LLD.md | Conversation CRUD/presentation and OpenCode-only submit; Recent Chats pagination/sidebar Load more and pending-create journal retry work. Send is visibly enabled despite the disabled-until-qualified target gate; no strict single-flight/outbox/restart recovery; Stop is ineffective for OpenCode; failed-turn Retry has no UI (pending-create journal retry is available); attachments/voice/more-options are inert; empty state is Antigravity placeholder | New Chat, Recent Chats, Load more chats, composer, agent picker, message/history, Stop/Retry, static empty canvas | 001 IN_REVIEW | Enforce Send qualification gate; single-flight, crash/restart recovery, real stop/quiescence, retry UI/route, attachment/context/voice and truthful empty state; packaged Windows OpenCode journey and owner USER acceptance |
| 2 | Bring Your Own Agent | AGENT-FABRIC.md, AGENT-CONTROL.md, AGENT-ACCOUNT-ROUTE-MATRIX.md, DELEGATION.md | 15 curated candidates/ACP metadata and bounded version observations; auth UNKNOWN/readiness NOT_PROBED. Codex/OpenCode probes only; OpenCode incompatible, Codex not dispatched. No installer/native sign-in; descriptor/credential routes 503; binding enable omits fresh AC-004 descriptor gate; model/effort and Usage remain target. ENABLED binding is eligibility, not worker/session readiness | Agent catalog/ACP metadata, bounded probes, Workspace bindings/default lead, subagent profiles/history/performance/quota; no install/auth/Usage/model picker journey | 002 DRAFT; Delegation owner contracts | Exact first OS/harness/auth tuple; descriptor gate; installer/auth; dispatchable adapter and worker USER journey; child Attempt/Environment/Trust admission |
| 3 | Persistent Coworkers | COWORKERS-TARGET.md, COWORKER-FLOWS.md, LEARNING.md, RESPONSIBILITIES.md | Coworker CRUD/revision/status/Workspace primary and CoworkerContextPolicy persistence are served. Editor saves lead/delegation/profile/interaction/failover/context/notification preferences with incomplete consumers; pause does not fence already-enabled Automation occurrences and Task composer blocks PAUSED Coworkers for owner Tasks. Home Quick Create does not open a chat; Conversation has no Coworker origin; Task alone pins Coworker origin. ContextDocument create accepts only USER/PERSONAL_PROFILE and WORKSPACE/WORKSPACE_NOTES; Coworker/Goal kinds are rejected. CoworkerContextPolicy is not enforced by retrieval. Learning gates are deterministic only; no LearningService/store/routes/Inbox. Presence returns 503. | Coworker roster/editor/status/primary, Agent Settings worker-profile assignment and Task-origin selection are served; Coworker chat, Coworker Memory/Knowledge browser, scoped retrieval, Learning Inbox and full responsibility enforcement remain target | 009 DRAFT | Close B-24/B-25; align pause/admission semantics; Coworker-owned Conversation, scoped ContextDocument/retrieval/revoke/purge, policy consumers, learning decision boundary, presence and desktop USER restart/isolation journey |
| 4 | Real Computer and Workbench | DESKTOP-SHELL-LLD.md, ENVIRONMENTS.md, CHROME-BROWSER-LLD.md, V1-INTEGRATED-TERMINAL-LLD.md | Files is Resource projection/preview; Browser iframe is not Chrome; Terminal fabricates successful exit codes and ConPTY/Job Object claims without process; Diffs use modulo/static sample; Activity enum has no rendered branch; Computer provider unavailable. No BrowserSession/PTY/Computer route or provider | Workbench Files, embedded Browser preview, simulated Terminal, synthetic Diffs; no Activity or live Computer surface | 008 DRAFT | B-22 browser provider/mode; B-23 PTY/ConPTY provider and Environment lifecycle; B-32 Computer-use include/defer and OS provider; remove fake success/containment claims; actual revision/VCS diff and USER acceptance |
| 5 | Capabilities, Plugins, MCP and Connections | CAPABILITY-FABRIC.md, CAPABILITY-INVOCATIONS.md, LITESPM-LITECOWORK-INTEROP-LLD.md, CONNECTORS-WORKFLOWS.md | Discover projection; connect-or-install 503 and test UNKNOWN; Approval decision is real; narrow Task Resource-read grant and read-only Invocation GET only. No account Connection/OAuth, provider read/write, CapabilityBroker dispatch, Effect reconciliation or LiteSPM lifecycle bridge | Discover Explore/Installed/Managed/Connections and Needs You ApprovalDetail; all Discover row actions disabled, no account connection or capability grant/write UI | 006 DRAFT | Pinned LiteSPM bridge conformance; exact provider OAuth; scoped read/write; exact-digest approval; invocation/effect dispatch, idempotency/reconciliation and revoke USER path |
| 6 | Real Tasks and Multi-Agent Work | TASK-RUNTIME.md, ENVIRONMENTS.md, TRUST.md, DELEGATION.md | Task CRUD/spec revision/readiness; planning says dispatch unavailable; plan submission and Attempt/effect control target-only. Workspace DelegationProfile CRUD/revisions/disable/archive/history/performance exist in Agent Settings; SQLite child-admission guard reads profile allowlist/budget but no daemon/Operator caller or worker dispatcher exists | Work list/detail, objective edit, readiness; Agent Settings subagent profiles and performance/quota views; Plan/Activity/Workers/Inspector target | 005 DRAFT | One safe Task plan→Attempt→Environment→Trust→Effect→verification vertical slice, including a real delegated child Attempt and its profile/budget admission |
| 7 | Routines and Automations | ROUTINES.md, AUTOMATION.md, RESPONSIBILITIES.md, CONNECTORS-WORKFLOWS.md | Local 30-second TriggerHost and recurring schedule settlement materialize READY Tasks only. Automation definitions paginate, but occurrence history UI drops next_cursor. retry_policy/wake_policy have no consumer. Notification producer writes PENDING rows only; no sender/status consumer; preferred_channels ignored; ON_CONDITION is only DEPENDENCY_BLOCKED; quiet hours not editable. Pause success copy and automation_operator.rs header are stale. WorkflowBackendAdapter remains target-only | Automations/templates/editor/health/first-page Runs; Routine editor/run; Notifications history/preferences; no provider workflow selector | 010 DRAFT | Durable request recovery; occurrence history pagination; consume retry/wake policies or remove misleading controls; notification sender/channel selection/quiet-hours/condition contract; sleep/offline/misfire/DST/dedupe qualification; Task execution; later qualified provider adapter |
| 8 | Memory, Knowledge and Learning | CONTEXT.md, LEARNING.md, WORLD-RESOURCES.md | ResourceService accepts Personal Profile and Workspace Notes ContextDocuments; desktop Library creates Workspace Notes only. Workspace/Resource lexical search and read-only grounded-retrieval route exist, but no durable ContextAttachment/receipt, Context Used UI or Conversation wiring. WorkspaceRoot add is unavailable on Windows; scan is Linux-only metadata census, not ingestion. ZIPs are opaque; source preview worker is dev-feature-only and absent from release sidecars. Coworker/Goal ContextDocument create is rejected; Coworker retrieval policy is not wired; Coworker-private MemoryRecord persistence and LearningService do not exist. | Resource picker/Library, Workspace Notes and lexical search served; Context Used, Coworker Memory/Knowledge, Learning Inbox, review/revoke and scoped attachment remain target | 003 and 009 DRAFT | Resolve first qualified OS/formats and watcher policy; root content ingestion and exact revision provenance; durable ContextAttachment/retrieval receipt and Conversation wiring; Coworker/Workspace scope fences, revoke/purge and any separately accepted learning lifecycle |
| 9 | Library and Finished Work | ARTIFACTS-EVIDENCE.md, PRESENTATION-RUNTIME.md, RICH-RESPONSE.md | Artifact list/version/content, bounded text/plain append, promote/archive, safe previews and exact-version local Save As; no createArtifact route/UI, professional format engine or durable ExportReceipt. Office imports remain Resources; Task outputs read-only without producer | Saved/Task outputs/Archived Library views; ArtifactWorkbench exact version/preview/compare/text edit/Save As; no create/import-convert entry point | 007 DRAFT | B-21 exact format engine/round-trip; B-30 initial Artifact create/provenance transaction; B-31 durable export receipt distinct from local Save As; Task producer dependency |
| 10 | Desktop Shell, Settings and Runtime | RUNTIME-LIFECYCLE.md, LOCAL-OPERATOR-IPC.md, DESKTOP-SHELL-LLD.md, TRUST.md | Tauri shell, daemon and OS-authenticated IPC source exist; mounted SettingsView exposes four categories only (General/Workspaces/Agents/Runtime). Appearance values are inert component state; shortcut labels are not wired; WorkspaceRoot/policy/enrollment controls and Runtime error/retry recovery affordances are not rendered. Windows named-pipe source exists, but packaged OS-peer USER qualification and Windows/macOS service-manager integration remain open | Sidebar, Workspace picker, SettingsView, Runtime diagnostics, Workbench | 011 DRAFT | Qualify authenticated IPC on declared OS; implement/qualify actual preference persistence and apply; reconcile every visible control and error/retry state; package/service-manager, restart/offline/recovery and USER tests |

## 3. Required UI-to-Core trace

For every user-visible action, feature packages and ledgers must trace:

User outcome → exact visible control → React component → typed Tauri command → authenticated local IPC → composed Operator route/native action → authorization and Workspace scope → owning service → transaction/version/fence → persisted event/receipt/projection → stream/reconnect → recovery behavior → acceptance evidence.

A route without a visible control is a backend surface, not a completed user journey. A control without a real owner operation is a prototype, not an implementation.

## 4. Transport boundary

React controls do not own durable truth. The Tauri bridge calls the local Operator through OS-authenticated IPC; the daemon verifies scope and invokes the owning service. The WebView must not receive Runtime secrets, raw native session handles, process ownership or authority-bearing provider tokens.

## 5. SDD lifecycle

Feature 001 is IN_REVIEW: core decisions B-01–B-10 are locked but dependent API/UI/error/recovery subcontracts remain draft. Features 002–011 are DRAFT. Structural validation is not owner acceptance or implementation authorization. See V1-FEATURE-SDD-REGISTER.md and ../specs/README.md.

## 6. Current release blockers

- Conversation dispatch recovery and one exact native provider qualification.
- One Task execution slice with real isolation, mediated Effect and reconciliation.
- Real Workbench Terminal, Browser and Computer providers; remove fake success/sample diffs.
- LiteSPM bridge and Connection lifecycle integration.
- Coworker-scoped Conversation, memory and responsibility execution.
- Professional Artifact round-trip and Intelligent UI acceptance.
- Reconcile SettingsView's four active categories against the 16-category/187-control target; implement or remove inert appearance selectors and shortcut labels, expose or remove unrendered pending/error/conflict recovery props, correct the Localhost transport label, and verify every visible action against its owner service.
- Qualify Windows WorkspaceRoot creation and scan behavior (the route regression tests pass, but Add remains unavailable and scanning is unsupported); finish frontend, packaged desktop, OS/provider and USER E2E.

See IMPLEMENTATION-STATUS.md for evidence and ACCEPTANCE-AND-E2E-PLAN.md for release gates.


## 7. Cross-cutting concern-to-owner map

| Concern | Normative owner set | Current coverage and implementation boundary |
|---|---|---|
| Domain schema, identity and lifecycle | DATA-MODEL.md, STATE-MACHINES.md, SCHEMAS.md, `schemas/*.json`, `schemas/sqlite-v*.sql` | Broad target model and machine contracts exist. A field or enum in a schema is not evidence that every producer, validator, route and UI consumer accepts it; use the source-backed crosswalks in IMPLEMENTATION-STATUS.md and the owning domain docs. |
| API and authenticated transport | API.md, `schemas/operator-api.openapi.yaml`, PROTOCOLS.md, LOCAL-OPERATOR-IPC.md | 229 OpenAPI operations are defined; 173 are registered by the composed Operator, one local-only route is outside OpenAPI and 56 operations are unmounted. Registration is not service/provider/UI qualification. |
| Events, projections and integration | EVENTS.md, `schemas/domain-event.schema.json`, SERVICES.md, PRESENTATION-RUNTIME.md, FAILURE-RECOVERY.md | 148 versioned event types are schema-aligned. Domain events, Operator stream frames and UI reducer actions are distinct. Each event needs a real producer/consumer and recovery path before its UI is treated as live. |
| Flows, races and failure recovery | FLOWS.md, V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md, FAILURE-RECOVERY.md and domain owners | FL01–FL25 and race classes define acceptance targets. Only the exact tests listed in IMPLEMENTATION-STATUS.md are evidence; structural or unit passes do not qualify full journeys. |
| HLD, ownership and rationale | ../ARCHITECTURE.md, COVERAGE-MATRIX.md, `adr/`, domain owner contracts | Architecture defines boundaries and ownership; ADRs explain accepted rationale. An implementation or product proposal cannot silently create a second authority. |
| Requirements and feature acceptance | SPECIFICATION-STANDARD.md, `specs/001-*` through `specs/011-*`, V1-PRODUCT-RELEASE-CONTRACT.md | All 11 feature packages exist; Feature 001 is IN_REVIEW and Features 002–011 are DRAFT. Package structure is not owner acceptance. |
| UI behavior and typed actions | V1-UI-SCREEN-CONTRACT.md, V1-UI-ACTION-STATE-LEDGER.md, DESKTOP-SHELL-LLD.md, EXPERIENCE.md, NATIVE-COMPOSER-LLD.md, each feature `contracts/ui-behavior.md` | Screen and action targets are mapped to actual mounted components in SEMANTIC-SERVED-UI-MAP.md. Every visible control must be checked against its actual typed command and owner route; target-only controls stay visibly unavailable. |
| Tests and release evidence | TESTING.md, ACCEPTANCE-AND-E2E-PLAN.md, BENCHMARKS.md, feature TASKS.md and EARS | Rust workspace, focused frontend and structural checks have recorded results. Full serial frontend, packaged Tauri, cross-OS, provider and owner USER journeys remain explicit qualification gates. |
| Market and reference research | COMPETITIVE-RESEARCH.md, REFERENCE-IMPLEMENTATIONS.md, pinned study docs | Research is evidence for design choices, not product requirements or implementation authority. Verify current vendor versions and licenses before adopting a pattern. |
| Prototype-to-production promotion | IMPLEMENTATION-MASTER-PLAN.md, IMPLEMENTATION.md, SPECIFICATION-STANDARD.md, AGENTS.md | Spikes remain isolated. Promotion requires owner decision, feature SDD, machine contract, source wiring, UI crosswalk, tests and evidence in one reviewed change. |

## 8. Required feature-package and UI contract checklist

Every material feature package must contain the artifacts required by SPECIFICATION-STANDARD.md and `.specify/templates/`: README, SPEC, PLAN, TASKS, DATA-MODEL, RESEARCH, WIRING and the feature contracts. The minimum cross-layer evidence is:

1. `SPEC.md`: problem, goals/non-goals, HLD/LLD, authority, security, errors/recovery, EARS acceptance, dependencies and named open decisions.
2. `DATA-MODEL.md`: identities, records, lifecycle, persistence, event ownership, retention and migration links.
3. `contracts/api-events.md`: exact operationIds, request/response semantics, authorization, events produced/consumed and target-versus-served status.
4. `contracts/errors-recovery.md`: typed errors, retries, conflicts, ambiguous outcomes, cancellation, restart and recovery behavior.
5. `contracts/ui-behavior.md`: exact screens/controls, state transitions, validation, typed command mapping, loading/empty/stale/offline/denied/unavailable/pending/success/error states and recovery.
6. `TASKS.md`: stable reviewable tasks, dependencies, file scope, tests and evidence paths.
7. `RESEARCH.md`: pinned versions/SHAs, primary sources, alternatives, license/security notes and unresolved questions.
8. `WIRING.yaml`: components, owners, calls, events, transactions, fences and recovery edges.

UI behavior rules:

- The UI renders authoritative projections and sends typed commands; it does not own durable domain truth.
- Every domain mutation maps to one canonical operation or an explicitly documented local-only action that does not mutate domain state.
- Async surfaces define empty, loading, error, partial, offline and long-running behavior plus domain-specific stale, ambiguous, denied and needs-auth states.
- Errors use canonical typed codes and recovery guidance; do not parse English strings.
- Link each control to the owning action-ledger and feature UI contract. A route without a control is not a complete user journey; a button without a real owner operation is not an implementation.

## 9. Research, prototype and promotion workflow

1. Research primary vendor documentation and pinned source revisions; record exact version, date, license, security boundary and alternatives in the owning RESEARCH document.
2. Borrow user-visible behavior only when it fits LiteCowork's local-first, chat-first, Task/Trust/Effect and native-harness ownership boundaries. Do not copy source without license review.
3. Prototype novel or conflicting behavior in an isolated worktree; keep it out of the active product path until the owner decision and SDD are accepted.
4. Promote through the canonical domain owner, machine schemas, API/event/error contracts, WIRING, UI screen/action map, tests, traceability and release evidence together.

## 10. Canonical UI-to-Core wiring

```text
User outcome
  -> mounted screen and exact control
  -> typed React/Tauri command
  -> OS-authenticated local IPC
  -> composed Operator route or explicitly local native action
  -> authorization and Workspace fence
  -> owning service and transaction/version check
  -> persisted aggregate, event and idempotency receipt
  -> projection/stream and UI reconciliation
  -> restart, timeout, denial and ambiguous-result recovery
  -> CODE/SYSTEM/owner USER acceptance evidence
```

| Direction | Canonical contract | Owner |
|---|---|---|
| UI -> Core | OpenAPI operationId, RequestId, If-Match, idempotency key and typed request/response | API.md, LOCAL-OPERATOR-IPC.md, `schemas/operator-api.openapi.yaml` |
| Core -> UI durable state | Read-model projections and versioned domain events; raw domain events are not sent directly to WebView | SERVICES.md, EVENTS.md, PRESENTATION-RUNTIME.md |
| Core -> UI transient state | Typed OperatorStreamFrame with explicit correlation, ordering and replay behavior | PRESENTATION-RUNTIME.md, `schemas/operator-stream.schema.json` |
| Failure -> UI | Canonical error code, retryability, request identity and owner recovery action | SCHEMAS.md, FAILURE-RECOVERY.md, feature `contracts/errors-recovery.md` |

## 11. Related indexes

- V1 design index: V1-DESIGN-INDEX.md
- Feature SDD register: V1-FEATURE-SDD-REGISTER.md
- Spec-task traceability: SPEC-TASK-TRACEABILITY.md
- Semantic served UI map: SEMANTIC-SERVED-UI-MAP.md
- Current implementation status: IMPLEMENTATION-STATUS.md
- Acceptance and E2E plan: ACCEPTANCE-AND-E2E-PLAN.md
