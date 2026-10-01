# Services and Ownership

This file fixes service ownership and dependency direction. Domain method/value contracts
are defined in their owner documents; implementations return typed results and do not
expose database models directly.

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

### CapabilityBroker

Defined in `CAPABILITY-FABRIC.md`. Owns task-scoped capability resolution/grants/activation/invocation coordination.

### RuntimeMesh

Defined in `RUNTIME-MESH.md`. Owns runtime identity/presence/replication/leases/handoff.

### EnvironmentProvider

Defined in `ENVIRONMENTS.md`. Provider boundary; EnvironmentManager owns canonical environment records.

### ArtifactStore

Defined in `ARTIFACTS-EVIDENCE.md`.

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
  claim_occurrence(ClaimOccurrenceRequest) -> AutomationOccurrence
  materialize_task(OccurrenceId) -> TaskId
  settle_occurrence(SettleOccurrenceRequest) -> AutomationOccurrence
}
```

### ChannelAdapter

Defined in `CHANNELS.md`.

### ConnectionService

Owns connection references and lifecycle metadata. It never stores provider credential
bytes. Account-specific authorization and package behavior remain with the external
provider/LitePSM contract.

### ChannelService

Owns ChannelBinding identity, assurance, action permissions, and revocation. It does not
own Conversation or Task truth; it calls ConversationService/TaskService after identity
and authorization checks.

## Internal application services

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

Owns AutomationOccurrence claiming/dedup and invokes AutomationService.materialize_task.

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
TaskService -> StateStore, EventStore, TrustService, PlacementService, CompletionEvaluator
AttemptRunner -> AgentAdapter, CapabilityBroker, EnvironmentManager, LeaseCoordinator, Artifact/Effect services
PlacementService -> RuntimeMesh read models, Agent registry, Trust policy, Environment offers
CapabilityBroker -> LitePSM adapter (contract deferred), TrustService, EffectService, Runtime inventory
RuntimeMesh -> EventStore, BlobStore, StateStore, transport adapter
EnvironmentManager -> EnvironmentProvider adapters, StateStore, EventStore
ArtifactStore -> BlobStore, StateStore, EventStore
TrustService -> StateStore, EventStore, SecretStorePort
CompletionEvaluator -> Verifier registry, StateStore
AutomationService -> StateStore, EventStore, TaskService
TriggerCoordinator -> AutomationService
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
