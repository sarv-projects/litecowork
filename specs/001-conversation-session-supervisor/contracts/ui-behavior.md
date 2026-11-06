# UI behavior — Conversation Send and turn progress (Feature 001)

**Status:** DRAFT — binds UI to owner contracts; not pixels.  
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

## 8. Current source crosswalk — 2026-11-06

- The mounted Conversation surface is ConversationsPage.tsx. Recent Chats and message history are cursor-paginated; Workspace/list-epoch fencing, conversation-ID deduplication, local per-Conversation drafts and the unreadable pending-create journal pause/retry are implemented.
- The active-turn projection exposes status only, not aggregate version. The composer treats only RUNNING and presentation-stream transient turns as running; OPEN, WAITING_DEPENDENCY and CANCEL_REQUESTED are not consistently fenced. A 202 response can clear the local sending flag while the durable turn remains OPEN. The backend submit transaction does not enforce B-01 strict single-flight.
- Stop is shown for RUNNING, but the handler searches only the presentation-stream transient-turn collection. The OpenCode Operator path does not emit those turns. If one exists, the call hardcodes expectedVersion=1. The Operator cancel route transitions RUNNING to CANCEL_REQUESTED and immediately to CANCELLED without a native interrupt or observed quiescence.
- The Operator retry route exists, but the UI has no retry call site or failed-turn affordance. The route transitions FAILED to RUNNING with a new session pointer before calling a dispatcher reservation that requires OPEN and no session pointer; retry is not integrated correctly.
- The agent-binding selector and Send are wired, but Send remains enabled despite the target disabled-until-qualified gate; it does not require a qualified binding/provider or Operator readiness and can submit with no Workspace default binding, relying on a backend error. The Add context/file and Voice Input buttons and More options button have no handlers; Turbo is static; the @/slash composer copy has no mention/action implementation. Submit always sends resource_refs=[]; there is no attachment picker or Context Used UI.
- The empty Conversation canvas is hard-coded Antigravity branding and capability copy, not a LiteCowork capability inventory.
- Production ConversationApiState::from_ref constructs AgentSessionSupervisor and submit/retry call dispatch_turn. The module header in conversation_operator.rs still says dispatch is intentionally absent; that comment is stale. The actual dispatch branch is OpenCode-only and remains unqualified.
- See [IMPLEMENTATION-STATUS.md](../../../docs/IMPLEMENTATION-STATUS.md), [SETTINGS-AND-SHELL-SURFACE-AUDIT.md](../../../docs/SETTINGS-AND-SHELL-SURFACE-AUDIT.md) and Feature 001 SPEC for the current source/owner crosswalk.
