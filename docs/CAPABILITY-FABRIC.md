# Capability Fabric

The Capability Fabric gives workers progressive access to task-relevant operations.
AgentCowork owns a small gateway and scoped authorization edge; LitePSM owns the package
ecosystem and lifecycle.

## Ownership

LitePSM owns discovery, resolution, installation, package verification, updates,
provider supervision, and removal. AgentCowork asks what can satisfy a requirement,
what is installed, whether a Runtime/agent can use it, how to launch it, which exact
version/digest is resolved, and whether it is healthy.

AgentCowork stores CapabilityRef, CapabilityGrant, CapabilityActivation, and
CapabilityOffer. A catalog entry, installed package, connected account, binding
compatibility, activation, and authorization are distinct states.

## Resolution and invocation

1. Agent requests a capability by semantic requirement or stable ID.
2. Broker queries LitePSM and filters by version, digest, source, health, binding
   compatibility, Runtime/Environment availability, Task constraints, and policy.
3. User or existing policy selects a candidate and exact scope; Core records a
   Task/Attempt-bound grant.
4. Attach native typed tools when the AgentAdapter supports session-scoped MCP. Otherwise
   invoke through the Cowork Gateway proxy.
5. Every proxy invocation carries workspace, Task, Step, Attempt, session, Runtime,
   grant, trace, and lease identity. Trust admits or denies it and logs Core-mediated
   effects.
6. Revocation blocks later calls and invalidates stale handles. In-flight behavior
   follows the bounded authorization lease.

## Skills and plugins

Skills are bounded procedural content and cannot grant tools or permissions. Resolve an
exact version/digest on demand. Attach through an agent-supported native skills/context
mechanism when possible, otherwise provide a bounded Task context overlay. Never rewrite
private agent configuration or copy host content into native agent directories.

Plugins are bundles; each MCP, skill, agent, workflow adapter, model adapter, or UI
surface is separately compatible and permissioned. Installing/enabling a package is
never a blanket execution grant.

## Direct and proxy modes

Use native attachment when the binding can accept the scoped capability. Use proxy
execution when it cannot. If attachment can change only at a safe turn/session boundary,
proxy for the active turn and apply native attachment at the next boundary. Capability
discovery must not force a running session restart.
