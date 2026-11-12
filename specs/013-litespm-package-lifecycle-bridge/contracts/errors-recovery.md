# Error and Recovery Contract - Feature 013

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
**Owners:** Capability Fabric, Storage, Security, Desktop UI.
**Canonical owners:** [Failure Recovery](../../../docs/FAILURE-RECOVERY.md), [interop LLD section 6](../../../docs/LITESPM-LITECOWORK-INTEROP-LLD.md), [API](../../../docs/API.md).

## 1. Classification and client rule

| Class | Examples | Server action | Client action |
|---|---|---|---|
| Owner/scope denial | foreign principal, foreign Workspace, `RUNTIME_MISMATCH`, archived Workspace | deny before any LiteSPM call | show denial; no retry |
| Service not qualified | B-36/B-37 not accepted, row not qualified | 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` | verb disabled with reason |
| MCP contract mismatch | unpinned/mismatched MCP protocol or server, invalid/missing tool manifest, unadvertised operation | `LITESPM_PROTOCOL_INCOMPATIBLE` / `LITESPM_OPERATION_NOT_ADVERTISED` | show update-required or unavailable |
| Process identity unproven/refused | Runtime process owner cannot prove the expected local bridge identity, or evidence names a different owner | refuse service readiness; record `UNPROVEN` or `REFUSED` | show service unavailable; no fallback and no claim that MCP stdio alone authenticates the process |
| Inventory unavailable | LiteSPM read failed or timed out | `PACKAGE_INVENTORY_UNAVAILABLE`; never empty page | show unavailable with Retry |
| Profile unsupported | project scope, `mcp`, unknown host version | `PACKAGE_HOST_PROFILE_UNSUPPORTED` | verb disabled with reason |
| Plan stale | expired plan, changed fingerprint | `PACKAGE_PLAN_EXPIRED` | re-plan; old approval unusable |
| Approval conflict | consumed approval, replay with changed payload | `PACKAGE_APPROVAL_CONSUMED` / `IDEMPOTENCY_CONFLICT` | refresh receipt |
| Target busy | another receipt `DISPATCHING`/`UNKNOWN` for the target | `PACKAGE_OPERATION_UNKNOWN_PENDING` | show Reconcile first |
| Apply uncertain | timeout or lost response after MCP tool dispatch | settle `UNKNOWN`; query operation identity only if the pinned published MCP bridge exposes a qualified lookup/reconcile tool; otherwise keep `UNKNOWN` and block further mutation | show "Outcome unknown"; show Reconcile only when that tool is supported |
| LiteSPM terminal failure | precondition changed, write refused | settle `FAILED` with LiteSPM code | show cause; offer re-plan or rollback plan |
| Drift | host entry edited outside LiteSPM | read-only diff; keep-local default | owner chooses resolution |

## 2. Retry classes

- **SAFE_READ_RETRY:** status, inventory, describe; operation lookup only when the pinned tool manifest advertises it and its semantics are safe to retry.
- **SAFE_LOCAL_REPLAY:** same principal, key and payload for approval or apply returns the stored record.
- **RECONCILE_BEFORE_RETRY:** any apply in `DISPATCHING` or `UNKNOWN`; never re-send apply.
- **NEW_INTENT_ONLY:** a changed plan, scope or host requires a new plan and approval.

## 3. Crash windows

| Window | Recovery |
|---|---|
| After approval commit, before apply | Approval stays GRANTED until expiry; no mutation happened |
| After `begin_apply` commit, before MCP tool dispatch | Startup recovery queries by apply key only if the pinned tool surface supports lookup; otherwise retain `UNKNOWN` because absence of evidence is not proof that no write occurred |
| After MCP tool dispatch, before response | `UNKNOWN`; query original operation identity only through a published pinned lookup/reconcile tool |
| After LiteSPM success, before `settle_apply` | Published operation lookup returns terminal success; settle with read-back, otherwise retain `UNKNOWN` |
| LiteSPM restart mid-apply | LiteSPM's own journal recovers; LiteCowork queries after reconnect |
| UI closed during apply | Receipt state survives; UI rehydrates from `GET /v1/package-operations/{receipt_id}` |

## 4. Never

Never report success from a timeout, never repeat an apply to "fix" an UNKNOWN, never restore host files, never treat a transport-ACL-only result as verified identity, never treat an empty failed read as "nothing installed".
