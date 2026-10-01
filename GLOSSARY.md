# LiteCowork

This glossary fixes the product's domain language. It names the durable work, workers,
execution locations, permissions, and evidence that LiteCowork coordinates.

## Work and execution

**Conversation**:
The human-facing exchange that can contain ordinary discussion and references to one
or more durable Tasks.
_Avoid_: Chat session as the work record

**Task**:
The durable outcome a user wants accomplished, including its requirements, progress,
outputs, and unresolved decisions.
_Avoid_: Run, workflow instance

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

## Workers and execution places

**Agent**:
An external reasoning worker that owns its model choice, reasoning, and native tools.
_Avoid_: Model

**AgentSession**:
One agent-specific reasoning context used by an Attempt; it may be replaceable or
unavailable after failure.
_Avoid_: Task, durable transcript

**Runtime**:
A running LiteCowork daemon with identity, roles, presence, and execution capacity.
_Avoid_: Environment

**Environment**:
The execution substrate in which an Attempt acts, such as a workspace, worktree,
container, VM, browser, desktop session, or remote sandbox.
_Avoid_: Runtime

**Capability**:
A versioned operation or set of operations that an authorized Attempt can use through a
provider, agent-native integration, or LiteCowork Gateway.
_Avoid_: Permission, package listing

**LitePSM**:
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
A stable, optionally revision-pinned reference to content or a resource that a Runtime
can resolve.
_Avoid_: Assumed shared local path

**Human channel**:
An external messaging transport that maps authenticated messages into a Conversation;
it does not own a Task.
_Avoid_: Agent protocol
