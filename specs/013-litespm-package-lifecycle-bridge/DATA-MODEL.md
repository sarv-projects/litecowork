# Data Model - LiteSPM Package Lifecycle Bridge

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
**Feature:** 013 | **Authority:** LITESPM-LITECOWORK-INTEROP-LLD.md sections 1-3, STORAGE.md, EVENTS.md, TRUST.md, CAPABILITY-FABRIC.md.

LiteCowork stores stable external refs, owner approvals and operation receipts only (interop LLD section 1). It does not store inventory, catalog rows, plans beyond their digest, host config or any LiteSPM-owned record. No table below exists today; the 2026-11-20 migration chain ended at v34, and the current 2026-11-23 chain ends at `sqlite-v36.sql`. v40 remains allocated to this package (DEFERRED with the whole package).

## 1. Existing records reused

| Record | Source | Use |
|---|---|---|
| Workspace | `workspaces` | Owner and scope for every approval and receipt |
| DiscoverCatalogEntry (projection) | `crates/storage-core/src/discover.rs` | LiteSPM inventory rows are mapped into it with `DiscoverEntrySource::LiteSpmPackage`, `authoritative: false` |
| DiscoverProviderStatus (projection) | `crates/storage-sqlite/src/discover.rs` | Becomes a function of live MCP initialization, the pinned tool manifest and Runtime-owned process identity evidence |
| request_dedup | `sqlite-v1.sql` | Idempotency receipts for approval and apply commands |
| domain_events | `sqlite-v1.sql` | Package operation events (proposed names in SPEC 7.2) |

## 2. Proposed records (not yet in source)

| Record | Key and scope | Fields | Owner |
|---|---|---|---|
| LiteSpmServiceObservation | (runtime_id, local_incarnation_id) | mcp_protocol_version, server_info, bridge_pin, tool_manifest_digest, advertised_tools, process_identity_class (`RUNTIME_OWNED`, `UNPROVEN`, `REFUSED`), process_identity_evidence_ref, observed_at | PackageLifecycleService |
| PackageOperationApproval | approval_id; workspace_id | principal_id, plan_digest (`sha256:`), operation_kind (`INSTALL`, `ADOPT`, `UNMANAGE`, `REMOVE`, `UPDATE`, `REPAIR`), host_profile_id, scope (`USER`), litespm_listing_ref, expected_fingerprint, expires_at, state, version, created_at | Storage / Trust |
| PackageOperationReceipt | receipt_id; unique approval_id | workspace_id, apply_key, litespm_operation_ref, state, litespm_error_code, readback_fingerprint, dispatched_at, settled_at, version | Storage |

The observation row is Runtime-incarnation-local and is refreshed on MCP initialize/tool-list validation; it is not replicated. Do not persist native process IDs, stdio handles, credentials or raw tool payloads. `RUNTIME_OWNED` is emitted only from the owning process/session service's evidence, never inferred from stdio alone.

## 3. Lifecycles

```text
PackageOperationApproval: GRANTED -> CONSUMED | EXPIRED | REVOKED
PackageOperationReceipt:  DISPATCHING -> SUCCEEDED | FAILED | UNKNOWN
                          UNKNOWN -> RECONCILED_SUCCEEDED | RECONCILED_FAILED
```

- `begin_apply` consumes the approval (CAS on state and version) and inserts the `DISPATCHING` receipt in one transaction.
- Partial unique index: at most one receipt in `DISPATCHING` or `UNKNOWN` per `(workspace_id, host_profile_id, scope)`.
- An expired approval cannot be consumed; expiry is checked inside the transaction against the daemon clock.

## 4. Invariants

- No column holds package contents, host file contents, environment values or LiteSPM approval token bytes.
- `plan_digest`, `expected_fingerprint` and `readback_fingerprint` are digests, validated with the same `sha256:` + 64 lowercase hex rule as `valid_digest` in `capability_bridge.rs`.
- A receipt in `SUCCEEDED` or `RECONCILED_SUCCEEDED` requires `litespm_operation_ref` and `readback_fingerprint`.
- Installed state creates no CapabilityAssignment, CapabilityGrant, Activation or Invocation row.

## 5. Events (proposed)

| Event | Payload (no secrets, no package content) |
|---|---|
| `package.operation.approved.v1` | approval_id, workspace_id, operation_kind, host_profile_id, scope, plan_digest, expires_at |
| `package.operation.dispatched.v1` | receipt_id, approval_id, apply_key digest |
| `package.operation.settled.v1` | receipt_id, state, litespm_operation_ref, readback_fingerprint, litespm_error_code |
| `package.operation.reconciled.v1` | receipt_id, prior_state, new_state, litespm_operation_ref |

## 6. Retention, backup and migration

- Approvals and receipts are retained as provenance for the Workspace lifetime and included in Feature 017 backups; service observations are excluded from backup.
- Forward-only migration; schema fingerprint updated; old-binary compatibility assessed (old binaries ignore the new tables).
- Workspace archive (STATE-MACHINES) rejects new approvals and applies; settled receipts stay readable.
