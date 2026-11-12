# Data model - Coworkers and Scoped Memory

**Feature:** 009 | **Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 amendment IN_REVIEW. A-91 gates manual-memory schema/persistence; not a claim of implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

### Present in the current source tree

- Coworker head and immutable CoworkerRevision, including the persisted CoworkerDefinition and CoworkerContextPolicy JSON.
- Workspace `primary_coworker_id`.
- Task `origin_coworker_id` and `origin_coworker_revision` pins.
- Resource-owned ContextDocument metadata/status and local purge-plan/receipt state for the currently accepted owner scopes (Personal Profile and Workspace Notes).
- Conversation ID, Workspace ID, title, active AgentBinding/session, version and timestamps. The current Conversation record has no Coworker origin/revision.

### Target records, not currently persisted as Coworker-scoped records

- Coworker-owned ContextDocument / Coworker-private MemoryRecord metadata and the Resource revision/body that it references.
- Immutable Coworker origin on a Coworker-owned Conversation.
- LearningCandidate and CoworkerLearningPolicy revision records. The deterministic Rust types in `storage-core` are not SQLite records or a storage contract.
- A Coworker-scoped retrieval/ContextAttachment receipt linking the actual Coworker context to a specific Conversation turn or Task packet.

The generic ContextDocument schema names PERSONAL_PROFILE, COWORKER_NOTES, WORKSPACE_NOTES and GOAL_NOTES. Current domain creation validation accepts only the first and third kinds with the corresponding USER and WORKSPACE owner_ref; it rejects Coworker and Goal ownership. Do not model a Coworker ContextDocument as present merely because an enum, Settings option or target record is defined.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

No migration has shipped for this package yet; the source chain ends at v32. The build allocation reserves v38 for Feature 009 Conversation Coworker origin and memory fields, but no v38 file or source schema exists. A-91 blocks finalizing the manual-memory metadata fields until the Memory/Coworker owners resolve B-24 against LEARNING.md §7; no candidate id may be fabricated and no second lifecycle may be added. B-24/B-25 and the A-18 create boundary are accepted (2026-11-09). Conversation origin, COWORKER_NOTES creation, CoworkerCapabilityAssignment and responsibility proposals must be promoted through their canonical owners and T02/T04/T11/T12. The automatic LearningCandidate store belongs to Feature 020; owner-scoped Resource records remain Resource/Coworker/Conversation-owned, not a second memory database.
