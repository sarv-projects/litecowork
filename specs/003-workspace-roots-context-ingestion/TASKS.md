# Tasks - Workspace Roots and Context Ingestion

**Feature:** 003 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.0 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)

## Historical baseline (2026-11-09; superseded)

Verified in source on 2026-11-09 (branch `docs/audit-competitor-superset`):

- Root lifecycle store: `crates/storage-core/src/lib.rs` (`create_workspace_root`, `list_workspace_roots`, `update_workspace_root_status`, `resume_workspace_root`, `list_workspace_root_revalidation_candidates`, `commit_workspace_root_revalidation`, `get_workspace_root_scan_candidate`); SQLite in `crates/storage-sqlite/src/workspace_root_membership.rs`, `workspace_root_observations.rs`, `workspace_root_cleanup.rs` with tests in `workspace_root_scan_tests.rs` and `workspace_root_membership_migration_tests.rs`.
- Root add: Tauri `add_workspace_root` in `apps/litecowork-ui/src-tauri/src/lib.rs` calls local-only `POST /__litecowork_local/workspace-root-selection`, mounted in `apps/litecoworkd/src/operator.rs`. `create_workspace_root_from_native_selection` is implemented for Linux/macOS; the `#[cfg(not(any(target_os = "linux", target_os = "macos")))]` fallback returns 503 "Persistent folder access is not qualified on this platform". Test: `apps/litecoworkd/src/workspace_root_selection_operator_tests.rs` (Unix IPC only).
- Scan: `scan_workspace_root` in `apps/litecoworkd/src/workspace_root_scan_operator.rs` returns 501 `SCAN_UNSUPPORTED_PLATFORM` on every non-Linux target; census helpers are `#[cfg(target_os = "linux")]`.
- Windows primitives exist but are not used by root add or scan: `open_directory_path_without_reparse`, `VerifiedDirectory`, `DirectoryIdentity`, `windows_absolute_paths_overlap` in `crates/windows-local-security/src/lib.rs`; Windows identity branches in `apps/litecoworkd/src/local_filesystem.rs`.
- Index and retrieval: `crates/storage-sqlite/src/resource_index.rs`, `rag_retrieval.rs`, `crates/domain-rag/src/local_lexical.rs`, served by `apps/litecoworkd/src/rag_retrieval_operator.rs` (`POST /v1/resources/retrieval`). Read-only; not called by Send; no receipt.
- Send: `conversation_operator.rs` accepts and validates `ConversationTurnResourceRef` (<= 256, same Workspace) and stores `resource_refs` on the message; `apps/litecoworkd/src/agents/conversation_prompt.rs` refuses non-empty refs with `ResourceMaterializationRequired`.
- UI: Persistent folders section in `apps/litecowork-ui/src/App.tsx` (Add always enabled when the Operator is ready); `workspaces/workspace-root-scan.ts` disables Scan only after a 501. `ConversationsPage.tsx` submits `resourceRefs: []`, shows "@ to mention" placeholder copy and an "Add context or file" button with no handler. `resources/retrieval-grounding.ts` decoder has no production call site.

Gaps: Windows add/scan (GA row), Windows path-identity tests, root content ingestion, persisted ContextAttachment, retrieval receipt from Send, @-reference picker, Context Used, qualification on both rows.

The preceding audit is preserved for provenance only. At the 2026-11-12 checkout (`6adadab`), the Conversation composer has an implemented Resource picker and sends selected, revision-pinned `resourceRefs`; Feature 001 performs bounded text materialization at dispatch. This does not implement WorkspaceRoot content ingestion, does not wire grounded retrieval into Send, and does not create durable ContextAttachment/retrieval receipts or a Context Used projection. Root content indexing and native Windows root qualification remain open; use the individual task statuses and `docs/IMPLEMENTATION-STATUS.md` as current evidence.

## 003-T01 - Promote accepted 003 contracts into owner docs and machine schemas

- **Status:** NOT STARTED
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 001 contract-promotion task (turn admission and `resource_refs` materialization contract must be promoted in the same change set or before); orchestrator assigns the next free migration number.
- **Owner:** Resource, Conversation, Storage (contract owners)
- **Allowed paths:** `docs/WORLD-RESOURCES.md`, `docs/CONTEXT.md`, `docs/STORAGE.md`, `docs/API.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/domain-event.schema.json`, `docs/schemas/error-codes.schema.json`, `docs/schemas/sqlite-v35.sql` (new, allocated in docs/LITECOWORK-BUILD-TASKS.md; v32 is the highest present), `crates/storage-sqlite/src/lib.rs` (migration registration only), this package
- **Contract:** SPEC §5 (B-13, B-14 with B-28 rider), §7.2, §15 AC-013 to AC-019; WORLD-RESOURCES roots; CONTEXT attachments
- **Change intent:** Promote `CommitContextAttachment`, `CommitRetrievalReceipt`, `ContextUsedEntry` and `ContextAttachmentError::{RootNotActive, RevisionConflicted, IntegrityFailure, OverBound}` into CONTEXT and STORAGE; add `context_attachments` and `retrieval_receipts` tables with `UNIQUE(workspace_id, idempotency_key)` and foreign keys to the turn and Resource revision; document the Windows root-selection rules (no-reparse open, volume serial + file id identity, UNC/drive-relative/device/ADS rejection) in WORLD-RESOURCES; add a platform-availability field for root add and scan to the Operator readiness or root list response (accepted target interface, not yet in source; exact field fixed here); register any new error code (root-not-active) in SCHEMAS and `error-codes.schema.json`.
- **Acceptance:**
  - [ ] Owner prose and machine schemas change in one reviewed change; no parallel schema authority.
  - [ ] Migration is additive, immutable and registered with `include_str!` like `SQLITE_V1_DDL` in `crates/storage-sqlite/src/lib.rs`; upgrade from v28 test passes.
  - [ ] `scripts/audit_v1_contracts.py` and the package validator pass.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/audit_v1_contracts.py` and `cargo test -p storage-sqlite --locked migration` (filter to be added by this task)
- **Evidence:** promoted doc diffs; migration test name in CURRENT-RUN.md
- **Stop:** stop if a field conflicts with a Feature 001 turn contract; resolve with the 001 owner before migration.

## 003-T02 - Qualify root lifecycle storage on both rows

- **Status:** PARTIAL (Linux, 2026-11-10: `workspace_root_lifecycle` tests in `crates/storage-sqlite/src/workspace_root_lifecycle_tests.rs` pass 3/3 covering pause hides root from scan candidates, stale-version loser, revoke deletes private bindings and is terminal (AC-005), idempotent replay, changed-payload refusal and foreign principal NotFound. NOT yet covered: AC-001 fault injection between root and binding insert, AC-002 overlap body redaction (overlap is decided in the daemon adapter, not the store), concurrent pause/revoke race, Windows row. Create/list/pause/resume/revoke store methods in `crates/storage-core/src/lib.rs` and SQLite implementation with tests in `crates/storage-sqlite/src/workspace_root_scan_tests.rs`; overlap, revoke and status-gating negative coverage not proven on Windows)
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Resource and Storage
- **Allowed paths:** `crates/storage-sqlite/src/workspace_root_membership.rs`, `crates/storage-sqlite/src/workspace_root_cleanup.rs`, `crates/storage-sqlite/src/tests.rs`, `crates/storage-sqlite/src/workspace_root_lifecycle_tests.rs` (new), `crates/storage-core/src/lib.rs`
- **Contract:** SPEC §7a, §10, §11; AC-001, AC-002, AC-004, AC-005
- **Change intent:** Add a lifecycle test module covering atomic Resource + root + location + private binding commit, overlap rejection for ACTIVE/PAUSED/UNAVAILABLE ancestors and descendants, revoke deleting private bindings and replication membership, refusal to reactivate a REVOKED root, and stale Workspace version conflicts. Fix any partial-write path found.
- **Acceptance:**
  - [ ] AC-001: fault injection between root and binding insert leaves no partial row.
  - [ ] AC-002: overlap error body contains no locator bytes (assert on serialized error).
  - [ ] AC-004: PAUSED and UNAVAILABLE roots are excluded from search and context assembly queries.
  - [ ] AC-005: revoke then resume returns a typed refusal; root id never returns to ACTIVE.
  - [ ] Concurrent pause and revoke on the same version: exactly one commits, the loser gets STALE_WORKSPACE_VERSION.
  - [ ] Duplicate idempotency key with identical payload replays; changed payload conflicts.
- **Command:** `cargo test -p storage-sqlite --locked workspace_root_lifecycle` (filter to be added by this task)
- **Evidence:** test names and revision in CURRENT-RUN.md; IMPLEMENTATION-STATUS.md "Workspaces and resources" row
- **Stop:** any partial write under fault injection blocks every later root task.

## 003-T03 - Implement Windows WorkspaceRoot add on the GA row

- **Status:** DEFERRED (Windows-only per owner instruction 2026-11-10; to be tested on the owner's Windows laptop later; nothing here is claimed done) - NOT STARTED (Windows fallback of `create_workspace_root_from_native_selection` in `apps/litecoworkd/src/operator.rs` returns 503; Tauri `add_workspace_root` already has a `#[cfg(windows)]` path-encoding branch in `apps/litecowork-ui/src-tauri/src/lib.rs`)
- **DEFERRED/BLOCKED note:** Why: Windows-only, owner deferred to later laptop testing (A-62). Unblocks when: the owner runs the Windows build on the laptop; Linux implementation needs no unblocking. Test later at: native Windows 11 or 10 x64 on the owner's laptop, not WSL2.
- **Depends on:** T01, T02
- **Cross-package prerequisites:** Feature 011 Windows named-pipe Operator IPC (Wave 0) for the end-to-end call; the handler itself is unit-testable without it.
- **Owner:** Resource, Security/Trust
- **Allowed paths:** `apps/litecoworkd/src/operator.rs` (remove the 503 fallback for Windows and route to the new module), `apps/litecoworkd/src/workspace_root_selection_operator.rs` (new; holds the per-OS handlers), `apps/litecoworkd/src/local_filesystem.rs`, `apps/litecoworkd/src/workspace_root_selection_operator_tests.rs`, `crates/windows-local-security/src/lib.rs` (only if a missing primitive is needed), `apps/litecowork-ui/src-tauri/src/lib.rs` (`add_workspace_root` Windows branch)
- **Contract:** SPEC §7.2 Windows root selection; WORLD-RESOURCES roots; AC-013, AC-001, AC-002, AC-008
- **Change intent:** Add a `#[cfg(windows)]` `create_workspace_root_from_native_selection` that decodes the Tauri-encoded path, rejects UNC, `\\?\` device, drive-relative and alternate-data-stream forms, opens with `open_directory_path_without_reparse`, records `DirectoryIdentity` (volume serial + file id) as the private binding, checks overlap with `windows_absolute_paths_overlap` against every non-REVOKED root, and commits through the existing root store in one transaction. Keep the 503 fallback only for targets that are neither Unix nor Windows. Move the Linux/macOS handler into the new module so `operator.rs` (9,500+ lines) does not grow.
- **Acceptance:**
  - [ ] AC-013: a local NTFS folder is admitted with identity bound; response carries no path.
  - [ ] AC-013 negative: UNC share, `C:relative`, `\\?\GLOBALROOT`, `folder:stream`, junction and directory symlink selections are refused with no row written.
  - [ ] AC-002: case-variant and parent/child overlap on Windows is rejected without revealing the other root's locator.
  - [ ] Duplicate selection with the same path-scoped idempotency key replays the first receipt.
  - [ ] Folder deleted between picker return and open fails closed with no binding.
- **Command:** `cargo test -p litecoworkd --locked workspace_root_selection` and on the Windows row `cargo test -p windows-local-security --locked` and `cargo test -p litecoworkd --locked --target x86_64-pc-windows-msvc windows_workspace_root_selection` (filter to be added by this task)
- **Evidence:** Windows row test log in CURRENT-RUN.md with OS build; IMPLEMENTATION-STATUS.md row updated from 503
- **Stop:** if no-reparse open or identity cannot be proven on the Windows row, keep Add disabled (T08) and do not ship.

## 003-T04 - Windows startup identity revalidation for saved roots

- **Status:** Windows part DEFERRED (owner instruction 2026-11-10); Linux part not reached in this slice. PARTIAL (`list_workspace_root_revalidation_candidates` and `commit_workspace_root_revalidation` exist in `crates/storage-core/src/lib.rs`; Windows identity comparison through `RawDirectoryIdentity::matches_binding` in `apps/litecoworkd/src/local_filesystem.rs` is compiled but not exercised by a Windows root; startup call sites in `apps/litecoworkd/src/runtime.rs`)
- **DEFERRED/BLOCKED note:** Why: Windows part owner deferred (A-62); Linux part was not reached in the slice. Unblocks when: Windows laptop for the Windows part; Linux part is plain implementation work. Test later at: the owner's Windows laptop, and native Linux for the Linux part.
- **Depends on:** T03
- **Cross-package prerequisites:** NONE
- **Owner:** Resource
- **Allowed paths:** `apps/litecoworkd/src/runtime.rs` (startup revalidation call site), `apps/litecoworkd/src/local_filesystem.rs`, `apps/litecoworkd/src/runtime_revalidation_tests.rs`, `crates/storage-sqlite/src/workspace_root_membership.rs`
- **Contract:** SPEC §7a, §11, §12; AC-003
- **Change intent:** Make startup revalidation reopen each ACTIVE and PAUSED Windows root with `open_directory_path_without_reparse` and compare volume serial + file id; on mismatch mark ACTIVE as UNAVAILABLE, keep PAUSED as PAUSED with location UNAVAILABLE, never reopen REVOKED.
- **Acceptance:**
  - [ ] AC-003: folder replaced by a same-name folder (new file id) marks the root UNAVAILABLE.
  - [ ] AC-003: folder replaced by a junction to the original target is UNAVAILABLE, not rebound.
  - [ ] Keyring identity loss blocks startup before the root transaction and does not mark roots UNAVAILABLE.
  - [ ] Restart twice in a row is idempotent (no extra status events).
- **Command:** `cargo test -p litecoworkd --locked workspace_root_revalidation` (filter to be added by this task)
- **Evidence:** Windows and Linux row logs in CURRENT-RUN.md
- **Stop:** any rebind on identity mismatch is a security defect; halt the package.

## 003-T05 - Implement the Windows bounded manual rescan

- **Status:** DEFERRED (Windows-only per owner instruction 2026-11-10; to be tested on the owner's Windows laptop later; nothing here is claimed done) - NOT STARTED (non-Linux returns 501 `SCAN_UNSUPPORTED_PLATFORM` at `scan_workspace_root`; Linux census in `scan_linux_root`; `windows_directory_entry_is_traversable` and `WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT` exist in `apps/litecoworkd/src/local_filesystem.rs`, whose comment states Windows traversal stays unsupported until a race-free handle-relative open API exists)
- **DEFERRED/BLOCKED note:** Why: Windows-only, owner deferred (A-62). Unblocks when: as T03. Test later at: the owner's Windows laptop.
- **Depends on:** T03
- **Cross-package prerequisites:** NONE
- **Owner:** Resource
- **Allowed paths:** `crates/windows-local-security/src/lib.rs`, `apps/litecoworkd/src/workspace_root_scan_operator.rs`, `apps/litecoworkd/src/workspace_root_scan_census.rs` (new, platform-neutral bounds and finalize logic), `apps/litecoworkd/src/local_filesystem.rs`, `crates/storage-sqlite/src/workspace_root_scan_tests.rs`, `apps/litecowork-ui/src-tauri/src/workspace_root_scan_bridge_tests.rs`
- **Contract:** SPEC §5 B-14 with rider, §7a bounds; AC-014, AC-009, AC-011, AC-008
- **Change intent:** Extract the bound checks (`ensure_before_scan_deadline`, entry/depth/locator budgets, `finalize_scan_result`) into a platform-neutral module, keep the Linux traversal, and add a handle-relative enumeration primitive to `crates/windows-local-security` (directory enumeration by handle plus open-by-id or relative open, so a path swap between enumerate and open cannot redirect traversal), then a Windows traversal that uses it, skips entries rejected by `windows_directory_entry_is_traversable`, revalidates the root with `VerifiedDirectory::revalidate_path` before commit, and commits FILE membership only. macOS keeps the 501.
- **Acceptance:**
  - [ ] AC-014: 1,000-entry, depth-32 and 1 MiB locator bounds hold on Windows; the scan stops at the deadline and commits a partial census.
  - [ ] AC-011: unseen Resources are not tombstoned by a partial census.
  - [ ] AC-009: a third concurrent scan is refused; two settle with CAS on the Workspace version.
  - [ ] AC-008: junctions, symlinks and mount points inside the root are not traversed and appear as skipped.
  - [ ] Root replaced during scan fails revalidation and commits nothing.
- **Command:** `cargo test -p litecoworkd --locked workspace_root_scan` and on the Windows row `cargo test -p litecoworkd --locked --target x86_64-pc-windows-msvc windows_workspace_root_scan` (filter to be added by this task)
- **Evidence:** scan logs per row in CURRENT-RUN.md; 501 row removed from IMPLEMENTATION-STATUS.md for Windows only
- **Stop:** if reparse detection cannot be guaranteed per entry, keep Scan disabled on Windows.

## 003-T06 - Symlink, reparse-point and path-identity denial matrix

- **Status:** DEFERRED (Windows-only per owner instruction 2026-11-10; to be tested on the owner's Windows laptop later; nothing here is claimed done) - NOT STARTED
- **DEFERRED/BLOCKED note:** Why: Windows-only reparse-point and path-identity matrix, owner deferred (A-62). Unblocks when: as T03. Test later at: the owner's Windows laptop; the Linux symlink part can be tested here.
- **Depends on:** T03, T05
- **Cross-package prerequisites:** NONE
- **Owner:** Security/Trust, Resource
- **Allowed paths:** `crates/windows-local-security/src/lib.rs` (tests module), `apps/litecoworkd/src/workspace_root_path_identity_tests.rs` (new), `apps/litecoworkd/src/runtime_windows_security_tests.rs`
- **Contract:** SPEC §5 LOCKED (no-follow handles, exact OS identity), §10; AC-008, AC-013, AC-002
- **Change intent:** One table-driven test matrix per OS: Windows directory symlink, junction, volume mount point, `subst` drive, 8.3 short name vs long name, case variants, trailing dot/space, `\\?\` prefix, UNC loopback (`\\localhost\C$`), ADS, hard-linked files inside the root; Linux symlink, bind mount, `..` traversal, non-UTF-8 names. Each case asserts admit/refuse, identity and that no locator appears in events or error bodies.
- **Acceptance:**
  - [ ] AC-008: every link/reparse case is refused or skipped; no traversal escapes the root.
  - [ ] AC-013: 8.3 short name and long name of the same folder resolve to one identity and overlap-reject (or both refuse, per the T01 decision).
  - [ ] AC-002: overlap detection uses components, never string prefix (`C:\a` vs `C:\ab`).
  - [ ] Locator redaction asserted on every error and event payload.
- **Command:** `cargo test -p windows-local-security --locked` and `cargo test -p litecoworkd --locked workspace_root_path_identity` (filter to be added by this task)
- **Evidence:** matrix result table in CURRENT-RUN.md per OS build
- **Stop:** any escape is a release blocker for the Windows row.

## 003-T07 - Bounded content ingestion from ACTIVE roots (B-13)

- **Status:** NOT STARTED (scan records metadata only; the index in `crates/storage-sqlite/src/resource_index.rs` covers uploaded Resources only)
- **Depends on:** T05
- **Cross-package prerequisites:** NONE
- **Owner:** Resource, Storage
- **Allowed paths:** `apps/litecoworkd/src/workspace_root_ingest_operator.rs` (new), `apps/litecoworkd/src/operator.rs` (route mount only), `crates/storage-sqlite/src/resource_index.rs`, `crates/domain-rag/src/limits.rs`, `crates/domain-rag/src/local_lexical.rs`, `apps/litecoworkd/src/resource_index_operator_tests.rs`
- **Contract:** SPEC §5 B-13; WORLD-RESOURCES indexing; AC-007, AC-012, AC-004
- **Change intent:** Add an owner-initiated "Index folder" operation that, for observed FILE members of an ACTIVE root with an allowlisted extension (TXT, Markdown, CSV, JSON, JSONL, NDJSON, Rust, Python, TOML, YAML, JavaScript, TypeScript, CSS), reads at most 1 MiB through the verified root handle outside SQLite, computes the content digest, commits a ResourceRevision and index entry per file, and stores other formats as opaque or skips them. Reuse the existing allowlist and control-character/term-count bounds; do not add a second allowlist.
- **Acceptance:**
  - [ ] AC-007: an over-bound or binary file fails that file only; other files and other roots continue.
  - [ ] AC-012: file modified between read and commit is detected by digest and not indexed as the old revision.
  - [ ] AC-004: a root paused mid-ingest stops further commits.
  - [ ] Repeat ingest with unchanged files creates no new revisions.
- **Command:** `cargo test -p litecoworkd --locked workspace_root_ingest` and `cargo test -p domain-rag --locked` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** any file read inside a SQLite transaction or outside the root handle halts the task.

## 003-T08 - Platform-honest Persistent folders UI

- **Status:** PARTIAL (Persistent folders section in `apps/litecowork-ui/src/App.tsx`, status labels in `apps/litecowork-ui/src/workspaces/workspace-root-status.ts`, scan unsupported handling in `workspace-root-scan.ts`; Add is not disabled before selection on Windows; no post-picker confirm)
- **Depends on:** T01, T03, T05
- **Cross-package prerequisites:** Feature 011 app-wide keyboard and focus baseline.
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/workspaces/WorkspaceRootsPanel.tsx` (new, extracted from `App.tsx`), `apps/litecowork-ui/src/App.tsx`, `apps/litecowork-ui/src/workspaces/workspace-root-status.ts`, `apps/litecowork-ui/src/workspaces/workspace-root-scan.ts`, `apps/litecowork-ui/src/workspaces/workspace-root-mutations.ts`, `apps/litecowork-ui/tests/workspace-root-status.test.ts`, `apps/litecowork-ui/tests/workspace-root-platform-availability.test.ts` (new)
- **Contract:** contracts/ui-behavior.md; V1-UI-ACTION-STATE-LEDGER; AC-010, AC-015
- **Change intent:** Read the T01 platform-availability field; disable Add and Scan with a visible reason before the picker when unavailable; add an explicit confirm step after the picker showing display name, watch policy METADATA and replication NONE; add an "Index folder" action wired to T07; render ACTIVE, PAUSED, UNAVAILABLE and REVOKED with distinct text and repair hints; loading, empty, denied, offline and stale-version states visible.
- **Acceptance:**
  - [ ] AC-015: on an unqualified platform the picker never opens and the reason is announced.
  - [ ] AC-010: PAUSED is never shown as REVOKED; UNAVAILABLE is never shown as a successful scan.
  - [ ] Stale Workspace version shows a refresh prompt and loses no input.
  - [ ] Keyboard-only add, confirm, pause, resume, revoke and scan with visible focus.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; V1-UI-ACTION-STATE-LEDGER rows updated in T16
- **Stop:** do not enable Add on Windows until T06 passes.

## 003-T09 - ContextAttachment and retrieval-receipt stores

- **Status:** NOT STARTED (no table, store trait or event exists; `CONTEXT_ATTACHMENT` appears only as an AgentControl capability enum)
- **Depends on:** T01, T02
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Conversation context owner
- **Allowed paths:** `crates/storage-core/src/context_attachment.rs` (new), `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/context_attachments.rs` (new), `crates/storage-sqlite/src/context_attachment_tests.rs` (new), `crates/storage-sqlite/src/lib.rs`
- **Contract:** SPEC §7.2; DATA-MODEL; AC-018, AC-012, AC-016, AC-004, AC-005
- **Change intent:** Implement `ContextAttachmentStore::commit_context_attachment` and a `RetrievalReceiptStore::commit_retrieval_receipt` in one SQLite IMMEDIATE transaction (`tx-attach`): verify the root is ACTIVE, the revision is pinned and its digest matches, the Conversation version matches `expected_conversation_version`, then insert receipt, attachments and idempotency receipt. Add a `list_context_used(conversation_id, turn_id)` projection.
- **Acceptance:**
  - [ ] AC-018: attachments persist and reload identically after reopening the database.
  - [ ] AC-012: digest mismatch returns `INTEGRITY_FAILURE` and writes nothing.
  - [ ] AC-004/AC-005: PAUSED, UNAVAILABLE or REVOKED root returns root-not-active.
  - [ ] AC-016: identical replay returns the original receipt; changed payload under the same key conflicts.
  - [ ] Wrong Workspace, foreign principal and stale Conversation version fail with no partial write.
- **Command:** `cargo test -p storage-sqlite --locked context_attachment` and `cargo test -p storage-core --locked context_attachment` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** any external call inside `tx-attach` halts the task.

## 003-T10 - Call retrieval from Send and commit the receipt before dispatch

- **Status:** NOT STARTED (`rag_retrieval_operator.rs` is not called by `conversation_operator.rs`)
- **Depends on:** T09
- **Cross-package prerequisites:** Feature 001 composer attachments handler and B-08 ResourceRef materialization (the prompt encoder must stop refusing refs); Feature 001 durable dispatch outbox.
- **Owner:** Conversation, Resource
- **Allowed paths:** `apps/litecoworkd/src/context_attachment_service.rs` (new), `apps/litecoworkd/src/conversation_operator.rs` (turn admission call site only, coordinated with the 001 owner), `apps/litecoworkd/src/rag_retrieval_operator.rs` (extract a callable retrieval function from `grounded_retrieval`), `apps/litecoworkd/src/conversation_operator_tests.rs`
- **Contract:** SPEC §6 actor sequence steps 4 and 5; CONTEXT; AC-016, AC-018, AC-006
- **Change intent:** Extract the body of `grounded_retrieval` into a service function; at turn admission, when the owner enabled grounding or attached explicit refs, run retrieval outside SQLite, then commit receipt + attachments via T09 before the 001 dispatch outbox row is written, and hand only the committed pinned refs to 001 materialization. Insufficient evidence is committed as an outcome, never as a fake citation.
- **Acceptance:**
  - [ ] AC-016: every grounded turn has exactly one receipt committed before dispatch.
  - [ ] AC-006: the context package sent to the agent contains only committed attachments.
  - [ ] Retrieval provider unavailable commits PROVIDER_UNAVAILABLE and the turn proceeds without grounding, labelled.
  - [ ] Duplicate Send with the same idempotency key does not run retrieval twice.
- **Command:** `cargo test -p litecoworkd --locked conversation_turn_retrieval_receipt` (filter to be added by this task)
- **Evidence:** test names; cross-layer trace in T14
- **Stop:** if 001 turn admission cannot place the receipt before dispatch, stop and resolve with the 001 owner.

## 003-T11 - @-reference ResourceRef picker in the Feature 001 composer

- **Status:** PARTIAL - the Conversation composer has a Resource picker and submits selected revision-pinned `resourceRefs` (Feature 001 text materialization); this task's WorkspaceRoot ingestion-backed selection behavior remains incomplete.
- **Depends on:** T09
- **Cross-package prerequisites:** Feature 001 composer attachments handler (owns submit and `resourceRefs` wiring).
- **Owner:** Desktop UI, Resource
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ResourceRefPicker.tsx` (new), `apps/litecowork-ui/src/conversations/resource-ref-picker.ts` (new), `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (picker mount and button handler only, coordinated with the 001 owner), `apps/litecowork-ui/src/resources/resource-search-request.ts`, `apps/litecowork-ui/tests/resource-ref-picker.test.ts` (new)
- **Contract:** SPEC §15 AC-017; NATIVE-COMPOSER-LLD; CONTEXT
- **Change intent:** On "@" or the "Add context or file" button open a picker that queries existing Resource search for the current Workspace (debounced, bounded page), shows only Resources under ACTIVE roots or admitted uploads, pins each selection to its current revision id, renders removable chips, and passes `{workspace_id, resource_id, revision_id}` to the 001 submit. Unsupported formats are listed as not attachable with a reason.
- **Acceptance:**
  - [ ] AC-017: picker is a labelled listbox; arrow keys, Enter, Escape and chip removal work without a mouse; screen reader announces result counts.
  - [ ] Resources of another Workspace or a REVOKED root never appear.
  - [ ] Offline Operator shows an error state and keeps the typed text.
  - [ ] Revision changes after selection keep the pinned revision and show it.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; NATIVE-COMPOSER-LLD and UI ledgers updated in T16
- **Stop:** never imply that an ordinary uploaded Resource was ingested from a WorkspaceRoot; keep root-backed selection unavailable until the root content/revision path is implemented and can be reauthorized at dispatch.

## 003-T12 - Context Used surface for grounded answers

- **Status:** NOT STARTED (`apps/litecowork-ui/src/resources/retrieval-grounding.ts` decoder exists with no production call site)
- **Depends on:** T10
- **Cross-package prerequisites:** Feature 001 committed message projection.
- **Owner:** Desktop UI, Conversation
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ContextUsedPanel.tsx` (new), `apps/litecowork-ui/src/resources/retrieval-grounding.ts`, `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs` (read command), `apps/litecoworkd/src/conversation_operator.rs` (read route for `list_context_used`, accepted target interface), `apps/litecowork-ui/tests/knowledge-grounding.test.ts`, `apps/litecowork-ui/tests/context-used.test.ts` (new)
- **Contract:** SPEC §2, §6; AC-006, AC-010
- **Change intent:** Add a "Context used" disclosure under each grounded message listing committed attachments with Resource name, exact revision, root status label and cited span; open the exact revision read-only in the Workbench; show "insufficient evidence" and "grounding unavailable" outcomes from the receipt.
- **Acceptance:**
  - [ ] AC-006: entries come only from committed attachments and receipts; a bare search hit never appears.
  - [ ] AC-010: a later-revoked root shows REVOKED on the entry without re-exposing bytes.
  - [ ] Empty, loading and error states are distinct and keyboard reachable.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `cargo test -p litecoworkd --locked context_used_projection` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** if the projection cannot be sourced from receipts, do not render the panel.

## 003-T13 - Failure, race and recovery suite

- **Status:** NOT STARTED
- **Depends on:** T04, T05, T10, T11
- **Cross-package prerequisites:** Feature 001 startup sweep and dispatch outbox.
- **Owner:** Resource, Conversation, Storage
- **Allowed paths:** `apps/litecoworkd/src/context_attachment_recovery_tests.rs` (new), `apps/litecoworkd/src/runtime_recovery_tests.rs`, `crates/storage-sqlite/src/context_attachment_tests.rs`
- **Contract:** SPEC §11, §12; contracts/errors-recovery.md; AC-019, AC-018, AC-003, AC-009
- **Change intent:** Fault-injection and race tests: revoke between pick and dispatch, pause during ingest, crash after receipt commit before dispatch, crash during scan, daemon restart during Windows rescan, duplicate Send replay, cancellation of a turn after receipt commit.
- **Acceptance:**
  - [ ] AC-019: revoke-during-send refuses the attachment and no bytes reach the agent.
  - [ ] AC-018: crash after receipt commit reloads the same receipt; retrieval is not re-run with a different result for that turn.
  - [ ] AC-003: restart during a Windows rescan leaves no partial census and rebinds only on exact identity.
  - [ ] AC-009: two concurrent scans plus a revoke settle deterministically.
  - [ ] Cancelled turn keeps its receipt for provenance and dispatches nothing.
- **Command:** `cargo test -p litecoworkd --locked context_attachment_recovery` and `cargo test -p storage-sqlite --locked context_attachment` (filter to be added by this task)
- **Evidence:** test names and fault points in CURRENT-RUN.md
- **Stop:** any UNKNOWN state reported as success halts qualification.

## 003-T14 - Cross-layer traces for every 003 user operation

- **Status:** NOT STARTED
- **Depends on:** T08, T11, T12
- **Cross-package prerequisites:** NONE
- **Owner:** Resource, Desktop UI
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `contracts/api-events.md`), `docs/SEMANTIC-SERVED-UI-MAP.md`
- **Contract:** AGENTS.md §4; AC-006, AC-013, AC-017
- **Change intent:** Write the trace (user outcome, control, typed UI command, IPC/Operator route, authorization, service, transaction/fence, record/event/receipt, projection, restart, evidence) for: add root, confirm, pause/resume/revoke, rescan, index folder, @-reference attach, grounded Send, Context Used open.
- **Acceptance:**
  - [ ] Every control maps to a served route; no route lacks a UI path or an explicit "no UI by design" note.
  - [ ] Each trace cites the test that proves its restart step.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/audit_served_routes.py`
- **Evidence:** trace tables in contracts/ui-behavior.md
- **Stop:** an untraced control blocks T15.

## 003-T15 - Qualify E2E-03 on the Windows and Linux release rows

- **Status:** NOT STARTED
- **Depends on:** T06, T07, T13, T14
- **Cross-package prerequisites:** Feature 001 qualified OpenCode v2.0.32 Send route; Feature 011 packaged desktop and Windows named-pipe IPC; Feature 019 release-matrix rows and evidence bundle process.
- **Owner:** Resource, Desktop UI, release owner
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md` (evidence rows only), `CURRENT-RUN.md`, this package
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-03; SPEC §15 all AC IDs; AC-013, AC-014, AC-016, AC-019
- **Change intent:** Run E2E-03 (attach folder, index supported Resources, ask grounded question, revoke root) in the packaged desktop on Windows 11/10 x64 with OpenCode v2.0.32, then on Linux x64; include denial (reparse folder, overlap), recovery (restart mid-scan, revoke-during-send) and duplicate replay. Collect CODE, SYSTEM and owner USER evidence per row.
- **Acceptance:**
  - [ ] Exact-revision citations shown in Context Used; revoked root excluded from later attachment and retrieval.
  - [ ] AC-013/AC-014 pass on native Windows (WSL2 is not evidence).
  - [ ] AC-016/AC-019 pass in the packaged build.
  - [ ] Owner USER sign-off recorded per row with build, OS, agent version and auth route.
- **Command:** `cargo test -p litecoworkd --locked workspace_root` and `pnpm --dir apps/litecowork-ui test` and `python3 scripts/v1_platform_matrix.py`
- **Evidence:** evidence bundle path per ACCEPTANCE-AND-E2E-PLAN; CURRENT-RUN.md; IMPLEMENTATION-STATUS.md row
- **Stop:** a failing denial or recovery case on a row keeps that row unqualified.

## 003-T16 - Documentation and ledger sync

- **Status:** NOT STARTED
- **Depends on:** T15
- **Cross-package prerequisites:** orchestrator-owned register and traceability files.
- **Owner:** Resource (docs owner for this package)
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json` (regenerated), `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md` (B-13, B-14 rows closed), this package
- **Contract:** AGENTS.md §3, §4; AC-010, AC-015
- **Change intent:** Regenerate the served-route inventory, update Workspaces and resources capability rows (Windows add/scan, retrieval receipt, Context Used), update UI ledgers for Add/Scan/Index/@-picker/Context Used, and mark SPEC §4 rows with the new evidence.
- **Acceptance:**
  - [ ] `scripts/audit_served_routes.py` output matches the mounted routes.
  - [ ] No document claims a level above the recorded evidence.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of the regenerated inventory; validator PASS line
- **Stop:** an inventory mismatch blocks T18.

## 003-T17 - Context Debugger surface (P-15)

- **Status:** NOT STARTED - added by owner approval on 2026-11-09 (docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md). Folds proposed task P-15 from docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md section 8 into this LOCKED package.
- **Depends on:** T16
- **Cross-package prerequisites:** Feature 001 turn settlement so a committed turn exists to inspect.
- **Owner:** Resource, Conversation, Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/`, `apps/litecoworkd/src/`, `specs/003-workspace-roots-context-ingestion/contracts/ui-behavior.md`, adjacent tests.
- **Contract:** SPEC §15 AC-018, AC-020; CONTEXT; PRESENTATION-RUNTIME.
- **Change intent:** add a Context Debugger surface that shows exactly what entered the model context for a turn: the attachments, the retrieved passages with their source revisions, the skills in scope, and the applied limits (budget, truncation notice). It reads the durable retrieval receipt and ContextAttachment records (AC-016, AC-018); it reports only what was actually sent, never a reconstruction, and it never materializes bytes outside an admitted root.
- **Acceptance:**
  - [ ] The surface lists attachments, retrieved passages with exact source revisions, in-scope skills and the applied limits for a committed turn (AC-020).
  - [ ] Values come from the stored retrieval receipt and ContextAttachment records and match after restart (AC-018, AC-020).
  - [ ] Negative: a turn with no retrieval shows an explicit "no retrieval" state, not an empty or fabricated list.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** UI test output and contracts/ui-behavior.md row in Completion evidence.
- **Stop:** the surface cannot read a durable receipt; show unavailable rather than reconstruct context.

## 003-T18 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T17
- **Cross-package prerequisites:** Feature 019 release qualification sign-off process.
- **Owner:** Resource, Conversation, Storage, Security/Trust, Desktop UI; repository owner sign-off
- **Allowed paths:** this package (status fields), `docs/IMPLEMENTATION-STATUS.md`
- **Contract:** SPECIFICATION-STANDARD lifecycle; SPEC §15 AC-001 to AC-020
- **Change intent:** Assemble the evidence bundle (per-task revision, command, result, OS, harness, reviewer); move status ACCEPTED/LOCKED to IMPLEMENTING when T02 starts and to VERIFIED only after owner sign-off on both rows.
- **Acceptance:**
  - [ ] Every AC ID has passing evidence on each declared row or a recorded owner exception.
  - [ ] Owner sign-off recorded with date and revision.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle; README and SPEC status lines; IMPLEMENTATION-STATUS.md
- **Stop:** any missing denial or recovery evidence keeps the package IMPLEMENTING.

## Dependency order

T01 -> T02 -> T03 -> (T04, T05) -> T06; T05 -> T07; (T01, T03, T05) -> T08; (T01, T02) -> T09 -> (T10, T11); T10 -> T12; (T04, T05, T10, T11) -> T13; (T08, T11, T12) -> T14; (T06, T07, T13, T14) -> T15 -> T16 -> T17 (Context Debugger, P-15) -> T18 (VERIFIED). The Windows track (T03-T06) and the Send track (T09-T12) run in parallel after T02.

## Cross-package dependencies

Other packages renumber their tasks in the same authoring wave, so prerequisites are named by package and gap item; the orchestrator maps them to task IDs.

| This task | Other package task | Reason |
|---|---|---|
| T01 | Feature 001 contract promotion | Turn admission and `resource_refs` contract change together |
| T03, T15 | Feature 011 Windows named-pipe Operator IPC (Wave 0) | Windows root add is reached only through authenticated IPC |
| T08 | Feature 011 app-wide keyboard and focus baseline | Shared focus model for the folders panel |
| T10, T11 | Feature 001 composer attachments handler + B-08 materialization | Prompt encoder refuses refs today (`ResourceMaterializationRequired`) |
| T10, T13 | Feature 001 durable dispatch outbox + startup sweep | Receipt must commit before the outbox row |
| T12 | Feature 001 committed message projection | Context Used hangs off committed messages |
| T15 | Feature 001 qualified OpenCode v2.0.32 route; Feature 019 release-matrix rows | E2E-03 needs a real Send on each row |
| (consumer) | Feature 014 Conversation delete fences | 014 removes attachments from projections using T09 stores |
| (consumer) | Feature 004 and 005 Resource materializer | Reuse pinned refs and ContextAttachments |

## Implementation-language and optimization notes

- Keep all path identity, traversal, ingestion and receipt logic in Rust in the daemon (`apps/litecoworkd`, `crates/windows-local-security`, `crates/storage-sqlite`). The UI only renders status and sends typed commands; it never inspects paths.
- `apps/litecoworkd/src/operator.rs` is over 9,500 lines. T03 moves both root-selection handlers into `workspace_root_selection_operator.rs` instead of adding a third cfg branch there.
- T05 shares one platform-neutral census module across Linux and Windows. Today every bound helper is `#[cfg(target_os = "linux")]`, so a Windows port without extraction would duplicate the bounds and risk drift.
- T10 extracts retrieval into a service function so Send calls it in-process; going through the HTTP route from inside the daemon would add a second auth hop and serialization for no benefit.
- `apps/litecowork-ui/src/App.tsx` is over 4,500 lines; T08 extracts the Persistent folders section into `WorkspaceRootsPanel.tsx` so its states are unit-testable.
- T11 debounces the picker query and reuses `resource-search-request.ts` paging rather than adding a new search client.

## Completion evidence

For each task record in CURRENT-RUN.md and the evidence bundle: git revision, exact command, pass/fail result and test names, OS and build (for example Windows 11 23H2 x64, Ubuntu 24.04 x64), harness and version (OpenCode v2.0.32) where a Send is involved, auth route, reviewer, and the AC IDs covered. A task is DONE only with that record; a passing structural validator is not evidence.
