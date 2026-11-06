# API and events - Workspace Roots and Context Ingestion

Workspace-root route crosswalk (current source, not the target): list, pause, resume, revoke, scan and paginated observations are mounted. Public OpenAPI `POST /workspace-roots` (add) and `PATCH /workspace-roots/{rootId}` (update) remain target-only. Desktop Add uses the separate authenticated local-only `POST /__litecowork_local/workspace-root-selection` route after a native picker; its daemon handler is compiled for Linux/macOS and the Windows fallback returns 503 `UNAVAILABLE`. Scan is mounted only as a route: the implementation is Linux-only and every non-Linux target returns 501 `SCAN_UNSUPPORTED_PLATFORM`. Scan records metadata only, not content revisions.

Resource quick-import, chunked upload, revision upload/commit, content read, text-index rebuild/search and grounded retrieval routes are mounted. A ZIP metadata-preview route is source-mounted, but its isolated worker bridge requires the non-default Linux `source-zip-worker` development feature; release sidecars omit it and the current UI does not call it. It is not extraction or content ingestion. Grounded retrieval is a read-only local lexical search that returns exact revision/digest citations; it creates no durable ContextAttachment, event or retrieval receipt and is not wired into Conversation send. Text indexing is limited to the current allowlist and 1 MiB UTF-8 bound; see SPEC §4.

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. This DRAFT does not declare new event types.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.
