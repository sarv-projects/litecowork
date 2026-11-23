# Tasks - Conversation History Management

**Feature:** 014 | **Status:** IN_REVIEW - not owner-accepted; T01 blocks every task; tasks not started unless marked.
**Spec revision:** 0.1 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** domain owners named in SPEC.md. Blocker B-38 accepted as proposed 2026-11-10; whole-SPEC acceptance pending.

## Current baseline

Source cross-check completed 2026-11-12 at HEAD `64c4351` plus the shared worktree. The repository is on `main`; branch and migration facts below reflect this checkout, not the superseded 2026-11-09 baseline.

- **Operator:** `apps/litecoworkd/src/conversation_operator.rs` `routes_with_state` mounts create/list/get, turn submit/retry/cancel, agent-binding change, rename, archive, unarchive and a status filter. Search, deletion impact, delete and deletion-status routes are absent. Default list is ACTIVE and accepts ARCHIVED; the existing cursor remains newest-first by `(created_at, conversation_id)`, limit 1-100. Test `conversation_operator_cursor_paginates_more_than_fifty_and_fences_workspace_and_owner` plus history route tests in `conversation_operator_tests.rs`.
- **Storage:** `ConversationRecord` still has no status fields. Migration v31 (`conversation_history_state`) and `ConversationHistoryStore` implement version-fenced rename/archive/unarchive/status listing; the migration chain is wired through v32. Search index, tombstone and purge records are absent. `conversation_messages.content_json` is plaintext; `conversation.message.added.v1` carries `content_digest`; `conversation.created.v1` carries no title. Precedents: ContextDocument purge and Resource HMAC term index.
- **Tauri:** `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs` exposes list, create, get, set binding, turn commands, presentation; none for history management.
- **UI:** sidebar "Recent Chats" in `apps/litecowork-ui/src/App.tsx` maps loaded conversations with `conv.title || "Untitled"` and still shows literal "now" on the selected row; no history search, row actions or virtualization. `ConversationsPage.tsx` now uses `conversationTitle` (`Untitled conversation` when empty) and renders More options disabled with an unavailable label. The new compact navigation does not implement Conversation history actions.
- **Error registry:** `docs/schemas/error-codes.schema.json` has `STALE_VERSION`, `IDEMPOTENCY_CONFLICT`, `INVALID_ARGUMENT`, `FORBIDDEN`, `NOT_FOUND`, `WORKSPACE_ARCHIVED`; no Conversation archive/delete code.

Gaps to V1: every goal in SPEC section 2, plus qualification on both rows.

## 014-T01 - Owner acceptance of SPEC and blockers

- **Status:** PARTIAL - B-38 accepted as proposed (2026-11-10, sarv-projects) and treated as unblocking implementation by owner instruction; WIRING YAML syntax was repaired on 2026-11-19 and structural validation passes. The whole-SPEC ACCEPTED/LOCKED flip and O-02, O-03, O-06 and O-07 remain pending, so SPEC and WIRING statuses are unchanged and this task is not DONE.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE (orchestrator records B-38 in the register and review packet).
- **Owner:** Product (repository owner); Conversation and Privacy reviewers.
- **Allowed paths:** this package; `docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md` and the register rows (orchestrator only).
- **Contract:** SPEC sections 2, 3, 5, 17; AC-009, AC-020.
- **Change intent:** owner accepts or amends B-38 (tombstone with immediate fenced purge, or the 30-day restore-window alternative) and resolves O-02, O-03, O-06 and O-07. On acceptance set SPEC `**Status:** ACCEPTED/LOCKED`, revision 1.0, and WIRING `status: "ACCEPTED/LOCKED"`.
- **Acceptance:**
  - [x] B-38 recorded ACCEPTED as proposed (sarv-projects, 2026-11-10).
  - [ ] SPEC and WIRING statuses match; validator passes.
  - [ ] Negative: until this closes, no history-management control is enabled (AC-020).
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** owner acceptance line in SPEC metadata; register row B-38.
- **Stop:** owner rejects delete in V1; delete verbs stay unavailable and the package scope narrows.

## 014-T02 - Promote accepted interfaces into owner docs and schemas

- **Status:** PARTIAL - `docs/schemas/sqlite-v31.sql` (side table `conversation_history_state`, deviation from the column-on-`conversations` wording, see docs/STORAGE.md) is wired and migration tests pass; STORAGE, EVENTS and BUILD-TASKS version text synced. The source-emitted `conversation.renamed.v1`, `conversation.archived.v1` and `conversation.unarchived.v1` types and payloads are now registered in EVENTS/schema under user-directed implementation review (2026-11-20). NOT done: OpenAPI route contracts, `error-codes.schema.json` entries for `CONVERSATION_ARCHIVED`/`CONVERSATION_DELETED`/`CONVERSATION_NOT_QUIESCENT`, STATE-MACHINES lifecycle section, deletion events, search index and purge tables (later migrations).
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 001 contract promotion of `CONVERSATION_TURN_IN_PROGRESS` (001 T01); Feature 017 review of tombstone and manifest backup inclusion.
- **Owner:** API, State Machines, Events, Storage owners.
- **Allowed paths:** `docs/API.md`, `docs/STATE-MACHINES.md`, `docs/EVENTS.md`, `docs/STORAGE.md`, `docs/DATA-MODEL.md`, `docs/SCHEMAS.md`, `docs/DESKTOP-SHELL-LLD.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, a forward-only migration using the then-current number allocated in `docs/LITECOWORK-BUILD-TASKS.md`, this package.
- **Contract:** SPEC sections 7.2, 8, 12; AC-001, AC-004, AC-009.
- **Change intent:** add a Conversation lifecycle section to STATE-MACHINES (`ACTIVE <-> ARCHIVED`, `-> DELETION_PENDING -> DELETED`); promote the eight routes, error codes `CONVERSATION_ARCHIVED`, `CONVERSATION_DELETED`, `CONVERSATION_NOT_QUIESCENT`, the `index_state` field, events `conversation.renamed.v1`, `conversation.archived.v1`, `conversation.unarchived.v1`, `conversation.deletion_requested.v1`, `conversation.deleted.v1`; use the v31 side-table model for lifecycle state (do not add status columns to `conversations`); a later allocated forward-only migration adds the list index, search index and purge manifest/receipt tables. Each interface remains accepted-target/not-yet-in-source until its implementation lands.
- **Acceptance:**
  - [ ] Each name defined once in its owner doc and machine schema.
  - [ ] Migration is the next immutable `sqlite-vNN.sql`; earlier migrations byte-identical; existing rows migrate to `ACTIVE`.
  - [ ] `conversation.renamed.v1` schema carries `title_digest` and rejects a `title` field.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py` and `cargo test -p storage-sqlite --locked schema`
- **Evidence:** PR link and command output in Completion evidence.
- **User-directed clarification (2026-11-20; not formal domain-owner acceptance):** the current storage transactions already emit the rename/archive/unarchive event types, so their closed payload contracts are promoted here. Unknown versioned types remain accepted by the generic envelope only for opaque forward-compatibility; local event creation must use a registered type. This does not mark Feature 014 ACCEPTED/LOCKED or enable delete/search behavior.
- **Evidence (2026-11-20, contract correction only):** the three current source event names and their exact `workspace_id`, `conversation_id`, `changed_by`, `aggregate_version`, and rename-only `title_digest` payloads are now present in the canonical registry and schema. Architecture/schema validation remains to be run; unrelated lifecycle/API/deletion promotion remains open.
- **Stop:** a name collides with an owner definition; resolve at the owner.

## 014-T03 - Storage: Conversation status, rename and archive with CAS and idempotency

- **Status:** PARTIAL - storage layer done (2026-11-10): `ConversationHistoryStore` (`rename_conversation`, `set_conversation_archived`, `get_conversation_status`, `list_conversations_by_status`) in `crates/storage-sqlite/src/conversations.rs`; CAS, request-dedup replay and changed-payload conflict, rename event carries `title_digest` only, archive refused with `CONVERSATION_NOT_QUIESCENT` while a turn is nonterminal, `create_conversation_turn` and the AgentBinding change reject a non-ACTIVE Conversation inside their own transactions. Tests `conversation_history_*` (5) pass. NOT done: `ConversationRecord` was not extended (status lives in the side table); event and error-code schema promotion (T02). The stale claim that the existing `list_conversations` returns archived rows to served callers was RESOLVED 2026-11-10 by T07: the Operator's `GET /v1/conversations` now delegates to `list_conversations_by_status`, so the served default page no longer exposes ARCHIVED rows. The desktop bridge's own `list_conversations` command (Feature 011) and the trait default are unchanged and still return every status. Operator routes are T07 and are now PARTIAL: rename, archive, unarchive and the status filter are served.
- **Depends on:** T01, T02
- **Cross-package prerequisites:** Feature 001 storage single-flight (001 T11) so "no nonterminal turn" is checkable in one query.
- **Owner:** Storage, Conversation.
- **Allowed paths:** `crates/storage-core/src/conversation.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/conversation_history_tests.rs` (new).
- **Contract:** SPEC sections 7.2, 8; AC-001 to AC-005, AC-015.
- **Change intent:** add `ConversationStatus`, extend `ConversationRecord` with `status`, `archived_at`, `deleted_at`; add `ConversationHistoryStore::rename_conversation` and `set_conversation_archived` as IMMEDIATE transactions that check owner, Workspace ACTIVE, Conversation status and `expected_version`, then write row, event (title digest only), snapshot and receipt; archive also checks no nonterminal turn. `list_conversations` gains a status filter (default ACTIVE). Every existing write path that admits a turn or changes the binding rejects non-ACTIVE status inside its own transaction.
- **Acceptance:**
  - [ ] Rename commits title, version, event and receipt atomically; event has no title (AC-001).
  - [ ] Stale version, invalid title, foreign owner, archived Workspace write nothing (AC-002).
  - [ ] Exact replay returns stored result; changed payload is `IDEMPOTENCY_CONFLICT` (AC-003).
  - [ ] Archive refused while a turn is nonterminal; archived refuses submit and binding change (AC-004).
  - [ ] Unarchive restores ACTIVE (AC-005).
  - [ ] Two renames from one version: one wins, one `STALE_VERSION` (AC-015).
- **Command:** `cargo test -p storage-sqlite --locked conversation_history` (filter to be added by this task)
- **Evidence:** test output.
- **Stop:** a status check would require an external call inside a transaction.

## 014-T04 - Storage: search index, in-transaction maintenance and resumable backfill

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01). PARTIAL precedent: `resource_index.rs` tokenizer and HMAC term tokens.
- **Depends on:** T01, T02, T03
- **Cross-package prerequisites:** Feature 001 owners review the index hook added to `create_conversation_turn` and `commit_conversation_agent_response` (same file, 001 T02/T11 edit it).
- **Owner:** Storage.
- **Allowed paths:** `crates/storage-sqlite/src/conversation_search.rs` (new), `crates/storage-sqlite/src/conversations.rs` (index hook calls only), `crates/storage-sqlite/src/lib.rs`, `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversation_search_tests.rs` (new).
- **Contract:** SPEC F-01, F-02, sections 7.2, 10; AC-006, AC-007, AC-008.
- **Change intent:** implement `search_conversations` over title and committed USER/AGENT message text using the F-01 option accepted at T01 (recommended: HMAC term tokens per Workspace key, shared tokenizer extracted from `resource_index.rs`); index rows written in the same transaction as the message insert; a backfill worker with a per-Workspace watermark sets `index_state` `BUILDING` until done; snippets read from `conversation_messages` at query time under the same status check.
- **Acceptance:**
  - [ ] Title and message matches returned, scoped by owner and Workspace, with bounded snippet and stable cursor (AC-006).
  - [ ] Notices, drafts, rich documents, other Workspaces and deleted Conversations excluded; archived only on request (AC-007).
  - [ ] Rolled-back message never searchable; committed message searchable at commit; backfill reports BUILDING (AC-008).
  - [ ] Query over 256 characters or 32 terms is `INVALID_ARGUMENT`.
- **Command:** `cargo test -p storage-sqlite --locked conversation_search` (filter to be added by this task)
- **Evidence:** test output; chosen F-01 option recorded in DATA-MODEL.
- **Stop:** the index would store recoverable plaintext that purge cannot remove.

## 014-T05 - Storage: delete tombstone, sealed purge manifest and read fences

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01; B-38). PARTIAL precedent: `context_document_purge.rs` `begin_context_document_purge`.
- **Depends on:** T01, T03, T04
- **Cross-package prerequisites:** Feature 001 context loader and presentation readers accept the status check; Agent Runtime proves every Conversation AgentSession CLOSED/LOST and its Runtime-local host binding released before the tombstone; Feature 006 provides the unsettled Conversation-scoped invocation/grant/activation query.
- **Owner:** Storage, Privacy.
- **Allowed paths:** `crates/storage-sqlite/src/conversation_purge.rs` (new), `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/rich_presentations.rs` (status check only), `crates/storage-sqlite/src/lib.rs`, `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversation_purge_tests.rs` (new).
- **Contract:** SPEC B-38, sections 7.2, 8, 10; AC-009, AC-010, AC-015.
- **Change intent:** `request_conversation_deletion` checks quiescence and version, then in one IMMEDIATE transaction sets `DELETION_PENDING`, clears the title, seals the manifest (targets listed in B-38 item 3, bounded like the ContextDocument manifest) and writes `conversation.deletion_requested.v1` and the receipt; `conversation_deletion_impact` counts linked Tasks, Artifacts and memory sources; every reader (`get_conversation`, `list_conversations`, messages, presentation, `rich_presentations`, `load_conversation_context_for_turn`, search) checks status inside its own transaction.
- **Acceptance:**
  - [ ] Delete with an active turn, unsettled scoped invocation, or active/unknown AgentSession host-binding cleanup writes nothing (AC-009).
  - [ ] After the tombstone every reader returns `CONVERSATION_DELETED` (AC-010).
  - [ ] A context load started before the tombstone fails its final status check (AC-010).
  - [ ] Submit racing delete: exactly one commits (AC-015).
- **Command:** `cargo test -p storage-sqlite --locked conversation_purge` (filter to be added by this task)
- **Evidence:** test output.
- **Stop:** a reader exists that cannot be fenced inside its own transaction; list it and block delete.

## 014-T06 - Conversation purge reconciler and startup resume

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T05
- **Cross-package prerequisites:** Feature 011 Runtime startup recovery hook (`recover_and_persist_startup_blockers` in `apps/litecoworkd/src/runtime.rs`).
- **Owner:** Storage, Runtime.
- **Allowed paths:** `crates/storage-sqlite/src/conversation_purge.rs`, `apps/litecoworkd/src/conversation_purge_worker.rs` (new), `apps/litecoworkd/src/runtime.rs` (one call to start the worker), adjacent tests.
- **Contract:** SPEC B-38 item 4; AC-011, AC-013.
- **Change intent:** a bounded worker loops `list_pending_conversation_purge_targets` -> remove row or blob outside a transaction -> `acknowledge_conversation_purge_target` only after physical removal -> `complete_conversation_deletion_if_all_acknowledged`; idempotent per target; started at Runtime startup; FTS segment or index pages removed per F-01 choice.
- **Acceptance:**
  - [ ] Each target receipt only after removal; failures stay pending and retry (AC-011).
  - [ ] Restart mid-purge resumes; DELETED only after all receipts (AC-011).
  - [ ] Canary title and message strings absent from DB file, WAL, index and blob store after DELETED (AC-013).
- **Command:** `cargo test -p litecoworkd --locked conversation_purge_worker` (filter to be added by this task)
- **Evidence:** test output including canary scan.
- **Stop:** a target cannot be physically removed (for example WAL retention); fix checkpointing before DELETED.

## 014-T07 - Operator routes: rename, archive, unarchive and status-aware existing routes

- **Status:** PARTIAL - routes done (2026-11-10). Shipped in `apps/litecoworkd/src/conversation_operator.rs`: `PATCH /v1/conversations/{conversation_id}` (`rename_conversation`), `POST .../archive` and `POST .../unarchive` (`archive_conversation`/`unarchive_conversation` through one `set_conversation_archived_state` helper), each requiring `If-Match` and `Idempotency-Key` and returning `200 Conversation` with `x-correlation-id`; `status` (`ACTIVE` default, `ARCHIVED`) added to the `list_conversations` query and delegated to `list_conversations_by_status`. A dedicated `map_history_store_error` returns the accepted 014 codes (`STALE_VERSION` 412, `IDEMPOTENCY_CONFLICT` 409, `CONVERSATION_ARCHIVED` 409, `CONVERSATION_DELETED` 410, `WORKSPACE_ARCHIVED` 422) and maps the storage's `CONVERSATION_NOT_QUIESCENT` to `CONVERSATION_TURN_IN_PROGRESS`; `CONVERSATION_ARCHIVED`/`CONVERSATION_DELETED` are also mapped on the shared mapper the pre-existing submit, retry and binding-change routes use. Tests: `conversation_rename_route_commits_a_digest_only_event_and_fences_version_and_key` and `conversation_archive_routes_gate_on_turn_quiescence_and_filter_the_history_list` in `apps/litecoworkd/src/conversation_operator_tests.rs` (the task named a new `conversation_history_operator_tests.rs`; the existing operator test module was used instead so the routes share its fixtures). DEVIATIONS to review: a foreign Workspace answers `404 NOT_FOUND` (the storage ownership check the sibling Conversation routes already use), not the `FORBIDDEN` this task text names; `{"title": null}` is accepted by the contract shape and refused with `INVALID_ARGUMENT` because a Conversation title is never empty. REMAINING: nothing else in this task's change intent; the `CONVERSATION_TURN_IN_PROGRESS` search is left to T08. The canonical `conversation_history_operator` filter named below was not created - the tests run under the existing `conversation` filter.
- **Depends on:** T01, T03
- **Cross-package prerequisites:** NONE
- **Owner:** Operator, Conversation.
- **Allowed paths:** `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/conversation_history_operator_tests.rs` (new).
- **Contract:** SPEC section 7.2; AC-001 to AC-005.
- **Change intent:** add `PATCH /v1/conversations/{conversation_id}` (`rename_conversation` handler), `POST .../archive`, `POST .../unarchive` requiring If-Match and Idempotency-Key, reusing `idempotency_key`, `selected_workspace` and `ensure_workspace_owner_for`; add `status` query to `list_conversations`; map `CONVERSATION_ARCHIVED` from storage on submit, retry and binding change.
- **Acceptance:**
  - [x] Each route returns the committed record with the new version (AC-001, AC-004, AC-005).
  - [x] Missing If-Match or Idempotency-Key is `INVALID_ARGUMENT` (AC-002); a foreign Workspace is `NOT_FOUND`, following the sibling Conversation routes rather than the `FORBIDDEN` named here (AC-002, deviation recorded above).
  - [x] Replay returns stored response (AC-003).
- **Command:** `cargo test -p litecoworkd --locked conversation` (T07 shipped its routes into the existing operator test module, so the route tests run under the existing `conversation` filter; the `conversation_history_operator` filter named in the original task text was not created)
- **Evidence:** test output.
- **Stop:** NONE beyond test failure.

## 014-T08 - Operator route: Conversation search

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T04, T07
- **Cross-package prerequisites:** NONE
- **Owner:** Operator.
- **Allowed paths:** `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/conversation_history_operator_tests.rs`.
- **Contract:** SPEC sections 7.2, 10; AC-006, AC-007, AC-008.
- **Change intent:** add `GET /v1/conversations/search` with query, scope, Workspace-bound cursor (same encoding as `encode_cursor`/`decode_cursor`) and limit 1-50; run the store call on `spawn_blocking`; return `index_state`; never log query text.
- **Acceptance:**
  - [ ] Bounds enforced; cursor bound to Workspace (AC-006).
  - [ ] Exclusions hold at the route level (AC-007).
  - [ ] `index_state=BUILDING` returned during backfill (AC-008).
- **Command:** `cargo test -p litecoworkd --locked conversation_search_route` (filter to be added by this task)
- **Evidence:** test output.
- **Stop:** NONE beyond test failure.

## 014-T09 - Operator routes: deletion impact, delete and deletion status

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T05, T06, T07
- **Cross-package prerequisites:** NONE
- **Owner:** Operator, Privacy.
- **Allowed paths:** `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/conversation_history_operator_tests.rs`.
- **Contract:** SPEC B-38, section 7.2; AC-009, AC-012.
- **Change intent:** add `GET .../deletion-impact`, `DELETE /v1/conversations/{conversation_id}` (If-Match, Idempotency-Key, 202) and `GET .../deletion`; every other Conversation route maps `CONVERSATION_DELETED` to 410.
- **Acceptance:**
  - [ ] Impact lists linked Task, Artifact and memory-source counts and blockers (AC-012).
  - [ ] Delete returns 202 `DELETION_PENDING`; blocked delete writes nothing (AC-009).
  - [ ] Linked Tasks remain readable with a deleted-origin marker (AC-012).
- **Command:** `cargo test -p litecoworkd --locked conversation_delete_route` (filter to be added by this task)
- **Evidence:** test output.
- **Stop:** NONE beyond test failure.

## 014-T10 - Tauri bridge commands for history management

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T07, T08, T09
- **Cross-package prerequisites:** Feature 011 packaged sidecar and Windows named-pipe IPC.
- **Owner:** Desktop bridge.
- **Allowed paths:** `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs` (command registration only).
- **Contract:** SPEC section 7.2; AC-003, AC-014.
- **Change intent:** add `search_conversations`, `rename_conversation`, `archive_conversation`, `unarchive_conversation`, `get_conversation_deletion_impact`, `delete_conversation`, `get_conversation_deletion_status`; send If-Match from `expected_version`; one idempotency key per user intent; bound query length; no logging of query or title.
- **Acceptance:**
  - [ ] Inputs bounded and path components encoded.
  - [ ] Replay of an interrupted command reuses its key (AC-003).
  - [ ] No title or query text in bridge logs (AC-014).
- **Command:** `cargo test -p litecowork-ui --locked conversation_bridge` (filter to be added by this task)
- **Evidence:** test output.
- **Stop:** NONE beyond test failure.

## 014-T11 - Sidebar search UI with accessible states

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T10
- **Cross-package prerequisites:** Feature 011 app-wide keyboard and focus baseline and sidebar shell ownership.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/App.tsx` (replace the Recent Chats block with the component), `apps/litecowork-ui/src/conversations/ConversationHistoryList.tsx` (new), `apps/litecowork-ui/src/conversations/conversation-history-state.ts` (new), `apps/litecowork-ui/src/conversations/conversations-page.css`, `apps/litecowork-ui/src/styles.css`, `apps/litecowork-ui/tests/conversation-history-state.test.ts` (new).
- **Contract:** SPEC sections 6, 14; contracts/ui-behavior.md; AC-016, AC-020.
- **Change intent:** labelled search field above Recent Chats; 250 ms debounce; `AbortController` per request (pattern from `DiscoverPage.tsx`); pure reducer states `IDLE`, `LOADING`, `RESULTS`, `NO_RESULTS`, `INDEX_BUILDING`, `ERROR`, `OFFLINE`, `DENIED`; results in a polite live region; Active/Archived filter; virtualized or count-capped list; remove the literal "now" text and show persisted time.
- **Acceptance:**
  - [ ] Each state distinct and announced; stale responses ignored (AC-016).
  - [ ] Field reachable by keyboard with a visible label; Escape clears.
  - [ ] Unserved route renders unavailable state, not an inert field (AC-020).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test output; UI ledger row updated in T18.
- **Stop:** control enabled without an owner route.

## 014-T12 - Row menu and header actions: rename, archive, unarchive, delete

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T10, T11
- **Cross-package prerequisites:** Feature 001 composer and header truthfulness work (001 T18) touches the same header; sequence edits.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/ConversationHistoryList.tsx`, `apps/litecowork-ui/src/conversations/ConversationDeleteDialog.tsx` (new), `apps/litecowork-ui/src/conversations/conversation-history-state.ts`, `apps/litecowork-ui/tests/conversation-history-actions.test.ts` (new).
- **Contract:** SPEC sections 6, 14; contracts/ui-behavior.md; AC-017, AC-020.
- **Change intent:** row menu and header "More options" open Rename (inline field, Enter commits, Escape cancels), Archive/Unarchive and Delete; Delete dialog shows impact counts, says deletion removes LiteCowork-owned local records but not native-harness or external-channel copies, states older backups are unchanged, traps focus, Escape cancels and restores focus; controls disable with a reason while a turn or session cleanup is active; the fabricated header fallback has already been replaced by `conversationTitle` / "Untitled conversation"; open views of a deleted Conversation show a deleted state.
- **Acceptance:**
  - [ ] Keyboard-only rename, archive and delete paths work; focus restored (AC-017).
  - [ ] Delete requires dialog confirmation and shows impact (AC-017).
  - [ ] No fabricated title text remains (AC-020).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test output.
- **Stop:** NONE beyond test failure.

## 014-T13 - Cross-package delete fences (retrieval, context, memory, artifacts, logs)

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T05, T09
- **Cross-package prerequisites:** Feature 003 retrieval/context readers, Feature 007 Artifact association, Feature 009 Coworker memory sources, Feature 018 diagnostics bundle, Feature 005/007 Evidence redaction decision (O-02).
- **Owner:** Conversation with each consuming owner.
- **Allowed paths:** this package; consuming owners change their own readers under their own tasks; `apps/litecoworkd/tests/conversation_history_management.rs` (fence assertions).
- **Contract:** release contract section 6 invariant 6; SPEC G-05; AC-010, AC-012, AC-014.
- **Change intent:** publish a single storage predicate `conversation_is_readable(workspace_id, conversation_id)` and require each consuming reader to call it inside its transaction; add end-to-end assertions that context packaging, retrieval sources, memory candidates and Artifact association reject a deleted Conversation; add a log-capture canary test covering the daemon and bridge.
- **Acceptance:**
  - [ ] Each consuming reader listed with its fence test name (AC-010).
  - [ ] Linked Artifacts and Tasks remain with deleted-origin markers (AC-012).
  - [ ] Canary absent from captured logs and diagnostics (AC-014).
- **Command:** `cargo test -p litecoworkd --test conversation_history_management --locked -- --nocapture`
- **Evidence:** fence table in contracts/errors-recovery.md.
- **Stop:** a consuming owner cannot fence; delete stays disabled until it can.

## 014-T14 - Failure, race and canary matrix; create the canonical test target

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T06-T09
- **Cross-package prerequisites:** NONE
- **Owner:** Conversation, Storage, Testing.
- **Allowed paths:** `apps/litecoworkd/tests/conversation_history_management.rs` (new).
- **Contract:** SPEC section 11; contracts/errors-recovery.md; AC-011, AC-013, AC-014, AC-015.
- **Change intent:** daemon + SQLite + blob store integration: kill after tombstone commit, mid-purge and before completion; restart resumes; concurrent rename/rename, submit/archive, submit/delete; replay after crash; canary scans of DB file, WAL, index and blobs.
- **Acceptance:**
  - [ ] Every crash point ends in a consistent state with no readable content after tombstone (AC-011, AC-013).
  - [ ] Races commit exactly one mutation (AC-015).
  - [ ] Logs free of canary (AC-014).
- **Command:** `cargo test -p litecoworkd --test conversation_history_management --locked -- --nocapture`
- **Evidence:** test output in Completion evidence.
- **Stop:** any state exposes content after the tombstone.

## 014-T15 - Performance budget and list virtualization

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T04, T11
- **Cross-package prerequisites:** Feature 019 soak harness (`scripts/v1_soak_harness.py`) and release-row hardware definition.
- **Owner:** Storage, Desktop UI.
- **Allowed paths:** `crates/storage-sqlite/src/conversation_search_tests.rs`, `apps/litecowork-ui/tests/conversation-history-state.test.ts`, this package.
- **Contract:** SPEC section 12; AC-018.
- **Change intent:** seeded benchmark (10,000 Conversations, 200,000 messages) for search and list; measure p95 on each release row; confirm or revise the proposed budget through owner review.
- **Acceptance:**
  - [ ] Search page p95 at or under budget on row 1 and row 2 (AC-018).
  - [ ] Sidebar renders a bounded number of rows regardless of total (AC-018).
- **Command:** `cargo test -p storage-sqlite --locked conversation_search_benchmark -- --ignored` (filter to be added by this task) and `python3 scripts/v1_soak_harness.py`
- **Evidence:** measurements per row.
- **Stop:** budget missed; record and return to owner before VERIFIED.

## 014-T16 - Cross-layer trace for each user-visible operation (AGENTS.md section 4)

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T12, T14
- **Cross-package prerequisites:** NONE
- **Owner:** Conversation, Desktop UI.
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `WIRING.yaml`).
- **Contract:** AGENTS.md section 4; AC-001, AC-006, AC-009.
- **Change intent:** record control -> Tauri command -> route -> authorization -> service -> transaction/fence -> event/receipt -> projection -> restart -> test for Search, Rename, Archive, Unarchive and Delete.
- **Acceptance:**
  - [ ] Each operation has a complete row with real test names.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** trace table in contracts/ui-behavior.md.
- **Stop:** an operation has no owner route; keep its control disabled.

## 014-T17 - Qualification on the release rows

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T11-T16
- **Cross-package prerequisites:** Feature 019 release matrix rows and evidence bundle; Feature 017 backup/restore for AC-019; Feature 001 Send qualified on the row (to create real history).
- **Owner:** Product, Testing.
- **Allowed paths:** this package; evidence report path chosen by Feature 019.
- **Contract:** ACCEPTANCE-AND-E2E-PLAN E2E-01 (reload of committed history) and E2E-12 (tombstone preservation); AC-016, AC-017, AC-019.
- **Change intent:** packaged desktop on Windows 11/10 x64 + OpenCode v2.0.32, then Linux x64: create real Conversations, search, rename, archive, unarchive, delete, kill the daemon mid-purge, restart, back up during DELETION_PENDING and restore; denial cases (active turn, stale version, foreign Workspace). Ask the orchestrator to add a dedicated E2E id (O-04).
- **Acceptance:**
  - [ ] CODE, SYSTEM and owner USER evidence per row, including denial and recovery.
  - [ ] Restore never makes deleted content readable (AC-019).
  - [ ] Keyboard and screen-reader pass on both rows (AC-016, AC-017).
- **Command:** `cargo test -p litecoworkd --test conversation_history_management --locked -- --nocapture` and `pnpm --dir apps/litecowork-ui test` and `python3 scripts/v1_platform_matrix.py`
- **Evidence:** Feature 019 evidence bundle; IMPLEMENTATION-STATUS.md rows.
- **Stop:** USER evidence missing on a row.

## 014-T18 - Documentation sync

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01, T17
- **Cross-package prerequisites:** orchestrator owns register, specs/README.md and traceability index.
- **Owner:** Documentation owners.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/API.md` (status lines), this package.
- **Contract:** AGENTS.md sections 4-5; all AC IDs.
- **Change intent:** update Conversation status rows; regenerate the served-route inventory; update sidebar and header UI ledger rows; close B-38 register row (orchestrator).
- **Acceptance:**
  - [ ] Served-route inventory lists the new routes from source.
  - [ ] Status text never exceeds recorded evidence.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff and command output.
- **Stop:** a claim lacks evidence.

## 014-T19 - Package VERIFIED for V1

- **Status:** NOT STARTED (unblocked by B-38 per owner instruction 2026-11-10; not reached in this slice; formerly BLOCKED on owner acceptance, T01).
- **Depends on:** T01-T18
- **Cross-package prerequisites:** Feature 019 release sign-off process.
- **Owner:** Product (repository owner).
- **Allowed paths:** this package; `docs/IMPLEMENTATION-STATUS.md`.
- **Contract:** SPEC sections 14-15; AC-001 to AC-020.
- **Change intent:** assemble the evidence bundle; move IN_REVIEW -> ACCEPTED/LOCKED (T01) -> IMPLEMENTING -> VERIFIED after owner sign-off on both rows.
- **Acceptance:**
  - [ ] Canonical command and UI suite pass on both rows.
  - [ ] Owner USER sign-off per row including denial and recovery.
  - [ ] SPEC and WIRING VERIFIED; validator passes.
- **Command:** `cargo test -p litecoworkd --test conversation_history_management --locked -- --nocapture` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle path referenced from IMPLEMENTATION-STATUS.md.
- **Stop:** any row lacks USER evidence.

## Implementation and optimization notes (Rust and TypeScript)

Findings from reading the source. They are folded into the tasks above and add no scope.

- **Search index choice (Rust/SQLite, T04):** Conversation text is already plaintext in `conversation_messages`, so a plaintext FTS5 table adds a second copy that purge must also clean (FTS5 keeps deleted terms in segments unless `secure-delete` is on). Reusing the HMAC term-token design from `resource_index.rs` makes purge a plain row delete and shares one tokenizer with Resource search. FTS5 is only an option after confirming the bundled `libsqlite3-sys 0.38.2` build flags.
- **Index in the message transaction (Rust, T04):** writing index rows inside `create_conversation_turn` and `commit_conversation_agent_response` costs one bounded insert batch and removes the "committed but not yet indexed" window, so the UI only needs `BUILDING` during backfill.
- **List index (SQLite, T02/T03):** `list_conversations` sorts by `(created_at, conversation_id)` with a `workspace_id` filter and only the UNIQUE `(workspace_id, conversation_id)` constraint; adding `(workspace_id, status, created_at DESC, conversation_id DESC)` makes the default ACTIVE page an index range scan.
- **Reuse the purge machinery (Rust, T05/T06):** `context_document_purge.rs` already implements sealed manifests, per-target receipts, verified-empty completion and restart; a Conversation purge built on the same shape avoids a second deletion framework and inherits its review.
- **Title digest in events (Rust, T03):** emitting `title_digest` instead of the title keeps deleted titles out of the immutable journal, so delete never needs to rewrite events.
- **Sidebar extraction (TypeScript, T11/T12):** the Recent Chats block lives inside the 4,000+ line `App.tsx`. Moving it to `ConversationHistoryList.tsx` with a pure `conversation-history-state.ts` reducer makes search debounce, abort and state transitions testable with the existing `node --test` runner and enables virtualization. Authority (status fences, versions, purge) stays in the Rust daemon.

## Dependency order

T01 -> T02 -> T03 -> T04 -> T05 -> T06. T03 -> T07. (T04, T07) -> T08. (T05, T06, T07) -> T09. (T07, T08, T09) -> T10 -> T11 -> T12. (T05, T09) -> T13. (T06-T09) -> T14. (T04, T11) -> T15. (T12, T14) -> T16. (T11-T16) -> T17 -> T18 -> T19.

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T02, T03 | Feature 001 contract promotion and storage single-flight (`CONVERSATION_TURN_IN_PROGRESS`) | Archive and delete reuse the active-turn check |
| T04 | Feature 001 final-response commit hardening (same `conversations.rs` functions) | Index hook lands in the same transactions; sequence edits |
| T05 | Feature 006 Conversation-scoped grant/invocation query | Delete quiescence check |
| T06 | Feature 011 Runtime startup recovery hook | Purge resume at startup |
| T10 | Feature 011 packaged sidecar and Windows named pipe | Desktop path |
| T11 | Feature 011 sidebar shell and keyboard baseline | Shared shell |
| T12 | Feature 001 header and composer truthfulness | Same header file |
| T13 | Features 003, 007, 009, 018 readers; Feature 005/007 Evidence redaction | Delete fences across owners |
| T15, T17, T19 | Feature 019 release matrix, soak harness and evidence bundle | Qualification per row |
| T17 | Feature 017 backup/restore | AC-019 restore behaviour |

Cross-package task IDs are named by outcome because other packages are being renumbered in the same planning pass; the orchestrator resolves them in the traceability index. Wave: 014 is in Wave 1 (gate A) and needs only the Feature 001 aggregate.

## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), harness and version (OpenCode v2.0.32 for real history), evidence class (CODE, SYSTEM, USER), report path and reviewer. Statuses above are source-audit classifications at this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
