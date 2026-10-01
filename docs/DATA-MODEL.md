# Data Model

All IDs are opaque, globally unique and stable. Durable records carry creation time;
mutable aggregate records also carry `updated_at` and an optimistic `version` unless
explicitly immutable. Timestamps are RFC 3339 UTC. Canonical entities and their
relationships are defined here; shared enums are in `SCHEMAS.md`, transitions in
`STATE-MACHINES.md`, and SQL representation in `schemas/sqlite-v1.sql`.

## Workspace

```text
Workspace {
  workspace_id: WorkspaceId
  name: string
  owner_principal_id: PrincipalId
  replication_policy: ReplicationPolicy
  hub_runtime_id: RuntimeId?
  status: ACTIVE | ARCHIVED
  created_at: Timestamp
  version: u64
}

ReplicationPolicy = LOCAL_ONLY | METADATA_ONLY | ACTIVE_TASK_INPUTS |
  SELECTED_FOLDERS | FULL_WORKSPACE
```

The default for cloud-enabled personal workspaces is `ACTIVE_TASK_INPUTS`. A policy
selects which resources may replicate; it never authorizes a capability or secret.

## Conversation

```text
Conversation {
  conversation_id: ConversationId
  workspace_id: WorkspaceId
  title: string?
  status: ACTIVE | ARCHIVED
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
  lead_attempt_id: AttemptId?
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
  attempt_id: AttemptId?
  native_session_ref: string?
  status: AgentSessionStatus
  started_at: Timestamp
  last_event_at: Timestamp?
  closed_at: Timestamp?
  version: u64
}
```

A Task-scoped lead session may create the initial PlanRevision before Steps and execution
Attempts exist. An execution AgentSession is bound to one Attempt. A plan revision made
during execution records both its producing session and Attempt.

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

Unique: `(artifact_id, version)`.

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
}
```

## Automation

```text
Automation {
  automation_id: AutomationId
  workspace_id: WorkspaceId
  name: string
  trigger: TriggerSpec
  task_template: TaskTemplate
  execution_policy: AutomationExecutionPolicy
  status: ENABLED | PAUSED | DISABLED
  created_at: Timestamp
  version: u64
}

AutomationOccurrence {
  occurrence_id: OccurrenceId
  automation_id: AutomationId
  automation_version: u64
  scheduled_key: string
  scheduled_for: Timestamp
  task_id: TaskId?
  status: PENDING | CLAIMED | STARTED | COMPLETED | SKIPPED | FAILED
  created_at: Timestamp
  updated_at: Timestamp
}
```

Unique: `(automation_id, scheduled_key)` prevents duplicate logical occurrences.

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
  event_kind: INBOUND | EDIT | DELETE | OUTBOUND_DELIVERY
  payload_digest: string
  conversation_id: ConversationId?
  message_id: MessageId?
  received_at: Timestamp
  state: RECEIVED | MATERIALIZED | REJECTED | AMBIGUOUS | SETTLED
}
```

Unique `(channel_binding_id, provider_event_id)` prevents duplicate materialization.
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
  blob://<digest>
```

Resource references are immutable identifiers or revision-pinned locators whenever correctness depends on content identity.

## Cross-entity invariants

- Every TaskSpecRevision belongs to one Task and has a unique monotonic revision.
- Every PlanRevision names the TaskSpecRevision it planned against and the producing
  Attempt; a plan never mutates after publication.
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
- A Task cannot be completed while a mandatory criterion lacks its required evidence,
  an Approval is unresolved, a required child Attempt is active, or an Effect remains
  ambiguous.
- Deleting or archiving a user-visible entity never erases evidence needed for audit,
  effect reconciliation, or an in-progress replication cursor.
