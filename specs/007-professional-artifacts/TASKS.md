# Tasks - Professional Artifacts One Format

**Feature:** 007 | **Status:** Base revision 1.0 ACCEPTED/LOCKED; revision 1.1 Saved View amendment IN_REVIEW. Existing tasks are authorized only within their accepted 1.0 contracts; Saved View work requires T01/T02 promotion and renewed owner review.
**Spec revision:** Base 1.0 accepted / Saved View amendment 1.1 TARGET-IN_REVIEW | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)

## Current baseline

Verified in source on 2026-11-09 (branch `docs/audit-competitor-superset`):

- Routes in `apps/litecoworkd/src/artifact_operator.rs`: `GET /v1/artifacts`, `GET /v1/library`, `GET /v1/artifacts/{artifact_id}`, version metadata and content (`download_artifact_version`), edit-head (`get_text_edit_head`), `POST .../text-version` (`append_text_version`), promote, archive (`library_command`), `GET /v1/tasks/{task_id}/artifacts`. No create route, no export route.
- Store traits in `crates/storage-core/src/artifact.rs`: `ArtifactReadStore`, `ArtifactLibraryWriteStore`, `ArtifactVersionWriteStore` (`stage_text_content`, `append_artifact_version`, `resolve_artifact_version_append_replay`); SQLite in `crates/storage-sqlite/src/artifacts.rs`, tests in `artifact_tests.rs`. `ArtifactRecord` has `task_id` and no Conversation link.
- Tauri: `artifact_read`, `artifact_library_command`, `artifact_save_as`, `artifact_edit_head`, `artifact_append_text_version` in `apps/litecowork-ui/src-tauri/src/artifact_bridge.rs`; `resource_save_as` in `resource_save_bridge.rs`.
- UI in `apps/litecowork-ui/src/artifacts/`: `ArtifactLibrary.tsx` (no create control), `ArtifactWorkbench.tsx` (edit gated on `media_type == "text/plain"`), `StructuredTextPreview.tsx` (restricted Markdown renderer with raw fallback), `artifact-api.ts` (`supportsTextPreview`), `text-version-diff-model.ts` (bounded LCS `lineDiff`), `TaskArtifactOutputs.tsx`.
- Office: `apps/litecoworkd/src/environment_office_qualification.rs` readiness probes only.

Gaps: create (B-30), Markdown adapter (B-21), export receipt (B-31), declared capability matrix with DOCX/XLSX/PPTX/PDF read-only or unsupported, Conversation association, restart/compare qualification on both rows, and the TARGET-only Saved View create/open/update/archive lifecycle. See `contracts/saved-view.md`; that amendment is not implementation authority until owner review and T01/T02 promotion.

## 007-T01 - Promote accepted 007 contracts into owner docs and machine schemas

- **Status:** PARTIAL - selected and pinned Comrak 0.59.0 (`=0.59.0`, BSD-2-Clause, upstream commit `ac9d2081fe21b151ad3b3e42fea0e870e6272999`, MSRV 1.89) in `apps/litecoworkd/Cargo.toml` with default features disabled; research records the upstream release and the remaining adapter conformance checks. Owner/API/event/migration promotion is not complete.
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 001 committed-message identity (message id used by `ConversationOrigin`); orchestrator assigns the next free migration number.
- **Owner:** Artifacts, Storage, Product (B-31 caveat)
- **Allowed paths:** `docs/ARTIFACTS-EVIDENCE.md`, `docs/API.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/domain-event.schema.json`, `docs/schemas/error-codes.schema.json`, `docs/schemas/saved-view-artifact.schema.json`, `docs/schemas/sqlite-v38.sql` (new, allocated in docs/LITECOWORK-BUILD-TASKS.md; v36 is wired and v38 remains the current reservation after v37), `crates/storage-sqlite/src/lib.rs` (migration registration only), `apps/litecoworkd/Cargo.toml` (Markdown engine dependency pin), this package
- **Contract:** SPEC §5 B-21, B-30, B-31, capability matrix; §7.2; AC-010, AC-006, AC-016, AC-017, AC-019, AC-020..AC-027 (TARGET amendment)
- **Change intent:** Promote `CreateArtifact` (with `ConversationOrigin` and `RICH_PRESENTATION_SAVED_VIEW`), `ArtifactFormatAdapter`, `CommitArtifactExport`, `FormatCapability`, `FormatError` and `ArtifactError` into ARTIFACTS-EVIDENCE; add atomic Saved View creation in `SAVED` status only for explicit save intent, exact Saved View version append, Library kind filter, the Markdown publish route and export/capability routes to OpenAPI; register `artifact.exported.v1` but no new Saved View event; add `docs/schemas/saved-view-artifact.schema.json`, Artifact export receipt storage and Conversation origin columns; pin the Markdown engine. The Saved View write remains a typed exact-source request whose bytes are assembled and validated by the daemon, not client-supplied arbitrary JSON. T01 cannot close until Feature 016 T01/T02 agree on stable input IDs/profile validation and the Feature 007 amendment receives cross-owner review.
- **Acceptance:**
  - [ ] Owner prose and machine schemas change in one reviewed change; one error code for unsupported media. Saved View AC-020..AC-027 remain blocked until Artifact/Storage/Conversation/Privacy/Backup/Product review and matching Feature 016 T01/T02 promotion; they preserve B-40 and do not admit changed session values.
  - [ ] The conflicting B-31 order in SPEC §7 versus HLD/API/T06, and its write-before-receipt recovery, are resolved and owner-approved under A-81 before its export command/event/schema is promoted.
  - [x] Product recorded the local-export receipt destination class (B-31 caveat): `LOCAL_OWNER_SELECTED_FILE` (sarv-projects, 2026-11-10).
  - [ ] Migration is additive and immutable; upgrade from v28 test passes.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/audit_v1_contracts.py` and `cargo test -p storage-sqlite --locked migration` (filter to be added by this task)
- **Evidence:** promoted doc diffs; dependency pin in RESEARCH.md; migration test in CURRENT-RUN.md
- **Stop:** the destination class is confirmed (2026-11-10); T06 also remains blocked on A-81 until receipt/effect ordering and local-write recovery are reviewed and owner-approved.

## 007-T02 - Atomic Artifact create store (B-30)

- **Status:** NOT STARTED (no create method in `crates/storage-core/src/artifact.rs`; tests seed Artifact rows directly)
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Artifacts, Storage
- **Allowed paths:** `crates/storage-core/src/artifact.rs`, `crates/storage-sqlite/src/artifacts.rs`, `crates/storage-sqlite/src/artifact_create_tests.rs` (new), `crates/storage-sqlite/src/blob.rs`
- **Contract:** SPEC §5 B-30, §7.2 `CreateArtifact`; ARTIFACTS-EVIDENCE create; AC-010, AC-012, AC-009
- **Change intent:** Add `ArtifactCreateStore::create_artifact(CreateArtifact) -> CommittedArtifactCreate` that stages the first-version blob outside SQLite, then in one IMMEDIATE transaction inserts Artifact, ARTIFACT Resource, ResourceRevision 1, ArtifactVersion 1, provenance and input edges, optional Conversation origin, snapshot, canonical events and principal-scoped idempotency receipt. Preserve normal source/status rules. The explicit `RICH_PRESENTATION_SAVED_VIEW` source is the only path that creates directly in `SAVED`; it must atomically recheck active Workspace, exact committed message/presentation digests and source deletion fencing, and store a self-contained canonical envelope.
- **Acceptance:**
  - [ ] AC-010: fault injection at each insert leaves no visible partial record; staged blob becomes GC-eligible.
  - [ ] AC-012: identical replay returns the original receipt; changed payload under the same key conflicts.
  - [ ] AC-009: an Imported source never relabels the original Resource.
  - [ ] Archived Workspace, foreign principal and wrong Workspace are refused with no write.
- **Command:** `cargo test -p storage-sqlite --locked artifact_create` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** any external call inside the transaction halts the task.

## 007-T03 - Markdown format adapter (B-21)

- **Status:** NOT STARTED
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Artifacts
- **Allowed paths:** `apps/litecoworkd/src/artifact_format.rs` (new), `apps/litecoworkd/src/artifact_format_tests.rs` (new), `apps/litecoworkd/Cargo.toml`
- **Contract:** SPEC §5 B-21, §7.2 `ArtifactFormatAdapter`; AC-019, AC-007, AC-011
- **Change intent:** Implement `ArtifactFormatAdapter` for `text/markdown` with the T01-pinned engine: `validate` (UTF-8, <= 1 MiB, CommonMark + GFM tables), `round_trip` (parse to AST, serialize to canonical form, require byte-stable second pass), `export` (`text/markdown` canonical bytes, <= 10 MiB). Raw HTML blocks and inline HTML are preserved as inert text and never executed; links are not fetched.
- **Acceptance:**
  - [ ] AC-019: property test over generated documents: `round_trip(round_trip(x)) == round_trip(x)`.
  - [ ] AC-019: invalid UTF-8 or unbalanced constructs return `ValidationFailed`.
  - [ ] AC-007: 1 MiB + 1 byte input and over-bound export return `TooLarge`.
  - [ ] AC-011: `<script>`, `javascript:` links and SVG with handlers stay inert text in output and validation.
  - [ ] Shared fixture set also passes through the UI preview (T09) so both dialects agree on tables and fallback.
- **Command:** `cargo test -p litecoworkd --locked artifact_format` (filter to be added by this task)
- **Evidence:** test names and engine version in CURRENT-RUN.md
- **Stop:** if the engine cannot produce a byte-stable canonical form, stop and return to the owner with the B-21 engine choice.

## 007-T04 - Markdown version publish with CAS

- **Status:** PARTIAL (`append_text_version` and `ArtifactVersionWriteStore::append_artifact_version` publish `text/plain` with `If-Match`, content version, Resource version and parent ResourceRevision pins)
- **Depends on:** T02, T03
- **Cross-package prerequisites:** NONE
- **Owner:** Artifacts, Storage
- **Allowed paths:** `apps/litecoworkd/src/artifact_operator.rs`, `crates/storage-sqlite/src/artifacts.rs`, `crates/storage-sqlite/src/artifact_tests.rs`
- **Contract:** SPEC §7, §11; AC-001, AC-013, AC-008
- **Change intent:** Allow `text/markdown` through the append path only after the T03 adapter validates and round-trips the bytes outside SQLite; keep `text/plain` behavior unchanged; refuse every other media type with `UNSUPPORTED_MEDIA_TYPE`. Add a separate `saved-view-versions` append path only after the shared Rust validator verifies the closed Saved View envelope, embedded RichPresentation and exact source lineage; pin both Artifact and Resource versions. The update body must reference a new committed eligible presentation and must not include view-local calculator state. Never route Saved View JSON through Markdown parsing or the generic text append.
- **Acceptance:**
  - [ ] AC-001: save commits a new immutable version or leaves the prior version current.
  - [ ] AC-013: two concurrent publishes against one head commit exactly one; the loser gets `STALE_VERSION` with no partial version, receipt or event.
  - [ ] AC-008: crash between staging and commit leaves the head unchanged.
  - [ ] Archived Artifact returns `ARTIFACT_ARCHIVED`.
- **Command:** `cargo test -p storage-sqlite --locked artifact_versions` and `cargo test -p litecoworkd --locked artifact_markdown_publish` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** any Markdown save that bypasses the adapter halts the task.

## 007-T05 - Declared format capability matrix producer

- **Status:** NOT STARTED (capability is inferred in the UI: `supportsTextPreview` in `artifact-api.ts` and the `text/plain` edit gate in `ArtifactWorkbench.tsx`)
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Artifacts
- **Allowed paths:** `apps/litecoworkd/src/artifact_capabilities.rs` (new), `apps/litecoworkd/src/artifact_operator.rs` (route mount), `apps/litecoworkd/src/artifact_capabilities_tests.rs` (new)
- **Contract:** SPEC §5 declared V1 matrix; release contract §4; AC-016, AC-005
- **Change intent:** Serve the SPEC §5 matrix as a static, versioned table keyed by exact media type, including the Markdown engine and the Saved View profile/renderer versions. The Saved View row permits only validated presentation-version updates from a new committed source; session-modified input values are excluded under B-40. Export, generic editing and compare stay unsupported. Every write route checks the same table server-side, so a client that ignores the matrix still gets `UNSUPPORTED_MEDIA_TYPE`.
- **Acceptance:**
  - [ ] AC-016: DOCX, XLSX, PPTX and PDF report edit and export UNSUPPORTED and view UNSUPPORTED (or READ_ONLY for PDF only after T10 qualification).
  - [ ] AC-005: an edit or export request for an unsupported type is refused server-side with no write.
  - [ ] Media-type parameters and case (`Text/Markdown; charset=utf-8`) normalize to one row.
- **Command:** `cargo test -p litecoworkd --locked artifact_capabilities` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** a matrix row that claims more than its tests prove is a release blocker.

## 007-T06 - Export command and ArtifactExportReceipt (B-31)

- **Status:** BLOCKED on A-81 owner review. The Product gate is closed: destination class `LOCAL_OWNER_SELECTED_FILE` confirmed 2026-11-10. It also depends on T01, T03 and T05. SPEC §7 currently orders receipt before external work, while T06/API order receipt after the verified local write. No export record, route or event exists.
- **Depends on:** T01, T03, T05
- **Cross-package prerequisites:** NONE
- **Owner:** Artifacts, Storage, Security
- **Allowed paths:** `crates/storage-core/src/artifact.rs`, `crates/storage-sqlite/src/artifact_exports.rs` (new), `crates/storage-sqlite/src/artifact_export_tests.rs` (new), `apps/litecoworkd/src/artifact_operator.rs`
- **Contract:** SPEC §5 B-31, §7.2 `CommitArtifactExport`; contracts/api-events.md export ordering; AC-006, AC-012, AC-011, AC-008
- **Change intent:** Pin the exact version and digest, run `ArtifactFormatAdapter::export` outside SQLite, return bytes to the Tauri bridge for the native-picker write, verify the written digest, then commit `ArtifactExportReceipt` and `artifact.exported.v1` with destination class only. A failed or cancelled write commits no receipt. Native Save As stays separate and unchanged.
- **Acceptance:**
  - [ ] AC-006: receipt is distinct from the version commit and lists exact version, format, engine version and digest.
  - [ ] AC-012: replay returns the original receipt; changed payload conflicts.
  - [ ] AC-011: no local path appears in the receipt, event or logs.
  - [ ] AC-008: crash after write before receipt commit leaves no receipt; restart reports the export as not recorded rather than successful.
- **Command:** `cargo test -p storage-sqlite --locked artifact_export` and `cargo test -p litecoworkd --locked artifact_export_route` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** no receipt may claim success for an unverified write. Do not implement the export route or receipt store until A-81 resolves both ordering and restart recovery with owner approval; keep Save As only in the meantime.

## 007-T07 - Operator routes and Tauri bridge commands

- **Status:** NOT STARTED
- **Depends on:** T02, T04, T05, T06
- **Cross-package prerequisites:** Feature 011 Windows named-pipe Operator IPC for the Windows row.
- **Owner:** Artifacts, Desktop UI (bridge)
- **Allowed paths:** `apps/litecoworkd/src/artifact_operator.rs`, `apps/litecoworkd/src/artifact_create_operator.rs` (new, to keep `artifact_operator.rs` focused), `apps/litecowork-ui/src-tauri/src/artifact_bridge.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs` (handler registration), `apps/litecowork-ui/src/artifacts/desktop-artifact-api.ts`, `apps/litecowork-ui/src/artifacts/artifact-api.ts`
- **Contract:** contracts/api-events.md accepted target operations; AC-010, AC-006, AC-016
- **Change intent:** Mount `POST /v1/artifacts`, the Markdown publish route, `POST /v1/artifacts/{artifact_id}/exports` and the capability read with owner authorization, Workspace scope, `Idempotency-Key` and `If-Match`; add Tauri commands `artifact_create`, `artifact_export` and `artifact_capabilities` next to the existing `artifact_read`; generate typed TS clients from the promoted OpenAPI.
- **Acceptance:**
  - [ ] AC-010: authenticated create returns the receipt; unauthenticated or foreign principal is refused.
  - [ ] AC-006: export bytes never cross WebView IPC; only status and receipt id do.
  - [ ] AC-016: capability read matches the T05 table.
  - [ ] Duplicate bridge calls with the same idempotency key produce one receipt.
- **Command:** `cargo test -p litecoworkd --locked artifact_operator` and `cargo test -p litecowork-ui --locked artifact_bridge` (filter to be added by this task)
- **Evidence:** test names; served-route inventory diff in T16
- **Stop:** any byte payload above 10 MiB crossing IPC halts the task.

## 007-T08 - Conversation association of Artifacts

- **Status:** NOT STARTED (`ArtifactRecord` has `task_id` only; no Conversation producer)
- **Depends on:** T02, T07
- **Cross-package prerequisites:** Feature 001 committed message projection and message ids; Feature 014 Conversation delete fences (consumer of the association).
- **Owner:** Artifacts, Conversation, Desktop UI
- **Allowed paths:** `crates/storage-sqlite/src/artifacts.rs`, `apps/litecoworkd/src/artifact_operator.rs` (`conversation_id` list filter), `apps/litecowork-ui/src/conversations/ConversationArtifacts.tsx` (new), `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (message action mount only, coordinated with the 001 owner), `apps/litecowork-ui/tests/conversation-artifacts.test.ts` (new)
- **Contract:** SPEC §5 LOCKED "Provenance links Conversation or Task"; AC-017, AC-018
- **Change intent:** Add "Save as Artifact" on committed Agent and user messages, creating a UserAuthored Markdown Artifact with `ConversationOrigin {conversation_id, message_id}`; add a "Files from this chat" list using `GET /v1/artifacts?conversation_id=`; show "From conversation" with a link in the Workbench provenance panel.
- **Acceptance:**
  - [ ] AC-017: origin is written in the create transaction; another Workspace's Conversation or an uncommitted message is refused.
  - [ ] AC-017: association survives restart and appears in both directions.
  - [ ] AC-018: the action is unavailable while the create command is not served.
  - [ ] Saving the same message twice with one idempotency key yields one Artifact.
- **Command:** `cargo test -p storage-sqlite --locked artifact_conversation_origin` and `pnpm --dir apps/litecowork-ui test` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** if 001 message ids are not stable across restart, stop and resolve with the 001 owner.

## 007-T09 - Library and Workbench create, Markdown edit and export UI

- **Status:** PARTIAL (`ArtifactLibrary.tsx` lists and opens; `ArtifactWorkbench.tsx` edits `text/plain`, compares, promotes, archives and Saves As; no create, no Markdown edit, no export receipt history)
- **Depends on:** T07
- **Cross-package prerequisites:** Feature 011 app-wide keyboard and focus baseline; Feature 016 rich presentation fallback.
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/artifacts/ArtifactLibrary.tsx`, `apps/litecowork-ui/src/artifacts/ArtifactWorkbench.tsx`, `apps/litecowork-ui/src/artifacts/MarkdownEditor.tsx` (new), `apps/litecowork-ui/src/artifacts/ArtifactExportHistory.tsx` (new), `apps/litecowork-ui/src/artifacts/StructuredTextPreview.tsx`, `apps/litecowork-ui/src/artifacts/artifact-access-state.ts`, `apps/litecowork-ui/src/artifacts/artifact-workbench.css`, `apps/litecowork-ui/tests/artifact-publication.test.ts`, `apps/litecowork-ui/tests/artifact-save-as.test.ts`, `apps/litecowork-ui/tests/artifact-capabilities.test.ts` (new)
- **Contract:** contracts/ui-behavior.md; PRESENTATION-RUNTIME; AC-004, AC-015, AC-016, AC-018, AC-005
- **Change intent:** Add "New Markdown document" in Library; replace the hardcoded `text/plain` gate with the capability matrix; add a Markdown source editor with preview, draft preservation on `STALE_VERSION` and explicit rebase; add Export with a receipt history list; keep Save As labelled "Save a local copy" and distinct from Export; loading, empty, error, denied, offline and stale states.
- **Acceptance:**
  - [ ] AC-018: version 1 appears only after the create receipt returns.
  - [ ] AC-016: controls follow the matrix for every media type in the fixture set.
  - [ ] AC-004: preview failure shows the raw Markdown source.
  - [ ] AC-015: keyboard order, accessible names and roles; read-only and diff state conveyed by text, not color.
  - [ ] AC-005: no Save is offered for a read-only type.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; UI ledgers updated in T16
- **Stop:** any control enabled without a matrix row halts the task.

## 007-T10 - DOCX, XLSX, PPTX and PDF read-only or unsupported rendering

- **Status:** PARTIAL (Library uploads keep these as Resources with preview/no-preview or Save original; there is no declared matrix and no explicit unsupported card in `ArtifactWorkbench.tsx`)
- **Depends on:** T05, T09
- **Cross-package prerequisites:** Feature 016 Interactive Artifact Runtime posture (no document script runs in the chat WebView).
- **Owner:** Desktop UI, Presentation, Security
- **Allowed paths:** `apps/litecowork-ui/src/artifacts/UnsupportedFormatCard.tsx` (new), `apps/litecowork-ui/src/artifacts/ArtifactWorkbench.tsx`, `apps/litecowork-ui/src/resources/TaskResourceTextPreview.tsx`, `apps/litecowork-ui/src-tauri/src/resource_save_bridge.rs` (labels only), `apps/litecowork-ui/tests/artifact-unsupported-formats.test.ts` (new)
- **Contract:** release contract §4 ("Do not present a PDF preview as full DOCX editing"); SPEC §5 matrix; AC-016, AC-005, AC-011
- **Change intent:** For DOCX, XLSX, PPTX and PDF render a metadata card (name, type, size, digest, revision), the matrix level as text ("Read-only" or "Not supported in this version") and "Save original" through `resource_save_as` or `artifact_save_as`. Decide PDF V1 posture: keep UNSUPPORTED unless a sandboxed renderer that runs no document script is qualified here, then READ_ONLY. Office probes in `environment_office_qualification.rs` are never shown as editing support.
- **Acceptance:**
  - [ ] AC-016: no edit, export or compare control renders for these four types.
  - [ ] AC-016: a PDF view, if enabled, is labelled read-only and never as DOCX editing.
  - [ ] AC-011: PDF and Office macros or scripts never execute.
  - [ ] AC-005: Save original never claims an Artifact save or export receipt.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; screenshot evidence per row in T15
- **Stop:** if a PDF renderer cannot be proven script-free, ship UNSUPPORTED.

## 007-T11 - Stored-version compare

- **Status:** PARTIAL (`TextVersionDiff.tsx` with `lineDiff` in `text-version-diff-model.ts`, bounded to 400 lines and 160,000 cells, comparing loaded stored versions)
- **Depends on:** T04
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Artifacts
- **Allowed paths:** `apps/litecowork-ui/src/artifacts/text-version-diff-model.ts`, `apps/litecowork-ui/src/artifacts/TextVersionDiff.tsx`, `apps/litecowork-ui/tests/text-version-diff.test.ts`, `apps/litecowork-ui/tests/artifact-comparison.test.ts`
- **Contract:** SPEC §15; AC-003, AC-014
- **Change intent:** Compare two committed Markdown versions fetched by exact version number, never the unsaved buffer. Replace the O(n x m) LCS table with a Myers O(ND) diff so Markdown documents beyond 400 lines up to the 1 MiB bound can be compared, or move compare into the daemon (see notes); keep the existing exact side-by-side fallback above the bound.
- **Acceptance:**
  - [ ] AC-003: diff inputs are stored versions; an unsaved edit does not change the diff.
  - [ ] AC-014: compare of two 1 MiB documents stays responsive or shows a deferred-load state.
  - [ ] Hostile input (one huge line, many small changes) cannot exhaust UI memory.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test names and timing in CURRENT-RUN.md
- **Stop:** a regression of the existing bounded fallback halts the task.

## 007-T12 - Imported and Task-generated source classes

- **Status:** BLOCKED for the Task-generated class (Feature 005 produces no Task output today; owner: Task); NOT STARTED for Imported
- **DEFERRED/BLOCKED note:** Why: the Task-generated class needs Feature 005 Task output, which does not exist; the Imported class is plain work. Unblocks when: Feature 005 Task execution producing output. Test later at: native Linux after Feature 005 qualifies, then the owner's native Windows x64 laptop (release matrix row 1); WSL2 and containers never count.
- **Depends on:** T02, T07
- **Cross-package prerequisites:** Feature 003 Resource admission (pinned revisions); Feature 005 Task output producer for the Task-generated class.
- **Owner:** Artifacts, Resource
- **Allowed paths:** `crates/storage-sqlite/src/artifacts.rs`, `apps/litecoworkd/src/artifact_create_operator.rs`, `apps/litecowork-ui/src/artifacts/ArtifactLibrary.tsx` ("Make editable copy" on an eligible Resource)
- **Contract:** SPEC §5 B-30 source classes; AC-009, AC-010
- **Change intent:** Imported: copy an admitted Resource revision (Markdown or plain text only, per matrix) into a new ARTIFACT Resource with an input edge to the source revision; never relabel. Task-generated: accept `TaskGenerated{task_id, attempt_id}` only when 005 presents a committed output.
- **Acceptance:**
  - [ ] AC-009: the source Resource is unchanged and still listed as a Resource.
  - [ ] AC-010: import is atomic and pins the source revision digest.
  - [ ] Import of an unsupported type is refused with `UNSUPPORTED_MEDIA_TYPE`.
- **Command:** `cargo test -p storage-sqlite --locked artifact_import` (filter to be added by this task)
- **Evidence:** test names in CURRENT-RUN.md
- **Stop:** do not accept a TaskGenerated create without a committed 005 output.

## 007-T13 - Failure, restart and reopen suite

- **Status:** NOT STARTED
- **Depends on:** T04, T06, T08
- **Cross-package prerequisites:** NONE
- **Owner:** Artifacts, Storage
- **Allowed paths:** `crates/storage-sqlite/src/artifact_recovery_tests.rs` (new), `apps/litecoworkd/src/runtime_recovery_tests.rs`, `apps/litecowork-ui/tests/artifact-request-epoch.test.ts`
- **Contract:** SPEC §11, §12; contracts/errors-recovery.md; AC-002, AC-008, AC-012, AC-014
- **Change intent:** Fault-inject create, publish and export at every step; restart the daemon and reopen; replay requests after restart; race archive against publish; cancel the export picker.
- **Acceptance:**
  - [ ] AC-002: reopen after restart loads the last committed bytes and provenance, including the Conversation origin.
  - [ ] AC-008: no crash point yields a partial version or receipt.
  - [ ] AC-012: replay after restart returns the pre-crash receipt.
  - [ ] AC-014: reopen of a 1 MiB document within the 500 ms local-store budget or a deferred-load state.
  - [ ] Archive racing publish: one wins; the other gets `ARTIFACT_ARCHIVED` or `STALE_VERSION`.
- **Command:** `cargo test -p storage-sqlite --locked artifact_recovery` and `cargo test -p litecoworkd --locked artifact_restart` (filter to be added by this task)
- **Evidence:** fault points and results in CURRENT-RUN.md
- **Stop:** any UNKNOWN reported as success halts qualification.

## 007-T14 - Cross-layer traces for every 007 user operation

- **Status:** NOT STARTED
- **Depends on:** T08, T09, T10
- **Cross-package prerequisites:** NONE
- **Owner:** Artifacts, Desktop UI
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `contracts/api-events.md`), `docs/SEMANTIC-SERVED-UI-MAP.md`
- **Contract:** AGENTS.md §4; AC-010, AC-006, AC-017
- **Change intent:** Write the trace (outcome, control, typed command, IPC/route, authorization, service, transaction, record/event/receipt, projection, restart, evidence) for New Markdown document, Save as Artifact, edit and publish, compare, export, Save a local copy, Save original, promote, archive.
- **Acceptance:**
  - [ ] Every control maps to a served route or a declared native-only operation.
  - [ ] Each trace cites its restart test.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/audit_served_routes.py`
- **Evidence:** trace tables in contracts/ui-behavior.md
- **Stop:** an untraced control blocks T15.

## 007-T15 - Qualify E2E-07 on the Windows and Linux release rows

- **Status:** NOT STARTED
- **Depends on:** T11, T12, T13, T14
- **Cross-package prerequisites:** Feature 011 packaged desktop and Windows IPC; Feature 001 qualified Conversation (for Save as Artifact); Feature 019 release-matrix rows and evidence bundle.
- **Owner:** Artifacts, Desktop UI, release owner
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md` (evidence rows only), `CURRENT-RUN.md`, this package
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-07; SPEC §15 all AC IDs; AC-016, AC-017, AC-019
- **Change intent:** Run E2E-07 (create Artifact, edit, version, export, reopen) in the packaged desktop on Windows x64 then Linux x64, including Save as Artifact from a Conversation, unsupported DOCX/PDF cards, export replay, crash during save and restart reopen. Collect CODE, SYSTEM and owner USER evidence per row.
- **Acceptance:**
  - [ ] Immutable versions, Markdown round-trip fidelity and provenance shown after restart.
  - [ ] AC-016 verified with real DOCX, XLSX, PPTX and PDF files.
  - [ ] Owner USER sign-off per row with build and OS recorded.
- **Command:** `cargo test -p litecoworkd --locked artifact` and `pnpm --dir apps/litecowork-ui test` and `python3 scripts/v1_platform_matrix.py`
- **Evidence:** evidence bundle path; CURRENT-RUN.md; IMPLEMENTATION-STATUS.md "Artifacts and intelligent presentation" row
- **Stop:** a failing denial or recovery case keeps that row unqualified.

## 007-T16 - Documentation and ledger sync

- **Status:** NOT STARTED
- **Depends on:** T15
- **Cross-package prerequisites:** orchestrator-owned register and traceability files.
- **Owner:** Artifacts (docs owner for this package)
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json` (regenerated), `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md` (B-21, B-30, B-31 rows closed), `apps/litecowork-ui/src/artifacts/README.md`, this package
- **Contract:** AGENTS.md §3, §4; AC-016, AC-018
- **Change intent:** Regenerate the served-route inventory, update the Artifacts capability rows (create, Markdown, export receipt, matrix, Conversation association), update UI ledgers and SPEC §4.
- **Acceptance:**
  - [ ] Inventory matches mounted routes.
  - [ ] No document claims a level above recorded evidence.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** inventory diff; validator PASS line
- **Stop:** an inventory mismatch blocks T17.

## 007-T17 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T16
- **Cross-package prerequisites:** Feature 019 release qualification sign-off process.
- **Owner:** Artifacts, Presentation, Storage, Desktop UI; repository owner sign-off
- **Allowed paths:** this package (status fields), `docs/IMPLEMENTATION-STATUS.md`
- **Contract:** SPECIFICATION-STANDARD lifecycle; SPEC §15 AC-001 to AC-019
- **Change intent:** Assemble the evidence bundle; move status ACCEPTED/LOCKED to IMPLEMENTING when T02 starts and to VERIFIED only after owner sign-off on both rows. The Task-generated class may remain BLOCKED on Feature 005 with an owner-recorded exception.
- **Acceptance:**
  - [ ] Every AC ID has passing evidence per row or a recorded owner exception.
  - [ ] Owner sign-off recorded with date and revision.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle; README and SPEC status lines; IMPLEMENTATION-STATUS.md
- **Stop:** missing denial or recovery evidence keeps the package IMPLEMENTING.

## Dependency order

T01 -> (T02, T03, T05); (T02, T03) -> T04; (T01, T03, T05) -> T06; (T02, T04, T05, T06) -> T07; (T02, T07) -> T08; T07 -> T09; (T05, T09) -> T10; T04 -> T11; (T02, T07) -> T12; (T04, T06, T08) -> T13; (T08, T09, T10) -> T14; (T11, T12, T13, T14) -> T15 -> T16 -> T17. T05 (matrix) can ship before any new write path so the UI stops guessing capabilities early.

## Cross-package dependencies

Other packages renumber their tasks in the same authoring wave, so prerequisites are named by package and gap item; the orchestrator maps them to task IDs.

| This task | Other package task | Reason |
|---|---|---|
| T01, T08 | Feature 001 committed message identity and projection | `ConversationOrigin` references a committed message id |
| T07, T15 | Feature 011 Windows named-pipe Operator IPC; packaged desktop | Windows row needs authenticated IPC |
| T09 | Feature 011 app-wide keyboard and focus baseline | Shared focus model |
| T09, T10 | Feature 016 rich presentation and Interactive Artifact Runtime posture | Fallback and no-script rendering |
| T12 | Feature 003 Resource admission; Feature 005 Task output producer | Imported and Task-generated source classes |
| T15, T17 | Feature 019 release-matrix rows and evidence bundle | E2E-07 qualification per row |
| (consumer) | Feature 014 Conversation delete fences | Uses the Conversation origin from T08 |

## Implementation-language and optimization notes

- Put the B-21 Markdown validator and canonical serializer in Rust in the daemon (`artifact_format.rs`), so one engine decides validity for publish and export. The UI keeps only the render-only `StructuredTextPreview.tsx`; T03 and T09 share a fixture set so the two dialects do not drift.
- Keep the capability matrix in Rust (T05) and serve it; today the UI hardcodes `text/plain` in `ArtifactWorkbench.tsx` and `supportsTextPreview` in `artifact-api.ts`, which is two sources of truth.
- `lineDiff` in `text-version-diff-model.ts` builds an O(n x m) `Uint16Array` table and gives up above 400 lines. A Myers O(ND) diff in TS, or a daemon-side diff in Rust (for example the `similar` crate, to be pinned), would cover the 1 MiB bound with less memory. A daemon-side diff also guarantees AC-003 (stored versions only).
- Export bytes stay in Rust between the daemon and the Tauri process; only status and receipt ids cross WebView IPC, matching the existing `artifact_save_as` pattern.
- New create and export handlers go in `artifact_create_operator.rs` rather than growing `artifact_operator.rs`.

## Completion evidence

For each task record in CURRENT-RUN.md and the evidence bundle: git revision, exact command, pass/fail result and test names, OS and build (for example Windows 11 23H2 x64, Ubuntu 24.04 x64), Markdown engine id and version, harness and version (OpenCode v2.0.32) where Save as Artifact uses a Conversation, reviewer, and the AC IDs covered. A task is DONE only with that record; a passing structural validator is not evidence.
