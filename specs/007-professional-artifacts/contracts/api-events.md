# API and events - Professional Artifacts One Format

## Currently served Artifact operations

- `GET /v1/artifacts`: authenticated Workspace-scoped Artifact list with optional Library status / Task filters and cursor pagination.
- `GET /v1/library`: saved-only Library projection.
- `GET /v1/artifacts/{artifact_id}` and `GET /v1/artifacts/{artifact_id}/versions/{version}`: exact Artifact and immutable version metadata.
- `GET /v1/artifacts/{artifact_id}/versions/{version}/content`: exact managed version content with stored media type, size and digest checks; the local desktop bridge has a 10 MiB transfer bound.
- `GET /v1/artifacts/{artifact_id}/edit-head`: edit preconditions for an existing managed `text/plain` Artifact up to 1 MiB.
- `POST /v1/artifacts/{artifact_id}/text-version`: bounded owner-authored UTF-8 text append to an existing Artifact. The request pins the Artifact aggregate version through `If-Match`, plus content version, Resource aggregate version and exact parent ResourceRevision; `Idempotency-Key` replays the original receipt. Storage commits the paired ResourceRevision and immutable ArtifactVersion plus events and receipt atomically.
- `POST /v1/artifacts/{artifact_id}/promote` and `/archive`: explicit TRANSIENT→SAVED and SAVED→ARCHIVED metadata transitions with expected aggregate version and idempotency receipt. Content versions and Resource heads are unchanged.
- `GET /v1/tasks/{task_id}/artifacts`: read-only committed Task Artifact output projection; no Task execution/create support is implied.

## Missing operations / target-only owner contract

- There is no `POST /v1/artifacts` or other production Artifact create command in the composed daemon. The schema/event registry contains `artifact.created.v1`, and `docs/ARTIFACTS-EVIDENCE.md` specifies atomic create semantics, but no active production producer emits that event.
- There is no general managed binary/format publication operation. The version write store stages only UTF-8 `text/plain` bytes up to 1 MiB and appends to an already-existing Artifact.
- There is no format-specific editor/export handler and no durable ExportReceipt record/event/route. Native `artifact_save_as` is a separate Tauri local write returning only SAVED/CANCELLED; it creates no Artifact/Resource revision or domain event.
- Do not generate TypeScript/OpenAPI operations for create/export until the Artifact owner contract and `docs/schemas/` are changed together and a composed handler is served.

## Canonical event boundary

Use canonical owner events already registered in `docs/EVENTS.md` and `docs/schemas/domain-event.schema.json`: `artifact.created.v1`, `artifact.version.created.v1`, the matching `resource.created.v1` / `resource.revision.observed.v1`, and existing Artifact Library promotion/archive events. The current daemon emits the version-created event for text append and Library state events; it does not emit the initial artifact-created event because no create handler exists.

## Required target create ordering

Authorize the owner and Workspace; validate source Task/Attempt or user-authored/imported source; verify and stage exact content bytes outside SQLite; in one authoritative transaction create the Artifact and ARTIFACT Resource, first ResourceRevision, version 1, provenance/input dependency edges, aggregate snapshots, canonical events and principal-scoped idempotency receipt. A duplicate key with a different logical payload conflicts. No format process/provider call occurs inside SQLite.

## Required target export ordering

Pin the exact immutable ArtifactVersion and digest, authorize the export destination class, invoke the selected format engine outside SQLite, validate the exported bytes and digest, then commit a durable ExportReceipt idempotently. Never put a raw local filesystem path into shared events. Native desktop Save As stays a distinct native operation and must not be presented as a durable provider/export receipt.

## Error and compatibility contract

Reuse canonical errors in `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`, including `STALE_VERSION`, `IDEMPOTENCY_CONFLICT`, `ARTIFACT_ARCHIVED`, `WORKSPACE_ARCHIVED`, `PROVIDER_UNAVAILABLE`, `INVALID_ARGUMENT`, `INTEGRITY_FAILURE` and `UNSUPPORTED_MEDIA_TYPE` where their canonical conditions apply. Unsupported formats remain explicitly read-only/unavailable; do not map preview success to edit/export success.
