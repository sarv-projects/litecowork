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

  steer(SteerTaskRequest) -> TaskSpecRevision | ConversationMessage
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

Creation is atomic: Task + TaskSpecRevision(1) + `task.created` event. The selected lead AgentBinding is stored on Task and referenced by the initial spec as `preferred_lead_agent_binding_id`; the placement preference is pinned in that spec. A new Task enters planning without creating a Step, Attempt, ExecutionLease, or Environment.
The operator appends the originating ConversationMessage in the same command boundary
when the Task came from a message. A standalone Task may omit a Conversation.

## Initial planning session

TaskService creates a PlanningAssignment for the current lead binding and TaskSpecRevision. PlanningCoordinator asks AgentSessionSupervisor to start a LEAD_PLANNING AgentSession. The session has Task read, plan proposal, and user-clarification tools only; it has no Attempt, lease, Environment write access, consequential capability invocation, or artifact publication. TaskService changes READY to RUNNING only after the session is ready. Plan acceptance creates/promotes a PlanRevision and materializes Steps; only then can an execution Attempt be admitted. A planning session may be replaced without changing Task identity or fabricating an Attempt.

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
until settled. An authorized LEAD_PLANNING AgentSession may propose the initial plan without
an Attempt. For later proposals, the producer must be the currently assigned lead planning
session or the current lead execution Attempt under a valid lease. TaskService validates
producer authority and plan structure, then alone promotes the PlanRevision.

## Lead-agent change

`change_lead_agent` validates that the AgentBinding belongs to the same Workspace, is enabled,
and has a compatible Runtime/protocol. It records the requested lead binding and emits a
`task.lead_agent.changed.v1` event. New planning and plan submissions are authorized only
for the new binding. Existing Attempts retain their attempt-scoped AgentBinding and lease
while they drain or are cancelled at a safe boundary. Planning replacement creates a fresh
LEAD_PLANNING session without an Attempt. A replacement execution Attempt may not acquire
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
   worker constraints.
2. choose placement and create/attach a suitable Environment. Failed provisioning
   produces no active lease.
3. allocate Attempt and lease IDs.
4. inside one StateStore transaction, recheck Step exclusivity; append Attempt(CREATED),
   acquire the next lease epoch, bind the lease to the Attempt, set the Step's current
   Attempt, and append the corresponding events.
5. commit; only then expose the Attempt as authoritative.
6. start the AgentSession; transition Attempt through PREPARING to RUNNING only when the
   adapter confirms readiness.

If the transaction conflicts, no authoritative Attempt or lease is committed. The
provisioned Environment is released or retained only under its cleanup policy.
LeaseCoordinator is the only lease writer; the transaction boundary coordinates it with
AttemptRunner and TaskService without exposing a partially bound pair.

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
records both parents. Running Attempts are notified; the lead agent decides whether to
revise the PlanRevision. Child Attempts receive only relevant changes, or finish against
their recorded revision and require applicability review before integration.

## Cancellation

Cancellation is cooperative first, forceful second.

```text
Task CANCEL_REQUESTED
  -> signal active children
  -> AgentAdapter.cancel/interrupt
  -> stop new capability grants and consequential Effects
  -> wait grace period
  -> revoke leases / terminate contained environments when policy permits
  -> reconcile every STARTED/ACKNOWLEDGED/AMBIGUOUS Effect
  -> CANCELLED when authoritative work has settled
```

Cancellation never erases produced artifacts/evidence/history.

## ResumePacket

```text
ResumePacket {
  task_id
  task_spec_revision
  current_plan_revision
  active_step_ids[]
  completed_step_ids[]
  important_decisions[]
  artifact_refs[]
  evidence_refs[]
  unresolved_questions[]
  failed_strategies[]
  remaining_acceptance_criteria[]
  open_effects[]
  capability_locks[]
  generated_at
  source_attempt_id?
}
```

Portable continuation must work from this packet without a native agent transcript.
The packet is encrypted/authorized as a Task artifact, has a digest, and includes only
the selected state needed to resume. It must not embed secret bytes, private hidden
prompts, a full agent transcript, or machine-local credentials.

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
reopened; follow-up work creates a new Task linked by provenance.

## Errors

```text
TASK_NOT_FOUND
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
