# Data Model - Conversation History Management

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
**Feature:** 014 | **Authority:** DATA-MODEL.md, STATE-MACHINES.md, STORAGE.md, EVENTS.md, SCHEMAS.md.

This feature extends the canonical Conversation record. It must not create a second Conversation catalog or a second deletion framework. Only the `conversation_history_state` side table exists in source (migration v31, `docs/schemas/sqlite-v31.sql`, 014-T03 storage layer, 2026-11-10); every other record below (search index, purge plans, purge receipts) is still target and takes a later number than the current v35 chain. The 2026-11-20 migration chain ended at `sqlite-v34.sql`; v35 was wired on 2026-11-23 for Feature 002 Runtime-local auth observations.

## 1. Existing records

| Record | Source | Feature use |
|---|---|---|
| Conversation | `conversations` (`sqlite-v1.sql`); `ConversationRecord` | Unchanged; status and timestamps live in the `conversation_history_state` side table (v31) |
| ConversationMessage | `conversation_messages` (plaintext `content_json`) | Search source; purge target |
| ConversationTurn | `conversation_turns` | Active-turn check; purge of content-bearing fields |
| AgentSession | `agent_sessions`; Runtime-local `agent_session_host_bindings` | Session metadata may remain as provenance, but every session must be CLOSED/LOST and its local host binding released before tombstone; the opaque native handle is never copied to the purge manifest or public projection |
| UserRequest / UserRequestResponse | `user_requests`, `user_request_responses` | Quiescence check; purge targets |
| RichPresentation | `rich_presentations` (`sqlite-v12.sql`) and document blobs | Fence and purge target |
| AggregateSnapshot | `aggregate_snapshots.blob_ref_json` | Snapshot blobs for Conversation and messages are purge targets |
| Request receipt | `request_dedup.response_json` | Conversation command responses are purge targets |
| DomainEvent | `domain_events` | Retained; message events carry digests only |
| Task | `tasks.conversation_id` | Retained; origin shown as deleted conversation |
| Capability grants/activations/invocations | `capability_*` with `conversation_id` | Quiescence check; Evidence retention per O-02 |

## 2. Side table (shipped, v31) and proposed records (not yet in source)

| Record | Fields | Notes |
|---|---|---|
| ConversationHistoryState (side table, SHIPPED in migration v31 as `conversation_history_state`) | `conversation_id` (primary key), `workspace_id`, `status` (`ACTIVE`, `ARCHIVED`, `DELETION_PENDING`, `DELETED`), `archived_at`, `deleted_at`, `updated_at` | Replaces the earlier plan to add status columns to `conversations`: released migrations are immutable, so a Conversation with no row here is ACTIVE and `conversations` is unchanged. Title is never copied into the side table; clearing the title at tombstone writes only the `conversations` row (target, not yet implemented). Rust: `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, tests in `conversation_history_tests.rs` |
| ConversationSearchTerm | `workspace_id`, `conversation_id`, `message_id` (null for title), `term_token`, `key_version` | F-01 option (a): HMAC tokens, no plaintext |
| ConversationSearchIndexState | `workspace_id`, `state` (`BUILDING`, `READY`), `watermark` | Backfill progress |
| ConversationPurgePlan | `plan_id`, `workspace_id`, `conversation_id`, `sealed_at`, `target_count` | Immutable once sealed |
| ConversationPurgeReceipt | `plan_id`, `target_id`, `target_kind`, `target_ref`, `state` (`PENDING`, `ACKNOWLEDGED`), `acknowledged_at` | One per target |

List index (target): `(workspace_id, status, created_at DESC, conversation_id DESC)`. Shipped today: `idx_conversation_history_state_workspace_status` on `(workspace_id, status)` in v31 only; the ordered list index is not built yet.

## 3. Lifecycle

```text
ACTIVE <-> ARCHIVED
ACTIVE | ARCHIVED -> DELETION_PENDING -> DELETED
```

- Rename: ACTIVE only; version +1.
- Archive: ACTIVE with no nonterminal turn; unarchive: ARCHIVED -> ACTIVE.
- Delete: ACTIVE or ARCHIVED, quiescent; DELETION_PENDING fences all reads; DELETED after every receipt is acknowledged. DELETED is terminal.

## 4. Purge manifest target kinds (B-38)

`MESSAGE_ROW`, `SEARCH_TERMS`, `TURN_CONTENT`, `USER_REQUEST_DETAIL`, `USER_REQUEST_RESPONSE`, `RICH_PRESENTATION_ROW`, `RICH_PRESENTATION_BLOB`, `SNAPSHOT_BLOB`, `REQUEST_RECEIPT_RESPONSE`, `CHANNEL_THREAD_MAPPING`. Bounded like the ContextDocument manifest; an oversized manifest refuses delete with no partial write. `AgentSessionHostBinding` is not a purge target: deletion admission first proves the session CLOSED/LOST and the local binding released; UNKNOWN cleanup blocks tombstoning. Native-harness transcripts and external channel copies remain outside this local purge boundary.

## 5. Events (source-backed and proposed)

The first three event types below are emitted by the current rename/archive/unarchive
storage transactions and are registered in `docs/EVENTS.md` and
`docs/schemas/domain-event.schema.json` under user-directed implementation review.
Feature 014 remains IN_REVIEW; this source alignment does not record formal domain-owner
acceptance. The deletion events remain target-only until their owner tasks and routes exist.

| Event | Payload |
|---|---|
| `conversation.renamed.v1` | workspace_id, conversation_id, title_digest (SHA-256 digest or null only when the title is cleared), changed_by (USER principal), aggregate_version; never title text |
| `conversation.archived.v1` | workspace_id, conversation_id, changed_by (USER principal), aggregate_version |
| `conversation.unarchived.v1` | workspace_id, conversation_id, changed_by (USER principal), aggregate_version |
| `conversation.deletion_requested.v1` | conversation_id, aggregate_version, plan_id, target_count |
| `conversation.deleted.v1` | conversation_id, plan_id |

## 6. Retention, backup and migration

- Tombstone and events retained for the Workspace lifetime; no title or content in either.
- Feature 017 backups include the tombstone, sealed plan and receipts; a restore resumes purge.
- Content inside older backups is outside the purge guarantee and the delete dialog says so.
- Forward-only migration; existing rows become `ACTIVE`; schema fingerprint updated.
