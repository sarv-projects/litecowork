# Services and Ownership

This file fixes service ownership and dependency direction. Domain method/value contracts
are defined in their owner documents; implementations return typed results and do not
expose database models directly.

### WorkspaceService

```text
create(CreateWorkspaceRequest) -> Workspace
get(WorkspaceId) -> Workspace
list(WorkspaceQuery) -> Page<WorkspaceSummary>
update_replication_policy(UpdateWorkspacePolicyRequest) -> Workspace
archive(ArchiveWorkspaceRequest) -> Workspace
```

The authenticated owner is the only Workspace principal in v1. Policy updates are prospective; they do not erase already replicated data. Archive requires all Tasks to be terminal and all Automations disabled, then makes the Workspace read-only. The service rejects every domain mutation for archived Workspaces, including Task changes, capability grants/activation, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel work. Authorized reads and Artifact/Resource downloads remain available.

## Public application ports

### ConversationService

```text
create_conversation(CreateConversationRequest) -> Conversation
append_message(AppendMessageRequest) -> ConversationMessage
get_conversation(ConversationId) -> ConversationView
list_messages(ConversationId, Cursor?, Limit) -> Page<ConversationMessage>
map_channel_thread(MapChannelThreadRequest) -> ConversationId
```

Owns Conversation, ConversationMessage, and channel-thread mapping. Append and
Task-materialization requests originating from one user message use one transaction so
the Task cannot exist without its source message or vice versa.

### TaskService

Defined in `TASK-RUNTIME.md`. Owns Task, TaskSpecRevision, PlanRevision, Step and high-level Attempt orchestration.

### AgentAdapter

Defined in `AGENT-FABRIC.md`. Protocol boundary to external agents.

### AgentBindingService

```text
list_profiles(WorkspaceId, RuntimeId?, Cursor?, Limit) -> Page<AgentProfileView>
list_bindings(WorkspaceId, AgentBindingQuery, Cursor?, Limit) -> Page<AgentBindingView>
get_binding(AgentBindingId) -> AgentBindingView
create_binding(CreateAgentBindingRequest) -> AgentBinding
enable_binding(AgentBindingId, expected_version) -> AgentBinding
disable_binding(AgentBindingId, expected_version) -> AgentBinding
```

Profiles are Runtime-discovered inventory; bindings are durable Workspace authorization
records. Creation requires a currently observed profile and starts disabled. Enablement
checks TrustService and Runtime availability. Disablement blocks new admission without
rewriting already admitted Attempts or sessions.

### CapabilityBroker

Defined in `CAPABILITY-FABRIC.md`. Owns task-scoped capability resolution/grants/activation/invocation coordination.

### RuntimeMesh

Defined in `RUNTIME-MESH.md`. Owns runtime identity/presence/replication/leases/handoff.

### EnvironmentProvider

Defined in `ENVIRONMENTS.md`. Provider boundary; EnvironmentManager owns canonical environment records.

### ArtifactStore

Defined in `ARTIFACTS-EVIDENCE.md`. It commits a digest-verified initial version on create; every later version uses expected Artifact aggregate version. Library promotion and archive use the same optimistic concurrency boundary and emit their events atomically.

### TrustService

Defined in `TRUST.md`.

### Verifier

Defined in `ARTIFACTS-EVIDENCE.md`.

### AutomationService

```text
interface AutomationService {
  create(CreateAutomationRequest) -> Automation
  update(UpdateAutomationRequest) -> Automation
  pause(AutomationId, expected_version) -> Automation
  resume(AutomationId, expected_version) -> Automation
  disable(AutomationId, expected_version) -> Automation
  get(AutomationId) -> AutomationView
  list(WorkspaceId, Cursor?, Limit) -> Page<AutomationSummary>
}
```

### ChannelAdapter

Defined in `CHANNELS.md`.

### ConnectionService

Owns connection references and lifecycle metadata. It never stores provider credential
bytes. Account-specific authorization and package behavior remain with the external
provider/LitePSM contract. Provider-owned setup creates or updates the Connection after
its authentication flow; Operator API exposes only normalized metadata, status, and
disconnect. Provider-specific setup UI, callbacks, and credential exchange are outside
the Operator API and remain deferred with the integration contract.

```text
list(WorkspaceId, ConnectionQuery, Cursor?, Limit) -> Page<ConnectionSummary>
get(ConnectionId) -> ConnectionView
disconnect(ConnectionId, expected_version) -> ConnectionView
```

### ChannelService

Owns ChannelBinding identity, assurance, action permissions, and revocation. It does not
own Conversation or Task truth; it calls ConversationService/TaskService after identity
and authorization checks.

```text
list_bindings(WorkspaceId, ChannelBindingQuery, Cursor?, Limit) -> Page<ChannelBindingSummary>
get_binding(ChannelBindingId) -> ChannelBindingView
update_allowed_actions(ChannelBindingId, expected_version, ChannelAction[]) -> ChannelBindingView
revoke_binding(ChannelBindingId, expected_version) -> ChannelBindingView
```

Channel-provider setup creates the binding from authenticated provider identity; the
owner then reviews its allowed actions. The provider, not the caller, supplies the
authenticated identity and assurance level.

## Internal application services

### PlanningCoordinator

Coordinates a TaskService-authorized PlanningAssignment with AgentSessionSupervisor. It may start only a LEAD_PLANNING session for the current lead binding and TaskSpecRevision; it does not author or promote PlanRevision records.

### AgentSessionSupervisor

Owns AgentSession lifecycle for both LEAD_PLANNING and STEP_EXECUTION sessions. It invokes the negotiated AgentAdapter and emits normalized lifecycle outcomes; it cannot mutate Task state directly.

### AttemptRunner

Responsibilities:
- prepare Attempt
- start AgentSession
- feed TaskPacket/context
- consume normalized agent events
- route host gateway calls
- checkpoint
- settle Attempt

Must not decide intellectual plan quality.

### PlacementService

```text
select(PlacementRequest) -> PlacementDecision
```

Uses hard constraints first: runtime availability, agent compatibility, required resources/secrets, environment offer, policy, failover class. Preferences/cost may break ties but never violate hard constraints.

### LeaseCoordinator

Only component allowed to acquire/renew/release authoritative ExecutionLease records.

### CompletionEvaluator

Collects criteria, outputs, child status, approvals and open Effects; invokes Verifier registry; asks TaskService for final transition.

### EffectService / EffectReconciler

Own effect ledger transitions and post-failure reconciliation.

### EvidenceService

Append-only Evidence writer. No update/delete except administrative retention process that preserves audit semantics.

### EnvironmentManager

Chooses provider adapter, persists canonical Environment state, enforces cleanup/checkpoint rules.

### ProjectionService

Consumes domain events and builds Task/Conversation/LiveDesk/Notification projections. Projection failure never mutates domain truth.

### TriggerCoordinator

```text
claim(ClaimOccurrenceRequest) -> OccurrenceClaim
request_manual_occurrence(ManualOccurrenceRequest) -> AutomationOccurrence
materialize_task(OccurrenceId, claim_epoch) -> TaskId
settle_occurrence(SettleOccurrenceRequest, claim_epoch) -> AutomationOccurrence
```

Derives the canonical key, pins the current AutomationRevision in the claim transaction,
checks enabled/overlap policy, and fences late workers by claim_epoch. Task creation and
its occurrence reference commit atomically. It invokes TaskService; it does not own Task
execution.

### NotificationService

Projects Task/Approval/Automation outcomes to operator surfaces/channels according to notification policy. Notifications are not domain completion truth.

### AuditService

Appends authorization, approval, Runtime pairing/revocation, secret-lease, and sensitive
Effect records. It has no update API. Retention must preserve evidence needed for active
Effect reconciliation and recovery.

### LitePSM adapter boundary

CapabilityBroker depends on one outbound adapter implementing the future LitePSM
contract. The base URL is selected in `CAPABILITY-FABRIC.md`. Method names, payloads,
authentication, retries, version negotiation, package manifests, and activation details
are deliberately unspecified until LitePSM's authoritative interface is supplied. No
other LiteCowork service may call LitePSM directly.

## Infrastructure ports

### StateStore

Transactional storage for current aggregate state and rebuildable projections. Aggregate
mutation and its DomainEvent append commit atomically. It exposes unit-of-work/transaction
scope to owning services, not raw SQL to domain code.

### EventStore

Append/read the immutable per-origin domain event stream. Replication acceptance validates
origin authority and aggregate transition before a remote event updates canonical state.

### EventBus

In-process/local pub-sub for committed event delivery to projections; not source of truth.

### BlobStore

Content-addressed immutable blob storage.

### SecretStorePort

Stores actual secret bytes; domain sees SecretRef/leases only.

### Clock

Injectable time source for deterministic tests.

### IdGenerator

Injectable opaque ID generator.

## Transaction rule

An application command performs validation before the transaction where possible, then
rechecks mutable versions, authorization, and lease/fence inside the transaction. The
transaction commits aggregate state, its DomainEvent(s), and request-dedup result
together. External network/provider calls never run while holding a StateStore
transaction. If a multi-record invariant spans Task/Step/Attempt/Lease, one owning
application command coordinates service owners inside a single transaction scope.

## Dependency constraints

```text
ConversationService -> StateStore, EventStore
WorkspaceService -> StateStore, EventStore, TrustService
TaskService -> StateStore, EventStore, TrustService, PlacementService, CompletionEvaluator, PlanningCoordinator
AgentBindingService -> RuntimeMesh read models, AgentAdapter discovery, TrustService, StateStore, EventStore
PlanningCoordinator -> AgentSessionSupervisor, AgentAdapter, TaskService read port
AgentSessionSupervisor -> AgentAdapter, StateStore, EventStore
AttemptRunner -> AgentSessionSupervisor, CapabilityBroker, EnvironmentManager, LeaseCoordinator, Artifact/Effect services
PlacementService -> RuntimeMesh read models, Agent registry, Trust policy, Environment offers
CapabilityBroker -> LitePSM adapter (contract deferred), TrustService, EffectService, Runtime inventory
RuntimeMesh -> EventStore, BlobStore, StateStore, transport adapter
EnvironmentManager -> EnvironmentProvider adapters, StateStore, EventStore
ArtifactStore -> BlobStore, StateStore, EventStore
TrustService -> StateStore, EventStore, SecretStorePort
CompletionEvaluator -> Verifier registry, StateStore
AutomationService -> StateStore, EventStore
TriggerCoordinator -> AutomationService, TaskService, StateStore, EventStore
ProjectionService -> EventStore/read stream + projection stores
ChannelService -> TrustService, ConversationService, TaskService
ConnectionService -> TrustService, SecretStorePort, external provider adapter
AuditService -> StateStore, EventStore
```

Forbidden:
- AgentAdapter mutating Task/Artifact/Effect DB directly
- UI mutating StateStore directly
- provider adapters emitting user-visible completion without domain service transition
- LitePSM package metadata bypassing TrustService for activation
- AgentAdapter or provider writing domain tables without the owning service
- a replicated event bypassing origin authorization, revision, or fencing checks
- a notification changing Task/Approval/Automation state
- any service holding a storage transaction open during a network/provider call

## Error and idempotency contract

Mutating commands accept a `RequestId` scoped to the authenticated principal. The same
ID and request digest returns the prior outcome; reusing it with a different digest is
`CONFLICT`. Commands subject to races require an expected aggregate version. Services
return the common typed error from `SCHEMAS.md`; adapters map provider-specific failures
without leaking secrets.
