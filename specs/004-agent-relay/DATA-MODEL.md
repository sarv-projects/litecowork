# Data model - Agent Relay on One Conversation

**Feature:** 004 | **Updated:** 2026-11-13 (current-evidence crosswalk only; target revision remains 1.0)

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

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

No migration is authorized by this file. B-15 and B-16 are accepted; the portable package is derived and producer fields already exist. A durable truncation/refusal receipt may require a migration, but A-66 leaves its storage placement open. If an owner-approved T01 decision adds a table, allocate the then-next free migration after the currently wired v32 and the v33-v46 allocations recorded in `docs/LITECOWORK-BUILD-TASKS.md`; v47 is only a candidate if it remains free at that time.

## Historical source snapshot (2026-11-06; superseded by the current cross-check below)

- `ConversationContextPackage` is an internal derived value, not an independently persisted aggregate or new table. The storage reader constructs it from the committed Conversation message prefix through the USER turn's exact event revision.
- It carries the target AgentBinding and message-level `source_agent_binding_id` for AGENT messages. SYSTEM_NOTICE is excluded. Native session IDs, handles, private harness transcripts and credentials are not part of the portable prompt package.
- The current implementation enforces 1,024 source messages / 4 MiB for the package and 256 KiB for encoded native text. B-15 (accepted 2026-11-09) adopts these as the V1 bounds; they become schema only when 004-T01 promotes them into SCHEMAS.md and the OpenAPI document.
- At that source snapshot, the context record could represent `resource_refs`, but no materializer was wired before prompt encoding. Feature 001 T14 later added dispatch-time authorization/materialization for eligible text ResourceRefs. There is still no persisted relay ContextAttachment or durable Context Used receipt in this slice.
- Per-turn `agent_binding_id` and `agent_session_id` are existing Conversation message projection fields (declared nullable in the OpenAPI `ConversationMessage` schema); the Tauri bridge passes them through, and the desktop snapshot parser retains both. The transcript resolves the producing profile from the stored binding identity; provider-native session handles remain private.

## Current source boundary (2026-11-13)

- `ConversationContextPackage` remains a derived read snapshot through the exact committed USER-message revision; it is not a durable aggregate or table. The builder orders the bounded package by aggregate revision, excludes `SYSTEM_NOTICE`, and carries `source_agent_binding_id` on prior AGENT messages. The SQLite loader fetches no more than 1,024 message rows, but first scans the full conversation for orphan messages and runs COUNT/SUM aggregates over the committed prefix; that full-prefix work means AC-011's bounded-work guarantee is not yet proven.
- Feature 001 T14 reauthorizes attached ResourceRefs and materializes eligible text before prompt encoding. Binary/root content remains unavailable. The accepted B-15 Workspace instruction preamble is not assembled into the current package. No typed, exact-version ArtifactRef read/authorization/materialization path is wired into Conversation dispatch.
- `window_conversation_context` can return an in-memory `ContextReduction`, and the supervisor currently discards it as `_context_reduction`. It is neither a durable relay refusal/truncation receipt nor a UI-visible provenance record. B-15 still requires refusal with an audited receipt when the relay cannot fit the accepted bounds; A-66 leaves its durable storage location for T01 to define.
- The message projection and desktop parser retain nullable LiteCowork `agent_binding_id` and `agent_session_id`; the UI resolves committed producer labels from the stored binding identity. This is CODE-level projection/display evidence, not a qualified heterogeneous relay or proof that native session handles entered portable context.

See the [2026-11-13 Feature 004 source cross-check](../../docs/audit/FEATURE-004-CURRENT-CROSSCHECK-2026-11-13.md) for exact source ranges, hashes, task states and limits. No source behavior or locked B-15/B-16 decision is changed by this crosswalk.
