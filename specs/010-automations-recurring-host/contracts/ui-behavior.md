# UI behavior - Automations and Recurring Host

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Flows and states

- Automations list shows next fire, last occurrence, Task link and notification.
- Disabled and paused are distinct.
- Misfire is visible.

## Commands

The UI renders owner state and sends the Operator command named by the canonical API. It does not encode a second state machine. TARGET controls stay disabled until a served handler exists.

## Validation

Empty, denied, stale version, offline and unqualified provider states are visible. Success requires a receipt or a verified observation.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## Current source crosswalk — 2026-11-06

- Automations list is cursor-paginated and has Load more. Detail separately loads the pinned Automation/Routine revisions, health and occurrence history. Occurrence history consumes only the first page and discards `next_cursor`; no older-occurrence control is rendered.
- Create/revise writes PAUSED definitions. The editor exposes Manual and five-field local Cron schedule; One time is visibly unavailable, and webhook/connector-event hosting is not supported. Existing trigger sets are preserved unless the owner explicitly chooses replacement. Coworker pinning is optional and snapshots an exact active Coworker revision.
- Automation execution policy fields are not equally active: max concurrency/overlap and schedule misfire are consumed by the trigger host; retry and wake policies have no production consumer. Placement and budget are TaskSpec inputs, not proof of execution or a cloud worker.
- The Pause success message still claims trigger execution is unavailable in this build, despite the live 30-second local TriggerHost. The automation_operator.rs module header also incorrectly says recurring hosting and `/resume` are unavailable.
- Run once requires exactly one Manual trigger and creates an occurrence plus ordinary READY Task. Create test Task creates a READY Task without occurrence/cursor. Neither starts planning or an agent.
- Notifications page shows cursor-paginated delivery history and versioned policy forms. Delivery history is not an outbound sender: current producer creates PENDING rows only; no delivery worker advances them. The policy form has a free-text channel list (not consumed by the producer), offers ON_CONDITION without a condition editor (domain meaning is only DEPENDENCY_BLOCKED), and displays quiet hours without editing them; save preserves the stored quiet-hours object or null.
- Routines page has paginated list, create/revise/archive, advanced JSON contract editor, exact revision verification, typed text/ResourceRef inputs, Run now and process-local exact-request retry/discard. Run now creates a READY Task only. Unresolved retry envelopes do not survive application process exit.
- External workflow selection/invocation and graph authoring remain unavailable; there is no provider adapter or in-Core graph canvas.

## External workflow-backed Routine UI

- The Automation/Routine UI does not contain an in-Core node canvas or graph palette in V1.
- Target only: after a qualified WorkflowBackendAdapter exists, the user may choose an existing provider workflow, inspect its provider identity/reference and supported input schema, map pinned Routine values, validate and invoke it under the Task/CapabilityInvocation path. No production adapter is currently present, so this selector/invocation UI is not served.
- The provider's own editor is the manual graph-authoring surface. An external editor link is shown only when the provider supplies a safe, reviewed URL; LiteCowork does not embed a generic provider editor.
- A user may use the connected provider's own AI builder/MCP authoring tools (for example, n8n Assistant/MCP or Activepieces Build by Asking/MCP) inside that provider, subject to its feature availability and permissions. The resulting graph remains provider-owned and is reviewed/published there.
- LiteCowork agent assistance may draft Routine content and input mappings. It cannot claim to create/update/publish an external graph; no production WorkflowBackendAdapter/WorkflowAuthoringAdapter is present in this checkout.
- Provider node states are shown, if at all, as clearly labelled external observations. They never masquerade as LiteCowork Task Steps or as Needs You approvals.
- The provider selection, graph link and invocation UI remain disabled/unavailable until the provider adapter and Task/CapabilityInvocation/Trust path are served and qualified.
- If the adapter is missing, stale, disconnected or cannot validate the input, the workflow is unavailable with a repair path; no fake canvas or success indicator appears.
