# API, Event and Presentation Contract - Feature 001

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification. Links to global owners are normative.  
**Canonical owners:** [API](../../../docs/API.md), [Events](../../../docs/EVENTS.md), [Schemas](../../../docs/SCHEMAS.md), [Storage](../../../docs/STORAGE.md), [Presentation Runtime](../../../docs/PRESENTATION-RUNTIME.md).

## 1. Existing submit route

`POST /v1/conversations/{id}/turns` accepts bounded `content[]` (at most 256 blocks, 1 MiB and 256 ResourceRefs) and optional `presentation_preference: AUTO | SIMPLE | RICH`. Submission uses principal-scoped `Idempotency-Key`; expected Conversation version is validated according to the API owner contract. The USER message and OPEN ConversationTurn are persisted before supervisor admission.

**Current active-source behavior:** the Operator commits USER+OPEN, events, receipt and the v29 dispatch intent in one SQLite transaction, with the same-Conversation single-flight guard. It returns `202 Accepted` after handing the committed turn to `AgentTurnCoordinator`, which starts the supervisor asynchronously; the supervisor claims the intent using the dispatching Runtime incarnation before host/binding/profile preflight or STARTING reservation; only a confirmed claimant may settle setup failures, while a losing or ambiguous claim cannot settle the turn and remains for reconciliation. Exact replay returns the stored receipt and does not authorize a second prompt. When the supervisor is unavailable, the admission-unavailable path settles FAILED and returns `503 DEPENDENCY_UNAVAILABLE`. A 202 means the turn was durably admitted, not that a provider prompt was accepted or that a native response will complete. Startup recovery settles prior-incarnation unsettled intents FAILED rather than replaying uncertain prompts. T20 CODE now maps early setup/provider failures to closed diagnostic classes, attempts exact turn/session settlement, retries transient storage errors up to three times, and relies on startup recovery when settlement still cannot commit; no provider/storage detail is emitted by the structured dispatch diagnostic. Provider acceptance, exhaustive fault/recovery evidence and packaged qualification remain separate.

**Exact-replay dispatch guard (current CODE):** the Operator can hand an idempotent replay to the coordinator again. Before dispatch work, the supervisor keeps one in-process owner for the exact Workspace/Conversation/turn/retry attempt; a duplicate invocation during that worker's lifetime does not run preflight or settle its attempt. The durable dispatch-intent compare-and-set, claimed by the dispatching Runtime incarnation before host/binding/profile reads, is the cross-Runtime owner fence. A non-winning or ambiguous claim cannot settle; startup recovery reconciles durable intents left behind. Focused tests cover the registry, setup-failure and recovery paths; a packaged concurrent HTTP replay remains unqualified.

**A-113 / T30 exact client replay (USER-DELEGATED direction, revision 1.2 IN_REVIEW):** after authenticated Workspace/Conversation ownership is established and the request is syntactically valid, the exact principal/key/payload receipt is looked up before mutable Resource availability or fresh-admission checks. An exact receipt returns the immutable admission receipt captured at commit (normally status `OPEN`) and cannot start another dispatch, even if the current turn has since moved to another lifecycle state; the desktop reloads the canonical projection for current state. A changed body under the same key conflicts. For an exact match, a supplied binding must equal the immutable binding pinned in `conversation.turn.created.v2`. Fresh admission, with no receipt, still performs current Resource/binding checks and atomically commits the existing USER/OPEN/intent/receipt transaction. This ordering permits the desktop to recover a previously committed send even if a Resource was later revoked. The user-delegated contract does not replace the canonical authorization boundary or add a new route/schema/event.

Existing routes also include:
- `PATCH /v1/conversations/{id}/agent-binding` with If-Match and Idempotency-Key; affects future turns.
- `POST /v1/conversations/{id}/turns/{turn_id}/retry` and `.../cancel` under the owner API contract; implementation readiness must be verified before enabling UI.
- `GET /v1/conversations/{id}/presentation?cursor=&limit=` for bounded durable projection.
- `GET /v1/conversations/{id}/messages?cursor=` is declared; current implementation status must be checked from API owner before use.
- `GET /v1/conversations/{id}/usage` is target-only in the current contract; not part of this feature.

### Atomic first turn for New Chat

`POST /v1/conversations/first-turn` accepts the create fields plus the same bounded `content[]`, optional `resource_refs[]`, the exact eligible `agent_binding_id` displayed by the New Chat selector, and `presentation_preference` used by normal turn admission. The UI materializes the visible Workspace-default choice to its exact binding ID before journaling, so restart replay cannot drift after a default change. It requires the authenticated local peer, selected `X-Workspace-ID` matching `workspace_id`, an ACTIVE Workspace owned by the principal and a principal-scoped `Idempotency-Key`. The exact binding is persisted as the new Conversation's initial override and is rechecked for same-Workspace, enabled and lead-eligible status in the transaction. It returns `202` with `{conversation, turn}` only after the create, USER message, OPEN turn, canonical events, receipt and UNCLAIMED dispatch intent commit together. The Conversation's returned version is 2. The turn receipt carries turn/message identity and status OPEN. No provider call is made inside the storage transaction.

Exact retry uses the same key and byte-equivalent canonical payload and returns the same Conversation and turn receipt without a second message, event or intent. A changed payload under that key is a conflict. Refusals leave no Conversation, USER message, turn, event or receipt. After commit, the route hands the admitted turn to the existing coordinator; a lost response is safe to replay from the private Tauri journal. The existing `POST /v1/conversations` remains create-only.

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

This final-result operation accepts only terminal outcomes (`COMPLETED`, `FAILED`, `CANCELLED`). UserRequest `WAITING_USER` and `WAITING_DEPENDENCY` transitions use the B-06 owner operations and are rejected at this boundary before any write; an answer receipt cannot settle a waiting turn or close its active session. The A-93 source correction is user-delegated implementation direction recorded 2026-11-19, not an owner re-acceptance or live-provider continuation evidence.

## 5. Presentation and streaming

The durable presentation snapshot/upsert cursor is the canonical reconnect mechanism. Reconnect must re-read an authorized projection. Tauri bridge applies bounded backpressure and resumes by opaque cursor. Duplicate semantic message IDs are idempotently reduced; version/cursor gaps trigger snapshot reconciliation.

The implemented transient `turn.delta` frames follow this contract: their payload must be bounded and carry exact Workspace, Conversation, turn, retry ordinal, AgentSession and monotonic per-session/turn sequence identity. They are local, ephemeral and not domain events. Do not persist partial delta as ConversationMessage. A gap may be replayed only if the adapter guarantees stable replay; otherwise discard delta buffer and resync. A final committed message supersedes any transient text.

### T25 durable activity recovery

The authenticated `ConversationPresentationSnapshot` carries nullable `activity_turn`, the latest turn's exact `turn_id`, `retry_ordinal` and closed lifecycle `status`, read in the same SQLite transaction as the page and source frontier. The desktop uses this tuple only to request the existing per-turn activity page and to merge transient `turn.activity` frames with identical Conversation/turn/retry identity. It does not use this field for Stop, Retry, permission liveness, Effect truth or provider identity. `active_turn` and `retryable_turn` remain separate projections with their existing command authority. A current daemon snapshot must include the field, including explicit `null` when no turn exists. The client tolerates omission for legacy daemon responses and local empty snapshots; for those responses it can fall back to active/retryable identities, while completed-turn activity remains unavailable until a current snapshot is served. Activity fetch failure remains non-fatal to the semantic transcript. This restores the latest completed turn's REPORTED activity after reload without changing activity persistence, adding a migration, or reconstructing older turn history.

### T31 exact message turn lineage (A-114)

Each `ConversationPresentationMessage` wrapper includes required nullable `turn_status` alongside `message`, `rich_presentation` and `linked_items`. The Operator reads the status by the message's immutable `turn_id` and exact `conversation_id` in the same SQLite snapshot transaction; current daemon replies include explicit `null` when no exact owning turn exists. The same wrapper is used for HTTP pages, stream snapshots and `conversation_projection_upsert`, so the generated OpenAPI and operator-stream schemas share the field. A legacy response that omits the field is accepted by the client only as unknown/null and can never enable an action. `turn_status` is read-only lineage metadata, not command authority or a second status record. Feature 005 promotion uses it only as a UI visibility condition; its Task create transaction independently validates message append events, the exact ordered USER/AGENT pair and COMPLETED turn.

### T27 snapshot/reply operations and transient frame

The `GET .../turns/{turn_id}/permissions` snapshot and `POST .../permissions/{prompt_token}/reply` operations are SERVED partial for an authenticated owner and exact persisted turn retry ordinal. GET reads only the Runtime-local registry and does not reconstruct state from history. POST revalidates the persisted RUNNING turn and ACTIVE AgentSession, then submits the closed decision through a bounded mailbox to the active adapter owner. Neither operation mutates SQLite Conversation state. The client token is opaque and scoped to the authenticated Workspace, Conversation, exact turn/retry, AgentSession and Runtime incarnation. The reply body is closed to `ALLOW_ONCE` and `REJECT_SESSION_PENDING`; the latter is explicitly session-wide for requests pending when OpenCode applies it. Native permission request/session IDs and absolute raw paths are absent from API and stream payloads; a bounded, validated existing Workspace-relative display label may be included, otherwise the projection is generic. The registry caps one attempt at 32 prompts and the Runtime at 256 retained prompts; mailbox teardown discards the exact ended attempt's prompts and tokens so sequential attempts do not exhaust the Runtime cap. Active scopes are not evicted. Negative route tests and synthetic registry/reconciliation tests pass; there is no successful route-to-real-provider evidence.

`turn.permission` is ephemeral and carries Workspace/Conversation/turn/retry correlation, a bounded sequence, the opaque prompt token, current state, sanitized action/explanation and bounded resource display names. A normalized-event producer and private registry emit the initial PENDING frame as CODE; the Runtime displays an existing resource as a Workspace-relative label only after bounded path validation under the canonical active Workspace root; outside-root absolute paths use a generic external label, while missing, linked/reparse or otherwise uncertain paths use generic labels. No absolute path is projected. Duplicate notifications republish current state, and reply-command transitions publish sequenced REPLYING and resolved-state frames. Reconciliation that proves a request is still pending clears the attempted decision; an absent/unreadable request is `OUTCOME_UNKNOWN` with no action. If the attempt later closes, `SESSION_ENDED` preserves the attempted decision only as unconfirmed context. Attempt mailbox teardown clears that exact scope to reclaim bounded capacity; mailbox teardown publishes sequenced SESSION_ENDED frames before exact-scope removal. It is not a DomainEvent, receipt, grant, Effect or durable activity record. Reconnect can call the served live snapshot route while the owning turn/session remains active; persisted `PERMISSION/ASKED` remains historical REPORTED activity. See [native permission design](native-permission-design.md) for transitions, provider reconciliation and recovery. typed Tauri commands and inline/Needs You consumers have CODE coverage; real-provider behavior, packaged journey and qualification remain absent.

`transient_frames.rs` implements bounded turn.delta and turn.settled producers consumed by the supervisor/Operator stream path. A production rich.draft publisher and full native-provider qualification remain open under Feature 016 and T09. Source producers are not SYSTEM/USER evidence.

## 6. Security contract

No native handle, provider credential, cookie, hidden provider history or unredacted raw provider frame may appear in route payload, event/snapshot, cursor page, browser bridge, diagnostics or logs. ResourceRefs require separate owner/Workspace/revision authorization; no resource permission is implied by a transcript reference. Native provider events remain untrusted and cannot create Effects, approvals or trust grants.

## 7. OpenCode v2.0.32 adapter event contract

This is an adapter contract for the pinned native provider, not a LiteCowork public event schema. The upstream v2.0.32 envelope carries `id`, `created`, `type`, and `data`; `metadata` and `location` are optional, and durable provider events also carry a durable aggregate cursor. The normalizer reads `type` and `data` only. It correlates session-scoped events exclusively through `data.sessionID`; it never treats `location`, the SSE `event:` value, or legacy top-level aliases as session identity or event type.

| OpenCode event family | LiteCowork internal observation | Boundary |
|---|---|---|
| `session.text.delta`, `session.text.ended` | `TextDelta`, `TextEnded` | Bounded text is transient until the Conversation response transaction commits. |
| `session.reasoning.started`, `session.reasoning.ended` | Hidden reasoning boundaries | Reasoning delta and content are not returned or projected. |
| `session.tool.input.started`, `.progress`, `.success`, `.failed` | Tool start/progress/settlement | Native harness activity is REPORTED; it is not a LiteCowork Effect receipt. |
| `permission.asked` | Bounded `PermissionAsked` | Native permission remains distinct from LiteCowork Trust grants and UserRequest continuation. |
| `session.step.started`, `.ended`, `.failed` | Step start/end/failure; per-step token counts from terminal step events | Token counts are native-reported per-step usage, not billing authority; `files[]` does not prove a mediated file Effect. |
| `session.usage.updated` | `UsageSnapshot` with cumulative native-session totals | This is a session-wide snapshot, not a per-turn delta; the per-turn usage collector must not add it to step usage. |
| `session.execution.started`, `.succeeded`, `.failed`, `.interrupted` | Execution start and terminal observation | Session correlation is mandatory; Stop still requires observed cancellation/quiescence rules. |

Known, correlated events without a LiteCowork projection are ignored; unknown event names, missing envelope fields, foreign sessions and uncorrelated session events are dropped and counted. The pinned OpenCode adapter does not produce LiteCowork `TodoUpdated` or `FileChanged` observations: `todo.updated` and `file.edited` are not in the pinned v2.0.32 session-event catalogue. A different provider may map equivalent supported events through the closed internal enum only after its own pinned contract and tests. Do not convert a step's `files[]` list to a verified change or Effect. Extend mappings only from a pinned provider contract and a same-change test; real provider capture remains T23 evidence.

OpenCode documents `/api/event` as volatile: events may be missed during disconnection or when a consumer falls behind. The adapter therefore establishes a successful event subscription before dispatching the prompt and passes that same stream to the turn driver. If subscription fails, do not send the prompt. If prompt dispatch fails, drop the stream. Timeout-after-prompt ambiguity remains governed by dispatch reconciliation; this ordering change does not authorize a prompt retry.

The fixture `apps/litecoworkd/src/agents/fixtures/opencode_v2_0_32_contract_events.jsonl` is synthetic, schema-derived test data. It is not a captured turn and does not satisfy T23 or qualify a provider journey. Exact pinned source references and the evidence distinction are in [RESEARCH.md §8.2](../RESEARCH.md#82-pinned-opencode-v2032-event-envelope-and-volatile-feed).

