# Data model - Conversation co-edit and take-control

**Feature:** 012 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- ConversationTurn - reused from Feature 001; take-control settles it `CANCELLED` after observed quiescence. No new turn type.
- ConversationControlEvent - immutable `OWNER_TOOK_CONTROL` / `OWNER_RELEASED_CONTROL` record correlated to the taken-over turn. Implemented storage record in migration v32; service/API/schema promotion remains separately tracked by TASKS T01/T05.
- OwnerSideNote - bounded owner message, kind `OWNER_SIDE_NOTE`, correlated to a non-terminal turn id, excluded from that turn's native prompt. Implemented v32 storage record; no served authoring operation or UI.

## Relations

Workspace is the security boundary. Conversation owns the turn, control events and side-notes; child records carry workspace and conversation id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Take-control: cancel request -> observe native quiescence (outside the transaction) -> atomic settle `CANCELLED` + control-event. A racing completion/failure wins and remains authoritative. Side-note: single bounded append, valid only while the correlated turn is non-terminal. Terminal states are explicit; unknown is not success.

## Persistence and events

Reuse the Feature 001 turn-settlement events. Any new control-event or side-note event requires a same-change edit to [docs/EVENTS.md](../../docs/EVENTS.md) and `docs/schemas/domain-event.schema.json`, and any new message kind requires a same-change edit to the Conversation owner and `docs/schemas/`.

## Retention

Follow the owner retention rules. Purge removes future visibility of side-note payload bytes; the control-event receipt of the take-over may remain as provenance.

## Migration

Storage layer shipped on 2026-11-10 (012-T05 partial): migration v32, `docs/schemas/sqlite-v32.sql`, adds `conversation_control_events` (append-only, UPDATE and DELETE triggers abort, `UNIQUE(turn_id, kind)`, kinds `OWNER_TOOK_CONTROL` and `OWNER_RELEASED_CONTROL`) and `conversation_owner_side_notes` (body 1 to 4000 characters, correlated to a turn, never a `conversation_messages` row). The `sqlite-v32.sql` number replaces the v30 that earlier text named. There is still no served route, no domain service wiring and no UI, so the current-source-boundary notes below about routes and UI remain true. See specs/012 TASKS.md T05.

## Current source boundary (2026-11-12)

- ConversationCoeditStore and v32 control-event/side-note tables exist with storage tests. No co-edit/take-control service, route or owner-control UI is served.
- Side notes are stored separately from conversation_messages; the ordinary context builder does not load that table. End-to-end exclusion and provider continuation qualification remain open.
- Feature 001 has cancellation/quiescence CODE foundations; Feature 012 still needs atomic owner-control settlement, racing-completion handling, provider evidence and packaged recovery qualification.
- Native session handles, credentials and private harness transcripts remain excluded from public projections/events and backups.
