# API, Event and Presentation Contract - Feature 001

**Status:** DRAFT; links to global owners are normative.  
**Canonical owners:** [API](../../../docs/API.md), [Events](../../../docs/EVENTS.md), [Schemas](../../../docs/SCHEMAS.md), [Storage](../../../docs/STORAGE.md), [Presentation Runtime](../../../docs/PRESENTATION-RUNTIME.md).

## 1. Existing submit route

`POST /v1/conversations/{id}/turns` accepts bounded `content[]` (at most 256 blocks, 1 MiB and 256 ResourceRefs) and optional `presentation_preference: AUTO | SIMPLE | RICH`. Submission uses principal-scoped `Idempotency-Key`; expected Conversation version is validated according to the API owner contract. The USER message and OPEN ConversationTurn are persisted before supervisor admission.

**Current mounted behavior:** while the native supervisor is unavailable, the server settles the turn FAILED with `CONVERSATION_SESSION_ADMISSION_UNAVAILABLE` and returns `503 DEPENDENCY_UNAVAILABLE`. This is not a successful send and must not be rendered as one. The feature must not change that behavior to a 202/accepted status until real supervisor and result writer are connected.

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

## 4. Proposed final response commit

B-04 requires a storage contract to atomically write:
1. one immutable AGENT ConversationMessage with exact Conversation/Turn/AgentSession/AgentBinding provenance;
2. `conversation.message.added.v1` with committed revision;
3. terminal ConversationTurn projection and `conversation.turn.settled.v1`;
4. both aggregate snapshots and the response idempotency receipt.

The current ConversationTurnStore port does not expose this final-response commit. The proposed operation and payload shape in SPEC.md are not current API. Storage must verify event versions/payloads and the existing tables before approving exact fields.

## 5. Presentation and streaming

The durable presentation snapshot/upsert cursor is the canonical reconnect mechanism. Reconnect must re-read an authorized projection. Tauri bridge applies bounded backpressure and resumes by opaque cursor. Duplicate semantic message IDs are idempotently reduced; version/cursor gaps trigger snapshot reconciliation.

If transient `turn.delta` frames are added, their payload must be bounded and carry exact Workspace, Conversation, turn, retry ordinal, AgentSession and monotonic per-session/turn sequence identity. They are local, ephemeral and not domain events. Do not persist partial delta as ConversationMessage. A gap may be replayed only if the adapter guarantees stable replay; otherwise discard delta buffer and resync. A final committed message supersedes any transient text.

Current local Operator does not emit `turn.delta`, `turn.settled` or `rich.draft`. This must be updated only after an actual producer/contract/UI path is integrated and qualified.

## 6. Security contract

No native handle, provider credential, cookie, hidden provider history or unredacted raw provider frame may appear in route payload, event/snapshot, cursor page, browser bridge, diagnostics or logs. ResourceRefs require separate owner/Workspace/revision authorization; no resource permission is implied by a transcript reference. Native provider events remain untrusted and cannot create Effects, approvals or trust grants.

