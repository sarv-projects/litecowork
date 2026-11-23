# 003 - Workspace Roots and Context Ingestion

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-13
**Implementation state (source cross-check 2026-11-13):** Selected CODE/SERVED foundations only. WorkspaceRoot list/pause/resume/revoke/scan and paginated metadata-observation routes exist; Linux root lifecycle storage tests pass. Native folder add is supported on Linux/macOS but fails closed on Windows; metadata scan is Linux-only and does not read/index content. Grounded lexical retrieval is served, but Send does not call it and no durable ContextAttachment/retrieval receipt or “Context Used” projection exists. T01 is BLOCKED on the owner decision in A-69; T02 and T08 are PARTIAL; T09/T10 are BLOCKED on A-65 and A-69; ingestion, indexing, attachment/retrieval integration, Context Debugger and qualification remain open. The current composer picker selects pre-existing, admitted text Resources from an existing Conversation using a button; it is not the accepted @-reference listbox and does not ingest WorkspaceRoot contents. Windows root operations require native Windows testing; migration v36 remains allocated and unwritten. See TASKS.md, docs/IMPLEMENTATION-STATUS.md and the [2026-11-13 source cross-check](../../docs/audit/FEATURE-003-CURRENT-CROSSCHECK-2026-11-13.md).
**Owners:** Resource, Conversation, Storage, Security/Trust, Desktop UI.
**Owner acceptance:** repository owner (sarv-projects), 2026-11-09, via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md). B-13 (UTF-8 lexical text set first) and B-14 (manual rescan, no V1 watcher) are accepted.
**Release target:** V1 release matrix rows Windows 11/10 `x86_64-pc-windows-msvc` with OpenCode v2.0.32 first; Linux x86_64 desktop second.

## User outcome

The owner attaches a folder as a persistent WorkspaceRoot, asks a grounded question with citations from exact revisions, and revokes the root so later retrieval is fenced.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. Owner acceptance authorizes implementation of this package; it does not change what the code does. At source checkpoint `6adadab539a62ca1c8b70621c7f88e1785cfe8c2` (cross-checked 2026-11-13), Windows root add returns 503 (`create_workspace_root_from_native_selection` fallback in `apps/litecoworkd/src/operator.rs`), scan returns 501 `SCAN_UNSUPPORTED_PLATFORM` on every non-Linux target (`apps/litecoworkd/src/workspace_root_scan_operator.rs`), and grounded retrieval (`apps/litecoworkd/src/rag_retrieval_operator.rs`) is not called by Send and writes no receipt. The current composer picker loads one page of admitted text Resources through an **Add context or file** button for an existing Conversation, then submits exact revision-pinned `resourceRefs`; it does not respond to typing `@`, attach before a Conversation exists, or implement the accepted listbox/search/arrow-key/count behavior. Feature 001 materializes eligible text at dispatch. That is distinct from ingesting files under WorkspaceRoots. There is no persisted ContextAttachment or Context Used projection. A-65 and A-69 remain open; see the source cross-check and TASKS.md before touching receipt/schema work.

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
