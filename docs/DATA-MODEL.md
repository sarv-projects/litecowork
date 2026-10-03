# Data Model

IDs are opaque, globally unique and stable. Durable records carry the creation or receipt time relevant to their lifecycle. Mutable aggregates carry an optimistic `version`; `updated_at` appears where it belongs to that aggregate's update contract. Immutable records have no update API. Timestamps are RFC 3339 UTC. Canonical entities and relationships are defined here; shared enums are in `SCHEMAS.md`, transitions in `STATE-MACHINES.md`, and SQL representation in `schemas/sqlite-v1.sql`.

## Workspace

```text
Workspace {
  workspace_id: WorkspaceId
  name: string
  owner_principal_id: PrincipalId
  replication_policy: ReplicationPolicy
  replication_scope_refs: ResourceRef[]
  hub_runtime_id: RuntimeId?
  status: ACTIVE | ARCHIVED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

ReplicationPolicy = LOCAL_ONLY | METADATA_ONLY | ACTIVE_TASK_INPUTS |
  SELECTED_FOLDERS | FULL_WORKSPACE
```

The default for a local-only Workspace is `LOCAL_ONLY`. When the user enables cloud for a personal Workspace, the default is `ACTIVE_TASK_INPUTS`. `SELECTED_FOLDERS` requires one or more revision-pinned folder ResourceRefs. A policy selects which resources may replicate; it never authorizes a capability or secret. A policy change applies prospectively and does not erase content already copied to another Runtime. Archiving is allowed only after Tasks are terminal and Automations are disabled; an archived Workspace is read-only; existing authorized Artifact/Resource reads remain available, while all domain mutations—including Task materialization, Artifact/Library changes, connection changes, and inbound channel work—are rejected.

## Conversation

```text
Conversation {
  conversation_id: ConversationId
  workspace_id: WorkspaceId
  title: string?
  created_at: Timestamp
  version: u64
}

ConversationMessage {
  message_id: MessageId
  conversation_id: ConversationId
  author: PrincipalRef
  role: USER | AGENT | SYSTEM_NOTICE | CHANNEL
  content: MessageContent[]
  resource_refs: ResourceRef[]
  source_channel_ref: ChannelThreadRef?
  created_at: Timestamp
}
```

Messages are append-only; edits create a replacement/revision event rather than destructive rewrite of history.

## Task

```text
Task {
  task_id: TaskId
  workspace_id: WorkspaceId
  conversation_id: ConversationId?
  current_spec_revision: u64
  current_plan_revision: u64?
  status: TaskStatus
  lead_agent_binding_id: AgentBindingId?
  priority: LOW | NORMAL | HIGH
  created_by: PrincipalRef
  created_at: Timestamp
  updated_at: Timestamp
  completed_at: Timestamp?
  version: u64
}
```

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
  input_refs: ResourceRef[]
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

## Agent records

```text
AgentProfile {
  agent_profile_id: AgentProfileId
  provider_key: string
  display_name: string
  adapter_kind: ACP | A2A | SDK | CLI | TERMINAL
  capabilities: AgentCapabilities
  discovered_at: Timestamp
}

AgentBinding {
  agent_binding_id: AgentBindingId
  workspace_id: WorkspaceId
  agent_profile_id: AgentProfileId
  runtime_id: RuntimeId?
  auth_ref: SecretRef?
  configuration: JsonObject
  enabled: bool
  created_at: Timestamp
  version: u64
}

AgentSession {
  agent_session_id: AgentSessionId
  task_id: TaskId
  agent_binding_id: AgentBindingId
  session_kind: LEAD_PLANNING | STEP_EXECUTION
  attempt_id: AttemptId?
  native_session_ref: string?
  status: AgentSessionStatus
  started_at: Timestamp
  last_event_at: Timestamp?
  closed_at: Timestamp?
  version: u64
}
```

A `LEAD_PLANNING` AgentSession is Task-scoped and has no Attempt; it may propose the initial PlanRevision before Steps or execution Attempts exist. It may only clarify intent, read the Task packet, and propose a plan; it has no Environment write access and cannot invoke consequential capabilities or publish artifacts. A `STEP_EXECUTION` AgentSession is bound to exactly one Attempt. A plan proposed during execution records both its producing session and Attempt. Only TaskService may promote an authorized proposal to the current PlanRevision.

## Runtime

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
  resource_capacity: ResourceCapacity
  last_seen: Timestamp
  version: u64
}
```

Runtime advertised inventory is maintained separately as offers so it can expire without rewriting runtime identity.

```text
RuntimeOffer {
  runtime_id: RuntimeId
  offer_kind: AGENT | CAPABILITY | ENVIRONMENT | CHANNEL | TRIGGER
  offer_ref: string
  compatible: boolean
  constraints: JsonObject
  observed_at: Timestamp
  expires_at: Timestamp
}
```

## Environment

```text
Environment {
  environment_id: EnvironmentId
  runtime_id: RuntimeId
  provider_kind: string
  class: LOCAL_WORKSPACE | GIT_WORKTREE | CONTAINER | VM | CLOUD_SANDBOX | REMOTE_MACHINE | BROWSER | DESKTOP
  status: EnvironmentStatus
  locator: JsonObject
  isolation: IsolationSpec
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

EnvironmentCheckpoint {
  checkpoint_id: EnvironmentCheckpointId
  environment_id: EnvironmentId
  provider_ref: string
  digest: string?
  created_at: Timestamp
}
```

## Capability records

```text
CapabilityRef {
  capability_id: string
  source: string
  package_version: string
  digest: string
  component: string?
}

CapabilityLock {
  task_id: TaskId
  capability_ref: CapabilityRef
  locked_at: Timestamp
}

CapabilityGrant {
  capability_grant_id: CapabilityGrantId
  task_id: TaskId
  attempt_id: AttemptId?
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

CapabilityActivation {
  activation_id: CapabilityActivationId
  capability_ref: CapabilityRef
  runtime_id: RuntimeId
  mode: DIRECT_MCP | GATEWAY_PROXY | NATIVE_AGENT | REMOTE_PROVIDER
  provider_handle: JsonObject
  status: CapabilityActivationStatus
  health: HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

CapabilityOffer {
  runtime_id: RuntimeId
  capability_ref: CapabilityRef
  compatible: bool
  constraints: JsonObject
  observed_at: Timestamp
  expires_at: Timestamp
}
```

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

## Artifact

```text
Artifact {
  artifact_id: ArtifactId
  workspace_id: WorkspaceId
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
  created_by_attempt: AttemptId?
  input_refs: ResourceRef[]
  content_digest: string
  storage_ref: BlobRef
  media_type: string
  size_bytes: u64
  provenance: ProvenanceRecord
  verification_refs: EvidenceId[]
  created_at: Timestamp
}
```

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
  request_digest: string
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
  payload_digest: string?
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
  verifier_kind: string
  subject_refs: ResourceRef[]
  status: PENDING | RUNNING | PASSED | FAILED | INCONCLUSIVE
  evidence_refs: EvidenceId[]
  started_at: Timestamp?
  completed_at: Timestamp?
}
```

## Approval

```text
Approval {
  approval_id: ApprovalId
  task_id: TaskId
  requested_by_attempt: AttemptId?
  kind: string
  action_summary: string
  target_ref: ResourceRef
  scope_digest: string
  action_digest: string
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

## Automation

```text
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
  trigger: TriggerSpec
  task_template: TaskTemplate
  execution_policy: AutomationExecutionPolicy
  authored_by: PrincipalRef
  created_at: Timestamp
}

AutomationOccurrence {
  occurrence_id: OccurrenceId
  automation_id: AutomationId
  automation_revision: u64
  occurrence_key: string
  scheduled_for: Timestamp?
  trigger_input_ref: ResourceRef?
  trigger_payload_digest: string?
  task_id: TaskId?
  status: PENDING | CLAIMED | STARTED | COMPLETED | SKIPPED | FAILED
  claim_epoch: u64
  claim_expires_at: Timestamp?
  created_at: Timestamp
  updated_at: Timestamp
}
```

Unique: `(automation_id, occurrence_key)` prevents duplicate logical occurrences across retries and AutomationRevision changes. Each occurrence pins the immutable revision that created it. Every claim increments `claim_epoch`; only the current epoch may materialize or settle the occurrence, fencing a worker whose claim expired.

## ExecutionLease

```text
ExecutionLease {
  lease_id: ExecutionLeaseId
  task_id: TaskId
  step_id: StepId
  attempt_id: AttemptId
  runtime_id: RuntimeId
  epoch: u64
  fencing_token: string
  state: ACTIVE | RELEASING | RELEASED | EXPIRED | REVOKED
  checkpoint_ref: ResourceRef?
  acquired_at: Timestamp
  renew_by: Timestamp
  expires_at: Timestamp
  version: u64
}
```

At most one ACTIVE lease per Step.

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
  payload_digest: string?
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
  payload_digest: string
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

## Event

Canonical event envelope is specified in `EVENTS.md`.

## ResourceRef

```text
ResourceRef =
  artifact://<artifact>/<version?>
  workspace-file://<runtime>/<file>/<version>
  git://<repo>/<commit>/<path>
  connector://<provider>/<resource>/<revision?>
  browser://<runtime>/<session>/<tab>
  secret://<vault>/<ref>
  workspace-folder://<runtime>/<folder-id>/<revision?>
  blob://<digest>
```

Resource references are immutable identifiers or revision-pinned locators whenever correctness depends on content identity.

## Cross-entity invariants

- Every TaskSpecRevision belongs to one Task and has a unique monotonic revision.
- Every PlanRevision names the TaskSpecRevision it planned against and an authorized producing AgentSession. A Task-scoped LEAD_PLANNING session has no Attempt; an execution-produced plan also records the producing Attempt. A plan never mutates after publication.
- A Step belongs to one Task and one PlanRevision. Replanning retires or supersedes
  affected Steps; it does not rewrite completed Attempt history.
- An Attempt belongs to one Step and has one selected Runtime and Environment. Replacing
  the worker or execution location creates a new Attempt unless the same active lease
  and existing Attempt are safely resumed.
- A CapabilityGrant is narrower than or equal to the permissions requested for its
  CapabilityRef and is bound to a Task, optional Attempt, operation set, and resource
  scope.
- An ArtifactVersion points only to committed content whose digest has been verified.
- Evidence and AuditRecord are append-only. Corrections append new records.
- AutomationOccurrence pins an immutable AutomationRevision; its occurrence key is independent of that revision.
- An archived Workspace has no nonterminal Tasks or enabled Automations and admits no new Task or inbound channel work.
- A Task cannot be completed while a mandatory criterion lacks its required evidence,
  an Approval is unresolved, a required child Attempt is active, or an Effect remains
  ambiguous.
- Deleting or archiving a user-visible entity never erases evidence needed for audit,
  effect reconciliation, or an in-progress replication cursor.
