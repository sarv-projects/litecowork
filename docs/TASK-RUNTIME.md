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

`PlanningAssignment` is an internal, transient dispatch envelope, not a persisted domain
entity or lifecycle. It contains the Task ID, expected Task version, pinned TaskSpec
revision, current lead AgentBinding, idempotency RequestId, and immutable planning-context
packet reference. PlanningCoordinator consumes it to admit one durable `TASK_PLANNING`
AgentSession. The AgentSession and Task aggregate are the durable records; at most one
planning session may be STARTING/ACTIVE/INTERRUPTING/CLOSING for a Task. A changed Task
version, spec revision, lead binding, or non-runnable Task status invalidates the envelope.
Replacing a planner creates a new envelope and AgentSession while retaining the same Task.

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
  preferred_lead_agent_binding_id?
  placement_preference?
}
```

Creation resolves the lead binding from an explicit `preferred_lead_agent_binding_id`,
otherwise the Workspace default. The binding must belong to the Workspace, be enabled,
and have a currently eligible endpoint. If no eligible binding exists, creation returns
`AGENT_UNAVAILABLE`, creates no Task/turn, and preserves the unsent composer draft so
setup can finish first. Creation is atomic: Task + TaskSpecRevision(1) + `task.created`
event. The selected lead AgentBinding is stored on Task and referenced by the initial
spec as `preferred_lead_agent_binding_id`; the placement preference is pinned in that
spec. A new Task enters planning without creating a Step, Attempt, ExecutionLease, or
Environment.
There is no persisted Task `DRAFT` state in v1; incomplete/unsent composer content remains
in the Operator until admission succeeds.
The operator appends the originating ConversationMessage in the same command boundary
when the Task came from a message. A standalone Task may omit a Conversation.

## Initial planning session

TaskService creates a transient PlanningAssignment for the current lead binding and TaskSpecRevision. PlanningCoordinator asks AgentSessionSupervisor to start a durable TASK_PLANNING AgentSession. The session has Task read, plan proposal, and user-clarification tools only; it has no Attempt, lease, Environment write access, consequential capability invocation, or artifact publication. TaskService changes READY to RUNNING only after the session is ready. Plan acceptance creates/promotes a PlanRevision and materializes Steps; only then can an execution Attempt be admitted. A planning session may be replaced without changing Task identity or fabricating an Attempt. User clarification closes the current planning session; after a valid response, the coordinator builds a new envelope against the current TaskSpec revision and starts a fresh session.

## Plan acceptance

`submit_plan` validates:
- references exactly the current TaskSpec revision; stale proposals are rejected with STALE_SPEC_REVISION
- Step IDs unique within plan
- no dependency cycles
- all dependencies exist
- acceptance criteria are representable
- requested capabilities are structurally valid

It does not evaluate whether the plan is intellectually good.

Plan acceptance is one TaskService transaction: append immutable PlanRevision, advance
Task.current_plan_revision, create the Step records, supersede obsolete unstarted Steps,
and append all related events. It returns PlanAcceptance. A crash therefore cannot leave a
current PlanRevision with no corresponding Steps. The internal materialization helper is
idempotent but is not a separately observable command. A completed Step remains historical;
an unstarted obsolete Step becomes `SUPERSEDED`; an active Step is cancellation-requested
or allowed to reach a safe boundary under the new revision, and its lease remains authoritative
until settled. An authorized TASK_PLANNING AgentSession may propose the initial plan without
an Attempt. For later proposals, the producer must be the currently assigned lead planning
session or the current lead execution Attempt under a valid lease. TaskService validates
producer authority and plan structure, then alone promotes the PlanRevision.

## Lead-agent change

`change_lead_agent` validates that the AgentBinding belongs to the same Workspace, is enabled,
and has a compatible Runtime/protocol. It records the requested lead binding and emits a
`task.lead_agent.changed.v1` event. New planning and plan submissions are authorized only
for the new binding. Existing Attempts retain their attempt-scoped AgentBinding and lease
while they drain or are cancelled at a safe boundary. Planning replacement creates a fresh
TASK_PLANNING session without an Attempt. A replacement execution Attempt may not acquire
the Step until its prior lease is released, expired, or revoked and open Effects are reconciled. The Task and its history
remain stable.

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
old lease or resume that Attempt. Recovery marks the old try abandoned after authority
and Effect reconciliation; safe continuation creates a new Attempt and higher lease
epoch. Replacing only the AgentSession may reuse an Attempt only while the same Runtime
incarnation and lease remain current.

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
