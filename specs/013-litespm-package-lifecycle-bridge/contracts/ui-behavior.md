# UI behavior - Discover inventory, plan review and lifecycle verbs (Feature 013)

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification. Binds UI to owner contracts; not pixels.
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md), interop LLD section 8.
**Wire:** [api-events.md](api-events.md).

The UI renders LiteSPM inventory and receipts and sends typed commands. It never contacts LiteSPM, never decides authorization and never parses English error strings.

## 1. Screens and controls

| Screen / surface | Control | Command | Today |
|---|---|---|---|
| Discover > Installed / native | Inventory list (LiteSPM rows plus existing Runtime host observations, labelled separately) | `package_inventory_request` | Runtime host observations only |
| Discover > Managed | Managed list with per-row verbs | `package_inventory_request` (managed filter) | "Not available" panel |
| Row verb: Use existing | Opens Feature 006 grant flow; no package command | none here | absent |
| Row verb: Manage with LiteSPM (adopt) | Plan review dialog | `package_plan_request` kind ADOPT | absent |
| Row verb: Install to agent | Plan review dialog | `package_plan_request` kind INSTALL | disabled by `connectActionOutcome()` |
| Row verb: Update / Repair | Plan review dialog | kind UPDATE / REPAIR | absent |
| Row verb: Stop managing | Plan review dialog | kind UNMANAGE | absent |
| Row verb: Remove installation | Plan review dialog with irreversible warning | kind REMOVE | absent |
| Plan review dialog | Approve, Cancel | `package_approval_request` then `package_apply_request` | absent |
| Receipt banner | Reconcile (when UNKNOWN), View details | `package_operation_reconcile`, `package_operation_get` | absent |
| Service status note | Read-only text | `package_service_status` | provider message from `DiscoverProviderStatus` |

## 2. States

| Surface | States | Meaning |
|---|---|---|
| Inventory | `LOADING`, `READY`, `EMPTY`, `UNAVAILABLE`, `OFFLINE`, `STALE` | `EMPTY` only when LiteSPM answered with zero items; `UNAVAILABLE` when the read failed |
| Verb | `ENABLED`, `DISABLED(reason)` | Reason from host profile, pinned MCP tool manifest or qualification; shown via `aria-describedby` |
| Operation | `PLANNING`, `AWAITING_APPROVAL`, `APPLYING`, `SUCCEEDED`, `FAILED`, `UNKNOWN`, `RECONCILING` | `UNKNOWN` never styled as success or failure; when the pinned MCP tools lack operation lookup, explain that reconciliation is unavailable and keep the mutation gate closed |
| Service | `NOT_QUALIFIED`, `READY`, `PROCESS_IDENTITY_UNPROVEN`, `INCOMPATIBLE`, `REFUSED` | Show MCP protocol/tool-manifest status and the Runtime-owned identity evidence class; never imply stdio or MCP metadata alone proves OS identity |

## 3. Transitions

1. Open tab -> `LOADING` -> `READY`/`EMPTY`/`UNAVAILABLE`.
2. Verb -> `PLANNING` -> dialog `AWAITING_APPROVAL` with diff, host, scope, hazards, expiry countdown.
3. Approve -> `APPLYING`; Cancel or Escape -> close, focus returns to the verb button, no command sent.
4. Receipt terminal -> `SUCCEEDED` (inventory refresh) or `FAILED` (cause, re-plan, rollback plan).
5. Timeout -> `UNKNOWN` with Reconcile; other mutating verbs for the same target disabled with reason.
6. App restart -> rehydrate open receipts from `GET /v1/package-operations/{receipt_id}`.

## 4. Rules

- Four facts per row, separately labelled: package availability, Connection authentication, Coworker assignment, use permission. No single "Connected" pill.
- Package text (names, summaries, diffs, skill content) is rendered as plain text.
- Approve is disabled after expiry and while another operation for the target is unsettled.
- No irreversible action before the exact diff is shown; Remove shows which owned nodes LiteSPM will remove.
- Keyboard: every verb, dialog control and banner action reachable by Tab; Enter activates; Escape cancels; focus trapped in the dialog and restored on close; status changes announced via `role="status"`, failures via `role="alert"`.

## 5. Cross-layer trace (completed by 013-T17)

| Verb | Control | Tauri command | Route | Authorization | Transaction | Event | Restart | Test |
|---|---|---|---|---|---|---|---|---|
| View inventory | Installed / native tab | `package_inventory_request` | `GET /v1/package-inventory` | owner + Workspace | none | none | stateless | to be added |
| Install to agent | row verb | `package_plan_request`, `package_approval_request`, `package_apply_request` | plans, approvals, apply | owner desktop principal only | tx-record-approval, tx-begin-apply, tx-settle-apply | approved, dispatched, settled | startup sweep | to be added |
| Reconcile | receipt banner | `package_operation_reconcile` | reconcile | owner | tx-reconcile | reconciled | startup sweep | to be added |
