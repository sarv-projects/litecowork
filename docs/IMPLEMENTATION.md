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

Do not create every crate before needed; this is an ownership map. Early implementation may combine closely related crates while preserving dependency boundaries.

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

## Stage 0 — contract repair

Before runtime implementation, validate the frozen `litecowork.*` Gateway namespace,
Conversation AgentSession and retry contracts, Task pause/recovery semantics, canonical
error registry, typed event payload schemas, verification-to-spec binding, external
ArtifactContent union, ApprovalUse consumption, Resource identity/location split, and
versioned Needs You item identity. Store AgentSession scope/Runtime provenance and
CapabilityActivation scope in canonical records; keep native AgentSession and provider
handles in Runtime-local immutable bindings, with derived host-use counts and settlement-
gated release. Register RuntimeIncarnations before accepting records that reference them.
Validate the persistent Environment preview/create digest contract against the OpenAPI
request schemas before adding its Operator UI.
The schema/API/event/storage validators must run in CI.

## Stage 1 — local substrate

Build separate Operator and `litecoworkd` executables with single-instance locking,
incarnation recovery, startup policy, safe drain, sleep/wake coordination, Workspace, explicit Workspace instructions/roots, Resource and
World Index, deterministic local resource search, Event/SQLite/Blob stores, Operator API,
verified encrypted WorkspaceBackupManifest create/restore, and local-only UI. Restore
into an empty installation must register a new Runtime identity and must not revive old
leases. Whole-machine observation remains opt-in and separately isolated.
Measure daemon startup/idle CPU/RAM, Task creation, resource search, event-to-UI latency,
crash recovery, and CapabilityHost activation/binding overhead against a deterministic
LiteSPM adapter fixture. Record provider-start time separately so the fixture is not
misreported as real provider startup. Freeze numeric Stage 1 targets from that measured
baseline before Stage 1 exits; do not guess targets without measurements. Stage 3 then
adds real LiteSPM/MCP activation and provider-recovery latency without changing the
control-plane measurements into an upstream service guarantee.

Stage 1 also implements Workspace-persistent Environment preview/list/create/suspend/resume/
destroy commands behind provider conformance, with explicit budget enforcement policy,
preview digest revalidation, idempotent provisioning recovery, and safe provider-private
locator handling. Operator UI surfaces the reviewed estimate, enforcement strength,
retention and blockers. Needs You becomes a deduplicated query projection over canonical
Approval, UserRequest and actionable Task blockers, with stable item IDs and owner-routed
actions. No inbox aggregate command mutates its source record.

## Stage 2 — first agent

Build lazy AgentHost ensure/retain/release and idle shutdown, then Conversation-scoped chat and Task-scoped planning plus Attempt execution, one ACP
AgentAdapter, Task/Event/SQLite stores, ResumePacket, and minimal desktop
Conversation/Task/Live Desk.

Acceptance: simple conversation has a Conversation AgentSession and no Task; a Task starts
with an attempt-free planning session, promotes a PlanRevision, materializes Steps, and
executes one Step; kill the worker and resume from portable Task state. Confirm no Live
Desk lane appears before a Step Attempt exists.

## Stage 3 — first capability

Add the LiteCowork Gateway, CapabilityHostSupervisor and normalized host view, internal
LiteSPM client port, one MCP capability, CapabilityInvocation, Effect/Evidence,
DependencyService, and a deterministic Verifier.
LiteSPM wire/API and package contracts remain intentionally out of scope until its
service authority is available.

Acceptance: both PACKAGE_COMPONENT and MCP_SKILL CapabilityRefs pass their normalized
schemas; a Task pins the exact digest; read-only and asynchronous invocations recover
after AgentSession loss; approval and Effect semantics remain independent; a changed
input revision invalidates dependent outputs/evidence idempotently; provider failures
respect call-admission circuits; package restart limits belong to LiteSPM. Record local capability search/activation and
invocation recovery baselines for the first provider. Prove that compatible scoped
Activations share one safe host with a derived use count, while incompatible isolation
contexts receive separate hosts and every call still enforces its own Grant/fence.

## Stage 4 — delegated worker profiles

Add versioned DelegationProfiles, adapter-negotiated session options, native versus
host-delegation reporting, accepted-Plan Step admission, bounded TaskPacket and
ResultEnvelope, child-scoped grants/leases/budgets, isolated worktrees, and independent
verification. Support at least two qualified harnesses and multiple profiles for one
AgentBinding before making heterogeneous routing claims. Add deterministic selection,
cost/latency/quality preferences, bounded retries, verification-based escalation, and
explicit profile selection with no silent fallback.

Acceptance: a lead can delegate a READY Step to another harness and to a second profile
of the same harness; each child has independent provenance and authority; profile edits
affect only future Attempts; worker output cannot complete a Step without its configured
verification; failed verification creates a new bounded Attempt; native children are not
misrepresented as LiteCowork Attempts. Measure verified outcomes, premium usage, total
known cost, latency, retries, and user rescues against an all-premium baseline.

## Stage 5 — Runtime Mesh and continuation

Add pairing, presence, replication, leases/fencing, blob replication, and explicit handoff.
Before cloud GA, verify backup restore on a new Runtime, test Workspace event/blob
recovery, and prove old lease epochs cannot return. Only then implement automatic safe
failover.

## Stage 6 — warmth, shared Environments, and deadline-sensitive execution

Implement independently owned host/session/capability/Environment warm policies, bounded
prewarming without model invocation, pressure-based eviction, Environment sharing scopes,
private worktree defaults, and browser Environment control leases. Add deadline-sensitive
preflight as best-effort scheduling, a semantics-preserving execution ladder from
structured APIs through browser/computer use, bounded ActionBatch operations, and human
takeover for actions requiring user authority. No hard real-time guarantee is made.

Acceptance: warm resources never bypass fresh auth, Trust, lease, config, or Environment
checks; one browser control lease fences concurrent actors; critical work fails before
the time window when a required precondition is unavailable; fallback preserves
Effect/Evidence and approval semantics.

## Stage 7 — Coworker and responsibility surfaces

Add Coworker identity/revisions, one explicitly selected primary Coworker, passive Goals,
provenance-backed Suggestions, Resource-backed editable context, Home/Coworker/Work/
Needs You/Subagents surfaces, responsive layouts, reduced-motion behavior, and outcome-
based progress projections. Suggestions require user action; Goal progress derives from
verified Task outcomes; context edits use Resource revision conflict handling.

## Stage 8 — channels and automation

One ChannelAdapter (Telegram or Email) through same Conversation/Task store.

Add Routines and pinned RoutineRevisions, remote channels and notification preferences/
delivery, then multi-trigger schedule/manual/webhook providers that create deduplicated
ordinary Tasks. Verify stable cursors across edits, misfire behavior after sleep/restart,
and Hub-triggered work waiting for local execution dependencies. Broader trigger variants
remain disabled until their providers pass conformance.

## Stage 9 — richer surfaces and reusable work

Add the MCP Apps host, MCP Skills extension provider, user-reviewed SkillProposal draft /
redaction flow with LiteSPM publication only after its contract is available, richer
Workbench providers, and optional isolated machine observation, browser, and computer-use
integrations.

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
