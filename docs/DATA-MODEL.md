# Data Model

IDs are opaque, globally unique and stable. Durable records carry the creation or receipt time relevant to their lifecycle. Mutable aggregates carry an optimistic `version`; `updated_at` appears where it belongs to that aggregate's update contract. Immutable records have no update API. Timestamps are RFC 3339 UTC. Canonical entities and relationships are defined here; shared enums are in `SCHEMAS.md`, transitions in `STATE-MACHINES.md`, and SQL representation in `schemas/sqlite-v1.sql`.

## Workspace

```text
Workspace {
  workspace_id: WorkspaceId
  name: string
  owner_principal_id: PrincipalId
  replication_policy: ReplicationPolicy
  replication_scope_root_ids: WorkspaceRootId[]
  current_instruction_revision: u64?
  default_agent_binding_id: AgentBindingId?
  hub_runtime_id: RuntimeId?
  status: ACTIVE | ARCHIVED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

ReplicationPolicy = LOCAL_ONLY | METADATA_ONLY | ACTIVE_TASK_INPUTS |
  SELECTED_FOLDERS | FULL_WORKSPACE
```

`replication_scope_root_ids` is the Workspace projection of normalized
`workspace_replication_roots` rows. Workspace creation starts with an empty selection;
`SELECTED_FOLDERS` is set only after roots have been created and selected in a separate
versioned policy update.

### WorkspaceInstructionRevision

```text
WorkspaceInstructionRevision {
  workspace_id: WorkspaceId
  revision: u64
  parent_revisions: u64[]
  content_ref: ResourceRef
  content_digest: Sha256Digest
  authored_by: PrincipalRef
  created_at: Timestamp
}
```

Instruction revisions are immutable, bounded, digest-checked, and treated as untrusted
content. Tasks pin the selected revision. Updating Workspace instructions affects future
Tasks; an existing Task changes context only through an explicit TaskSpecRevision.
Creation is Hub-authoritative and carries `If-Match` plus explicit `parent_revisions`.
Revision 1 has no parents; an ordinary edit must name exactly the current instruction
revision. A stale request returns `STALE_WORKSPACE_VERSION` and creates no revision. An
explicit merge may name multiple existing parents. Offline edits on a non-Hub Runtime are
pending intents until the authoritative Hub accepts them; they are never silently
last-writer-wins.
Workspace has at most one default enabled AgentBinding; a Conversation may override it.
Changing the default affects newly admitted Conversation turns and Tasks only.
The Workspace default and Conversation override are preferences for admission, not
permanent ownership: every ConversationTurn records its chosen AgentBinding, and each
Task records its lead binding. Missing or disabled selection fails admission with
`AGENT_UNAVAILABLE`; the user draft remains available and no partially admitted turn or
Task is created.

The default for a local-only Workspace is `LOCAL_ONLY`. When the user enables cloud for a personal Workspace, the default is `ACTIVE_TASK_INPUTS`. `SELECTED_FOLDERS` requires one or more active `WorkspaceRootId`s; the user first adds each persistent root, then selects those stable root identities for replication. The root follows new observed revisions and descendants under its authorized folder boundary; the policy does not pin one folder-content revision. Workspace policy and each root's replication setting intersect, and neither can broaden the other's authority. Task-produced outputs follow the Workspace policy. A policy selects which resources may replicate; it never authorizes a capability or secret. A policy change applies prospectively and does not erase content already copied to another Runtime. Archiving is allowed only after Tasks are terminal and Automations are disabled; an archived Workspace is read-only; existing authorized Artifact/Resource reads remain available, while all domain mutations—including Task materialization, Artifact/Library changes, connection changes, and inbound channel work—are rejected.

## WorkspaceBackupManifest

```text
WorkspaceBackupManifest {
  backup_id: BackupId
  workspace_id: WorkspaceId
  schema_version: u32
  event_cursors: RuntimeEventCursor[]
  database_snapshot_ref: BlobRef
  blob_manifest_ref: BlobRef
  blob_manifest_digest: Sha256Digest
  encryption_key_ref: BackupKeyRef
  integrity_digest: Sha256Digest
  manifest_authentication: string
  created_at: Timestamp
  verified_at: Timestamp
}
```

Only a complete, integrity-checked backup receives a manifest row; failed/in-progress
capture state is not represented as a restorable backup. The manifest is immutable.
Database snapshot and blob-manifest contents are encrypted at rest under the referenced
key; the key bytes are never stored in LiteCowork. Event cursors identify the last
included sequence for each origin Runtime at the snapshot barrier. A v1 backup is a
complete point-in-time recovery set, not an incremental delta. Restore seeds replication
cursors at those positions, registers a new Runtime identity, and never restores active
lease authority; later authorized peer events may advance state through ordinary Mesh
replication.
Opaque SecretRefs may be present in the snapshot, but secret bytes are not included and
their availability must be revalidated after restore. There is exactly one cursor per
origin Runtime. `integrity_digest` is SHA-256 over the RFC 8785 canonical manifest fields
excluding `integrity_digest` and `manifest_authentication`; the provider-generated
`manifest_authentication` authenticates those fields, including the digest, under the
referenced key. Stored BlobRef digests cover the encrypted bytes; `blob_manifest_digest`
covers the canonical decrypted blob-list bytes.

## Conversation

```text
Conversation {
  conversation_id: ConversationId
  workspace_id: WorkspaceId
  title: string?
  active_agent_binding_id: AgentBindingId?
  created_at: Timestamp
  version: u64
}

ConversationMessage {
  message_id: MessageId
  conversation_id: ConversationId
  author: PrincipalRef
  role: USER | AGENT | SYSTEM_NOTICE | CHANNEL
  agent_session_id: AgentSessionId?
  agent_binding_id: AgentBindingId?
  turn_id: ConversationTurnId?
  content: MessageContentBlock[]
  resource_refs: ResourceRef[]
  source_channel_ref: ChannelThreadRef?
  created_at: Timestamp
}

ConversationTurn {
  turn_id: ConversationTurnId
  conversation_id: ConversationId
  user_message_id: MessageId
  agent_session_id: AgentSessionId?  # current/most recent session; messages retain per-response provenance
  status: OPEN | RUNNING | WAITING_USER | WAITING_DEPENDENCY | COMPLETED | FAILED | CANCEL_REQUESTED | CANCELLED
  retry_ordinal: u32
  created_at: Timestamp
  settled_at: Timestamp?
  version: u64
}
```

Messages are append-only; edits create a replacement/revision event rather than destructive rewrite of history. Agent responses record the AgentSession and binding that produced them. Switching agents creates/selects a session without rewriting prior provenance.

Each submitted user turn is durable independently of whether it materializes a Task. The
turn selects the active Conversation AgentBinding and owns its response lifecycle.

## Task

```text
Task {
  task_id: TaskId
  workspace_id: WorkspaceId
  conversation_id: ConversationId?
  current_spec_revision: u64
  current_plan_revision: u64?
  status: TaskStatus
  resume_status: TaskStatus?
  routine_id: RoutineId?
  routine_revision: u64?
  automation_id: AutomationId?
  automation_occurrence_id: OccurrenceId?
  lead_agent_binding_id: AgentBindingId
  blocking_conditions: Blocker[]
  priority: LOW | NORMAL | HIGH
  created_by: PrincipalRef
  created_at: Timestamp
  updated_at: Timestamp
  completed_at: Timestamp?
  version: u64
}
```

Every Task has a selected lead AgentBinding at creation. The field is nullable only in
historical/imported records that predate a binding; v1 creation rejects a missing or
disabled binding. `blocking_conditions` is the current actionable explanation for a
BLOCKED/NEEDS_USER projection. Each change is evented; resolving a condition removes it
from the current projection without deleting its historical event.

### TaskSpecRevision

Immutable.

```text
TaskSpecRevision {
  task_id: TaskId
  revision: u64
  parent_revisions: u64[]
  objective: string
  constraints: string[]
  non_goals: string[]
  required_outputs: OutputRequirement[]
  acceptance_criteria: AcceptanceCriterion[]
  approvals_required: ApprovalRequirement[]
  input_refs: PinnedResourceRef[]
  workspace_instruction_revision: u64?
  budget: BudgetSpec?
  deadline: Timestamp?
  source_message_refs: MessageId[]
  placement_preference: PlacementPreference
  preferred_lead_agent_binding_id: AgentBindingId?
  authored_by: PrincipalRef
  created_at: Timestamp
}
```

Unique: `(task_id, revision)`.
Every parent revision belongs to the same Task and has a lower revision number. The
initial revision has no parents. When concurrent revisions share one parent, the Task
stays pointed at the last common revision and becomes `NEEDS_USER`; a resolution
revision records all sibling parents before becoming current.

### PlanRevision

Immutable.

```text
PlanRevision {
  task_id: TaskId
  revision: u64
  task_spec_revision: u64
  produced_by_agent_session_id: AgentSessionId
  produced_by_attempt_id: AttemptId?
  steps: PlannedStep[]
  reason_for_revision: string?
  created_at: Timestamp
}
```

Core validates graph structure, references and cycles. Core does not judge strategy quality.

The agent's proposed graph uses stable logical keys before Step IDs are allocated:

```text
PlannedStep {
  logical_key: string
  title: string
  objective: string
  depends_on_logical_keys: string[]
  required_capabilities: CapabilityRequirement[]
  acceptance_criteria: AcceptanceCriterion[]
}
```

Logical keys are unique within a PlanRevision. Every dependency names another key in
that revision; TaskService rejects missing keys and cycles before acceptance. Accepted
keys are mapped to durable Step IDs atomically with the PlanRevision pointer update.

## Step

```text
Step {
  step_id: StepId
  task_id: TaskId
  plan_revision: u64
  logical_key: string?
  title: string
  objective: string
  dependencies: StepId[]
  required_capabilities: CapabilityRequirement[]
  acceptance_criteria: AcceptanceCriterion[]
  status: StepStatus
  current_attempt_id: AttemptId?
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}
```

## Attempt

```text
Attempt {
  attempt_id: AttemptId
  task_id: TaskId
  step_id: StepId
  parent_attempt_id: AttemptId?
  agent_binding_id: AgentBindingId
  agent_session_id: AgentSessionId?
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  environment_id: EnvironmentId
  capability_grant_ids: CapabilityGrantId[]
  execution_lease_id: ExecutionLeaseId?
  failover_class: FailoverClass
  checkpoint_ref: ResourceRef?
  status: AttemptStatus
  failure: FailureRecord?
  started_at: Timestamp?
  settled_at: Timestamp?
  created_at: Timestamp
  version: u64
}
```

The Attempt's `agent_session_id` points to its current/most recent execution session and
may be updated by an audited same-Attempt session replacement. Each AgentSession remains
an immutable historical record tied to the Attempt; replacing the pointer does not change
the Attempt's pinned AgentBinding, Runtime/incarnation, Environment, or lease identity.

## Agent records

```text
AgentProfile {
  agent_profile_id: AgentProfileId
  provider_key: string
  display_name: string
  endpoints: AgentEndpoint[] # stable protocol identities; current availability comes from RuntimeOffer
  discovered_at: Timestamp
}

AgentEndpoint {
  endpoint_id: AgentEndpointId
  agent_profile_id: AgentProfileId
  protocol: ACP | A2A | SDK | API | CLI | TERMINAL
  topology: LOCAL_INTERACTIVE | REMOTE_AGENT_SERVICE | VENDOR_SERVICE | PROCESS_ADAPTER
  protocol_version?: string
  capabilities: AgentCapabilities
}

AgentEndpointBinding { # Runtime-local operational relation
  endpoint_id: AgentEndpointId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  endpoint_ref: string # private command/socket/URL locator without credentials; never replicated/backed up
  observed_at: Timestamp
  expires_at: Timestamp?
}

EndpointSelectionPolicy {
  mode: AUTO_COMPATIBLE | PINNED_ENDPOINT
  endpoint_id?: AgentEndpointId
  required_features: AgentFeature[]
  preferred_topologies: AgentEndpointTopology[]
}

AgentBinding {
  agent_binding_id: AgentBindingId
  workspace_id: WorkspaceId
  agent_profile_id: AgentProfileId
  runtime_id: RuntimeId?
  endpoint_selection_policy: EndpointSelectionPolicy
  auth_ref: SecretRef? # reference only; never auth bytes
  configuration: JsonObject # non-secret options only
  enabled: bool
  created_at: Timestamp
  version: u64
}

AgentSession {
  agent_session_id: AgentSessionId
  scope: AgentSessionScope
  task_spec_revision: u64? # required for Task scopes; absent for Conversation
  agent_binding_id: AgentBindingId
  endpoint_id: AgentEndpointId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  configuration_digest: Sha256Digest?
  status: AgentSessionStatus
  started_at: Timestamp
  last_event_at: Timestamp?
  closed_at: Timestamp?
  version: u64
}

AgentSessionHostBinding { # Runtime-local operational relation
  agent_session_id: AgentSessionId
  host_instance_id: AgentHostInstanceId
  native_session_ref: string? # opaque, adapter-owned, never replicated/backed up
  bound_at: Timestamp
}
```

`AgentSessionScope` is a tagged union: `CONVERSATION {conversation_id,
conversation_turn_id}`, `TASK_PLANNING {task_id}`, or `ATTEMPT_EXECUTION {task_id,
attempt_id}`. A Conversation session is bound to exactly one durable turn and cannot
be reused for another turn. Task-scoped sessions pin the exact `TaskSpecRevision` used
to construct their bounded context packet. For `TASK_PLANNING`, this must be the Task's
current revision at admission. For `ATTEMPT_EXECUTION`, it is the revision referenced
by the Attempt's Step PlanRevision, even if the Task later receives a newer spec.
Conversation sessions have no
Task mutation authority and may invoke only authorized read-only capabilities. Planning
sessions have no Attempt/lease/Environment write access and may clarify intent, read the
Task packet, and propose a plan. Execution sessions require exactly one admitted Attempt,
its current lease, Environment, and scoped grants. Only TaskService promotes an authorized
plan proposal. The session's Runtime and incarnation are immutable provenance. `endpoint_id`
is a historical selection identity and need not resolve to a live `AgentEndpoint` offer on
another peer. The optional `AgentSessionHostBinding` is Runtime-local, must match that
Runtime/incarnation and the selected endpoint, and is removed when the session settles; a
replacement Runtime never receives it.

`PlanningAssignment` is an internal transient admission envelope, not a durable entity.
The Task and its `TASK_PLANNING` AgentSession are the records of truth; at most one active
planning session per Task is enforced by storage. A replacement planner uses a new session
and a freshly built envelope pinned to the current TaskSpec revision.

## Runtime and incarnation

```text
Runtime {
  runtime_id: RuntimeId
  workspace_id: WorkspaceId
  device_identity: DeviceIdentity
  runtime_version: SemVer
  platform: string
  architecture: string
  roles: RuntimeRole[]
  trust_zone: TrustZone
  availability: RuntimeAvailability
  startup_policy: RuntimeStartupPolicy
  current_incarnation_id: RuntimeIncarnationId?
  resource_capacity: ResourceCapacity
  last_seen: Timestamp
  version: u64
}

RuntimeIncarnation {
  runtime_incarnation_id: RuntimeIncarnationId
  runtime_id: RuntimeId
  process_started_at: Timestamp
  litecowork_version: SemVer
  recovered_from_unclean_shutdown: boolean
  recovery_state: STARTING | RECOVERING | READY | DEGRADED | DRAINING | STOPPING | STOPPED
  ready_at: Timestamp?
  stopped_at: Timestamp?
  version: u64
}

RuntimeIncarnationLocalObservation {
  runtime_incarnation_id: RuntimeIncarnationId
  os_boot_id: string?
  observed_at: Timestamp
  diagnostic_ref: ResourceRef?
}
```

`RuntimeIncarnation` is a durable Runtime Mesh registry record. An authenticated Runtime
registers it before publishing offers or events that refer to it; Workspace peers retain
the compact public record so replicated aggregates can preserve origin-incarnation
references. `os_boot_id` and diagnostic details remain in the local-only observation and
never replicate or enter Workspace backups. RuntimeId identifies the paired installation;
every daemon lock-holder start creates a new incarnation. Process-bound handles, local
offers, and observation cursors are tagged with the incarnation that observed them.
Runtime lifecycle and boot recovery are defined in `RUNTIME-LIFECYCLE.md`.
`RuntimeIncarnationLocalObservation` is keyed by incarnation and remains local to the
Runtime. It may help determine whether the OS itself rebooted, but it is not authority for
lease or Effect settlement.

Runtime advertised inventory is maintained separately as offers so it can expire without rewriting runtime identity.

```text
RuntimeOffer {
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  offer_kind: AGENT_ENDPOINT | CAPABILITY_PROVIDER | ENVIRONMENT_PROVIDER | CHANNEL_ADAPTER | TRIGGER_PROVIDER | APPLICATION
  offer_ref: string
  compatible: boolean
  readiness: AVAILABLE | STARTABLE | STARTING | READY | BUSY | DEGRADED | OFFLINE | NEEDS_AUTH | UNAVAILABLE
  constraints: JsonObject
  observed_at: Timestamp
  expires_at: Timestamp
}
```

For `offer_kind = APPLICATION`, `offer_ref` is a normalized `ApplicationRef`, not a
filesystem path or process ID. `constraints` carries only bounded placement evidence such
as installed version, supported actions/Environment classes, and launch requirements.
Application availability uses the same expiring `OfferReadiness` projection as other
Runtime offers.

```text
ApplicationRef {
  platform: string
  package_or_bundle_id: string
}

ApplicationInstanceBinding { # Runtime-incarnation-local operational binding
  application_instance_id: string
  application_ref: ApplicationRef
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  ownership: LITECOWORK | USER | EXTERNAL
  process_identity_ref: string? # opaque process-start identity, not PID alone
  state: STARTING | READY | BUSY | DEGRADED | STOPPING | STOPPED | FAILED
  attached_at: Timestamp
  last_observed_at: Timestamp
}
```

These bindings and process/window handles are local operational state, never replicated or
included in Workspace backups. A user-owned or external process is detached, not stopped,
by LiteCowork. Window title/document observation is not part of application inventory.

`AgentProfileView` joins stable AgentEndpoint identities with their unexpired
`AGENT_ENDPOINT` RuntimeOffers. Each endpoint observation carries endpoint, Runtime,
incarnation, readiness, compatibility, observation time, and expiry; it contains no local
endpoint locator.

## Environment

```text
Environment {
  environment_id: EnvironmentId
  runtime_id: RuntimeId
  provider_kind: string
  class: LOCAL_WORKSPACE | GIT_WORKTREE | CONTAINER | VM | CLOUD_SANDBOX | REMOTE_MACHINE | BROWSER | DESKTOP
  lifetime: ATTEMPT | TASK_RETAINED | WORKSPACE_PERSISTENT
  owner_workspace_id: WorkspaceId
  owner_task_id: TaskId?
  status: EnvironmentStatus
  health: HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
  name: string
  source_resources: PinnedResourceRef[]
  resource_limits: ResourceLimits
  network_policy: NetworkPolicy
  budget_ceiling: BudgetSpec
  budget_enforcement_policy: REQUIRE_PROVIDER_ENFORCED | ALLOW_HOST_MONITORED
  budget_enforcement: PROVIDER_ENFORCED | HOST_MONITORED | UNAVAILABLE
  provision_preview_digest: Sha256Digest?
  retention_expires_at: Timestamp?
  backup_policy: EXCLUDED | INCLUDE_CHECKPOINTS
  created_by_incarnation_id: RuntimeIncarnationId?
  isolation: IsolationSpec
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

EnvironmentProviderBinding { # Runtime-local operational relation
  environment_id: EnvironmentId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  provider_kind: string
  opaque_locator_ref: string
  observed_at: Timestamp
  expires_at: Timestamp?
}

EnvironmentProvisionPreviewRecord {
  preview_digest: Sha256Digest
  workspace_id: WorkspaceId
  authenticated_principal_id: PrincipalId
  normalized_request_digest: Sha256Digest
  selected_runtime_id: RuntimeId?
  selected_runtime_incarnation_id: RuntimeIncarnationId?
  resource_revision_refs: PinnedResourceRef[]
  provider_offer_ref: string?
  provider_offer_revision: string?
  policy_digest: Sha256Digest
  quote_digest: Sha256Digest?
  expires_at: Timestamp
  status: ISSUED | CONSUMED | EXPIRED
  consumed_by_request_id: RequestId?
  environment_id: EnvironmentId?
  created_at: Timestamp
}
```

`EnvironmentProvisionPreviewRecord` is short-lived admission state, not a replicated
Workspace fact. It contains no provider locator, credentials, or raw secret material.
The record is consumed atomically with persistent Environment admission and budget
reservation; successful Environment/event history retains only the preview digest.
The preview digest is SHA-256 of RFC 8785 canonical JSON for an object with exactly
`schema = litecowork.environment-provision-preview.v1`, `workspace_id`,
`authenticated_principal_id`, `normalized_request_digest`, selected Runtime and
incarnation IDs (nullable only when no Runtime is eligible), source Resource revision
refs in request order, provider offer ref/revision, policy digest, nullable quote digest,
and expiry. `normalized_request_digest` is SHA-256 of RFC 8785 canonical JSON for the
validated request after documented defaults are applied and with `preview_digest`
omitted. The same authenticated principal must confirm; changed body or eligibility basis
cannot consume the record. Workspace archival, restore, or expiry invalidates unconsumed
previews; backups and cross-Runtime replication omit preview records.
The default preview lifetime is five minutes, shortened to an earlier provider offer/quote
expiry. Issuance is authenticated and rate-limited per Workspace/principal. The digest is
an opaque lookup key, never a credential.

```text
AgentHostInstance {
  host_instance_id: AgentHostInstanceId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  agent_profile_id: AgentProfileId
  endpoint_id: AgentEndpointId
  hosting_mode: REMOTE_API | REMOTE_A2A | LOCAL_SHARED_DAEMON | LOCAL_PER_SESSION | EMBEDDED_SDK | EXTERNAL_PROCESS
  state: STARTING | READY | BUSY | DEGRADED | STOPPING | STOPPED | FAILED
  process_identity_ref: string?  # opaque identity, never PID alone
  ownership: LITECOWORK | EXTERNAL | REMOTE
  active_session_count: u32 # derived from local AgentSessionHostBinding rows
  started_at: Timestamp
  last_used_at: Timestamp
  idle_since: Timestamp?
}

CapabilityHostInstance {
  host_instance_id: CapabilityHostInstanceId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  capability_ref: CapabilityRef
  capability_identity_digest: Sha256Digest
  configuration_digest: Sha256Digest
  isolation_partition_digest: Sha256Digest
  sharing_policy: EXCLUSIVE | TASK_ISOLATED | TRUST_PARTITION_SHARED
  hosting_mode: LOCAL_MANAGED | REMOTE_PROVIDER
  provider_instance_ref: string? # opaque LitePSM/provider reference; Runtime-private
  state: STARTING | READY | BUSY | DEGRADED | STOPPING | STOPPED | FAILED
  health: HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
  active_activation_count: u32 # derived view value; never independently incremented
  observed_at: Timestamp
  expires_at: Timestamp
  version: u64
}

CapabilityActivationHostBinding {
  activation_id: CapabilityActivationId
  host_instance_id: CapabilityHostInstanceId
  provider_handle_ref: string? # Runtime-private, activation-specific, never replicated
  bound_at: Timestamp
}

ExecutionDependencyPlan {
  plan_id: ExecutionDependencyPlanId
  task_id: TaskId
  step_id: StepId
  task_version: u64
  step_version: u64
  task_spec_revision: u64
  plan_revision: u64
  selected_runtime_id: RuntimeId?
  plan_digest: Sha256Digest
  environment_candidates: EnvironmentPlacementCandidate[]
  nodes: ExecutionDependencyNode[]
  status: READY | WAITING | INFEASIBLE | STALE
  computed_at: Timestamp
  expires_at: Timestamp
}

EnvironmentPlacementCandidate {
  candidate_id: string
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  environment_id: EnvironmentId
  environment_version: u64
  readiness: AVAILABLE | STARTABLE | STARTING | READY | BUSY | DEGRADED | OFFLINE | NEEDS_AUTH | UNAVAILABLE
  eligible: boolean
  dependency_node_ids: string[]
  blockers: Blocker[]
}

ExecutionDependencyNode {
  node_id: string
  kind: RESOURCE | AGENT_HOST | ENVIRONMENT | CAPABILITY | SECRET | APPLICATION | RUNTIME_ROLE
  ref: string
  depends_on_node_ids: string[]
  runtime_id?: RuntimeId
  readiness: AVAILABLE | STARTABLE | STARTING | READY | BUSY | DEGRADED | OFFLINE | NEEDS_AUTH | UNAVAILABLE
  blocker_code?: string
}
```

The durable `Environment` contains no provider locator or native handle. An
`EnvironmentProviderBinding` is local operational state, tagged with the incarnation that
observed it. After restart, an old binding is stale; the provider must verify and attach
again before the current incarnation receives a binding. Bindings and their locator
material are excluded from events, replication, and Workspace backups. Provider-only
Environment checkpoints are also local optimizations: their opaque provider reference is
stored only in `EnvironmentCheckpointProviderBinding`. A checkpoint digest identifies
provider-reported snapshot bytes but does not make those bytes available or portable.
Cross-Runtime recovery uses the Task `ResumePacket`, Resource/Artifact references, and a
new Environment. If a future provider exports portable checkpoint bytes, those bytes must
become a separately content-addressed Blob/Artifact with an explicit replication and
backup policy; the provider handle alone never qualifies.

`AgentHostInstance` is Runtime-operational inventory, not replicated Task truth; it is
reconciled at every incarnation. Remote API/A2A routes may use a logical instance without
a local process. `ExecutionDependencyPlan` is a short-lived, read-only placement/preparation
projection, not a PlanRevision or workflow. It contains no secret bytes and is recomputed
when any referenced offer, resource, Runtime incarnation, grant, Environment, or provider
state changes. Its digest binds the exact Task/Step/spec/plan versions and candidate basis.
Only eligible candidates are accepted by a recovery override; a stale digest never starts
an Attempt. Candidate IDs are opaque and expire with the plan.

```text
EnvironmentCheckpoint {
  checkpoint_id: EnvironmentCheckpointId
  environment_id: EnvironmentId
  digest: Sha256Digest
  portable_snapshot_ref: BlobRef? # present only when provider exports actual snapshot bytes
  created_at: Timestamp
}

EnvironmentCheckpointProviderBinding { # Runtime-local operational relation
  checkpoint_id: EnvironmentCheckpointId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  provider_ref: string # opaque, provider-owned, never replicated/backed up
  observed_at: Timestamp
}

EnvironmentControlLease {
  control_lease_id: EnvironmentControlLeaseId
  environment_id: EnvironmentId
  task_id: TaskId
  attempt_id: AttemptId
  owner_kind: AGENT | HUMAN
  owner_ref: AgentSessionId | PrincipalRef
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  epoch: u64
  issuer_key_version: u32
  fencing_token_digest: Sha256Digest
  state: ACTIVE | RELEASING | RELEASED | EXPIRED | REVOKED
  acquired_at: Timestamp
  expires_at: Timestamp
  version: u64
}
```

`EnvironmentCheckpoint.digest` identifies the provider checkpoint representation. When
`portable_snapshot_ref` is present, it is the content-addressed byte object and its digest
must equal `digest`; that is the only form eligible for cross-Runtime restore. Provider-only
checkpoints may retain the digest and local provider binding without exporting bytes. The
`Environment.backup_policy` controls whether portable checkpoint blobs are included in a
Workspace backup; it cannot turn a provider handle into backup content. Environment
checkpoint metadata alone never makes a checkpoint portable.

An EnvironmentControlLease arbitrates interactive input within an Environment; it is
distinct from the Runtime ExecutionLease. Every mediated browser/desktop input carries
the current control epoch. Human takeover increments the epoch and invalidates queued or
late AgentSession commands. Returning control to an agent requires a fresh observation,
drift reconciliation, explicit user action, and a new epoch.

The durable lease record stores only `fencing_token_digest` and the non-secret
`issuer_key_version`. `FencingCredential` is an HMAC-derived pseudorandom secret delivered
over authenticated private control to the current enforcing provider; it is never part of
the Operator projection, event payload, aggregate-state blob, log, or Workspace backup.
The credential is stable within its lease epoch; authoritative `expires_at` limits its
validity. Runtime-local credential bindings are incarnation-scoped and discarded on
restart. Every use also requires authenticated caller identity and current lease/epoch
authorization; possession alone is insufficient.

## Capability records

```text
CapabilityRef # normalized value type defined in SCHEMAS.md

CapabilityLock {
  task_id: TaskId
  capability_ref: CapabilityRef
  locked_at: Timestamp
}

CapabilityGrant {
  capability_grant_id: CapabilityGrantId
  scope: CapabilityGrantScope
  capability_ref: CapabilityRef
  allowed_operations: string[]
  resource_scope: ResourceScope
  secret_refs: SecretRef[]
  granted_by: PrincipalRef
  expires_at: Timestamp?
  status: ACTIVE | REVOKED | EXPIRED
  created_at: Timestamp
  version: u64
}

CapabilityScope =
  CONVERSATION { conversation_id: ConversationId }
  | TASK_PLANNING { task_id: TaskId }
  | ATTEMPT_EXECUTION { task_id: TaskId, attempt_id: AttemptId }

CapabilityGrantScope = CapabilityScope

CapabilityActivation {
  activation_id: CapabilityActivationId
  capability_ref: CapabilityRef
  scope: CapabilityScope
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  mode: DIRECT_MCP | GATEWAY_PROXY | NATIVE_AGENT | REMOTE_PROVIDER
  status: CapabilityActivationStatus
  health: HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

```

`RuntimeOffer` is the single expiring availability projection. It references a typed
offer record by kind; there is no second `CapabilityOffer` source of truth. Runtime offer
compatibility is advisory and must be rechecked at admission.

`CapabilityHostInstance` is a Runtime-local operational view, not a LitePSM package record
or replicated Task aggregate. It reports normalized provider identity, isolation,
readiness, health freshness, and LiteCowork use counts; process IDs, package internals,
credentials, and LitePSM protocol fields remain private/opaque. Its active Activation
count is derived from local `CapabilityActivationHostBinding` rows joined to nonterminal
`CapabilityActivation` records (`STARTING`, `ACTIVE`, or `STOPPING`), never independently
incremented. `CapabilityActivation.health` is the result of that Activation's readiness
check; host health is the latest shared-instance health observation. They have different
scopes and timestamps. Host observations are tied to a Runtime incarnation and expire;
after restart the provider adapter must revalidate before reuse. Both host instances and
host bindings are operational local state, excluded from event state blobs and Workspace
backups.
Sharing requires matching pinned capability/configuration identity and authorized
isolation partition plus declared concurrency support. Unknown or stateful providers
default to Task isolation; sharing additionally requires explicit provider compatibility
and Trust approval. For `EXCLUSIVE`, the isolation partition is unique to the Activation;
for `TASK_ISOLATED`, it is unique to the Task; `TRUST_PARTITION_SHARED` uses only a
TrustService-issued partition. Otherwise the Supervisor uses an exclusive or Task-isolated
instance.

The external package's component shape is not modeled here; LitePSM owns that contract.
LiteCowork persists only normalized references and the digest/version used by a Task.

## Connections and human channels

```text
Connection {
  connection_id: ConnectionId
  workspace_id: WorkspaceId
  external_provider_ref: string
  account_ref: string?
  secret_refs: SecretRef[]
  status: CONNECTING | CONNECTED | DEGRADED | REAUTH_REQUIRED | DISCONNECTED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

ChannelBinding {
  channel_binding_id: ChannelBindingId
  workspace_id: WorkspaceId
  connection_id: ConnectionId?
  provider_ref: string
  external_account_ref: string
  identity_ref: PrincipalRef
  assurance_level: AssuranceLevel
  allowed_actions: ChannelAction[]
  status: ACTIVE | REVOKED | DEGRADED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

```

Provider setup supplies the authenticated identity and assurance level. A newly created
binding starts with `allowed_actions = []`; the owner explicitly grants each action.
Changing allowed actions increments the binding aggregate version and is authorized by
TrustService. A revoked binding is terminal in v1.

```text
ChannelThreadMapping {
  channel_binding_id: ChannelBindingId
  provider_thread_id: string
  conversation_id: ConversationId
  created_at: Timestamp
}
```

Connection and ChannelBinding store references and authority metadata, never credential
bytes. LitePSM/provider-specific account schemas remain outside LiteCowork.
Their `external_provider_ref`, `provider_ref`, account, and thread identifiers are stable
non-secret logical IDs. They must not encode access tokens, signed URLs, or private
endpoint locators; provider adapters retain any such material in private storage.

```text
ProviderCircuit {
  runtime_id: RuntimeId
  provider_kind: CAPABILITY | ENVIRONMENT | CHANNEL
  provider_ref: string
  status: CLOSED | OPEN | HALF_OPEN
  failure_window_started_at: Timestamp?
  consecutive_failures: u32
  open_until: Timestamp?
  half_open_probe_id: string?
  updated_at: Timestamp
  version: u64
}
```

ProviderCircuit controls LiteCowork call admission after repeated failures. It is not a
provider process supervisor and does not count or command process restarts. LitePSM owns
package process lifecycle; Environment and Channel adapters use their owning lifecycle
services. `provider_ref` is a normalized non-secret provider identity, never a process
handle, URL credential, or upstream bearer token.

## Artifact

```text
Artifact {
  artifact_id: ArtifactId
  workspace_id: WorkspaceId
  resource_id: ResourceId       # stable Resource identity for this named output
  task_id: TaskId?
  kind: string
  display_name: string
  current_version: u64
  library_status: TRANSIENT | SAVED | ARCHIVED
  created_at: Timestamp
  version: u64
}

ArtifactVersion {
  artifact_id: ArtifactId
  version: u64
  resource_revision_id: ResourceRevisionId
  created_by_attempt: AttemptId?
  input_refs: PinnedResourceRef[]
  content: ArtifactContent
  provenance: ProvenanceRecord
  verification_refs: EvidenceId[]
  created_at: Timestamp
}
```

```text
ArtifactContent =
  MANAGED_BLOB {
    storage_ref: BlobRef
    content_digest: Sha256Digest
    media_type: string
    size_bytes: u64
  }
  | EXTERNAL_RESOURCE {
    resource_ref: PinnedResourceRef
    provider_revision?: string
    observed_digest?: Sha256Digest
    observed_at: Timestamp
  }
```

Managed content is immutable, content-addressed, and digest-verified before publication.
External content pins a provider Resource/revision and may omit a content digest when
the provider does not expose one; it is not represented as a local BlobRef. A later
provider revision creates a new ArtifactVersion or an explicit stale observation.

Every Artifact has one stable `Resource` of kind `ARTIFACT`; every ArtifactVersion maps to
exactly one immutable ResourceRevision of that Resource. For a managed version, the
ResourceRevision carries the blob digest and its ArtifactStore location resolves the BlobRef.
For an external version, the ResourceRevision records the observed provider revision/digest
when available, while ArtifactContent retains the source ResourceRef. The Artifact's
ResourceRef is the reusable input identity; pinning `resource_revision_id` selects the exact
ArtifactVersion. ArtifactStore appends the ArtifactVersion and ResourceRevision and updates
both current-version pointers atomically. The Artifact Resource has a strong internal
ProviderIdentity scoped to `litecowork.artifact-store` and keyed by ArtifactId; its
ResourceLocation uses an opaque ArtifactStore locator, never a local path. ResourceService
indexes the Artifact identity and resolves its bytes through the ArtifactStore's registered
ResourceLocationProvider adapter; WorldIndexer does not watch ArtifactStore paths.

For every ArtifactVersion, `input_refs` is the canonical dependency set. ArtifactStore
requires it to equal the distinct pinned refs in `provenance.source_inputs` and all
`provenance.transformations[].inputs`; an `EXTERNAL_RESOURCE` content ref must also be in
that set. The ResourceInput records retain any observed byte digest, while `input_refs`
provides the normalized dependency identity used by DependencyService. Missing or extra
provenance inputs reject publication so a consumed source cannot escape invalidation.

Unique: `(artifact_id, version)`. `current_version` must reference an existing version of the same Artifact. Publishing uses optimistic concurrency on Artifact.version; it allocates the next integer version and updates `current_version` atomically. Artifact.version is the mutable aggregate revision, distinct from the immutable content version. Library-state transitions increment Artifact.version but do not change current_version.

## Effect / Evidence

```text
Effect {
  effect_id: EffectId
  task_id: TaskId
  attempt_id: AttemptId
  capability_ref: CapabilityRef?
  operation: string
  target: ResourceRef | string
  idempotency_key: string?
  state: EffectState
  request_digest: Sha256Digest
  dispatch_ordinal: u32
  result_ref: ResourceRef?
  observed_state: JsonObject?
  verification_ref: EvidenceId?
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

Evidence {
  evidence_id: EvidenceId
  task_id: TaskId
  subject_ref: string
  level: REPORTED | OBSERVED | VERIFIED
  kind: string
  producer: PrincipalRef | ServiceRef
  payload_ref: ResourceRef?
  payload_digest: Sha256Digest?
  created_at: Timestamp
}
```

Each new observation is a new immutable Evidence row; an earlier `REPORTED` record is
not upgraded in place to `OBSERVED` or `VERIFIED`.

```text
VerificationRun {
  verification_run_id: VerificationRunId
  task_id: TaskId
  criterion_id: string
  task_spec_revision: u64
  criterion_digest: Sha256Digest
  verifier_kind: string
  verifier_version: string
  subject_refs: PinnedResourceRef[]
  inputs: ResourceInput[]
  status: PENDING | RUNNING | PASSED | FAILED | INCONCLUSIVE
  evidence_refs: EvidenceId[]
  started_at: Timestamp?
  completed_at: Timestamp?
}
```

Verification is valid only for the exact immutable criterion in the pinned
TaskSpecRevision and exact revision-pinned `inputs` set. Retaining a
criterion ID while changing its text does not preserve evidence validity. A changed
criterion, verifier version where semantics change, or input revision/observed digest requires a
new VerificationRun. Each `ResourceInput` keeps the logical Resource revision and, when
available, the digest of the bytes actually consumed together so the association cannot
drift through parallel arrays. This consumer-observed digest is distinct from an optional
provider-observed digest stored on `ResourceRevision`.

## Approval

```text
Approval {
  approval_id: ApprovalId
  task_id: TaskId
  requested_by_attempt: AttemptId?
  kind: string
  action_summary: string
  target_ref: ResourceRef
  scope_digest: Sha256Digest
  action_digest: Sha256Digest
  risk: SAFE | SENSITIVE | HIGH_IMPACT
  required_assurance: AssuranceLevel
  status: PENDING | APPROVED | DENIED | EXPIRED | CANCELLED
  requested_at: Timestamp
  expires_at: Timestamp?
  resolved_by: PrincipalRef?
  resolved_at: Timestamp?
  version: u64
}
```

Approval binds `action_digest`, target ResourceRef, required assurance, Task, and (when
applicable) Attempt. A changed action digest requires a new Approval.

```text
ApprovalUse {
  approval_use_id: ApprovalUseId
  approval_id: ApprovalId
  effect_id?: EffectId
  capability_grant_id?: CapabilityGrantId
  request_digest: Sha256Digest
  consumed_at: Timestamp
}
```

Approval consumption is append-only and atomic with the authorized operation admission.
An approval has at most one use; unique `approval_id` prevents replay. Exactly one of
`effect_id` or `capability_grant_id` is present, according to the approved action. The
TrustService recomputes the action binding from the actual target and request, requires
it to match the Approval's `action_digest`, and stores the canonical request digest in
`ApprovalUse`; it never consumes an approval for a merely similar request.

```text
SecretLease {
  secret_lease_id: SecretLeaseId
  secret_ref: SecretRef
  task_id: TaskId
  attempt_id: AttemptId?
  capability_ref: CapabilityRef?
  runtime_id: RuntimeId
  allowed_usage: string[]
  status: ACTIVE | REVOKED | EXPIRED
  issued_at: Timestamp
  expires_at: Timestamp
  version: u64
}
```

## Routine and Automation

```text
Routine {
  routine_id: RoutineId
  workspace_id: WorkspaceId
  name: string
  current_revision: u64
  status: ACTIVE | ARCHIVED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

RoutineRevision {
  routine_id: RoutineId
  revision: u64
  objective_template: string
  instructions: string
  input_schema: JsonSchema
  constraints: string[]
  non_goals: string[]
  required_outputs: OutputRequirement[]
  acceptance_criteria: AcceptanceCriterion[]
  approvals_required: ApprovalRequirement[]
  input_bindings: TaskInputBinding[]
  required_capabilities: CapabilityRequirement[]
  preferred_agent_binding_id: AgentBindingId?
  placement_preference: PlacementPreference
  budget_ceiling: BudgetSpec?
  verification_policy: JsonObject
  authored_by: PrincipalRef
  created_at: Timestamp
}

Automation {
  automation_id: AutomationId
  workspace_id: WorkspaceId
  name: string
  current_revision: u64
  status: ENABLED | PAUSED | DISABLED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

AutomationRevision {
  automation_id: AutomationId
  revision: u64
  routine_id: RoutineId
  routine_revision: u64
  triggers: TriggerSpec[]
  execution_policy: AutomationExecutionPolicy
  authored_by: PrincipalRef
  created_at: Timestamp
}

AutomationOccurrence {
  occurrence_id: OccurrenceId
  automation_id: AutomationId
  automation_revision: u64
  routine_id: RoutineId
  routine_revision: u64
  trigger_id: string
  occurrence_key: string
  trigger_host_runtime_id: RuntimeId
  scheduled_for: Timestamp?
  covered_misfire_range: { from: Timestamp, to: Timestamp, count: u32 }?
  trigger_input_ref: ResourceRef?
  trigger_payload_digest: Sha256Digest?
  task_id: TaskId?
  blockers: Blocker[]
  status: PENDING | CLAIMED | WAITING_DEPENDENCY | STARTED | COMPLETED | SKIPPED | FAILED
  claim_epoch: u64
  claim_expires_at: Timestamp?
  created_at: Timestamp
  updated_at: Timestamp
}

AutomationCursor {
  automation_id: AutomationId
  trigger_id: string
  active_automation_revision: u64
  trigger_host_runtime_id: RuntimeId
  host_epoch: u64
  cursor_digest: Sha256Digest
  last_seen_digest: Sha256Digest?
  last_observation_ref: ResourceRef?
  next_scheduled_at: Timestamp?
  last_checked_at: Timestamp
  observation_gap_since: Timestamp?
  version: u64
}
```

Opaque provider cursors are not fields of the replicated AutomationCursor. They live in
an encrypted Runtime-local `AutomationTriggerBinding`, keyed by the active host epoch;
events and projections carry only a digest of the encrypted cursor binding. Triggers
without opaque cursors digest their canonical trigger checkpoint. A new TriggerHost can
take ownership only after provider-specific cursor transfer/reconciliation or a bounded
rescan.

```text
AutomationTriggerBinding { # Runtime-local operational secret
  automation_id: AutomationId
  trigger_id: string
  trigger_host_runtime_id: RuntimeId
  host_epoch: u64
  cursor_ciphertext: EncryptedBytes
  encryption_key_version: u32
  cursor_digest: Sha256Digest # digest of ciphertext, never of the plaintext cursor
  state: ProviderContinuationBindingStatus
  updated_at: Timestamp
  version: u64
}
```

RoutineRevision and AutomationRevision are immutable. Each occurrence pins both revisions
and one trigger identity. Unique `(automation_id, trigger_id, occurrence_key)` prevents
replay across revision edits; the trigger host and cursor have one active owner in v1.
`WAITING_DEPENDENCY` means a due occurrence already has a visible Task but mandatory
execution dependencies are currently unavailable. Full schemas and trigger behavior are
in `ROUTINES.md` and `AUTOMATION.md`.

## ExecutionLease

```text
ExecutionLease {
  lease_id: ExecutionLeaseId
  task_id: TaskId
  step_id: StepId
  attempt_id: AttemptId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  epoch: u64
  issuer_key_version: u32
  fencing_token_digest: Sha256Digest
  state: ACTIVE | RELEASING | RELEASED | EXPIRED | REVOKED
  checkpoint_ref: ResourceRef?
  acquired_at: Timestamp
  renew_by: Timestamp
  expires_at: Timestamp
  version: u64
}
```

At most one ACTIVE lease per Step. An Attempt and its ExecutionLease are bound to the
same Runtime incarnation. A daemon restart never rebinds an old Attempt or lease to the
new incarnation; after reconciliation, continuation creates a new Attempt and a strictly
higher lease epoch. The raw credential is issued only to the authenticated Runtime/provider
path, never to the Agent or Operator; the durable record and replicated state retain only
its digest.

## Handoff and audit

```text
Handoff {
  handoff_id: HandoffId
  task_id: TaskId
  step_id: StepId
  source_attempt_id: AttemptId
  source_runtime_id: RuntimeId
  target_runtime_id: RuntimeId?
  phase: HandoffPhase
  resume_packet_ref: ResourceRef?
  blockers: string[]
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

AuditRecord {
  audit_record_id: AuditRecordId
  workspace_id: WorkspaceId
  principal: PrincipalRef
  action: string
  resource_ref: ResourceRef?
  decision: ALLOW | DENY | REQUIRE_APPROVAL
  reason_code: string
  correlation_id: CorrelationId
  occurred_at: Timestamp
  payload_digest: Sha256Digest?
}
```

Audit records are append-only and contain no secret bytes. Handoff phase semantics are in
`RUNTIME-MESH.md`; state owners are in `STATE-MACHINES.md`.

## Channel event receipts

```text
ChannelEventReceipt {
  channel_binding_id: ChannelBindingId
  provider_event_id: string
  event_kind: INBOUND | EDIT | DELETE
  payload_digest: Sha256Digest
  conversation_id: ConversationId?
  message_id: MessageId?
  received_at: Timestamp
  claim_epoch: u64
  claim_expires_at: Timestamp?
  state: RECEIVED | PROCESSING | ACCEPTED | REJECTED | FAILED
}
```

Unique `(channel_binding_id, provider_event_id)` prevents duplicate materialization. Each PROCESSING claim increments `claim_epoch`; only the current claim epoch may accept/reject/fail the receipt, so a late worker cannot overwrite a reclaimed receipt.
Raw provider payload is retained only when policy requires it and then as a bounded,
access-controlled Artifact/Resource, not embedded in the event receipt.

## CapabilityInvocation

```text
CapabilityInvocation {
  invocation_id: CapabilityInvocationId
  workspace_id: WorkspaceId
  scope: CapabilityInvocationScope
  agent_session_id: AgentSessionId
  capability_grant_id: CapabilityGrantId
  activation_id: CapabilityActivationId
  capability_ref: CapabilityRef
  operation: string
  request_digest: Sha256Digest
  idempotency_key: string?
  status: CapabilityInvocationStatus
  provider_task_status: ProviderTaskStatus?
  provider_task_created_at: Timestamp?
  provider_task_expires_at: Timestamp?
  provider_task_ttl_ms: u64?
  provider_poll_after_ms: u64?
  provider_updated_at: Timestamp?
  partial_result_refs: ResourceRef[]
  result_refs: ResourceRef[]
  effect_id: EffectId?
  failure: FailureRecord?
  created_at: Timestamp
  updated_at: Timestamp
  completed_at: Timestamp?
  version: u64
}
```

Provider-specific task handles and cursors are not fields of this replicated aggregate.
They are held only in encrypted Runtime-local `CapabilityInvocationProviderBinding`
records. Treat every opaque provider task ID as potentially bearer-authorizing. The Runtime
uses its current provider/Connection authorization for each operation; a Runtime-local
binding is never exposed by Operator API, DomainEvent, aggregate-state blob, Mesh sync, or
Workspace backup. If it is unavailable after recovery, reconcile by a provider-supported
lookup or mark the Invocation `AMBIGUOUS`; never reconstruct a handle from a digest.

```text
CapabilityInvocationProviderBinding { # Runtime-local operational secret
  invocation_id: CapabilityInvocationId
  runtime_id: RuntimeId
  last_runtime_incarnation_id: RuntimeIncarnationId
  binding_ciphertext: EncryptedBytes # provider task ref/cursor, versioned provider codec
  encryption_key_version: u32
  binding_digest: Sha256Digest # digest of ciphertext, never of the plaintext handle/cursor
  state: ProviderContinuationBindingStatus
  observed_at: Timestamp
  version: u64
}
```

The encrypted binding is persisted only in the owning Runtime's local StateStore, with its
key held outside the database. Backup restore without that key marks the binding unavailable
and triggers provider reconciliation; a digest is an integrity check, not a recovery token.

Read-only and long-running operations have durable Invocation state without an Effect.
An Invocation may be Conversation-scoped only for a read-only operation; Task/Attempt
scope is required for consequential operations. See `CAPABILITY-INVOCATIONS.md`.

## UserRequest and response

```text
UserRequest {
  request_id: UserRequestId
  workspace_id: WorkspaceId
  conversation_id: ConversationId?
  conversation_turn_id: ConversationTurnId? # required exactly for Conversation scope
  task_id: TaskId?
  attempt_id: AttemptId?
  agent_session_id: AgentSessionId
  invocation_id: CapabilityInvocationId?
  kind: QUESTION | DECISION | RESOURCE_SELECTION | EXTERNAL_AUTHORIZATION
  interaction_mode: FORM | EXTERNAL_URL
  prompt: string
  response_schema: JsonSchema?
  choices: Choice[]
  status: PENDING | ANSWERED | DISMISSED | EXPIRED | CANCELLED
  expires_at: Timestamp?
  created_at: Timestamp
  resolved_at: Timestamp?
  resolved_by: PrincipalRef?
  response_digest: Sha256Digest?
  version: u64
}
```

`EXTERNAL_AUTHORIZATION` is Runtime-materialized only for an explicitly supported
provider URL handoff; it is never an Approval and cannot authorize an Effect or grant.
`EXTERNAL_URL` requires this kind, a null `response_schema`, and no form choices. Its raw
URL, query, fragment, and provider message remain in the encrypted Runtime-local
ProviderInputBinding. The replicated/API UserRequest contains a host-authored safe summary
and no URL. A `FORM` response is non-sensitive workspace input. An `EXTERNAL_URL` response
is exactly `{action: "accept" | "decline" | "cancel"}` and contains no submitted values.
`accept` means only that the user completed/confirmed the external interaction from the UI;
provider authentication remains unconfirmed until the provider task is reconciled.

Exactly one scope tuple is present and it must match the originating AgentSession:
Conversation scope has `conversation_id` and `conversation_turn_id`; planning scope has
`task_id` only; Attempt scope has both `task_id` and `attempt_id`. A ConversationTurn must
belong to that Conversation and name the originating session as its current/most-recent
session. A linked CapabilityInvocation has the same parent scope and references that exact
AgentSession; the Invocation's Conversation scope intentionally does not duplicate the
turn ID. Provider input keys are not
part of UserRequest, DomainEvent, or Operator API; an encrypted Runtime-local
`ProviderInputBinding` maps a provider request key to the UserRequest and Invocation.
Approval remains its own Trust entity and is not a UserRequest kind; the Needs You
projection may show both records.

```text
ProviderInputBinding { # Runtime-local encrypted secret/outbox
  runtime_id: RuntimeId
  invocation_id: CapabilityInvocationId
  request_id: UserRequestId
  provider_input_key_ciphertext: EncryptedBytes
  provider_input_payload_ciphertext: EncryptedBytes # raw method/params, including any state-bearing URL
  encryption_key_version: u32
  provider_input_key_tag: Sha256Digest # Runtime-keyed tag for local deduplication only
  input_request_digest: Sha256Digest
  response_id: UserRequestResponseId?
  response_digest: Sha256Digest?
  delivery_status: ProviderInputDeliveryStatus
  dispatch_count: u32
  last_dispatch_at: Timestamp?
  last_provider_observation_at: Timestamp?
  retry_safety_proof_digest: Sha256Digest? # local authenticated proof for one exact duplicate-safe retry
  failure_code: ErrorCode?
  updated_at: Timestamp
  version: u64
}
```

`(invocation_id, provider_input_key_tag)` and `request_id` are unique in the owning
Runtime's private store. The immutable UserRequestResponse and a PENDING binding are
committed together locally after the replicated response is received. A successful
`tasks/update` acknowledgement is only `ACKNOWLEDGED`; the binding becomes `ACCEPTED` only
after a subsequent authenticated provider observation shows that the input key is no
longer outstanding. If dispatch outcome is uncertain, reconcile `tasks/get`; resend only
when the provider contract and the exact key/digest make the same response duplicate-safe.
Before that retry, persist a Runtime-local `retry_safety_proof_digest` over the authenticated
observation and negotiated adapter contract; the dispatch guard requires it and increments
`dispatch_count`. Clear the proof when a later dispatch becomes ambiguous. Otherwise retain
`AMBIGUOUS` and do not replay it. If the UserRequest expires before a
response, the binding becomes `EXPIRED`; that closes only this user-response path. It does
not prove that the provider task stopped waiting, so the InvocationRunner cancels or
reconciles the provider operation before releasing the ConversationTurn/Task dependency.
On Task cancellation, an `AWAITING_RESPONSE` or `PENDING` binding is atomically withdrawn
as `CANCELLED`; a PENDING response remains immutable history but is never sent. A
`DISPATCHED`, `ACKNOWLEDGED`, or `AMBIGUOUS` binding is not rewritten to `CANCELLED`:
InvocationRunner reconciles delivery and the provider operation before the Task can settle.
After Task resume, a queued PENDING response is delivered only after TaskService has
revalidated the Task, pinned capability/Grant and provider continuation, and confirmed the
operation remains eligible. A planning-scoped Invocation requires a fresh active planner
for the same current lead and TaskSpec revision; an Attempt-scoped Invocation requires its
same source Attempt to be current again under a new lease epoch on the same Runtime
incarnation and Environment. If a new Attempt/Runtime is needed, the old provider task is
reconciled/cancelled first and the saved response can be used only as context for new
authorized work. If any check fails, it stays queued and the Task exposes a blocker; a
stale approval or lease is never reused.
The ConversationTurn remains `WAITING_DEPENDENCY` until the provider operation is
quiescent, then settles retryably as `FAILED` with `USER_REQUEST_EXPIRED`. For a
Task/Attempt-scoped request, the affected Step is `BLOCKED`, while independent Steps may
continue; the Task is `NEEDS_USER` only when no independent progress remains. A late user
response is rejected with `USER_REQUEST_EXPIRED` and cannot revive the old provider key.

The user response is a separate immutable `UserRequestResponse` record:

```text
UserRequestResponse {
  response_id: UserRequestResponseId
  request_id: UserRequestId
  response: JsonValue
  response_digest: Sha256Digest
  responded_by: PrincipalRef
  responded_at: Timestamp
}
```

A free-text answer is not an Approval unless TrustService separately authorizes it as
one.

## Usage and budget

```text
UsageObservation {
  usage_observation_id: UsageObservationId
  workspace_id: WorkspaceId
  task_id: TaskId?
  environment_id: EnvironmentId?
  attempt_id: AttemptId?
  invocation_id: CapabilityInvocationId?
  agent_session_id: AgentSessionId?
  source: AGENT_REPORTED | PROVIDER_REPORTED | HOST_MEASURED
  metric: TOKEN_INPUT | TOKEN_OUTPUT | WALL_TIME | COST | BYTES_READ | BYTES_WRITTEN | CALL_COUNT
  quantity: decimal?   # null exactly when confidence = UNKNOWN; never encode unknown as 0
  unit: string
  currency?: string
  confidence: EXACT | ESTIMATED | UNKNOWN
  observed_at: Timestamp
  source_ref?: string
}

BudgetReservation {
  reservation_id: BudgetReservationId
  workspace_id: WorkspaceId
  budget_scope: TASK | ENVIRONMENT
  task_id: TaskId?       # required for TASK scope; forbidden for ENVIRONMENT scope
  environment_id: EnvironmentId? # required for ENVIRONMENT scope; forbidden for TASK scope
  attempt_id: AttemptId?
  metric: string
  quantity: decimal
  unit: string
  currency?: string
  state: RESERVED | COMMITTED | RELEASED | EXPIRED
  created_at: Timestamp
  expires_at: Timestamp?
}
```

BudgetService enforces only host-observable/reservable ceilings. Unknown native-agent
usage remains unknown and is never treated as zero. Usage observations may attribute one
measurement to both its using Task and persistent Environment; enforcement creates
separate reservations for the respective budget owner, never one ambiguous shared
reservation. Persistent Environment cost and wall-time ceilings are cumulative from
provisioning and have no implicit periodic reset. Provider-enforced budgets remain active
while LiteCowork is disconnected; host-monitored budgets become `UNKNOWN` while the
Runtime/provider cannot report usage and may overshoot during that gap.

## Notifications

```text
NotificationPreference {
  workspace_id: WorkspaceId
  event_class: string
  policy: ALWAYS | ON_SUCCESS | ON_FAILURE | ON_CONDITION | SILENT
  preferred_channels: ChannelBindingId[]
  quiet_hours?: QuietHours
  version: u64
}

NotificationDelivery {
  delivery_id: DeliveryId
  workspace_id: WorkspaceId
  dedupe_key: string
  source_event_id: EventId
  channel_binding_id: ChannelBindingId?
  status: PENDING | SENDING | SENT | FAILED | SUPPRESSED
  attempt_count: u32
  next_attempt_at?: Timestamp
  last_error_code?: ErrorCode
  created_at: Timestamp
  settled_at?: Timestamp
}
```

Sent means the channel acknowledged delivery; it never means the related Task completed.

## SkillProposal

```text
SkillProposal {
  skill_proposal_id: SkillProposalId
  workspace_id: WorkspaceId
  source_task_id: TaskId
  source_artifact_id: ArtifactId
  draft_resource_ref: ResourceRef
  draft_digest: Sha256Digest
  redaction_status: PENDING | PASSED | FAILED
  status: DRAFT | REVIEW | APPROVED | REJECTED | PUBLISHED
  approved_by?: PrincipalRef
  published_capability_ref?: CapabilityRef
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}
```

LiteCowork owns draft creation, secret/task-specific data redaction, verification, and
user approval. LitePSM owns publication/package lifecycle; its contract is intentionally
not defined here.

## Resource identity and location

```text
Resource {
  resource_id: ResourceId
  workspace_id: WorkspaceId
  kind: FILE | FOLDER | ARTIFACT | CONNECTOR_OBJECT | WEB_RESOURCE | OTHER
  provider_identity: ProviderIdentity
  identity_digest: Sha256Digest?  # present only for a verified identity eligible for deduplication
  display_name: string
  current_revision_id: ResourceRevisionId?
  sensitivity: SensitivityClass
  provenance: ProvenanceRecord
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

ProviderIdentity {
  provider_instance_id: string
  stable_object_id: string # stable opaque pseudonym, never a raw path or credential
  identity_confidence: STRONG | PROVIDER_SCOPED | WEAK
  file_identity: FileIdentity?
}

FileIdentity {
  filesystem_instance_id: string # Runtime-keyed pseudonym
  volume_id: string? # Runtime-keyed pseudonym
  file_id: string # Runtime-keyed pseudonym
  generation: string? # Runtime-keyed pseudonym
  platform_kind: string
}

FileIdentityBinding { # Runtime-local raw operating-system identity
  location_id: ResourceLocationId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  raw_filesystem_instance_id: string
  raw_volume_id: string?
  raw_file_id: string
  raw_generation: string?
  platform_kind: string
  observed_at: Timestamp
}

ResourceRevision {
  resource_revision_id: ResourceRevisionId
  resource_id: ResourceId
  parent_revision_ids: ResourceRevisionId[]
  provider_revision: string?
  content_digest: Sha256Digest?
  size_bytes: u64?
  media_type: string?
  observed_at: Timestamp
  created_by: PrincipalRef | ServiceRef
}

ResourceLocation {
  location_id: ResourceLocationId
  resource_id: ResourceId
  runtime_id: RuntimeId?
  environment_id: EnvironmentId?
  connection_id: ConnectionId?
  provider_ref: string?
  locator_ref_id: string # stable non-secret resolver key; never a path, URL, or provider handle
  availability: ResourceLocationAvailability
  writable: bool
  observed_revision_id: ResourceRevisionId?
  observed_digest: Sha256Digest?
  observed_at: Timestamp
  last_checked_at: Timestamp?
}

ResourceLocationBinding { # Runtime-local operational relation
  location_id: ResourceLocationId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  locator_ref_id: string
  private_locator: string # raw path/provider locator/tab handle; local only
  observed_at: Timestamp
  expires_at: Timestamp?
}

WorkspaceRoot {
  workspace_root_id: WorkspaceRootId
  workspace_id: WorkspaceId
  resource_id: ResourceId
  location_id: ResourceLocationId
  display_name: string
  watch_policy: METADATA | CONTENT_DIGESTS | SELECTED_TEXT_EXTRACTION
  replication_policy: NONE | ACTIVE_TASKS | SELECTED_WORKSPACE_POLICY
  status: WorkspaceRootStatus
  added_by: PrincipalRef
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

ResourceEdge {
  edge_id: ResourceEdgeId
  workspace_id: WorkspaceId
  from_resource_id: ResourceId
  from_revision_id: ResourceRevisionId?
  relation: CONTAINS | DERIVED_FROM | REFERENCES | SAME_PROVIDER_OBJECT
  to_resource_id: ResourceId
  to_revision_id: ResourceRevisionId?
  observed_at: Timestamp
  provenance: ProvenanceRecord
}

DependencyEdge {
  dependency_edge_id: DependencyEdgeId
  workspace_id: WorkspaceId
  source_resource_id: ResourceId
  source_revision_id: ResourceRevisionId
  dependent_kind: ARTIFACT_VERSION | VERIFICATION_RUN
  dependent_ref: InvalidationDependentRef
  created_at: Timestamp
}

InvalidationRecord {
  invalidation_record_id: InvalidationRecordId
  dependency_edge_id: DependencyEdgeId
  observed_revision_id: ResourceRevisionId
  reason_code: string
  created_at: Timestamp
}

ResourceUploadSession {
  upload_id: ResourceUploadId
  workspace_id: WorkspaceId
  display_name: string
  media_type: string
  expected_size_bytes: u64
  expected_digest: Sha256Digest?
  chunk_size_bytes: u64
  state: ResourceUploadState
  received_ranges: UploadRange[]  # derived from immutable ResourceUploadChunk rows
  next_missing_offset: u64        # derived progress projection
  expires_at: Timestamp
  resource_id: ResourceId?
  created_at: Timestamp
  version: u64
}

UploadRange {
  start_offset: u64
  end_offset_inclusive: u64
  sha256: Sha256Digest
}

ResourceUploadChunk {
  upload_id: ResourceUploadId
  chunk_index: u64
  start_offset: u64
  end_offset_exclusive: u64
  sha256: Sha256Digest
  temporary_blob_ref: string
  received_at: Timestamp
}

InvalidationDependentRef =
  `artifact://<workspace-id>/<artifact-id>@v<version>`
  | `verification://<verification-run-id>`
```

An InvalidationRecord is unique for `(dependency_edge_id, observed_revision_id)`.
`observed_revision_id` is the newly observed revision that invalidated the exact older
revision pinned by the DependencyEdge. DependencyService processing is idempotent for
duplicate observation events; a later source revision creates a distinct record.

`DependencyEdge` is a normalized, immutable, rebuildable reverse-index projection of
`ArtifactVersion.input_refs` and each `VerificationRun.inputs[].resource_ref`. Each input
ResourceRef must pin a ResourceRevision. `ResourceEdge` remains for observed structural/provenance
relationships; it is not the dependency index. On ArtifactVersion or VerificationRun
creation, the owning transaction persists its input refs and derives one DependencyEdge
per input before the aggregate event is acknowledged. The event/aggregate state retains
the authoritative input refs so the index can be rebuilt. An edge points to the exact
revision consumed; an InvalidationRecord points to that edge and the newly observed source
revision that made that dependent stale. Freshness is a projection and never mutates immutable
ArtifactVersion, VerificationRun, or Evidence records.

Root replication settings restrict Workspace replication: `NONE` denies transfer of that
root, `ACTIVE_TASKS` permits only Task-required transfer when the Workspace policy allows
it, and `SELECTED_WORKSPACE_POLICY` inherits the Workspace policy. No root setting can
broaden the Workspace policy. `SELECTED_FOLDERS` follows the active selected root identity
across its newly observed revisions; it does not freeze one content revision.

`identity_digest` is a canonical provider-identity key, not a content digest. It is present
only when the provider supplies a verified identity tuple with `STRONG` or
`PROVIDER_SCOPED` confidence, and is unique within a Workspace. A weak identity has no
digest eligible for cross-location deduplication; path/size/mtime similarity cannot merge
logical Resources. Such observations remain separate Resources until stronger evidence
establishes identity. When present, the digest is `sha256:` followed by 64 lowercase
hexadecimal characters.

`current_revision_id` points only to the unique head of the immutable ResourceRevision
ancestry graph. It is null when no revision is known or when multiple incomparable heads
exist; the service derives the head set from the graph. Each ResourceRevision parent must
belong to the same Resource, and the graph must be acyclic. A normal observation extends
the last revision observed at that location; a provider that supplies verified ancestry
may report its actual parent set. Independent edits from a common parent remain sibling
heads and are never resolved by timestamp, Runtime priority, or digest ordering.

A pinned ResourceRef may resolve to one branch while a Resource is conflicted. An
unpinned ResourceRef returns `RESOURCE_CONFLICT` until a single head is established.
Creating a genuine merged revision requires a provider-observed or user-produced output
whose ancestry names all merged heads; choosing one branch does not erase the others.

Freshness is a derived projection for a requested revision and candidate location; it is
not one global Resource field because replicas may differ. `WORLD-RESOURCES.md` defines
observation, indexing, search, resolution, race defenses, and invalidation behavior.
ResourceRef uses stable identity (`resource://<workspace-id>/<resource-id>@<revision-id?>`);
filesystem paths, Runtime IDs, connector handles, and browser tabs are location/provider
details. `ResourceSearchResult` is a projection, not a durable entity.

## Event

Canonical event envelope is specified in `EVENTS.md`.

## ResourceRef

```text
ResourceRef =
  workspace_id: WorkspaceId
  resource_id: ResourceId
  revision_id?: ResourceRevisionId
```

The display form is `resource://<workspace-id>/<resource-id>@<revision-id?>`. A
ResourceRef names logical identity, not a path, Runtime, Environment, provider handle, or
storage blob. Those locators are kept in ResourceLocation or provider-owned records.
Artifact content uses `ArtifactContent`; secret authority uses SecretRef/SecretLease;
neither is encoded as an arbitrary ResourceRef URI. A reference is revision-pinned when
correctness depends on exact content identity.

`PinnedResourceRef` is a `ResourceRef` with `revision_id` required. ArtifactVersion input
refs, VerificationRun subjects/inputs, and external ArtifactContent references use this
form because provenance, verification, and staleness depend on an exact revision.

## Cross-entity invariants

- Every TaskSpecRevision belongs to one Task and has a unique monotonic revision.
- Every Task pins the WorkspaceInstructionRevision used at creation; a later Workspace instruction edit does not silently change existing Task context.
- Every PlanRevision names the TaskSpecRevision it planned against and an authorized producing AgentSession pinned to that same revision. A Task-scoped TASK_PLANNING session has no Attempt; an execution-produced plan also records the producing Attempt. A plan never mutates after publication.
- A Step belongs to one Task and one PlanRevision. Replanning retires or supersedes
  affected Steps; it does not rewrite completed Attempt history.
- An Attempt belongs to one Step and has one selected Runtime and Environment. Replacing
  the worker or execution location creates a new Attempt unless the same active lease
  and existing Attempt are safely resumed.
- A placement override selects only an eligible candidate from the exact unexpired
  ExecutionDependencyPlan digest; the created Attempt pins the candidate's Runtime and
  Environment, while older Attempt bindings remain immutable.
- Every AgentSession has exactly one scope variant and only the authority allowed by that variant. A Conversation session binds one exact ConversationTurn; a Task-scoped session pins the TaskSpecRevision used to build its context. It stores durable Runtime/incarnation provenance; its native session handle and host binding live only in an immutable Runtime-local `AgentSessionHostBinding`.
- Every Attempt pins the Runtime incarnation selected at admission. Its Environment and ExecutionLease must belong to the same Runtime/incarnation. A restarted daemon cannot resume the old Attempt under its prior lease; it reconciles and, if work is safe to continue, creates a new Attempt and lease epoch.
- Durable ExecutionLease and EnvironmentControlLease records contain only fencing-credential digests plus a non-secret issuer-key version. HMAC-derived raw credentials are held only in Runtime/provider process memory, absent from event payloads, aggregate-state blobs, Operator projections, logs, and Workspace backups; every request also authenticates its caller and rechecks current lease ownership/epoch/expiry.
- A peer or Workspace backup never receives an AgentSession native handle or host binding. A replacement Runtime creates a new AgentSession after fresh admission instead of reviving a foreign local handle.
- Every CapabilityInvocation's Workspace/scope tuple, AgentSession, active CapabilityGrant, and CapabilityActivation must agree at admission. Its parent must also be live: a Conversation invocation belongs to the current RUNNING turn and its current session; a planning invocation belongs to a RUNNING Task, current lead binding, and current TaskSpecRevision; an execution invocation belongs to the exact RUNNING Attempt, its pinned PlanRevision's TaskSpecRevision, and its current unexpired ACTIVE ExecutionLease. Activation scope, Runtime/incarnation, and exact normalized CapabilityRef match the AgentSession, while Grant and Invocation match the same capability and scope.
- A managed CapabilityActivation has one origin-Runtime-local HostBinding while it uses a provider; the binding must match its Runtime incarnation, exact CapabilityRef, and scope. Bindings do not replicate or enter Workspace backups; per-host use counts derive from bindings joined to nonterminal Activation rows, and sharing never combines Grants or authority.
- AgentEndpoint command/socket/URL locators, Environment provider locators, ResourceLocation private locators, and checkpoint handles exist only in Runtime-local bindings keyed to the exact Runtime incarnation; peers and Workspace backups receive neither handles nor raw locator material. Durable ResourceLocations contain only a stable non-secret resolver key. Provider-only checkpoint digests are not treated as portable state. A checkpoint's optional `portable_snapshot_ref` is the explicit content-addressed exception; copying its bytes still requires both the Environment backup policy and Workspace replication policy to allow it.
- Every ResourceLocation locator key is non-secret and stable for that location; its raw filesystem/provider locator exists only in a `ResourceLocationBinding` on the Runtime that can resolve it. That binding must match the current Runtime incarnation and durable location key.
- `AgentEndpoint` is stable protocol/topology identity; its executable command, socket, or URL is available only through a current Runtime-local `AgentEndpointBinding`. RuntimeOffers advertise endpoint readiness without exposing the locator.
- FileIdentity tokens are keyed pseudonyms using a stable local Runtime identity key; raw platform filesystem/volume/file identifiers exist only in Runtime-local `FileIdentityBinding` rows. The key and raw identifiers never enter Workspace backups or replication. A Runtime key loss lowers identity confidence and triggers bounded re-indexing; content equality alone does not restore identity.
- Every UserRequest scope tuple equals its originating AgentSession. When linked to a CapabilityInvocation, its Conversation/Task/Attempt parent scope matches and the Invocation references the exact originating AgentSession; the UserRequest's Conversation turn ID equals that session's bound turn. Provider input keys exist only in encrypted Runtime-local bindings and are unique within their Invocation.
- Every Conversation-scoped UserRequest names exactly one ConversationTurn in the same Conversation, and that turn's current/most-recent AgentSession equals the request origin; only that turn may resume from its response.
- Conversation- and Task-planning-scoped CapabilityInvocations are read-only and have no Effect; only an Attempt-scoped Invocation may represent a consequential operation.
- Every VerificationRun pins the TaskSpec revision, criterion digest, verifier version, revision-pinned subject refs, and paired `ResourceInput` values; changed requirements or inputs require new verification.
- Every DependencyEdge corresponds to exactly one revision-pinned input ref on its dependent ArtifactVersion or VerificationRun; the reverse index is rebuildable from aggregate state/events.
- Every InvalidationRecord names one DependencyEdge and a newly observed revision of the same source Resource; duplicate edge/revision invalidations are idempotently rejected.
- Every consumed one-time Approval has exactly one immutable ApprovalUse with a unique ApprovalId.
- A provisioned Environment's budget ceiling and enforcement policy are immutable in v1; a limit increase requires a distinct Environment and a fresh Attempt, never mutation of an existing Attempt's Environment binding.
- A CapabilityInvocation is durable independently of any linked Effect or AgentSession process.
- A CapabilityGrant is narrower than or equal to the permissions requested for its
  CapabilityRef and is bound to exactly one Conversation, Task-planning session, or
  Attempt scope. Conversation/planning grants are read-only, secret-free, and expiring;
  Attempt grants may allow mutations only under that Attempt's policy and lease.
- Every CapabilityInvocation records the exact CapabilityGrant that authorized it; the
  Invocation scope must equal or be narrower than the grant scope.
- Conversation-scoped AgentSessions and Invocations have no Task/Attempt authority; a
  Conversation-scoped CapabilityGrant is bounded to that Conversation and a read-only
  operation/resource scope.
- A managed ArtifactVersion points only to committed content whose digest has been verified; an external ArtifactVersion pins a Resource/revision and records any digest the provider supplies.
- Evidence and AuditRecord are append-only. Corrections append new records.
- AutomationOccurrence pins immutable AutomationRevision and RoutineRevision records plus a stable trigger ID; its occurrence key is independent of definition revision.
- An archived Workspace has no nonterminal Tasks, unsettled Conversation turns/Invocations, enabled Automations, active grants/SecretLeases/control leases, or live persistent Environment workload. Watchers are stopped; retained environments may remain suspended. It admits no new Task or inbound channel work. Archived Routines cannot be selected for new Tasks or Automations.
- Resource identity is stable across locations; unavailable locations do not imply a Resource is absent, and stale source revisions invalidate downstream evidence without mutating its history.
- A Task cannot be completed while a mandatory criterion lacks its required evidence,
  an Approval is unresolved, a required child Attempt is active, or an Effect remains
  ambiguous.
- Deleting or archiving a user-visible entity never erases evidence needed for audit,
  effect reconciliation, or an in-progress replication cursor.

- Environment Runtime and owner Task (when present) belong to `owner_workspace_id`.
  ATTEMPT/TASK_RETAINED require an owner Task; WORKSPACE_PERSISTENT has no owner Task.
  WORKSPACE_PERSISTENT requires exactly one consumed provision-preview digest and a
  recorded user budget-enforcement policy/result. Persistent provision authority is
  separate from each later Task/Attempt's use authority. A BudgetReservation has exactly
  one budget owner; one use may create independent Task and Environment
  reservations, never a combined reservation with ambiguous enforcement.
