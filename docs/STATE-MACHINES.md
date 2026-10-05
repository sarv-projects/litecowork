# State Machines and Transition Ownership

State transitions are domain rules, not UI conventions. A command validates the current
state and expected version, applies one legal transition, appends its event, and updates
the current-state record in one transaction. An invalid transition returns a typed
conflict and writes no transition event. UI projections never cause transitions.

## Workspace

```text
ACTIVE -> ARCHIVED
```

WorkspaceService alone archives a Workspace. Archive is allowed only when every Task is terminal and every Automation is DISABLED. Quiescence also requires Conversation turns and scoped Invocations to be settled, no active grants/SecretLeases/control leases, and persistent Environments with no live workload. Authorized watchers/triggers stop before the read-only transition. Retained Environment state may remain suspended under storage/backup policy; archive never silently destroys it. Unknown provider quiescence blocks archive with `WORKSPACE_NOT_QUIESCENT`. The transition is terminal in v1. Reads remain available; existing authorized Artifact/Resource downloads remain available. Every domain mutation is rejected, including Task changes, capability grants/activation, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel materialization. Archiving does not delete data or erase copies already replicated to another Runtime. A replication-policy change is an ACTIVE -> ACTIVE versioned update; it affects future transfers and never silently deletes existing remote copies.

`default_agent_binding_id` changes are ACTIVE -> ACTIVE Workspace aggregate updates.
WorkspaceService validates that the selected binding exists in the same Workspace and is
enabled and lead-eligible, applies the expected Workspace version, then emits
`workspace.default_agent_binding.changed.v1`. Clearing the default is explicit and emits
the same event with `to_agent_binding_id: null`. A no-op change emits no event. The
binding's runtime/endpoint availability is checked again at each new turn/Task admission;
an unavailable default is never silently replaced.

`lead_eligible` is a separate versioned AgentBinding boolean and emits
`agent.binding.lead_eligibility.changed.v1`. A Workspace default cannot point to a
disabled or non-lead-eligible binding. Making the current default worker-only first
requires an explicit Workspace default change; already admitted Tasks remain pinned.

`primary_coworker_id` is another ACTIVE -> ACTIVE Workspace update. WorkspaceService
requires an active or paused same-Workspace Coworker, applies the expected Workspace version, and
emits `workspace.primary_coworker.changed.v1`; null explicitly clears the selection.
Changing it does not rewrite existing Task origin or lead provenance.

`SELECTED_FOLDERS` can be selected only by a versioned policy update that supplies at
least one active WorkspaceRoot owned by this Workspace. WorkspaceService commits the
normalized membership rows, Workspace version, and policy event atomically. Workspace
creation starts with an empty root selection; its create API cannot choose this policy.
If a selected root becomes paused, unavailable, or revoked, transfer from that root stops
until an eligible root is available; the resolver never substitutes another folder or
Runtime path. A revocation does not delete content already replicated elsewhere.

Instruction revisions are immutable child records. WorkspaceService accepts the first
revision with no parents, then an ordinary revision only when its single parent is the
current head and the caller's Workspace `expected_version` matches. An explicit merge may
name multiple existing parents. A stale offline update becomes a pending intent and is
rejected for explicit rebase/merge when the Hub reconnects; the service does not use HLC
or last-writer-wins to select a head.

## Task

```text
READY -> RUNNING -> VERIFYING -> COMPLETED
  |        |            |  |  |  |
  |        |            |  |  |  +-> FAILED (recovery exhausted)
  |        |            |  |  +----> BLOCKED
  |        |            |  +-------> NEEDS_USER
  |        |            +----------> INCOMPLETE
  |        |                         -> READY (after recovery)
  |        +-> WAITING_USER -> READY
  |        +-> BLOCKED -> READY
  |        +-> PAUSE_REQUESTED -> PAUSED
  +-> CANCEL_REQUESTED -> CANCELLED
READY | RUNNING | WAITING_USER | BLOCKED | VERIFYING -> PAUSE_REQUESTED
PAUSE_REQUESTED -> PAUSED | prior nonterminal state
PAUSED -> READY | WAITING_USER | BLOCKED | NEEDS_USER | CANCEL_REQUESTED
READY | RUNNING | WAITING_USER | BLOCKED | VERIFYING | NEEDS_USER | INCOMPLETE |
PAUSE_REQUESTED | PAUSED -> CANCEL_REQUESTED -> CANCELLED
```

- `READY -> RUNNING`: TaskService after either (a) the authorized lead TASK_PLANNING AgentSession becomes ready, or (b) a READY Task with an accepted PlanRevision admits its first/recovery Step Attempt. Planning has no Step, Attempt, ExecutionLease, or Environment. A Step Attempt may start only after a PlanRevision is accepted and its Steps are materialized.
- `RUNNING -> VERIFYING`: TaskService accepts a completion proposal; the proposal is
  not evidence of completion.
- `VERIFYING -> COMPLETED`: CompletionEvaluator recommends; TaskService commits only
  when every mandatory criterion has sufficient evidence, required children are settled,
  approvals are resolved, and no Effect is ambiguous.
- `VERIFYING -> READY | INCOMPLETE | BLOCKED | NEEDS_USER | FAILED`: TaskService from
  verification and recovery results. A failed criterion with an eligible recovery returns
  the Task to `READY`; a later admitted recovery Attempt moves it to `RUNNING`. There is no
  direct `VERIFYING -> RUNNING` transition. `FAILED` is allowed only after bounded
  recovery is exhausted or the outcome is permanently unrecoverable; recoverable
  Step/Attempt failure does not fail the Task.
- `WAITING_USER -> READY`: TaskService after a valid user response/approval only when no
  provider-input delivery remains unsettled. If the response is bound to MCP `input_required`,
  the affected Attempt remains `WAITING_RESOURCE` and the Task becomes `BLOCKED` when no
  independent Step can proceed; otherwise the Task remains `RUNNING`. Unblock/admission
  occurs only after the exact provider input is accepted or reconciled to a terminal outcome.
- `BLOCKED -> READY`: TaskService after the blocking resource/policy condition
  is resolved and placement is revalidated.
- A TaskSpec revision is accepted with the Task's expected version and explicit parent
  revision(s). If it changes the head while planning is active, the same transaction fences
  that session from new Invocations and plan acceptance; PlanningCoordinator closes it and
  settles its read-only Invocations before opening a replacement session. PlanRevision
  insertion requires the producer AgentSession's pinned spec revision to equal the
  revision being planned. Active Attempts remain pinned to their Step's PlanRevision and
  continue only while their ordinary Attempt/lease authority remains valid; their results
  receive applicability review before integration.
- `NEEDS_USER | INCOMPLETE -> READY`: TaskService after explicit recovery or revised
  requirements. Recoverable retry creates a new Attempt for a Step; there is no Task
  retry command and no new Task identity.
- `READY | RUNNING | WAITING_USER | BLOCKED | VERIFYING -> PAUSE_REQUESTED`: TaskService
  accepts explicit pause, stops admitting work, fences completion finalization and new
  VerificationRuns and plan acceptance, requests safe-boundary interruption, checkpoints
  portable state, asks PlanningCoordinator/AgentSessionSupervisor to close the active
  TASK_PLANNING AgentSession, and asks AttemptRunner to settle host Attempts. It also
  asks InvocationRunner to settle in-flight Task-planning/Attempt CapabilityInvocations,
  EffectReconciler to reconcile open Effects, and LeaseCoordinator to release leases.
  InvocationRunner requests provider cancellation and waits
  for authoritative terminal state; an `INPUT_REQUIRED` invocation confirmed quiescent
  may remain pending. VerificationRuns already in progress may settle against their pinned
  criteria/inputs and append Evidence, but cannot finalize the Task after the pause wins
  its aggregate-version race; all active runs must be terminal before `PAUSED`. An
  ambiguous Effect, still-running Invocation, or unsettled VerificationRun keeps the Task
  in `PAUSE_REQUESTED` with a visible blocker and, where input is required, a linked
  UserRequest; it cannot be treated as safely quiesced.
- `PAUSE_REQUESTED -> PAUSED`: TaskService only after the active TASK_PLANNING session,
  host Attempts, and mutating or still-running Invocations have stopped/settled, confirmed `INPUT_REQUIRED` Invocations
  are quiescent, required checkpoints are committed, Effects are reconciled, and leases
  are released/expired/revoked. A source Attempt for a confirmed quiescent provider input
  may remain `WAITING_RESOURCE` without a lease if its checkpoint and Environment are
  retained; other interrupted Attempts are abandoned. Pause never erases pending
  UserRequests or cancels the Task.
- `PAUSE_REQUESTED -> prior nonterminal state`: TaskService may roll back the pause
  request when a safe checkpoint cannot be achieved, with an explicit reason. An
  unresolved Effect/Invocation remains in `PAUSE_REQUESTED` while the user supplies
  information or reconciliation completes; it does not masquerade as a completed pause.
- `PAUSED -> READY | RUNNING | WAITING_USER | BLOCKED | NEEDS_USER`: TaskService handles explicit
  resume by revalidating inputs, resources, grants, secrets, Environment, budget, and
  unresolved Effects. Normally continuation uses a fresh Attempt and higher lease epoch.
  A retained `WAITING_RESOURCE` Attempt may resume in place only on the same
  RuntimeIncarnation and Environment, with its plan/input/grant still valid, checkpoint
  reconciled, a new higher-epoch lease, and a fresh AgentSession; this allows an exact
  provider `input_required` response to continue its source Invocation safely. Expired
  leases and old sessions are never revived. After Runtime handoff/restart, Environment
  replacement, or plan invalidation, do not dispatch the old provider input; reconcile or
  cancel that Invocation before a replacement Attempt can consume the saved response as
  context. `RUNNING` is committed only with a current resumed Attempt and lease.
- A Task-scoped UserRequest response during `PAUSE_REQUESTED` returns a versioned
  `CONFLICT` and leaves the request pending. In `PAUSED`, ConversationService may append
  the validated response, but provider-input delivery stays queued and Task remains
  `PAUSED` until explicit resume; resume revalidates the source Attempt/lease and only
  then authorizes exact-key delivery. TrustService may record an Approval decision while paused, but no ApprovalUse,
  Effect, grant, or Attempt is admitted until resume. A winning cancellation closes
  pending requests and rejects later responses.
- Plan proposals and pause/cancel serialize on the Task aggregate version. A plan accepted
  before pause/cancel wins may be retained as history; after `PAUSE_REQUESTED` or
  `CANCEL_REQUESTED` commits, proposals from the old planning session are rejected and that
  session cannot create Steps or authorize further work. Pause/cancel waits until that
  session and its Task-scoped Invocations are settled.
- `CANCEL_REQUESTED -> CANCELLED`: TaskService only after the active TASK_PLANNING session,
  host Attempts, Task-planning/Attempt CapabilityInvocations, and active VerificationRuns
  settle and required open Effects are
  reconciled. A provider's cancellation acknowledgement alone is insufficient; if work
  may still run, a VerificationRun is unsettled, or an Effect remains ambiguous, keep the
  Task `CANCEL_REQUESTED` with a visible blocker. The `task.status.changed` event is
  consumed by ConversationService and TrustService, which close pending Task-scoped
  UserRequests and pending Approvals as `CANCELLED`; their records are never mutated
  directly by TaskService. Task-scoped response and approval admission checks the Task
  status in the same authoritative transaction, so cancellation winning first fences later
  use. TaskService finalizes only after those closures settle. Already-resolved records
  remain immutable, and an approved-but-unused Approval cannot be consumed after its Task
  enters cancellation. Completion and cancellation serialize on the Task
  aggregate version: a completion committed first wins; after cancellation
  commits, a late verification result may be recorded but cannot complete the Task. That
  status preserves cancellation intent without falsely claiming cancellation or user
  resolution.
- `PAUSE_REQUESTED | PAUSED -> CANCEL_REQUESTED`: an explicit cancellation supersedes a
  pending or completed pause. Pause-specific continuation stops; any committed portable
  checkpoint remains historical state, not proof that work stopped. Ordinary cancellation
  settlement rules still apply.
- `FAILED` is terminal only after recovery policy is exhausted and no ambiguous Effect or
  unsettled child remains. `COMPLETED`, `FAILED`, and `CANCELLED` are terminal. Follow-up
  work after terminal failure creates a new Task linked by provenance.

An unresolved approval is represented by Task `WAITING_USER` or Attempt
`WAITING_APPROVAL`, according to whether the whole Task is paused. A resource wait is
represented by Attempt `WAITING_RESOURCE`; the Task is `BLOCKED` only when no other
required Step can proceed.

The Task's `TaskSpecRevision` pins a `LeadFailoverPolicy`; omission on Task creation
resolves and stores the Coworker default or an explicit `DISABLED` policy. A lead change is
an ACTIVE -> ACTIVE Task aggregate update and is allowed only at a safe planning boundary.
`ASK` creates a Needs You decision and does not change the lead. `ALLOW_LISTED` requires a
fresh trigger observation matching the pinned policy, remaining `max_lead_changes`, and
the next ordered fallback that passes current Workspace, lead eligibility, endpoint,
Runtime/incarnation, authentication, Trust, resource, deadline, and budget checks. The
TaskService transaction fences new work from the previous lead and appends
`task.lead_agent.changed.v1` with `cause=POLICY_FAILOVER`, the producing service actor,
the pinned TaskSpec revision, and trigger observation. An owner-initiated change records
`cause=OWNER_REQUEST` and a principal actor. Both causes retain every admitted Attempt and
its original binding; the new lead receives a bounded handoff projected from committed
Task state. If no fallback qualifies or the policy's change limit is reached, the current
lead remains pinned and TaskService opens a Needs You blocker. No event is emitted for a
failed candidate.

## Step

```text
PENDING -> READY -> RUNNING -> VERIFYING -> COMPLETED
   |          |        |  |          |  +-> FAILED
   |          |        |  |          +----> RUNNING
   |          |        |  +-> BLOCKED -> READY
   |          |        +-> WAITING_USER -> READY
   |          +-> SUPERSEDED
   +-> SUPERSEDED
PENDING | READY | WAITING_USER | BLOCKED | FAILED -> CANCELLED
RUNNING -> CANCEL_REQUESTED -> CANCELLED
```

TaskService owns Step transitions. `READY` requires all dependencies complete, inputs available, policy satisfied, and a feasible placement. A Step may have sequential Attempts; only its active lease's Attempt is authoritative. `WAITING_USER` pauses the Step for a user decision; `BLOCKED` means a required resource/policy condition prevents progress. `FAILED -> READY` is legal only through explicit `recover_step`, after failure classification, retry-budget check, and Effect reconciliation. Replanning preserves completed Steps and marks obsolete unstarted Steps SUPERSEDED. COMPLETED, CANCELLED, and SUPERSEDED are terminal.

## Attempt

```text
CREATED -> PREPARING -> RUNNING
PREPARING -> FAILED | ABANDONED | CANCEL_REQUESTED
RUNNING -> WAITING_APPROVAL | WAITING_RESOURCE | CHECKPOINTING
RUNNING -> COMPLETED | FAILED | ABANDONED | CANCEL_REQUESTED
WAITING_APPROVAL | WAITING_RESOURCE -> RUNNING | FAILED | ABANDONED | CANCEL_REQUESTED
CHECKPOINTING -> RUNNING | FAILED | ABANDONED
CANCEL_REQUESTED -> CANCELLED
```

AttemptRunner owns transitions and requires a current ExecutionLease for all
LiteCowork-mediated mutations. Attempt's Runtime ID, RuntimeIncarnation ID, and
Environment identity are immutable; the AgentSession and ExecutionLease must match that
Runtime/incarnation. `COMPLETED` means the worker settled this try; the Task may still be
unverified. `ABANDONED` means the Attempt lost authority or its Runtime/session disappeared.
A replacement try gets a new Attempt ID. An AgentSession restart may retain the Attempt
only within the same Runtime incarnation with a valid lease and consistent checkpoint. A
Task pause may release that lease while preserving a quiescent provider-input Attempt in
`WAITING_RESOURCE`; explicit resume can reacquire a strictly higher lease epoch for the
same Attempt and start a fresh AgentSession after reconciliation. A daemon restart creates
a new incarnation, so the old Attempt cannot be resumed or renewed by it; reconcile it,
then create a new Attempt and strictly higher lease epoch if continuation is safe.

## AgentSession

```text
STARTING -> ACTIVE -> INTERRUPTING -> ACTIVE
    |         |  |         +-> LOST
    |         |  +-> CLOSING -> CLOSED
    +--------> LOST
```

AgentSessionSupervisor owns durable session states. `CONVERSATION` sessions carry a
Conversation and exactly one ConversationTurn; `TASK_PLANNING` sessions are Task-scoped,
pin the TaskSpecRevision, and have no Attempt; `ATTEMPT_EXECUTION` sessions reference
exactly one Attempt and pin its PlanRevision's TaskSpecRevision. A lost Conversation/planning
session does not fail its Conversation/Task by itself. A closed/lost session cannot return
to ACTIVE. An opaque native session/resume handle belongs only to a local
`AgentSessionHostBinding`; same-incarnation resume requires fresh adapter validation. A
new Runtime incarnation always creates a new AgentSession and never receives the prior
local handle.

`AgentHostInstance` is separate Runtime-operational state:

```text
STARTING -> READY <-> BUSY
STARTING | READY | BUSY -> DEGRADED | FAILED
DEGRADED -> READY only after successful probe
READY | DEGRADED -> STOPPING -> STOPPED | FAILED
BUSY -> READY after references/concurrency demand fall
```

`AgentHostSupervisor` owns this state. `active_session_count` is derived from local
`AgentSessionHostBinding` records joined to nonterminal AgentSessions; it is not an
independently incremented counter. `READY -> BUSY` means the selected endpoint has
reached its admitted concurrency limit, not that the host process itself is unhealthy.
`DEGRADED -> READY` requires a successful probe. `STOPPED` and `FAILED` are terminal for
that instance; a later start creates a new instance. A locally spawned process is stopped
only when LiteCowork owns it and all AgentSession references are released. Externally owned shared
services are never killed by idle cleanup. An owned shared host may stop only after all
references are released and its lifetime policy allows cleanup.

## ConversationTurn

```text
OPEN -> RUNNING -> WAITING_USER -> WAITING_DEPENDENCY -> RUNNING
          |  |          |                 |              +-> WAITING_USER
          |  |          |                 +-> FAILED
          |  |          +-> CANCEL_REQUESTED
          |  +-> COMPLETED
          +-> FAILED | CANCEL_REQUESTED
FAILED -> RUNNING (explicit retry only)
OPEN -> CANCELLED
RUNNING | WAITING_USER | WAITING_DEPENDENCY -> CANCEL_REQUESTED
CANCEL_REQUESTED -> CANCELLED | COMPLETED | FAILED
```

ConversationService creates the turn and owns user-request/cancel transitions.
AgentTurnCoordinator owns dispatch, settlement, retry, and continuation after a structured
Conversation-scoped UserRequest response. On a valid response to the request linked to the
current WAITING_USER turn, ConversationService atomically stores the immutable response and
moves the turn to WAITING_DEPENDENCY before enqueueing continuation. This status means the
user has answered but an external provider, agent host, or required session startup has not
yet become ready; it is not another request for user input. For an MCP `input_required`
request, `tasks/update` acknowledgement alone does not move the turn forward: InvocationRunner
polls `tasks/get` until that exact key is no longer outstanding or the provider reaches a
terminal state. Provider-confirmed acceptance permits AgentTurnCoordinator to start a fresh
Conversation session. If the response is rejected and the provider remains `INPUT_REQUIRED`,
the old request closes and a new UserRequest is created for the current provider key, moving
the turn back to `WAITING_USER`. Ambiguous acceptance leaves the turn `WAITING_DEPENDENCY`
and forbids a new AgentSession until reconciliation.
After session readiness is confirmed, it atomically sets the turn to RUNNING, updates its
current session pointer, and emits `conversation.turn.resumed.v1` with the UserRequest ID.
The response and continuation are idempotent by request identity; a stale/dismissed request
cannot resume a turn. A retry is legal only from
`FAILED`, before the turn is cancelled or completed, and while the Conversation and
selected AgentBinding remain usable. It increments `retry_ordinal`, starts a fresh
Conversation-scoped AgentSession, and emits `conversation.turn.retried.v1` atomically
with setting the current session and status to `RUNNING`. It does not erase or rewrite
partial agent messages: each remains linked to the AgentSession that produced it. A
completed response is appended once per successful turn settlement; duplicate retries
are rejected by expected turn version/idempotency key. AgentSession ID on the turn is a
current/most-recent pointer, while each message permanently records the session and binding
that produced it.

ConversationService may move an undispatched OPEN turn directly to CANCELLED or request
cancellation for a running/waiting turn. AgentTurnCoordinator asks the adapter to stop;
the turn settles as CANCELLED only after stop/closure is observed. If final completion or
failure wins the race before stop is confirmed, that observed outcome settles the turn.
Late session events after settlement are retained only as adapter diagnostics and cannot
append a user-visible final response.

Each admitted turn attempt uses a fresh Conversation-scoped AgentSession. On turn
completion/failure/cancellation or a durable wait (`WAITING_USER` or
`WAITING_DEPENDENCY`), the coordinator waits for adapter
quiescence, transitions the session through CLOSING to CLOSED/LOST, and releases its local
host binding. A provider rejection that requires revised input creates a new UserRequest and
returns the turn to WAITING_USER. An irrecoverable provider/startup failure moves the turn
to FAILED with a retryable reason after bounded reconciliation. A user response resumes the
same ConversationTurn with a new AgentSession only after its dependency is ready; an explicit
retry likewise uses a new session. Durable Conversation messages and structured
UserRequest/response records supply context, so a waiting turn does not pin an agent process.
For Task planning, the same close-on-settlement rule applies to a PlanningAssignment. An
Attempt may use sequential execution sessions only while its immutable execution identity,
Runtime incarnation, valid lease, and reconciled Invocation/checkpoint state remain valid.
Yielding an Attempt session does not settle the Attempt when a durable provider Invocation
is still running; a fresh session may be admitted only after its result is available and
Attempt authority is rechecked.

## AgentBinding

```text
DISABLED -> ENABLED -> DISABLED
```

AgentBindingService owns enablement and version checks. A new binding is DISABLED.
Disabling immediately prevents new planning/Attempt admission; sessions already admitted
remain pinned and settle under their current lifecycle, lease, and Effect rules. A
disabled binding can be enabled again after TrustService and runtime eligibility checks.
The service rejects disabling the current Workspace default or removing its lead
eligibility until the owner explicitly clears or changes the Workspace default; these
remain separate versioned aggregate commands and events. Changing `lead_eligible` to
false blocks new lead selection but leaves already admitted Tasks and sessions pinned;
delegated worker eligibility is independent.

## DelegationProfile

```text
ENABLED <-> DISABLED
ENABLED | DISABLED -> ARCHIVED
```

DelegationProfileService owns status and immutable revision creation. An edit creates a
new revision and advances the current head with expected-version checking. `ARCHIVED` is
terminal. Disable/archive blocks new admissions immediately after commit; it does not
rewrite or cancel child Attempts that already pinned a revision. An Attempt must retain
its profile revision provenance even after the profile is archived. A name change creates
a revision, and normalized names are unique per AgentBinding among non-archived profiles.
Duplication requires a non-archived source and creates revision 1 of a new disabled
profile on the same AgentBinding; it copies no execution or authority state. Export/import
is not supported in v1.

## Coworker

```text
ACTIVE <-> PAUSED
ACTIVE | PAUSED -> ARCHIVED
```

CoworkerService owns revisions and status. Pause blocks proactive and scheduled
Coworker-originated Task admission, but an explicit owner-submitted Task may still name a
PAUSED Coworker and follows ordinary Task/Trust admission. Pause does not pause or cancel
existing Tasks. Resume revalidates eligible lead/profile bindings and trigger dependencies.
An ARCHIVED Coworker cannot be selected for new Task origin. Archive requires
no active Coworker-owned Automation and no nonterminal Coworker-originated Task; archived
identity, revisions, Tasks, and Artifacts remain readable. Coworker revision updates
affect future admission only. A primary Coworker must be explicitly cleared or changed
by WorkspaceService before archive; archive never silently rewrites Workspace.primary.

## Goal

```text
ACTIVE <-> PAUSED
ACTIVE | PAUSED -> COMPLETED
COMPLETED -> ACTIVE
ACTIVE | PAUSED | COMPLETED -> ARCHIVED
```

GoalService owns revisions/status. Only an authenticated owner command completes a Goal;
verified Task outcomes update a derived progress projection and may offer a completion
Suggestion. An owner may reopen a completed Goal with a versioned status command;
reopening does not reverse Task outcomes or prior completion history. Completion and
reopening do not change linked Tasks or Routines. Archive is terminal and preserves
history.

## Suggestion

```text
PROPOSED -> ACCEPTED | DISMISSED | EXPIRED
```

SuggestionService resolves once using expected version and current expiry. Acceptance of
a Task action creates the ordinary Task and resolves the Suggestion in one transaction.
Opening a Routine/Automation editor can resolve the Suggestion as accepted, but saving
remains an explicit command to the owning service. Dismissal and expiry are terminal;
late acceptance returns `SUGGESTION_EXPIRED` and creates no work.

Snoozing is a versioned visibility update on a `PROPOSED` Suggestion, not a status
transition. `snoozed_until` must be in the future and no later than `expires_at`; the
event is committed before the card is hidden. Expiry wins over a later snooze. Workspace
SuggestionKind preferences default to unmuted. Muting atomically updates the preference
and resolves all currently proposed Suggestions of that kind as `DISMISSED` with reason
`MUTED_KIND`; future proposals of that kind are suppressed until unmuted. Unmuting never
revives old Suggestions. An owner dismissal suppresses only the same dedupe key for 30
days; expiry and acceptance do not create a dismissal cooldown.

## DemonstrationSession

```text
CREATED -> CAPTURING <-> PAUSED -> REVIEW -> CONVERTED
    |          |          |         |
    +----------+----------+---------+-> ABORTED
```

DemonstrationSessionService records a bounded semantic interaction trace in a Resource.
`CONVERTED` means a SkillProposal was created, not that a Skill was installed or
published. Capture policy is immutable and pins maximum duration (at most 30 minutes),
actions (at most 1,000), trace bytes (at most 5 MiB), Environment class, and sensitive
region behavior. At a cap, capture pauses or moves to review; it cannot silently discard
the limit and continue. `PAUSED` preserves the trace and can resume only in the same
authorized Environment after a fresh observation. `ABORTED` and `CONVERTED` are terminal.
Raw screen/video data is not retained unless separately consented and classified under
Resource retention policy.

## Runtime

```text
PAIRING -> STARTING -> RECOVERING -> ONLINE <-> DEGRADED
                                      |             |
                                      +-> DRAINING -+
                                           ↓
                                        OFFLINE -> ONLINE
PAIRING | STARTING | RECOVERING | ONLINE | DEGRADED | DRAINING | OFFLINE -> REVOKED
```

RuntimeMesh owns paired identity, presence, and revocation. RuntimeLifecycleService owns
local daemon startup/drain and publishes readiness to RuntimeMesh. Presence expiry changes
availability; it does not itself expire a lease. Revocation rejects new authentication
and authority from that Runtime.

Every daemon process start creates a distinct `RuntimeIncarnation`:

```text
STARTING -> RECOVERING
RECOVERING -> READY | DEGRADED | STOPPING
READY <-> DEGRADED (readiness gates lost/restored)
READY | DEGRADED -> DRAINING -> STOPPING -> STOPPED
STARTING -> STOPPING on unrecoverable startup failure
```

Recovery failure leaves the incarnation DEGRADED or STOPPED with a typed blocker; it never
advertises ONLINE readiness. Suspend/wake that preserves the process keeps the incarnation
but invalidates stale observation/readiness claims until they are revalidated.
RuntimeLifecycleService owns incarnation transitions; RuntimeMesh owns the Runtime
availability projection.

## Environment

```text
NEW -> PROVISIONING -> READY | FAILED
PROVISIONING -> FAILED
READY <-> BUSY
READY | BUSY -> CHECKPOINTING -> READY | SUSPENDED | FAILED
SUSPENDED -> PROVISIONING (provider reattach/resume)
FAILED -> DESTROYING
READY | SUSPENDED -> DESTROYING
DESTROYING -> DESTROYED | FAILED
```

EnvironmentManager owns canonical state; the provider performs substrate operations.
Suspend from BUSY first stops new admission and waits for Attempt, Invocation, Effect,
control-lease and checkpoint holds to settle; the Environment remains BUSY/CHECKPOINTING
until safe. Resume enters PROVISIONING while provider identity, retained source revisions,
isolation and health are revalidated. A failed resume remains FAILED and cannot satisfy
placement. `DESTROYING` is allowed only when no authoritative Attempt needs the Environment
and required checkpoints/artifacts/effect reconciliation are preserved. Destruction
failure remains visible and retry reconciles provider identity before repeating. `DESTROYED`
is terminal; its metadata/provenance remain readable.

`lifetime`, Workspace owner, Runtime/provider identity, source/resource/network/budget
configuration, and provision digest are immutable. An owner may change a persistent
Environment's sharing scope only between `COWORKER_PRIVATE` and `WORKSPACE_SHARED`, while
it is `SUSPENDED` and has no active Attempt, Invocation, control lease, unresolved Effect,
or checkpoint hold. The change updates the matching owner field, increments Environment
version, and emits `environment.sharing_scope.changed.v1` atomically. `USER_SHARED` is not
admitted in v1. The command never resumes or attaches the Environment; later use receives
fresh placement and authority checks.

EnvironmentProvisionPreviewRecord follows `ISSUED -> CONSUMED | EXPIRED`. Issuance is
short-lived and read-only with respect to providers. Exactly one create command may
consume an unexpired record; consumption, Workspace budget reservation, PROVISIONING
Environment row, and initial domain event commit in one transaction before provider I/O.
An archive or restore invalidates outstanding previews. Expiry and cleanup cannot delete
a consumed record while its Environment retains the digest provenance.

Workspace-persistent Environments require a user-visible budget/retention policy. If the
provider cannot enforce a requested cost ceiling, the provisioning preview says so and
the user may choose a different Runtime/provider or an explicit monitor-only policy; the
system must not label a monitored budget as provider-enforced.
BudgetService accounts cumulative Environment usage in the budget's pinned currency. A
stale/unavailable observation projects `UNKNOWN`; it never becomes zero or authorizes more
host-monitored spend. At `LIMIT_REACHED`, EnvironmentManager stops new use admission and
enters the ordinary safe CHECKPOINTING/suspend path. Provider-enforced caps remain active
independently; host-monitored caps can overshoot while the monitor is offline. No implicit
budget reset or top-up occurs. In v1 the ceiling and enforcement policy are immutable
after provision; no in-place update transition exists. When the limit is reached, affected
Steps become `BLOCKED` with `BUDGET_EXCEEDED` after safe settlement. A new Environment
may be selected or provisioned, but the old Attempt's Environment binding is immutable;
continuation uses a fresh Attempt after any required state is explicitly made available as
Resources/Artifacts. Private provider state is not assumed clonable.

## EnvironmentControlLease

```text
ACTIVE(owner=AGENT, epoch n) -> ACTIVE(owner=HUMAN, epoch n+1)
ACTIVE(owner=HUMAN, epoch n) -> ACTIVE(owner=AGENT, epoch n+1)
ACTIVE -> RELEASING -> RELEASED
ACTIVE -> EXPIRED | REVOKED
```

EnvironmentManager owns input-control leases separately from Runtime ExecutionLeases.
Owner handoff is an atomic owner/owner_ref change with a strictly increasing epoch while
the lease remains ACTIVE; it is not an additional lifecycle status. Takeover stops
admission of agent input, drains or rejects in-flight input, increments the control epoch,
and grants control to the authenticated human. Returning to an Agent requires a fresh
Environment observation and drift/effect reconciliation. Commands from an earlier epoch
are rejected even if queued before takeover, and are never replayed.

## CapabilityGrant, SecretLease, Activation, and Invocation

```text
CapabilityGrant: ACTIVE -> REVOKED | EXPIRED
SecretLease: ACTIVE -> REVOKED | EXPIRED
CapabilityInvocation:
  CREATED -> DISPATCHED | CANCELLED
  DISPATCHED -> SUCCEEDED | FAILED | WAITING | INPUT_REQUIRED | AMBIGUOUS
  WAITING <-> INPUT_REQUIRED
  WAITING | INPUT_REQUIRED -> DISPATCHED | CANCEL_REQUESTED | AMBIGUOUS
  DISPATCHED -> CANCEL_REQUESTED | SUCCEEDED | FAILED | AMBIGUOUS
  CANCEL_REQUESTED -> CANCELLED | SUCCEEDED | FAILED | AMBIGUOUS
  AMBIGUOUS -> DISPATCHED | WAITING | INPUT_REQUIRED | CANCEL_REQUESTED |
               SUCCEEDED | FAILED | CANCELLED

Activation lifecycle:
STARTING -> ACTIVE -> STOPPING -> STOPPED
    |          |          +-> FAILED
    +-> FAILED +-> FAILED

CapabilityBroker/CapabilityHostSupervisor retains an Activation while its scope is active
or any linked CapabilityInvocation is nonterminal, including after the originating
AgentSession closes. STOPPING is legal only when the scope has settled and all linked
Invocations are terminal/reconciled; provider-host use references are released afterward.

Runtime-local provider continuation bindings are operational state, not replicated
Invocation/Automation fields:

```text
ProviderContinuationBinding:
  AVAILABLE -> RECONCILIATION_REQUIRED | UNAVAILABLE
  RECONCILIATION_REQUIRED -> AVAILABLE | UNAVAILABLE

ProviderInputBinding:
  AWAITING_RESPONSE -> PENDING | REJECTED | EXPIRED | CANCELLED
  PENDING -> DISPATCHED | CANCELLED
  DISPATCHED -> ACKNOWLEDGED | AMBIGUOUS | REJECTED
  ACKNOWLEDGED -> ACCEPTED | AMBIGUOUS | REJECTED
  AMBIGUOUS -> DISPATCHED | ACCEPTED | REJECTED
```

Binding transitions are owned by InvocationRunner or the trigger host's recovery
coordinator, using authenticated provider observations. `ACCEPTED`, `REJECTED`, and
`EXPIRED`, and `CANCELLED` are terminal for that provider input key. `CANCELLED` means the
local response outbox was withdrawn before dispatch; it is never inferred from a Task
cancellation after dispatch. A binding may enter `CANCELLED` only from
`AWAITING_RESPONSE` or `PENDING`. `AMBIGUOUS -> DISPATCHED` is allowed only after an
authenticated provider observation proves the same input key is still outstanding and
the negotiated provider contract makes resending the exact same response digest
duplicate-safe; it must not change the key or response. After `DISPATCHED`, uncertain
delivery must be reconciled; provider-task cancellation and input delivery are separate
facts. Expiring the local UserRequest does
not prove the provider stopped waiting: the InvocationRunner must request cancellation or
reconcile the provider task, and the ConversationTurn remains blocked until that provider
dependency is settled. An unknown provider task status maps to `UNKNOWN` and Invocation
`AMBIGUOUS`; no action or input delivery is admitted until an updated adapter or provider
reconciliation resolves it.

Activation identity (`Workspace`, scope tuple, CapabilityRef, Runtime, incarnation, and
mode) is immutable after creation. Invocation admission requires the Grant, Activation,
and Invocation scopes to match exactly and to match the AgentSession's parent
Conversation/Task/Attempt scope, Workspace, capability, and Runtime/incarnation. It also
requires the bound ConversationTurn to be the current RUNNING turn, a planning session to
match the RUNNING Task's lead binding/current spec revision, or an execution session to
match its RUNNING Attempt, pinned PlanRevision, and unexpired ACTIVE lease. A local
`CapabilityActivationHostBinding` may be removed only after
the Activation reaches FAILED or STOPPED; replicated Activation state never carries the
host ID or provider handle.

Health is an independent observation, not a lifecycle state. Each probe may record
HEALTHY, DEGRADED, UNHEALTHY, or UNKNOWN without changing Activation.status.

`CapabilityHostInstance` is Runtime-operational state distinct from a scoped Activation:

```text
STARTING -> READY <-> BUSY
STARTING | READY | BUSY -> DEGRADED | FAILED
DEGRADED -> READY only after a fresh successful probe
READY | DEGRADED -> STOPPING -> STOPPED | FAILED
```

CapabilityHostSupervisor owns LiteCowork's normalized observation and scoped use-reference
view; LiteSPM remains authoritative for package/provider process execution and its own
global process reference count. LiteCowork's activation count is derived by joining
Runtime-local HostBinding rows to nonterminal Activation records. `BUSY` means provider capacity is currently exhausted,
not that the provider is unhealthy. Expired health becomes `UNKNOWN` and is ineligible for
new work until revalidated. A zero LiteCowork activation count releases LiteCowork's
provider-use reference; it does not claim the process stopped if LiteSPM or another client
still holds it. A later start creates a new LiteCowork host instance after reconciliation.

CapabilityBroker and TrustService jointly authorize grant creation; TrustService owns
revocation/expiry decisions. CapabilityBroker owns activation lifecycle and records
activation-readiness observations independently; CapabilityHostSupervisor reports shared
host health. A revoked or expired grant is never
reactivated; issue a new grant. Deactivation does not delete Effect or Evidence history.

CapabilityBroker creates the Invocation and request digest before dispatch. Losing
authoritative provider state may move a `DISPATCHED`, `WAITING`, or `INPUT_REQUIRED`
Invocation to `AMBIGUOUS`. InvocationRunner owns provider dispatch, polling, resume,
cancellation request, result storage, and settlement.
Cancellation is cooperative; a late provider success is recorded and linked Effects are
reconciled. AMBIGUOUS is unresolved, not terminal; it can be left only after authoritative
provider state/reconciliation is observed. A Conversation-scoped Invocation is read-only
and cannot create an Effect. A pre-dispatch `CREATED` Invocation can be locally cancelled;
once dispatched, only authoritative provider state can settle cancellation. A provider
`INPUT_REQUIRED` status is quiescent only when the provider confirms it is awaiting input.

```

## ProviderCircuit

```text
CLOSED -> OPEN -> HALF_OPEN -> CLOSED
                      +------> OPEN
```

ProviderCircuitService owns a versioned call-admission circuit per Runtime/provider
identity. A rolling-window threshold opens the circuit; `open_until` controls when one
exclusive half-open probe may start. Probe success closes and resets the failure window;
failure reopens with bounded backoff. OPEN rejects new calls but does not start, restart,
or stop a process. LiteSPM owns package/provider process lifecycle; Environment and Channel
owners manage their own adapter lifecycle. Circuit transitions do not rewrite
CapabilityActivation, Invocation, Task, or Effect history.

## BudgetReservation

```text
RESERVED -> COMMITTED | RELEASED | EXPIRED
```

BudgetService owns reservations. Commit records observed/charged usage; release or expiry
returns unused capacity. All three outcomes are terminal. A further reservation is a new
record, so concurrent Attempts cannot reuse a released/expired reservation identity.

## SkillProposal

```text
DRAFT -> REVIEW -> APPROVED -> PUBLISHED
   |       |          |
   +-------+--------> REJECTED
```

SkillProposalService owns draft/redaction/review state; TrustService authorizes approval;
PUBLISHED is recorded only after LiteSPM confirms publication. A failed redaction remains
in DRAFT with `redaction_status=FAILED` and cannot enter REVIEW. REJECTED and PUBLISHED
are terminal; revisions create a new proposal/draft rather than rewriting reviewed text.

## ResourceUploadSession

```text
OPEN -> CONTENT_RECEIVED -> COMMITTED
  |             |
  +-> EXPIRED   +-> FAILED | EXPIRED
```

ResourceUploadService owns the session. A complete contiguous chunk set changes OPEN to
CONTENT_RECEIVED. Commit verifies size, media policy, and the whole-object digest before
creating a Resource and ResourceRevision; failed final verification is terminal and
requires a new upload. Identical chunk replay is idempotent; a conflicting chunk range
fails with UPLOAD_OFFSET_CONFLICT. Chunk metadata is local transfer state, not replicated
or journaled as one event per chunk; only session creation and lifecycle transitions are
DomainEvents.

## Artifact and ArtifactVersion

`TRANSIENT -> SAVED -> ARCHIVED`

ArtifactStore owns Library state. Promotion changes TRANSIENT to SAVED; archive changes SAVED to ARCHIVED and is terminal in v1. Both transitions require the expected Artifact aggregate version and increment it; archive is idempotent and emits no duplicate transition event. Existing ArtifactVersion records and authorized reads remain available after archive. ArtifactVersion is immutable and append-only. Publishing requires the expected Artifact aggregate version, assigns the next integer version, and advances current_version atomically; a concurrent publisher receives STALE_VERSION and cannot silently replace or branch another Attempt's version.

Each Artifact owns one stable `ARTIFACT` Resource. Every ArtifactVersion maps to one
ResourceRevision of that Resource, and its current head must match Artifact.current_version.
Creation/version publication commits both aggregates, revision ancestry, and their events
in one unit of work. An Artifact ResourceRef is valid Task input; a pinned revision selects
the matching ArtifactVersion. Archive changes Library visibility but does not invalidate
or remove the Artifact Resource or its immutable revisions.

## Effect and Evidence

```text
PROPOSED -> STARTED -> ACKNOWLEDGED -> OBSERVED -> VERIFIED
    |          |             |             |
    |          +-> FAILED    +-> FAILED     +-> AMBIGUOUS
    |          +-> AMBIGUOUS  +-> AMBIGUOUS
    +-> FAILED
AMBIGUOUS -> RECONCILING
RECONCILING -> OBSERVED | FAILED | AMBIGUOUS
RECONCILING -> STARTED only after confirmed-not-occurred or safe same-key idempotent retry
```

EffectService owns local Effect transitions; EffectReconciler can append a resolution
only with provider/independent evidence. A retry returns the same logical Effect to
`STARTED` only after a recorded reconciliation decision, increments `dispatch_ordinal`,
and reuses its idempotency key. Otherwise do not re-dispatch. `NEEDS_USER` is a Task/Step
outcome, never an Effect state. If a user resolves ambiguity, append the decision and
supporting evidence; do not erase the prior uncertainty. `VERIFIED` requires a
VerificationRun or approved human verification at the criterion's required assurance.

Evidence records are immutable append-only facts with a level of `REPORTED`, `OBSERVED`,
or `VERIFIED`. Higher assurance creates a new Evidence record; it does not mutate an
older record.

## VerificationRun

```text
PENDING -> RUNNING -> PASSED | FAILED | INCONCLUSIVE
```

VerifierRegistry starts the run; VerifierRunner records its result; CompletionEvaluator
consumes it. Runs have a bounded deadline; timeout settles the run as `INCONCLUSIVE` with
a typed reason. Terminal VerificationRuns are immutable. A retry is a new run. During
Task pause/cancellation, existing runs settle normally against their pinned inputs, but
CompletionEvaluator is fenced from finalizing the Task; no new run starts while the Task
is `PAUSE_REQUESTED` or `CANCEL_REQUESTED`. Runs that exceed their deadline become
`INCONCLUSIVE`, allowing the lifecycle coordinator to settle or surface another blocker.

## Approval

```text
PENDING -> APPROVED | DENIED | EXPIRED | CANCELLED
```

TrustService owns approval and SecretLease state. A SecretLease is revoked on explicit revocation or expires at its bounded expiry; either terminal state rejects further use. TrustService owns approval resolution. Terminal decisions are immutable. Approval binds
the exact action digest, target, scope, and required assurance; material action changes
require a new Approval.

An APPROVED one-time Approval is consumed by appending exactly one immutable ApprovalUse
in the same admission transaction as the authorized Effect or grant. TrustService
recomputes the actual action binding and requires it to match `Approval.action_digest`;
the use stores the canonical request digest for audit/deduplication. A capability-grant
approval is consumed by grant creation, while an operation approval is consumed by
Effect admission; one ApprovalUse names exactly one target. Unique `approval_id` rejects
replay. An unused approval may expire or be cancelled, but cannot be consumed for a
different request or resource revision.

## UserRequest and NotificationDelivery

```text
UserRequest: PENDING -> ANSWERED | DISMISSED | EXPIRED | CANCELLED
NotificationDelivery: PENDING -> SENDING -> SENT | FAILED | AMBIGUOUS | SUPPRESSED
FAILED -> PENDING (bounded retry with backoff)
AMBIGUOUS -> SENT | FAILED (only after provider reconciliation; otherwise remains AMBIGUOUS)
```

Each `SENDING` attempt pins `attempt_runtime_id` and `attempt_host_epoch` when using a
ChannelBinding. A host reassignment does not rewrite this provenance. A retry is admitted
only after the prior attempt is definitively not delivered; it then pins the current host
assignment. Only a `SENT` attempt whose Runtime/epoch still owns the current assignment
may create a reply target.

ConversationService owns UserRequest creation and resolution; an authenticated user
response is append-only and is not an Approval unless TrustService separately validates
it. A Conversation-scoped UserRequest names exactly one ConversationTurn, and the originating
AgentSession must be that turn's current session. Resolving it can continue only that
still-`WAITING_USER` turn, first moving it to `WAITING_DEPENDENCY`. Task-planning or
Attempt-scoped requests never resume a ConversationTurn. For an MCP input request, the
InvocationRunner records the response against the exact local key and waits for authoritative
provider state; an acknowledgement alone is not acceptance. Task/Attempt scope remains
`WAITING_RESOURCE`/`BLOCKED` when provider input delivery is the only available progress
path, or continues other independent Steps. The answer remains visible in Conversation
context with its request link. If the selected AgentBinding or Runtime cannot start after a
valid response, preserve the answer and settle the ConversationTurn `FAILED` with a retryable
reason; the user can retry after fixing availability.

`FORM` requests are non-sensitive typed input. `EXTERNAL_URL` is reserved for
Runtime-materialized `EXTERNAL_AUTHORIZATION` requests: only an explicit authenticated
handoff can reveal the provider URL, and the saved response is action-only. The
out-of-band page cannot grant a LiteCowork Approval. Sensitive-looking form schemas and
unsupported MCP embedded methods fail closed and do not create a deliverable response.

The response transaction checks the parent state: Conversation responses require the exact
turn still be `WAITING_USER`; Task-scoped responses are rejected while the Task is
`PAUSE_REQUESTED`, `CANCEL_REQUESTED`, or terminal. A valid response may be committed while
the Task is `PAUSED`, but its ProviderInputBinding remains queued and cannot dispatch until
explicit resume and fresh continuation authorization. UserRequestResponse is immutable;
the request may become `ANSWERED` only when its digest, responder, and timestamp match that
response record, and resolution cannot move back to `PENDING`.

An expired Conversation UserRequest rejects further responses with `USER_REQUEST_EXPIRED`.
For a provider-backed request, its Runtime-local input binding becomes `EXPIRED`, and
InvocationRunner requests provider cancellation or reconciles the task. The turn remains
`WAITING_DEPENDENCY` until the provider operation is known to be quiescent; it then settles
`FAILED` with a retryable expiry reason. A Task/Attempt-scoped request marks its Step
`BLOCKED`; TaskService sets `NEEDS_USER` only when no independent Step can proceed. Expiry
never fabricates a response or resumes an AgentSession.
NotificationService owns delivery attempts and preference evaluation. Dedupe key is
unique per logical notification. `SENT` means transport acknowledgement only, never Task
completion. Delivery retry count and backoff are bounded.

## Resource, WorkspaceRoot, and invalidation

Resource identity and immutable revisions are append-only. ResourceLocation availability
and freshness are observations owned by the WorldIndexer/provider adapter. WorkspaceRoot
transitions `ACTIVE <-> PAUSED`, `ACTIVE | PAUSED -> UNAVAILABLE`,
`UNAVAILABLE -> ACTIVE` only after root identity revalidation, and
`ACTIVE | PAUSED | UNAVAILABLE -> REVOKED`; revocation is terminal and stops future
observation/search/exposure/replication under that root. Resource changes append
InvalidationRecords for dependent artifacts and verification projections. ArtifactStore
and VerifierRunner create immutable DependencyEdges from exact pinned input ResourceRefs;
DependencyService maintains/rebuilds the reverse index and appends invalidations when a
new revision makes a consumed revision stale. Original ArtifactVersion, VerificationRun,
and Evidence records are never mutated. DependencyEdges have no update transition;
corrections require rebuilding the derived index from authoritative aggregate state/events.

ResourceRevision forms an acyclic, same-Resource ancestry DAG. New observations append a
revision with explicit parent IDs; concurrent edits remain sibling heads. The Resource's
`current_revision_id` is non-null only when the graph has exactly one head. Zero revisions
means unknown; multiple heads mean conflicted. No timestamp or Runtime priority resolves
the conflict. A pinned reference may select a branch; an unpinned reference returns
`RESOURCE_CONFLICT`. A merge appends a revision whose parents include all merged heads.
The append, parent edges, location observation, unique-head pointer update, invalidations,
and `resource.revision.observed.v1` event commit atomically. A revision-upload session
pins the Resource version and exact current parent-head set before accepting content; its
commit rechecks both and either appends one immutable revision or returns `RESOURCE_CONFLICT`.
An explicit merge names every current head. No append operation chooses a branch implicitly.

ContextDocument is a Resource classification with this state machine:

```text
ACTIVE <-> REVOKED
ACTIVE | REVOKED -> DELETION_PENDING -> DELETED
```

ResourceService owns status transitions and event writes. Revocation immediately blocks
future context resolution/attachment but retains the content. `DELETION_PENDING` is a
replicated tombstone that blocks all content reads while Core-managed blobs/indexes are
purged. The purge reconciler records one immutable receipt per required replica and exact
revision set; only after all required acknowledgements (and any provider-backed deletion
confirmation) may ResourceService commit `DELETED`. A ContextDocument with no purge targets
can complete only after the reconciler verifies that the owned replica inventory is empty.
Historical Task/Evidence references retain IDs and digests, not access to deleted bytes.
Concurrent content edits use the same Resource revision DAG and expected-version rules;
status changes and revision appends serialize on the Resource version.

## Automation and occurrence

Automation definition content is immutable by revision. An update creates AutomationRevision(n+1) and advances the mutable Automation.current_revision pointer; pause/resume/disable changes only lifecycle status. Existing occurrences retain their pinned revision.

```text
Automation: ENABLED <-> PAUSED; ENABLED | PAUSED -> DISABLED
Occurrence: PENDING -> CLAIMED -> STARTED -> COMPLETED | FAILED
            CLAIMED -> WAITING_DEPENDENCY -> STARTED | SKIPPED | FAILED
            PENDING -> SKIPPED
            CLAIMED -> PENDING only if its claim expires before Task creation
```

AutomationService owns definition state. TriggerCoordinator owns occurrence claims and
settlement. A Routine and each RoutineRevision have a separate lifecycle:

```text
Routine: ACTIVE -> ARCHIVED
RoutineRevision: append-only; edits create a new revision
```

RoutineService owns active/archive status and immutable revisions. Existing Tasks and
occurrences retain pinned revisions; an archived Routine cannot seed new work. Resuming
a paused Automation must reject an archived Routine until the Automation is rebound to
an active RoutineRevision. `DISABLED`
is terminal for an Automation definition. A due occurrence may materialize a Task while
the Task waits for its execution dependencies; `WAITING_DEPENDENCY` is distinct from a
future `PENDING` schedule. The unique
`(automation_id, trigger_id, occurrence_key)` key makes retries one logical occurrence
even when an Automation has a newer revision. Task creation, pinned Routine provenance,
and occurrence Task reference commit atomically. Every successful claim increments
`claim_epoch`; materialize/settle commands must present that epoch, so an expired claimant
cannot commit after a later claimant has taken over.

Each AutomationRevision contains one or more stable TriggerSpecs with independent `ANY`
semantics. TriggerCoordinator owns one fenced host/cursor per trigger. Cursor advancement,
delivery receipt dedupe, and occurrence creation commit atomically. Cursor identity is
`(automation_id, trigger_id)`; `active_automation_revision` is a reference, not part of
that identity. Routine lifecycle and cursor changes are journaled; host epoch and
expected cursor version fence competing observers. Hub schedules may fire
while an execution Runtime is offline; local file/process watchers are hosted by the
Runtime that can observe those resources. Misfire handling follows the TriggerSpec policy;
resume/wake revalidates watcher cursors and records gaps instead of inventing missed events.

## ExecutionLease

```text
ACTIVE -> RELEASING -> RELEASED
ACTIVE -> EXPIRED | REVOKED
```

LeaseCoordinator alone acquires, renews, releases, expires, or revokes leases. Epoch is
monotonic per Step. Acquisition binds the Attempt's exact Runtime incarnation. Renewal
requires the same authenticated Runtime/incarnation, Attempt, epoch, and valid
runtime-private fencing credential. The durable lease stores only the credential digest;
the raw credential is never replicated or returned by the Operator API. It is not
standalone authority: every use authenticates the caller and rechecks the active lease,
incarnation, epoch, expiry, and requested operation. A new daemon incarnation cannot
renew an old incarnation's lease. An expired/released/revoked lease cannot be revived; a
new owner receives a strictly higher epoch. Mediated writes validate the current fence at
the authority that commits the mutation.

## Handoff, Connection, and ChannelBinding

```text
Handoff: REQUESTED -> DRAINING_SOURCE -> CHECKPOINTING -> REPLICATING
  -> RECONCILING -> LEASE_RELEASE -> TARGET_PREPARE -> TARGET_LEASE
  -> TARGET_ATTEMPT -> COMPLETED
Any nonterminal Handoff -> FAILED with a typed reason and preserved source state.

Connection: CONNECTING -> CONNECTED -> DEGRADED -> CONNECTED
  CONNECTING | CONNECTED | DEGRADED -> REAUTH_REQUIRED | DISCONNECTED
  DISCONNECTED | REAUTH_REQUIRED -> CONNECTING

ChannelBinding: ACTIVE -> DEGRADED -> ACTIVE; ACTIVE | DEGRADED -> REVOKED

ChannelHostAssignment: ACTIVE -> DRAINING -> ACTIVE(new Runtime, higher host_epoch)
  ACTIVE -> ACTIVE(same Runtime, renewed bounded lease)
  DRAINING -> ACTIVE(same Runtime, same host_epoch) only if source lease remains valid
    and target reassignment has not committed
  ingress_continuity: CONTINUOUS -> GAP_ACCEPTED only with explicit owner confirmation;
    GAP_ACCEPTED is historical for that assignment and never silently returns to CONTINUOUS
  expired lease -> reassignment only after expiry + clock-skew safety margin

ChannelReplyTarget: ACTIVE -> CONSUMED | CLOSED | EXPIRED
```

ChannelService creates a target only after acknowledged, reply-capable delivery to a
single pending FORM UserRequest. The immutable response commit and target consumption
are atomic with the inbound ChannelEventReceipt acceptance. Invalid-schema replies do not
consume the target. Binding revocation/authority loss, prompt invalidation, and request
resolution/expiry close active targets; terminal targets never reopen. Provider message
references stay Runtime-local. RuntimeMesh owns ChannelHostAssignment and its monotonic
epoch/lease. A target pins its Runtime and host epoch; a reply is accepted only when these
match the current unexpired assignment. Reassignment makes prior targets immediately
non-authoritative, even if physical cleanup on an offline Runtime must wait. A late inbound
receipt from an old host epoch cannot commit a message, UserRequestResponse, or Task command.

RuntimeMesh owns Handoff and channel-host coordination, ConnectionService owns
account-connection metadata, and ChannelService owns channel identity/assurance binding.
A Task handoff target cannot start before source lease release/expiry and eligibility
checks pass. Channel-host reassignment requires source settlement or authoritative lease
expiry and clock-skew margin, a compatible provider offer, SecretRef placement, and a
provider-supported ingress cursor transfer or stable-ID replay window. The Hub increments
the host epoch before the target Runtime may receive work. Connection
status changes are provider-reported or owner-requested through ConnectionService.
ChannelBinding allowed-action changes increment aggregate version without changing
lifecycle status; TrustService authorizes any authority increase. New bindings have no
allowed actions until the owner grants them. Revocation is terminal.

## ChannelEventReceipt

```text
RECEIVED -> PROCESSING -> ACCEPTED | REJECTED | FAILED
```

ChannelService claims a receipt by setting PROCESSING, incrementing `claim_epoch`, and
setting a bounded claim expiry. Each committed receipt gets a per-binding sequence that
increases within its origin host epoch. An unclaimed RECEIVED row may be claimed by the current
assigned host after lease validation. The receipt's origin Runtime/host epoch is immutable;
current claim Runtime/host epoch may change only after the old claim expires or its host
epoch is authoritatively fenced by a committed assignment, and then only under the current
assignment and valid host lease. This is the only PROCESSING -> PROCESSING transition and
increments `claim_epoch`. A changed payload digest conflicts. Every completion command
presents the current claim and host epochs; late results from an expired claimant or
reassigned host are rejected. ACCEPTED, REJECTED, and FAILED are terminal. Only INBOUND,
EDIT, and DELETE provider events use this receipt; outbound delivery is tracked as a
separate Effect/Evidence outcome.

For receipt updates, the only PROCESSING -> PROCESSING transition is an expired or fenced
claim being reclaimed with `claim_epoch + 1`. Terminal settlement clears `claim_expires_at`
but preserves claimant identity/epoch for audit. The provider cursor cannot move past an
event until its receipt is durable and, for a non-authoritative Runtime, acknowledged by Hub
replication. A changed payload digest for an existing provider event ID is a conflict, not
a new receipt.

The opaque ChannelIngressCursorBinding is Runtime-local and incarnation-scoped. After a
restart it must be validated or marked RECONCILIATION_REQUIRED before polling resumes.
Receipt persistence precedes cursor advancement; a non-authoritative Runtime also waits for
the Hub replication receipt before acknowledging deferred provider ingress. If it cannot
meet the durability barrier, it pauses ingress rather than advance the cursor.

## Transition ownership and event rule

| Aggregate | Transition owner |
|---|---|
| Workspace | WorkspaceService |
| WorkspaceBackupManifest | BackupService; immutable and published only after verification |
| Conversation and messages | ConversationService |
| Task | TaskService; CompletionEvaluator supplies verification result |
| Task lead binding | TaskService; emits `task.lead_agent.changed.v1` when the versioned assignment changes; existing Attempts remain pinned |
| Step and PlanRevision promotion | TaskService; promotion requires an authorized session/Attempt |
| AutomationRevision | AutomationService (append-only creation) |
| Attempt | AttemptRunner |
| AgentSession | AgentSessionSupervisor |
| DelegationProfile | DelegationProfileService |
| Coworker | CoworkerService |
| Goal | GoalService |
| Suggestion | SuggestionService |
| DemonstrationSession | DemonstrationSessionService |
| AgentBinding | AgentBindingService |
| Runtime and RuntimeOffer | RuntimeMesh |
| RuntimeIncarnation | RuntimeLifecycleService; RuntimeMesh publishes availability projection |
| AgentHostInstance | AgentHostSupervisor; Runtime-local operational state, not replicated Task truth |
| Routine and RoutineRevision | RoutineService |
| ProviderCircuit | ProviderCircuitService |
| BudgetReservation | BudgetService |
| SkillProposal | SkillProposalService; TrustService for approval |
| ResourceUploadSession | ResourceUploadService |
| Environment | EnvironmentManager |
| EnvironmentControlLease | EnvironmentManager, with authenticated User authority for takeover |
| CapabilityGrant | TrustService through CapabilityBroker |
| CapabilityActivation | CapabilityBroker; health observations are separate from lifecycle transitions |
| CapabilityHostInstance | CapabilityHostSupervisor; Runtime-local operational view, not replicated Task truth |
| Artifact and ArtifactVersion | ArtifactStore |
| Effect | EffectService and EffectReconciler |
| Evidence | EvidenceService; verifier results are appended by VerifierRunner |
| VerificationRun | VerifierRunner |
| Approval, SecretLease, PolicyDecision | TrustService |
| Automation | AutomationService |
| AutomationOccurrence | TriggerCoordinator |
| AutomationCursor | TriggerCoordinator, with fenced TriggerHost ownership |
| ExecutionLease | LeaseCoordinator |
| Handoff | RuntimeMesh |
| Connection | ConnectionService |
| ChannelBinding and ChannelEventReceipt | ChannelService |
| ChannelHostAssignment | RuntimeMesh; lease/epoch, provider compatibility, and credential placement checks |
| ChannelIngressCursorBinding | ChannelService on the assigned Runtime; encrypted operational state only |
| ChannelReplyTarget | ChannelService; target is authorized only while its Runtime/host epoch remains current |
| ConversationTurn | ConversationService and AgentTurnCoordinator |
| CapabilityInvocation | InvocationRunner; Broker authorizes and creates |
| ApprovalUse | TrustService, atomically with authorized admission |
| UserRequest and response | ConversationService |
| NotificationPreference and NotificationDelivery | NotificationService |
| Resource, ResourceRevision, WorkspaceRoot, ResourceEdge | WorldIndexer / ResourceService |
| ResourceLocation freshness | WorldIndexer and owning provider adapter |
| DependencyEdge | ArtifactStore/VerifierRunner write; DependencyService maintains rebuildable reverse index |
| InvalidationRecord | DependencyService |

Every committed transition emits exactly one domain transition event in the same
transaction. Rejected commands emit no state-change event; diagnostic logs may record the
rejection without changing the aggregate.

## Occurrence settlement guards

TriggerCoordinator projects ordinary Task outcomes: Task COMPLETED settles occurrence
COMPLETED; terminal Task FAILED settles FAILED; terminal Task CANCELLED settles SKIPPED
with cancellation reason. A waiting occurrence with a materialized Task cannot be skipped
until cancellation/reconciliation has settled that Task. Task pause, NEEDS_USER, VERIFYING
or recoverable failure does not falsely settle an occurrence. Once started, Task blockers
are shown from Task state; occurrence STARTED is not a claim of uninterrupted execution.
