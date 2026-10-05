# Storage LLD

## Storage ports

```text
StateStore
EventStore
BlobStore
```

Domain/application code depends on ports, never SQLite/Postgres/S3 directly.

`runtime_incarnations` is the compact authenticated Runtime Mesh registry required to
validate references from replicated aggregates. Local OS boot IDs and diagnostics live in
`runtime_incarnation_local_observations`, which is never replicated or backed up.
`agent_host_instances`, `agent_session_host_bindings`, `capability_host_instances`,
`capability_activation_host_bindings`, `environment_provider_bindings`, and
`environment_checkpoint_provider_bindings`, `agent_endpoint_bindings`, `resource_location_bindings`, and
`file_identity_bindings` are Runtime-local operational state, not Workspace replication
aggregates. Workspace backup/restore omits these rows and their opaque process/provider/
session handles, raw paths, locators, and OS file identifiers; a restored or restarted
Runtime rebuilds host views from current offers and re-probes or reattaches before
admission.
Agent session and capability activation counts are derived from local bindings joined to
nonterminal durable session/activation records, not stored or copied between Runtimes.
Environment checkpoint metadata may replicate, but provider-only handles do not. Only a
checkpoint with an exported `portable_snapshot_ref` identifies transferable bytes; its
blob is included in a Workspace backup only when the Environment's pinned `backup_policy`
is `INCLUDE_CHECKPOINTS` and the Workspace replication policy also permits it.

## Local deployment

- SQLite 3.38 or later with JSON functions enabled for relational/domain projections
  and the local event index.
- local content-addressed blob directory for artifacts/checkpoints.
- immutable aggregate-state records referenced by each DomainEvent; these are separate
  from periodic projection-rebuild snapshots and stored in the same BlobStore.

## Personal/self-hosted server

- SQLite acceptable for single-user/single-hub use.
- filesystem or S3-compatible BlobStore.

## Managed/multi-user

- Postgres StateStore/EventStore implementation.
- S3-compatible BlobStore.
- queue/stream implementation behind internal port if needed.

## Relational tables

Minimum tables:

```text
workspaces
workspace_instruction_revisions
conversations
conversation_messages
conversation_turns

tasks
task_spec_revisions
plan_revisions
steps
attempts

delegation_profiles
delegation_profile_revisions
coworkers
coworker_revisions
goals
goal_revisions
goal_task_links
goal_routine_links
suggestions
suggestion_preferences
demonstration_sessions

agent_profiles
agent_endpoints
agent_endpoint_bindings
agent_bindings
agent_sessions
agent_session_host_bindings

runtimes
runtime_offers
provider_circuit_states
environments
environment_provision_previews
environment_checkpoints
environment_provider_bindings
environment_checkpoint_provider_bindings
execution_leases

capability_grants
secret_leases
capability_activations
capability_activation_host_bindings
capability_host_instances
capability_locks
connections
channel_bindings
channel_host_assignments # replicated Workspace routing aggregate; host credential excluded
channel_thread_mappings
channel_event_receipts

artifacts
artifact_versions

effects
evidence
verification_runs
approvals
approval_uses

resources
resource_revisions
resource_revision_parents
resource_locations
resource_location_bindings
file_identity_bindings
workspace_roots
workspace_replication_roots
resource_edges
dependency_edges
invalidation_records
capability_invocations
user_requests
user_request_responses
usage_observations
budget_reservations
notification_preferences
notification_deliveries
skill_proposals
environment_control_leases
resource_upload_sessions
resource_upload_chunks
aggregate_snapshots
event_archive_segments
workspace_backup_manifests

runtime_incarnations
runtime_incarnation_local_observations # Runtime-local OS boot ID/diagnostic data; excluded from backup/replication
agent_host_instances                   # local operational records; not Task replication truth
agent_session_host_bindings            # local opaque agent handles; excluded from backup/replication
capability_activation_host_bindings    # local opaque provider handles; excluded from backup/replication
capability_host_instances              # local provider observations/use groups; not LitePSM package truth
capability_invocation_provider_bindings # encrypted provider task handles/cursors; local-only
provider_input_bindings                 # encrypted MCP input envelope/keys and response outbox; local-only
automation_trigger_bindings             # encrypted provider cursors; local-only, host-epoch scoped
channel_ingress_cursor_bindings          # encrypted provider cursors; local-only, host-epoch/incarnation scoped
channel_reply_targets                    # opaque provider message refs; local-only, excluded from backup/replication
channel_host_lease_records               # Hub-only renewable control state; digest only, not Workspace backup
routines
routine_revisions
automations
automation_revisions
automation_occurrences
automation_cursors
handoffs
audit_records
pairing_tokens
replication_cursors
replication_receipts
replication_aggregate_positions
pending_replication_events

domain_events
request_dedup
```

## Required constraints/indexes

```text
UNIQUE task_spec_revisions(task_id, revision)
UNIQUE plan_revisions(task_id, revision)
UNIQUE artifact_versions(artifact_id, version)
UNIQUE artifact_versions(artifact_id, resource_id, version)
FOREIGN KEY artifacts(artifact_id, current_version) -> artifact_versions(artifact_id, version), deferred
FOREIGN KEY artifacts(artifact_id, resource_id, current_version) -> the matching ArtifactVersion and Resource revision
FOREIGN KEY artifact_versions(resource_id, resource_revision_id) -> resource_revisions(resource_id, resource_revision_id)
trigger guard_artifact_initial_revision -> initial ArtifactVersion ResourceRevision equals Resource head
trigger guard_artifact_current_revision -> Artifact current-version change names current Resource head
UNIQUE domain_events(event_id)
UNIQUE domain_events(workspace_id, origin_runtime_id, origin_sequence)
UNIQUE replication_receipts(workspace_id, receiver_runtime_id, origin_runtime_id, origin_sequence)
UNIQUE automation_revisions(automation_id, revision)
UNIQUE routine_revisions(routine_id, revision)
UNIQUE delegation_profile_revisions(delegation_profile_id, revision)
UNIQUE non-archived delegation profile name_key(workspace_id, agent_binding_id)
UNIQUE coworker_revisions(coworker_id, revision)
UNIQUE goal_revisions(goal_id, revision)
UNIQUE goal_task_links(goal_id, revision, task_id)
UNIQUE goal_routine_links(goal_id, revision, routine_id, routine_revision)
UNIQUE automation_occurrences(automation_id, trigger_id, occurrence_key)
UNIQUE INDEX uq_suggestions_open_dedupe(workspace_id, dedupe_key) WHERE status = 'PROPOSED'
INDEX suggestions(workspace_id, dedupe_key, resolved_at DESC) WHERE status = 'DISMISSED' # 30-day exact-key cooldown lookup
PRIMARY KEY automation_cursors(automation_id, trigger_id)
UNIQUE channel_event_receipts(channel_binding_id, origin_host_epoch, ingress_sequence)
PRIMARY KEY channel_ingress_cursor_bindings(channel_binding_id, host_epoch)
FOREIGN KEY automation_cursors(automation_id,active_automation_revision) -> automation_revisions
FOREIGN KEY runtime-bound handles(runtime_id,runtime_incarnation_id) -> runtime_incarnations
RuntimeIncarnation catalog is authenticated Workspace Mesh metadata; persist it before accepting any aggregate that references the incarnation
runtime_incarnation_local_observations is local-only and excluded from Workspace backups/replication
environment_provider_bindings, environment_checkpoint_provider_bindings, agent_endpoint_bindings, resource_location_bindings, and file_identity_bindings are local-only, exact-incarnation bindings excluded from Workspace backups/replication
capability_invocation_provider_bindings, provider_input_bindings, and automation_trigger_bindings are encrypted Runtime-local operational state and are excluded from Workspace backups/replication; restore marks them unavailable and requires provider reconciliation or a bounded rescan
channel_reply_targets are Runtime-local message-correlation state and are excluded from Workspace backups/replication; restore cannot resolve replies to prior channel prompts, which remain actionable through the Operator inbox
channel_ingress_cursor_bindings are encrypted Runtime-local operational state, excluded from Workspace backups/replication; their last-committed sequence must reference a durable ChannelEventReceipt
channel_event_receipts keep immutable origin fields; only RECEIVED -> PROCESSING, fenced/expired PROCESSING -> PROCESSING reclaim, and current-lease PROCESSING -> terminal are legal
the stable Runtime identity HMAC key used to pseudonymize file identity is stored in the OS keystore, never in SQLite or Workspace backup
portable Environment checkpoint blobs are included only when `backup_policy = INCLUDE_CHECKPOINTS` and Workspace replication permits
FOREIGN KEY capability_host_instances(runtime_id,runtime_incarnation_id) -> runtime_incarnations
FOREIGN KEY environment_provider_bindings(environment_id,runtime_id,provider_kind) -> environments
FOREIGN KEY environment_provider_bindings(runtime_id,runtime_incarnation_id) -> runtime_incarnations
FOREIGN KEY environment_checkpoint_provider_bindings(checkpoint_id) -> environment_checkpoints
FOREIGN KEY environment_checkpoint_provider_bindings(runtime_id,runtime_incarnation_id) -> runtime_incarnations
environment/checkpoint provider bindings must match the Environment Runtime/provider and current Runtime incarnation at admission
FOREIGN KEY resource_location_bindings(location_id,locator_ref_id) -> resource_locations
FOREIGN KEY resource_location_bindings(runtime_id,runtime_incarnation_id) -> runtime_incarnations
resource_location_bindings must match the durable location key and current local Runtime incarnation
FOREIGN KEY file_identity_bindings(location_id) -> resource_locations
FOREIGN KEY file_identity_bindings(runtime_id,runtime_incarnation_id) -> runtime_incarnations
file_identity_bindings must match the current local Runtime incarnation; the Runtime HMAC key is held by the OS keystore and never enters a Workspace backup
FOREIGN KEY agent_endpoint_bindings(endpoint_id) -> agent_endpoints
FOREIGN KEY agent_endpoint_bindings(runtime_id,runtime_incarnation_id) -> runtime_incarnations
agent_endpoint_bindings must match the current Runtime incarnation; endpoint refs are private command/socket/URL locators
AgentEndpointBinding locator refresh requires same endpoint/Runtime/incarnation; deletion requires its AgentHost instances drained
FOREIGN KEY agent_session_host_bindings(agent_session_id) -> agent_sessions
FOREIGN KEY agent_session_host_bindings(host_instance_id) -> agent_host_instances
AgentSessionHostBinding must match the AgentSession Runtime/incarnation, selected endpoint, and profile of the joined AgentHostInstance
AgentSession.endpoint_id is an immutable historical identity and has no FK to expiring Runtime-local agent_endpoints
AgentSessionHostBinding deletion is allowed only after AgentSession is CLOSED or LOST
FOREIGN KEY capability_activations(runtime_id,runtime_incarnation_id) -> runtime_incarnations
CapabilityActivation scope tuple must satisfy its tagged-union CHECK and point to the same Workspace as its scope entity
CapabilityActivation scope, Runtime/incarnation, and CapabilityRef must match the session/invocation at admission
FOREIGN KEY capability_activation_host_bindings(activation_id) -> capability_activations
FOREIGN KEY capability_activation_host_bindings(host_instance_id) -> capability_host_instances
CapabilityActivationHostBinding must match the exact normalized CapabilityRef, Runtime, and incarnation of both joined records
CapabilityActivationHostBinding deletion is allowed only after Activation is FAILED or STOPPED

delegation profile, coworker, goal, suggestion, and demonstration aggregates replicate as
Workspace domain state; immutable revisions remain available to Tasks that pinned them
Task origin Coworker revision and child Attempt DelegationProfile revision are immutable
provenance references; insert guards enforce the Workspace, AgentBinding, enabled-profile,
and pinned Coworker allowlist relationship
DelegationProfile current name/name_key is a projection of its exact current revision; a
partial unique index on `(workspace_id, agent_binding_id, name_key)` applies only to
non-archived profiles, and revision rows reject update/delete
Workspace primary_coworker_id must reference an ACTIVE or PAUSED Coworker in the same Workspace; clearing it is explicit
Goal related Task/Routine references are same-Workspace and revision-pinned; they are stored on GoalRevision and do not own or mutate linked records
Suggestion deduplication applies only while PROPOSED; resolved suggestions remain in history
CoworkerRevision and GoalRevision rows reject update/delete; GoalService inserts a new revision, advances its head, then inserts that revision's same-Workspace immutable link rows in one transaction
Goal link rows may be inserted only for the current non-archived revision and reject update/delete
Suggestion proposal content/provenance is immutable after creation; only snooze and terminal resolution fields may change under SuggestionService version checks
suggestion_preferences stores only explicit Workspace kind settings; absent rows project as unmuted/version 0
Suggestion snooze is versioned and bounded by expiry; dismissed-key cooldown uses `resolved_at` and `DISMISSED_BY_OWNER` only
DemonstrationSession Environment, trace Resource, and SkillProposal references must all
belong to its Workspace; it records semantic trace ResourceRefs, never raw credential
values or unbounded screen recordings in relational rows
AgentHarnessDescriptor and QuotaObservation are time-bounded observations; only non-secret descriptor digests may be pinned to AgentSession/event history, and provider-private handles are excluded from backup/replication
WorkerPerformance, GoalProgress, TaskProgress, RoutineHealth, and Coworker presence are rebuildable projections, not canonical backup aggregates
ContextDocuments are versioned Resources and follow the Workspace Resource backup/replication policy; provider embeddings/indexes are rebuildable and never authoritative
Coworker revisions may name only same-Workspace enabled worker profiles and an enabled,
lead-eligible default binding at revision creation; future disablement is rechecked at
admission
LiteCowork active_activation_count -> derived COUNT of nonterminal Activations joined through local host bindings; never stored as an independent counter
AgentHostInstance.active_session_count -> derived COUNT of nonterminal AgentSessions joined through local host bindings; never stored as an independent counter
UNIQUE request_dedup(principal_id, request_id)
UNIQUE approval_uses(approval_id)
UNIQUE capability_locks(task_id, capability_ref_key_digest)
UNIQUE capability_locks(task_id, identity_kind, source, capability_id, component)
UNIQUE notification_deliveries(workspace_id, dedupe_key)
UNIQUE user_request_responses(request_id)
UNIQUE resource_upload_chunks(upload_id, chunk_index)
UNIQUE dependency_edges(source revision, dependent kind/ref)
UNIQUE invalidation_records(dependency_edge_id, observed_revision_id)
UNIQUE non-null verified resource identity digest within Workspace
FOREIGN KEY resources(resource_id,current_revision_id) -> resource_revisions(resource_id,resource_revision_id), deferred
FOREIGN KEY resource_revision_parents(resource_id,child_revision_id) -> resource_revisions(resource_id,resource_revision_id)
FOREIGN KEY resource_revision_parents(resource_id,parent_revision_id) -> resource_revisions(resource_id,resource_revision_id)
FOREIGN KEY resource_locations(resource_id,observed_revision_id) -> resource_revisions(resource_id,resource_revision_id)
FOREIGN KEY workspace_roots(resource_id,location_id) -> resource_locations(resource_id,location_id)
FOREIGN KEY workspace_replication_roots(workspace_id,workspace_root_id) -> WorkspaceRoot in the same Workspace
FOREIGN KEY ResourceEdges/DependencyEdges -> revisions of their stated Resource
FOREIGN KEY InvalidationRecords -> a DependencyEdge and a newly observed revision of that Resource
UNIQUE active EnvironmentControlLease per Environment
Attempt execution identity is immutable: its Step, AgentBinding, Environment, Runtime and RuntimeIncarnation cannot be rebound
FOREIGN KEY attempts(task_id,parent_attempt_id) -> a parent Attempt in the same Task
FOREIGN KEY attempts(task_id,step_id) -> the same Task's Step
FOREIGN KEY attempts(task_id,attempt_id,agent_session_id,runtime_id,runtime_incarnation_id,agent_binding_id) -> an AgentSession for that exact Attempt, selected AgentBinding and incarnation
FOREIGN KEY attempts(environment_id,runtime_id) -> an Environment on the same Runtime; an admission guard also requires the AgentBinding and Environment to share the Task's Workspace and, for task-owned Environments, the same owner Task
FOREIGN KEY execution_leases(task_id,step_id,attempt_id,runtime_id,runtime_incarnation_id) -> the exact Attempt
FOREIGN KEY environment_control_leases(task_id,attempt_id,environment_id,runtime_id,runtime_incarnation_id) -> the exact Attempt and its exact Environment
execution_leases and environment_control_leases persist only fencing_token_digest and non-secret issuer_key_version; raw FencingCredentials are process-private and excluded from SQLite, events, projections, logs and backups
```

The `agent_session_id` on an Attempt may be null while the Attempt is admitted and its
AgentSession is starting. Once present, its composite foreign key requires the session to
match the Attempt's Task, Attempt, selected AgentBinding, Runtime and Runtime incarnation.
Session replacement within one still-valid Attempt is performed as an audited state
transition under the same incarnation and lease; it cannot change the Attempt's pinned
execution identity. `agent_session_id` is a current/most-recent pointer and may change only
to another session whose composite identity matches that exact Attempt; the prior session
remains immutable history and its local binding is removed after CLOSED/LOST. An
EnvironmentControlLease is constrained to the exact Environment
already pinned by its Attempt, not merely another Environment hosted by the same Runtime.
The raw fencing credential is delivered only over authenticated private control to the
enforcer; durable storage retains its digest for validation and audit, never the bearer
value. The versioned HMAC issuer key lives in an OS keystore/HSM outside SQLite and
Workspace backups; deduplication stores only the non-secret lease view and request digest.
Lease RequestId receipts are scoped to authenticated Runtime/incarnation, reject reuse
with a different request digest, and are retained through the lease expiry plus retry
horizon; a raw derived credential is never placed in `request_dedup.response_json`.

Composite foreign keys bind Task revision pointers, Step plan ownership, Step current Attempt, Attempt-to-Step membership, and PlanRevision producer session/Attempt to the same Task. ConversationTurn pointers use a deferred composite reference to the exact `(conversation_id, conversation_turn_id, agent_session_id)` triple; a Session cannot be attached to another turn. A PlanRevision's producer AgentSession must pin the same TaskSpecRevision. The live AgentSession authority view checks the current turn, current planning head/lead, or the exact running Attempt/Step and its unexpired active lease on the current Runtime incarnation. Admission triggers bind each CapabilityInvocation's Workspace/scope to that live session and its active, unexpired Grant/Activation, including exact operation, capability, and Runtime/incarnation identity. The first dispatch repeats the owner check, closing the admission-to-dispatch race. UserRequest admission likewise binds its scope to the originating session and optional Invocation; Conversation-scoped requests additionally require the exact ConversationTurn and its current AgentSession. Identity/provenance fields are immutable after creation. The service repeats the same checks at command boundaries and maps internal SQL constraints to typed domain errors; raw database messages never cross Operator API. Execution lease exclusivity is enforced transactionally for active Step ownership. Implementation may use unique partial index where supported or serializable/locking transaction otherwise.

UserRequestResponse insertion is serialized with the parent: Conversation scope requires its
exact turn to remain `WAITING_USER`; Task scope rejects `PAUSE_REQUESTED`, cancellation, and
terminal states while permitting an answer to be saved during `PAUSED`. `UNIQUE(request_id)`
allows one response, update/delete triggers make it append-only, and the request can become
`ANSWERED` only when response digest, actor, and timestamp match. Provider-input outbox
dispatch then rechecks the exact response, closed source session, current continuation
session/lease, Grant, Activation, provider binding, and Runtime incarnation. SQL guard
signals remain internal and map to owning-domain errors such as `CONFLICT` or
`INVOCATION_AMBIGUOUS`.

UserRequest persistence also enforces `FORM` versus `EXTERNAL_URL` shape. External URL
requests must be `EXTERNAL_AUTHORIZATION`, have no response schema or choices, and keep
their raw URL/provider payload only in encrypted Runtime-local provider bindings. Their
response JSON must contain exactly one `action` value (`accept`, `decline`, or `cancel`).
The external handoff endpoint reads that encrypted binding only after owner authorization;
its response is no-store and excluded from event replication and backup. Ordinary form
responses are non-secret Workspace data and are never the secure credential-entry path.

Every persisted digest uses the canonical `Sha256Digest` encoding (`sha256:` plus 64
lowercase hexadecimal characters). SQLite checks this shape for scalar digest columns;
application storage ports validate digests nested in JSON/event payloads, and BlobStore
recomputes content digests before committing bytes. Provider revision identifiers remain
opaque and are not treated as content digests.

Indexes required on:
- Task by workspace/status/updated_at
- Step by task/status
- Attempt by task/step/status
- Effect by task/state
- Approval by Task/status (Workspace filtering joins through Task)
- Artifact by task/current_version
- ArtifactVersion by content digest
- Event by workspace/HLC and entity stream
- Runtime by workspace/availability
- CapabilityHostInstance by Runtime/state/expiry; active-use count by host/status
- ProviderCircuit by status/open_until
- AutomationOccurrence by automation/status/created_at and claim expiry
- ChannelEventReceipt by state/claim_expires_at
- ChannelIngressCursorBinding by Runtime incarnation/channel binding/state; local-only and never exported
- Resource by Workspace/display name and ResourceLocation by Resource/availability
- ResourceRevision ancestry by child/parent and Resource heads by Resource
- CapabilityInvocation by Workspace/status/updated_at; its encrypted Runtime-local provider binding is retrieved by Invocation ID, never by plaintext provider handle
- UserRequest by Workspace/status/created_at; local provider-input bindings use Runtime/delivery-state indexes and a unique Invocation/local-tag constraint
- ReplicationReceipt by Workspace/receiver/origin/sequence and disposition
- pending replication events by aggregate identity/reason; aggregate replica positions by receiver/status
- UsageObservation by Task/observed_at
- NotificationDelivery by status/next_attempt_at

## Transactions

Atomic boundaries:
- Workspace create/update/archive + event
- Workspace replication-policy update + selected-root set + Workspace aggregate version/event
- Task + initial TaskSpecRevision + event
- PlanRevision append + current-revision promotion + Step materialization + related events in one transaction
- lease acquisition + Attempt authoritative ownership
- Effect PROPOSED before external mutation
- Artifact row + stable ARTIFACT Resource + initial ArtifactVersion/ResourceRevision v1 + input DependencyEdges + synchronized current-version/head pointers + Resource and Artifact events after content is committed
- Later ResourceRevision/head + ArtifactVersion + input DependencyEdges + synchronized current-version/head pointers + both events after blob digest commit; update Artifact.current_version last so its trigger checks the matching Resource head
- VerificationRun creation + paired exact ResourceInput refs/observed digests + reverse DependencyEdges + verification event/state blob
- Artifact promotion/archive + aggregate version + transition event
- Approval resolution + policy/audit event
- AutomationRevision append + current-revision pointer + event
- AutomationOccurrence claim + pinned revision + Task creation reference
- ApprovalUse insertion + one-time approval consumption + associated Effect/grant binding
- capability lock insertion + canonical CapabilityRef key digest; a Task cannot replace the selected package/Skill revision after locking
- CapabilityInvocation creation + request deduplication before provider dispatch
- Resource revision/location observation + downstream invalidation records
- ArtifactVersion/VerificationRun immutable input refs (including paired ResourceInput digests) + DependencyEdge reverse-index rows
- ResourceRevision append + parent-edge rows + current-head recomputation + location observation + event + dependent invalidations
- upload chunk acceptance + offset/hash verification; final commit only after full digest verification; session lifecycle event and Resource creation commit atomically
- notification claim/delivery settlement under stable dedupe key
- EnvironmentControlLease epoch increment + previous-owner fencing
- EnvironmentProvisionPreview single-use consumption + Workspace budget reservation + PROVISIONING Environment + creation event
- authenticated replication receipt insertion + Workspace receipt-cursor advancement
- aggregate state application + per-aggregate applied revision + pending-event settlement

Do not keep DB transaction open across network/provider calls.

`capability_ref_key_digest` is SHA-256 over RFC 8785 canonical JSON of the complete
normalized `CapabilityRef` from `SCHEMAS.md`. The lock row stores its tagged identity kind,
source identity, optional package version, content/manifest digest, and component URI/id;
it does not invent a package version for an MCP Skill.

## Optimistic concurrency

Mutable aggregate commands include `expected_version` where user/process races are possible. Mismatch returns conflict and caller re-reads/reconciles. Artifact SQL constraints additionally require sequential content versions, legal Library transitions, aggregate-version increments, immutable ArtifactVersion rows, and no new content after archive.

## Blob storage

Blob key is the plaintext content digest, with workspace-scoped encryption handled by
the BlobStore. Blob commits are immutable. Artifact writes:
1. stream temporary object
2. compute digest
3. fsync/commit provider object
4. create immutable manifest
5. reference from ArtifactVersion/Event only after commit

Before each domain-event transaction, the writer serializes and commits the canonical
post-transition aggregate record as an `AggregateStateRef` blob. The database transaction
then writes the event, state reference, and aggregate projection together. If the database
transaction fails, the unreferenced state blob is eligible for garbage collection after
the normal safety window. Replication authenticates and persists the event envelope before
advancing the Workspace-scoped receipt cursor. A missing allowed state blob keeps aggregate
application pending but does not block receipt of unrelated later events. A
policy-disallowed event is represented only by an authenticated, payload-free omission
receipt; it does not create a DomainEvent on the receiving Runtime. A later event with an
aggregate revision gap remains pending until its authorized predecessor history arrives or
a safe authorized snapshot establishes a baseline. `replication_cursors.highest_received_contiguous_sequence`
is the received transfer position, not an applied-state position; `replication_receipts`
preserves each event/omission disposition, `pending_replication_events` tracks events
awaiting state or a baseline, and `replication_aggregate_positions` records each
aggregate's applied revision. This per-event record is a current-state reconstruction aid;
the immutable event stream remains necessary for lifecycle history and audit.

Garbage collection must retain any blob referenced by durable records, pending replication manifests, checkpoints needed for recovery or retention policy.

External ArtifactContent is stored as a version-pinned ResourceRef/provider revision and
optional observed digest; it has no BlobStore object requirement. Resource identity and
locations are separate rows. A local absolute path or provider locator is opaque location
data and never becomes the stable ResourceId.

## Backup and disaster recovery

A consistent Workspace backup captures a transactionally consistent database snapshot,
per-origin event replication cursors, a complete manifest of referenced blobs, schema
version, integrity digest, and encryption-key reference. The backup is verified before it
is advertised as restorable. Restore validates the manifest and every required blob,
restores the point-in-time snapshot/event history through its recorded cursors, and
registers a new Runtime identity; it must not resurrect old ExecutionLease epochs. Restore
drills are required before cloud GA.
The snapshot is a Workspace-filtered serialization, not a byte-for-byte copy of the local
SQLite database. It excludes pairing challenges, Environment provision previews, OS boot
observations, AgentEndpoint bindings, Agent/Capability host instances and bindings,
CapabilityInvocation provider bindings, ProviderInputBindings, AutomationTriggerBindings,
Environment provider bindings, Environment checkpoint provider bindings,
ResourceLocation/FileIdentity bindings, transient
upload chunks, active leases, and
other Runtime-local process/provider handles. It includes durable Environment and
checkpoint metadata only. Portable checkpoint blobs are selected only if both the
Environment's `INCLUDE_CHECKPOINTS` policy and Workspace replication policy permit them.
Restore registers a new Runtime identity and incarnation, drops all old local bindings,
and requires provider reattachment before an Environment can be used.
Backups are separate from Runtime Mesh replication and are not a substitute for it.
Backup manifests are control-plane recovery metadata stored with their backup target; they
are not replicated Workspace DomainEvents. Create/restore request deduplication and the
corresponding AuditRecord are retained. The database snapshot excludes the manifest row
being created, so it never recursively contains a reference to its own output.
The canonical immutable record is `WorkspaceBackupManifest` in `DATA-MODEL.md`. A row is
inserted only after the database snapshot, per-origin event cursors, blob manifest, and
every referenced object pass integrity verification; the transaction persists its
manifest reference/digest, snapshot reference, schema version, encryption-key reference,
integrity digest, provider-generated manifest authentication value, creation time, and
verified time. In-progress or failed attempts are operational records, not restorable
manifests. The verified timestamp is non-null.

Restore runs on a locally authenticated recovery surface against a new installation. It
checks schema compatibility and key availability, verifies/decrypts each required object,
restores the point-in-time snapshot/event history through the manifest cursors, rebuilds
projections, and generates a new Runtime identity.
It never restores live leases, fencing epochs, or secret bytes; secret references are
revalidated and unavailable connections require reauthentication. Backup blobs remain
encrypted under the existing key reference, and loss of that key makes the backup
unrestorable.

## Event snapshots and archive

Aggregate snapshots accelerate projection rebuilds but do not replace event identities,
  audit records, or the retained source stream. Cold archive may move immutable event
  segments to blob storage after every required peer has acknowledged the Workspace-scoped
  receipt positions and retention holds permit it. A segment includes its origin sequence range and content digest; the
active journal retains a verifiable manifest and supports cursor-based retrieval.

## Resumable uploads

Upload sessions pin expected size, optional final digest, chunk size, expiry, and current
received ranges. HTTP ranges are inclusive; chunk rows persist half-open byte intervals
using `start_offset` and `end_offset_exclusive`. A chunk index fixes its only legal start
and maximum size, and ranges cannot overlap. A chunk is accepted idempotently only when
upload ID, exact offset/range, chunk digest, and bytes match. Conflicting reuse returns
`UPLOAD_OFFSET_CONFLICT`. Commit
requires contiguous full coverage and a matching whole-object digest before a Resource or
Artifact reference becomes visible. Expired temporary chunks are garbage-collected.

## Migrations

- monotonically numbered schema migrations.
- migrations are forward-only in production; rollback is restore/forward-fix.
- runtime refuses to open a database newer than its supported schema major.
- managed rolling upgrades use expand/migrate/contract pattern.

### Routine and trigger Workspace integrity

RoutineRevision, AutomationRevision, AutomationOccurrence and AutomationCursor carry a
redundant `workspace_id` storage key. Composite foreign keys require their parent, pinned
Routine revision, trigger-host Runtime and produced Task to belong to that Workspace.
Task Routine/Occurrence provenance uses the same composite keys. These columns enforce
isolation; they do not introduce another mutable Workspace assignment. Cursor identity
remains `(automation_id, trigger_id)` across revisions.

Environment lifetime constraints require an owner Task for ATTEMPT/TASK_RETAINED and
forbid a Task owner for WORKSPACE_PERSISTENT. Composite foreign keys keep its Runtime and
owner Task in the owner Workspace. Persistent Environment creation binds one one-use
preview record, matching Workspace/principal/request/eligibility basis, budget reservation,
Environment, and creation event. SQLite triggers reject an absent/expired preview, a
second Environment using the same digest, or a transition to CONSUMED without its matching
Environment. The Environment stores the digest without a foreign key to the operational
preview record so event replication and restore do not require that ephemeral record.
Unconsumed previews are ephemeral admission challenges: backups and
cross-Runtime replication omit them, archive/restore expires them, and cleanup removes
expired unreferenced records. The Environment retains the consumed digest as consent
provenance. Usage observations can reference an Environment in the same Workspace;
BudgetReservations select exactly one TASK or ENVIRONMENT budget owner, with Task use of a
persistent Environment accounted against both budgets via separate reservation rows.
Workspace archive uses an admission lock plus fenced
quiescence checks across turns, Invocations, grants/secret/control leases and persistent
workloads; a database status update alone cannot prove that an external process stopped.
