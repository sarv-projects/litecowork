# Data model - Professional Artifacts One Format

**Feature:** 007 | **Status:** Base revision 1.0 ACCEPTED/LOCKED; Saved View revision 1.1 amendment IN_REVIEW after owner direction 2026-11-17. The amendment is TARGET, not source implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. New tables and columns are added only by the TASKS T01 migration promoted together with the owner documents.

## Records

- Artifact identity (`ArtifactRecord` in `crates/storage-core/src/artifact.rs`: `artifact_id`, `workspace_id`, `resource_id`, `task_id`, `kind`, `display_name`, `current_version`, `library_status`, `created_at`, `version`).
- Immutable version and content digest (`ArtifactVersionRecord`).
- Provenance links and dependency edges.
- Conversation origin (accepted target interface, not yet in source): nullable `source_conversation_id` and `source_message_id` on the Artifact, written only by the create transaction.
- Export receipt (accepted target interface, not yet in source): `export_receipt_id`, `workspace_id`, `artifact_id`, `artifact_version`, `export_format`, `transformer_version`, `destination_class` (V1 value `LOCAL_OWNER_SELECTED_FILE`), `exported_digest`, `actor`, `created_at`, idempotency key. No local path.
- Format capability matrix (accepted target interface, not yet in source): daemon-owned static table per media type (SPEC §5); not persisted per Artifact.
- Saved View (TARGET amendment): an Artifact with `kind=SAVED_VIEW`, media type `application/vnd.litecowork.saved-view+json`, and canonical content described in [`contracts/saved-view.md`](contracts/saved-view.md) and [`docs/schemas/saved-view-artifact.schema.json`](../../docs/schemas/saved-view-artifact.schema.json). It has no separate table or ID.
- Saved View source lineage (TARGET amendment): exact Workspace, Conversation, committed Message and RichPresentation IDs/digests; the semantic fallback and RichPresentation snapshot are content bytes, while the source link remains provenance only and is not a live dependency edge.
- Saved View input state (TARGET amendment): the immutable RichPresentation's declared defaults are part of its snapshot. No session-modified calculator values or separate input-state record are captured; this preserves Feature 016 B-40.

## Relations

Workspace is the security boundary. Child records carry workspace id. A Conversation origin must reference a committed message in the same Workspace. For Saved Views, the Artifact remains readable as an independent user-created snapshot after source deletion; Feature 014 controls how the origin link is masked/marked. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success. A Saved View create uses the existing B-30 atomic transaction and may start `SAVED` only for the explicit `RICH_PRESENTATION_SAVED_VIEW` source; updates from a new committed presentation append ArtifactVersion/ResourceRevision pairs using both expected versions and the owner receipt. Calculator values changed during a session remain transient under B-40.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md): `artifact.created.v1` (registered, no producer today), `artifact.version.created.v1`, and their paired Resource events. Saved Views add no event type; events carry IDs, version and digest metadata only, never semantic text, numeric input values or presentation bytes. `artifact.exported.v1` is an accepted target interface (not yet in source) and requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain. Conversation delete (Feature 014) does not delete an associated Artifact; it fences the origin link in projections per the 014 contract. Saved View payloads follow Artifact backup/retention and remain independent of their source Conversation; no second local state store may restore or resurrect them.

## Migration

One additive migration, allocated `docs/schemas/sqlite-v37.sql` (TASKS T01; see the Migration allocation in docs/LITECOWORK-BUILD-TASKS.md). No migration exists today.

## Current implementation boundary (2026-11-06, re-checked 2026-11-09)

- `ArtifactRecord`, `ArtifactVersionRecord`, backing ARTIFACT Resource and ResourceRevision are canonical durable owner records. The active code reads these records and appends a new version to an already-existing Artifact; it has no production create port/Operator command.
- Initial create is a distinct target transaction: first committed managed content plus Artifact identity, backing Resource, ResourceRevision 1, ArtifactVersion 1, provenance/dependency edges, snapshots, creation events and principal-scoped receipt must agree atomically. `artifact.created.v1` is registered but not emitted by the current daemon.
- `MANAGED_BLOB` version append currently means manual UTF-8 `text/plain` edit only, capped at 1 MiB. `EXTERNAL_RESOURCE` is metadata/link-only and cannot be edited or downloaded by the current desktop path.
- Current preview media support is broader than edit: Markdown, CSV/TSV, JSON, XML and YAML can be safely previewed; only `text/plain` is accepted by the user text-version append.
- Native `Save As` reads one exact MANAGED_BLOB version up to 10 MiB and writes locally. It has no persisted ExportReceipt and does not alter Artifact/Resource version/event history.
- Office readiness probes and UI accepted upload extensions do not imply Artifact parser/editor/export support.
