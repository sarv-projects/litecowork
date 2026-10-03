# Storage LLD

## Storage ports

```text
StateStore
EventStore
BlobStore
```

Domain/application code depends on ports, never SQLite/Postgres/S3 directly.

## Local deployment

- SQLite for relational/domain projections and event index.
- local content-addressed blob directory for artifacts/checkpoints.

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
conversations
conversation_messages

tasks
task_spec_revisions
plan_revisions
steps
attempts

agent_profiles
agent_bindings
agent_sessions

runtimes
runtime_offers
environments
environment_checkpoints
execution_leases

capability_grants
capability_activations
capability_locks
connections
channel_bindings
channel_thread_mappings
channel_event_receipts

artifacts
artifact_versions

effects
evidence
verification_runs
approvals

automations
automation_revisions
automation_occurrences
handoffs
audit_records
pairing_tokens
replication_cursors

domain_events
request_dedup
```

## Required constraints/indexes

```text
UNIQUE task_spec_revisions(task_id, revision)
UNIQUE plan_revisions(task_id, revision)
UNIQUE artifact_versions(artifact_id, version)
FOREIGN KEY artifacts(artifact_id, current_version) -> artifact_versions(artifact_id, version), deferred
UNIQUE domain_events(event_id)
UNIQUE domain_events(origin_runtime_id, origin_sequence)
UNIQUE automation_revisions(automation_id, revision)
UNIQUE automation_occurrences(automation_id, occurrence_key)
UNIQUE request_dedup(principal_id, request_id)
```

Composite foreign keys bind Task revision pointers, Step plan ownership, Step current Attempt, Attempt-to-Step membership, and PlanRevision producer session/Attempt to the same Task. PlanningSession and execution-session rules that span rows are also enforced by the owning service. Execution lease exclusivity is enforced transactionally for active Step ownership. Implementation may use unique partial index where supported or serializable/locking transaction otherwise.

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
- AutomationOccurrence by automation/status/created_at and claim expiry
- ChannelEventReceipt by state/claim_expires_at

## Transactions

Atomic boundaries:
- Workspace create/update/archive + event
- Task + initial TaskSpecRevision + event
- PlanRevision append + current-revision promotion + Step materialization + related events in one transaction
- lease acquisition + Attempt authoritative ownership
- Effect PROPOSED before external mutation
- Artifact row + initial ArtifactVersion v1 + current-version pointer + both create/version events after the blob digest is committed
- Later ArtifactVersion row + artifact current-version pointer + version event after blob digest commit
- Artifact promotion/archive + aggregate version + transition event
- Approval resolution + policy/audit event
- AutomationRevision append + current-revision pointer + event
- AutomationOccurrence claim + pinned revision + Task creation reference

Do not keep DB transaction open across network/provider calls.

## Optimistic concurrency

Mutable aggregate commands include `expected_version` where user/process races are possible. Mismatch returns conflict and caller re-reads/reconciles. Artifact SQL constraints additionally require sequential content versions, legal Library transitions, aggregate-version increments, immutable ArtifactVersion rows, and no new content after archive.

## Blob storage

Blob key is content digest. Writes:
1. stream temporary object
2. compute digest
3. fsync/commit provider object
4. create immutable manifest
5. reference from ArtifactVersion/Event only after commit

Garbage collection must retain any blob referenced by durable records, pending replication manifests, checkpoints needed for recovery or retention policy.

## Migrations

- monotonically numbered schema migrations.
- migrations are forward-only in production; rollback is restore/forward-fix.
- runtime refuses to open a database newer than its supported schema major.
- managed rolling upgrades use expand/migrate/contract pattern.
