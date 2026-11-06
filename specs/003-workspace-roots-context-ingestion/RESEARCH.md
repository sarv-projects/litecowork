# Research - Workspace Roots and Context Ingestion

**Feature:** 003 | **Checked:** 2026-11-06 (source, route and UI audit; no native root scan/provider Conversation executed)

## Sources

Authorities: docs/WORLD-RESOURCES.md, docs/CONTEXT.md, docs/STORAGE.md, docs/API.md, docs/TRUST.md. Reject wholesale machine scan (competitor desktop agents often index home by default). Copy: explicit folder consent and citation receipts. Owner-contract references last checked 2026-11-01; source and UI audit checked 2026-11-06.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Source and UI audit (2026-11-06)

Inspected `apps/litecoworkd/src/operator.rs`, `workspace_root_scan_operator.rs`, `resource_archive_operator.rs`, `rag_retrieval_operator.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/src/App.tsx`, `workspaces/WorkspaceRootObservations.tsx`, `workspaces/workspace-root-mutations.ts`, `workspace-root-scan.ts`, `resources/retrieval-grounding.ts`, `presentation/rich-presentation.ts`, and `crates/storage-sqlite/src/{resource_index.rs,rag_retrieval.rs}`. Cross-checked the composed route inventory and the Feature 003 API/UI contracts.

- The public OpenAPI `addWorkspaceRoot` route is target-only, but the desktop has a distinct private path: Tauri opens the native picker, derives a path-scoped idempotency key, and sends the selected path to authenticated local-only `POST /__litecowork_local/workspace-root-selection`. The daemon commits root, Resource, location and private identity binding together. This is not a public REST route.
- That private selection handler is compiled for Linux/macOS only. The Windows fallback returns `503 UNAVAILABLE` with “Persistent folder access is not qualified on this platform”. Tauri has a Windows path-encoding branch, but the daemon handler still rejects Windows. `App.tsx` leaves Add persistent folder enabled and only surfaces the failure after the user chooses a folder. On supported targets, choosing a folder immediately submits the mutation; there is no separate post-picker preview/confirm, and the UI hardcodes `METADATA` watch policy plus `NONE` replication policy.
- The public list/pause/resume/revoke and scan/observations routes are mounted. The scan handler is explicitly Linux-only: every non-Linux build returns `501 SCAN_UNSUPPORTED_PLATFORM`. The UI does not know this before the first attempt; after the error it records an unsupported state and disables later scans.
- The Linux scan is a metadata census only. It records file membership/observations and does not read file contents, create content revisions or build text indexes. It is bounded (1,000 entries, depth 32, 1 MiB relative-locator budget, bounded deadline and two concurrent scans). The UI copy correctly says names/metadata only and no watching/indexing.
- Resource upload/quick-import/revision routes are real and separate from WorkspaceRoot scanning. The Library accepts ZIP/PDF/Office/image files as opaque original Resources; the mounted `ZipIntakeNotice` reports extraction unavailable and says contents remain opaque, unindexed and unattached. A source ZIP metadata-preview route/worker exists, but the bridge is available only with the non-default Linux `source-zip-worker` development feature; release sidecars build without it, and the current UI has no call site. This does not provide extraction, child-Resource provenance or ingestion. The local text index accepts only allowlisted UTF-8 text: `.txt`, `.md`, `.markdown`, `.csv`, `.json`, `.jsonl`, `.ndjson`, `.rs`, `.py`, `.toml`, `.yaml`, `.yml`, `.js`, `.jsx`, `.ts`, `.tsx`, `.css` and matching text MIME types. Each indexed Resource is limited to 1 MiB, with NUL/control-character and term-count checks. PDF, Office, archive, image, audio and video are not indexable by this provider.
- `POST /resources/retrieval` is a real local lexical retrieval route. It returns citations pinned to exact Resource revision and digest, or explicit insufficient evidence/provider-unavailable. It is read-only and appends no event, durable receipt, ContextAttachment, Effect or Artifact; it is not called by the Conversation composer.
- No persisted ContextAttachment domain record/table/event or production UI call site was found. `resources/retrieval-grounding.ts` contains a strict decoder/view helper but has no production import/call site. The chat UI has no Context Used surface. The rich-presentation parser also rejects documents with nonempty citation arrays, despite a renderer branch for citation blocks.
- The Resource/Library/Workbench UI shows Resource projections and read-only revision previews. `WorkspaceRootObservations` is an explicit metadata browser; opening an observed Resource does not prove it has a content revision. Root membership and a Resource upload/index are distinct lifecycles.
- This means the intended Windows local folder journey is not implemented on the current checkout: root add returns 503, scan returns 501, and no content is ingested from roots. The separate Resource upload/index route does not satisfy automatic root ingestion or Context Used.

These are source/route/UI findings. No native root selection, live scan, content extraction from a root, or provider-backed Conversation was executed during this audit.
## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) and refresh URLs before lock.
- No native root selection, live scan, root content extraction or provider-backed Conversation was executed. Source inspection shows Windows add/scan are explicitly unavailable; the Linux scan path remains unqualified by a live USER run.

## Unresolved questions

See SPEC section 5 blockers. Owners are named on the package README.
