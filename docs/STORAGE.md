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

artifacts
artifact_versions

effects
evidence
verification_runs
approvals

automations
automation_occurrences

domain_events
request_dedup
```

## Required constraints/indexes

```text
UNIQUE task_spec_revisions(task_id, revision)
UNIQUE plan_revisions(task_id, revision)
UNIQUE artifact_versions(artifact_id, version)
UNIQUE domain_events(event_id)
UNIQUE domain_events(origin_runtime_id, origin_sequence)
UNIQUE automation_occurrences(automation_id, scheduled_key)
UNIQUE request_dedup(principal_id, request_id)
```

Execution lease exclusivity is enforced transactionally for active Step ownership. Implementation may use unique partial index where supported or serializable/locking transaction otherwise.

Indexes required on:
- Task by workspace/status/updated_at
- Step by task/status
- Attempt by task/step/status
- Effect by task/state
- Approval by workspace/status
- Artifact by task/current_version
- Event by workspace/HLC and entity stream
- Runtime by workspace/availability

## Transactions

Atomic boundaries:
- Task + initial TaskSpecRevision + event
- Plan current-revision promotion + event
- lease acquisition + Attempt authoritative ownership
- Effect PROPOSED before external mutation
- ArtifactVersion row + blob manifest after blob digest committed
- Approval resolution + policy/audit event
- AutomationOccurrence claim + Task creation reference

Do not keep DB transaction open across network/provider calls.

## Optimistic concurrency

Mutable aggregate commands include `expected_version` where user/process races are possible. Mismatch returns conflict and caller re-reads/reconciles.

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
