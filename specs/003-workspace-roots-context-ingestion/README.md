# 003 - Workspace Roots and Context Ingestion

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-11
**Implementation state (2026-11-11):** Selected CODE/SERVED foundations only. WorkspaceRoot list/pause/resume/revoke/scan and paginated metadata-observation routes exist; Linux root lifecycle storage tests pass. Native folder add is supported on Linux/macOS but fails closed on Windows; metadata scan is Linux-only and does not read/index content. Grounded lexical retrieval is served, but Send does not call it and no durable ContextAttachment/retrieval receipt or “Context Used” projection exists. T08 is PARTIAL; T02 is PARTIAL; ingestion, indexing, attachment/retrieval integration, Context Debugger and qualification remain open. Windows root operations require native Windows testing; migration v35 remains allocated and unwritten. See TASKS.md and docs/IMPLEMENTATION-STATUS.md.
**Owners:** Resource, Conversation, Storage, Security/Trust, Desktop UI.
**Owner acceptance:** repository owner (sarv-projects), 2026-11-09, via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md). B-13 (UTF-8 lexical text set first) and B-14 (manual rescan, no V1 watcher) are accepted.
**Release target:** V1 release matrix rows Windows 11/10 `x86_64-pc-windows-msvc` with OpenCode v2.0.32 first; Linux x86_64 desktop second.

## User outcome

The owner attaches a folder as a persistent WorkspaceRoot, asks a grounded question with citations from exact revisions, and revokes the root so later retrieval is fenced.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. Owner acceptance authorizes implementation of this package; it does not change what the code does. On the 2026-11-12 checkout, Windows root add returns 503 (`create_workspace_root_from_native_selection` fallback in `apps/litecoworkd/src/operator.rs`), scan returns 501 `SCAN_UNSUPPORTED_PLATFORM` on every non-Linux target (`apps/litecoworkd/src/workspace_root_scan_operator.rs`), and grounded retrieval (`apps/litecoworkd/src/rag_retrieval_operator.rs`) is not called by Send and writes no receipt. The composer can select existing Resources and send revision-pinned `resourceRefs`; Feature 001 materializes eligible text at dispatch. That is distinct from ingesting files under WorkspaceRoots. There is no persisted ContextAttachment or Context Used projection. The implementation boundary is mapped in SPEC.md §4 and docs/IMPLEMENTATION-STATUS.md; do not infer qualification from a matching route, type or screen.

By owner approval on 2026-11-09 (see [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md)), this package folds proposed task P-15 (003-T17, Context Debugger surface showing exactly what entered model context). Source: [docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](../../docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) section 8.

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Dependencies

Feature 001 owns Send, the composer and ResourceRef materialization (B-08); this package owns WorkspaceRoots, ContextAttachments, retrieval receipts, Context Used and the ResourceRef picker it surfaces in the 001 composer. Feature 011 owns the Tauri shell and the Windows named-pipe Operator IPC. Feature 014 consumes the retrieval fences on Conversation delete.

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Specification standard](../../docs/SPECIFICATION-STANDARD.md). Domain narrative: [RESEARCH.md](RESEARCH.md).
