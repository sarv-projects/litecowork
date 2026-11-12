# Tasks - Coworkers and Scoped Memory

**Feature:** 009 | **Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 amendment IN_REVIEW. Existing tasks remain authorized only within revision 1.0 scope; A-91 blocks manual-memory schema/persistence. Tasks not started unless marked.
**Spec revision:** 1.1 (amendment IN_REVIEW; baseline 1.0 accepted) | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** Coworkers, Conversation, Memory, Trust, Desktop UI. Owner acceptance: 2026-11-09 via docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md (B-24, B-25, A-18 rider, A-20/A-34 vocabulary).

## Current baseline

Source cross-check refreshed 2026-11-12 against active source checkpoint `6adadab539a62ca1c8b70621c7f88e1785cfe8c2` on branch `main`; docs-only handoff commits do not change this source baseline.

- **Operator:** `apps/litecoworkd/src/coworker_operator.rs` mounts `GET/POST /v1/coworkers`, `GET /v1/coworkers/{coworker_id}`, `POST .../revisions`, `GET .../revisions/{revision}`, `POST .../status` (`change_status`, `ResponsibilityCommand::SetCoworkerStatus` with `If-Match` and `Idempotency-Key`), `GET .../presence` (`presence_unavailable`, 503 `DEPENDENCY_UNAVAILABLE`) and `POST /v1/workspaces/{workspace_id}/primary-coworker`. `DomainError::CoworkerInactive` maps to `COWORKER_PAUSED`. There is no Coworker Conversation, memory, assignment or responsibility route.
- **Storage:** `crates/storage-sqlite/src/coworkers.rs` `SqliteCoworkerStore` persists Coworker heads, immutable revisions and events (tests in `crates/storage-sqlite/src/coworker_tests.rs`, for example `coworker_sqlite_revisions_events_replay_and_restart`). `lifecycle_facts` computes `has_active_automation` (any ENABLED Automation or non-terminal occurrence pinned to the Coworker) and sets `resume_reconciled: !has_active_automation`. Because `check_coworker_transition` in `crates/domain-responsibility/src/service.rs` allows ACTIVE to PAUSED unconditionally but rejects PAUSED to ACTIVE with `ReconciliationRequired` when `resume_reconciled` is false, a Coworker paused while it has an enabled Automation cannot be resumed until that Automation is disabled or paused. `check_origin_status(Paused, Proactive)` exists in the same file with no production caller.
- **Pause at occurrence time:** `validate_coworker_provenance` in `crates/storage-sqlite/src/automation_admission.rs` rejects Task materialization when the pinned Coworker is not ACTIVE (`"Automation Coworker is not active"`). The TriggerHost (`settle_one_trigger` in `apps/litecoworkd/src/automation_operator.rs`) records that as a deferral blocker and the cursor keeps naming the slot, so a paused Coworker produces a failing retry every 30 s pass, not a recorded skip. Enable time checks Coworker ACTIVE in `validate_automation_references` (`crates/storage-sqlite/src/coworkers.rs`).
- **Conversation:** `ConversationRecord` in `crates/storage-core/src/conversation.rs` has `conversation_id`, `workspace_id`, `title`, `active_agent_binding_id`, `version`, `created_at` and no Coworker origin; the `conversations` table (`docs/schemas/sqlite-v1.sql`) matches. The migration chain now ends at `docs/schemas/sqlite-v32.sql`; the 009 migration is allocated v38 (docs/LITECOWORK-BUILD-TASKS.md).
- **ContextDocument create:** `valid_context_document_metadata` in `crates/domain-workspace/src/lib.rs` accepts only `PERSONAL_PROFILE` (USER owner = principal) and `WORKSPACE_NOTES` (WORKSPACE owner = selected Workspace). `ContextDocumentCreateMetadata.kind` in the canonical OpenAPI schema is narrowed to those two values by accepted A-18; record/read metadata and the Settings editor still expose all four kinds, so distinguish those types. The generated `apps/litecowork-ui/src/api/operator-api.ts` has been regenerated from the corrected A-18 description. `ContextOwnerRef` already has a COWORKER variant (`workspace_id`, `coworker_id`). Status and deletion routes are `/v1/resources/{resource_id}/context-document/status` and `/deletion` in `apps/litecoworkd/src/operator.rs`, with purge in `crates/storage-sqlite/src/context_document_purge.rs`.
- **Retrieval:** `POST /v1/resources/retrieval` in `apps/litecoworkd/src/rag_retrieval_operator.rs` over `crates/storage-sqlite/src/rag_retrieval.rs` is Workspace or explicit-Resource scoped lexical search with no Coworker filter and no `CoworkerContextPolicy` consumer.
- **Learning:** `crates/storage-core/src/coworker_learning.rs` defines `LearningCandidate`, `CoworkerLearningPolicy`, `MemoryRecordMetadata` (required `source_candidate_id`, free-text `status`) and pure gates. No store, route or caller exists. The automatic pipeline belongs to Feature 020.
- **Tauri:** `apps/litecowork-ui/src-tauri/src/coworker_bridge.rs` `coworker_request` forwards a fixed operation set (list, get, presence, revision, create, revise, status, primary); `context_document_deletion_bridge.rs` handles ContextDocument deletion.
- **UI:** `apps/litecowork-ui/src/coworkers/HomeCoworkerOnboarding.tsx` creates a general Assistant and can set Workspace primary; it opens no chat. `CoworkerSettings.tsx` (700 lines) edits policy and offers `COWORKER_NOTES` as an allowed context kind. `apps/litecowork-ui/src/App.tsx` rejects a PAUSED Coworker for owner-saved Home work ("This Coworker is paused. Choose an active Coworker to save work from Home."), contrary to the owner-Task rule. Tests: `tests/home-onboarding.test.ts`, `tests/coworker-focus-plan.test.ts`, `tests/coworker-roster-request-epoch.test.ts`, `tests/context-document-deletion.test.ts`.

Gaps to V1: no Coworker-owned Conversation, no COWORKER_NOTES create path, no Coworker-scoped retrieval, no memory UI, Quick Create opens no chat, pause is enforced only as a retry loop and blocks resume, no capability assignment or responsibility operation, and no CODE/SYSTEM/USER qualification on either release row.

## 009-T01 - Promote accepted Coworker contracts into owner docs and schemas

- **Status:** PARTIAL - owner acceptance of B-24/B-25, A-18 and A-20/A-34 is recorded. No accepted interface is promoted: no Coworker Conversation route, assignment, responsibility proposal or Coworker origin column exists in `docs/schemas/`. Register A-91 leaves the manual-memory metadata/status representation unresolved; do not finalize the Feature 009 memory schema until its owner decision is recorded.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE. Feature 020 reviews the `MemoryRecordMetadata` decision because it owns the candidate path; Feature 006 reviews the assignment shape because it owns grants.
- **Owner:** Coworkers, Conversation, Memory, API, Events and Storage owners.
- **Allowed paths:** `docs/COWORKERS-TARGET.md`, `docs/RESPONSIBILITIES.md`, `docs/LEARNING.md`, `docs/CONTEXT.md`, `docs/DATA-MODEL.md`, `docs/API.md`, `docs/EVENTS.md`, `docs/STORAGE.md`, `docs/STATE-MACHINES.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/domain-event.schema.json`, `docs/schemas/error-codes.schema.json`, `docs/schemas/sqlite-v38.sql` (reserved by the current build allocation; not written), this package.
- **Contract:** SPEC sections 5, 7.2, 15; AC-009, AC-017, AC-019, AC-021; SPECIFICATION-STANDARD section 7.
- **Change intent:** promote accepted Conversation origin, assignment, responsibility and API/event/error contracts in their canonical owners and schemas. Keep `COWORKER_NOTES` creation unavailable until the Coworker-scoped Resource path verifies Coworker and Workspace ownership. Resolve register A-91 at the Memory/Coworker owners before defining manual-memory metadata or finalizing the memory portion of `sqlite-v38.sql`: do not create a second lifecycle, treat the automatic-only `source_candidate_id` as optional without owner approval, or forge a LearningCandidate. Pin the AC-013 per-document and per-Coworker memory bounds and the AC-016 retrieval budget in the owner contract. T04 owns the paired create-schema/validator change after the same-Workspace owner check is included.
- **Acceptance:**
  - [x] B-24 and B-25 owner acceptance recorded with the A-18 rider (SPEC section 5).
  - [ ] A-91 owner decision recorded before manual-memory metadata or status fields are finalized.
  - [ ] Every promoted name exists exactly once in its owner doc and machine schema; no parallel definition remains in this package.
  - [ ] The migration is the next immutable `sqlite-vNN.sql`; older migrations are byte-identical and the schema fingerprint test passes.
  - [ ] Negative: the OpenAPI validator rejects an assignment or responsibility payload that carries a grant, approval or `enabled` trigger field.
  - [ ] One documented status vocabulary for owner-authored memory; no second state machine.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py` and `cargo test -p storage-sqlite --locked schema` (fingerprint filter to be added by this task if absent)
- **Evidence:** owner decision links, contract/schema diff and command output in Completion evidence. B-24/B-25/A-18 remain accepted decisions; T17 updates implementation evidence and does not close or reopen those decision rows.
- **Stop:** a promoted name conflicts with an existing owner definition; resolve it at the owner, not in this package.

## 009-T02 - Store an immutable Coworker origin on Conversations

- **Status:** NOT STARTED. `ConversationRecord` and the `conversations` table have no Coworker field.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 001 Conversation aggregate and versioning (the record is shared; coordinate the migration with the 001 storage tasks).
- **Owner:** Conversation, Storage.
- **Allowed paths:** `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/conversation_turn_tests.rs`, `crates/storage-sqlite/src/lib.rs`, `docs/schemas/sqlite-v38.sql` (the Feature 009 migration reserved in the build allocation; not yet written).
- **Contract:** SPEC sections 6, 7.2; COWORKERS-TARGET "Coworker-owned Conversations must belong to the same Workspace"; AC-001, AC-017.
- **Change intent:** add `owner_coworker_id: Option<String>` and `owner_coworker_revision: Option<u64>` to `ConversationRecord` (both null or both set) and the columns plus a `(workspace_id, owner_coworker_id, created_at)` index. `create_conversation` in `SqliteConversationStore` validates in the same transaction that the Coworker exists in the same Workspace, is not ARCHIVED, and that the revision exists; PAUSED is allowed because an owner chat is owner-origin work. No update path writes these columns. Add `list_conversations_for_coworker` with a cursor. Write `conversation.coworker.assigned.v1` in the create transaction.
- **Acceptance:**
  - [ ] AC-017: a Coworker Conversation stores the pinned id and revision; a later Coworker revision does not change it.
  - [ ] AC-001: an ordinary Conversation still stores both fields null and needs no Coworker.
  - [ ] Negative: a Coworker from another Workspace, an ARCHIVED Coworker, an unknown revision or one field without the other is rejected and nothing is written.
  - [ ] Idempotency: the same create key and payload returns the original Conversation; the same key with another Coworker conflicts.
  - [ ] Restart: the origin survives reopen of the SQLite file.
- **Command:** `cargo test -p storage-sqlite --locked coworker_conversation` (filter to be added by this task) and `cargo test -p storage-core --locked conversation`
- **Evidence:** test names in Completion evidence.
- **Stop:** the change would let any update path rewrite the origin.

## 009-T03 - Serve Coworker Conversation create/list through the Operator and Tauri bridge

- **Status:** NOT STARTED.
- **Depends on:** T02
- **Cross-package prerequisites:** Feature 011 authenticated IPC on both release rows. Feature 001 Send and Feature 002 qualified binding are required before the opened Conversation can send a reply and before E2E qualification; neither blocks creating or reopening an UNBOUND Conversation.
- **Owner:** Operator/API, Conversation, Desktop bridge.
- **Allowed paths:** `apps/litecoworkd/src/coworker_operator.rs`, `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/conversation_operator_tests.rs`, `apps/litecowork-ui/src-tauri/src/coworker_bridge.rs`, `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs`, `apps/litecowork-ui/src/coworkers/coworker-api.ts`, `apps/litecowork-ui/src/coworkers/desktop-coworker-api.ts`, `apps/litecowork-ui/src/api/operator-api.ts` (regenerated).
- **Contract:** SPEC sections 6, 9, 10; AC-017; docs/LOCAL-OPERATOR-IPC.md.
- **Change intent:** mount `POST /v1/coworkers/{coworker_id}/conversations` (Idempotency-Key required) and `GET /v1/coworkers/{coworker_id}/conversations` (cursor) in `coworker_operator::routes`, using `authorized_workspace` and the existing `valid_id` check. Return the Conversation presentation used by `conversation_operator.rs` so the composer can open it. Add `conversations` and `createConversation` operations to the fixed operation match in `coworker_request`; do not add a generic path passthrough. Extend `ConversationRecord` projection types in the UI with the origin fields.
- **Acceptance:**
  - [ ] AC-017: create then list returns the Conversation under that Coworker only; `GET /v1/conversations` shows it with its origin.
  - [ ] Denial: missing or wrong Workspace header, foreign Coworker id, ARCHIVED Coworker and unauthenticated peer each return a typed error and write nothing.
  - [ ] Duplicate replay returns the original receipt.
  - [ ] Bridge test asserts only the declared paths are reachable.
- **Command:** `cargo test -p litecoworkd --locked coworker_conversation` (filter to be added by this task) and `cargo test -p litecowork-ui --locked coworker_bridge`
- **Evidence:** test names; served-route inventory delta recorded in T17.
- **Stop:** the route would need a new authority path outside the authenticated Operator.

## 009-T04 - Re-widen the ContextDocument create enum to accept COWORKER_NOTES (A-18/B-24 rider)

- **Status:** NOT STARTED. The narrowed enum and the two-kind validator are in source today.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 003 owns the Resource upload/create path; review the change with its owner.
- **Owner:** Memory, Workspace domain, API schema owner.
- **Allowed paths:** `docs/schemas/operator-api.openapi.yaml` (`ContextDocumentCreateMetadata` only), `docs/DATA-MODEL.md` ("ContextDocument create ownership"), `docs/CONTEXT.md`, `crates/domain-workspace/src/lib.rs` (`valid_context_document_metadata` and its tests), `apps/litecoworkd/src/operator.rs` (Resource upload begin/commit owner checks), `apps/litecoworkd/src/resource_upload_archive_operator_tests.rs` (owner-scope and replay regression coverage), `apps/litecowork-ui/src/api/operator-api.ts` (regenerated), `apps/litecowork-ui/src/coworkers/CoworkerSettings.tsx`.
- **Contract:** SPEC section 5 (B-24 rider), section 7; register A-18; AC-010, AC-018.
- **Change intent:** in one reviewed change, add `COWORKER_NOTES` to `ContextDocumentCreateMetadata.kind` and extend `valid_context_document_metadata` with an exact COWORKER owner_ref shape (`kind`, selected `workspace_id`, `coworker_id`). The authenticated Resource upload caller must verify in the owner transaction, both when admitting the upload and when committing it, that the Coworker exists, belongs to that Workspace and is not ARCHIVED; reject stale or mismatched owners before Resource/ContextDocument commit. `GOAL_NOTES` stays rejected. Update the accepted A-18 owner prose and generated client type. The Coworker Settings Add Memory control remains unavailable until T05 serves the complete scoped read/write path.
- **Acceptance:**
  - [ ] AC-018: COWORKER_NOTES with a same-Workspace Coworker is accepted.
  - [ ] AC-010 and AC-018 negatives: GOAL_NOTES, a COWORKER owner from another Workspace, an unknown or ARCHIVED Coworker, extra owner fields and a USER owner on COWORKER_NOTES are rejected with `CONTEXT_OWNER_KIND_REJECTED`.
  - [ ] Schema, validator and DATA-MODEL.md change in the same commit; the contract test fails if one changes without the others.
  - [ ] A generic Resource upload cannot create a Coworker note without the same-Workspace, non-ARCHIVED owner check; archiving between upload admission and commit leaves no committed Resource.
- **Command:** `cargo test -p domain-workspace --locked context_document` and `python3 scripts/validate_architecture.py` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names and the commit SHA covering all three files.
- **Stop:** the schema and validator cannot land in one change; do not widen either alone.

## 009-T05 - Serve owner-authored Add Memory as a Coworker-scoped ContextDocument (B-24)

- **Status:** NOT STARTED. No Coworker memory path exists; `coworker_learning.rs` has no caller.
- **Depends on:** T01, T04
- **Cross-package prerequisites:** Feature 003 ResourceService upload/commit; Feature 020 owns the automatic LearningCandidate pipeline (link only, no shared code path).
- **Owner:** Memory, ResourceService, Trust.
- **Allowed paths:** `apps/litecoworkd/src/operator.rs` (Resource upload commit for COWORKER_NOTES), `apps/litecoworkd/src/coworker_operator.rs` (`GET .../memory`), `crates/storage-sqlite/src/resource_index.rs`, `crates/storage-sqlite/src/coworkers.rs`, `apps/litecowork-ui/src-tauri/src/coworker_bridge.rs`. Do not alter the automatic-only `crates/storage-core/src/coworker_learning.rs` metadata shape unless the A-91 owner decision explicitly assigns that change to its owner.
- **Contract:** SPEC sections 5 (B-24), 7.2, 10; LEARNING.md section 7; AC-008, AC-009, AC-013, AC-014.
- **Change intent:** After T01 records the A-91 owner decision, Add Memory commits a user-authored ContextDocument revision with COWORKER owner, source `USER_AUTHORED`, and B-24 status ACTIVE in the approved manual-memory representation through the existing upload/commit path. It never creates a `LearningCandidate` and never fills `source_candidate_id`. Enforce the T01 per-document and per-Coworker bounds before commit. Serve `GET /v1/coworkers/{coworker_id}/memory` as a filtered ContextDocument list. Memory text is stored as inert data; events carry ids and digests only.
- **Acceptance:**
  - [ ] AC-008: the committed record has owner-authored provenance and no candidate id.
  - [ ] AC-013: an over-bound document or Coworker total returns `MEMORY_TOO_LARGE` / `MEMORY_QUOTA_EXCEEDED` and nothing partial is committed.
  - [ ] AC-009 and AC-014: memory text containing instructions, links or secret-shaped strings grants no permission, Connection, package or schedule, and no raw local path or body appears in an event.
  - [ ] Denial: another Coworker's or Workspace's id returns `COWORKER_SCOPE_DENIED`.
- **Command:** `cargo test -p litecoworkd --locked coworker_memory` (filter to be added by this task) and `cargo test -p storage-sqlite --locked coworker_memory`
- **Evidence:** test names in Completion evidence.
- **Stop:** the design needs a second memory database or a forged candidate record.

## 009-T06 - Revoke and purge Coworker memory with fences and receipts

- **Status:** PARTIAL - ContextDocument status and deletion routes (`/v1/resources/{resource_id}/context-document/status`, `/deletion`) and `crates/storage-sqlite/src/context_document_purge.rs` exist for the two served kinds, with tests in `context_document_purge_tests.rs`. They are not exercised for COWORKER_NOTES and retrieval does not consult them per Coworker.
- **Depends on:** T05
- **Cross-package prerequisites:** Feature 003 retrieval delete fences; Feature 014 delete semantics if B-38 changes the purge window.
- **Owner:** Memory, Storage.
- **Allowed paths:** `crates/storage-sqlite/src/context_document_purge.rs`, `crates/storage-sqlite/src/context_document_purge_tests.rs`, `apps/litecoworkd/src/operator.rs` (status/deletion handlers), `apps/litecowork-ui/src-tauri/src/context_document_deletion_bridge.rs`.
- **Contract:** SPEC sections 5, 11; AC-004, AC-011, AC-015.
  - **Change intent:** apply the existing Resource status and deletion transactions to COWORKER_NOTES using the A-91-approved mapping from B-24's REVOKED/PURGE_PENDING outcomes; both bump the scope version that retrieval and context assembly read, so a fenced record cannot be returned. The deletion receipt keeps ids, digests and timestamps without bytes. Serialize Add Memory and revoke/purge on the same Coworker scope with the existing CAS version.
- **Acceptance:**
  - [ ] AC-004: after revoke or purge the record is absent from retrieval and context assembly; no capability is granted.
  - [ ] AC-011: purge leaves a receipt without the deleted bytes; the blob is unreachable.
  - [ ] AC-015: a concurrent Add Memory and purge on the same scope commit in one order; purged bytes never return and a stale writer gets a version conflict.
  - [ ] Crash between deletion plan and blob removal resumes on restart and never resurrects the record.
- **Command:** `cargo test -p storage-sqlite --locked context_document_purge`
- **Evidence:** test names in Completion evidence.
- **Stop:** purge would need to delete bytes without a durable receipt.

## 009-T07 - Coworker-scoped retrieval that enforces CoworkerContextPolicy

- **Status:** NOT STARTED. Retrieval has no Coworker filter; `CoworkerContextPolicy` has no consumer.
- **Depends on:** T05, T06
- **Cross-package prerequisites:** Feature 003 retrieval receipt called from Send and Context Used; Feature 001 context dispatch.
- **Owner:** Retrieval, Memory.
- **Allowed paths:** `apps/litecoworkd/src/rag_retrieval_operator.rs`, `crates/storage-sqlite/src/rag_retrieval.rs`, `crates/storage-sqlite/src/rag_retrieval_tests.rs`, `crates/domain-rag/src/`.
- **Contract:** SPEC sections 6, 10; CONTEXT.md; AC-003, AC-006, AC-016.
- **Change intent:** add an optional Coworker scope to the retrieval request. When a Coworker Conversation retrieves, include that Coworker's COWORKER_NOTES plus Workspace kinds allowed by its `allowed_context_kinds`, cap results at `max_retrieved_items`, and exclude every other Coworker's documents. Coworker-less retrieval excludes all COWORKER_NOTES. Apply the filter in the SQL `WHERE` clause on indexed `(workspace_id, owner kind, coworker_id)` columns, never as a post-filter over a Workspace-wide scan.
- **Acceptance:**
  - [ ] AC-003: Coworker A memory is never returned for Coworker B or for an ordinary Conversation.
  - [ ] AC-016: retrieval for one Coworker reads only that scope within the T01 budget (query-plan assertion plus timing on a seeded corpus).
  - [ ] AC-006: after restart the same requests return the same scoped results.
  - [ ] Denial: a forged Coworker id from another Workspace returns `COWORKER_SCOPE_DENIED`.
- **Command:** `cargo test -p storage-sqlite --locked rag_retrieval` and `cargo test -p litecoworkd --locked rag_retrieval`
- **Evidence:** test names and the recorded query plan.
- **Stop:** the filter cannot be expressed in the indexed query.

## 009-T08 - Quick Create opens an UNBOUND first Coworker chat (B-25)

- **Status:** PARTIAL - `HomeCoworkerOnboarding.tsx` creates the Coworker (and can set primary) with `tests/home-onboarding.test.ts`; it opens no chat and does not show the unbound state.
- **Depends on:** T03
- **Cross-package prerequisites:** None to implement the unbound chat shell. A qualified Feature 002 binding plus the Feature 001 Send gate is required before the chat can send; the package's T16 task owns the packaged journey after those prerequisites qualify.
- **Owner:** Desktop UI, Coworkers.
- **Allowed paths:** `apps/litecowork-ui/src/coworkers/HomeCoworkerOnboarding.tsx`, `apps/litecowork-ui/src/coworkers/coworker-api.ts`, `apps/litecowork-ui/src/coworkers/desktop-coworker-api.ts`, `apps/litecowork-ui/src/App.tsx` (navigation only), `apps/litecowork-ui/tests/home-onboarding.test.ts`.
- **Contract:** SPEC section 5 (B-25), 7.2 `QuickCreateCoworker`; AC-002, AC-012, AC-023.
- **Change intent:** Quick Create sends the Coworker create with one idempotency key and no `default_lead_agent_binding_id`, then the Coworker Conversation create (T03) with a second key, then opens that Conversation. If the second commit fails, keep the created Coworker, show "Coworker created; chat not opened" with Retry that reuses the second key. The chat shows "No agent bound" with a link to agent setup and Send disabled until a qualified binding exists.
- **Acceptance:**
  - [ ] AC-002: Quick Create lands in a chat scoped to the new Coworker id.
  - [ ] AC-023: the Coworker has no lead binding and the UI never invents one.
  - [ ] AC-012: a failed first-chat commit leaves the Coworker; Retry does not create a second Coworker.
  - [ ] Double-click and reload during create produce one Coworker and one Conversation.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names in Completion evidence.
- **Stop:** the flow would need to fabricate or auto-select an AgentBinding.

## 009-T09 - Coworker chats and Memory & Knowledge UI

- **Status:** NOT STARTED. `CoworkerSettings.tsx` has roster, detail, revision form, status actions and presence (503) but no chats list or memory view.
- **Depends on:** T03, T05, T06
- **Cross-package prerequisites:** Feature 011 app-wide keyboard and focus baseline.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/coworkers/CoworkerSettings.tsx`, `apps/litecowork-ui/src/coworkers/CoworkerMemory.tsx` (new), `apps/litecowork-ui/src/coworkers/coworker-memory-state.ts` (new), `apps/litecowork-ui/src/coworkers/coworker-settings.css`, `apps/litecowork-ui/tests/coworker-memory-state.test.ts` (new), `specs/009-coworkers-scoped-memory/contracts/ui-behavior.md`.
- **Contract:** SPEC section 6; contracts/ui-behavior.md; docs/V1-UI-ACTION-STATE-LEDGER.md; AC-001, AC-007.
- **Change intent:** Coworker detail gains "Chats" (list from T03, New chat button) and "Memory & Knowledge" (list from T05 with source, revision, status; Add Memory form; Revoke and Purge with confirmation). Each action has loading, empty, error, denied (`COWORKER_SCOPE_DENIED`), offline (Runtime unavailable), stale (version conflict, reload prompt) and recovery states, held in a pure `coworker-memory-state.ts` reducer. Presence stays labelled unavailable while the route returns 503. Ordinary New Chat stays in the left nav with no Coworker.
- **Acceptance:**
  - [ ] AC-007: all controls are keyboard reachable, have accessible names, announce status changes through a live region, and restore focus after dialogs.
  - [ ] AC-001: New Chat works with zero Coworkers.
  - [ ] Each state above renders from a reducer unit test; no state is styled as success when data is absent.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; UI ledger rows updated in T17.
- **Stop:** a control has no owner route.

## 009-T10 - Coworker pause fences enabled Automations and resume reconciles them

- **Status:** PARTIAL - admission rejects a non-ACTIVE Coworker (`validate_coworker_provenance`) and enable checks ACTIVE, but the TriggerHost turns that into a 30 s retry loop, `resume_reconciled: !has_active_automation` blocks resume while an Automation is enabled, `check_origin_status` has no caller, and the Home composer rejects PAUSED for owner work.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 010 Coworker PAUSED recheck at occurrence settlement (010 owns the TriggerHost change; this task owns Coworker status semantics, resume reconciliation and the composer).
- **Owner:** Coworkers (Responsibility domain), Desktop UI.
- **Allowed paths:** `crates/domain-responsibility/src/service.rs`, `crates/domain-responsibility/src/tests.rs`, `crates/storage-sqlite/src/coworkers.rs` (`lifecycle_facts`), `crates/storage-sqlite/src/coworker_tests.rs`, `apps/litecowork-ui/src/App.tsx` (recipient selection copy and gate), `apps/litecowork-ui/src/coworkers/CoworkerSettings.tsx` (pause copy).
- **Contract:** SPEC sections 7a, 11; RESPONSIBILITIES.md; AC-020.
- **Change intent:** define resume reconciliation: PAUSED to ACTIVE commits when every enabled Automation pinned to the Coworker has its cursor advanced past slots that elapsed while paused (per its misfire policy, using the 010 skip records), instead of requiring zero enabled Automations. Route proactive admission through `check_origin_status(status, WorkOrigin::Proactive)` so the domain rule has one caller. Change the Home gate so a PAUSED Coworker is allowed for owner-saved Tasks (`WorkOrigin::Owner`) and ARCHIVED stays rejected. Pause copy says scheduled runs are skipped while paused.
- **Acceptance:**
  - [ ] AC-020: with an enabled Automation, pause then resume succeeds and no Task is created for slots inside the paused window.
  - [ ] AC-020: an owner-saved Task naming a PAUSED Coworker is admitted; an ARCHIVED one is rejected.
  - [ ] Race: pause committed between slot planning and Task admission still yields no Task (admission recheck in the same transaction).
- **Command:** `cargo test -p domain-responsibility --locked coworker` and `cargo test -p storage-sqlite --locked coworker` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test names in Completion evidence.
- **Stop:** resume would back-fire slots from the paused window beyond the misfire policy.

## 009-T11 - Responsibilities as proposals that become admitted Routines, with no grant

- **Status:** NOT STARTED. Routine and Automation aggregates exist (`apps/litecoworkd/src/routine_operator.rs`, `automation_operator.rs`); no responsibility record or route exists.
- **Depends on:** T01, T10
- **Cross-package prerequisites:** Feature 010 saved-PAUSED Automation rule; Feature 020 may produce proposals later (it must use this proposal path).
- **Owner:** Coworkers, Automation, Trust.
- **Allowed paths:** `crates/domain-responsibility/src/models.rs`, `crates/domain-responsibility/src/service.rs`, `crates/storage-sqlite/src/coworkers.rs`, `crates/storage-sqlite/src/coworker_tests.rs`, `apps/litecoworkd/src/coworker_operator.rs`, `apps/litecowork-ui/src-tauri/src/coworker_bridge.rs`, `apps/litecowork-ui/src/coworkers/CoworkerResponsibilities.tsx` (new).
- **Contract:** SPEC sections 5 LOCKED, 7.2 `ResponsibilityProposal`; COWORKERS-TARGET StandingResponsibility; AC-019.
- **Change intent:** add the responsibility aggregate with DRAFT proposals. `POST .../responsibilities` stores a DRAFT that admits nothing. Owner commit links existing same-Workspace Routine revisions and creates or links Automations, which are saved PAUSED by the existing Automation rule; enabling stays a separate Automation resume. No path creates a CapabilityGrant, Approval or enabled trigger. Responsibility status joins Coworker status as a fence on proactive admission.
- **Acceptance:**
  - [ ] AC-019: a DRAFT proposal creates no Task and no trigger cursor; commit links PAUSED Automations only.
  - [ ] Negative: a cross-Workspace Routine ref, a stale Coworker revision and a payload with grant fields are rejected.
  - [ ] Duplicate commit returns the original receipt.
  - [ ] UI lists proposals with Review and Commit, separate from enabled Automations.
- **Command:** `cargo test -p domain-responsibility --locked responsibility` and `cargo test -p litecoworkd --locked coworker_responsibility` (filter to be added by this task)
- **Evidence:** test names in Completion evidence.
- **Stop:** the proposal would need to enable a trigger or grant a capability.

## 009-T12 - Coworker capability assignment as a ceiling

- **Status:** NOT STARTED. No assignment record or route exists.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 006 Connection and CapabilityGrant (grants stay there); Feature 013 package inventory (installation stays there).
- **Owner:** Coworkers, Capability Fabric, Trust.
- **Allowed paths:** `crates/domain-responsibility/src/models.rs`, `crates/storage-sqlite/src/coworkers.rs`, `crates/storage-sqlite/src/coworker_tests.rs`, `apps/litecoworkd/src/coworker_operator.rs`, `apps/litecowork-ui/src-tauri/src/coworker_bridge.rs`, `apps/litecowork-ui/src/coworkers/CoworkerSettings.tsx`.
- **Contract:** SPEC section 7.2 `CoworkerCapabilityAssignment`; COWORKERS-TARGET; release contract Area 5 item 5.6; AC-009, AC-021.
- **Change intent:** versioned assign, revise and revoke with expected version and Idempotency-Key, events `coworker.capability_assignment.*.v1`. The record stores scope and operation-class ceilings only. Invocation-time checks (connection auth, Workspace policy, Trust, Task lease) stay in 006. Revoke never uninstalls or disconnects.
- **Acceptance:**
  - [ ] AC-021: assign, revise and revoke are versioned; two Coworkers share one Connection and revoking one assignment leaves the other.
  - [ ] AC-009: no assignment write creates a grant, approval, install or Connection authentication.
  - [ ] Negative: stale version conflicts; a Connection from another Workspace is rejected.
- **Command:** `cargo test -p storage-sqlite --locked capability_assignment` (filter to be added by this task) and `cargo test -p litecoworkd --locked capability_assignment`
- **Evidence:** test names in Completion evidence.
- **Stop:** assignment would be read anywhere as authority to invoke.

## 009-T13 - Failure and recovery for Coworker chat, memory and pause

- **Status:** NOT STARTED.
- **Depends on:** T02-T12
- **Cross-package prerequisites:** Feature 011 Runtime restart; Feature 001 startup sweep for Conversation turns.
- **Owner:** Coworkers, Storage, Conversation.
- **Allowed paths:** `crates/storage-sqlite/src/coworker_tests.rs`, `crates/storage-sqlite/src/context_document_purge_tests.rs`, `apps/litecoworkd/src/conversation_operator_tests.rs`, `apps/litecoworkd/tests/coworker_scoped_memory.rs` (new).
- **Contract:** SPEC sections 11, 12; contracts/errors-recovery.md; AC-005, AC-006, AC-012, AC-015.
- **Change intent:** add a daemon-level integration test that kills and restarts the daemon between Coworker create and first-chat create, during purge, and during a pause/materialization race; replays duplicate requests; and checks a Task naming a stale Coworker revision.
- **Acceptance:**
  - [ ] AC-012: crash after Coworker commit leaves a usable Coworker; retry creates exactly one chat.
  - [ ] AC-006: after restart Coworker chats and memory reload with no cross-scope result.
  - [ ] AC-015: crash mid-purge resumes and never resurrects bytes.
  - [ ] AC-005: a Task naming a stale Coworker version conflicts and does not switch Coworker.
- **Command:** `cargo test -p litecoworkd --test coworker_scoped_memory --locked` (test to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** a recovery path infers success from an intent record.

## 009-T14 - Cross-Workspace leakage fixture (CODE and owner USER)

- **Status:** NOT STARTED.
- **Depends on:** T07, T09, T13
- **Cross-package prerequisites:** Feature 019 evidence bundle format.
- **Owner:** Security, Coworkers, QA.
- **Allowed paths:** `apps/litecoworkd/tests/coworker_scoped_memory.rs`, `apps/litecowork-ui/tests/browser/` (fixture scenario), `scripts/fixtures/coworker_cross_workspace.json` (new), this package.
- **Contract:** SPEC section 10; ACCEPTANCE-AND-E2E-PLAN E2E-09; AC-003, AC-022.
- **Change intent:** a seeded fixture with Workspaces W1 and W2, two Coworkers each, Coworker chats, COWORKER_NOTES memory and Workspace notes. The CODE test calls every list, get, retrieval, memory and Conversation route with each Workspace header and foreign ids. The USER script walks the owner through the same checks in the packaged app and records screenshots and results.
- **Acceptance:**
  - [ ] AC-022: no route returns another Workspace's Coworker, chat, memory or retrieval hit, before and after restart.
  - [ ] AC-003: within one Workspace, no Coworker sees another Coworker's memory.
  - [ ] Owner USER run recorded on each release row.
- **Command:** `cargo test -p litecoworkd --test coworker_scoped_memory --locked cross_workspace` (filter to be added by this task)
- **Evidence:** CODE output plus USER report path in Completion evidence.
- **Stop:** any leak; fix it before T16.

## 009-T15 - Cross-layer trace for each Coworker operation

- **Status:** NOT STARTED.
- **Depends on:** T03, T05, T06, T08-T12
- **Cross-package prerequisites:** NONE
- **Owner:** Coworkers and Desktop UI reviewers.
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `contracts/api-events.md`, `WIRING.yaml`).
- **Contract:** AGENTS.md section 4; AC-007.
- **Change intent:** for Quick Create, New Coworker chat, Add Memory, Revoke, Purge, Pause/Resume, Commit responsibility and Assign capability, record: user outcome, exact control, typed UI command, Tauri operation, Operator route, authorization, service owner, transaction/version/fence, event/receipt, projection, restart behavior and the test that proves it. Add the matching WIRING edges.
- **Acceptance:**
  - [ ] Every operation has a complete row; no control lacks a route and no route lacks a control.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** the trace table in contracts/ui-behavior.md.
- **Stop:** a link in the chain is missing in source.

## 009-T16 - Qualify E2E-09 on the release matrix rows

- **Status:** BLOCKED - needs Feature 001 Send and the Feature 002 OpenCode binding on Windows x64 (owners: Conversation, Agent Runtime); Linux x64 row second.
- **DEFERRED/BLOCKED note:** Why: E2E-09 needs Feature 001 Send and the Feature 002 OpenCode binding on a real row. Unblocks when: 001 and 002 INTEGRATED on the row. Test later at: the owner's Windows laptop, then native Linux.
- **Depends on:** T13, T14, T15
- **Cross-package prerequisites:** Feature 001 qualified Send; Feature 002 OpenCode v2.0.32 binding; Feature 003 Context Used; Feature 011 packaged desktop; Feature 019 release matrix.
- **Owner:** Product, QA, Coworkers.
- **Allowed paths:** this package; evidence paths named by Feature 019.
- **Contract:** ACCEPTANCE-AND-E2E-PLAN E2E-09; SPEC section 15; AC-001 to AC-023.
- **Change intent:** run the packaged desktop journey: optional Coworker, Quick Create, chat with OpenCode v2.0.32, Add Memory, scoped retrieval visible in Context Used, revoke, restart, cross-Coworker and cross-Workspace denial, pause with an enabled Automation and resume. Record CODE, SYSTEM and owner USER evidence on Windows x64 first, then Linux x64.
- **Acceptance:**
  - [ ] E2E-09 passes on each row with denial and recovery included.
  - [ ] Owner USER sign-off per row.
- **Command:** `cargo test -p litecoworkd --test coworker_scoped_memory --locked` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** evidence bundle path per row.
- **Stop:** a row lacks USER evidence; that row stays unqualified.

## 009-T17 - Synchronize status documents and inventories

- **Status:** NOT STARTED.
- **Depends on:** T16
- **Cross-package prerequisites:** NONE
- **Owner:** Documentation owner, orchestrator.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, generated served-route inventory, this package.
- **Contract:** AGENTS.md section 4 steps 5-6.
- **Change intent:** update Coworker rows in IMPLEMENTATION-STATUS.md to the evidence reached, regenerate the served-route inventory with `scripts/audit_served_routes.py`, and update UI ledgers for controls that actually exist. Keep B-24, B-25 and A-18 recorded as accepted owner decisions; record implementation and qualification evidence separately, including any remaining owner decisions such as A-91.
- **Acceptance:**
  - [ ] Route inventory lists the new Coworker routes as served; nothing else changes class without evidence.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of the regenerated inventory.
- **Stop:** a status row would claim more than recorded evidence.

## 009-T18 - Package VERIFIED for V1

- **Status:** NOT STARTED.
- **Depends on:** T01-T17
- **Cross-package prerequisites:** Feature 019 evidence bundle process and release qualification matrix.
- **Owner:** Product (sign-off), Coworkers, Memory, Security.
- **Allowed paths:** this package; `docs/IMPLEMENTATION-STATUS.md`.
- **Contract:** SPEC sections 15-16; AC-001 to AC-023; SPECIFICATION-STANDARD lifecycle.
- **Change intent:** assemble the evidence bundle (per-task revision, command, result, OS, harness, reviewer); move the accepted implementation baseline to IMPLEMENTING at the first implementation merge only after confirming the applicable SPEC revision is owner-accepted, and to VERIFIED after owner sign-off on both release rows.
- **Acceptance:**
  - [ ] Canonical command and T16 pass on Windows x64 + OpenCode v2.0.32 and on Linux x64.
  - [ ] SPEC and WIRING status equal VERIFIED and the validator passes.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle path referenced from IMPLEMENTATION-STATUS.md.
- **Stop:** any row lacks USER evidence.

## Implementation and optimization notes (Rust and TypeScript)

Findings from reading the source, folded into the tasks above. They add no scope.

- **Pause retry loop (Rust, T10 with Feature 010):** a paused Coworker makes `materialize_scheduled_slot` fail inside a full admission transaction on every 30 s pass, forever. Recording one skip and advancing the cursor removes that repeated write work and the misleading blocker.
- **Resume deadlock (Rust, T10):** `resume_reconciled: !has_active_automation` is a correctness bug more than a cost, but the fix also lets `lifecycle_facts` drop the second `EXISTS` over occurrences for the resume path.
- **Scoped retrieval in SQL (Rust/SQLite, T07):** filter by Coworker in the indexed query. Post-filtering a Workspace-wide lexical result in Rust would make cost grow with the whole corpus and break AC-016.
- **Bridge operations stay typed (Rust/Tauri, T03, T05, T11, T12):** `coworker_request` matches a fixed operation list. Keep adding named operations; a generic path passthrough would widen the WebView's reach.
- **UI state in pure TypeScript modules (T09):** `CoworkerSettings.tsx` is 700 lines. Put the memory view in its own component with a pure reducer, following the existing `coworker-focus-plan.ts` and `coworker-roster-request-epoch.ts` pattern, so `node --test` covers it without a browser.
- **Authority stays in Rust:** scope checks, memory bounds, redaction and the learning gates in `coworker_learning.rs` stay in the daemon. The UI only displays them; do not reimplement them in TypeScript.

## Dependency order

T01 -> T02 -> T03 -> T08. T01 -> T04 -> T05 -> T06 -> T07. T03, T05, T06 -> T09. T01 -> T10 -> T11. T01 -> T12. T02-T12 -> T13. T07, T09, T13 -> T14. T03, T05, T06, T08-T12 -> T15. T13, T14, T15 -> T16 -> T17 -> T18. T10 can start right after T01 and should land with the Feature 010 settlement recheck.

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T02, T03 | Feature 001 Conversation aggregate, versioning and Send | The Coworker origin is a column on the shared Conversation record; a usable chat needs Send |
| T03, T09 | Feature 011 authenticated IPC and keyboard baseline | Routes run over the Operator; UI follows the app-wide focus rules |
| T04, T05, T06 | Feature 003 Resource upload/commit, retrieval delete fences | COWORKER_NOTES uses the Resource create path and its fences |
| T05 | Feature 020 LearningCandidate pipeline | Automatic learning lives in 020; owner-authored memory never creates a candidate |
| T06 | Feature 014 delete semantics (B-38) | A changed purge window must apply to Coworker memory too |
| T07 | Feature 003 Context Used and retrieval receipt from Send | Scoped results reach the agent only through that path |
| T08, T16 | Feature 002 OpenCode v2.0.32 binding | Quick Create stays UNBOUND until 002 qualifies a binding |
| T10 | Feature 010 Coworker PAUSED recheck at occurrence settlement | 010 changes the TriggerHost; 009 owns status semantics and resume |
| T11 | Feature 010 saved-PAUSED Automation rule | Committed responsibilities link Automations that start PAUSED |
| T12 | Feature 006 Connection and CapabilityGrant; Feature 013 package inventory | Assignment is a ceiling; grants and installs stay with their owners |
| T14, T16, T18 | Feature 019 release matrix and evidence bundle | Qualification is per release row |

Cross-package tasks are named by outcome because the other packages are being renumbered in the same release-planning pass; the orchestrator resolves them to IDs in the traceability index.

## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), harness and version (OpenCode v2.0.32), auth route, evidence class reached (CODE, SYSTEM, USER), report path and reviewer. Statuses above are source-audit classifications at the date of this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
| T01 (acceptance part) | this revision | `python3 .specify/scripts/validate_feature_package.py` | see orchestrator run | n/a | n/a | documentation | repository owner |
