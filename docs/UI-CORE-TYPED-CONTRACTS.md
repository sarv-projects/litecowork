# UI and Core Typed Contracts

**Status:** normative companion — behavior and wire shapes, not pixels  
**Updated:** 2026-11-01  
**Authority:** subordinate to [API.md](API.md), [SCHEMAS.md](SCHEMAS.md), [EVENTS.md](EVENTS.md), [LOCAL-OPERATOR-IPC.md](LOCAL-OPERATOR-IPC.md), [PRESENTATION-RUNTIME.md](PRESENTATION-RUNTIME.md). This file indexes how the desktop UI talks to Core; it does not add routes or error codes.

## 1. Design rule

The UI **renders authoritative state** and **sends typed commands**. It does not own durable domain truth, parse English error strings, or invent side effects. Local reducer state (draft text, scroll, focus, layout prefs) is not replicated as domain events.

See [FEATURE-AND-UI-CONTRACT-COVERAGE.md](FEATURE-AND-UI-CONTRACT-COVERAGE.md) for the full engineering checklist and per-feature `contracts/ui-behavior.md` requirement.

## 2. UI → Core: typed commands and queries

| Layer | Contract | Owner |
|---|---|---|
| Logical API | OpenAPI 3.1 `operationId`, request/response schemas, `RequestId`, `If-Match`, idempotency | [API.md](API.md), `schemas/operator-api.openapi.yaml` |
| Desktop transport | OS-authenticated framed IPC to the same Operator handlers | [LOCAL-OPERATOR-IPC.md](LOCAL-OPERATOR-IPC.md) |
| Native shell | Narrow Tauri commands; no secrets or arbitrary paths in WebView | [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md), [NETWORK-SECURITY.md](NETWORK-SECURITY.md) |
| Workspace scope | `X-Workspace-ID` / IPC equivalent on every scoped call | [PROTOCOLS.md](PROTOCOLS.md) §1 |

**UI implementation rule:** map each button/menu to exactly one `operationId` (or a documented local-only reducer action with no domain mutation). Settings mutations follow the typed pattern in [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md) (`SettingsMutation`).

**Queries:** use Operator read routes and projections defined in owner services; do not bypass Core to reach SQLite or agent native APIs from the WebView.

## 3. Core → UI: typed push and refresh

| Mechanism | Purpose | Schema / doc |
|---|---|---|
| **Operator stream** | Live turn progress, projections, resync | `OperatorStreamFrame` in [PRESENTATION-RUNTIME.md](PRESENTATION-RUNTIME.md) |
| **REST read models** | Hydration after navigation, restart, stream gap | OpenAPI response schemas + service projections in [SERVICES.md](SERVICES.md) |
| **Domain events** | Durable truth | [EVENTS.md](EVENTS.md), `schemas/domain-event.schema.json` — consumed by Core/projections, **not** raw in WebView |

Stream frame variants (closed set):

```text
STREAM_READY | PROJECTION | TURN_DELTA | TURN_SETTLED | RICH_DRAFT | RESYNC_REQUIRED
```

UI reducer actions in [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md) and [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md) are **local** until explicitly promoted; they must not be mistaken for domain event names ([EVENTS.md](EVENTS.md)).

## 4. Shared error shapes

| Source | UI must use |
|---|---|
| API errors | OpenAPI error body + stable `failure_code` / error enum |
| Taxonomy | [SCHEMAS.md](SCHEMAS.md), `schemas/error-codes.schema.json` (131 codes) |
| Admission / turn | Typed admission unavailable and turn settlement codes — not string matching |
| Ambiguous / retry | Lock UI on `AMBIGUOUS`; reconcile with same idempotency key ([FAILURE-RECOVERY.md](FAILURE-RECOVERY.md)) |

Universal presentation rules: [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md) § Universal UI state contract.

## 5. Per-feature UI behavior specs

Each `specs/NNN-*/contracts/ui-behavior.md` must list:

1. Flows and screen transitions  
2. States: empty, loading, error, partial, offline, long-running (+ domain states)  
3. Validation and typed failure presentation  
4. Table: control → reducer action → `operationId`  
5. Stream/projection usage  
6. Test IDs (`UIV1-*`, feature AC-*)

Template: [.specify/templates/contracts/ui-behavior.md](../.specify/templates/contracts/ui-behavior.md). Example: [specs/001-conversation-session-supervisor/contracts/ui-behavior.md](../specs/001-conversation-session-supervisor/contracts/ui-behavior.md).

## 6. Testability without UI

Core remains testable by:

- Calling Operator routes (HTTP or IPC test harness) with fixture identities  
- Asserting domain events, storage rows and error codes  
- Running stream contract tests on `OperatorStreamFrame` producers  

UI tests (USER) assert reducer mapping and accessibility; they do not redefine domain semantics ([TESTING.md](TESTING.md), [ACCEPTANCE-AND-E2E-PLAN.md](ACCEPTANCE-AND-E2E-PLAN.md)).

## 7. Research is not frozen

Competitor IPC/bridge patterns (AionUi, OpenWork, OpenDots templates, etc.) inform **tests and shape** only. Re-validate pinned SHAs in feature `RESEARCH.md` before **ACCEPTED/LOCKED** ([COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md), [REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md)).
