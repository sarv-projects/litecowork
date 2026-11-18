# API and events - Professional Artifacts One Format

**Status:** Base revision 1.0 ACCEPTED/LOCKED; Saved View revision 1.1 API amendment IN_REVIEW after owner direction 2026-11-17. Saved View target operations are not served.

## Currently served Artifact operations

- `GET /v1/artifacts`: authenticated Workspace-scoped Artifact list with optional Library status / Task filters and cursor pagination.
- `GET /v1/library`: saved-only Library projection.
- `GET /v1/artifacts/{artifact_id}` and `GET /v1/artifacts/{artifact_id}/versions/{version}`: exact Artifact and immutable version metadata.
- `GET /v1/artifacts/{artifact_id}/versions/{version}/content`: exact managed version content with stored media type, size and digest checks; the local desktop bridge has a 10 MiB transfer bound.
- `GET /v1/artifacts/{artifact_id}/edit-head`: edit preconditions for an existing managed `text/plain` Artifact up to 1 MiB.
- `POST /v1/artifacts/{artifact_id}/text-version`: bounded owner-authored UTF-8 text append to an existing Artifact. The request pins the Artifact aggregate version through `If-Match`, plus content version, Resource aggregate version and exact parent ResourceRevision; `Idempotency-Key` replays the original receipt. Storage commits the paired ResourceRevision and immutable ArtifactVersion plus events and receipt atomically.
- `POST /v1/artifacts/{artifact_id}/promote` and `/archive`: explicit TRANSIENT→SAVED and SAVED→ARCHIVED metadata transitions with expected aggregate version and idempotency receipt. Content versions and Resource heads are unchanged.
- `GET /v1/tasks/{task_id}/artifacts`: read-only committed Task Artifact output projection; no Task execution/create support is implied.
- No Saved View create, kind-filter, or update operation is served. Existing Artifact reads/archive do not prove Saved View support.

## Missing operations / target-only owner contract

- There is no `POST /v1/artifacts` or other production Artifact create command in the composed daemon. The schema/event registry contains `artifact.created.v1`, and `docs/ARTIFACTS-EVIDENCE.md` specifies atomic create semantics, but no active production producer emits that event.
- There is no general managed binary/format publication operation. The version write store stages only UTF-8 `text/plain` bytes up to 1 MiB and appends to an already-existing Artifact.
- There is no format-specific editor/export handler and no durable ExportReceipt record/event/route. Native `artifact_save_as` is a separate Tauri local write returning only SAVED/CANCELLED; it creates no Artifact/Resource revision or domain event.
- Do not generate TypeScript/OpenAPI operations for create/export until the Artifact owner contract and `docs/schemas/` are changed together and a composed handler is served.

## Accepted target operations (not yet in source)

- `POST /v1/artifacts` (B-30): create with source class UserAuthored, Imported or TaskGenerated, optional `conversation_origin {conversation_id, message_id}`, `Idempotency-Key`; emits `artifact.created.v1`.
- Markdown version publish: the existing `text-version` append generalized to `text/markdown` through the B-21 adapter, or a sibling route fixed in TASKS T01; same `If-Match` and parent-revision pins.
- `POST /v1/artifacts/{artifact_id}/exports` (B-31): exact version, export format, destination class; commits `ArtifactExportReceipt` and `artifact.exported.v1` (event to be registered in EVENTS.md and `domain-event.schema.json`).
- Format capability matrix read (route name fixed in T01) and `GET /v1/artifacts?conversation_id=` filter for the Conversation association.
- `POST /v1/artifacts` accepts `source.kind=RICH_PRESENTATION_SAVED_VIEW` with exact source IDs/digests only. The request does not carry client-authored semantic text, RichPresentation bytes or calculator state; the daemon reconstructs and validates the envelope from committed owner records and atomically creates a `SAVED_VIEW` Artifact in `SAVED` status with the standard create receipt.
- `GET /v1/library?kind=SAVED_VIEW` (or the equivalent exact Library filter selected in T01) returns Workspace-scoped Saved View summaries. Open uses existing exact Artifact/version/content reads; Archive uses the existing `/archive` operation.
- `POST /v1/artifacts/{artifact_id}/saved-view-versions` appends a canonical envelope from a newly committed exact eligible same-Conversation presentation. It requires `If-Match`, expected Artifact and Resource versions, exact parent ResourceRevision, source digests and `Idempotency-Key`; it returns the standard version receipt. Local calculator values remain transient. Cross-Conversation changes use create-as-new.
- Saved Views have no delete, export, generic Markdown edit, compare or arbitrary JSON upload operation in V1. The internal envelope is not a user-export format.

## Canonical event boundary

Use canonical owner events already registered in `docs/EVENTS.md` and `docs/schemas/domain-event.schema.json`: `artifact.created.v1`, `artifact.version.created.v1`, the matching `resource.created.v1` / `resource.revision.observed.v1`, and existing Artifact Library promotion/archive events. The current daemon emits the version-created event for text append and Library state events; it does not emit the initial artifact-created event because no create handler exists. Saved Views add no event type; event bodies carry bounded IDs, versions and digests only, never semantic text, input values, RichPresentation bytes or local paths.

## Required target create ordering

Authorize the owner and Workspace; validate the source and construct its exact content bytes; stage the bounded blob outside SQLite; then, in one authoritative transaction, reauthorize and re-read the source, recheck its Workspace, commit/deletion fence and exact digests, and atomically create the Artifact and ARTIFACT Resource, first ResourceRevision, version 1, provenance/input dependency edges, aggregate snapshots, canonical events and principal-scoped idempotency receipt. For `RICH_PRESENTATION_SAVED_VIEW`, validate the embedded immutable RichPresentation's declared defaults and ranges; the request accepts no session-modified calculator state. Initialize `library_status=SAVED` only for this explicit Save to Library intent. If the source changed or was deleted while the blob was staged, the transaction writes no Artifact records; the unreferenced staged blob is handled by the existing orphan cleanup. A duplicate key with a different logical payload conflicts. No format process/provider call occurs inside SQLite.

A Saved View version update uses the same outside-transaction blob staging and inside-transaction source recheck. The append transaction reauthorizes the Workspace and Artifact, confirms the Artifact and backing Resource are still writable, compares both expected head versions and the exact parent ResourceRevision, and revalidates the newly committed same-Conversation source and digest before appending the paired immutable ArtifactVersion/ResourceRevision, provenance, events and receipt. A source deletion, archive, or head conflict creates no new version; the previously committed head remains readable, and any staged but unreferenced blob is left to orphan cleanup. The request carries source identity and expected versions, never client-authored envelope bytes or session-modified control values.

## Required target export ordering

Pin the exact immutable ArtifactVersion and digest, authorize the export destination class, invoke the selected format engine outside SQLite, validate the exported bytes and digest, then commit a durable ExportReceipt idempotently. Never put a raw local filesystem path into shared events. Native desktop Save As stays a distinct native operation and must not be presented as a durable provider/export receipt.

## Error and compatibility contract

Reuse canonical errors in `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`, including `STALE_VERSION`, `IDEMPOTENCY_CONFLICT`, `ARTIFACT_ARCHIVED`, `WORKSPACE_ARCHIVED`, `PROVIDER_UNAVAILABLE`, `INVALID_ARGUMENT`, `INTEGRITY_FAILURE` and `UNSUPPORTED_MEDIA_TYPE` where their canonical conditions apply. Unsupported formats remain explicitly read-only/unavailable; do not map preview success to edit/export success.
