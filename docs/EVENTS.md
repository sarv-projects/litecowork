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

The local `ResourceTextIndex` is a rebuildable, non-replicated projection over exact
ResourceRevision bytes. Index insertion/rebuild does not append a DomainEvent and cannot
change Resource or Task truth. Resource lifecycle events remain authoritative; the index
must be rebuildable from retained, verified Resource blobs, and its encrypted snapshot and
term rows are removed only through the owning Resource/ContextDocument purge contract.
The local StateStore keeps a durable per-Workspace/per-origin allocator row. It advances
in the same transaction as the event and aggregate projection, survives event archival,
and is not itself replicated. Replicas preserve the authenticated source sequence and do
not allocate it locally.
Events are append-only; corrections are new events. `payload_digest` is computed over the
RFC 8785 JSON Canonicalization Scheme (JCS) serialization of `payload` and checked on
replication. Canonical aggregate-state blob bytes use the same JCS encoding before their
SHA-256 digest is computed. Implementations reject values outside the JSON/I-JSON domain
required by RFC 8785 rather than hashing implementation-specific encodings. Any
signature/key format remains a transport/security implementation choice, but the
authenticated origin must be known.
`aggregate_state_ref` is mandatory for every event. It points to an immutable,
content-addressed serialization of the complete post-transition aggregate record and
records that aggregate's revision and record-schema version. This is distinct from
periodic `AggregateSnapshot`: every event has one state reference so a receiving Runtime
can reconstruct the exact current record without guessing omitted fields from a digest or
partial event payload. Events remain necessary for lifecycle history, audit, and side-effect
semantics; the state blob is not permission to replay a command or external Effect.
`entity_revision` is mandatory and must equal `aggregate_state_ref.entity_revision`.
For `task.plan.revised.v1`, the Task snapshot includes its updated current-plan pointer,
TaskSpec head, immutable current PlanRevision, and the Steps materialized by that
acceptance transaction. Each `step.created.v1` points to the complete initial Step record.
These snapshots make plan/Step reconstruction possible after replication or restart; the
event payload alone is not a substitute for them.

RuntimeOffer and CapabilityHostInstance readiness/health observations are expiring
Runtime-operational inventory, not replicated Task aggregates or durable domain events.
They update the Runtime read projection and its Operator inventory subscription.
CapabilityActivation lifecycle and Invocation outcomes remain durable domain events; a host
health observation cannot rewrite those histories.
`RuntimeIncarnation` is registered and versioned through the authenticated Runtime Mesh
control protocol before any offer or event can reference it; its compact public record is
retained with Workspace replication metadata. OS boot IDs and local diagnostics are never
in event state or Workspace backup. `RuntimeWorkspaceBinding` is security/control-plane
metadata, not a Workspace Task-domain event. Local enrollment is stored in the local
Runtime StateStore and audited. Mesh pairing and revocation are authenticated Runtime Mesh
control records with versioned receipts; they are not relayed as ordinary Workspace
events. A Workspace Runtime list is projected only after checking an active binding for
the selected Workspace. `AgentSessionHostBinding` and
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
workspace.primary_coworker.changed.v1
workspace.archived.v1
workspace.instructions.revision.created.v1
workspace.root.created.v1
workspace.root.status.changed.v1
agent.binding.created.v1
resource.created.v1
resource.created.v2 # versioned provenance may include folder_import
resource.revision.created.v1 # owner-authored immutable Resource revision append
resource.revision.observed.v1
resource.context_document.status.changed.v1
resource.context_document.purge.acknowledged.v1
resource.location.changed.v1
resource.edge.created.v1
resource.invalidation.created.v1
resource.upload.created.v1
resource.upload.status.changed.v1
conversation.created.v1
conversation.message.added.v1
conversation.turn.created.v1
conversation.turn.created.v2 # includes immutable PresentationPreference
conversation.turn.retried.v1
conversation.turn.resumed.v1
conversation.turn.status.changed.v1
conversation.turn.settled.v1
rich.presentation.published.v1

task.created.v1
task.spec.revised.v1
task.plan.revised.v1
task.lead_agent.changed.v1
task.status.changed.v1
task.pause.requested.v1
task.paused.v1
task.resumed.v1
task.coworker.origin.pinned.v1

step.created.v1
step.status.changed.v1
step.recovery.accepted.v1

attempt.created.v1
attempt.status.changed.v1
attempt.checkpointed.v1
attempt.failure.recorded.v1

agent.session.starting.v1 — claims a bounded durable STARTING reservation; it is not adapter readiness and grants no Task authority.
agent.session.started.v1 — records adapter readiness after an atomic transition to ACTIVE; its payload/state must not include native handles.
agent.session.lost.v1
agent.session.closed.v1
agent.binding.changed.v1
agent.binding.lead_eligibility.changed.v1
agent.session.harness_descriptor.pinned.v1

delegation_profile.created.v1
delegation_profile.revised.v1
delegation_profile.status.changed.v1
delegation.admitted.v1

coworker.created.v1
coworker.revised.v1
coworker.status.changed.v1

goal.created.v1
goal.revised.v1
goal.status.changed.v1

suggestion.proposed.v1
suggestion.resolved.v1
suggestion.visibility.changed.v1
suggestion.preference.changed.v1
demonstration.status.changed.v1

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
channel.host.assignment.changed.v2
channel.inbound.received.v1
channel.receipt.changed.v1
channel.outbound.settled.v1

environment.created.v1
environment.state.changed.v1
environment.checkpoint.created.v1
environment.control.lease.changed.v1
environment.sharing_scope.changed.v1

capability.lock.created.v1
secret.lease.issued.v1
secret.lease.revoked.v1

handoff.created.v1
handoff.phase.changed.v1
audit.record.created.v1
```

ResourceUpload chunk receipts are local transfer progress, not one event per chunk.
`version` advances with the session lifecycle and is pinned to each lifecycle event;
`progress_version` fences accepted chunk writes but is not an event revision. The final
chunk emits OPEN -> CONTENT_RECEIVED, TTL expiry emits OPEN/CONTENT_RECEIVED -> EXPIRED,
successful finalization emits CONTENT_RECEIVED -> COMMITTED, and terminal stored-content
integrity failure emits CONTENT_RECEIVED -> FAILED. Each event, aggregate snapshot, and
matching projection transition commit atomically. Transient storage/database failures do
not produce FAILED and leave the upload retryable.

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
| `resource.created` | `resource_id`, `workspace_id`, `kind`, `identity_digest?`, `provenance`, `context_document?` (safe kind/owner/status metadata only; new ContextDocuments start ACTIVE), `aggregate_version` |
| `resource.revision.created.v1` | `resource_id`, `resource_revision_id`, `parent_revision_ids[]`, `content_digest`, `size_bytes`, `media_type`, `created_by`, `aggregate_version` |
| `resource.revision.observed` | `resource_id`, `resource_revision_id`, `parent_revision_ids[]`, `provider_revision?`, `content_digest?`, `observed_at` |
| `resource.context_document.status.changed` | `resource_id`, `from`, `to`, `changed_by`, `purge_manifest_digest?`, `purge_target_count?`, `aggregate_version`; manifest fields are required when status changes to DELETION_PENDING or DELETED |
| `resource.context_document.purge.acknowledged` | `resource_id`, `replica_ref` (stable non-secret identity, never a locator), `replica_kind`, `runtime_id?`, `runtime_incarnation_id?`, `target_revision_ids[]`, `receipt_digest`, `acknowledged_at`, `aggregate_version` |
| `resource.location.changed` | `location_id`, `resource_id`, `availability`, `observed_revision_id?`, `observed_at` |
| `resource.edge.created` | `edge_id`, `from_resource_id`, `to_resource_id`, `relation`, `observed_at` |
| `resource.invalidation.created` | `invalidation_record_id`, `dependency_edge_id`, `observed_revision_id`, `reason_code`, `created_at` |
| `resource.upload.created` | `upload_id`, `workspace_id`, `expected_size_bytes`, `expected_digest`, `chunk_size_bytes`, `expires_at`, `resource_id?`, `expected_resource_version?`, `parent_revision_ids[]?`, `aggregate_version` |
| `resource.upload.status.changed` | `upload_id`, `from`, `to`, `resource_id?`, `reason_code?`, `aggregate_version` |

Runtime-start WorkspaceRoot revalidation reuses the canonical
`workspace.root.status.changed.v1` and `resource.location.changed.v1` events. The root event
is emitted only when status changes; for revalidation its `reason_code` is one of `ROOT_IDENTITY_REVALIDATED`,
`NO_PRIOR_BINDING`, `LOCATOR_BINDING_MISSING`, `FILE_IDENTITY_BINDING_MISSING`,
`BINDING_MISMATCH`, `UNSUPPORTED_PLATFORM`, `INVALID_LOCATOR`, `IDENTITY_CHANGED`, or
`IDENTITY_UNAVAILABLE`. Owner transitions use `USER_PAUSED`, `USER_RESUMED`, and
`USER_REVOKED`; the event payload schema constrains statuses and these reason codes. The
location event records the observed `AVAILABLE` or `UNAVAILABLE`
state on every revalidation attempt. Both events, their safe aggregate snapshots, status and
location projections, idempotency receipt, and private current-incarnation bindings are
committed atomically. Raw paths and operating-system file identities are absent from event
payloads, snapshots, receipts, and public projections. OS-principal key loss happens before
this process and does not currently produce these events.

Resource search, including `ON_DEMAND_CONTENT`, emits no domain event: it creates no durable
Resource/index state. Content-mode matches and snippets are request-local projections and
must not be copied into domain events, ordinary logs, or aggregate snapshots.
| `conversation.created` | `conversation_id`, `created_by` |
| `conversation.message.added` | `message_id`, `conversation_id`, `author`, `content_digest`, `resource_refs` |
| `conversation.turn.created` | `turn_id`, `conversation_id`, `user_message_id`, `agent_binding_id`, `aggregate_version` |
| `rich.presentation.published` | `presentation_id`, `conversation_id`, `message_id`, `schema_version`, `renderer_contract_version`, `semantic_content_digest`, `document_digest`, `document_size_bytes`, `producer_agent_session_id?`, `host_instruction_digest?`, `host_skill_refs[]`, `aggregate_version` |
| `conversation.turn.retried` | `turn_id`, `prior_agent_session_id?`, `agent_session_id`, `retry_ordinal`, `aggregate_version` |
| `conversation.turn.resumed` | `turn_id`, `user_request_id`, `prior_agent_session_id?`, `agent_session_id`, `aggregate_version` |
| `conversation.turn.status.changed` | `turn_id`, `from`, `to`, `reason_code`, `agent_session_id?`, `aggregate_version` |
| `conversation.turn.settled` | `turn_id`, `from`, `to`, `reason_code?`, `agent_session_id?`, `aggregate_version` |
| `task.spec.revised` | `task_id`, `revision`, `parent_revisions[]`, `spec_digest`, `authored_by` |
| `task.plan.revised` | `task_id`, `revision`, `task_spec_revision`, `produced_by_agent_session_id`, `produced_by_attempt_id?`, `step_ids[]`, `aggregate_version` |
| `task.lead_agent.changed` | `task_id`, `from_agent_binding_id`, `to_agent_binding_id`, `cause`, `actor`, `task_spec_revision`, `trigger_observation?`, `aggregate_version`; policy failover requires a typed, fresh trigger observation |
| `task.status.changed` | `task_id`, `from`, `to`, `reason_code`, `actor`, `blocking_conditions[]`, `aggregate_version` |
| `task.pause.requested` | `task_id`, `resume_status`, `request_id`, `requested_by`, `aggregate_version` |
| `task.paused` | `task_id`, `resume_packet_ref`, `resume_packet_digest`, `settled_attempt_ids[]`, `released_lease_ids[]`, `aggregate_version` |
| `task.resumed` | `task_id`, `from`, `to`, `resumed_by`, `aggregate_version` |
| `step.created` | `step_id`, `task_id`, `plan_revision`, `logical_key?`, `dependencies[]` |
| `step.status.changed` | `step_id`, `task_id`, `from`, `to`, `reason_code`, `aggregate_version` |
| `step.recovery.accepted` | `task_id`, `step_id`, `prior_attempt_id`, `new_attempt_id`, `retry_ordinal`, `recovery_reason`, `aggregate_version` |
| `attempt.created` | `attempt_id`, `task_id`, `step_id`, `parent_attempt_id` (nullable), `agent_binding_id`, `delegation_profile_id` (nullable), `delegation_profile_revision` (nullable), `runtime_id`, `runtime_incarnation_id`, `environment_id`, `lease_id`; delegated attempts require the parent and pinned profile pair together |
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
| `capability.invocation.created` | `invocation_id`, `scope`, `agent_session_id`, `capability_grant_id`, `activation_id`, `operation`, `request_digest`, `execution_method`, `action_batch?`, `aggregate_version` |
| `capability.invocation.dispatched` | `invocation_id`, `dispatch_ordinal`, `execution_method`, `provider_task_status?`, `provider_task_created_at?`, `provider_task_expires_at?`, `provider_task_ttl_ms?`, `provider_poll_after_ms?`, `dispatched_at`, `aggregate_version` |
| `capability.invocation.checkpointed` | `invocation_id`, `provider_task_status?`, `provider_task_expires_at?`, `provider_task_ttl_ms?`, `provider_updated_at?`, `provider_poll_after_ms?`, `partial_result_refs[]`, `observed_at`, `aggregate_version` |
| `capability.invocation.status.changed` | `invocation_id`, `from`, `to` (status values: CREATED, DISPATCHED, WAITING, INPUT_REQUIRED, CANCEL_REQUESTED, SUCCEEDED, FAILED, CANCELLED, AMBIGUOUS), `result_refs[]`, `effect_id?`, `failure_code?`, `aggregate_version` |
| `usage.observed` | `usage_observation_id`, `task_id?`, `environment_id?`, `attempt_id?`, `invocation_id?`, `metric`, `quantity`, `unit`, `currency?`, `confidence`, `observed_at` (`quantity` is null only when `confidence` is UNKNOWN) |
| `budget.reservation.changed` | `reservation_id`, `budget_scope`, `task_id?`, `environment_id?`, `attempt_id?`, `metric`, `quantity`, `unit`, `currency?`, `from?`, `to`, `aggregate_version` |
| `capability.lock.created` | `task_id`, `capability_ref`, `aggregate_version` |
| `secret.lease.*` | `secret_lease_id`, `task_id`, `attempt_id?`, `runtime_id`, `scope_digest`, `expires_at?` |
| `artifact.created` | `artifact_id`, `workspace_id`, `resource_id`, `task_id?`, `kind`, `current_version`, `library_status`, `aggregate_version` |
| `artifact.version.created` | `artifact_id`, `resource_id`, `version`, `resource_revision_id`, `input_refs[]`, `content_kind`, `content_digest?`, `storage_ref?`, `resource_ref?`, `provider_revision?`, `created_by_attempt?`, `aggregate_version` |
| `artifact.library.promoted` | `artifact_id`, `from`, `to`, `aggregate_version` |
| `artifact.library.archived` | `artifact_id`, `from`, `to`, `aggregate_version` |
| other `effect.*` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal?` |
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
| `automation.revision.created` | `automation_id`, `revision`, `definition_digest`, `authored_by`, `coworker_ref?` |
| `automation.status.changed` | `automation_id`, `from`, `to`, `aggregate_version` |
| `automation.occurrence.*` | `occurrence_id`, `automation_id`, `automation_revision`, `routine_id`, `routine_revision`, `trigger_id`, `trigger_host_runtime_id`, `occurrence_key`, `version`, `claim_epoch`, `claim_expires_at?`, `trigger_input_ref?`, `trigger_payload_digest?`, `task_id?`, `from?`, `to` |
| `connection.state.changed` | `connection_id`, `from`, `to`, `provider_ref` |
| `channel.binding.changed` | `channel_binding_id`, `connection_id?`, `from`, `to`, `assurance_level`, `allowed_actions[]` |
| `channel.host.assignment.changed.v1` | `channel_binding_id`, `from_runtime_id?`, `runtime_id`, `host_epoch`, `status`, `ingress_continuity`, `ingress_gap_since?`, `aggregate_version` |
| `channel.inbound.received` | `channel_binding_id`, `provider_event_id`, `origin_runtime_id`, `origin_host_epoch`, `ingress_sequence`, `event_kind`, `payload_digest` |
| `channel.receipt.changed` | `channel_binding_id`, `provider_event_id`, `claim_runtime_id`, `claim_host_epoch`, `from`, `to`, `claim_epoch`, `claim_expires_at?` |
| `channel.outbound.settled` | `channel_binding_id`, `runtime_id`, `host_epoch`, `delivery_id`, `provider_event_id?`, `state`, `result_digest?` |
| `handoff.*` | `handoff_id`, `task_id`, `step_id`, `source_attempt_id`, `source_runtime_id`, `target_runtime_id?`, `phase` |
| `audit.record.created` | `audit_record_id`, `principal`, `action`, `decision`, `reason_code`, `payload_digest?` |
| `agent.binding.created` | `agent_binding_id`, `workspace_id`, `agent_profile_id`, `runtime_id?`, `from_enabled`, `to_enabled`, `aggregate_version`, `requested_by` |
| `channel.host.assignment.changed.v2` | `channel_binding_id`, `from_runtime_id?`, `runtime_id`, `host_epoch`, `status`, `ingress_continuity`, `ingress_gap_since?`, `source_release_basis?`, `source_drain_proof_ref?`, `source_drain_proof_digest?`, `continuity_proof_ref?`, `continuity_proof_digest?`, `ingress_gap_decision?`, `aggregate_version` |
| `conversation.turn.created.v2` | `turn_id`, `conversation_id`, `user_message_id`, `agent_binding_id`, `aggregate_version`, `presentation_preference` |
| `effect.acknowledged` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal?` |
| `effect.ambiguous` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `ambiguity_reason_digest`, `dispatch_ordinal?` |
| `effect.failed` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `failure_code`, `failure_digest?`, `failure_retryable`, `dispatch_ordinal?` |
| `effect.observed` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `observation_evidence_id`, `dispatch_ordinal?` |
| `effect.proposed` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal`, `capability_invocation_id`, `execution_method` |
| `effect.reconciliation.started` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal?` |
| `effect.retry.authorized` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `retry_evidence_id`, `retry_basis`, `dispatch_ordinal?` |
| `effect.started` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal?` |
| `effect.verified` | `effect_id`, `task_id`, `attempt_id`, `from?`, `to`, `operation`, `target_digest`, `dispatch_ordinal?` |
| `resource.created.v2` | `resource_id`, `workspace_id`, `kind`, `identity_digest?`, `provenance`, `aggregate_version`, `context_document?` |
| `resource.revision.created` | `resource_id`, `resource_revision_id`, `parent_revision_ids[]`, `content_digest`, `size_bytes`, `media_type`, `created_by`, `aggregate_version` |

| `task.created` | `task_id`, `conversation_id?`, `initial_spec_revision`, `created_by`, `origin_coworker_id`, `origin_coworker_revision`, `routine_id?`, `routine_revision?`, `automation_id?`, `automation_occurrence_id?` |
| `workspace.primary_coworker.changed` | `workspace_id`, `from_coworker_id`, `to_coworker_id`, `changed_by`, `aggregate_version` |
| `suggestion.preference.changed` | `workspace_id`, `kind`, `from_muted`, `to_muted`, `changed_by`, `aggregate_version` |
| `task.coworker.origin.pinned` | `task_id`, `coworker_id`, `coworker_revision`, `aggregate_version` |
| `agent.binding.lead_eligibility.changed` | `agent_binding_id`, `workspace_id`, `from`, `to`, `aggregate_version`, `requested_by` |
| `agent.session.harness_descriptor.pinned` | `agent_session_id`, `scope`, `task_spec_revision` (nullable for conversation scope), `descriptor_digest`, `features_digest`, `effective_config_digest?`, `observed_at` |
| `delegation_profile.created` | `delegation_profile_id`, `workspace_id`, `agent_binding_id`, `current_revision`, `status`, `aggregate_version` |
| `delegation_profile.revised` | `delegation_profile_id`, `revision`, `revision_digest`, `authored_by`, `aggregate_version` |
| `delegation_profile.status.changed` | `delegation_profile_id`, `from`, `to`, `aggregate_version` |
| `delegation.admitted` | `parent_attempt_id`, `child_attempt_id`, `step_id`, `delegation_profile_id`, `delegation_profile_revision`, `agent_binding_id`, `endpoint_id`, `runtime_id`, `runtime_incarnation_id`, `environment_id`, `descriptor_digest`, `selection_policy`, `eligible_candidate_count`, `candidate_set_digest`, `selection_policy_version`, `budget_reservation_ids[]`, `capability_grant_ids[]`, `aggregate_version` |
| `coworker.created` | `coworker_id`, `workspace_id`, `current_revision`, `status`, `aggregate_version` |
| `coworker.revised` | `coworker_id`, `revision`, `revision_digest`, `authored_by`, `aggregate_version` |
| `coworker.status.changed` | `coworker_id`, `from`, `to`, `aggregate_version` |
| `goal.created` | `goal_id`, `workspace_id`, `current_revision`, `status`, `aggregate_version` |
| `goal.revised` | `goal_id`, `revision`, `revision_digest`, `authored_by`, `aggregate_version` |
| `goal.status.changed` | `goal_id`, `from`, `to`, `aggregate_version` |
| `suggestion.proposed` | `suggestion_id`, `workspace_id`, `coworker_id?`, `dedupe_key`, `kind`, `source_refs[]`, `goal_refs[]`, `proposed_action`, `proposed_by`, `latency_class_hint?`, `proposal_digest`, `expires_at`, `aggregate_version` |
| `suggestion.resolved` | `suggestion_id`, `from`, `to`, `resolved_by`, `resolution_reason`, `result_task_id?`, `aggregate_version` |
| `suggestion.visibility.changed` | `suggestion_id`, `from_snoozed_until`, `to_snoozed_until`, `changed_by`, `aggregate_version` |
| `demonstration.status.changed` | `demonstration_session_id`, `environment_id`, `from`, `to`, `capture_policy_digest`, `captured_action_count`, `captured_trace_bytes`, `trace_resource_id?`, `skill_proposal_id?`, `aggregate_version` |
| `environment.sharing_scope.changed` | `environment_id`, `from`, `to`, `changed_by`, `aggregate_version` |

`conversation.turn.status.changed` records nonterminal state changes. Its `to` values are
`RUNNING`, `WAITING_USER`, `WAITING_DEPENDENCY`, and `CANCEL_REQUESTED`; `reason_code` is
required. `conversation.turn.settled` is reserved for terminal outcomes, and its `to`
values are `COMPLETED`, `FAILED`, and `CANCELLED`.

`channel.host.assignment.changed.v1` remains unchanged. New assignment history that needs
source-release or ingress-gap provenance uses `channel.host.assignment.changed.v2`; the
Operator projection adds nullable provenance fields without changing the meaning of older
records. `source_drain_proof_ref` is an opaque `ChannelHostDrainProofRef` containing the
SQL proof row's `proof_id` plus its `channel_binding_id`, `workspace_id`, `runtime_id`, and
`host_epoch`; it is not a `ResourceRef`. `source_drain_proof_digest` is the row's
`proof_digest`. For a cross-Runtime assignment that becomes `ACTIVE`, v2 MUST record
`source_release_basis`: `SAFE_SETTLEMENT_PROOF` requires both the scoped
`source_drain_proof_ref` and its digest; SQL `release_kind = QUIESCENT` maps to
`SAFE_SETTLEMENT_PROOF`. SQL `release_kind = EXPIRY_PLUS_SKEW` maps to
`LEASE_EXPIRY_PLUS_SKEW`, records that the Hub-authoritative source lease reached its
pinned `safe_reassign_after`, and MUST omit both proof fields. For current assignment
epoch N, the projection joins the immutable release row for epoch N-1; for QUIESCENT it
joins `drain_proof_id` to the immutable proof row and exposes the scoped reference and
`proof_digest`. Epoch 1 has no prior release and therefore no source-release provenance.
The proof is a non-secret record of source settlement/reconciliation; never include
credentials, provider message references, or raw provider payloads in it or in the event.

For a cross-Runtime `ACTIVE` assignment with `ingress_continuity = CONTINUOUS`, v2 also
includes `continuity_proof_ref` and `continuity_proof_digest`. The opaque reference contains
the immutable SQL continuity-proof `proof_id` and its assignment scope: binding, Workspace,
source Runtime/epoch, and target Runtime/epoch. Its digest is the row's `proof_digest`.
Projection joins the proof row for target epoch N and verifies its source scope is the prior
assignment at epoch N-1. `GAP_ACCEPTED` assignments have no continuity proof; the explicit
owner decision is represented by `ingress_gap_decision`. Initial assignments and legacy
rows with no retained proof provenance expose neither continuity-proof field.

RuntimeMesh service validation MUST compare the event target `runtime_id` with the prior
assignment: the source-release basis is required only when they differ, and
`from_runtime_id` must equal that exact prior Runtime. The service must reject source proof
or expiry provenance for same-Runtime resume and reject a cross-Runtime activation with
missing/mismatched release provenance. It resolves the release row for the prior
`channel_binding_id`/`workspace_id`/`runtime_id`/`host_epoch`, validates the release kind
and its eligibility (`QUIESCENT` with its proof row, or `EXPIRY_PLUS_SKEW` only after
`safe_reassign_after`), and for a proof verifies that `proof_id`, binding, Workspace,
source Runtime, and source epoch exactly match that immutable proof row and prior
assignment. JSON Schema validates field shape and proof pairing, but cannot compare a
payload's Runtime IDs with each other or with prior aggregate state.

For an ingress-continuous cross-Runtime activation, the service also verifies that the
continuity-proof reference and digest identify the immutable proof row for the exact
channel binding and Workspace, that its source Runtime/epoch equals the prior assignment,
that its target Runtime/epoch equals the new assignment, and that its digest matches the
row. It rejects missing proof on that path and rejects continuity-proof provenance on a
gap-accepted or same-Runtime resume path. The reference is opaque and non-authorizing; it
is not a `ResourceRef`.

When a v2 assignment change records `ingress_continuity = GAP_ACCEPTED`, it MUST include
`ingress_gap_decision` with the authenticated owner `decided_by`, `decided_at`,
`audit_record_id`, and nonempty `reason_code`. The referenced audit record records the
explicit acceptance command and its decision. The assignment stores
`ingress_gap_decision_audit_id`; projection joins that ID to the same-Workspace immutable
AuditRecord and maps its principal, `occurred_at`, and `reason_code` to the decision
object. The AuditRecord is committed atomically with the assignment event, names the same
principal, uses action `channel.host.ingress_gap.accept` and decision `ALLOW`, and carries
the same reason code. SQLite triggers enforce record identity, Workspace, action, decision,
and ChannelBinding reference. RuntimeMesh service validation additionally checks exact
actor, timestamp and reason-code equality with the AuditRecord, plus that its
`payload_digest` covers the canonical non-secret command identity, target Runtime,
expected assignment version, and explicit acceptance flag. These fields are provenance,
not authority that can be replayed. Legacy
`GAP_ACCEPTED` events without this object remain valid for
history; a v1 projection shows decision provenance as unavailable and MUST NOT infer an
actor or synthesize an audit reference. Consumers validate v2 provenance according to its
conditional requirements and continue to decode v1 with its original closed schema.

For `suggestion.proposed.v1`, every `source_refs[]` entry is a `PinnedResourceRef` and
every `goal_refs[]` entry is a `GoalRevisionRef`; both types pin exact same-Workspace
revisions.

`CapabilityInvocation.execution_method` is chosen by the adapter before Invocation
creation; `capability.invocation.dispatched.v1` repeats the immutable value. A batch's
members repeat the same route and ordered batch digest. `UNKNOWN` remains explicit when
the provider cannot prove a more specific route; the UI must not infer one from agent text.

### Ownership and replication for responsibility/delegation events

| Event | Owning aggregate / transition owner | Replication rule |
|---|---|---|
| `workspace.primary_coworker.changed.v1` | Workspace / WorkspaceService | Workspace state; Coworker must be same Workspace and ACTIVE or PAUSED |
| `suggestion.preference.changed.v1` | SuggestionPreference / SuggestionService | Workspace-scoped preference replicates; muting also resolves currently proposed items of that kind in the same transaction |
| `task.coworker.origin.pinned.v1` | Task / TaskService | Immutable Task origin provenance; Coworker revision is pinned |
| `agent.binding.lead_eligibility.changed.v1` | AgentBinding / AgentBindingService | Workspace authorization state; no endpoint locator or credentials |
| `agent.session.starting.v1` | AgentSession / AgentSessionStore | Reserves a Task planner slot before native startup; includes only normalized scope/selection IDs and Runtime incarnation, never a native handle |
| `agent.session.started.v1` | AgentSession / AgentSessionSupervisor | Adapter readiness and ACTIVE state; first planning also transitions Task to RUNNING atomically |
| `agent.session.harness_descriptor.pinned.v1` | AgentSession / AgentSessionSupervisor | Non-secret descriptor/feature/config digests and session scope only; no descriptor contents or handles |
| `delegation_profile.*.v1` | DelegationProfile / DelegationProfileService | Profile revisions/status replicate; provider-native option values remain opaque, non-secret adapter-owned values |
| `delegation.admitted.v1` | Task / TaskService admission transaction | Attempt/profile/revision/grant/reservation identity is durable; candidate digest contains no prompts or secret config |
| `coworker.*.v1` | Coworker / CoworkerService | Identity and immutable preferences replicate under Workspace policy; no agent session state |
| `goal.*.v1` | Goal / GoalService | Immutable Goal revision/status and same-Workspace references replicate |
| `suggestion.*.v1` | Suggestion / SuggestionService owner actions; TaskService atomic admission for TASK acceptance | Proposal digest, provenance refs, status, and result Task ref replicate; source content remains in Resources |
| `suggestion.visibility.changed.v1` | Suggestion / SuggestionService | Visibility timestamps and owner identity only; snooze cannot extend past expiry, null clears a snooze |
| `demonstration.status.changed.v1` | DemonstrationSession / DemonstrationSessionService | Status and trace Resource ID only; semantic trace Resource follows normal replication policy |
| `resource.context_document.status.changed.v1` | Resource / ResourceService | Status tombstone and owner identity replicate; never includes document content |
| `resource.context_document.purge.acknowledged.v1` | Resource / ContextDocumentPurgeReconciler | Replica identity, target revisions, Runtime incarnation (when applicable), and receipt digest only; no content or locator |
| `environment.sharing_scope.changed.v1` | Environment / EnvironmentManager | Scope change requires expected-version and current-use check; provider locators remain local |

Accepting a `TASK` Suggestion appends the ordinary `task.created.v1` event and
`suggestion.resolved.v1` (`to=ACCEPTED`, `resolution_reason=ACCEPTED_BY_OWNER`,
`result_task_id=<created Task>`) with both aggregate snapshots in the same SQLite
transaction and Workspace event-sequence allocation. A receiving Runtime can therefore
observe the linked Task and terminal Suggestion together; neither event is emitted if the
transaction aborts. Acceptance creates only a `READY` Task and does not dispatch work.
The Operator returns a separate bounded `SuggestionTaskAcceptanceReceipt`: the initial
receipt is constructed from the same transaction's committed aggregate values, while an
idempotent replay validates the persisted Task request receipt and accepted Suggestion
together. The receipt is not another event and does not alter event payloads or sequence
allocation.

The revision digest in `goal.revised.v1` covers the complete immutable GoalRevision,
including Task IDs, exact RoutineRevision references, and exact `ArtifactVersionRef`
values. The event does not copy Artifact content or change the Artifact aggregate.

`QuotaObservation`, candidate ranking detail, warm process state, WorkerPerformance,
TaskProgress, GoalProgress, RoutineHealth, and Coworker presence are operational or
rebuildable projections, not new replicated domain events. Usage/budget records continue
to use their existing event family. Do not encode per-token streams, temporary worker
messages, native session IDs, process IDs, secret bytes, or cost values that the provider
did not report.

Payloads never contain raw secret bytes, native hidden prompts, or unbounded terminal,
video, or token streams. ResourceRef and digest carry large/sensitive payloads by
reference.

`rich.presentation.published.v1` belongs to the independent immutable RichPresentation
aggregate. It binds one committed AGENT ConversationMessage using exact Conversation and
Workspace identities, semantic/document digests, schema versions, bounded document size,
optional AgentSession provenance, and non-secret HostSkill refs. The canonical document
and BlobRef are in the aggregate-state record; the event does not duplicate the document.
It is emitted only after the semantic message exists. It does not update or supersede
`conversation.message.added.v1`, and may be replicated/applied later. A missing document
blob leaves the rich aggregate pending/unavailable while semantic Conversation state stays
usable. Per-token deltas, rich draft operations, renderer state, checklist state, downloads,
and rendering telemetry are transient or local and never become DomainEvents.

Legacy `conversation.turn.created.v1` records project `presentation_preference=AUTO`.
New turn admission writes the versioned v2 payload; a preference change is not a later
mutation of the turn. There is intentionally no message-event v2 for presentation and no
event for renderer/checklist interaction state.

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
  origin_coworker_id?
  origin_coworker_revision?
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

Local Library commands append `artifact.library.promoted.v1` or
`artifact.library.archived.v1` with the post-transition aggregate state and receipt in the
same transaction. Recorded replay and fresh current-version already-archived archive do
not append an event. No Resource revision event is produced by a Library-only transition.

## Event versioning

`resource.created.v1` keeps its original closed payload. Resource creation with
`provenance.folder_import` emits `resource.created.v2`, whose nested provenance schema
explicitly permits the new field.

An owner-authored upload that appends bytes to an existing managed Resource emits
`resource.revision.created.v1`; it does not emit `resource.created`. The event carries the
exact parent head set and verified content digest/size/media type. The immutable revision,
parent edges, current Resource head/version, managed local location, revision-scoped index
projection, dependent invalidations, upload lifecycle, event/snapshot, and idempotency
receipt commit atomically. If the Resource version or complete head set changed since upload
admission, commit conflicts and does not publish a revision. Replaying a committed upload
returns its stored commit receipt rather than appending a second revision.

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

## Proposed Coworker event family (not in current allowed event types)

When owning aggregate/schema implementation is ready, introduce typed, redacted events for immutable Coworker-owned Conversation association, Coworker capability assignment creation/change/revocation, memory policy revision, candidate evaluation and Resource-backed memory commit/supersession, StandingResponsibility create/revise/status. Persisted health changes are optional; prefer derived health from real Task/trigger state. Every event requires authenticated actor, Workspace, entity, committed revision, source digest, request/causation and correlation IDs. Never include memory plaintext, credentials, OAuth URLs, native hidden reasoning or provider task handles. Until added to domain-event.schema.json and tests, these names are proposals only, not valid emitted events. Full draft names are in [Coworker target](COWORKERS-TARGET.md).
