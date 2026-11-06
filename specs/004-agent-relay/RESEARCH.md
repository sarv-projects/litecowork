# Research - Agent Relay on One Conversation

**Feature:** 004 | **Checked:** 2026-11-06

## Sources

Authorities: docs/DELEGATION.md (to avoid collapsing relay into HOST_DELEGATED), docs/NATIVE-COMPOSER-LLD.md, docs/FLOWS.md, feature 001. Reject vendor multi-agent routers that share one hidden transcript. Owner-contract references are retained as design constraints; current source/route/UI evidence is recorded below.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Source and route audit (2026-11-06)

Inspected the composed Conversation route in `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, `agents/codex_app_server.rs`, `agents/codex_profile_probe.rs`, `agents/conversation_prompt.rs`, the context package builder and reader in `crates/storage-core/src/conversation.rs` and `crates/storage-sqlite/src/conversations.rs`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `conversation-agent-selection.ts`, `conversation-agent-surface.ts`, and the generated `operator-api.ts` contract.

- `PATCH /v1/conversations/{conversation_id}/agent-binding` is composed and served by `conversation_operator.rs`. It requires the selected Workspace, `If-Match` version and `Idempotency-Key`, commits `conversation.agent_binding.changed.v1`, and returns the updated Conversation. The desktop also uses its Tauri command bridge to persist the selected binding for future turns.
- A submitted turn pins `agent_binding_id` in `conversation.turn.created.v2`. The Conversation message projection includes `agent_binding_id` and `agent_session_id`; these are available as source data for producer attribution. This is not the same as the UI displaying that attribution.
- `AgentSessionSupervisor::execute_agent_turn` has one provider arm: `provider_key == "opencode"`. It spawns a bounded OpenCode v2.0.32 server, creates a fresh native session, activates the durable Conversation AgentSession and sends the portable prompt. Other provider keys reach the unsupported/unconfigured path. The current implementation therefore has one adapter branch, not two.
- `CodexAppServer` includes a context-aware `turn_start_with_conversation_context` method and protocol tests, but the type is used by Codex profile probing and is not integrated into `AgentSessionSupervisor::execute_agent_turn`. Its existence is not a Codex Conversation execution path.
- Feature 002's OpenCode profile probe deliberately records `compatible=false`; the AgentCatalog UI therefore does not allow a new OpenCode binding to be created from that probe. Codex can produce a compatible/startable profile observation and an enabled lead-eligible binding, but the Conversation supervisor has no Codex dispatch arm. Thus the current visible picker cannot provide a complete supported A→B→A sequence.
- `buildConversationAgentChoices` filters by same Workspace, `enabled`, `leadEligible`, and unique binding identity. It does not check whether the provider key has an active Conversation supervisor adapter or whether a fresh AgentControlDescriptor/auth observation exists. The descriptor-aware `deriveConversationAgentSurface` helper has tests but no production import/call site; Feature 002's descriptor route currently fails closed with 503.
- The backend message projection carries `agent_binding_id` and `agent_session_id`, but the desktop `ConversationMessageView` type omits both. `parseConversationSnapshot` reconstructs each message using only message ID, role, timestamp, content and rich-presentation reference, dropping the producer/session identity even if the response contains it. The transcript renderer therefore cannot resolve or display the producing agent; AC-005 is not implemented.
- The composer includes an `Add context or file` button with a title, but it has no click handler. It does not attach a Resource or file to the next turn.
- The storage layer builds `ConversationContextPackage` through the exact committed USER-message event revision, orders by aggregate revision, excludes `SYSTEM_NOTICE`, and carries prior AGENT messages' source binding identity. The native prompt encoder rejects any nonempty `resource_refs` and RESOURCE content using `ResourceMaterializationRequired`; no Resource materializer is wired into this relay path.
- Internal bounds are 1,024 source messages / 4 MiB for the context package and 256 KiB for the encoded native text prompt. These are implementation constants, not an owner-approved portable-context schema/retention policy; blocker B-15 remains unresolved.
- Changing the Conversation binding commits Conversation state only. It does not create a Task, Attempt, DelegationProfile, Environment, lease, grant or Task lead-handoff event. This matches the intended separation between ordinary relay and delegated Task execution.

These are source/route/UI findings. No second-provider native Conversation, Resource materialization, rendered producer-label flow, or A→B→A USER journey was executed. Feature 004 remains DRAFT.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) before a future market-positioning decision.
- No second-provider native Conversation, Resource materialization, rendered producer-label flow or A→B→A USER journey was executed.

## Unresolved questions

See SPEC section 5 blockers. Owners are named on the package README.
