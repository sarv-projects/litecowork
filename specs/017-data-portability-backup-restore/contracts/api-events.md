# API and Event Contract - Feature 017

**Status:** IN_REVIEW - pending owner acceptance of B-41; does not authorize implementation; not a claim of implementation or qualification.
**Canonical owners:** [API](../../../docs/API.md), [Events](../../../docs/EVENTS.md), [Schemas](../../../docs/SCHEMAS.md), [Storage](../../../docs/STORAGE.md), [OpenAPI](../../../docs/schemas/operator-api.openapi.yaml).

## 1. Existing declared operations (target_only today)

| Operation | Path | Current state | Change proposed by this package |
|---|---|---|---|
| `listWorkspaceBackups` | `GET /workspaces/{workspaceId}/backups` | target_only | Serve; add `archive_presence` (PRESENT, MISSING) to the view |
| `getWorkspaceBackup` | `GET /workspaces/{workspaceId}/backups/{backupId}` | target_only | Serve; never return key material (`encryption_key_ref` stays readOnly reference text only) |
| `createWorkspaceBackup` | `POST /workspaces/{workspaceId}/backups` | target_only, 201 with manifest | Serve as 202 with a `BackupOperation`; body `{mode, passphrase (writeOnly), destination_ref?}`; Idempotency-Key required |
| `restoreWorkspaceBackup` | `POST /recovery/backups/{backupId}/restore` | target_only | Withdraw from the live Operator (B-41); offline restore replaces it |

## 2. PROPOSED new operations

| Operation | Method and path | Request | Response | Notes |
|---|---|---|---|---|
| `previewWorkspaceBackup` | `GET /workspaces/{workspaceId}/backup-preview?mode=` | none | `{estimated_bytes, row_count, blob_count, included_classes[], excluded_classes[], secret_exclusion_statement}` | Read only; estimates labelled |
| `verifyBackupArchive` | `POST /backup-verifications` | `{archive_ref, passphrase}` | 202 `BackupOperation` (kind VERIFY) | No mutation of Workspace data |
| `importWorkspaceBackup` | `POST /workspace-imports` | `{archive_ref, passphrase, expected_workspace_id, confirm_absent_workspace: true}` | 202 `BackupOperation` (kind IMPORT) | `expected_workspace_id` comes from a prior authenticated Verify result; reauthenticate the archive and require exact id match; fails `WORKSPACE_IMPORT_CONFLICT` if it exists |
| `getBackupOperation` | `GET /backup-operations/{operationId}` | none | `BackupOperation` | Polling source for progress |
| `cancelBackupOperation` | `POST /backup-operations/{operationId}/cancel` | If-Match version | `BackupOperation` | After the commit point returns the committed state |
| `checkDataCompatibility` | `GET /runtime/data-compatibility` | none | `{schema_version, minimum_supported_schema_version, compatible, reason}` | Same verdict as `check_upgrade_compatibility` |
| `listWorkspaceDeletionStatus` | `GET /workspaces/{workspaceId}/deletion-status?cursor=&limit=` | none | page of `{target_ref, owner, state, age_seconds, last_error_code}` | Retry goes to the owner route |

`BackupOperation`: `{operation_id, workspace_id: string | null, kind, mode, state, phase, progress_bytes, total_bytes, backup_id, error_code, created_at, updated_at, version}`. `workspace_id` is null only for Workspace-neutral VERIFY; CREATE and IMPORT bind a known Workspace before admission. `archive_ref` and `destination_ref` are opaque handles issued by the Tauri bridge for an owner-chosen file; the daemon resolves them to a regular local file and rejects symlinks, network paths and reserved device names.

All routes: authenticated local IPC only, principal from the peer, `Cache-Control: no-store`, request bodies excluded from request logging. Routes with a Workspace path require a matching `X-Workspace-ID`; Verify has no Workspace header because archive identity is not trusted until authentication; Import requires `expected_workspace_id` and a matching header after the owner has reviewed the Verify result. Get/cancel operation routes authorize the principal against the stored operation owner and Workspace.

## 3. Tauri commands (PROPOSED)

`preview_workspace_backup`, `create_workspace_backup`, `list_workspace_backups`, `get_backup_operation`, `cancel_backup_operation`, `verify_backup_archive`, `import_workspace_backup`, `restore_installation_offline`, `list_workspace_deletion_status`, `check_data_compatibility`. `restore_installation_offline` has no Operator route: it calls the Feature 011 stop gate and the `litecoworkd backup restore` sidecar.

## 4. Events (PROPOSED)

| Event | Emitted by | Payload | Excluded |
|---|---|---|---|
| `workspace.backup.created.v1` | `tx-backup-manifest-publish` | backup_id, workspace_id, mode, schema_version, integrity_digest, created_at | paths, key references, passphrase, sizes per blob |
| `workspace.imported.v1` | `tx-workspace-import-commit` | workspace_id, source backup_id, schema_version, revoked_grant_count, imported_at | archive path, passphrase, row content |

Operation progress is a projection, not a DomainEvent.

## 5. Existing CLI (unchanged contract, CODE)

`litecoworkd backup create --data-dir --workspace --backup-id --passphrase-file`, `verify --archive --passphrase-file`, `restore --archive --destination --passphrase-file`, `upgrade-check --data-dir`. Each prints one JSON document; errors print `{"status":"error","error":...}` on stderr. 017-T04 adds `--data-dir` to `verify` and `restore`.
