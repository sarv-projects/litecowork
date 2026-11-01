# UI behavior - Native Agent Install, Auth and Binding

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Flows and states

- Settings > Agent Registry lists distributions, then separate install / auth / binding / readiness.
- Composer shows model and effort only after a fresh descriptor advertises them.
- Unavailable route shows repair text, not a disabled send that looks successful.

## Commands

The UI renders owner state and sends the Operator command named by the canonical API. It does not encode a second state machine. TARGET controls stay disabled until a served handler exists.

## Validation

Empty, denied, stale version, offline and unqualified provider states are visible. Success requires a receipt or a verified observation.
