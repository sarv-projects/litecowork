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
  startup_policy
  current_incarnation_id?
  offers: RuntimeOffer[]
  resource_capacity
  last_seen
}
```

Descriptors are snapshots/projections, not source-of-truth for Task state.
`offers` is the single bounded inventory projection, typed by `offer_kind` and carrying
its own `observed_at`/`expires_at`. AgentEndpoint offers, capability-provider offers,
environment-provider offers, channel adapters, and trigger providers use the same
availability envelope; there is no separate CapabilityOffer truth. AgentProfile and
AgentEndpoint identities are stable; their RuntimeOffer observations and local
AgentEndpointBindings are incarnation-scoped and can expire independently of Runtime
identity or Workspace bindings.
For `offer_kind = AGENT_ENDPOINT`, `offer_ref` is the stable `AgentEndpointId`; the offer
contains readiness/compatibility only, while the local binding resolves the executable or
remote endpoint locator.

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

## Runtime incarnation registry

Every daemon start creates a new `RuntimeIncarnationId` under its stable RuntimeId. Before
that process may publish presence, offers, AgentSessions, CapabilityActivations, or other
records containing the incarnation ID, it registers the authenticated incarnation with the
Workspace Hub. The Hub durably stores and distributes the compact `RuntimeIncarnation`
record to authorized Workspace peers; this is Mesh control metadata, not a Task-domain
event. Runtime lifecycle state updates are versioned and authenticated by the paired device
key. A duplicate identical registration is idempotent; reuse of an incarnation ID with
different identity fields or a stale state version is rejected. A Runtime with an
unregistered incarnation cannot publish ONLINE presence or work events.

The shared registry contains RuntimeId, incarnation ID, process start time, LiteCowork
version, unclean-recovery flag, lifecycle state, and state version. OS boot IDs and local
diagnostics are held only in `RuntimeIncarnationLocalObservation`; they are neither
distributed nor included in Workspace backups. Registry entries remain available while any
durable event, session, activation, Environment, lease/audit record, or backup cursor
references them. Peers apply aggregate state that names an incarnation only after its
authenticated registry record is present. This ordering makes incarnation foreign keys
valid without treating a stale presence frame as proof of current execution authority.

## Pairing

1. New runtime requests pairing token from Workspace Hub or trusted operator.
2. Token contains workspace ID, nonce, expiry and allowed initial roles.
3. New runtime submits device public key + token.
4. Hub validates token, creates Runtime record and trust binding.
5. Mutual TLS/session authentication starts.
6. Pairing token becomes unusable.

Tokens are single-use and short-lived. The Hub chooses expiry under deployment policy;
an operator request may ask for a shorter lifetime but cannot extend the policy maximum.
The bearer token is returned once to the authenticated Operator surface, stored only as a
verifier/digest, and excluded from domain events, logs, inventory, and replication. A
successful pairing atomically consumes it; expired/replayed tokens are rejected.

## Presence

Runtimes send heartbeat every configurable interval.

```text
PresenceFrame {
  runtime_id
  runtime_incarnation_id
  sequence
  status
  resource_capacity
  offers_digest
  active_attempts
  timestamp
}
```

Presence frames bind sequence numbers to the authenticated Runtime incarnation. A new
incarnation resets its frame sequence; frames from an earlier incarnation cannot restore
readiness or revive old handles. Presence TTL should be > heartbeat interval and configurable. `OFFLINE` is a projection; leases use independent expiry rules.

## Mesh service

```text
interface RuntimeMesh {
  pair(PairRuntimeRequest) -> Runtime
  revoke_runtime(RuntimeId) -> Ack
  get_runtime(RuntimeId) -> RuntimeDescriptor
  list_runtimes(WorkspaceId) -> RuntimeDescriptor[]

  register_incarnation(RuntimeIncarnationRegistration) -> RuntimeIncarnation
  publish_incarnation_state(RuntimeIncarnationStateUpdate) -> Ack

  publish_presence(PresenceFrame) -> Ack
  advertise_offers(RuntimeOffers) -> Ack

  acquire_lease(AcquireLeaseRequest) -> ExecutionLeaseGrant
  renew_lease(RenewLeaseRequest) -> LeaseRenewalResult
  release_lease(ReleaseLeaseRequest) -> ExecutionLease
  validate_fence(FencingCredential, StepId, AuthenticatedRuntime) -> FenceDecision

  assign_channel_host(AssignChannelHostRequest) -> ChannelHostLeaseGrant
  renew_channel_host_lease(RenewChannelHostLeaseRequest) -> ChannelHostLeaseGrant
  release_channel_host_lease(ReleaseChannelHostLeaseRequest) -> ChannelHostAssignment

  replicate_events(EventBatch) -> ReplicationAck
  fetch_events(EventCursor) -> EventBatch

  push_artifact_manifest(ArtifactManifest) -> Ack
  request_blob(BlobRequest) -> BlobTransferHandle

  request_handoff(HandoffRequest) -> HandoffPlan
  accept_handoff(AcceptHandoffRequest) -> AttemptRef
}
```

Lease request/response values are internal Mesh contracts, not Operator API schemas:

```text
AcquireLeaseRequest {
  request_id
  task_id
  step_id
  attempt_id
  runtime_id
  runtime_incarnation_id
}

RenewLeaseRequest {
  request_id
  lease_id
  task_id
  step_id
  attempt_id
  runtime_id
  runtime_incarnation_id
  epoch
  expected_lease_version
  credential: FencingCredential
}

LeaseRenewalResult {
  lease: ExecutionLease
  renewed: boolean
}

ReleaseLeaseRequest {
  request_id
  lease_id
  runtime_id
  runtime_incarnation_id
  expected_lease_version
  credential: FencingCredential
}

ExecutionLeaseGrant {
  lease: ExecutionLease       # durable record contains credential digest only
  credential: FencingCredential # ephemeral SecretBytes, returned only over authenticated Mesh
}

FencingCredential {
  opaque_value: SecretBytes
  issuer_key_version
  workspace_id
  lease_id
  task_id
  step_id
  attempt_id
  runtime_id
  runtime_incarnation_id
  epoch
}
```

`CredentialIssuer` derives the opaque value with domain-separated HMAC-SHA-256 from a
versioned issuer key and the immutable lease identity `(workspace_id, lease_id, task_id,
step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch)`. It serializes the domain
tag, key version, and UTF-8 IDs with an unambiguous length-prefixed canonical encoding
before HMAC. The issuer key is held in an
OS keystore/HSM, never SQLite or Workspace backup. The lease stores `issuer_key_version`
and `SHA-256(opaque_value)`, not the value. An issuer key version remains available until
all leases and deduplication retries that reference it are terminal/expired. If the key is
lost, fail closed: reconcile affected work, expire/revoke its leases, and issue new epochs;
never reconstruct authority from a backup alone.

Credential validity is checked against the authoritative lease's current state and
`expires_at`; the opaque credential is stable within one lease epoch and does not embed an
expiry. Renewal therefore extends the authoritative expiry without rotating the secret.
The enforcing provider receives the renewed lease view over authenticated private control.
If that response is lost, the provider keeps its last known earlier deadline and stops
early; the Runtime retries the same `request_id` and reads the committed result. No work
continues beyond the last confirmed expiry.

Acquire, renew, and release are idempotent by RequestId scoped to the authenticated
Runtime/incarnation. The same ID and request digest returns the original committed result;
reuse with a different digest is a conflict. Deduplication retains the non-secret lease
view only, long enough to cover the lease lifetime and retry horizon. A repeated acquire
can re-derive and return the same ephemeral credential from its key version; raw values
are never cached in `request_dedup`. Renewal returns only the lease view; the credential
does not change within an epoch. Credentials are never included in a `DomainEvent` or
projection, and are never delivered to an Agent. Provider adapters receive them through
authenticated private control and redact them from diagnostics. Environment-control
credentials follow the same derivation using the control-lease identity and owner epoch;
takeover advances the epoch and derives a different credential. Operator clients use
authenticated identity plus `expected_control_epoch` and see only an
`EnvironmentControlLeaseView`.

## Event replication

Replication unit is domain events, not database pages.

Every DomainEvent carries `aggregate_state_ref` for the complete post-transition record
at its mandatory `entity_revision`. For an eligible event, the Mesh authenticates and
persists its envelope, then obtains and verifies the content-addressed state blob before
aggregate application. The receiving Hub validates the blob digest, Workspace
authorization, entity/revision/schema binding, and event authority before advancing that
aggregate. If the blob is temporarily unavailable, the received event remains pending;
it may not be applied from its partial payload. If policy excludes the event, the source
sends only a `POLICY_OMITTED` receipt. This event-level state blob is distinct from
periodic aggregate snapshots and Artifact content transfers.

Per Workspace and origin Runtime:
- events have a strictly increasing `origin_sequence` allocated within that Workspace.
- receiver stores `(workspace_id, origin_runtime_id, origin_sequence)` uniquely.
- duplicate batches are safe.
- receiver ACKs the highest contiguous **durably received transfer position** plus missing
  gaps; this is not an aggregate-application cursor.
- batches may be compressed.
- backpressure is signaled explicitly.

Each sequence is resolved by exactly one authenticated `ReplicationReceipt`:

```text
ReplicationReceipt {
  workspace_id
  receiver_runtime_id
  origin_runtime_id
  origin_sequence
  disposition: EVENT_STORED | POLICY_OMITTED
  event_id?                 # present only for EVENT_STORED
  envelope_digest?          # present only for EVENT_STORED
  policy_revision           # source policy used to make this disposition
  omission_commitment?      # opaque keyed commitment; present only for POLICY_OMITTED
  received_at
}
```

`receiver_runtime_id` names the Runtime whose durable receipt/application state is
represented; an origin may keep a copy of the acknowledgement for retention decisions.
`origin_runtime_id` identifies the Runtime that originally allocated and authenticated the
event, even when another Runtime relays it. An `EVENT_STORED` receipt means the
authenticated envelope has been persisted and can be re-fetched; it does not claim that the aggregate projection was applied. A
`POLICY_OMITTED` receipt contains no event type, entity ID, payload, or blob locator. It is
authenticated by the origin Runtime and advances only the Workspace-scoped transfer
cursor. The marker reveals that one sequence position was omitted; that count/timing side
channel is an explicit residual metadata leak. Receipts are immutable and deduplicated by
their full Workspace/receiver/origin/sequence key.

Aggregate application is tracked separately by `(workspace, entity_type, entity_id)` and
revision. A received event is applied only when its verified state blob and transition
authority are valid and the aggregate revision is contiguous. A revision gap, including
one caused by a policy omission or expired history, is stored as pending and marks that
aggregate `SNAPSHOT_REQUIRED`; unrelated aggregates continue. The source can bootstrap an
authorized aggregate snapshot at revision N, after which the receiver applies eligible
events above N. If current policy does not authorize that snapshot, the aggregate remains
unavailable on that peer. When policy later broadens, snapshot bootstrap establishes
current state; retained authorized history may be backfilled separately for audit. A
snapshot never authorizes replay of an Effect or command.

```text
EventCursor {
  workspace_id
  per_origin: Map<RuntimeId, Sequence>
}
```

Each origin allocates a strictly increasing sequence per Workspace in the same commit that
appends an event. Receivers deduplicate by `(workspace_id, origin_runtime_id,
origin_sequence)` and acknowledge only the highest contiguous receipt position; gaps
remain explicit until the event or an authenticated policy-omission receipt arrives.
Request IDs make retried commands idempotent independently of event-batch deduplication.

### Runtime-private provider input and URL handoff

Raw MCP input envelopes, provider task/input keys, and URL-mode sign-in URLs remain in the
encrypted local binding on the Runtime that owns the Invocation. They are excluded from
Workspace events, aggregate snapshots, replication queues, backups, and ordinary Operator
projections. When an owner explicitly opens a pending URL-mode UserRequest from another
device, the Hub forwards a live authenticated request to the source Runtime and relays the
no-store response to that Operator over its authenticated transport. This relay is
transient in memory: it is not queued for offline delivery, written to a Mesh receipt, or
stored in an idempotency response. If the origin Runtime is unavailable, opening the
handoff fails; the URL is never copied into shared state to make it available later.

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

- Workspace instruction revisions: Hub-authoritative append-only revisions. Updates use
  Workspace `If-Match` and must parent the current head; stale offline/device writes are
  retained as pending intents and rejected for explicit rebase/merge on reconnect. They
  never replace the current pointer by HLC or last-writer-wins.
- TaskSpecRevision: append-only. Concurrent revisions with same parent create siblings; hub chooses neither silently. UI/lead resolves by producing a new revision that names both parents in reconciliation metadata.
- PlanRevision: append-only. A current authorized TASK_PLANNING session or currently leased lead execution Attempt may propose a revision; TaskService alone validates and promotes it against the current TaskSpecRevision.
- ConversationMessage: append-only; ordering projection uses HLC + origin sequence.
- ArtifactVersion: immutable and sequential per Artifact. Concurrent publishers use Artifact aggregate `version`; one commit wins, stale publishers must re-read and explicitly rebase or create a separate Artifact. There is no implicit branch or current-version promotion.
- Approval resolution: first valid terminal resolution wins; later conflicting resolution rejected.
- ExecutionLease: epoch/fencing is authoritative.
- CapabilityInvocation: append-only request digest and provider lifecycle. Duplicate
  replication is idempotent by InvocationId/provider sequence; conflicting terminal
  provider observations are retained as evidence and escalated rather than last-writer-wins.
- mutable aggregate commands created offline are pending intents, not shared committed
  truth. On reconnect, stale expected versions are rejected and re-evaluated by their
  owning service. TaskSpecRevision siblings are preserved for explicit resolution;
  conflicting approval, lease, grant, policy, or Artifact publication commands never
  resolve by wall-clock order.

No generic last-writer-wins for consequential state.

## Execution lease

Acquire:
1. verify no unexpired ACTIVE lease for Step.
2. increment epoch from max historical epoch.
3. create a cryptographically pseudorandom, HMAC-derived fencing credential bound to the
   exact Runtime incarnation, Attempt, Step, and epoch.
4. persist the lease and only the credential digest atomically with ownership projection.
5. emit `lease.acquired`.

The credential is returned only over authenticated Mesh control transport to the owning
Runtime. It is delivered to the in-process lease client/provider boundary through a
private channel; the Agent, Operator, event payload, aggregate-state blob, logs, and
Workspace backup never receive the raw credential. The lease record and replicated
state contain only `fencing_token_digest`. Credential use also requires authenticated
Runtime identity and an active lease check, so possession alone is not sufficient.

Renew requires the same authenticated Runtime ID and current Runtime incarnation, Attempt,
epoch, credential, request ID, and expected lease version. Renewal preserves the credential
within the epoch and extends only authoritative lease expiry. A new daemon incarnation cannot renew a prior incarnation's lease;
it waits for authoritative release/expiry and Effect reconciliation, then creates a fresh
Attempt with a new lease epoch if continuation is eligible. An AgentSession may be
replaced without replacing the Attempt only while remaining inside the same Runtime
incarnation and retaining the same valid lease.

A new lease after expiry always has a larger epoch. Every Core-mediated mutation that
can conflict with another executor validates the active fence at the authority that
commits the mutation. The authority compares the credential digest and authenticated
caller against the current lease; a stale incarnation or epoch is rejected even if an
old credential remains in a provider process.

## Channel host assignment and fencing

RuntimeMesh assigns each active ChannelBinding to exactly one Runtime through a durable
`ChannelHostAssignment`. The Workspace Hub is the assignment authority. Assignment
records pin `runtime_id`, monotonically increasing `host_epoch`, status, ingress-continuity
assessment, and aggregate version. A separate Runtime Mesh `ChannelHostLeaseRecord` holds the current opaque lease
ID, bounded expiry, and fencing-credential digest. Lease renewals update this operational
control record; they do not create assignment revisions or domain events. The raw
credential is delivered only to the authenticated assigned Runtime and never enters
events, state blobs, logs, Operator responses, or backups. ChannelBinding identity, action
grants, and provider credentials are not changed by assignment.

```text
ChannelHostLeaseRecord { # Hub/control-plane metadata, separate from domain aggregate
  channel_binding_id
  runtime_id
  host_epoch
  lease_id
  fencing_token_digest
  expires_at
  control_version
}

ChannelHostLeaseGrant { # private Mesh response; deliver only to authenticated owner Runtime
  assignment: ChannelHostAssignment
  lease_id
  fencing_credential: FencingCredential
  expires_at
}

AssignChannelHostRequest {
  request_id
  channel_binding_id
  target_runtime_id
  expected_assignment_version?
  accept_ingress_gap: boolean # required true only when cursor transfer/replay cannot prove continuity
}

RenewChannelHostLeaseRequest {
  request_id
  channel_binding_id
  runtime_id
  host_epoch
  expected_control_version
  fencing_credential: FencingCredential
}
```

The Runtime must hold an unexpired host lease to poll/receive provider events, claim or
complete `ChannelEventReceipt`s, or dispatch outbound channel Effects. Renewal requires
the same authenticated Runtime and current host epoch. The host checks expiry locally
before an external dispatch; a disconnected Runtime cannot renew or begin new outbound
mutations. In-flight sends at lease expiry become AMBIGUOUS and are reconciled before
retry. Provider receipts are deduplicated by `(channel_binding_id, provider_event_id)`;
claim epochs and host epochs both fence late workers. Per-host-epoch `ingress_sequence`
orders receipts independently of provider timestamps. A receipt is committed to the
Workspace event journal and acknowledged by Hub replication before the host advances its
encrypted local provider cursor or acknowledges deferred provider ingress. A target host
replays from the latest Hub-replicated receipt and deduplicates by provider event ID.

Reassignment requires explicit RuntimeMesh admission, a compatible channel-provider offer,
SecretRef availability, and safe source settlement or authoritative source-lease expiry
plus clock-skew margin. The Hub increments `host_epoch` before admitting the new Runtime.
`ingress_continuity = CONTINUOUS` requires a verified provider cursor transfer or replay
that reaches the last Hub-replicated receipt within the provider's declared replay window.
Otherwise, the Operator command must explicitly accept an observation gap; the assignment
records `GAP_ACCEPTED` with its timestamp. The UI preserves that status, and no later scan
can erase the historical uncertainty. New hosts do not poll until their Runtime-local,
incarnation-scoped cursor binding is validated or rebuilt. Receipt claims may be reclaimed
only after their bounded claim expires or the old host epoch has been committed as fenced;
the current host lease is required in both cases.
Old Runtime-local `ChannelReplyTarget`s become non-authoritative immediately because their
host epoch no longer matches; if the old Runtime is reachable, it closes them as cleanup.
Opaque provider message references are never copied to the new Runtime. A reply to an old
prompt falls back to the Operator inbox. Reassignment does not resend an existing
NotificationDelivery; any replacement prompt requires a new explicitly authorized
delivery and reply target. If source send/receipt state is ambiguous, reconciliation gates
handoff rather than risking a duplicate external message or accepted command. An unreplicated
receipt or unknown cursor also blocks a continuity-safe move unless the owner explicitly
accepts the recorded ingress gap.

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
and checks the FailoverClass. A cached credential cannot authorize Hub-mediated writes
after its lease expires, its Runtime incarnation is superseded, or a higher epoch is
committed.

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

On reconnect, Runtimes exchange per-Workspace/per-origin receipt cursors, send missing
immutable events or policy-omission receipts, and fetch any authorized state blobs needed
for aggregate application. They apply only events that pass current origin authorization,
revision, and fencing checks. Concurrent TaskSpec revisions remain siblings for explicit
resolution; no last-writer-wins rule applies to consequential state.

## Backup, restore, snapshot, and event retention

A Workspace backup is a consistent recovery set containing the relational checkpoint,
per-origin/per-Workspace event cursors and required event history, immutable blob manifest and referenced
objects, schema version, encryption/key identifiers, and integrity digests. A v1 backup
is a complete point-in-time set: the consistent database snapshot, included event history,
and blob manifest share one barrier, and each cursor records the last included sequence
for its origin Runtime. Changes after that barrier belong to a later full backup; v1 does
not define incremental chains. Credentials, native agent state, and local OS secrets are
excluded.

Restore authenticates the manifest, verifies all digests and schema compatibility, restores
the database and blobs to an isolated location, checks all referenced manifests and event
cursors, then rebuilds projections before the Workspace is made writable. A restore to a
new Runtime requires explicit identity re-pairing and new lease epochs; old fencing
authority is never restored. Backup restore drills and recovery-point/recovery-time
objectives are deployment requirements and are measured before hosted production.
