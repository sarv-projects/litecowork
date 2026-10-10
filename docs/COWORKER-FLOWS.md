# Coworker target interaction flows, failure paths and acceptance

**Status:** accepted design target, not implemented API. **Implementation guardian:** E09-S02 (cross-cutting E08-S01/S03, E04 and E10). Complements [FLOWS](FLOWS.md), [AUTOMATION](AUTOMATION.md), [RESPONSIBILITIES](RESPONSIBILITIES.md) and [UI](../implementation/UI.md). Existing numbered F18, F19, F40, F41, F55 and F72 remain authoritative for canonical Trigger/Task/Effect transitions; the C-series adds Coworker relationship and UX requirements without replacing them.

## CF01 — Quick Create to chat

Precondition: active authorized Workspace; Coworker optional. User enters name and purpose and presses Create. UI retains RequestId across lost receipts, calls CoworkerService.create with default eligible native lead binding reference only when that binding exists, commits revision; CoworkerConversationService then creates a same-Workspace Conversation with immutable owner and opens it. If session admission is not implemented, surface real identity and explain chat unavailable; do not fabricate an Agent reply or Conversation. On timeout retry exact idempotency key; 409 keeps draft; no lead means repair with draft retained, not auto-switch. New Coworker defaults to no approved standing responsibility, private scoped memory policy, and no auto-provisioned connectors. Creation and conversation record are either combined with a durable repair cursor or reported as two independently recoverable committed operations.

## CF02 — Customize and skip optional steps

User may choose chief/model/effort from actual native adapter profile, permitted external worker profiles, eligible knowledge sources, optional apps/MCP/skills/local apps, memory policy and draft responsibilities. Each installed/connected source has its own secure owner flow and is not guaranteed ready merely because catalogued. Skip bypasses optional onboarding without silently granting rights, installing packages, or enabling automations. Save/return retains draft as a local temporary draft where supported; otherwise leaving warns about unsaved changes. Changing Workspace mid-flow aborts stale requests and blocks cross-Workspace binding. Confirmation creates Coworker identity, but enabling an unattended responsibility always requires separate exact reviewed acceptance.

## CF03 — Task-time missing connector

An active Task needs Calendar. CapabilityBroker returns typed not-connected blocker and persists UserRequest referencing exact Task/Attempt/service/version; UI displays Connect Calendar. Owner chooses provider system-browser OAuth (no passwords entered in chat), secure connection check and Coworker assignment. TaskService rechecks original Task, current Workspace, approval/grant, lease/Effect state, source freshness and provider readiness, then resumes only permitted unfinished work. Auth cancel/denial retains blocker. Global connector already installed -> reuse eligible account without reinstall. Disconnect one Coworker does not remove another's account. Ambiguous external operation is reconciled before repeat.

## CF04 — Scheduled responsibility proposal and enable

User requests daily watch and Friday briefing. Native chief proposes typed Goal + multiple Routine revisions and Automation trigger definitions; Core verifies syntax, timezone/DST, qualified TriggerHost, monitored inputs, actions, max usage, overlap, stop criteria, health and source authorization. Review card shows What/When/Where/Needs You; owner explicitly confirms. StandingResponsibilityService atomically records versioned grouping/authorization with outbox for trigger hosting, or leaves it paused/blocked until readiness; do not show Enabled early. Each AutomationOccurrence creates one canonical Task under existing admission, with exact pinning to revisions. No second scheduler or model planner.

## CF05 — Proactive event/watch without model heartbeat flood

Authenticated connector event, Resource watcher or due condition enters qualified TriggerHost. Verify signature/cursor/host epoch, deduplicate source ID/key, deterministic filter/threshold/cooldown; if no change, record a supported observation outcome without model call or notification. If meaningful, verify preauthorized source and action policy: a read-only investigation may admit a bounded Task; new authority can create only a Suggestion/Needs You. An email, web page or skill body cannot grant itself execution/learning privileges. Event feed gaps are explicitly recorded; no fake “nothing changed” when nothing was checked. Thousands of configured tools are lazily discovered, not started.

## CF06 — Needs You and durable continuation

Owner must approve a consequential action, supply a preference, unlock desktop or reconnect an app. Persist exact UserRequest or Approval with expiry, action target, assurance and Task identity; dedupe notifications. The Task stays waiting without the chat window. When user returns, verify exact principal and non-expired version, attach response once, revalidate pinned grants/lease/Effects, and continue only unsettled Steps. Stale/no longer eligible request shows completed/expired with fresh repair. Credentials go through provider secure flow, never collected in ordinary Message content.

## CF07 — Window close, daemon restart, sleep/wake/offline

A Coworker has a Friday trigger and open Workbench document. Closing app window does not stop daemon if separately consented background policy; unsaved local editor draft follows its own save/close behavior. Lock may block GUI control; APIs may continue. Sleep/power-off stops local actions, leaving gap and missed due slots. On resume new Runtime incarnation reconstructs durable Task/trigger state, validates provider cursors/resources/grants/leases/Effects and applies SKIP/RUN_ONCE_WHEN_AVAILABLE/CATCH_UP_BOUNDED. No fake due-time completion. A cloud TriggerHost could later record due work but cannot access offline local-only files or browser.

## CF08 — Crash during external send

Effect proposed and exact-action admitted atomically with Invocation, Trust, grants/approvals, lease, parent and journal. External provider may have performed send, daemon crashes before receipt. On restart mark AMBIGUOUS, query provider using recorded idempotency/lookup; if proven succeeded, settle with evidence; if proven absent and policy permits, safely retry; if unknown, show Needs You and suppress automatic repeat. Never infer cancellation from process exit or send twice because a schedule retried. No provider call at all if atomic admission layer remains unavailable.

## CF09 — Source scope revoke

Owner removes Research Coworker's Drive assignment during Task. Assignment revision is persisted, future invocations check new scope and fail closed. An already dispatched action requires Effect reconciliation and Task safe stop policy. Shared installed Drive connection remains valid for other independently authorized Coworkers. Context and memory already extracted require their own deletion policy; revoking connector alone does not claim to erase native agent prompts already sent. Task never upgrades existing scope when a new resource appears.

## CF10 — Chief switch across chats

Owner changes selected native chief or default effort/model. New turn selects supported eligible AgentBinding and creates a separate AgentSession where required; existing Task pins and Attempt leases remain. LiteCowork Resource-backed memory is retrieved per Coworker policy and exact current revision; no hidden native transcript, model reasoning or provider credential is copied. If new provider fails, preserve user draft, show precise blocker and offer reselect; do not silently choose another model or treat previous session as transferable.

## CF11 — Source/skill/provider drift

Manifest digest changes, native agent model no longer listed, installed local app loses automation provider, login expires, or MCP task extension changes. Capability catalog records status as UNKNOWN/DEGRADED/INCOMPATIBLE with observation time, invalidates affected activations or queued admissions, and leaves existing Tasks pinned with a blocker. Owner can inspect package source and reconnect/review, but no automatic update of skill instructions or broadening of grants. A discarded incompatible setup must not remove saved Work/Conversations.

## CF12 — Pause new and Pause all safely

Pause new work changes Coworker or StandingResponsibility admission state with expected version and durable event. Already committed running Tasks continue. Pause all safely additionally requests TaskService pause for each linked current Task and shows Pausing safely until every eligible Attempt, Invocation and Effect reaches safe state. Unknown external effects remain blockers; native process kill alone is insufficient. Resume requires fresh trigger health, source access, native agent eligibility, policy and account revalidation. Archive remains separate and checks independently owned records.

## CF13 — Evidence-bound report and Workbench

A weekly Routine says the report is done. TaskService requires expected ArtifactVersion, exact source digests, required file/media type, accepted verification criteria, and reconciled Effects before claiming VERIFIED. Missing file -> Not finished; bounded repair or Needs You. Successful result creates a concise linked outcome in Coworker's conversation activity but not a fabricated chat assistant turn. Clicking output opens exact ArtifactVersion in resizable right Workbench. Dirty edits prompt Save/Discard/Stay before switching; restore always appends new version, not rewriting history.

## CF14 — Notification relevance, quiet hours and dedupe

A Task or monitor outcome emits real settled event. NotificationService evaluates exact Coworker preference and user quiet hours once per event/recipient, persists dedupe key and delivery status, sends to eligible authenticated channels. Routine no-change check is quiet. Needs You stays discoverable even when push suppressed; important success may be batched into digest. Delivery retry cannot create another Task or repeat external action. Channel outage is a notification state, not a Task failure; read receipts do not automatically approve actions.

## CF15 — Coworker-to-Coworker handoff

Research Coworker prepares report, user asks Writing Coworker to draft announcement. Chief proposes recipient Coworker, selected immutable ArtifactVersions, new Task objective, authorization and budget. User confirms scoped handoff. New Task pins recipient Coworker, its chief and independent grants, and source references only; private memory and native agent tokens are excluded. Replayed handoff uses idempotent receipt; cycles/depth/usage are bounded and visible. Each Coworker's activity references the same canonical recipient Task without cloning it.

## CF16 — Template import and generated integration

Template and conversation-derived Coworker settings remain inert reviewable proposals with empty secrets and disabled unattended triggers. The user can accept role and suggested tools, but must authenticate accounts and enable schedules separately. A generated MCP/skill requires manifest digest/source trust check, isolated verification, standard package installation and normal Scope/Trust before use. A malicious template/skill instruction cannot auto-grant disk roots, memory sharing, remote channel approval or unlimited background checks.

## CF17 — Memory extraction/quiet review/conflict and deletion

An eligible committed Conversation turn or independently verified completed Task result enters bounded candidate extraction. Sensitive/third-party content and opted-out temporary chat are excluded before any model call or persist. Qualified agent returns candidate classification, evidence digest, source ref and scope; deterministic policy validates trust, existing memory versions, consent, budget, and freshness. Private eligible nonconflicting candidate becomes a versioned Resource ContextDocument silently with a quiet indicator only when significant. Correction supersedes old revision; conflicts remain reviewable instead of overwriting. User can revoke or delete: immediate new-read barrier, pending provider purge saga, visible receipt state, fresh native session after revocation.

## CF18 — Global stop and delete/archival

Global Stop unattended work first lists active schedules/monitors, currently admitted Tasks, native processes, Runtime dependencies and ambiguous external Effects. Confirmation applies an explicit new-work fence and appropriate safe Task pause, not a blind process kill. Deleting or archiving Coworker independently previews chat retention, private memory, shared connections, Artifacts and subscribed triggers; it does not delete all linked records by default. Deletion reports external provider purge incomplete when true. Other Coworkers' legitimate access stays intact.

## Cross-flow invariants and tests

Test every control with: empty/loading/success/stale revision/offline/partial provider result/missing consent/cancel/failure/duplicate RequestId/late response. Verify same-Workspace ownership and pinned revisions, no leaking private Coworker memory, no automatic permission gain from Skills, and truthful Unknown/Blocked states. UI tests cover keyboard, screen reader, focus return, dirty editor, reduced motion, 200% zoom, large inventories, contextual Connect repair and no ghost chat. System tests cover real close/restart/locked/sleep states, duplicate webhook, DST and ambiguous sent Effect on each qualified OS/provider. See [UI](../implementation/UI.md) and [target data model](COWORKERS-TARGET.md).


## CF19 — Coworker switches chief agent without losing native functionality

1. Owner opens Coworker Settings > Chief agent or uses the chat Agent override.
2. UI lists enabled lead-eligible bindings, not registry entries or raw provider accounts.
3. Selected binding's fresh AgentControlDescriptor is loaded.
4. Model/Reasoning/additional session options and native slash / @ / input capabilities
   rebuild from that descriptor.
5. Coworker memory/knowledge/responsibilities do not move into the agent's native memory
   or provider configuration.
6. Existing durable Tasks keep their pinned lead/Attempt provenance; new chat turns/new
   work use the new selection.
7. If the new agent lacks support for draft inputs, draft remains and Send is blocked with
   exact unsupported items.

## CF20 — Coworker encounters agent setup drift

1. Coworker has a configured AgentBinding.
2. Native agent is updated, signed out, provider key expires, or option schema changes.
3. New turn admission detects stale descriptor/auth/configuration.
4. No silent fallback occurs.
5. UI shows **Needs setup** on the agent control and a direct action to the exact Agent
   Registry detail.
6. User signs in/updates/configures through the adapter-owned flow.
7. Descriptor refresh succeeds; original draft remains intact and may be sent.
8. Active Tasks already admitted under the old descriptor follow normal settlement and
   are not rewritten.
