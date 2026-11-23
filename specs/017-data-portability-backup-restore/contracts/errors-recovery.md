# Error and Recovery Contract - Feature 017

**Status:** TARGET / IN_REVIEW - whole-SPEC owner acceptance remains pending. A-117/A-118 authorize only storage T03 CODE; the routes and recovery operations below are not served or qualified.
**Owners:** Storage, Operator/API, Runtime Lifecycle, Security, Desktop UI.
**Canonical owners:** [Failure Recovery](../../../docs/FAILURE-RECOVERY.md), [Storage](../../../docs/STORAGE.md), [API](../../../docs/API.md), [Security](../../../docs/SECURITY.md).

## 1. Error classes (PROPOSED codes promoted by 017-T02)

| Code | HTTP | Cause | Server action | Client action |
|---|---|---|---|---|
| `FORBIDDEN` / `NOT_FOUND` (existing) | 403 / 404 | Foreign principal or Workspace | Deny before any work | Do not retry unchanged |
| `BACKUP_OPERATION_IN_PROGRESS` | 409 | Another nonterminal operation | No write | Show the running operation |
| `BACKUP_ARCHIVE_INVALID` | 422 | Bad magic or version, truncated, size or count over cap, path escape, digest mismatch | Stop before staging rows | Offer choosing another file |
| `BACKUP_PASSPHRASE_REJECTED` | 422 | Wrong passphrase, or too short on create | No key material retained | Re-enter passphrase |
| `BACKUP_SCHEMA_INCOMPATIBLE` | 422 | Archive schema newer than Runtime or below minimum | No staging | Show required build |
| `WORKSPACE_IMPORT_CONFLICT` | 409 | Archive Workspace id already exists | No write | Explain no overwrite or merge in V1 |
| `RUNTIME_RUNNING` | 409 | Offline restore while Runtime lock held | No write | Retry after stop |
| `BACKUP_INTERRUPTED` | n/a (operation state) | Previous incarnation crashed mid-operation | Startup sweep removes staging and partial archive | Offer to start again |
| `DEPENDENCY_UNAVAILABLE` (existing) | 503 | Keystore unavailable, disk error | Settle FAILED, delete partial archive | Show repair path |
| `STALE_VERSION` (existing) | 412 | Stale If-Match on cancel | No change | Refresh operation |

## 2. Recovery rules

The current storage-only T03 snapshot path checks its three-way table classification,
removes excluded local/authority rows and unfinished upload state on the copied database,
filters Workspace rows, and verifies the resulting foreign keys before archiving. These
steps are CODE only. Durable operation replay, startup cleanup, cross-installation import,
offline restore orchestration and packaged crash/restart recovery remain later tasks.

- **Create crash:** the archive is written under a temporary name and renamed only after read-back verification; the manifest is published in the same transaction as SUCCEEDED. On restart, `sweep_backup_staging` removes `.backup-staging-*` and unpublished archive files and settles the operation FAILED `BACKUP_INTERRUPTED`.
- **Create uncertain outcome:** the client replays with the same Idempotency-Key; the daemon returns the stored operation. It never creates a second archive for one key.
- **Import crash:** staging is outside the live database; the live import commit is one transaction. Before commit nothing is visible; after commit the Workspace is complete. Startup sweep handles staging.
- **Offline restore crash:** the destination is a new directory and the swap is a rename after success. If the swap did not happen, the original installation restarts unchanged. If it did, the restored installation starts, mints a new Runtime identity and resumes purge reconciliation.
- **Cancellation race:** cancel is honored only before the commit point (seal for create, transaction for import). After it, cancel returns the committed state.
- **Duplicate replay:** same principal, key and payload returns the original operation; same key with a changed payload fails with the existing `IDEMPOTENCY_CONFLICT` code.
- **Disk full:** the operation fails with `DEPENDENCY_UNAVAILABLE`, deletes partial files and never publishes a manifest.
- **Keystore unavailable on import:** fail before commit; no plaintext blob is left on disk.
- **Lost passphrase:** unrecoverable by design; the UI warns before create.

## 3. Restored authority (never live again)

On every restore and import: `capability_grants` REVOKED, `execution_leases`, `environment_control_leases` and `secret_leases` absent, `runtime_incarnations` STOPPED, new Runtime identity on first start, Connections need re-authentication, AgentSessions from the backup are not resumable.

## 4. Delete fences across restore

Acknowledged purged content is never reintroduced. Pending purge targets resume reconciliation before the restored Workspace serves retrieval or context. A target is shown as purged only after its acknowledgement commits.
