# UI behavior — {{FEATURE_NAME}} (behavior, not pixels)

**Status:** DRAFT  
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md).  
**Wire:** [LOCAL-OPERATOR-IPC.md](../../../docs/LOCAL-OPERATOR-IPC.md) → machine API in [operator-api.openapi.yaml](../../../docs/schemas/operator-api.openapi.yaml) and [SCHEMAS.md](../../../docs/SCHEMAS.md).

UI renders authoritative projections and stream frames; sends typed Operator commands. No English-string error parsing.

## 1. User flow and transitions

| Step | Screen / surface | Transition |
|---|---|---|
| | | |

## 2. UI states

| Surface | States | User-visible meaning |
|---|---|---|
| | | |

Include: empty, loading, error, partial, offline, long-running; plus domain states (STALE, AMBIGUOUS, NEEDS_AUTH, …).

## 3. Validation and failure presentation

| Validation | Rule | Typed failure / UI state |
|---|---|---|

## 4. Commands (UI → core)

| UI control | Local reducer action | OpenAPI operationId |
|---|---|---|

## 5. Events and stream (core → UI)

| Source | Type | UI use |
|---|---|---|
| Operator stream | OperatorStreamFrame ([PRESENTATION-RUNTIME.md](../../../docs/PRESENTATION-RUNTIME.md)) | |
| Read APIs | Projections | |

## 6. Error shapes

Link [SCHEMAS.md](../../../docs/SCHEMAS.md) for canonical error codes and [FAILURE-RECOVERY.md](../../../docs/FAILURE-RECOVERY.md) for recovery semantics.

## 7. Tests

List UIV1-* / AC-* / flow IDs and TASKS.md evidence commands.
