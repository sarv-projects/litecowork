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
  entity_revision?
  hlc_timestamp
  correlation_id
  causation_id
  schema_version
  type
  payload
  recorded_at
  payload_digest
}
```

The journal is the immutable history used for replication, audit projections, and
rebuildable read models. Current aggregate rows are transactionally maintained with
their events; every successful domain command appends its event(s) in the same commit.
An `origin_sequence` is allocated per Runtime in that commit and is unique/monotonic.
Events are append-only; corrections are new events. `payload_digest` is computed over a
canonical serialization and checked on replication. Any signature/key format remains a
transport/security implementation choice, but the authenticated origin must be known.

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
workspace.archived.v1
conversation.created.v1
conversation.message.added.v1

task.created.v1
task.spec.revised.v1
task.plan.revised.v1
task.lead_agent.changed.v1
task.status.changed.v1

step.created.v1
step.status.changed.v1

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

lease.acquired.v1
lease.renewed.v1
lease.released.v1
lease.expired.v1

capability.grant.created.v1
capability.grant.revoked.v1
capability.grant.expired.v1
capability.activation.status.changed.v1
capability.activation.health.changed.v1

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

automation.created.v1
automation.revision.created.v1
automation.status.changed.v1
automation.occurrence.created.v1
automation.occurrence.claimed.v1
automation.occurrence.settled.v1

connection.state.changed.v1
channel.binding.changed.v1
channel.inbound.received.v1
channel.receipt.changed.v1
channel.outbound.settled.v1

environment.created.v1
environment.state.changed.v1
environment.checkpoint.created.v1

capability.lock.created.v1
capability.offer.changed.v1
secret.lease.issued.v1
secret.lease.revoked.v1

handoff.created.v1
handoff.phase.changed.v1
audit.record.created.v1
```

The per-event payload schema is represented by the following field contract (all IDs
reference existing immutable/domain records):

| Event family | Required payload fields |
|---|---|
| `workspace.created` | `workspace_id`, `owner_principal_id`, `replication_policy`, `replication_scope_refs[]` |
| `workspace.replication_policy.changed` | `workspace_id`, `from`, `to`, `replication_scope_refs[]`, `aggregate_version` |
| `workspace.archived` | `workspace_id`, `archived_by`, `aggregate_version` |
| `conversation.created` | `conversation_id`, `created_by` |
| `conversation.message.added` | `message_id`, `conversation_id`, `author`, `content_digest`, `resource_refs` |
| `task.created` | `task_id`, `conversation_id?`, `initial_spec_revision`, `created_by` |
| `task.spec.revised` | `task_id`, `revision`, `parent_revisions[]`, `spec_digest`, `authored_by` |
| `task.plan.revised` | `task_id`, `revision`, `task_spec_revision`, `produced_by_agent_session_id`, `produced_by_attempt_id?`, `step_ids[]`, `aggregate_version` |
| `task.lead_agent.changed` | `task_id`, `from_agent_binding_id?`, `to_agent_binding_id`, `aggregate_version`, `requested_by` |
| `task.status.changed` | `task_id`, `from`, `to`, `reason_code`, `actor`, `aggregate_version` |
| `step.created` | `step_id`, `task_id`, `plan_revision`, `logical_key?`, `dependencies[]` |
| `step.status.changed` | `step_id`, `task_id`, `from`, `to`, `reason_code`, `aggregate_version` |
| `attempt.created` | `attempt_id`, `task_id`, `step_id`, `parent_attempt_id?`, `agent_binding_id`, `runtime_id`, `environment_id`, `lease_id` |
| `attempt.status.changed` | `attempt_id`, `from`, `to`, `reason_code`, `aggregate_version` |
| `attempt.checkpointed` | `attempt_id`, `resume_packet_ref`, `digest`, `source_spec_revision` |
| `attempt.failure.recorded` | `attempt_id`, `failure_code`, `failure_signature`, `retryable` |
| `agent.session.*` | `agent_session_id`, `task_id`, `session_kind`, `attempt_id?`, `agent_binding_id`, `session_state`, `reported_at` |
| `agent.binding.changed` | `agent_binding_id`, `workspace_id`, `agent_profile_id`, `runtime_id?`, `from_enabled`, `to_enabled`, `aggregate_version`, `requested_by` |
| `runtime.*` | `runtime_id`, `device_id?`, `availability`, `offers_digest?`, `key_version?` |
| `environment.*` | `environment_id`, `runtime_id`, `from?`, `to`, `provider_kind`, `reason_code?` |
| `lease.*` | `lease_id`, `task_id`, `step_id`, `attempt_id`, `runtime_id`, `epoch`, `expires_at?` |
| `capability.grant.*` | `grant_id`, `task_id`, `attempt_id?`, `capability_ref`, `scope_digest`, `expires_at?` |
| `capability.activation.status.changed` | `activation_id`, `capability_ref`, `runtime_id`, `from`, `to`, `reason_code?`, `aggregate_version` |
| `capability.activation.health.changed` | `activation_id`, `runtime_id`, `from`, `to`, `observed_at`, `reason_code?`, `aggregate_version` |
| `capability.lock.created` | `task_id`, `capability_ref`, `digest`, `package_version`, `source_ref` |
| `secret.lease.*` | `secret_lease_id`, `task_id`, `attempt_id?`, `runtime_id`, `scope_digest`, `expires_at?` |
| `artifact.created` | `artifact_id`, `workspace_id`, `task_id?`, `kind`, `current_version`, `library_status`, `aggregate_version` |
| `artifact.version.created` | `artifact_id`, `version`, `content_digest`, `storage_ref: BlobRef`, `created_by_attempt?`, `aggregate_version` |
| `artifact.library.promoted` | `artifact_id`, `from`, `to`, `aggregate_version` |
| `artifact.library.archived` | `artifact_id`, `from`, `to`, `aggregate_version` |
| `effect.*` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal?` |
| `evidence.created` | `evidence_id`, `task_id`, `subject_ref`, `level`, `kind`, `producer`, `payload_digest?` |
| `verification.*` | `verification_run_id`, `task_id`, `criterion_id`, `verifier_kind`, `status`, `evidence_refs[]` |
| `approval.*` | `approval_id`, `task_id`, `action_digest`, `required_assurance`, `from?`, `to`, `resolved_by?` |
| `automation.created` | `automation_id`, `current_revision`, `status`, `aggregate_version` |
| `automation.revision.created` | `automation_id`, `revision`, `definition_digest`, `authored_by` |
| `automation.status.changed` | `automation_id`, `from`, `to`, `aggregate_version` |
| `automation.occurrence.*` | `occurrence_id`, `automation_id`, `automation_revision`, `occurrence_key`, `claim_epoch`, `claim_expires_at?`, `trigger_input_ref?`, `trigger_payload_digest?`, `task_id?`, `from?`, `to` |
| `connection.state.changed` | `connection_id`, `from`, `to`, `provider_ref` |
| `channel.binding.changed` | `channel_binding_id`, `connection_id?`, `from`, `to`, `assurance_level`, `allowed_actions[]` |
| `channel.inbound.received` | `channel_binding_id`, `provider_event_id`, `event_kind`, `payload_digest` |
| `channel.receipt.changed` | `channel_binding_id`, `provider_event_id`, `from`, `to`, `claim_epoch`, `claim_expires_at?` |
| `channel.outbound.settled` | `channel_binding_id`, `delivery_id`, `provider_event_id?`, `state`, `result_digest?` |
| `handoff.*` | `handoff_id`, `task_id`, `step_id`, `source_attempt_id`, `source_runtime_id`, `target_runtime_id?`, `phase` |
| `audit.record.created` | `audit_record_id`, `principal`, `action`, `decision`, `reason_code`, `payload_digest?` |

Payloads never contain raw secret bytes, native hidden prompts, or unbounded terminal,
video, or token streams. ResourceRef and digest carry large/sensitive payloads by
reference.

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
  version
  content_digest
  storage_ref: BlobRef
  created_by_attempt?
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
- additive optional fields do not require new major event type when consumers can ignore them.
- breaking payload changes create `.v2`.
- events are never rewritten in storage.
- projections may upcast old events through deterministic migration functions.
- unknown future event types are stored/replicated even if a projection cannot interpret them.

## Ephemeral streams

Do not persist as domain events:
- token deltas
- mouse/animation frames
- raw terminal byte streams
- transient screenshots
- live video frames

These use Operator/Agent streaming channels and may be promoted to artifacts/evidence when required.
