# LiteCowork Architecture

**Status:** vNext target architecture. This file is the current system authority.
Supporting documents may add detail but must not contradict it. Decision records in
`docs/adr/` explain why a boundary exists; they do not silently amend this contract.
The former `ARCH/` corpus is preserved locally under the ignored `archives_docs/` tree
for historical reference only.

## 1. Product definition

LiteCowork is a **durable execution workspace around external agents**. Its central object is
the user's durable Task, not a model, agent, browser, computer, MCP server, or device.

```text
User intent → Conversation → durable Task
             → Agent(s) + Capability(s) + Runtime(s) + Environment(s)
             → Attempts → Effects and Artifacts → Evidence and Verification → Outcome
```

A compatible agent can be a worker; capabilities can be discovered and activated as
needed; any LiteCowork Runtime can host an Attempt; compatible environments can be
selected for that Attempt; and other agents can collaborate. None of these owns the
user's Task.

## 2. Load-bearing invariants

1. **Task truth survives replaceable workers.** A Task is durable; an AgentSession and
   an Attempt are replaceable. Correctness never depends on preserving a private agent
   transcript or teleporting a live process.
2. **External agents own reasoning.** LiteCowork stores and presents plans, checks
   structure and policy, and coordinates execution. It does not provide a second
   reasoning loop, planner, or hidden Goal Keeper.
3. **Keep identities distinct.** Task, Step, Attempt, Agent, AgentSession, Runtime,
   Environment, Capability, Effect, Artifact, Evidence, and ExecutionLease are separate
   concepts with separate owners.
4. **Govern only mediated effects.** Core Trust, authorization tickets, audit, and
   verification govern calls routed through LiteCowork. An agent's native tools and
   effects remain under that agent's policy and carry reported or observed provenance;
   LiteCowork must not claim a Core receipt for them.
5. **Discover progressively.** Workers receive a small LiteCowork Gateway surface and
   discover only the capability or skill needed. An installed catalog is not a grant.
6. **Replicate domain truth, not databases.** Runtimes exchange versioned domain events,
   immutable artifacts, resource manifests, and fenced ownership. They do not sync
   SQLite files or copy arbitrary agent state.
7. **Prefer structured operations.** Core-mediated API and protocol operations precede
   browser or OS accessibility where suitable. Native agent tools remain the agent's
   choice and are identified honestly.
8. **Completion requires evidence.** Worker self-report is a proposal. Completion is
   determined from required outputs, effect reconciliation, approvals, and acceptance
   evidence.
9. **Direct attachment cannot bypass Core guarantees.** Use direct capability tools only
   when the provider can enforce the required grant, Effect, idempotency, and fencing
   contract. Otherwise route consequential calls through the LiteCowork Gateway.

## 3. Canonical concepts and ownership

| Concept | Meaning and owner |
|---|---|
| Conversation | User-visible exchange across surfaces. An ordinary question need not create a Task. |
| Task | Durable outcome the user wants, owned by the Task Runtime. |
| TaskSpecRevision | Immutable revision of objective, constraints, outputs, criteria, approvals, and budget. |
| PlanRevision | Agent-proposed, versioned plan stored by Core; Core validates shape and policy but does not invent strategy. |
| Step | Semantic unit from the current plan. |
| Attempt | One worker's execution of one Step, admitted and tracked by the Task Runtime. |
| AgentProfile / AgentBinding | Discovered agent and its negotiated host binding, owned by Agent Fabric. |
| AgentSession | Agent-specific reasoning session; optional native session handles are optimizations, not Task truth. |
| Runtime | A running `litecoworkd` instance, with identity, role, presence, and resource offers. |
| Environment | The actual place an Attempt acts: local workspace, worktree, container, VM, browser, desktop, or remote sandbox. |
| CapabilityRef | Reference to a capability package/service/version, resolved through the independent LitePSM ecosystem. |
| CapabilityGrant | Task/Attempt-scoped authorization for an exact capability and operation. |
| ExecutionLease | Fenced authority for one Runtime to own an Attempt at a particular epoch. |
| Effect | A proposed or executed real-world consequence with reconciliation state. |
| Artifact / ArtifactVersion | Durable output identity, immutable content version, storage reference, and provenance. |
| Evidence | References and observations that support a claim about an Effect or acceptance criterion. |

The portable Task checkpoint is the recovery source of truth. Agent session snapshots
and environment snapshots may accelerate resume but are never required for correctness.

## 4. Core and adapter boundary

### LiteCowork Core owns

- Conversation identity and the durable Task model: revisions, plan/step projection,
  Attempts, scheduling admission, budgets, cancellation, and ResumePackets.
- Agent Fabric contracts and host-created delegation lifecycle, without taking over an
  agent's internal subagent system or reasoning.
- LiteCowork Capability Gateway, CapabilityBroker, scoped grants, activations, and LitePSM
  client integration.
- Runtime identity, pairing, presence, event/artifact replication, execution leases,
  fencing, handoff, and failover coordination.
- EnvironmentProvider contract and Attempt placement, not domain-specific intelligence.
- Trust decisions, approvals, secret references/leases, and audit for Core-mediated
  calls.
- Durable event journal, Artifact/Effect/Evidence records, verification orchestration,
  automation trigger-to-Task creation, and operator projections/API.

These responsibilities may start as a modular monolith. They are not a mandate to
create a network of internal microservices or dozens of crates before a vertical slice
works.

### Capability/provider territory

Office operations, browser automation, computer-use reasoning, coding intelligence,
search/RAG, connectors, cognitive memory, model routing, workflow engines, machine-wide
observation, and domain-specific research are not first-party Core domains. They may be
provided by an external agent, MCP server, skill, plugin, connector, environment
provider, or other compatible service. Removing one should remove that category of
work, not break Task durability or runtime coordination.

### LitePSM is independent

LitePSM owns package ecosystem truth and lifecycle. LiteCowork owns only task-specific
references, grants, activations, and offers. A listing is not trusted code, an installed
package, an account connection, or an authorization grant. The selected LitePSM service
base URL is `https://litepsm.sarveshbh-2022.workers.dev/`. Its API, authentication,
manifest, package taxonomy, and install/activation contract remain owned by LitePSM and
are deliberately not specified here.

## 5. LiteCowork Capability Gateway

Every controlled worker receives one small permanent MCP gateway, not the entire
installed tool inventory. The initial primitive family is:

```text
litecowork.capabilities.search / describe / activate / invoke / list_active
litecowork.skills.search / load
litecowork.agents.search / delegate / status / message / cancel
litecowork.task.read / update_plan / finish
litecowork.artifacts.read / publish
litecowork.user.ask
```

Resolve an exact package version and digest through LitePSM, check binding compatibility,
scope, policy, and budget, then create a task-scoped grant. Direct attachment is allowed
only when the provider can enforce the required grant and Effect/fence contract. Route
consequential calls through the Gateway when those guarantees cannot be enforced on the
direct path. If attachment can change only at a session boundary, use the proxy for the
current turn and attach natively at the next safe boundary. Never rewrite a discovered
agent's private configuration or copy host skills into its native directories.

## 6. Agent, Runtime, and Environment fabrics

### Agent Fabric

Negotiate features per binding; never infer support from an agent's name. The adapter
surface covers discovery/probe, authentication where supported, session start/resume,
send/steer/interrupt/cancel, capability/context attachment, event streaming, optional
snapshot, and close. Prefer ACP when supported, then A2A, native SDK/API, structured CLI,
or terminal adaptation where qualified. These protocols serve different boundaries and
none is universal.

The lead agent proposes decomposition and worker choice. Core checks eligibility,
permissions, placement, isolation, budget, concurrency, depth, and deadline, then creates
child Attempts. Native subagents stay agent-owned and are only observed when the agent
reports them; host delegation creates durable LiteCowork Attempts.

### LiteCowork Runtime and Mesh

`litecoworkd` is headless and uses the same contracts on a workstation, home server,
VPS, container, or managed cloud. A Runtime may provide workspace-hub, executor,
resource-node, channel-host, trigger-host, and operator-endpoint roles. A standalone
install combines the roles it needs. The Workspace Hub coordinates ownership and
durable state; it is not an AI reasoning service and need not perform all execution.

The Mesh provides runtime identity/pairing, presence, domain-event and artifact
replication, inventory, leases/fencing, remote invocation, handoff/failover, and channel
availability. One authoritative Hub is sufficient for an individual workspace in the
initial system; do not build consensus/HA algorithms without a demonstrated need.

### Environment Fabric

Runtime means the LiteCowork daemon. Environment means the execution substrate. A
Runtime can host or reach multiple Environments. Providers expose probe, create, attach,
execute, resource exposure, optional checkpoint/restore, and destroy operations. Initial
provider candidates include local workspace, Git worktree, container, VM, remote machine,
cloud sandbox, browser, and desktop. Only qualified providers are advertised as
available.

## 7. Durable state, events, and replication

The event journal is the durable change history for Conversations, Tasks, plans, Steps,
Attempts, Effects, approvals, Artifacts, leases, and Runtime presence. Events carry a
stable event ID, workspace/entity identity, origin Runtime and sequence, optional entity
revision, logical timestamp, correlation/causation IDs, schema version, type, and payload.
Use a Hybrid Logical Clock or equivalent ordering scheme across Runtimes.

Replicate domain events, immutable artifacts, resource manifests, Task revisions,
capability locks, and execution ownership. Do not replicate live database files. Local
SQLite plus local content-addressed storage is suitable for a standalone Runtime; storage
adapters can later use Postgres and S3-compatible blobs without changing domain
contracts. Never put token deltas, video frames, mouse animation, terminal byte streams,
or raw screenshots into the durable journal. Native private prompts, transcripts,
credentials, and agent memory are not synchronized by default.

Frontend event streams are projections over domain truth, not the domain log. The UI
protocol can be replaced independently of the durable event model.

## 8. Continuation, leases, and effects

Cloud continuation is a property of the runtime model, not a special Environment Fabric
feature. A live process normally cannot move between machines. When ownership changes,
the Task advances through a new Attempt:

```text
local Attempt → safe checkpoint → reconcile open Effects → release/expire lease
             → cloud obtains next fencing epoch → new Attempt from ResumePacket
```

An ExecutionLease identifies Task, Step, Attempt, Runtime, epoch/fencing token, expiry,
and checkpoint. Every mediated mutation checks the current epoch. An old Runtime's stale
Core calls are rejected. A fence cannot stop an agent's own unfenced native tools, so
automatic failover is allowed only when the Task's effects and environment make it safe.

Each Effect records the operation/target, Attempt, idempotency identity where available,
request digest, lifecycle state, result reference, observed post-state, and verification
reference. States include proposed, started, acknowledged, observed, verified, failed,
and ambiguous. After a crash, reconcile an ambiguous Effect before retrying; never repeat
a non-idempotent action merely because its response was lost.

Continuation classes:

- **SAFE_PORTABLE:** consequential effects are mediated/fenced or contained in a
  disposable isolated Environment; automatic continuation can be allowed.
- **REPLAYABLE:** operations are read-only or provably idempotent; reconcile first, then
  a new Attempt may run.
- **HANDOFF_REQUIRED:** native or otherwise unfenced side effects require explicit
  handoff or user review.
- **LOCAL_BOUND:** work needs an unavailable local resource and waits for that Runtime.

Cloud eligibility also checks required inputs, agent and capability availability,
secret placement, environment reproducibility, unresolved Effects, policy, and budget.

## 9. Artifacts, evidence, and completion

Artifacts are first-party durable records. Each ArtifactVersion has a content digest,
storage reference, creator Attempt, input references, provenance, verification references,
and creation time. Providers create or edit content; Core owns version identity and
provenance. Library is a user-facing projection, and saving or publishing is explicit.

Evidence distinguishes:

- **REPORTED:** a worker claims an action or result.
- **OBSERVED:** LiteCowork or a provider saw a response or state change.
- **VERIFIED:** a suitable independent check confirmed the required postcondition.

Prefer deterministic checks (schema, hash, tests, provider reconciliation, structured
postconditions); use semantic/model review only when criteria require judgment. A worker
calling `litecowork.task.finish` proposes completion. The Task Runtime checks required
outputs, active children, approvals, ambiguous Effects, and acceptance evidence before
marking the Task complete. Other outcomes include verifying, needs-user, incomplete, and
blocked.

## 10. Protocol boundaries

Keep these protocols separate:

1. **Operator:** desktop/web/mobile/CLI to `litecoworkd`.
2. **Mesh:** Runtime to Runtime identity, sync, presence, leases, and remote invocation.
3. **Agent:** Runtime to external agent over ACP, A2A, SDK/API, or CLI adapter.
4. **Capability:** Agent to LiteCowork Gateway MCP, Broker, LitePSM, or provider.
5. **Environment:** Attempt to EnvironmentProvider.
6. **Human channels:** Telegram, Slack, Discord, Teams, email, and webhook adapters feed
   authenticated messages into the same Conversation/Task model; they do not own a
   separate bot task store.

Channel assurance determines whether an identity may view, steer, or approve sensitive
work. A weakly authenticated channel cannot approve high-risk actions.

## 11. User experience

Primary navigation is Home, Tasks, Library, Automations, and Discover. One composer
handles quick questions and durable tasks; no Chat/Cowork/Agent mode switch is required.
Advanced agent, model, budget, access, and placement controls are progressive disclosures.

Live Desk shows actual Task/Step/Attempt state, real capability use, Runtime placement,
artifacts, verification, and blocked/needs-user state. It must not invent agent activity,
fake third-party interfaces, imply that a process teleported, or show verification before
a verifier ran. Reduced-motion support preserves the same state transitions.

## 12. Explicitly outside first-party Core

Do not recreate first-party model routing, cognitive memory/vector retrieval, a universal
World Model, Office implementation, browser/computer-use intelligence, code intelligence,
search/research engine, connector marketplace, large workflow runtime, machine-wide
observer, or a massive built-in skill catalog. Integrate these as external agents,
capabilities, LitePSM packages, or EnvironmentProviders. Core stays capable by composing
replaceable parts rather than implementing every domain.

## 13. Build sequence

1. Local `litecoworkd`, Operator surface, durable Conversation/Task, and one qualified
   external AgentAdapter.
2. LiteCowork Gateway, LitePSM client, one capability, scoped grant, Effect/Artifact record,
   and verifier; kill and replace the agent session from a portable ResumePacket.
3. Heterogeneous host delegation with bounded TaskPacket and ResultEnvelope.
4. Two Runtimes, event/artifact replication, execution leases, fencing, and explicit
   handoff; test failure during an ambiguous Effect before enabling automatic failover.
5. One remote human channel on the same Conversation/Task.
6. Automation that creates an ordinary Task.

Do not build cloud continuation, messaging, broad domain providers, or elaborate
Workbench before the preceding vertical slice proves its contracts.

## 14. Contract map and authority

[`docs/COVERAGE-MATRIX.md`](docs/COVERAGE-MATRIX.md) indexes the complete HLD, LLD,
API, persistence, security, UI, motion, operations, and acceptance contracts. Each
concern has one normative document; other documents link to it. The source reconciliation
and dispositions are recorded in [`docs/SOURCE-RECONCILIATION.md`](docs/SOURCE-RECONCILIATION.md).

## 15. Initial implementation shape

Start as one headless Runtime plus an Operator application and a small set of domain
modules. A candidate source layout is:

```text
apps/desktop/
crates/domain/{conversation,task,artifact,effect,evidence}/
crates/runtime/{supervisor,attempt_runner}/
crates/mesh/{identity,presence,replication,leases,transport}/
crates/agents/{adapter,acp,a2a,cli}/
crates/capabilities/{gateway,broker,litepsm}/
crates/environments/{local,worktree,container,remote}/
crates/{trust,verification,automation,events,storage,operator-api}/
```

This is a starting boundary, not a requirement to create empty modules. Introduce a
module only when a working vertical slice needs it.
