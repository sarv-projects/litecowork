
# V1 design, implementation and UI index

This index connects the complete V1 target to the active implementation and its user-visible surfaces. The target is defined by V1-PRODUCT-RELEASE-CONTRACT.md and its domain owners. Current implementation status is defined by IMPLEMENTATION-STATUS.md. This index is a navigation and coverage artifact; it does not create new domain semantics.

## Product areas and current delivery state

| V1 area | Normative target | Current source-backed state | Primary user interface | Feature SDD |
|---|---|---|---|---|
| 1. Universal Chat | PRODUCT.md, EXPERIENCE.md, NATIVE-COMPOSER-LLD.md, STATE-MACHINES.md | Conversation CRUD/pagination and OpenCode-only submit exist; Send is enabled despite target disabled-until-qualified gate; no strict single-flight/outbox/restart recovery; Stop ineffective, retry UI absent, attachment/voice/more-options inert, empty state uses Antigravity placeholder | New Chat, Recent Chats, Load more, composer, agent picker, message/history, Stop/Retry and static empty state | 001 |
| 2. Bring Your Own Agent | AGENT-FABRIC.md, AGENT-CONTROL.md, AGENT-ACCOUNT-ROUTE-MATRIX.md, V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md, DELEGATION.md | 15 curated/ACP metadata and bounded version probes; auth UNKNOWN/readiness NOT_PROBED. Codex/OpenCode probes only; OpenCode incompatible, Codex not dispatched. No install/native sign-in; descriptor/credential routes 503; enable misses fresh AC-004 descriptor gate; no Usage projection/model picker | Agent catalogue, bounded probes, Workspace bindings/default lead, subagent profile/history/performance/quota; install/auth/model/Usage target-only | 002; Delegation owner contracts |
| 3. Persistent Coworkers | COWORKERS-TARGET.md, COWORKER-FLOWS.md, LEARNING.md, RESPONSIBILITIES.md | Coworker definition/revision/status/primary persist; editor saves lead/delegation/profile/interaction/failover/context/notification preferences with incomplete consumers; pause gating is partial and Task composer blocks paused Coworkers; Quick Create does not open a chat; Conversation has no Coworker origin; context policy is not enforced; ContextDocument owner validator rejects Coworker/Goal kinds; learning gates are not a LearningService; presence returns 503 | Roster/editor/status/primary and Task-origin selection are served; chat, Coworker Memory browser, scoped retrieval, Learning Inbox, true proactive pause and full responsibility enforcement remain target UI/behavior | 009 |
| 4. Real Computer and Workbench | DESKTOP-SHELL-LLD.md, ENVIRONMENTS.md, CHROME-BROWSER-LLD.md, V1-INTEGRATED-TERMINAL-LLD.md | Files Resource projection/preview; Browser iframe only; Terminal fabricates success and ConPTY/Job Object claims without process; Diffs synthetic; Activity not rendered; Computer provider unavailable. No browser/terminal/computer route/provider | Workbench Files, embedded preview, simulated Terminal, synthetic Diffs; no Activity or Computer surface | 008 |
| 5. Capabilities, Plugins, MCP and Connections | CAPABILITY-FABRIC.md, CAPABILITY-INVOCATIONS.md, LITESPM-LITECOWORK-INTEROP-LLD.md | Discover projection; connect/test fail closed; Approval resolve real; narrow Resource-read grant and read-only Invocation history; no account Connection/OAuth, provider dispatch or Effect reconciliation | Discover Explore/Installed/Managed/Connections; Needs You ApprovalDetail; no connected account/read/write lifecycle | 006 |
| 6. Real Tasks and Multi-Agent Work | TASK-RUNTIME.md, ENVIRONMENTS.md, TRUST.md, DELEGATION.md | Task records/spec revisions/readiness exist; plan/Attempt dispatch, mediated effects and worker orchestration unavailable | Work list, Task detail, planning-readiness, future Plan/Activity/Workers | 005 |
| 7. Routines and Automations | ROUTINES.md, AUTOMATION.md, RESPONSIBILITIES.md, CONNECTORS-WORKFLOWS.md | Local 30-second TriggerHost settles supported schedules and materializes READY Tasks only. Occurrence history UI drops next_cursor; retry/wake policies have no consumer. Notification records remain PENDING with no sender; channel preferences are ignored, ON_CONDITION is only DEPENDENCY_BLOCKED and quiet hours are not editable. External WorkflowBackendAdapter is target-only; no Core graph runtime/canvas | Automations/templates/editor/health/first-page Runs; Routine editor/run; Notifications history/preferences | 010 |
| 8. Memory, Knowledge and Learning | CONTEXT.md, LEARNING.md, WORLD-RESOURCES.md | ResourceService accepts Personal Profile/Workspace Notes; desktop Library creates Workspace Notes only. Workspace/Resource lexical search and grounded-retrieval endpoint exist, but retrieval is read-only and not wired to Conversation; no ContextAttachment/Context Used surface. Coworker/Goal ContextDocument create is rejected; Coworker retrieval policy is not wired; no LearningService or Coworker-private memory persistence. WorkspaceRoot scan is metadata-only; Windows add/scan unavailable; ZIPs remain opaque and unindexed | Resource picker/Library, Workspace Notes, lexical search are served; Context Used, Coworker Memory/Knowledge, review/revoke UI and learning inbox remain target | 003, 009 |
| 9. Library and Finished Work | ARTIFACTS-EVIDENCE.md, PRESENTATION-RUNTIME.md | Immutable Artifact reads, bounded text/plain append, promote/archive, safe preview and exact-version local Save As; no create route/UI, professional engine or ExportReceipt; Office files are Resources, Task outputs have no producer | Saved/Task outputs/Archived Library and ArtifactWorkbench; no create/import-convert | 007 |
| 10. Desktop Shell, Settings and Runtime | RUNTIME-LIFECYCLE.md, LOCAL-OPERATOR-IPC.md, DESKTOP-SHELL-LLD.md | Tauri shell, daemon and OS-authenticated local Operator IPC exist; mounted SettingsView exposes four categories only, with exact Workspace/Agent/Runtime operations mapped in SETTINGS-AND-SHELL-SURFACE-AUDIT; 16-category IA/187 controls remain target-only; release not qualified | Sidebar, Workspace selector, Settings (General/Workspaces/Agents/Runtime), runtime diagnostics, right Workbench | 011 |

## Vertical journey crosswalk

| Journey | UI entry | Backend/service path | Durable truth | Release gate |
|---|---|---|---|---|
| Plain Conversation | New Chat → composer → Send | Tauri command → authenticated Operator → Conversation service → native supervisor | User message, turn, receipt, agent message and terminal settlement | Real native agent, stream/reconnect, stop/retry, crash recovery |
| Workspace context | Resource picker/Library → attach | Resource upload/index/search and authorization | exact Resource/revision/hash/scope | Format parser, path boundary, stale/revoke, source-grounded prompt |
| Coworker setup | Coworkers → Create/Edit | Coworker Operator/service and revision store | Coworker identity/revision/assignment | first chat, scoped memory, authority boundaries and restart |
| Durable Task | Conversation action → Run as Task | Task service → plan/attempt admission → Environment/Trust/Effect | Task spec/plan/attempt/effect/evidence | complete one safe Task, not just create a record |
| Scheduled work | Automations → preview → enable | Automation revision + TriggerHost + occurrence materialization | exact occurrence and Task link | host sleep/offline/misfire/restart and notification behavior |
| Capability use | Discover → LiteSPM plan → authorize | LiteSPM bridge + capability gateway + grant/effect | package operation receipt and invocation receipt | pinned bridge, compatibility, approval, idempotency, rollback |
| Workbench action | Workbench view | provider-specific environment/session | actual process/browser/diff/artifact identity | real provider, policy, lease, cancellation, recovery |
| Artifact delivery | Library/Task output | artifact service + format handler | committed artifact revision, hash and provenance | one complete format, reopen/export/compare |

## Cross-cutting design boundaries

### Agent architecture

BYOA is the primary model. Native agent harnesses own their reasoning, authentication, prompt/context strategy, private session and native tools. LiteCowork owns Workspace/Conversation/Task truth, cross-agent authorization, presentation and recovery. A direct provider-key agent is a separate optional execution model that requires LiteCowork to own a full model/tool loop; adding a credential field alone is not an implementation.

### Trust and delegation

Tool descriptions and MCP affordances tell an agent what may be requested; they do not authorize it. Core Trust must evaluate trusted server-side context at every mediated execution boundary. A Gateway cannot govern native shell/filesystem/subagent actions that bypass it. A delegate request is a proposal; a child Attempt requires an accepted parent Task/Plan Step, eligible worker, bounded scope, lease, Environment and admission.

### LiteSPM

LiteSPM is the single cross-agent package lifecycle owner. Discovery level, compatibility, host installation, native Connection, Coworker assignment, Task grant, capability invocation and verified effect are separate. Do not create a second package manager or an always-running LiteSPM daemon without a qualified upstream requirement.

### External workflows

Workflow graphs remain external/provider-owned in V1. The target is for a Routine to pin an existing provider workflow reference and input/output/verification requirements, then invoke it as one CapabilityInvocation inside an admitted Task once a qualified WorkflowBackendAdapter exists. No production adapter is currently present. Provider node/run state remains external observation, not LiteCowork Task Step state. The OpenBot source study is a UX/architecture reference, not a decision to add an in-Core graph canvas/runtime. See CONNECTORS-WORKFLOWS.md and OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md.

### Cloud and mobile

Cloud continuation is V2 Task rehydration: portable Task specification and committed checkpoints, effect reconciliation, Hub-owned lease fencing, a new Attempt and a new Runtime/Environment. A live native process/session does not teleport. Local resources, OS credentials and unsupported tools remain placement blockers. Remote mobile control is a separate authenticated remote gateway integration; it is not implied by a generic MCP server or a native mobile app.

### Intelligent UI and Mission Control

Typed declarative UI is validated against a bounded schema and rendered from registered components. No arbitrary code execution in the chat surface. Interactive app artifacts need isolated execution. Mission Control is a view over real Task/Step/Attempt/Effect/Artifact/Evidence records inside Work/Task detail, not a decorative graph or second orchestration engine.

## Active machine and UI contracts

- Machine route state: schemas/served-route-inventory.json
- UI action/state IDs: V1-UI-ACTION-STATE-LEDGER.md
- Settings operations: V1-SETTINGS-CONTRACT-LEDGER.md
- Function owner contracts: V1-FUNCTION-CONTRACT-LEDGER.md
- Feature packages: ../specs/README.md
- Screen contract: V1-UI-SCREEN-CONTRACT.md
- End-to-end release gates: ACCEPTANCE-AND-E2E-PLAN.md

No design-depth, route-count or SDD-structure report replaces exact integration and USER evidence.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## Pinned reference study

- [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md): OpenBot's model picker, Tasks page, Usage analytics and visual/agent-edited workflow graph. The current LiteCowork adoption boundary is provider-owned external workflows; no Core graph runtime is part of V1.
