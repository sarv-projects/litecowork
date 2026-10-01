# Runtime Mesh LLD

## Purpose

Runtime Mesh coordinates multiple `litecoworkd` instances without turning the product into a distributed consensus system.

It owns runtime identity, pairing, presence, event replication, artifact replication, execution leases/fencing, remote invocation metadata, continuation/handoff coordination and runtime inventory.

## RuntimeDescriptor

```text
RuntimeDescriptor {
  runtime_id
  device_identity
  runtime_version
  platform
  architecture
  roles[]
  trust_zone
  availability
  agents[]
  capability_offers[]
  environment_offers[]
  resource_capacity
  last_seen
}
```

Descriptors are snapshots/projections, not source-of-truth for Task state.

## Runtime identity

Each Runtime has a device key pair generated on first initialization.

```text
DeviceIdentity {
  device_id
  public_key
  key_version
  issued_at
  display_name?
}
```

Private keys remain local. Rotation creates a new key version signed by the currently trusted key when possible; recovery pairing is required if old key is unavailable.

## Pairing

1. New runtime requests pairing token from Workspace Hub or trusted operator.
2. Token contains workspace ID, nonce, expiry and allowed initial roles.
3. New runtime submits device public key + token.
4. Hub validates token, creates Runtime record and trust binding.
5. Mutual TLS/session authentication starts.
6. Pairing token becomes unusable.

Tokens are single-use and short-lived.

## Presence

Runtimes send heartbeat every configurable interval.

```text
PresenceFrame {
  runtime_id
  sequence
  status
  resource_capacity
  offers_digest
  active_attempts
  timestamp
}
```

Presence TTL should be > heartbeat interval and configurable. `OFFLINE` is a projection; leases use independent expiry rules.

## Mesh service

```text
interface RuntimeMesh {
  pair(PairRuntimeRequest) -> Runtime
  revoke_runtime(RuntimeId) -> Ack
  get_runtime(RuntimeId) -> RuntimeDescriptor
  list_runtimes(WorkspaceId) -> RuntimeDescriptor[]

  publish_presence(PresenceFrame) -> Ack
  advertise_offers(RuntimeOffers) -> Ack

  acquire_lease(AcquireLeaseRequest) -> ExecutionLease
  renew_lease(RenewLeaseRequest) -> ExecutionLease
  release_lease(ReleaseLeaseRequest) -> ExecutionLease
  validate_fence(FencingToken, StepId) -> FenceDecision

  replicate_events(EventBatch) -> ReplicationAck
  fetch_events(EventCursor) -> EventBatch

  push_artifact_manifest(ArtifactManifest) -> Ack
  request_blob(BlobRequest) -> BlobTransferHandle

  request_handoff(HandoffRequest) -> HandoffPlan
  accept_handoff(AcceptHandoffRequest) -> AttemptRef
}
```

## Event replication

Replication unit is domain events, not database pages.

Per origin runtime:
- events have monotonic `origin_sequence`.
- receiver stores `(origin_runtime_id, origin_sequence)` uniquely.
- duplicate batches are safe.
- receiver ACKs highest contiguous sequence plus missing gaps.
- batches may be compressed.
- backpressure is signaled explicitly.

```text
EventCursor {
  per_origin: Map<RuntimeId, Sequence>
}
```

Each origin allocates a strictly increasing sequence in the same commit that appends an
event. Receivers deduplicate by `(origin_runtime_id, origin_sequence)` and acknowledge
only the highest contiguous sequence; gaps remain explicit until retransmitted. Request
IDs make retried commands idempotent independently of event-batch deduplication.

HLC is `(physical_ms, logical_counter, runtime_id)`. On local event creation, advance
the physical component to `max(local_wall_ms, last_physical_ms)` and increment the
logical counter if the physical component did not advance. On receipt, set physical to
the maximum of local wall, local HLC physical, and received HLC physical; set logical
according to which physical values tied (increment the maximum tied counter, otherwise
zero). Runtime ID is a deterministic tie-breaker for display only. Store receipt time
separately when operational latency matters.

Event ordering across runtimes uses HLC timestamps for presentation and causality hints;
correctness must rely on entity revisions/leases, not wall-clock order alone. HLC never
decides identity, authorization, or lease validity.

Replication is not blind event acceptance. The receiving Hub authenticates the origin,
checks that the origin Runtime/service was allowed to issue the event, validates the
aggregate's expected revision and transition owner, and checks any applicable fencing
token before applying it to canonical state. A rejected event cannot mutate the
aggregate or user projection. Workers and agents request domain commands; they never
author domain events.

## Conflict handling

Entity-specific rules:

- TaskSpecRevision: append-only. Concurrent revisions with same parent create siblings; hub chooses neither silently. UI/lead resolves by producing a new revision that names both parents in reconciliation metadata.
- PlanRevision: append-only, but only authoritative lead Attempt may mark a revision current.
- ConversationMessage: append-only; ordering projection uses HLC + origin sequence.
- ArtifactVersion: immutable; concurrent versions may coexist until one is promoted current.
- Approval resolution: first valid terminal resolution wins; later conflicting resolution rejected.
- ExecutionLease: epoch/fencing is authoritative.

No generic last-writer-wins for consequential state.

## Execution lease

Acquire:
1. verify no unexpired ACTIVE lease for Step.
2. increment epoch from max historical epoch.
3. create cryptographically random fencing token.
4. persist lease atomically with ownership projection.
5. emit `lease.acquired`.

Renew requires same runtime, attempt, epoch and token.

A new lease after expiry always has a larger epoch. Every Core-mediated mutation that
can conflict with another executor validates the active fence at the authority that
commits the mutation.

### Offline boundary

An offline Runtime may continue only an already-leased Attempt within the locally known
lease lifetime and its failover policy. It cannot acquire or renew a Hub lease while
disconnected. Without an online fencing authority, it must not dispatch new consequential
remote mutations. Local-only computation may continue in an isolated Environment and
queue immutable outputs/events. An operation that can affect a shared/external resource
after lease expiry is not automatically failover-safe unless the target provider enforces
fencing or the operation has a stable idempotency/reconciliation contract.

The Hub does not grant a replacement owner solely because a heartbeat expired. It waits
for lease expiry plus the configured clock-skew safety margin, reconciles open Effects,
and checks the FailoverClass. A cached token cannot authorize Hub-mediated writes after a
higher epoch is committed.

## Handoff

```text
HandoffRequest {
  task_id
  step_id
  source_attempt_id
  source_runtime_id
  preferred_target_runtime_id?
  reason
}
```

Handoff phases:

```text
REQUESTED
DRAINING_SOURCE
CHECKPOINTING
REPLICATING
RECONCILING
LEASE_RELEASE
TARGET_PREPARE
TARGET_LEASE
TARGET_ATTEMPT
COMPLETED
FAILED
```

Target Attempt must not start before source lease is released/expired and required
artifacts/checkpoint are available. Each phase transition is persisted on a Handoff and
emits an event. A failed handoff preserves the source Task/checkpoint; it never implies
that ownership moved. Retry creates a new Handoff after both Runtimes are re-evaluated.

## Unexpected runtime loss

1. presence becomes stale.
2. wait for lease expiry; do not infer safety from heartbeat alone.
3. mark Attempt uncertain/abandoned after ownership is lost.
4. reconcile open Effects.
5. compute ContinuationEligibility.
6. create fresh Attempt only for SAFE_PORTABLE/REPLAYABLE work.
7. otherwise wait for user/runtime.

## Artifact replication

Artifacts are content-addressed by digest.

Transfer protocol:
- exchange manifest first.
- skip blobs already present.
- chunk large blobs.
- verify chunk digest and final digest.
- resumable by chunk index.
- never mark ArtifactVersion available on target until full digest verifies.

## Hub failure

v1 has one logical authoritative Hub. If unavailable:
- local tasks may continue only according to local execution policy and cached authority.
- no new cross-runtime leases requiring hub coordination.
- cross-device sync pauses.
- hub-hosted triggers/channels pause.
- runtimes reconnect and reconcile when hub returns.

Do not elect a new Hub automatically in v1.

On reconnect, Runtimes exchange per-origin cursors, send missing immutable events and
artifact manifests, and apply only events that pass current origin authorization,
revision, and fencing checks. Concurrent TaskSpec revisions remain siblings for explicit
resolution; no last-writer-wins rule applies to consequential state.
