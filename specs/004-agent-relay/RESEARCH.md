# Research - Agent Relay on One Conversation

**Feature:** 004 | **Checked:** 2026-11-06; source crosswalk re-checked 2026-11-12. The older pinned rival-source review remains dated 2026-11-06.

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

## Sources

Authorities: docs/DELEGATION.md (to avoid collapsing relay into HOST_DELEGATED), docs/NATIVE-COMPOSER-LLD.md, docs/FLOWS.md, feature 001. Reject vendor multi-agent routers that share one hidden transcript. Owner-contract references are retained as design constraints; current source/route/UI evidence is recorded below.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Borrowable-design sources (pinned)

| Source | Exact pinned URL | License | What informs relay | Verdict |
|---|---|---|---|---|
| OpenWork lazy capability gateway (`search_capabilities`/`execute_capability`) | https://github.com/different-ai/openwork/tree/5493b18a6af64d56454658a53da00eabbc6e033d | MIT except `/ee` (Enterprise Edition License) | The lazy gateway pattern shows admission of a named operation without holding provider state in the caller; it maps to building each relay turn's input from committed records rather than a live A session. | Prototype the admission shape; do not import `/ee` paths or adopt the OpenCode loop as a second authority. |
| OpenWork `packages/automations` engine/ports doctrine (`engine.ts`, `ports.ts`, `contracts.ts`) | https://github.com/different-ai/openwork/blob/5493b18a6af64d56454658a53da00eabbc6e033d/packages/automations/src/engine.ts | MIT except `/ee` | Receipts/events/results stay readable after the admitting process stops. This validates relay reloading only committed turns and rebuilding B's input from durable state, never a cached A handle. | Prototype the durable-readable doctrine against STATE-MACHINES; do not vendor the TypeScript runtime. |

These confirm that a second agent's input can be a pure function of durable committed state, which is exactly the portable-context contract B-15 proposes. No OpenWork code is imported; the patterns are prototype/reference inputs only.

## Source and route audit (2026-11-06)

Inspected the composed Conversation route in `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, `agents/codex_app_server.rs`, `agents/codex_profile_probe.rs`, `agents/conversation_prompt.rs`, the context package builder and reader in `crates/storage-core/src/conversation.rs` and `crates/storage-sqlite/src/conversations.rs`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `conversation-agent-selection.ts`, `conversation-agent-surface.ts`, and the generated `operator-api.ts` contract.

- `PATCH /v1/conversations/{conversation_id}/agent-binding` is composed and served by `conversation_operator.rs`. It requires the selected Workspace, `If-Match` version and `Idempotency-Key`, commits `conversation.agent_binding.changed.v1`, and returns the updated Conversation. The desktop also uses its Tauri command bridge to persist the selected binding for future turns.
- A submitted turn pins `agent_binding_id` in `conversation.turn.created.v2`. The Conversation message projection includes `agent_binding_id` and `agent_session_id`; these are available as source data for producer attribution. This is not the same as the UI displaying that attribution.
- `AgentSessionSupervisor::execute_agent_turn` has one provider arm: `provider_key == "opencode"`. It spawns a bounded OpenCode v2.0.32 server, creates a fresh native session, activates the durable Conversation AgentSession and sends the portable prompt. Other provider keys reach the unsupported/unconfigured path. The current implementation therefore has one adapter branch, not two.
- `CodexAppServer` includes a context-aware `turn_start_with_conversation_context` method and protocol tests, but the type is used by Codex profile probing and is not integrated into `AgentSessionSupervisor::execute_agent_turn`. Its existence is not a Codex Conversation execution path.
- Feature 002's OpenCode profile probe deliberately records `compatible=false`; the AgentCatalog UI therefore does not allow a new OpenCode binding to be created from that probe. Codex can produce a compatible/startable profile observation and an enabled lead-eligible binding, but the Conversation supervisor has no Codex dispatch arm. Thus the current visible picker cannot provide a complete supported A→B→A sequence.
- `buildConversationAgentChoices` filters by same Workspace, `enabled`, `leadEligible`, unique binding identity and active Conversation supervisor adapter support. It does not check whether a fresh AgentControlDescriptor/auth observation exists. The descriptor-aware `deriveConversationAgentSurface` helper has tests but no production import/call site; Feature 002's descriptor route is wired but still fails closed with 503 absent a qualified producer.
- The backend message projection and desktop `ConversationMessageView`/`parseConversationSnapshot` retain nullable `agent_binding_id` and `agent_session_id`; committed messages display their producer from the stored binding. Focused UI tests cover producer parsing and immutable stream identity. This does not establish a second provider or cross-layer relay.
- The composer `Add context or file` control opens a Resource picker for an existing Conversation and sends eligible text ResourceRefs. The Feature 001 supervisor reauthorizes and materializes those eligible text references at dispatch before encoding. Binary files, WorkspaceRoot files, first-message attachment on a new Conversation, Workspace retrieval at Send and durable Context Used receipts remain unavailable; the picker does not imply any of those capabilities.
- The storage layer builds `ConversationContextPackage` through the exact committed USER-message event revision, orders by aggregate revision, excludes `SYSTEM_NOTICE`, and carries prior AGENT messages' source binding identity. The native prompt encoder itself rejects nonempty, unmaterialized references with `ResourceMaterializationRequired`; the Feature 001 supervisor is the only current materialization path before that encoder.
- Internal bounds are 1,024 source messages / 4 MiB for the context package and 256 KiB for the encoded native text prompt. B-15 was accepted on 2026-11-09 with these as the V1 bounds; they are not yet promoted into SCHEMAS.md/OpenAPI (004-T01).
- Changing the Conversation binding commits Conversation state only. It does not create a Task, Attempt, DelegationProfile, Environment, lease, grant or Task lead-handoff event. This matches the intended separation between ordinary relay and delegated Task execution.

These are source/route/UI findings. Producer attribution and eligible text-reference materialization have CODE evidence, but no second-provider native Conversation, full cross-layer A→B→A relay, or A→B→A USER journey was executed. Feature 004 is ACCEPTED/LOCKED (2026-11-09); none of these behaviors is qualified by acceptance.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) before a future market-positioning decision.
- No second-provider native Conversation, full SQLite-to-desktop A→B→A relay, or A→B→A USER journey was executed. Eligible text ResourceRef materialization is present in Feature 001 CODE; root/binary ingestion and Context Used remain unsupported.

## Unresolved questions

See SPEC section 5 blockers. Owners are named on the package README.


## Implementation efficiency and language-placement observations (2026-11-09)

Source-grounded notes for the implementer; each is folded into the task named and none changes a locked decision.

- **Provider dispatch should be a closed enum (Rust).** Adding Claude Code as a second string arm in `execute_agent_turn` would repeat the `OPENCODE`/`opencode` mismatch. Use Feature 002's `NativeProviderKey` and give each arm its own function (`run_opencode_turn`, `run_claude_code_turn`) instead of growing the 250-line inline match (004-T05).
- **Per-turn host spawn.** The OpenCode arm spawns a full `opencode serve` process for every turn (`hosting_mode: LOCAL_PER_SESSION`). That satisfies fresh-session-per-turn but costs a server start each turn. SPEC §7 allows host reuse as a host-supervisor choice as long as the AgentSession and native session are new per turn; a warm per-incarnation host is a Feature 001 host-supervisor optimization, measured before adoption. Claude Code's non-interactive mode is a per-invocation process by nature, so no host pool is proposed for it (004-T05 note only).
- **Producer identity belongs in the typed parse (TypeScript).** The Tauri bridge forwards `serde_json::Value`, so no Rust change is needed to preserve producer fields; the fix is in `parseConversationSnapshot`. Validating the two fields there (bounded id or null) keeps one parser as the trust boundary (004-T02).
- **Picker gating should consume a daemon projection.** `deriveConversationAgentSurface` reimplements descriptor/auth checks in TypeScript without a production caller. The daemon should report per-binding dispatchability and freshness (Feature 002 descriptor projection) and the picker should only render it (004-T06).
- **Portable export ordering.** The export is a pure function over a revision-ordered committed prefix; build it with one bounded indexed query capped at 1,024 messages rather than loading full history and trimming in memory (004-T03, AC-011).
