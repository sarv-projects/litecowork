# Shared Schemas and Error Taxonomy

This document defines language-neutral value types shared across domains. Entity
ownership and fields are in `DATA-MODEL.md`; legal transitions are in
`STATE-MACHINES.md`. These definitions are canonical.

## Primitive conventions

- IDs are opaque, stable, non-empty strings; Rust may wrap them in distinct newtypes.
- Timestamps are RFC 3339 UTC instants. Cross-Runtime ordering uses the HLC in
  `EVENTS.md`, not wall-clock comparison.
- Durations are integer milliseconds. Sizes and sequences are unsigned integers.
- Content digests identify their algorithm; SHA-256 is the v1 content digest.
- Missing authority, permission, identity, or policy data is never assigned an
  implicit permissive default.
- Wire enum values are stable uppercase strings. Unknown values are rejected for
  commands and retained opaquely in forward-compatible replication where safe.

## Identifiers

```text
WorkspaceId ConversationId MessageId TaskId TaskSpecRevisionId PlanRevisionId
StepId AttemptId AgentProfileId AgentBindingId AgentSessionId RuntimeId
EnvironmentId EnvironmentCheckpointId CapabilityId CapabilityGrantId
CapabilityActivationId ArtifactId EffectId EvidenceId
VerificationRunId ApprovalId AutomationId OccurrenceId ExecutionLeaseId
HandoffId ConnectionId ChannelBindingId PrincipalId SecretRefId
SecretLeaseId AuditRecordId EventId RequestId CorrelationId
```

## Status enums

```text
WorkspaceStatus = ACTIVE | ARCHIVED
ReplicationPolicy = LOCAL_ONLY | METADATA_ONLY | ACTIVE_TASK_INPUTS | SELECTED_FOLDERS | FULL_WORKSPACE
AgentSessionKind = LEAD_PLANNING | STEP_EXECUTION
ChannelEventKind = INBOUND | EDIT | DELETE
ChannelEventReceiptStatus = RECEIVED | PROCESSING | ACCEPTED | REJECTED | FAILED

TaskStatus = DRAFT | READY | RUNNING | WAITING_USER | BLOCKED | VERIFYING |
  NEEDS_USER | INCOMPLETE | COMPLETED | FAILED | CANCEL_REQUESTED | CANCELLED

StepStatus = PENDING | READY | RUNNING | WAITING_USER | BLOCKED | VERIFYING |
  COMPLETED | FAILED | CANCEL_REQUESTED | CANCELLED | SUPERSEDED

AttemptStatus = CREATED | PREPARING | RUNNING | WAITING_APPROVAL |
  WAITING_RESOURCE | CHECKPOINTING | COMPLETED | FAILED | ABANDONED |
  CANCEL_REQUESTED | CANCELLED

AgentSessionStatus = STARTING | ACTIVE | INTERRUPTING | CLOSING | CLOSED | LOST
RuntimeAvailability = PAIRING | ONLINE | DEGRADED | DRAINING | OFFLINE | REVOKED
EnvironmentStatus = NEW | PROVISIONING | READY | BUSY | CHECKPOINTING |
  SUSPENDED | FAILED | DESTROYING | DESTROYED
CapabilityGrantStatus = ACTIVE | REVOKED | EXPIRED
CapabilityActivationStatus = STARTING | ACTIVE | FAILED | STOPPING | STOPPED
ActivationHealth = HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
ArtifactLibraryStatus = TRANSIENT | SAVED | ARCHIVED
ApprovalStatus = PENDING | APPROVED | DENIED | EXPIRED | CANCELLED
AutomationStatus = ENABLED | PAUSED | DISABLED
OccurrenceStatus = PENDING | CLAIMED | STARTED | COMPLETED | SKIPPED | FAILED
ClaimEpoch = monotonically increasing u64 per claimable receipt/occurrence
LeaseStatus = ACTIVE | RELEASING | RELEASED | EXPIRED | REVOKED
FailoverClass = SAFE_PORTABLE | REPLAYABLE | HANDOFF_REQUIRED | LOCAL_BOUND
EvidenceLevel = REPORTED | OBSERVED | VERIFIED
EffectState = PROPOSED | STARTED | ACKNOWLEDGED | RECONCILING | OBSERVED |
  VERIFIED | FAILED | AMBIGUOUS

SecretLeaseStatus = ACTIVE | REVOKED | EXPIRED
ConnectionStatus = CONNECTING | CONNECTED | DEGRADED | REAUTH_REQUIRED | DISCONNECTED
ChannelBindingStatus = ACTIVE | DEGRADED | REVOKED
HandoffPhase = REQUESTED | DRAINING_SOURCE | CHECKPOINTING | REPLICATING |
  RECONCILING | LEASE_RELEASE | TARGET_PREPARE | TARGET_LEASE | TARGET_ATTEMPT |
  COMPLETED | FAILED
```

The transition rules and owners for every status are in `STATE-MACHINES.md`.
`ExpectedRevision` is `ABSENT` for first creation or a concrete current aggregate
revision for an update. The command transaction rechecks it before commit.

## ResourceRef

```text
ResourceRef {
  scheme: string
  authority?: string
  id: string
  revision?: string
  digest?: string
  metadata?: object
}
```

Use a revision-pinned reference when correctness depends on exact input. V1 schemes are
listed in `DATA-MODEL.md`. A local absolute path is never presumed to exist on another
Runtime.

## Conversation content

```text
MessageContentBlock =
  TEXT { text: string }
  | RESOURCE { resource_ref: ResourceRef, display_name?: string }
```

Content blocks are ordered and immutable after message creation. Binary content is
always carried by a ResourceRef; the message body never embeds arbitrary binary data.

## Referenced values

```text
PrincipalRef {
  principal_id: PrincipalId
  kind: USER | SERVICE | RUNTIME | AGENT | CHANNEL_IDENTITY
}

SecretRef {
  secret_ref_id: SecretRefId
  provider_ref: string
  placement: LOCAL_ONLY | CLOUD_AVAILABLE | RUNTIME_BOUND | EXTERNAL_AGENT_OWNED
}

BlobRef {
  digest: string
  size_bytes: u64
  media_type: string
}

ApprovalRequirement {
  requirement_id: string
  action_class: string
  required_assurance: AssuranceLevel
  mandatory: boolean
}

CapabilityRequirement {
  semantic_requirement: string
  operation_ids: string[]
  resource_scope?: ResourceScope
}

ResourceScope {
  resource_refs: ResourceRef[]
  operation_ids: string[]
  constraints: JsonObject
}
```

References do not carry secret bytes or grant authority. Full scope and lifecycle
semantics are defined by the owning domain document.

## Criteria, outputs, budgets

```text
AcceptanceCriterion {
  criterion_id: string
  description: string
  subject_refs?: ResourceRef[]
  required_evidence: EvidenceLevel
  verifier_hint?: string
  mandatory: boolean
}

OutputRequirement {
  output_id: string
  description: string
  kind?: string
  media_type?: string
  destination?: ResourceRef
  mandatory: boolean
}

BudgetSpec {
  max_wall_time_ms?: u64
  max_cost_minor_units?: u64
  currency?: string
  max_tokens?: u64
  max_child_attempts?: u32
  max_concurrency?: u32
}
```

LiteCowork enforces only quantities it can observe. Unknown native-agent spend is
reported as unknown, not zero. A currency is required when a monetary ceiling exists.

## Placement and continuation

```text
PlacementPreference = AUTO | LOCAL_ONLY | CLOUD_PREFERRED | CLOUD_ONLY |
  RUNTIME(RuntimeId)

ContinuationEligibility {
  eligible: boolean
  required_inputs_available: boolean
  compatible_agent_available: boolean
  capabilities_available: boolean
  secrets_available: boolean
  environment_reproducible: boolean
  open_effects_reconciled: boolean
  failover_class: FailoverClass
  policy_allows_remote_execution: boolean
  budget_available: boolean
  blockers: Blocker[]
}
```

Preference never overrides trust, input, resource, lease, capability, or failover rules.

## Retry and failure

```text
RetryPolicy {
  max_attempts: u32
  initial_backoff_ms: u64
  max_backoff_ms: u64
  multiplier: number
  jitter: boolean
  retryable_error_codes: string[]
}

FailureRecord {
  code: ErrorCode
  category: TRANSIENT | PERMANENT | POLICY | USER_REQUIRED | AMBIGUOUS
  safe_message: string
  retryable: boolean
  signature: string
  correlation_id: CorrelationId
  details?: object
  occurred_at: Timestamp
}
```

RetryPolicy alone never makes a consequential operation safe; provider idempotency or
successful reconciliation is also required.

## Assurance and errors

```text
AssuranceLevel = VIEW_ONLY | STEER_SAFE | APPROVE_SAFE |
  APPROVE_SENSITIVE | LOCAL_STRONG
ChannelAction = VIEW | STEER | APPROVE_SAFE | APPROVE_SENSITIVE
```

Domain errors contain a typed code, safe message, retryability, correlation ID, and
optional structured details. Public errors never reveal secret bytes or unauthorized
resource existence.

```text
NOT_FOUND UNAUTHORIZED FORBIDDEN POLICY_DENIED APPROVAL_REQUIRED
WORKSPACE_ARCHIVED WORKSPACE_NOT_QUIESCENT STALE_WORKSPACE_VERSION
INVALID_ARGUMENT INVALID_TRANSITION STALE_VERSION STALE_SPEC_REVISION CONFLICT
ARTIFACT_ARCHIVED INVALID_ARTIFACT_TRANSITION
TASK_TERMINAL INVALID_PLAN PLAN_CYCLE STEP_NOT_READY LEASE_CONFLICT STALE_FENCE
PLACEMENT_UNAVAILABLE RUNTIME_UNAVAILABLE ENVIRONMENT_UNAVAILABLE
CAPABILITY_UNAVAILABLE CAPABILITY_UNHEALTHY SECRET_UNAVAILABLE RESOURCE_UNAVAILABLE
AUTOMATION_NOT_FOUND AUTOMATION_REVISION_NOT_FOUND AUTOMATION_DISABLED INVALID_TRIGGER
STALE_CLAIM_EPOCH MISFIRE_LIMIT_EXCEEDED OCCURRENCE_CONFLICT
EFFECT_AMBIGUOUS EFFECT_RECONCILIATION_REQUIRED VERIFICATION_FAILED RATE_LIMITED
TIMEOUT DEPENDENCY_UNAVAILABLE UNSUPPORTED_VERSION INTEGRITY_FAILURE INTERNAL
```

The Operator error envelope is specified in `API.md`. `TriggerSpec` is a tagged union
defined in `AUTOMATION.md`; trigger deliveries are deduplicated before Task creation.
