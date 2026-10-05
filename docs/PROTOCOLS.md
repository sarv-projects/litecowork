# Protocol Contracts

## 1. Operator Protocol

Purpose: human clients control/view a Runtime.

Transport:
- local desktop: IPC preferred
- remote/web/mobile: authenticated HTTPS + WebSocket/stream transport

Workspace scope:
- every Workspace-scoped HTTP request carries `X-Workspace-ID`; local IPC carries the
  equivalent selected-Workspace context field
- Workspace list/create are the only unscoped Operator operations
- a Workspace ID in a path or body must equal the selected context; resource IDs are
  resolved to their owning Workspace and are never authorization credentials
- missing or mismatched context is rejected as `FORBIDDEN` without revealing whether a
  referenced Workspace or resource exists
- WebSocket subscriptions are bound to the selected context; the subscription's
  `workspace_id` must match `X-Workspace-ID`

Responsibilities:
- conversations/messages/turns
- Task CRUD/steer/pause/resume/cancel/Step recovery
- approvals
- user requests and notifications
- Library/Artifact access
- Workspace roots and deterministic resource search
- automation management
- runtime/device status
- live UI projection stream

Operator protocol never exposes raw database tables.

## 2. Mesh Protocol

Purpose: trusted LiteCowork runtimes coordinate.

Responsibilities:
- runtime auth/pairing
- presence
- event replication
- artifact manifests/blob transfer
- leases/fencing
- handoff
- capability/environment offer exchange

Every mutating mesh call is authenticated to a Runtime identity.

## 3. Agent Protocol

Purpose: Runtime controls external worker agents.

Select by topology and negotiated feature requirements: ACP for interactive
client-to-agent sessions, A2A for independent remote agent systems, vendor SDK/API where
it provides a richer supported boundary, and qualified structured CLI/terminal fallback.
This is not a universal ranking. AgentProfile may expose multiple AgentEndpoints.

AgentAdapter normalizes protocol differences; domain code depends only on normalized contracts.

Host delegation is an explicit LiteCowork Gateway surface, separate from native
subagents. The canonical internal tool is `litecowork.agents.delegate(DelegateRequest)`;
status/result/steer/cancel commands address the returned `ChildAttemptRef` and validate
the current Task/profile versions and Attempt authority. The complete request, admission,
TaskPacket, ResultEnvelope, and failure rules are in `DELEGATION.md` and their canonical
envelope schemas are in `schemas/delegation.schema.json`. Agent-native tools
that spawn native subagents remain harness-owned and are not intercepted or represented as
LiteCowork Attempts unless a qualified adapter explicitly reports them.

Delegated communication has two planes. Durable control changes (admission, user
steering, cancellation, result settlement, Effect, Artifact, Evidence, lease) use domain
commands/events. Streaming text, heartbeats, short worker messages, and browser
observations use an authenticated, bounded ephemeral channel and can be dropped/replayed
from the last durable checkpoint. Ephemeral messages cannot change Task status or grant
authority. Session and channel credentials are scoped to the current Attempt/Runtime
incarnation and are not replicated.

## 4. Capability Protocol

Purpose: worker discovers/uses external capabilities through LiteCowork Gateway and CapabilityBroker.

Primary portable surface is MCP-compatible host tools. A direct native tool surface is
provided through a host-managed broker relay so calls still create durable Invocations;
the alternate surface is the stable `litecowork.capabilities.invoke` proxy tool. A
worker's independently connected, unmediated native tool is classified as agent-owned
and does not inherit LiteCowork's Invocation, Effect, or fencing guarantees.

LiteCowork targets the stateless MCP base protocol dated `2026-07-28`; initialize/session
state is not assumed. Extensions are negotiated independently per provider and operation:
Tasks, Apps, and Skills may be available or absent. MCP Task handles persist on
CapabilityInvocation and are never confused with LiteCowork Task identity. MCP Apps
execute in a sandboxed iframe and use host-mediated tool/resource access. MCP Skills may
be server-published `SKILL.md` resources identified by server identity and URI. These
protocol extensions do not define or replace LiteSPM package contracts.

## 5. Environment Provider Protocol

Purpose: Attempt acquires/uses execution substrate.

Provider may be in-process adapter, local daemon, remote API or plugin, but normalized lifecycle is defined in `ENVIRONMENTS.md`.

## Compatibility

Every protocol handshake exposes:

```text
ProtocolHello {
  protocol_name
  protocol_version
  implementation_version
  feature_flags[]
}
```

For LiteCowork-owned protocols, incompatible major versions fail explicitly and optional
features are negotiated rather than inferred. This `ProtocolHello` shape is not imposed
on third-party MCP, ACP, A2A, or LiteSPM endpoints; their own version negotiation applies.

## Standards references

- [MCP stateless base protocol update (2026-07-28)](https://blog.modelcontextprotocol.io/posts/2026-07-28/)
- [Versioned MCP Tasks extension (currently Draft)](https://tasks.extensions.modelcontextprotocol.io/specification/draft/tasks)
- [MCP Apps overview](https://apps.extensions.modelcontextprotocol.io/api/documents/overview.html)
- [MCP Skills](https://skills.extensions.modelcontextprotocol.io/specification/stable/skills)
- [Agent Client Protocol overview](https://agentclientprotocol.com/protocol/overview)
- [A2A key concepts](https://a2a-protocol.org/latest/topics/key-concepts/)
