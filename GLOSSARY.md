# LiteCowork Glossary

This glossary fixes the product's domain language. It names the durable work, workers,
execution locations, permissions, and evidence that LiteCowork coordinates.

## Work and execution

**Conversation**:
The human-facing exchange that can contain ordinary discussion and references to one
or more durable Tasks.
_Avoid_: Chat session as the work record

**Presentation Runtime**:
The Operator read/render path that turns authorized projections and bounded Agent output
into typed user-visible items; it does not own durable work or permission state.
_Avoid_: Second event store, agent UI authority

**PresentationItem**:
A typed, source-linked presentation value derived from a projection; it is neither a
domain event nor a durable aggregate.
_Avoid_: UI-owned Task state

**Task**:
The durable outcome a user wants accomplished, including its requirements, progress,
outputs, and unresolved decisions.
_Avoid_: Run, workflow instance

**Routine**:
A user-reviewed reusable work definition. Its immutable revisions define the job, inputs,
outputs and criteria independently of any trigger or execution.
_Avoid_: Skill, workflow engine

**Automation**:
A revision-pinned Routine plus trigger definitions and execution policy. Each logical
occurrence can create one ordinary Task; trigger host and execution host are separate.
_Avoid_: Agent process, schedule alone

**AutomationOccurrence**:
One deduplicated trigger delivery or due schedule slot, with pinned Routine/Automation
revisions, claim authority, dependency state and its produced Task reference.
_Avoid_: Agent Attempt

**TaskSpecRevision**:
An immutable version of the user's objective, constraints, required outputs, acceptance
criteria, approvals, and budget.
_Avoid_: Mutable task prompt

**PlanRevision**:
An immutable plan proposed by a lead agent and structurally checked and stored by
LiteCowork.
_Avoid_: Core-generated plan

**Step**:
A semantic unit of work in a PlanRevision.
_Avoid_: Attempt

**Attempt**:
One worker's bounded execution try for a Step.
_Avoid_: Task run, agent session

**DelegationProfile**:
A versioned, user-enabled configuration for one AgentBinding to receive bounded host-
delegated work.
_Avoid_: Installed agent, model alias

**Lead failover policy**:
A Task-pinned rule that may permit a bounded lead-agent change when a listed condition is
observed; it does not inherit grants or approval and always starts a fresh AgentSession.
_Avoid_: Silent model fallback

**ActionBatch**:
A bounded grouping of independently admitted capability operations. Each member retains
its own Invocation, idempotency key, optional Effect, and Evidence.
_Avoid_: Transaction, atomic macro

**Execution method**:
The normalized path a mediated capability operation actually used, recorded as observed
provenance rather than inferred from the requested capability.
_Avoid_: UI animation, agent claim

**WarmHold**:
A short-lived Runtime-local operational request to keep an eligible execution dependency
ready before admission; it carries no Task authority and may be evicted.
_Avoid_: AgentSession, ExecutionLease

**Coworker interaction policy**:
Narrow defaults that preserve or add confirmation/handoff friction around existing Trust
decisions; they never grant authority or reduce mandatory checks.
_Avoid_: Autonomy grant, authority ceiling

**Native subagent**:
A child worker created and governed inside an external agent's native harness.
_Avoid_: LiteCowork Attempt

**Host-delegated worker**:
An external agent started by LiteCowork for a bounded Step under a child Attempt,
separate lease, grants, Environment, and verification.
_Avoid_: Native subagent

**Coworker**:
A user-facing identity and preference bundle that organizes Tasks and context without
owning execution or authority.
_Avoid_: AgentSession, autonomous Task owner

**Goal**:
A user-authored desired outcome whose progress is projected from verified work.
_Avoid_: Scheduler, background planner

**Suggestion**:
An expiring, provenance-backed proposal from a registered producer that requires an
explicit user action.
_Avoid_: Approval, automatic Task

**Environment sharing scope**:
The set of eligible Tasks/Attempts allowed to reuse an Environment, independent of how
long that Environment may exist.
_Avoid_: CapabilityGrant, Task lease

**AgentHarnessDescriptor**:
A time-bounded, non-secret observation of one adapter's supported harness features and
session options.
_Avoid_: Native configuration copy

## Workers and execution places

**Agent**:
An external reasoning worker that owns its model choice, reasoning, and native tools.
_Avoid_: Model

**AgentSession**:
One agent-specific reasoning context scoped to a Conversation, Task planning, or an
Attempt; it may be replaceable or unavailable after failure.
_Avoid_: Task, durable transcript

**WorkspaceInstructionRevision**:
An immutable, explicitly authored Workspace guidance revision that a Task pins when its
specification is created or revised.
_Avoid_: Hidden agent memory

**Runtime**:
A stable identity for one installed `litecoworkd` Runtime; current process state belongs to its RuntimeIncarnation.
_Avoid_: Environment

**RuntimeWorkspaceBinding**:
An independently revocable authorization relationship between one Runtime installation and one Workspace, including its enrollment mode and permitted Runtime roles.
_Avoid_: Runtime identity, implied Workspace membership

**RuntimeIncarnation**:
One daemon lock-holder process lifetime under a persistent Runtime identity. Restart
creates a new incarnation and requires revalidation of process handles and observations.
_Avoid_: New device identity

**AgentHostInstance**:
Runtime-local operational host or endpoint handle serving admitted AgentSessions, with
explicit ownership and use references. Installed software need not have a running host.
_Avoid_: AgentSession, durable Task

**Environment**:
The execution substrate in which an Attempt acts, such as a workspace, worktree,
container, VM, browser, desktop session, or remote sandbox.
_Avoid_: Runtime

**Capability**:
A versioned operation or set of operations that an authorized Attempt can use through a
provider, agent-native integration, or LiteCowork Gateway.
_Avoid_: Permission, package listing

**LiteSPM**:
The independent product that owns capability/package ecosystem discovery and lifecycle;
LiteCowork consumes its contract without duplicating that system.
_Avoid_: LiteCowork marketplace

## Effects and results

**Effect**:
A proposed or attempted action that can change state outside the reasoning process.
_Avoid_: Tool call report

**Artifact**:
A durable named result or input resource with versioned content identity and provenance.
_Avoid_: Temporary file path

**Evidence**:
An immutable record supporting a claim about an Effect, Artifact, or acceptance
criterion.
_Avoid_: Agent assertion treated as proof

**ExecutionLease**:
Time-bounded, epoch-fenced authority for one Runtime to own execution of a Step's
current Attempt.
_Avoid_: Lock without fencing

**ResourceRef**:
A stable logical Resource reference, optionally pinned to a revision/digest, independent
of the Runtime, Environment, connector, or path where it is currently available.
_Avoid_: Assumed shared local path

**ResourceLocation**:
A provider/runtime/environment-scoped locator and availability record for a logical
Resource.
_Avoid_: Resource identity

**WorkspaceRoot**:
A user-authorized persistent folder Resource and location that LiteCowork may observe
under an explicit watch policy.
_Avoid_: One-time attachment

**World Index**:
A factual, bounded index of explicitly granted Resources, identities, revisions,
locations, relationships, freshness, and deterministic search results.
_Avoid_: Reasoning world model, cognitive memory

**CapabilityInvocation**:
A durable record of one capability operation, including dispatch, asynchronous provider
task state, cancellation, partial results, and final results.
_Avoid_: Effect; LiteCowork Task

**Human channel**:
An external messaging transport that maps authenticated messages into a Conversation;
it does not own a Task.
_Avoid_: Agent protocol

## Accepted Coworker target terminology

Coworker: optional persistent named assistant identity, not a new executable agent. Ordinary session: Conversation with no Coworker owner. Standing responsibility: explicitly enabled, versioned user-facing grouping of Goals/Routines/Automations that fences new unattended Task admissions; not a scheduler. Connection: provider-installed/authenticated resource; Coworker assignment restricts its use without creating a live Grant. Knowledge: authorized source retrieval, not necessarily saved memory. Memory candidate: untrusted extraction proposal; memory record: versioned Resource-backed ContextDocument admitted under scoped policy. Proactive activity: confirmed schedule/event/monitor-driven work, not automatic model thought. Current code may not yet implement these accepted targets.


## Agent lifecycle/control terms

**AgentModule** — Independently versioned bundle that owns one agent family's lifecycle
adapter, runtime adapter, and optional LiteSPM capability bridge. Core selects modules by
registry identity/capability contract, never brand conditionals.

**AgentRegistryEntry** — Cached descriptive ACP/distribution metadata. It does not prove
installation, authentication, entitlement, compatibility, or authorization.

**AgentInstallationObservation** — Runtime-local observation of whether an agent is
missing/installing/installed/update-available/updating/broken and which version was
observed.

**AgentControlDescriptor** — Time-bounded sanitized non-secret projection of an agent's
auth methods/state, non-secret config schema, session options, native input/command/
reference surface, version information, and negotiated capabilities.

**AgentLifecycleAdapter** — AgentModule boundary for install/update/auth/native-config and
control-descriptor projection. It does not execute Conversation/Task reasoning.

**AgentCapabilityBridge** — AgentModule policy/translation boundary determining how one
LiteSPM-managed capability may be exposed to that agent.

**Credential slot** — Adapter-declared authentication/configuration slot. It is either
native-agent-owned or a LiteCowork SecretStore slot. The slot ID may be durable; secret
bytes never are.

**Native surface** — Agent-owned slash commands, @ references, input types,
provider/model/session controls, extensions, subagents, config and related harness behavior
preserved by LiteCowork.
