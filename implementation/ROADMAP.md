# Delivery roadmap

The dependency graph in [backlog](backlog.json) is authoritative for ready work.
No invented calendar schedule; one-week planning/review windows follow [process](PROCESS.md).

| Epic | Increment and gate | Dependencies | First owner demo |
|---|---|---|---|
| [E01](epics/E01.md) | Development foundation / G0 | none | Use desktop client and adversarial HTTP client against same daemon. |
| [E02](epics/E02.md) | Desktop shell and resources / G1 | E01 | Create two blockers, resolve one, replay event and observe unchanged inbox. |
| [E03](epics/E03.md) | Full native agent and durable work / G1 | E02 | Run native-feature suite outside and inside host and report actual differences. |
| [E04](epics/E04.md) | Trust, capabilities and effects / G1 | E03 | Produce diff/report, independently check it and show verified versus reported status. |
| [E05](epics/E05.md) | Heterogeneous workforce and cost / G2 | E04 | Run same code task corpus with baseline and delegated policy. |
| [E06](epics/E06.md) | Documents, RAG and local models / G2 | E04 | Drag a ZIP, inspect readiness and citation, attempt disallowed cloud task. |
| [E07](epics/E07.md) | Environments, browser and office work / G2 | E05, E06 | Operate permitted local app with keyboard takeover and recorded real activity. |
| [E08](epics/E08.md) | Coworker, goals, context and product UX / G2 | E05, E06 | Owner completes task/approval/takeover solely by keyboard and reduced motion. |
| [E09](epics/E09.md) | Responsibilities, scheduling and notifications / G2 | E07, E08 | Complete Task with app closed; reopen correct Artifact from notification. |
| [E10](epics/E10.md) | Reusable skills and capability UI / G2 | E09 | Teach sandbox browser task, review/test draft, approve package publication when qualified. |
| [E13](epics/E13.md) | Desktop/local V1 production release / G3 | E10 | Install/update/uninstall on supported desktop OS and execute incident drill. |
| [E11](epics/E11.md) | Post-V1 cloud continuation and messaging | E13 | Close laptop for cloud-eligible work; local-app Task explicitly waits. |
| [E12](epics/E12.md) | Post-V1 Remote Runtime | E11 | Execute code task remotely, disconnect mid-run and reconcile on reconnect. |

## First implementation cycle

Start with E01-S01: pin Rust/Python development tools, build the minimal `litecoworkd`
help/version executable, and make the local/CI contract gate reproducible. This is a
build foundation, not an operational daemon. Then implement E01-S02 storage while running
SP02, E01-S03 lifecycle, and E01-S04 authenticated transport while running SP03. Run SP01
with E02-S01 before committing to the desktop shell; run SP04 alongside E03-S01 before
selecting the first native adapter. G1 still requires a complete verified local task, not
a successful shell page or a set of mocks.

E05–E10 complete local workforce, RAG, local models, browser/office, persistent identity,
context, responsibility, rich outputs and reusable skills. Early Coworker naming can be
introduced in E02 as UI preparation, but E08 owns durable identity/allowlist semantics.
E06 is independent of E05 after E04; choose according to owner review bandwidth.

E08 is intentionally split into reviewable UI increments: E08-S04 establishes truthful
Work/activity projections; E08-S06 adds typed PresentationItems and reconnect-safe
transient streaming; E08-S07 delivers the Artifact Workbench and immutable version
history; E08-S05 validates accessibility and nontechnical/technical end-to-end journeys
across those surfaces. Context intake and source readiness remain E06-S05; context
editing/revocation remains E08-S03. E08-S08 completes semantic-first optional
RichPresentation, bounded Host Guidance, safe presentation compilation, and exact-version
deliverable rendering after the semantic Conversation path and Artifact bindings exist.
These are staged implementations of the finalized desktop/local V1 feature set, not scope
cuts. E08-S08 does not pull Cloud continuation or Remote Runtime into the V1 critical path.

E11 and E12 are post-V1: cloud deployment/pairing/handoff/channel, then the same Runtime
on a remote machine. E13 closes desktop/local V1 production evidence and release before
either post-V1 program is scheduled. Tests, local backup primitives, diagnostics and
security are developed continuously; E13 is qualification/soak, not the first time they
are implemented. Every earlier local story includes its actual failure paths and
operational requirements.

## Scope-preserving decomposition

A story can contain multiple implementation tasks. Split into IDs such as E06-S02-A,
retain parent acceptance, add explicit dependency edges in backlog, and update validator
support if a new ID shape is introduced. Do not call an epic done merely because its
first representative screen exists. All finalized APIs/transitions and assigned scenarios
must be accounted for by the epic's exit. Integrated provider breadth remains an explicit
qualification matrix; a feature advertised unavailable cannot meet a required release gate.

## Reconciled Coworker implementation sequence

E08-S01/04 first establish optional chat-first ownership with real Conversation creation/list/last-active and honest provider-disabled states, Quick Create, and preserved Workbench context. E04/E10 enable qualified connection assignments; E08-S03 gains deterministic scoped memory candidate eligibility and qualified extraction, independently of proactive actions. E09 then enables reviewed standing responsibilities, trigger outbox/recovery, background service, missing-connection/Needs You and run history using the existing Task engine. This target modifies feature acceptance, not the actual dependency-ready status; backlog and machine-level contract changes require corresponding verified story updates.


## Second-pass Coworker/readiness audit — 2026-10-10

This table distinguishes target specification, code presence, and actual qualification. **BOUNDED** means a real tested subcomponent exists; it does not mean the end-user feature is done. No row becomes DONE without its owning CODE + SYSTEM + USER evidence.

| Capability | Current source evidence | Status | Next owner |
|---|---|---|---|
| Ordinary Conversation list/read | Conversation catalog/presentation exists; composer is read-only | PARTIAL | E03-S02 / E08-S04 |
| Provider-backed chat send | No qualified desktop native turn admission | NOT DONE | E03-S01/S02 |
| Coworker CRUD/revision/status | Real API/UI/storage slices | PARTIAL | E08-S01 acceptance |
| Coworker-owned Conversation + last-active | No Conversation-owner field/service/route; unrelated environment/activity fields do not satisfy it | NOT DONE | E08-S01 |
| Quick Create then chat | Creation editor simplified; still settings-oriented | PARTIAL | E08-S01 |
| Coworker capability assignment | Capability fabric exists; no CoworkerCapabilityAssignment type/schema/source | NOT DONE | E10/E04 |
| Large lazy connection catalog | Discovery/activation contracts exist; consumer catalog not qualified | PARTIAL | E10-S01 |
| Task-time Connect + resume | UserRequest/capability pieces exist; no full Coworker repair flow | NOT DONE | E04/E10 |
| Manual ContextDocuments | Resource-backed editor/history slice exists | PARTIAL | E08-S03 |
| Automatic quiet private memory | No MemoryCandidate or CoworkerMemoryPolicy implementation found | NOT DONE | E08-S03 |
| StandingResponsibility group | No storage/API/domain type found | NOT DONE | E09-S01/S02 |
| Manual Automation occurrence | Real manual occurrence admission slice | BOUNDED | E09-S02 |
| Scheduled/provider/event TriggerHost | Only manual pinned local TriggerHost path observed | NOT DONE | E09-S02 |
| Proactive deterministic watch/no-change | Architecture only | NOT DONE | E09-S02/provider |
| Background daemon/window independence | Runtime lifecycle foundations; no qualified unattended Coworker trigger host | PARTIAL | E01/E09 |
| Trust evaluator | Bounded evaluator committed/tested | BOUNDED | E04 atomic admission |
| Capability invocation persistence | Durable transitions/storage tests; dispatch deliberately gated | BOUNDED | E04 dispatch |
| Attempt process-scope journal | Journal and regression tests committed | BOUNDED | E07 Task process isolation |
| Needs You/approval | Inbox/request foundations exist | PARTIAL | E02/E04 end-to-end |
| Artifact Workbench | Real history/edit/compare/Save As slices | PARTIAL | E08-S07 |
| Unified right Workbench shell + Start | No shell/Start component; current Artifact Workbench is embedded | NOT DONE | E08-S05/S07 |
| Built-in Browser panel | No final unified Workbench browser surface qualified | NOT DONE | E07/E08 |
| Computer/takeover panel | Control-lease architecture groundwork; no final UI qualification | NOT DONE | E07/E08 |
| Terminal panel | No final unified shell integration | NOT DONE | E07/E08 |
| Notifications/quiet hours | Story planned; no complete delivery path | NOT DONE | E09-S04 |
| Coworker-to-Coworker handoff | No dedicated scoped handoff implementation | NOT DONE | E05/E08 |
| Cloud continuation | Explicitly future; local-only desktop target | OUT OF DESKTOP V1 | Future placement |

A grep hit is not implementation proof. In particular, owner_coworker_id currently exists for Environment ownership and does NOT establish Coworker-owned Conversations. Likewise the local TriggerHost identity used by manual occurrences is not a recurring/event trigger host.


## Agent-module implementation sequence

E03 now has a second internal sequence after the existing bounded native-agent/session
foundation:

1. **E03-S06** — AgentModuleRegistry + live/cached ACP Registry discovery.
2. **E03-S07** — install/update/repair lifecycle and Runtime-local observations.
3. **E03-S08** — AgentControlDescriptor + versioned AgentBinding configuration/defaults.
4. **E03-S09** — native authentication, secure credential slots and native-config launch.
5. **E03-S10** — agent-adaptive composer with dependent Model/session options and native
   slash/reference/input surfaces.
6. **E03-S11** — per-agent/version/OS conformance packs against the original harness.
7. **E04-S06** — cross-agent LiteSPM capability bridges after both AgentModule and
   Capability Fabric contracts are available.

This sequence is intentionally incremental. Do not delete the current Codex/OpenCode
probe paths at S06; migrate them behind AgentModules and remove brand-specific UI/Tauri
branches only after equivalent behavior has CODE + SYSTEM evidence.

The detailed current-source refactor map is in
[AGENT-MODULE-AUDIT.md](AGENT-MODULE-AUDIT.md).


## Connector/workflow/learning implementation refinement — 2026-10-10

No new control plane is introduced. Accepted designs map into existing epics:

1. **E04-S01/S02/S03/S06:** SecretStore, ConnectorPackage qualification, AuthAttempt/
   ConnectionService, progressive discovery, connector Invocation/Effect reconciliation
   and cross-agent LiteSPM exposure.
2. **E08-S03:** LearningCandidate/CoworkerLearningPolicy/private memory
   conflict/revocation. Instruction changes remain Coworker revisions.
3. **E09-S02/S03/S04:** real schedule/provider trigger hosts, connector event subscription,
   optional EventRelay, honest missed-run/history and notifications.
4. **E03-S02 + E09-S02:** idempotent Task follow-up Conversation for Automation Continue,
   with native-resume optimization only after AgentModule qualification.
5. **E10-S01/S02:** connector/skill package UX plus Activepieces community-Piece
   compatibility spike and package-security fixtures.
6. **E04/E09:** external WorkflowBackendAdapter; qualify one user-owned backend end to end
   before broad provider claims.

Suggested dependency order:

~~~text
SecretStore/Connection schema
 -> one read-only direct connector
 -> one reconciled write
 -> provider trigger + scheduled occurrence
 -> first-party app bundle
 -> Task follow-up Conversation
 -> EventRelay spike
 -> external WorkflowBackend
 -> Activepieces Piece compatibility
 -> optional hosted auth/tool backends
 -> broader catalog qualification

Parallel after Coworker ownership:
LearningCandidate -> private memory -> instruction diff review -> Skill/Routine proposals
~~~

Do not mark any row complete from architecture work alone. Each machine promotion updates
migrations/OpenAPI/events/coverage and passes CODE/SYSTEM/USER evidence.

## V1 detailed design integration — 2026-10-12

Prioritize the critical path: E03 native conversation send + agent control/composer -> E08 unified right Workbench/semantic rendering -> E07 qualified Chrome/Computer provider and takeover -> E04/E05/E09/E10 integrated delegated workflows -> E13 actual OS/user release qualification. Existing dependencies govern actual scheduling; parallel independent Rust/UI/adapter fixture lanes after typed contracts freeze (AGENTS.md). See V1-DESIGN-COVERAGE.md for UAT IDs.

## 2026-10-13 critical-path gate reconciliation

Do not add a second roadmap or change story dependencies. Before claiming G1, E03-S01/S02 must prove a real provider-backed Send/Stop/reconnect; E07-S00 must qualify a Runtime-owned planner isolation mechanism, then E03-S03 must activate a producer-authenticated planner and atomically accept its versioned plan. E01-S04 owns authenticated transport/ErrorCode correctness. G2 additionally requires E08-S03's scoped private learner and E07/E08's qualified browser/terminal/shell surfaces; Windows root identity and restart behavior remain fail-closed until E02-S03/E07-S00 real Windows tests pass. G3's E13-S03/S04 release matrix must record remaining unsupported OS/provider cells rather than silently accept them. Full existing-story trace and tests: [V1 contract reconciliation](V1-CONTRACT-RECONCILIATION-2026-10-13.md).
