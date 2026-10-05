# Implementation Architecture

## Repository shape

```text
apps/
  litecoworkd/             # headless Runtime/service executable
  operator-desktop/        # Tauri + React UI, tray, later Quick Entry

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
    agent-host-supervisor/
    lifecycle/
    dependency-planner/
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
    litespm/

  environments/
    core/
    local/
    worktree/
    container/
    remote/

  artifacts/
  trust/
  verification/
  routines/
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

Do not create every crate before needed; this is an ownership map. Early implementation
may combine closely related crates while preserving dependency boundaries.
`apps/operator-web`, Postgres and S3 adapters are future optional shapes, not V1
clients/providers; V1 is desktop-first, followed by cloud Runtime and then remote Runtime.

## Dependency direction

```text
apps/operator UI
      -> operator-api client

application/runtime services
      -> domain + ports

adapters (ACP, LiteSPM, SQLite, S3, channels)
      -> ports/domain types

domain
      -> common primitives only
```

Forbidden dependencies:
- domain -> Tauri/React
- task domain -> ACP/A2A implementation
- task domain -> LiteSPM implementation
- any domain -> SQLite/Postgres concrete driver
- AgentAdapter -> direct Task database mutation
- capability adapter -> UI

## Delivery order and story execution

Stage measurements include CapabilityHost activation/binding overhead, daemon/task
startup, and event-to-UI latency. Freeze numeric Stage 1 targets from actual recorded
hardware/workload baselines; do not guess them.

The contract-level release order is **desktop/local first, cloud continuation second,
remote Runtime third**. The detailed Agile stories, stack qualification, provider research,
RAG, UI acceptance, real-work tests and release gates are in
[`../implementation/README.md`](../implementation/README.md). The current authority is
still this document plus `ARCHITECTURE.md` and each domain contract; a backlog summary
never overrides them.

Delivery gates are:

1. Foundation, verified toolchain, contract CI, durable local storage, authenticated API
   and independent Runtime lifecycle.
2. Installable local desktop vertical slice: Workspace/Resources, native harness, durable
   Conversation and Task, attempt-free planning, accepted Steps, Attempts, recovery,
   mediated capability/effect/evidence and one independently verified Artifact.
3. Complete finalized local feature surface: workforce/delegation, files/folders/ZIP and
   RAG, local models, browser/office/research, Coworker/context/Goals/Suggestions,
   Workbench, accessibility, responsibilities and skills, each qualified with real
   providers and useful user cases.
4. Cloud deployment, one qualified human channel, replication/fencing/continuation and
   restore with no DB-file or native private-state sync.
5. Remote Runtime enrollment, placement, authority, availability and recovery.
6. Production installer/security/performance/real-work qualification and owner release.

Begin with contract/toolchain and stack spikes, not an assumption that ACP fits every
native harness. Implement native harness adapters through qualified full-fidelity
interfaces; an ACP adapter is used only when supported semantics are sufficient. RAG
parsers/indexes remain provider implementations. A Python sidecar is isolated and
resource-limited; it never owns Task or Trust state. Do not add Postgres/Kubernetes or
many small crates before a measured need. Cloud qualification uses a long-lived isolated
Runtime, not a serverless function as a presumed equivalent.

The roadmap stories split lifecycle/resource, Conversation/Task, Trust/capabilities,
UI, cloud and remote work into reviewable increments. Each domain owner must implement
its own API/event/schema/SQL changes even when the cross-contract inventory has an
architecture guardian. Test fixtures establish deterministic control-plane behavior;
provider release claims require system tests against the provider and real-user acceptance.

## Documentation and implementation gates

Before a domain is implemented, its canonical schema, legal transitions/owner, command/event contract, authorization rule, failure/recovery behavior, UI projection, and compatibility policy must agree across its owner document and the shared schema/API/event/storage references. LiteSPM package/API/manifest details remain intentionally deferred to the configured LiteSPM service; LiteCowork specifies its internal Broker/Supervisor ports, normalized Runtime-local host/readiness view, scoped activation references, and stable user-facing capability references without inventing LiteSPM's wire contract.

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
- LiteSPM client wire/package contract

Upgrade principle: in-progress Tasks remain pinned to immutable spec/plan/capability/artifact revisions; runtime upgrades must not silently mutate their semantics.
