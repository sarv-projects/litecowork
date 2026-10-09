# Storage LLD

## Storage ports

```text
StateStore
EventStore
BlobStore
ResourceStore
RuntimeLifecycleStore
```

Domain/application code depends on ports, never SQLite/Postgres/S3 directly.

`StateStore` owns version-checked aggregate projection reads and atomic writes of an
aggregate projection, its event(s), request-deduplication receipt, and the local
Workspace/origin sequence allocator. `EventStore` reads immutable streams/cursors and
accepts authenticated replication through the transaction owner. `BlobStore` provides
content-addressed `put/get/verify` operations and provider-mediated deletion of an exact
object only after its owner has fenced new references and proved the object unreferenced;
it does not make domain decisions. The SQLite adapter exposes one bounded writer executor and process-local
`SqliteWriterMetricsSnapshot` observations for outstanding commands and elapsed time in
bounded-channel sends. Outstanding commands include callers blocked while submitting,
queued commands and the active command awaiting its response; they are not exact channel
occupancy. Send elapsed time includes local call overhead and is not database execution
time. A snapshot reads independent atomics and is best-effort under concurrent load, not a
cross-field transactional sample. These observations are ephemeral operational state,
never persisted or replicated.

### Task admission from a manual Routine run

`SqliteWorkspaceStore::create_task` performs Routine-specific admission inside its existing
`IMMEDIATE` Task creation transaction. When `TaskCreateCommit.routine_admission` is
present, storage verifies the paired Task routine ID/revision and request identity, reads
the Routine head/status and exact immutable revision in the selected Workspace, requires
`ACTIVE` plus exact current revision, re-materializes bounded inputs from that revision,
compares the proposed TaskSpec mapping, and then validates each pinned Resource revision
and selected lead. Only after every guard succeeds can it insert Task, TaskSpecRevision,
aggregate snapshot, conditional Routine provenance on `task.created.v1`, and the
idempotency receipt. A conflict or invalid binding aborts the transaction; no partial Task
or success receipt is retained. Ordinary non-Routine Task creation must not include
Routine request fields or a Routine admission envelope.

The RunRoutine receipt digest uses the exact `RunRoutine` request payload so retries
remain identical even if Workspace defaults change after the first commit. Receipt lookup
remains scoped to the authenticated principal/Workspace command boundary. The route
authenticates and checks owner scope before replay. An exact retry may replay without
requiring the Routine to remain current; a new key is revalidated against the current
ACTIVE head.

### Local Automation ManualTrigger admission

The local owner ManualTrigger operation uses the same Task creation writer transaction,
but has an AutomationOccurrence admission envelope in addition to the Routine admission.
An exact owner/request/payload receipt is resolved before mutable dependency checks and is
checked again inside the `IMMEDIATE` transaction to serialize concurrent submissions. A
new request rechecks the active Workspace, exact current Automation/version and pinned
Routine revision, non-DISABLED Automation status, one unambiguous ManualTrigger, Coworker
state, local Runtime incarnation and active TRIGGER_HOST Workspace-binding version,
eligible lead, bounded Routine bindings, and exact Resource pins. Any failure leaves no
Task, occurrence, transition event, or success receipt.

Occurrence snapshots are content-addressed aggregate-state blobs prepared alongside the
Task snapshot. The transaction inserts PENDING/version 1, claims/version 2 with
claim_epoch 1, creates the READY Task, then links it at STARTED/version 3/claim_epoch 1;
all three occurrence events/snapshots, the Task event/snapshot, and request receipt commit
together. Migration 11 persists the independent occurrence version and SQLite requires
each later update to increase it exactly once. This path is one-shot only: it does not
create/advance an AutomationCursor, enable the Automation, create a Plan, or start agent
execution. The Automation page does not yet expose Run now; recurring TriggerCoordinator
hosting and occurrence settlement remain unavailable.

`ResourceStore::read_resource_content_bounded` accepts an optional immutable revision pin
(omitted means current head) and requires providers to resolve that exact same-Resource,
same-Workspace revision and check its indexed size before reading/decrypting blob bytes. A provider must not implement this by
calling an unbounded read and checking the returned `Vec`; that would defeat the memory
bound. Both bounded and trusted unbounded Resource reads check ContextDocument status at
the SQLite read-admission boundary before returning a BlobRef to the caller: ordinary
Resources and `ACTIVE` ContextDocuments may be read; `REVOKED`, `DELETION_PENDING`, and
`DELETED` ContextDocuments are rejected with status-specific internal errors. The owner
Operator maps these to `CONTEXT_DOCUMENT_NOT_ACTIVE` with a status-appropriate message;
metadata reads remain available. A read admitted while `ACTIVE` may finish if a status
transition commits afterward. Derived-state writers must recheck the exact status and
source pin in their final transaction, so an in-flight read cannot publish an index after
revocation/deletion. The Operator currently caps a single IPC Resource response at 10 MiB
and workspace instruction input at 64 KiB. The unbounded read port is reserved for trusted
internal consumers that have their own explicit size contract.

If BlobStore access fails after read admission, SQLite re-reads the Resource's current
ContextDocument status. A confirmed transition returns the status-specific inactive error;
an active, absent, or unprovable status preserves the original BlobStore/integrity error.
The Operator reports unavailable content as `RESOURCE_LOCATION_UNAVAILABLE` and verified
content-integrity failures as `INTEGRITY_FAILURE`; a failed status recheck does not mask the
original read failure.

The desktop's bounded `ON_DEMAND_CONTENT` path reads an exact ResourceRevision from the
local managed encrypted BlobStore using the digest-verifying bounded read path. For a
historical revision, immutable revision metadata supplies its digest and length; the
adapter does not follow the mutable current locator to obtain old bytes. A managed local
provider must be available for the Resource and the content-addressed object must still
exist. If not, the read fails as external or unavailable rather than substituting the
current head. The local text-index path
uses a distinct `RESOURCE_INDEX` BlobPurpose encrypted with the Workspace/purpose key and
stores only its digest plus workspace-keyed HMAC term tokens in SQLite migration v7.
Extracted text, query terms, and snippets are never stored as plaintext in SQLite. The
index is a rebuildable projection over exact Resource revisions, not an evented aggregate
or replicated history. The local key provider keeps versioned Workspace/purpose keys in
the OS credential store; the database and blob tree hold no key material, and no key is
exposed to an agent or Operator response. Key versions remain stable across daemon
restarts. Rotation creates a new active key version while retaining older versions so
indexed queries can search each version (up to eight retained indexed versions per
Workspace); explicit reindexing from verified authorized Resource bytes moves a revision
to the active version. If an index key version is unavailable or the searchable-version
bound is exceeded, indexed search fails closed and never tries plaintext or silently
omits that version. If the active-version pointer is absent while a version-1 credential
already exists, key initialization and rotation fail with a recovery-required error; they
must not overwrite the existing version-1 credential. An encrypted snapshot or term projection can be rebuilt from the source Resource
when its own authorized Resource key and source bytes remain available. ZIP, PDF/Office,
OCR, embeddings, semantic/local-model retrieval and persistent WorkspaceRoot crawling
remain outside this provider slice.

Owner-triggered local text-index rebuild uses the same rebuildable projection tables and
BlobPurpose. The request carries the authenticated principal, RequestId, Workspace,
Resource, exact current ResourceRevision ID, and expected source digest. Pinned metadata
declines unsupported/oversized files without reading their bytes; eligible text is read
through the bounded digest-verifying path and index preparation happens outside the SQLite writer transaction; final commit opens an
IMMEDIATE transaction, rechecks owner/active Workspace and exact head+digest (including
active ContextDocument status), replaces/removes the projection, and inserts the encoded
typed result in `request_dedup` atomically. Receipt lookup happens before source access so
a lost-response retry returns the original result even if the source later changes or the
Workspace is archived. Reuse of that principal/RequestId with a different normalized
Resource/revision/digest payload conflicts. The response contains no extracted text or
terms. This projection-only maintenance command does not append domain events or mutate
Resource, Task, or Artifact aggregates.

The adapter performs no provider, filesystem-encryption-key, or network call while a SQL
transaction is open.
Blob bytes are committed and verified before a transaction can reference them; a failed
transaction may leave an unreferenced blob for delayed garbage collection. Deletion is
idempotent for a missing object, must reject unsafe object types, and must durably remove
the exact content-addressed object. Providers that cannot implement these semantics must
return an unsupported-operation error; callers must not emulate deletion by traversing or
clearing a BlobStore directory.

Every production BlobStore construction requires an injected `WorkspaceBlobKeyProvider`
and versioned encryption implementation. Keys are scoped to a Workspace and purpose,
kept outside SQLite and backups, and zeroized when practical. If the key provider or
encryption implementation is unavailable, the store fails closed; plaintext storage is
not a fallback. Test-only key providers are not valid production providers.

`storage-sqlite::OsWorkspaceBlobKeyProvider` is the desktop implementation. It stores
versioned blob keys and the active-version pointer in the platform credential store
(macOS Keychain, Windows Credential Manager, and the Linux Secret Service/keyutils
backend selected by the configured `keyring` features). Credential account labels contain
only a SHA-256 scope pseudonym, not the Workspace ID. Key bytes are created with the OS
CSPRNG and zeroized in process memory where supported. A missing, locked, or unavailable
credential store and malformed items fail closed. There is no local-file fallback.
`rotate_key` activates a new version while retaining old versions for reads; rotation,
recovery, and platform backend behavior still require system qualification and
cryptographic review before production claims.

`storage-sqlite::OsRuntimeDeviceIdentityProvider` uses a separate keyring service and a
SHA-256 account label derived from the canonical private data-directory path. Its UTF-8
credential contains the Ed25519 seed, RuntimeId binding, key version, and issuance time;
the seed is wrapped in zeroizing buffers in process memory and never written to a local
file or SQLite. The public DeviceIdentity is persisted with the Runtime descriptor. This
is an OS credential-store secret and may be exportable; hardware-backed key protection is
not assumed. A missing/corrupt credential or identity mismatch fails closed. Moving the
data directory needs an explicit identity-recovery procedure.

`LocalWorkspaceStorage::open(state_directory, config)` is the local composition entry
point for the daemon: it opens `litecowork.sqlite3` and the `blobs/` store under the same
private state root and injects `OsWorkspaceBlobKeyProvider`. The daemon must retain this
composition for its lifetime and must not construct a test key provider in production.

`runtime_incarnations` is the compact authenticated Runtime Mesh registry required to
validate references from replicated aggregates. Local OS boot IDs and diagnostics live in
`runtime_incarnation_local_observations`, which is never replicated or backed up.
The local `RuntimeLifecycleStore` atomically registers the public Runtime descriptor,
incarnation, and local observation through the bounded writer. Its operation does not
create Workspace bindings or Mesh presence. It never stores device private-key material.
Runtime-to-Workspace authorization is stored separately in
`runtime_workspace_bindings`; Runtime IDs identify installations, not Workspace
membership. SQLite v1 and v2 are immutable baselines. V3 backfills one binding per legacy
Runtime/Workspace pair and updates `attempt_environment_owner_guard` to require an active
binding. V4 removes `runtimes.workspace_id` and rebuilds the dependent Environment,
ChannelHostAssignment, AutomationOccurrence, and AutomationCursor tables. New writes to
these Workspace-scoped Runtime relations require an ACTIVE RuntimeWorkspaceBinding with
the matching role (`EXECUTOR`, `CHANNEL_HOST`, `TRIGGER_HOST`, or `WORKSPACE_HUB`). The
attempt admission guard additionally checks the current Runtime incarnation and active
`EXECUTOR` role. Runtime listings are Workspace-scoped projections through active
bindings; Runtime identity and installation inventory remain Workspace-neutral. Local
Runtime registration still requires explicit post-Workspace enrollment and must not invent
a Workspace to satisfy historical storage shapes.

The v4 migration also validates trigger-enforced scope relationships for copied rows before
commit; `foreign_key_check` alone cannot validate active status or role membership. A legacy
database with orphaned or incompatible live Runtime references fails closed for repair.
Revoking a Workspace binding requires clearing a Mesh hub pointer and draining ChannelHost
assignments/leases and enabled TriggerHost cursors first. Task Attempts remain governed by
their own lease and Effect reconciliation lifecycle.
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

- `runtime-state.json` and `runtime.lock` are installation-local bootstrap/lifecycle state,
  stored beside the local database and excluded from Workspace backup/replication. They
  contain no credentials. `runtime-state.json` is atomically replaced, and the daemon
  holds an OS file lock for its lifetime. After SQLite opens, the daemon persists its
  installation-scoped Runtime and new RuntimeIncarnation in the local catalog before
  recovery; this does not require a Workspace or create a RuntimeWorkspaceBinding.
  Authenticated Mesh publication is separate and occurs only after pairing.
- SQLite 3.38 or later with JSON functions enabled for relational/domain projections
  and the local event index.
- The state directory is private to the owning OS user; on Unix, create/check it as
  `0700` and the SQLite main file as `0600`. WAL/SHM sidecars remain under that directory.
  Other OSes must use an equivalent ACL and qualify it in platform tests before release.
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
goal_artifact_links
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
context_document_purge_plans             # Resource-owned immutable purge target manifest
context_document_purge_receipts          # exact per-replica deletion acknowledgements
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
resource_upload_chunk_requests
resource_upload_blob_reservations        # operational write intents; local recovery state
resource_upload_blob_gc_fences            # operational delete fences; local recovery state
aggregate_snapshots
event_archive_segments
workspace_backup_manifests

runtime_incarnations
runtime_incarnation_local_observations # Runtime-local OS boot ID/diagnostic data; excluded from backup/replication
agent_host_instances                   # local operational records; not Task replication truth
agent_session_host_bindings            # local opaque agent handles; excluded from backup/replication
capability_activation_host_bindings    # local opaque provider handles; excluded from backup/replication
capability_host_instances              # local provider observations/use groups; not LiteSPM package truth
capability_invocation_provider_bindings # encrypted provider task handles/cursors; local-only
provider_input_bindings                 # encrypted MCP input envelope/keys and response outbox; local-only
automation_trigger_bindings             # encrypted provider cursors; local-only, host-epoch scoped
channel_ingress_cursor_bindings          # encrypted provider cursors; local-only, host-epoch/incarnation scoped
channel_reply_targets                    # opaque provider message refs; local-only, excluded from backup/replication
channel_host_lease_records               # Hub-only renewable control state; digest only, not Workspace backup

Resource upload chunks are stored as authenticated encrypted BlobStore objects under the
separate `RESOURCE_UPLOAD_CHUNK` purpose. SQLite stores only their digest, byte range,
index, and opaque content-addressed reference. A durable blob reservation is committed
before each BlobStore put, then consumed atomically with its chunk receipt. This makes a
crash between blob write and receipt discoverable after the upload session expires. The
bounded collector claims only expired reservations with no accepted chunk reference and
no live reservation for the same `(WorkspaceId, digest)`. A durable GC fence blocks new
reservations while the exact encrypted object is removed; only then are reservation and
fence rows cleared. A crash after claiming leaves a `DELETING` reservation/fence for the
next daemon incarnation to retry. The collector is serialized by the single-instance local
daemon lifecycle and processes at most 100 objects per 30-second sweep. Shared digests with
any accepted chunk reference are retained. A Resource commit transaction inserts the
Resource/revision/location/event and commit receipt while transitioning its upload session
to `COMMITTED`; it never exposes temporary chunk bytes through catalog or Task APIs. This
collector only reclaims unreferenced upload-chunk objects; it does not garbage-collect
committed chunk data or general Resource/Artifact blobs.
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
schema_migrations
workspace_origin_sequences
```

Conversation catalog creation is a serialized SQLite transaction that inserts the
Conversation, appends its aggregate-state-backed `conversation.created.v1` event, and
stores the idempotency receipt together. Owner-scoped list/get operations read the
Conversation projection only; message history remains served through the bounded
Conversation presentation snapshot. This does not imply turn dispatch or agent execution.

The local-only `schema_migrations` table records the monotonically numbered migration,
its immutable SQL-source checksum, the fingerprint of the resulting SQLite schema
objects, and application time. `PRAGMA user_version` is the
supported schema version marker. Fresh databases apply `sqlite-v1.sql`, then
`sqlite-v2.sql`, then `sqlite-v3.sql` in one transaction, followed by the V4 table rebuild
in its own immediate transaction with foreign-key enforcement temporarily disabled outside
the transaction and restored on every exit path. V4 runs `foreign_key_check` before
commit. Existing databases verify each recorded source checksum before applying remaining
forward migrations; V1-V4 source files and checksums remain immutable. V5 adds
PlanRevision update/delete rejection, Step identity/deletion protection, and unique logical
keys within each Task/PlanRevision. Initial-plan acceptance binds its idempotency digest to
the normalized typed plan. In the same transaction it rechecks the producer's host binding,
current ready Runtime incarnation, live endpoint binding, active Workspace EXECUTOR binding,
and enabled lead eligibility. Endpoint expiry uses the daemon's UTC admission time rather
than caller event time; this depends on the local system clock and still needs clock-skew
qualification. The receipt digest includes the normalized plan and producer session. V2 adds upload
lifecycle/progress columns, folder provenance, chunk request and blob-GC recovery tables,
and write guards that can be expressed additively. It backfills committed Resource IDs from
durable upload status events when available. V3 adds `runtime_workspace_bindings`,
backfills explicit bindings from legacy Runtime/Workspace pairs, and makes Attempt
admission require an active binding. V4 removes the remaining Workspace ownership column
and four composite foreign keys from the installation-scoped Runtime representation;
multi-Workspace use also depends on enforcing role-scoped active bindings in admission and
readiness services. Historical rows retain nullable legacy digest
and prior chunk-size values; a missing digest or size/chunk values outside current limits
makes a session non-resumable and non-committable. Folder provenance is never inferred from
legacy display names. V8 adds Routine revision/head guards. V9 adds Effect legal-transition,
initial-PROPOSED, and delete guards plus append-only Evidence update/delete guards. Earlier
migration sources and checksums remain immutable. These guards enforce storage invariants;
they do not provide Trust authorization, provider dispatch, Effect reconciliation, or
production-safe Task execution. V6 rebuilds `resource_locations` to add `UNAVAILABLE` while preserving
existing rows and dependent foreign keys; v1-v5 SQL sources and checksums remain immutable.
At Runtime startup, bounded root-revalidation candidates are read and each result is committed
in an immediate transaction with its WorkspaceRoot transition, ResourceLocation observation,
safe events/snapshots, idempotency receipt, and current-incarnation private locator/raw identity
bindings. A failed identity check atomically removes any partial current-incarnation pair and
marks the root and location unavailable. Private binding rows are never included in events,
snapshots, API results, or receipts. The local-only
`workspace_origin_sequences` table
persists the last committed event sequence for each `(workspace_id, origin_runtime_id)`;
it advances in the same transaction as the event and aggregate projection, so archival
cannot cause sequence reuse. Neither table is a replicated Workspace aggregate.

## Required constraints/indexes

```text
UNIQUE task_spec_revisions(task_id, revision)
UNIQUE plan_revisions(task_id, revision)
UNIQUE steps(task_id, plan_revision, logical_key) WHERE logical_key IS NOT NULL
UNIQUE artifact_versions(artifact_id, version)
UNIQUE artifact_versions(artifact_id, resource_id, version)
FOREIGN KEY artifacts(artifact_id, current_version) -> artifact_versions(artifact_id, version), deferred
FOREIGN KEY artifacts(artifact_id, resource_id, current_version) -> the matching ArtifactVersion and Resource revision
FOREIGN KEY artifact_versions(resource_id, resource_revision_id) -> resource_revisions(resource_id, resource_revision_id)
trigger guard_artifact_initial_revision -> initial ArtifactVersion ResourceRevision equals Resource head
trigger guard_artifact_current_revision -> Artifact current-version change names current Resource head
UNIQUE domain_events(event_id)
UNIQUE domain_events(workspace_id, origin_runtime_id, origin_sequence)
PRIMARY KEY workspace_origin_sequences(workspace_id, origin_runtime_id)
PRIMARY KEY schema_migrations(version)
UNIQUE replication_receipts(workspace_id, receiver_runtime_id, origin_runtime_id, origin_sequence)
UNIQUE automation_revisions(automation_id, revision)
UNIQUE routine_revisions(routine_id, revision)
UNIQUE delegation_profile_revisions(delegation_profile_id, revision)
UNIQUE non-archived delegation profile name_key(workspace_id, agent_binding_id)
UNIQUE coworker_revisions(coworker_id, revision)
UNIQUE goal_revisions(goal_id, revision)
UNIQUE goal_task_links(goal_id, revision, task_id)
UNIQUE goal_artifact_links(goal_id, revision, artifact_id, artifact_version)
UNIQUE goal_routine_links(goal_id, revision, routine_id, routine_revision)
UNIQUE automation_occurrences(automation_id, trigger_id, occurrence_key)
`automation_occurrences.version` starts at 1 and advances exactly once per persisted transition; event/snapshot `entity_revision` equals `version`. `claim_epoch` is an independent worker fence and advances only on claim/reclaim.
UNIQUE INDEX uq_suggestions_open_dedupe(workspace_id, dedupe_key) WHERE status = 'PROPOSED'
INDEX suggestions(workspace_id, dedupe_key, resolved_at DESC) WHERE status = 'DISMISSED' # 30-day exact-key cooldown lookup
PRIMARY KEY automation_cursors(automation_id, trigger_id)
UNIQUE channel_event_receipts(channel_binding_id, origin_host_epoch, ingress_sequence)
PRIMARY KEY channel_ingress_cursor_bindings(channel_binding_id, host_epoch)
FOREIGN KEY automation_cursors(automation_id,active_automation_revision) -> automation_revisions
FOREIGN KEY runtime-bound handles(runtime_id,runtime_incarnation_id) -> runtime_incarnations
Runtime and RuntimeIncarnation catalogs are installation-local durable state; persist them before recovery, and publish the compact incarnation record to authorized Workspace peers only after authenticated Mesh pairing and before any replicated aggregate references it
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
AgentHostStore persists only Runtime-local AgentHostInstance lifecycle state; create requires the current READY incarnation and matching endpoint/profile, and transitions use expected-state compare-and-set without domain events
TASK_PLANNING admission commits a version-1 STARTING AgentSession, its complete aggregate-state blob reference, `agent.session.starting.v1`, and RequestId receipt in one transaction after rechecking Task/spec/lead, owner, enabled binding, endpoint binding, and current READY Runtime incarnation
STARTING claims the unique planner slot but is not ready and cannot authorize planning tools; recovery enumeration includes stranded STARTING sessions across Runtime incarnations
adapter startup occurs outside SQLite; activation must atomically commit AgentSession ACTIVE and first-planning Task RUNNING only after readiness

The concrete desktop SQLite adapter currently fails closed before either operation. Both
`start_task_planning_session` and `activate_task_planning_session` return
`TASK_PLANNING_ISOLATION_UNAVAILABLE` until Runtime-owned isolation admission evidence is
produced and rechecked at the storage boundary. Start rejection occurs before aggregate
blob writes, session/event insertion, or RequestId receipt creation; activation rejection
occurs before session lookup or Task mutation. The rows above describe the eventual storage
contract and must not be read as evidence that planner admission is enabled.
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
UNIQUE resource_upload_chunk_requests(upload_id, request_id)
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
- Effect row + immutable aggregate snapshot + `effect.proposed.v1` + idempotency receipt
  before any later dispatch; proposal rechecks the active Attempt/Invocation/grant and the
  current Runtime incarnation/ExecutionLease fence. This foundation does not dispatch.
- Effect transition + versioned snapshot + event + idempotency receipt
- append-only Evidence row + snapshot + `evidence.created.v1` + idempotency receipt
- Artifact row + stable ARTIFACT Resource + initial ArtifactVersion/ResourceRevision v1 + input DependencyEdges + synchronized current-version/head pointers + Resource and Artifact events after content is committed
- Later ResourceRevision/head + ArtifactVersion + input DependencyEdges + synchronized current-version/head pointers + both events after blob digest commit; update Artifact.current_version last so its trigger checks the matching Resource head
- VerificationRun creation + paired exact ResourceInput refs/observed digests + reverse DependencyEdges + verification event/state blob
- Artifact promotion/archive + aggregate version + full Artifact state snapshot + transition event + principal-scoped idempotency receipt; an already-archived current-version archive writes only its no-op receipt
- Approval resolution + policy/audit event
- AutomationRevision append + current-revision pointer + event
- AutomationOccurrence claim + pinned revision + Task creation reference
- ApprovalUse insertion + one-time approval consumption + associated Effect/grant binding
- capability lock insertion + canonical CapabilityRef key digest; a Task cannot replace the selected package/Skill revision after locking
- CapabilityInvocation creation + request deduplication before provider dispatch
- Resource revision/location observation + downstream invalidation records
- ArtifactVersion/VerificationRun immutable input refs (including paired ResourceInput digests) + DependencyEdge reverse-index rows
- ResourceRevision append + parent-edge rows + current-head recomputation + location observation + event + dependent invalidations
- Resource revision-history page read + cursor lookup, returning a bounded append-order keyset page so parent revisions precede child revisions without materializing the full ancestry in the daemon; current SQLite may still scan matching rows because append order is not yet indexed
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
the BlobStore and an injected WorkspaceBlobKeyProvider. `BlobRef.digest` and
`size_bytes` describe the verified plaintext bytes; encrypted storage metadata records
the cipher/key version separately and never changes the domain identity of the blob.
Blob commits are immutable. Artifact writes:
1. stream temporary object
2. compute digest
3. fsync/commit provider object
4. create immutable manifest
5. reference from ArtifactVersion/Event only after commit

The v1 JCS encoder orders object keys and follows RFC 8785 number formatting. Typed integer
values outside the exact I-JSON safe-integer range are rejected before persistence; a
domain schema that needs a larger exact integer must define a decimal-string wire form.
This prevents Rust `u64` precision from silently changing in JSON digests.

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
ContextDocument tombstones, sealed purge target manifests, and acknowledgement receipts
are durable Resource recovery state and remain in backups. Content blobs whose purge was
acknowledged are not reintroduced by restore; their Resource/revision identities and
digests remain for historical Task/Evidence provenance.
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

Upload sessions pin expected size, required whole-content digest, chunk size, expiry, and current
received ranges. HTTP ranges are inclusive; chunk rows persist half-open byte intervals
using `start_offset` and `end_offset_exclusive`. A chunk index fixes its only legal start
and maximum size, and ranges cannot overlap. A chunk is accepted idempotently only when
upload ID, exact offset/range, chunk digest, and bytes match. Conflicting reuse returns
`UPLOAD_OFFSET_CONFLICT`. Commit
requires contiguous full coverage and a matching whole-object digest before a Resource or
Artifact reference becomes visible. Expired temporary chunks are garbage-collected.

## Migrations

- monotonically numbered schema migrations.
- each applied migration records a SHA-256 checksum of its exact embedded SQL bytes and
  a fingerprint of all non-internal `sqlite_master` schema objects; reopening refuses a
  changed migration checksum or missing/changed schema object.
- a clean install applies migration 1, writes its migration row, and advances
  `PRAGMA user_version` in one transaction. A crash leaves either the old schema/version
  or the complete new schema/version.
- opening a database with a newer `user_version`, an unversioned non-empty schema, a
  missing migration record, or a checksum mismatch fails closed for explicit recovery.
- migrations are forward-only in production; rollback is restore/forward-fix.
- runtime refuses to open a database newer than its supported schema major.
- managed rolling upgrades use expand/migrate/contract pattern.
- migration 7 adds revision-scoped `resource_text_indexes` and keyed-token projection rows;
  extracted snapshots remain encrypted Workspace BlobStore objects and are never FTS
  plaintext.
- migration 8 makes Routine revisions append-only and guards Routine head updates to
  append a revision or perform the one-way `ACTIVE` → `ARCHIVED` transition. Routine
  storage persists reusable definitions only; it does not execute or schedule them.
- migration 9 guards initial Effect state and legal transitions, rejects direct Effect
  deletion, and enforces Evidence append-only writes. `OBSERVED`/`VERIFIED` Evidence
  admission is deliberately disabled until an independently authenticated observer/verifier
  contract exists; it must not be synthesized by a Runtime request. The migration validates
  its schema transactionally. Because earlier writers cannot prove historical append-only
  or producer provenance, migration 9 preflights that both tables are empty and fails closed
  if either contains rows; a dedicated provenance recovery workflow is not yet implemented.
- migration 10 adds append-only Goal links to exact same-Workspace Artifact versions;
  unlinking is represented by a new Goal revision, never by deleting historical link rows.
- migration 11 adds `AutomationOccurrence.version`, backfills from the latest durable
  occurrence event revision (or 1 when none exists), and rejects any row update that does
  not advance the aggregate version exactly once. This version is independent of
  `claim_epoch`; only claim/reclaim changes the fencing epoch. Local ManualTrigger
  admission checks the active TRIGGER_HOST Runtime incarnation and Workspace binding
  version inside the same Task transaction, then atomically records PENDING v1, CLAIMED
  v2, and STARTED v3 snapshots/events with matching `entity_revision` values.
- migration 12 pins `PresentationPreference` on ConversationTurns (legacy rows default to
  `AUTO`) and creates the immutable one-per-message `rich_presentations` table. Composite
  Workspace/Conversation ownership and an insert guard require a committed same-Workspace
  AGENT message. Updates/deletes are rejected. The table stores only bounded metadata and
  a content-addressed BlobRef; canonical presentation bytes are not stored as SQLite text.
  Its blob is a Workspace backup/GC reference root when present, but restore may omit a
  missing enhancement and still restore semantic ConversationMessages.
  The current isolated SQLite `RichPresentationStore` adapter qualifies only version-1
  semantic `TEXT_SLICE`, `LAYOUT`, and `DIVIDER` documents with empty source/action
  references. It checks the current Workspace owner before looking up the committed
  AGENT message, requires ACTIVE Workspace status for publication (archived Workspace
  reads remain owner-authorized), validates canonical semantic/document digests, exact UTF-8 slices and
  per-block provenance, and atomically appends immutable metadata plus the publication
  event. Document bytes use the distinct encrypted `RICH_PRESENTATION` BlobPurpose;
  aggregate snapshots use `AGGREGATE_STATE`. Reads verify the document again. Missing
  bytes leave the semantic Message intact. Host-bound/source-bearing blocks, HostSkill
  provenance, ConversationMessage publication, Operator endpoints, and backup/GC
  integration remain separate implementation work. The trusted Core publisher allocates
  presentation/Event identities; agent intent and external clients cannot choose them.
  The storage port returns the same opaque identity-unavailable error for ID/message
  collisions without another Workspace identity or aggregate version.
- migration 13 replaces `environment_identity_immutable` so Environment sharing scope and
  Coworker/principal ownership, along with the existing immutable identity/configuration
  fields, cannot be changed after creation. Lifecycle state (`status`, `health`, and
  provider-reported `budget_enforcement`), `updated_at`, and `version` remain mutable under
  their owning transition/version guards. The original migration definitions are unchanged.

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
