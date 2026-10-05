# End-to-End Flows

Each flow's durable writes occur through the owning service. Domain events commit in the
same transaction as aggregate updates; UI is a projection and never drives truth
directly. The listed sequence is normative unless a linked owner contract is stricter.

## F00 — Workspace creation, replication policy, and archive

Actors: User, Operator UI, WorkspaceService, TrustService, RuntimeMesh.

1. The user creates a Workspace. WorkspaceService commits `LOCAL_ONLY` and an empty selected-root set when no other allowed initial policy is supplied, then emits `workspace.created`.
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
request or the Workspace default. If none is available, return `AGENT_UNAVAILABLE`, keep
the composer draft, and create no Task or ConversationTurn; first-use setup runs before
the user resubmits.

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
3. Broker searches LitePSM normalized catalog.
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

1. lead calls `litecowork.agents.delegate` with DelegateRequest.
2. Core validates depth, budget, capabilities, runtime/environment and isolation.
3. child Step/Attempt created.
4. bounded TaskPacket delivered to child AgentSession.
5. child runs independently.
6. child returns ResultEnvelope + refs.
7. parent is notified.
8. lead integrates/rejects/retries.

No parent full transcript is copied by default.

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

1. LitePSM reports a newer package through its separately defined contract.
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

Actors: User, SkillProposalService, redactor/validator, TrustService, LitePSM adapter.

1. After successful work, the user or Agent explicitly requests a reusable Skill draft.
2. SkillProposalService creates a `SKILL_DRAFT` Artifact from selected method/provenance,
   strips Task-specific data and secrets, and records redaction/validation results.
3. User reviews exact content and approves or rejects the proposal. Approval is not
   inferred from a successful Task.
4. On approval, LiteCowork hands the reviewed content/reference to LitePSM publication
   once its contract is available; LitePSM owns package versioning/distribution.

Events: `skill.proposal.created.v1`, `skill.proposal.status.changed.v1`, Artifact and
ApprovalUse events where applicable.

Failure behavior: a redaction failure blocks publication; LiteCowork never stores a hidden
autonomous memory rewrite or claims publication before LitePSM acknowledgement.

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
| F34 | SkillProposal review/redaction/publication state | Show exact draft; publication is not claimed before LitePSM confirms |
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


## F37 — Runtime boot and incarnation recovery

**Actors/preconditions:** OS service manager or authenticated Operator; installed Runtime,
exclusive installation lock, accessible StateStore and authorized startup policy.

1. Acquire the lock and create a RuntimeIncarnation; open storage and apply supported
   migrations before admitting commands that mutate Task execution.
2. Validate journal/checkpoints; recover claims and reconcile stale leases, Effects, and
   owned process handles. Restore scheduler cursors and mark watcher gaps stale.
3. Reconnect the Hub, refresh offers, and resume authorized resource observation.
4. Publish readiness only after mandatory recovery gates pass. Installed workers remain
   cold; recovery never starts every agent/provider/application.

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
2. Put the source assignment into `DRAINING`: stop new receipt claims and outbound sends,
   allow in-flight reads to settle, and reconcile outbound sends that may be ambiguous.
3. Settle/flush source receipts to the Workspace Hub before releasing the lease, or wait
   for authoritative expiry plus clock-skew margin if the source is unavailable. A heartbeat
   loss alone never grants the target ownership. An unreplicated receipt prevents a
   continuity-safe move unless the owner explicitly accepts a possible gap.
4. RuntimeMesh atomically increments `host_epoch`, commits the target Runtime assignment,
   issues a new bounded host lease over authenticated Mesh control, and appends
   `channel.host.assignment.changed.v1`. The target starts/attaches only its channel
   adapter when this assignment is active and its provider/secret checks pass.
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
