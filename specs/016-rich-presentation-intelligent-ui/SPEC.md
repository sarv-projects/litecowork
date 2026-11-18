# Feature Specification - Intelligent Workspace Presentation Runtime

- **Feature ID:** 016 | **Status:** IN_REVIEW | **Revision:** 0.4 (proposed)
- **Owner acceptance:** revised V1 scope AC-001..AC-041 is a TARGET proposal and requires explicit owner acceptance at T01. B-40 was accepted as proposed on 2026-11-10; it preserves bounded SAFE_CALCULATOR with transient-only values and the PREVIEW_ONLY executable Artifact posture. On 2026-11-17 the owner directed that Saved Views remain in V1 and Feature 007 B-21 be amended. The Saved View contract is drafted, but cross-owner review and T01/T02 remain open. Nothing in this revision authorizes implementation.
- **Change control:** after acceptance this becomes a locked contract; any later change needs a new revision and renewed owner approval (SPECIFICATION-STANDARD section 7).
- **Target:** LiteCowork V1 local desktop: master plan P10 "Intelligent UI and media", build tasks BT-06.3, BT-06.4 and BT-06.5, E2E-11 "Rich UI response with presentation disabled", release contract sections 3-4 "product areas" and "Workbench and deliverables". The revised proposal extends the conversational answer into a bounded interactive view, a contextual Canvas and a Library-backed Saved View; it does not turn LiteCowork into a hosted model router or executable app platform.
- **Release rows:** Windows 11/10 `x86_64-pc-windows-msvc` with OpenCode v2.0.32 first, Linux x86_64 second. Nothing in this package needs an Environment provider, so both rows can qualify; the Interactive Artifact Runtime stays PREVIEW_ONLY on both rows in V1.
- **Owners:** Presentation Runtime, Conversation, Artifacts, Storage, Operator/API, Desktop UI, Security, Accessibility, Product.
- **Dependencies:** Feature 001 (committed semantic AGENT message is authoritative; ordinary Send and producer identity), Feature 003/006 only for exact authorized data sources, Feature 007 (Artifact create/version/Library lifecycle and Saved View compatibility), Feature 008 (contextual Workbench Canvas), Feature 011 (CSP, display preferences and authenticated IPC), Feature 014 (Conversation retention/deletion), Feature 017 (backup/restore), Feature 019 (CSP audit and release rows).

> **Revision authority:** Sections 1-18 retain the earlier technical baseline and AC-001..AC-019 for history and traceability. Section 19 is historical. Section 20 is the current revision 0.4 TARGET proposal and supersedes earlier text where scope conflicts; it does not override global owner contracts or authorize code. The owner has decided to keep Saved Views in V1; T25 cross-owner review and T01 whole-scope acceptance must close before T02 promotes the full profile.

## 1. Problem and user outcome

LiteCowork's product direction is one local, conversation-first workspace that can present supported work as readable answers, analysis, deliverables and safe interactive views. The current RichPresentation is an immutable, message-bound display projection, but the runtime is not yet an end-to-end experience: validators accept five block kinds, while no production publisher exists and chart, calculator, saved-view and Canvas paths are not integrated. The design must serve native agents that only return text as well as exact routes that can provide qualified structured intent. It must keep semantic content authoritative, map all optional structure into one validated host-owned format, and preserve a complete answer if any presentation path is missing, disabled, invalid or interrupted.

**Outcome:** the user can move from a readable answer to a registered interactive view and expand that same view into the right-side Workbench Canvas. When a committed response has grounded data, LiteCowork can present accessible tables, accurately labelled metrics, charts with data-table fallback and bounded local calculations. The proposal adds separate explicit controls to remember inputs on this device, save a reusable view to Library, reopen its exact version, update by appending a reviewed version, and ask for a conversational change through an ordinary new turn. Those host actions are not component actions and require renewed owner decisions because accepted B-40 currently keeps calculator state transient. The semantic response remains complete. No hidden model call or arbitrary generated code is used; executable custom applications remain outside Feature 016 V1 and the separate Artifact Runtime remains PREVIEW_ONLY.

## 2. Goals

- G-01 One server-side Rust validator owns the RichPresentation v1 bounds and block allowlist; the TypeScript validator mirrors it exactly and both pass one shared golden corpus.
- G-02 The proposed renderer profile is closed: TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE, METRIC, CHART, SAFE_CALCULATOR and explicitly admitted local input bindings. B-46 and T02 must promote the exact profile; every unknown kind, field or binding fails closed to semantic text.
- G-03 Calculations use only a restricted arithmetic AST validated in Rust at publish time and evaluated by a pure TypeScript reducer over local state. Explicit remembered-state or user-reviewed send actions are separate host-owned operations, never implicit component behavior.
- G-04 A CHART is bound to a semantic-message table, drawn by host React components as SVG elements, and always exposes an equivalent accessible data table.
- G-05 A host-side binder publishes RichPresentations after the semantic message commits, outside the commit transaction; presentation failure never blocks or changes turn settlement.
- G-06 The owner can disable rich presentation; the semantic answer remains complete and readable (E2E-11).
- G-07 Generated media is stored as a provenance-bearing Artifact (Conversation, turn, AgentSession, digest) through the Feature 007 create path, never fetched from a model-supplied URL.
- G-08 Interactive Artifacts open PREVIEW_ONLY and inert in V1 until E08-S09 CODE, SYSTEM and owner USER evidence exists for the exact OS and Runtime profile.
- G-09 No executable model content reaches the chat WebView: no eval, no script, no model HTML, CSS or SVG markup, no `dangerouslySetInnerHTML` on agent output, CSP `script-src 'self'` unchanged.
- G-10 An eligible presentation can expand into the existing contextual Canvas with exact message/document/controller identity; no new permanent specialist navigation tab is added.
- G-11 The owner selected Saved Views for V1; save/open/update/archive use the proposed Feature 007 immutable-version and durable-receipt lifecycle after cross-owner review and contract promotion. Saved answer state is not an executable app or a second schema.
- G-12 Native agents are optional proposal sources, not UI contract authorities. Unsupported or unqualified ingress uses deterministic host formatting or semantic-only fallback; ACP alone implies neither a UI tree nor child-agent feed.

## 3. Non-goals

No arbitrary generated HTML, JavaScript, CSS, model SVG, code execution, or generic model-authored host action is admitted. No Interactive Artifact execution or arbitrary sandbox-generated mini-apps are included in this Feature 016 V1 proposal; the Interactive Artifact Runtime remains a separate PREVIEW_ONLY owner path. No schema-declared block becomes supported until validators, renderer, fallback and evidence are promoted together. Host-owned Save, Forget, Expand, Send, Update and Library controls are separate typed owner operations, never model-authored actions. Generated media, Artifact creation and durable saved-view data use Feature 007 and other existing owners; Feature 016 does not duplicate their stores or lifecycle.

## 4. Current implementation/evidence boundary

Source claims rechecked against the current working tree based on `main` HEAD `f146bb3a187449d5348c5bf2e380c122c6d290c0`, 2026-11-17. The rich-presentation validator/store/reader and renderer were inspected directly; no Feature 016 runtime implementation was added during this review. The separate Feature 002 Tauri bridge change is tracked in `CURRENT-RUN.md`. B-40's accepted baseline has no migration and keeps calculator input state transient; revision 0.4 keeps any separately remembered-input record gated on T22 allocation and owner approval.

| Capability | Current evidence | Limitation |
|---|---|---|
| Desktop validator | CODE: `apps/litecowork-ui/src/presentation/rich-presentation.ts` (`parseRichPresentationDocument`, `parseRichPresentationResponse`, `resolveRichResponseContent`, `isValidatedRichPresentation`); constants `MAX_DOCUMENT_BYTES = 512 * 1024`, `MAX_SEMANTIC_BYTES = 1024 * 1024`, `MAX_BLOCKS = 200`, `MAX_DEPTH = 8`, `MAX_JSON_VALUES = 30_000`, `MAX_JSON_DEPTH = 32`; accepts TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE; RFC 8785 canonical bytes and SHA-256 checks; any failure returns null | Charts, media, host projections and actions are not interpreted; citations and actions must be empty |
| Rust validator | CODE: `validate_metadata`, `validate_document`, `validate_block` in `crates/storage-sqlite/src/rich_presentations.rs`; accepts the same five V1 kinds as the desktop (TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE) with the same closed keys, and holds CARD titles, CALLOUT summaries and TABLE headers and cells to the semantic text | CHART and SAFE_CALCULATOR are not accepted yet (renderer contract version 2 belongs to T03) |
| Size cap | `MAX_RICH_PRESENTATION_BYTES: u64 = 524_288` in `crates/storage-core/src/rich_presentation.rs`, matching `MAX_DOCUMENT_BYTES = 512 * 1024` in the desktop validator and `document_size_bytes.maximum` in `docs/schemas/operator-stream.schema.json` | Aligned on all three surfaces; `rich_presentation_document_cap_agrees_with_the_desktop_and_the_wire_schema` fails if any one of them changes alone |
| Store | CODE: `RichPresentationStore` (`publish_rich_presentation`, `read_rich_presentation`, conversation presentation reads) and `SqliteRichPresentationStore`; table from migration `rich_presentation_optional_v12` | `publish_rich_presentation` is called only by tests (`rich_presentation_read_tests` in `apps/litecoworkd/src/presentation_operator.rs`); no production producer |
| Read route | CODE/SERVED: `GET /v1/rich-presentations/{presentation_id}` (`get_rich_presentation`), `GET /v1/conversations/{conversation_id}/presentation`, `POST /v1/stream` in `apps/litecoworkd/src/presentation_operator.rs`; Tauri `get_rich_presentation` and `valid_rich_presentation_response` in `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs` | No publish route (by design: publishing is daemon-internal) |
| Renderer | CODE/UI: `apps/litecowork-ui/src/presentation/RichResponseView.tsx` renders TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE with an accessible table region and a "Read the original response" disclosure; used by `ConversationsPage.tsx`. Semantic fallback uses `apps/litecowork-ui/src/artifacts/StructuredTextPreview.tsx`, a bounded hand-written Markdown subset. | No chart, image or Intelligent UI rendering. Unsupported Markdown falls back for the whole message; no GFM table, horizontal-rule or nested-list rendering; no code label/copy/highlighting; TEXT_SLICE is plain text. These partial adjacent capabilities do not satisfy AC-019/T18. |
| Fallback tests | CODE: `apps/litecowork-ui/tests/presentation-runtime-fallback.test.mjs` (escaping, unsupported kinds, malformed payloads for `PresentationRuntime.tsx` and `presentation-types.ts`), `rich-presentation.test.ts`, `rich-response-view-policy.test.ts` | No shared Rust and TypeScript corpus |
| Schemas | `docs/schemas/rich-presentation.schema.json` (13 block kinds, root_blocks 200, accessibility_summary 4000), `docs/schemas/presentation-intent.schema.json` (strategies, elements 40) | No separate Intelligent UI schema: calculator/interaction extensions must be promoted into the existing canonical contracts; former competing illustrative document removed |
| Rust schema library | NOT FOUND: no `jsonschema` crate in `Cargo.lock`; hand-written `closed_object` validation is the repo pattern; `serde_json_canonicalizer` and `sha2` are present | |
| Errors | NOT FOUND: no `PRESENTATION_*` codes; routes use `INVALID_ARGUMENT`, `FORBIDDEN`, `INTEGRITY_FAILURE`, `INTERNAL` | |
| Generated media | NOT FOUND: no ingestion pipeline; Feature 007 B-30 Artifact create is planned (no create route today) | CSP `img-src 'self' asset: http://asset.localhost data:` has no `litecowork-resource:` scheme |
| Interactive Artifact Runtime | NOT FOUND in code; `docs/schemas/interactive-artifact-runtime.schema.json` and `interactive-artifact-package.schema.json` exist; `PresentationRuntime.tsx` renders MCP_APP as "Interactive app content is unavailable in this renderer." | E08-S09 evidence absent |
| CSP | `apps/litecowork-ui/src-tauri/tauri.conf.json`: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' asset: http://asset.localhost data:; style-src 'self' 'unsafe-inline'; script-src 'self'` | Must stay at least this strict |

## 5. Decision register

### Inherited LOCKED invariants (canonical owners; not feature approval)

| ID | Invariant | Authority |
|---|---|---|
| L-01 | The committed semantic ConversationMessage is authoritative; RichPresentation is an optional display improvement that never blocks turn completion. | RENDERING-CONFORMANCE-LLD, PRESENTATION-RUNTIME, release contract section 4 |
| L-02 | RichPresentation v1 is a static, display-only subset with a closed block list; it is not an executable UI container. | RICH-RESPONSE "Intelligent UI and image content audit", PRESENTATION-RUNTIME |
| L-03 | No executable JavaScript, no eval, no script tags, no CSS imports and no network URLs in Intelligent UI documents; calculations use a bounded arithmetic-only AST evaluated by a pure reducer. | INTELLIGENT-UI-AND-IMAGE-CONTENT section 3.1 |
| L-04 | Chart binding is required and selects exactly one source; numbers are finite; charts include an accessible summary and a table fallback. | RICH-RESPONSE "Tables and charts" |
| L-05 | Images refer only to authorized pinned Resource or Artifact revisions; direct model URLs are not fetched; autoplay is false; alt text is required. | RICH-RESPONSE "Diagrams and media", INTELLIGENT-UI section 4 |
| L-06 | Interactive Artifact Run is enabled only for an exact qualified OS/Runtime profile after E08-S09 evidence; until then the Workbench remains inert preview or source only. | V1-PRODUCT-RELEASE-CONTRACT section 4, INTERACTIVE-ARTIFACT-RUNTIME |
| L-07 | Do not execute arbitrary model-supplied HTML or JavaScript in the chat WebView. | AGENTS.md section 8 |
| L-08 | Rendered controls cannot forge or settle host state (Approvals, Tasks, Effects, grants). | ACCEPTANCE-AND-E2E-PLAN E2E-11, P10 exit |

### FLEXIBLE

| ID | Choice | Allowed options | Constraint |
|---|---|---|---|
| F-01 | Validator crate shape | New `crates/domain-presentation` or a module inside `storage-core` | One Rust definition of bounds and allowlist; no `storage-sqlite` dependency |
| F-02 | Golden corpus format | JSON files with expected verdicts | Read by both `cargo test` and `node --test` |
| F-03 | Chart drawing | Hand-written React SVG elements | No new npm dependency without RESEARCH justification; no model-supplied SVG |
| F-04 | Media display scheme | Existing `asset:` protocol through an authorized read, or a new scheme added to CSP | CSP change goes through Feature 011 and Feature 019 audit |

### DECISIONS (B-40 accepted as proposed 2026-11-10)

| ID | Topic | Proposed resolution | Canonical authority and consequence |
|---|---|---|---|
| B-40 | First Intelligent UI component and Interactive Artifact Runtime V1 posture | **ACCEPTED as proposed (owner sarv-projects, 2026-11-10).** (1) **First component: `SAFE_CALCULATOR`** (the canonical SAFE_CALCULATOR target with declared numeric inputs): up to 8 labelled numeric inputs whose initial values are bound to cells of a TABLE in the same semantic message, one `formula_ast` from the restricted CalcAst (LITERAL, VARIABLE, BINARY_OP `+ - * / %`, UNARY_OP `-`, FUNCTION `MIN MAX ROUND ABS CLAMP`; at most 64 nodes, depth at most 12, finite literals only), output formats `NUMBER`, `INTEGER`, `PERCENTAGE`, `CURRENCY_USD`, and a required `fallback_text` that states the formula and the result for the original values. State is view-local, never persisted, never sent to the agent and never used as an action. Divide by zero or a non-finite result shows "Cannot calculate with these values" and the fallback text. It is carried as a new block kind `SAFE_CALCULATOR` in RichPresentation renderer contract version 2 (schema version stays 1 for existing kinds), so there is one validated document per message. (2) **CHART** is a static block in the same contract version, bound only to a semantic-message TABLE slice in V1. (3) **Interactive Artifact Runtime V1 posture: PREVIEW_ONLY on both release rows.** Selecting an interactive Artifact opens an inert preview (title, exact version, provenance, package validation result, source view); there is no Run control in V1, only the text "Running interactive apps is not available in this version." Run may be enabled only by a later accepted revision after E08-S09 CODE, SYSTEM and owner USER evidence for the exact packaged build and OS. | [INTELLIGENT-UI-AND-IMAGE-CONTENT.md](../../docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md), [RICH-RESPONSE.md](../../docs/RICH-RESPONSE.md), [INTERACTIVE-ARTIFACT-RUNTIME.md](../../docs/INTERACTIVE-ARTIFACT-RUNTIME.md). Exercises the restricted AST and reducer (BT-06.4) with the smallest attack surface; keeps BT-06.5 honest. Owner: Presentation Runtime with Product and Security; decision needed from the repository owner. |

B-40 was accepted as proposed on 2026-11-10 and remains the current component-level boundary. The former baseline-versus-extension split is superseded: revision 0.4 proposes one complete V1 scope across AC-001..AC-041. That revised scope is still TARGET and IN_REVIEW; the owner's decision to keep Saved Views in V1 and amend Feature 007 B-21 settles that product direction only. Feature 007's Saved View contract still needs cross-owner review, then T01 must record whole-scope acceptance or exclusions, and T02 must promote owner contracts and schemas before implementation. No implementation is authorized by this proposal. Current source evidence is in section 4 and [docs/IMPLEMENTATION-STATUS.md](../../docs/IMPLEMENTATION-STATUS.md).

**Historical revision 0.3 decision note (superseded by §20 revision 0.4):** B-40 still governs calculator behavior: the component is local-only, transient, and cannot submit or persist. B-46 remains open for any separate host-owned remembered-input or input-sharing behavior. The owner has since selected Saved Views for V1 and directed the Feature 007 B-21 amendment; T25 records that direction, with technical cross-owner review and T01/T02 still open.

## 6. High-level design

```mermaid
flowchart TD
  SUP[AgentSessionSupervisor] -->|final semantic text| CS[commit_conversation_agent_response]
  CS --> DB[(SQLite)]
  CS -->|after commit| BIND[PresentationBinder]
  BIND --> VAL[domain-presentation validator]
  VAL -->|valid| PUB[RichPresentationStore publish]
  PUB --> DB
  SUP -->|harness file output| MED[MediaIngestionService]
  MED -->|MIME sniff, size, digest| BLOB[(BlobStore)]
  MED --> ART[Feature 007 Artifact create]
  ART --> DB
  DB --> OP[Presentation Operator GET routes and stream]
  OP --> BR[Tauri conversation_bridge]
  BR --> TSV[rich-presentation.ts validator]
  TSV --> VIEW[RichResponseView + ChartBlock + SafeCalculatorBlock]
  TSV -->|invalid or disabled| SEM[Semantic text view]
  LIB[Library and Workbench] -->|interactive Artifact| PREV[Inert preview only]
```

### Trust boundaries

| Boundary | Rule |
|---|---|
| Native harness to daemon | Semantic text, presentation intent and files are untrusted until validated; only the binder creates RichPresentation documents |
| Daemon to WebView | Only validated documents with digest bindings; the WebView validates again and falls back on any mismatch |
| WebView rendering | Host React components only; model strings are rendered as text nodes; no `dangerouslySetInnerHTML`, no eval, no model SVG |
| Media | Bytes are content-addressed and sniffed; displayed only through an authorized Artifact read |

### Actor sequence: rich answer with chart and calculator (target)

1. The native turn settles; `commit_conversation_agent_response` (Feature 001) commits the AGENT message.
2. After commit, `PresentationBinder` derives a document from the semantic text: Markdown tables become TABLE blocks bound by slice digest; an eligible numeric table may add a CHART and, when a validated canonical calculation-intent variant has been promoted by T02/T20 and supplied by the harness, a SAFE_CALCULATOR.
3. `domain-presentation::validate_document` checks bounds, allowlist, bindings and the CalcAst; on success the store publishes and emits `rich.presentation.published.v1`; on failure nothing is published and a counter increments.
4. The desktop reads the Conversation presentation, fetches the document through `get_rich_presentation`, validates again in `rich-presentation.ts` and renders, or shows semantic text.

### Ownership

| Owner | Responsibility | Excluded |
|---|---|---|
| domain-presentation (new Rust crate) | Bounds, allowlist, closed-object checks, CalcAst validation, chart binding checks | Storage, I/O |
| Storage | Publish transaction, idempotency, reads | Interpreting blocks beyond calling the validator |
| PresentationBinder (daemon) | Deterministic derivation from committed semantic content | Changing the semantic message |
| MediaIngestionService (daemon) | Sniff, bound and hash harness files; request Feature 007 Artifact create | Fetching URLs |
| Desktop validator and views | Defensive re-validation, rendering, accessibility, view-local calculator state | Host state, actions |
| Feature 011 | CSP, display preference persistence | Presentation semantics |

## 7. Low-level design and interfaces

### 7.1 Existing CODE signatures (real paths)

```ts
// apps/litecowork-ui/src/presentation/rich-presentation.ts
export async function parseRichPresentationDocument(value: unknown, semanticText: string,
  expectedMessageId: string, expectedSemanticContentDigest: string): Promise<ValidatedRichPresentation | null>;
export async function parseRichPresentationResponse(value: unknown,
  expected: RichPresentationResponseBinding, semanticText: string): Promise<ValidatedRichPresentation | null>;
export function resolveRichResponseContent(semanticText: string, presentation?: ValidatedRichPresentation | null):
  { semanticText: string; presentation: ValidatedRichPresentation | null; primary: "RICH" | "SEMANTIC" };
export type RichBlock = TEXT_SLICE | LAYOUT | CARD | CALLOUT | TABLE;
```

```rust
// crates/storage-core/src/rich_presentation.rs
pub const MAX_RICH_PRESENTATION_BYTES: u64 = 524_288;
pub const RICH_PRESENTATION_MEDIA_TYPE: &str = "application/vnd.litecowork.rich-presentation+json";
pub trait RichPresentationStore { fn publish_rich_presentation(&self, ..) -> ..; fn read_rich_presentation(&self, ..) -> ..; /* conversation reads */ }
// crates/storage-sqlite/src/rich_presentations.rs (private)
fn validate_metadata(..); fn validate_document(..); fn validate_block(..); // TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE
```

Served: `GET /v1/rich-presentations/{presentation_id}` (`get_rich_presentation`), Tauri `get_rich_presentation` in `conversation_bridge.rs`.

### 7.2 Accepted-target signatures (PROPOSED in design; not yet in source)

```rust
// crates/domain-presentation/src/lib.rs (new crate)
pub const MAX_DOCUMENT_BYTES: usize = 512 * 1024;   // the single document cap; storage already matches it
pub const MAX_BLOCKS: usize = 200;
pub const MAX_DEPTH: usize = 8;
pub const MAX_JSON_VALUES: usize = 30_000;
pub const MAX_JSON_DEPTH: usize = 32;
pub const MAX_CALC_NODES: usize = 64;
pub const MAX_CALC_DEPTH: usize = 12;
pub const MAX_CALC_INPUTS: usize = 8;
pub enum V1BlockKind { TextSlice, Layout, Card, Callout, Table, Chart, SafeCalculator }
pub enum CalcAstNode { Literal(f64), Variable { field_id: String }, Binary { op: BinaryOp, left: Box<CalcAstNode>, right: Box<CalcAstNode> },
    Unary { op: UnaryOp, operand: Box<CalcAstNode> }, Function { name: CalcFunction, args: Vec<CalcAstNode> } }
pub struct ValidationContext<'a> { pub semantic_text: &'a str, pub message_id: &'a str, pub semantic_content_digest: &'a str }
pub fn validate_document(bytes: &[u8], ctx: &ValidationContext) -> Result<ValidatedDocument, PresentationRejection>;
pub fn validate_calc_ast(node: &CalcAstNode, inputs: &[String]) -> Result<(), PresentationRejection>;
pub enum PresentationRejection { TooLarge, NotCanonical, DigestMismatch, UnknownKey, UnsupportedBlock, DepthExceeded,
    TooManyBlocks, BindingMismatch, NonFiniteNumber, CalcTooComplex, CalcUnknownVariable, ProvenanceInvalid }

// apps/litecoworkd/src/presentation_binder.rs (new)
pub async fn bind_after_commit(&self, committed: &CommittedConversationAgentResponse) -> Result<Option<CommittedRichPresentation>, BindError>;

// apps/litecoworkd/src/media_ingestion.rs (new)
pub async fn ingest_harness_file(&self, origin: MediaOrigin /* conversation_id, turn_id, agent_session_id */, path: &AdmittedPath)
    -> Result<ArtifactVersionRef, MediaRejection>; // png, jpeg, webp, gif; <= 20 MiB; SVG rejected in V1
```

```ts
// apps/litecowork-ui/src/presentation/calc-reducer.ts (new)
export type CalcState = Readonly<Record<string, number>>;
export function evaluateCalc(ast: CalcAstNode, state: CalcState): { ok: true; value: number } | { ok: false; reason: "DIVIDE_BY_ZERO" | "NON_FINITE" };
// apps/litecowork-ui/src/presentation/ChartBlock.tsx, SafeCalculatorBlock.tsx (new)
```

Accepted target error codes: `PRESENTATION_REJECTED` (with a `reason` from `PresentationRejection`), `MEDIA_TYPE_UNSUPPORTED`, `MEDIA_TOO_LARGE`. Accepted target events: existing `rich.presentation.published.v1`; new `rich.presentation.rejected.v1` (ids and reason only). Accepted target schema: `docs/schemas/rich-presentation.schema.json` gains renderer contract version 2 with `SAFE_CALCULATOR` and a V1 allowlist profile; new `docs/schemas/calc-ast.schema.json`. Local display preference `rich_presentation_enabled` (default true) is owned by Feature 011's display preferences.

## 8. Data model and lifecycle

See [DATA-MODEL.md](DATA-MODEL.md). Reuses `rich_presentations` (migration `rich_presentation_optional_v12`), blobs with purpose `RICH_PRESENTATION`, Artifact and ArtifactVersion records from Feature 007. A RichPresentation is immutable with one record per committed message; host-owned publication identity is replayed, and a binder upgrade cannot create a second record for that message. No new SQLite table is required; media provenance fields ride on the Feature 007 ArtifactVersion `provenance` value.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Publishing happens after the semantic commit in its own transaction; media blob writes happen before the Artifact create transaction; no external call is inside any transaction.

## 10. Security, trust and privacy

- Model output is data. Every string renders as a React text node. A lint and test gate fails the build on `dangerouslySetInnerHTML`, `eval`, `new Function` or `innerHTML` in `apps/litecowork-ui/src/presentation/`.
- CSP stays `script-src 'self'`; Feature 019 audits it. Adding an image scheme is a Feature 011 change with a test.
- Presentation documents carry no actions, citations, links or host projections in V1; they cannot reach a Tauri command.
- The calculator never sends values anywhere; it is not an action and has no submit.
- Media: magic-byte sniff must match the declared type; size at most 20 MiB; dimensions bounded (at most 16384 pixels per side). Stripping EXIF metadata from a display copy is open question O-03; the original bytes are kept as the Artifact.
- OpenIntelligentUI is a test-idea reference only; its renderer, eval use and CSP are rejected.

## 11. Errors, edge cases and recovery

Full matrix in [contracts/errors-recovery.md](contracts/errors-recovery.md). Key cases: a document at exactly 512 KiB passes and at 512 KiB + 1 byte is rejected on both sides; a TABLE whose cells no longer match the semantic text falls back; a CHART with a non-numeric series cell falls back to its table; a calculator divide by zero shows the reason; a crash after blob write and before Artifact commit leaves an orphan blob that the sweep removes; replay of a publish with the same id and digest returns the stored record, a different digest is a conflict; a stream gap resyncs from the Conversation presentation read.

## 12. Configuration and compatibility

Renderer contract version 1 documents (existing kinds) stay valid; the proposed version 2 profile may add METRIC, CHART, SAFE_CALCULATOR and the admitted local input types only after B-46/T02. A desktop that does not know the accepted profile falls back to semantic text. `rich_presentation_enabled` defaults and turn-captured retry behavior must match the accepted Rich Response contract. Optional remembered-input state is off by default and uses a separately approved local owner store; Saved View snapshots use Feature 007, never browser localStorage. Both release rows use their supported WebView rules and require exact conformance evidence.

## 13. Observability

Count published, rejected (by reason), fallback-rendered and disabled views; validator duration; media ingest accepted and rejected (by reason). Never log semantic text, document content or media bytes; log ids, digests and reasons.

## 14. EARS acceptance criteria

- AC-001 WHEN the binder publishes a RichPresentation, THE SYSTEM SHALL validate it in Rust with `MAX_DOCUMENT_BYTES` 512 KiB, at most 200 blocks, depth at most 8, closed keys, RFC 8785 canonical bytes and matching document and slice digests, and IF the document is 512 KiB + 1 byte or larger, THEN THE SYSTEM SHALL reject it with `PRESENTATION_REJECTED` (`TooLarge`). Verify: boundary unit tests in `domain-presentation`.
- AC-002 WHEN a document contains a block kind, field, binding or local control outside the accepted canonical profile, THE SYSTEM SHALL reject it at publish, and the desktop SHALL render the semantic text for any such document it receives. The proposed profile is listed in §20.4 and remains TARGET pending B-46/T01/T02. Verify: corpus tests on both sides.
- AC-003 WHEN the Rust and TypeScript validators process the shared golden corpus, THE SYSTEM SHALL produce identical accept or reject verdicts for every file. Verify: parity test run by `cargo test` and `pnpm test`.
- AC-004 WHILE rich presentation is disabled, missing, invalid or failed to render, THE SYSTEM SHALL show the complete semantic message, and presentation work SHALL NOT delay or change turn settlement. Verify: E2E-11 and a binder fault-injection test.
- AC-005 WHEN any presentation content is rendered, THE SYSTEM SHALL render model strings only as text nodes and SHALL NOT use `dangerouslySetInnerHTML`, `eval`, `new Function`, model-supplied HTML, CSS or SVG markup, or fetch an external URL; any Markdown navigation link admitted by AC-019 SHALL be an explicit `https:` link, and the WebView CSP SHALL keep `script-src 'self'`. Verify: static gate test, hostile corpus test and CSP audit.
- AC-006 IF a document contains action references, citation references, HOST_PROJECTION or MCP_APP, THEN THE SYSTEM SHALL reject it, and no rendered presentation element SHALL invoke a Tauri command that creates or settles an Approval, Task, Effect or grant. A semantic TEXT_SLICE may contain ordinary Markdown links only under AC-019's host-rendered `https:` policy; a link is not an action or citation reference. Verify: hostile corpus and bridge-call spy test.
- AC-007 WHEN a SAFE_CALCULATOR is validated, THE SYSTEM SHALL accept only CalcAst nodes LITERAL, VARIABLE, BINARY_OP (`+ - * / %`), UNARY_OP (`-`) and FUNCTION (MIN, MAX, ROUND, ABS, CLAMP) with at most 64 nodes, depth at most 12, finite literals and variables that name declared inputs, and IF evaluation divides by zero or yields a non-finite value, THEN THE SYSTEM SHALL show "Cannot calculate with these values" with the fallback text. Verify: Rust validator tests and TypeScript reducer property tests.
- AC-008 WHEN a CHART is rendered, THE SYSTEM SHALL draw it from host SVG components using only the bound semantic TABLE values, show its accessible summary, and offer an equivalent data table reachable by keyboard; IF the binding does not match the semantic table or a value is not finite, THEN THE SYSTEM SHALL render the table fallback. Verify: chart component tests.
- AC-009 WHEN the SAFE_CALCULATOR renders, THE SYSTEM SHALL label every input, announce the result in an `aria-live="polite"` region, and SHALL NOT itself persist values or send them to an agent. Under the current accepted B-40 baseline, values remain transient. Only if revised B-46 is accepted may a separate host-owned Remember or ordinary Send operation carry a user-selected snapshot after consent/review; the component itself still cannot invoke IPC or submit. Verify: component bridge-call spy plus explicit host-operation consent/send tests.
- AC-010 WHEN rich content is displayed, THE SYSTEM SHALL meet keyboard operation, visible focus, accessible names, `<th scope>` headers, text contrast 4.5:1 and reduced-motion rules, with no serious or critical axe violations. Verify: accessibility audit in the UI test suite.
- AC-011 WHEN a native harness produces an image file in the admitted Workspace, THE SYSTEM SHALL ingest it only if the sniffed type is PNG, JPEG, WebP or GIF and the size is at most 20 MiB, store the bytes content-addressed, and create an Artifact version through Feature 007 with provenance (Conversation, turn, AgentSession, digest). Verify: ingestion integration test.
- AC-012 IF a media file is SVG, oversized, has a type mismatch, or is referenced only by a URL, THEN THE SYSTEM SHALL reject it with `MEDIA_TYPE_UNSUPPORTED` or `MEDIA_TOO_LARGE`, create no Artifact, fetch nothing and keep the semantic answer. Verify: negative ingestion tests.
- AC-013 WHILE no E08-S09 CODE, SYSTEM and owner USER evidence exists for the exact build and OS, THE SYSTEM SHALL open interactive Artifacts as an inert PREVIEW_ONLY view with no Run control and no script execution. Verify: Workbench component test and packaged check on both rows.
- AC-014 WHEN a publish is replayed with the same presentation id and digest, THE SYSTEM SHALL return the stored record; IF the same id arrives with a different digest, or two publishes race for one message, THEN exactly one SHALL commit and the other SHALL get a conflict with no partial write. Verify: storage concurrency tests.
- AC-015 IF the daemon crashes after a media blob write and before the Artifact commit, or the presentation stream has a gap, THEN THE SYSTEM SHALL remove the orphan blob on the next sweep, and SHALL resync the Conversation presentation from the durable read without duplicating messages. Verify: crash-injection and stream resync tests.
- AC-016 WHEN validating a 512 KiB document, THE SYSTEM SHALL finish the Rust validation within 50 ms and the TypeScript validation within 150 ms at p95, and WHEN rendering 200 blocks THE SYSTEM SHALL not block the main thread for more than 100 ms in a single task on the release-row machine. Verify: benchmark and render trace.
- AC-017 WHEN the binder derives a document, THE SYSTEM SHALL use only committed semantic content and validated PresentationIntent elements with provenance SEMANTIC_MESSAGE or MODEL_INTENT, and SHALL NOT alter the semantic message. Verify: binder unit tests.
- AC-018 WHEN the owner turns rich presentation off in Settings, THE SYSTEM SHALL show semantic text immediately for all messages and persist that preference locally across restart; WHEN the owner opens "Read the original response" on one message, THE SYSTEM SHALL reveal that message's semantic text without changing the global preference. The Settings toggle SHALL be keyboard operable with an accessible name. Verify: preference and UI tests.
- AC-019 WHEN an agent answer renders Markdown (task P-03), THE SYSTEM SHALL degrade per block so that GFM tables, horizontal rules and nested lists render without a whole-message raw fallback, SHALL give each code block a language label, a copy button and lazy highlighting that returns React nodes, SHALL render TEXT_SLICE through the same Markdown renderer, and SHALL emit no HTML strings; a golden corpus of real agent answers SHALL render with no whole-message raw fallback. Verify: per-block degradation and golden-corpus render tests.

## 15. Executable acceptance command

**Proposed feature acceptance command (the Rust crate and E2E target do not exist yet and require owner acceptance plus TASKS 016-T03 and 016-T16):**

```bash
cargo test -p domain-presentation --locked && cargo test -p litecoworkd --test rich_presentation_intelligent_ui --locked -- --nocapture && pnpm --dir apps/litecowork-ui test
```

Do not mark the feature VERIFIED until the accepted revision's AC-001 to AC-041 pass on each release row with packaged SYSTEM and owner USER evidence. Unit tests, schema validation, route presence and component screenshots are not release qualification.

## 16. Out-of-bounds paths and side effects

This package is IN_REVIEW and authorizes nothing yet. After acceptance each task names its allowed paths. Out of scope for Feature 016 ownership: semantic message storage/final-commit transaction (Feature 001), Artifact create/version/Library internals (Feature 007), Workbench shell and live Computer/Browser/Terminal providers (Feature 008), CSP and Settings shell (Feature 011), Conversation retention/deletion (Feature 014), backup/restore (Feature 017), and Interactive Artifact Runtime execution (separate E08-S09 owner). Feature 016 integrates through their accepted commands and records but does not implement a competing owner. Do not run native harnesses or write user files during design validation.

## 17. Assumptions and open questions

| ID | Question | Evidence/limitation | Owner | Gate |
|---|---|---|---|---|
| O-01 | Does OpenCode v2.0.32 emit any structured presentation intent, or only Markdown? | Binder derives from Markdown first; intent is optional | Presentation Runtime | 016-T08 |
| O-02 | Which image scheme to display Artifacts: `asset:` or a new scheme | CSP today lacks `litecowork-resource:` | Feature 011, Security | 016-T10 |
| O-03 | Should EXIF be stripped from a display copy | Privacy improvement; not required by owner docs | Security | 016-T10 |
| O-04 | Reference machine for AC-016 | Feature 019 defines release-row machines | Release | 016-T14 |
| O-05 | Interactive Artifact Runtime execution timeline after V1 | E08-S09 owns it | Product | B-40 |

## 18. Requirement traceability

| Requirement | Canonical owner | Task | Acceptance |
|---|---|---|---|
| Owner acceptance and B-40 | OWNER-ACCEPTANCE-REVIEW-PACKET, register | 016-T01 | AC-013 |
| Contract promotion | RICH-RESPONSE, SCHEMAS, error registry, EVENTS | 016-T02 | AC-001 / AC-002 / AC-006 |
| Single Rust validator and drift fix | RICH-RESPONSE, PRESENTATION-RUNTIME | 016-T03 | AC-001 / AC-002 / AC-006 |
| Shared corpus and TypeScript parity | RENDERING-CONFORMANCE-LLD | 016-T04 | AC-003 |
| Restricted CalcAst and reducer | INTELLIGENT-UI section 3 | 016-T05 | AC-007 |
| Chart with table fallback | RICH-RESPONSE "Tables and charts" | 016-T06 | AC-008 |
| SAFE_CALCULATOR component | INTELLIGENT-UI, B-40 | 016-T07 | AC-009 / AC-010 |
| Binder and publish after commit | PRESENTATION-RUNTIME, STORAGE | 016-T08 | AC-004 / AC-017 |
| Disable preference and semantic fallback | V1 release contract section 4, E2E-11 | 016-T09 | AC-004 / AC-018 |
| Generated media as provenance Artifacts | INTELLIGENT-UI section 4, ARTIFACTS-EVIDENCE | 016-T10 | AC-011 / AC-012 |
| Interactive Artifact PREVIEW_ONLY | INTERACTIVE-ARTIFACT-RUNTIME | 016-T11 | AC-013 |
| No-executable-content gate and CSP | AGENTS.md section 8, INTELLIGENT-UI section 7 | 016-T12 | AC-005 / AC-006 |
| Failure and recovery | FAILURE-RECOVERY | 016-T13 | AC-014 / AC-015 |
| Performance budgets | BENCHMARKS | 016-T14 | AC-016 |
| Cross-layer traces | AGENTS.md section 4 | 016-T15 | AC-004 / AC-018 |
| Release-row qualification (E2E-11) | ACCEPTANCE-AND-E2E-PLAN | 016-T16 | AC-004 / AC-005 / AC-010 / AC-013 |
| Documentation sync | IMPLEMENTATION-STATUS | 016-T17 | AC-013 |
| Per-block Markdown degradation and code/TEXT_SLICE rendering (P-03) | INTELLIGENT-UI-AND-IMAGE-CONTENT | 016-T18 | AC-019 |
| VERIFIED transition | SPECIFICATION-STANDARD | 016-T19 | AC-001, AC-002, AC-003, AC-004, AC-005, AC-006, AC-007, AC-008, AC-009, AC-010, AC-011, AC-012, AC-013, AC-014, AC-015, AC-016, AC-017, AC-018, AC-019 |

The revised V1 trace for Canvas, state, Saved View, conversational modification, provider ingress and release evidence is in §20.12; it supersedes the baseline-only trace above when those capabilities are accepted.

## 19. Superseded revision 0.2 extension proposal (historical TARGET)

This section records the prior 0.2 proposal for traceability. **It is superseded as a scope and task gate by §20.** Its AC-020..AC-028 text is not a second normative proposal and does not constitute acceptance. The revised V1 proposal below reuses those stable AC identifiers, expands and reorders their scope, and adds AC-029..AC-041. No prior T01 or B-40 decision accepts §20.

**Historical revision 0.2 decision state:** at that point A-84 split baseline acceptance (T01) from extension acceptance (T25). Revision 0.3 §20 supersedes that split with one whole-scope T01 review after the Saved View boundary decision at T25. A-85 still keeps any model-visible proposal tool unavailable until its model input, host sequence and replay contract are resolved. B-40's view-local calculator and PREVIEW_ONLY app posture remain accepted; optional persistence and new-turn modification still require explicit owner decisions and must not silently broaden B-40.

**Historical revision 0.2 boundary (superseded by §20 revision 0.4):** this earlier text distinguished the message-bound RichPresentation, optional device-local `PresentationInteractionState`, executable `INTERACTIVE_APP` ArtifactVersion and isolated Artifact Runtime data. It did not yet include the later owner-selected Saved View. The current direction is a Feature 007-owned non-executable Artifact snapshot with exact source lineage, semantic fallback and declared presentation defaults only; no session-modified values are saved or sent. The draft contract and its remaining cross-owner, T01 and T02 gates are recorded in §20 and `specs/007-professional-artifacts/contracts/saved-view.md`.

The LLD supplies HLD/LLD interfaces, optional scoped MCP tool ingress, native structured output capability gates, fallback, typed metrics, deterministic calculations, state data model, authenticated state-command transaction, lineage, provenance, races and recovery. Existing global schemas remain the only machine authorities. T02/T20 must promote calculation intent in the existing schema: current schema has no CALC. No arbitrary native tool event becomes a host invocation. No hidden rendering model, provider substitution, global harness config write or stack replacement is adopted.

### Additional acceptance (gated extension; baseline ACs unchanged)

- AC-020 WHEN the exact native route lacks qualified structured output or scoped MCP injection, THE SYSTEM SHALL preserve bounded semantic output and deterministic formatting without a hidden model call or native config rewrite. Verify: unsupported/ignored guidance fixtures and provider descriptor evidence (T20).
- AC-021 WHEN a native tool_call reports an arbitrary function, UI schema or executable body, THE SYSTEM SHALL treat it only as a correlated observation and SHALL NOT dispatch host operations; wrong-session/retry/incarnation proposals SHALL be refused. Verify: adapter/bridge hostile fixtures (T20/T24).
- AC-022 WHEN a user expands or collapses a registered presentation, THE SYSTEM SHALL preserve the exact document, inputs and state revision through one controller, move/restore focus predictably and SHALL NOT invoke an agent or run an Artifact. Verify: controller identity, keyboard/high-zoom and bridge-spy tests (T21).
- AC-023 WHEN data are labeled as metrics, THE SYSTEM SHALL require explicit units/time/aggregation and preserve bound semantics: example total INR 12.08 lakh, latest June INR 3.15 lakh and January-to-June change 162.5%; missing definitions SHALL show a table or clarification. Verify: latest/sum/change, reordered/missing periods, scale mismatch and zero denominator fixtures (T20/T24).
- AC-024 WHEN an unregistered operation such as sipFutureValue is proposed, THE SYSTEM SHALL reject it; any later admitted numerical kernel SHALL have reviewed rate-period, contribution-timing, compounding, zero-rate, finite-domain and stability/rounding tests. Verify: unknown-operation corpus and reviewed kernel fixture set (T20).
- AC-025 WHERE remembered state is separately accepted, THE SYSTEM SHALL retain only allowlisted input data after explicit off-by-default consent, enforce exact namespace/quota/authorization and require expected revision and RequestId; timeout-after-commit SHALL reconcile the same receipt and conflicts SHALL preserve the draft. Verify: persistence denial, quota, replay, timeout, two-window and crash tests (T22/T24).
- AC-026 WHEN Forget, deletion or revocation races an in-flight save, THE SYSTEM SHALL fence old writes and late acknowledgements, clear protected live data and prevent restart/backup from resurrecting it. Verify: save/forget and revocation/restart fixtures (T22/T24).
- AC-027 WHERE conversational modification is separately accepted, THE SYSTEM SHALL send only an explicitly previewed selected snapshot through a new ordinary turn, create a new message and presentation with host-recorded parent/base-revision lineage, and SHALL NOT overwrite newer local inputs or the original message. Verify: no implicit-send spy, stale-base/cancel/retry/new-message tests (T23/T24).
- AC-028 WHEN a user revisits or attempts to save a rich interactive answer, THE SYSTEM SHALL distinguish its immutable message-bound RichPresentation, Feature 016's separately consented answer-input state, a committed interactive ArtifactVersion, and any separately consented Artifact Runtime app data; it SHALL NOT treat either local data store as a Library Artifact or imply that a PREVIEW_ONLY Artifact is runnable. Save/open/update/delete controls require the corresponding authorized, served and recoverable Artifact owner path. A standalone reusable Saved View requires a separate cross-owner accepted revision and canonical record/API/event/retention contract before implementation. Verify: reopen the exact source message, confirm B-40 input reset, capability-gate save controls, verify exact Artifact version/provenance/receipt recovery, and prove the two local-state owners remain isolated (T25 and any later authorized Feature 007/Artifact Runtime task).

### Extension trace and gates

| Requirement | Owner/LLD | Task | Acceptance |
|---|---|---|---|
| Qualified native ingress, canonical intent and data semantics | LLD sections 2-5; RICH-RESPONSE, AgentModule | T20 | AC-020/021/023/024 |
| Shared inline/Canvas controller | LLD section 6; PRESENTATION-RUNTIME | T21 | AC-022 |
| Consent/revision/retention/deletion | LLD section 7; Storage, Workspace/Conversation deletion owners | T22 | AC-025/026 |
| Explicit new-turn modification | LLD section 7; Feature 001 Conversation, provenance | T23 | AC-027 |
| Saved answer, remembered inputs and reusable Artifact boundary | Feature 007 Artifact owner; Data Model and UI behavior; A-103 | T25, then any separately authorized cross-owner task | AC-028 |
| Hostile/recovery/packaged evidence | LLD sections 9-10; Feature 019 | T24 | AC-020 through AC-028 |

Open gates under that superseded design: these notes are retained as historical review context only. Current gates are listed in §20 and TASKS.md; do not start T20-T24 based on this section.

## 20. Revision 0.4: intelligent workspace presentation (TARGET V1 proposal)

### 20.1 Product decision and V1 boundary

The revised proposal turns the narrow rich-answer baseline and the useful 0.2 interaction proposal into one end-to-end V1 capability. The user describes a goal in Conversation; LiteCowork preserves the selected native agent's semantic answer and presents supported data through a safe host-owned component registry. A view may remain inline, expand in the contextual Workbench Canvas, and be explicitly saved as a reusable Library view. It is one workspace experience, not a separate specialist mode.

"Universal" means one canonical, extensible declarative contract and a host registry that accepts optional proposals from different capable native agents. It does not mean every agent emits the same structure, every application can be rebuilt in V1, or ACP itself carries a universal UI tree. Plain text and Markdown remain first-class. If a native route has no qualified structured ingress, deterministic host rendering may use only clearly identified semantic structures or supported structured results; otherwise the full semantic response is shown. No hidden rendering model, provider substitution, hosted BYOK router, native config rewrite, or mandatory agent-side UI generation is introduced.

The V1 proposal includes a bounded native declarative interactive-view runtime and durable Saved View behavior behind Feature 007's owner-backed Artifact/Library path. It does **not** include arbitrary generated executable apps. Such apps need the separate OS-isolated Artifact Runtime and its qualification; that Runtime remains PREVIEW_ONLY. This boundary keeps the product broad in user outcomes while keeping all executed code host-owned and deterministic.

### 20.2 User experience and surface ownership

| Surface | User sees | Owner and invariant |
|---|---|---|
| Conversation | Semantic answer streams and commits first; rich rendering loads after commit. The original response is always available. | Feature 001 owns turn identity and semantic text. Presentation failure cannot change settlement. |
| Inline interactive answer | A compact combination of prose, metrics, tables, charts and supported local controls. It preserves the producing AgentSession and exact source bindings. | Feature 016 renders one immutable message-bound RichPresentation; no generated element grants authority. |
| Expanded Canvas | The same answer/view opens in the existing right Workbench pane. Expand/collapse preserves the presentation digest, current local inputs, scroll context and focus. | Feature 008 owns the contextual object shell; Feature 016 supplies the Canvas object and one controller. Opening a tab is not authorization or execution. |
| Saved Views in Library | The owner selected V1 Saved Views and directed a Feature 007 B-21 amendment. The proposed non-executable snapshot can be reopened and updated through immutable Artifact versions after review. | Feature 007 revision 1.1 remains IN_REVIEW pending Artifact, Storage, Conversation, Privacy, Backup and Product review, plus Feature 016 T01/T02. No new permanent sidebar tab is proposed. |
| Activity / tools / computer | Relevant status and source details remain inspectable through owner-backed Activity or Inspector views. | Features 001/008/015 retain lifecycle and control ownership. Presentation cannot invent subagents, progress, approvals, Effects, or verification. |

The frequent left navigation does not gain a chart/app/tool tab. Conversation owns the answer; Library owns saved objects; Workbench owns the currently open contextual object. The right pane is not a fixed browser/file/terminal inventory. The live computer preview and Take over/Stop run controls remain Feature 008-owned and require their separate consent, provider and qualification gates.

### 20.3 HLD and runtime topology

```mermaid
flowchart LR
  U[User intent] --> C[Conversation composer]
  C --> A[Selected native AgentModule]
  A -->|semantic output + optional correlated intent/data| N[Feature 001 normalization]
  N --> M[Committed semantic message]
  M --> B[Feature 016 deterministic binder]
  N -. qualified structured proposal .-> B
  B --> V[Rust presentation validator]
  V -->|valid| S[Immutable RichPresentation store]
  V -->|unsupported or invalid| F[Semantic fallback]
  S --> R[Authenticated read and stream projection]
  R --> T[Defensive TypeScript validation]
  T --> H[Registered host React renderer]
  H --> I[Inline view + local controller]
  I --> W[Feature 008 Workbench Canvas]
  I --> L[Explicit Save View request]
  L --> O[Feature 007 Artifact/Library owner]
  O --> Lb[Versioned Library projection]
  F --> U
```

Runtime order: (1) selected native harness produces untrusted observations; (2) Feature 001 correlates and commits the complete semantic message; (3) Feature 016 deterministically derives or normalizes a presentation outside the message transaction; (4) the canonical validator binds the document to exact message, source and producer identities; (5) an immutable presentation is stored and projected; (6) the desktop revalidates before rendering; (7) inline and Canvas views share one controller; (8) explicit save/update goes through Feature 007 and returns a durable receipt. A saved view is not published until that receipt is committed. A presentation error never delays or retries the Conversation turn.

### 20.4 One canonical declarative contract and component registry

`docs/schemas/rich-presentation.schema.json` remains the single final-view tree contract. `presentation-intent.schema.json` remains the optional untrusted ingress intent, and ACP/provider envelopes remain adapter inputs rather than a second frontend schema. A future input format such as A2UI may be considered only as a version-pinned adapter that maps a bounded subset into these existing owners; it may not introduce its own renderer, action runtime, or persistent state authority. ACP does not guarantee UI payloads or subagent listings.

Proposed V1 registered families (each requires promotion in the canonical schema and implementation on both validator/renderer sides):

| Family | Proposed behavior | Required source/fallback |
|---|---|---|
| Semantic Markdown | Headings, paragraphs, links, lists, tables, horizontal rules and code render with block-local plain-text fallback; code copy/highlighting returns host React nodes. | Complete original semantic message remains available; links are HTTPS-only. |
| Layout, Card, Callout | Host-owned responsive layout and hierarchy; no CSS or style names from the agent. | Ordered readable stack and semantic text. |
| Table | Accessible, bounded tabular data; sorting/filter controls only if separately registered and local. | Same source values and row order remain inspectable. |
| Metric | Label, finite value, unit/currency/scale, interval and aggregation; exact source binding. | A table or semantic clarification when any definition is absent. |
| Chart | First profile: BAR, LINE or AREA from an authorized semantic table or exact authorized structured result. | Equivalent accessible data table and summary. |
| SAFE_CALCULATOR | B-40's at-most-eight numeric inputs, bounded CalcAst and local evaluation. Further common inputs (range/select) require the new B-46 owner decision and canonical schema promotion; no unreviewed calculator kernel. | Formula, original values and result are described in semantic text; invalid arithmetic shows the declared fallback. |
| Saved View controls | Host-owned Save, Open, Update and Archive controls appear only when Feature 007 returns the corresponding capability and durable receipt. | No control is implied by a presentation block or a stored input value; V1 has no hard-delete control. |

The component registry is closed per accepted renderer profile. A component cannot contain arbitrary markup, script, CSS imports, event-handler strings, network URLs or a model-supplied function. User-editable inputs bind only to declared primitive state. Calculations use the bounded AST or a separately owner-reviewed deterministic kernel. Unknown kinds/fields/functions are refused, not guessed. A schema-declared or catalog-listed type is not enabled until the exact runtime supports it.

### 20.5 Presentation choice, metrics and provenance

Presentation policy is host-owned and versioned. `AUTO` prefers readable prose for ordinary questions, tables for explicit comparisons/tabular facts, charts only when source and metric semantics are clear, and a bounded interactive view only when the answer contains a useful approved input/calculation relationship or a user explicitly asks to explore. `SIMPLE` suppresses decorative composition while preserving functional owner controls; `RICH` requests eligible structure but never requires it. The user's preference is captured on the turn and reused for retry per Rich Response. There is no required second model call.

Every metric and chart value must pin its source and declare measure, units/currency, scale, time interval and aggregation. "Total", "latest", "average" and "change" are distinct operations. For example, a time series with values 120000, 155000, 148000, 210000, 260000, 315000 may show June as INR 3.15 lakh, January–June total as INR 12.08 lakh, and start-to-end change as 162.5%; these are three different values, not interchangeable labels. Missing or contradictory meaning yields a table or clarification. A candidate presentation may not invent data, units, citations, verification, source lineage or provider identity.

The semantic answer remains the factual explanation and caveats. A RichPresentation may reorganize and interact with explicitly bound data, but every user-visible number must be traceable to committed semantic content or an authorized exact-version result. `BlockProvenance` retains host-assigned producer/source identity. Native AgentSession, provider-reported tool call, LiteCowork Task worker, committed Artifact and verified outcome remain distinct evidence levels.

### 20.6 Stateful views and Saved View lifecycle

There are four separate state/data identities; the UI labels them plainly:

1. **Transient view state:** controller values used while this message is open. B-40's default calculator remains view-local until the owner accepts the persistence extension.
2. **Remembered answer inputs:** an optional, explicit, device-local state for one exact message/presentation digest. It is not a Library item, Artifact, agent memory, or shared Workspace fact. It requires consent, expected revision, same-receipt reconciliation, deletion fencing, bounded quota and backup/retention decisions.
3. **Saved View:** a user-requested reusable Library object derived from one immutable RichPresentation, with exact source Conversation/message/presentation digests and semantic fallback. The owner selected this for V1 and directed a narrow Feature 007 B-21 amendment; revision 1.1 proposes the Artifact representation and is IN_REVIEW. A Saved View preserves the RichPresentation's original declared defaults. Values changed in an open calculator remain transient under accepted B-40 and are not persisted or sent. No standalone SavedView identity, table, store, route or navigation destination is introduced.
4. **Interactive Artifact app/runtime data:** an executable `INTERACTIVE_APP` Artifact and its isolated runtime state, owned by Feature 007 and `INTERACTIVE-ARTIFACT-RUNTIME.md`. This is not a Saved View and remains PREVIEW_ONLY in V1.

Saving a view is an explicit host command, not an implicit presentation action. It commits an exact immutable version and reports success only after Feature 007's durable receipt. Reopening loads that exact version after current authorization; it never silently follows a newer head or executes code. Calculator changes are transient and reset to the saved RichPresentation defaults on reopen. Updating a Saved View requires a new committed eligible presentation and a separate confirmation that appends a new immutable ArtifactVersion using the owner's expected-version/idempotency rules. A conflict preserves the review draft. Changing the originating answer is a separate, explicit Conversation Send that creates a new message and presentation with parent/base revision lineage; current calculator values are not included automatically. No original message or ArtifactVersion is overwritten.

Remembered answer-input persistence remains a separate contract from the saved reusable object. If both are accepted, their namespace and deletion/revocation rules must not overlap. No browser localStorage, provider configuration mutation, native agent memory injection or silent cross-device sync is allowed.

### 20.7 UI state, actions and failure paths

| State | User-visible behavior | Recovery |
|---|---|---|
| Semantic streaming | Existing Conversation stream is shown; no half-validated UI tree flashes. | On commit, the stable presentation may load independently. |
| Presentation pending/unavailable | Complete semantic answer remains visible; optional progress is display-only and not a turn state. | Read/retry presentation only; never resend the agent turn. |
| Invalid/unsupported block | The affected block falls back safely where the accepted contract allows; otherwise show the complete semantic answer. | Keep original answer and display a concise non-authoritative reason in Inspector. |
| Metric semantics missing | No misleading metric/chart is rendered. | Show source table or ask a plain-language clarification. |
| Local calculation invalid | Show bounded reason and declared fallback; no host call. | Restore valid input or Reset to original values. |
| Canvas expand/collapse | Same message, document digest, input revision and controller. | Restore focus to originating control; no new task, session or provider call. |
| Save/update pending or ambiguous | Retain operation identity and draft; never show Saved until receipt resolves. | Reconcile same RequestId and digest before retry; conflict keeps draft. |
| Saved View unavailable/revoked/deleted | Show an unavailable state without stale data. | Reauthorize exact version; do not silently substitute head. |
| Conversation modification | Preview selected values and the text/context to be sent; explicit Send only. | Cancel leaves original answer and saved version unchanged. |

Only host-owned controls call owner routes. A structured input may change local reducer state; it cannot call Tauri/Operator directly, create Task/Effect/Approval/Artifact, change Trust, claim completion, or send a message. External navigation, publishing and data access stay behind their existing confirmation/authorization owners.

### 20.8 Security and trust boundaries

| Boundary | Requirement |
|---|---|
| Native agent -> adapter | Every event, tool result and UI proposal is untrusted and correlated to exact provider/version/auth route, AgentSession, Conversation, turn and retry. Unsupported ingress falls back to semantic text. |
| Adapter -> binder | Only allowlisted, bounded input fields enter the canonical intent model. Provider credentials, native session handles, private transcript/reasoning and unmediated tool effects do not enter presentation records. |
| Binder -> storage | Rust validates closed objects, limits, canonical bytes, exact source bindings, producer provenance, metric semantics and calculator AST. Publish after semantic commit, idempotently. |
| Storage -> WebView | Authenticated exact-message read; defensive TypeScript validation; stale digest/version falls back. |
| Renderer -> host | Registered React components only. Local reducer state only, except explicit host chrome commands routed through owner APIs. CSP remains `script-src 'self'`; no `eval`, `new Function`, `dangerouslySetInnerHTML`, model HTML/JS, remote code or arbitrary iframe. |
| Canvas -> Workbench | View selection/reopen reauthorizes through the owning object. Opening Canvas does not capture screen, start a process, resume a Task, launch an app or grant authority. |
| Saved View -> ArtifactStore | Exact type, source refs, declared presentation defaults only, expected version, authorization and RequestId; idempotent receipt; no direct SQLite or file write from React. |

### 20.9 Proposed owner decisions and gates

| Decision | Proposal | Required owners / gate |
|---|---|---|
| B-46: declarative controls and explicit state boundary | Keep B-40 SAFE_CALCULATOR as the first view and its component behavior transient/local with no direct submit. Decide whether the closed V1 profile may add METRIC plus NUMBER, RANGE and SELECT controls, and whether separately labelled host-owned Remember-on-device and Ask-to-change controls may persist or send an explicitly selected snapshot after consent/review. No generic action/form submission, autosave, model-triggered persistence or implicit provider disclosure. This is a proposed extension to B-40 and is not accepted yet. | Product, Presentation Runtime, Security, Accessibility, Privacy, Storage and Conversation; T01 before T02 schema/API promotion. |
| Saved View representation | The product owner decided to keep Saved Views in V1 and amend Feature 007 B-21 on 2026-11-17. Feature 007 revision 1.1 proposes one bounded non-executable Artifact media type with exact source lineage, semantic fallback, immutable update from a new committed presentation, receipt/recovery, and archive-only removal. It preserves B-40: view-local modified values are neither persisted nor sent; no SavedView side table, route owner or permanent navigation destination is added. The detailed Artifact/Library/API/schema contract is in Feature 007 `contracts/saved-view.md`; its cross-owner review remains open. | Feature 007, Presentation Runtime, Storage, Privacy, Conversation, Backup and Product; T25 review before T01. |
| Local remembered answer state | Include only after explicit opt-in, per-device scope, version/digest partition, bounded quota, replay receipts, forget fence, backup/retention and deletion/revocation behavior are accepted. | Presentation Runtime, Storage, Feature 011, Conversation/Workspace retention, Backup; T22. |
| Native intent input | Text path always works; structured proposals require exact provider capability evidence and A-85 input/sequence/replay mapping. A2UI is an optional pinned adapter candidate, not the canonical schema. | AgentModule, Presentation Runtime, Security, API/Schema; T20. |
| Executable generated apps | Excluded from Feature 016 V1. The separate Runtime remains preview-only until its own OS process isolation and release qualification. | Feature 007, Runtime, Security, Feature 019; no scope implied by this proposal. |

### 20.10 New and revised EARS acceptance criteria

AC-001..AC-019 remain the Feature 016 foundation, subject to the revised contract promotion. The following criteria replace the former §19 wording for AC-020..AC-028 and add AC-029..AC-041. They are all TARGET until T01 and the named cross-owner gates are accepted.

- **AC-020:** WHEN a selected native route has no qualified UI-structure ingress, THE SYSTEM SHALL preserve and render its complete semantic answer using only deterministic, supported formatting, without hidden model calls, provider switching or native configuration writes.
- **AC-021:** WHEN a provider reports an arbitrary function, UI schema, HTML, JavaScript, handler, or out-of-scope intent, THE SYSTEM SHALL treat it as untrusted data, reject or safely display it, and SHALL NOT dispatch host operations or change host state.
- **AC-022:** WHEN a user expands or collapses an eligible answer, THE SYSTEM SHALL preserve the same committed message, RichPresentation digest, local state revision and controller across inline/Canvas surfaces, restore focus predictably, and SHALL NOT invoke an agent or app runtime.
- **AC-023:** WHEN a metric or chart is published, THE SYSTEM SHALL bind each value to an exact semantic table or authorized result and require explicit measure, units/currency/scale, time interval and aggregation; missing or contradictory semantics SHALL produce a table or clarification instead of a guessed metric.
- **AC-024:** WHEN an interactive calculation is published, THE SYSTEM SHALL use only B-40's bounded CalcAst or another explicitly registered and owner-reviewed deterministic kernel; unknown operations, including unregistered `sipFutureValue`, SHALL be rejected.
- **AC-025:** WHERE the owner enables Remember on this device, THE SYSTEM SHALL persist only allowlisted primitive inputs for the exact Workspace/Conversation/message/presentation digest after explicit consent, with expected revision, RequestId, body digest, bounded quota and a durable receipt; state SHALL NOT be sent to an agent or stored in browser localStorage.
- **AC-026:** WHEN Forget, deletion, revocation or Workspace removal races a remembered-state write, THE SYSTEM SHALL fence late writes and acknowledgements, remove protected live data, and prevent restart/backup restore from resurrecting it.
- **AC-027:** IF a later accepted B-46 contract enables Ask to change this view, WHEN the user chooses it, THE SYSTEM SHALL preview the exact user-authored text and every separately approved context field, require explicit ordinary Conversation Send, create a new message/presentation with host-recorded parent/base lineage, and SHALL NOT mutate the source message or send on input change. Under the current accepted B-40 boundary, session-modified calculator values are excluded from the send.
- **AC-028:** WHEN a user saves or reopens an interactive answer, THE SYSTEM SHALL distinguish a message-bound RichPresentation, optional separately gated remembered answer inputs, a versioned Saved View Artifact, and executable INTERACTIVE_APP/runtime data. Save/Open/Update/Archive SHALL be shown only when Feature 007 serves the corresponding authorized operation and receipt; opening shall reauthorize the exact version, restore only the presentation's original declared defaults, and never imply code execution. Session-modified values remain transient under B-40.
- **AC-029:** WHEN presentation policy is AUTO, SIMPLE or RICH, THE SYSTEM SHALL apply one versioned deterministic host policy, capture the preference on the turn for retry, preserve essential semantic content and functional owner controls, and make no mandatory second model call.
- **AC-030:** WHEN the renderer receives a component document, THE SYSTEM SHALL accept only the closed, versioned component profile promoted in the canonical RichPresentation schema; unknown kind, field, binding or control SHALL fail closed and preserve semantic fallback.
- **AC-031:** WHEN RichPresentation displays a number, source, unit/scale, time interval and operation SHALL remain inspectable; any value not derivable from committed semantic content or an authorized exact-version result SHALL be rejected.
- **AC-032:** WHEN an admitted NUMBER, RANGE or SELECT control changes, THE SYSTEM SHALL update only declared local state and approved deterministic derived values; controls SHALL be keyboard-accessible, visibly labelled, bounded, resettable and unable to invoke IPC, Task, Effect, Approval, Artifact or provider operations.
- **AC-033:** WHEN a user explicitly saves an eligible view, THE SYSTEM SHALL create one durable Library Artifact through the Feature 007 owner path, bind its exact source Conversation/message/presentation digests and self-contained semantic/RichPresentation snapshot, and show success only after the idempotent durable receipt commits. Session-modified input values are excluded.
- **AC-034:** WHEN a user reopens a Saved View, THE SYSTEM SHALL read and reauthorize the exact immutable ArtifactVersion in the current Workspace, restore the original declared presentation defaults, and never silently substitute a newer version or an uncommitted local state. If the source Conversation was deleted, the self-contained Saved View remains readable while its origin link is masked or marked under Feature 014.
- **AC-035:** WHEN the user updates a Saved View from a new eligible committed presentation in the same Conversation, THE SYSTEM SHALL append a new immutable ArtifactVersion under the owner's expected-version and idempotency contract; stale conflicts preserve the review draft and require explicit confirmation. Updating from another Conversation creates a new Saved View.
- **AC-036:** WHEN the user requests a conversational modification to a Saved View, THE SYSTEM SHALL use an ordinary Conversation Send and SHALL NOT automatically include transient calculator values; updating the Library object requires a separate explicit Update from answer review and receipt. User-provided message text remains subject to the normal Conversation contract.
- **AC-037:** WHEN presentation composition or a component fails, THE SYSTEM SHALL leave the committed Conversation settlement unchanged, keep the semantic answer visible and record only bounded identifiers/reasons without logging content or credentials.
- **AC-038:** WHEN an answer or component is generated from a native provider, THE SYSTEM SHALL retain producer/session/source provenance and SHALL NOT render reported native activity as LiteCowork-verified Task, Effect, Approval, progress or completion.
- **AC-039:** WHILE a provider is streaming or a RichPresentation is not committed, THE SYSTEM SHALL render no partial executable or stateful component tree; reconnect and stream gaps SHALL rehydrate from the committed message/presentation projection.
- **AC-040:** WHEN a Saved View itself or separately owned remembered state is inaccessible, stale, denied, deleted, corrupt or has an unknown schema, THE SYSTEM SHALL show a truthful unavailable/recovery state and preserve any unsaved draft without rendering stale protected data. Deleting the source Conversation masks or marks Saved View provenance but does not make its self-contained snapshot unavailable.
- **AC-041:** WHEN the user opens Canvas, saves, reopens, updates, forgets or modifies a view, THE SYSTEM SHALL preserve conversation scroll/composer state, use the owning Feature 007/008/011/001 command as applicable, and SHALL NOT add a permanent Saved Apps sidebar destination or create a duplicate object/state owner.

### 20.11 Delivery sequence and release gates

1. **Owner decision and semantic promotion:** resolve B-46 and Saved View representation (T25), then accept this whole revised scope (T01); reconcile Rich Response, Presentation Runtime, Feature 007 Artifact, Workbench Canvas, V1 product/UI contracts, canonical schemas and API/event owners in T02 before code.
2. **Safe output foundation:** Rust/TypeScript validator parity and hostile corpus; Markdown and TEXT_SLICE renderer; deterministic binder and complete semantic fallback; chart/table and B-40 calculator; explicit producer/source provenance.
3. **Interactive workspace:** registered bindings/reducer, inline-to-Canvas controller, accessibility and exact-source metric checks. No code execution or host actions.
4. **Durable continuity:** opt-in answer-state persistence with receipts and forget fences (T22); Saved View create/reopen/versioned update using Feature 007 (T23); ordinary-turn conversational modification with explicit preview/send.
5. **Qualification:** E2E-11 remains the presentation-disabled/malicious-content fallback journey. T02 should add and assign a separate rich-answer-to-Canvas continuity journey (candidate E2E-18; ID must be checked against the global plan), then T24 must exercise accepted capabilities including provider fallback, denial, duplicate/replay, timeout-after-commit, stale digest/version, two-window update, forget/save race, deletion/revocation, crash/restart/reconnect, keyboard/zoom/screen-reader on exact Windows and Linux release rows. Saved View and remembered-input steps are included only if their T25/T01/T02 owner gates accept them.

The candidate journey begins with an ordinary turn through a qualified native route, commits semantic content before any rich projection, reads the same valid view inline and in Canvas, changes a bounded input locally, then verifies source identity, keyboard/focus/scroll/composer continuity, safe fallback and no hidden provider call. Conditional steps cover explicit consent and restart recovery for remembered inputs, Feature 007 Saved View create/reopen/version conflict, and preview-then-Send modification as a new ordinary Conversation turn. Negative/recovery cases include unsupported ingress, invalid/unbound data, denied save/state operations, timeout after possible commit, duplicate replay, source deletion, restart/reconnect and stale version. T02 must remove steps whose owner capability is deferred; no test ID here is canonical until T02 updates the global E2E plan.

Feature 016 cannot be VERIFIED until the semantic answer, rich renderer, interactive local state, durable Saved View lifecycle and all required cross-owner journeys are qualified on each declared row. Sandboxed custom app execution is not a substitute and remains separately PREVIEW_ONLY.

### 20.12 Traceability

| Outcome | Owner | Task | Acceptance |
|---|---|---|---|
| Semantic message and retry identity | Feature 001 | T08/T16 | AC-004, 017, 020, 037-039 |
| Intent ingress and component/metric contract | Feature 001/002, Presentation, API/schema | T02/T20 | AC-020/021/023/024/029-032/038 |
| Inline renderer and safe fallback | Presentation/Desktop UI/Security/Accessibility | T03-T07/T12/T18 | AC-001-010, 030-032, 037, 039 |
| Contextual Canvas | Feature 008 and Presentation | T21 | AC-022/041 |
| Remember/Forget answer state | Presentation/Storage/011/014/017 | T22 | AC-025/026/040 |
| Saved View Artifact and Library | Feature 007, Presentation, Storage | T23 | AC-028, 033-036, 040/041 |
| Packaged release qualification | Feature 019, Product and repository owner | T16/T24 | AC-001-041 |
