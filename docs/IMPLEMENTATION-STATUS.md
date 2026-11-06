[Reading 295 lines from start (total: 295 lines, 0 remaining)]

[Reading 279 lines from start (total: 279 lines, 0 remaining)]

[Reading 262 lines from start (total: 262 lines, 0 remaining)]


# Implementation and release status

This document records the behavior present in the active source tree and the evidence available for this checkout. It is the implementation-status authority paired with the normative target in [Product](PRODUCT.md), [V1 product release contract](V1-PRODUCT-RELEASE-CONTRACT.md), [Architecture](../ARCHITECTURE.md), and the owner contracts.

It does not redefine product requirements. Source code establishes what is implemented and wired; route composition establishes what is mounted; provider and UI tests establish only the behavior they actually exercise; release qualification requires exact platform, build, agent/provider, and owner evidence.

## Product and release posture

**Release posture: pre-release development. V1 is not release-qualified.**

The active checkout contains the Rust workspace, the litecoworkd daemon, the Tauri/React desktop application, storage and domain crates, machine-readable contracts, and feature specifications. It is not a documentation-only checkout.

The product target remains a local-first, chat-first desktop workspace. An ordinary Conversation does not require a Coworker or Task. Coworkers are persistent configuration/context identities. Tasks are durable records for admitted outcome-oriented execution. These concepts must remain separate in both the data model and interface.

No claim of general production readiness is supported by the evidence recorded here. In particular, a healthy Runtime, a mounted Operator route, a passing unit test, a profile probe, or a UI component is not proof of a complete user journey.

## Evidence vocabulary

Use one status vocabulary across this document, ledgers, feature specifications and release reviews.

| Status | Meaning | Does not prove |
|---|---|---|
| TARGET | Normative behavior is specified | Any implementation |
| CODE | Source code exists and is compiled or unit-exercised | Runtime route, provider or UI wiring |
| SERVED | The operation is mounted by the composed authenticated local Operator and reaches its owning service/storage path | Native provider integration, UI usability or end-to-end qualification |
| INTEGRATED | A real provider/harness and persisted product path work together | All OS/provider failure and recovery behavior |
| QUALIFIED | The exact build, OS, agent/provider version and auth route pass CODE, SYSTEM and owner USER evidence, including denial, timeout, restart and recovery | Untested combinations |

These are progressive evidence states, not marketing badges. A capability may have different evidence states for different agents, OS versions, formats or operations.

## Runtime and Operator readiness

RuntimeState.READY means daemon startup and the required startup/recovery checks completed without a blocking lifecycle error. DEGRADED means one or more lifecycle blockers remain. The local Operator exposes authenticated IPC and the application uses it for local domain operations.

operatorReady and /v1/operator/readiness describe local Operator availability. They do **not** mean Task planning, Attempt admission, mediated Effects, browser control, computer use or safe command execution are available.

The desktop App bootstrap checks Runtime process status and attempts `start_local_runtime` only when no process is running and the daemon executable is available, then refreshes Runtime and Workspace projections. Linux Tauri startup integrates a marked per-user `systemd --user` unit; Windows/macOS startup currently launches the daemon directly and does not register a Scheduled Task/LaunchAgent or implement service-manager install/update/uninstall. Windows named-pipe IPC and the isolated Windows security crate are source-implemented; the Windows workspace run passed 36 `windows-local-security` and 18 `operator-ipc` tests, but packaged cross-user peer-authentication, secret-buffer and full USER qualification remain open. Linux/macOS Unix IPC and lifecycle-file source paths are not platform-qualified by that Windows test run. See RUNTIME-LIFECYCLE.md and LOCAL-OPERATOR-IPC.md.

The Task planning-readiness operation is explicit about the current boundary: dispatch_available is false, planning_started is false, agent_session_started is false and plan_created is false. Task creation persists an ordinary Task record; editing its objective/specification persists revisions. Neither action starts planning or an agent.

The CLI help text now states the actual boundary: a configured native Conversation adapter may dispatch a normal Conversation turn; Task planning, Attempt admission, mediated Effect execution and completion verification remain unavailable. Runtime READY reports startup/recovery health only, and the Runtime is DEGRADED only when a startup or recovery blocker is present. The CLI help contract has a regression test in apps/litecoworkd/src/main.rs. Runtime lifecycle health and Task execution readiness are separate facts.

## Served Operator surface

The composed Operator router is built in apps/litecoworkd/src/operator.rs and merges the domain routers used by the daemon. The source-backed route inventory is generated at [served-route-inventory.json](schemas/served-route-inventory.json).

The inventory currently records:

- 173 OpenAPI operations registered by the composed Operator router.
- One authenticated local-only workspace-root selection route outside OpenAPI.
- 56 OpenAPI operations that remain target-only and are not registered by the daemon router.

This is a route-registration inventory, not a claim that 173 product capabilities are fully integrated. Individual handlers may deliberately fail closed, return a read-only projection, expose a readiness check, or persist only a pre-execution record. A route count must never be substituted for a qualified-feature count.

The target-only operations include important Task execution/control and placement surfaces, runtime lifecycle management, remote/runtime enrollment operations, connections, some channel operations, workspace root mutation and backup/restore. Consult the machine inventory for the exact method, path and operation ID instead of duplicating those lists here.

## Current capability findings

### Conversation and native agents

The desktop has real cursor-paginated Recent Chats (including a parent-sidebar Load more chats action with Workspace/list-epoch fencing and conversation-ID deduplication), Conversation create, snapshot/message pagination, drafts, agent-binding selection and a Send route. Despite the normative disabled-until-qualified gate, the visible Send button is enabled for a non-empty draft whenever the local isTurnRunning predicate is false; it does not require a qualified AgentBinding/provider route or Operator readiness and can submit without a Workspace default binding, relying on the backend error. The visible Stop control is not an effective OpenCode-turn cancellation path. If the pending-create journal cannot be read, New Chat is paused and the user can explicitly retry the journal read before creating another Conversation. The Operator persists a submitted user message and OPEN turn with an idempotency receipt before dispatch is attempted. The supervisor has a real OpenCode server/session/turn branch pinned to the OpenCode v2.0.32 protocol path, collects the response and commits the agent response/turn settlement.

The current dispatch implementation has one real execution branch for provider key opencode. Other provider keys fail closed as unsupported or unconfigured for execution. Agent discovery, executable probes, profile descriptors and binding management are not equivalent to a working native turn adapter. Do not advertise Codex, Claude Code, Gemini CLI, Cline or another agent as turn-capable until its exact adapter path is implemented and qualified.

The current OpenCode implementation is code-level integration evidence, not a qualified Windows desktop journey. It starts a local OpenCode server/session for the dispatched turn and shuts down the locally owned server when that turn finishes; this is not proof of full native session persistence or seamless private-session migration between agents.

Several Conversation lifecycle gaps remain release blockers. Submission commits durable OPEN state before detached asynchronous dispatch; there is no durable outbox/claim or startup sweep, and exact receipt replay does not prove a worker is alive or re-admit missing work. The submit transaction also does not enforce the accepted one-active-turn-per-Conversation rule (the stale-snapshot append test admits two distinct OPEN turns). Retry advances FAILED to RUNNING with a new session pointer before the STARTING reservation that requires OPEN/no session pointer, so retry dispatch is inconsistent. RUNNING cancellation commits CANCELLED without a native interrupt or observed quiescence. Errors before the supervisor's inner execution handler can be logged by the detached Operator caller while leaving an OPEN turn unsettled. The desktop UI adds separate stop/retry defects: its active-turn projection omits aggregate version, Stop searches only for transient stream turns (which the current OpenCode Operator path does not emit), its cancel call hardcodes If-Match version 1, and no retry API call or failed-turn retry affordance exists. OPEN/WAITING_DEPENDENCY/CANCEL_REQUESTED are not consistently treated as active by the composer; WAITING_USER continuation semantics also need an explicit owner decision. The composer’s Add context/file and Voice Input buttons and More options button have no handlers, Turbo is static, the @/slash affordance is placeholder copy, and every submit sends resource_refs: []. The no-selection canvas contains hard-coded Antigravity branding and unsupported capability claims. The conversation_operator.rs module header still says dispatch is absent, although production FromRef constructs AgentSessionSupervisor and submit/retry handlers call it; that source comment is stale. Keep Send fail-closed until these concurrency, retry, cancel, error-settlement, UI projection/control and restart/recovery gaps are fixed and the exact OS/provider/auth/UI route is qualified.

### Conversation agent relay

The Conversation-level agent-binding switch is real and served. `PATCH /v1/conversations/{conversation_id}/agent-binding` requires the selected Workspace, current Conversation `If-Match` version and an idempotency key, then commits `conversation.agent_binding.changed.v1`. The selected binding is used for future turns; an already-admitted turn/session is not migrated. Each created turn pins its binding in `conversation.turn.created.v2`, and message projections carry `agent_binding_id` plus `agent_session_id`.

This does not implement the promised heterogeneous A→B→A relay. The active `AgentSessionSupervisor` has one `provider_key=opencode` execution branch. `CodexAppServer` contains a context-aware turn-start method and tests, but is not connected to the Conversation supervisor. Feature 002's OpenCode profile probe marks the Runtime offer incompatible, so the current Agent Catalog flow cannot create a new OpenCode binding; Codex may be probed/bound but cannot be dispatched by the Conversation supervisor. No currently selectable pair completes the advertised relay journey.

The portable-context builder is real internal code: it reads the committed Conversation prefix through the exact USER-message event revision, orders by aggregate revision, excludes SYSTEM_NOTICE and carries the originating AgentBinding identity for prior AGENT messages. Internal limits are 1,024 messages / 4 MiB for the context package and 256 KiB for encoded native text. These are implementation constants, not an owner-approved portable-context/retention contract. Nonempty ResourceRefs and RESOURCE blocks are refused by the native prompt encoder with `ResourceMaterializationRequired`; no Resource materializer is wired into relay.

The desktop agent picker filters by same Workspace, enabled and lead-eligible bindings, but does not validate active supervisor adapter support or fresh descriptor/auth readiness. The descriptor-aware `conversation-agent-surface.ts` helper has tests but no production call site. The backend message projection carries per-message producer binding/session identity, but the desktop `ConversationMessageView` type omits both fields and `parseConversationSnapshot` reconstructs messages without them; the transcript cannot render producer attribution. The composer `Add context or file` control is inert; no Resource attachment or Context Used UI is connected.

Feature 004 remains DRAFT. B-15 (portable-context schema, allowed references and size/retention policy) and B-16 (second qualified AgentModule identity/OS/version/auth route) remain unresolved. The exact A→B→A UI journey, resource materialization, producer attribution and failure/recovery route have not been executed. Keep this area explicitly partial; do not present agent selection as proof of multi-agent interoperability.

### Native agent registry, installation, auth and binding

The desktop's Agent Registry area is a real but partial catalogue/probe/configuration surface, not a complete agent lifecycle manager.

- Served: ACP Registry list/refresh, local installation inventory, profile list, explicit Codex/OpenCode profile probes, AgentBinding list/get/create/enable/disable, profile control-descriptor GET/refresh and credential-slot submission routes. The composed route inventory records the exact mounted operations.
- ACP Registry discovery is bounded, cached metadata only. The daemon disables redirects and limits request/response time; all returned entries are projected with `NO_ADAPTER`. The UI correctly separates remote Registry presence from local installation and Workspace binding.
- Local installation inventory uses bounded version-only probes and Cline package metadata for the curated candidates. Authentication remains `UNKNOWN`; session readiness remains `NOT_PROBED`. No install/update operation is served.
- Only Codex and OpenCode have explicit Workspace-scoped profile probes. Codex produces bounded App Server observations; OpenCode produces provider/model catalogue observations but deliberately marks its Runtime offer incompatible. Neither probe establishes inference access or a qualified native session.
- The descriptor GET/refresh routes currently return `503 DEPENDENCY_UNAVAILABLE` because no qualified AgentModule descriptor producer is wired. Credential submission also fails closed with `503` until exact durable replay protection exists. Native auth-begin, native-config-open and registry install/update remain target-only.
- AgentBinding mutations are real and auditable. Storage requires a fresh compatible Runtime endpoint offer for create/enable, but enable does not yet enforce the feature's separate fresh AgentControlDescriptor requirement. Caller-supplied `auth_ref` is rejected.
- `AgentCatalogSettings` displays 15 curated candidates, ACP Registry distributions, local installation state, Codex/OpenCode profile probes, Workspace bindings/default lead, per-binding quota observations, and Workspace DelegationProfile create/revise/duplicate/disable/archive, revision-history and per-category Past work performance panels. These are catalogue/configuration and read-only provider/performance projections; they do not establish native installation/authentication or child Attempt execution. New worker profiles are created disabled; ENABLED means eligible for policy selection, not a running worker. The editor's fixed safety defaults are profile guidance/target constraints, not a live Trust/Environment boundary. Auth and session readiness remain unknown/not probed; native controls inspection reaches the unavailable AgentControlDescriptor route.
- Cross-feature gap: OpenCode has a Conversation dispatch code path but cannot be newly bound through this UI because its offer is deliberately incompatible. Codex can produce a compatible profile/binding but is not dispatched by the active Conversation supervisor. Neither combination proves an end-to-end user-selectable native agent.

Keep the feature DRAFT and do not infer qualification from these served operations.

### Workspaces and resources

The daemon has real Workspace and Resource routes. Resource quick import, chunked upload, revision upload/commit, content read, list/detail/search, text-index rebuild, index-state, grounded retrieval, locations/dependencies and context-document status/deletion routes are present. The local text index is a bounded lexical provider over allowlisted UTF-8 text formats only (1 MiB per Resource, plus control-character and term-count checks); PDF, Office, archives, images, audio and video are not indexable by this provider.

WorkspaceRoot list, pause, resume, revoke, scan and paginated observations routes are present. Desktop root creation uses a separate authenticated local-only folder-picker route, not the public OpenAPI `addWorkspaceRoot` operation. The private handler commits Resource/root/location/private binding together on Linux/macOS; the Windows fallback returns `503 UNAVAILABLE`, despite the Tauri UI leaving Add persistent folder enabled. On supported targets, the picker selection immediately commits with hardcoded `METADATA`/`NONE` policies and no separate preview/confirm. Root scan is Linux-only; Windows/macOS return `501 SCAN_UNSUPPORTED_PLATFORM`. The scan is metadata-only: it records membership/observations, does not read content, create content revisions or index files, and does not provide watching. The UI reports that watching/indexing are not active and only marks scanning unsupported after the first failed attempt. Public WorkspaceRoot update remains target-only.

The Library and Workbench Files views consume real Workspace Resource projections and can show an exact revision as a read-only text preview. The Library accepts ZIP/PDF/Office/image originals as opaque Resources; its mounted ZipIntakeNotice correctly reports ZIP extraction unavailable and says contents are not unpacked, indexed or attached. A source ZIP metadata-preview route/worker exists, but the bridge requires the non-default Linux `source-zip-worker` development feature, release sidecars omit it, and no current UI call site invokes the route; it does not implement extraction or child-Resource ingestion. Grounded retrieval is a real read-only local lexical endpoint returning revision/digest-bound citations or explicit insufficient evidence; it does not commit a ContextAttachment, event or durable retrieval receipt and is not called by Conversation send. No Context Used UI or persisted ContextAttachment record was found. The `retrieval-grounding.ts` decoder exists without a production call site. This is not a general live filesystem editor, and a Resource index is not proof that arbitrary local directories are admitted or that their full contents are available.

### Coworkers and scoped context

The authenticated Coworker routes currently serve:
- GET/POST `/v1/coworkers` (Workspace-scoped list/create);
- GET `/v1/coworkers/{id}` and GET `/revisions/{revision}`;
- POST `/revisions` (immutable definition revision), POST `/status` (pause/resume/archive state), and POST `/v1/workspaces/{id}/primary-coworker`.
Coworker identity, revision, status and Workspace primary selection are stored in the existing Coworker/CoworkerRevision and Workspace records. The Coworker revision persists context, interaction, lead-failover, delegation and notification configuration. validate_coworker_definition checks duplicate context kinds, the maximum item count (100) and requires require_user_confirmation_for_memory=true; that legacy confirmation field is not the separate CoworkerLearningPolicy. The current UI edits name/role, default lead binding, delegation strategy, enabled worker-profile allowlist, interaction defaults, lead-failover preferences, context kinds/item limit/Task-summary retention and notification preferences. It does not expose avatar selection, delegation-budget editing or a delegation-policy-source selector; delegation_policy_source is absent from the current TS mutation input and daemon field allowlist.

Policy-consumer audit (source checked 2026-11-06):
- default_lead_agent_binding_id is consumed when Task creation resolves the lead (explicit Task lead, then Coworker default, then Workspace default). Task provenance pins Coworker ID/revision.
- lead_failover_policy is copied into the TaskSpec at Task creation, but no production daemon/Operator path invokes the lead-failover evaluator or dispatches an automatic fallback lead. The UI's ASK/ALLOW_LISTED choices are saved configuration, not working failover.
- interaction_policy is persisted but no Trust evaluator/Effect admission consumer reads it. The UI correctly says it cannot create Grants, but it is not yet an active per-Coworker approval policy.
- notification_policy is persisted but no Coworker-specific delivery consumer reads it. The settings do not change Notifications delivery.
- delegation_strategy is persisted but no production worker selector/dispatcher reads it.
- enabled_delegation_profile_ids are validated against Workspace DelegationProfile records and read by the SQLite child-Attempt admission guard; delegation_budget_policy is also read by that storage admission primitive. No production daemon/Operator caller invokes child-Attempt admission, so these are not a live worker-dispatch enforcement path. The Agent Settings UI's DelegationProfile CRUD/history/performance projections do not mean a child worker can run.
- context_policy is persisted but no retrieval caller applies allowed kinds, max items or Task-summary retention. The legacy memory-confirmation field is not a memory-learning policy.

The desktop has an optional Home setup card to create a general Assistant and set the Workspace primary. The UI explicitly says this only prefills future Task settings, is not required for ordinary Conversation, and does not start an agent. Coworker Settings edits the Coworker definition/revisions and status. Its context editor currently offers Personal profile, Coworker notes, Workspace notes and Goal notes, item limit, and Task-summary retention. Its helper says durable memory proposals require review and automatic quiet learning is inactive; however, no Coworker Memory proposal/review UI or Coworker-private memory persistence exists. Treat that copy as a future-facing disclosure, not evidence of a working proposal inbox. The four-kind UI/schema does not match the narrower domain metadata validator below.

Important gaps and mismatches:
- **No Coworker-owned Conversation path:** Conversation create accepts only Workspace and title; ConversationRecord stores Conversation ID, Workspace, title, active AgentBinding, version and created time. It has no immutable Coworker origin or revision. Quick Create does not open a first Coworker chat. Coworker selection is currently available in the Task composer and is persisted as Task origin Coworker ID/revision; that is Task provenance, not Conversation ownership. Task execution is not integrated.
- **Coworker pause is not a complete proactive-admission fence:** the status transition to PAUSED can commit while enabled Automations remain. The Automation lifecycle gate checks that the Coworker is active when an Automation is enabled, but the TriggerHost/occurrence settlement path does not re-check current Coworker status; the origin-status helper has no production caller. Task creation storage rejects ARCHIVED Coworkers but does not reject PAUSED. Therefore an already-enabled Automation may still materialize READY Tasks after its Coworker is paused, despite CoworkerSettings copy saying pause stops new proactive/scheduled work. Separately, the Home Task composer rejects a PAUSED Coworker for explicit owner Task creation, although the canonical RESPONSIBILITIES contract allows owner-submitted Tasks to name a PAUSED Coworker. Resume is blocked while active Automations require reconciliation. Treat current pause semantics as partial and the UI copy/selection behavior as misaligned with the target.
- **Presence is target-only:** GET `/v1/coworkers/{id}/presence` is mounted but intentionally returns 503 `DEPENDENCY_UNAVAILABLE` after verifying the Coworker, because committed Task, UserRequest, Attempt and Runtime projections required for presence are not yet available. The UI displays the "Live availability could not be loaded" explanation; do not show synthetic availability or task counts.
- **Context ownership is narrower than the UI/schema:** `CoworkerContextPolicy.allowed_context_kinds` is persisted in the Coworker revision, but the source search found no retrieval caller enforcing its allowed kinds, max item count or `retain_task_summaries`. The Workspace domain's ContextDocument metadata validator accepts only PERSONAL_PROFILE owned by the authenticated USER and WORKSPACE_NOTES owned by the current WORKSPACE. It explicitly rejects COWORKER_NOTES and GOAL_NOTES until their authoritative aggregate services exist. The generic OpenAPI `ContextDocumentCreateMetadata` enum still lists all four values, so the schema currently overstates the accepted create contract. Those two choices are currently offered in the Coworker Settings UI and enum/schema, but cannot be created through the current Resource upload path.
- **Retrieval is Workspace/Resource-scoped, not Coworker-scoped:** `POST /v1/resources/retrieval` authorizes the selected Workspace owner and accepts either Workspace scope or explicit pinned Resource refs. Its request has no Coworker ID/revision or Coworker context policy. The local provider is lexical-only; retrieval is read-only and is not called by Conversation send. No Coworker Memory list/add/edit/revoke browser or Context Used attachment flow is served.
- **Learning gates are not a learning service:** `storage-core/src/coworker_learning.rs` exports typed candidate/policy records, deterministic source/sensitivity/redaction/conflict/auto-commit gates and an event-payload builder. Its module header explicitly says it neither classifies free text, runs a model nor persists anything. The production SkillProposal extraction path reuses only `redact_sensitive_content` from this module (`storage-core/src/skill.rs`); no production caller of `evaluate_learning_candidate`, `classify_learning_conflict` or the Coworker learning policy was found. There is no `storage-sqlite` learning store/schema/migration, LearningService, Operator learning-policy/candidate/memory route, learning event producer or Learning Inbox UI. The module tests exercise deterministic pure functions only; they do not qualify extraction or memory persistence. `docs/LEARNING.md` remains an accepted target architecture, not an active product capability.

A Coworker record or saved configuration does not mean a continuously running agent process exists. Scoped Coworker memory, first Coworker-owned chat, memory management, approved responsibility execution, Connection assignment, autonomous worker orchestration and cloud persistence must be evidenced independently.

### Tasks and multi-agent execution

Task create/list/get, objective/specification revision and planning-readiness surfaces exist. These are durable records and preflight/readiness surfaces.

The safe Task execution path is not integrated: planning dispatch, accepted-plan production, Attempt admission, Environment assignment, worker dispatch, Trust-mediated execution, Effect reconciliation and independent verification are not a complete running pipeline. A Task displayed as READY is a persisted Task state, not proof that a worker is running. No synthetic progress, completion percentage, ETA, worker timeline or success state may be displayed.

The Work screen currently provides Task list/detail, objective editing and a planning-readiness check. It does not yet provide a real Plan/Steps/Attempts/Effects execution timeline or Task controls such as pause/resume/steer/cancel through a functioning execution service.

#### Safe Task Path: explicit Task records exist; execution is not integrated

The authenticated Operator serves Task create/list/get, Task-spec revision create/list, read-only planning-readiness, plan-revision list, Step list and Conversation Task list. Task Artifact, capability-invocation, presentation/progress and verification-run reads are also served. Task creation is idempotent and commits only Task plus initial TaskSpecRevision; it deliberately starts no native session, plan, Step, Attempt or Environment. Task lead handoff commits a lead change but does not dispatch a worker.

The Home surface is a standalone “New task” composer. It saves an objective, optional Coworker/default lead and pinned Resource revision references, then opens Work. No Conversation-page promotion action is wired. Ordinary Conversation Send remains distinct and does not create a Task.

Planning readiness is a read-only diagnostic. It builds a bounded internal TaskPlanningPacket but returns `dispatch_available=false`, `planning_started=false`, `agent_session_started=false` and `plan_created=false`. `LocalPlanningPreflight::require_dispatch()` remains dead-code and always returns an error; no production caller of `admit_step_attempt` was found in the daemon.

The plan-submission POST, Attempt admission/list, Effect list/dispatch, step recovery, Task pause/resume/cancel/steer and Environment provision/control APIs remain target-only. Domain and SQLite records/admission transitions and tests exist, but they are not proof that the daemon dispatches a Task.

`probe_environment_provider()` reports platform observations, not an attached Environment. Windows reports local workspace plus Job Object process-tree capability; this does not attest filesystem roots, network deny or all resource budgets. Linux requires bubblewrap presence but explicitly notes isolation is not qualified. Budget enforcement is unavailable; checkpoint/restore and workspace-persistent lifetimes are false.

`IsolationAttestation`/`QuiescenceObservation` models and fail-closed comparison logic exist but are not wired into a production Attempt path. `attempt_process_scope.rs` explicitly states its journal/containment remains staged until exact Runtime scope evidence is connected. The served capability-grant operation currently issues bounded Task-planning Resource-read grants, not file-write execution authority. No production Effect dispatcher/reconciler/independent verification pipeline is connected to Attempts.

The Work UI lists/filter/paginates Tasks, edits objective revisions, shows pinned Resource inputs, read-only Step history, planning readiness, persisted presentation/progress and committed Artifact outputs. It does not expose plan acceptance, Attempt start, Environment selection, Effect approval, cancel/recover or other live execution controls. Planning readiness is explicitly labelled read-only; “Latest saved progress” is a projection, not proof of a live worker. Feature 005's promised single independently verified isolated file edit is not implemented.

Feature 005 remains DRAFT. B-17 (first GA OS/build, Environment provider/version and evidence across filesystem/process/network/resource enforcement) and B-18 (one precise edit class plus digest/path/size/atomicity/rollback/verification semantics) remain blocked.

#### Golden Connection and one capability write

The Discover catalog/read projection and Runtime host observations are served. Discover's `connect-or-install` route requires the owner Workspace/runtime and idempotency key but deliberately returns `503 DEPENDENCY_UNAVAILABLE` (`LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`) without creating a receipt or granting scope. Its capability-test route contacts no provider and reports `UNKNOWN`. The desktop Discover screen exposes Explore, Installed/native, Managed and Connections tabs, but Managed says package management is not connected, Connections says account Connections cannot be read, and `connectActionOutcome()` disables every row action. Public package discovery/installation and Connector UI are not qualified.

The OpenAPI Connection operations `listConnections`, `getConnection` and `disconnectConnection` remain target-only; there is no served account Connection create/start OAuth/callback/refresh flow. A local OS SecretStore adapter and write-only trait exist, but no Feature006 Connection service uses them. The separate Feature002 AgentProfile credential-slot route currently returns 503 before SecretStore write pending durable replay protection.

Approval list/get/resolve are real. Approval resolution checks If-Match, Idempotency-Key, authenticated local-owner assurance and the exact action digest on APPROVE, then commits `approval.resolved.v1`. Needs You ApprovalDetail renders and resolves that durable decision, preserving the request ID across retry. Its UI explicitly says approval records the exact-action decision only and does not grant reusable capability authority or run the action.

The served CapabilityGrant create/revoke operation is currently narrow: bounded, expiring Task-planning `workspace.resource.read` access to explicit Resource refs. CapabilityInvocation APIs are read-only GET list/detail. Their Operator module explicitly says it does not create, transition or dispatch Invocations; the SQLite store rejects DISPATCHED with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`. The Trust evaluator is compiled as dead code/unwired; it does not dispatch providers, issue SecretLeases or create a permit. There is no atomic CapabilityBroker admission transaction, provider write, Effect dispatcher/reconciler or provider-backed verification path.

Feature006 remains DRAFT. B-19 (one exact provider and public OAuth/PKCE recipe, scopes, refresh/revoke and OS/host support) and B-20 (one exact provider write operation, grant/approval binding, idempotency and reconciliation evidence) remain owner decisions. No provider account was connected, no OAuth or provider read/write was executed, no Invocation was dispatched and no external Effect was reconciled. Do not represent package catalog, approval decision, generic Task read grant or Invocation read record as a working account Connection/capability workflow.

### Trust, Effects, Environments, browser and computer

The Trust evaluator and several environment, isolation, browser and computer-use modules compile in the workspace, but the daemon bootstrap does not wire them into a qualified Task execution/admission path. A type, test fixture or module is not an execution boundary.

A LiteCowork Gateway controls only calls that actually pass through it. Native shell/filesystem tools, hidden native subagents and other direct harness actions remain governed by the native agent/OS unless a qualified hook or OS boundary mediates them. Reported native actions must not be represented as LiteCowork-authorized or reconciled Effects.

**Browser current truth:** `WorkbenchPanel` renders a generic HTML iframe preview with a local React-held URL (default `http://127.0.0.1:4242/v1/health`). It is not Chrome/Chromium, not a persistent browser profile/session, and not a Playwright/CDP controller. Switching away from the Browser branch unmounts the iframe; closing the pane or changing Workspace resets the React state. `browser_consent.rs` and `browser_control_service.rs` contain policy helpers for managed profiles, consent, browser-version refusal, actor/lease/epoch/tab-generation validation and takeover/return, but `main.rs` explicitly compiles both modules with `allow(dead_code)` and comments that no Operator route, browser provider or Chrome process is connected. Their call sites are test-only. Environment take/return-control APIs are target-only and are not the browser helper path. There is no BrowserSession store, route, native browser process, tab observation, action dispatch, durable consent UI or takeover/return USER flow. The iframe must be labelled only as embedded preview, not as an agent browser.

**Computer-use current truth:** `environment_computer_use.rs` explicitly says it is a policy boundary, not a provider, and every capture/input request is refused. Its provider probe reports unsupported on non-Linux; Linux with display still reports unsupported because portal screen-cast/remote-desktop paths are not qualified, and no display is unavailable. Screen capture and input-injection flags are always false. The module is `allow(dead_code)` in the daemon bootstrap. No computer-use route, capture backend, input injection backend or live UI exists. Do not show a screen/cursor/permission badge that implies readiness.

A Windows Job Object is a process-tree containment primitive. It does not by itself prove filesystem-root confinement, network denial, credential separation or that a process was actually admitted to a Task Environment. No Workbench browser or terminal process currently uses one.

### Terminal and diff presentation

**Terminal current truth:** `WorkbenchPanel` owns `terminalScrollback`, input, command history and history index in React state. `handleRunTerminalCommand` does not invoke Tauri IPC, `Command`, a shell, or any daemon terminal service. `help`, `status`, `dir/ls`, `whoami` and `version` append fixed sample strings. `dir/ls` lists Resource projections rather than directory entries. The generic-command branch appends a fabricated `completed with exit code 0` message without executing a process. The pane is labelled `pwsh 7 · Windows ConPTY` and `KILL_ON_CLOSE`, and initial scrollback claims Job Object containment; neither is backed by a real PTY/ConPTY process. There is no terminal Tauri command, PTY/ConPTY spawn bridge, daemon TerminalSessionService, session store, shell-profile registry, stdin/stdout/stderr stream, resize, process identity, detach/reattach or explicit Stop handler in the active Workbench route.

React-local scrollback can remain visible while changing tabs only because the WorkbenchPanel component remains mounted. Closing the pane unmounts it; Workspace changes remount it because the panel is keyed by Workspace. This is not a resident shell and does not preserve a child process across tab switches or application restart. The current shell-engine, terminal scrollback and command-approval settings are local preferences/state, not persisted Environment policy.

**Diff current truth:** `selectedDiffFile` is always null. The Diffs pane transforms current Resource preview lines using a fixed modulo rule to label some as additions/deletions; when there is no preview it renders a hard-coded example patch. The summary badge is fixed at `+4 additions / -1 deletions`. It does not inspect Git, exact ResourceRevision pairs, Artifact versions, changed files or verification evidence. Do not interpret this as a real diff or successful modification.

**Activity current truth:** `WorkbenchTab` includes an `activity` union member, but there is no Activity navigation control, `setActiveTab("activity")` call or Activity render branch. The final render branch falls through to Diffs. Activity is not a rendered Workbench surface.

A real Terminal must be owned by a daemon Environment/terminal-session service, with qualified PTY/ConPTY spawn, exact shell executable/profile/argv/cwd/allowlisted environment, process identity, actual Environment containment, bounded output/backpressure, resize, detach/reattach, explicit stop and honest quiescence/lost-session behavior after Runtime restart. A diff must be computed from actual immutable revision/content pairs or a VCS change set. Activity must use committed events/Attempt/Effect/process observations or be placed in Work/Task detail according to the canonical screen contract.

### Automations and Routines

The local AutomationTriggerHostWorker is implemented and started by OperatorServer::start. It polls persisted cursors every 30 seconds, checks current Runtime Workspace bindings and settles supported recurring schedules. The automation_operator.rs module header is stale where it says recurring hosting and /resume are unavailable; the route and worker are served. The AutomationsPage Pause success message is also stale: it says trigger execution is unavailable in this build even though the local host is running. The UI supports cursor-paginated definitions, create/revise, templates, health, pause/resume/disable, Manual Run and Create test Task. Occurrence history reads only its first API page and discards next_cursor, so older history cannot be loaded. Manual Run creates an occurrence plus READY Task; Create test Task creates a READY Task without occurrence/cursor. Neither starts planning or agent execution.

Execution-policy audit: max_concurrent_occurrences, overlap_policy and schedule misfire are consumed by the host. retry_policy and wake_policy are persisted but have no production daemon consumer; no Task retry executor or Runtime wake provider is wired. Placement/budget are copied into Task inputs but do not imply a Task executor or cloud placement. Local recurrence requires an available Runtime; laptop-off execution is not provided.

Notification audit: settlement inserts PENDING delivery rows, but no production sender/consumer or status-transition writer for SENDING/SENT/FAILED/AMBIGUOUS/SUPPRESSED exists. The Notifications Operator reads delivery history and writes preferences only. preferred_channels is stored but not read by the outcome producer, which chooses the first active ChannelBinding or treats any active Runtime Workspace binding as a local channel. ON_CONDITION means only DEPENDENCY_BLOCKED, not a named condition. Quiet hours are evaluated if present but cannot be edited in the current UI. A PENDING row is not delivery.

A Manual Run or due scheduled occurrence can materialize an ordinary READY Task pinned to the relevant Automation/Routine revisions. This does not start Task planning, an agent session or execution. The trigger host is a scheduler/materializer, not a second Task engine. The machine must remain awake and the Runtime must remain available for local recurrence; laptop-off execution is not provided.

### Discover and LiteSPM

Discover currently reads Workspace-local skill records and bounded Runtime-observed capability-host records, and can open a review-only detail disclosure. It distinguishes Explore, Installed/native, Managed and Connections.

The LiteSPM install/adopt/update/repair/unmanage/remove bridge is not qualified or connected as the package-lifecycle authority. The Managed and Connections sections correctly report unavailability. Do not build a second package inventory/config writer inside LiteCowork. The version-pinned LiteSPM service contract, receipts, plan digest, target-host compatibility, idempotency and UNKNOWN reconciliation remain prerequisites.

### Artifacts and intelligent presentation

**Artifact status: real immutable read/version model and narrow text editor; no general Artifact creation or qualified professional format workflow.** The authenticated Operator currently serves Artifact list, saved-only Library, exact Artifact/version metadata and content, text edit-head, bounded text-version append, promote/archive, and Task Artifact output reads. The active production storage ports include `ArtifactReadStore`, `ArtifactLibraryWriteStore` and a narrow `ArtifactVersionWriteStore`; there is no production Artifact-create method or `POST /v1/artifacts` route. Canonical `artifact.created.v1` is registered in the event schema and `docs/ARTIFACTS-EVIDENCE.md`, but no active production producer was found. Artifact rows in store tests are seeded directly. A fresh Workspace may therefore have an empty Artifact Library.

The only content publication write path currently exposed by the Artifact Operator is user-authored UTF-8 `text/plain` append, at most 1 MiB, to an existing managed Artifact. It pins the aggregate version, content version, backing Resource version and exact parent ResourceRevision, verifies the BlobStore digest/size/type, and atomically appends a new immutable ArtifactVersion with the matching ResourceRevision, events and idempotency receipt. It does not create the first Artifact or provide a general agent/provider binary publication API. Promotion/archive are real, idempotent metadata-only TRANSIENT→SAVED and SAVED→ARCHIVED transitions; they do not create or modify content.

The UI distinguishes preview from edit. Bounded text preview up to 1 MiB supports plain text, Markdown, CSV/TSV, JSON, XML and YAML variants through a restricted renderer. The editable append route accepts only managed `text/plain`; Markdown/CSV/JSON preview does not mean those formats have an editor. `ArtifactWorkbench` supports exact-version history/compare, bounded literal line diff, provenance display, text restore as a new immutable version, promotion/archive and Save As. Task output UI is read-only and consumes already committed Artifact records. Library itself has no Artifact creation/import-convert button.

Native Save As is a genuine local file operation for an exact managed Artifact version up to 10 MiB. Tauri rechecks Artifact/version/ResourceRevision identity and content digest/media/size; after the native picker it fetches and verifies the exact blob then writes through same-directory staging and atomic rename. Only SAVED/CANCELLED returns to WebView. Save As creates no Artifact/Resource revision, domain event or durable ExportReceipt. Linked external content is not fetched by this path. Do not describe it as provider export or receipt-backed publishing.

Office/PDF readiness code observes whether DOCX/XLSX/PPTX/PDF capability might be available; it is not an editor/parser/exporter or format round-trip. Library uploading those extensions creates Resources, not Artifacts. ZIP intake remains a conservative source-build/Linux-only slice and does not extract Office content into Artifact versions. No accepted professional format engine or format-specific verification path is present. Feature 005's Task execution is also unavailable, so Task-produced Artifact publication has no live producer.

Feature 007 remains DRAFT. Its missing release journey is explicit: one selected professional format/editor/parser/export engine with round-trip qualification (B-21), initial Artifact creation and provenance transaction (B-30), and a durable export receipt distinct from native Save As (B-31). These IDs are globally unique; Feature 008 separately owns B-22 browser provider, B-23 PTY/ConPTY provider and B-32 Computer-use decision. The target five-layer presentation model remains; safe rich-presentation fallback is not evidence of a professional document authoring engine. Interactive Artifact execution remains a separate unqualified runtime.

The target five-layer presentation model remains: semantic response; rich presentation; generated media; declarative Intelligent UI; isolated interactive artifact. Declarative chat presentation must use bounded schemas and registered components, never arbitrary HTML/JavaScript/eval. The semantic answer remains authoritative when rich rendering fails. Interactive artifacts require a separately qualified isolated runtime; a restrictive iframe/CSP alone is not a sufficient isolation claim.

### Usage and telemetry

No complete authenticated UsageSummary, UsageItems or QuotaSnapshot projection and no end-user Usage dashboard are served by the active Operator. Budget-admission types and worker cost observations are not a historical inference-usage ledger. The target contract in `V1-TELEMETRY-USAGE-RESILIENCE-LLD.md` defines per-inference observations, token/cost confidence, price snapshots, separate quota snapshots, local-harness readers, deduplication and aggregation. The ingestion, durable storage, OpenAPI, Operator, TypeScript and UI chain is not complete. Usage totals must remain unavailable until that chain is integrated and tested; unknown cost must remain UNKNOWN rather than render as zero. The target dashboard filters and provider/account breakdowns remain target behavior, not shipped controls.

### External workflow integration

The active source has no in-Core WorkflowGraph schema, graph editor, node/edge revision store, graph-run engine or generic code-node runtime. `WorkflowBackendAdapter` and provider-owned workflow integration are target contracts only; no production adapter is served or qualified. The intended boundary is one provider-owned workflow invocation inside an already-admitted LiteCowork Task, with the external provider retaining ownership of its graph, nodes, versions, internal execution and code steps. LiteCowork does not own or recreate a generic visual workflow engine.

Manual graph authoring remains in the external provider's own editor. A future provider-specific authoring adapter requires a separate owner decision covering draft and validation, revision/digest, effect summary, explicit confirmation, idempotent publish and ambiguous-result recovery. OpenBot, n8n and Activepieces are reference material only; the inspected OpenBot graph surface is not adopted as a Core runtime or security template. See `CONNECTORS-WORKFLOWS.md`, `PRODUCT-STRATEGY.md`, `COMPETITIVE-RESEARCH.md` and `OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md` for the respective normative boundary and research evidence.

### Needs You and other read surfaces

Needs You reads persisted user-request, approval and attention projections and exposes only the actions backed by their current owner operations. Notification, Goal, Suggestion and other Workspace read routes do not establish a complete delivery, scheduling or action lifecycle; each must be qualified against its own producer, persistence, UI and recovery path.

## Test and validation evidence

Validation evidence from the connected Windows development machine:

| Check | Result | Boundary |
|---|---|---|
| `CARGO_BUILD_JOBS=1 cargo test --workspace --locked` | **PASS — exit code 0; 0 failed targets** | Full Windows workspace run completed in 1,383.28 seconds. `litecoworkd` 359 passed / 1 ignored; CLI 4; `storage-core` 110; `storage-sqlite` 555; `agent_registry_cache` integration 2; Windows local security 36; local staging 24; operator IPC 18; domain and browser integration targets all passed. The deterministic backup failure-path test passed in the full SQLite target. Two tests were intentionally ignored (daemon-declared ignore and Windows Credential Manager side-effecting integration). The single-job Windows workspace run completed successfully |
| `litecoworkd` unit target | **359 passed, 0 failed, 1 ignored** | Includes the six Windows WorkspaceRoot route tests and Windows local-filesystem locator/reopen tests |
| `litecoworkd --test cli` | **4 passed, 0 failed** | Help and CLI behavior distinguish native Conversation dispatch from managed Task execution and Runtime lifecycle readiness |
| Composed Operator route inventory | Passed; matches **173 served / 1 local-only / 56 target-only** | Registration only, not integration/qualification |
| Architecture/schema validator | Passed active checks; one warning | 148 typed events, JSON Schemas, error codes, OpenAPI structure, SQLite, Gateway names, canonical concepts, product naming and active Markdown links pass. Optional `openapi-spec-validator` is not installed, so its external meta-schema pass was not run |
| Cross-owner consistency guard | Passed **36 assertions** | Checks only the explicit design invariants encoded in the script |
| V1 contract/route/UI audit | Passed **0 findings** | Structural crosswalk only |
| Design-depth audit | Passed **0 structural findings** | Inventory and ownership structure only |
| Feature package validator | Passed **11/11 packages** | SDD structure, not owner acceptance |
| Markdown link audit | Passed; **0 broken relative links** | Limited literal Markdown-link parser; excludes archives, derived reading notes and reference repositories |
| Active documentation corpus | **257 files / 54,497 lines** at the generated manifest | First-party Markdown/MDX inventory excluding archives, reading digests, repo_take and generated audit output; separate Markdown audit scanned 258 files / 54,797 lines and found 0 broken relative links |
| Frontend production build | Passed; API type check, TypeScript and Vite completed | Main JS bundle 1.05 MB minified / 287 KB gzip; Vite warns over 500 KB |
| Focused Conversation + responsive tests | **24 passed, 0 failed** | Includes list cursor pagination, pending-create journal retry affordance, repeated-cursor protection, drawer semantics and responsive navigation/canvas assertions |
| WorkspaceRoot route filter on Windows | **6 passed, 0 failed** | Real SQLite route fixture now uses canonical platform-specific locator and file identity data; no production locator validation was relaxed |
| Full serial frontend suite | **Incomplete; no final TAP summary** | The full serial command did not produce an aggregate result. The focused Conversation + responsive suite passes 24/24, including cursor pagination, pending-create journal retry, repeated-cursor protection, drawer semantics and responsive navigation/canvas assertions |
| Settings browser accessibility test | **1 static check passed, 1 browser check failed** | Static WCAG AA quiet-button contrast assertion passed. The standalone Chrome/DevTools test timed out on `Page.navigate` before the production Settings UI could be checked; this is a browser-harness failure, not a Settings assertion pass |
| Packaged Tauri / full provider E2E | Not qualified | No packaged desktop or exact OpenCode USER journey was established in this run |

The structural validators and focused frontend coverage are useful evidence, not release qualification. The Windows Rust workspace tests are green for this checkout, but this does not qualify the full product journey. The Conversation commit-to-dispatch recovery gap, incomplete Task execution path and real Workbench-provider gaps remain release blockers.

## Release blockers

1. Preserve the passing Windows Rust workspace result; keep the six passing Windows WorkspaceRoot route regressions separate from the currently unsupported Windows folder-scan provider. Retain the intentionally ignored credential-store integration test as a separate qualification gate.
2. Close the Conversation commit-to-dispatch crash gap with durable dispatch admission/outbox or an equivalent recoverable state transition. Prove retry, replay, crash and restart behavior.
3. Qualify the complete native OpenCode Conversation journey, then implement and separately qualify each additional native agent route before exposing it as Send-capable.
4. Connect Task planning, accepted PlanRevision, Attempt admission, Environment, Trust, Effects, worker execution, reconciliation and verification as one recoverable vertical slice.
5. Remove the simulated Terminal and generated sample Diffs; implement real process-backed terminal sessions and actual diffs.
6. Replace the iframe preview label/behavior with an honest preview surface until a real browser Environment/controller exists; design Computer use separately.
7. Disable or implement the inert Settings controls and cache-clearing notices.
8. Qualify LiteSPM's versioned service integration; preserve one package manager and separate installation, connection, assignment and invocation authority.
9. Complete one professional Artifact format and one declarative Intelligent UI path with schema validation, fallback semantics and evidence.
10. Align every screen with the release contract: no visual state may imply a Task, approval, Effect, process, browser, progress or completion that the owner service has not established.
11. Run frontend type/build/tests, packaged Tauri + daemon tests, Windows OS/security tests, restart/crash/denial tests and owner USER acceptance on the declared versions.
12. Keep cloud continuation and remote/mobile control outside local V1 until portable Task state, control-plane fencing, remote authentication and exact placement semantics are accepted and implemented.

## Implementation ownership and next engineering order

The shortest safe route is not to implement more top-level screens. It is to make the promises already exposed by the UI truthful and complete in vertical slices:

1. Conversation dispatch durability and the qualified OpenCode route.
2. One safe Task plan/Attempt path with a real isolated Environment and a single mediated file operation.
3. Real terminal session ownership and evidence-bearing diffs; remove mock success.
4. Coworker-scoped Conversation/memory path and the minimum useful artifact format.
5. LiteSPM observe/plan/apply/reconcile with explicit grants and receipts.
6. One useful browser/connection capability; then broader Workbench and automation features.
7. Heterogeneous delegation, richer Intelligent UI, and progressive multi-agent visualization backed by durable Task events.

The [UI screen contract](V1-UI-SCREEN-CONTRACT.md) defines the intended product interaction; this status document records which behaviors the current source actually supports.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]