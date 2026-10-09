# Task Runtime LLD

## Responsibilities

Task Runtime owns durable intent, plan revisions, Steps, Attempts, completion evaluation,
cancellation/steering coordination, placement admission, and portable Task checkpoints.

It does not own reasoning strategy, model routing, domain tools or external-agent private memory.

## Service contract

```text
interface TaskService {
  create(CreateTaskRequest) -> Task
  get(TaskId) -> TaskView
  list(TaskQuery) -> Page<TaskSummary>

  revise_spec(ReviseTaskSpecRequest) -> TaskSpecRevision
  request_initial_plan(RequestPlanRequest) -> PlanningAssignment
  submit_plan(SubmitPlanRequest) -> PlanAcceptance
  change_lead_agent(ChangeLeadAgentRequest) -> Task
  create_attempt(CreateAttemptRequest) -> Attempt
  recover_step(RecoverStepRequest) -> Attempt

  steer(SteerTaskRequest) -> TaskSpecRevision | ConversationMessage
  request_pause(PauseTaskRequest) -> Task
  resume(ResumeTaskRequest) -> Task
  authorize_input_continuation(InvocationId, UserRequestId, expected_task_version) -> InvocationContinuationAuthorization | Denied
  request_cancel(CancelTaskRequest) -> CancelResult

  block(BlockTaskRequest) -> Task
  unblock(UnblockTaskRequest) -> Task

  checkpoint(CreateResumePacketRequest) -> ResourceRef
  propose_completion(CompletionProposal) -> VerificationRunId
  finalize(FinalizeTaskRequest) -> Task
}
```

Plan acceptance result:

```text
PlanAcceptance {
  plan_revision: PlanRevision
  materialized_steps: Step[]
  task_version: u64
}
```

The authenticated submit command is:

```text
SubmitPlanRequest {
  task_spec_revision: u64
  steps: PlannedStep[]
  reason_for_revision: string?
}
```

`task_id` comes from the route, `expected_task_version` comes from `If-Match`, and
producer provenance comes from the authenticated request context; none is accepted as
authority from the JSON body. The producer context is bound to either the current active
`TASK_PLANNING` AgentSession or the current lead Attempt's active AgentSession and valid
lease. A planning session must pin `task_spec_revision`; an execution producer must be
authorized to plan against that exact current TaskSpec revision. TaskService records the
authenticated producer's AgentSession and optional Attempt in PlanRevision. An owner
credential without a producer-scoped authority cannot submit a plan by naming a session
or Attempt. Planning tools receive the current Task version after AgentSession activation
(including the initial `READY -> RUNNING` transition); a version captured before
activation is stale and cannot be used as the submit precondition.

The route requires the current Task aggregate version in `If-Match`, while
`task_spec_revision` must equal the Task's current TaskSpec head. Both checks occur in the
acceptance transaction with producer/session/lease revalidation. A Task version mismatch
returns `409 CONFLICT` (or the typed `STALE_TASK_VERSION`); a spec mismatch returns
`409 STALE_SPEC_REVISION`. A proposal whose authenticated producer is no longer current,
active, or authorized is rejected with `403 FORBIDDEN` or `409 CONFLICT`, without
creating a PlanRevision or Steps. The Task must still be in a plan-accepting state
(`READY` or `RUNNING`); pause/cancel and acceptance serialize on the Task version, and a
winning pause/cancel fences a late proposal.

The command requires `Idempotency-Key`. Its receipt is scoped to the authenticated
producer, Workspace, Task route, and key, and stores the normalized request digest and
committed `PlanAcceptance`. Repeating the same key and digest returns the original
response and creates no additional PlanRevision, Steps, or events, even if the Task has
since advanced. Reusing a key with a different route, precondition, or normalized body
returns `409 CONFLICT`. Authentication and Workspace ownership are still checked before
replay, and the receipt must match the authenticated idempotency subject. A replay does
not re-run mutable producer-admission checks after the original command committed;
otherwise a planner that closed after successful acceptance could not safely recover a
lost response. A new key is subject to current preconditions and full producer
revalidation.

## TaskProgressProjection

`TaskProgressProjection` is a rebuildable read model returned by
`GET /v1/tasks/{id}/progress`. It contains:

```text
TaskProgressProjection {
  task_id: TaskId
  computed_at: Timestamp
  last_activity_at?: Timestamp
  last_activity_source?: TASK_EVENT | STEP_EVENT | ATTEMPT_EVENT | CAPABILITY_INVOCATION | PROVIDER_PROGRESS | ENVIRONMENT_OBSERVATION
  last_evidence_at?: Timestamp
  activity_summary?: string
  active_workstreams: TaskWorkstreamProjection[]
  blockers: Blocker[]
  newest_artifact?: ArtifactVersionRef
}

TaskWorkstreamProjection {
  step_id: StepId
  title: string
  step_status: StepStatus
  active_attempt_ids: AttemptId[]
  worker_labels: string[]
  last_activity_at?: Timestamp
}
```

The current desktop/local implementation reads the Task, its current-plan Steps, the
latest committed event for each of those aggregates and each Step's persisted
`current_attempt_id`, latest Evidence creation time, blockers, and current Artifact
versions from one bounded SQLite read transaction. `last_activity_at` is the newest
committed Task/Step/current-Attempt event timestamp. A terminal current Attempt can
therefore be the latest historical activity source; only nonterminal current Attempts
appear in `active_workstreams`. The read is bounded to 100 Steps and 200 Task Artifacts;
an over-limit source set returns a bounded error rather than silently truncating the
progress projection. `last_evidence_at` comes from the newest persisted Evidence row and
is not interchangeable with activity.

This implementation does not yet read CapabilityInvocation transitions or fresh
provider/Environment observations, and the progress projection does not imply their
absence from the underlying Task. Their source fields remain unavailable until those
producers are integrated. Heartbeats, token deltas, process liveness, and “still running”
status do not count as activity. `activity_summary` is an allowlisted short label for
recognized committed event types; an unrecognized newest event may yield a null summary
while retaining its event timestamp/source. Worker labels are looked up from the current
Workspace binding/profile when available and do not prove process/provider liveness.
Missing source rows produce null timestamps/summary. The projection contains no
percentage or ETA and never changes Task truth/version.

`PlanningAssignment` is an internal, transient dispatch envelope, not a persisted domain
entity or lifecycle. It contains the Task ID, expected Task version, pinned TaskSpec
revision, current lead AgentBinding, idempotency RequestId, and immutable planning-context
packet reference. PlanningCoordinator consumes it to admit one durable `TASK_PLANNING`
AgentSession. The AgentSession and Task aggregate are the durable records; at most one
planning session may be STARTING/ACTIVE/INTERRUPTING/CLOSING for a Task. A changed Task
version, spec revision, lead binding, or non-runnable Task status invalidates the envelope.
Replacing a planner creates a new envelope and AgentSession while retaining the same Task.

### TaskPlanningReadiness projection

The desktop's `GET /v1/tasks/{id}/planning-readiness` operation is a separate read-only
diagnostic, not a PlanningAssignment admission command. It requires the selected Workspace
and current Task version (`If-Match`), checks the persisted Task/lead/endpoint facts, and
may construct the bounded PlanningAssignment packet transiently to reuse the local
preflight. It does not reserve an AgentSession, acquire a lease, call an adapter/provider,
activate the Task, create a Plan/Step/Attempt/Environment, or write an event. Its public
projection contains only Task ID/version/spec revision/status, observation time, a
sanitized blocker enum list, and literal false values for dispatch/session/plan creation.
It never returns the packet, objective/model context, provider handle, or endpoint ID.
This local implementation always reports `dispatch_available=false`; removing all
preflight blockers is not evidence that the separate planning-admission contract is
qualified or enabled. A stale Task version returns a conflict and requires a fresh Task
read before another check.

## CreateTaskRequest

```text
{
  workspace_id
  conversation_id?
  source_message_refs[]
  objective
  constraints[]
  non_goals[]
  required_outputs[]
  acceptance_criteria[]
  approvals_required[]
  budget?
  deadline?
  coworker_id?: CoworkerId
  expected_coworker_version?: u64
  preferred_lead_agent_binding_id?
  placement_preference?
}
```

`coworker_id` is an explicit origin selection. The Operator may prefill it from the
Workspace's `primary_coworker_id`, but sends the selected ID so a concurrent primary
change cannot silently switch this Task to another Coworker. Omitting it creates work
without Coworker origin. When supplied, TaskService reads the current Coworker head and
pins its ID/revision to Task in the creation transaction; `expected_coworker_version`,
when supplied, rejects a stale editor with `CONFLICT`. The caller never supplies a
Coworker revision as authority. A paused Coworker is allowed only for an explicit
owner-submitted Task; proactive/scheduled admission requires ACTIVE. An archived Coworker
is rejected.

Lead selection uses the first configured value in this order: explicit
`preferred_lead_agent_binding_id`, the selected Coworker revision's
`default_lead_agent_binding_id`, then Workspace `default_agent_binding_id`. Once a value
is selected, it must belong to this Workspace, remain enabled and lead-eligible, and have
a currently eligible endpoint. An unavailable explicit/Coworker/Workspace selection
fails with `AGENT_UNAVAILABLE`; Core does not skip it and silently substitute a lower
precedence binding. If no binding is configured, creation returns `AGENT_UNAVAILABLE`,
creates no Task/turn, and preserves the unsent composer draft so setup can finish first.
Creation atomically stores Task + TaskSpecRevision(1) + `task.created` and, when present,
the exact Coworker origin revision. The selected lead AgentBinding is stored on Task and
referenced by the initial spec as `preferred_lead_agent_binding_id`; the placement
preference is pinned in that spec. A new Task enters planning without creating a Step,
Attempt, ExecutionLease, or Environment.

The initial TaskSpecRevision also pins the effective `LeadFailoverPolicy`: an explicit
Task request wins, otherwise the selected Coworker revision's default applies, otherwise
Core stores `DISABLED` with empty triggers/fallbacks and zero automatic changes. A TaskSpec
revision inherits the prior policy unless the owner explicitly supplies a replacement.
Failover candidates are ordered, same-Workspace, enabled, lead-eligible bindings and are
rechecked for Runtime, endpoint, auth, Trust, input availability, and budget at each use.
The policy can change the lead binding only; it does not copy grants, approvals, secrets,
native session state, or Environment control. Every lead change gets a fresh planning
AgentSession and bounded LeadHandoffPacket.
There is no persisted Task `DRAFT` state in v1; incomplete/unsent composer content remains
in the Operator until admission succeeds.
The operator appends the originating ConversationMessage in the same command boundary
when the Task came from a message. A standalone Task may omit a Conversation.

Every `input_refs[]` item must identify a unique same-Workspace Resource revision and have
exactly the three `PinnedResourceRef` members
`workspace_id`, `resource_id`, and `revision_id`; storage rejects non-objects, missing or
empty members, and unknown fields even though `TaskSpecRevisionRecord` retains the wire
values as JSON. Task creation and pre-planning TaskSpec revision admission also check the
current owner status of any ContextDocument in the same SQLite admission transaction; `REVOKED`,
`DELETION_PENDING`, and `DELETED` documents cannot be newly pinned as Task inputs. This
check does not reserve the content for the Task: the ResourceResolver repeats scope,
revision, availability, and ContextDocument status checks whenever content is actually
resolved, since an owner may revoke a document after Task admission. Existing immutable
Task provenance remains visible after revocation, but a later execution/context projection
must not expose the retained bytes.

### Manual Routine Task materialization

The local manual Routine route accepts only standalone creation: a non-null Conversation
origin is rejected until its message and Task can be admitted atomically. It resolves the
requested immutable RoutineRevision and materializes its bounded inputs, but SQLite is the
admission authority. In the same immediate Task transaction as Task, TaskSpecRevision(1),
`task.created.v1`, and the RequestId receipt, storage rechecks that the Routine remains
`ACTIVE` at exactly the requested revision, re-renders the supplied inputs from that
revision, verifies the selected lead and every pinned Resource revision belong to the
selected Workspace, and compares the resulting TaskSpec fields to the proposed commit.
Any failed check rolls back all Task/event/receipt writes.

Success leaves the Task `READY` with no PlanRevision, Step, PlanningAssignment,
AgentSession, Attempt, lease, Environment, CapabilityGrant, or Effect. The exact
`routine_id`/`routine_revision` are persisted on Task, and the TaskSpec materializes the
objective, instructions-as-untrusted-context, constraints, non-goals, Resource inputs,
outputs, acceptance criteria, approvals, placement, and budget. Required capability and
verification policy remain available through the immutable Task-pinned RoutineRevision;
future planning/Trust/verifier admission must re-resolve them. This creation transaction
does not claim that those policies have been enforced or that the Task has begun.

The RequestId digest is based on the owner's exact normalized run command (Workspace,
Routine, requested revision, input object, standalone origin), not a mutable Workspace
lead default. The authenticated owner/Workspace check happens before receipt replay. An
exact retry returns the original Task even after the Routine head changes; a new RequestId
must pass current ACTIVE/head admission again. A same-key request with changed inputs or
revision conflicts.

### Revising a saved Task before planning

An owner may append a new TaskSpecRevision while the Task is `READY`, has no accepted
PlanRevision, and has no live `TASK_PLANNING` AgentSession. The command is conditional on
the current Task aggregate version and exactly one parent: the current TaskSpec head.
Unspecified fields are copied from that head, so an objective-only editor cannot clear
inputs, constraints, output requirements, approvals, budget, placement, or lead failover
policy accidentally. The current desktop editor exposes only the objective field. A
successful revision atomically advances the Task's spec pointer/version and writes the
immutable revision, aggregate-state snapshot, `task.spec.revised.v1` event and idempotency
receipt. It creates no planning or execution records. An identical retry recovers the same
revision even if the response was lost; reusing its key for changed content conflicts.

If Task version/spec head changed, a planner became live, the Task left `READY`, or a Plan
was accepted, the edit is rejected. The owner reloads current Task state and uses the normal
steering/replan lifecycle after planning has begun. The storage transaction repeats all
state, owner, Workspace, input Resource revision, and lead-binding checks; the UI is not an
admission authority.

## Initial planning session

TaskService creates a transient PlanningAssignment for the current lead binding and TaskSpecRevision. PlanningCoordinator asks AgentSessionSupervisor to reserve a durable `STARTING` TASK_PLANNING AgentSession in a short storage transaction. Storage rechecks Task version/status/spec/lead, Workspace ownership, binding eligibility, endpoint binding, and current READY Runtime incarnation while claiming the unique planner slot. The coordinator then starts the native adapter outside the transaction. Only observed adapter readiness permits a serialized activation that transitions the session to `ACTIVE` and a first-planning Task from `READY` to `RUNNING` atomically. A `STARTING` session grants no planning tools or invocation authority. After daemon restart, stranded prior-incarnation `STARTING` sessions are reconciled before another planner is admitted.

That is the target admission contract, not the current desktop implementation status. The
current SQLite `AgentSessionStore` returns
`StoreError::Invalid("TASK_PLANNING_ISOLATION_UNAVAILABLE")` from both planner start and
activation before writing a session snapshot, event, request receipt, host binding, or Task
transition. This storage-level gate protects direct internal callers as well as the
readiness route. It may be removed only when a Runtime-owned, current-incarnation
`IsolationAttestation` and the remaining Trust, native-capability, process containment,
lease, and session-settlement checks are produced and transactionally revalidated at
admission; no caller-supplied Boolean or request field can satisfy the gate.

The session has Task read, plan proposal, and user-clarification tools only; it has no Attempt, lease, Environment write access, consequential capability invocation, or artifact publication. Plan acceptance creates/promotes a PlanRevision and materializes Steps; only then can an execution Attempt be admitted. A planning session may be replaced without changing Task identity or fabricating an Attempt. User clarification closes the current planning session; after a valid response, the coordinator builds a new envelope against the current TaskSpec revision and starts a fresh session.

The current Codex transport includes typed read-only thread and plan-turn constructors
with a structured initial-plan output schema. The current official App Server documentation
describes an explicit restricted-readable-roots policy, but the installed local Codex CLI
`0.162.0` schema bundle does not expose that policy field and the LiteCowork transport has
not negotiated or applied it. This path is therefore unqualified; `readOnly` alone must not
be treated as Task Resource scope. The profile probe also does not prove native MCP/app/
plugin/hook tools disabled or mediated. Planner admission remains unavailable until the
installed protocol applies and qualifies restricted roots, non-filesystem tools are
disabled/mediated, and Task-specific Environment identity, process containment, and
recovery are integrated.

## Plan acceptance

`submit_plan` validates:
- references exactly the current TaskSpec revision; stale proposals are rejected with STALE_SPEC_REVISION
- `steps` is non-empty and each `logical_key` is unique within the proposal; durable Step IDs are allocated by TaskService
- no dependency cycles
- every `depends_on_logical_keys` reference exists in the proposal
- acceptance criteria are representable
- requested capabilities are structurally valid

It does not evaluate whether the plan is intellectually good.

Plan acceptance is one TaskService transaction: append immutable PlanRevision, advance
Task.current_plan_revision, create the Step records, supersede obsolete unstarted Steps,
and append all related events. It returns PlanAcceptance with the committed Task version.
A crash therefore cannot leave a current PlanRevision with no corresponding Steps. The
transaction and idempotency receipt commit together; event replay does not repeat Step ID
allocation. A completed Step remains historical; an unstarted obsolete Step becomes
`SUPERSEDED`; an active Step is cancellation-requested or allowed to reach a safe boundary
under the new revision, and its lease remains authoritative until settled. An authorized
TASK_PLANNING AgentSession may propose the initial plan without an Attempt. For later
proposals, the producer must be the currently assigned lead planning session or the
current lead execution Attempt under a valid lease. TaskService validates producer
authority and plan structure, then alone promotes the PlanRevision. Acceptance does not
otherwise change Task status; planning has already moved the Task to `RUNNING` when the
lead session became ready.

The current source slice implements only PlanRevision 1 submitted from an active
`TASK_PLANNING` session while the Task is `RUNNING`. It rejects empty/oversized plans,
duplicate logical keys, missing/self/duplicate dependencies, cycles, and malformed
capability/criterion shapes. SQLite repeats structural checks and atomically persists the
Task/Plan/Step snapshots, events, and idempotency receipt. Plan/Step read ports and GET
routes are wired. Execution-produced replans and the POST Operator route remain
unavailable: current Operator authentication has no trusted producer-scoped session
assertion, and a body-supplied AgentSession ID is not authority. This is not yet
end-to-end planning; no native adapter invokes the service.

## Lead-agent change

`change_lead_agent` validates that the AgentBinding belongs to the same Workspace, is enabled,
and has a compatible Runtime/protocol. It records the requested lead binding and emits a
`task.lead_agent.changed.v1` event. New planning and plan submissions are authorized only
for the new binding. Existing Attempts retain their attempt-scoped AgentBinding and lease
while they drain or are cancelled at a safe boundary. Planning replacement creates a fresh
TASK_PLANNING session without an Attempt. A replacement execution Attempt may not acquire
the Step until its prior lease is released, expired, or revoked and open Effects are reconciled. The Task and its history
remain stable.

`LeadFailoverService` handles only provider-confirmed unavailable/quota-exhausted/runtime-
unavailable observations. `DISABLED` never changes the lead automatically. `ASK` adds a
Needs You choice from its ordered fallback list. `ALLOW_LISTED` may change the lead only
when the exact trigger is enabled by the pinned TaskSpecRevision policy, the fallback is
listed, `max_lead_changes` remains, and all ordinary lead-admission checks pass. It first
settles or safely checkpoints the prior planning session, reconciles Invocations/Effects,
and preserves active Attempts under their original bindings/leases. The transaction emits
`task.lead_agent.changed.v1` with cause `POLICY_FAILOVER` and the triggering observation;
the user command uses cause `OWNER_REQUEST`. Ambiguous provider state, an active unsafe
Effect, or missing fallback readiness blocks the change rather than bypassing recovery.

## Attempt creation

Attempt creation requires:
- Step is READY or explicitly retried from recoverable failure
- selected AgentBinding compatible
- selected Runtime online/eligible
- Environment available or provisionable
- required grants resolved or known pending approval
- no conflicting ACTIVE lease for same Step

The Attempt references its ExecutionLease, while the lease references the Attempt. They
are therefore admitted in one authoritative transaction rather than in separate
observable writes:

1. validate the current PlanRevision, Step readiness, TaskSpec revision, policy, and
   worker constraints; recompute/confirm the current ExecutionDependencyPlan digest.
2. choose a specific eligible Runtime incarnation and create/attach a suitable
   Environment on that Runtime. Failed provisioning produces no active lease.
3. allocate Attempt and lease IDs.
4. inside one StateStore transaction, recheck Step exclusivity; append Attempt(CREATED),
   pin its Runtime and incarnation, acquire the next lease epoch for that exact
   incarnation, bind the lease to the Attempt, set the Step's current Attempt, and append
   the corresponding events.
5. commit; only then expose the Attempt as authoritative.
6. start the AgentSession; transition Attempt through PREPARING to RUNNING only when the
   adapter confirms readiness.

If the transaction conflicts, no authoritative Attempt or lease is committed. The
provisioned Environment is released or retained only under its cleanup policy.
LeaseCoordinator is the only lease writer; the transaction boundary coordinates it with
AttemptRunner and TaskService without exposing a partially bound pair.

Attempt Runtime/incarnation and Environment Runtime are immutable and must match. The
AgentSession and ExecutionLease must also match that same Runtime incarnation. The lease
record stores only a digest of its raw fencing credential; the credential is delivered
over private authenticated Runtime/provider control and never to the Agent or Operator.
If `litecoworkd` restarts after Attempt admission, its new incarnation cannot renew the
old lease or resume that Attempt. Recovery first proves lease expiry or authoritative
incarnation fencing and commits the old lease as EXPIRED/REVOKED, abandons the old
Attempt, and blocks its Step/Task. This transition fences durable lease authority; it does
not prove that a provider process or descendant stopped. Invocation/process containment
and Effect reconciliation must settle before safe continuation can create a new Attempt
with a higher lease epoch. Replacing only the AgentSession may reuse an Attempt only while
the same Runtime incarnation and lease remain current. The current SQLite startup recovery
slice supports only SQLite-clock expiry and leaves the Task blocked; it does not yet
implement explicit revocation fencing, process proof, Effect reconciliation, or continuation.

## Steering

Steering classes:

```text
CHAT_ONLY
TASK_SPEC_CHANGE
PLAN_HINT
URGENT_INTERRUPT
CANCEL
```

Task-spec changes create immutable `TaskSpecRevision(n+1)` with the current revision as
parent. Concurrent edits based on the same parent become siblings; neither wins by
last-writer-wins. The lead agent or user resolves them by creating a new revision that
records both parents. Adopting a new current revision fences the active planning session's
new capability calls and plan submissions in the same Task aggregate transaction. The
PlanningCoordinator then settles its in-flight read-only Invocations and closes that
session before a new PlanningAssignment is built against the new head. Running Attempts
remain pinned to the TaskSpecRevision referenced by their accepted PlanRevision; they are
not silently rewritten. The lead agent decides whether to revise the PlanRevision. Child
Attempts receive only relevant changes, or finish against their recorded revision and
require applicability review before integration.

## Cancellation

Cancellation is cooperative first, forceful second.

```text
Task CANCEL_REQUESTED
  -> fence completion finalization and new verification starts
  -> signal active children
  -> AgentAdapter.cancel/interrupt
  -> reject new capability grants and CapabilityInvocations
  -> publish task.status.changed(CANCEL_REQUESTED); ConversationService and TrustService
     close pending Task-scoped UserRequests and Approvals through their owned transitions
  -> close the active TASK_PLANNING AgentSession; reject late plan proposals after cancel wins
  -> request cancellation of in-flight Task-planning/Attempt CapabilityInvocations
  -> await authoritative provider terminal state
  -> await active VerificationRuns to settle
  -> wait grace period
  -> revoke leases / terminate contained environments when policy permits
  -> reconcile every STARTED/ACKNOWLEDGED/AMBIGUOUS Effect
  -> CANCELLED when Attempts, Invocations, VerificationRuns, pending Task-scoped
     UserRequests/Approvals, and Effects have settled
```

An MCP `tasks/cancel` acknowledgement records intent only. If an Invocation cannot be
confirmed stopped or completed, keep the Task in `CANCEL_REQUESTED` with a visible
blocker; do not call it `CANCELLED`. Conversation-scoped Invocations are not part of a
Task's cancellation set. Cancellation and completion serialize on the Task aggregate
version: if completion commits first, cancellation returns the already terminal result;
after `CANCEL_REQUESTED` commits, CompletionEvaluator cannot complete the Task. Active
VerificationRuns settle against their pinned criteria/inputs and may append Evidence, but
their result cannot finalize a Task after cancellation has won. Cancellation never erases
produced artifacts/evidence/history. Cancellation is valid from `PAUSED` and supersedes an
in-progress `PAUSE_REQUESTED`; a Task-scoped response racing with cancellation is accepted
only if it commits first, and any resulting provider work is still cancelled/reconciled.
After cancellation commits, unresolved requests are closed and cannot enqueue work. Already
resolved UserRequests/Approvals remain immutable, and an approved but unused one-time
Approval cannot authorize new work after cancellation.

## ResumePacket

```text
ResumePacket {
  task_id
  task_spec_revision
  workspace_instruction_revision?
  current_plan_revision
  active_step_ids[]
  completed_step_ids[]
  important_decisions[]
  artifact_refs[]
  evidence_refs[]
  unresolved_questions[]
  failed_strategies[]
  remaining_acceptance_criteria[]
  open_invocation_refs[]
  open_effects[]
  capability_locks[]
  generated_at
  source_attempt_id?
}
```

Portable continuation must work from this packet without a native agent transcript.
The instruction revision is the exact pin from TaskSpecRevision, not the Workspace's
latest revision. Open Invocation references let recovery query retained provider state;
they are never permission to blindly redispatch an ambiguous call. The packet does not
carry live grants/SecretLeases/lease authority into a replacement Attempt.
The packet is encrypted/authorized as a Task artifact, has a digest, and includes only
the selected state needed to resume. It must not embed secret bytes, private hidden
prompts, a full agent transcript, or machine-local credentials.

## Pause and resume

Pause is a durable Task lifecycle operation distinct from cancellation. `request_pause`
stops new Attempt and planning-session admission, asks active execution workers to stop at
a safe boundary, closes the active TASK_PLANNING session, writes a
portable ResumePacket, settles or abandons host Attempts, reconciles open Effects, and
releases leases. For a Task paused from `VERIFYING`, it also fences completion finalization
and starts no new VerificationRuns; already-running runs may settle against their pinned
criteria/inputs and append Evidence, but cannot complete the Task after the pause wins the
Task-version race. `PAUSED` waits until those runs are terminal. If a worker or verifier
cannot settle safely, the command reports that pause is still pending or unsafe; it does
not claim the process stopped. An ambiguous Effect must be reconciled or retained as a
blocker before pause is reported complete.

`resume` rechecks Workspace and Task state, current resource revisions/locations, pinned
capabilities, grants, secret placement, budget, Environment reproducibility, and open
Effects. It returns the Task to an executable, running, or blocked state through TaskService. An
interrupted Attempt normally becomes `ABANDONED` and continuation uses a fresh Attempt
with a higher lease epoch. The narrow exception is a quiescent `INPUT_REQUIRED` provider
Invocation whose source Attempt remains current: on the same Runtime incarnation and
Environment, TaskService may return that Attempt from `WAITING_RESOURCE` to `RUNNING`
with a newly acquired higher lease epoch and a fresh AgentSession after checkpoint and
Effect reconciliation. The old lease/session are never revived. This same-Attempt path is
not available after daemon restart, Runtime handoff, Environment replacement, or plan
invalidation. In those cases the provider task must be reconciled/cancelled before new
work; its UserRequest response can be included as bounded context for a newly authorized
Invocation only after the old operation is settled. A pending UserRequest remains pending
and may put the Task back into `WAITING_USER`.

Pause also fences new CapabilityInvocation creation and asks InvocationRunner to stop
in-flight Task-scoped (including planning) and Attempt-scoped invocations. A provider cancellation acknowledgement is not proof
that work stopped: wait for an authoritative terminal provider state and reconcile any
linked Effect. A confirmed `INPUT_REQUIRED` invocation is quiescent and may remain pending
while the Task is paused; its UserRequest is preserved and no response is sent until
resume. A Task-scoped answer received after `PAUSED` may be stored immutably, but remains
queued until explicit resume. A planning-scoped Invocation may deliver it only after a
fresh active planner is admitted for the same current lead and TaskSpec revision. An
Attempt-scoped Invocation may deliver it only after its source Attempt is re-admitted on
the same Runtime incarnation and Environment with its checkpoint/Effects reconciled, a
new higher-epoch lease, and a fresh AgentSession. If a new Attempt or Runtime is required,
do not send the answer to the old provider task: reconcile/cancel that Invocation first,
then make the saved response bounded context for newly authorized work. In every scope,
InvocationRunner dispatches only the exact stored provider key and response digest after
the owner service revalidates the Task/turn, Grant, capability lock, provider binding, and
Environment; it cannot create another capability operation or revive an old lease/session.
A response during `PAUSE_REQUESTED` is rejected with `CONFLICT` and the request stays
pending. An Approval decision may likewise be recorded while paused, but its one-time use cannot be
consumed until resume. If cancellation wins before dispatch, the local response outbox is
withdrawn; after dispatch, delivery and provider work must be reconciled and are never
relabeled as locally cancelled. An ambiguous or still-running invocation keeps pause
pending or becomes an actionable blocker; TaskService does not report `PAUSED` while it
may still act.

```text
PauseTaskRequest {
  task_id
  expected_task_version
  requested_by
  reason?
}

ResumeTaskRequest {
  task_id
  expected_task_version
  requested_by
}
```

Pause results distinguish `PAUSE_REQUESTED`, `PAUSED`, and unresolved/unsafe blockers.
Pause is idempotent for the same request ID; resume is rejected unless the Task is
`PAUSED`.

## Recover a Step

There is no Task retry command. A Task is failed only after bounded recovery is exhausted
or the outcome is permanently unrecoverable. Recoverable failure is represented on an
Attempt and Step while the Task remains active, blocked, incomplete, or needs user input.
`recover_step` verifies that the Step is recoverable, budget/retry limits remain, required
Effects are reconciled, and current inputs/plan/policy still apply; it then creates a new
Attempt for that Step. It never reopens a terminal Task or reuses an Attempt ID.
Before a user chooses a particular existing Environment, the Operator reads the current
step placement plan. The plan lists eligible Environment candidates and blockers and pins
the Task/Step/spec/plan versions, Runtime incarnations, Environment versions, resource
revisions, and relevant offers into `plan_digest`. A recovery override names a candidate
and that digest. TaskService recomputes eligibility atomically before creating an Attempt;
an expired or changed plan returns `STALE_VERSION` and creates nothing. Without an
override, ordinary placement policy chooses a feasible candidate. A provisioned
replacement Environment is never silently substituted for the prior Attempt's Environment.

```text
RecoverStepRequest {
  task_id
  step_id
  expected_task_version
  expected_step_version
  recovery_reason
  placement_override?: {
    candidate_id
    plan_digest
  }
  requested_by
}
```

`placement_override` is all-or-nothing. Its `candidate_id` must identify an eligible
existing Environment in the current preview; the request cannot supply a Runtime or
Environment locator directly. This ties user choice to a current, policy-checked candidate
and lets the resulting Attempt record its actual Runtime and Environment.

## Completion proposal

Agent proposal includes:

```text
CompletionProposal {
  task_id
  proposed_by_attempt
  claimed_outputs[]
  claimed_criteria[]
  unresolved[]
  summary
}
```

Task Runtime enters VERIFYING and invokes verifier selection. Completion requires:
- no required child Attempts active
- required outputs exist
- required approvals resolved
- no unresolved AMBIGUOUS effects
- deterministic checks passed
- acceptance criteria evaluated at required evidence level

`INCOMPLETE`, `BLOCKED`, and `NEEDS_USER` preserve the Task and can be resumed only by a
valid new command/condition. A final `COMPLETED`, `FAILED`, or `CANCELLED` Task is not
reopened; follow-up work creates a new Task linked by provenance. `FAILED` is committed
only after recovery is exhausted and no active child, lease, or ambiguous Effect remains.
The current actionable reasons for `BLOCKED` and `NEEDS_USER` are stored as
`Task.blocking_conditions[]`, with a stable blocker ID, error code, safe user-facing
message, optional subject reference, resolution hint, and optional UserRequest. Clearing
a blocker removes it from the current Task projection and emits the updated full blocker
list; prior event/state snapshots remain immutable.

## Errors


```text
TASK_NOT_FOUND
AGENT_UNAVAILABLE
STALE_TASK_VERSION
STALE_SPEC_REVISION
INVALID_PLAN
PLAN_CYCLE
STEP_NOT_READY
LEASE_CONFLICT
PLACEMENT_UNAVAILABLE
POLICY_DENIED
APPROVAL_REQUIRED
CAPABILITY_UNAVAILABLE
ENVIRONMENT_UNAVAILABLE
TASK_TERMINAL
INVALID_TRANSITION
STALE_FENCE
INVALID_RESOURCE_REF
CONFLICT
```

## Host delegation admission

Host delegation uses the `DelegateRequest` contract in [`DELEGATION.md`](DELEGATION.md).
TaskService accepts a request only from the current RUNNING parent Attempt's active
AgentSession and current lease. `step_id` must identify a READY Step in the Task's current
accepted PlanRevision, with all dependencies complete and no active Attempt. The requested
objective, inputs, outputs, and acceptance criteria may narrow that Step; they cannot
expand it or change the pinned TaskSpec.

If the plan has no suitable Step, the lead submits a PlanRevision proposal and TaskService
accepts it through the existing plan-revision path. Delegation never creates an
unplanned Step or edits a PlanRevision. This keeps every child result attached to an
accepted requirement and makes later verification/recovery reproducible.

Admission checks current profile and binding versions, descriptor freshness, Task and
profile budgets, Task-wide/parent/profile concurrency, depth, deadline, Runtime and
Environment compatibility, input revision/freshness, Trust policy, and child-scoped
grants. Attempt, Step binding, ExecutionLease, budget reservation, and the delegation
admission event commit atomically. AgentSession startup follows the normal Attempt
`CREATED -> PREPARING -> RUNNING` lifecycle. A pre-commit rejection creates no Attempt or
lease; a post-commit startup failure is an ordinary failed Attempt with preserved
provenance.

The TaskSpec `max_child_attempts` and `max_concurrency` ceilings always apply. A
DelegationProfile may impose lower limits but cannot raise Task limits. Depth is computed
from immutable `parent_attempt_id` links and checked again in the admission transaction.
The active-child count is derived from current nonterminal Attempts, not cached state.
Retry/escalation creates another Attempt for the same recoverable Step and consumes the
same Task recovery budget.

Lead handoff, delegated work, and worker replacement remain distinct commands. A lead
change closes/drains the planning session and produces a bounded handoff projection; it
does not change active Attempt identity, profile pinning, or parent links. A replacement
worker is a new Attempt admitted only after the prior Attempt and its Effects are settled.

## Coworker Task origin, autonomy and handoff target

Ordinary Conversations have no required Coworker origin. Coworker conversations and standing responsibilities optionally bind durable Coworker identity/revision at Task admission. A completed Routine run does not mark its Goal or StandingResponsibility completed unless explicit verified stop criteria hold. A Coworker-to-Coworker handoff is a NEW Task under the recipient identity and separate grants, carrying only explicitly approved Artifact/Resource refs, not source private memory, credentials or native session. Bound task budgets and cooldown prevent circular autonomous chains; every child has provenance and depth/cycle controls. Pausing a Coworker blocks new unattended Task admissions without mutating already running Attempts, while Pause all safely requests ordinary Task pause and awaits Effect/Invocation reconciliation.
