# 016 - Intelligent Workspace Presentation Runtime

- **Status:** IN_REVIEW - revision 0.4 is a TARGET proposal. The owner directed Saved Views to remain in V1 and Feature 007 B-21 to be amended; cross-owner review remains open. T01 must still record explicit owner acceptance of the complete revised V1 scope before implementation.
- **Revision:** 0.4 (proposed) | **Updated:** 2026-11-17 | **Decision:** B-40 remains accepted as proposed (2026-11-10), preserving bounded SAFE_CALCULATOR, transient-only changed values, no arbitrary model HTML/JS, and PREVIEW_ONLY executable Artifacts. Saved Views preserve declared defaults but not session-modified values. The former 0.2 extension is incorporated into the revised V1 proposal only where its new cross-owner gates are accepted.
- **Implementation state (source rechecked 2026-11-17):** Partial CODE/SERVED foundations. Desktop and storage validators agree on five block kinds and a 512 KiB cap; semantic fallback and a read path exist. No production publisher, global preference/fallback control, chart/calculation renderer, generated-media provenance flow, Saved View lifecycle, or qualified Interactive Artifact Runtime exists. Feature 007 first-Artifact creation is not served. This revision changes TARGET design only; no code or route was added. See TASKS.md and docs/IMPLEMENTATION-STATUS.md.
- **Owners:** Presentation Runtime, Conversation, Artifacts, Storage, Operator/API, Desktop UI, Security, Accessibility and Product.
- **Release rows:** Windows x64 with OpenCode v2.0.32 first, Linux x64 second. No Environment provider is needed.

## User outcome

LiteCowork starts with the user's selected native agent and presents its answer as readable conversation, useful structured content, or a safe interactive view when the committed answer supports one and the accepted profile enables it. An eligible view can expand into the contextual Workbench Canvas without changing its message identity or current in-session values. The owner directed Saved Views to remain in V1 and Feature 007 B-21 to be amended. After cross-owner acceptance and implementation, the user can save a supported view into Library, reopen its exact version and append a reviewed new version from a new committed presentation; changed calculator values remain transient under B-40. Unsupported agents and malformed proposals still produce a complete readable semantic answer. No generated executable code runs in the chat WebView; executable Artifact Runtime remains separately gated and PREVIEW_ONLY in V1.

## Current truth

The desktop validator (`apps/litecowork-ui/src/presentation/rich-presentation.ts`, 512 KiB, 200 blocks, depth 8) and the Rust storage validator (`crates/storage-sqlite/src/rich_presentations.rs`) accept the same five V1 block kinds (TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE) under the same 512 KiB cap, and the served wire schema carries that cap too. Nothing in production publishes a RichPresentation. There is no chart, image or Intelligent UI renderer, no media ingestion, no disable preference and no Interactive Artifact Runtime code. The read route and the semantic fallback view exist.

Under this package's IN_REVIEW status, proposed task P-03 is folded in as 016-T18: per-block Markdown degradation with GFM tables, horizontal rules and nested lists; code blocks with a language label, copy button and lazy highlighting; TEXT_SLICE rendered through the same renderer; and a golden corpus of real agent answers that renders with no whole-message raw fallback and no HTML strings. Source: [docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md](../../docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md) section 9.

## Package

- [SPEC.md](SPEC.md) - problem, decisions (B-40), HLD/LLD, security, EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency graph, gates, risks.
- [TASKS.md](TASKS.md) - ordered tasks to VERIFIED for V1.
- [DATA-MODEL.md](DATA-MODEL.md) - records, lifecycles, bounds.
- [RESEARCH.md](RESEARCH.md) - pinned references (OpenIntelligentUI) and decisions.
- [WIRING.yaml](WIRING.yaml) - components, edges, transactions, boundaries.
- [contracts/](contracts/README.md) - API/events, errors/recovery, UI behavior.

## Dependencies

Feature 001 (committed semantic message, ordinary Send and producer/lineage identity), Features 003/006 only for exact authorized sources, Feature 007 (Artifact create/version/Library lifecycle and proposed Saved View compatibility), Feature 008 (contextual Workbench Canvas and provider ownership boundaries), Feature 011 (CSP, display preferences and authenticated IPC), Feature 014 (Conversation deletion/retention), Feature 017 (backup/restore), Feature 019 (CSP audit and release rows). A package dependency is not evidence that its route is served or owner review is complete.

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Rich Response](../../docs/RICH-RESPONSE.md), [Presentation Runtime](../../docs/PRESENTATION-RUNTIME.md), [Intelligent UI and image content](../../docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md), [Interactive Artifact Runtime](../../docs/INTERACTIVE-ARTIFACT-RUNTIME.md), [Rendering conformance](../../docs/RENDERING-CONFORMANCE-LLD.md), [Artifacts and evidence](../../docs/ARTIFACTS-EVIDENCE.md), [Schemas](../../docs/SCHEMAS.md).

## Revision 0.4 design review

The proposal treats semantic answers, registered interactive views, expanded Canvas and Saved Views as one conversation-first experience with staged delivery. The same canonical RichPresentation document remains the only UI tree; provider adapters may supply optional structured intent, while deterministic host formatting and the complete semantic answer serve routes without that capability. The owner direction is recorded; cross-owner review of Feature 007's Artifact contract, T01 whole-scope acceptance and T02 contract promotion remain open. See SPEC §20, PLAN, TASKS and the ACP presentation pipeline LLD. This is a design revision only: no Feature 016 code or Saved View route is implemented.
