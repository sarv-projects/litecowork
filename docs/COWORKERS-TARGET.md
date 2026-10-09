# LiteCowork Coworkers — Consolidated Product, UX & Architecture Specification

**Status:** Accepted Coworker target design, reconciled 2026-10-10. **Not an assertion of implemented functionality.**
**Authority:** This target UX/domain proposal supplements `ARCHITECTURE.md` and the owning contracts linked from `docs/COVERAGE-MATRIX.md`. Existing implemented schemas and APIs remain authoritative until explicitly revised; proposed definitions below are not already-running endpoints or storage.
**Purpose:** Single handoff contract for product design, desktop experience, domain modelling, integrations, long-running autonomy, memory, safety/permissions, events, and implementation testing.

## 0. Decisions — frozen

1. Coworkers are optional, specialized **persistent identities/responsibility bundles** associated with Workspace. Ordinary sessions continue to use direct chief-agent/model/effort selection; selecting a Coworker is NOT mandatory.
2. Left sidebar has **Coworkers** navigation separate from standard sessions, with + New Coworker. Clicking an existing Coworker opens its **most recently active conversation**. A Coworker has multiple conversations, and + New chat starts another.
3. Conversation-first center with a spacious composer, selected native chief agent, native model and effort visible; attractive rich messages and truthful collapsible activity; right Workbench remains available for documents, browsers, apps, charts, spreadsheets, maps, and artifacts. No obligatory task-management dashboard.
4. Creation offers **Quick Create** (name and responsibility, other defaults inherited) and **Customize Setup** (identity, chief agent, eligible external workers, memory/knowledge, optional integrations, responsibilities, review). The outputs are the same Coworker type. No mandatory connectors or schedules.
5. No arbitrary numerical limit on **configured** connectors, MCP servers, skills, permitted apps or knowledge sources; no promise of infinite live concurrency/performance. Discover and activate tools lazily. Installed/connected/assigned/authorized/activated/invoked are distinct.
6. Authentication and model-provider configuration remain **native-agent-specific**: Claude Code and Codex login under respective agents, OpenCode models/providers under OpenCode; NO second global LLM provider router in LiteCowork.
7. **Settings → Subagents** is separate from Agents: install/enable whole native harnesses as eligible external worker profiles; chief can also use its own native subagents. The same native harness can be chief and an external worker; access is re-scoped.
8. Scoped memory with Coworker-private default, explicit Workspace- and personal-sharing, and conversation-only contexts. **Automatic learning** enabled for eligible sources, **quiet** UI. Source access is not equivalent to memory-learning or sharing permission.
9. A Coworker is NOT a new planning/model runtime. Its chief native agent owns reasoning, planning and delegation; LiteCowork owns sessions, permissions, durable tasks, triggers, state, effects, evidence, and execution safety. Core never maintains a second unrestricted cognitive planner.
10. Desktop-first: close window and approved background service continues eligible work while machine is on. Locked, sleeping, shut-down, offline and missing-dependent-app states are represented honestly. Future cloud placement must reuse the same domain model, not require a product redesign.
11. Natural-language standing responsibilities are proposed/reviewed, and **explicitly enabled**. Autonomous background research follows authorized scope and bounded budgets. Permission prompts are reserved for meaningful new authority or consequential action, not every read or tool call.

## 1. Information architecture and core UX

**Navigation:**
- New session; global search; standard chat history.
- Coworkers (expand list; + Create). Coworker menu includes New chat, Conversations, Responsibilities, Work, Memory & Knowledge, Connections & Tools, Settings, Pause new work, Pause all safely, Archive.
- Global Work/Tasks, Automations (Routines/Automations/Runs), Needs You, Library/Artifacts, Discover, Settings.
- Distinguish navigation destination from user-facing data owner. An Automation run and a Task can appear under both the Coworker and global Work, but remain **one canonical record**.

**Initial Coworker open:** last-active Conversation, not a home dashboard; no active chat → empty centered composer with avatar/name/purpose; no faux status. Header shows separate proactive status, actual activity, and Runtime eligibility. Chief/model/effort selectable from real adapter catalog. + menu: files, folder, screenshot, knowledge/library, connect app. Workbench initially closed; later selected artifacts open there. Multitask runs show concise outcome/update cards with links to dedicated Run details, not 10,000 tool-message entries dumped into main chat context.

**New-conversation semantics:** New chat under Coworker keeps identity/eligible memory and allowed knowledge, but starts clean transcript and separate AgentSession. Previous messages aren't silently replayed. Most-recent ordering uses persisted activity (not display-only local clock). Coworker rename doesn't rewrite historical Task provenance. Chat deletion/archive should clearly separate removing a view from deleting actual Task artifacts and retained memories. A temporary chat disables learning from that conversation while preserving ordinary Task audit necessary for safety; it does not bypass Workspace execution policy.

**Quick Create:** name + role/purpose, optional avatar → create, inherit Workspace default eligible lead if any → open Coworker chat. No connection required. If no lead is eligible, show compact connect/select-agent repair, retain drafts. Do not implicitly enable unattended work. **Customize:** step 1 identity; 2 default chief/model/effort (native options); 3 eligible externally enabled subagents; 4 memory/knowledge; 5 optional apps/MCP/skills/local apps; 6 responsibilities/notifications; 7 understandable review. Every stage after identity may be skipped; save draft; users can later edit all preferences.

**Editing settings:** versioned, save applies to future turns and new Tasks. An in-flight Task retains its pinned TaskSpec/AgentBinding/CoworkerRevision/Grant; changing settings never silently upgrades it. Pausing new autonomous work is distinct from pausing in-flight work at safe boundaries.

**Interaction pattern:** Users describe desired outcomes; the assistant makes a proposed operation/responsibility, shows when/where/how and anything important needing approval; user confirms enabling standing behavior. Plain-language summaries first. Advanced configuration is optional and placed under disclosure, not as mandatory permission matrix.

## 2. Canonical concept model and ownership

| User term | Authoritative domain owner | Responsibility |
| --- | --- | --- |
| Coworker | CoworkerService/CoworkerRevision | identity, context and interaction defaults, eligible chief/worker profiles, proactive preferences, notification preferences |
| Chat | Conversation/Turn/Message | human continuity; current turns, provenance and contextual outputs |
| Responsibility | Proposed **StandingResponsibility** grouping aggregate | user-facing standing obligation and linkage; *no agent or execution engine* |
| Goal | GoalService | passive desired outcome + acceptance and references; does not self-execute |
| Routine | RoutineService | versioned reusable task specification |
| Automation | AutomationService/TriggerCoordinator | triggers and occurrence admission; not a workflow engine |
| Work | TaskService, Attempt/AgentSession | durable reasoning/execution lifecycle and results |
| Connector/Skill | LiteSPM/provider, CapabilityBroker, TrustService | installation/connection, compatibility, scoped authorization, activation and invocation |
| Memory | ResourceService versioned ContextDocuments + optional extraction/retrieval provider | scoped durable facts/preferences, revisions, deletion and provenance |
| Artifact | ArtifactStore/ResourceService | exact versioned outputs and provenance |
| Needs You | UserRequest/Approval/Notifications | human decisions and scoped communication |

**Why StandingResponsibility?** User-facing request `monitor source daily and produce a Friday report` can reference a Goal, two Routines, multiple Automations, and many Tasks. Neither one Goal nor one Automation alone models this family. The grouping aggregate is a transactional *configuration/relationship container* with a state gate and no independent planner, lease or executor. Existing Automations remain authoritative for triggers and Task creation. No obligation to add this aggregate for simple one-shot runs; use it for standing obligations with multiple linked components.

## 3. Responsibility/autonomy semantics

**Supported initiation modes:** immediate on-request Task; one-shot or recurring schedule; authenticated connector event/webhook/channel event; authorized Resource change; task/runtime/process completion; bounded condition watch; proactively suggested task. Trigger kinds are supported **only when a concrete provider is qualified**, not simply because a schema union names them.

**Create from chat:** native chief can propose (not enable) a typed Responsibility draft containing purpose, expected output, success/stop criteria, inputs, trigger(s), source/auth dependencies, execution location, approval categories, schedule/timezone and misfire semantics, resource budget, run overlap policy, notification behavior, expiration/review date. Show simple confirmation card `What / When / Where / What needs you / Turn on`. Only explicit user confirmation atomically writes standing configuration and enables trigger hosting (or stages definition PAUSED, then enables after readiness). If unsupported or ambiguous trigger: show precisely what is missing; no silent substitute.

**Responsible follow-through:** explicit accepted Goal + approved monitoring/read-only sources may produce bounded Suggestions. Automatic goal continuation *without a new user message* is allowed only if owner authorized a defined standing responsibility/trigger + finite actions/budgets. A mere Goal is passive. The model cannot turn an observation into a new unrestricted permission grant. For unrequested opportunities: propose a suggestion with source and why; only execute when it matches previously accepted authority, or after user acceptance. Avoid AI heartbeat loops by default; use provider subscriptions, deterministic filters, local resource watchers, and bounded scans before spawning a model.

**Runs:** a scheduled/event occurrence creates one canonical durable Task pinned to exact Routine/Automation/Coworker revisions. Trigger host placement independent of execution placement. Each run may have a dedicated reviewable result thread; the main Coworker chat shows a concise linked update/activity item rather than ingesting the full run transcript. User can continue talking to Coworker while work runs.

**Scheduling:** precise IANA timezone; DST gap/fold defined; `SKIP`, `RUN_ONCE_WHEN_AVAILABLE`, `CATCH_UP_BOUNDED`; explicit overlap `SKIP`, `QUEUE`, `CANCEL_OLD` safely, `ALLOW` within available concurrency. A quiet check that finds no relevant change results in `NO_ACTION` (a result, not Task status), without user notification. Distinct events are deduplicated via trigger ID, source event ID, occurrence key, digest and fenced host epoch; cross-trigger correlation/coalescing cannot be assumed—requires an explicit opt-in policy and tests.

**Monitoring:** signal source identity, observation cadence or event feed, safe filter, freshness/cursor, reset/rescan, tolerance to gaps, action/notification threshold, cooldown, debounce, expiration, max investigation budget; per-source throttling to avoid quotas. New source not auto-included. Feedback `irrelevant`, `don't alert me again`, `more like this` changes a reviewable preference, not root permissions. No indefinite agent thought loops.

**Proactive status states:** `OFF` (no auto work); `ACTIVE` (authorized triggers may admit work); `PAUSED_NEW_WORK`; `NEEDS_ATTENTION`; `UNAVAILABLE` on required dependency; `ARCHIVED`. Activity and Runtime status remain separate projections. Don't invent emotions or synthetic typing. `Pause new work` fences new proactive admissions; `Pause all safely` also requests pause on linked running Tasks through TaskService and may remain `PAUSE_REQUESTED` when an Effect cannot be safely interrupted. `Archive` blocks new work and requires dependencies/routines handled, but never deletes history accidentally.

**Responsibility lifecycle:** Draft → Enabled ↔ Paused → Completed/Expired/Archived; separate `HEALTHY | DEGRADED | BLOCKED | UNKNOWN` derived projection. Completed means configured responsibility closed by user or explicit terminal/stop criteria validated under approved policy; it is *not inferred from a single successful Task* and does not magically mark Goal success. Extension/update creates new revision; existing executions keep pins. Reauthorization required if intended access or sensitive consequences broaden.

## 4. The user-facing Responsibilities view

Conversation stays default. `Coworker → Responsibilities` is a secondary, lightweight list:
- active responsibility name/purpose, last outcome, next run (or condition trigger), source, desktop availability, Next/Recent result, pause/edit/run-test;
- proposed responsibilities separated from enabled, shown with review affordance;
- concise `Needs You` inline marker and deep link;
- advanced details expand to source conditions, schedules, recovery, budgets, concurrency, receipts, pinned revisions;
- sortable active/upcoming/needs-attention/paused/finished; empty state `Tell [Coworker] what you want it to take care of` rather than technical blank forms.

**Runs:** work record opens full execution thread with timeline, evidence, artifacts, source refs, real progress, exact Runtime/approval blockers and pause/cancel/steer. No hidden second work engine. Normal Workbench stays available for editing produced documents, files, rich app outputs; opening Responsibilities should not silently discard a dirty Workbench draft.

**Notifications:** event-based, user-notification preference tiers `Needs my decision`, `Finished important work`, `Significant change`, `Routine summary`, `Quiet`; quiet hours and batched digest for low priority; avoid duplicate notices across chat, desktop tray, email or messaging channels. Deliver through existing Notification/Channel adapters, with channel identity assurance appropriate for approval. Muted alert does not erase auditable Task state. A Coworker may report `No update` only with evidence of an actual completed observation.

## 5. Add Connections & Tools (optional and unlimited configured count)

Entry in creation Customize flow, later Coworker Settings, and contextual `Connect` from a blocked task. One searchable catalog with Apps/accounts, MCP servers, Skills, local applications/Desktop, Knowledge. Installed/shared connection can be assigned without reauthentication or reinstall when allowed. Account sign-in is owned by provider and secure Connection flow. Package install/discovery/updates remain LiteSPM-owned. Skills use verified manifests and qualified loader. Unsupported installed local apps are not labeled available merely because the executable exists.

**Default UX:** Select source → sign in/install if necessary → short plain-language identity/what it can do summary → add to Coworker. Avoid per-tool toggle walls. Progressive operation/resource restrictions and audit in `Manage access`, with action-time approval or preapproved narrowly scoped responsibility policies when necessary.

**Authoritative distinctions:** `catalogued`, `installed`, `authenticated`, `assigned`, `currently ready`, `currently granted`, `activated`, `invoked`. Assignment to Coworker is a ceiling and cannot issue a CapabilityGrant or Approval. Native unmediated tools cannot be falsely claimed restricted/recorded by LiteCowork. For consequential tools require LiteCowork mediation or a formally qualifying harness-native enforcement bridge; otherwise disable unattended consequential execution.

**No global provider model-management:** Model options belong to each native chief; OpenCode/Claude/Codex provider settings are agent-specific. External subagent profiles are global user-configured in Settings → Subagents and selected for Coworker use as a restrictive allowlist.

## 6. Memory, knowledge, continuity

**Scopes:** conversation-only (never auto-save), Coworker-private (automatic default), Workspace-shared (explicit configured rules), personal-shared (explicit separately authorized). Access to Gmail/file/Drive is not permission to mine every item into long-term memory. Protected categories and sensitive third-party data are never automatically stored in long-term inferred memory. Temporary chat disables memory extraction from that conversation, including deferred task summaries, unless user explicitly overrides with a reviewed save.

**Learning:** queued candidate extraction via qualified agent-backed capability (no independent Core reasoning), at meaningful turn or Task-result boundaries; candidate includes assertion, class (preference/fact/decision/status/procedure), confidence and evidence, source refs and freshness, scope eligibility. Deterministic gate tests scope, source trust, sensitivity rules, relevance, retention and budget. Reconcile with existing memory; versioned supersession for corrections, mark conflict/stale where ambiguous, do not silently merge incompatible claims. Automatic private commit within preauthorized rules; broadened shared scope requires prior explicit policy. `quiet`: no modal per-save, optional subtle indicator for salient changes, complete activity history and edit/revoke/delete in Memory & Knowledge.

**Retrieval:** query based on current Task/turn and authorized scopes; search (deterministic first, optional semantic provider), fetch bounded pinned revision, recheck final authorization/freshness before attaching. `Context used` lists real sent attachments/retrieval receipts, never asserts an agent read all indexed files. Native harness hidden transcripts/prompts are not synchronized. Changing chief preserves LiteCowork memory, not private agent-owned history.

**Pruning:** stale detection with time-sensitive expiry; provider-confirmed deletion status; redact private memory from other Coworkers and delegations. Memory shouldn't turn model speculation into authoritative status or task completion. Skill publication is a separate approval review, not auto-generated executable capability.

## 7. Desktop runtime and continuity

**Desktop default:** `LOGIN_BACKGROUND` after informed consent; per-user `litecoworkd` service independent of window, with OS supervisor (Windows per-user Task, macOS LaunchAgent, Linux systemd --user) where qualified. GUI close does not cancel work. Explicit `Stop background work`, `Launch at login`, and safe shutdown preview. No hidden elevated service.

**Machine sleep/off:** local code does not execute; maintain persisted schedule deadlines and watcher cursors, record observation gaps; on wake/restart revalidate Runtime incarnation, resources, service health, grants, approvals, leases, still-open external Effects; apply misfire policy. Locked screen may prevent interactive desktop control; wait for unlock. Local-only browser/auth resources cannot magically move to a cloud worker. Future cloud execution chooses another eligible environment through Task placement, with same Task/Automation models and portability constraints.

**No silent fake resumption:** An external action dispatched before a crash but not acknowledged is `AMBIGUOUS`; reconcile provider state before considering retry. Stop/cancel cannot declare success before external Effects settle. If no safe continuation exists, block and ask user.

## 8. Architecture and data-model extensions (additive, non-authoritative where specified)

**Existing authority remains:** Coworker, Goal, Routine/Automation/Occurrence, Task/Attempt/AgentSession, Resource/ContextDocument, CapabilityGrant/Activation/Invocation, Effect, Evidence, Approval/UserRequest, Notification. `StandingResponsibility` is only a user-facing grouping and admission-fencing boundary.

### Proposed new or revised records

```text
Conversation {
  ...existing
  owner_coworker_id?: CoworkerId  # immutable at creation; nullable for ordinary sessions
  last_activity_at: Timestamp     # durable server-derived projection/order, not clock supplied by client
  learning_policy_snapshot?: NO_LEARNING | COWORKER_DEFAULT
}

StandingResponsibility {
  responsibility_id: ResponsibilityId
  workspace_id: WorkspaceId
  coworker_id: CoworkerId
  current_revision: u64
  status: DRAFT | ENABLED | PAUSED | COMPLETED | EXPIRED | ARCHIVED
  created_at, updated_at, version
}
StandingResponsibilityRevision {
  responsibility_id, revision
  title, purpose, success_criteria[], stop_criteria[]
  linked_goal_ref?: GoalRevisionRef
  routine_refs: RoutineRevisionRef[]
  automation_refs: AutomationRevisionRef[]
  max_agent_checks?, expiration_at?, evaluation_window?
  usage_policy_ref?, notification_policy, action_authorization_profile_ref?
  authored_by, created_at
}

CoworkerCapabilityAssignment {
  assignment_id, workspace_id, coworker_id, connection_or_capability_ref
  allowed_resource_scopes[], allowed_operation_classes[]
  eligible_runtime_scope?, learning_eligibility
  status: ENABLED | PAUSED | REVOKED
  current_revision, version
}

CoworkerMemoryPolicy {
  mode: OFF | AUTOMATIC_PRIVATE | AUTOMATIC_WITH_APPROVED_SHARING
  eligible_source_kinds[]
  disabled_for_temporary_conversations: true
  extraction_budget?, retention_policy_ref?, notification_mode: QUIET
}

MemoryCandidate { # proposal/work queue; not automatically an authoritative fact
  candidate_id, workspace_id, coworker_id, source_refs[]
  source_digests[], extractor_identity, proposed_scope, classification
  content_ref, status: QUEUED | EVALUATING | ACCEPTED | REJECTED | CONFLICTED
  dedupe_key, created_at, expires_at, version
}

MemoryRecordMetadata { # on versioned Resource ContextDocument, not a second blob store
  scope_owner_ref, learning_method: EXPLICIT | AUTOMATIC
  support_refs[], confidence_label?, observed_at?, review_after?, supersedes_ref?
  sensitivity_tier, extraction_receipt_ref, access_policy_version
}
```

**Additional required invariants:**
- Coworker-owned Conversations must belong to the same Workspace. `owner_coworker_id` cannot be reassigned silently to another Coworker. Global ordinary conversation need not have one.
- Responsibility links must be same-Workspace and Coworker association; each Automation pinned `coworker_ref` must be compatible, and admission checks BOTH Coworker and Responsibility status for new proactive Tasks. Existing Tasks unchanged when policies or grouping are revised.
- Edits use expected versions/RequestId and immutable revisions; enable/disable is atomic with trigger registration or durable reconciliable outbox. Never say `enabled` without successful admission/trigger host readiness confirmation. Roll back or show blocked when partially configured.
- Automatic memory commits need an auditable identity **and** policy snapshot; source never auto-authorizes its own memory proposal. Revocation blocks future retrieval; purge proof is statusful, may remain incomplete for external providers.
- Capability assignment is a *ceiling*, not a live grant; on invocation recheck connection auth, source scope, Workspace policy, TrustService, current Task lease, effective Coworker settings, and provider readiness.
- No tool/schema flood: CapabilityBroker progressive search/describe; actively selected subset only. Provider-specific direct attachment only when qualified; mediation for consequential actions and durable effects.
- Typed connection and UserRequest errors never ask the model to collect credentials in normal chat. Sign-in goes through provider-owned secure flow.
- Do not claim future reach from a sleeping/offline local-only Runtime.

### Events (proposed additions to existing EVENTS.md)

```text
conversation.coworker.assigned.v1       # only at creation; immutable origin
coworker.capability_assignment.created.v1
coworker.capability_assignment.revised.v1
coworker.capability_assignment.revoked.v1
coworker.memory_policy.revised.v1
coworker.memory_candidate.status_changed.v1
coworker.memory_record.committed.v1
coworker.memory_record.superseded.v1
coworker.responsibility.created.v1
coworker.responsibility.revised.v1
coworker.responsibility.status_changed.v1
coworker.responsibility.health_changed.v1  # only if derived change is persisted; otherwise omit
```

Every event has authenticated actor/service identity, Workspace and entity ID, `entity_revision`, immutable source references and timestamps, RequestId/causation/correlation, and redacted payload. Events never contain credentials, provider handles, native hidden reasoning, raw memory extracted from private sources, or OAuth callback URLs. Resource revision/create/status and existing automation/Task events continue to be emitted by their owners; no duplicate `task.started` from ResponsibilityService.

### Services and APIs (proposed)

```text
CoworkerService.create/revise/pause/archive
CoworkerConversationService.create/list/reopen/last_active
CoworkerAssignmentService.assign/revise/revoke/list
StandingResponsibilityService.prepare_draft/commit/enable/pause/resume/complete/list
StandingResponsibilityProjection.get_detail/list_runs/get_health
MemoryLearningPolicyService.get/revise
MemoryCandidateCoordinator.enqueue/evaluate/reconcile/commit/expire
PersonalContextService.retrieve_scoped/list_sources/revoke
TriggerCoordinator.claim/observe/misfire_reconcile
TaskService.admit/resume/pause/cancel/verify
NeedsYouService aggregate UserRequests/Approvals
```

Suggested UI route families:
- `/v1/coworkers/{id}/conversations`, `POST /v1/coworkers/{id}/conversations`
- `/v1/coworkers/{id}/capability-assignments`, `POST/PATCH/DELETE`
- `/v1/coworkers/{id}/memory-policy`, `GET/PATCH`
- `/v1/coworkers/{id}/memory`, `GET` from filtered ContextDocuments; `PATCH/DELETE` through ResourceService semantics
- `/v1/coworkers/{id}/responsibilities`, `GET/POST` (draft and commit separated)
- `/v1/responsibilities/{id}`, `GET/PATCH`, `/pause`, `/resume`, `/run-now`, `/history`
- `/v1/coworkers/{id}/activity` read-only composite projection of actual events/tasks

Do not override currently documented concrete API paths, IDL or contracts; exact REST surface must be reconciled with `docs/schemas/operator-api.openapi.yaml`. Host-local config and secrets never enter replicated API objects.

## 9. Complete critical flows

**F-C01 Create Quick:** user submits identity/purpose + RequestId → validate Workspace/name/version → create CoworkerRevision; default eligible lead binding or null → create first Conversation referencing Coworker → open composer. If creation response times out, replay same RequestId returns same Coworker+Conversation; never duplicates.

**F-C02 Create Advanced:** stage optional agent/worker scopes, memory settings, connectors (each authenticated independently), responsibilities as *drafts* → final review commits Coworker → separately confirms enabling standing triggers only after dependency checks. Partial OAuth failure does not erase Coworker draft; no implicit enabled Automation.

**F-C03 Add on demand:** Coworker says it needs Calendar → present provider `Connect` entry → user signs in via provider secure flow → attach existing Connection to Coworker policy → readiness check → original waiting Task revalidates current scope and resumes when safe. Do not repeat action until idempotency/Effect reconciled.

**F-C04 User creates recurring work:** user asks to check daily + brief weekly → native chief proposes responsibility with Goal+Routines+Automations → host validates recurrence, timezone, sources, budgets, source and execution placement → user enables → atomic durable snapshot/outbox → each trigger fires independently through standard TriggerCoordinator and materializes pinned Tasks.

**F-C05 Proactive signal:** connector event authorized → normalize event/cursor → deterministic filter/dedupe → within standing budget? if no meaningful change, NO_ACTION record and no notification; if interest, propose Suggestion; if explicitly preauthorized exact action, commit a Task through normal path. Reader content cannot issue an authorization instruction.

**F-C06 Needs You:** Task blocks on credential renewal/permission/choice → persist UserRequest or Approval → notifications deduped; user returns after hours → answer attached to exactly same Task and scope; reauthorize/reconcile; don't rerun completed Steps blindly.

**F-C07 Offline/restart:** laptop sleeps before 9AM task → host resumes 1PM → cursor reconciles misses and applies configured RUN_ONCE/SKIP/BOUNDED; locked desktop work waits unlock; no fake 9AM completion. If cloud TriggerHost eventually exists, remote due occurrence can be recorded while waiting for local files.

**F-C08 Crash during send:** email request dispatched; host crashes before ack → Effect AMBIGUOUS; query provider sent-items/idempotency receipt → if proven sent, settle with actual evidence; if proven absent, safe retry; if unknown, Needs You. Never resend blindly.

**F-C09 User edits scopes:** revoke Drive while task running → no new reads/invocations; active Task follows trust cancellation/blocked policy; no permission resurrection after resume. Coworker memory already obtained is distinct and requires explicit memory deletion/revocation rules.

**F-C10 Switch chief agent:** update Coworker future default → next turn or Task uses new eligible binding; existing Tasks keep their current AgentBinding. Context packet uses authorized LiteCowork memory and exact sources; do not claim underlying native session secrets or transcripts transferred.

**F-C11 Routine drift:** source or skill manifest changed → mark degraded, require new integrity/authority review before re-enabling affected action; do not silently update pinned Routine/Skill or skip verification.

**F-C12 Pause all:** block new proactive admissions with fenced status version; request per-Task graceful pause; render `Pausing safely` until Attempts, Effects, Invocations reach safe state; not instantaneous hard kill.

**F-C13 Agent output evidence:** a run says 'done' but requested file missing → verification fails/incomplete; repair within budget with evidence; otherwise notify Needs You. Never treat agent tool completion or model claims as sufficient outcome verification.

**F-C14 Notification delivery:** Task done → create idempotent notification policy decision → suppress low-significance results per preference → report meaningful result once to chosen channels, with timestamp and exact linked run; channel outage retries don't duplicate actions.

## 10. Verification and quality gates

**Core safety and correctness:**
- Ten or 10,000 installed capabilities do not expand per-session tool list without search/describe; no arbitrary product-imposed count cap; measure latency/CPU/OS process limits instead.
- No unintended network/process activity for idle Coworker with no due triggers.
- Auth expired, absent model, unavailable app, permission denied, unsupported trigger/adapter, or root revoked produce distinct honest blockers.
- Trigger event duplications, restart, cursor rollback, host-epoch races, DST ambiguous times and missed slots produce zero duplicate Task identity; bounded catch-up.
- Signature/replay validation on external events. Trigger inputs and malicious emails/websites cannot request unauthorized grants, acquire credentials, or grant permanent memory-sharing.
- No token/credential values, native hidden reasoning, agent-private transcripts or provider cursor blobs in shared events/logs/backup/UI.
- Two Coworkers share one installed connector without duplicate install; revoke assignment A does not uninstall connection B; disconnect globally shows dependencies.
- Automatic private memory never leaks across Coworkers without explicit shared scope; temporary chats never enter learning queue. Changed/revoked ContextDocument cannot reappear through cached retrieval after policy barrier.
- Automation allowed to execute only after actual source readiness, Task budget admission, and live Trust checks. Consequential sends/payments/deletes do not bypass required approvals; preauthorized recurring action must match exact policy.
- Native unmediated calls are labeled honestly; never claim Core-enforced provenance where no mediation exists.
- User can close window; daemon remains under per-user background mode. Lock/sleep/shutdown semantics tested separately on Windows/macOS/Linux.
- Crash during externally ambiguous effect never auto-repeats without provider reconciliation and policy.
- Pause new work does not cancel existing Task; Pause all safely requests real Task transitions, surfaces unsettled blockers.
- Derived completion uses Artifact/Evidence/version/current input checks. Partial/inconclusive findings are not marked verified.

**Consumer UX:**
- New Coworker in seconds, zero connectors, no mandatory wizard for simple users; Advanced path skips sections and supports draft recovery.
- Existing Coworker reopens most recently active chat across app restart; + New chat clean but retains authorized persistent context; chief/model/effort selectors are accessible and use real catalogs.
- Two ordinary-user tests: create Research Assistant, attach calendar when first needed, approve meaningful action, get linked report; create Travel Assistant, attach relevant sources, request itinerary, open output in right Workbench without task dashboard hijack.
- One power-user test: 100+ configured MCP/tools/skills with bounded discovery and no flood; specialist Coworkers retain isolation; native and host-delegated worker task authority is verifiable.
- Notifications distinguish completed work from waiting, blocked, unavailable, Needs You, no-change; no fake heartbeats or role-play presence.
- Accessibility: screen-reader semantics, keyboard navigation, focus restoration, reduced motion, responsive Workbench and no confusing destructive button hierarchy.
- Threat tests: prompt injection, external API rejection, file path races, stale approvals, OAuth expiry, partial connected-app outage, stale memory conflict, changed Skill digest, concurrent agent switch, partial background startup.

**Measured release gates:** zero unauthorized cross-scope memory disclosures or capability grants; zero duplicate consequential actions in seeded replay tests; all approved Schedule/misfire test vectors; all background lifecycle paths verified per platform supported; p50/p95 per-category trigger-to-admit and task completion, notification relevance/false positives, retry and verification precision, memory extraction budget per Task. Report factual coverage rather than claiming universal ability.

## 11. Documentation and implementation change map

**Rewrite/extend normatively:**
1. `docs/PRODUCT.md` — session-first ordinary Home and optional Coworkers, desktop autonomy promise and restrictions.
2. `docs/EXPERIENCE.md` — replace dashboard-first Coworker page and required default-Coworker composer; add full Create/Chat/Responsibility/Connections/Memory UI, notifications/quiet behavior, Workbench invariants.
3. `docs/DESIGN-SYSTEM.md`, `docs/PRESENTATION-RUNTIME.md`, `docs/MOTION.md` — detailed sidebar/header/composer/Workbench tokens and truthful progress/activity.
4. `docs/RESPONSIBILITIES.md` — add StandingResponsibility grouping and admissions, Coworker conversation association, scoped automatic memory policies, source learning rules, assignment ceilings.
5. `docs/AUTOMATION.md`, `docs/ROUTINES.md` — responsibility linkage, trigger semantics, condition/no-action gates, dedupe, host health, user-readable review.
6. `docs/CONTEXT.md` — bounded scoped retrieval, automatic-memory candidate lifecycle, context-used receipts and retention/deletion behavior.
7. `docs/CAPABILITY-FABRIC.md`, `docs/CAPABILITY-INVOCATIONS.md`, `docs/TRUST.md`, `docs/NETWORK-SECURITY.md` — assignment projection vs grants, onboarding, native bypass limitations and approvals.
8. `docs/RUNTIME-LIFECYCLE.md`, `docs/FAILURE-RECOVERY.md`, `docs/TASK-RUNTIME.md`, `docs/ENVIRONMENTS.md` — background-startup, sleep/lock/wake, checkpoint, true effect settlement.
9. `docs/DATA-MODEL.md`, `docs/SCHEMAS.md`, `docs/EVENTS.md`, `docs/API.md`, `docs/schemas/operator-api.openapi.yaml` — typed records/commands/events and implementation conformance.
10. `docs/FLOWS.md` — new F-C01..F-C14 with detailed actor calls and failure paths; keep existing F18/F19/F40/F41/F55/F72 and reconcile conflicts.
11. `implementation/UI.md`, `implementation/SCOPE.md`, relevant platform/trigger/capability test plans — staged delivery and explicit proof of running code vs desired contracts.

**Priority sequence, without artificially shrinking the final design:**
- **Foundation:** Conversation ownership/persistent recency, Create UX, chief/worker selection, connected-capability assignment, low-friction native auth, read-only memory retrieval, clean Workbench/chat.
- **Reliable desktop autonomy:** OS background supervisor, concrete supported TriggerHosts, durable Routine/Automation/Occurrence, proper Tasks, retry/effect reconciliation, Needs You, honest outcome cards and desktop notifications.
- **Persistent intelligence:** automatic quiet scoped memory, dedupe/corrections/deletion, outcome learning; configurable standing responsibility grouping and deterministic proactive Suggestions.
- **Broad integrations and expansion:** qualified cross-app/local computer workflows, richer browser/office/media artifacts and semantic providers; future remote/cloud execution path through existing Runtime/Environment model.

## 12. Competitive reference, as of October 2026

- OpenAI Dots: persistent always-on identity, cloud computer, connected apps, proactive read-only research, Custom Rules, background work/approvals. https://openai.com/index/introducing-dots/ and https://help.openai.com/en/articles/20001530-getting-started-with-your-dot
- Grok Bot: persistent Bot roster, chats/skills/routines, event triggers and human-in-loop; caution that Bots share a computer and not every resource is isolated. https://x.ai/news/designing-grok-bot and https://docs.x.ai/grok-bot/skills-routines-and-automations
- Meta Muse: goals, proactive suggestions, connected apps, actions, audit and approvals. https://ai.meta.com/muse/
- Claude Cowork: prompt-to-scheduled task and independent run sessions; newly remote scheduled work. https://support.claude.com/en/articles/13854387-schedule-recurring-tasks-in-claude-cowork
- Kimi Work: browser/desktop and local scheduled tasks, but missed triggers during app close/sleep not replayed. https://www.kimi.ai/help/features/scheduled-tasks
- OpenClaw: configurable recurring heartbeat, monitor scratch, cron/event automation and background gateway. https://docs.openclaw.ai/heartbeat
- Eigent: schedule, webhook and app event automation. https://test.eigent.ai/docs/scheduled
- OpenWork: native OpenCode-based desktop app with skills/MCP/providers; don't infer durable background correctness from a README. https://github.com/different-ai/openwork

**Final contract:** A Coworker is a persistent identity that owns **responsibilities, permitted context, and access preferences**; it **does not own a new AI execution engine**. The selected native harness reasons and works, the LiteCowork substrate admits/checkpoints/reconciles/verifies, and the user experiences one clean, trusted, long-lived assistant across chats, integrations, desktop work and future cloud placement.

## 13. Completeness decisions: discoverability, coworker-to-coworker handoff, portability and advanced autonomy

### A. Autonomy levels, not permission-toggle walls

The general conversation composer never exposes an enterprise permissions grid. User-facing Coworker setting has three simple explanations while actual grants stay policy-enforced:

- **Ask me:** perform user-requested Tasks, show suggestions, no unattended triggers.
- **Keep an eye on things:** enabled, approved read-only monitors may run and notify when relevant; no unrelated mutations.
- **Take care of approved routines:** permitted standing responsibilities may execute scheduled/event-driven Tasks and authorized scoped actions while honoring separate Trust approvals.

These are explanatory presets with exact policy compilation, not universal authority tokens. New Coworkers start in **Ask me** until an explicit responsibility is enabled. Automatic *memory* is independent of automatic *action*. If the user confirms one narrow recurring routine, do not implicitly enable unrelated proactive background observation.

### B. Coworker-to-Coworker collaboration

Coworker identity and external native-agent worker profile are different entities. A Coworker is not silently another worker executable. To collaborate:

1. User may ask `Research Assistant, share the approved briefing with Writing Assistant and ask it to prepare a draft`.
2. Chief proposes a scoped handoff to a **new Task associated with Writing Assistant**, with exact source Artifacts, deliverables and chosen permissions. No private memory is implicitly transferred.
3. The receiving Coworker's selected chief runs under its own eligible AgentBinding, ContextPolicy and Task permissions.
4. A durable `CoworkerHandoff` (origin/target, Task refs, source attachments, status, approvals and evidence) is a relationship/projection, not a new agent-to-agent transport or planner. No circular automation chains without lineage/depth limits.
5. Show a quiet `Handed off to Writing Assistant` item in both Coworkers' activity, with same linked canonical Task. The user can inspect exactly what context moved.

**No unreviewed group shared memory.** Group chats/rosters are a later optional UI view over scoped Conversation participation and explicit shared Resources, not automatically a merged memory pool or universally shared desktop credentials. The same principle applies if a user shares a Coworker configuration with someone else: export instructions and package *references*, not tokens, private memories, logins or OS paths. Imported Coworker remains disabled until binding/connectors and permissions are re-established.

### C. Templates and discovery

The empty Coworkers screen should offer **Create from scratch**, **From a successful conversation**, and **Use a template** (examples: Researcher, Travel planner, Office helper, Project coordinator, File organizer). Templates are editable examples of role descriptions, suggested Routines and optional tools; they MUST NOT pre-authorize accounts, auto-enable monitoring or silently install packages. Created from conversation extracts a redacted proposed purpose/rules/eligible inputs for owner review; no indiscriminate transcript dumping. Full on-boarding respects simple/advanced paths.

### D. User correction, feedback and learned skills

On a completed run: `Useful`, `Not relevant`, `Fix result`, `Don't do this again`. Feedback may update Coworker memory, notification thresholds, or produce a reviewed Routine/Skill revision. It cannot silently authorize expanded scopes or publication of executable code. Reusable successful workflows become **SkillProposal** only after redaction, testing and explicit approval, using existing LiteSPM package lifecycle for actual installation.

### E. Generated missing capabilities

If the chief needs an unavailable capability, it may propose **Connect an existing tool** or **Build a reusable skill/integration** through a reviewable capability-development Task. A generated connector or script is not immediately trusted: treat it as untrusted code, isolate/test it, require publisher/source review and the normal package/secret/authorization checks. The user never has to see package manifests on normal flows, but the system must not bypass integrity because an AI authored the file.

### F. Channels and remote control

A Coworker can have communication bindings (desktop notifications, app chat, eligible Slack/Telegram/email or other supported channels), with authenticated source identity and assurance. Channel messages map to existing Conversation or Task; they do not create a shadow Bot executor. A weakly authenticated remote message cannot approve highly consequential desktop actions. From an off laptop, remote channels may receive preexisting messages through a reachable provider, but cannot cause an offline-only Runtime to execute. Future cloud/always-on node uses the same ChannelBinding and Task structures.

### G. Product trust details

Users need global `What is running?` and `Stop unattended work` controls with an honest preview of affected triggers, Tasks, native processes, and unsettled Effects. `Delete Coworker` is not an immediate synonym for deleting conversation history, documents, credentials, shared skills or other Coworkers' connections. Support **Archive**, optional selective export, and deliberate destructive deletion with separate data categories and provider cleanup receipts. Inactivity policy may suggest reviewing old background responsibilities, but never silently re-enable them after an explicit pause.

### H. Additional tests

- Collaboration handoff transfers only selected Artifact versions/Task inputs, not source Coworker's private memory or implicit authority. Replayed handoff doesn't produce duplicate Tasks.
- Template import creates inert eligible configurations with disabled triggers and unset secrets; malicious template cannot grant scope.
- Generated skill/plugin cannot activate before manifest/digest, scope and Trust requirements; a source code update revalidates its grants.
- Channel identity spoof and weak-origin remote approvals fail closed; notification delivery idempotency verified.
- Creating/deleting/archiving a Coworker has no destructive effect on independently owned linked resources or globally installed connectors unless the owner explicitly selects such actions.
- Distinguish automatic memory learning from proactive execution: turning one on doesn't turn the other on.