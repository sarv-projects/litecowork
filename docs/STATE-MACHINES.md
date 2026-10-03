# State Machines and Transition Ownership

State transitions are domain rules, not UI conventions. A command validates the current
state and expected version, applies one legal transition, appends its event, and updates
the current-state record in one transaction. An invalid transition returns a typed
conflict and writes no transition event. UI projections never cause transitions.

## Workspace

```text
ACTIVE -> ARCHIVED
```

WorkspaceService alone archives a Workspace. Archive is allowed only when every Task is terminal and every Automation is DISABLED. The transition is terminal in v1. Reads remain available; existing authorized Artifact/Resource downloads remain available. Every domain mutation is rejected, including Task changes, capability grants/activation, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel materialization. Archiving does not delete data or erase copies already replicated to another Runtime. A replication-policy change is an ACTIVE -> ACTIVE versioned update; it affects future transfers and never silently deletes existing remote copies.

## Task

```text
DRAFT -> READY -> RUNNING -> VERIFYING -> COMPLETED
           |         |            |  |  |  |
           |         |            |  |  |  +-> FAILED
           |         |            |  |  +----> BLOCKED
           |         |            |  +-------> NEEDS_USER
           |         |            +----------> INCOMPLETE
           |         |                         -> RUNNING (after recovery)
           |         +-> WAITING_USER -> RUNNING
           |         +-> BLOCKED -> RUNNING
           +-> CANCEL_REQUESTED -> CANCELLED
RUNNING -> CANCEL_REQUESTED -> CANCELLED
```

- `DRAFT -> READY`: TaskService after required intent fields validate.
- `READY -> RUNNING`: TaskService after the authorized lead LEAD_PLANNING AgentSession becomes ready. This planning phase has no Step, Attempt, ExecutionLease, or Environment. A Step Attempt may start only after a PlanRevision is accepted and its Steps are materialized.
- `RUNNING -> VERIFYING`: TaskService accepts a completion proposal; the proposal is
  not evidence of completion.
- `VERIFYING -> COMPLETED`: CompletionEvaluator recommends; TaskService commits only
  when every mandatory criterion has sufficient evidence, required children are settled,
  approvals are resolved, and no Effect is ambiguous.
- `VERIFYING -> RUNNING | INCOMPLETE | BLOCKED | NEEDS_USER | FAILED`: TaskService from
  verification and recovery results.
- `WAITING_USER -> RUNNING | READY`: TaskService after a valid user response/approval.
- `BLOCKED -> RUNNING | READY`: TaskService after the blocking resource/policy condition
  is resolved and placement is revalidated.
- `NEEDS_USER | INCOMPLETE -> RUNNING | READY`: TaskService after explicit recovery or
  revised requirements.
- `CANCEL_REQUESTED -> CANCELLED`: TaskService only after host Attempts settle and
  required open Effects are reconciled. If an Effect remains ambiguous, remain paused in
  `NEEDS_USER` with cancellation intent recorded.
- `COMPLETED`, `FAILED`, and `CANCELLED` are terminal. Follow-up work creates a new Task.

An unresolved approval is represented by Task `WAITING_USER` or Attempt
`WAITING_APPROVAL`, according to whether the whole Task is paused. A resource wait is
represented by Attempt `WAITING_RESOURCE`; the Task is `BLOCKED` only when no other
required Step can proceed.

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

TaskService owns Step transitions. `READY` requires all dependencies complete, inputs available, policy satisfied, and a feasible placement. A Step may have sequential Attempts; only its active lease's Attempt is authoritative. `WAITING_USER` pauses the Step for a user decision; `BLOCKED` means a required resource/policy condition prevents progress. A retryable `FAILED` Step may return to READY after explicit recovery. Replanning preserves completed Steps and marks obsolete unstarted Steps SUPERSEDED. COMPLETED, CANCELLED, and SUPERSEDED are terminal.

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
LiteCowork-mediated mutations. `COMPLETED` means the worker settled this try; the Task
may still be unverified. `ABANDONED` means the Attempt lost authority or its Runtime/
session disappeared. A replacement try gets a new Attempt ID. A same-Runtime session
restart may retain the Attempt only while the same lease remains valid and its checkpoint
is consistent.

## AgentSession

```text
STARTING -> ACTIVE -> INTERRUPTING -> ACTIVE
    |         |  |         +-> LOST
    |         |  +-> CLOSING -> CLOSED
    +--------> LOST
```

AgentSessionSupervisor owns session states. LEAD_PLANNING sessions are Task-scoped and have no Attempt; STEP_EXECUTION sessions reference exactly one Attempt. A lost planning session does not fail its Task by itself. A closed/lost session cannot return to ACTIVE; replacement creates a new session or uses an explicitly supported native resume handle.

## AgentBinding

```text
DISABLED -> ENABLED -> DISABLED
```

AgentBindingService owns enablement and version checks. A new binding is DISABLED.
Disabling immediately prevents new planning/Attempt admission; sessions already admitted
remain pinned and settle under their current lifecycle, lease, and Effect rules. A
disabled binding can be enabled again after TrustService and runtime eligibility checks.

## Runtime

```text
PAIRING -> ONLINE <-> DEGRADED
             |  \       |
             |   -> DRAINING -> OFFLINE
             +-------------> OFFLINE -> ONLINE
OFFLINE | ONLINE | DEGRADED -> REVOKED
```

RuntimeMesh owns identity, pairing, presence, draining, and revocation. Presence expiry
changes availability; it does not itself expire a lease. Revocation rejects new
authentication and authority from that Runtime.

## Environment

```text
NEW -> PROVISIONING -> READY <-> BUSY
          |              |       |
          +-> FAILED     +-> SUSPENDED -> READY
                         +-> CHECKPOINTING -> READY | SUSPENDED | FAILED
READY | SUSPENDED | FAILED -> DESTROYING -> DESTROYED
```

EnvironmentManager owns canonical state; the provider performs substrate operations.
`DESTROYING` is allowed only when no authoritative Attempt needs the Environment and
required checkpoints/artifacts/effect reconciliation are preserved. `DESTROYED` is
terminal.

## CapabilityGrant and CapabilityActivation

```text
CapabilityGrant: ACTIVE -> REVOKED | EXPIRED
SecretLease: ACTIVE -> REVOKED | EXPIRED

Activation lifecycle:
STARTING -> ACTIVE -> STOPPING -> STOPPED
    |          |          +-> FAILED
    +-> FAILED +-> FAILED

Health is an independent observation, not a lifecycle state. Each probe may record
HEALTHY, DEGRADED, UNHEALTHY, or UNKNOWN without changing Activation.status.
```

CapabilityBroker and TrustService jointly authorize grant creation; TrustService owns
revocation/expiry decisions. CapabilityBroker owns activation lifecycle and records provider-health observations independently. A revoked
or expired grant is never reactivated; issue a new grant. Deactivation does not delete
Effect or Evidence history.

## Artifact and ArtifactVersion

`TRANSIENT -> SAVED -> ARCHIVED`

ArtifactStore owns Library state. Promotion changes TRANSIENT to SAVED; archive changes SAVED to ARCHIVED and is terminal in v1. Both transitions require the expected Artifact aggregate version and increment it; archive is idempotent and emits no duplicate transition event. Existing ArtifactVersion records and authorized reads remain available after archive. ArtifactVersion is immutable and append-only. Publishing requires the expected Artifact aggregate version, assigns the next integer version, and advances current_version atomically; a concurrent publisher receives STALE_VERSION and cannot silently replace or branch another Attempt's version.

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
consumes it. Terminal VerificationRuns are immutable. A retry is a new run.

## Approval

```text
PENDING -> APPROVED | DENIED | EXPIRED | CANCELLED
```

TrustService owns approval and SecretLease state. A SecretLease is revoked on explicit revocation or expires at its bounded expiry; either terminal state rejects further use. TrustService owns approval resolution. Terminal decisions are immutable. Approval binds
the exact action digest, target, scope, and required assurance; material action changes
require a new Approval.

## Automation and occurrence

Automation definition content is immutable by revision. An update creates AutomationRevision(n+1) and advances the mutable Automation.current_revision pointer; pause/resume/disable changes only lifecycle status. Existing occurrences retain their pinned revision.

```text
Automation: ENABLED <-> PAUSED; ENABLED | PAUSED -> DISABLED
Occurrence: PENDING -> CLAIMED -> STARTED -> COMPLETED | FAILED
            PENDING -> SKIPPED
            CLAIMED -> PENDING only if its claim expires before Task creation
```

AutomationService owns definition state. TriggerCoordinator owns occurrence claims and
settlement. `DISABLED` is terminal for that definition. The unique
`(automation_id, occurrence_key)` key makes retries one logical occurrence even when the Automation has a newer revision; Task creation
and the occurrence's Task reference are committed atomically. Every successful claim increments
`claim_epoch`; materialize/settle commands must present that epoch, so an expired claimant
cannot commit after a later claimant has taken over.

## ExecutionLease

```text
ACTIVE -> RELEASING -> RELEASED
ACTIVE -> EXPIRED | REVOKED
```

LeaseCoordinator alone acquires, renews, releases, expires, or revokes leases. Epoch is
monotonic per Step. Renewal requires the same Runtime, Attempt, epoch, and fencing token.
An expired/released/revoked lease cannot be revived. A new owner receives a strictly
higher epoch. Mediated writes validate the current token at the authority that commits
the mutation.

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
```

RuntimeMesh owns Handoff coordination, ConnectionService owns account-connection
metadata, and ChannelService owns channel identity/assurance binding. A handoff target
cannot start before source lease release/expiry and eligibility checks pass.
Connection status changes are provider-reported or owner-requested through
ConnectionService. ChannelBinding allowed-action changes increment aggregate version
without changing lifecycle status; TrustService authorizes any authority increase.
New bindings have no allowed actions until the owner grants them. Revocation is terminal.

## ChannelEventReceipt

```text
RECEIVED -> PROCESSING -> ACCEPTED | REJECTED | FAILED
```

ChannelService claims a receipt by setting PROCESSING, incrementing `claim_epoch`, and setting a bounded claim expiry. A worker may reclaim an expired PROCESSING receipt without creating a second receipt. Every completion command must present the current epoch; late results from an expired claimant are rejected. ACCEPTED, REJECTED, and FAILED are terminal. Only INBOUND, EDIT, and DELETE provider events use this receipt; outbound delivery is tracked as a separate Effect/Evidence outcome.

## Transition ownership and event rule

| Aggregate | Transition owner |
|---|---|
| Workspace | WorkspaceService |
| Conversation and messages | ConversationService |
| Task | TaskService; CompletionEvaluator supplies verification result |
| Task lead binding | TaskService; emits `task.lead_agent.changed.v1` when the versioned assignment changes; existing Attempts remain pinned |
| Step and PlanRevision promotion | TaskService; promotion requires an authorized session/Attempt |
| AutomationRevision | AutomationService (append-only creation) |
| Attempt | AttemptRunner |
| AgentSession | AgentSessionSupervisor |
| AgentBinding | AgentBindingService |
| Runtime and RuntimeOffer | RuntimeMesh |
| Environment | EnvironmentManager |
| CapabilityGrant | TrustService through CapabilityBroker |
| CapabilityActivation | CapabilityBroker; health observations are separate from lifecycle transitions |
| Artifact and ArtifactVersion | ArtifactStore |
| Effect | EffectService and EffectReconciler |
| Evidence | EvidenceService; verifier results are appended by VerifierRunner |
| VerificationRun | VerifierRunner |
| Approval, SecretLease, PolicyDecision | TrustService |
| Automation | AutomationService |
| AutomationOccurrence | TriggerCoordinator |
| ExecutionLease | LeaseCoordinator |
| Handoff | RuntimeMesh |
| Connection | ConnectionService |
| ChannelBinding and ChannelEventReceipt | ChannelService |

Every committed transition emits exactly one domain transition event in the same
transaction. Rejected commands emit no state-change event; diagnostic logs may record the
rejection without changing the aggregate.
