# Data model - Agent Relay on One Conversation

**Feature:** 004 | **Status:** DRAFT

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- PortableContextPackage — derived, not a native transcript.
- Per-turn AgentBinding id on the committed message.
- Distinct AgentSession per harness admission.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

None in this revision. Blockers in SPEC section 5 must be closed before a migration is designed.

## Current source boundary (2026-11-06)

- `ConversationContextPackage` is an internal derived value, not an independently persisted aggregate or new table. The storage reader constructs it from the committed Conversation message prefix through the USER turn's exact event revision.
- It carries the target AgentBinding and message-level `source_agent_binding_id` for AGENT messages. SYSTEM_NOTICE is excluded. Native session IDs, handles, private harness transcripts and credentials are not part of the portable prompt package.
- The current implementation enforces 1,024 source messages / 4 MiB for the package and 256 KiB for encoded native text. These are code constants, not accepted portable-context schema/retention policy; B-15 remains blocked.
- The context record can represent `resource_refs`, but prompt encoding refuses nonempty references/RESOURCE content until an explicit resource materializer exists. There is no persisted ContextAttachment or durable Context Used receipt in this relay slice.
- Per-turn `agent_binding_id` and `agent_session_id` are existing Conversation message projection fields; the current UI does not render the producing binding.