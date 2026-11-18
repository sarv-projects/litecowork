# API, Event and Presentation Contract - Feature 001

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification. Links to global owners are normative.  
**Canonical owners:** [API](../../../docs/API.md), [Events](../../../docs/EVENTS.md), [Schemas](../../../docs/SCHEMAS.md), [Storage](../../../docs/STORAGE.md), [Presentation Runtime](../../../docs/PRESENTATION-RUNTIME.md).

## 1. Existing submit route

`POST /v1/conversations/{id}/turns` accepts bounded `content[]` (at most 256 blocks, 1 MiB and 256 ResourceRefs) and optional `presentation_preference: AUTO | SIMPLE | RICH`. Submission uses principal-scoped `Idempotency-Key`; expected Conversation version is validated according to the API owner contract. The USER message and OPEN ConversationTurn are persisted before supervisor admission.

**Current active-source behavior:** the Operator commits USER+OPEN, events, receipt and the v29 dispatch intent in one SQLite transaction, with the same-Conversation single-flight guard. It returns `202 Accepted` after handing the committed turn to `AgentTurnCoordinator`, which starts the supervisor asynchronously; the supervisor claims the intent using the dispatching Runtime incarnation before host/binding/profile preflight or STARTING reservation; only a confirmed claimant may settle setup failures, while a losing or ambiguous claim cannot settle the turn and remains for reconciliation. Exact replay returns the stored receipt and does not authorize a second prompt. When the supervisor is unavailable, the admission-unavailable path settles FAILED and returns `503 DEPENDENCY_UNAVAILABLE`. A 202 means the turn was durably admitted, not that a provider prompt was accepted or that a native response will complete. Startup recovery settles prior-incarnation unsettled intents FAILED rather than replaying uncertain prompts. T20 CODE now maps early setup/provider failures to closed diagnostic classes, attempts exact turn/session settlement, retries transient storage errors up to three times, and relies on startup recovery when settlement still cannot commit; no provider/storage detail is emitted by the structured dispatch diagnostic. Provider acceptance, exhaustive fault/recovery evidence and packaged qualification remain separate.

**Exact-replay dispatch guard (current CODE):** the Operator can hand an idempotent replay to the coordinator again. Before dispatch work, the supervisor keeps one in-process owner for the exact Workspace/Conversation/turn/retry attempt; a duplicate invocation during that worker's lifetime does not run preflight or settle its attempt. The durable dispatch-intent compare-and-set, claimed by the dispatching Runtime incarnation before host/binding/profile reads, is the cross-Runtime owner fence. A non-winning or ambiguous claim cannot settle; startup recovery reconciles durable intents left behind. Focused tests cover the registry, setup-failure and recovery paths; a packaged concurrent HTTP replay remains unqualified.

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
- Current CODE includes CONVERSATION_TURN_IN_PROGRESS, version/retry-aware active-turn projection, internal final-response identity, dispatch intent and turn usage records. TASKS T01 tracks remaining public contract promotion; an internal implemented field is not automatically a promoted API or qualification claim.

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

The current `ConversationTurnStore` exposes `commit_conversation_agent_response`, and the OpenCode supervisor invokes it. The typed request carries explicit retry_ordinal/provider_response_id and checks them at commit. Author JSON/provenance validation and the complete negative/recovery matrix still require their owning task evidence. Existing tests do not establish all rollback, conflicting-replay, wrong-scope and crash-window cases. This is CODE, not QUALIFIED.

## 5. Presentation and streaming

The durable presentation snapshot/upsert cursor is the canonical reconnect mechanism. Reconnect must re-read an authorized projection. Tauri bridge applies bounded backpressure and resumes by opaque cursor. Duplicate semantic message IDs are idempotently reduced; version/cursor gaps trigger snapshot reconciliation.

The implemented transient `turn.delta` frames follow this contract: their payload must be bounded and carry exact Workspace, Conversation, turn, retry ordinal, AgentSession and monotonic per-session/turn sequence identity. They are local, ephemeral and not domain events. Do not persist partial delta as ConversationMessage. A gap may be replayed only if the adapter guarantees stable replay; otherwise discard delta buffer and resync. A final committed message supersedes any transient text.

### T27 snapshot/reply operations and transient frame

The `GET .../turns/{turn_id}/permissions` snapshot and `POST .../permissions/{prompt_token}/reply` operations are SERVED partial for an authenticated owner and exact persisted turn retry ordinal. GET reads only the Runtime-local registry and does not reconstruct state from history. POST revalidates the persisted RUNNING turn and ACTIVE AgentSession, then submits the closed decision through a bounded mailbox to the active adapter owner. Neither operation mutates SQLite Conversation state. The client token is opaque and scoped to the authenticated Workspace, Conversation, exact turn/retry, AgentSession and Runtime incarnation. The reply body is closed to `ALLOW_ONCE` and `REJECT_SESSION_PENDING`; the latter is explicitly session-wide for requests pending when OpenCode applies it. Native IDs and paths are absent from API and stream payloads. The registry caps one attempt at 32 prompts and the Runtime at 256 retained prompts; mailbox teardown discards the exact ended attempt's prompts and tokens so sequential attempts do not exhaust the Runtime cap. Active scopes are not evicted. Negative route tests and synthetic registry/reconciliation tests pass; there is no successful route-to-real-provider evidence.

`turn.permission` is ephemeral and carries Workspace/Conversation/turn/retry correlation, a bounded sequence, the opaque prompt token, current state, sanitized action/explanation and bounded resource display names. A normalized-event producer and private registry emit the initial PENDING frame as CODE; generic resource labels remain until safe Workspace-relative display logic is implemented. Duplicate notifications republish current state, and reply-command transitions publish sequenced REPLYING and resolved-state frames. Reconciliation that proves a request is still pending clears the attempted decision; an absent/unreadable request is `OUTCOME_UNKNOWN` with no action. Attempt mailbox teardown clears that exact scope to reclaim bounded capacity; mailbox teardown publishes sequenced SESSION_ENDED frames before exact-scope removal. It is not a DomainEvent, receipt, grant, Effect or durable activity record. Reconnect can call the served live snapshot route while the owning turn/session remains active; persisted `PERMISSION/ASKED` remains historical REPORTED activity. See [native permission design](native-permission-design.md) for transitions, provider reconciliation and recovery. typed Tauri commands and inline/Needs You consumers have CODE coverage; real-provider behavior, packaged journey and qualification remain absent.

`transient_frames.rs` implements bounded turn.delta and turn.settled producers consumed by the supervisor/Operator stream path. A production rich.draft publisher and full native-provider qualification remain open under Feature 016 and T09. Source producers are not SYSTEM/USER evidence.

## 6. Security contract

No native handle, provider credential, cookie, hidden provider history or unredacted raw provider frame may appear in route payload, event/snapshot, cursor page, browser bridge, diagnostics or logs. ResourceRefs require separate owner/Workspace/revision authorization; no resource permission is implied by a transcript reference. Native provider events remain untrusted and cannot create Effects, approvals or trust grants.

