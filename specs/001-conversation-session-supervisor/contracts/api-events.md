# API, Event and Presentation Contract - Feature 001

**Status:** DRAFT; links to global owners are normative.  
**Canonical owners:** [API](../../../docs/API.md), [Events](../../../docs/EVENTS.md), [Schemas](../../../docs/SCHEMAS.md), [Storage](../../../docs/STORAGE.md), [Presentation Runtime](../../../docs/PRESENTATION-RUNTIME.md).

## 1. Existing submit route

`POST /v1/conversations/{id}/turns` accepts bounded `content[]` (at most 256 blocks, 1 MiB and 256 ResourceRefs) and optional `presentation_preference: AUTO | SIMPLE | RICH`. Submission uses principal-scoped `Idempotency-Key`; expected Conversation version is validated according to the API owner contract. The USER message and OPEN ConversationTurn are persisted before supervisor admission.

**Current active-source behavior:** when `ConversationApiState` has a supervisor, the Operator commits USER+OPEN and returns `202 Accepted` after starting detached `dispatch_turn`; exact replay returns the stored receipt. When the supervisor is absent, the admission-unavailable path settles FAILED and returns `503 DEPENDENCY_UNAVAILABLE`. A 202 currently means the turn was durably admitted, not that a provider prompt was accepted or that a native response will complete. The detached-dispatch crash window and logged-error paths remain unresolved.

Existing routes also include:
- `PATCH /v1/conversations/{id}/agent-binding` with If-Match and Idempotency-Key; affects future turns.
- `POST /v1/conversations/{id}/turns/{turn_id}/retry` and `.../cancel` under the owner API contract; implementation readiness must be verified before enabling UI.
- `GET /v1/conversations/{id}/presentation?cursor=&limit=` for bounded durable projection.
- `GET /v1/conversations/{id}/messages?cursor=` is declared; current implementation status must be checked from API owner before use.
- `GET /v1/conversations/{id}/usage` is target-only in the current contract; not part of this feature.

## 2. Admission request and receipt

- Authentication supplies principal identity; client body cannot select another owner.
- Workspace and Conversation identity are validated under authenticated scope.
- The idempotency key is stable for one submit payload and principal scope. Exact replay returns original commit; changed payload under same key conflicts.
- A committed USER message is retained when downstream admission fails. UI must distinguish saved-but-not-sent/failure from successful agent response.
- Binding is pinned by the immutable `conversation.turn.created.v2` event. Later selector/default changes never reroute it.
- No new public route or response field is approved by this DRAFT.

## 3. Existing event families

Reuse events already defined in EVENTS.md / schemas:
- `conversation.turn.created.v2` - immutable turn authority and presentation preference.
- `conversation.message.added.v1` - USER/AGENT semantic message addition and actual next aggregate revision.
- `agent.session.starting.v1` - durable reservation; no readiness claim.
- `agent.session.started.v1` - observed session readiness/activation; handle excluded.
- `agent.session.lost.v1`, `agent.session.closed.v1` - session lifecycle.
- `conversation.turn.status.changed.v1` - exact status transition, including OPEN -> RUNNING activation.
- `conversation.turn.settled.v1` - terminal turn settlement.
- `conversation.turn.retried.v1`, `conversation.turn.resumed.v1` - explicit fresh-session retry/continuation.

The exact event payload fields are governed by EVENTS.md and `docs/schemas/`. Do not invent new names or alter payloads only in this feature file. The activation storage update is not a separate public route.

## 4. Implemented final response commit and remaining gaps

B-04's storage operation exists in storage-core/SQLite and is called by the OpenCode supervisor. Its current transaction attempts to atomically write:
1. one immutable AGENT ConversationMessage with exact Conversation/Turn/AgentSession/AgentBinding provenance;
2. `conversation.message.added.v1` with committed revision;
3. terminal ConversationTurn projection and `conversation.turn.settled.v1`;
4. both aggregate snapshots and the response idempotency receipt.

The current `ConversationTurnStore` exposes `commit_conversation_agent_response`, and the OpenCode supervisor invokes it. The typed request does not yet carry explicit retry ordinal/provider-response identity, and author JSON is passed through from the caller; verify/strengthen these provenance checks. Existing tests do not establish all rollback, conflicting-replay, wrong-scope and crash-window cases. This is CODE, not QUALIFIED.

## 5. Presentation and streaming

The durable presentation snapshot/upsert cursor is the canonical reconnect mechanism. Reconnect must re-read an authorized projection. Tauri bridge applies bounded backpressure and resumes by opaque cursor. Duplicate semantic message IDs are idempotently reduced; version/cursor gaps trigger snapshot reconciliation.

If transient `turn.delta` frames are added, their payload must be bounded and carry exact Workspace, Conversation, turn, retry ordinal, AgentSession and monotonic per-session/turn sequence identity. They are local, ephemeral and not domain events. Do not persist partial delta as ConversationMessage. A gap may be replayed only if the adapter guarantees stable replay; otherwise discard delta buffer and resync. A final committed message supersedes any transient text.

Current local Operator does not emit `turn.delta`, `turn.settled` or `rich.draft`. This must be updated only after an actual producer/contract/UI path is integrated and qualified.

## 6. Security contract

No native handle, provider credential, cookie, hidden provider history or unredacted raw provider frame may appear in route payload, event/snapshot, cursor page, browser bridge, diagnostics or logs. ResourceRefs require separate owner/Workspace/revision authorization; no resource permission is implied by a transcript reference. Native provider events remain untrusted and cannot create Effects, approvals or trust grants.

