# API and events - Agent Relay on One Conversation

## Existing served operations reused

- `PATCH /v1/conversations/{conversation_id}/agent-binding` (`setConversationAgentBinding`) is served by `apps/litecoworkd/src/conversation_operator.rs`. It requires the selected Workspace, `If-Match` with the current Conversation aggregate version, and `Idempotency-Key`; it commits `conversation.agent_binding.changed.v1` and returns the updated Conversation. The binding change applies to future turns; it does not migrate or interrupt an already admitted native session.
- `POST /v1/conversations/{conversation_id}/turns` (`submitConversationTurn`) and the failed-turn retry operation remain owned by Feature 001. Each created turn pins its selected `agent_binding_id` in `conversation.turn.created.v2`. Feature 004 does not introduce a separate relay-send API.
- Conversation message projections already contain `agent_binding_id` and `agent_session_id`. The current UI does not render the producer binding, so AC-005 is not satisfied by the existence of these fields alone.

## Events

Reuse the canonical event contracts only: `conversation.agent_binding.changed.v1`, `conversation.turn.created.v2`, `agent.session.starting.v1`, and `agent.session.started.v1`, together with the Feature 001 settlement events. This feature does not declare a new event type or schema in DRAFT.

## Runtime boundary

The current `AgentSessionSupervisor` dispatches only `provider_key=opencode`. `CodexAppServer` profile/protocol code is not a Conversation dispatch route. Do not represent the Codex provider as relay-capable until a supervisor adapter is wired and qualified. Feature 002's current OpenCode profile offer is incompatible for new binding creation, so the current UI has no supported second binding to select.

## Target-only work

No second-provider relay endpoint is currently specified or required. Any new context receipt, selected-resource materialization contract, producer-display projection field, or new event must be promoted in the canonical owner document and `docs/schemas/` in the same reviewed change before implementation.

## Errors and compatibility

Use the existing owner error codes: stale Conversation version, unavailable/incompatible AgentBinding, unsupported native session override/configuration change, quota exhaustion, and Feature 001 turn admission/settlement failures. Do not invent relay-specific error codes in this package.

Event names must be present in [docs/EVENTS.md](../../../docs/EVENTS.md) with schemas in `docs/schemas/domain-event.schema.json`. Error codes must be present in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) and `docs/schemas/error-codes.schema.json`.