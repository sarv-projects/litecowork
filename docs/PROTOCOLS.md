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
- conversations/messages
- Task CRUD/steer/cancel
- approvals
- Library/Artifact access
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

Preferred implementations: ACP, A2A, native SDK/API, structured CLI, terminal compatibility.

AgentAdapter normalizes protocol differences; domain code depends only on normalized contracts.

## 4. Capability Protocol

Purpose: worker discovers/uses external capabilities through LiteCowork Gateway and CapabilityBroker.

Primary portable surface is MCP-compatible host tools; backend may attach direct MCP servers or proxy invocation.

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

Major incompatible versions must fail explicitly. Optional features are negotiated rather than inferred.
