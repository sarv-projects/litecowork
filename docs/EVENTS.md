# Event Journal and Event Schemas

## Envelope

```text
DomainEvent {
  event_id
  workspace_id
  entity_type
  entity_id
  origin_runtime_id
  origin_sequence
  entity_revision
  hlc_timestamp
  correlation_id
  causation_id
  schema_version
  type
  payload
  aggregate_state_ref: AggregateStateRef
  recorded_at
  payload_digest
}
```

Opaque provider task IDs, provider cursors, MCP input-request keys, credentials, and native
session/process handles are forbidden from replicated event envelopes and payloads. Emit
only normalized status/timing observations and non-reversible digests; local encrypted
bindings are excluded from Mesh replication and Workspace backups.

The journal is the immutable history used for replication, audit projections, and
rebuildable read models. Current aggregate rows are transactionally maintained with
their events; every successful domain command appends its event(s) in the same commit.
An `origin_sequence` is allocated monotonically per `(workspace_id, origin_runtime_id)`
in that commit. It is not a Runtime-global sequence: peers and acknowledgements are
Workspace-scoped, and a Workspace cursor must never wait for another Workspace's events.
Events are append-only; corrections are new events. `payload_digest` is computed over a
canonical serialization and checked on replication. Any signature/key format remains a
transport/security implementation choice, but the authenticated origin must be known.
`aggregate_state_ref` is mandatory for every event. It points to an immutable,
content-addressed serialization of the complete post-transition aggregate record and
records that aggregate's revision and record-schema version. This is distinct from
periodic `AggregateSnapshot`: every event has one state reference so a receiving Runtime
can reconstruct the exact current record without guessing omitted fields from a digest or
partial event payload. Events remain necessary for lifecycle history, audit, and side-effect
semantics; the state blob is not permission to replay a command or external Effect.
`entity_revision` is mandatory and must equal `aggregate_state_ref.entity_revision`.

RuntimeOffer and CapabilityHostInstance readiness/health observations are expiring
Runtime-operational inventory, not replicated Task aggregates or durable domain events.
They update the Runtime read projection and its Operator inventory subscription.
CapabilityActivation lifecycle and Invocation outcomes remain durable domain events; a host
health observation cannot rewrite those histories.
`RuntimeIncarnation` is registered and versioned through the authenticated Runtime Mesh
control protocol before any offer or event can reference it; its compact public record is
retained with Workspace replication metadata. OS boot IDs and local diagnostics are never
in event state or Workspace backup. `AgentSessionHostBinding` and
`CapabilityActivationHostBinding` are local operational relations: native session handles,
provider handles, and host-instance IDs never enter replicated event payloads or aggregate
state blobs. `AgentEndpointBinding` keeps endpoint command/socket/URL locators local.
`EnvironmentProviderBinding` and `EnvironmentCheckpointProviderBinding` are
also local-only; provider locators and checkpoint handles never enter events or aggregate
state blobs. `ResourceLocationBinding` is local-only too; only a stable non-secret location
resolver key can appear in durable state. Raw filesystem identity tuples are held in
Runtime-local `FileIdentityBinding`; replicated FileIdentity fields are keyed pseudonyms.
A portable checkpoint may reference
content-addressed bytes through its durable aggregate state, subject to Workspace
replication policy. For `agent.session.*`
and `capability.activation.*` events, the payload's
`runtime_id` must equal the authenticated `origin_runtime_id`, and its incarnation must be
registered for that Runtime before the event is accepted. A relay preserves original event
identity and cannot author a foreign Runtime's session or activation transition.
Raw execution/control fencing credentials are Runtime-private and never enter the event
envelope, payload, `aggregate_state_ref` blob, replication receipt, or backup. Durable lease
state may carry only `fencing_token_digest`; the digest is not usable as a credential.

The writer commits/fsyncs the state blob before committing the event and its projection
transaction. A failed DB commit may leave an unreferenced blob for garbage collection.
Replication authenticates and durably stores the envelope before acknowledging receipt.
It transfers and verifies the referenced blob before applying the event; an event with an
unavailable state blob remains pending and cannot advance the aggregate projection. The
receiver validates digest,
workspace scope, entity identity, entity revision, record schema and transition authority,
then atomically applies the state record and event. The per-origin contiguous receipt
cursor and per-aggregate applied revision are separate: a stored event may be acknowledged
while waiting for its state blob or a missing aggregate baseline. State blobs contain only canonical
domain records and authorized ResourceRefs; never secret bytes, hidden prompts, or raw
agent transcripts. Workspace replication policy and TrustService authorize state-blob
transfer independently of event-envelope transfer.

## EventStore

```text
interface EventStore {
  append(ExpectedRevision, DomainEvent[]) -> AppendResult
  read_stream(EntityRef, from_revision?) -> Event[]
  read_global(EventCursor, limit) -> EventBatch
  has(EventId) -> bool
  accept_replication(AuthenticatedOrigin, EventBatch) -> ReplicationResult
}
```

Remote events are validated for authenticated origin, entity ownership, expected
revision, transition owner, idempotency, and current fencing authority before they update
canonical aggregate state. Unknown event versions may be stored for forwarding but cannot
be applied to a projection that does not understand them. A rejected event cannot change
domain truth.

## Durable event registry

Minimum v1 registry:

```text
workspace.created.v1
workspace.replication_policy.changed.v1
workspace.default_agent_binding.changed.v1
workspace.archived.v1
workspace.instructions.revision.created.v1
workspace.root.created.v1
workspace.root.status.changed.v1
resource.created.v1
resource.revision.observed.v1
resource.location.changed.v1
resource.edge.created.v1
resource.invalidation.created.v1
resource.upload.created.v1
resource.upload.status.changed.v1
conversation.created.v1
conversation.message.added.v1
conversation.turn.created.v1
conversation.turn.retried.v1
conversation.turn.resumed.v1
conversation.turn.settled.v1

task.created.v1
task.spec.revised.v1
task.plan.revised.v1
task.lead_agent.changed.v1
task.status.changed.v1
task.pause.requested.v1
task.paused.v1
task.resumed.v1

step.created.v1
step.status.changed.v1
step.recovery.accepted.v1

attempt.created.v1
attempt.status.changed.v1
attempt.checkpointed.v1
attempt.failure.recorded.v1

agent.session.started.v1
agent.session.lost.v1
agent.session.closed.v1
agent.binding.changed.v1

runtime.paired.v1
runtime.revoked.v1
runtime.availability.changed.v1
runtime.offer.changed.v1
provider.circuit.changed.v1

lease.acquired.v1
lease.renewed.v1
lease.released.v1
lease.expired.v1

capability.grant.created.v1
capability.grant.revoked.v1
capability.grant.expired.v1
capability.activation.status.changed.v1
capability.activation.health.changed.v1
capability.invocation.created.v1
capability.invocation.dispatched.v1
capability.invocation.checkpointed.v1
capability.invocation.status.changed.v1
usage.observed.v1
budget.reservation.changed.v1

artifact.created.v1
artifact.version.created.v1
artifact.library.promoted.v1
artifact.library.archived.v1

effect.proposed.v1
effect.started.v1
effect.acknowledged.v1
effect.observed.v1
effect.verified.v1
effect.failed.v1
effect.ambiguous.v1
effect.reconciliation.started.v1
effect.retry.authorized.v1

evidence.created.v1
verification.started.v1
verification.completed.v1

approval.requested.v1
approval.resolved.v1
approval.use.consumed.v1
user.request.created.v1
user.request.resolved.v1
notification.preference.changed.v1
notification.delivery.changed.v1
skill.proposal.created.v1
skill.proposal.status.changed.v1

routine.created.v1
routine.revision.created.v1
routine.status.changed.v1
automation.cursor.changed.v1
automation.occurrence.status.changed.v1
automation.created.v1
automation.revision.created.v1
automation.status.changed.v1
automation.occurrence.created.v1
automation.occurrence.claimed.v1
automation.occurrence.settled.v1

connection.state.changed.v1
channel.binding.changed.v1
channel.host.assignment.changed.v1
channel.inbound.received.v1
channel.receipt.changed.v1
channel.outbound.settled.v1

environment.created.v1
environment.state.changed.v1
environment.checkpoint.created.v1
environment.control.lease.changed.v1

capability.lock.created.v1
secret.lease.issued.v1
secret.lease.revoked.v1

handoff.created.v1
handoff.phase.changed.v1
audit.record.created.v1
```

The per-event payload schema is represented by the following field contract (all IDs
reference existing immutable/domain records):

`schemas/domain-event.schema.json` validates the envelope and the per-type payload schema
validates each concrete `type`. The validator requires one payload branch for every
registry entry; the broad `{ "type": "object" }` fallback is forbidden. Optional fields
may be absent; fields marked `?` are optional. All envelope and payload objects are closed
and validate known fields and types. Adding, removing, renaming, or changing the type or
meaning of a payload field creates a new event type major (`.v2`); consumers may
store/forward unknown versions but cannot apply them to projections they do not understand.

| Event family | Required payload fields |
|---|---|
| `workspace.created` | `workspace_id`, `owner_principal_id`, `replication_policy`, `replication_scope_root_ids[]` |
| `workspace.replication_policy.changed` | `workspace_id`, `from`, `to`, `replication_scope_root_ids[]`, `aggregate_version` |
| `workspace.default_agent_binding.changed` | `workspace_id`, `from_agent_binding_id`, `to_agent_binding_id`, `changed_by`, `aggregate_version` |
| `workspace.archived` | `workspace_id`, `archived_by`, `aggregate_version` |
| `workspace.instructions.revision.created` | `workspace_id`, `revision`, `parent_revisions[]`, `content_ref`, `content_digest`, `authored_by` |
| `workspace.root.created` | `workspace_root_id`, `workspace_id`, `resource_id`, `location_id`, `added_by`, `aggregate_version` |
| `workspace.root.status.changed` | `workspace_root_id`, `from`, `to`, `reason_code`, `aggregate_version` |
| `resource.created` | `resource_id`, `workspace_id`, `kind`, `identity_digest?`, `provenance`, `aggregate_version` |
| `resource.revision.observed` | `resource_id`, `resource_revision_id`, `parent_revision_ids[]`, `provider_revision?`, `content_digest?`, `observed_at` |
| `resource.location.changed` | `location_id`, `resource_id`, `availability`, `observed_revision_id?`, `observed_at` |
| `resource.edge.created` | `edge_id`, `from_resource_id`, `to_resource_id`, `relation`, `observed_at` |
| `resource.invalidation.created` | `invalidation_record_id`, `dependency_edge_id`, `observed_revision_id`, `reason_code`, `created_at` |
| `resource.upload.created` | `upload_id`, `workspace_id`, `expected_size_bytes`, `expected_digest?`, `chunk_size_bytes`, `expires_at`, `aggregate_version` |
| `resource.upload.status.changed` | `upload_id`, `from`, `to`, `resource_id?`, `reason_code?`, `aggregate_version` |
| `conversation.created` | `conversation_id`, `created_by` |
| `conversation.message.added` | `message_id`, `conversation_id`, `author`, `content_digest`, `resource_refs` |
| `conversation.turn.created` | `turn_id`, `conversation_id`, `user_message_id`, `agent_binding_id`, `aggregate_version` |
| `conversation.turn.retried` | `turn_id`, `prior_agent_session_id?`, `agent_session_id`, `retry_ordinal`, `aggregate_version` |
| `conversation.turn.resumed` | `turn_id`, `user_request_id`, `prior_agent_session_id?`, `agent_session_id`, `aggregate_version` |
| `conversation.turn.settled` | `turn_id`, `from`, `to`, `reason_code?`, `agent_session_id?`, `aggregate_version` |
| `task.created` | `task_id`, `conversation_id?`, `initial_spec_revision`, `created_by` |
| `task.spec.revised` | `task_id`, `revision`, `parent_revisions[]`, `spec_digest`, `authored_by` |
| `task.plan.revised` | `task_id`, `revision`, `task_spec_revision`, `produced_by_agent_session_id`, `produced_by_attempt_id?`, `step_ids[]`, `aggregate_version` |
| `task.lead_agent.changed` | `task_id`, `from_agent_binding_id?`, `to_agent_binding_id`, `aggregate_version`, `requested_by` |
| `task.status.changed` | `task_id`, `from`, `to`, `reason_code`, `actor`, `blocking_conditions[]`, `aggregate_version` |
| `task.pause.requested` | `task_id`, `resume_status`, `request_id`, `requested_by`, `aggregate_version` |
| `task.paused` | `task_id`, `resume_packet_ref`, `resume_packet_digest`, `settled_attempt_ids[]`, `released_lease_ids[]`, `aggregate_version` |
| `task.resumed` | `task_id`, `from`, `to`, `resumed_by`, `aggregate_version` |
| `step.created` | `step_id`, `task_id`, `plan_revision`, `logical_key?`, `dependencies[]` |
| `step.status.changed` | `step_id`, `task_id`, `from`, `to`, `reason_code`, `aggregate_version` |
| `step.recovery.accepted` | `task_id`, `step_id`, `prior_attempt_id`, `new_attempt_id`, `retry_ordinal`, `recovery_reason`, `aggregate_version` |
| `attempt.created` | `attempt_id`, `task_id`, `step_id`, `parent_attempt_id?`, `agent_binding_id`, `runtime_id`, `runtime_incarnation_id`, `environment_id`, `lease_id` |
| `attempt.status.changed` | `attempt_id`, `from`, `to`, `reason_code`, `aggregate_version` |
| `attempt.checkpointed` | `attempt_id`, `resume_packet_ref`, `digest`, `source_spec_revision` |
| `attempt.failure.recorded` | `attempt_id`, `failure_code`, `failure_signature`, `retryable` |
| `agent.session.*` | `agent_session_id`, `scope`, `task_spec_revision`, `agent_binding_id`, `endpoint_id`, `runtime_id`, `runtime_incarnation_id`, `session_state`, `reported_at` |
| `agent.binding.changed` | `agent_binding_id`, `workspace_id`, `agent_profile_id`, `runtime_id?`, `from_enabled`, `to_enabled`, `aggregate_version`, `requested_by` |
| `runtime.*` | `runtime_id`, `device_id?`, `availability`, `offers_digest?`, `key_version?` |
| `provider.circuit.changed` | `runtime_id`, `provider_kind`, `provider_ref`, `from`, `to`, `failure_window_started_at?`, `consecutive_failures`, `open_until?`, `aggregate_version` |
| `environment.created` | `environment_id`, `runtime_id`, `to`, `provider_kind`, `lifetime`, `from?`, `reason_code?`, `provision_preview_digest?` (mandatory when the lifetime value is WORKSPACE_PERSISTENT) |
| `environment.state.changed` | `environment_id`, `runtime_id`, `from?`, `to`, `provider_kind`, `reason_code?` |
| `environment.checkpoint.created` | `checkpoint_id`, `environment_id`, `runtime_id`, `content_digest`, `created_at` |
| `environment.control.lease.changed` | `control_lease_id`, `environment_id`, `task_id`, `attempt_id`, `runtime_id`, `runtime_incarnation_id`, `owner_kind`, `owner_ref`, `epoch`, `from_owner_kind?`, `to_owner_kind?`, `from?`, `to`, `aggregate_version` |
| `lease.*` | `lease_id`, `task_id`, `step_id`, `attempt_id`, `runtime_id`, `runtime_incarnation_id`, `epoch`, `expires_at?` |
| `capability.grant.*` | `capability_grant_id`, `scope`, `capability_ref`, `scope_digest`, `expires_at?` |
| `capability.activation.status.changed` | `activation_id`, `capability_ref`, `scope`, `runtime_id`, `runtime_incarnation_id`, `from`, `to`, `reason_code?`, `aggregate_version` |
| `capability.activation.health.changed` | `activation_id`, `scope`, `runtime_id`, `runtime_incarnation_id`, `from`, `to`, `observed_at`, `reason_code?`, `aggregate_version` |
| `capability.invocation.created` | `invocation_id`, `scope`, `agent_session_id`, `capability_grant_id`, `activation_id`, `operation`, `request_digest`, `aggregate_version` |
| `capability.invocation.dispatched` | `invocation_id`, `dispatch_ordinal`, `provider_task_status?`, `provider_task_created_at?`, `provider_task_expires_at?`, `provider_task_ttl_ms?`, `provider_poll_after_ms?`, `dispatched_at`, `aggregate_version` |
| `capability.invocation.checkpointed` | `invocation_id`, `provider_task_status?`, `provider_task_expires_at?`, `provider_task_ttl_ms?`, `provider_updated_at?`, `provider_poll_after_ms?`, `partial_result_refs[]`, `observed_at`, `aggregate_version` |
| `capability.invocation.status.changed` | `invocation_id`, `from`, `to`, `result_refs[]`, `effect_id?`, `failure_code?`, `aggregate_version` |
| `usage.observed` | `usage_observation_id`, `task_id?`, `environment_id?`, `attempt_id?`, `invocation_id?`, `metric`, `quantity`, `unit`, `currency?`, `confidence`, `observed_at` (`quantity` is null only when `confidence` is UNKNOWN) |
| `budget.reservation.changed` | `reservation_id`, `budget_scope`, `task_id?`, `environment_id?`, `attempt_id?`, `metric`, `quantity`, `unit`, `currency?`, `from?`, `to`, `aggregate_version` |
| `capability.lock.created` | `task_id`, `capability_ref`, `aggregate_version` |
| `secret.lease.*` | `secret_lease_id`, `task_id`, `attempt_id?`, `runtime_id`, `scope_digest`, `expires_at?` |
| `artifact.created` | `artifact_id`, `workspace_id`, `resource_id`, `task_id?`, `kind`, `current_version`, `library_status`, `aggregate_version` |
| `artifact.version.created` | `artifact_id`, `resource_id`, `version`, `resource_revision_id`, `input_refs[]`, `content_kind`, `content_digest?`, `storage_ref?`, `resource_ref?`, `provider_revision?`, `created_by_attempt?`, `aggregate_version` |
| `artifact.library.promoted` | `artifact_id`, `from`, `to`, `aggregate_version` |
| `artifact.library.archived` | `artifact_id`, `from`, `to`, `aggregate_version` |
| `effect.*` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal?` |
| `evidence.created` | `evidence_id`, `task_id`, `subject_ref`, `level`, `kind`, `producer`, `payload_digest?` |
| `verification.started` | `verification_run_id`, `task_id`, `criterion_id`, `task_spec_revision`, `criterion_digest`, `verifier_kind`, `verifier_version`, `subject_refs[]`, `inputs[]`, `status`, `evidence_refs[]`, `aggregate_version` |
| `verification.completed` | `verification_run_id`, `task_id`, `criterion_id`, `task_spec_revision`, `criterion_digest`, `verifier_kind`, `verifier_version`, `subject_refs[]`, `inputs[]`, `status`, `evidence_refs[]`, `aggregate_version` |
| `approval.*` | `approval_id`, `task_id`, `action_digest`, `required_assurance`, `from?`, `to`, `resolved_by?` |
| `approval.use.consumed` | `approval_use_id`, `approval_id`, `effect_id?`, `capability_grant_id?`, `request_digest`, `consumed_at` |
| `user.request.created` | `request_id`, `workspace_id`, `conversation_id?`, `conversation_turn_id?`, `task_id?`, `attempt_id?`, `agent_session_id`, `invocation_id?`, `kind`, `interaction_mode`, `response_schema_digest?`, `expires_at?` |
| `user.request.resolved` | `request_id`, `from`, `to`, `resolved_by`, `response_digest?`, `channel_binding_id?`, `provider_event_id?`, `aggregate_version` |
| `notification.preference.changed` | `workspace_id`, `event_class`, `policy`, `preferred_channels[]`, `aggregate_version` |
| `notification.delivery.changed` | `delivery_id`, `source_event_id`, `dedupe_key`, `from`, `to`, `channel_binding_id?`, `attempt_runtime_id?`, `attempt_host_epoch?`, `attempt_count`, `reason_code?` |
| `skill.proposal.created` | `skill_proposal_id`, `source_task_id`, `source_artifact_id`, `draft_resource_ref`, `draft_digest`, `aggregate_version` |
| `skill.proposal.status.changed` | `skill_proposal_id`, `from`, `to`, `redaction_status`, `approved_by?`, `aggregate_version` |
| `routine.created` | `routine_id`, `current_revision`, `status`, `aggregate_version` |
| `routine.revision.created` | `routine_id`, `revision`, `definition_digest`, `authored_by` |
| `routine.status.changed` | `routine_id`, `from`, `to`, `aggregate_version` |
| `automation.cursor.changed` | `automation_id`, `trigger_id`, `active_automation_revision`, `trigger_host_runtime_id`, `host_epoch`, `cursor_digest`, `next_scheduled_at?`, `last_checked_at`, `observation_gap_since?`, `aggregate_version` |
| `automation.created` | `automation_id`, `current_revision`, `status`, `aggregate_version` |
| `automation.revision.created` | `automation_id`, `revision`, `definition_digest`, `authored_by` |
| `automation.status.changed` | `automation_id`, `from`, `to`, `aggregate_version` |
| `automation.occurrence.*` | `occurrence_id`, `automation_id`, `automation_revision`, `routine_id`, `routine_revision`, `trigger_id`, `trigger_host_runtime_id`, `occurrence_key`, `claim_epoch`, `claim_expires_at?`, `trigger_input_ref?`, `trigger_payload_digest?`, `task_id?`, `from?`, `to` |
| `connection.state.changed` | `connection_id`, `from`, `to`, `provider_ref` |
| `channel.binding.changed` | `channel_binding_id`, `connection_id?`, `from`, `to`, `assurance_level`, `allowed_actions[]` |
| `channel.host.assignment.changed` | `channel_binding_id`, `from_runtime_id?`, `runtime_id`, `host_epoch`, `status`, `ingress_continuity`, `ingress_gap_since?`, `aggregate_version` |
| `channel.inbound.received` | `channel_binding_id`, `provider_event_id`, `origin_runtime_id`, `origin_host_epoch`, `ingress_sequence`, `event_kind`, `payload_digest` |
| `channel.receipt.changed` | `channel_binding_id`, `provider_event_id`, `claim_runtime_id`, `claim_host_epoch`, `from`, `to`, `claim_epoch`, `claim_expires_at?` |
| `channel.outbound.settled` | `channel_binding_id`, `runtime_id`, `host_epoch`, `delivery_id`, `provider_event_id?`, `state`, `result_digest?` |
| `handoff.*` | `handoff_id`, `task_id`, `step_id`, `source_attempt_id`, `source_runtime_id`, `target_runtime_id?`, `phase` |
| `audit.record.created` | `audit_record_id`, `principal`, `action`, `decision`, `reason_code`, `payload_digest?` |

Payloads never contain raw secret bytes, native hidden prompts, or unbounded terminal,
video, or token streams. ResourceRef and digest carry large/sensitive payloads by
reference.

For `channel.inbound.received.v1`, `ingress_sequence` is monotonic within the binding and
origin host epoch. For `channel.receipt.changed.v1`, `PROCESSING -> PROCESSING` means only
that an expired or fenced claim was reclaimed with a larger claim epoch; a terminal
transition clears `claim_expires_at`. Reusing a provider event ID with a different payload
digest is a conflict and cannot overwrite the immutable origin event.

`user.request.resolved.v1` includes `channel_binding_id` and `provider_event_id` together
only when the response came from a channel. The provider event is the authenticated
inbound response receipt; the opaque provider message reference used to correlate its
reply-to target stays Runtime-local and is never replicated in the event journal.

Every digest-valued field uses `Sha256Digest`: `sha256:` followed by 64 lowercase
hexadecimal characters. Provider revision IDs and opaque resource locators are not digests.

For `provider.circuit.changed`, `consecutive_failures` is an integer in the unsigned 32-bit range (0 through 4,294,967,295), matching `ProviderCircuit.consecutive_failures` in `DATA-MODEL.md`. The event schema and SQLite constraint enforce this bound.

## Example payloads

```text
TaskCreatedV1 {
  task_id
  conversation_id?
  initial_spec_revision
  created_by
}

AttemptCreatedV1 {
  attempt_id
  task_id
  step_id
  parent_attempt_id?
  agent_binding_id
  runtime_id
  environment_id
  lease_id
}

ArtifactCreatedV1 {
  artifact_id
  workspace_id
  task_id?
  kind
  current_version
  library_status
  aggregate_version
}

ArtifactVersionCreatedV1 {
  artifact_id
  resource_id
  version
  resource_revision_id
  input_refs[]               # each ResourceRef pins its input ResourceRevision
  content_kind: MANAGED_BLOB | EXTERNAL_RESOURCE
  content_digest?          # required for MANAGED_BLOB; optional observed digest for EXTERNAL_RESOURCE
  storage_ref?              # required only for MANAGED_BLOB
  resource_ref?             # required only for EXTERNAL_RESOURCE
  provider_revision?        # optional external revision pin
  created_by_attempt?
  aggregate_version
}

VerificationStartedV1 {
  verification_run_id
  task_id
  criterion_id
  task_spec_revision
  criterion_digest
  verifier_kind
  verifier_version
  subject_refs[]
  inputs[]                   # ResourceInput pairs exact revisions with observed byte digests
  status
  evidence_refs[]
  aggregate_version
}

ArtifactLibraryTransitionV1 {
  artifact_id
  from: TRANSIENT | SAVED
  to: SAVED | ARCHIVED
  aggregate_version
}
```

## Event versioning

- event `type` includes major schema version.
- any payload field-set/type/meaning change, including adding an optional field, creates a
  new event type major such as `.v2`. This matches the closed payload schemas and prevents
  older consumers from rejecting a nominally “additive” field after partial application.
- events are never rewritten in storage.
- projections may upcast old events through deterministic migration functions.
- unknown future event types are stored/replicated even if a projection cannot interpret them.

## Snapshots, compaction, and cold archive

Snapshots are rebuild accelerators, not replacements for immutable event identity or
audit. `AggregateSnapshot` stores `(workspace_id, entity_type, entity_id,
through_revision, projection_schema_version, state_digest, blob_ref, created_at)` and is
accepted only after replay verification against the source stream. A projection can load
the latest compatible snapshot then replay subsequent events; it must also support a full
rebuild from retained events.

Compaction removes materialized projection history only when the snapshot/checkpoint and
all required consumer receipt cursors make it safe. Event payloads needed for active
Effects, approvals, disputes, provenance, or audit retention remain available. Cold
archive segments are immutable, digest-verified, indexed by Workspace/origin sequence,
and restoreable; compaction never changes origin sequence or rewrites an event. Retention
policy is explicit per event class. A full rebuild starts from a compatible retained
snapshot, then replays every later event; where policy prohibits retention of earlier
events, replay begins at the oldest retained snapshot rather than pretending history is
complete back to genesis. Deletion is prohibited while any recovery,
replication, legal hold, or audit dependency references the event.

## Ephemeral streams

Do not persist as domain events:
- token deltas
- mouse/animation frames
- raw terminal byte streams
- transient screenshots
- live video frames

These use Operator/Agent streaming channels and may be promoted to artifacts/evidence when required.

## Routine and trigger lifecycle journal

Routine creation/revision/archive and cursor ownership/checkpoint changes commit with
their aggregate state and typed events. `cursor_digest` identifies the encrypted
Runtime-local cursor binding; raw opaque provider cursors are excluded from all replicated
aggregate state and event payloads. Cursor updates use expected version and host epoch. Unchanged
trigger identities preserve their cursor across Automation edits; source changes create
a new trigger identity. `automation.occurrence.status.changed` records dependency waits
and resumptions that are neither a new claim nor final settlement.

The v1 registry is an unpublished implementation candidate. Changes here refine that
candidate; once published, incompatible payload changes require a new event version and
an explicit consumer migration.
