# Tasks - Data Portability, Backup and Restore

**Feature:** 017 | **Status:** IN_REVIEW - formal whole-SPEC acceptance remains pending. A-117/A-118 record USER-DELEGATED authority for the storage-only T03 containment slice; this is not owner acceptance or qualification.
**Spec revision:** 0.2 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** Storage, Operator/API, Desktop UI, Security/Trust, Runtime Lifecycle, Product (see SPEC.md). Delivery wave: Wave 6 (gate G).

## Current checkout evidence

Rechecked against the implementation on `main` during the 2026-11-23 T03 continuation. This supersedes the 2026-11-09 source baseline below; that dated audit is retained in git history, not as current truth.

- **Storage contract and snapshot (CODE):** `storage-core::backup` has 98 Workspace-scoped table mappings, 3 installation metadata names (`schema_migrations`, `runtimes`, `runtime_incarnations`), and 41 excluded local/authority tables. The SQLite snapshot checks this map against the migrated schema, removes active upload state and other excluded registries, trims non-selected Workspace rows from the excluded content registry, then filters Workspace rows. It derives blob candidates from that same `VACUUM INTO` copy and removes the retained registry subset before final snapshot validation. Upload removal, excluded-table cleanup and Workspace filtering run in one immediate transaction with deferred foreign-key checks, allowing child/parent records to be pruned together and rejecting invalid retained references at commit; the committed copy is vacuumed and revalidated.
- **Current evidence (CODE only):** the post-migration table-map check covers 142 application tables and rejects an unclassified table; two-Workspace isolation includes a foreign Conversation/message/turn and native-session binding; blob-nomination, upload-state, canary, FK and round-trip tests cover this storage slice. The final focused command/result is recorded in `CURRENT-RUN.md`.
- **Still not served:** backup create/verify/list/import/restore and deletion status are not reachable through an authenticated Operator route, Tauri bridge or Settings panel. Four OpenAPI operations remain target-only; no backup `backup_operations` table exists.
- **Still incomplete for V1:** portable re-encryption, restore identity/time corrections, crash staging sweep, restrictive Windows staging DACL, restore/import routes, Settings UI, deletion-status list, durable progress/cancellation/single-flight, packaged E2E and release-row qualification remain in later tasks. The CLI and storage implementation are CODE evidence only.
- **Formal state:** Feature 017 remains IN_REVIEW and T01 remains BLOCKED for whole-SPEC owner acceptance. A-117/A-118 authorize the scoped T03 CODE change only; neither is owner ratification or qualification.

## 017-T01 - Owner acceptance of SPEC and blockers

- **Status:** BLOCKED for formal whole-SPEC acceptance - B-41 was accepted as proposed (2026-11-10, sarv-projects), but full SPEC review and the ACCEPTED/LOCKED flip remain pending. A-117/A-118 permit scoped T03 implementation only; this task is not complete.
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
  - [ ] Negative: no task below starts while this task is BLOCKED unless an exact USER-DELEGATED implementation direction is recorded in this register and the owning package; such direction never completes T01 or changes the package lifecycle.
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

- **Status:** DONE (CODE only) under USER-DELEGATED A-117/A-118. Native-Windows focused verification passes 15/15 backup tests. The code filters to one Workspace, validates an exhaustive three-way table map, excludes local/authority tables, drops resumable upload state with dependent chunk/request receipts, scopes blob candidates from the same `VACUUM INTO` copy before excluding their local registry, and scans seeded sensitive/local canaries including a foreign Conversation body and a native-session handle. Row pruning is one immediate transaction with deferred FK checks and post-commit snapshot validation. Feature 017 remains IN_REVIEW and T01 remains BLOCKED; no SERVED, INTEGRATED, SYSTEM, USER or QUALIFIED evidence is claimed.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-44 Streaming secret redactor for subprocess output that holds back a buffer whose prefix could still become a secret.** Class: **ADOPT (algorithm port)**. Upstream permalinks: [scripts/packaging-run.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-run.mjs), [scripts/packaging-step.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-step.mjs). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 8 and 14 (row P-44). Why: Whole-chunk replacement leaks a secret split across chunks. `packagingOutputRedactor` is a small pure algorithm, so it ports to Rust directly. It sits beside the masking in `scripts/v1_secret_scan.py` and the secret canary rule (T03 of package 017). Reuse: Algorithm only, reimplement in Rust (or in the LiteCowork TypeScript tree for UI logic); the upstream source has only Node imports so the algorithm ports directly, but the file itself is not imported. If any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
- **Depends on:** A-117/A-118 USER-DELEGATED scope decision. This storage-only correction may precede T02 because it adds no public interface; T02 remains a gate for routes and operation-store work.
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Security.
- **Allowed paths:** `crates/storage-core/src/backup.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/backup.rs`, `crates/storage-sqlite/src/backup_table_map_tests.rs` (new), `crates/storage-sqlite/src/lib.rs`, `specs/017-data-portability-backup-restore/`, `docs/STORAGE.md`, `docs/DATA-MODEL.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/IMPLEMENTATION-STATUS.md`, `CURRENT-RUN.md`, `docs/audit/EXTERNAL-AI-AUDIT-DISPOSITION-2026-11-19.md`, and the generated `docs/audit/V1-MARKDOWN-AUDIT.md`, `docs/audit/V1-MARKDOWN-INVENTORY.csv` and `docs/audit/v1-cross-contract-guard.json` outputs refreshed by the required documentation validators.
- **Contract:** SPEC B-41 (scope), L-02, L-05; AC-002, AC-003, AC-013.
- **Change intent:** add `BACKUP_WORKSPACE_SCOPED_TABLES` (direct column or owner predicate/join), `BACKUP_INSTALLATION_TABLES`, and a complete `BACKUP_EXCLUDED_TABLES`; after `VACUUM INTO`, retain only the requested Workspace rows, drop excluded local/authority rows and reclaim pages. Temporarily retain only the requested Workspace's rows from the excluded content registry while filtering the copied database, derive blob candidates from that same filtered copy, then remove the registry before final validation. This keeps the archive's database and blob list consistent if the live source changes after the copy. Remove `OPEN`/`CONTENT_RECEIVED` upload sessions and their chunk/request rows before filtering so an archive cannot imply resumability without the temporary chunk objects. Apply related row deletion in one immediate transaction with deferred FK checks, commit only when retained references remain valid, then vacuum and revalidate the copy. Validate the map in production and with a post-migration `sqlite_master` test so an unclassified table fails closed. Seed canaries for key material, SecretStore/token payloads, native locators, raw file identity, a foreign-Workspace Conversation body, an actual native session reference and passphrase; scan staged snapshot, archive, manifest, decrypted payloads and extracted files. Keep generic `request_dedup` out because it has no Workspace identity; use the future Workspace-bound backup operation receipt for backup retries.
- **Acceptance:**
  - [x] Two-Workspace fixture: snapshot A contains only A's Workspace/event rows, the blob candidate collector cannot nominate B's registry blob, and candidate discovery reads the same `VACUUM INTO` copy that is filtered before the local registry is removed (AC-003).
  - [x] A post-migration schema table map covers all 142 application tables exactly once; an unclassified table fails closed (AC-003).
  - [x] Seeded SecretStore/token, key-material, native-locator, raw file identity and passphrase canaries are absent from the staged snapshot, archive, manifest and all decrypted/extracted payload files (AC-002, AC-013).
  - [x] Resumable upload states and their chunk/request receipts are removed while terminal upload history remains (B-41).
  - [x] Foreign-Workspace Conversation/message/turn rows and the associated local native-session handle are absent from the staged snapshot; canary scans cover staged and archive payload forms (AC-002, AC-003, AC-013).
  - [x] Related excluded and foreign-Workspace parent/child rows are removed in one transaction; retained foreign keys validate after commit, and the existing backup/restore round-trip test passes.
- **Command:** `cargo test -p storage-sqlite --locked backup`
- **Completion evidence (CODE only):** final `cargo test -p storage-sqlite --locked backup -- --nocapture` passed 15 tests, 0 failed, 614 filtered out. Coverage includes `snapshot_contains_only_the_requested_workspace_and_its_events`, `snapshot_drops_resumable_uploads_and_their_chunk_receipts`, `every_current_migrated_application_table_has_one_valid_backup_classification`, `backup_table_map_rejects_a_new_unclassified_application_table`, the staged/archive/manifest/decrypted-payload canary test with foreign Conversation and native-session data, and the existing round-trip case. See `CURRENT-RUN.md` for exact host, duration and checks. No SERVED, INTEGRATED, SYSTEM, USER or QUALIFIED evidence is claimed.
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
- **Depends on:** T01-T17, T19
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

- **Workspace-filtered copy (Rust, T03, CODE):** `build_filtered_snapshot` uses `VACUUM INTO` for a consistent source copy, validates that copy's requested Workspace and schema metadata, derives blob candidates from only its selected Workspace rows, and removes all other Workspace and excluded local/authority records while keeping the schema intact. Row pruning is one immediate transaction with deferred FK checks, so parent/child rows are removed together and invalid retained references fail at commit; final `VACUUM` reclaims removed pages. The explicit table-classification map plus a migrated-`sqlite_master` completeness test fails closed when future migrations add an unmapped table. The map currently covers 142 application tables; portable blob re-encryption, startup cleanup and platform ACLs remain separate work.
- **Hand-written PBKDF2 (Rust, T03/T05):** `pbkdf2_hmac_sha256` at `crates/storage-sqlite/src/backup.rs` line 1915 reimplements the KDF on top of `hmac`. The RustCrypto `pbkdf2` crate gives the same output with a reviewed implementation; add a published test vector before swapping so existing archives still open. Argon2id is a container-v2 option (F-03), not a silent change.
- **Windows DACL gap (Rust, T04):** `ensure_private_directory` only sets 0700 on Unix. `windows_local_security::secure_current_user_directory` and `secure_current_user_file` already exist and `storage-sqlite` already depends on that crate.
- **CLI service root (Rust, T04):** `verify` and `restore` build `LocalBackupService::new(std::env::current_dir())`; the result depends on where the CLI was launched. Take `--data-dir` explicitly.
- **Monolithic blocking call (Rust, T06):** `create_backup` gives no progress and cannot be cancelled. A `BackupProgress` trait checked between 1 MiB chunks gives both at near-zero cost and lets the Operator stay responsive under `spawn_blocking`.
- **Receipt clock (Rust, T04):** `restored_at: manifest.verified_at.clone()` reports the wrong time; pass the restore clock in like `BackupCreateRequest::now`.
- **UI boundary (TypeScript, T10/T11):** keep all crypto, filtering and verification in the Rust daemon. The WebView keeps only a pure `backup-state.ts` reducer (testable with the existing `node --test` runner), clears the passphrase on submit, and renders by error code. Use native dialogs in Tauri so archive paths and bytes never enter the WebView.

## Dependency order

T01 -> T02; A-117/A-118 -> storage-only T03. T02 remains required for T04 and T06; T05 follows T03; T07 follows T03, T04 and T06; T08 follows T05, T06 and T07. T09 follows T04. T10 follows T07 and T08, then T11. T12 follows T07 and T10. T13 follows T04-T08, then T14. T15 follows T07-T12. T16 follows T01-T15, then T17. A-122 authorizes the bounded-code direction for T19; T14 must establish the slowest-row full-operation and peak-memory evidence before the KDF range is finalized, and T19 must close before package-verification T18. All package work remains behind T01 except the exact storage-only T03 slice authorized by A-117/A-118; that exception does not complete T01, accept the SPEC, or authorize public interface work before T02.

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


## 017-T19 - Bound and measure container-v1 KDF work

- **Status:** IN PROGRESS — bounded CODE and local optimized Rust timing are recorded on native Windows (2026-11-24); T14 slowest-row, full-operation and peak-memory evidence remains open before the range can be finalized. Feature 017 remains IN_REVIEW; this does not complete T01 or claim Storage/Security ratification.
- **Depends on:** A-122 direction; T14 for release-hardware performance evidence before the range is locked.
- **Cross-package prerequisites:** none; no new schema or migration is permitted for this change.
- **Owner:** Storage, Security, Release.
- **Allowed paths:** `crates/storage-core/src/backup.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/backup.rs`, `specs/017-data-portability-backup-restore/SPEC.md`, `TASKS.md`, `RESEARCH.md`, `docs/STORAGE.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/IMPLEMENTATION-STATUS.md`, `CURRENT-RUN.md`.
- **Contract:** SPEC §5 F-05, §7, §10, §11, §12 and A-122; v1 creation accepts 600,000 through 1,000,000 inclusive, default 600,000; v1 reads preserve the previous 100,000 minimum while capping at 1,000,000, pending required native release-row timing.
- **Change intent:** define canonical minimum/maximum constants in storage-core; refuse out-of-range create requests and untrusted archive headers before PBKDF2; preserve container version; add boundary and malformed-header tests, including proof rejected headers do not invoke key derivation; benchmark the Rust implementation at minimum, default and maximum and record OS, CPU, command and result. T14 owns full operation baselining and the owner-reviewed responsiveness target; if the slowest declared release row exceeds that target, revise A-122 and the container-v1 decision before marking this task done. No latency budget is invented before T14 measures.
- **Acceptance:**
  - [x] Creation accepts exactly 600,000 and 1,000,000, rejects below and above, and uses 600,000 by default.
  - [x] Verification accepts the legacy 100,000 read floor and rejects values below it or above 1,000,000 before PBKDF2, returning the established typed integrity error without leaking KDF parameters into public messages.
  - [x] Creation refuses values below 600,000, so legacy low-cost values can only be read, never newly emitted; no v1 migration or byte-format change is introduced.
  - [x] Local optimized Rust timing with OS, CPU, command and result is recorded in RESEARCH §0.1; full backup latency, peak behavior and slowest-row confirmation remain T14/SYSTEM gates.
- **Command:** `cargo test -p storage-sqlite --lib --locked kdf -- --nocapture`; `cargo test -p storage-sqlite --lib --locked backup_and_restore_round_trip_preserves_the_workspace_and_its_blobs -- --nocapture`; `cargo test -p storage-sqlite --lib --release --locked measure_container_kdf_work_factors -- --ignored --nocapture`; T14 owns release-row full-operation timing.
- **Evidence:** exact native Windows results are in CURRENT-RUN.md and RESEARCH §0.1. CODE only; full performance baseline, release-row confirmation and formal owner ratification remain open.
- **Stop:** Rust timing exceeds the owner-reviewed responsiveness target after T14, an existing archive within the defined compatibility range is rejected, or a malformed header can reach PBKDF2 before bounds validation.

## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), harness and version where relevant (OpenCode v2.0.32 for fixture Conversations), evidence class reached (CODE, SYSTEM, USER), report path and reviewer. Statuses above are source-audit classifications at this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
| T03 | `98929d0` (`fix(backup): scope snapshots to one workspace`) | `cargo test -p storage-sqlite --locked backup -- --nocapture` | PASS — 15 passed, 0 failed; 614 filtered out; 81.48s | Windows 11 Home Single Language x64, 10.0.26300, build 26300 | N/A | CODE | Codex; pushed to `origin/main`; no served or qualified journey |
