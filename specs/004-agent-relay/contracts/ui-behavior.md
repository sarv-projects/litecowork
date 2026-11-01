# UI behavior - Agent Relay on One Conversation

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Flows and states

- Agent picker between turns.
- Transcript labels producer.
- Failure on B does not auto-send to A.

## Commands

The UI renders owner state and sends the Operator command named by the canonical API. It does not encode a second state machine. TARGET controls stay disabled until a served handler exists.

## Validation

Empty, denied, stale version, offline and unqualified provider states are visible. Success requires a receipt or a verified observation.
