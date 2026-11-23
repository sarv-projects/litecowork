# Research and Decision Log - Data Portability, Backup and Restore

**Feature:** 017 | **Status:** IN_REVIEW - pending owner acceptance of B-41; does not authorize implementation; not a claim of implementation or qualification.
**Checked:** 2026-11-09 - source audit of the checkout on branch `docs/audit-competitor-superset`; external sources are not web-verified in this pass.

## 1. Repository sources inspected

| Source | Path | Finding | Limit |
|---|---|---|---|
| CLI | `apps/litecoworkd/src/backup_operator.rs`, `apps/litecoworkd/src/main.rs` (line 55, HELP) | Four subcommands, one JSON document each, passphrase from file, restore must never be on the Operator socket | `verify`/`restore` use `current_dir()` as service root; no CLI tests for backup in `apps/litecoworkd/tests/cli.rs` |
| Storage contract | `crates/storage-core/src/backup.rs` | Container constants, 26 excluded tables, revoked-on-restore list, manifest and outcome types, schema verdict | Exclusion is a deny list |
| Storage implementation | `crates/storage-sqlite/src/backup.rs` | `VACUUM INTO` snapshot, excluded-table trim with trigger restore, chunked XChaCha20-Poly1305, PBKDF2, read-back verification, staging, all-or-nothing restore, live-lock refusal, grant revocation, 11 tests | No per-Workspace row filter; blobs need source keys; Windows directories not DACL-protected; `restored_at` from manifest |
| Key provider | `crates/storage-sqlite/src/os_key_provider.rs` (`OsWorkspaceBlobKeyProvider`, `keyring` crate 3.6.3) | Workspace blob keys live in the OS keystore | Keys are per OS user and machine |
| Windows security | `crates/windows-local-security/src/lib.rs` (`secure_current_user_directory`, `secure_current_user_file`) | Owner-only DACL helpers exist; `storage-sqlite` already depends on the crate | Not called by backup code |
| Schema | `docs/schemas/sqlite-v1.sql` (`workspace_backup_manifests`), `docs/schemas/sqlite-v34.sql` (latest wired; `backup_operations` is allocated v46) | Immutable manifest table | No operation table |
| API | `docs/schemas/operator-api.openapi.yaml` lines 214-276; `docs/schemas/served-route-inventory.json` | Four backup operations declared; all `target_only` | Restore operation conflicts with CLI rule |
| Owner prose | `docs/STORAGE.md` "Backup and disaster recovery", `docs/FAILURE-RECOVERY.md`, `docs/SECURITY.md` "Secrets" | Workspace-filtered snapshot, new Runtime identity, tombstone/purge carry, secrets never in storage | STORAGE says "Workspace-filtered"; code does not filter rows |
| Product | `docs/V1-PRODUCT-RELEASE-CONTRACT.md` Area 10 and section 6 invariant 6; `docs/IMPLEMENTATION-MASTER-PLAN.md` P11; `docs/ACCEPTANCE-AND-E2E-PLAN.md` E2E-12 | Backups/recovery required; delete fences; E2E-12 integrity, secret exclusion, tombstone and evidence preservation | Target |
| Settings | `docs/V1-UI-SCREEN-CONTRACT.md` category 13; `docs/V1-SETTINGS-CONTRACT-LEDGER.md` rows `previewBackup` to `confirmRestore`, `inspectStorageUsage` | Explicit preview and receipt; no silent overwrite | Ledger is proposed design |
| UI | `apps/litecowork-ui/src/settings/SettingsView.tsx` | Four categories only | No backup UI |
| Deletion | `apps/litecoworkd/src/operator.rs` context-document deletion routes; `apps/litecowork-ui/src-tauri/src/context_document_deletion_bridge.rs`; `runtime_workspace_replay_process_tests.rs` | Per-Resource purge with restart reconciliation test | No Workspace-level list |

## 2. Pinned external sources

| Source | Version or SHA | License | Use | Decision |
|---|---|---|---|---|
| RustCrypto `chacha20poly1305` | 0.10.1 (`crates/storage-sqlite/Cargo.toml`) | Apache-2.0 OR MIT | XChaCha20-Poly1305 container chunks | Keep (already a dependency) |
| RustCrypto `hmac` | 0.12.1 (`crates/storage-sqlite/Cargo.toml`) | Apache-2.0 OR MIT | Current hand-written PBKDF2 core | Keep; replace hand-written loop with RustCrypto `pbkdf2` after a test vector proves identical output (F-03) |
| `keyring` | 3.6.3 (`crates/storage-sqlite/Cargo.toml`) | Apache-2.0 OR MIT | OS keystore access for blob keys | Keep; never export key material |
| `fs2` | 0.4 | Apache-2.0 OR MIT | Runtime lock probe | Keep |
| SQLite `VACUUM INTO` | SQLite bundled by `rusqlite` in `Cargo.lock` | Public domain | Consistent snapshot without long writer lock | Keep; add row filter on the copy |
| nightly-labs/openbot | `6759fa3ee857f4f4966865e12de99ae84ab6de07` ([REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md) REF-OPENBOT) | PolyForm Noncommercial 1.0.0 | Reference for deleting usage with an agent; no backup feature was found in the cited files | Reject copying code (license); concept only |

Not pinned and used as concepts only: age file encryption format and restic/borg repository designs (considered for portable format; rejected for V1 because the existing `LCBK1` container already meets the requirements and switching would break existing archives).

## 3. Alternatives considered

| Topic | Alternative | Decision |
|---|---|---|
| Workspace scope | Keep whole-database copy and label it installation backup | Rejected: contradicts STORAGE "Workspace-filtered" and leaks other Workspaces |
| Workspace scope | Build snapshot by `ATTACH` + `INSERT SELECT` per table | Rejected for V1: loses byte-identical schema the restore path relies on; delete-on-copy is simpler |
| Portability | Export blob keys wrapped by the passphrase | Rejected: key material would leave the keystore (L-05) |
| Portability | Re-encrypt blob plaintext inside the container (B-41 PROPOSED) | Proposed |
| Restore over live | Replace a live Workspace in place | Rejected: no safe fence for in-flight turns, leases and readers |
| Restore over live | Import only an absent Workspace; whole restore offline (B-41 PROPOSED) | Proposed |
| Passphrase transport | File path as the CLI does | Rejected for desktop: the UI would need to write a secret file; IPC no-store body is used, except the sidecar offline restore which uses an owner-only temp file deleted immediately |
| KDF | Argon2id now | Deferred to a container v2 (F-03, O-05) |

## 4. Verified source defects (inputs to tasks)

1. `build_filtered_snapshot` does not filter rows by Workspace (T03).
2. `ensure_private_directory` applies no DACL on Windows (T04).
3. `restore_into_staging` sets `restored_at` from `manifest.verified_at` (T04).
4. `new_runtime_identity_required: true` is reported but no identity is minted (T04).
5. No startup sweep of `.backup-staging-*` (T04).
6. `verify`/`restore` CLI service root is `current_dir()` (T04).
7. Blob envelopes require the source keystore (T05).
8. OpenAPI `restoreWorkspaceBackup` on the live Operator contradicts the CLI header rule (T02).

## 5. Performance baseline (to be filled by 017-T14)

| Fixture | Rows | Blob bytes | OS/build | Create time | Verify time | Peak RSS growth | Operator p95 during create |
|---|---|---|---|---|---|---|---|

## 6. Unresolved questions

See SPEC section 17 (O-01 to O-06).

## DeepSeek Harness proposals (PROPOSED, owner review required)

Research note, not a decision. Source: [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159) at the pinned SHA (MIT, Copyright (c) 2026 DeepSeek). Local read-only clone `repo_take/deepseek-harness` is gitignored and never staged. Upstream paths below were checked to exist at that SHA on 2026-11-10.

Read-depth limits from the study (do not over-trust): `scripts/` about 58% read, tests about 5% read, installer UI text partly unread. See [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 0 and 15 before turning any item into a design.

- **P-41 Installer and uninstaller safety matrix (data-home overlap, known-folder blocklist, handle pinning, no reparse traversal, honest progress).** Class **ADAPT (as test cases only)**. Upstream: [installer/uninstall-data.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/uninstall-data.h), [installer/uninstall.nsh](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/uninstall.nsh), [installer/path.nsh](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/path.nsh), [installer/extract.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/extract.h), [installer/progress.h](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/installer/progress.h), [tests/expected/windows-uninstall.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-uninstall.json), [tests/expected/windows-installer.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-installer.json). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 11 (11.7) and 14 (row P-41). Lesson: The five uninstall cases (interactive, silent, upgrade, replaced, home-inside) are a ready test outline. The C++ and NSIS code is not reused and the native GDI+ renderer is REJECT. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). If the case list is copied verbatim: if any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment. Where it applies: T08 of package 019; T04 of package 017. Deferral: DEFERRED; Windows laptop.
- **P-44 Streaming secret redactor for subprocess output that holds back a buffer whose prefix could still become a secret.** Class **ADOPT (algorithm port)**. Upstream: [scripts/packaging-run.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-run.mjs), [scripts/packaging-step.mjs](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/packaging-step.mjs). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 8 and 14 (row P-44). Lesson: Whole-chunk replacement leaks a secret split across chunks. `packagingOutputRedactor` is a small pure algorithm, so it ports to Rust directly. It sits beside the masking in `scripts/v1_secret_scan.py` and the secret canary rule (T03 of package 017). Reuse: Algorithm only, reimplement in Rust (or in the LiteCowork TypeScript tree for UI logic); the upstream source has only Node imports so the algorithm ports directly, but the file itself is not imported. If any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment. Where it applies: T07 of package 018; T03 of package 017. Deferral: Not started. Rust unit tests with a secret split at every byte offset run on Linux.

