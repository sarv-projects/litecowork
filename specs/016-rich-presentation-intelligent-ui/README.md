# 016 - Rich Presentation, Intelligent UI and Generated Media

- **Status:** IN_REVIEW - baseline AC-001..AC-019 awaits owner acceptance at T01; revision 0.2 extension AC-020..AC-027 needs a separate owner decision at T25. B-40 is accepted as proposed only and does not accept either full scope or authorize implementation.
- **Revision:** 0.2 (proposed) | **Updated:** 2026-11-12 | **Decision:** B-40 (first Intelligent UI component and Interactive Artifact Runtime V1 posture) ACCEPTED as proposed 2026-11-10 (A-62); baseline acceptance is pending at T01, and the section 19 extension requires a separate T25 decision (A-84).
- **Implementation state (2026-11-12):** Partial CODE/SERVED foundations. Desktop and storage validators agree on five block kinds and a 512 KiB cap; semantic fallback and a read path exist. No production publisher, global preference/fallback control, chart/calculation renderer, generated-media provenance flow, or Interactive Artifact Runtime is wired. Package remains IN_REVIEW; baseline work is gated on T01 and revision 0.2 extension work on T25, canonical contract promotion and its own task authorization. See TASKS.md and docs/IMPLEMENTATION-STATUS.md.
- **Owners:** Presentation Runtime, Conversation, Artifacts, Storage, Operator/API, Desktop UI, Security, Accessibility and Product.
- **Release rows:** Windows x64 with OpenCode v2.0.32 first, Linux x64 second. No Environment provider is needed.

## User outcome

Answers can show tables, a chart with an equivalent data table, and one accessible "what-if" calculator built from the answer's own numbers. Turning rich presentation off, or any validation failure, shows the full original text. Generated images become Library Artifacts with their source Conversation and turn. Interactive app Artifacts open as an inert preview; nothing runs in the chat WebView.

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

Feature 001 (committed semantic message), Feature 007 (Artifact create and Conversation association), Feature 011 (CSP, display preferences, IPC), Feature 019 (CSP audit, release rows).

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Rich Response](../../docs/RICH-RESPONSE.md), [Presentation Runtime](../../docs/PRESENTATION-RUNTIME.md), [Intelligent UI and image content](../../docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md), [Interactive Artifact Runtime](../../docs/INTERACTIVE-ARTIFACT-RUNTIME.md), [Rendering conformance](../../docs/RENDERING-CONFORMANCE-LLD.md), [Artifacts and evidence](../../docs/ARTIFACTS-EVIDENCE.md), [Schemas](../../docs/SCHEMAS.md).

## Revision 0.2 design review

The requested declarative interaction integration is documented in [ACP presentation pipeline LLD](../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) and SPEC section 19. Proposals: qualified native intent ingress (ACP is not the UI protocol), registered deterministic components, one state controller across inline/expanded Workbench Canvas, separately opt-in device-local input retention and explicitly previewed new-turn conversational modification. No second rendering LLM, arbitrary model code, provider/config rewrite or replacement stack. Existing B-40 state remains view-local. T01 accepts only baseline AC-001..AC-019; T25 is the separate owner gate for the extension; T20-T24 remain BLOCKED. A-85 also keeps the proposal tool unavailable until its canonical MCP input/sequence/replay contract is settled. Original T01-T24 IDs are retained; T25 is appended as a decision gate.
