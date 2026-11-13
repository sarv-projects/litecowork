# UI behavior - Agent Relay on One Conversation

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Target flows and states

- The agent picker is shown in the Conversation composer and changes the binding for future turns only. The current turn keeps the provider/session with which it was admitted.
- Only same-Workspace, enabled, lead-eligible bindings whose AgentModule is actually dispatchable and whose current descriptor/auth/session readiness is acceptable may be selectable for a new turn.
- A rejected or stale binding leaves the draft intact and shows the exact unavailable/incompatible reason. It must not silently send to the previous agent or workspace default.
- Every committed AGENT response displays its producing agent/profile label. Resolve the stored `agent_binding_id` to the binding/profile display name; when unavailable, show a neutral unavailable label plus the stable binding identifier. Never infer the producer from the currently selected picker value.
- Returning to a previously used provider is a new Conversation AgentSession and a new native session unless an owner-approved resume contract says otherwise. Show the historical producer per message; do not imply hidden native memory was transferred.
- Selected Resource/artifact context is visible as explicit attachments with source identity and admission status. The UI must not claim a Resource is included until the backend has authorized and materialized it for the selected provider.
- Switching agents does not create a Task, child Attempt, worker, approval, lease or delegated execution.

## Current implementation crosswalk (2026-11-13; not the target)

- `ConversationsPage` displays the selected Conversation/Workspace binding picker and persists changes through the Tauri `set_conversation_agent_binding` command. The UI notice explicitly states that the change is for future turns and an already-running session keeps its current agent.
- The current choice builder filters by same Workspace, enabled, lead-eligible and a provider key with a Conversation supervisor dispatch arm (currently OpenCode only). It does not check fresh descriptor, current compatible Runtime offer, authentication or session readiness. `conversation-agent-surface.ts` contains descriptor/auth logic with unit tests, but no production component imports or invokes it. Feature 002 can publish a compatible OpenCode offer for the exact accepted probe tuple, but that is not proof of an authenticated or qualified user route; no second dispatch path is qualified.
- The backend message projection carries nullable LiteCow `agent_binding_id` and `agent_session_id`, and the Tauri bridge forwards them unchanged as JSON. The desktop snapshot parser retains both; each committed AGENT message now resolves its producer label from the stored binding ID, with a neutral unknown-agent fallback. Parser/render evidence exists at CODE level; the full storage-to-reload AC-005 journey, picker recovery states and cross-provider UI qualification remain open.
- The current composer `Add context or file` button opens the existing-Conversation Resource picker. Eligible text ResourceRefs are authorized and materialized by Feature 001 at dispatch. Binary/WorkspaceRoot ingestion, retrieval receipts and Context Used presentation are not connected to this flow.
- The supervisor dispatches OpenCode only. A compatible OpenCode offer is possible only for exact CLI 2.0.32 with a complete probe and observed host stop, and is not qualification. Codex profile probing exists but Codex has no Conversation supervisor branch. Thus there is no qualified A→B→A flow; do not attribute this gap to an always-incompatible OpenCode offer.
- The current send package does not include the B-15 pinned Workspace instruction preamble or a typed exact-version ArtifactRef path. The prompt-window helper's omission value is not persisted or shown; no Context Used or relay refusal/truncation receipt may be displayed as complete.
- The storage loader returns at most 1,024 messages but first scans for orphan rows and runs COUNT/SUM across the entire committed prefix. The actual list of loaded rows is bounded; the total history work is not yet shown to be bounded.

## Required UI regression cases after implementation

- A→B→A on one Conversation: correct producer label on all three turns and no native-session identity reuse.
- A binding disabled, stale, incompatible, unauthenticated, or unsupported by the active supervisor: picker disabled/reason shown; draft preserved; zero fallback dispatch.
- A profile/descriptor refresh that changes compatibility while the picker is open: selection revalidated at admission and stale choice rejected.
- Resource attachment selected, rejected, revoked, or materialization unavailable: visible exact state; no false Context Used badge or silent omission.
- History pagination/reload after multiple providers: producer labels remain bound to the message's stored binding, not the currently selected agent.

## Validation

Validation must distinguish TARGET from the current partial UI. Source-level helper tests do not prove a rendered flow. Success requires a committed turn receipt, durable producer identity and a rendered label sourced from that identity.
