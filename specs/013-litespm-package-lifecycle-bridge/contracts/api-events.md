# API and Event Contract - Feature 013

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
**Canonical owners:** [API](../../../docs/API.md), [Events](../../../docs/EVENTS.md), [Schemas](../../../docs/SCHEMAS.md), [Capability Fabric](../../../docs/CAPABILITY-FABRIC.md).

## 1. Existing routes (served today)

| Route | Behaviour today | Source |
|---|---|---|
| `GET /v1/discover/search?q=&kind=&cursor=&limit=` | Owner-scoped page of Workspace skills plus up to 50 Runtime host observations; `provider_status.package_service_qualified=false` | `apps/litecoworkd/src/discover_operator.rs` |
| `GET /v1/discover/items/{item_id}` | Detail with capability summaries and `provider_note` | same |
| `POST /v1/capabilities/{capability_id}/connect-or-install` | Requires Workspace, Runtime match, `Idempotency-Key`; always 503 `DEPENDENCY_UNAVAILABLE`, `details.reason=LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`, `requested_scope_granted=false`; no receipt | same |
| `POST /v1/capabilities/{capability_id}/test` | Always `health=UNKNOWN`; contacts no provider | same |

## 2. Proposed routes (not yet in source)

All require the selected Workspace header and the authenticated owner. Mutating routes require `Idempotency-Key`; approval and apply also require `If-Match` on the approval version where an approval exists.

| Route | Request | Response |
|---|---|---|
| `GET /v1/package-service/status` | none | `mcp_protocol_version`, `server_info`, `tool_manifest_digest`, `process_identity_class`, `process_identity_evidence_ref`, `observed_at`; no daemon descriptor, endpoint or peer-auth fields are invented |
| `GET /v1/package-inventory` | `host_profile_id`, `scope`, `cursor`, `limit` (1-200) | `items[]` (LiteSPM identity, host, scope, locator, provenance, management_state, observed_fingerprint, observed_at, readiness), `next_cursor`, `stale` |
| `POST /v1/package-operations/plans` | `operation_kind`, `host_profile_id`, `scope`, `litespm_listing_ref` or `litespm_registration_ref`, `expected_fingerprint` | `plan_digest`, `diff` (plain text or structured), `hazards[]`, `expires_at`, `grants_included=false` |
| `POST /v1/package-operations/approvals` | `plan_digest`, `host_profile_id`, `scope` | `approval_id`, `state=GRANTED`, `expires_at`, `version` |
| `POST /v1/package-operations/approvals/{approval_id}/apply` | none | `receipt_id`, `state` (`DISPATCHING`, `SUCCEEDED`, `FAILED`, `UNKNOWN`) |
| `GET /v1/package-operations/{receipt_id}` | none | receipt state, `litespm_operation_ref`, `readback_fingerprint`, `litespm_error_code` |
| `POST /v1/package-operations/{receipt_id}/reconcile` | none | reconciled receipt state |

Approval routes refuse any request whose principal is not the desktop owner session or that carries an agent, Conversation, Task or Coworker origin.

## 3. LiteSPM semantic operations used (wire names come from the pinned published MCP contract)

inventory, describe, plan, owner approval correlation, apply, operation get, adopt, unmanage, remove, update, repair, reconcile. The daemon methods recorded in interop LLD Appendix B4 are research only and are not MCP tool names. At T02, map each semantic operation only to a tool schema present at the pinned published MCP bridge. An operation that is not advertised or cannot preserve LiteCowork owner approval, idempotency and recovery semantics remains unavailable.

## 4. Proposed events

`package.operation.approved.v1`, `package.operation.dispatched.v1`, `package.operation.settled.v1`, `package.operation.reconciled.v1`; payloads in [DATA-MODEL.md](../DATA-MODEL.md) section 5. Events carry digests and refs only.

## 5. Discover projection rules

- LiteSPM rows use `source=LITE_SPM_PACKAGE` and `authoritative=false`.
- `install_state` reflects LiteSPM management state; `auth_state` comes from Feature 006, never from installation.
- `provider_status.package_service_qualified` is true only while the live MCP initialization and tool manifest match the pin, the Runtime-owned process identity meets its exact OS profile, and the release-row profile is qualified.
