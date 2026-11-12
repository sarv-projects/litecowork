# UI behavior — Conversation Send and turn progress (Feature 001)

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification. Binds UI to owner contracts; not pixels.  
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md) (UIV1-016–022, 049–057), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md), [NATIVE-COMPOSER-LLD.md](../../../docs/NATIVE-COMPOSER-LLD.md).  
**Wire:** [LOCAL-OPERATOR-IPC.md](../../../docs/LOCAL-OPERATOR-IPC.md) → OpenAPI in [api-events.md](api-events.md).

UI **renders** conversation projections and stream frames; **sends** typed Operator commands. It does not parse English error strings.

## 1. User flow and transitions

| Step | Screen / surface | Transition |
|---|---|---|
| F1 | Conversation route, composer visible | User has eligible binding or sees repair state |
| F2 | User edits draft | Local reducer only; no domain mutation |
| F3 | Send | Admission command → turn OPEN/RUNNING/WAITING_* per state machine |
| F4 | Streaming | Stream frames + projection reconcile; scroll policy per shell LLD |
| F5 | Stop | Cancel command; terminal turn settlement |
| F6 | Failure / ambiguous | Typed error + repair; draft preserved where specified |
| F7 | Reload / restart app | Read models + RESYNC_REQUIRED if stream gap |

No navigation to Task or Coworker is required for F1–F7.

## 2. UI states (composer and message stream)

| Surface | States | User-visible meaning |
|---|---|---|
| Composer eligibility | `EMPTY`, `NEEDS_AGENT`, `AUTH_REQUIRED`, `ELIGIBLE`, `SENDING`, `WAITING`, `AMBIGUOUS`, `SETTLED`, `OFFLINE`, `STALE` | From binding descriptor + turn coordinator; disabled control **must** show reason ([V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md)) |
| Agent picker | `LOADING`, `READY`, `AUTH_REQUIRED`, `STALE` | Atomic refresh of model/mode/effort; draft text retained |
| Message stream | `LOADING`, `STREAMING`, `COMMITTED`, `DEGRADED`, `ERROR`, `PARTIAL` | Semantic message authoritative; rich draft optional and droppable |
| Runtime / admission | `UNAVAILABLE`, `DEGRADED` | Serving daemon ≠ qualified supervisor; fail-closed until feature verified |

## 3. Validation and failure presentation

| Validation | Rule | On failure (typed, not string parsing) |
|---|---|---|
| Attachments | Resource pins + agent descriptor support at send time | `UNSUPPORTED`, `STALE`, `DENIED` UI states; show repair link to context picker |
| Send while prior turn active | B-01 is LOCKED: strict single-flight; reject a concurrent submit with CONVERSATION_TURN_IN_PROGRESS (HTTP 409), preserve the draft and show the active-turn state | Operation error code + ledger state |
| Binding unavailable | No send; preserve draft | `NEEDS_AGENT` / `AUTH_REQUIRED` |
| Idempotency replay | Same `RequestId` returns same outcome or `AMBIGUOUS` lock | UI locks retry until reconcile ([errors-recovery.md](errors-recovery.md)) |
| UserRequest response | Immutable response + exact provider input | UIV1-140–147; AC-017–019 |

## 4. Commands (UI → core)

Map each control to **OpenAPI operationId** (see [api-events.md](api-events.md)). Transport: Tauri → [LOCAL-OPERATOR-IPC.md](../../../docs/LOCAL-OPERATOR-IPC.md) → same handlers as HTTP Operator.

| UI control (ledger) | Reducer action (local) | Domain command (typed) |
|---|---|---|
| Send | `TurnAdmissionRequested` / `TurnRequested` | Conversation message submit / turn admission (exact operationId in api-events) |
| Stop | `CancelRequested` | Turn cancel (owner API) |
| Retry | `RetryRequested` | Idempotent retry per owner rules |
| Agent change | `SelectedBindingChanged` | Binding read + descriptor refresh; send uses new binding only on new admission |
| UserRequest approve/deny | `UserRequestResponded` | UserRequest response commit (owner API) |

Local-only: draft text, scroll anchor, IME state — not persisted as domain events.

## 5. Events and stream (core → UI)

| Source | Type | UI use |
|---|---|---|
| Operator stream | `OperatorStreamFrame` variants ([PRESENTATION-RUNTIME.md](../../../docs/PRESENTATION-RUNTIME.md)) | Live token/draft/settlement; gap → `RESYNC_REQUIRED` |
| Read APIs | Conversation + turn projections | Hydration after navigation/restart |
| Domain events | e.g. `conversation.message.added.v1`, `conversation.turn.settled.v1` | Not consumed directly in WebView; materialize via projection/stream |

## 6. Error shapes

Use [SCHEMAS.md](../../../docs/SCHEMAS.md) and `schemas/error-codes.schema.json`. UI maps:

- HTTP/OpenAPI error body → toast/banner + control state
- Admission `failure_code` → composer eligibility
- `AMBIGUOUS` → pending lock UI (no duplicate send)

## 7. Tests (behavior IDs)

| ID | Scope |
|---|---|
| UIV1-016–022 | Composer send/stop/retry/eligibility |
| UIV1-023–036 | Agent picker + attachments + native palette |
| UIV1-049–057 | Stream reconcile, degraded rich presentation |
| AC-001–019 | Feature EARS in [SPEC.md](../SPEC.md) |
| FL01–FL05 | [V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](../../../docs/V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md) |

System/USER qualification requires the packaged desktop, daemon and exact real harness route ([ACCEPTANCE-AND-E2E-PLAN.md](../../../docs/ACCEPTANCE-AND-E2E-PLAN.md)); it is not established for this checkout.

## 8. Current source crosswalk — 2026-11-12

- The desktop Conversation page has durable turn projection, retryable-turn and version-aware Stop/Retry controls, all-nonterminal composer fencing, bounded stream reconciliation and text-only ResourceRef attachment picker/materialization. Its current Send computation is only dispatch eligibility: `isSendEligible` checks draft, Workspace presence, Operator readiness, the composer fence and `bindingEligible`; `buildConversationAgentChoices` defines that as a same-Workspace enabled lead-eligible binding whose profile provider key is OPENCODE. It does not check fresh descriptor/Runtime offer/auth/session readiness/safety eligibility or exact route qualification, so Send may enable for an unqualified tuple. The accepted disabled-until-qualified gate is not implemented and remains a release blocker (T09/AC-016). Stop is currently exposed only for RUNNING: the daemon cancellation route handles RUNNING and OPEN, but rejects WAITING_USER/WAITING_DEPENDENCY with 409 `turn_command_unavailable`; those states remain composer-fenced and the UI omits Stop. This is a current fail-closed limitation, not conformance with accepted AC-024; the route/state-machine/UI disagreement is open in register A-79. Unsupported voice, arbitrary actions, binary files and WorkspaceRoot attachments remain unavailable; the first message of a not-yet-created Conversation cannot carry these refs. Context Used receipts are not yet integrated.
- The Operator commits USER+OPEN, events, receipt and the v29 dispatch intent atomically and enforces same-Conversation single-flight. Submit/retry hand off to the thin `AgentTurnCoordinator`, which starts the supervisor asynchronously. The Supervisor claims the intent before STARTING and uses the OpenCode Server v2.0.32 path. This is CODE/SERVED, not a qualified provider journey.
- Stop and retry have daemon/storage paths and fixture coverage. Stop waits for observed quiescence, but an ambiguous OpenCode completion/interrupt race remains unresolved. Early supervisor errors during host/profile/claim/reservation can return without settling until daemon-start recovery. Retry creates a fresh ordinal/session and intent.
- Startup recovery settles unsettled prior-incarnation intents FAILED with `RUNTIME_RESTARTED`, releases orphan STARTING sessions LOST, and never replays an uncertain prompt. This is code-level recovery; no real process-kill or packaged restart qualification is recorded.
- `turn.delta` and `turn.activity` have bounded producers; activity is explicitly REPORTED and can be reloaded for the current/retryable turn. The pinned OpenCode event fixture is not a live provider capture. UserRequest continuation is blocked on an input-request event and approved B-06 mapping. Native permission is display-only; the pinned reply route is not integrated.
- Source status and qualification evidence are maintained in [IMPLEMENTATION-STATUS.md](../../../docs/IMPLEMENTATION-STATUS.md), [CURRENT-RUN.md](../../../CURRENT-RUN.md), and [TASKS.md](../TASKS.md). No Conversation route is QUALIFIED.

## 9. Cross-layer trace per user-visible operation (001-T21, 2026-11-10)

Chain per row: outcome, exact control, typed UI command, authenticated Operator route, authorization, service owner, transaction/version/fence, aggregate/event/receipt, projection/stream, restart/recovery, evidence. Evidence class is CODE unless stated; nothing here is QUALIFIED (T09 needs a live release-matrix run). The Tauri bridge maps the Conversation commands; the crate has compiled on the connected Windows host, but the packaged desktop journey remains unqualified.

| Operation | Control and command | Route and authorization | Owner, transaction, fence | Event, projection, recovery | Evidence |
|---|---|---|---|---|---|
| Send | Send button or Enter in the composer (`sendTurn`) -> `submit_conversation_turn` | `POST /v1/conversations/{id}/turns`; authenticated local peer, `X-Workspace-ID`, `Idempotency-Key`; Conversation must belong to the owner and Workspace; attachments reauthorized (`check_turn_resources`) | `AgentTurnCoordinator` after one transaction that commits the USER message, OPEN turn, dispatch intent and receipt; B-01 single-flight returns 409 `CONVERSATION_TURN_IN_PROGRESS` | `conversation.message.added.v1`; presentation snapshot and `turn.delta`/`turn.settled` frames; the durable dispatch intent is claimed once and `recover_conversation_sessions` runs at startup | `conversation_turn_submit_*`, `conversation_turn_delta_*`, `conversation_usage_*` |
| Stop | "Stop responding" button (`cancelTurn`) is offered only for projected RUNNING; WAITING_USER/WAITING_DEPENDENCY have no Stop control pending provider-aware support -> `cancel_conversation_turn` | `POST /v1/conversations/{id}/turns/{turn_id}/cancel`, `If-Match` = aggregate version, `Idempotency-Key` | RUNNING: Supervisor `request_cancel` signals the attempt; the turn moves to CANCEL_REQUESTED, and CANCELLED is committed only after observed quiescence. OPEN can be failed as not yet dispatched. Waiting states are rejected by the current handler (`turn_command_unavailable`, HTTP 409); target cancellation requires resolving the pending provider input/continuation safely. | RUNNING: `conversation.turn.status.changed.v1`, `conversation.turn.settled.v1`; an unconfirmed stop stays CANCEL_REQUESTED and is reconciled at restart. Waiting states have no supported Stop projection/control. | `conversation_cancel_commits_only_the_request_and_never_fabricates_cancelled`, `conversation_stop_signal_reaches_only_the_registered_attempt_and_is_released`; source cross-check of daemon waiting-state rejection |
| Retry | Retry button in the inline FAILED card (`retryTurn`) -> `retry_conversation_turn` | `POST /v1/conversations/{id}/turns/{turn_id}/retry`, `If-Match`, `Idempotency-Key` | FAILED -> OPEN with `retry_ordinal` + 1 and one new intent, no second USER message; stale `If-Match` is 412, a non-FAILED turn 409 | New attempt streams under its own ordinal; stale ordinals are dropped by the reducer | `conversation_stop_and_retry_failed_turn_lifecycle`, `conversation_retry_is_refused_while_another_turn_is_in_flight` |
| Reload | Opening or re-selecting a Conversation, `Load earlier messages`, stream reconnect -> `get_conversation_presentation`, `subscribe_conversation_stream` | `GET /v1/conversations/{id}/presentation`; authenticated stream with Workspace and Conversation scope | Read-only; cursor bound to Workspace and Conversation | Committed messages are authoritative; a sequence gap or `stream.resync_required` drops transient text and re-reads; a restarted app rebuilds from SQLite | presentation snapshot tests, `conversation-stream.test.ts`, `presentation-stream.test.ts` |
| Activity | Collapsed "Activity reported by the agent" card and plan line (`TurnActivityView`) -> `list_conversation_turn_activity` | `GET /v1/conversations/{id}/turns/{turn_id}/activity?retry_ordinal=` | Read-only; REPORTED records written by the supervisor's `TurnActivityRecorder` before each `turn.activity` frame | Not a DomainEvent; re-read for the active or failed turn after restart | `turn_activity_records_*` (daemon, storage), `turn-activity.test.ts` |
| Attach | "+" in the composer (`openPicker`) -> `list_resources`, then `submit_conversation_turn` with `resourceRefs` | Resource list route, then the Send route | `materialize_conversation_resources` reauthorizes each ref at dispatch; a refusal fails the turn before dispatch | Inlined text carries id, revision and digest; no ContextAttachment record yet (003-T09) | `conversation_resource_materialization_*`, `conversation-attachments.test.ts` |
| Answer UserRequest | **GAP, no hop exists.** Needs You answers UserRequests through `user_request_operator.rs`, but no Conversation turn creates a Conversation-scoped UserRequest because the OpenCode adapter surfaces no provider input request | `POST /v1/user-requests/{id}/respond` exists for other scopes | Domain WAITING_USER/WAITING_DEPENDENCY transitions exist, with no producer | none | open task: 001-T12 (BLOCKED) |
| Native permission | **PARTIAL.** `PermissionAsked` becomes a PERMISSION activity card worded "the agent asked for permission ... LiteCowork is not asking". No answer control: there is no verified answer channel (ACP `request_permission` is 002-T17; no OpenCode permission-reply capture) | none | none; never a LiteCowork grant or Effect | REPORTED record only | open task: 001-T27 (PARTIAL) |

