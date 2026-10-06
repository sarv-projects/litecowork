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
- JSON values covered by a digest use RFC 8785 JCS canonical bytes. Numeric values outside
  the RFC 8785/I-JSON domain are rejected or represented as strings by the owning schema.
- Missing authority, permission, identity, or policy data is never assigned an
  implicit permissive default.
- Wire enum values are stable uppercase strings. Unknown values are rejected for
  commands and retained opaquely in forward-compatible replication where safe.

```text
JsonValue = null | boolean | number | string | JsonValue[] | JsonObject
JsonObject = map<string, JsonValue>
```

## Identifiers

```text
WorkspaceId WorkspaceInstructionRevisionId ConversationId ConversationTurnId MessageId
TaskId TaskSpecRevisionId PlanRevisionId StepId AttemptId AgentProfileId
AgentEndpointId AgentBindingId AgentSessionId AgentHostInstanceId CapabilityHostInstanceId RuntimeId RuntimeIncarnationId
EnvironmentId EnvironmentCheckpointId EnvironmentControlLeaseId CapabilityId CapabilityGrantId
CapabilityActivationId CapabilityInvocationId ArtifactId EffectId EvidenceId
VerificationRunId ApprovalId ApprovalUseId AutomationId OccurrenceId ExecutionLeaseId
HandoffId ConnectionId ChannelBindingId PrincipalId SecretRefId SecretLeaseId
UserRequestId UserRequestResponseId UsageObservationId BudgetReservationId DeliveryId SkillProposalId
RoutineId ExecutionDependencyPlanId DelegationProfileId CoworkerId GoalId SuggestionId
DemonstrationSessionId WarmHoldId ActionBatchId
ResourceId ResourceRevisionId ResourceLocationId WorkspaceRootId ResourceEdgeId DependencyEdgeId
InvalidationRecordId ResourceUploadId BackupId AuditRecordId EventId RequestId CorrelationId ServiceId
```

## Status enums

```text
WorkspaceStatus = ACTIVE | ARCHIVED
AgentProtocol = ACP | A2A | SDK | API | CLI | TERMINAL
AgentEndpointTopology = LOCAL_INTERACTIVE | REMOTE_AGENT_SERVICE | VENDOR_SERVICE | PROCESS_ADAPTER
EndpointSelectionMode = AUTO_COMPATIBLE | PINNED_ENDPOINT
NativeHarnessIntegrityMode = NATIVE_UNMODIFIED | NATIVE_PLUS_BRIDGE
AgentDelegationMode = NATIVE_INTERNAL | HOST_DELEGATED | CAPABILITY_EXECUTOR
DelegationProfileStatus = ENABLED | DISABLED | ARCHIVED
DelegationStrategy = NATIVE_DEFAULT | BALANCED | COST_SAVER | HOST_DELEGATION_ONLY
OptimizationPreference = QUALITY_FIRST | BALANCED | COST_FIRST | LATENCY_FIRST
DelegationProfileSelection = AUTOMATIC | PREFER | REQUIRE
ExecutionLatencyClass = STANDARD | INTERACTIVE | DEADLINE_SENSITIVE
WarmTrigger = ACTIVE_TASK | RECENT_USE | USER_SELECTED | QUOTA_LOW | PREDICTED_FAILOVER | DEADLINE_APPROACHING
TaskCategory = SOFTWARE_ENGINEERING | RESEARCH | WRITING | DATA_ANALYSIS | OFFICE | BROWSER | PERSONAL_ADMIN | OTHER
EnvironmentSharingScope = ATTEMPT_PRIVATE | TASK_SHARED | COWORKER_PRIVATE | WORKSPACE_SHARED | USER_SHARED
CoworkerStatus = ACTIVE | PAUSED | ARCHIVED
GoalStatus = ACTIVE | PAUSED | COMPLETED | ARCHIVED
SuggestionStatus = PROPOSED | ACCEPTED | DISMISSED | EXPIRED
SuggestionAction = TASK | OPEN_ROUTINE_EDITOR | OPEN_AUTOMATION_EDITOR
SuggestionKind = TASK_OPPORTUNITY | ROUTINE_OPPORTUNITY | AUTOMATION_OPPORTUNITY
SuggestionTrigger = TASK_OUTCOME_COMMITTED | ROUTINE_HEALTH_CHANGED | AUTHORIZED_RESOURCE_CHANGE | OWNER_CONFIGURED_CHECK
ContextDocumentKind = PERSONAL_PROFILE | COWORKER_NOTES | WORKSPACE_NOTES | GOAL_NOTES
ContextDocumentStatus = ACTIVE | REVOKED | DELETION_PENDING | DELETED
ExecutionMethod = STRUCTURED_API | STRUCTURED_BROWSER | ACCESSIBILITY_BROWSER |
  SCREEN_COMPUTER_USE | DETERMINISTIC_LOCAL | NATIVE_AGENT_TOOL | UNKNOWN
DemonstrationSensitiveRegionPolicy = PAUSE_ON_DETECTION | OMIT_SENSITIVE_FIELDS
LeadFailoverMode = DISABLED | ASK | ALLOW_LISTED
LeadFailoverTrigger = AGENT_UNAVAILABLE | QUOTA_EXHAUSTED | RUNTIME_UNAVAILABLE
InteractionDefault = STANDARD_TRUST_POLICY | REQUIRE_OWNER_APPROVAL | HANDOFF_TO_OWNER
ContextOwnerRef = USER(PrincipalId) | WORKSPACE(WorkspaceId) | COWORKER(WorkspaceId, CoworkerId) | GOAL(WorkspaceId, GoalId)
BudgetThresholdAction = WARN | REDUCE_CONCURRENCY | PREFER_CHEAPER | REQUIRE_APPROVAL | STOP_NEW_DELEGATION
DemonstrationSessionStatus = CREATED | CAPTURING | PAUSED | REVIEW | CONVERTED | ABORTED
AgentFeature = session.resume | session.steer | session.interrupt | session.cancel | session.fork |
  input.text | input.image | input.file | input.resources | extension.mcp_stdio |
  extension.mcp_http | extension.skills | extension.plugins | extension.dynamic_attach |
  reporting.tool_calls | reporting.plan | reporting.usage | reporting.native_subagents |
  reporting.approvals | environment.cwd | environment.extra_directories
RuntimeRole = WORKSPACE_HUB | EXECUTOR | RESOURCE_NODE | CHANNEL_HOST | TRIGGER_HOST | OPERATOR_ENDPOINT
TrustZone = PERSONAL_DEVICE | USER_CLOUD | MANAGED_CLOUD
SensitivityClass = PUBLIC | PERSONAL | CONFIDENTIAL | RESTRICTED
EndpointSelectionPolicy = {
  mode: EndpointSelectionMode,
  endpoint_id?: AgentEndpointId,
  required_features: AgentFeature[],
  preferred_topologies: AgentEndpointTopology[]
}
SemVer = string  # Semantic Versioning 2.0.0, including major.minor.patch
ResourceCapacity = {
  cpu_cores?: number,
  memory_available_bytes?: u64,
  storage_available_bytes?: u64,
  max_concurrent_attempts?: u32,
  sampled_at: Timestamp
}
ResourceUploadState = OPEN | CONTENT_RECEIVED | COMMITTED | FAILED | EXPIRED
ReplicationPolicy = LOCAL_ONLY | METADATA_ONLY | ACTIVE_TASK_INPUTS | SELECTED_FOLDERS | FULL_WORKSPACE
AgentSessionScope =
  CONVERSATION { conversation_id, conversation_turn_id }
  | TASK_PLANNING { task_id }
  | ATTEMPT_EXECUTION { task_id, attempt_id }
CapabilityScope =
  CONVERSATION { conversation_id }
  | TASK_PLANNING { task_id }
  | ATTEMPT_EXECUTION { task_id, attempt_id }
CapabilityGrantScope = CapabilityScope
CapabilityInvocationScope = CapabilityScope
ChannelEventKind = INBOUND | EDIT | DELETE
ChannelEventReceiptStatus = RECEIVED | PROCESSING | ACCEPTED | REJECTED | FAILED
ChannelHostAssignmentStatus = ACTIVE | DRAINING
ChannelIngressContinuity = CONTINUOUS | GAP_ACCEPTED
ChannelIngressCursorBindingStatus = AVAILABLE | RECONCILIATION_REQUIRED | UNAVAILABLE
ConversationTurnStatus = OPEN | RUNNING | WAITING_USER | WAITING_DEPENDENCY | COMPLETED | FAILED |
  CANCEL_REQUESTED | CANCELLED
# FAILED -> RUNNING is permitted only through AgentTurnCoordinator.retry_turn;
# the retry increments retry_ordinal and starts a new AgentSession.

TaskStatus = READY | RUNNING | WAITING_USER | BLOCKED | VERIFYING |
  NEEDS_USER | INCOMPLETE | PAUSE_REQUESTED | PAUSED | COMPLETED | FAILED |
  CANCEL_REQUESTED | CANCELLED

StepStatus = PENDING | READY | RUNNING | WAITING_USER | BLOCKED | VERIFYING |
  COMPLETED | FAILED | CANCEL_REQUESTED | CANCELLED | SUPERSEDED

AttemptStatus = CREATED | PREPARING | RUNNING | WAITING_APPROVAL |
  WAITING_RESOURCE | CHECKPOINTING | COMPLETED | FAILED | ABANDONED |
  CANCEL_REQUESTED | CANCELLED

AgentSessionStatus = STARTING | ACTIVE | INTERRUPTING | CLOSING | CLOSED | LOST
RoutineStatus = ACTIVE | ARCHIVED
AutomationOccurrenceStatus = PENDING | CLAIMED | WAITING_DEPENDENCY | STARTED | COMPLETED | SKIPPED | FAILED
RoutineRevisionRef = { routine_id: RoutineId, revision: u64 }
GoalRevisionRef = { goal_id: GoalId, revision: u64 }
CoworkerRevisionRef = { coworker_id: CoworkerId, revision: u64 }
RuntimeAvailability = PAIRING | STARTING | RECOVERING | ONLINE | DEGRADED | DRAINING | OFFLINE | REVOKED
RuntimeStartupPolicy = MANUAL | LOGIN_BACKGROUND | ALWAYS_ON_SERVICE
RuntimeIncarnationState = STARTING | RECOVERING | READY | DEGRADED | DRAINING | STOPPING | STOPPED
AgentHostState = STARTING | READY | BUSY | DEGRADED | STOPPING | STOPPED | FAILED
AgentHostingMode = REMOTE_API | REMOTE_A2A | LOCAL_SHARED_DAEMON | LOCAL_PER_SESSION | EMBEDDED_SDK | EXTERNAL_PROCESS
AgentHostOwnership = LITECOWORK | EXTERNAL | REMOTE
OfferReadiness = AVAILABLE | STARTABLE | STARTING | READY | BUSY | DEGRADED | OFFLINE | NEEDS_AUTH | UNAVAILABLE
EnvironmentStatus = NEW | PROVISIONING | READY | BUSY | CHECKPOINTING |
  SUSPENDED | FAILED | DESTROYING | DESTROYED
EnvironmentLifetime = ATTEMPT | TASK_RETAINED | WORKSPACE_PERSISTENT
EnvironmentClass = LOCAL_WORKSPACE | GIT_WORKTREE | CONTAINER | VM | CLOUD_SANDBOX | REMOTE_MACHINE | BROWSER | DESKTOP
EnvironmentHealth = HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
EnvironmentBudgetEnforcementPolicy = REQUIRE_PROVIDER_ENFORCED | ALLOW_HOST_MONITORED
EnvironmentBudgetEnforcement = PROVIDER_ENFORCED | HOST_MONITORED | UNAVAILABLE
EnvironmentProvisionPreviewStatus = ISSUED | CONSUMED | EXPIRED
NeedsYouItemKind = APPROVAL | USER_REQUEST | TASK_BLOCKER
NeedsYouStatus = OPEN | RESOLVED
ResourceLimits = { cpu_millis: u32, memory_bytes: u64, storage_bytes: u64, max_processes?: u32, max_lifetime_seconds?: u64 }
NetworkPolicy = { mode: NONE | RESTRICTED, allowed_domains: string[], max_response_bytes: u64, max_download_bytes: u64, deny_private_networks: true }
CapabilityGrantStatus = ACTIVE | REVOKED | EXPIRED
ProviderCircuitStatus = CLOSED | OPEN | HALF_OPEN
CapabilityActivationStatus = STARTING | ACTIVE | FAILED | STOPPING | STOPPED
CapabilityHostState = STARTING | READY | BUSY | DEGRADED | STOPPING | STOPPED | FAILED
CapabilityHostMode = LOCAL_MANAGED | REMOTE_PROVIDER
CapabilityHostSharingPolicy = EXCLUSIVE | TASK_ISOLATED | TRUST_PARTITION_SHARED
CapabilityInvocationStatus = CREATED | DISPATCHED | WAITING | INPUT_REQUIRED |
  CANCEL_REQUESTED | SUCCEEDED | FAILED | CANCELLED | AMBIGUOUS
ProviderTaskStatus = WORKING | INPUT_REQUIRED | COMPLETED | FAILED | CANCELLED | UNKNOWN
ProviderContinuationBindingStatus = AVAILABLE | RECONCILIATION_REQUIRED | UNAVAILABLE
ProviderInputDeliveryStatus = AWAITING_RESPONSE | PENDING | DISPATCHED | ACKNOWLEDGED |
  ACCEPTED | AMBIGUOUS | REJECTED | EXPIRED | CANCELLED
ActivationHealth = HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
ArtifactLibraryStatus = TRANSIENT | SAVED | ARCHIVED
ApprovalStatus = PENDING | APPROVED | DENIED | EXPIRED | CANCELLED
UserRequestStatus = PENDING | ANSWERED | DISMISSED | EXPIRED | CANCELLED
UserRequestKind = QUESTION | DECISION | RESOURCE_SELECTION | EXTERNAL_AUTHORIZATION
UserRequestInteractionMode = FORM | EXTERNAL_URL
UserRequestScope = CONVERSATION { conversation_id, conversation_turn_id }
  | TASK_PLANNING { task_id }
  | ATTEMPT_EXECUTION { task_id, attempt_id }
NotificationDeliveryStatus = PENDING | SENDING | SENT | FAILED | AMBIGUOUS | SUPPRESSED
SkillProposalStatus = DRAFT | REVIEW | APPROVED | REJECTED | PUBLISHED
ResourceFreshness = CURRENT | STALE | CONFLICTED | UNKNOWN | UNAVAILABLE
ResourceLocationFreshness = CURRENT | STALE | UNKNOWN | UNAVAILABLE
DependencyFreshness = CURRENT | STALE | CONFLICTED | UNKNOWN
ResourceLocationAvailability = AVAILABLE | OFFLINE | PLACEHOLDER | REVOKED | UNKNOWN
WorkspaceRootStatus = ACTIVE | PAUSED | REVOKED | UNAVAILABLE
VerificationRunStatus = PENDING | RUNNING | PASSED | FAILED | INCONCLUSIVE
AutomationStatus = ENABLED | PAUSED | DISABLED
OccurrenceStatus = PENDING | CLAIMED | WAITING_DEPENDENCY | STARTED | COMPLETED | SKIPPED | FAILED
ClaimEpoch = monotonically increasing u64 per claimable receipt/occurrence
ResourceIdentityConfidence = STRONG | PROVIDER_SCOPED | WEAK
ResourceEdgeRelation = CONTAINS | DERIVED_FROM | REFERENCES | SAME_PROVIDER_OBJECT
InvalidationDependentKind = ARTIFACT_VERSION | VERIFICATION_RUN
Blocker {
  blocker_id: string
  code: ErrorCode
  safe_message: string
  subject_ref?: ResourceRef | ServiceRef
  resolution_hint: string
  user_request_id?: UserRequestId
  approval_id?: ApprovalId
  created_at: Timestamp
}
NeedsYouItem {
  item_id: Sha256Digest       # stable hash defined by the canonical encoding below
  kind: NeedsYouItemKind
  status: NeedsYouStatus
  approval_id?: ApprovalId
  user_request_id?: UserRequestId
  task_id?: TaskId
  blocker_id?: string
  title: string
  safe_summary: string
  created_at: Timestamp
  resolved_at?: Timestamp
}
NeedsYouFilter = { status?: NeedsYouStatus, task_id?: TaskId, kind?: NeedsYouItemKind }
ArtifactContentKind = MANAGED_BLOB | EXTERNAL_RESOURCE
UsageConfidence = EXACT | ESTIMATED | UNKNOWN
UsageSource = AGENT_REPORTED | PROVIDER_REPORTED | HOST_MEASURED
BudgetReservationStatus = RESERVED | COMMITTED | RELEASED | EXPIRED
BudgetScope = TASK | ENVIRONMENT
EnvironmentBudgetStatus = WITHIN_LIMIT | LIMIT_REACHED | UNKNOWN
EnvironmentBudgetUsage = {
  status: EnvironmentBudgetStatus,
  consumed_cost_minor_units?: u64,
  currency?: string,
  consumed_wall_time_ms?: u64,
  confidence: UsageConfidence,
  observed_at?: Timestamp
}
EnvironmentControlOwnerKind = AGENT | HUMAN
EnvironmentControlLeaseStatus = ACTIVE | RELEASING | RELEASED | EXPIRED | REVOKED
LeaseStatus = ACTIVE | RELEASING | RELEASED | EXPIRED | REVOKED
FailoverClass = SAFE_PORTABLE | REPLAYABLE | HANDOFF_REQUIRED | LOCAL_BOUND
EvidenceLevel = REPORTED | OBSERVED | VERIFIED
EffectState = PROPOSED | STARTED | ACKNOWLEDGED | RECONCILING | OBSERVED |
  VERIFIED | FAILED | AMBIGUOUS

ArtifactContent =
  MANAGED_BLOB { storage_ref: BlobRef, content_digest: Sha256Digest, media_type: string, size_bytes: u64 }
  | EXTERNAL_RESOURCE { resource_ref: PinnedResourceRef, provider_revision?: string, observed_digest?: Sha256Digest, observed_at: Timestamp }

SecretLeaseStatus = ACTIVE | REVOKED | EXPIRED
ConnectionStatus = CONNECTING | CONNECTED | DEGRADED | REAUTH_REQUIRED | DISCONNECTED
ChannelBindingStatus = ACTIVE | DEGRADED | REVOKED
HandoffPhase = REQUESTED | DRAINING_SOURCE | CHECKPOINTING | REPLICATING |
  RECONCILING | LEASE_RELEASE | TARGET_PREPARE | TARGET_LEASE | TARGET_ATTEMPT |
  COMPLETED | FAILED
```

`NATIVE_DEFAULT` preserves harness policy and offers no host-delegation preference;
`BALANCED` presents enabled host profiles alongside native capability; `COST_SAVER` guides
the lead toward lower known-cost eligible workers subject to quality/verification policy;
`HOST_DELEGATION_ONLY` is available only when the selected adapter can explicitly enforce
the restriction without rewriting native configuration. Unsupported restrictions fail
configuration validation.

`EscalationPolicy.max_worker_attempts` is 1..8 including the first child Attempt;
fallback profile IDs are ordered, unique, and cannot exceed the remaining attempt count.
Each fallback is revalidated at admission, and every admitted retry records a new Attempt
and profile revision.

`ResourceCapacity` is an approximate, timestamped placement hint. It is not an
authorization decision or a reservation; admission rechecks current capacity. A
`TrustZone` names the administrative boundary hosting a Runtime. It does not replace
per-resource sensitivity, capability grants, secret placement, or approval checks.
`SensitivityClass` is classification metadata; TrustService defines the policy mapping
and callers never gain authority from a label alone.

For `UsageObservation`, `confidence=UNKNOWN` requires a null quantity; zero is a real
measurement. COST observations with comparable exact/estimated values require an explicit
ISO 4217 currency. BudgetService never adds unlike currencies and never treats missing
telemetry as zero.

`NeedsYouItem.item_id` is `sha256:` plus lowercase SHA-256 of these bytes: UTF-8
`litecowork.needs-you.item.v1`, one zero byte, then each UTF-8 field prefixed by its
unsigned 32-bit big-endian byte length. Fields are the uppercase item kind and source
identity: `APPROVAL, approval_id`; `USER_REQUEST, user_request_id`; or
`TASK_BLOCKER, task_id, blocker_id`. A blocker linked to an Approval or UserRequest uses
that underlying identity and kind, so it produces one inbox item. Title, summary, status,
timestamps, and notification delivery state are excluded. The ID is a projection key, not
a bearer credential or authorization token.

For `EndpointSelectionPolicy`, `PINNED_ENDPOINT` requires `endpoint_id`; `AUTO_COMPATIBLE`
forbids it. An endpoint must support every `required_features` entry and satisfy the
Runtime/Workspace constraints. Preferred topology ordering is a per-binding preference,
not a global protocol ranking.

For JSON APIs and events, these tagged unions serialize using the `kind` discriminator:
`{"kind":"CONVERSATION","conversation_id":"..."}`,
`{"kind":"TASK_PLANNING","task_id":"..."}`, or
`{"kind":"ATTEMPT_EXECUTION","task_id":"...","attempt_id":"..."}`. A decoder
rejects fields from other variants.

The transition rules and owners for every status are in `STATE-MACHINES.md`. Runtime incarnations represent daemon process lifecycles under one persistent RuntimeId. Agent host status and ExecutionDependencyPlan are Runtime-operational projections, not cross-Runtime Task truth. Persistent Environments require explicit lifetime policy.
`AutomationCursor` has a composite identity `(automation_id, trigger_id)` and is not assigned a second opaque ID. A RoutineRevision is likewise keyed
by `(routine_id, revision)`.

`ExpectedRevision` is `ABSENT` for first creation or a concrete current aggregate
revision for an update. The command transaction rechecks it before commit.

Every mutating application command receives an out-of-band `CommandContext`; its
`request_id` is populated from HTTP `Idempotency-Key` or the equivalent local IPC field,
and it carries the authenticated principal, selected Workspace, and correlation ID. These
transport metadata are not repeated in every command body. Expected aggregate versions
remain explicit command preconditions.

```text
CommandContext {
  authenticated_principal: PrincipalRef
  workspace_id?: WorkspaceId
  request_id: RequestId
  correlation_id: CorrelationId
}

ExpectedRevision = ABSENT | u64
```

## ResourceRef

```text
ResourceRef {
  workspace_id: WorkspaceId
  resource_id: ResourceId
  revision_id?: ResourceRevisionId
}
```

```text
PinnedResourceRef = ResourceRef with revision_id required

ArtifactVersionRef {
  workspace_id: WorkspaceId
  artifact_id: ArtifactId
  version: u64
}

ResourceInput {
  resource_ref: PinnedResourceRef
  observed_digest?: Sha256Digest
}
```

`ResourceInput.observed_digest`, when present, hashes the exact bytes presented to the
consumer. It is distinct from the optional provider-observed `ResourceRevision.content_digest`;
the ResourceRef identifies only the logical Resource and optional immutable revision. If
both digests are available, they must match; a mismatch is an input-integrity failure.

`resource://<workspace-id>/<resource-id>@<revision-id?>` is the display form. ResourceRef
identifies a logical Resource independently of its locations. Paths, Runtime IDs,
connector handles, browser tabs, secret handles, and blob storage references are not
Resource identity. Use a revision-pinned reference when correctness depends on exact
input. A local absolute path is never presumed to exist on another Runtime.

```text
CapabilityRef {
  capability_id: CapabilityId
  identity_kind: PACKAGE_COMPONENT | MCP_SKILL
  source: string
  package_version?: string
  digest: Sha256Digest
  component?: string
}
```

`source` is the LiteSPM-normalized source identity for `PACKAGE_COMPONENT` and the
host-authenticated MCP server identity for `MCP_SKILL`. Package components require an
opaque, non-empty `package_version` exactly as normalized by LiteSPM; LiteCowork does not
parse or order it. `component` is their provider component identifier when applicable.
MCP Skills require `component` to be the exact advertised `SKILL.md` URI, omit
`package_version`, and use the pinned manifest digest as `digest`. Their stable identity
is server identity plus exact URI; display names are never identity. This is LiteCowork's
internal normalized shape and does not define LiteSPM's package/API contract.
`capability_ref_key_digest` is SHA-256 over RFC 8785 canonical JSON of this complete
normalized value; storage uses it only as a deterministic key, never as a replacement for
the referenced content digest.

## Other shared values

```text
JsonSchema = JsonObject
Sha256Digest = `sha256:` followed by 64 lowercase hexadecimal characters

TriggerPlacement = HUB | SPECIFIC_RUNTIME | AUTO
WakePolicy = NEVER | TRY_WAKE | REQUIRE_RUNTIME_AWAKE
MisfirePolicy =
  SKIP
  | RUN_ONCE_WHEN_AVAILABLE
  | CATCH_UP_BOUNDED { max_occurrences: u32 }

TriggerSpec {
  trigger_id: string
  placement: TriggerPlacement
  runtime_id?: RuntimeId
  trigger: TriggerDefinition
}

TriggerDefinition is the tagged union defined in `AUTOMATION.md`. AutomationRevision
contains one or more TriggerSpecs and uses ANY semantics; there is no implicit AND.
Supported schemas do not imply an enabled provider: an unsupported trigger kind returns
`TRIGGER_UNSUPPORTED` and cannot be activated until a qualified TriggerHost provider is
available. `AutomationCursor` advancement is versioned and fenced to one TriggerHost.

AutomationCursor {
  automation_id: AutomationId
  trigger_id: string
  active_automation_revision: u64
  trigger_host_runtime_id: RuntimeId
  host_epoch: u64
  cursor_digest: Sha256Digest
  last_seen_digest?: Sha256Digest
  last_observation_ref?: ResourceRef
  next_scheduled_at?: Timestamp
  last_checked_at: Timestamp
  observation_gap_since?: Timestamp
  version: u64
}

AutomationTriggerBinding { # Runtime-local encrypted provider state
  automation_id: AutomationId
  trigger_id: string
  trigger_host_runtime_id: RuntimeId
  host_epoch: u64
  cursor_ciphertext: EncryptedBytes
  encryption_key_version: u32
  cursor_digest: Sha256Digest # digest of encrypted cursor bytes, not plaintext
  state: ProviderContinuationBindingStatus
  updated_at: Timestamp
  version: u64
}

CapabilityInvocationProviderBinding { # Runtime-local encrypted provider state
  invocation_id: CapabilityInvocationId
  runtime_id: RuntimeId
  last_runtime_incarnation_id: RuntimeIncarnationId
  binding_ciphertext: EncryptedBytes
  encryption_key_version: u32
  binding_digest: Sha256Digest # digest of ciphertext, never plaintext provider handles
  state: ProviderContinuationBindingStatus
  observed_at: Timestamp
  version: u64
}

ProviderInputBinding { # Runtime-local encrypted provider state and response outbox
  runtime_id: RuntimeId
  invocation_id: CapabilityInvocationId
  request_id: UserRequestId
  provider_input_key_ciphertext: EncryptedBytes
  provider_input_payload_ciphertext: EncryptedBytes # raw method/params; may contain a state-bearing URL
  encryption_key_version: u32
  provider_input_key_tag: Sha256Digest # Runtime-keyed local deduplication tag
  input_request_digest: Sha256Digest
  response_id?: UserRequestResponseId
  response_digest?: Sha256Digest
  delivery_status: ProviderInputDeliveryStatus
  dispatch_count: u32
  last_dispatch_at?: Timestamp
  last_provider_observation_at?: Timestamp
  retry_safety_proof_digest?: Sha256Digest # local digest of authenticated key-outstanding/replay-safe evidence
  failure_code?: ErrorCode
  updated_at: Timestamp
  version: u64
}

UserRequestResponse {
  response_id: UserRequestResponseId
  request_id: UserRequestId
  response: JsonValue
  response_digest: Sha256Digest
  responded_by: PrincipalRef
  response_channel_ref?: {
    channel_binding_id: ChannelBindingId
    provider_event_id: string
  }
  responded_at: Timestamp
}

Choice {
  choice_id: string
  label: string
  description?: string
  value: JsonValue
}

QuietHours {
  timezone: string
  weekdays: Weekday[]
  start_local_time: string  # HH:MM, 24-hour local time
  end_local_time: string    # HH:MM; an earlier value means the interval crosses midnight
}
Weekday = MONDAY | TUESDAY | WEDNESDAY | THURSDAY | FRIDAY | SATURDAY | SUNDAY

ProvenanceRecord {
  created_by_attempt?: AttemptId
  provider_ref?: string
  capability_ref?: CapabilityRef
  source_inputs: ResourceInput[]
  transformations: ProvenanceTransformation[]
  tool_reports: PinnedResourceRef[]
}

ProvenanceTransformation {
  operation: string
  inputs: ResourceInput[]
  output_digest?: Sha256Digest
  capability_ref?: CapabilityRef
}
```

`UserRequest` mode invariants: `FORM` is non-sensitive structured Workspace input;
`EXTERNAL_URL` is allowed only with `kind=EXTERNAL_AUTHORIZATION`, a null response schema,
and no form choices. The raw provider URL and input envelope are held only in the source
Runtime's encrypted ProviderInputBinding. An external URL response is exactly one
`action` property with `accept`, `decline`, or `cancel`; it carries no form `content`.
The user response to a FORM is ordinary non-secret user input. Schema validation plus
credential-like field checks are defense in depth and do not prove arbitrary prose contains
no secret.

`JsonSchema` is an inline JSON Schema 2020-12 object used to validate one bounded
`UserRequestResponse`; it cannot resolve remote `$ref` values or fetch network resources.
Implementations enforce parser depth/size limits. `QuietHours` uses the local calendar
and IANA timezone; the start weekday owns an overnight interval. Equal start/end times
mean no quiet interval for that day.

`response_channel_ref` is present exactly for a channel-originated response; its provider
event is the same authenticated event committed in the ChannelEventReceipt. The
Runtime-local reply target consumed by that event is not part of the replicated response.

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
  provider_ref: string # stable opaque SecretStore key; not secret bytes, a path, or bearer authority
  placement: LOCAL_ONLY | CLOUD_AVAILABLE | RUNTIME_BOUND | EXTERNAL_AGENT_OWNED
}

ServiceRef {
  service_id: ServiceId
}

ChannelThreadRef {
  channel_binding_id: ChannelBindingId
  provider_thread_id: string
}

ChannelReplyTarget {
  runtime_id: RuntimeId
  channel_binding_id: ChannelBindingId
  host_epoch: u64
  provider_message_ref: string # Runtime-local, never replicated or logged
  delivery_id: DeliveryId
  user_request_id: UserRequestId
  status: ACTIVE | CONSUMED | CLOSED | EXPIRED
  consumed_by_provider_event_id?: string
  created_at: Timestamp
  expires_at: Timestamp
  closed_at?: Timestamp
}

BlobRef {
  digest: Sha256Digest
  size_bytes: u64
  media_type: string
}

BackupKeyRef = opaque provider-owned encryption-key reference; never key material

RuntimeEventCursor {
  # Workspace is supplied by the containing WorkspaceBackupManifest.
  origin_runtime_id: RuntimeId
  last_sequence: u64
}

ReplicationReceipt {
  workspace_id: WorkspaceId
  receiver_runtime_id: RuntimeId
  origin_runtime_id: RuntimeId
  origin_sequence: u64
  disposition: EVENT_STORED | POLICY_OMITTED
  event_id?: EventId
  envelope_digest?: Sha256Digest
  policy_revision: u64
  omission_commitment?: string
  received_at: Timestamp
}

ReplicationAggregatePosition {
  workspace_id: WorkspaceId
  receiver_runtime_id: RuntimeId
  entity_type: string
  entity_id: string
  applied_revision: u64
  state_digest?: Sha256Digest
  status: CURRENT | SNAPSHOT_REQUIRED | POLICY_WITHHELD
  updated_at: Timestamp
}

PendingReplicationEvent {
  workspace_id: WorkspaceId
  receiver_runtime_id: RuntimeId
  origin_runtime_id: RuntimeId
  origin_sequence: u64
  event_id: EventId
  state_blob_available: boolean
  pending_reason: STATE_BLOB_MISSING | REVISION_GAP | SNAPSHOT_REQUIRED
  created_at: Timestamp
}

AggregateStateRef {
  blob: BlobRef
  entity_revision: u64
  record_schema_version: u32
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

For `SecretRef`, `provider_ref` is non-secret and conveys no authority by itself. A
SecretStore adapter resolves it only after TrustService issues a matching short-lived
SecretLease. Raw provider-native locators remain inside the SecretStore implementation;
secret bytes and private store locators never enter Agent-visible context, domain events,
or ordinary Workspace backups.

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

BoundedDecision {
  source_event_id: EventId
  summary: string
}

EscalationPolicy {
  max_worker_attempts: u32 # 1..8; includes the first child Attempt and is budget-capped
  fallback_profile_ids: DelegationProfileId[] # tried in this order after recoverable failure
  on_exhaustion: RETURN_TO_LEAD | NEEDS_YOU
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

UsageQuantity {
  quantity: decimal
  unit: string
  currency?: string
  confidence: EXACT | ESTIMATED
}

CostLimit {
  amount_minor_units: u64
  currency: ISO4217Currency
}

TaskSpecProposal {
  objective: string
  task_category?: TaskCategory
  constraints: string[]
  input_refs: PinnedResourceRef[]
  required_outputs: OutputRequirement[]
  acceptance_criteria: AcceptanceCriterion[]
  budget?: BudgetSpec
  delegation_budget_policy?: DelegationBudgetPolicy
  deadline?: Timestamp
}

DelegationBudgetPolicy {
  max_concurrent_children?: u32 # 1..8; can only lower platform limit
  max_host_delegation_depth?: u32 # 0..2; can only lower platform limit
  max_per_attempt?: BudgetSpec
  max_per_task?: BudgetSpec
  max_per_profile?: BudgetSpec
  on_threshold: BudgetThresholdAction
}

NotificationPolicy {
  blockers: ALWAYS | SILENT
  completion: ALWAYS | ON_SUCCESS | SILENT
  failures: ALWAYS | SILENT
}

WarmPolicy {
  host: COLD | TTL | PIN_WHILE_ACTIVE
  native_session: CLOSE_ON_SETTLE | REUSE_IF_SAFE
  capability_hosts: COLD | TTL
  browser_environment: COLD | TASK | WORKSPACE
  local_model: PROVIDER_DEFAULT | KEEP_RECENT_HINT
  ttl_ms?: u64
  max_memory_bytes?: u64
  max_idle_cost?: CostLimit
  triggers: WarmTrigger[]
}

WarmHold {
  warm_hold_id: WarmHoldId
  target: AgentBindingId | DelegationProfileId | CapabilityRef | EnvironmentId
  reason: ACTIVE_TASK | USER_SELECTED | QUOTA_LOW | PREDICTED_FAILOVER | DEADLINE_APPROACHING
  priority: u8
  created_at: Timestamp
  expires_at: Timestamp
  evictable: true
}

LeadFailoverPolicy {
  mode: LeadFailoverMode
  triggers: LeadFailoverTrigger[]
  fallback_agent_binding_ids: AgentBindingId[] # ordered, unique
  max_lead_changes: u32 # 0..3
}

LeadFailoverTriggerObservation {
  trigger: LeadFailoverTrigger
  affected_agent_binding_id: AgentBindingId
  source: ServiceRef # adapter, quota observer, or Runtime observer
  source_observation_ref: string # non-secret stable observation identity
  runtime_incarnation_id?: RuntimeIncarnationId
  observed_at: Timestamp
  expires_at: Timestamp
}

CoworkerInteractionPolicy {
  read_only_work: InteractionDefault
  draft_creation: InteractionDefault
  external_mutation: InteractionDefault
  destructive_action: InteractionDefault
  financial_commitment: InteractionDefault
}

ActionBatchMemberRef {
  action_batch_id: ActionBatchId
  ordinal: u32 # zero-based, unique and strictly less than operation_count
  operation_count: u32 # 1..64, repeated identically on every member
  batch_digest: Sha256Digest # repeated ordered ActionBatch digest
}

ActionBatch {
  action_batch_id: ActionBatchId
  operations: ActionBatchOperation[] # 1..64, ordered
  abort_conditions: PredicateRef[]
  max_duration_ms?: u64
  digest: Sha256Digest # canonical digest of ordered operation envelopes and conditions
}

ActionBatchOperation {
  operation: string
  request: JsonObject
  preconditions: PredicateRef[]
  postconditions: PredicateRef[]
  idempotency_key: string
}

DemonstrationCapturePolicy {
  max_duration_ms: u64
  max_actions: u32
  max_trace_bytes: u64
  allowed_environment_class: BROWSER | DESKTOP
  sensitive_region_policy: DemonstrationSensitiveRegionPolicy
}

PredicateRef = string # bounded, versioned capability/provider predicate; never executable source

SuggestionCandidate { # ephemeral producer output, not durable until accepted by SuggestionService
  proposed_by: ServiceRef
  trigger: SuggestionTrigger
  coworker_id?: CoworkerId
  reason: string
  source_refs: PinnedResourceRef[]
  goal_refs: GoalRevisionRef[]
  proposed_action: SuggestionAction
  proposed_task_spec?: TaskSpecProposal
  expires_at: Timestamp
}

BoundedSuggestionContext {
  workspace_id: WorkspaceId
  coworker_revision_ref?: CoworkerRevisionRef
  source_refs: PinnedResourceRef[]
  goal_refs: GoalRevisionRef[]
  task_refs: TaskId[]
  context_digest: Sha256Digest
}

DelegatedWorkerPolicy {
  capability_allowlist: CapabilityRef[]
  maximum_effect_risk: SAFE | SENSITIVE | HIGH_IMPACT
  filesystem_write_scope: WORKTREE_ONLY | ATTEMPT_PRIVATE | EXPLICIT_SHARED
  external_effects: DENY | REQUIRE_EXISTING_POLICY
  secret_access: NONE | TASK_SCOPED_GRANTS_ONLY
}

DelegatedEnvironmentPolicy {
  placement_preference: PlacementPreference
  isolation: REQUIRED | PREFERRED
  sharing_scope: EnvironmentSharingScope
}

QuotaObservation {
  agent_binding_id: AgentBindingId
  source: string
  observed_at: Timestamp
  expires_at?: Timestamp
  state: QuotaState
  remaining_hint?: UsageQuantity
  reset_at?: Timestamp
}

QuotaState = NORMAL | LOW | EXHAUSTED | UNKNOWN

WorkerPerformanceProjection {
  delegation_profile_id: DelegationProfileId
  task_category: TaskCategory
  sample_count: u64
  verifier_pass_rate?: number
  median_latency_ms?: u64
  median_observed_cost?: UsageQuantity
  retry_rate?: number
  human_intervention_rate?: number
  failure_categories: string[]
  confidence: LOW | MEDIUM | HIGH
}

CoworkerContextPolicy {
  allowed_context_kinds: ContextDocumentKind[]
  max_retrieved_items: u32
  retain_task_summaries: bool
  require_user_confirmation_for_memory: true
}

ContextDocumentMetadata {
  kind: ContextDocumentKind
  owner_ref: ContextOwnerRef
  status: ContextDocumentStatus
  purge_manifest_digest?: Sha256Digest
  purge_target_count?: u32
}

ContextDocumentPurgeTarget {
  replica_ref: string # stable, non-secret replica identity; never a locator
  replica_kind: BLOB | DERIVED_INDEX
  runtime_id?: RuntimeId
  runtime_incarnation_id?: RuntimeIncarnationId
  target_revision_ids: ResourceRevisionId[]
}

ContextDocumentPurgePlan { # immutable Resource-owned subrecord, not a separate aggregate
  manifest_digest: Sha256Digest
  target_count: u32
  targets: ContextDocumentPurgeTarget[]
  sealed_at: Timestamp
}
```

`ContextDocumentMetadata` classifies a versioned Resource; it does not create another
content store or version chain. New documents start `ACTIVE`. `REVOKED` documents are
excluded from future context retrieval while their content remains retained. `DELETION_PENDING`
blocks new reads and waits for all owned blob/index replicas to confirm purge. `DELETED`
retains only Resource/revision identity, digests, provenance, and the tombstone needed by
historical Task references; content bytes and derived provider indexes are unavailable.
`PERSONAL_PROFILE`, `COWORKER_NOTES`, `WORKSPACE_NOTES`,
and `GOAL_NOTES` require USER, COWORKER, WORKSPACE, and GOAL owner refs respectively.
Owner IDs must resolve in the Resource's Workspace (USER must be that Workspace's
authenticated owner). Content edits create ordinary ResourceRevisions.

Only Core ResourceService changes ContextDocument status. Revocation blocks future
retrieval but retains content. Deletion first seals an immutable `ContextDocumentPurgePlan`
over every Core-owned blob/index replica and exact revision set (including a verified empty
set), then commits `DELETION_PENDING` with the manifest digest/count, replicates the
tombstone, and blocks all content reads. Non-secret receipts are retained per plan target;
an acknowledgement must match its exact replica, incarnation, and revisions. `DELETED` is
committed only when receipt count equals the sealed target count and every receipt is
acknowledged; provider-backed sources remain pending until their adapter confirms
deletion. Tombstones, the sealed target manifest, and purge receipts replicate under
Workspace policy. A `replica_ref` is a stable non-secret identity, never a storage locator.
The tombstone retains Resource/revision identity, ancestry, digests, provenance, and
historical Task references without retaining content bytes.

`TaskCategory` is a bounded, owner/lead-proposed routing/evaluation label. If no explicit
category is available, use `OTHER`; Core does not infer it from private Task text. It may
be revised only through a TaskSpecRevision.

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
  retryable_error_codes: ErrorCode[]
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

`ErrorCode` is the enum in `schemas/error-codes.schema.json`. Commands use the common
taxonomy; domain contracts may document which subset they return. A code is not added to
the public taxonomy without updating that registry and its API/schema references.

RetryPolicy alone never makes a consequential operation safe; provider idempotency or
successful reconciliation is also required.

## Assurance and errors

```text
AssuranceLevel = VIEW_ONLY | STEER_SAFE | APPROVE_SAFE |
  APPROVE_SENSITIVE | LOCAL_STRONG
ChannelAction = VIEW | STEER | RESPOND | APPROVE_SAFE | APPROVE_SENSITIVE
```

Domain errors contain a typed code, safe message, retryability, correlation ID, and
optional structured details. Public errors never reveal secret bytes or unauthorized
resource existence.

```text
AGENT_UNAVAILABLE
AGENT_NOT_LEAD_ELIGIBLE
AGENT_SESSION_OVERRIDE_UNSUPPORTED AGENT_NATIVE_CONFIG_CHANGED AGENT_QUOTA_EXHAUSTED
LEAD_FAILOVER_POLICY_INVALID
NOT_FOUND UNAUTHORIZED FORBIDDEN POLICY_DENIED APPROVAL_REQUIRED
WORKSPACE_ARCHIVED WORKSPACE_NOT_QUIESCENT STALE_WORKSPACE_VERSION
INVALID_ARGUMENT INVALID_TRANSITION STALE_VERSION STALE_TASK_VERSION STALE_SPEC_REVISION CONFLICT
ARTIFACT_ARCHIVED INVALID_ARTIFACT_TRANSITION
TASK_NOT_FOUND TASK_TERMINAL INVALID_PLAN PLAN_CYCLE STEP_NOT_READY LEASE_CONFLICT STALE_FENCE
INVALID_RESOURCE_REF RESOURCE_CONFLICT RESOURCE_REVISION_PARENT_MISMATCH TASK_PAUSE_UNSAFE TASK_ALREADY_PAUSED RECOVERY_EXHAUSTED
CONTEXT_DOCUMENT_NOT_ACTIVE CONTEXT_DOCUMENT_OWNER_SCOPE_MISMATCH
DELEGATION_PROFILE_DISABLED DELEGATION_PROFILE_ARCHIVED DELEGATION_PROFILE_INCOMPATIBLE
DELEGATION_PROFILE_OPTIONS_INVALID
DELEGATION_DEPTH_EXCEEDED DELEGATION_CONCURRENCY_EXCEEDED
PLACEMENT_UNAVAILABLE RUNTIME_UNAVAILABLE ENVIRONMENT_UNAVAILABLE
CAPABILITY_UNAVAILABLE CAPABILITY_UNHEALTHY SECRET_UNAVAILABLE RESOURCE_UNAVAILABLE
AUTOMATION_NOT_FOUND AUTOMATION_REVISION_NOT_FOUND AUTOMATION_DISABLED INVALID_TRIGGER
OCCURRENCE_NOT_FOUND TRIGGER_UNSUPPORTED TRIGGER_HOST_UNAVAILABLE
ROUTINE_NOT_FOUND ROUTINE_REVISION_NOT_FOUND ROUTINE_ARCHIVED ROUTINE_INPUT_INVALID
RUNTIME_STOP_BLOCKED RUNTIME_STARTUP_UNAVAILABLE
STALE_CLAIM_EPOCH MISFIRE_LIMIT_EXCEEDED OCCURRENCE_CONFLICT
EFFECT_AMBIGUOUS EFFECT_RECONCILIATION_REQUIRED VERIFICATION_FAILED RATE_LIMITED
INVOCATION_NOT_FOUND INVOCATION_AMBIGUOUS PROVIDER_INPUT_UNSUPPORTED SENSITIVE_INPUT_UNSUPPORTED
USER_REQUEST_NOT_FOUND USER_REQUEST_EXPIRED
APPROVAL_ALREADY_CONSUMED RESOURCE_STALE RESOURCE_LOCATION_UNAVAILABLE
UPLOAD_OFFSET_CONFLICT UPLOAD_EXPIRED BUDGET_EXCEEDED
WORKER_QUALITY_FLOOR_UNMET WORKER_BUDGET_EXCEEDED
DEADLINE_EXECUTION_PRECONDITION_FAILED COWORKER_PAUSED COWORKER_ARCHIVED COWORKER_HAS_ACTIVE_WORK GOAL_ARCHIVED SUGGESTION_EXPIRED
DEMONSTRATION_CAPTURE_LIMIT_REACHED
TIMEOUT DEPENDENCY_UNAVAILABLE UNSUPPORTED_VERSION INTEGRITY_FAILURE INTERNAL
CHANNEL_INGRESS_GAP_CONFIRMATION_REQUIRED
```

The machine-readable registry at `schemas/error-codes.schema.json` is canonical for wire
codes. API and domain schemas reference it; this prose list is generated/checked against
that registry.

The Operator error envelope is specified in `API.md`. `TriggerSpec` is a tagged union
defined in `AUTOMATION.md`; trigger deliveries are deduplicated before Task creation.
