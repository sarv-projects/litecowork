# Research - Workspace Roots and Context Ingestion

**Feature:** 003 | **Research/source audit:** 2026-11-06; grounding re-checked 2026-11-09; current source cross-check: 2026-11-13 (see the linked audit report; no native root scan/provider Conversation executed)
**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

## Sources

Authorities: docs/WORLD-RESOURCES.md, docs/CONTEXT.md, docs/STORAGE.md, docs/API.md, docs/TRUST.md. Reject wholesale machine scan (competitor desktop agents often index home by default). Copy: explicit folder consent and citation receipts. Owner-contract references last checked 2026-11-01; source and UI audit checked 2026-11-06.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Borrowable-design sources (pinned)

| Source | Exact pinned URL | License | What informs ingestion | Verdict |
|---|---|---|---|---|
| ZCode `packages/desktop` thin-shell / Host / CLI split | https://github.com/zai-org/ZCode/tree/a9fe401eede6e6e5e01be9048679c206fe6f02d9 | Apache-2.0 + NOTICE | The Host/shell ownership split informs keeping the folder-selection picker (shell) separate from the daemon root/location commit and the metadata scanner (Host), so OS handles and canonical paths stay Runtime-local. | Port the ownership-split pattern only; do not drop in React or the ZCode Host runtime. |
| DeepSeek Harness `apps/desktop` typed IPC with sender assert | https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159 | MIT | Typed IPC with sender assertion and contextBridge preloads informs the authenticated local-only `workspace-root-selection` route boundary: the untrusted WebView never passes a raw path that bypasses validation. | Prototype the typed-IPC/sender-assert pattern against LOCAL-OPERATOR-IPC; port the pattern, not the code. |

No ZCode or DeepSeek Harness code is imported; both are prototype/reference inputs that reinforce the shell/Host trust boundary this slice already requires.

## Historical source and UI audit (2026-11-06; superseded)

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
## Grounding re-check (2026-11-09)

- Windows add: `create_workspace_root_from_native_selection` has a `#[cfg(any(target_os = "linux", target_os = "macos"))]` implementation and a `#[cfg(not(any(target_os = "linux", target_os = "macos")))]` fallback in `apps/litecoworkd/src/operator.rs` that returns 503 `UNAVAILABLE`; the route is mounted at the local-only `/__litecowork_local/workspace-root-selection` path.
- Windows primitives that exist and are reusable: `windows_local_security::open_directory_path_without_reparse`, `VerifiedDirectory::{identity, revalidate_path}`, `DirectoryIdentity::{volume_serial_number, file_id}` and `windows_absolute_paths_overlap` (ordinal case-insensitive, fails closed on UNC, drive-relative and noncanonical paths) in `crates/windows-local-security/src/lib.rs`. `apps/litecoworkd/src/local_filesystem.rs` already has `target_os = "windows"` branches for identity binding and projection.
- Windows scan: `scan_workspace_root` returns 501 under `#[cfg(not(target_os = "linux"))]`; the census helpers (`scan_linux_root`, `revalidate_scan_root_before_commit`, `finalize_scan_result`, `ensure_before_scan_deadline`) are all `#[cfg(target_os = "linux")]`.
- Send path: `ConversationTurnResourceRef { workspace_id, resource_id, revision_id }` is accepted and validated by `conversation_operator.rs` (at most 256 refs, same Workspace) and stored in `resource_refs` on the message; the OpenCode prompt encoder (`apps/litecoworkd/src/agents/conversation_prompt.rs`) refuses non-empty refs with `ResourceMaterializationRequired`. The composer in `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` sends `resourceRefs: []`, shows placeholder copy "@ to mention" and an "Add context or file" button with no handler.
- Retrieval: `rag_retrieval_operator.rs` serves `POST /v1/resources/retrieval` over `crates/domain-rag/src/local_lexical.rs` and `crates/storage-sqlite/src/rag_retrieval.rs`; no caller in the conversation path.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) and refresh URLs before lock.
- No native root selection, live scan, root content extraction or provider-backed Conversation was executed. Source inspection shows Windows add/scan are explicitly unavailable; the Linux scan path remains unqualified by a live USER run.

## Unresolved questions

See SPEC section 5 blockers. Owners are named on the package README.

## Current source cross-check — 2026-11-13

At source checkpoint `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`, the source review reconfirmed that Linux/macOS root add commits through the private authenticated local route while Windows returns 503; scanning is Linux-only and metadata-only; indexed Resource upload and read-only lexical retrieval remain separate from WorkspaceRoot ingestion; and retrieval is not called by Send. The existing Conversation attachment button loads one page of admitted text Resources and submits pinned refs, but does not implement the accepted `@` listbox interaction or first-message attachment. No persisted ContextAttachment/retrieval receipt or Context Used view exists.

The review also verified two open cross-owner design blockers rather than choosing between conflicting documents: A-65 (separate receipt/attachment transaction versus Feature 001's atomic turn/outbox admission) and A-69 (required `root_id`/`retrieval_receipt_id` in SPEC LLD versus optional fields in DATA-MODEL). Feature 003 T01, T09 and T10 must remain blocked as recorded in TASKS.md. Exact package lines/hashes and bounded source ranges are in [FEATURE-003-CURRENT-CROSSCHECK-2026-11-13.md](../../docs/audit/FEATURE-003-CURRENT-CROSSCHECK-2026-11-13.md).
