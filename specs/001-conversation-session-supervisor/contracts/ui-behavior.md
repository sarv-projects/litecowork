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
| Send while prior turn active | B-01 policy (reject vs queue) — until locked, show blocking reason from admission response | Operation error code + ledger state |
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

System/USER qualification requires packaged desktop + daemon + real harness ([ACCEPTANCE-AND-E2E-PLAN.md](../../../docs/ACCEPTANCE-AND-E2E-PLAN.md)); not claimed in docs-first checkout.
