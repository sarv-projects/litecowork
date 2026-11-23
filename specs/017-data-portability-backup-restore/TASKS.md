# Tasks - Data Portability, Backup and Restore

**Feature:** 017 | **Status:** IN_REVIEW - B-41 accepted as proposed 2026-11-10, SPEC acceptance pending; implementation not authorized; tasks not started unless marked.
**Spec revision:** 0.1 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** Storage, Operator/API, Desktop UI, Security/Trust, Runtime Lifecycle, Product (see SPEC.md). Delivery wave: Wave 6 (gate G).

## Current baseline

Verified against the checkout on branch `docs/audit-competitor-superset` on 2026-11-09.

- **CLI:** `apps/litecoworkd/src/main.rs` line 55 dispatches `backup` to `backup_operator::run` in `apps/litecoworkd/src/backup_operator.rs`, which implements `create`, `verify`, `restore` and `upgrade_check`, prints one JSON document per command, and reads the passphrase from a file (`read_passphrase`). Its header says restore must never be reachable over the Operator socket. `verify` and `restore` construct `LocalBackupService::new(std::env::current_dir())`, so the service state directory is the caller's working directory, not the installation.
- **Storage:** `crates/storage-core/src/backup.rs` holds the contract (`BACKUP_CONTAINER_MAGIC`, PBKDF2 constants, size caps, `BACKUP_EXCLUDED_TABLES` with 26 tables, `BACKUP_REVOKED_ON_RESTORE_TABLES`, `BackupArchiveManifest`, `SchemaCompatibility`, outcomes). `crates/storage-sqlite/src/backup.rs` (2,437 lines) holds `LocalBackupService` (`create_backup`, `verify_backup`, `restore_backup`, `check_upgrade_compatibility`), `build_filtered_snapshot` (`VACUUM INTO` of the whole database, then `empty_table` on excluded tables only), `revoke_live_authority`, `verify_restored_blobs`, `ensure_runtime_is_not_running` (`fs2` exclusive lock probe on `runtime.lock`), a hand-written `pbkdf2_hmac_sha256`, and 11 tests.
- **Schema:** `workspace_backup_manifests` with trigger `immutable_workspace_backup_manifest_update` in `docs/schemas/sqlite-v1.sql`; the 2026-11-20 migration chain ends at `docs/schemas/sqlite-v34.sql` (`backup_operations` is allocated v46). OpenAPI declares `listWorkspaceBackups`, `createWorkspaceBackup`, `getWorkspaceBackup`, `restoreWorkspaceBackup`; all four are `target_only` in `docs/schemas/served-route-inventory.json`. No `BACKUP_*` error code exists in `docs/schemas/error-codes.schema.json`.
- **Deletion status:** `GET/POST /v1/resources/{resource_id}/context-document/deletion` are served from `apps/litecoworkd/src/operator.rs`; `apps/litecowork-ui/src-tauri/src/context_document_deletion_bridge.rs` exists; `runtime_workspace_replay_process_tests.rs` has `spawned_daemon_reconciles_context_document_purge_after_restart_and_exact_replay`.
- **UI:** `apps/litecowork-ui/src/settings/SettingsView.tsx` `SettingsCategory` = `general | workspaces | agents | runtime`; no backup UI or Tauri backup command exists (grep of `apps/litecowork-ui/src` and `src-tauri/src` finds only Environment `backup_policy` types).

Gaps to V1: no route, bridge or UI; snapshot not Workspace-filtered; no portable restore on a new machine; plaintext staging survives a crash; Windows staging has no owner-only DACL (`ensure_private_directory` `#[cfg(not(unix))]` branch); `restored_at` set from `manifest.verified_at`; no new Runtime identity minted; no operation record, progress, cancellation or single-flight; no deletion status list; no CODE/SYSTEM/USER qualification on either row.

## 017-T01 - Owner acceptance of SPEC and blockers

- **Status:** BLOCKED - B-41 (backup scope, encryption at rest, restore-over-live policy) accepted as proposed (2026-11-10, sarv-projects); whole-SPEC acceptance and the ACCEPTED/LOCKED flip remain pending. The package is IN_REVIEW.
- **DEFERRED/BLOCKED note:** Why: whole-SPEC acceptance and the ACCEPTED/LOCKED flip are pending although B-41 is accepted. Unblocks when: owner reviews and accepts the SPEC. Test later at: decision only.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE (orchestrator records B-41 in docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md).
- **Owner:** Product (repository owner), Storage, Security.
- **Allowed paths:** `specs/017-data-portability-backup-restore/` only.
- **Contract:** SPEC sections 2, 3, 5 (B-41); AC-001 to AC-020; SPECIFICATION-STANDARD lifecycle.
- **Change intent:** owner reviews scope, the two archive modes, passphrase policy and the no-restore-over-live rule; on acceptance flip SPEC and WIRING status to ACCEPTED/LOCKED, bump revision to 1.0, record the acceptance line, and mark B-41 ACCEPTED with any rider.
- **Acceptance:**
  - [ ] Owner decision on B-41 recorded with date and any rider.
  - [ ] SPEC and WIRING status match and the validator passes.
  - [ ] Negative: no task below starts while this task is BLOCKED.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** register row B-41 and SPEC section 5.
- **Stop:** owner rejects the portable export mode or the restore policy; revise SPEC before any task starts.

## 017-T02 - Promote accepted interfaces into owner docs and schemas

- **Status:** NOT STARTED - OpenAPI has four target-only backup operations; no backup error codes, operation table or import route exist.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 011 promotion of the stop/quiescence operation (offline restore calls it); Feature 019 agreement on `checkDataCompatibility` for updater pre-flight.
- **Owner:** Storage, API, Events, Security owners.
- **Allowed paths:** `docs/STORAGE.md`, `docs/API.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/SECURITY.md`, `docs/FAILURE-RECOVERY.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `docs/schemas/sqlite-vNN.sql` (new; next free number at implementation time), this package.
- **Contract:** SPEC sections 5 (B-41), 7.2, 8, 12; AC-006, AC-017.
- **Change intent:** add operations `previewWorkspaceBackup`, `verifyBackupArchive`, `importWorkspaceBackup`, `getBackupOperation`, `cancelBackupOperation`, `checkDataCompatibility`, `listWorkspaceDeletionStatus`; extend `createWorkspaceBackup` with `mode`, `passphrase` (writeOnly, no-store) and `destination_ref`; withdraw `restoreWorkspaceBackup` from the live Operator and document the offline restore path in STORAGE; add errors `BACKUP_OPERATION_IN_PROGRESS`, `BACKUP_ARCHIVE_INVALID`, `BACKUP_PASSPHRASE_REJECTED`, `BACKUP_SCHEMA_INCOMPATIBLE`, `BACKUP_INTERRUPTED`, `WORKSPACE_IMPORT_CONFLICT`, `RUNTIME_RUNNING`; add events `workspace.backup.created.v1`, `workspace.imported.v1`; add table `backup_operations` in one forward-only migration; document the Workspace table map rule in STORAGE "Backup and disaster recovery". Each new name is labelled "accepted target interface (not yet in source)" until its task lands.
- **Acceptance:**
  - [ ] Every new name exists once in its owner doc and machine schema; no parallel definition in this package.
  - [ ] The new migration is the next immutable `sqlite-vNN.sql`; older migrations are byte-identical; schema fingerprint test passes.
  - [ ] `restoreWorkspaceBackup` no longer appears as a live Operator operation (AC-017).
  - [ ] Negative: schema validation rejects a create request without `mode` and a response that contains `passphrase`.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py` and `cargo test -p storage-sqlite --locked schema`
- **Evidence:** PR link and command output in Completion evidence.
- **Stop:** a name collides with an existing owner definition; resolve at the owner.

## 017-T03 - Workspace-scoped snapshot, table map and secret canary scan

- **Status:** NOT STARTED - `build_filtered_snapshot` carries every Workspace's rows; exclusion is a deny list only.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-44 Streaming secret redactor for subprocess output that holds back a buffer whose prefix could still become a secret.** Class: **ADOPT (algorithm port)**. Upstream permalinks: [scripts/packaging-run.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-run.mjs), [scripts/packaging-step.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-step.mjs). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 8 and 14 (row P-44). Why: Whole-chunk replacement leaks a secret split across chunks. `packagingOutputRedactor` is a small pure algorithm, so it ports to Rust directly. It sits beside the masking in `scripts/v1_secret_scan.py` and the secret canary rule (T03 of package 017). Reuse: Algorithm only, reimplement in Rust (or in the LiteCowork TypeScript tree for UI logic); the upstream source has only Node imports so the algorithm ports directly, but the file itself is not imported. If any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
- **Depends on:** T01, T02
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Security.
- **Allowed paths:** `crates/storage-core/src/backup.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/backup.rs`, `crates/storage-sqlite/src/backup_table_map_tests.rs` (new), `crates/storage-sqlite/src/lib.rs`.
- **Contract:** SPEC B-41 (scope), L-02, L-05; AC-002, AC-003, AC-013.
- **Change intent:** add `BACKUP_WORKSPACE_SCOPED_TABLES` (table to Workspace predicate or join) and `BACKUP_INSTALLATION_TABLES`; after `VACUUM INTO`, delete rows of every other Workspace per the map (reuse `empty_table`/`delete_triggers_for_table` trigger handling), then `VACUUM`; add a completeness test that reads `sqlite_master` after all migrations and fails if a table is in none of the three lists; add a canary test that seeds a keyring entry value, SecretStore bytes, an OAuth token, a native session handle and the passphrase, then scans the staged plaintext snapshot and the decrypted container for each.
- **Acceptance:**
  - [ ] Two-Workspace fixture: backup of A contains zero rows, blobs or events of B (AC-003).
  - [ ] An unclassified table fails the completeness test (AC-003).
  - [ ] No canary value appears in staged or decrypted payload (AC-002, AC-013).
  - [ ] Foreign keys and the existing round-trip test still pass after filtering.
- **Command:** `cargo test -p storage-sqlite --locked backup`
- **Evidence:** test names and output in Completion evidence.
- **Stop:** a table cannot be attributed to one Workspace; raise it as a STORAGE owner question before shipping.

## 017-T04 - Restore correctness fixes, staging sweep and Windows private directories

- **Status:** NOT STARTED - `restore_into_staging` sets `restored_at: manifest.verified_at`; no new Runtime identity is minted; no startup sweep; Windows `ensure_private_directory` applies no DACL.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-41 Installer and uninstaller safety matrix (data-home overlap, known-folder blocklist, handle pinning, no reparse traversal, honest progress).** Class: **ADAPT (as test cases only)**. Upstream permalinks: [installer/uninstall-data.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/uninstall-data.h), [installer/uninstall.nsh](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/uninstall.nsh), [installer/path.nsh](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/path.nsh), [installer/extract.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/extract.h), [installer/progress.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/progress.h), [tests/expected/windows-uninstall.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-uninstall.json), [tests/expected/windows-installer.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-installer.json). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 11 (11.7) and 14 (row P-41). Why: The five uninstall cases (interactive, silent, upgrade, replaced, home-inside) are a ready test outline. The C++ and NSIS code is not reused and the native GDI+ renderer is REJECT. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). If the case list is copied verbatim: if any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
- **Depends on:** T01, T02
- **Cross-package prerequisites:** Feature 011 Runtime incarnation and startup-blocker foundation.
- **Owner:** Storage, Runtime Lifecycle, Security.
- **Allowed paths:** `crates/storage-sqlite/src/backup.rs`, `apps/litecoworkd/src/backup_operator.rs`, `apps/litecoworkd/src/runtime.rs`, `apps/litecoworkd/src/runtime_recovery_tests.rs`, `apps/litecoworkd/tests/cli.rs`.
- **Contract:** SPEC L-03, sections 4, 10, 12; AC-007, AC-012, AC-013.
- **Change intent:** set `restored_at` from the restore clock; write a restore marker so the first `runtime::run` after restore mints a new Runtime identity and incarnation through the existing startup path before serving; add `sweep_backup_staging` and call it from `recover_and_persist_startup_blockers` in `runtime.rs`; on Windows call `windows_local_security::secure_current_user_directory` / `secure_current_user_file` for staging, `backups` and archive files; make `verify`/`restore` CLI take `--data-dir` for the service instead of `current_dir`; reject reserved device names in `validate_backup_id`; add CLI tests for create/verify/restore JSON output and passphrase non-echo.
- **Acceptance:**
  - [ ] Restore receipt time is the restore time, not the manifest time.
  - [ ] First start after restore registers a new Runtime identity; old incarnations stay STOPPED (AC-007).
  - [ ] Startup removes leftover `.backup-staging-*` directories (AC-012).
  - [ ] Windows: staging and archive ACL grant only the current user (checked with the existing Windows security test helpers) (AC-013).
  - [ ] Negative: backup id `CON` or `..\x` is rejected.
- **Command:** `cargo test -p storage-sqlite --locked backup` and `cargo test -p litecoworkd --locked --test cli` and `cargo test -p litecoworkd --locked runtime_recovery`
- **Evidence:** test output per OS in Completion evidence.
- **Stop:** the new identity cannot be minted without the Feature 011 incarnation contract; keep restore CLI-only until it can.

## 017-T05 - Portable export and import re-encryption

- **Status:** NOT STARTED - blob envelopes are copied verbatim with `BackupKeyReference` only; restore needs the source keystore keys.
- **Depends on:** T01, T03
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Security.
- **Allowed paths:** `crates/storage-core/src/backup.rs`, `crates/storage-sqlite/src/backup.rs`, `crates/storage-sqlite/src/os_key_provider.rs`, `crates/storage-sqlite/src/blob.rs` (only if a re-wrap helper is needed).
- **Contract:** SPEC B-41 (encryption, two modes); AC-009, AC-002.
- **Change intent:** add `BackupArchiveMode`; for `PortableExport` decrypt each blob with the local `WorkspaceBlobKeyProvider` and stream its plaintext as authenticated container chunks under the container key, recording plaintext digest and size; on import generate new local keys through `OsWorkspaceBlobKeyProvider` and re-encrypt each blob before the import commit; the manifest for portable mode carries no key references; keep `InstallationBackup` behavior unchanged.
- **Acceptance:**
  - [ ] Export from installation A imports into installation B with a fresh key provider; every Artifact digest matches (AC-009).
  - [ ] Portable container never contains a key, only ciphertext under the container key (AC-002).
  - [ ] Installation backup round trip test still passes.
  - [ ] Negative: wrong passphrase on a portable archive fails before any blob is written.
- **Command:** `cargo test -p storage-sqlite --locked backup_portable` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** re-encryption would require holding a whole blob in memory above the chunk bound; redesign to streaming first.

## 017-T06 - Durable backup operation record, single-flight, progress and cancellation

- **Status:** NOT STARTED - `create_backup` is one blocking call with no progress or cancellation; no operation table.
- **Depends on:** T01, T02
- **Cross-package prerequisites:** NONE
- **Owner:** Storage.
- **Allowed paths:** `crates/storage-core/src/backup.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/backup.rs`, `crates/storage-sqlite/src/backup_operations.rs` (new), `crates/storage-sqlite/src/lib.rs`.
- **Contract:** SPEC 7.2, 7.3, F-01; AC-011, AC-018.
- **Change intent:** add `BackupOperationRecord`, `BackupOperationState`, `BackupProgress`, `BackupOperationStore` (`admit_backup_operation`, `settle_backup_operation`, `list_backup_manifests`); admission is an IMMEDIATE transaction with a partial unique index on nonterminal operations per installation; settlement writes manifest + state + event + receipt atomically; `create_backup_with` and `import_workspace` call `BackupProgress::phase` and check `cancelled()` between chunks; cancellation deletes the partial archive and settles CANCELLED.
- **Acceptance:**
  - [ ] Second admission while one is nonterminal fails `BACKUP_OPERATION_IN_PROGRESS`; same Idempotency-Key replay returns the first operation (AC-011).
  - [ ] Cancel before seal leaves no archive and state CANCELLED; cancel after commit returns SUCCEEDED (AC-018).
  - [ ] Fault after manifest insert rolls back the whole settlement.
  - [ ] Two threads admitting at once: exactly one row.
- **Command:** `cargo test -p storage-sqlite --locked backup_operation` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** settlement needs an external call inside the transaction.

## 017-T07 - Daemon BackupService and Operator routes

- **Status:** NOT STARTED - no backup route is mounted; the four OpenAPI operations are target-only.
- **Depends on:** T01, T03, T04, T06
- **Cross-package prerequisites:** Feature 011 authenticated IPC on Windows named pipe and Linux socket.
- **Owner:** Operator/API, Storage.
- **Allowed paths:** `apps/litecoworkd/src/backup_routes.rs` (new), `apps/litecoworkd/src/backup_routes_tests.rs` (new), `apps/litecoworkd/src/operator.rs` (router mount only), `apps/litecoworkd/src/backup_operator.rs` (share helpers only).
- **Contract:** SPEC 7.2 route table; AC-001, AC-010, AC-017, AC-019.
- **Change intent:** mount `listWorkspaceBackups`, `getWorkspaceBackup`, `previewWorkspaceBackup`, `createWorkspaceBackup` (202 + operation), `verifyBackupArchive`, `getBackupOperation`, `cancelBackupOperation`, `checkDataCompatibility` behind `super::authenticate` and `ensure_workspace_owner_for`; run work in `spawn_blocking` with a `BackupProgress` implementation that updates the operation row at phase boundaries; add `Cache-Control: no-store` (same pattern as `no_store_middleware` in `agent_quota_observation_operator.rs`); exclude request bodies of these routes from any request logging; do not mount any whole-installation restore route.
- **Acceptance:**
  - [ ] Create returns 202, the operation reaches SUCCEEDED and the manifest is listed (AC-001, AC-019).
  - [ ] Foreign principal and wrong Workspace get 403/404 with no work started (AC-017).
  - [ ] List marks a manifest whose archive was deleted as MISSING (AC-019).
  - [ ] `checkDataCompatibility` equals `check_upgrade_compatibility` for current, newer and older schemas (AC-010).
  - [ ] Served-route inventory shows no `/recovery/backups/{backupId}/restore` (AC-017).
- **Command:** `cargo test -p litecoworkd --locked backup_routes` (filter to be added by this task) and `python3 scripts/audit_served_routes.py`
- **Evidence:** test output; served-route inventory diff.
- **Stop:** a handler would hold a SQLite write transaction across file I/O.

## 017-T08 - Live import of an absent Workspace

- **Status:** NOT STARTED - only `restore_backup` into an empty destination exists.
- **Depends on:** T01, T05, T06, T07
- **Cross-package prerequisites:** Feature 003 ContextDocument purge reconciliation entry point (restored pending purges must resume).
- **Owner:** Storage, Operator/API.
- **Allowed paths:** `crates/storage-sqlite/src/backup.rs`, `crates/storage-sqlite/src/backup_import.rs` (new), `apps/litecoworkd/src/backup_routes.rs`, `apps/litecoworkd/src/backup_routes_tests.rs`.
- **Contract:** SPEC B-41 (restore-over-live), 7.3 import; AC-005, AC-006, AC-008.
- **Change intent:** `LocalBackupService::import_workspace` stages and verifies the archive, migrates the staged copy if needed, refuses `WORKSPACE_IMPORT_CONFLICT` if the `workspace_id` exists, then in one transaction inserts the Workspace rows, revokes grants, marks Connections re-authentication required, preserves tombstones and purge plans/receipts, settles the operation and writes `workspace.imported.v1`; after commit, schedule purge reconciliation before the Workspace is selectable; mount `importWorkspaceBackup`.
- **Acceptance:**
  - [ ] Import into an installation without that Workspace succeeds; data and digests match (AC-008).
  - [ ] Import of an existing Workspace id fails with no write (AC-006).
  - [ ] Fault at each stage leaves the live installation unchanged (AC-005).
  - [ ] Acknowledged purged blobs are absent and pending purges resume before retrieval (AC-008).
- **Command:** `cargo test -p storage-sqlite --locked backup_import` (filter to be added by this task) and `cargo test -p litecoworkd --locked backup_routes`
- **Evidence:** test output in Completion evidence.
- **Stop:** an imported row references an installation-level row that does not exist; fail import, do not invent the parent.

## 017-T09 - Offline whole-installation restore from the desktop

- **Status:** NOT STARTED - restore runs only from the CLI; there is no authenticated Operator stop (`main.rs` HELP says so).
- **Depends on:** T01, T04
- **Cross-package prerequisites:** Feature 011 authenticated stop and quiescence gate, packaged sidecar launch and restart.
- **Owner:** Runtime Lifecycle, Desktop (Tauri), Storage.
- **Allowed paths:** `apps/litecowork-ui/src-tauri/src/backup_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/runtime_lifecycle_bridge.rs` (call sites only), `apps/litecowork-ui/src-tauri/src/lib.rs` (handler registration).
- **Contract:** SPEC B-41 (offline restore), AC-020.
- **Change intent:** `restore_installation_offline` requests the Feature 011 stop, waits for observed quiescence and lock release, runs the bundled `litecoworkd backup restore --archive --destination <new empty dir> --passphrase-file <owner-only temp file>` with the temp file deleted immediately after, swaps directories only on success, restarts the Runtime and returns the parsed receipt; on any failure restarts the original installation untouched.
- **Acceptance:**
  - [ ] Restore with a running Runtime that does not quiesce aborts with no data change (AC-020).
  - [ ] Successful restore restarts and the receipt is shown (AC-020).
  - [ ] The temporary passphrase file is owner-only and deleted on every exit path (AC-013).
- **Command:** `cargo test -p litecowork-ui --locked backup_bridge` (filter to be added by this task)
- **Evidence:** test output; packaged run recorded under T16.
- **Stop:** quiescence cannot be observed on a row; keep offline restore UNAVAILABLE on that row.

## 017-T10 - Tauri bridge commands

- **Status:** NOT STARTED - no backup command in `apps/litecowork-ui/src-tauri/src`.
- **Depends on:** T01, T07, T08
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop (Tauri).
- **Allowed paths:** `apps/litecowork-ui/src-tauri/src/backup_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/src/settings/storage-backup/backup-api.ts` (new).
- **Contract:** SPEC 7.2 Tauri commands; AC-013, AC-014.
- **Change intent:** add `preview_workspace_backup`, `create_workspace_backup`, `list_workspace_backups`, `get_backup_operation`, `cancel_backup_operation`, `verify_backup_archive`, `import_workspace_backup`, `list_workspace_deletion_status`, `check_data_compatibility`; hold the passphrase as `Zeroizing<String>`; use the native save/open dialogs so the WebView never handles paths beyond a display label; typed error mapping by code, never by message text.
- **Acceptance:**
  - [ ] Each command forwards Idempotency-Key and If-Match where the route requires them.
  - [ ] Passphrase never appears in Tauri logs or returned values (AC-013).
  - [ ] Unknown error codes map to a generic recoverable state, not success.
- **Command:** `cargo test -p litecowork-ui --locked backup_bridge` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test output in Completion evidence.
- **Stop:** NONE beyond test failure.

## 017-T11 - Settings "Storage & Backup" UI

- **Status:** NOT STARTED - `SettingsCategory` has no storage-backup entry.
- **Depends on:** T01, T10
- **Cross-package prerequisites:** Feature 011 Settings shell category registration and app-wide keyboard/focus baseline.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/settings/SettingsView.tsx` (category entry only), `apps/litecowork-ui/src/settings/storage-backup/StorageBackupPanel.tsx` (new), `apps/litecowork-ui/src/settings/storage-backup/backup-state.ts` (new), `apps/litecowork-ui/src/settings/storage-backup/storage-backup.css` (new), `apps/litecowork-ui/tests/storage-backup-state.test.ts` (new), `apps/litecowork-ui/tests/storage-backup-panel.test.mjs` (new).
- **Contract:** SPEC AC-014, AC-013; contracts/ui-behavior.md; V1-SETTINGS-CONTRACT-LEDGER rows `previewBackup`, `exportWorkspace`, `verifyManifest`, `importBackup`, `previewRestore`, `confirmRestore`.
- **Change intent:** panel with backup list (state, size, created, mode, archive PRESENT/MISSING), Create backup flow (preview, mode choice, passphrase + confirm with no-recovery warning, progress, cancel), Verify archive, Import Workspace, Restore installation (offline, typed confirmation phrase), Check data compatibility; pure `backup-state.ts` reducer for loading/empty/list/progress/error/denied/offline/stale; clear the passphrase field and state on submit and on unmount; controls whose route is not served are omitted, not disabled-looking-active.
- **Acceptance:**
  - [ ] Every state in contracts/ui-behavior.md renders with a real handler (AC-014).
  - [ ] Empty state says "No backups yet" and shows no size or date placeholders.
  - [ ] Keyboard-only path completes create and cancel; progress uses a polite live region; focus returns to the list.
  - [ ] Passphrase is absent from component state after submit (AC-013).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test output; V1-SETTINGS-CONTRACT-LEDGER rows updated by T17.
- **Stop:** a control has no served route; omit it.

## 017-T12 - Deletion and purge status with retry

- **Status:** PARTIAL - per-Resource ContextDocument deletion read and request routes are served (`operator.rs`, `context_document_deletion_bridge.rs`); no Workspace-level list or Settings surface exists.
- **Depends on:** T01, T07, T10
- **Cross-package prerequisites:** Feature 003 ContextDocument purge owner route; Feature 014 Conversation delete tombstones (B-38) when they exist.
- **Owner:** Storage, Desktop UI; data owners 003 and 014 for retry semantics.
- **Allowed paths:** `apps/litecoworkd/src/backup_routes.rs`, `crates/storage-sqlite/src/backup.rs` (read query only) or `crates/storage-sqlite/src/deletion_status.rs` (new), `apps/litecowork-ui/src/settings/storage-backup/DeletionStatusPanel.tsx` (new), `apps/litecowork-ui/tests/deletion-status.test.mjs` (new).
- **Contract:** SPEC G-08, L-04, L-06; AC-015, AC-008.
- **Change intent:** `listWorkspaceDeletionStatus` reads `context_document_purge_plans` and `context_document_purge_receipts` (and later 014 tombstones) into rows (target, owner, state PENDING/FAILED/ACKNOWLEDGED, age, last error code); Retry calls the owner route with a stable idempotency key; the UI shows "Purged" only for ACKNOWLEDGED.
- **Acceptance:**
  - [ ] Pending, failed and acknowledged targets render with owner and age (AC-015).
  - [ ] Retry replays with the same key and never shows success before acknowledgement (AC-015).
  - [ ] After restore, pending targets appear and resume (AC-008).
- **Command:** `cargo test -p litecoworkd --locked deletion_status` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** a retry would need a new owner route; open it in the owning package.

## 017-T13 - Failure and recovery suite and canonical test target

- **Status:** NOT STARTED - storage tests cover corrupt archive, wrong passphrase, failed restore, non-empty destination, live lock and failed create; no crash-window, cancellation race or route-level replay test; `apps/litecoworkd/tests/backup_restore.rs` does not exist.
- **Depends on:** T01, T04-T08
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Operator/API.
- **Allowed paths:** `apps/litecoworkd/tests/backup_restore.rs` (new), `crates/storage-sqlite/src/backup.rs` (test module), `crates/storage-sqlite/tests/backup_fixtures.rs` (new, if needed).
- **Contract:** SPEC section 11, contracts/errors-recovery.md; AC-004, AC-005, AC-011, AC-012, AC-018.
- **Change intent:** spawn the real daemon binary and kill it at each phase (SNAPSHOT, BLOBS, SEAL, VERIFY, import staging, import commit); fuzz container header, chunk lengths and manifest bytes with bounded inputs; disk-full simulation via a size-limited temp directory where the OS supports it; duplicate replay and cancel races at the route level.
- **Acceptance:**
  - [ ] After each kill, restart sweeps staging, settles FAILED `BACKUP_INTERRUPTED` and advertises nothing (AC-012).
  - [ ] Every malformed archive returns a typed error before key derivation beyond the header (AC-004).
  - [ ] Live data unchanged after every injected import failure (AC-005).
  - [ ] Replay and cancel races resolve to one outcome (AC-011, AC-018).
- **Command:** `cargo test -p litecoworkd --test backup_restore --locked -- --nocapture`
- **Evidence:** test output per OS.
- **Stop:** a crash window leaves plaintext or a half archive that startup cannot classify.

## 017-T14 - Performance baseline and streaming bounds

- **Status:** NOT STARTED - container uses 1 MiB chunks; no measured memory or latency baseline.
- **Depends on:** T01, T13
- **Cross-package prerequisites:** Feature 019 soak/performance harness (`scripts/v1_soak_harness.py`).
- **Owner:** Storage, Release.
- **Allowed paths:** `apps/litecoworkd/tests/backup_restore.rs`, `crates/storage-sqlite/benches/backup.rs` (new, optional), this package RESEARCH.md (baseline record).
- **Contract:** SPEC AC-016; O-04.
- **Change intent:** generate the reference fixture (row count and blob bytes recorded in RESEARCH), sample peak RSS and Operator read latency during create/verify/import, assert memory growth is independent of archive size (two fixture sizes), record the baseline, then propose thresholds for owner review.
- **Acceptance:**
  - [ ] Peak RSS growth differs by less than one chunk-size multiple between the two fixture sizes (AC-016).
  - [ ] Progress reported at every phase (AC-016).
  - [ ] Baseline numbers recorded with OS, build and hardware.
- **Command:** `cargo test -p litecoworkd --test backup_restore --locked backup_memory_bound -- --nocapture` (filter to be added by this task)
- **Evidence:** RESEARCH.md baseline table and CURRENT-RUN.md entry.
- **Stop:** memory grows with archive size; fix streaming before any qualification.

## 017-T15 - Cross-layer trace for each user-visible operation

- **Status:** NOT STARTED.
- **Depends on:** T01, T07-T12
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Desktop UI.
- **Allowed paths:** `specs/017-data-portability-backup-restore/contracts/ui-behavior.md`, `specs/017-data-portability-backup-restore/WIRING.yaml`.
- **Contract:** AGENTS.md section 4; AC-001, AC-014, AC-020.
- **Change intent:** for Create backup, Verify archive, Import Workspace, Restore installation, Cancel, Check data compatibility and Retry purge record: user outcome, exact control in `StorageBackupPanel.tsx` / `DeletionStatusPanel.tsx`, Tauri command, Operator route (or sidecar CLI for offline restore), authorization, service owner, transaction/version/fence, record/event/receipt, projection, restart/recovery and evidence test.
- **Acceptance:**
  - [ ] Every operation has a complete row; no route lacks a UI path and no control lacks a route.
  - [ ] WIRING edges marked served only with T16 evidence.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** updated contracts/ui-behavior.md.
- **Stop:** a hop is missing; open a task, do not document it as present.

## 017-T16 - Qualify E2E-12 on the release matrix rows

- **Status:** NOT STARTED.
- **Depends on:** T01-T15
- **Cross-package prerequisites:** Feature 019 release matrix rows and evidence bundle process; Feature 011 packaged sidecar and Windows named-pipe IPC; data owners 001, 003, 007 and 014 able to produce the fixture records (Conversation, ContextDocument tombstone and purge, Artifact revision).
- **Owner:** Product (USER sign-off), Storage, Security, Release.
- **Allowed paths:** this package; `docs/IMPLEMENTATION-STATUS.md` evidence rows (via T17).
- **Contract:** ACCEPTANCE-AND-E2E-PLAN E2E-12; AC-008, AC-009, AC-020 and all denial/recovery ACs.
- **Change intent:** on the packaged desktop for Windows x64 and Linux x64: create installation backup and portable export, verify, import the export on a second clean installation (different OS user so the keystore differs), offline restore on the first, inspect provenance (Artifact digests, Evidence, tombstones, purge receipts), and run denial cases (wrong passphrase, existing Workspace, live Runtime, corrupted archive) and a kill during create.
- **Acceptance:**
  - [ ] CODE: canonical command passes on each row.
  - [ ] SYSTEM: packaged run log per row with build, OS, revision.
  - [ ] USER: owner sign-off for E2E-12 on each row including denial and recovery.
  - [ ] No secret canary found in any produced archive (AC-002).
- **Command:** `cargo test -p storage-sqlite --locked backup && cargo test -p litecoworkd --test backup_restore --locked -- --nocapture && pnpm --dir apps/litecowork-ui test`
- **Evidence:** evidence bundle path per row; IMPLEMENTATION-STATUS.md row via T17.
- **Stop:** any row lacks USER evidence; that row stays unqualified and the Settings category shows the feature as unavailable there.

## 017-T17 - Documentation and inventory sync

- **Status:** NOT STARTED.
- **Depends on:** T01, T15, T16
- **Cross-package prerequisites:** NONE (the orchestrator owns shared ledgers; this task supplies the edits for review).
- **Owner:** Storage, Documentation owners.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json`, `docs/V1-SETTINGS-CONTRACT-LEDGER.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `CURRENT-RUN.md`, this package.
- **Contract:** AGENTS.md sections 4-5; SPEC section 4; AC-019.
- **Change intent:** regenerate the served-route inventory, update the backup/restore rows per release row and evidence class, update the Settings ledger Backup and Storage rows, close B-41 in the register, refresh SPEC section 4.
- **Acceptance:**
  - [ ] `python3 scripts/audit_served_routes.py` matches the committed inventory.
  - [ ] No row claims QUALIFIED without the T16 bundle for that exact row.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of status documents.
- **Stop:** evidence and claim disagree; downgrade the claim.

## 017-T18 - Package VERIFIED for V1

- **Status:** NOT STARTED.
- **Depends on:** T01-T17
- **Cross-package prerequisites:** Feature 019 evidence bundle process and release qualification matrix.
- **Owner:** Product (sign-off), Storage, Security.
- **Allowed paths:** this package; `docs/IMPLEMENTATION-STATUS.md`.
- **Contract:** SPEC sections 14-15; AC-001 to AC-020; SPECIFICATION-STANDARD lifecycle.
- **Change intent:** assemble the evidence bundle (per-task revision, command, result, OS, reviewer); move the package ACCEPTED/LOCKED -> IMPLEMENTING at the first implementation merge and IMPLEMENTING -> VERIFIED after owner sign-off on both rows.
- **Acceptance:**
  - [ ] Canonical command passes on Windows x64 and Linux x64.
  - [ ] Owner USER sign-off for E2E-12 on each row, including denial and recovery.
  - [ ] SPEC and WIRING status equal VERIFIED and the validator passes.
- **Command:** `cargo test -p storage-sqlite --locked backup && cargo test -p litecoworkd --test backup_restore --locked -- --nocapture && pnpm --dir apps/litecowork-ui test` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle path referenced from IMPLEMENTATION-STATUS.md.
- **Stop:** any row lacks USER evidence.

## Implementation and optimization notes (Rust and TypeScript)

Findings from reading the source, folded into the tasks above. They add no scope.

- **Whole-database copy (Rust, T03):** `build_filtered_snapshot` copies every Workspace with `VACUUM INTO` and then trims only excluded tables. Deleting foreign-Workspace rows from the copy keeps the schema byte-identical (the property the current code relies on for migration-ledger compatibility) and the final `VACUUM` reclaims their pages. A table-classification constant plus a `sqlite_master` completeness test turns the current deny list into an allow list that new migrations cannot bypass.
- **Hand-written PBKDF2 (Rust, T03/T05):** `pbkdf2_hmac_sha256` at `crates/storage-sqlite/src/backup.rs` line 1915 reimplements the KDF on top of `hmac`. The RustCrypto `pbkdf2` crate gives the same output with a reviewed implementation; add a published test vector before swapping so existing archives still open. Argon2id is a container-v2 option (F-03), not a silent change.
- **Windows DACL gap (Rust, T04):** `ensure_private_directory` only sets 0700 on Unix. `windows_local_security::secure_current_user_directory` and `secure_current_user_file` already exist and `storage-sqlite` already depends on that crate.
- **CLI service root (Rust, T04):** `verify` and `restore` build `LocalBackupService::new(std::env::current_dir())`; the result depends on where the CLI was launched. Take `--data-dir` explicitly.
- **Monolithic blocking call (Rust, T06):** `create_backup` gives no progress and cannot be cancelled. A `BackupProgress` trait checked between 1 MiB chunks gives both at near-zero cost and lets the Operator stay responsive under `spawn_blocking`.
- **Receipt clock (Rust, T04):** `restored_at: manifest.verified_at.clone()` reports the wrong time; pass the restore clock in like `BackupCreateRequest::now`.
- **UI boundary (TypeScript, T10/T11):** keep all crypto, filtering and verification in the Rust daemon. The WebView keeps only a pure `backup-state.ts` reducer (testable with the existing `node --test` runner), clears the passphrase on submit, and renders by error code. Use native dialogs in Tauri so archive paths and bytes never enter the WebView.

## Dependency order

T01 -> T02 -> (T03, T04, T06 in parallel) -> T05 (after T03) -> T07 (after T03, T04, T06) -> T08 (after T05, T06, T07). T09 after T04. T10 after T07, T08 -> T11. T12 after T07, T10. T13 after T04-T08 -> T14. T15 after T07-T12. T16 after T01-T15 -> T17 -> T18. Every task depends on T01.

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T02, T09 | Feature 011 stop/quiescence operation and packaged sidecar | Offline restore must stop the Runtime safely first |
| T04 | Feature 011 Runtime incarnation and startup-blocker foundation | A restored installation must mint a new Runtime identity at first start |
| T07 | Feature 011 Windows named-pipe IPC and Linux socket | Routes are only reachable over authenticated local IPC |
| T08, T12 | Feature 003 ContextDocument purge owner route and reconciliation | Imported pending purges must resume; Retry uses the owner route |
| T12 | Feature 014 Conversation delete tombstones (B-38) | Conversation deletions must appear in deletion status and survive backup |
| T11 | Feature 011 Settings shell category registration and keyboard baseline | The category lives in the 011 shell |
| T14 | Feature 019 soak/performance harness | Shared measurement method |
| T16, T18 | Feature 019 release matrix rows and evidence bundle; Features 001, 003, 007 fixture producers | E2E-12 needs real Conversation, ContextDocument and Artifact records on each row |
| T02, T07 | Feature 019 updater pre-flight | `checkDataCompatibility` gates an update that would migrate the schema |

Cross-package task IDs are named by outcome because other packages' task plans are being renumbered in the same planning pass; the orchestrator resolves them to IDs in the traceability index.

## DeepSeek Harness proposals (PROPOSED, owner review required)

Status: PROPOSED. None of the items below is an accepted task. They come from the DeepSeek Harness studies ([desktop deep-dive](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) register A-63, proposals P-31 to P-46; [plugin deep-dive](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) register A-58). Owner review is required before any item becomes a numbered task in this package. They are labelled `PROP-Pnn` so they do not collide with the stable task IDs that `.specify/scripts/validate_feature_package.py` checks. On acceptance: assign the next free task ID in this file, copy the upstream permalink, class and reuse statement into the task, and update the [reuse map](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md). Upstream pin `fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159` (MIT, Copyright (c) 2026 DeepSeek). No upstream code is imported (AGENTS.md section 7).

### Proposals owned elsewhere that touch this package (pointer only)

- **PROP-P41 (P-41) Installer and uninstaller safety matrix (data-home overlap, known-folder blocklist, handle pinning, no reparse traversal, honest progress)** [PROPOSED, not accepted, owner review required]: row owned by package 019; class **ADAPT (as test cases only)**; would touch here: T08 of package 019; T04 of package 017. Upstream permalinks: [installer/uninstall-data.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/uninstall-data.h), [installer/uninstall.nsh](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/uninstall.nsh), [installer/path.nsh](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/path.nsh), [installer/extract.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/extract.h), [installer/progress.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/progress.h), [tests/expected/windows-uninstall.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-uninstall.json), [tests/expected/windows-installer.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-installer.json). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 11 (11.7) and 14 (row P-41). Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). If the case list is copied verbatim: if any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
- **PROP-P44 (P-44) Streaming secret redactor for subprocess output that holds back a buffer whose prefix could still become a secret** [PROPOSED, not accepted, owner review required]: row owned by package 018; class **ADOPT (algorithm port)**; would touch here: T07 of package 018; T03 of package 017. Upstream permalinks: [scripts/packaging-run.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-run.mjs), [scripts/packaging-step.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-step.mjs). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 8 and 14 (row P-44). Reuse: Algorithm only, reimplement in Rust (or in the LiteCowork TypeScript tree for UI logic); the upstream source has only Node imports so the algorithm ports directly, but the file itself is not imported. If any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.


## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), harness and version where relevant (OpenCode v2.0.32 for fixture Conversations), evidence class reached (CODE, SYSTEM, USER), report path and reviewer. Statuses above are source-audit classifications at this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
