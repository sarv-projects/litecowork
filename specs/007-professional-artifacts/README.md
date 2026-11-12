# 007 - Professional Artifacts One Format

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-12
**Implementation state (2026-11-12):** Partial foundations only. Existing Artifact read/version/Library routes, bounded `text/plain` append editing and native Save As remain. Comrak 0.59.0 is pinned, but no Markdown format adapter/editor, production Artifact creation/provenance route, durable export receipt, Task-generated source, or packaged authoring/export journey exists. T12 remains blocked for Task-generated Artifacts; T06 is blocked on A-81's receipt/effect ordering and local-export recovery review. Migration v37 is allocated and unwritten. See TASKS.md and docs/IMPLEMENTATION-STATUS.md.
**Owners:** Artifacts, Presentation, Storage, Desktop UI.
**Owner acceptance:** repository owner (sarv-projects), 2026-11-09, via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md). Accepted: B-21 Markdown (`text/markdown`) first with no Office engine, B-30 `POST /v1/artifacts` create, B-31 `ArtifactExportReceipt` distinct from native Save As.
**Release target:** V1 release matrix rows Windows 11/10 `x86_64-pc-windows-msvc` first; Linux x86_64 desktop second.

## User outcome

The owner creates, revises, exports and reopens one professional deliverable (a Markdown document) and can see provenance, the source Conversation and version differences. Other formats are honestly read-only or unsupported.

## Current truth

The active checkout has an Artifact read/Library/version model and a narrow managed `text/plain` append editor (`apps/litecoworkd/src/artifact_operator.rs`, `apps/litecowork-ui/src/artifacts/`). It has no production Artifact create command, no Markdown editor, no durable export-receipt flow, no declared capability matrix and no Conversation association (`ArtifactRecord` carries `task_id` only). Previewing Markdown/CSV/JSON is not editing those formats; uploading DOCX/XLSX/PPTX/PDF creates a Resource, not an Artifact. Native Save As (`artifact_save_as`, `resource_save_as`) is an exact-version local file save, not the export receipt. Acceptance authorizes implementation and does not change these facts; see SPEC.md §4 and docs/IMPLEMENTATION-STATUS.md.

The accepted B-31 destination class is known, but SPEC §7's receipt-before-external-call sentence conflicts with the HLD/API/T06 write-before-receipt path. The local write can also complete before its receipt, with no complete restart/user-recovery contract. T06 is held under A-81 until ordering and recovery are reviewed and approved; a missing receipt must not be described as proof that no file was written.

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Dependencies

Feature 003 (Resource admission for the Imported source class), Feature 001 (committed messages for Conversation association), Feature 005 (Task-generated outputs only; the user-authored Markdown path is independent), Feature 014 (Conversation delete fences on associated Artifacts), Feature 016 (rich presentation fallback).

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Specification standard](../../docs/SPECIFICATION-STANDARD.md). Domain narrative: [RESEARCH.md](RESEARCH.md).
