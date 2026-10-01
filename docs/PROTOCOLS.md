# Protocol Boundaries

AgentCowork composes protocols; no one protocol is universal.

| Boundary | Participants | Purpose |
|---|---|---|
| Operator | Desktop/web/mobile/CLI ↔ `agentcoworkd` | User interaction and read/write projections. |
| Mesh | `agentcoworkd` ↔ `agentcoworkd` | Identity, presence, event/artifact replication, leases, and remote invocation. |
| Agent | Runtime ↔ external agent | Session, messages, control, capability/context attachment, and reported events. |
| Capability | Agent ↔ Cowork Gateway ↔ Broker/LitePSM/provider | Scoped capability discovery and invocation. |
| Environment | Attempt ↔ EnvironmentProvider | Placement and execution in a concrete substrate. |
| Human channel | Telegram/Slack/Discord/Teams/email/webhook ↔ channel adapter | Deliver authenticated messages into the shared Conversation/Task model. |

ACP and A2A serve different agent boundaries. Native SDK/API and CLI adapters are
qualified alternatives. MCP is the initial shared capability transport, not a
replacement for agent-native tools. Frontend lifecycle events are projections from the
durable domain journal; the UI stream is not the journal.

Every adapter negotiates support and reports unavailable features. Do not infer
capability from a product name. Channel assurance controls view, steer, and approval
rights independently; receipt of a message never implies authority to approve sensitive
effects.
