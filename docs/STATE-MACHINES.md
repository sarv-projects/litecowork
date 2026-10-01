# State Machines and Transition Ownership

State transitions are domain rules, not UI conventions. A command validates the current
state and expected version, applies one legal transition, appends its event, and updates
the current-state record in one transaction. An invalid transition returns a typed
conflict and writes no transition event. UI projections never cause transitions.

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
- `READY -> RUNNING`: TaskService when an eligible Attempt acquires a lease and starts.
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
             |        |  |         |  +----> INCOMPLETE
             |        |  |         +-------> FAILED
             |        |  +-----------------> BLOCKED -> READY
             |        +--------------------> FAILED
             +-----------------------------> SUPERSEDED
RUNNING -> CANCEL_REQUESTED -> CANCELLED
```

TaskService owns Step transitions. `READY` requires all dependencies complete, inputs
available, policy satisfied, and a feasible placement. A Step may have sequential
Attempts; only its active lease's Attempt is authoritative. Replanning preserves
completed Steps and marks obsolete unstarted Steps `SUPERSEDED`.

## Attempt

```text
CREATED -> PREPARING -> RUNNING -> CHECKPOINTING -> RUNNING
              |           |  |  |  |  |  |
              |           |  |  |  |  |  +-> ABANDONED
              |           |  |  |  |  +----> FAILED
              |           |  |  |  +-------> WAITING_RESOURCE -> RUNNING
              |           |  |  +----------> WAITING_APPROVAL -> RUNNING
              |           |  +-------------> COMPLETED
              |           +----------------> FAILED | ABANDONED
              +----------------------------> FAILED
RUNNING -> CANCEL_REQUESTED -> CANCELLED
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

AgentAdapterSupervisor owns session states. A lost session does not fail its Task by
itself. A closed/lost session cannot return to ACTIVE; resume creates a new session or
uses an explicitly supported native resume handle.

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

Activation: STARTING -> HEALTHY <-> DEGRADED
                  |          |       |
                  +-> FAILED +-> STOPPING -> STOPPED
                            +-> FAILED
```

CapabilityBroker and TrustService jointly authorize grant creation; TrustService owns
revocation/expiry decisions. CapabilityBroker owns activation health/lifecycle. A revoked
or expired grant is never reactivated; issue a new grant. Deactivation does not delete
Effect or Evidence history.

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

TrustService owns approval resolution. Terminal decisions are immutable. Approval binds
the exact action digest, target, scope, and required assurance; material action changes
require a new Approval.

## Automation and occurrence

```text
Automation: ENABLED <-> PAUSED; ENABLED | PAUSED -> DISABLED
Occurrence: PENDING -> CLAIMED -> STARTED -> COMPLETED | FAILED
            PENDING -> SKIPPED
            CLAIMED -> PENDING only if its claim expires before Task creation
```

AutomationService owns definition state. TriggerCoordinator owns occurrence claims and
settlement. `DISABLED` is terminal for that definition. The unique
`(automation_id, scheduled_key)` key makes retries one logical occurrence; Task creation
and the occurrence's Task reference are committed atomically.

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

ChannelBinding: ACTIVE -> DEGRADED -> ACTIVE; ACTIVE | DEGRADED -> REVOKED
```

RuntimeMesh owns Handoff coordination, ConnectionService owns account-connection
metadata, and ChannelService owns channel identity/assurance binding. A handoff target
cannot start before source lease release/expiry and eligibility checks pass.

## Transition ownership and event rule

| Aggregate | Transition owner |
|---|---|
| Conversation and messages | ConversationService |
| Task | TaskService; CompletionEvaluator supplies verification result |
| Step and PlanRevision promotion | TaskService |
| Attempt | AttemptRunner |
| AgentSession | AgentAdapterSupervisor |
| Runtime and RuntimeOffer | RuntimeMesh |
| Environment | EnvironmentManager |
| CapabilityGrant | TrustService through CapabilityBroker |
| CapabilityActivation | CapabilityBroker |
| Effect | EffectService and EffectReconciler |
| Evidence | EvidenceService; verifier results are appended by VerifierRunner |
| VerificationRun | VerifierRunner |
| Approval, SecretLease, PolicyDecision | TrustService |
| Automation | AutomationService |
| AutomationOccurrence | TriggerCoordinator |
| ExecutionLease | LeaseCoordinator |
| Handoff | RuntimeMesh |
| Connection | ConnectionService |
| ChannelBinding | ChannelService |

Every committed transition emits exactly one domain transition event in the same
transaction. Rejected commands emit no state-change event; diagnostic logs may record the
rejection without changing the aggregate.
