# Intelligent UI, declarative presentation and image content

**Authority:** Subordinate to [Architecture](../ARCHITECTURE.md), [Rich Response](RICH-RESPONSE.md), [Presentation Runtime](PRESENTATION-RUNTIME.md), [State Machines](STATE-MACHINES.md) and [Artifacts and Evidence](ARTIFACTS-EVIDENCE.md). Existing owner contracts remain normative; Feature 016 revision 0.4 is a TARGET proposal, IN_REVIEW. The owner selected Saved Views for V1 and directed a Feature 007 B-21 amendment; its bounded Artifact contract is drafted for cross-owner review. This product direction does not accept the full Feature 016 scope or authorize implementation or qualification.

## 1. Output boundaries

| Class | User outcome | State and authority |
|---|---|---|
| Semantic response | Complete readable conversational answer | Immutable committed ConversationMessage; authoritative |
| RichPresentation | Optional cards, tables and layout | Immutable bounded projection, one record per message; never host truth |
| Generated image | Inspect and keep an image | Exact authorized ArtifactVersion/Resource revision and provenance |
| Declarative Intelligent UI | Explore numbers using registered controls | Accepted B-40 target: bounded SAFE_CALCULATOR with transient view-local state |
| Interactive Artifact | Operate a generated application | Separate [isolated runtime](INTERACTIVE-ARTIFACT-RUNTIME.md); B-40 keeps V1 PREVIEW_ONLY |

Do not execute arbitrary model HTML, JavaScript, CSS or SVG in the chat WebView. Native harnesses retain reasoning, authentication, model catalogs and tools. LiteCowork supplies no hidden rendering LLM or second agent loop. Rich output is optional; facts, formula assumptions, warnings, citations and deliverables remain readable in the semantic answer.

## 2. One schema and one compilation path

[presentation-intent.schema.json](schemas/presentation-intent.schema.json) owns agent proposals; [rich-presentation.schema.json](schemas/rich-presentation.schema.json) owns final documents; [operator-stream.schema.json](schemas/operator-stream.schema.json) owns stream frames. There is no separate IntelligentUiDocument, alternate node union or local stream-frame schema. The former illustrative type listing is removed because it duplicated machine authority and implied unadmitted components.

Selected native agent output -> qualified AgentModule correlation/normalization -> bounded turn-scoped intent -> semantic message commit -> deterministic binder outside that transaction -> Rust validator/source authorization -> immutable publication -> authenticated Operator/Tauri read -> defensive desktop validation -> registered React component or full semantic fallback.

Detailed HLD, LLD, ingress, tool contract, metrics, state and recovery: [ACP presentation pipeline](ACP-PRESENTATION-PIPELINE-LLD.md). ACP is an agent session protocol, not a UI protocol. Use documented native structured output when supported; an optional scoped local presentation MCP proposal tool requires verified session injection support; otherwise bounded semantic text works. A native `tool_call` observation is never an arbitrary executable host request. Host guidance remains [zero-authority](HOST-GUIDANCE.md).

Current intent schema does not express a calculator. B-40 calculation intent must be promoted in the existing schema with its validation and semantic bindings before use; never claim a nonexistent CALC variant. CHART_FROM_RESULT requires a real authorized invocation; Markdown table charts are deterministic host derivation, not fictitious tool results.

## 3. Registered deterministic interaction

The current implemented profile accepts TEXT_SLICE, LAYOUT, CARD, CALLOUT and TABLE under 512 KiB/200-block/depth-8 limits. Accepted B-40 defines CHART and SAFE_CALCULATOR as the first renderer-v2 target, pending its owner-reviewed contract promotion. Revision 0.4 proposes METRIC plus bounded NUMBER/RANGE/SELECT controls only after B-46/T01/T02 accept and promote them. Tabs, generic forms, arbitrary operations, maps, remote assets and plugin renderers are not admitted merely because another library offers them.

SAFE_CALCULATOR has at most eight labeled numeric inputs bound to original semantic table cells, a restricted arithmetic AST of at most 64 nodes/depth 12, a formula explanation and original-value fallback. Allowed arithmetic is B-40's LITERAL/VARIABLE, `+ - * / %`, unary minus, MIN/MAX/ROUND/ABS/CLAMP; non-finite or divide-by-zero results show a bounded error and fallback. No eval, function loading or universal string operation dispatcher. Derived outputs recompute locally and never become Evidence or verified financial results.

Metric bindings must declare measure, units/currency/scale, time interval and aggregation. Latest and total are distinct. The proposal's monthly revenue example totals INR 12.08 lakh; INR 3.15 lakh is June revenue; 162.5% is January-to-June change. Missing semantics yields a table or clarification. New SIP numerical kernels and INR locale formatting require reviewed rate conversion, contribution timing, compounding, zero-rate/domain/stability/rounding semantics and fixtures; they are not existing approved arithmetic.

## 4. Inline, Workbench Canvas and Saved Views

Canvas is an expanded presentation in the existing right Workbench. A host-owned Expand/Collapse control keeps the same validated document, controller, inputs and revision; it does not invoke an agent or run an Artifact. Return focus to the originating control; support keyboard, screen reader, high zoom/reflow, reduced motion, labeled inputs, polite result announcements and equivalent data tables. Registry failures preserve the complete original answer.

B-40 keeps calculator input transient and prevents the component from submitting or persisting it. Proposed B-46 separately asks whether the closed profile may add metrics and bounded input controls, and whether host controls can **Remember on this device** or **Ask to change this view** after explicit consent/review; none of that behavior is accepted yet. Remember would use a quota-limited exact Workspace/Conversation/message/presentation-digest partition through authenticated host commands, expected revision, durable receipts and deletion fences. No browser storage or background model synchronization. Conversational modification would preview and send only user-authored text and any separately approved context as a new normal turn; under current B-40, changed calculator values are excluded. A result creates a new message/presentation and never rewrites original semantic text. The owner selected Saved Views for V1 and directed Feature 007 B-21 to be amended. The drafted Artifact contract stores the immutable RichPresentation and its declared defaults, not session-modified values; cross-owner review and T01/T02 promotion remain pending. See Feature 016 SPEC §20 and its T20-T25 gates.

## 5. Generated media

The native harness may deliver an admitted Workspace file. Daemon ingestion sniffs PNG/JPEG/WebP/GIF magic bytes, checks declared type, limits bytes to 20 MiB and dimensions to 16,384 per side, computes SHA-256, and creates a provenance-bearing ArtifactVersion through Feature 007. Provenance pins Conversation, turn, public AgentSession identity and bytes digest. SVG is rejected in V1; no sanitizer is qualified. Model URLs are not fetched, and URL-only output is not an ingested Artifact.

Artifact bytes load only through an exact authorized read. A new `litecowork-resource:` scheme is not implemented or promised; Feature 011 owns any CSP/scheme decision and Feature 019 audits it. Image display never autoplays and includes alt text or an explicit missing-description label. Composer image input is a separate descriptor-qualified input path; generated-image support does not prove image-input support.

## 6. Evidence and gates

Partial CODE/SERVED validator/read/fallback foundations exist. No production proposal publisher, calculation/chart renderer, expanded state controller, persistent input store, Saved View lifecycle or image ingestion is claimed. The right-side Canvas and Save/Remember controls are design proposals, not current UI. Saved Views are selected product direction with a TARGET owner contract, not a served feature. Existing stream ready/projection/resync producers are distinct from absent turn-delta/settled/rich-draft producers. [Implementation status](IMPLEMENTATION-STATUS.md) and Feature 016 TASKS record exact evidence.

Required checks include shared Rust/TypeScript hostile and boundary corpus, no executable content/CSP, wrong-scope/retry/proposal rejection, truthful units/aggregation, pure numerical fixtures, controller expansion identity, persistence consent/quota/revocation, duplicate/changed-body receipts, timeout-after-commit reconciliation, conflicting windows, forget/save race, new-message modification lineage, restart/reconnect and accessible semantic fallback. Packaged SYSTEM and owner USER evidence are required per exact OS/WebView/provider/version/auth route; schema or renderer success alone is not QUALIFIED.

## 7. Detailed fallback, transport and image-input preservation

These details retain the useful behavior of the earlier architecture while removing its duplicate TypeScript/stream definitions. Component vocabulary describes targets; only the explicitly admitted V1 profile is renderable.

| Proposed family | Accessible deterministic fallback | Admission boundary |
|---|---|---|
| Layout | Vertical ordered stack | Existing LAYOUT; bounded columns/gap tokens only |
| Tabs/accordion | Labeled sequential sections/headings | Future registry/schema review, no current component promise |
| Data explorer | Accessible table with bounded scroll region | Existing TABLE is read-only; sorting/filter/pagination/export need registered host behavior |
| Metrics | Label, value, unit, interval and aggregation text | Proposed only under B-46; require canonical schema/renderer promotion and cannot infer success or verification from color/badge |
| Chart | Equivalent data table and textual accessible summary | B-40 BAR/LINE/AREA target; original numbers retained |
| Inputs/calculator | Original values, formula and original result; error reason for invalid inputs | B-40 labeled numeric inputs only; required deterministic fallback |
| Image | Alt text or explicit missing-description label, exact authorized Artifact open/save controls | Feature 007; no model-generated direct download URL |
| MCP App | Ordinary bounded tool/resource result with unavailable reason | Separate sandbox/origin/capability contract, never native chat execution |
| Callout | Labeled text/blockquote | Existing CALLOUT with semantics bound to message |

A component exception is caught by a host ErrorBoundary, leaves the complete semantic answer visible and may announce a concise display fallback once. It never changes turn state. ARIA state matches the actual control (`aria-expanded`, `aria-selected`, `aria-controls` where applicable); keyboard operation includes Tab/Space/Enter/Arrow keys according to the admitted control pattern. Charts use labels/patterns rather than color alone, visible text contrast is at least 4.5:1, and reduced motion disables optional animation. Determinism concerns document validation, numerical state transitions and equivalent readable results on supported Windows/Linux profiles, not identical browser pixel geometry or an unsupported macOS qualification claim.

Canonical stream behavior remains in PRESENTATION-RUNTIME and operator-stream.schema.json: native text is coalesced/throttled (target at most 60 frames/second), draft frames are ephemeral, and turn-settled follows semantic commit only. Draft fences bind Workspace/Conversation/turn/retry/AgentSession/draft; stale or duplicate sequence does not apply. Existing client bounds include 32 open blocks, depth 8, 512 KiB draft bytes, 1,000 frames, 200 total blocks and 1 MiB frame cap. Queue overflow/gap drops optional draft and requests canonical resync; committed message replaces transient text. No producer/display implementation is claimed, and no custom `OperatorStreamFrame` union is introduced here.

Image input remains a separate TARGET journey: native picker -> Tauri bounds/header validation -> admitted Resource storage -> exact revision reference -> adapter projection only when fresh AgentControlDescriptor reports `input.images=SUPPORTED` for that route. The actual provider must document how bytes or file references are supplied; do not invent a universal CLI flag or assume base64 support. Unsupported input shows an explicit capability limitation and asks the owner whether to send only a Workspace reference; it does not imply the agent saw image pixels. Retrying an image read retries only the authorized read, never generation. Inline zoom/lightbox/inspection and export are host controls over exact authorized revisions; they do not confer file-write/export authority outside Feature 007. No media scheme, sanitizer, image-input route or renderer is qualified by this prose.
