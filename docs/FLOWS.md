# End-to-End Flows

Each flow's durable writes occur through the owning service. Domain events commit in the
same transaction as aggregate updates; UI is a projection and never drives truth
directly. The listed sequence is normative unless a linked owner contract is stricter.

## F00 — Workspace creation, replication policy, and archive

Actors: User, Operator UI, WorkspaceService, TrustService, RuntimeMesh.

**Desktop/local V1 scope:** creation uses `LOCAL_ONLY`; cloud replication policy choices,
RuntimeMesh transfer, and remote execution remain post-V1. If a previously stored
Workspace policy is non-local, Settings reports it as inactive and exposes an explicit
owner reset to `LOCAL_ONLY`. The generic policy-change steps below describe the future
replication flow, not an active V1 transfer path.

1. The user creates a Workspace. The desktop retains the same RequestId if the response is lost and the same name/policy is retried. WorkspaceService commits the chosen supported initial policy (default `LOCAL_ONLY`) and an empty selected-root set, then emits `workspace.created`; the idempotency receipt commits with the aggregate, event, and origin sequence. `SELECTED_FOLDERS` is rejected until roots exist.
2. The user adds persistent WorkspaceRoots for selected folders. A one-time message attachment does not create a root.
3. The UI explains each replication scope before the user explicitly enables cloud replication. `SELECTED_FOLDERS` requires one or more active WorkspaceRoot IDs in this Workspace; it follows new revisions under each root rather than pinning one snapshot.
4. WorkspaceService applies a versioned prospective policy update and stores the selected root IDs transactionally with `workspace.replication_policy.changed`.
5. A policy edit never erases already replicated bytes or grants permissions/secrets. Archive is accepted only when every Task is terminal and every Automation is disabled. WorkspaceService commits `ARCHIVED`; reads and existing authorized artifact/resource downloads remain available, while every domain mutation is rejected, including Task changes, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel materialization.

Events: `workspace.created.v1`, `workspace.replication_policy.changed.v1`,
`workspace.default_agent_binding.changed.v1`, `workspace.archived.v1`.

UI: show the selected policy and its scope; an archived Workspace is visibly read-only. No archive/delete animation occurs before the archive event is committed.

## F01 — Simple conversation, no Task

Actors: User, Operator UI, ConversationService, AgentTurnCoordinator,
AgentSessionSupervisor, active AgentBinding.

Preconditions: Workspace is ACTIVE; an enabled Conversation override or Workspace-default
AgentBinding and compatible AgentEndpoint are available; the user message and selected
context pass Workspace scope. If none is eligible, the draft is retained and the user is
directed to connect/enable/select an AgentBinding before a turn is created.

1. User submits a message. ConversationService appends the ConversationMessage and
   ConversationTurn before dispatch.
2. AgentTurnCoordinator starts a fresh `AgentSessionScope.CONVERSATION` session bound to
   this ConversationTurn;
   it has a `conversation_id` and no Task, Step, Attempt, lease, or Environment.
3. ContextService sends the current WorkspaceInstructionRevision, selected visible
   messages, and explicitly attached ResourceRefs. No task-only material is implicitly
   included.
4. The agent produces a response. ConversationService appends it with the producing
   AgentSession, AgentBinding, and ConversationTurn provenance. The coordinator observes
   adapter quiescence, closes the session, and releases its host-use reference before the
   turn settles. If the agent creates a structured UserRequest, the same close/release
   occurs while the turn waits; the UserRequest pins this exact turn/session, and the user's
   response first moves that turn to `WAITING_DEPENDENCY`; it continues only that turn in
   a fresh session after required provider input acceptance and session readiness, using a
   bounded Conversation projection that includes the request and immutable response.

Events: `conversation.message.added.v1`, `conversation.turn.created.v1`,
`agent.session.started.v1`, `agent.session.closed.v1`, `user.request.created.v1`,
`user.request.resolved.v1`, `conversation.turn.retried.v1`,
`conversation.turn.resumed.v1`, `conversation.turn.settled.v1`, and any applicable
`capability.invocation.*.v1`.

No Task/Step/Attempt/ExecutionLease is created. A Conversation-scoped Gateway credential
allows only the explicitly granted read-only methods. If the request needs durable
outcome work or a consequential action, ordinary Task materialization rules apply before
that work is admitted.

Failure behavior: a lost session marks the turn FAILED and retryable at the turn level;
it does not fail a Task. An explicit retry moves that same turn back to RUNNING, increments
its retry ordinal, and creates a replacement Conversation session. Prior partial agent
output remains provenance-tagged and is not treated as a final response.

UI: ordinary conversation; no Task card or fake Live Desk lane. Show the selected agent
only when the user opens advanced details.

## F02 — Conversation materializes a Task

Precondition: Task admission resolves an enabled, eligible AgentBinding from an explicit
request, selected Coworker default, or Workspace default in that precedence order. The
first configured choice is authoritative; if unavailable, return `AGENT_UNAVAILABLE`,
keep the composer draft, and create no Task or ConversationTurn. First-use setup runs
before the user resubmits.

1. User asks for outcome-oriented work or explicitly creates a Task from a message.
2. ConversationService persists the ConversationMessage.
3. TaskService creates Task + TaskSpecRevision(1), pins the current
   WorkspaceInstructionRevision, and binds the source message in one transaction; if the
   message and Task originate in one command, both commit together.
4. TaskService pins the selected lead AgentBinding and placement preference.
5. PlanningCoordinator starts a Task-scoped `TASK_PLANNING` AgentSession without an Attempt,
   lease, Environment, or consequential capability grant.
6. After the session is ready, Task becomes RUNNING. The lead proposes PlanRevision(1);
   TaskService validates/promotes it and materializes Steps. Only then are Step Attempts
   admitted.
7. UI message expands into a Task card only after `task.created.v1` is durable. The planning
   phase may show a concise “Planning” status but creates no Live Desk work lane until a
   Step Attempt exists. Ambiguous intent remains a Conversation or gets a clarifying
   question; Core runs no hidden intent planner.

## F03 — Local Task happy path

1. Workspace is ACTIVE and local-only or otherwise permits the selected local resources.
2. A lead planning session proposes a plan; TaskService promotes PlanRevision and materializes Steps.
3. Step becomes READY.
4. placement selects local Runtime + Environment.
5. Attempt + lease + Step ownership commit atomically in the Hub transaction.
6. AgentSession starts after commit; Attempt becomes RUNNING only on adapter readiness.
7. agent executes.
8. Artifact/Effect/Evidence are recorded.
9. agent proposes finish.
10. verification runs.
11. Task COMPLETED.

## F04 — Capability discovery and direct attachment

1. worker needs operation not currently available.
2. `litecowork.capabilities.search` -> CapabilityBroker.
3. Broker searches LiteSPM normalized catalog.
4. worker calls describe/select.
5. Broker resolves exact package version/digest.
6. Trust evaluates grant/approval.
7. provider installed/started on eligible Runtime if needed.
8. direct native tools only through a host-managed broker relay that records each
   CapabilityInvocation and enforces the session-scope grant plus required Effect/fence
   contract; otherwise select the Gateway proxy tool. An independently connected tool is
   agent-owned and outside those guarantees.
9. activation lifecycle status becomes `ACTIVE`; a separate health observation records `HEALTHY`.
10. worker invokes the capability; a direct mutation cannot bypass authorization or
    recovery. If neither path preserves required guarantees, the operation is unavailable.

UI: source/capability appears only after activation/use.

## F05 — Capability proxy fallback

Same as F04 through grant, but agent cannot hot-attach.

1. Gateway keeps capability behind `litecowork.capabilities.invoke`.
2. current turn uses proxy.
3. at safe next session boundary, direct attach may occur if supported.
4. Task does not restart solely for capability discovery.

## F06 — Approval-gated capability

1. grant request requires sensitive permission.
2. TrustService returns Approval(PENDING).
3. Attempt -> WAITING_APPROVAL.
4. UI shows exact requested action and scope.
5. user approves on adequate-assurance surface.
6. TrustService records the user decision. For this capability-escalation flow, the
   authorized command atomically creates the scoped CapabilityGrant and consumes the
   Approval in one ApprovalUse bound to that grant and the exact approved request digest.
   A later consequential invocation requires its own Effect admission and, if its risk
   policy requires approval, a separate ApprovalUse bound to that Effect. Attempt resumes
   only after activation and secret prerequisites are healthy.

Denied/expired approval produces policy failure or revised plan; no effect is executed.

## F07 — Host delegation

**Actors/preconditions:** Lead AgentSession, DelegationCoordinator, TaskService,
WorkerSelectionService, AgentSessionSupervisor, TrustService; Task is RUNNING under the
current parent Attempt/ExecutionLease. The request names a READY Step in the current
accepted PlanRevision. If there is no suitable Step, the lead proposes a PlanRevision
through TaskService; delegation cannot add a Step directly.

1. Lead submits `DelegateRequest` with objective, referenced inputs/Artifacts,
   acceptance criteria, required capabilities, optimization policy, isolation, budget,
   deadline, and optional preferred DelegationProfile ID.
2. Core verifies the active parent lease/fence, accepted PlanRevision and Step readiness,
   TaskSpec revision, ancestry/depth, Task/profile concurrency, Workspace policy, and
   remaining recovery/budget limits. It filters profiles by binding/profile status,
   adapter descriptor freshness, negotiated option support, Runtime/Environment/auth,
   Trust, and required features before ranking eligible candidates.
3. In one owning transaction, TaskService rechecks all mutable versions, reserves the
   child budget, issues only child-scoped grants under current policy, records the child
   Attempt pinned to the profile revision, and appends `delegation.admitted.v1`. It never
   copies parent Approvals or SecretLeases. Provider/process startup happens after commit.
4. Core builds a bounded TaskPacket from pinned Task/Plan/Workspace revisions, explicit
   input refs, decisions, child grants, and acceptance criteria. The selected adapter
   starts an independent AgentSession and Environment under a new ExecutionLease.
5. The child reports a ResultEnvelope and referenced output/Artifact/Evidence records.
   TaskService checks revision applicability, reconciles Effects, and runs the configured
   verifier. The lead receives the result with its source revision and verification state.
6. The lead may integrate, reject, or request a bounded new Attempt. A new worker/profile
   is a new Attempt; no existing Attempt changes agent identity in place.

No parent full transcript is copied by default. Native subagents reported by the lead
remain harness-owned and do not create host-delegated child Attempts.

## F08 — Child failure

1. child AgentSession lost or Attempt fails.
2. other independent child Attempts continue.
3. recovery ladder chooses resume/replace/new Attempt.
4. if child objective no longer worth retrying, lead revises plan.
5. parent sees structured blocker/failure, not a fabricated result.

## F09 — Task steering during execution

1. user sends steering message.
2. classify as task-spec change, plan hint, interrupt or chat-only.
3. spec change creates TaskSpecRevision(n+1).
4. lead AgentSession receives new revision.
5. relevant child Attempts are updated/cancelled at safe boundary.
6. results produced against stale revision are marked with their source revision and require applicability check.

## F10 — Cancellation

Actors: User, TaskService, PlanningCoordinator, AttemptRunner, AgentAdapter,
InvocationRunner, CompletionEvaluator, VerifierRunner, EffectReconciler, LeaseCoordinator,
ConversationService, TrustService.

1. User requests cancellation with the expected Task version. TaskService serializes this
   against completion on the Task aggregate version. If completion already committed, the
   cancel command returns that terminal result. Otherwise Task becomes `CANCEL_REQUESTED`,
   fencing CompletionEvaluator and admitting no new grants, Attempts, Effects,
   CapabilityInvocations, or VerificationRuns.
2. ConversationService and TrustService consume the Task status event and transition
   pending Task-scoped UserRequests and pending Approvals to `CANCELLED`. Their owner
   services make these changes; TaskService waits for their closure events. Responses
   committed before cancellation remain immutable, but any resulting provider work is
   still cancelled and reconciled.
3. TaskService rejects late plan proposals after cancellation wins the Task-version race;
   PlanningCoordinator asks AgentSessionSupervisor to close the active TASK_PLANNING
   session.
   Signal host-owned Attempts and ask AgentAdapters to interrupt/cancel.
4. Ask InvocationRunner to cancel each in-flight Task-planning/Attempt CapabilityInvocation.
   Provider acknowledgement records cancellation intent only; wait for authoritative
   provider terminal state. Conversation-scoped Invocations are outside the Task.
5. Reconcile open Effects and settle/abandon Attempts; terminate isolated Environments
   where policy allows.
6. Let active VerificationRuns settle against their pinned criteria and `ResourceInput` values;
   append their results/Evidence, but reject any late completion finalization because the
   Task is already `CANCEL_REQUESTED`.
7. Task becomes `CANCELLED` only after the planning session, Attempts, Invocations,
   VerificationRuns, pending Task-scoped requests/Approvals, and Effects have settled. If a provider may still be
   acting, an owner-service closure or verifier remains unsettled, or an Effect remains
   ambiguous, keep `CANCEL_REQUESTED` and show the blocker; do not claim cancellation.

Cancellation is valid from `PAUSED` and supersedes `PAUSE_REQUESTED`; a saved ResumePacket
remains history and is not resumed. A late Task-scoped user response after cancellation
is rejected and cannot start an AgentSession or provider delivery. Cancellation withdraws
an `AWAITING_RESPONSE`/`PENDING` local provider-input outbox as `CANCELLED`; a dispatched
or ambiguous response is reconciled and the provider Invocation must settle before the
Task becomes `CANCELLED`.

Events: `task.status.changed.v1`, Attempt and CapabilityInvocation status events,
`agent.session.closed.v1` for the planning session, `user.request.resolved.v1` and
`approval.resolved.v1` for owner-service closures, `verification.started.v1`/
`verification.completed.v1` for already-active runs, lease and Effect events, and the
final Task status event.

## F11 — Artifact production/revision

1. provider creates bytes/draft.
2. BlobStore commits content digest.
3. ArtifactStore creates immutable ArtifactVersion.
4. event emitted.
5. UI shows artifact only after version exists.
6. Later edit publishes version N+1 with expected Artifact aggregate version; prior versions remain addressable.
7. If a concurrent publisher wins, the stale publisher receives `STALE_VERSION`; preserve its draft and require explicit rebase or separate Artifact publication.

## F12 — Completion verification

1. worker calls finish proposal.
2. Task -> VERIFYING.
3. CompletionEvaluator enumerates mandatory criteria/outputs/effects/approvals.
4. VerifierRegistry selects deterministic verifier first.
5. evidence appended.
6. all mandatory criteria pass -> COMPLETED.
7. failed criterion with a permitted recovery -> READY, then a new Step Attempt moves the
   Task to RUNNING; without a permitted recovery, settle as INCOMPLETE, BLOCKED,
   NEEDS_USER, or terminal FAILED according to the classified outcome.
8. uncertain semantic criterion -> NEEDS_USER or independent verifier agent.

## F13 — Graceful local -> cloud handoff

1. user closes laptop or requests cloud continuation.
2. source Attempt reaches a safe boundary; unsafe/unfenced work requires explicit review.
3. ResumePacket written.
4. events/artifacts required by Task replicated.
5. open Effects reconciled.
6. source lease RELEASED.
7. target cloud Runtime validates eligibility.
8. target acquires higher-epoch lease after eligibility is rechecked.
9. fresh Attempt/AgentSession starts; no live process is transferred.
10. Task continues.

UI sequence: `This computer -> Saving progress… -> Cloud`, only after actual handoff phases.

## F14 — Unexpected device loss

1. heartbeat stale.
2. wait for execution lease expiry.
3. Attempt becomes ABANDONED/uncertain.
4. reconcile open Effects.
5. evaluate FailoverClass + ContinuationEligibility.
6. SAFE_PORTABLE/REPLAYABLE -> fresh Attempt elsewhere only after lease expiry/skew
   guard, reconciliation, and provider-fence checks.
7. HANDOFF_REQUIRED -> wait/ask.
8. LOCAL_BOUND -> wait for local resource.

## F15 — Ambiguous external effect

Example: email send request left process, network failed before response.

1. Effect already STARTED.
2. no trustworthy result -> AMBIGUOUS.
3. Reconciler checks provider state/id/message digest.
4. found -> OBSERVED/VERIFIED.
5. confirmed absent -> same idempotency identity may retry according to policy.
6. cannot determine -> Task NEEDS_USER; do not blindly repeat.

## F16 — Offline Runtime reconnect

1. local Runtime was offline while Hub received new events.
2. reconnect authenticates identity.
3. exchange replication cursors.
4. transfer immutable blobs referenced by eligible locally committed events, verify their
   digests, then submit event envelopes; download Hub events and referenced blobs in the
   same dependency order.
5. Hub validates origin authority, expected revisions, transition owner, and fences.
6. Offline mutable commands are submitted as pending intents and revalidated against the
   Hub's current aggregate version; they are not uploaded as already committed events.
   Entity-specific conflict rules apply, and rejected stale intents/events cannot change
   projections.
7. stale lease/fence cannot regain authority. A Runtime cannot reclaim an Attempt merely
   because it has locally stored events from an expired epoch.

## F17 — Runtime pairing

1. strong operator creates short-lived pairing token.
2. new Runtime generates key and submits token + public key.
3. Hub binds runtime identity.
4. runtime capabilities/environments advertised.
5. UI displays new device/runtime.

## F18 — Automation trigger and occurrence

1. Trigger host authenticates the source, normalizes any external payload, computes its digest, and resolves a bounded content-addressed ResourceRef; raw secret material is excluded.
2. Trigger host derives the revision-independent occurrence key from stable trigger identity.
3. TriggerCoordinator atomically claims `(automation_id, occurrence_key)`, stores the input ref/digest on first claim, and verifies the
   Automation is enabled, captures its current immutable AutomationRevision, and increments
   `claim_epoch`. If an edit races the claim, the transaction order decides the pinned
   revision; the claim is never reinterpreted afterward.
4. Same-key/same-digest duplicate returns the existing logical occurrence/Task. Same-key
   delivery with a different digest is rejected and audited; it cannot overwrite input.
   A reclaimed claim has a higher epoch; stale claimants cannot create or settle its Task.
5. Overlap policy is evaluated against that pinned revision.
6. A Task is created from the revision-pinned template and bounded input ref; the occurrence
   Task reference commits atomically with Task creation.
7. Task executes through the normal Task Runtime; the occurrence settles from Task outcome.
8. Result artifact/notification follows ordinary Artifact/Channel rules.

## F19 — Conditional monitor with no change

1. Automation occurrence creates Task.
2. agent/capability checks condition.
3. condition false.
4. Task completes with a `NO_ACTION` result artifact/evidence as policy requires. `NO_ACTION`
   is a result value, not a Task status.
5. notification suppressed.

When condition becomes true, notify according to policy and optionally disable automation.

## F20 — Telegram/email Task

1. ChannelAdapter receives deduplicated external message.
2. external identity maps to ChannelBinding/Workspace.
3. same Conversation mapping used.
4. Task materializes normally if needed.
5. cloud Runtime executes.
6. result response sent to same channel.

No separate bot Task store.

## F21 — Sensitive approval from weak channel

1. task requests sensitive effect.
2. Approval requires higher assurance than channel provides.
3. channel receives "approval required" + deep link.
4. strong LiteCowork surface authenticates user and resolves approval.
5. Task resumes.

## F22 — Package update while Task running

1. LiteSPM reports a newer package through its separately defined contract.
2. running Attempt remains pinned to CapabilityLock digest/version.
3. new Tasks may resolve newer version according to policy.
4. upgrade never mutates an in-flight capability silently.

## F23 — OAuth/secret expires mid-Task

1. capability invocation returns auth failure.
2. the activation remains `ACTIVE` unless its lifecycle actually fails; health is recorded as `DEGRADED` or `UNHEALTHY` independently.
3. Attempt WAITING_RESOURCE/approval as appropriate.
4. reconnect/re-auth creates/updates SecretRef outside transcript.
5. new SecretLease issued.
6. retry only if effect safety permits.

## F24 — Parallel code workers

1. lead delegates independent changes.
2. each child receives distinct GitWorktree Environment.
3. each produces diff/branch Artifact.
4. lead integrates in dedicated integration Environment.
5. tests verifier runs after merge.

Never permit two workers to mutate same checkout without explicit serialization.

## F25 — User changes lead agent

1. User requests the new AgentBinding; TaskService validates Workspace ownership, enabled
   status, protocol compatibility, and placement availability.
2. TaskService records the requested lead binding and emits `task.lead_agent.changed.v1`.
   New planning/plan submissions use the new binding; existing Attempts retain their own
   agent identity and current lease while they drain or are cancelled at a safe boundary.
3. If a `TASK_PLANNING` session is active, PlanningCoordinator closes it and starts a new
   planning session; no Attempt is fabricated.
4. TaskService writes a portable ResumePacket when execution context must be replaced.
   A replacement execution Attempt starts only after the prior lease is settled and open
   Effects are reconciled.
5. Task identity and prior session/Attempt history remain unchanged.

## F26 — Artifact Library archive

The desktop also explicitly promotes a TRANSIENT Artifact after confirmation using
`POST /v1/artifacts/{id}/promote`, If-Match and Idempotency-Key. The same owner-scoped
writer commits SAVED, one aggregate increment, `artifact.library.promoted.v1`, its state
snapshot and receipt. Neither promotion nor archive changes immutable content. Unconfirmed
responses offer an unchanged retry; stale-version rejection requires refresh/review.
The mounted Library surface removes a changed row from the current status filter only
after a validated committed response, and keeps the open Workbench/history readable.

Precondition: the Workspace is active and the Artifact is SAVED. The user confirms the named Artifact and understands that archive removes it from the default Library view while preserving authorized version reads.

1. Operator sends `POST /v1/artifacts/{id}/archive` with Idempotency-Key and If-Match.
2. ArtifactStore authenticates/authorizes the owner, checks the aggregate version and SAVED state, then atomically commits ARCHIVED, increments Artifact.version, and appends `artifact.library.archived.v1`.
3. A replay with the same key returns its recorded result. A fresh command after archive with current If-Match returns the current representation without a second transition event; stale If-Match conflicts.
4. Existing immutable ArtifactVersions remain readable under current authorization. A linked Artifact archive does not mutate or delete the external provider object.
5. UI removes the item from the default Library projection after the committed event and retains it in archived/history views.

## F27 — Connection and channel binding setup, permissions, and revocation

Actors: User, provider-owned setup surface/ChannelAdapter, ConnectionService, ChannelService, TrustService.

1. The user starts setup through the selected integration's provider-owned flow. The
   Operator API does not prescribe OAuth, device-code, webhook-secret, or callback
   mechanics.
2. The provider authenticates its account and external identity. Credential bytes stay
   in the provider-owned secret mechanism; LiteCowork receives references and normalized
   status only.
3. ConnectionService records the Connection state. For a human-facing channel,
   ChannelService records the ChannelBinding using provider-attested identity and
   assurance with `allowed_actions = []`.
4. RuntimeMesh assigns the binding to one eligible ChannelHost Runtime and issues a
   bounded host lease. The selected host must have the channel adapter and authorized
   SecretRef placement; the Operator sees the Runtime and availability.
5. The owner reviews the binding and explicitly sets allowed actions. TrustService
   authorizes the change; an authority increase may require Approval. The versioned
   binding update and `channel.binding.changed.v1` event commit atomically.
6. On disconnect, ConnectionService blocks new capability use through that Connection;
   any linked ChannelBinding is non-authorizing even before its status projection
   updates. Binding revocation separately blocks inbound commands. Existing Conversations,
   Tasks, Artifacts, Effects, and audit history remain readable; no external account or
   credential is deleted.

Events: `connection.state.changed.v1`, `channel.binding.changed.v1`,
`channel.host.assignment.changed.v1`.

UI: show provider status, authenticated identity, assurance level, and granted actions.
Never show a binding as authorized while its allowed-action set is empty or its status
is REVOKED.

## F28 — Persistent WorkspaceRoot and deterministic search

Actors: User, Operator UI, ResourceService, WorldIndexer, local Runtime.

Precondition: Workspace is ACTIVE; the user explicitly selects one folder Resource and
one observed location.

1. ResourceService validates the location identity and records WorkspaceRoot with an
   explicit metadata/content watch policy and independent replication policy.
2. WorldIndexer observes only beneath that granted root and emits Resource/Revision/
   Location facts; it performs no home/drive scan.
3. Operator search returns deterministic name/type/date/freshness/text matches with
   stable ResourceRefs, available locations, and match reasons. Search uses no model
   tokens and does not attach results to agent context.
4. User or Agent explicitly selects a revision-pinned ResourceRef. ResourceResolver
   validates location, digest/revision, policy, and access before exposure.
5. If watcher overflow or Runtime loss occurs, affected location freshness becomes
   `UNKNOWN`/`STALE`; bounded reconciliation revalidates identity before it returns to
   current availability.
6. If two locations report independent revisions from a common ancestor, ResourceService
   preserves both heads, sets `current_revision_id` to null, and projects `CONFLICTED`.
   Unpinned resolution returns `RESOURCE_CONFLICT`; the user can inspect revision ancestry
   and pin a branch, or produce a verified merge whose ancestry includes the heads merged.
   Selecting a branch never deletes or hides its sibling.

Events: `workspace.root.created.v1`, `resource.created.v1`,
`resource.revision.observed.v1`, `resource.location.changed.v1`.

UI: distinguish one-time attachment from persistent “Add to Workspace”; show root policy,
freshness and location. When conflicted, show the revision branches and require an explicit
pin or merge before using an unpinned reference. No selected search result enters agent
context silently.

## F29 — Asynchronous MCP Task-backed capability call

Actors: Agent, LiteCowork Gateway, CapabilityBroker, InvocationRunner, MCP server,
ConversationService/TaskService.

Preconditions: The server advertises and negotiates the versioned Tasks extension for
this request; the AgentSession has a matching scoped read/write grant; any consequential
operation has the required Approval/Effect/fence path.

1. Gateway authenticates the session credential. CapabilityBroker checks scope/grant and
   commits CapabilityInvocation with immutable request digest before provider dispatch.
2. `tools/call` returns `resultType: task`; InvocationRunner persists the opaque provider
   task ID only in the encrypted Runtime-local invocation binding, then commits initial
   status, creation time, expiry/latest TTL, and poll hint before acknowledging the Gateway
   caller.
3. InvocationRunner polls via `tasks/get` or consumes negotiated status notification,
   respecting the provider poll hint and TTL; provider state updates the Invocation, not
   LiteCowork Task status.
4. On `input_required`, materialize a UserRequest from each bounded input request and
   store its provider request key only in the encrypted Runtime-local ProviderInputBinding.
   Validate and persist the user's response. For a Conversation-scoped request, move its
   exact ConversationTurn to `WAITING_DEPENDENCY`; for a Task/Attempt request, keep the
   associated Attempt in `WAITING_RESOURCE` or the Task `BLOCKED` if no other Step can
   proceed. InvocationRunner sends the matching key/value through `tasks/update`. An
   acknowledgement is not acceptance: poll `tasks/get` until the exact key is no longer
   outstanding and the provider has authoritative state, or the provider reaches a terminal
   state that can be reconciled. Acceptance permits fresh-session continuation. A rejected
   response with provider state still `INPUT_REQUIRED` creates a new UserRequest for the
   current provider key and returns the same ConversationTurn to `WAITING_USER`; unresolved
   acceptance leaves it `WAITING_DEPENDENCY` and starts no new AgentSession. A repeated poll
   cannot create a second inbox item or resume another turn/Attempt.
5. On completion, store result refs and settle Invocation. A returned tool `isError` is
   result content; extension-level `failed` is an Invocation failure.
6. On cancellation, `tasks/cancel` acknowledgement records intent only. Keep the
   Invocation `CANCEL_REQUESTED` until provider state/result is observed and any Effect
   reconciled.

Events: `capability.invocation.created.v1`, `dispatched.v1`, `checkpointed.v1`, and
`status.changed.v1`, plus any UserRequest/Effect/Evidence events.

Failure behavior: provider TTL expiry or lost handle does not imply success or cancellation;
mark `AMBIGUOUS`, reconcile where possible, otherwise ask the user. If the initial handle
response is lost, do not redispatch without proven provider idempotency/lookup. The MCP
task ID is never presented as a LiteCowork Task ID.

If the linked UserRequest expires before an answer, mark its local ProviderInputBinding
`EXPIRED`, cancel or reconcile the provider operation, and keep the ConversationTurn in
`WAITING_DEPENDENCY` until the operation is quiescent. Then fail the turn retryably with
`USER_REQUEST_EXPIRED`; for Task/Attempt scope block the affected Step and allow unrelated
Steps to continue.

UI: show one capability operation under its real Conversation/Task lane; never create a
second user-visible Task for the provider handle.

## F30 — Pause and resume a Task

Actors: User, Operator UI, TaskService, AttemptRunner, AgentAdapter, InvocationRunner,
CompletionEvaluator, VerifierRunner, EffectReconciler, LeaseCoordinator.

Precondition: Task is nonterminal and user has authority to manage it.

1. User requests pause with current Task version. TaskService commits
   `PAUSE_REQUESTED`, records the prior resumable state, stops new Attempt admission, and
   fences CompletionEvaluator from finalizing or starting new VerificationRuns.
2. TaskService fences late plan proposals. PlanningCoordinator asks AgentSessionSupervisor
   to close any active TASK_PLANNING session; AttemptRunner asks active execution workers
   for a safe-boundary interrupt and checkpoints a portable ResumePacket. It does not claim
   to stop an external process without evidence.
3. InvocationRunner stops new calls, requests cancellation for active Task-planning/Attempt provider operations,
   and waits for authoritative terminal status. A confirmed `INPUT_REQUIRED` invocation
   is quiescent and remains linked to its UserRequest until Task resume.
4. Let already-active VerificationRuns settle against their pinned criteria and `ResourceInput` values.
   Their Evidence remains valid for that pinned revision, but they cannot finalize the
   Task while pause is pending.
5. Reconcile every open Effect, settle/abandon Attempts, and release their leases. A
   quiescent `INPUT_REQUIRED` Invocation may retain its source Attempt in `WAITING_RESOURCE`
   without a live lease, but only with a committed checkpoint and reconciled Environment.
6. Only when the planning session, required host work, active provider calls, and
   VerificationRuns are settled, and Effects are reconciled, does TaskService commit
   `PAUSED`.
7. On resume, revalidate input revisions/locations, grants, secrets, capability locks,
   Environment, budget, and open Effects. Re-admit a retained provider-input Attempt only
   on the same Runtime incarnation and Environment, with a fresh AgentSession and higher
   lease epoch. Other work uses a new Attempt; preserve pending UserRequests.
8. If a Task-scoped UserRequest is answered after the Task reaches `PAUSED`, persist the
   response and show it as queued; do not deliver it to the provider or start an Attempt.
   On explicit resume, planning-scope input waits for a fresh active planner under the
   same current lead/TaskSpec; Attempt-scope input waits for the retained source Attempt
   to be re-admitted under a fresh lease. If the source Attempt/Runtime cannot be
   retained, reconcile or cancel the old Invocation; a new Attempt receives the response
   only as bounded context for newly authorized work. During
   `PAUSE_REQUESTED`, reject a response with `CONFLICT` and leave the request pending.
   Approved-but-unused records remain unconsumed until resumed work admits the exact
   action.

Events: `task.pause.requested.v1`, `agent.session.closed.v1`, `attempt.checkpointed.v1`,
`capability.invocation.status.changed.v1`, `verification.completed.v1` for any run already
in progress, lease/effect reconciliation events, `task.paused.v1`, and `task.resumed.v1`.

Failure behavior: an unsafe checkpoint, unresolved Effect, unconfirmed provider
Invocation, or unsettled VerificationRun leaves the Task `PAUSE_REQUESTED` with a visible
blocker and linked UserRequest when input is needed; it never reports `PAUSED` prematurely.
If completion commits before the pause command, the Task is already complete and cannot be
paused. Pause never cancels or deletes the Task.

UI: show `Pausing safely` with real stages, then `Paused`; resume remains disabled until
state revalidation completes. Motion follows `MOTION.md`.

## F31 — Human takes over Browser/Desktop control

Actors: User, Operator UI, EnvironmentManager, AgentSessionSupervisor,
EnvironmentProvider, EffectReconciler.

Precondition: An Attempt has an active EnvironmentControlLease owned by an Agent and the
user is authenticated on an adequate-assurance Operator surface.

1. User submits takeover with the current control epoch.
2. EnvironmentManager stops admitting agent input, drains or rejects in-flight input,
   invalidates queued commands, atomically changes owner to HUMAN, increments epoch, and
   delivers the new runtime-private fencing credential only to the enforcing provider over
   authenticated control. Operator receives the new owner/epoch view without the credential.
3. EnvironmentProvider rejects every old-epoch Agent command. Queued commands from the
   old epoch are discarded and never replayed.
4. LiteCowork captures a fresh Environment observation and reconciles external state and
   Effects. Human actions are observed/reported under the human identity.
5. To return control, user explicitly requests it with the current epoch. The system
   refreshes observation, reconciles drift/Effects, and only then grants Agent ownership
   under a new epoch and refreshed AgentSession context.

Events: `environment.control.lease.changed.v1`, observation/effect/evidence records.

Failure behavior: a stale expected epoch is rejected; if state cannot be observed or an
Effect is ambiguous, keep the human in control or require a user decision. Never resume
queued agent input.

UI: identify the actual Browser/Desktop Environment, show who currently controls it, and
animate only after the owner/epoch event commits. Human control is not a new Attempt and
does not silently change the Runtime ExecutionLease.

## F32 — Workspace instruction revision and Task pinning

Actors: User, WorkspaceService, TaskService, ContextService, AgentSessionSupervisor.

1. User saves new instructions as an immutable WorkspaceInstructionRevision with content
   digest and author.
2. WorkspaceService advances the current revision pointer atomically with its event.
3. New Conversation turns receive that current revision at a safe boundary. Existing Task
   contexts stay pinned to their TaskSpec revision.
4. To apply new instructions to an existing Task, user explicitly creates a new
   TaskSpecRevision naming the selected instruction revision; the prior Task history is
   preserved.

Events: `workspace.instructions.revision.created.v1` and, only when the Task is revised,
`task.spec.revised.v1`.

UI: show revision history and identify the instruction revision used by each active Task.
Instruction edits do not silently rewrite already-running agent context.

## F33 — Resource change invalidates derived outputs

Actors: WorldIndexer, DependencyService, ArtifactStore, VerificationService,
Operator UI.

1. A provider observation records a new ResourceRevision and location freshness.
2. DependencyService follows exact input revision edges and appends InvalidationRecords
   for derived ArtifactVersions and VerificationRuns that used the changed source.
3. Immutable ArtifactVersion/Evidence/VerificationRun history remains intact; only the
   current freshness projection becomes STALE.
4. Re-verification creates a new VerificationRun bound to the new TaskSpec criterion
   digest and input digests. Rebuilding an output creates a new ArtifactVersion.

Events: `resource.revision.observed.v1`, `resource.invalidation.created.v1`, and any new
verification/artifact events.

UI: label stale outputs and the exact changed input; never keep a green verification mark
for stale inputs.

## F34 — Reusable Skill proposal

Actors: User, SkillProposalService, redactor/validator, TrustService, LiteSPM adapter.

1. After successful work, the user or Agent explicitly requests a reusable Skill draft.
2. SkillProposalService creates a `SKILL_DRAFT` Artifact from selected method/provenance,
   strips Task-specific data and secrets, and records redaction/validation results.
3. User reviews exact content and approves or rejects the proposal. Approval is not
   inferred from a successful Task.
4. On approval, LiteCowork hands the reviewed content/reference to LiteSPM publication
   once its contract is available; LiteSPM owns package versioning/distribution.

Events: `skill.proposal.created.v1`, `skill.proposal.status.changed.v1`, Artifact and
ApprovalUse events where applicable.

Failure behavior: a redaction failure blocks publication; LiteCowork never stores a hidden
autonomous memory rewrite or claims publication before LiteSPM acknowledgement.

UI: show proposal state, redaction status, exact reviewable draft, and publication result.

## F35 — Notification delivery after a Task event

Actors: ProjectionService, NotificationService, ChannelAdapter, User.

1. A committed Task/Approval/Automation event is evaluated against the current
   NotificationPreference and quiet-hours policy.
2. NotificationService creates one deduplicated NotificationDelivery with a stable key.
3. ChannelAdapter attempts delivery under rate limit/backoff; retries reuse the same
   logical delivery identity where provider semantics allow.
4. A known failure may retry under the bounded policy. A timeout after possible provider
   acceptance becomes `AMBIGUOUS`; NotificationService reconciles before retry and leaves
   it open if delivery cannot be determined. `SENT` means provider acknowledgement only.
   Task/Approval state is read independently from the domain projection.

Events: `notification.preference.changed.v1`, `notification.delivery.changed.v1`, and
the source domain event.

Failure behavior: duplicate source events cannot create duplicate logical deliveries;
delivery exhaustion or unresolved ambiguity becomes visible without altering Task
completion. Ambiguous notifications are not resent automatically.

UI: distinguish “notification delivered” from “Task completed”; show mute/quiet-hours and
channel fallback state.

## F36 — Workspace backup and restore

Actors: BackupService, StateStore, EventStore, BlobStore, RuntimeMesh, Workspace owner.

1. BackupService obtains a consistent Workspace snapshot, per-origin event cursors,
   referenced blob manifest, schema version, integrity digest, and encryption-key reference.
2. It verifies every object and publishes an immutable WorkspaceBackupManifest only after
   all checks pass; incomplete capture has no restorable manifest.
3. Restore starts only on a locally authenticated empty installation. It validates the
   key/schema, verifies and decrypts required data, loads the point-in-time snapshot and
   included event history through the manifest cursors, and rebuilds projections before
   enabling writes.
4. Restored installation registers a new Runtime identity; it never revives old leases or
   fencing epochs. Events created after the backup barrier arrive only through later
   authorized Mesh replication. Secret bytes are not in the backup; Connections are
   revalidated and may require reauthentication. Active Effects are reconciled before
   Task execution resumes.

Records: `workspace_backup_manifests`, a backup-operation AuditRecord, and the resulting
new Runtime registration. Backup manifests/bytes are control-plane recovery metadata, not
ordinary replicated Workspace domain events.

Failure behavior: incomplete/invalid backups are rejected as unrestorable; source history
remains untouched. Restore drills must prove no expired Runtime regains authority.

UI: show last verified backup time, restore point, and any missing artifact/effect blocker.

## Flow outputs and UI projection

| Flow | Durable result | Required UI projection / failure behavior |
|---|---|---|
| F00 | Workspace policy/lifecycle event | Show policy scope; archived Workspace is read-only |
| F01 | Conversation messages only | Ordinary exchange; no Task card |
| F02 | Task + initial spec linked to source message | Task card follows commit; planning is shown without a fake lane |
| F03 | Plan, Steps, lease, Attempt, outputs, verification | Lanes reflect persisted state; completion follows evaluator |
| F04 | Capability lock, grant, activation, invocation/Effect | Show capability only when active/used; unsafe path is unavailable |
| F05 | Proxy call, optional next-boundary attachment | Current turn continues without a forced restart |
| F06 | Approval, grant, resumed Attempt | Show exact scope and assurance; denial/expiry is not execution |
| F07 | Child Attempt and ResultEnvelope | Branch appears only after child creation; Inspector exposes ownership |
| F08 | Child failure/recovery event | Only affected lane stops; independent children continue |
| F09 | New TaskSpecRevision or steering message | Show revision impact and stale child results |
| F10 | Cancel intent, settled Attempts/Effects, terminal or needs-user state | Show Stopping until Effects reconcile; preserve outputs/history |
| F11 | Immutable ArtifactVersion and expected-version publication | Show only after blob digest and manifest commit; preserve stale concurrent drafts |
| F12 | VerificationRun/Evidence and Task result | Indicator follows a real verifier; no checkmark on fail/inconclusive |
| F13 | Handoff phases, old lease release, new Attempt | Show Cloud only after target lease; failure retains source location |
| F14 | Loss detection, lease expiry, reconciliation, eligibility | Show uncertainty/blocker until takeover is safe |
| F15 | Ambiguous Effect and reconciliation evidence | Never display success or resend while uncertain |
| F16 | Accepted/rejected events and updated cursor | Show reconnect/stale/conflict; no silent last-writer-wins |
| F17 | Paired Runtime identity and offers | Device appears only after one-use token validation/authentication |
| F18 | Revision-pinned occurrence and ordinary Task | One logical occurrence/Task; stale claim epochs are fenced |
| F19 | Completed Task with `NO_ACTION` result | Suppress notification only under saved policy |
| F20 | Shared Conversation/Task plus channel receipt | Same Task identity across surfaces; delivery failure is separate |
| F21 | Approval routed to stronger surface | Weak channel shows a link, not an enabled sensitive action |
| F22 | Existing CapabilityLock unchanged | In-flight Task stays pinned; no silent upgrade |
| F23 | Degraded activation health and SecretLease recovery | Show reconnect/reauth blocker; retry under Effect policy only |
| F24 | Isolated worktrees, diff Artifacts, integration result | Preserve branch provenance; conflicting edits block merge |
| F25 | New lead binding/session and Attempt history | Planning replacement creates no Attempt; execution replacement waits for lease safety |
| F26 | Artifact library archive event | Remove from default Library projection; preserve authorized version reads and linked source |
| F27 | Connection/ChannelBinding state and action changes | Show provider-attested identity, explicit authority, and preserved history on revocation |
| F28 | WorkspaceRoot, Resource observations, deterministic search | Show stable ResourceRef, freshness, location and explicit attachment; no token-backed semantic claim |
| F29 | Durable Invocation and provider task lifecycle | One capability activity; provider handle is never shown as a LiteCowork Task |
| F30 | Pause/resume events, ResumePacket, settled Attempts/Effects and fresh leases | Show real pause stages/blockers; never conflate pause with cancellation |
| F31 | EnvironmentControlLease owner/epoch transition and fresh observation | Show current human/Agent controller; reject stale queued input |
| F32 | Workspace instruction revision and optional TaskSpecRevision pin update | Show exact revision used; active Task context does not silently change |
| F33 | ResourceRevision and InvalidationRecords | Mark affected Artifact/Evidence stale while preserving immutable history |
| F34 | SkillProposal review/redaction/publication state | Show exact draft; publication is not claimed before LiteSPM confirms |
| F35 | NotificationDelivery state | Show transport acknowledgement separately from Task status |
| F36 | Verified backup manifest/restore checks/new Runtime identity | Show recoverability and blockers; never restore old lease authority |
| F37 | Runtime incarnation readiness and recovery mutations | No installed worker launches solely from boot |
| F38 | Operational host reference and AgentSession state | Cold/starting/ready follows observed admission; idle stop cannot kill live sessions |
| F39 | Routine revision and explicitly confirmed Automation | Draft cancellation persists no reusable job or schedule |
| F40 | Revision-pinned occurrence, cursor and dependency blockers | Due/offline differs from not yet due; one logical run |
| F41 | Fresh offers/resources, reconciled Effects and misfire occurrences | No stale action replay or expired lease revival on wake |
| F42 | Read-only preview, accepted drain and eventual stop observation | Closing UI differs from stopping daemon; acceptance differs from stopped |
| F43 | Workspace-owned Environment lifecycle, budget and provider identity | Persistent compute remains visible/costed; every Task use gets fresh authority |
| F44 | Deduplicated Needs You projection | Inbox actions remain on the owning record; delivery status is not Task status |
| F45 | Exact channel reply to one pending FORM request | No implicit latest-request selection; no Approval or external-auth response through channel |
| F46 | New channel host assignment with fenced receipt/cursor continuity | Reject stale host input; require owner decision when ingress continuity cannot be proven |
| F47 | DelegationProfile created disabled then explicitly enabled | No process starts and no profile enters lead discovery before enable commit |
| F48 | Second revisioned worker profile on one AgentBinding | Preserve one installed harness identity and future-only profile revisions |
| F49 | Child Attempt, lease, grants, budget, and ResultEnvelope | Show child only after admission; worker output remains unverified until verifier pass |
| F50 | Failed child Attempt followed by bounded new-profile Attempt | Preserve failed Attempt/Evidence; stop escalation on ambiguity or exhausted budget |
| F51 | Active child remains pinned after profile disable/revision | Block future admissions; explicit cancel uses ordinary reconciliation |
| F52 | Quota-low observation and speculative prewarm | Never invoke fallback model or display prewarm as work |
| F53 | Confirmed lead quota exhaustion and policy-governed handoff | No silent lead switch; new lead gets a fresh session and bounded handoff packet |
| F54 | Changed native configuration digest and re-probe | Preserve user config; reject unsupported/stale overrides |
| F55 | Coworker created, selected primary, then first Task | Identity persists across lead changes; Task pins Coworker revision |
| F56 | Suggestion acceptance creates ordinary Task or opens editor | No direct execution, scheduling, or authority grant |
| F57 | Goal links verified Task outcomes, Evidence, and pinned Artifact versions | Progress remains derived; only owner changes Goal completion status |
| F58 | Concurrent ContextDocument Resource revisions | Preserve both changes and require explicit merge/rebase |
| F59 | Deadline preflight and bounded ActionBatch | Fail before effects when prerequisites fail; reconcile partial Effects before fallback |
| F60 | Shared browser profile takeover through control lease | One current controller; stale epoch input is discarded |
| F61 | Demonstration trace converted to SkillProposal | Review/redaction required; no publication before approval and LiteSPM confirmation |
| F62 | Delegation budget threshold reached | Stop new admissions under policy; never kill an Attempt mid-Effect |
| F63 | Child worker loss with potentially ambiguous Effect | Reconcile before retry/escalation; preserve independent children only when eligible |
| F64 | Coworker-private Environment reused by future Task | Fresh Attempt authority and one current browser-control owner |
| F65 | Suspended persistent Environment changes sharing scope | Reject active/ambiguous use; atomically commit owner/scope before later placement |
| F66 | Pinned Skill/Routine dependency drifts | Block unsafe replay, expose evidence, and create only an owner-reviewed repair proposal |
| F67 | Owner snoozes a Suggestion | Keep it proposed, hide until the bounded time or expiry, and preserve versioned history |
| F68 | Owner mutes a SuggestionKind | Atomically persist Workspace preference and dismiss current proposals of that kind |
| F69 | Owner reopens a completed Goal | Change Goal status only; retain prior completion, Tasks, and Evidence |
| F70 | Duplicate a DelegationProfile | Create revision 1 disabled under the same binding; preserve no authority or execution state |
| F71 | Policy-based lead failover | Fresh lead session and handoff only after pinned policy and admission checks |
| F72 | Coworker-owned Automation creates a Task | Pin Automation/Coworker revisions and admit an ordinary Task once |
| F73 | Bounded ActionBatch with partial failure | One Invocation per operation; reconcile committed Effects before continuing |
| F74 | Concurrent Resource revision edit | Preserve both edits and require explicit merge/rebase on stale heads |
| F75 | ContextDocument revocation/deletion | Block new reads immediately; retain purge tombstone until every registered target is acknowledged |
| F76 | Suggestion producer admission/suppression | Only registered bounded producers; dedupe, mute, cooldown and provenance before proposal |
| F77 | Bounded Teach-a-task capture | Semantic trace becomes a reviewed SkillProposal, not an executable replay |
| F78 | Workspace worker enablement vs Coworker assignment | Available workers are distinct from those permitted for one Coworker |
| F79 | Native turn interruption with live command | Stop/reap the owned process tree and prove writer quiescence before releasing its fence |
| F80 | Runtime restart during replacement startup | Recover durable state and do not assume the replacement process/session survived |
| F81 | Runtime restart after replacement Attempt is running | New incarnation fences old lease; reconcile Effects before any retry |
| F82 | Render, stream, and recover a Conversation presentation | Sequenced transient frames resume or replace from committed state without journaling deltas |
| F83 | Inspect context used for work | Show only exact authorized attachments/retrieval receipts actually resolved |
| F84 | Compare and restore an Artifact version | Compare immutable versions; restore by appending a new version |
| F85 | Import local files into a Workspace | User-selected bytes become digest-checked Resources with explicit provenance |
| F86 | Read and preview a committed local Resource | Resolve exact authorized revision and retain raw download fallback |
| F87 | Create/list local Workspace instruction revisions | Append immutable instructions; pin the exact revision used by future Task context |
| F88 | Edit Workspace instructions in desktop | Require current version/parent and preserve draft on conflict |
| F89 | Browse a large local Resource catalog | Bound pages and discard late Workspace responses |
| F90 | Resumable desktop Resource upload | Resume exact content digest; distinguish committed upload from lost response |
| F91 | Confirm authenticated local Operator incarnation | Tauri becomes ready only after identity-bound daemon readiness response |
| F92 | Search managed Resources | Enforce Workspace scope and label lexical/on-demand match semantics honestly |
| F93 | Enroll/revoke local Runtime for a Workspace | Versioned owner action; stop future access without rewriting Task history |
| F94 | Authenticate desktop Operator over local OS IPC | Per-install identity, peer validation and bounded authenticated requests |
| F95 | Explicit coding-agent profile probe | Display bounded observations only; unknown auth/inference stays unqualified |
| F96 | Select Workspace default lead agent | Versioned eligible binding; no silent lead/model fallback |
| F97 | Persist standalone Task envelope | Atomic READY Task/spec/event/receipt; no planning or Attempt implied |
| F98 | Claim durable Task planning startup | Producer-scoped session reservation and stale assignment fencing |
| F99 | Accept initial plan into durable Task state | Validate producer, TaskSpec and DAG; atomically append Plan/Steps |
| F100 | Save standalone Task from desktop composer | Preserve draft and Coworker/Workspace identity until committed receipt |
| F101 | Pin Library Resources to saved Task | Pin exact authorized Resource revisions; attachment does not imply content exposure |
| F102 | Load persisted Task plan/Steps in desktop | Validate Task identity and show only saved plan state |
| F103 | Revise an unplanned Task objective | Append TaskSpec revision only when READY and no Plan is accepted |
| F104 | Pause/resume persistent folder scope | Verify exact selected root identity; never broaden to a replacement path |
| F105 | Pin Coworker revision to paused Automation | Preserve exact origin configuration; definition remains paused |
| F106 | Check Task planning readiness | Read-only blocker projection; never dispatch or create execution records |
| F107 | Link/unlink Workspace work from Goal | Versioned provenance links only; Task/Routine/Artifact state remains authoritative |
| F108 | Accept actionable Suggestion as Task | Atomic ordinary READY Task plus Suggestion resolution; no execution authority |
| F109 | Read exact Coworker revision provenance | Return the pinned historical revision; never substitute the current head |
| F110 | Review saved Task outcome/activity snapshot | Show only committed fields; keep stale freshness distinct from Evidence and no live claim |
| F111 | Preview managed Markdown Artifact | Safe bounded rendering; malformed syntax falls back to raw text; external HTTPS requires confirmation |
| F112 | Inspect immutable TaskSpec history | Exact Task-scoped revisions, read-only; preserve cached history when offline |
| F113 | Compare immutable Artifact versions | Exact versions through bounded side-by-side or literal line diff; no semantic inference or mutation |
| F114 | Save an exact Artifact version from desktop | Native Save As for selected managed content up to 10 MiB; verify identity and digest; cancellation writes nothing |
| F125 | Recover an ambiguous ManualTrigger after navigation | Reuse exact Workspace-scoped request; only confirm a matching READY Task receipt |
| F126 | Recover ambiguous Goal mutations after navigation | Keep exact request/version/payload in bounded volatile Workspace-scoped recovery |
| F127 | Recover Suggestion and preference mutations after navigation | Retry exact owner command and validate matching receipt |
| F128 | Run an active Routine into a READY Task | Validate pinned Routine inputs; preserve exact request after ambiguous response; no execution implied |
| F129 | Recover Suggestion Task acceptance after navigation | Retry exact command and require an atomic linked READY-Task receipt |
| F130 | Browse persisted Tasks needing attention | Exact status queries; stale-safe navigation to a fresh Task read; no source mutation |
| F131 | Compose optional rich Conversation response | Semantic Message commits first; separately validated RichPresentation may upgrade it |
| F132 | Recover rich compilation, blob, or renderer failure | Preserve complete semantic answer through timeout, invalid schema, missing blob, or unsupported renderer |
| F133 | Present exact Artifact deliverables and ZIP | Bind committed ArtifactVersions; bundle only explicit immutable inputs |
| F134 | Stream and fence rich draft frames | Bounded transient layout frames cannot block turns or cross retries |
| F135 | Render portable rich response on a constrained channel | Flatten safely without adding authority or losing semantic fallback |


## F37 — Runtime boot and incarnation recovery

**Actors/preconditions:** OS service manager or authenticated Operator; installed Runtime,
exclusive installation lock, accessible StateStore and authorized startup policy.

1. Acquire the installation lock, create a fresh bootstrap incarnation ID, open StateStore
   and BlobStore, and apply supported migrations.
2. Load/create the stable Ed25519 DeviceIdentity through the OS credential store. Verify
   its RuntimeId binding and any existing Runtime catalog identity, then atomically persist
   the Runtime, new `RECOVERING` incarnation, and local observation. Credential failure
   or identity mismatch fails closed before Operator API startup.
3. Validate journal/checkpoints; recover claims and reconcile stale leases, Effects, and
   owned process handles. Restore scheduler cursors and mark watcher gaps stale.
4. Reconnect the Hub, refresh offers, and resume authorized resource observation.
5. Publish readiness only after mandatory recovery gates pass. Installed workers remain
   cold; recovery never starts every agent/provider/application. Local catalog registration
   creates no Workspace binding or Mesh presence.

**Failure/UI/postcondition:** storage or integrity failure prevents execution admission;
show recovery/degraded reasons. Local operational incarnation state and logs record boot
progress; domain mutations caused by recovery retain their normal typed events. A ready
Runtime has a fresh incarnation and no authority inherited merely from an old PID.

## F38 — Lazy agent admission and idle teardown

**Actors/preconditions:** AttemptRunner, AgentHostSupervisor, adapter; admitted session
scope and eligible endpoint, with authorization and budget already checked.

1. Single-flight `ensure_ready` for the endpoint/incarnation/hosting mode; acquire a host
   reference before spawning or attaching. Negotiate concurrency and isolation.
2. Start the AgentSession only after readiness. Remote API/A2A endpoints need no local
   process. Existing external processes remain externally owned.
3. On turn/assignment settlement or an authorized wait, observe adapter quiescence, close
   the AgentSession, and release its host binding. A waiting ConversationTurn resumes with
   a new session after a UserRequest response. A durable asynchronous CapabilityInvocation
   continues under InvocationRunner after the agent session closes; its result reaches a
   replacement Attempt session only after lease/incarnation/checkpoint revalidation.
4. At zero references, idle policy may stop an owned host; recheck references atomically
   before stopping.

**Failure/UI/postcondition:** startup failure blocks the requested session and consumes
its bounded retry policy, without starting unrelated agents. Display cold/starting/ready
from operational state; Task status follows session/Attempt events. Concurrent admission
cannot race idle teardown into killing an active worker.

## F39 — Save a Routine, run it, and schedule it

**Actors/preconditions:** user, Operator, RoutineService, TaskService, AutomationService;
authorized Workspace and source work visible to the user.

1. Prepare an unsaved, redacted Operator draft with inputs, outputs, criteria, capabilities,
   placement, verification and budget. Strip secrets and Task-specific private data.
2. User Save creates Routine and immutable RoutineRevision. A manual run validates input
   schema and creates an ordinary Task pinned to that revision.
3. Schedule opens a separate review of trigger definitions, host placement, misfire policy,
   permissions and execution dependencies. Only explicit confirmation enables Automation.
4. Later edits create revisions; existing Tasks and claimed occurrences retain their pins.

**Failure/UI/postcondition:** invalid inputs or archived Routine reject new work. Cancelling
an unsaved draft creates no Routine or Automation. Show Routines/Automations/Runs separately;
commit events reflect persisted revisions, never an optimistic claim of a scheduled run.

## F40 — Trigger fires while execution dependencies are offline

**Actors/preconditions:** TriggerCoordinator, Hub/local trigger host, TaskService, Placement;
enabled revision-pinned Automation and authorized trigger observation.

1. Deduplicate delivery by automation, stable trigger identity and logical occurrence key;
   fence the trigger host epoch and persist the occurrence/cursor transactionally.
2. Evaluate inputs and execution dependencies independently of where the trigger ran.
   A cloud schedule can become due while its local Excel/resource dependency is offline.
3. Record WAITING_DEPENDENCY with explicit blockers instead of starting an ineligible
   worker. Runtime/resource availability changes cause bounded eligibility reevaluation.
4. Admit work once dependencies, authorization and current claim authority are valid;
   preserve the original occurrence identity rather than manufacturing a second run.

**Failure/UI/postcondition:** expiry, revocation and revised policy are checked before
admission. Display “Due; waiting for this computer” and the actual dependencies. Distinct
triggers do not silently coalesce; notification delivery does not settle Task success.

## F41 — Sleep/wake and schedule catch-up

**Actors/preconditions:** OS resume notification, RuntimeResumeCoordinator, resource index,
TriggerCoordinator; a previously active Runtime or a new incarnation after restart.

1. Reconnect and validate incarnation/process identity and Hub authority. Observation gaps
   become stale until watcher cursors are validated or scoped rescans complete.
2. Reprobe offers; reconcile interrupted Effects and local execution before admitting
   deferred work. Expired leases never revive just because the machine wakes.
3. Evaluate missed schedule slots using the pinned timezone/recurrence semantics and
   SKIP, RUN_ONCE_WHEN_AVAILABLE or CATCH_UP_BOUNDED policy. Deduplicate against durable
   occurrences; condition watchers compare retained cursors and expose observation gaps.
4. Resume eligible local work with fresh leases/checkpoints; cloud work keeps its current
   ownership. Wake attempts remain optional and never a correctness prerequisite.

**Failure/UI/postcondition:** unresolved side effects or unknown watcher history block
unsafe replay. Show reconnect/stale/waiting states until reconciled; sleeping local work
is never presented as still executing.

## F42 — Explicit Runtime stop and safe drain

**Implementation status:** target flow only. The current daemon does not expose an
authenticated Operator stop endpoint or stop preview. Do not add a direct IPC-to-signal
shortcut. Implement this flow only after Attempt admission/drain, Effect reconciliation,
lease settlement, and local trigger/provider-reference ownership can provide the required
quiescence proof.

**Actors/preconditions:** authenticated Operator, RuntimeLifecycleService, TaskService,
Mesh; user requests stop against the current incarnation.

1. Preview dependent Tasks, local triggers, roots and shared services; present cancel,
   move eligible work, or explicit stop choices before committing the request.
2. Stop new local admission. Reach checkpoint boundaries, reconcile Effects and hand off
   eligible work through ordinary lease release/new Attempt semantics.
3. Preserve unresolved local-bound work and scheduler cursors. Shut down only owned
   processes/resources under their lifetime policy; never close a pre-existing user app.
4. Flush committed state and mark the incarnation stopped before process exit when possible.

**Failure/UI/postcondition:** unsafe drain reports concrete blockers. Forced process loss
uses unexpected-loss recovery and cannot claim a clean pause. UI exit alone does not issue
this flow; cloud Tasks survive an unrelated local Runtime stop.

## F43 — Provision and later reuse a persistent Environment

**Actors/preconditions:** Workspace owner, Operator, EnvironmentManager, provider,
TrustService; provider offer is current and the user supplies bounded source Resources,
placement, network, resource, retention, backup and budget policy.

1. Preview provider class, Runtime, estimated cost/retention and pinned Resource inputs.
   Reject missing budget ceilings and unsupported network/isolation requirements.
2. The preview issues a short-lived, principal-bound digest for the exact normalized
   request and selected eligibility/quote basis. After explicit confirmation, create
   echoes that digest; the service consumes it once, reserves budget and commits a
   Workspace-owned Environment without a Task owner plus its event before provider I/O.
   Return the current `PROVISIONING` view.
3. Verify provider handle identity and health before projecting `READY`. Record locator
   material in provider-private storage, never the Operator view or ordinary event.
4. A later Task references the Environment but receives fresh Task/Attempt grants,
   SecretLeases, control and execution leases. Resource revisions are revalidated.
5. Suspend waits for use refs, Invocations and Effects; resume probes identity/health and
   revalidates sources. Destroy additionally waits for checkpoint/artifact retention holds.
6. At `LIMIT_REACHED`, stop new admissions, safely checkpoint/suspend, settle in-flight
   Invocations/Effects, and mark dependent Steps `BLOCKED` with `BUDGET_EXCEEDED`. There is
   no v1 in-place top-up/reset. To select an existing Environment, the Operator requests a
   fresh step placement preview, shows eligible candidates/blockers, and sends the chosen
   candidate plus `plan_digest` in `recover_step`; TaskService revalidates the digest and
   eligibility before creating an Attempt. The user may instead run provision preview and
   confirmation for a distinct replacement, then refresh placement options. Private
   provider state is not cloned implicitly, and continuation starts a new Attempt with
   explicit inputs.

**Failure/UI/postcondition:** provision timeout is reconciled by provider request ID
before allocation retry. Unknown provider state blocks duplicate create/destroy. Display
provisioning/failure/current consumer, selected vs actual budget enforcement, cumulative
usage/confidence/observation time, and cost/retention policy; reaching the ceiling blocks
new use and starts safe suspension. The UI offers eligible replacement options and names
the state-transfer limitation. Substrate deletion does not delete Task provenance.
Workspace archive retains the Environment suspended only after provider quiescence is
established.

## F44 — Needs You aggregation and deduplication

**Actors/preconditions:** Operator query, NeedsYouQueryService, Approval/UserRequest/Task
projections; authenticated owner of the selected Workspace.

1. Read open underlying records, check Workspace access and materialize a rebuildable
   inbox projection. A linked blocker references its approval/request item rather than
   adding a duplicate. An unlinked blocker uses `(task_id, blocker_id)` identity.
2. Exclude notification deliveries, resolved requests and nonactionable progress from the
   open count. Include a local-Runtime blocker only when it names an actionable dependency.
3. Route action to the specific Approval/UserRequest/Task/Runtime contract; the inbox GET
   cannot mutate decisions. Late/expired projections are rechecked at command time.

**Failure/UI/postcondition:** offline cached counts are labeled stale. A failed/retried
notification does not duplicate an inbox item, and responding to a UserRequest never
resolves an Approval.

## F45 — Reply to one pending UserRequest from a channel

**Actors/preconditions:** Authenticated ChannelAdapter and ChannelBinding, owner with
`RESPOND`, NotificationService, ChannelService, ConversationService, and one pending,
unexpired FORM UserRequest with a channel-compatible response schema.

1. NotificationService sends one request-specific prompt through an adapter that
   supports provider reply references, pinning the assigned Runtime and host epoch for
   that attempt. After acknowledged delivery on the still-current assignment,
   ChannelService stores
   a Runtime-local `ChannelReplyTarget` mapping from
   `(channel_binding_id, provider_message_ref)` to that exact
   UserRequest. No target is created for a grouped inbox digest, Approval, EXTERNAL_URL,
   unsupported schema, failed send, or ambiguous delivery.
2. On inbound reply, ChannelService deduplicates and claims the ChannelEventReceipt. It
   requires the authenticated sender Principal to match the binding identity, active
   binding with current `RESPOND`, sufficient assurance, exact active `reply_to` target,
   same Workspace, pending/unexpired request, and compatible schema. Plain text without
   an exact reply-to does not select a request.
3. For a bounded root string schema, use the bounded text as the string value. For a
   choice request, require one unique exact choice ID or label and use its pinned value.
   Reject attachments, nested/multi-field schemas, external sign-in, Approval actions,
   invalid values, and suspicious credential-like content. Run the same schema and
   sensitive-input checks as the Operator response path.
4. In one owning-Store transaction, append immutable UserRequestResponse with channel
   provenance, resolve the request, consume the matched target with this exact provider
   event ID, close sibling targets, accept the ChannelEventReceipt, and append
   `user.request.resolved.v1` with the channel source pair. Conversation/Task continuation
   follows the normal response path and provider acceptance rules.
5. An invalid response rejects the receipt but leaves the target and UserRequest pending
   so a corrected reply can be sent. A response racing expiry, revocation, another
   response, or Task cancellation loses the serialized state transition and cannot
   continue work. A target mismatch never falls through to `STEER` in the same command.

**Failure/UI/postcondition:** Show a safe, non-sensitive correction prompt for rejected
schema input; do not reveal whether another Workspace has a matching target. External
sign-in and Approval notices link to the authenticated Operator surface. A channel reply
cannot create a grant, approve an Effect, or become a generic ConversationMessage.

## F46 — Reassign a channel host Runtime

**Actors/preconditions:** Workspace owner, ChannelService, RuntimeMesh, source and target
Runtimes, channel provider; active ChannelBinding and eligible target offer.

1. Show current Runtime, target adapter readiness, SecretRef availability, provider
   connection state, and any open inbound claims, outbound Effects, or reply targets.
   Preflight target credentials, provider compatibility, and whether ingress can resume
   losslessly from the latest Hub-replicated receipt. The versioned request must set
   `accept_ingress_gap = true` if provider cursor transfer/replay cannot prove continuity;
   otherwise reject it before changing assignment.
2. Serialize inbound receipt admission with the ACTIVE → DRAINING transition on the
   ChannelBinding. An ingress transaction may insert and acknowledge a receipt only after
   it verifies the current ACTIVE assignment, current unexpired source lease, and active
   CHANNEL_HOST binding; receipt insertion and origin sequence allocation commit atomically
   at the authoritative receipt store. A non-authoritative source acknowledges/defer-acks
   the provider only after the Hub confirms durable replication. The Hub serializes receipt
   admission against the drain transition for that binding. If ingress admission commits
   first, its receipt is included in drain reconciliation. If DRAINING commits first, the source inserts no new receipt and does
   not acknowledge/defer-ack that provider event; it leaves delivery retryable for the
   successor. If the provider cannot retry or replay it, the move requires explicit
   `accept_ingress_gap` and records that uncertainty.
3. After DRAINING commits, stop new claims, polls, and outbound sends. Let already-claimed
   receipts settle only under their unchanged, still-valid source lease. A pre-drain
   `RECEIVED` row may remain unclaimed if it is Hub-durable and included in the successor's
   replay frontier; the successor may claim it under its new lease. Reconcile outbound sends
   that may be ambiguous. If the source is available, RuntimeMesh verifies that no
   source-epoch receipt remains `PROCESSING`, outbound Effects are settled/reconciled, and
   every pre-drain `RECEIVED` receipt is Hub-durable and included in the successor's replay
   frontier, then records an immutable
   `ChannelHostDrainProof` pinned to the exact lease id/control version. Proof creation and
   receipt insertion share the binding-level serialization point, so a successful proof
   cannot race a later source receipt. If the source is unavailable or proof cannot be
   established, retain DRAINING and wait until the lease's pinned `safe_reassign_after`
   (expiry plus at least 30 seconds of clock-skew margin). A heartbeat loss or missing lease
   row alone never grants target ownership. Any source-local receipt not yet Hub-replicated
   remains an unsettled drain dependency and blocks quiescent proof. An unreplicated receipt blocks a continuity-safe
   move unless the owner explicitly accepts a possible gap.
4. RuntimeMesh releases the source lease, advances `host_epoch`, commits either the target
   ACTIVE assignment plus its new bounded lease, or an explicitly unassigned/degraded
   ChannelBinding when no eligible target is available, and appends the assignment event
   with source-release and continuity provenance. These changes are one Hub transaction:
   no committed ACTIVE assignment lacks exactly one matching current lease. Lease IDs are
   never reused; each host epoch receives a fresh fencing credential derived from a
   domain-separated immutable identity containing the ChannelBinding, Runtime, host epoch,
   and unique lease ID. A credential digest must differ from the current digest; duplicate
   lease identities/digests fail closed. The target starts/attaches only its channel adapter
   after the assignment and lease commit and its provider/secret checks pass.
5. The target validates/imports its provider cursor or replays from the receipt with the
   largest `(origin_host_epoch, ingress_sequence)` before it acknowledges or advances
   provider ingress. Old receipt claims and ChannelReplyTargets fail the current epoch
   check immediately.
   An identical provider redelivery may be reclaimed by the new host with a larger claim
   epoch; the immutable origin host remains in receipt history. The old Runtime closes
   target rows when it reconnects. The target Runtime does not copy provider message
   references or resend existing notifications; replies to old prompts route to the
   Operator inbox.

**Failure/UI/postcondition:** If source effects, receipt outcomes, replication receipts, or
ingress cursor position remain ambiguous, reassignment waits for reconciliation or an owner
decision. An adapter without stable event IDs and a resumable cursor/replay window cannot
be moved automatically; explicit owner confirmation records `GAP_ACCEPTED`, and the UI
identifies the potential observation gap. A target preflight failure leaves the old
assignment unchanged. If a later failure occurs while draining, the source may
resume only if its lease remains valid and no higher epoch has committed; otherwise the
channel is visibly unavailable until another eligible host is assigned. Stale source events
cannot create messages, answer UserRequests, or trigger Tasks. The UI shows the actual host
and handoff state; changing host does not change binding permissions.

If a Workspace Runtime binding is being revoked and there is no eligible replacement,
RuntimeMesh completes the safe release and removes the ChannelHostAssignment in the same
transaction that records its release; the ChannelBinding remains present but is marked
`DEGRADED`/unassigned. It accepts no new inbound or outbound work until a new eligible host
and lease are explicitly assigned. This unassigned path never deletes receipt history or
reply-target provenance. A committed DRAINING assignment retains its matching lease until
the atomic release-and-move or release-and-clear transaction; legacy DRAINING rows without
a lease cannot be safely inferred as released and fail migration preflight for repair.

## F47 — Enable an installed AgentBinding as a worker

**Actors/preconditions:** Workspace owner, AgentBindingService, AgentAdapter, TrustService;
the Runtime has discovered the AgentProfile and the owner enabled its Workspace binding.

1. Settings → Agents → Subagents lists every bound/discovered agent, including the current
   lead. Inventory presence does not make a profile eligible.
2. Owner selects Enable; UI queries the adapter's current option schema and
   AgentHarnessDescriptor and shows supported session options and required policy.
3. Owner enters a short routing description, instructions, session options, worker limits,
   Environment policy, and optimization preference. Save creates a disabled profile at
   revision 1; a separate action enables that revision.
4. ProfileService rechecks binding, Trust, feature compatibility, and option validity,
   then commits status.

**Failure/UI/postcondition:** No worker process or model call starts. Missing auth routes
to setup; stale/unsupported options block enablement with the field identified. Only
enabled profiles enter the lead's worker catalogue.

## F48 — Add a second worker profile for one installed agent

**Actors/preconditions:** Workspace owner, enabled AgentBinding, adapter option schema.

1. From AgentBinding details, choose Add profile; duplicate settings or start empty. This
   creates a distinct DelegationProfile, not a second installation.
2. Configure a supported session option and a separate routing description, policy
   ceiling, Environment, budget, and concurrency.
3. Save as an immutable revision, preview compatibility, then explicitly enable.

**Failure/UI/postcondition:** Adapter validates opaque model names; a rejected option does
not fall back to the lead model. Profiles keep separate identities and can be independently
selected, disabled, archived, measured, and budgeted.

## F49 — Lead delegates to a heterogeneous worker

**Actors/preconditions:** Active lead Attempt, accepted PlanRevision with a READY target
Step, eligible worker profile, current parent lease/fence.

1. Lead submits a bounded DelegateRequest naming the Step and required capabilities.
2. WorkerSelectionService filters by binding/profile state, descriptor freshness, option
   support, Runtime/Environment/auth, Trust, budget, concurrency/depth, isolation, and
   deadline before policy ranking. `PREFER` may fall back only when the request/policy
   permits; `REQUIRE` fails rather than substituting.
3. TaskService commits child Attempt/profile revision/admission provenance, a distinct
   lease, budget reservation, and child-scoped grants. AgentSession starts after commit.
4. UI adds a worker branch after child Attempt creation and labels it Working only after
   the AgentSession is active.
5. Outputs return by refs and are checked against pinned TaskSpec/Plan revisions before
   the lead integrates them.

**Failure/UI/postcondition:** Admission failure has no child Attempt or model call. A
missing Step requires plan revision through TaskService; no dynamic Step is inserted.

## F50 — Verification failure escalates to another profile

**Actors/preconditions:** Child result and pinned acceptance criteria; bounded escalation
policy and verifier are available.

1. Verifier evaluates exact input/criterion revisions; worker self-report is not Evidence
   of success.
2. On recoverable failure, TaskService records result and checks retry count, budget,
   deadline, and remaining eligible candidates.
3. Each escalation creates a new Attempt on the still-READY Step or an explicitly revised
   successor Step, with fresh session/lease/grants and VerificationRun.
4. Stop on pass, exhausted budget/attempts, no eligible worker, or nonrecoverable failure.

**Failure/UI/postcondition:** Attempt identity never changes and escalation never loops
without bound. Show unmet criteria and evidence; inconclusive is not a success checkmark.

## F51 — Disable or revise a profile during an active child Attempt

**Actors/preconditions:** Owner command races with admitted child Attempts.

1. ProfileService serializes status/revision changes using expected version.
2. Disable blocks new admission after commit; revision changes future Attempts only.
   Current children retain pinned profile revision/session options and continue only while
   binding, grants, lease, and parent Task remain valid.
3. Explicit cancellation follows safe-stop and Effect reconciliation.

**Failure/UI/postcondition:** If disable commits first, stale admission fails with no
fallback. If admission commits first, provenance remains. UI distinguishes “Disabled for
new work” from “Stopping current work”.

## F52 — Observe low quota and prewarm a fallback

**Actors/preconditions:** Active Task, quota observation from adapter/provider, eligible
fallback profile.

1. Adapter reports NORMAL, LOW, EXHAUSTED, or UNKNOWN with source/time; missing data is
   UNKNOWN.
2. On LOW, policy may prewarm through the relevant host owner: start/attach host, validate
   auth/config, resolve Runtime/Environment, and prepare bounded handoff context.
3. Prewarm creates no Attempt/session/model invocation, execution authority, or lead
   switch, and may be evicted under resource pressure.
4. Only a later authorized handoff/admission can start fallback work.

**Failure/UI/postcondition:** Show readiness only from current observations. Do not invent
quota percentages or imply a provider cache will be preserved.

## F53 — Lead quota exhaustion and lead change

**Actors/preconditions:** Current lead cannot continue; an eligible lead binding exists or
the owner can choose one.

1. Record EXHAUSTED only when definitively reported; otherwise classify unavailable quota
   as UNKNOWN. Stop new children from the old lead and reconcile existing children/Effects.
2. Build a LeadHandoffPacket from durable Task/Plan/Step/Artifact/Evidence/Effect state,
   remaining budget, and unresolved questions. Exclude transcripts, hidden reasoning,
   session handles, and secrets.
3. User selects the new lead or explicit Task/Coworker policy authorizes an eligible
   fallback. TaskService commits the lead change; a fresh AgentSession resumes from the
   packet under current Trust and lease rules.

**Failure/UI/postcondition:** Lead change differs from delegation and worker replacement.
Without authorization or viable lead, show a blocker; no silent model/binding switch.

## F54 — Native configuration changes while a host is warm

**Actors/preconditions:** AgentHostSupervisor has a warm host; adapter observes a change
to normalized non-secret effective configuration.

1. Before new admission, re-probe descriptor and option compatibility.
2. Existing sessions finish only if the adapter guarantees configuration is frozen and
   isolated; otherwise settle at a safe boundary and reconcile.
3. New admission requires the updated descriptor and explicit review of changed supported
   options/policy. LiteCowork never rewrites native config.

**Failure/UI/postcondition:** Return `AGENT_NATIVE_CONFIG_CHANGED` or
`AGENT_SESSION_OVERRIDE_UNSUPPORTED`; offer Review changes/Revalidate. Never restore old
configuration or silently select another model.

## F55 — Create a Coworker and start its first Task

**Actors/preconditions:** Workspace owner; agent setup may be complete or deferred.

1. First-run setup creates/selects the primary Coworker with the neutral defaults
   (“Assistant”, “General-purpose assistant”); renaming/role customization can be skipped
   and edited later. Optional context, notification, and worker preferences remain
   explicit; avatar is optional.
2. CoworkerService creates the identity and revision; it provisions no host and creates
   no background work.
3. The Operator sends the selected Coworker ID (normally prefilled from Workspace primary)
   and optional expected Coworker version with the ordinary Task request. TaskService
   snapshots the current Coworker revision in the same transaction as TaskSpec and source
   message creation. The client cannot submit a historical revision as current authority.
4. Lead binding resolution is explicit Task choice, then Coworker revision default, then
   Workspace default. The first configured choice must be usable; an unavailable choice
   fails without silent fallback. Lead eligibility, enabled profiles, context/resource
   permissions, budget, and Trust are rechecked before Attempt admission.

**Failure/UI/postcondition:** Missing eligible lead preserves the draft and opens setup.
A paused Coworker blocks proactive/scheduled admission but does not cancel existing Tasks;
an explicit owner-submitted Task may still name it and follows normal lead/Trust checks.
An archived Coworker cannot be selected as a new Task origin.

## F56 — Accept a Suggestion into ordinary work

**Actors/preconditions:** Owner sees a non-expired PROPOSED Suggestion with source
provenance and a valid TaskSpecProposal.

1. “Why this?” shows sources, intended action, expected authority, estimate confidence,
   and what acceptance creates.
2. SuggestionService rechecks expiry/status, proposal digest, exact pinned Resource and
   Goal revisions, source visibility/freshness, current lead/budget/Trust, and idempotency.
3. For TASK, one transaction creates an ordinary Task and resolves Suggestion with its
   Task ID. Execution continues only through ordinary Task admission.
4. Routine/Automation proposals open their editor and are not resolved as accepted until
   the owner saves through that owning service.

**Failure/UI/postcondition:** A race, stale/conflicted source, expiry, or invalid authority
leaves the proposal unaccepted. A stale source opens Review/update and cannot silently
advance to its latest revision. Accepting never grants permissions, installs packages,
sends, or schedules work by itself.

## F57 — Link a Goal and show evidence-backed progress

**Actors/preconditions:** Owner creates/revises Goal and references same-Workspace Tasks and
Routine revisions.

1. GoalService validates references and writes an immutable revision; links are
   provenance/context only.
2. The current read projection loads same-Workspace Task statuses and bounded committed
   Evidence IDs, plus Evidence IDs that resolve from each exact Goal-pinned Artifact
   version. It reports `PARTIAL` because VerificationRun and dependency-freshness
   readers are not integrated. `COMPLETED` Task status alone remains `UNVERIFIED`; only
   explicit `INCOMPLETE`, `FAILED`, or `CANCELLED` Task statuses map to `INCOMPLETE`.
   Unsupported verified/stale/conflicted counts are null with typed limitation codes.
3. When VerificationRun and dependency readers are integrated, the projector may report
   `VERIFIED`, `STALE`, or `CONFLICTED` only from criterion/input-bound records. Worker
   reports can suggest progress but cannot complete the Goal. Only owner command changes
   Goal status to COMPLETED.

**Failure/UI/postcondition:** Missing/stale/conflicted sources stay visible and do not
count as verified progress. Goal status never mutates linked Tasks or Routines.

## F58 — Resolve a concurrent ContextDocument edit

**Actors/preconditions:** Two devices read one Resource revision and submit edits.

1. Each edit proposes a new immutable ResourceRevision with parent and expected head.
2. First commit advances the Resource head; stale second edit returns conflict while both
   revisions remain available.
3. Owner compares, selects, or writes an explicit merged revision. A context provider
   indexes only committed revisions permitted by policy.

**Failure/UI/postcondition:** No last-writer-wins. Current TaskSpec/user input outranks
Workspace/Goal/Coworker documents and retrieved historical context; retrieval ranking
cannot silently resolve a content conflict.

## F59 — Deadline-sensitive preflight and execution

**Actors/preconditions:** Task requests `DEADLINE_SENSITIVE`, with bounded deadline,
authority, and a supported provider path.

1. Before the critical window, preflight checks Runtime/lease, lead/options, auth,
   Environment, input freshness, capability, budget, approval readiness, and fallback.
2. Any required failure is reported before execution; no unsafe partial sequence starts.
3. Execute through the fastest semantically equivalent authorized method:
   structured API, structured browser, accessibility browser, then computer use. Record
   the method and re-authorize a changed fallback.
4. ActionBatch runs bounded operations with per-operation pre/postconditions and abort
   checks. Consequential suboperations still have their own Effect/Evidence and
   reconciliation records.
5. At a human authorization boundary, stop before the consequence and transfer control
   through EnvironmentControlLease.

**Failure/UI/postcondition:** Stale page/input, unsupported fallback, missed deadline,
approval gap, or uncertain Effect stops safely and yields Needs You. Show observed checks
and elapsed time only; no realtime promise or fake ETA.

## F60 — Shared browser profile and human takeover

**Actors/preconditions:** Persistent browser Environment has explicit sharing scope and a
current Agent or Human EnvironmentControlLease.

1. EnvironmentManager verifies Workspace/Coworker ownership, health, and current epoch.
2. An action is accepted only from the lease owner/epoch. Another worker waits; it cannot
   concurrently type/click in the same shared profile.
3. Human takeover increments control epoch, discards queued stale agent input, and exposes
   controls only after HUMAN ownership commits.
4. Returning control requires fresh page observation, Resource/Effect reconciliation, and
   a new Agent control epoch.

**Failure/UI/postcondition:** A lease conflict queues no input. UI labels the current
controller and states that the control lease is separate from the Task ExecutionLease.

## F61 — Demonstration becomes a reviewed SkillProposal

**Actors/preconditions:** Owner explicitly starts a DemonstrationSession in a supported
Environment with semantic observation and input fencing.

1. Capture bounded semantic targets, page/resource state, and action intent while the
   owner operates; redact secrets to named placeholders.
2. On Finish, persist trace as a Resource and enter REVIEW. Owner edits steps, typed inputs,
   outputs, and verification criteria.
3. Conversion creates a draft SkillProposal through the existing test/review lifecycle;
   it is not installed or invoked automatically.
4. LiteSPM remains package discovery/lifecycle authority for a published package;
   LiteCowork owns only Task-scoped activation/grants.

**Failure/UI/postcondition:** Abort or secret detection follows retention policy and
settles the session; coordinate-only raw replay is never published as a Skill.

## F62 — Cost ceiling stops new delegation

**Actors/preconditions:** Task/profile BudgetSpec and Usage/BudgetReservations; a child is
active or requested.

1. BudgetService reconciles usage by unit, currency, source, and confidence. Unknown spend
   remains unknown, never zero.
2. At threshold, apply the pinned response: warn, reduce concurrency, prefer cheaper
   eligible profiles, require approval, or stop new delegation.
3. Existing Attempts settle/reconcile under their own grants/Effects; the ceiling does not
   kill a worker mid-effect or rewrite its model/profile.
4. Resume new admissions only after explicit budget/approval update under versioned policy.

**Failure/UI/postcondition:** Show observed provider units and known estimates with source,
confidence, and time; label unknown spend. Never show fabricated `$0.00` or silently
switch workers.

## F63 — Worker crashes with a potentially ambiguous Effect

**Actors/preconditions:** Child Attempt loses process/session during or after a
Core-mediated consequential operation.

1. Mark session/Attempt lost, retain relevant Effect/lease history, and prevent new
admission that could repeat unresolved external work.
2. EffectReconciler checks the idempotency identity and authoritative provider result.
   Never redispatch an uncertain operation blindly.
3. After reconciliation, settle the lost Attempt and evaluate retry/escalation. Any
   replacement is a new Attempt with new session/grants/lease and pinned source refs.
4. Notify the parent lead with reconciled outcome, Evidence, and unresolved blockers.

**Failure/UI/postcondition:** Ambiguity remains blocked or enters Needs You. Independent
children may continue only if their dependencies and parent authority remain valid. A
worker report never becomes verified completion by itself.

## F64 — Reuse a Coworker-private Environment

**Actors/preconditions:** Environment has `COWORKER_PRIVATE` sharing, explicit lifetime,
healthy provider observation, and a future Task from that Coworker.

1. EnvironmentManager verifies same Workspace/Coworker, classification, freshness,
   Runtime incarnation/provider binding, no conflicting active writer, and budget/retention.
2. Reuse creates a new Environment use reference, not an old Attempt lease, grant, browser
   control lease, or native session.
3. Concurrent code writers receive private worktrees/overlays; shared browser input has
   one current EnvironmentControlLease owner.
4. On settlement, release current use reference and retain/destroy only under explicit
   lifetime policy and verified provider result.

**Failure/UI/postcondition:** Missing provider handle, stale auth, conflict, or unhealthy
Environment makes it unavailable pending reconciliation/reprovision. UI separates “saved
for this Coworker” from “currently in use”.

## F65 — Change a persistent Environment sharing scope

**Actors/preconditions:** Workspace owner, EnvironmentManager, TrustService; a persistent
Environment is `SUSPENDED`, current-versioned, and has no active Attempt, Invocation,
control lease, unresolved Effect, or checkpoint hold.

1. The owner chooses between `COWORKER_PRIVATE` and `WORKSPACE_SHARED`. Confirmation names
   the current and proposed reuse boundaries and eligible Task set.
2. A Coworker-private target must name an active/paused same-Workspace Coworker; a
   Workspace-shared target clears the Coworker owner. `USER_SHARED` is unavailable.
3. EnvironmentManager rechecks expected version, suspension, holds, owner, and policy in
   one mutation transaction. It changes scope/owner and appends
   `environment.sharing_scope.changed.v1` atomically.
4. The Environment remains suspended. Later Tasks pass fresh placement, health, resource,
   Trust, and lease checks; no grant or control lease transfers.

**Failure/UI/postcondition:** A stale version returns conflict. Active use, unresolved
Effects, or checkpoint holds reject the command and retain the prior scope. The UI updates
only after commit and makes no claim that data was copied or a Task started.

## F66 — Detect pinned Skill or Routine dependency drift

**Actors/preconditions:** Routine-created Task, Skill/Capability adapter, verifier,
ProjectionService, TaskService, owner. The Task pins immutable Routine/Automation and
Capability revisions.

1. Before replay or a consequential operation, the adapter performs semantic preflight or
   the verifier checks the output against the pinned criteria. A transient outage is
   `WARNING`; only explicit incompatibility evidence is `DRIFTED`.
2. Persist blocker `SKILL_DRIFT_DETECTED` and its Evidence/observation reference.
   RoutineHealth becomes `DRIFTED`; no Skill, RoutineRevision, AutomationRevision, or
   TaskSpec pin is rewritten.
3. Needs You offers Review, disable the affected Automation, or prepare a repair. Repair
   creates a SkillProposal through the normal redaction/test/review path; publication
   waits for explicit approval and LiteSPM confirmation.
4. The owner explicitly revises the Routine and then any Automation revision that should
   adopt the new Skill. New Tasks pin new revisions; existing Attempts remain unchanged.

**Failure/UI/postcondition:** Unknown compatibility remains `UNKNOWN` and blocks unsafe
replay when required by policy. Failed repair leaves existing pins intact. The UI names
the evidence and affected revisions; it never claims automatic repair or silently reruns
an external Effect.

## F67 — Snooze a Suggestion

**Actors/preconditions:** Workspace owner, SuggestionService, current `PROPOSED`
Suggestion; owner supplies an expected version and one of the UI presets Later today,
Tomorrow, or Next week.

1. Resolve the preset against the service Clock. Require `now < snoozed_until <= expires_at`;
   presets later than expiry are unavailable and the card explains that the suggestion
   expires sooner. The API never silently clamps a timestamp.
2. In one optimistic mutation, persist `snoozed_until`, increment Suggestion version,
   and append `suggestion.visibility.changed.v1`. Status remains `PROPOSED`. A “Show now”
   action sets `snoozed_until=null` through the same command.
3. ProjectionService excludes it from Home/Ideas until that time. The Snoozed Ideas view
   can still list it and offers “Show now.” Expiration processing may settle it sooner; a
   snoozed record is never resurrected after terminal resolution.

**Failure/UI/postcondition:** Stale version returns conflict. An already expired or
resolved Suggestion returns `SUGGESTION_EXPIRED` or `CONFLICT` as appropriate and is not
made visible again. A worker or generated suggestion cannot snooze itself.

## F68 — Mute a SuggestionKind

**Actors/preconditions:** Workspace owner, SuggestionService; preference
version is current (missing preference means unmuted version zero).

1. The owner chooses “Don't suggest this type” for the card's deterministic kind or
   changes the kind in Settings. UI states that existing proposals of this kind will be
   cleared and future ones suppressed.
2. SuggestionService checks `If-Match` and authorization. In one transaction, it updates
   the preference and resolves every currently `PROPOSED` Suggestion of that kind as
   `DISMISSED` with reason `MUTED_KIND`, appending their resolution events and
   `suggestion.preference.changed.v1`.
3. SuggestionService checks the current preference and the exact-key 30-day dismissal
   cooldown on every new proposal. Muted proposals create no Suggestion or domain event;
   a metric may record suppression without source content.
4. Unmuting changes the preference for future proposals only; it does not reopen
   Suggestions cleared by the mute.

**Failure/UI/postcondition:** Version conflict changes nothing. The card set and Settings
label update only after commit. Preference state replicates with Workspace events; source
content, ranking scores, and suppressed proposal text do not.

## F69 — Reopen a completed Goal

**Actors/preconditions:** Goal owner, GoalService; Goal is `COMPLETED` and current version
is supplied.

1. Owner explicitly selects Reopen. GoalService applies the expected-version check, sets
   status to `ACTIVE`, increments the aggregate version, and appends
   `goal.status.changed.v1`.
2. GoalProgressProjection continues to show the existing linked Task/Evidence history and
   current progress. Reopening does not reset evidence, reopen Tasks, revise Routines, or
   create a Task.

**Failure/UI/postcondition:** A stale version returns `CONFLICT`; an `ARCHIVED` Goal
returns `GOAL_ARCHIVED`. The UI labels Goal status separately from derived progress.

## F70 — Duplicate a DelegationProfile

**Actors/preconditions:** Workspace owner and DelegationProfileService; source profile is
non-archived, its current version is known, and its AgentBinding remains in the Workspace.

1. Owner chooses Duplicate. UI shows the source's pinned current revision, adapter-option
   descriptor digest, and the settings that will be copied; it asks for a new profile
   name. It explains that the new profile starts disabled and has no authentication or
   execution history.
2. Operator sends `POST /v1/delegation-profiles/{id}/duplicate` with `If-Match`, a fresh
   `Idempotency-Key`, and the new name. DelegationProfileService resolves and authorizes
   the source in the selected Workspace, verifies the source version, normalizes the name
   (trim + Unicode NFC) and computes its Unicode case-folded key.
3. In one transaction, service allocates a new DelegationProfile on the source's same
   AgentBinding, checks the name-key uniqueness index, copies the exact current non-secret
   revision values as new revision 1 with the new name and current author/time, sets
   status `DISABLED`, and appends `delegation_profile.created.v1` with its canonical
   state blob. It copies no Attempts, AgentSessions, native handles, grants, Approvals,
   SecretLeases, budgets/reservations, Environments, Runtime placement, or performance
   projection. It never starts a process or invokes a model.
4. On response, UI shows the new disabled profile. If the pinned option descriptor is no
   longer current, it marks the profile “Needs review”; the owner must revise/revalidate
   it before enablement. Enabling is a separate command and rechecks binding, adapter
   options, Trust policy, Environment policy, and current descriptor.

**Failure/UI/postcondition:** Repeating the same idempotency key and normalized request
returns the same created profile before re-evaluating the old `If-Match`; using the same
key with different input conflicts. A stale source version returns `CONFLICT`; a source
archived before commit returns `DELEGATION_PROFILE_ARCHIVED`; duplicate name returns
`CONFLICT`. Any failure before commit creates no profile or event. The source profile and
all its Attempts remain unchanged. Portable export/import is outside v1.

## F71 — Policy-based lead failover

**Actors/preconditions:** TaskService, LeadFailoverService, current lead adapter/Runtime
observer; Task has a pinned `TaskSpecRevision` and `LeadFailoverPolicy`.

1. An authenticated observer records a typed, expiring
   `LeadFailoverTriggerObservation`. The observation identifies the affected binding,
   source, source observation, and Runtime incarnation where relevant; it contains no
   quota secret or provider handle.
2. LeadFailoverService loads the current TaskSpecRevision. `DISABLED` yields no action;
   `ASK` opens a Needs You decision without changing the Task lead. For
   `ALLOW_LISTED`, the service checks the trigger and change budget, then considers
   fallback bindings in their pinned order.
3. Each candidate is revalidated for Workspace membership, binding enabled state,
   `lead_eligible`, supported adapter options, endpoint, current Runtime/incarnation,
   authentication, Trust, inputs, deadline, and remaining budget. No candidate receives
   inherited grants, approvals, SecretLeases, native session IDs, or Attempt ownership.
4. At a safe planning boundary, TaskService uses expected Task version to fence new work
   from the old lead, settle/close its planning session, create a bounded handoff from
   durable Task/Artifact/Evidence state, and append `task.lead_agent.changed.v1` with
   `cause=POLICY_FAILOVER`, service actor, TaskSpec revision, and observation. A fresh
   lead session is admitted only after ordinary checks pass.

**Failure/UI/postcondition:** A stale observation, TaskSpec change, competing owner
change, or stale Task version causes re-evaluation; it never overrides the user's newer
choice. If no fallback is eligible or the change limit is reached, no lead-change event is
written and Needs You explains the blocker. Existing Attempts remain pinned. A manual
owner change uses `cause=OWNER_REQUEST` and a principal actor and follows the same fencing
and handoff boundary.

## F72 — Coworker-owned Automation creates a Task

**Actors/preconditions:** AutomationService, TriggerCoordinator, CoworkerService,
TaskService; the selected AutomationRevision pins a same-Workspace `CoworkerRevisionRef`.

1. TriggerCoordinator claims the occurrence under its existing trigger-host epoch and
   exact Automation/Routine revisions.
2. Before materializing a Task, TaskService checks that the Automation is still enabled,
   the current Coworker is `ACTIVE`, the pinned Coworker revision remains available, and
   its lead/profile bindings are currently eligible. A paused or archived Coworker blocks
   new Coworker-originated scheduled Tasks even if the occurrence was already claimed.
3. In one transaction, TaskService creates the ordinary Task and initial TaskSpecRevision,
   pins both Automation and Coworker provenance, advances the occurrence to STARTED, and
   writes their events. The effective lead-failover policy and current Coworker options are
   materialized in that Task's immutable TaskSpecRevision.
4. Existing Tasks are unaffected by later Coworker pause/archive or Automation edits.

**Failure/UI/postcondition:** A pause/archive racing Task creation serializes on the
Coworker/occurrence admission boundary. If pause/archive wins, no Task is created and the
occurrence remains blocked/skipped under its policy with an explanatory health signal. An
explicit owner-submitted Task may still use a PAUSED Coworker as origin.

## F73 — Bounded ActionBatch with partial failure

**Actors/preconditions:** AgentSession, adapter, CapabilityBroker, InvocationRunner,
EffectService; an Attempt-scoped grant covers each operation.

1. The adapter produces an ordered ActionBatch with 1–64 operations, versioned
   pre/postcondition refs, abort conditions, and a canonical digest. Core resolves one
   exact execution method before member admission.
2. CapabilityBroker admits every member as an ordinary CapabilityInvocation with the
   shared batch ID/count/digest, contiguous zero-based ordinal, same execution method,
   independent request digest/idempotency key, and its own authorization decision. The
   batch is not dispatched unless all members have been admitted successfully.
3. Immediately before each member dispatch, Core rechecks lease, grant, approval,
   preconditions, and abort conditions. A consequential member receives its own Effect;
   the Effect pins that Invocation and method.
4. If a member fails or an abort condition becomes true, later undispatched members are
   cancelled. Every earlier dispatched member and Effect is settled/reconciled separately
   before fallback or continuation. A single provider transport call is allowed only if
   it reports separately correlatable outcomes for every member.

**Failure/UI/postcondition:** There is no transaction-wide rollback claim. The Task
shows each member's actual status and any ambiguous Effect; Verification uses the
individual Evidence/Effects. Replaying the whole batch under new idempotency identities is
not an automatic recovery.

## F74 — Concurrent Resource revision edit

**Actors/preconditions:** ResourceService and two authenticated editors; both selected the
same Resource head/version.

1. Each editor creates a revision-upload session with `If-Match`, expected Resource
version, and the exact observed head revision set before uploading chunks.
2. The first valid commit verifies the digest, atomically appends the immutable
   ResourceRevision and parent edges, advances the head/version, and emits
   `resource.revision.observed.v1`.
3. The second commit rechecks expected version and exact current head set. If stale, it
   returns `RESOURCE_CONFLICT`, accepts no new revision, and preserves the uploaded bytes
   only as an unreferenced temporary blob subject to bounded cleanup.
4. The editor explicitly rebases or merges by naming every current head, then starts a new
   upload session. No last-writer-wins promotion occurs.

**Failure/UI/postcondition:** The UI identifies the changed heads and offers compare,
rebase, or explicit merge. A branch choice alone does not create a merge.

## F75 — ContextDocument revocation and deletion

**Local V1 implementation status:** the authenticated owner route currently implements
only `ACTIVE -> REVOKED` and `REVOKED -> ACTIVE`, with Resource-version compare-and-swap,
idempotency receipt, snapshot, and status event committed atomically. Existing SQLite read
admission denies new reads while revoked. The daemon does not yet wire Task-context
attachment invalidation or stop/replace an already-fed native session. Deletion planning,
`DELETION_PENDING`, replica receipts, and `DELETED` remain target behavior and are not
available through the local Operator.

**Actors/preconditions:** ResourceService, PersonalContextService, registered replica
providers, purge reconciler; owner supplies expected Resource version.

1. Revocation changes `ACTIVE -> REVOKED`, emits the status event, and immediately fences
   future ResourceResolver reads and new Task context attachment while retaining bytes.
2. Deletion enumerates every registered Core-owned content/index replica and the exact
   ResourceRevision IDs it stores, canonicalizes the ordered target list, computes the
   manifest digest, and writes an immutable purge plan. In one Resource transaction it
   creates one pending receipt per target, sets `DELETION_PENDING` with the digest/count,
   appends the tombstone event, and makes all content reads fail.
3. Each replica deletes the listed content and returns an idempotent acknowledgement bound
   to its target identity, Runtime incarnation if applicable, and exact revision set.
   Core records a receipt digest and `resource.context_document.purge.acknowledged.v1`.
4. The reconciler sets `DELETED` only when acknowledged receipts exactly cover the sealed
   plan. A zero-target plan is still sealed and verified. Provider-owned replicas remain
   pending until their registered adapter confirms deletion.

**Failure/UI/postcondition:** Retryable failures leave the Resource tombstoned and unreadable;
they never restore content. Active agent sessions that already received the content are
stopped/replaced at a safe boundary; native history cannot be recalled. Historical
Task/Evidence IDs and digests remain, while deleted content bytes do not enter backup
restore.

## F76 — Suggestion producer admission and suppression

**Actors/preconditions:** Registered `SuggestionProducer`, SuggestionService, Workspace
owner; producer receives only a bounded, authorized context projection.

1. A typed trigger invokes eligible producers; each returns ephemeral candidates with
   `proposed_by`, exact source/Goal revision refs, reason, action, optional TaskSpec, and
   expiry. A producer cannot write Suggestion or Task state.
2. SuggestionService authenticates producer registration, validates provenance/scope and
   expiry, derives the deterministic kind/dedupe key, checks mute preferences and the
   dismissal cooldown, and limits candidate volume.
3. Suppressed candidate text is discarded without a domain event. An accepted candidate
   becomes an immutable Suggestion with `proposed_by`; owner acceptance creates an ordinary
   Task/editor action through the normal service transaction.

**Failure/UI/postcondition:** Invalid, stale, cross-Workspace, muted, duplicate, or expired
candidates do not appear on Home. “Why this?” resolves only the safe pinned source refs
that the owner is authorized to inspect.

## F77 — Bounded Teach-a-task capture

**Actors/preconditions:** Owner, DemonstrationSessionService, currently controlled
Browser/Desktop Environment; immutable policy pins caps and sensitive-region handling.

1. Owner starts capture. The session validates Environment class and pins maximum duration,
   action count, trace bytes, and `PAUSE_ON_DETECTION` or `OMIT_SENSITIVE_FIELDS`.
2. Semantic element/action observations append to a bounded Resource trace. Credentials,
   passwords, one-time codes, and configured sensitive fields become placeholders or are
   omitted; coordinate-only traces do not become replay authority.
3. Owner may pause and resume capture in the same authorized Environment. Sensitive-region
   detection or any configured cap pauses/stops capture; the UI explains the reason and
   offers review, continue-after-redaction where allowed, or abort.
4. On completion, owner reviews the trace; conversion creates an ordinary SkillProposal
   draft with typed inputs and verification requirements. Publishing remains the existing
   separate approval/LiteSPM package flow.

**Failure/UI/postcondition:** Expired Environment authority, cap exhaustion, trace write
failure, or a Runtime-incarnation change pauses or aborts the capture. No partial trace is
silently promoted into an enabled Skill.

## F78 — Workspace worker enabled versus Coworker assignment

**Actors/preconditions:** Owner, DelegationProfileService, CoworkerService; both profile
and Coworker revisions use expected-version commands.

1. The owner enables an installed AgentBinding as a DelegationProfile. This only makes the
   profile Workspace-eligible; it does not start a process or grant it to any Coworker.
2. The Subagents roster shows `Enabled · used by Alex` or `Enabled · not assigned`, and a
   profile detail view lists Coworkers that currently allow it.
3. To assign a profile, CoworkerService creates a new CoworkerRevision whose
   `enabled_delegation_profile_ids` includes the enabled same-Workspace profile.
4. At every child Attempt admission, profile status, Coworker allowlist, AgentBinding,
   harness options, Trust, budget, Runtime, and Environment are revalidated.

**Failure/UI/postcondition:** Disabling or archiving a profile blocks new admissions but
does not rewrite/cancel Attempts that already pinned it. Removing a profile from a Coworker
revision affects future Tasks/children only.

## F79 — Native turn interruption with a live spawned command

**Actors/preconditions:** TaskService, AttemptRunner, native AgentAdapter, owned
AgentHost/Environment, and a replacement profile; an admitted sender is running a command
that can produce an observable heartbeat or write marker.

1. Request interruption and record the provider's turn acknowledgement/completion as
   provider turn state only. Do not infer process exit or revoke stale write risk from an
   `interrupted` status.
2. Stop or fence the owned host/Environment using the adapter's qualified containment
   mechanism. Wait for the relevant process scope to exit or otherwise prove it cannot write
   to the Attempt's mutable resources.
3. Reconcile outstanding CapabilityInvocations and Effects, capture the checkpoint, and
   settle/release the prior lease. If host stop or quiescence cannot be proved, keep the
   Attempt stopping/ambiguous and block replacement admission.
4. Verify the checkpoint/version manifest and admit a fresh replacement Attempt under a
   higher lease epoch. Observe the write marker through the handoff window and reject any
   stale sender write.

**Failure/UI/postcondition:** Turn interruption is not a safe-switch fence. Show “Stopping
sender” or a concrete blocker until the owned writer is fenced and quiescent. Do not report
handoff complete or start the replacement while old write authority may remain.

## F80 — Runtime restarts during replacement startup

**Actors/preconditions:** RuntimeLifecycleService, AttemptRunner, AgentHostSupervisor, and
TaskService; a replacement Attempt admission was committed under an older Runtime incarnation,
but startup did not reach a durable running state before that Runtime stopped unexpectedly.

1. Start the new Runtime in `RECOVERING` under a fresh `RuntimeIncarnation`. Keep new Task and
   worker admission closed while old-incarnation execution handles are reconciled.
2. Resolve the old Attempt's launch owner and every owned process/Environment handle using the
   platform's verified process identity and containment evidence. A PID or provider turn
   status alone is insufficient.
3. If all writers are proven quiescent, reconcile CapabilityInvocations and Effects, settle
   or abandon the prior Attempt through its ordinary recovery path, and release/fence its
   lease. Preserve the consumed lease epoch and pinned handoff checkpoint.
4. Admit a fresh Attempt only after recovery commits, using a higher lease epoch and a new
   AgentSession pinned to the new Runtime incarnation.
5. If owner identity, quiescence, Effect state, or lease fencing remains uncertain, keep the
   old Attempt unresolved, leave Runtime readiness degraded, and block replacement writers.

**Failure/UI/postcondition:** Never clear an `ADMITTED` slot or retry only because its Runtime
restarted. Expose the verified recovery state or the specific blocker. Recovery is idempotent;
it cannot create two replacement Attempts for the same lease epoch or replay an ambiguous
Effect.

## F81 — Runtime restarts after a replacement Attempt reaches RUNNING

**Actors/preconditions:** RuntimeLifecycleService, AttemptRunner, AgentHostSupervisor,
EnvironmentSupervisor, LeaseCoordinator, EffectReconciler, and TaskService; the replacement
Attempt and its owned process identity were durably recorded as `RUNNING` before the old Runtime
stopped unexpectedly.

1. Start the new Runtime in `RECOVERING`; close new Task/worker admission.
2. Resolve the previous Runtime incarnation, the Attempt's owned host/process tree, and its
   Environment using platform-verified identities and containment evidence. PID absence alone
   is insufficient where identity cannot be read; a live or ambiguous writer keeps the Attempt
   unresolved.
3. Reconcile pending CapabilityInvocations and Effects, then fence/release the prior lease.
   Provider turn status alone cannot establish process quiescence or Effect outcome.
4. After all writers are proven quiescent and Effects/lease are settled, mark the Attempt
   `ABANDONED`, preserve its checkpoint and consumed epoch, and clear its active Task ownership.
5. Admit one new Attempt using the unchanged Task truth, pinned checkpoint, a strictly higher
   lease epoch, and an AgentSession bound to the new Runtime incarnation.

**Failure/UI/postcondition:** Missing process identity, unreadable host state, active writers,
ambiguous Effects, or failed lease fencing leaves the old Attempt unresolved and blocks new
writers. Recovery is idempotent and never labels a previously running Attempt as a startup
failure. Any local PoC booleans for Effect/lease state are test scaffolding only.

## F82 — Render, stream, and recover a Conversation presentation

**Actors/preconditions:** Owner, Operator, ConversationService, PresentationProjector,
authenticated Operator stream; the Workspace is readable and the Conversation turn is
active or settled.

1. Operator fetches the authorized Conversation presentation snapshot and its projection
   revision/cursor. It renders committed messages and linked Task/Artifact/UserRequest items
   by stable source order.
2. For an active turn, the stream may deliver bounded `turn.delta` frames with a
   `retry_ordinal` and monotonic sequence within that retry. The UI appends real text and
   labels it in progress; the frames are transient and are not stored as ConversationMessages
   or domain events.
3. ConversationService commits the final response as a ConversationMessage. The new
   projection replaces transient text and supplies its durable source identity.
4. If the stream disconnects, the client fetches the latest authorized snapshot and resumes
   from its cursor. An expired cursor requires full projection replacement before new
   frames are applied. Duplicate, reordered, stale, or post-settlement deltas are ignored.
   If a live sequence gap cannot be replayed by the current adapter, the Operator drops
   the partial buffer and waits for committed output instead of joining discontinuous text.
5. A failed turn leaves no saved assistant answer from uncommitted deltas; the user may use
   the ordinary retry flow, with earlier committed messages preserved.

**Failure/UI/postcondition:** No historical typing animation is replayed. A partial failed
response is visibly incomplete and never appears as a committed answer. Presentation frames
cannot create a Task, resolve a UserRequest/Approval, or claim an Effect succeeded. See
[`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md) and the Operator stream in `API.md`.

## F83 — Inspect context used for work

**Actors/preconditions:** Owner, Operator, ContextService/ResourceResolver, and an authorized
Conversation turn or Task projection with recorded context attachment/retrieval provenance.

1. The user opens “Context used” from a Conversation response or Task detail.
2. Operator loads the exact resolved context receipt for that scope: explicit attachments,
   instruction revisions, selected ContextDocuments, and provider-retrieved source refs.
3. The view labels each source with scope and available revision/freshness data. It does not
   claim that unselected Resources or all prior conversations were read. Unknown provenance
   is disclosed as unknown.
4. Opening a source rechecks Workspace authorization and resolves the pinned revision.
   Revoked/deleted content is unavailable even when historical Task provenance remains.
5. Editing, revoking, or deleting a ContextDocument routes through ResourceService; the
   inspection surface itself is read-only.

**Failure/UI/postcondition:** Provider loss or missing provenance produces an explicit
limitation, not a fabricated source. Scope violations reveal no source metadata/content.
No memory record is autonomously created or changed.

## F84 — Compare and restore an Artifact version

**Actors/preconditions:** Owner, Workbench, ArtifactStore, ArtifactVersion/ResourceRevision,
and a renderer qualified for the selected content kind.

1. Workbench pins the selected Artifact ID and immutable current version, then loads the
   authorized version list and provenance.
2. If the renderer supports comparison, the user selects two committed versions. Otherwise
   the compare action is absent and both versions remain independently open/downloadable.
3. Restore selects an existing version as content input; ArtifactStore checks current
   Artifact aggregate version and publication state.
4. ArtifactStore appends a new ArtifactVersion and matching ResourceRevision with the
   selected prior content. It never moves the current-version pointer backward or removes
   later versions.
5. A concurrent publication returns the ordinary stale-version conflict. Workbench keeps
   any local dirty draft and offers explicit compare/rebase; it never overwrites it.

**Failure/UI/postcondition:** History order follows committed Artifact version numbers.
Restore is shown as complete only after ArtifactStore commits the new version. Renderer
failure does not imply content deletion. See `ARTIFACTS-EVIDENCE.md` and
[`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md).

## F85 — Import local files into the selected Workspace

**Actors/preconditions:** Workspace owner, desktop Library, authenticated local Operator,
ResourceStore, encrypted BlobStore; the selected Workspace is active and owned by the
local Principal.

1. The user selects files, a ZIP, or a folder as a one-time multi-file attachment. The
   desktop limits a selection to 100 files/100 MiB and each file to 100 MiB; a directory
   selection does not create a persistent WorkspaceRoot. Folder selection filters known
   credential/build paths (for example `.env`, private-key files, `.ssh`, `.aws`, `.git`,
   `node_modules`, and build output) and reports the excluded count. This filename/path
   filter is defense in depth, not secret-content scanning; ZIP internals remain opaque.
2. For folder selections, the WebView normalizes `webkitRelativePath` to slash-separated
   segments and retains that relative path as the compatibility display name. It never
   sends the absolute selected folder path. The WebView passes bounded upload chunks and
   this optional metadata to Tauri; the native client uses authenticated local IPC and
   sends each upload with the exact selected Workspace and a stable Idempotency-Key.
3. Operator rechecks Workspace ownership and ACTIVE status, validates the path at the
   daemon boundary, requires it to match display_name, and rejects absolute/drive/UNC,
   dot-segment, empty-segment, backslash, control-character, overlong, or excessive-depth
   paths. The path is never opened or resolved as a filesystem location.
4. Storage pins `folder_import` in the resumable session and its aggregate snapshot, includes
   it in idempotency and resume matching, and copies it from that session into Resource
   provenance at commit. It encrypts and verifies bytes, then transactionally writes the
   Resource, initial revision/location, event and idempotency receipt. Folder-derived
   Resources emit `resource.created.v2`; other uploads keep `resource.created.v1`.
5. The desktop shows the committed catalog metadata only after the API response. On
   Workspace change/reopen it reloads the bounded Resource list from the local Runtime.
   Content preview, extraction, indexing, and search are not implied by catalog presence.

**Failure/UI/postcondition:** Invalid selection/path, oversized files, inactive/foreign
Workspace, corrupted chunk/digest, BlobStore failure, or database conflict creates no
visible successful item. Retrying the same key and exact path returns the original upload
session; using the key with another path conflicts. Changing a folder path forces a new
resume identity and RequestId. A crash may leave an unreachable encrypted blob for later
garbage collection, but never a partial Resource row. Folder provenance is descriptive;
it does not authorize access to a WorkspaceRoot or file on a later run.

## F87 — Create and list local Workspace instruction revisions

**Actors/preconditions:** Workspace owner, desktop/Operator client, authenticated local
Operator, WorkspaceService, SQLite store, and encrypted Resource BlobStore. The user has
already imported a same-Workspace UTF-8 text Resource no larger than 64 KiB.

1. The client reads the current Workspace version and selects an exact Resource revision;
   the ResourceRef carries Workspace, Resource, and revision IDs, plus its content digest.
2. The client posts the proposed parent revision IDs and pinned ResourceRef with matching
   `X-Workspace-ID`, current Workspace `If-Match`, and an idempotency key. The daemon
   authenticates the native client and confirms Workspace ownership.
3. Operator resolves the Resource through storage, which verifies the encrypted bytes and
   digest. It rejects a foreign Workspace, non-current/unavailable revision, non-text MIME,
   invalid UTF-8, oversize content, or a digest mismatch.
4. WorkspaceService and SQLite validate the current Workspace version, ACTIVE status,
   next instruction revision, and same-Workspace parent revisions. One transaction writes
   the immutable revision, Workspace current-revision/version projection, aggregate state,
   `workspace.instructions.revision.created.v1`, origin sequence, and RequestId receipt.
5. A retry with the same key and payload returns the original committed result. Reusing
   that key for another payload conflicts. A stale version creates no revision.
6. The list endpoint checks the same owner and Workspace scope, then reads the indexed
   immutable revision projection in ascending order using a Workspace-bound opaque cursor.

**Failure/UI/postcondition:** No instruction text is copied into event payloads or logs;
the event stores a pinned ResourceRef and digest. Current implementation is local-only and
has no desktop editor, pagination, Hub authority, or TaskSpec pinning. The request can be
retried safely after a lost response, but this slice is not complete Workspace-instruction
product support.

## F86 — Read and preview a committed local Resource

**Actors/preconditions:** Workspace owner, desktop Library, authenticated local Operator,
ResourceStore and encrypted BlobStore; the Resource belongs to the selected owned
Workspace (active or archived/read-only), and the selected immutable revision has bytes in
the local managed encrypted BlobStore.

1. The owner requests a preview for a catalog Resource. Tauri authenticates the daemon's
   OS peer over local IPC and sends the selected Workspace header, Resource ID, and the
   revision ID displayed in that catalog result.
2. Operator verifies Workspace ownership before resolving the Resource; archived Workspaces
   remain readable under the existing owner-read contract. Storage resolves the exact
   requested `revision_id` within that Workspace/Resource; omitted pin selects current head.
   Provider locators are never returned to the client and an exact old revision is never
   replaced by the current head.
3. Storage checks ContextDocument status and requested byte bound before decrypting the
   Resource-purpose BlobStore object, then verifies byte length and SHA-256 against the
   selected immutable revision. The default local read ceiling is 10 MiB; desktop text
   preview requests a 1 MiB cap.
4. The local API sends bytes as `application/octet-stream` with `no-store` and `nosniff`.
   It does not allow a WebView or browser to execute the Resource.
5. Tauri permits an inline preview only for text-like media types, caps the response at
   1 MiB, and requires valid UTF-8. The UI displays the result as escaped plain text; larger,
   binary, external, unavailable, corrupt, or unsupported content remains metadata-only with
   an actionable explanation.

**Failure/UI/postcondition:** Foreign Workspace IDs and revisions not belonging to the
selected Resource are denied without returning bytes. Missing content returns unavailable;
external content returns `RESOURCE_CONTENT_EXTERNAL`; digest/length mismatch returns an
integrity error and no preview. A read creates no domain event and does not
implicitly attach the Resource to an AgentSession. HTML, SVG and other active content is
never rendered inline. This does not implement general download, binary rendering, content
indexing/search, folder-root search, or semantic RAG.

## F88 — Edit Workspace instructions in the desktop shell

**Actors/preconditions:** Workspace owner, Tauri desktop shell, authenticated local Operator,
and an ACTIVE Workspace. The owner may begin with no instruction revision.

1. On Workspace selection, the desktop loads immutable instruction history. If a current
   revision exists, it loads the referenced Resource through the bounded text preview path,
   pinning the exact `content_ref.revision_id`, and places the content in the editor. A
   generation guard discards results from a previously selected Workspace. A locally
   managed historical Resource revision remains readable after its head advances; external
   or unavailable content shows a typed error and is never replaced by newer bytes.
2. The owner edits the shared guidance and saves. The UI enforces a 64 KiB UTF-8 byte limit
   before any network request and creates a stable request identity for the unchanged draft.
   Editing the draft after a failed request starts a new identity.
3. Tauri imports the text as a same-Workspace `text/plain; charset=utf-8` Resource using a
   separate idempotency key, then asks the Operator to create an instruction revision from
   that exact Resource ID, revision ID, and digest using the Workspace version and current
   instruction revision as concurrency/parent inputs.
4. The UI reports the new revision only after the Operator returns a committed result,
   refreshes the Workspace version and immutable history, and explains that existing Tasks
   are unchanged.

**Failure/UI/postcondition:** If Resource import succeeds but instruction commit fails, the
text Resource remains in the Library and may be unreferenced; the UI reports the failure
and retains the same request identity for retry of the unchanged draft. Reusing that
identity replays the Resource import and instruction mutation. The current local slice has
no atomic cross-command transaction, orphan cleanup, explicit merge editor, or TaskSpec
instruction pinning. A late list/content response
from another Workspace cannot replace the current editor contents. A changed backing
Resource may make older instruction content unavailable when its managed bytes are absent;
it must never silently display the newer head.

**Tests to add in the later verification pass:** every instruction preview sends the pinned
`content_ref.revision_id`; missing revision refs are not previewed; after a Resource head
change, locally managed bytes at the pinned old revision are returned exactly; external or
missing old content fails without current-head fallback; Workspace switching still discards
late content responses.

## F89 — Browse a large local Resource catalog

**Actors/preconditions:** Workspace owner, desktop Library, authenticated local Operator,
and an owned Workspace with zero or more committed Resources.

1. The desktop requests the first metadata page with a bounded `limit`; the Operator checks
   owner and Workspace scope before asking storage for `limit + 1` rows.
2. SQLite orders current Resource summaries by `created_at DESC, resource_id DESC` and
   applies a keyset cursor. The cursor contains the selected Workspace and the last row's
   ordering key; it grants no access and cannot be reused with another Workspace.
3. Operator returns at most `limit` metadata summaries plus `next_cursor` only when another
   row exists. Content bytes, snippets, paths, and provider locators are never returned.
4. The desktop shows the first page and lets the owner load older pages. It appends only
   previously unseen Resource IDs and rejects empty/repeated cursors. Changing Workspace
   resets the catalog and invalidates outstanding page responses.
5. Name/type filtering operates only on rows already loaded. It does not claim to search
   older pages or Resource content; deterministic indexed search remains a separate flow.

**Failure/UI/postcondition:** Invalid limits, malformed cursors, and cursors bound to a
different Workspace are rejected. A storage failure leaves already loaded rows visible
and reports that the next page could not be loaded. Concurrent new Resources inserted
ahead of the cursor do not shift the older-page boundary. Listing creates no domain event,
does not read encrypted Resource bytes, and does not attach Resources to a Task.

## F90 — Resumable desktop Resource upload

**Actors/preconditions:** Workspace owner, desktop Library, Tauri native bridge, authenticated
local Operator, SQLite ResourceUploadStore, encrypted BlobStore. The user selected files
within the 100-file / 100-MiB selection limit; each file is at most 100 MiB.

1. Tauri creates an upload session with a stable request key and local metadata. The daemon
   binds it to the selected owner Workspace, fixes the chunk size at 4 MiB, and sets a
   24-hour expiry. A zero-byte file is immediately `CONTENT_RECEIVED` with no chunk rows.
   It commits `resource.upload.created.v1` with the complete initial session state. A daemon
   sweeper checks up to 100 due sessions every 30 seconds and commits the
   `EXPIRED` transition with a progress-version guard and aggregate-state-backed event.
2. For each non-empty file, the UI reads bounded slices. Before resuming an existing session,
   it hashes every previously received range from the newly selected file and reuses only
   exact matches. A v1 session without a whole-file digest, or with an old size/chunk limit
   outside the current contract, is not resumed; the desktop starts a new verified upload.
   A COMMITTED session is recovered only after the selected bytes match its pinned digest;
   the deterministic commit receipt returns the original Resource, which is inserted into
   the visible catalog by its exact ID. A digest-less legacy COMMITTED session cannot prove
   that it represents the selected bytes, so the desktop stops and asks the owner to review
   the Library. A migrated committed row without a deterministic receipt returns a typed
   conflict instead of creating a duplicate. The browser stores upload ID, request IDs, and
   file metadata only; file bytes stay transient.
3. Each PUT carries a chunk index, inclusive `Content-Range`, raw SHA-256 header, and
   idempotency key. The Operator authenticates the native caller, verifies Workspace
   ownership and exact range/digest. Storage commits an operational reservation before
   encrypting bytes with the dedicated `RESOURCE_UPLOAD_CHUNK` purpose, then commits the
   chunk receipt while consuming the reservation. Each accepted chunk
   advances `progress_version`; lifecycle `version` advances only on state transitions.
   The final contiguous chunk emits OPEN -> CONTENT_RECEIVED in the same transaction as
   its receipt and aggregate-state snapshot. Replaying identical bytes is idempotent;
   changed content for an accepted index conflicts.
4. The desktop commits only after all bytes are accepted. Storage loads chunks in index
   order, decrypts and verifies each digest/size, checks contiguous total coverage and the
   required whole-file digest, then creates the Resource, revision, location, event and
   replay receipt, the session `COMMITTED` state, and CONTENT_RECEIVED -> COMMITTED event
   in the same SQLite transaction.
5. Tauri appends the returned Resource to the visible catalog only after commit succeeds.
   If the process stops mid-upload, the user reselects the file; matching acknowledged
   chunks are skipped and missing chunks resume. Expired sessions start over. ZIP input
   remains an opaque Resource; this flow does not extract archives or add a persistent root.
6. The desktop may offer **Pause upload**. This is a client-side stop, not a server-side
   cancellation: it waits for the current create/chunk/commit request to settle, starts no
   further request or file, and retains the upload ID/request metadata so the same unchanged
   file can be reselected to resume within the session TTL. If the last in-flight request
   commits the Resource, the UI reports that committed result rather than claiming it was
   paused. No new upload state or domain event is created by the pause control.

**Failure/UI/postcondition:** Wrong Workspace, invalid size/range, expired session, or
conflicting replay is rejected without changing the Resource. Commit retries for one upload
ID resolve to the same Resource. Definite stored-content integrity failure durably
transitions CONTENT_RECEIVED -> FAILED with an aggregate-state-backed event before returning
`INTEGRITY_FAILURE`; the user must start a new upload. Transient blob/database failures leave
the session retryable and do not show a committed Resource. A failed metadata transaction
may leave an encrypted object with an expired reservation. The daemon's bounded sweep
claims expired reservations only when no accepted chunk references that Workspace/digest
and no unexpired reservation exists. It writes a durable GC fence before removing the exact
encrypted object, preventing a new chunk reservation during deletion. A crash after claim
leaves a `DELETING` reservation/fence for retry after daemon restart; missing-object removal
is idempotent. The single-instance daemon runs the collector after upload-expiry processing,
at most 100 candidates per 30-second tick. Cleanup failures are non-fatal to Operator
availability and leave the fence intact for retry. The local implementation journals
session creation and lifecycle transitions, but not operational reservations/GC state.

**Tests to add in the later verification pass:** Crash after reservation but before blob
write; crash after encrypted blob write but before receipt; shared-digest reservation and
accepted-reference retention; collector versus concurrent reservation/receipt; collector
restart with a persisted DELETING fence; missing-object retry; BlobStore remove failure
keeps the fence and retries; bounded batches and expiry order; zero-byte commit; exact 4-MiB and final
short chunk boundaries; 100-MiB maximum and +1 byte rejection; missing/out-of-order chunk;
wrong total/range/index; incorrect per-chunk and whole-content hashes; same-index exact
replay and changed-byte conflict; same chunk RequestId with changed payload conflicts;
concurrent identical chunk retries using the same or different RequestIds return the
accepted receipt after the first transaction consumes its reservation; a losing duplicate
reservation is removed in that replay transaction; replay of the final chunk after
CONTENT_RECEIVED or COMMITTED returns the current upload state; retries after lost
create/chunk/commit responses; malformed chunk geometry creates neither a durable
reservation nor a BlobStore object; resume
after daemon restart from reselected identical bytes; reject changed source file on resume;
cross-Workspace session lookup; owner mismatch; archived Workspace; durable TTL expiry;
expiry/commit and expiry/chunk races; sweeper restart and bounded-page behavior; crash before
and after commit transaction; BlobStore or DB failure leaves no partial Resource; encrypted
temporary chunk plaintext absent from disk; zero-byte digest; UI survives Workspace switch
and reports already committed files accurately.

**Additional pause-control verification:** Pause upload waits for the current Operator
request, sends no later chunk/file/commit request, preserves the resumable session, treats
a commit already in flight as completed, and resumes only after the same unchanged file is
reselected.

## F91 — Desktop confirms the authenticated local Operator incarnation

**Actors/preconditions:** desktop Tauri shell, `litecoworkd`, private local connection
descriptor; the daemon may still be DEGRADED and unable to execute Tasks.

1. Tauri derives the local endpoint from its private application-data directory. Shared
   endpoint validation checks the directory owner/mode and a non-symlink Unix socket.
2. Tauri opens a fresh local connection and validates the daemon's kernel-reported peer
   UID before writing any frame. Unsupported platforms fail closed; there is no bearer or
   loopback fallback.
3. Tauri sends a bounded `GET /v1/operator/readiness` frame with a unique request ID. The
   daemon validates the caller UID before reading the frame and dispatches through the
   existing Operator router with a private process-local authenticated-peer marker.
4. The response reports `operator_state=SERVING`, local bootstrap IDs, and the supported
   Operator contract version. The status projection compares those IDs with the daemon's
   lock-backed status before setting `operator_ready=true`; `SERVING` does not mean Task
   execution readiness. Subsequent domain calls use one authenticated request/response
   exchange per connection and retain their original RequestId for safe mutation retry.

**Failure/UI/postcondition:** lock held without a listener, invalid token, stale
descriptor, wrong Runtime/incarnation, unsupported contract version, malformed response,
or timeout leaves `operator_ready=false`; no Workspace/resource command is issued. An
already-running daemon is never spawned a second time. Runtime state may remain DEGRADED
while Operator API is SERVING. These local IDs are not Mesh identity and are not
replicated.

**Tests to add in the later verification pass:** lock acquired before listener startup;
listener bind/runtime initialization failure; correct authenticated handshake; missing and
wrong bearer; browser Origin and unexpected Host; stale descriptor after daemon restart;
descriptor Runtime mismatch; descriptor incarnation mismatch; unsupported contract
version; process alive but API unavailable; API serving while Runtime is DEGRADED; startup
wait deadline; repeated client acquisition after token rotation; hostile HTTP(S)_PROXY
environment variables cannot receive the bearer; exact snake_case wire/schema conformance;
private descriptor mode and symlink rejection; second start request while an authenticated
daemon is already serving.

## F92 — Search managed Resources by metadata or bounded on-demand text scan

**Actors/preconditions:** owner-authenticated desktop session; selected owner-accessible
Workspace (an archived Workspace remains read-only);
Resources already imported into the managed encrypted BlobStore.

1. The Library debounces the query and sends it through Tauri to the authenticated local
   Operator with the selected Workspace header. In `METADATA` mode, kind/freshness filters
   may be used with an empty query to browse the filtered catalog. `ON_DEMAND_CONTENT` and
   `INDEXED_CONTENT` are explicit user-selected modes and require non-empty search terms.
   Metadata and on-demand matching use ASCII-case-insensitive literal substrings; indexed
   mode tokenizes Unicode alphanumeric terms and requires every distinct query term.
2. The Operator validates Workspace ownership, mode, query, kind, freshness, limit, and
   cursor. A cursor is accepted only when its Workspace and exact query/mode/filter values
   match the request.
3. SQLite searches current managed revisions and selects one deterministic encrypted-blob
   location per Resource. Metadata mode filters display-name/media-type and returns its
   existing stable keyset page. On-demand mode scans no more than 20 candidates, reads at
   most 1 MiB per Resource and 8 MiB total, and considers only allowlisted UTF-8 text in
   request memory. Indexed mode token-matches only current revisions with an encrypted
   snapshot and matching HMAC key version. The query HMAC is derived outside the SQL
   transaction; unavailable Workspace keys fail the indexed mode closed. SQLite stores
   no text or raw term. ZIP, PDF/Office, unsupported/binary text, and WorkspaceRoot
   content are excluded. Indexed snippets are decrypted and digest-checked outside SQL,
   then revision and ContextDocument status are rechecked before return. Cursors bind
   Workspace, query, mode, and filters.
4. Tauri reduces the response to Library fields. The UI labels on-demand and indexed
   matches distinctly. A result includes a pinned ResourceRef; selecting Preview remains
   a separate explicit action and rechecks that pinned revision. Search results are not
   attached to a Task or AgentSession.

**Failure/UI/postcondition:** owner mismatch is forbidden; archived Workspace search is
read-only; malformed/oversized filters, unsupported/conflicted freshness, invalid limit,
malformed cursor, cross-Workspace cursor, or cursor/filter mismatch returns a typed client
error. Storage failure does not produce a partial success.
Content modes return only bounded snippets. The deterministic encrypted index does not
search path provenance, ZIP members, rich documents, or WorkspaceRoots, and does not
implement semantic RAG. No search result is implicit Task context. ContextDocument
revocation excludes matching rows immediately; deletion includes term rows and encrypted
snapshot blobs in its sealed purge target and does not claim completion until purge
receipts acknowledge them.

**Tests to add in the later verification pass:** matching and nonmatching Resource names;
media-type match; literal `%` and `_`; ASCII-case-insensitive match; empty query returns the
filtered catalog; direct API leading/trailing whitespace is preserved as literal query text;
exact kind and supported freshness filtering; one-owner/foreign-Workspace request;
all cursor mismatch cases; equal timestamps and inserted rows retain keyset semantics;
page boundary and limit bounds; absent managed location is excluded; observed digest or
revision mismatch reports stale; unavailable/revoked location does not claim current;
multiple managed locations still yield one deterministically selected location/result;
10k-Resource search latency is recorded as a qualification sample, not an unverified SLO;
Tauri maps the wire response; UI debounce cancels stale queries, Workspace changes clear
results/cursors, new Resources refresh active search, repeated next cursor is rejected,
selected result previews only after explicit click, a changed revision returns
`RESOURCE_CONFLICT` without previewing newer bytes, and no search result enters a Task
automatically. On-demand cases include UTF-8 allowlist and binary/control rejection,
case-insensitive literal matches, snippet bounds, 20-candidate/1-MiB-per-file/8-MiB
aggregate budgets, exact revision/digest recheck, and cursor continuation. Indexed cases
include upload-time index creation, revision replacement without stale term reuse, exact
Workspace/key-version isolation, HMAC key rotation with retained-key search and explicit
rebuild, missing-key fail-closed behavior, no plaintext in SQLite, encrypted snapshot
digest/authentication failure, current-revision/ContextDocument recheck, purge, Unicode
AND matching, query-term limits, ZIP/PDF/Office exclusion, and keyset pagination. Treat
elapsed search time as a qualification measurement, not an SLO.

## F93 — Enroll or revoke a local Runtime for one Workspace

**Actors/preconditions:** authenticated local Operator over qualified OS-peer IPC,
Workspace owner, RuntimeWorkspaceBindingService, StateStore, Runtime Mesh when pairing.

1. Workspace creation or an explicit owner action requests local enrollment. The service
   resolves the installation's stable RuntimeId and creates one ACTIVE
   `LOCAL_ENROLLMENT` binding for the selected Workspace. An exact idempotent retry returns
   the same binding; it does not create another Runtime identity.
2. The binding authorizes only Workspace-scoped local operations allowed by its roles and
   the separate Workspace/Trust checks. It creates no Mesh presence, replication grant,
   capability grant, or folder authority.
3. Optional cloud pairing is a separate owner action. The Hub consumes a single-use
   Workspace-scoped token, authenticates the device key, and records a `MESH_PAIRING`
   binding before allowing presence or replication.
4. Revoking one binding blocks new admissions, Workspace writes, root access, and
   replication on that binding. If the Runtime is currently selected as this Workspace's
   Mesh hub, the service first clears `hub_runtime_id` under the expected Workspace
   version. It also drains any ChannelHost assignment and releases its host lease. When a
   target is eligible, the assignment and target lease commit atomically; when none is
   eligible, RuntimeMesh atomically clears the released assignment and leaves its
   ChannelBinding visibly DEGRADED/unassigned. It reassigns or disables enabled TriggerHost
   cursors before revocation. It also settles all nonterminal AutomationOccurrences pinned
   to that TriggerHost; disabling an
   Automation prevents new occurrences but does not settle existing ones. Storage rejects
   revocation while these references remain live. Active Task work follows lease and Effect
   reconciliation; it is never silently moved to another Runtime. Installation-wide
   revocation is a separate operation affecting all bindings.

**Failure/UI/postcondition:** missing OS-peer authentication, mismatched Workspace,
inactive/revoked binding, owner mismatch, stale version, replayed token, a local enrollment
selected as a Mesh hub, undrained host/trigger references, or unavailable Hub fails closed.
The UI distinguishes “this computer can access this Workspace” from “this Runtime is
paired for cloud sync.” Runtime API readiness alone never displays a Workspace as enrolled.

Revocation cannot skip the ChannelHost ingress barrier. Receipt insertion and the
ACTIVE→DRAINING transition are serialized; once DRAINING commits, the source neither inserts
nor acknowledges new receipts. A live source must settle all source-epoch PROCESSING claims,
make every pre-drain RECEIVED row Hub-durable and part of the successor replay frontier, and
reconcile outbound Effects before a quiescent proof. The successor may claim a durable
pre-drain RECEIVED row using its new lease. If the source cannot prove this, revocation waits
through `safe_reassign_after` or leaves the binding unrevoked. Migration rejects ACTIVE assignments without a matching lease and DRAINING
assignments without a matching live lease, because v1 has no release tombstone from which
the safe release can be reconstructed.

**Tests to add in the later verification pass:** one installation with two Workspaces has
distinct bindings; revoking one preserves the other; local enrollment creates no Mesh
presence or replication; exact retry is idempotent; cross-Workspace root/admission requests
fail; revoked binding stops new work but does not rewrite a live Attempt; Mesh pairing
token replay/expiry/owner mismatch fails; global Runtime revoke invalidates every binding;
legacy v1/v2 Runtime rows backfill correctly through v3 and v4 removes the legacy Workspace
   column without losing Runtime/Workspace authorization or scoped rows; migration preflight
   rejects legacy ChannelHost rows whose `GAP_ACCEPTED` owner decision or later-epoch
   continuity proof cannot be reconstructed; preflight rejects
orphaned/wrong-role Environment, channel, trigger, or Workspace-hub references, including
a reusable Environment without an ACTIVE EXECUTOR binding and a revoked ChannelHost with
an unexpired lease; nonterminal TriggerHost occurrences cannot advance after revocation;
ChannelHost assignment reactivation under a revoked binding, illegal occurrence status
transitions, direct pre-linked/non-PENDING occurrence insertion, duplicate Tasks for one
occurrence, Task-link reassignment/mismatch, and terminal occurrence rewrites are rejected;
Automation claim epochs advance exactly once per new claim, expired claims alone requeue,
and materialization rejects expired claims or missing Task links; ChannelHost assignments
cannot skip draining, and lease identity/control versions cannot be rewritten or renewed
after expiry; in-flight receipt settlement is permitted only for the unchanged source claim
while draining; new claims/reclaims stop in DRAINING; live release needs an exact drain proof;
expired release waits through the pinned safety margin; assignment continuity provenance is
immutable within an epoch; malformed claim/lease timestamps and impossible legacy occurrence
status/epoch/expiry/Task tuples fail v4 migration preflight;
all migrated foreign keys pass `foreign_key_check`; injected v4 interruption leaves either
the complete v3 schema with foreign keys restored or the complete v4 schema; two Workspaces
can attach one Runtime with independently scoped EXECUTOR/CHANNEL_HOST/TRIGGER_HOST/
WORKSPACE_HUB roles; binding revocation rejects an uncleared Mesh hub or undrained
ChannelHost/TriggerHost, including nonterminal TriggerHost occurrences, and succeeds after
those references are settled.
ChannelHost ingress insertion racing DRAINING is serialized (insert wins and is reconciled,
or drain wins and source does not insert/ack); proof cannot race a RECEIVED/PROCESSING
source-epoch receipt; late provider delivery is retried/replayed or requires an audited gap
decision; release plus target lease assignment or explicit unassignment is atomic; every
committed ACTIVE assignment has exactly one matching lease; no-target Runtime revocation
leaves a DEGRADED/unassigned ChannelBinding with no authority on the old Runtime; migration
rejects ACTIVE/no-lease and DRAINING/no-lease ChannelHost rows; host lease IDs and derived
fencing credentials are unique across epochs and reuse fails closed.

## F94 — Authenticate desktop Operator over local OS IPC

**Actors/preconditions:** Tauri native process, `litecoworkd`, private application-data
directory, matching local Runtime OS-principal binding, shared `operator-ipc` protocol.

1. Tauri derives the endpoint from its trusted app-data directory; the WebView cannot
   provide or override it. Tauri opens a Unix-domain socket or local named pipe.
2. The OS enforces endpoint access. The daemon checks Unix peer UID against its persisted
   Runtime OS-user binding; separate Unix sessions with the same UID are within that same
   user trust boundary. Windows pipe creation uses a protected DACL restricted to the
   current logon SID and SYSTEM and rejects remote clients. Missing/mismatched identity
   closes the connection before the request header/body is read.
3. The desktop sends a bounded, versioned frame with request ID, allowlisted method/path,
   allowlisted logical headers, and declared raw body length. The daemon rejects invalid
   lengths, unsupported versions, absolute/traversal paths, forbidden headers, and
   incomplete frames before route dispatch. Each connection carries one request; extra
   bytes cannot create a second dispatch and the server closes after responding.
4. The adapter attaches an internal authenticated-local-peer marker and dispatches through
   the same Operator handler path as the logical API. Workspace owner, Trust, expected
   version, and idempotency checks still run there.
5. The daemon returns a bounded response frame carrying the same request ID. Tauri checks
   the authenticated readiness response against native Runtime/incarnation status before
   using normal routes.
6. After daemon restart, Tauri reconnects and performs readiness again. A mutation whose
   response was lost is reconciled using its original idempotency key; it is never replayed
   as a new command or silently sent to another Runtime.

**Failure/UI/postcondition:** wrong OS user, wrong Windows logon session, missing peer credentials, failed DACL,
unsafe endpoint, malformed/oversized frame, stale incarnation, or timeout fails closed.
There is no loopback/bearer fallback. Incomplete mutation delivery is shown as an
ambiguous result and reconciled by request identity. IPC peer authentication grants no
Workspace access, folder access, Capability grant, or Effect authority.

**Tests to add in the later verification pass:** cross-user denial on Unix and Windows;
second Windows logon session denial; document that a same-UID Unix session shares the
authorized user boundary; socket/pipe endpoint precreation and replacement;
symlink/wrong-owner stale socket; Unix missing/mismatched peer credential; Windows DACL
failure; malformed protocol version; forbidden headers; path traversal/absolute URL;
short/oversized frame; extra bytes cannot create a second request; slow header/body timeout; concurrent admission limit;
daemon crash before and after durable mutation commit; exact idempotent reconciliation;
restart readiness/incarnation mismatch; no WebView endpoint/identity exposure; no HTTP
fallback on any IPC failure. Run on each supported OS and record version/build evidence.

## F95 — Explicit local coding-agent profile probe and binding setup

**Actors/preconditions:** authenticated desktop owner, active Workspace, serving local
Operator, current Runtime incarnation, local Runtime enrollment for that Workspace,
AgentCatalogStore, bounded native adapter.

1. The owner opens Settings and selects a Workspace. Installation inventory may show
   that the Codex CLI or OpenCode CLI is installed, but this does not create an
   AgentProfile or claim authentication/session readiness.
2. If this exact Runtime incarnation is not enrolled for the selected Workspace, the
   UI shows `Not enrolled` and offers an explicit owner action. The Operator checks the
   owner and Workspace version; storage accepts only the exact current PERSONAL_DEVICE
   Runtime in `ONLINE/READY` or `DEGRADED/DEGRADED` serving state and records an ACTIVE
   local binding with `EXECUTOR` and `OPERATOR_ENDPOINT` roles. This is control-plane
   authorization only; it does not grant filesystem roots, Mesh presence, replication,
   Task readiness, or agent execution.
3. The owner explicitly requests `POST /v1/agent-profiles/probe` with `provider_key` set
   to `CODEX` or `OPENCODE`. The Operator requires the active local enrollment and
   resolves the Runtime-local executable. Codex uses a bounded App Server process and
   performs initialize, account/read without refresh, and one bounded model/list page.
   OpenCode launches its owned Server process with an allowlisted environment and reads
   only `GET /provider` and `GET /config/providers`, each under the existing bounded
   response limit. Both probes stop their owned direct process. Neither creates a
   Conversation turn, Task, AgentSession, Environment, model inference request, grant,
   or Effect.
4. The Operator persists the stable AgentProfile/AgentEndpoint identity, private
   Runtime-incarnation-bound executable locator, and sanitized expiring offer. The UI
   distinguishes installation, reported provider readiness, option catalog, endpoint
   compatibility/readiness, and the still-unverified session/inference lifecycle. For
   Codex, expandable probe details expose only allowlisted protocol/account/model-list
   observations. For OpenCode, they expose only bounded provider/model IDs and display
   names, plus the provider IDs OpenCode reported as connected. This is not an
   authentication or entitlement claim. OpenCode model entries are display-only;
   session model selection remains `NOT_QUALIFIED`, its offer is always incompatible,
   and no AgentBinding can be created or enabled from it. Neither probe exposes raw
   native payloads, configuration, credentials, or endpoint locators. Both report that
   inference was not tested and writer quiescence was not proven.
   Every explicit probe is a fresh observation, not an idempotent mutation; clients do
   not automatically retry it.
5. The owner may create a Workspace AgentBinding only from a fresh compatible offer.
   Creation is version 1 and disabled. The owner may then separately enable it under an
   expected version; enablement requires the offer to remain fresh and compatible.
   Neither creation nor enablement selects a Workspace default or starts an agent.
6. Future Task admission remains unavailable until Task planning, binding selection,
   AgentSession lifecycle, Environment/lease admission, and Effect reconciliation are
   integrated and qualified. Profile-probe `writer_quiescence_proven=false` is not
   sufficient evidence for safe switching.

**Failure/UI/postcondition:** no selected Workspace, non-owner, missing/stale enrollment,
inactive Workspace, Runtime incarnation change, executable absence, timeout, protocol
rejection, unsafe server request, process-stop uncertainty, incompatible/expired offer,
stale binding version, or unavailable storage fails closed. The UI keeps install inventory
separate from profile offers and bindings, displays `Could not check` on enrollment-read
failure, and never labels binding enablement as an executable Task capability.

**Tests for the later verification pass:** non-owner and cross-Workspace probe denied;
unenrolled or stale-incarnation Runtime cannot probe; `ONLINE/READY` and serving
`DEGRADED/DEGRADED` enroll, while DRAINING/OFFLINE/REVOKED do not; one exact ACTIVE
Workspace binding with required roles is returned; a Workspace can enroll multiple
local Runtime installations without sharing authority; profile probe executes only the
allowlisted bounded methods, never refreshes auth or performs inference, rejects server
requests, limits model options/catalog counts/response bytes, reaps its direct child, and
preserves unproven writer quiescence; OpenCode output contains only sanitized provider/
model IDs and display names plus reported connected IDs; catalog entries and reported
connection state never enable OpenCode binding admission; private executable locator/
account data/native protocol payloads never appear
in Operator responses, domain events, backups, or logs; offers expire and stale offers
cannot create/enable a binding; binding creation is disabled at version 1; enable is
version-fenced and idempotent; lost mutation responses reconcile using the original
RequestId; the UI handles loading, unknown/error enrollment, workspace changes, stale
offers, and disabled actions without claiming Task execution. Run native protocol and
process-containment qualification separately before calling switching production-safe.

## F96 — Select the Workspace default lead agent

**Actors/preconditions:** authenticated Workspace owner, active Workspace, enabled
same-Workspace AgentBinding with `lead_eligible=true` (or an explicit choice to clear
the default), Operator API, WorkspaceService, and the SQLite Workspace transaction.

1. Settings lists only bindings returned for the selected Workspace and offers enabled,
   lead-eligible bindings as selectable defaults. Enabling a binding alone never selects
   it.
2. The owner chooses one binding or explicitly chooses “No default agent.” The desktop
   submits `PATCH /v1/workspaces/{workspace_id}/default-agent-binding` with the selected
   Workspace header, current Workspace `If-Match` version, and a stable RequestId.
3. WorkspaceService commits the changed Workspace projection, version,
   `workspace.default_agent_binding.changed.v1`, and idempotency receipt together. The
   storage transaction rejects a binding from another Workspace or one that is disabled
   or not lead-eligible. A stale version conflicts; retrying the same RequestId replays
   the original result.
4. Settings updates the displayed default only from the committed Workspace response.
   Clearing the default is explicit. The change affects future admissions only; it does
   not start an agent, change an existing Task, or execute work.
5. Until TaskService, lead readiness admission, PlanningCoordinator, and durable
   AgentSession lifecycle exist, the composer remains unavailable and no Task is implied
   by selecting a default.

**Failure/UI/postcondition:** cross-Workspace selection, disabled/non-lead binding,
archived Workspace, malformed/null-omitted request, stale version, and reused RequestId
with another payload fail without mutating the Workspace. The selector reports the saved
default, supports an explicit unconfigured state, and does not claim that setup has
started work.

**Tests for the later verification pass:** set, clear, same-target update, exact idempotent
replay after a lost response, same RequestId/different payload conflict, stale Workspace
version, disabled binding, non-lead binding, cross-Workspace binding, archived Workspace,
and missing-versus-null request field; verify one event/version per committed update and
that existing Task/session bindings remain unchanged.

## F97 — Persist a standalone Task envelope (implementation increment)

**Actors/preconditions:** authenticated Workspace owner, active Workspace, selected enabled
lead-eligible AgentBinding, Operator API, `domain-task::TaskService`, and TaskStore.

1. The client submits a standalone `POST /v1/tasks` request with the selected Workspace
   header and RequestId. This increment rejects Conversation-message and Coworker origins;
   those require their own atomic source/admission services.
2. The Operator validates owner, Workspace state, and explicit-or-Workspace-default lead
   selection. It never substitutes another binding if the selected one is invalid.
3. TaskService builds Task + TaskSpecRevision(1) in `READY` state and calls TaskStore.
   One SQLite transaction commits Task, immutable initial spec, `task.created.v1`, aggregate
   snapshot, and idempotency receipt. Workspace instructions and pinned Resource revisions
   are rechecked by storage.
4. The endpoint returns the committed TaskView. This increment does not create a planning
   AgentSession, Plan, Step, Attempt, lease, Environment, or Invocation. It is durable Task
   persistence only; the desktop composer stays disabled until the planning coordinator can
   actually start and track the lead session.

**Failure/UI/postcondition:** missing/invalid lead, archived Workspace, malformed spec,
unsupported origin, stale instructions, out-of-Workspace/unpinned Resource, or RequestId
reuse with a different payload creates no Task. Exact retry returns the original committed
TaskView. No UI state says “Planning” or “Working” for this persistence-only increment.

**Tests for the later verification pass:** standalone create, owner and Workspace mismatch,
no configured lead, disabled/non-lead lead, explicit lead precedence, stale instruction
revision, invalid/missing Resource revision, conversation/Coworker-origin rejection,
unknown field rejection, exact RequestId replay after response loss, changed-payload key
reuse, transaction crash before/after commit, and assertion that no Plan/Step/Attempt/
AgentSession/lease/Environment rows are created.

## F98 — Claim durable Task planning session startup

**Actors/preconditions:** authenticated Workspace owner; READY or RUNNING Task; current
TaskSpec revision and lead; enabled lead-eligible AgentBinding; selected endpoint bound to
the current READY Runtime incarnation; AgentSessionStore.

1. PlanningCoordinator builds the transient assignment from a current Task view and
   selects a compatible endpoint/Runtime. No native session has started yet.
2. AgentSessionStore transactionally rechecks Task version/status/spec/lead, Workspace
   owner/status, AgentBinding eligibility, endpoint/profile match, endpoint binding expiry,
   and current Runtime incarnation readiness.
3. The transaction inserts exactly one `TASK_PLANNING` AgentSession in `STARTING`, stores
   the immutable aggregate-state reference, appends `agent.session.starting.v1`, and stores
   the RequestId receipt. The active-planner unique index rejects a competing assignment.
4. The caller may now start the adapter outside SQLite. `STARTING` grants no planning tools
   or Invocation authority. Adapter failure/recovery may atomically settle this row as LOST
   with `agent.session.lost.v1`; Task status is unchanged and the planner slot is released.
5. After adapter readiness, `TaskService::activate_planning_session` asks storage to
   revalidate the current Task/spec/lead, host, endpoint binding, Runtime incarnation,
   Workspace and binding eligibility. One transaction inserts the Runtime-local host
   binding, sets the session `ACTIVE`, transitions a first-planning Task to `RUNNING`, and
   appends the corresponding event snapshots. A Task already `RUNNING` receives no second
   status event. A stale/degraded host or Runtime cannot activate.
6. After daemon restart, stranded prior-incarnation `STARTING` sessions can be enumerated
   and explicitly settled as `LOST`; automatic coordinator reconciliation and adapter
   startup/retry orchestration are not implemented by this storage slice.

**Failure/UI/postcondition:** stale Task/spec/lead, unavailable owner/Workspace, disabled
binding, endpoint mismatch/expiry, Runtime/incarnation mismatch, idempotency-key reuse with
different input, or active planner conflict commits no new session. If activation loses a
readiness/version race, its transaction commits no ACTIVE session, host binding, or Task
transition; the existing STARTING reservation remains claimed until the coordinator proves
startup containment and settles it LOST. It must not retry activation against a different
host or silently reuse a native handle. A stranded STARTING row is discoverable by the
bounded recovery query; no UI may call it active work.

**Tests for the later verification pass:** exact RequestId replay, changed-payload replay
conflict, stale Task version/spec/lead, non-runnable Task state, wrong Workspace owner,
disabled/non-lead binding, wrong endpoint profile, endpoint bound to an old Runtime
incarnation, expired binding, non-READY Runtime, one-planner race, transaction crash
before/after commit, recovery query across old incarnations, LOST settlement releasing the
planner slot without changing Task status, activation after Runtime restart/degradation,
host stop/failure, binding disablement, endpoint expiry, wrong host profile/endpoint/
incarnation, invalid event schema version, rollback if a snapshot or event append fails,
and no native handle in the aggregate/event/request receipt.

## F99 — Accept an initial plan into durable Task state (storage slice)

**Actors/preconditions:** active Task-planning AgentSession for the current lead, RUNNING
Task, exact current TaskSpec revision, authenticated Workspace owner context, TaskService,
TaskStore, and SQLite v5.

1. The internal planner integration submits bounded proposed Steps using logical keys and
   dependency keys. The domain service rejects an empty or over-limit plan, malformed
   keys, duplicate keys/IDs, missing/self/duplicate dependencies, cycles, malformed
   acceptance criteria, and mismatched Step/event counts before constructing records.
2. TaskService materializes opaque Step IDs, resolves logical-key dependencies, and sets
   dependency-free Steps `READY`; dependent Steps begin `PENDING`. Each Step stays bound to
   PlanRevision 1. This slice accepts initial plans only; it does not accept execution
   replans because Attempt/lease producer authority is not implemented.
3. SQLite begins an immediate transaction. It replays an identical prior RequestId receipt
   or rejects key reuse with a different digest, checks Workspace owner/active status,
   compares Task version/spec/status/head, and revalidates that the producer session is
   ACTIVE, current-spec-pinned, `TASK_PLANNING`, no-Attempt, and the enabled current lead.
   It also requires the producer to be attached to a host on the Runtime's current
   `ONLINE`/`READY` incarnation, checks the live endpoint binding and active Workspace
   EXECUTOR binding, and requires every event's Runtime ID to match that producer Runtime.
   Endpoint expiry is evaluated against the daemon's UTC admission clock, not the event's
   recorded timestamp. The Workspace association comes from `runtime_workspace_bindings`;
   the installation-scoped Runtime row itself has no Workspace column.
4. The transaction appends immutable PlanRevision 1, inserts all Steps, advances
   `Task.current_plan_revision` and Task version, appends one `task.plan.revised.v1` plus
   one `step.created.v1` per Step with complete post-state blobs, stores the original
   PlanAcceptance receipt whose digest includes the normalized validated plan, and commits.
   Readers can then load PlanRevision history and Steps after daemon restart. Step rows
   cannot be deleted; lifecycle fields may advance while their identity remains fixed.
5. A retry with the same principal/RequestId and normalized body returns the stored result
   without another revision/event. A stale task/spec, closed or replaced planner, invalid
   owner/status, or SQLite failure commits no partial plan.

**Deliberate boundary:** the Operator API route is not wired to this command yet because
the current local Operator authentication proves Workspace-owner identity but does not
carry an AgentSession-scoped producer assertion. Do not accept a body-supplied session ID
as producer authority. No native agent currently invokes this service; it is a durable
domain/storage seam, not end-to-end agent planning.

**Tests for the later verification pass:** valid single- and multi-step plans; empty and
over-limit input; unique logical keys and Step IDs; missing/self/duplicate dependencies;
cycle rejection; malformed capability/criterion values; current/stale TaskSpec; wrong
Workspace; owner mismatch; non-RUNNING Task; wrong/closed/non-lead/spec-stale planner;
Attempt-bearing session rejection; concurrent submissions; Task pause/cancel/spec change
races; duplicate RequestId replay and changed-digest conflict; crash before commit and
after commit/before response; exact Step-ID replay; all event state blobs reconstruct the
Task pointer/Plan/Steps; no Step/Attempt admission before acceptance; re-read after daemon
restart; PlanRevision update/delete rejection; Step deletion rejection; and immutable Step
identity enforcement. Also cover an old Runtime incarnation, missing/mismatched host
binding, expired endpoint at admission time despite a fresh-looking event timestamp, revoked
Workspace EXECUTOR binding, non-string/extra `step_ids`, extra Step event payload keys,
duplicate event IDs, and same RequestId/plan submitted by a different AgentSession.

## F100 — Save a standalone Task from the desktop composer

**Actors/preconditions:** desktop owner, authenticated local Operator IPC, active selected
Workspace, TaskService and TaskStore. An explicit Task lead, selected Coworker default
lead, or Workspace default lead must resolve to an enabled, lead-eligible binding. The
Runtime is serving the current authenticated Operator endpoint.

1. The Home composer retains the unsent objective until the owner submits **Save Task**.
   It requires a non-empty objective within the UTF-8 limit and an active selected
   Workspace. If neither the selected Coworker nor Workspace supplies a lead, it routes the
   owner to Settings; it does not choose an arbitrary discovered agent. The owner may
   explicitly select existing Library Resources as Task inputs; each selection pins its
   exact `ResourceRevisionId`.
2. Tauri pins the exact Workspace, objective, selected Coworker ID and observed aggregate
   version (when available), resolved lead binding, ordered pinned input references and
   RequestId for this save. It does not submit a Coworker revision. It submits
   `POST /v1/tasks` over authenticated local IPC. The WebView never sees the IPC endpoint
   or peer identity.
3. The Operator resolves the selected Coworker's current immutable revision. Lead
   precedence is explicit Task lead, Coworker revision default, then Workspace default;
   the Coworker failover default is also copied into the initial TaskSpec when the request
   does not supply an explicit policy. Storage atomically rechecks Workspace ownership,
   Coworker status/current revision/optional expected version, resolved failover policy,
   that the exact lead binding is same-Workspace, enabled and lead-eligible, and every
   pinned Resource revision is same-Workspace and unique. ContextDocuments must be `ACTIVE`
   at admission; Resource resolution checks status again when bytes are requested. TaskService
   and SQLite persist the standalone Task in `READY`, initial TaskSpecRevision,
   `task.created.v1`, snapshots and idempotency receipt.
4. On a committed response, Tauri verifies the Task belongs to the selected Workspace and
   that its initial current spec revision is 1, and that returned Coworker origin matches
   the selected Coworker ID with a paired revision. It verifies the embedded TaskSpec's
   `task_id` matches the Task and its `revision` matches
   `task.current_spec_revision`; Workspace ownership is inherited through that Task
   relationship. It also checks the requested objective and ordered input references.
   Only then does the UI clear the matching draft and input selection and open the Task
   detail. The detail shows the pinned inputs and renders the actual `READY` status with a
   note that no accepted plan is currently saved. It does not infer that the Task has never
   been planned or executed. Pinning a Resource does not read its contents or grant an agent
   access. When Coworker origin exists, Task detail resolves and displays the exact pinned
   CoworkerRevision name; a missing historical revision is shown as unavailable rather
   than replaced by the current name. It does not display Planning/Working, start a provider, or create an
   AgentSession, Plan, Step, Attempt, lease, Environment, CapabilityInvocation, Effect, or
   Evidence.
5. If the local response is ambiguous while the window remains open, retrying the same
   Workspace/objective/ordered inputs reuses the original request envelope and RequestId;
   it retains the originally pinned Coworker/version/lead even if defaults change while
   the response is uncertain. The UI labels this as **Retry original save** and directs
   the owner to Work before discarding an unresolved request or creating a separate Task.
   Changing objective/inputs does not silently create another Task while that request is
   unresolved. The current client does not persist the draft/idempotency envelope across application
   restart. Workspace selection and primary navigation are disabled while the save is in
   flight; after an ambiguous result, the owner can retry the same request in the current
   window or check Work before submitting again after restart.

**Failure/UI/postcondition:** missing or disabled lead, archived Workspace, invalid
objective, transport failure, Task/TaskSpec identity or revision mismatch, returned
objective/input mismatch, or idempotency conflict never shows the Task as started.
Failed/ambiguous saves retain the draft and show a recoverable message. The storage
receipt prevents duplicate creation only when the same RequestId is reused; do not promise
cross-restart exactly-once behavior from this UI slice.

**Tests for the later verification pass:** active/inactive Workspace, Runtime unavailable,
missing Workspace/Coworker/explicit lead, Coworker-to-Workspace lead fallback, explicit
lead precedence, disabled/non-lead/cross-Workspace binding, paused/archived Coworker,
optional/stale Coworker version, Coworker revision change between read and commit, exact
Coworker origin ID/revision in Task and event, pinned Coworker failover default, UTF-8
boundary, empty draft, same-key exact replay, same-key changed-objective/input-reference conflict, response loss
after commit, rapid double-submit, returned Task/TaskSpec identity mismatch, TaskSpec
revision differing from the Task pointer, returned Workspace/objective/input mismatch, stale or
foreign Resource revision, revoked/deletion-pending/deleted ContextDocument input rejection,
no draft or input loss on failure, Workspace-specific input
selection surviving a Workspace switch, navigation/Workspace switch controls disabled
during submit, successful draft/input clear only after commit, and assertion that the save
creates no planner session, Plan,
Step, Attempt, lease, Environment or Invocation.

## F101 — Pin Library Resources to a saved Task

**Actors/preconditions:** desktop owner, authenticated local Operator IPC, active Workspace,
existing Resource revisions visible in the Library, and an enabled default lead binding
when the Task is eventually saved.

1. From Home, the owner chooses **Add inputs** and enters the selected Workspace Library.
2. A catalog or search result offers **Add to Task**. Selecting it pins the exact
   `workspace_id`, `resource_id`, and `revision_id`; it does not fetch the Resource body.
   Selecting the same revision again removes it. Choosing a different revision for the
   same Resource replaces the pending selection explicitly.
3. **Use in Task** returns to Home. The composer shows selected file names and that each
   exact revision is pinned. The owner can remove individual inputs or clear all. Selections
   are kept separately per Workspace so a Workspace switch cannot attach another
   Workspace's Resources.
4. **Save Task** includes the pinned `input_refs` in the same idempotent Task creation
   request. The daemon validates that each reference belongs to the selected Workspace
   and that the Resource revision exists before the Task and initial TaskSpecRevision are
   committed. The Task detail shows the durable references and names when the local
   presentation cache has them; otherwise it shows the Resource ID and exact revision.
5. The attached references do not themselves grant file access or trigger execution.
   A later ContextPlanner/AgentSession must independently resolve availability, policy,
   sensitivity and authority before making content available to an agent.

**Failure/UI/postcondition:** a stale/missing or cross-Workspace reference rejects the
entire Task save. The composer retains its objective and selected input references. An
ambiguous response retry uses the same RequestId only while the Workspace, objective,
lead binding and exact ordered reference list are unchanged. A changed selection gets a
new RequestId. No Resource content, extracted text, search snippet, or local path is copied
into the Task envelope.

**Tests for the later verification pass:** attach/remove/replace revision, exact TaskSpec
reference roundtrip, same-Workspace and foreign-Workspace refs, missing revision, Resource
revision change between search and save, unchanged retry and changed-ref idempotency,
Workspace-specific pending selections, selected names surviving save into Task detail,
fallback ID display after restart, no Resource bytes in Task/event payloads, and no agent,
Environment, Invocation, Effect or Evidence created by pinning.

## F102 — Load persisted Task plan and Step state in desktop details

**Actors/preconditions:** desktop owner, authenticated local Operator IPC, a Task in the
selected Workspace, and the existing read-only plan-history and Step routes.

1. Tauri loads the TaskView and checks that the Task belongs to the selected Workspace
   and that its embedded TaskSpec matches the Task ID and current-spec revision pointer.
2. If `current_plan_revision` is null, Task details say no accepted plan is currently saved
   and make no plan-history or Step request. The status remains **Ready**; the UI does not
   infer that the Task has never been planned or executed.
3. If a current plan exists, Tauri loads plan history and Steps through the authenticated
   Workspace-scoped routes. It selects only the PlanRevision named by the Task pointer,
   then checks Task identity, plan/spec revision bounds, planned logical keys, and
   materialized Step identities/count before constructing the desktop view.
4. Task details render the ordered persisted Steps and their actual stored statuses with
   plain-language labels. If the PlanRevision pins an older TaskSpecRevision, the view
   shows a stale-plan notice. It never implies that a Step has an Attempt, Evidence,
   verification result, or active worker unless those separately loaded records exist.
5. A malformed, incomplete, foreign-Task, or mismatched plan response fails the detail
   load rather than presenting partial rows as current Task truth.

**Tests for the later verification pass:** Task without plan performs no plan requests;
current PlanRevision and Steps display in logical order; multiple historical revisions
select only the Task pointer; stale plan displays the pinned/current spec revisions;
foreign Task IDs, wrong plan revision, missing/duplicate/mismatched logical keys, incomplete
Step rows, malformed payloads, offline Runtime, and response loss show recoverable errors;
Step status text remains understandable without color; no Attempt/Evidence is fabricated.

## F103 — Revise a saved, unplanned Task objective

**Actors/preconditions:** Workspace owner in the desktop Operator, authenticated local
Operator IPC, active Workspace, and a Task whose current state is `READY`, whose
`current_plan_revision` is null, and which has no live `TASK_PLANNING` AgentSession.

1. Task details show **Edit objective** only while the loaded Task satisfies the visible
   `READY`/no-Plan conditions. The owner edits a local draft; no domain state changes while
   typing or cancelling.
2. Save submits `parent_revisions=[current_spec_revision]`, the objective, `If-Match` with
   the loaded Task aggregate version, and an idempotency key. The desktop retains the same
   key only for a retry of the exact same Task/version/parent/objective tuple; changed text
   receives a new key.
3. Operator authenticates the Workspace owner and checks the selected Workspace. The
   TaskService checks the current Task and parent; SQLite repeats owner, active Workspace,
   version, READY/no-Plan, no-live-planner, exact Resource revision and lead eligibility
   checks inside an immediate transaction. Every pinned input remains unique and
   same-Workspace; ContextDocuments must be `ACTIVE` when the revision is admitted. The
   Resolver checks status again when bytes are later read.
4. On success, SQLite appends the next immutable TaskSpecRevision, advances the Task
   spec pointer/version, stores the complete Task aggregate snapshot, emits
   `task.spec.revised.v1`, and commits the idempotency receipt atomically. Unspecified
   fields inherit the old revision. No AgentSession, Plan, Step, Attempt, ExecutionLease,
   Environment, Invocation, Effect, or Evidence is created.
5. The UI validates the revision receipt, reloads the Task through its ordinary detail
   route, and displays the committed objective and spec revision. If the Task was planned
   concurrently after the edit committed, the reload displays that current plan and state.

**Failure/UI/postcondition:** stale Task version/parent, a live planner, non-READY status,
or an accepted Plan returns a conflict; the draft remains visible and offers **Reload latest
Task**. Invalid fields return `INVALID_ARGUMENT`. A lost response can be retried with the
same key and receives the original committed revision. Reusing the key after changing the
objective conflicts. Saving unchanged objective text is disabled and creates no revision.
Edits after planning starts use the ordinary steering/replanning
lifecycle; this editor never bypasses it.

**Tests for the later verification pass:** objective change appends exactly one revision
and event; unchanged fields remain byte/structure-equivalent; same-key retry after a lost
response returns the same revision/event without another aggregate write; changed payload
with same key conflicts; stale Task version and wrong parent conflict; READY with live
planner rejects; READY with Plan, RUNNING, terminal, archived Workspace and foreign owner
reject; missing/foreign Resource revision rejects when inputs are submitted; revoked,
deletion-pending, and deleted ContextDocuments reject in both Task creation and revision;
two concurrent
editors yield one head advance; unchanged objective creates no revision; composer/list/detail
show the committed revision; no agent process, planning session, Plan, Step, Attempt, lease,
Environment or Invocation starts.

## F104 — Pause and resume a persistent folder scope

**Actors/preconditions:** Workspace owner, desktop Library, authenticated local Operator,
`WorkspaceRootService`, SQLite, and the current local Runtime incarnation. Folder
watching/indexing are not implemented by this source slice.

1. The Library renders the persisted WorkspaceRoot status and aggregate version. It offers
   **Pause** only for `ACTIVE` and **Resume** only for `PAUSED`; `UNAVAILABLE` is shown as
   unavailable and cannot be resumed from that button. Revocation remains available for
   every non-revoked root.
2. The owner selects **Pause** or **Resume**. Tauri sends a bodyless request with the
   selected `X-Workspace-ID`, `If-Match` set to the displayed version, and a new
   `Idempotency-Key`. It does not optimistically change the row. Exact retry of the same
   action/request key returns the committed receipt; reusing that key with another payload
   conflicts.
3. The Operator authenticates the OS peer, checks Workspace ownership and the selected
   Workspace, parses the expected version/key, then calls `WorkspaceRootService`. The
   service validates the allowed transition and checks a prior idempotency receipt before
   building a new aggregate/event. SQLite repeats ownership, root identity, status, and
   expected-version checks in an immediate transaction.
4. Pause accepts only `ACTIVE -> PAUSED`. It increments the WorkspaceRoot version and
   atomically commits the root snapshot, `workspace.root.status.changed.v1` event with
   `USER_PAUSED`, and idempotency receipt. It preserves the local identity bindings,
   ResourceLocation observation, and selected-folder replication preference. Consumers
   must gate any future observation, search, exposure, or transfer on root status `ACTIVE`.
5. A fresh Resume accepts only `PAUSED -> ACTIVE`. Before committing, the Runtime reopens
   the saved directory without following symlinks, checks its current file identity against
   the private prior binding and keyed Resource projection, and retains the verified handle
   through the atomic status/binding/event/receipt commit. The transaction also requires the
   ResourceLocation to be `AVAILABLE`, the current Runtime incarnation, and `READY` or
   `DEGRADED` Runtime state. `UNAVAILABLE` is recovered only by Runtime identity
   revalidation; a successful startup revalidation does not resume a previously paused root.
   An exact idempotent replay returns its original receipt without another filesystem read.
   This check proves identity at resume admission only. Future filesystem consumers must
   independently revalidate/use a qualified handle-based provider; no watcher/content reader
   currently consumes the root.
6. After a validated response, Tauri checks WorkspaceRoot ID, Workspace ID, target status,
   and exactly one version advance before updating the Library row. On an error or
   ambiguous response it keeps the prior row, reports the failure, and reloads rather than
   implying a successful transition. The Library states that folder watching/indexing are
   not active in this build.

**Failure/UI/postcondition:** stale version returns a conflict and leaves state unchanged;
wrong current status returns a conflict; a resume with unavailable/stale identity bindings
leaves the root paused; unauthorized or archived Workspace requests fail; storage failure
commits neither status, event, aggregate snapshot, nor receipt. Pause/resume do not create a
Capability, Grant, SecretLease, Effect, Resource revision, or claim that content is indexed.

**Tests for the later verification pass:** ACTIVE pause, PAUSED resume, exact-key replay,
changed-payload key reuse, stale version, invalid transition, non-owner/foreign Workspace,
archived Workspace, current and stale Runtime incarnation, unavailable location, missing
locator binding, missing file-identity binding, Runtime recovery states, atomic failure at
projection/event/receipt writes, preservation of root selection/bindings on pause, PAUSED
preservation across successful/failed restart revalidation, UNAVAILABLE recovery without
implicit resume, response loss after commit, malformed/mismatched UI response, Workspace
switch during request, and an owner workflow that pauses a folder, restarts LiteCowork,
confirms identity recovery leaves it paused, then explicitly resumes it. Also qualify and
test replacement of the directory after startup revalidation and before a fresh resume; the
resume path must reject that swap and persist the resulting unavailable location. Verify
that exact replay does not re-open the path or mutate state.

## F105 — Pin an exact Coworker revision to a paused Automation

**Actors/preconditions:** Workspace owner, authenticated local Operator IPC, an Automation
definition editor, a saved Routine revision, and Coworker list/detail read routes. This flow
only creates or revises an inert Automation definition; it does not start a trigger host,
create an occurrence, or admit a Task.

1. The editor loads Coworkers from the selected Workspace and offers active Coworkers with
   their current revision numbers. Creating a definition starts with no Coworker pin. Editing
   defaults to **Keep existing pin**, preserving the exact historical revision or null.
2. Selecting a Coworker records its exact current revision. Before save, the client reloads
   that Coworker and rejects a Workspace mismatch, inactive status, or changed revision; the
   owner must refresh the selection rather than silently pin a newer revision.
3. Create and revise requests include `coworker_ref` explicitly as either
   `{ coworker_id, revision }` or `null`. The authenticated daemon checks the Coworker and
   exact immutable revision in the selected Workspace; the SQLite responsibility transaction
   repeats that reference check atomically with the Automation snapshot, event, revision, and
   idempotency receipt.
4. Creation persists `PAUSED`. Revision is accepted only for a paused Automation and advances
   its immutable definition revision. Choosing **No Coworker pin** explicitly clears the
   source preference; choosing **Keep existing pin** preserves the old reference. Existing
   Automation revisions and Tasks are never rewritten.
5. The UI validates the committed Automation receipt and reports the paused state. Coworker
   selection supplies only the revision-pinned lead/worker/context/interaction defaults for
   a future occurrence; it does not create a Grant, approve an Effect, start a Runtime, or
   make an Automation executable. Recurring resume/schedule hosting remain unavailable
   until their separate cursor, TriggerHost, and reconciliation services are implemented.
   A later owner ManualTrigger command is a one-shot PAUSED-safe path and does not activate
   recurring triggers (see F121).

**Failure/UI/postcondition:** if the Coworker head has changed before the editor's final
read, foreign Workspace, missing Coworker revision, stale Automation version, or idempotency
payload conflict, the prior Automation remains unchanged and the editor draft is preserved.
The desktop does not offer archived Coworkers for a new pin; the owner may preserve an
existing historical pin. If the Coworker changes after the final client read, the immutable
revision selected by the owner remains the one pinned rather than silently following the new
head. Future occurrence admission separately checks the Coworker's current status. No Task,
occurrence, session, lease, Environment, capability invocation, Effect, or Evidence is created.

**Tests for the later verification pass:** create with null/current Coworker pin; revise
preserving exact old pin, changing to current revision, and clearing; Coworker head changes
before the final selection read; paused/archived Coworker visibility and preservation of a
historical pin; foreign/missing revision; cross-Workspace ID; stale Automation version;
exact idempotent replay and changed-payload replay; Coworker changes after client recheck
still pin only the explicitly selected immutable revision; editor state survives errors; all
saves remain PAUSED and create no occurrence/Task/session/lease/Environment/Effect/Evidence.

## F106 — Check local Task planning readiness without starting work

**Actors/preconditions:** Workspace owner viewing a saved Task in the desktop, an
authenticated local Operator connection, the current Task version, and the read-only
`GET /v1/tasks/{id}/planning-readiness` route. This is a diagnostic only; planning dispatch
is unavailable in this build.

1. Task details offer **Check readiness** when no PlanRevision is currently saved. The
   action sends the selected Workspace, Task ID, and the Task version already loaded by
   the view. It does not submit objective text, a planning packet, provider information,
   or a session identifier.
2. Tauri calls the authenticated Operator with `X-Workspace-ID` and `If-Match`. It accepts
   only the strict bounded response shape, the requested Task ID/version, a known Task
   status and blocker enum set, and literal `false` for dispatch/session/Plan flags. It
   rejects unknown fields, duplicate/unknown blockers, or a response for another/stale
   Task version.
3. The daemon reauthorizes Workspace ownership, reloads the Task, and compares its current
   aggregate version with `If-Match`. It reports eligibility and sanitized local blockers.
   The preflight may construct its bounded planning packet transiently, but returns none of
   the objective/context, endpoint ID, provider handle, or packet content. It does not
   persist a PlanningAssignment or reserve an AgentSession. It creates no Plan, Step,
   Attempt, lease, or Environment; it starts no process, invokes no provider, and appends no
   domain state/event.
4. The desktop renders translated blocker descriptions and the observation time for that
   exact Task version. It always states that planning dispatch remains unavailable and
   that this result does not authorize or start work. If no local blocker is observed, the
   UI explicitly says that this does not mean planning can start. Stale responses clear
   the diagnostic and offer a Task reload; switching Tasks cannot display the earlier
   result.

**Failure/UI/postcondition:** a stale version returns a conflict without retrying against
newer state; authentication/Workspace mismatch and missing Task are reported generically;
malformed or over-limit responses fail closed. The screen never exposes blocker internals
as executable controls and creates no durable Task, AgentSession, Plan, Step, Attempt,
ExecutionLease, Environment, Invocation, Effect, or Evidence.

**Tests for the later verification pass:** exact-version success; missing/malformed/zero
If-Match; stale Task version; foreign Workspace/owner; missing Task; every supported
blocker; duplicate/unknown blocker; extra response field; mismatched Task/version/spec;
any true dispatch/session/Plan flag; bounded response; no event or storage mutation; no
provider/process/session start; request racing Task edit/plan acceptance; Task switch and
late-response suppression; offline Runtime; keyboard activation and screen-reader status;
and no-blocker result still explicitly denies start availability. Compare Task and event
state before/after the diagnostic to prove it remains read-only.

## F107 — Link or unlink Workspace work from a Goal

**Actors/preconditions:** Authenticated Workspace owner editing a non-archived Goal.
Task and Artifact pickers read the existing selected-Workspace catalog. Artifact links pin
the exact current version observed by the picker.

1. The editor loads Tasks and Artifacts through the authenticated local Operator bridge.
   Task choices are identified by Task ID; Artifact choices show name/type/current version.
   Every response is checked against the selected Workspace boundary.
2. The owner selects or clears Task links and selects or clears Artifact links. Linking
   never starts a Task. An Artifact selection records
   `{workspace_id, artifact_id, version}`; later Artifact versions do not retarget it.
3. Save sends a complete new Goal revision using the current Goal version in `If-Match`
   and a stable `Idempotency-Key`. GoalService validates each referenced Task,
   RoutineRevision, and ArtifactVersion exists in that Workspace, then commits the
   immutable revision, links, aggregate event/snapshot, and replay receipt atomically.
4. Unlinking omits that reference from the new revision. Prior Goal revisions keep their
   original links. Neither linking nor unlinking mutates the linked Task, Routine, or
   Artifact and neither operation changes execution authority.
5. The UI replaces the displayed Goal only with the committed receipt. A stale Goal
   version or missing/foreign reference preserves the draft and requires reload/review;
   it never reports an optimistic link as saved.

**Failure/UI/postcondition:** cross-Workspace and missing Artifact-version references are
rejected without a Goal revision/event. Artifact version changes during editing do not
silently advance a pin. Paused Goals may retain or revise links; archived Goals are
read-only. No Task, Attempt, AgentSession, lease, Environment, Effect, or Evidence is
created or modified.

**Tests for the later verification pass:** link/unlink Task; link/unlink exact Artifact
version; Artifact receives a newer version before save and the selected old version remains
pinned; missing Artifact version; cross-Workspace Task/Artifact; duplicate Artifact IDs;
archived Goal; stale Goal version; identical idempotent replay and payload conflict;
restart/readback of prior and current Goal revisions; no linked entity state change; picker
pagination, Workspace switch, offline/error and keyboard/screen-reader behavior.

## F108 — Owner accepts an actionable Suggestion as a Task

**Actors/preconditions:** Authenticated Workspace owner; active Workspace and local
Runtime binding; `PROPOSED` unexpired `TASK` Suggestion with a valid pinned TaskSpec
proposal; current `If-Match`; stable `Idempotency-Key`; an enabled lead-eligible binding
from the originating Coworker or Workspace default.

1. The owner reviews the proposal's objective and provenance, then selects **Create Task**.
   Before dispatch, the UI records the exact Suggestion ID/version, Workspace ID, and
   idempotency key in its bounded process-local recovery registry. It retains that same
   request across route navigation and offers exact retry if the response is lost; it
   never starts execution from the Suggestion action.
2. Operator authenticates the owner and selected Workspace, runs bounded event-backed
   expiry settlement, then checks the current Suggestion status/version/expiry and
   validates the exact proposal. It resolves the current
   Coworker revision/head version when the Suggestion has a Coworker origin, otherwise
   uses the Workspace lead default. The chosen lead must still be enabled and lead-eligible.
3. TaskService builds the normal `READY` Task and immutable initial TaskSpecRevision from
   the proposal. Objective, constraints, pinned Resource inputs, outputs, criteria,
   budgets, and deadline must match exactly. The Task pins Coworker origin when present.
4. One SQLite immediate transaction writes the Task, TaskSpec, Task event/snapshot and
   request receipt, then changes the Suggestion to `ACCEPTED`, records the owner and
   `result_task_id`, and writes its resolution event/snapshot. If either side fails, the
   entire transaction rolls back.
5. A created response is `201` with `disposition=CREATED`; an exact replay is `200` with
   `disposition=REPLAYED`. Both return a bounded `SuggestionTaskAcceptanceReceipt`. The
   UI requires the receipt Workspace and Task Workspace to match the selected Workspace,
   the Suggestion identity/status/version to match the submitted request, and the linked
   Task ID to match the READY Task ID before clearing recovery state. A lost-response
   retry returns the already linked Task; it does not create another Task or append
   another Suggestion resolution. A different request payload using the same idempotency
   key conflicts. Stale version, expiry, changed Coworker head, missing lead, unavailable
   source revision, or invalid proposal leaves no partial Task.
6. The desktop opens the committed Task in Work. Its status remains `READY` and the UI
   says it is saved for review. Planning, AgentSession creation, and execution require
   the ordinary independent Task admission flow.

**Failure/UI/postcondition:** non-Task Suggestions cannot use this endpoint. The normal
Suggestion resolve endpoint handles dismissal only. `ACCEPTED` TASK Suggestions always
link to the created Task; neither acceptance nor retry grants authority, creates an
Effect, invokes a provider, or runs code.

**Tests for the later verification pass:** happy path creates one READY Task and pins
exact inputs/Coworker revision; Task and Suggestion rows/events/snapshots/idempotency
receipt commit together; injected failure between Task insert and Suggestion update rolls
everything back; concurrent acceptors yield one linked Task; exact request replay returns
the same Task without duplicate events; same key with changed payload conflicts; response
loss and retry still opens the linked Task after Coworker revision changes; UI route
unmount/remount and Workspace A→B→A retain the exact acceptance RequestId; wrong Workspace,
Suggestion ID/status/version/result link, Task ID/Workspace/status/version, or HTTP
status/disposition receipts preserve the recovery entry; late response after explicit
discard cannot clear a different newer entry; stale
Suggestion version, expired/non-TASK/malformed proposal, foreign Workspace, archived
Workspace/Coworker, changed Coworker head, disabled lead, missing lead, missing or
cross-Workspace source Resource revision, and authorization revocation create no Task;
acceptance never creates a Plan/Step/Attempt/AgentSession/ExecutionLease/Environment/
CapabilityInvocation/Effect/Evidence; successful UI opens Work and does not show a
running state; offline Runtime, duplicate click, accessible busy/error state, and keyboard
activation.

## F109 — Read exact Coworker revision provenance

**Actors/preconditions:** Authenticated Workspace owner reviewing a Task that stores an
`origin_coworker_id` and `origin_coworker_revision`; active local Runtime Workspace
binding; the pinned historical revision remains retained.

1. Task detail reads the Task's immutable Coworker origin ID and revision number. It does
   not infer the origin from the Workspace primary Coworker or the current Coworker head.
2. The desktop calls `GET /v1/coworkers/{coworker_id}/revisions/{revision}` through the
   finite authenticated native Coworker bridge, carrying the selected Workspace context.
3. Operator verifies Workspace ownership and the current Runtime Workspace binding,
   reads that exact immutable revision from `SqliteCoworkerStore::get_revision`, then
   rechecks authorization before returning the definition, author, and creation time.
   The response is `no-store`; missing or foreign-scope data is returned as unavailable.
4. The client verifies both returned IDs against the Task pin and renders the historical
   Coworker definition as provenance. It does not use that definition as current Task
   authority or mutate either the Task or Coworker.

**Failure/UI/postcondition:** unavailable historical revision remains visibly unavailable;
the client never substitutes the Coworker's current revision. The command creates no
event, new revision, Task state, grant, or execution record.

**Tests for the later verification pass:** exact older revision differs from current and
is returned unchanged; nonexistent revision; zero/overflow/malformed path revision;
foreign Workspace header, non-owner, archived Coworker with retained history, revoked
Runtime Workspace binding during read, authorization recheck failure, changed/malformed
response ID or revision, no mutation/event, `Cache-Control: no-store`, finite Tauri path
mapping, and abort signal behavior.

## F110 — Review saved Task outcome and activity

**Actors/preconditions:** Authenticated Workspace owner opening one persisted Task in the
desktop Work view; the selected Workspace and Task identity are pinned for the request.

1. The desktop requests the finite authenticated Task presentation snapshot. The Operator
   returns only committed Task, current-plan Step, and Task-linked ArtifactVersion records;
   it does not infer live provider activity, verification, progress, or blockers that are
   absent from those records.
2. The client checks Workspace/Task identity and validates each bounded PresentationItem.
   Unsupported or malformed items are omitted with a visible count; late responses from a
   prior Task or Workspace are discarded.
3. The default view shows the saved objective/status first, then committed output cards,
   then at most three recent saved activity items. The activity count describes
   presentation items, not necessarily Steps. Full activity and source identifiers remain
   in collapsed detail disclosures.
4. `CURRENT` means the included persisted source records came from one consistent SQLite
   read snapshot. `STALE` or `UNKNOWN` values from other projection sources remain distinct.
   Snapshot time is not called “last verified” or evidence time. Refreshing an open/visible
   panel may reload the finite snapshot; it does not create a stream or synthesize activity.
5. If the local Runtime goes offline or refresh fails while this same Task is selected,
   the client retains the last successfully loaded snapshot for that Workspace/Task and
   labels it as possibly out of date. Selecting a different Task clears the prior snapshot;
   an old response cannot populate the new selection. If no snapshot was loaded, show the
   unavailable state without implying that cached content exists.

**Failure/UI/postcondition:** Offline state, stale data, and malformed items remain
visible without replacing the Task identity. No Task, Attempt, Effect, Evidence, or
Verification record is created or advanced by viewing the panel.

**Tests for the later verification pass:** empty snapshot; absent objective/activity/output;
unknown status; current/stale/unknown freshness labels; activity count with non-Step items; more
than three records and deterministic ordering; duplicate and invalid item; foreign or late
Task response; Runtime disconnect after a successful snapshot retains that same Task's
snapshot and marks it possibly out of date; a Task switch clears the prior snapshot; offline
with no saved snapshot shows unavailable; source IDs stay collapsed; no evidence claim from
activity; refresh does not mutate Task state or emit domain events.

## F111 — Preview a managed Markdown Artifact safely

**Actors/preconditions:** Workspace owner has opened an authorized immutable managed
ArtifactVersion whose media type is Markdown and whose content is within the existing
preview size bound. The version is resolved through the normal Artifact authorization
boundary.

1. The Workbench loads exact-version bytes and decodes UTF-8 strictly. Content is treated
   as untrusted text.
2. The client parses only the bounded supported subset: headings, paragraphs, simple
   lists, blockquotes, fenced code, inline emphasis/code, and HTTPS links. It creates React
   elements/text nodes; it never inserts source through raw HTML, fetches remote images, or
   interprets embedded HTML/commands.
3. A malformed or unsupported construct causes the whole preview to fall back to the
   original text rather than rendering a partially parsed document. The original immutable
   source remains available through the existing raw-text view/download path.
4. Opening an HTTPS link requires explicit confirmation showing the destination. The
   desktop opens it outside the Workbench with opener isolation; other protocols are not
   rendered as links.

**Failure/UI/postcondition:** Invalid UTF-8 or content outside preview bounds is not
rendered as Markdown. Renderer failure preserves the authorized raw content fallback and
does not change ArtifactVersion state.

**Tests for the later verification pass:** common supported syntax; malformed/unclosed
fence; nested/indented/table syntax fallback; raw HTML/script displayed only as escaped
text; `javascript:`, `file:`, credential-bearing, and malformed URLs never navigate;
HTTPS requires owner confirmation; no image/network request; strict UTF-8 and size limit;
renderer failure and download fallback; no Artifact mutation.

## F112 — Inspect immutable Task specification history

**Actors/preconditions:** Authenticated Workspace owner has opened a saved Task in the
desktop Work view. The Task identity and selected Workspace are fixed for the history read.

1. The history disclosure requests the exact Task's immutable specification revisions from
   the authenticated Workspace-scoped Operator route only when opened.
2. The Tauri bridge verifies Task identity, positive ascending revision numbers, parent
   revision ordering, author kind/identity, and timestamp presence before returning the
   bounded response to the WebView. Workspace ownership is enforced by the selected-workspace
   route; any Workspace identity supplied by storage output must match that selection.
3. The UI lists newest revision first and shows the objective, author, timestamp, and exact
   parent revisions. The view is read-only; it cannot restore, revise, or change Task state.
   The history head is compared with the Task detail's loaded current revision: equal heads
   identify “Current”; a newer history head is labeled “Latest saved” and offers “Reload
   Task”; a history head behind the detail is disclosed as stale/incomplete and offers
   explicit Task reload and history refresh. Reload is unavailable while an objective draft
   or unresolved revision request is open, so stale-data refresh cannot discard that draft.
   The UI never labels a non-head revision current.
4. On a Task/Workspace switch, prior history is cleared and late responses are discarded.
   If the Runtime disconnects after history loaded, the same Task's saved history remains
   visible with an out-of-date notice. If no history was loaded, the UI shows unavailable
   and allows an explicit retry after reconnect.

**Failure/UI/postcondition:** A malformed, foreign, or inconsistent response is rejected.
Viewing history creates no Task revision, event, Attempt, or provider call.

**Tests for the later verification pass:** lazy load; empty/malformed history; unordered or
duplicate revision IDs; invalid parent; foreign Task/Workspace; author/timestamp mismatch;
Task change during request; history head newer than the loaded Task; history head behind the
loaded Task; explicit Task reload and history refresh; reload blocked during dirty or
unresolved objective edits; offline after successful load; offline before first load;
refresh/retry; keyboard disclosure; no mutation or restore action.

## F113 — Compare immutable Artifact versions

**Actors/preconditions:** Workspace owner has an authorized Artifact open in the Workbench,
selected two distinct committed managed-text versions within the existing preview bounds.

1. The owner explicitly selects another committed version. The Workbench requests exact
metadata/content for the selected and compared versions through the
existing authenticated Artifact routes.
2. The client checks Artifact identity, selected version numbers, supported managed media
types, size bounds, strict UTF-8, and content authorization. Side-by-side view uses the
same bounded renderer as ordinary preview.
3. For text content, the owner may select a bounded literal line diff: at most 400 lines
per version, 160,000 LCS line-pairs, and 16,384 characters per line. Each view labels both
exact immutable Artifact versions and backing ResourceRevisions. Outside these limits, the
UI retains side-by-side rendering. It does not infer semantic changes.
4. Unsupported, missing, revoked, oversized, or invalid prior content leaves the selected
   version and history intact, keeps the selected-version preview available where possible,
   and shows a comparison preview error. Authorization failure for the selected version
   itself invalidates its metadata/content view. Compare cannot publish, restore, or mutate
   either version.

**Failure/UI/postcondition:** Both panes always correspond to the exact version labels.
Renderer/API failure does not silently substitute the current head or alter Artifact state.

**Tests for the later verification pass:** identity mismatch; unsupported media; missing or
revoked compared version; authorization denial for compared versus selected version; invalid
UTF-8; size limit; exact ResourceRevision labels; Markdown fallback consistency; equal,
empty, insertion, deletion, replacement, reorder, and CRLF-normalized line comparison;
hostile markup remains literal; every line-diff bound falls back without losing side-by-side
view; keyboard toggle and accessible summary; refresh/close while editing; dirty-draft
preservation; no Artifact mutation; download and provenance remain available after compare
preview failure.

## F114 — Save an exact Artifact version from desktop

**Actors/preconditions:** Workspace owner has an authorized managed ArtifactVersion selected
in the desktop Workbench; stored content is at most 10 MiB.

1. Before opening the dialog, the native bridge re-reads the Artifact and exact version
   metadata over authenticated local Operator IPC. It checks Workspace/Artifact/version
   identity, the selected ResourceRevision, media type, content digest, size, and the
   matching backing ResourceRevision. The dialog suggests a sanitized name containing the
   exact version number. Cancel performs no content read and creates or truncates no file.
2. After a destination is selected, the bridge reads only the exact managed version through
   the authorized content route. The daemon resolves the immutable version and verifies the
   stored blob against its digest; the native bridge checks returned media type and exact
   byte count before writing.
3. The native bridge enforces the 10 MiB limit and writes verified bytes through a
   same-directory temporary file followed by rename. Content bytes and the full destination
   path do not cross the WebView command boundary.
4. The UI reports Saved or Cancelled. External linked content and content above 10 MiB
   remain unavailable to this Save As path; no provider fetch or larger IPC allocation is
   attempted.

**Failure/UI/postcondition:** A failed metadata/content/integrity check leaves the chosen
destination untouched. A write failure is reported without claiming success. No Artifact,
Resource, Task, or Event is mutated.

**Tests for the later verification pass:** dialog cancel; exact selected historical version;
Workspace/Artifact/version mismatch; ResourceRevision/digest/size/media drift; archived but
readable version; external content; exactly 10 MiB and over-limit content; content-route
denial; digest mismatch; write/overwrite failure preserves prior target; safe filename;
no bytes or local path returned to WebView; no domain mutation.

## F115 — Save the original current Resource from Library

**Actors/preconditions:** Workspace owner selects a current Library Resource no larger than
10 MiB in the desktop. The operation is a local export, not a Resource mutation.

1. The UI offers **Save original…** only when the catalog size is within 10 MiB. The native
   command validates the selected Workspace/Resource/revision IDs, digest, size, and media
   type. It re-reads Resource detail and bounded revision history through authenticated
   local Operator IPC. Only a current `FILE` from the managed local-upload provider is
   eligible. It requires the detail head and revision record to identify the exact selected
   revision and match its digest, size, and media type. If a ContextDocument exists, its
   status must be `ACTIVE`; owner-facing errors distinguish revoked, deletion-pending, and
   deleted content.
2. After validation, the command opens the native save dialog with a sanitized suggested
   filename. Cancel returns `CANCELLED`, performs no content read, and touches no file.
3. After destination selection, the command requests only the selected revision through the
   authorized Resource content route. A concurrent head change or ContextDocument status
   transition is rejected by that route. The daemon verifies the managed blob; native code
   checks the exact media type and byte count, then writes through the existing atomic
   same-directory temporary-file path.
4. The WebView receives only `SAVED` or `CANCELLED`. Bytes and the full destination path
   stay native. ZIP files are copied as opaque original bytes; this flow does not parse or
   extract them. No Resource, Artifact, Task, or domain Event is created or changed.

**Failure/UI/postcondition:** Non-file, non-managed, external/unavailable, stale, inactive,
or over-limit content is refused without writing. A read admitted for an ACTIVE
ContextDocument may race a later revocation; the final content route rechecks status and
rejects that read before returning bytes. If authorization, metadata, size, media type, or
integrity cannot be verified, report the typed/safe failure and leave the selected target
untouched. No endpoint, path, or content is sent back to the WebView.

**Tests for the later verification pass:** cancel before content fetch; current exact head;
stale head before picker and after selection; digest/size/media mismatch; non-FILE kind;
external provider; unavailable blob; each non-ACTIVE ContextDocument status before and
after picker; exactly 10 MiB and over-limit; invalid filename sanitization; ZIP saved
byte-for-byte with no extraction; atomic write failure/overwrite behavior; no bytes or
destination path in IPC response; no domain mutation.

## F116 — Create a Workspace-owned ContextDocument note in Library

**Actors/preconditions:** Workspace owner, desktop Library, authenticated local Operator,
existing resumable Resource upload protocol, encrypted BlobStore. The selected Workspace is
active and owned by the local Principal.

1. The Library presents a title and short-text form. V1 offers only `WORKSPACE_NOTES`; the
   UI identifies the selected Workspace as the owner and does not offer personal, Coworker,
   or Goal scopes without the corresponding owner-selection and authorization flow.
2. The desktop normalizes the title into a safe `.md` Resource filename, bounds the note to
   64 KiB of UTF-8, computes its whole-content SHA-256, and sends the bytes through the
   existing Tauri resumable upload commands. The create request pins
   `context_document: {kind: WORKSPACE_NOTES, owner_ref: {kind: WORKSPACE, workspace_id}}`;
   it has no folder-import metadata. The metadata is part of the resume identity and the
   returned upload session must echo exactly the same two-field owner/kind create metadata
   before chunks are accepted by the desktop flow. Upload sessions have no status or purge
   fields; Resource commit supplies the initial ACTIVE ContextDocument status.
3. The Operator authenticates the owner, checks the Workspace, validates the metadata, and
   ResourceService/storage pin it in the upload session. The normal fixed-size chunk,
   digest, idempotency, expiry, and atomic Resource commit semantics apply. Commit creates
   an ordinary managed `FILE` Resource with immutable initial ResourceRevision and active
   ContextDocument metadata.
4. The desktop adds the committed Resource to the Library. The owner can use its ordinary
   revision history, append revisions, and ACTIVE/REVOKED controls. The note is not
   automatically added to an AgentSession and does not imply semantic RAG, memory curation,
   purge, or deletion.

**Failure/UI/postcondition:** Invalid owner/kind metadata, Workspace mismatch, upload
conflict, digest mismatch, expiry, or storage failure produces no successful Library item.
The same stable RequestId and exact content resumes/replays through the existing upload
contract; a ContextDocument upload cannot resume as an ordinary Resource upload or vice
versa. Workspace notes remain scoped to that Workspace. No new API route, upload protocol,
filesystem grant, or semantic retrieval behavior is introduced.

**Tests for the later verification pass:** owner scope/copy is explicit; exact entered body
bytes are preserved without implicit trim/newline; UTF-8 byte limit including multi-byte
text and exact 64 KiB boundary; empty note and invalid title; safe filename; metadata passes
through Tauri without status/purge fields; session echo mismatch fails closed; resume key
separates ordinary files from ContextDocuments; upload retry preserves request/chunk
idempotency; Workspace mismatch/archived owner rejection; exact digest/chunk coverage;
commit visibility only after Resource response; resulting metadata appears in history;
ordinary uploads remain metadata-free; no automatic agent context attachment or RAG claim.

## F117 — Preserve unsaved Resource revision drafts and require explicit rebasing

**Actors/preconditions:** Workspace owner is viewing a Resource's revision history. A
replacement file may be staged, or a supported text Resource may be open in the editor.

1. The editor compares text draft content with the exact text loaded from the selected
   Resource revision. Merely opening an unchanged text preview is not a dirty edit. A
   staged replacement file or changed text draft is unsaved local work.
2. If the revision upload returns `RESOURCE_CONFLICT`, the editor preserves the exact
   staged `File`, text draft, and original text baseline, clears any prior rebase approval,
   and reloads current Resource detail/history. It does not retry automatically. The owner
   must inspect the reloaded Resource version/current head and explicitly authorize the
   preserved complete draft as a new revision on that head. The approval is pinned to that
   Resource version/head; any later conflict invalidates it and requires review again. For
   a supported small text Resource, the editor can preview the exact refreshed head through
   the authenticated bounded text-read command before approval. The preview is pinned to
   that head and never replaces the owner's draft.
3. Selecting a different file while another file or changed text draft is staged does not
   replace it immediately. The new selection is pending until the owner confirms replacing
   the current draft. Canceling the native file picker changes nothing.
4. When the owner closes a dirty editor, the UI keeps it open and shows an accessible
   confirmation with `Keep editing` and `Discard draft and close`. No upload or Resource
   mutation occurs from opening this prompt.
5. `Keep editing` dismisses the prompt without changing the draft. `Discard draft and close`
   clears only the local staged file/text state and closes the panel; it does not alter
   committed Resource bytes, revisions, ContextDocument status, or upload sessions.
6. While an upload/commit is in progress, close remains disabled. ContextDocument
   availability changes are blocked while a local draft exists. If a status response is
   ambiguous, the existing idempotency request remains available for retry and editing or
   closing is paused until that status request is resolved; this prevents a committed
   revocation from silently hiding an unsaved draft.

**Failure/UI/postcondition:** A clean editor closes directly. A dirty editor cannot be
closed through its close control without an explicit discard decision. The prompt has no
effect on revision admission or conflict handling.

**Tests for the later verification pass:** unchanged loaded text closes without prompt;
changed text prompts; reverting text exactly to the loaded baseline clears dirty state;
selected file prompts; canceled file picker changes nothing; replacing either a staged file
or changed text requires explicit confirmation; keep editing preserves text/file selection;
discard clears only local draft and closes; a stale-head conflict preserves exact file,
text and baseline; no automatic retry; current Resource version/head is reloaded; explicit
rebase approval pins that exact version/head; text-head preview reads only the exact current
revision and cannot overwrite the draft; a late preview for a previous head cannot count as
review of the new head; a subsequent conflict invalidates approval;
close is disabled during upload/commit or unresolved status retry; availability change is
blocked while dirty; ambiguous status retry uses the same RequestId; upload conflict
behavior is unchanged; keyboard and screen-reader dialog operation; reduced motion; no
Resource/API call when a confirmation opens or when a draft is discarded.

## F118 — Preview an exact pinned Task Resource input

**Actors/preconditions:** Workspace owner viewing a saved Task in the desktop; the Task
contains a `ResourceRef` pinned to a Resource revision in that Workspace; the local
authenticated Tauri/Operator path is available.

1. Task detail displays each input's Resource name where available and its exact pinned
   revision ID. It offers a native `details` disclosure for a read-only text preview; the
   content read does not happen until the owner opens the disclosure.
2. The reusable UI sends the selected Workspace ID, Resource ID, and exact revision ID to
   the existing `preview_resource_text` Tauri command. It rejects a missing/foreign
   Workspace or incomplete pin before invoking. It does not send a current-head or
   unpinned request.
3. Tauri uses the authenticated local Operator content route with `revision_id` and
   `max_bytes=1048576`. Storage verifies owner scope, exact Workspace/Resource/revision
   membership, ContextDocument ACTIVE status, local managed provider availability, stored
   length/digest, and size before BlobStore access. Tauri accepts only its inert UTF-8 text
   preview allowlist and renders bytes as escaped plain text; it never interprets active
   content.
4. A historical local revision is previewed from its own immutable digest; it never retries
   against or substitutes the Resource's newer head. External, missing, oversized,
   inactive, invalid-UTF-8, or integrity-failing content reports a typed error. The owner
   may retry the same exact Workspace/Resource/revision pin.
5. Changing the input props, closing the disclosure, or unmounting suppresses any late
   response. A pin-specific key prevents text loaded for a previous Resource revision from
   appearing under the new label. Previewing does not mutate Task/Resource state, attach
   content to an AgentSession, create a Grant, or start planning/execution.

**Failure/UI/postcondition:** The exact revision ID stays visible in the disclosure. No
content is shown for a mismatched Workspace/Resource pin. No fallback to the current
Resource head occurs. The component is a user preview only and is not a
ContextPlanner or RAG implementation.

**Tests to add:** valid exact pin; missing/cross-Workspace ResourceRef rejected before
invoke; text response rendered as text (including HTML-looking strings); non-string
response; content exactly at 1 MiB and over the cap; unsupported media; invalid UTF-8;
missing Resource; historical pin returns its own content after a newer head commits;
cross-Resource revision ID is rejected; transient Runtime error and same-pin retry; disclosure is lazy; closing,
changing pins, switching Workspace, and unmounting suppress late responses and never show
prior content; exact revision label remains stable; preview invokes no mutation, Task,
Attempt, Grant, AgentSession, RAG or semantic retrieval.

**System/user acceptance:** Run against a real local Runtime with a selected Task input,
verify its exact immutable revision is displayed and previewed, then revise the Resource
and confirm the old Task pin still returns the old bytes rather than showing new bytes. Confirm a
nontechnical owner can preview without Inspector, and confirm the preview does not cause
agent work. Record OS/Runtime versions and sanitized evidence. This component is currently
mounted in Task detail as a lazy disclosure. This source integration remains unverified and
is not a ContextPlanner or RAG implementation.

## F119 — Run an active Routine into a saved READY Task

**Actors/preconditions:** Workspace owner, desktop Routine page, authenticated local
Operator, RoutineService, TaskService, SQLite TaskStore. Selected Workspace is ACTIVE; the
Routine is ACTIVE and its exact current immutable revision is loaded. The selected lead is
configured, enabled, and lead-eligible. A Routine with unsupported input bindings cannot
be run from this UI.

1. The owner opens an active Routine. The form renders only supported schema-bound TEXT
   inputs and same-Workspace Resource selectors. A Resource selection carries the exact
   immutable revision; opaque Resource IDs are never requested from the user. If a Resource
   cannot be selected from the loaded catalog, Run remains blocked until it is loaded.
2. The owner submits `POST /v1/routines/{id}/run` with `X-Workspace-ID`, stable
   `Idempotency-Key`, exact `routine_revision`, and the bounded input object. A non-null
   Conversation origin is rejected until Conversation/Task admission is atomic.
3. The authenticated route loads that immutable revision and materializes a bounded
   TaskSpec proposal. SQLite then opens the ordinary Task `IMMEDIATE` transaction and
   rechecks owner/Workspace, active Routine/current revision, exact input schema and
   rendered TaskSpec equivalence, lead eligibility, and every pinned same-Workspace
   Resource revision. These checks and Task/TaskSpec/event/receipt commit are one admission
   boundary; any failure creates no Task.
4. Success returns `TaskView` with Task `READY`, TaskSpec revision 1, and exact Routine
   ID/revision provenance. The UI shows “Saved Task · READY” and states that planning and
   execution have not started. It may open Task details only after checking Workspace,
   Task identity, Routine pin, TaskSpec identity, and status in the response.
5. A retry with the same authorized owner, selected Workspace, RequestId, revision, and
   normalized inputs returns the original committed Task without another event. Changed
   content under the same key conflicts. A new key is subject to the current ACTIVE/head
   checks. No Resource bytes are resolved, no agent session starts, and no Task execution
   record is created by this flow.

**Failure/UI/postcondition:** Invalid/unbound/extra input, missing lead, archived or stale
Routine revision, unavailable/cross-Workspace Resource, failed owner admission, or
idempotency mismatch returns an error with no Task and no success receipt. A stale form is
refreshed before a new RequestId is submitted; the original RequestId is retained for an
exact retry after an ambiguous response. Routine required-capability and verification
policies remain pinned on the source revision and are not claimed as enforced by this
save-only command.

**Tests for the later verification pass:** empty input schema; valid/invalid required TEXT;
valid/invalid TEXT enum choice;
byte/character limits; control characters; invalid/escaped JSON pointers; unsupported
integer/array/nested shapes rejected at revision admission; required Resource selection is
available only through the loaded same-Workspace catalog; optional Resource omitted safely;
cross-Workspace, duplicate, stale, or missing Resource
rejected; changed Routine head/archive racing the Task transaction; missing/ineligible lead;
Task status READY and exact spec/provenance; zero Plan/Step/AgentSession/Attempt/lease/
Environment/Effect/grant; same RequestId returns same Task/event; changed payload conflicts;
owner/Workspace mismatch denied before replay; restart after lost response; accessible
input form/result status; task-detail link rejects a mismatched Workspace or response.

## Shared flow invariants

- Every mutating command has an authenticated principal, RequestId, correlation ID, and
  expected aggregate version where concurrent edits are possible.
- Aggregate mutation and its event commit atomically. UI motion waits for the resulting
  projection event.
- Direct/native reports never receive stronger evidence than the adapter/provider
  actually proves.
- External content and trigger payloads are untrusted; they cannot add grants, approvals,
  or SecretLeases.
- Failure preserves committed Conversation, Task, Artifact, Effect, and Evidence history.
  Retries retain explicit provenance rather than rewriting history.

## F120 — Compare two exact Resource text revisions

**Actors/preconditions:** Workspace owner viewing Library revision history for one Resource.
Both selected revisions are committed members of that same Resource and have allowlisted
`text/plain`, `text/markdown`, or `text/x-markdown` media types with recorded sizes no
larger than 1 MiB. Selection is limited to revisions already loaded from that Resource's
paginated history.

1. The owner opens **Compare text revisions**. Neither revision selector defaults to a
head; the owner explicitly chooses both different revision IDs. Unsupported or oversized
revisions are not offered as choices.
2. On Compare, the UI pins Workspace, Resource, and both exact revision IDs and asks the
   Tauri native bridge to read each using the authenticated content route with
   `revision_id` and `max_bytes=1048576`. Both responses must identify the requested
   revision, meet the allowlisted media type and byte limit, decode as UTF-8, and pass the
   daemon's stored digest/length verification.
3. The UI presents the raw escaped text in two labeled panes, including exact revision IDs,
   observed timestamps, media types, and byte sizes. Markdown is displayed as source text;
   HTML/SVG and active content are never interpreted. No generated diff, merge, restore,
   mutation, or claim of semantic change detection is implied.
4. If either read fails, the existing Resource history/editor and any prior successful
   comparison remain unchanged; the comparison area shows the failed exact pin and offers
   retry for the same selections. Changing either selection invalidates in-flight responses
   so text from old pins cannot appear under new labels.

**Failure/UI/postcondition:** Cross-Resource or cross-Workspace IDs return no bytes. A
historical pin reads its own immutable managed blob after the head advances; missing or
external historical content returns a typed unavailable/unsupported error with no fallback
to current bytes. Inactive ContextDocuments deny both historical reads. The comparison is
read-only and creates no event, Task, Attempt, Grant, AgentSession, or index/RAG activity.

**Tests to add:** no implicit default selection; same ID rejected; revision IDs must be in
loaded history and belong to this Resource; unsupported HTML/SVG/JSON/binary and >1 MiB
entries cannot be selected; exactly 1 MiB accepted; historical content remains exact after
head advancement; cross-Resource and cross-Workspace pins return no bytes; archived
Workspace follows owner-read policy; revoked/deletion-pending/deleted ContextDocument
blocks both pins before BlobStore access; external provider returns
`RESOURCE_CONTENT_EXTERNAL`; missing local blob returns `RESOURCE_LOCATION_UNAVAILABLE`;
size bound checked before BlobStore fetch; digest/length mismatch returns
`INTEGRITY_FAILURE`; one failed pane does not clear current Resource/editor state or a
prior successful comparison; selection change/unmount suppresses late reads; response
revision headers must match pins; plain text/Markdown including HTML-looking strings is
rendered inertly; retry reuses exact pins; no mutation, active-content renderer, diff claim,
or semantic retrieval.

**System/user acceptance:** Compare two same-Resource Markdown revisions after publishing a
newer head; confirm both panes show their exact selected historical content and labels.
Select an external or inactive ContextDocument revision and confirm the existing Library
view remains available with a clear error. Record sanitized evidence; no tests or OS
qualification are implied by the UI source change.

## F121 — Owner runs a paused Automation through its local ManualTrigger

**Actors/preconditions:** Workspace owner, authenticated local Operator API, active
Workspace, exact current Automation version/revision, active pinned RoutineRevision, an
optional active pinned CoworkerRevision, a ManualTrigger, and the exact current local
Runtime incarnation with an active `TRIGGER_HOST` Workspace binding. Automation status may
be PAUSED or ENABLED; DISABLED is terminal. This is one-shot materialization only; it does
not enable recurring triggers, create a Plan, or start an agent.

1. The owner submits `POST /v1/automations/{id}/run` with selected Workspace,
   `Idempotency-Key`, `If-Match` Automation aggregate version, and exact
   `{ automation_revision, inputs }`. `inputs` is treated as the Routine input object and
   validated by the pinned RoutineRevision schema/bindings, including same-Workspace exact
   Resource revisions.
2. The daemon derives occurrence identity from ManualTrigger ID, authenticated Principal,
   RequestId, and Automation ID. It pins AutomationRevision/RoutineRevision/trigger and
   the local TriggerHost Runtime/incarnation/binding version. A recurring cursor, if one
   already exists, must name the same revision and local host; a paused never-enabled
   definition may have no cursor.
3. TaskStore opens one `IMMEDIATE` transaction and first checks the owner/request receipt.
   For a new command it atomically rechecks active Workspace, expected Automation version
   and current revision, non-DISABLED status, exact ManualTrigger, active Routine/Coworker,
   eligible lead, current Runtime incarnation/active TriggerHost binding, Routine input
   schema/bindings, and exact Resource pins. Any failed check rolls back without an
   occurrence or Task.
4. The transaction creates PENDING occurrence version 1, claims it as CLAIMED version 2
   with claim_epoch 1, writes those events and snapshots with matching `entity_revision`,
   creates ordinary Task/TaskSpecRevision(1) with status READY and exact provenance, then
   links the Task and advances the occurrence to STARTED version 3/claim_epoch 1. Task,
   occurrence, snapshots/events, and idempotency receipt commit together.
5. The API returns the STARTED occurrence identity/version and saved READY Task. The UI says
   “Saved Task · READY”; it must not display planning/agent work as started. The Automation
   remains PAUSED if it was paused. A same-key exact retry returns the recorded response;
   changed revision, expected version, inputs, or trigger conflicts.

**Failure/UI/postcondition:** Missing ManualTrigger, DISABLED Automation, stale head/version,
unsupported Hub/foreign Runtime placement, absent/revoked/stale local TriggerHost binding,
stale Runtime incarnation, archived Routine/Coworker, invalid input, or unavailable
Resource rejects before commit. If the response is lost after commit, retrying the exact
same request returns the original Task/occurrence even if the local host or dependency
availability has since changed; changed content under that RequestId never creates another
run. No recurring cursor is created by Run now, no Automation status changes, and no Plan,
Step, Attempt, AgentSession, lease, Environment, CapabilityInvocation, Effect, or Evidence
is created.

**Tests to add:** PAUSED and ENABLED with ManualTrigger; DISABLED rejection; missing or
duplicate ManualTrigger; Hub/foreign Runtime placement; paused definition with no cursor;
existing cursor must pin same revision/local host; Runtime incarnation swap and binding
revocation race; binding version mismatch; exact Automation/Routine/Coworker revision
pinning; archived Routine/Coworker; valid/invalid/extra inputs; stale/cross-Workspace
Resource revision; missing/ineligible lead; concurrent same RequestId creates one
occurrence/Task; same-key retry returns original response after host/dependency changes;
changed inputs/revision/If-Match conflicts; PENDING v1→CLAIMED v2→STARTED v3 snapshots and
event entity revisions; claim_epoch 0→1→1; transaction fault at each insert/event/Task/link
boundary leaves no partial state; Task READY with no execution records; Automation remains
PAUSED and no cursor is implicitly activated.

**System/user acceptance:** Create a paused Automation with one ManualTrigger and a Routine
with TEXT and exact Resource inputs. Run it from the desktop, confirm the Automation stays
paused, a single occurrence appears as STARTED, and its Task is READY with exact pinned
inputs and no Plan/Attempt. Retry the identical RequestId after simulating a lost response;
confirm the same Task and occurrence are returned. Change one input under that RequestId
and confirm conflict with no second Task. Record local OS/runtime version and sanitized
evidence; no test/build/validator or provider qualification is implied until the owner runs
the verification pass.

## F122 — Open a Goal-linked Task in Work

**Actors/preconditions:** Authenticated Workspace owner on the desktop Goals page, selected
Workspace, and a loaded Goal revision containing one or more same-Workspace
`related_task_ids`. The Goal and Task detail readers are available through the local
Operator. Each linked Task is an existing Task; Goal links are provenance only.

1. Goal details render each linked Task as an explicit **Open Task** button. If the Task is
   present in the bounded related-Task catalog, the row shows its objective and current
   status; otherwise it shows the Task ID and still permits navigation. The selected
   Workspace and Goal revision remain the source of the link.
2. The owner activates a Task row. The UI sets its local route target to that Task ID and
   selected Workspace, then opens Work directly at the Task details. Work requests the Task
   by that identity and only displays a response that belongs to the selected Workspace and
   requested Task ID.
3. The Work detail surface presents the Task's current objective, state, plan/execution
   projection, and available outputs under the ordinary Task detail contract. Returning to
   Goals leaves the Goal revision and its links unchanged.

**Durable-state/authority invariants:** This is navigation and a read only. It creates no
Task, TaskSpecRevision, PlanRevision, Step, Attempt, AgentSession, lease, Effect, Evidence,
Grant, or Goal revision. It does not submit a planning, execution, or Goal mutation command.
Task execution authority remains governed by ordinary Task admission and Trust contracts;
opening a link does not grant it.

**Failure, races, and recovery:** If the Task detail read fails, the Work page shows its
existing unavailable/error state and retains the Task ID for owner retry; the Goal link is
unchanged. A missing or cross-Workspace response is not rendered as the requested Task. If a
Goal link is concurrently removed after the Goal revision was loaded, the owner may still
open the Task from that loaded historical revision; the newer Goal revision remains
authoritative on reload. If the Task or Workspace changes while navigation is in flight,
the detail reader validates the current identity and the owner can return to Goals and retry
after reloading. No failure path starts or revises the Task.

**Tests to add:** linked Task button opens its exact ID in Work; objective/status display
when present in the related-Task catalog; ID fallback when absent from the current catalog
page; selected Workspace is carried into the route; Task response ID/Workspace mismatch is
rejected; detail read failure leaves Goal and Task unchanged; navigation from an archived or
completed Goal remains read-only; no mutation/admission command, Task/Goal event, Attempt,
AgentSession, lease, Effect, or Evidence is produced; returning to Goals leaves its links
unchanged.

**System/user acceptance:** Create a Goal linked to an existing READY Task, open Goal details,
activate **Open Task**, and confirm Work displays that Task without changing its status,
revision, or Goal link. Repeat with a completed Task. Record local OS/Runtime version and
sanitized evidence. This is a UI navigation flow, not evidence that Task execution is
available.

## F123 — Resolve an ambiguous Artifact text publish with the original request

**Actors/preconditions:** Authenticated Workspace owner in the desktop Artifact Workbench,
opening a non-archived Artifact with a managed, editable `text/plain` current version within
the 1 MiB edit limit. The owner has an open draft based on a freshly loaded edit head, and
the local Operator's append endpoint enforces Artifact version checks and idempotent request
receipts.

1. The owner selects **Publish new version**. The Workbench creates a request identity and
   captures it with the exact draft text, expected content version, expected Resource
   version, expected parent Resource revision, and expected Artifact aggregate version in a
   pending-save record. It calls the append API with those unchanged values.
2. On a committed response, the Workbench shows the returned immutable ArtifactVersion as
   published and reloads Artifact metadata. If the receipt is an idempotent replay of an
   earlier commit and another client has since advanced the Artifact, the Workbench reads
   the latest head before presenting current Artifact state; the replayed version is not
   mislabeled as the current head.
3. If the result is ambiguous, the Workbench preserves the pending-save record and exact
   draft. While it remains open, the editor cannot change text and ordinary **Discard draft**
   is disabled. The owner can choose **Retry unchanged**; this resends the same request ID,
   payload, and expected versions, allowing the server receipt to confirm the original
   commit or the version check to reject an uncommitted stale draft. A new request identity
   is not generated for this retry.
4. A definitive version conflict keeps the draft for review. The owner checks the latest
   version, compares the fresh head, and explicitly rebases the draft before a subsequent
   publication. Rebasing clears the old pending request and creates a new expected-version
   basis; it is not an automatic overwrite.

**Durable-state/authority invariants:** A successful publish appends one immutable
ArtifactVersion under the existing Artifact authorization and expected-version contract.
The client pending-save record is transient UI recovery state; server-side request receipt,
Artifact version, and domain event are durable truth. An ambiguous response is not proof of
success or failure. The retry must retain the original request identity and exact payload.
No path overwrites or rewinds history, changes the Artifact Library status, or creates a
Task, Attempt, Grant, AgentSession, or unrelated Effect.

**Failure, races, and recovery:** A transport/server failure with unconfirmed outcome keeps
the same pending save and presents **Retry unchanged**; editing and ordinary discard stay
locked until the request resolves. If another writer advanced the Artifact before an
uncommitted request is admitted, the expected-version check returns a conflict; the owner
must inspect and explicitly rebase. If the original request committed before its response
was lost, a same-key retry returns its original receipt rather than appending a second copy.
While a pending request exists, the Workbench's Close action is disabled; a changed
unpublished draft with no pending request requires explicit discard confirmation before
closing. The pending request ID is held only in mounted UI state, so an application restart
or other external unmount can lose that retry identity. On reopening after such an
interruption, the owner must inspect Artifact history/current head before composing another
edit. Authorization or missing-Artifact errors are shown without claiming publication;
after access is restored the owner refreshes the Artifact and resolves the visible state
before attempting a new edit.

**Tests to add:** request ID and exact payload are created once per pending save; ambiguous
failure preserves text and expected versions; textarea and ordinary discard are disabled
while pending; **Retry unchanged** reuses the identical request ID/payload/expected versions;
lost response after commit replays the same ArtifactVersion without duplication; retry before
commit with unchanged head appends once; stale expected version returns conflict without
overwriting; conflict review preserves draft and requires explicit latest-head read/rebase;
rebase clears the old pending save and uses the new head on next publish; concurrent advance
after a replay receipt causes latest-head refresh; Close is disabled while pending; a changed
draft without a pending request requires explicit close confirmation; external unmount/restart
recovery requires history inspection; no false success is shown before a receipt.

**System/user acceptance:** Edit a managed text Artifact and simulate a lost append response
after commit. Confirm the editor locks the draft, retry reuses the original request identity,
one new ArtifactVersion exists, and the UI shows a confirmed receipt/current head. Repeat with
a competing edit causing a version conflict; confirm the draft remains intact until the owner
reviews and explicitly rebases. Record local OS/Runtime version and sanitized evidence. This
flow does not establish production safety until the owning API, receipt, and concurrency
behavior are verified against the real local Runtime.

## F124 — Retry an unavailable Task detail read

**Actors/preconditions:** Authenticated Workspace owner opens an existing Task from Work or
a Goal link. The local Operator read may fail transiently or return an identity that does
not match the selected Workspace and Task.

1. The desktop requests `get_task` with the selected `workspace_id` and `task_id`.
2. Before rendering, it checks both identity fields in the response. A mismatch is treated
   as an unavailable read and is not displayed as the selected Task.
3. On failure, the owner sees the read error and **Retry Task details**. If Runtime is
   offline, retry is disabled until readiness returns.
4. The owner retries. The desktop repeats the same read for the same Workspace/Task; a
   valid response replaces the unavailable state. A second failure leaves the retry action
   available.

**Durable-state/authority invariants:** This flow performs no mutation. It creates no
Conversation, Task revision, Plan, Step, Attempt, lease, Effect, Evidence, AgentSession, or
Grant. A Goal link is not changed. Task authority remains governed by the normal admission
and execution contracts.

**Failure/recovery:** Network/Operator failures and identity mismatches are shown without
rendering an unrelated Task. Retry preserves the selected Workspace/Task identity. Switching
Workspace or Task mounts the corresponding detail view, so late results from another
identity are discarded. No automatic retry loop or fake progress is shown.

**Tests to add:** exact identity is sent; mismatched Workspace or Task response is rejected;
transient failure exposes retry; offline disables retry; successful retry loads only the
requested Task; late response after identity switch is not rendered; retry invokes only the
read command and creates no domain records/events.

**System/user acceptance:** With a local Task, inject a transient detail-read failure, use
**Retry Task details**, and confirm the same Task loads. Repeat with a mismatched response
fixture and confirm it is rejected, then return a valid response. Capture local OS/Runtime
versions and sanitized evidence. This proves UI read recovery only, not Task execution.

## F125 — Recover an ambiguous ManualTrigger request after UI navigation

**Actors/preconditions:** Authenticated Workspace owner in the desktop Operator, a saved
Automation revision with exactly one stable ManualTrigger ID, a pinned Routine revision,
and an active local Runtime/Workspace TriggerHost binding. The owner submits Run once and
the response is lost or otherwise leaves the outcome unconfirmed.

1. Before sending the mutation, the desktop stores a bounded volatile request envelope in
   process memory keyed by `(workspace_id, automation_id, automation_revision, trigger_id)`.
   It contains the original RequestId, Automation and Routine snapshots, and exact validated
   input object (text values and ResourceRefs only). No Resource bytes or secret-store
   material are copied; user-entered text remains only in volatile process memory and is
   not added to application logs or domain events by this recovery path.
2. If the owner navigates away and later returns to Automations in the same Workspace and
   process, the page restores a retry card from the registry. The owner can retry the exact
   original request; the daemon's idempotency receipt determines whether the original Task
   was already committed. The UI does not create a new RequestId automatically.
3. While any unresolved request exists for that Automation, another Run is blocked,
   including after selecting a newer Automation revision. Editing the visible input form
   cannot alter the stored request. Other Workspace entries are neither shown nor
   submitted by the current Workspace page.
4. A confirmed Task/occurrence receipt removes that registry entry. The owner may instead
   explicitly discard the retry record; the UI warns that a second Task may result if the
   first request committed. The registry has a fixed maximum size and rejects a new
   unresolved run when full; it never evicts an unknown-outcome request.

**Durable-state/authority invariants:** The registry is an in-process recovery optimization,
not Task truth or a durable receipt. The daemon's atomic occurrence/Task/idempotency
transaction remains authoritative. An exact retry reuses the original Workspace, version,
revision, trigger, inputs, and RequestId. Registry keys prevent cross-Workspace reuse. No
Task, trigger cursor, schedule state, approval, grant, or Effect is created by route
navigation or by restoring the retry card.

**Failure, races, and recovery:** Route unmount preserves the entry because the module-level
registry outlives the page component; a late response from the original call may still
settle it. Concurrent exact retries use the same RequestId and rely on server idempotency.
If the desktop process exits, volatile recovery is lost; the owner must inspect Work and
Automation occurrence history before deciding whether to run again. This flow does not
claim restart-safe recovery. Registry capacity failure is fail-closed. Explicit discard is
the only user action that abandons the saved retry identity before a confirmed receipt.

**Tests to add:** Route unmount/remount in the same process preserves RequestId and exact
inputs; TEXT values and ResourceRefs survive unchanged; Resource bytes and secret-store
material are absent; another
Workspace cannot read or submit the entry; changed inputs/current revision cannot replace
an unresolved request; same-key retry resolves the original receipt without a duplicate
Task; confirmed receipt removes the entry; explicit discard removes it and warns about
possible duplicate creation; capacity exhaustion retains all existing entries and rejects
new registration; process restart does not falsely claim recovery; late result after
unmount clears the entry without rendering state into a different Workspace.

**System/user acceptance:** Start a ManualTrigger run and simulate a lost response, navigate
to Work and back without closing the desktop process, and verify the original retry card is
available with the same Task inputs. Retry it and confirm one Task/occurrence receipt. Repeat
with a Workspace switch and verify the entry is not exposed there. Record local OS/Runtime
versions and sanitized evidence. This validates UI route recovery only; it does not prove
process-restart or production-safe Automation execution.

## F126 — Recover an ambiguous Goal mutation after UI navigation

**Actors/preconditions:** Authenticated Workspace owner in the desktop Operator, with a
Goal create, immutable revision, or status command. The owner submits the command and its
response is lost, invalid, or otherwise unconfirmed.

1. Before sending the mutation, GoalsPage stores a bounded volatile envelope in the
   desktop process, keyed by Workspace and logical Goal target. The envelope contains the
   exact operation, expected Goal version when applicable, full immutable revision/status
   payload, and original RequestId. Goal text and references remain only in volatile process
   memory for this recovery path; no browser storage, Resource bytes, or secret material is
   copied.
2. A failed or malformed response leaves the envelope unresolved. Returning to Goals in
   the same Workspace and process displays a retry card. Retrying sends the same exact
   payload and RequestId; an edited draft, newer version, or different status cannot replace
   an unresolved operation for the same target. The server idempotency receipt and owner
   authorization remain authoritative.
3. A response clears the envelope only after its Workspace and Goal identity and the
   returned status or revision match the saved operation. A mismatched response remains
   unresolved. A late response after explicit discard cannot clear a newer envelope.
4. The owner may explicitly discard the saved retry identity only after a warning explains
   that the original mutation may already have committed and that issuing another create
   or revision could duplicate work. Discard forgets local recovery data; it does not undo
   or cancel a committed Goal mutation. A fixed registry cap rejects a new pending mutation
   instead of evicting unknown outcomes. Entries from another Workspace are never rendered
   or submitted by the current Workspace page.

**Durable-state/authority invariants:** The registry is an in-process retry aid, not Goal
truth, an idempotency receipt, or an execution mechanism. GoalService's version checks,
owner authorization, immutable revision, and atomic idempotency receipt remain authoritative.
Goals remain passive; this flow never creates a Task, starts an AgentSession, or changes
linked Tasks/Routines/Artifacts. Retry identity does not survive desktop process exit.

**Failure, races, and recovery:** Route unmount preserves the request envelope because it
outlives the component. A Workspace switch filters the pending list by Workspace identity.
Concurrent exact retries use the same RequestId and depend on daemon idempotency. Revision
conflicts remain unresolved until the owner retries the exact request or explicitly
discards it after reviewing current Goal state. If the process exits, the volatile envelope
is lost; the owner must inspect the Goal list/current state before deciding whether to
submit a new mutation. This flow does not claim restart-safe recovery.

**Tests to add:** Create/revise/status response loss followed by same-process route
unmount/remount preserves RequestId, expected version, target, and exact payload; altered
draft/status cannot obtain a new RequestId while the target has an unresolved mutation;
same-key retry clears only on a matching same-Workspace receipt; malformed or mismatched
response preserves the envelope; Workspace switching never displays/submits another
Workspace's entry; late response after discard cannot remove a newer envelope; explicit
discard warns about a potentially committed create/revision; capacity exhaustion retains
all entries and rejects the next registration; process restart does not claim recovery or
automatically resubmit; Goal mutations never create Tasks or execute work.

**System/user acceptance:** Owner creates or revises a Goal, simulates a lost response,
navigates away and back without restarting LiteCowork, retries, and confirms the original
Goal/revision receipt without a duplicate. Repeat for a status command and a Workspace
switch. Record local OS/Runtime versions and sanitized evidence. This validates desktop
route recovery only; it does not prove Goal domain durability, process-restart recovery,
Goal execution, or production readiness.

## F127 — Recover an ambiguous Suggestion or preference mutation after navigation

**Actors/preconditions:** Authenticated Workspace owner on the local desktop; a Suggestion
owner action (dismiss, snooze, unsnooze) or kind-preference mutation has been submitted and
its result is not confirmed.

1. Before dispatch, the page stores the bounded exact request payload, expected version,
   Workspace ID, and RequestId in a process-local registry keyed by stable Workspace ID.
   Suggestion and preference changes use separate target keys. No browser storage,
   Resource bytes, or authentication material is written.
2. Navigating away and returning to the same Workspace restores the retry card, including
   after the per-Workspace API client is recreated by switching away and back. A different
   Workspace cannot render or submit the saved command. The retry sends the original
   request ID and exact payload through the same authenticated local Operator route.
3. Only a decoded same-Workspace Suggestion/Preference response matching the requested
   operation can clear that retry entry. A malformed or mismatched response keeps it.
   Mutations have a fixed per-Workspace and process-wide count bound; a full registry
   rejects new unresolved mutations without evicting existing entries.
4. The owner may explicitly discard an unresolved entry after reading a warning that the
   original change may already have committed. Discard removes only local retry metadata;
   it neither reverses server state nor cancels the original request.

**Durable-state/authority invariants:** The registry is a UI recovery aid, not durable
Suggestion/preference state or an idempotency receipt. The server transaction and owner
authorization remain authoritative. Recovery is process-local; restarting LiteCowork loses
these pending keys. This flow does not accept a Suggestion into a Task, start work, or add
authority.

**Failure/races:** Route unmount does not discard an unresolved entry. Workspace A→B→A
retains A's entry despite API client recreation. A timed-out/malformed response remains
retryable. A late response cannot authorize an action in another Workspace. Capacity failure
is closed. Explicit discard may allow a later duplicate logical change; the owner must
refresh and inspect current state first. Suggestion-to-Task acceptance recovery is covered
by F108 and F129, using the separate typed atomic receipt.

**Tests to add:** Lose responses after commit for each owner action and preference update;
remount and A→B→A recovery preserves RequestId/payload; submit to B is rejected; mismatched
Workspace/entity/status/version receipts retain the entry; matching replay clears it;
capacity retains all old entries; discard warns and removes only the local retry; app restart
does not claim or perform recovery; no command bypasses owner authorization.

**System/user acceptance:** Simulate a lost response for snooze and a preference update,
navigate away/back and across Workspaces without closing LiteCowork, retry and verify the
server's exact resulting state. Repeat for dismiss/unsnooze. Record OS/Runtime versions and
sanitized evidence. This validates same-process desktop route recovery only, not restart
recovery or Suggestion Task acceptance.

## F128 — Run an active Routine into an ordinary READY Task

**Actors/preconditions:** Authenticated Workspace owner; active Routine and immutable
current RoutineRevision; local Operator/Runtime available; selected lead is eligible; all
required Routine input bindings are supported; selected Resource inputs are exact
same-Workspace immutable revision references.

1. The Routine detail screen renders only supported TEXT and ResourceRef fields from the
   pinned RoutineRevision schema. The owner enters bounded text values and selects
   Resource revisions from the loaded Workspace catalog. No Resource bytes are copied into
   the form or command envelope.
2. Before dispatch, the page validates required values, enum values, character/UTF-8 byte
   limits, exact ResourceRef shape, and Workspace identity. Unsupported schemas or stale
   selections fail before a mutation is sent.
3. The desktop registers the exact Routine ID, revision, input object, and RequestId in a
   bounded volatile Workspace-scoped registry, then sends `POST /v1/routines/{id}/run`
   through the authenticated local Operator bridge. The server atomically revalidates
   Routine status/head, owner, eligible lead, and Resource revision availability while
   creating an ordinary Task, TaskSpecRevision, and idempotency receipt.
4. A receipt matching the selected Workspace/Routine/revision and READY Task is displayed
   and linked to Work. It creates no Plan, Step, AgentSession, Attempt, lease, Environment,
   CapabilityInvocation, Effect, Evidence, or VerificationRun. Agent planning/execution
   must be a separate supported path.
5. If no matching response arrives, the exact request remains in process memory across
   route unmount/remount. A retry may overlap an earlier hung call after remount but reuses
   the same Idempotency-Key and payload. Mismatched/malformed receipts remain unresolved.
   The owner may explicitly discard after a duplicate-risk warning. Process restart loses
   the retry envelope.

**Durable-state/authority invariants:** The Routine remains passive and unchanged. The
ordinary Task is durable execution truth and pins the exact Routine definition used for
materialization. The route does not create an AutomationOccurrence, schedule cursor, Grant,
Effect, or agent session. This is Task materialization only, not Routine execution.

**Failure/races:** Missing/invalid TEXT; unsupported enum value; overlong UTF-8; unsupported
input schema; Resource revision from another Workspace; Resource head changes after
selection; Routine archived/head changed; Workspace archived; lead disabled/ineligible;
duplicate RequestId with changed payload; response loss after commit; mismatched receipt;
overlapping exact retry; registry capacity; explicit discard; process restart. Any server
admission failure creates no partial Task. The UI never labels READY as Working.

**Tests to add:** Every supported/unsupported input schema; required/optional TEXT and
ResourceRef; exact Resource pin after catalog head change; cross-Workspace/stale references;
Routine/lead race; Task and receipt exact shape; no execution records; route remount and
Workspace switch; concurrent same-key replay returns one Task; payload mismatch conflicts;
capacity fails closed; malformed receipt retains identity; restart does not claim recovery;
keyboard and screen-reader form/error/retry behavior.

**System/user acceptance:** Create an active Routine with required and optional text and
Resource inputs, run it once, open its Task, and verify it stays READY with the exact pinned
Routine/Resource revision and no plan or agent activity. Simulate response loss, navigate
away/back and retry; verify one Task. Repeat with a Workspace switch, later Resource head,
invalid input, and app restart. Record local OS/Runtime versions and sanitized evidence.
This does not qualify Task execution or production-safe switching.

## F129 — Recover Suggestion Task acceptance after desktop navigation

**Actors/preconditions:** Authenticated Workspace owner; desktop Suggestion proposes an
unexpired actionable TASK; selected Workspace and local Operator are available; the
acceptance request has been sent but no valid bounded receipt has been confirmed.

1. Before the first send, the desktop stores the exact Workspace ID, Suggestion ID,
   expected Suggestion version, and RequestId in a bounded process-local registry. The
   registry is keyed by stable Workspace and Suggestion/version identity, not API client
   object identity, so route remount and Workspace A→B→A preserve the original command.
2. The authenticated route accepts the Suggestion through F108. Its atomic SQLite
   transaction creates the ordinary READY Task, stores the original Task creation
   response receipt, resolves the Suggestion with the linked Task ID, and appends both
   domain events. It does not plan or execute the Task.
3. The initial response is `201 CREATED`; an exact retry is `200 REPLAYED`. Both return
   `SuggestionTaskAcceptanceReceipt`. The desktop verifies top-level and Task Workspace
   IDs, Suggestion ID/status/version, Task status/version, matching linked Task IDs, and
   the HTTP status/disposition pairing before clearing the pending entry.
4. A valid receipt opens its READY Task. If the response is lost or malformed, the owner
   may retry the same request or explicitly discard the local retry after a warning that
   a Task may already exist. Discard never cancels or deletes server state.

**Recovery/authority invariants:** Registry state is volatile and process-local; it is
not the idempotency record or source of Task truth. SQLite and the authenticated Operator
remain authoritative. A retry uses the same RequestId and expected version. A different
Workspace cannot display or submit the entry. A delayed response after discard cannot
remove a replacement entry. Neither retry nor receipt grants execution authority.

**Failure/race cases:** Response loss after commit; response loss before commit; route
unmount/remount; Workspace switch away/back; malformed or wrong-Workspace receipt; wrong
Suggestion/Task link; inconsistent HTTP status/disposition; same-key replay; different
key conflict; explicit discard during an in-flight request; registry capacity exhausted;
process restart loses recovery metadata; owner authorization revoked before retry.

**Tests to add:** Assert exact request identity survives route changes and API recreation;
response loss after commit returns one original READY Task and one Suggestion resolution;
all mismatched receipt fields preserve the pending record; late response cannot remove a
new envelope; cap rejects without eviction; discard warns and only removes local state;
Workspace A entries never render or dispatch in B; process restart does not claim recovery.

**System/user acceptance:** On desktop, create a Task from an actionable Suggestion while
simulating a lost response after server commit. Navigate away and back, retry, and confirm
the exact linked Task opens once, remains READY, and appears only once in Work. Repeat with
Workspace A→B→A. Record OS/Runtime versions and sanitized evidence. This qualifies only
same-process desktop recovery, not restart-safe recovery or Task execution.

## F130 — Browse persisted Tasks needing attention

**Actors/preconditions:** Workspace owner, desktop Operator, authenticated `list_tasks`
route; selected Workspace is active for the page.

1. Query each supported exact Task status independently: `WAITING_USER`, `NEEDS_USER`,
   and `BLOCKED`. The page does not infer these states from provider output, notifications,
   heartbeats, or cached UI labels.
2. Validate each response item still has the requested status, merge by Task ID, and order
   by persisted `updated_at`. Keep one opaque cursor per status and fetch the next page only
   for statuses with a cursor. Refresh replaces the visible projection only after every
   first-page query succeeds.
3. If a same-Workspace refresh fails, retain the prior rows with an unavailable/stale
   notice. On Workspace change, clear the old Workspace view and ignore late responses.
   An unavailable first load is never rendered as an empty inbox.
4. Selecting a Task routes to Work/Task detail, which reloads current Task state. The
   attention row is read-only and cannot resolve a blocker or change Task state.

**Failure/UI/postcondition:** Explain that this is a Task-attention view. The full
Approval/UserRequest/actionable-blocker aggregation and its owning-resource resolution
routes remain unimplemented; do not label this page an Approval inbox or “all caught up.”
Stale Task state is corrected by the detail reread, not by assuming the old list status is
still current. Duplicate Tasks from pagination appear once.

## F131 — Compose an optional RichPresentation

**Actors/preconditions:** Owner, active Conversation turn, committed Agent response,
ConversationService, bounded rich compiler, authorized source resolvers, BlobStore.

1. The turn pins `PresentationPreference` at admission and uses the same value on retries.
   `SIMPLE` prevents model-composed decoration; `AUTO`/`RICH` may permit an enhancement.
2. The Agent streams ordinary coalesced semantic text. Optional host guidance/HostSkill
   delivery is negotiated and never required for Agent eligibility. The Agent may submit
   one bounded `litecowork.presentation.propose` intent; that intent is turn/session
   bound, ephemeral, and cannot set host state.
3. ConversationService commits the complete semantic ConversationMessage and settles the
   turn without waiting for rendering. The Operator displays the committed message.
4. RichPresentationCompiler compiles only against that exact message digest and authorized
   Artifact/Resource/invocation refs. It derives trusted host blocks from current Core
   projections, validates closed block schemas/limits, and creates canonical JCS bytes.
5. BlobStore verifies/stores the immutable document. ConversationService rechecks the
   same-Workspace AGENT message, exact digest, source refs, schema/size bound, and
   one-per-message constraint, then commits `RichPresentation` and
   `rich.presentation.published.v1` separately.
6. The Operator fetches the exact presentation document, verifies identity, digest,
   schema/size and source authorization, then upgrades that same message in place.

**Failure/UI/postcondition:** Semantic content remains complete and readable at every
point. An invalid intent, compiler timeout, unsupported renderer, or missing/corrupt blob
does not change the ConversationMessage or turn outcome, Task, Effect, Artifact, or
Verification. Presentation cannot be the only place a fact, citation, warning, action, or
deliverable exists. No Task planner or worker Attempt receives response-design guidance by
default.

## F132 — Recover RichPresentation failure

**Actors/preconditions:** Committed semantic ConversationMessage; optional presentation is
being compiled, replicated, fetched, or rendered.

1. On timeout or compile/schema failure, omit publication and retain the semantic response.
2. If metadata is published but the blob is missing, mark availability UNAVAILABLE and
   render the semantic response; retry the exact immutable fetch only when blob transfer
   can make it available.
3. If digest/semantic binding fails, reject the rich document, mark INTEGRITY_FAILED, and
   record a sanitized diagnostic; do not render any part of it.
4. If one block renderer throws, isolate that block with a safe fallback while preserving
   unrelated blocks and semantic content. Unknown schema versions fall back to the message.
5. Reconnect replaces ephemeral draft state from the current snapshot. No old typing,
   draft, or content entrance animation is replayed.

**Recovery invariants:** RichPresentation failure is not a turn failure. The response is
searchable/exportable as semantic text. Missing presentation bytes do not block restore.
No client retry bypasses Workspace authorization or digest validation.

## F133 — Present exact Artifacts and ZIP bundle

**Actors/preconditions:** A Task has committed ArtifactVersions; an authorized owner or
Conversation response references those exact versions; the deterministic bundle builder
is available.

1. The compiler validates each `ArtifactVersionRef` against the selected Workspace and
   source Task, then binds the exact version into a DeliverableGroup or paged collection.
   It never turns an Artifact ID into a mutable “latest” download URL.
2. Task-generated output exists as ArtifactVersion state before presentation. A
   Conversation AgentSession cannot publish the file.
3. An explicit owner request to bundle existing outputs sends selected exact versions and
   normalized relative paths to BundleBuilder. It rejects duplicates, traversal, unsafe
   entries, and unavailable source versions before publication.
4. The deterministic ZIP is published as an ordinary immutable `BUNDLE_ARCHIVE` Artifact
   with exact input pins and a canonical member manifest in provenance.
5. Download resolves the exact ArtifactVersion, rechecks owner/scope, and verifies bytes
   and digest before reporting success. Later source versions do not alter the ZIP.

**Failure/UI/postcondition:** Invalid/stale/missing inputs create no ZIP Artifact. A file
row is not shown as downloadable until an actual ArtifactVersion exists. Cancelled/failed
downloads do not change Artifact state or display success.

## F134 — Stream and fence RichPresentation draft frames

**Actors/preconditions:** Active Conversation turn/current retry; authenticated Operator
stream; optional rich renderer supports the draft version.

1. Frames bind Workspace, Conversation, turn, retry ordinal, AgentSession, draft ID, and
   monotonic sequence. Only the current active tuple is accepted.
2. Server coalesces text and enforces finite frame, queue, byte, open-block, block-count,
   and nesting bounds. A slow renderer does not block AgentSession reads or turn settlement.
3. Closing a block freezes its order/parent/body. Corrections append a replacement block.
4. Retry, cancellation, turn settlement, resync, and AgentSession replacement fence old
   frames; delayed prior-retry frames are discarded.
5. If replay is supported, a bounded draft snapshot repairs gaps. Otherwise the Operator
   drops the partial rich draft and continues semantic `turn.delta` handling.

**Failure/UI/postcondition:** Draft frames are not DomainEvents, durable messages,
replication state, or turn outcomes. No rich-only content is authoritative.

## F135 — Render a portable response on a constrained channel

**Actors/preconditions:** A committed semantic ConversationMessage with optional validated
RichPresentation and an authorized channel renderer.

1. PortableResponseRenderer starts from the semantic Message. It adds bounded table/card,
   chart/diagram text summaries, media attachments, Artifact names, and supported exact
   download/deep links only when the channel supports them.
2. Unsupported interactive blocks (MCP App, Workbench editor, local checklist state) are
   replaced by concise text and an Operator deep link when available.
3. Approval/UserRequest actions are handled only under their owning channel assurance and
   domain routes; rich formatting does not upgrade assurance.

**Failure/UI/postcondition:** Delivery remains useful if RichPresentation is missing. It
never exposes arbitrary URLs, local paths, private Artifact data, or stronger authority
than the authenticated channel supports.

## F136 — Prepare and revalidate an isolated local Task Environment

**Actors/preconditions:** Workspace owner, TaskRuntime, EnvironmentManager, Resource
reader, and an OS-qualified local EnvironmentProvider; the Task has an accepted Plan and a
selected Step, but this preparation flow does not admit an Attempt.

1. Resolve only the Task's exact Workspace-scoped Resource revision/digest pins. Reject
   missing, stale, foreign, duplicate, unsupported, symlinked, or path-traversing inputs
   before exposing bytes.
2. Stage verified immutable inputs under a provider-controlled read-only root and create a
   separate bounded writable output root. Do not expose a home directory, ambient
   workspace path, credential store, or unpinned Resource head.
3. Probe the concrete OS/provider implementation against the normalized filesystem,
   process-tree, network, and resource-limit policy. Produce a private
   `IsolationAttestation` tied to the exact Environment/Task, Runtime incarnation, provider
   version, OS build, and policy digest. `UNKNOWN` or unsupported controls fail closed.
4. Persist the ordinary Environment lifecycle transition and incarnation-local provider
   binding only after current owner, Task, budget, and Runtime admission checks commit. A
   lost provider response is reconciled by idempotent provider request identity; never
   retry an ambiguous allocation blindly.
5. Return a preparation result only. This flow creates no AgentSession, Attempt,
   ExecutionLease, Grant, CapabilityActivation, or Effect and does not start an external
   Agent. The ordinary admission path later revalidates the attestation and independently
   commits fresh Trust, budget, Environment-use, and lease state before dispatch.
6. After a process has run, require a current-incarnation positive
   `QuiescenceObservation` before reusing a writable root, releasing its writer fence, or
   cleaning the Environment. Timeout, lost identity, or unknown descendants leave it
   fenced and blocked pending recovery.
7. After daemon restart, resolve the private provider binding and re-probe the same
   Environment identity. A changed incarnation, path identity, policy digest, source
   digest, OS/provider version, or containment result invalidates the attestation and
   requires re-preparation; it never resumes an external Agent automatically.

**Failure/UI/postcondition:** Show “Environment could not be prepared” with a safe
reason and recovery action. A prepared Environment is not presented as a running Task.
No provider start, AgentSession, Attempt, lease, or Agent-side file access occurs from
preparation alone. If cleanup cannot prove process-tree quiescence, retain the Environment
and expose an actionable blocker instead of claiming deletion.
