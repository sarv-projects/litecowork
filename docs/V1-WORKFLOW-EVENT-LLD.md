# V1 Workflow, Cross-Agent Handoff, Automation, Events and Recovery LLD

Status NORMATIVE TARGET. Owners E03/E04/E05/E08/E09/E10, not evidence of served endpoints. Canonical owners: TASK-RUNTIME, DELEGATION, AUTOMATION, CAPABILITY-INVOCATIONS, EVENTS, STATE-MACHINES, STORAGE, CONNECTORS-WORKFLOWS, RUNTIME-LIFECYCLE and TRUST. This is their cross-surface implementation map, NOT a new orchestration engine.

## Unified entry points

~~~text
New Chat -> selected native AgentBinding -> ConversationTurn
"Run as Task" -> TaskSpecRevision + lead planning -> accepted PlanRevision -> Steps
Coworker chat -> CoworkerRevision-pinned Conversation -> optional Task
Saved Routine -> immutable RoutineRevision; no execution
Enable Automation -> active trigger policy -> Occurrence -> Task on admitted trigger
External event -> verified connection/event subscription -> dedup -> Occurrence -> Task
Task Continue -> canonical follow-up Conversation with Task context and exact references
Worker handoff -> accepted Plan Step + verified capability/agent selection -> child Attempt
One-off tool use -> Task/Turn-scoped CapabilityInvocation through granted Gateway
~~~

No duplicate Cowork/Code product, no independent shadow chat planner, no raw message interpreted as durable approval. Every process/event has a single authoritative owner. Ordinary Chat may be used without Coworker.

## Durable cross-module function contracts

| Service/function | Input/algorithm | Output and failure |
|---|---|---|
| createConversation | Workspace, optional Coworker revision, selected lead, RequestId | immutable Conversation identity, owner version; no Task |
| admitConversationTurn | Conversation, user input, exact binding/options, auth/trust, RequestId | one committed turn/message; blocked/ambiguous receipt |
| materializeTaskFromTurn | explicit Run as Task / accepted policy, objective, pins, limits | one TaskSpecRevision, no implicit dispatch |
| buildPlanningContext | accepted TaskSpec, source pins, policies/budgets | bounded immutable planning input, no Attempt |
| acceptPlanRevision | plan + TaskSpec version, Step dependencies, owner approval | atomic persisted PlanRevision and Steps |
| reserveStepAttempt | ready Step, binding/worker, Runtime/Environment, capability | admission ONLY if trust/effect/lease/sandbox guards pass |
| scheduleWorkUnit | eligible candidates, skills, quota estimates, deadlines, budget | deterministic selection plus reason; no unbounded model loops |
| startAgentSession | qualified exact harness, native config and auth | Runtime-private handle with public safe projection |
| normalizeAgentProgress | native event + attempt/session identity | ordered semantic/observed events; no fabricated verification |
| requestDelegation | accepted Plan Step, worker profile and grant constraints | child work with budget/parent provenance |
| checkStopCriteria | acceptance Evidence, limits, unresolved Effects, next candidate | COMPLETED/INCOMPLETE/NEEDS_USER/CONTINUE; no "done" from tool log |
| resumeOrRecoverTask | Task, prior lease/attempt/effects, checkpoint, Runtime incarnation | replay-safe new Attempt or explicit blocker |
| proposeCapabilityInvoke | connection/skill/tool, scope, effect class | permission/approval request, no direct unrecorded side effect |
| reconcileExternalEffect | idempotency/reconciliation key, provider observation | VERIFIED/AMBIGUOUS/FAILED with provenance; no blind resend |
| pauseTaskSafely | Task/revision, running action set | fenced future actions, pausing until safe point |
| switchLeadForFuture | user preference, binding qualification, current Task state | future-turn default or reviewed new Attempt; never teleport active provider |
| proposeRoutine | user draft/inferred suggestion, inputs, consent policy | inert proposal only |
| enableAutomation | immutable Routine/Automation revision, trigger and owner consent | activation only when host/source/grants ready |
| admitOccurrence | normalized trigger id + schedule slot/provider event identity | unique durable Occurrence + Task (if eligible) |
| reconcileMissedTrigger | durable host ledger, timezone/DST/policy | missed/skipped/catch-up as configured, no fake runs |
| continueAutomationConversation | pinned Task + originating owner, message | idempotent follow-up Conversation/turn; no hidden native session assumption |
| publishArtifact | source bytes, MIME, exact version, author, digest, RequestId | immutable ArtifactVersion receipt, verified separately |
| verifyOutcome | acceptance criteria, independent Evidence, effects, outputs | reported/observed/verified distinctions |
| updateNeedsYou | persisted UserRequest/Approval/Blocker + canonical status | deduped inbox state and notification eligibility |

## State machines and transitions

**ConversationTurn:** follow the exact canonical graph in [STATE-MACHINES](STATE-MACHINES.md), starting `OPEN -> RUNNING` and permitting `WAITING_USER -> WAITING_DEPENDENCY -> RUNNING`, terminal settlement and cancellation only along allowed edges. `CREATED`, `TURN_RUNNING` and `COMMITTED` are not canonical ConversationTurn states. Conversation creation and individual turn state are separate.

**Task:** follow the exact Task transition graph in [STATE-MACHINES](STATE-MACHINES.md). `Planning` describes the UI projection of `RUNNING` lead planning, not a persisted `PLANNING` enum. Not every listed state can transition directly to a terminal state; enforce explicit edges.

**Automation:** follow canonical enabled/paused/disabled/archived lifecycle in [STATE-MACHINES](STATE-MACHINES.md) and [AUTOMATION](AUTOMATION.md). Draft/review are interface proposal stages, not persisted `DRAFT` or `REVIEWED` Automation statuses unless separately promoted. Occurrences have distinct identities.

**Effect:** use only canonical Effect states/transitions in [STATE-MACHINES](STATE-MACHINES.md). Terms like prepared, observed, ambiguous and reconciled in explanations can refer to different evidence/transport phases and do not automatically define enum values.

Browser: session capability and control epoch over existing Environment lease; user control transfer fences stale agent actions (CHROME-BROWSER-LLD).

## Sequencing and idempotency

Every mutating Operator command carries an Idempotency-Key mapped to the domain RequestId, If-Match for expected aggregate version, and X-Workspace-ID where scoped. The local IPC request_id is a separate per-exchange correlation ID; authenticated transport resolves actor identity. Admission writes receipt, outbox and domain record atomically. A timeout reuses the same Idempotency-Key for safe receipt replay and reconciliation; never create a fresh key to repeat an uncertain external effect. Replays check idempotency class and external provider observation. Stop/pause prevents new admissions; already-emitted external messages are not presumed reversible. App shutdown does not erase Task identity.

Events use existing envelope, registry, correlation/causation, version and Workspace authority. The following are target event families: conversation.turn.*, agent.session.*, agent.control_descriptor.*, task.plan.*, attempt.*, capability.invocation.*, effect.*, approval.*, artifact.version.*, automation.occurrence.*, trigger.*, runtime.lifecycle.*, browser.session/control/action.*, ui notifications. UI-only events (click/focus/pane resize) are NOT durable domain events by default. For any new event, add schema, enum, authorized producer, consumer, compatibility migration, replay fixture and redaction test to docs/EVENTS.md + schemas/domain-event.schema.json before sending it.

## Cross-agent memory and handoff

- Each provider's native session, context window, capabilities and token cost remain native. LiteCowork owns a bounded portable Task envelope: objective, accepted plan and Step, relevant source pins, pending approvals, evidence, artifacts, budget, last committed progress, blocker, any explicit user steer.
- Handoff compiler constructs a compact source-cited summary and explicit resumption instructions. Missing/unknown facts are labeled as unknown, not assumed from old transcript.
- Before scheduling new worker, verify source scopes, provider eligibility, auth, context size, model capabilities, worker limits and inherited grants. No provider-private hidden state migration.
- Native subagents vs LiteCowork host delegation are distinct. Qualify native subagent outputs and budget/accounting separately; no misleading global child tasks.
- Timeout/retry: checkpoint at confirmed boundaries only; fallback worker may resume with a new Attempt, never reuse a revoked lease.
- "Verification" cannot mean trusting the same worker's completion sentence. Independent observations/Artifact content/effect receipts feed actual acceptance.
- Scheduler control loop has finite budgets and stop reasons; throttle/no progress/duplicate outputs/quota exhaustion cause pause/replan/escalation, never infinite retry.

## Automation UI and event flows

1. User opens Automations -> + New -> selects existing Routine or creates draft from chat -> enters What/When/Where/Owner/Conditions/Actions/permissions/timezone/quiet handling/concurrency/limits.
2. Preview next runs using real timezone IANA/DST rules and provider availability; warnings for sleeping local device and unreliable time-sensitive triggers; no guarantee if host offline.
3. Review exact scope/connections/grants; Enable commits a new immutable version and schedules via qualified host; canceled review creates no active trigger.
4. Trigger observed -> authenticate source, normalize event, dedup by source event/slot, admit Occurrence transaction, evaluate overlap/budget/policy.
5. Eligible Occurrence creates Task; UI shows actually persisted Task and outputs; provider down/missing grant -> Blocked/Needs You; no "running" without admission.
6. Callback/incoming provider event may arrive twice/out of order; use subscription cursor/outbox and dedup, never double external write.
7. Run now creates a new explicit manual occurrence and leaves recurrence policy unchanged.
8. Pause blocks new occurrences; Pause all safely additionally requests Task pause; show pending status while external sends unresolved.
9. Continue on completed/blocked automation opens exact Task follow-up, preserving current Workspace/Coworker and source references.
10. Notifications use actual changed status, quiet hours and dedup; mute does not delete Needs You entry.

## Failure/race catalog

Lost return receipt; duplicated trigger; stale CAS; provider token revocation; network partition after external send; crash before/after outbox; Runtime incarnation replacement; sleep through schedule; DST fall-back duplicate slot/spring-forward nonexistent slot; clock skew; task accepted with later revoked Workspace; source revised after pin; file delete while renderer active; Worker profile updated mid-Attempt; session compaction/context overflow; provider quota changes; orphan browser process; extension install drift; owner denies approval after queued action; unauthorized cross-Workspace handoff; user edits before takeover returns; ambiguous Artifact publication; Agent model fallback mismatch; no resources ready; malicious tool output prompting escalation. Each maps to blocking state, safe retry or reconciliation; every repair action is backed by a real API and tested crash boundary.

## Representative implementation placement

- Domain: crates/domain-task, domain-responsibility, domain-workspace, storage-core.
- Durable adapters: crates/storage-sqlite (outbox, idempotency, revisions, events, effects).
- Operator: apps/litecoworkd/src/{conversation_operator,automation_operator,routine_operator,operator,runtime}.rs and dedicated agent adapters.
- UI: apps/litecowork-ui/src/{conversations,automations,routines,needs-you,tasks,presentation,agents}.
- Browser/Workbench: qualified Environment provider plus desktop bridge; proposal in CHROME-BROWSER-LLD.
- Tests: domain transition/property suites, SQLite fault injection, end-to-end native harness, observable user scenarios.

Implementation follows the existing backlog, not an unversioned alternate roadmap. New service names here describe responsibilities; they are not declared served API methods until promoted to OpenAPI and actual routing.

## Reference docs

- https://modelcontextprotocol.io/specification/
- https://agentclientprotocol.com/
- https://docs.opencode.ai/docs/server/
- https://developers.openai.com/codex/
- docs/AUTOMATION.md, docs/DELEGATION.md, docs/TASK-RUNTIME.md, docs/CONNECTORS-WORKFLOWS.md.

## Flow and failure/interleaving coverage owner

The source-pinned UI → Operator → service → durable receipt/event/outbox → native/provider observation → recovery → UI chains and competing interleavings are enumerated in [V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md). It contains 25 flow chains and 24 explicit race/partial-failure gates. A proposed chain is not a verified implementation, and canonical State Machine/Effect/Automation transition rules remain authoritative.
