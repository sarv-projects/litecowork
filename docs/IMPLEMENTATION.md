# Implementation Architecture

## Repository shape

```text
apps/
  desktop/                 # Tauri + React operator UI

crates/
  domain/
    common/
    workspace/
    conversation/
    task/
    artifact/
    effect/
    evidence/

  runtime/
    supervisor/
    workspace-service/
    task-service/
    planning-coordinator/
    agent-session-supervisor/
    scheduler/
    attempt-runner/
    completion/
    reconciliation/

  mesh/
    identity/
    presence/
    replication/
    leases/
    handoff/
    transport/

  agents/
    core/
    acp/
    a2a/
    cli/

  capabilities/
    core/
    gateway/
    broker/
    litepsm/

  environments/
    core/
    local/
    worktree/
    container/
    remote/

  artifacts/
  trust/
  verification/
  automation/
  channels/
  events/
  storage/
    core/
    sqlite/
    postgres/
    blob-local/
    blob-s3/
  operator-api/
```

Do not create every crate before needed; this is an ownership map. Early implementation may combine closely related crates while preserving dependency boundaries.

## Dependency direction

```text
apps/operator UI
      -> operator-api client

application/runtime services
      -> domain + ports

adapters (ACP, LitePSM, SQLite, S3, channels)
      -> ports/domain types

domain
      -> common primitives only
```

Forbidden dependencies:
- domain -> Tauri/React
- task domain -> ACP/A2A implementation
- task domain -> LitePSM implementation
- any domain -> SQLite/Postgres concrete driver
- AgentAdapter -> direct Task database mutation
- capability adapter -> UI

## Milestone 1 — durable local vertical slice

Build only:
- local `litecoworkd`
- WorkspaceService with explicit replication policy and archive lifecycle
- Task/Event/SQLite stores
- Task-scoped LEAD_PLANNING session before Steps/Attempts
- one ACP AgentAdapter
- LiteCowork Gateway
- LitePSM client
- one MCP capability
- Artifact/Effect/Evidence
- one deterministic Verifier
- minimal desktop Conversation/Task/Live Desk

Acceptance: create a local-only Workspace; create a Task; start an attempt-free lead planning session; promote its PlanRevision and materialize Steps; execute one Step; kill the worker and resume from portable Task state. Confirm no Live Desk lane appears before a Step Attempt exists.

## Milestone 2 — heterogeneous agents

Add host delegation, bounded TaskPacket/ResultEnvelope, child lifecycle, worktree isolation and independent verifier.

## Milestone 3 — Runtime Mesh/cloud

Add pairing, presence, replication, leases/fencing, blob replication, explicit handoff. Only then implement automatic safe failover.

## Milestone 4 — remote channels

One ChannelAdapter (Telegram or Email) through same Conversation/Task store.

## Milestone 5 — automation

Schedule trigger -> deduplicated occurrence -> ordinary Task.

## Documentation and implementation gates

Before a domain is implemented, its canonical schema, legal transitions/owner, command/event contract, authorization rule, failure/recovery behavior, UI projection, and compatibility policy must agree across its owner document and the shared schema/API/event/storage references. LitePSM package/API/manifest details remain intentionally deferred to the configured LitePSM service; LiteCowork specifies only its internal broker port and stable user-facing capability references.

## Definition of done for a domain feature

A feature is done only when:
1. schema exists
2. legal state transitions defined
3. owning service command exists
4. event(s) defined
5. authorization rule defined
6. failure/recovery behavior defined
7. UI projection/state defined if user-visible
8. unit + contract/integration tests exist
9. observability fields/metrics exist where operationally relevant

## Versioning

Version independently:
- DB schema
- domain event schemas
- Operator API
- Mesh protocol
- AgentAdapter contract
- EnvironmentProvider contract
- Capability Gateway contract
- LitePSM client contract

Upgrade principle: in-progress Tasks remain pinned to immutable spec/plan/capability/artifact revisions; runtime upgrades must not silently mutate their semantics.
