# Repository reading map (derived research notes)

This directory contains reading digests, file inventories and working notes produced during repository inspection. It is a **derived research aid**, not a normative specification, current release declaration, source inventory or qualification record.

The active authority is:

1. Root ARCHITECTURE.md for system ownership and invariants.
2. docs/PRODUCT.md and docs/V1-PRODUCT-RELEASE-CONTRACT.md for product promise and release scope.
3. The owning domain documents and accepted ADRs.
4. docs/schemas/ for machine contracts.
5. docs/V1-UI-SCREEN-CONTRACT.md, V1-UI-ACTION-STATE-LEDGER.md and DESKTOP-SHELL-LLD.md for UI target behavior.
6. docs/IMPLEMENTATION-STATUS.md for source-backed implementation and release evidence.
7. docs/schemas/served-route-inventory.json for registered production route composition.
8. docs/ACCEPTANCE-AND-E2E-PLAN.md for exact release qualification.

Do not use this directory to decide whether a route is served, a feature is integrated, a control is enabled, a package is installable, a provider is qualified or a release is ready. Recheck the active source and canonical owner documents.

## Active repository shape

| Path | Role |
|---|---|
| apps/litecoworkd | Rust daemon and local Operator Runtime |
| apps/litecowork-ui | Tauri v2 / React desktop application and bridge |
| crates | Domain, storage, IPC, lifecycle, resource, environment and security crates |
| capabilities | Provider/capability slices, including bounded ZIP intake |
| docs | Product, architecture, domain owners, ADRs, machine schemas, UI contracts and evidence |
| specs | Feature SDD packages 001–011 |
| .specify | SDD templates and structural validation |
| scripts | Source-route/documentation/contract audit tools |
| repo_take | Pinned external reference repositories; research only |
| docs/archive and archives | Historical material; not current implementation truth |

## How to use the digests

- digest-docs-* summarize selected normative documents at the time they were read. They may describe older sections, dates or source status.
- digest-daemon-* and digest-crates-* summarize source modules. Verify every finding against the current file before using it.
- digest-ui-* summarize UI source and command wiring. Verify the exact component and Tauri command before using it.
- digest-specs-* summarize feature packages. The package's current README/SPEC/contracts and V1-FEATURE-SDD-REGISTER own current decisions.
- digest-archive-* describe historical archives and must not be used as active source.
- inventory.tsv and contracts-inventory.md are navigation/inspection aids, not qualification.

A digest is not automatically refreshed when a source file changes. A disagreement is resolved by the active source and canonical owner contract, not by the digest's timestamp or wording.

## Current source-of-truth links

- Product: ../docs/PRODUCT.md
- Architecture: ../ARCHITECTURE.md
- Implementation status: ../docs/IMPLEMENTATION-STATUS.md
- Screen/interaction contract: ../docs/V1-UI-SCREEN-CONTRACT.md
- Feature register: ../docs/V1-FEATURE-SDD-REGISTER.md
- Served route inventory: ../docs/schemas/served-route-inventory.json
- Documentation audit: ../docs/REPOSITORY-DOCUMENTATION-AUDIT.md
- Current work handoff: ../CURRENT-RUN.md

## Reading completeness boundary

A digest inventory can establish which files were inspected, not that every statement was semantically verified or that the system passed an end-to-end test. Product and release decisions require the named owner, current source, machine contract and executable CODE/SYSTEM/USER evidence.
