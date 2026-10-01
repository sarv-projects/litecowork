# Product Contract

## Promise

LiteCowork lets a user describe work once and have the Task survive agent replacement, process failure, device changes and capability changes while remaining understandable and controllable.

## Core user expectations

1. Simple questions remain conversation; outcome-oriented work becomes durable Task state.
2. User does not need to understand MCP, ACP, providers or runtime topology for ordinary use.
3. User can inspect internals when desired through Inspector.
4. Work may continue in cloud only when inputs, secrets, capabilities and effect safety permit it.
5. LiteCowork never claims completion solely because a worker says "done".
6. LiteCowork does not fake visual interaction when structured/API capabilities were used.
7. Connected capabilities are discovered progressively instead of flooding every agent context.
8. Parallel/heterogeneous agents may collaborate but lead reasoning remains agent-owned.
9. User can steer/cancel and see what is blocked or waiting for them.
10. Reusable procedures can be proposed as Skills and then become LitePSM-managed packages.

## Primary navigation

```text
Home
Tasks
Library
Automations
Discover
```

Technical surfaces such as Agents, MCP, Providers, Environments, Runtime details and protocols live under Settings/Inspector/Discover details.

## Conversation and Task behavior

The composer accepts questions and outcomes in the same place. Ordinary conversation
does not need a Task. A clear request for durable, outcome-oriented work may materialize
a Task; an explicit user request always may. The conversational lead may request
materialization, but ambiguous intent is clarified instead of being sent through a
separate hidden planner. A committed Task is linked to the originating Conversation and
message. See `EXPERIENCE.md` and `FLOWS.md` for the UI and command sequence.

Conversation preserves the human-facing exchange. Task records the durable work and
requirements. The user can steer, revise, pause, cancel, or inspect a Task without
depending on one agent transcript. A Task that continues on another Runtime starts a new
Attempt from portable state; no process migration is promised.

## Runtime choices

LiteCowork supports a local-only installation and can connect to a user-controlled or
managed always-available Runtime. Local and remote Runtime instances use the same domain
contracts. Cross-device availability is conditional on replicated inputs, eligible
agents/capabilities, secrets, Environment support, policy, budget, and safe Effects. The
UI names the actual blocker rather than presenting cloud continuation as unconditional.

The initial product is a single-user Workspace coordinated by one authoritative Hub.
Additional Runtimes may execute eligible Attempts. Multi-tenant collaboration, automatic
Hub consensus, and transparent migration of native processes are not v1 promises.

## Capability and trust expectations

Discover is the user-facing path to compatible agents and external capabilities. LitePSM
is the selected external package ecosystem; LiteCowork adds Task-scoped compatibility,
grants, activation, effect handling, and verification. The selected base URL and deferred
integration details are in `CAPABILITY-FABRIC.md`. Installing or connecting something
does not by itself authorize it for every Task.

The user sees the requested operation and scope when approval is needed. A message
channel may start or steer permitted work, but a weakly authenticated channel cannot
approve a high-impact action. Credentials are not copied to another Runtime without
explicit placement and secret-lease policy.

## Output and completion expectations

Providers may create documents, spreadsheets, code changes, research, or other
domain-specific outputs. LiteCowork owns Artifact identity, version, provenance, and
availability. Publishing/saving to Library is explicit. A completion label means the
Task's mandatory criteria met their declared evidence requirements; a worker's
completion claim alone is never enough.

## Product limits

LiteCowork reports what it can observe. It does not claim control over an external
agent's private tools, credentials, hidden usage, or unmediated side effects. Where an
action cannot be fenced or reconciled, continuation may require a handoff or user
decision. Those limits are shown as state, not hidden behind a success animation.

## Product non-goals for Core

LiteCowork Core is not:
- a universal model router
- a cognitive memory engine
- an Office suite
- a browser-agent implementation
- a computer-use model
- a code intelligence platform
- a web-search engine
- a second plugin marketplace
- a general workflow-engine competitor
