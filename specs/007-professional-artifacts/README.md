# 007 - Professional Artifacts One Format

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.3 | **Updated:** 2026-11-06
**Owners to approve:** Artifacts, Presentation, Storage, Desktop UI.

## User outcome

The owner creates, revises, exports and reopens one professional deliverable and can see provenance and version differences.

## Current truth

The active checkout has an Artifact read/Library/version model and a narrow managed `text/plain` append editor. It has no production Artifact create command or durable export-receipt flow, and no qualified professional document/Office editor. Previewing Markdown/CSV/JSON is not editing those formats; uploading DOCX/XLSX/PPTX/PDF creates a Resource, not an Artifact. Native Save As is an exact-version local file save, not the target format-export receipt. This package remains DRAFT; its source/route/UI boundary is mapped in SPEC.md, RESEARCH.md and docs/IMPLEMENTATION-STATUS.md.

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Specification standard](../../docs/SPECIFICATION-STANDARD.md). Domain narrative: [RESEARCH.md](RESEARCH.md).
