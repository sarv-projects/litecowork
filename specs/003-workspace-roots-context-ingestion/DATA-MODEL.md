# Data model - Workspace Roots and Context Ingestion

**Feature:** 003 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. New tables are added only by the TASKS T01 migration promoted together with the owner documents.

## Records

- WorkspaceRoot status ACTIVE | PAUSED | UNAVAILABLE | REVOKED (CODE: `crates/storage-core/src/lib.rs` root store methods, `crates/storage-sqlite/src/workspace_root_membership.rs`, `workspace_root_observations.rs`).
- Private local identity binding: Linux/macOS device + inode; Windows volume serial number + 128-bit file id (`windows_local_security::DirectoryIdentity`; the Windows branch in `apps/litecoworkd/src/local_filesystem.rs` is compiled but not reached by root add today).
- Resource revision and content digest.
- ContextAttachment (accepted target interface, not yet in source): `context_attachment_id`, `workspace_id`, `conversation_id`, `turn_id`, `resource_id`, `resource_revision_id`, `content_digest`, `root_id` (nullable for uploaded Resources), `retrieval_receipt_id` (nullable for explicit @-references), `created_at`, idempotency key.
- Retrieval receipt (accepted target interface, not yet in source): `retrieval_receipt_id`, `workspace_id`, `conversation_id`, `turn_id`, `query_digest`, `provider_id`, cited revision list, outcome GROUNDED | INSUFFICIENT_EVIDENCE | PROVIDER_UNAVAILABLE, `created_at`, idempotency key.
- Runtime-local private locator.

## Relations

Workspace is the security boundary. Child records carry workspace id. A ContextAttachment references one exact revision and, when it came from retrieval, one retrieval receipt. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. ContextAttachments and retrieval receipts are immutable once committed. Root revoke fences future attachment and retrieval; it does not rewrite committed attachments or the transcript that already received bytes. External effects happen after commit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md) (`workspace.root.created.v1`, `workspace.root.status.changed.v1`). Any ContextAttachment or retrieval-receipt event is an accepted target interface (not yet in source) and requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain. Conversation delete fences (Feature 014) remove attachments from future projections without deleting the receipt identity.

## Migration

One additive migration, allocated `docs/schemas/sqlite-v36.sql` (TASKS T01/T09; see the Migration allocation in docs/LITECOWORK-BUILD-TASKS.md). No 003 migration exists today; the 003-T02 workspace root lifecycle storage work (partial) uses existing tables.

**Open contract conflict (A-69):** this file makes `ContextAttachment.root_id` nullable for uploaded Resources and `retrieval_receipt_id` nullable for explicit ResourceRefs. SPEC §7.2 currently requires a retrieval receipt in `CommitContextAttachment` and a root id in `ContextUsedEntry`. These shapes are not interchangeable. Do not treat this proposal as the canonical schema or write v35 until the owner resolves A-69 and the Feature 001 admission boundary in A-65.
