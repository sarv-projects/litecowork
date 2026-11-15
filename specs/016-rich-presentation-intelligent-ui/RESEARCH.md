# Research and Decision Log - Rich Presentation, Intelligent UI and Generated Media

**Feature:** 016 | **Status:** IN_REVIEW - authored for owner review; does not authorize implementation.
**Source cross-check:** 2026-11-12, active source checkpoint `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`; documentation HEAD before this continuation was `815ec0ffeb2e9b1f7fe55e602e049224f75826a7`. The presentation/storage/bridge source paths have no source diff from the earlier source check; this documentation review changed no product source. **Upstream pin:** `f6e4388b26a64b9a0714943b08a1ce622b924eec` from the reference index; that pinned source was not re-fetched for this revision.

## 1. Repository sources inspected

| Source | Path | Finding |
|---|---|---|
| Desktop validator | `apps/litecowork-ui/src/presentation/rich-presentation.ts` | 512 KiB, 200 blocks, depth 8, 30,000 values, JSON depth 32; five kinds; RFC 8785 and SHA-256 checks; null on any failure |
| Renderer | `apps/litecowork-ui/src/presentation/RichResponseView.tsx` | Five kinds; table region with `tabIndex=0`; semantic disclosure |
| Rust validator | `crates/storage-sqlite/src/rich_presentations.rs` | Five baseline kinds; closed objects; canonical bytes; provenance rules |
| Store contract | `crates/storage-core/src/rich_presentation.rs` | 512 KiB cap; one immutable record per message; page limits |
| Operator | `apps/litecoworkd/src/presentation_operator.rs` | Read routes and stream; publish used only in tests |
| Schemas | `docs/schemas/rich-presentation.schema.json`, `presentation-intent.schema.json`, `interactive-artifact-runtime.schema.json` | 13 kinds in v1 schema; intent strategies; runtime IPC schema |
| Owner docs | `docs/RICH-RESPONSE.md`, `PRESENTATION-RUNTIME.md`, `INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `INTERACTIVE-ARTIFACT-RUNTIME.md`, `RENDERING-CONFORMANCE-LLD.md` | Display-only v1; CalcAst; chart table fallback; image pipeline; PREVIEW_ONLY |
| Plans | `docs/IMPLEMENTATION-MASTER-PLAN.md` P10, `docs/LITECOWORK-BUILD-TASKS.md` BT-06.3 to BT-06.5, `docs/ACCEPTANCE-AND-E2E-PLAN.md` E2E-11 | Exit: semantic answer complete when rendering disabled; malicious presentation cannot mint host state |
| E08-S09 | `docs/archive/legacy-implementation-c7e9c05/epics/E08.md` (historical) and `docs/BENCHMARKS.md` | "Isolated interactive Artifact Runtime" story that owns Run qualification |
| CSP | `apps/litecowork-ui/src-tauri/tauri.conf.json` | `script-src 'self'`; `img-src 'self' asset: http://asset.localhost data:` |
| Dependencies | `Cargo.lock`, `apps/litecowork-ui/package.json` | No jsonschema crate; no chart library; React 19 only |

## 2. Pinned external reference

| Reference | Pin | License | Use |
|---|---|---|---|
| REF-OPENINTELLIGENTUI `CopilotKit/OpenIntelligentUI` | `f6e4388b26a64b9a0714943b08a1ce622b924eec` | MIT at checked root LICENSE | UX, sandbox lifecycle, explicit bridge and renderer-test ideas only |

Source rows: [docs/REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md) (REF-OPENINTELLIGENTUI) and [docs/SOURCE-RECONCILIATION.md](../../docs/SOURCE-RECONCILIATION.md) "OpenIntelligentUI review for E08-S08". No `repo_take/` clone exists.

### Copy / prototype / reject for this feature

| Mechanism | Verdict | How it applies here |
|---|---|---|
| Renderer behaviour tests (`renderer.tsx`) | copy test ideas only | Hostile-input and fallback cases in the shared corpus; no source reuse |
| Schema shapes (`schema.ts`) | test-shape reference only | Cross-check corpus coverage; LiteCowork schema remains canonical |
| Host-action and failure tests (`sandbox-functions.ts`, `prompt-bridge.tsx`) | reference | Bridge-call spy test proving no rendered element changes host state |
| Standalone prototype speed | prototype (outside the product) | Allowed only for throwaway UX exploration; never shipped |
| Arbitrary generated HTML/CSS/JavaScript renderer | reject | Violates L-03 and AGENTS.md section 8 |
| `eval`, `unsafe-eval`, `unsafe-inline` scripts, CDN origins in its CSP | reject | CSP stays `script-src 'self'` |

## 3. Alternatives considered

| Topic | Option | Decision |
|---|---|---|
| Validator authority | Keep validation in `storage-sqlite` | Rejected: a pure crate is reusable and testable; storage calls it |
| Schema engine | Add the `jsonschema` crate | Rejected for V1: hand-written closed objects are the repo pattern and keep errors typed; revisit with RESEARCH justification |
| First Intelligent UI component | TABS, ACCORDION, DATA_TABLE, SAFE_CALCULATOR | SAFE_CALCULATOR proposed (B-40): it is the only one that exercises the restricted AST and reducer BT-06.4 asks for |
| Chart rendering | Chart library, Mermaid, model SVG, host React SVG | Host React SVG: no dependency, no model markup |
| Separate IntelligentUiDocument | Second document per message | Rejected: one validated RichPresentation per message with renderer contract version 2 keeps one digest binding and one fallback path |
| Media in presentation blocks | MEDIA block in V1 | Deferred: images are Artifacts shown in the Artifact view; MEDIA block can follow once the display scheme is settled |
| Interactive Artifact Run | Enable behind a flag | Rejected: release contract requires E08-S09 evidence first |

## 4. Optimization notes

- The TypeScript validator computes one SHA-256 per TEXT_SLICE; for 200 slices this is the main cost. If AC-016 fails, hash the encoded document once and verify slice ranges against that buffer instead of re-encoding.
- Move validation to publish time in Rust so the desktop's work is a confirmation pass on already-valid documents; rejected documents never cost render time.
- Keep chart geometry and the calculator reducer as pure TypeScript modules so they run under `node --test` without a DOM.
- Do not split rendering into a Web Worker unless the 100 ms long-task budget fails on the release-row machine.

## 5. Open research and decision register

| ID | Question | Status |
|---|---|---|
| R-01 | Does OpenCode v2.0.32 emit structured presentation intent? | Open; binder derives from Markdown first |
| R-02 | Image display scheme without loosening CSP | Open; Feature 011 |
| R-03 | EXIF stripping for display copies | Open; Security |
| B-40 | First component and Interactive Artifact Runtime posture | ACCEPTED as proposed 2026-11-10; whole package remains IN_REVIEW |
| A-84 | Baseline versus extension acceptance scope | DESIGN CONFLICT RESOLVED 2026-11-12; owner decisions remain pending at T01 (baseline only) and T25 (extension only) |
| A-85 | MCP model-visible input and `submitted_at_sequence` provenance | OPEN; T20 must define the canonical input mapping and trusted sequence/replay source before tool exposure |

## 6. Official interoperability research, checked 2026-11-12

Documentation findings are design research, not dependency adoption or provider qualification. Mutable pages below have no recorded immutable SHA/package lock; T20 must pin implementation versions, license and conformance corpus before adopting code or claiming protocol compatibility. No dependencies were installed.

| Primary source | Useful mechanism | Boundary/rejection | Version gate |
|---|---|---|---|
| [assistant-ui generative UI](https://www.assistant-ui.com/docs/tools/generative-ui) | Host component vocabulary; known-tool UI and backend-produced presentation parts are distinct patterns | Do not import its inference/tool runtime, arbitrary widget generation or assume its spec/present shapes are interchangeable; existing React/Tauri remains | Mutable docs; package/tag/SHA/license not pinned |
| [json-render introduction](https://json-render.dev/docs) | Catalog-constrained JSON rendered by host components; data bindings and computed values can separate data from layout | Existing rich-presentation schema remains authority; no second spec engine, arbitrary action executor or automatic code export | Mutable docs; package/tag/SHA/license not pinned |
| [A2UI 1.0 protocol](https://github.com/a2ui-project/a2ui/blob/main/specification/v1_0/docs/a2ui_protocol.md), [official launch post](https://developers.googleblog.com/a2ui-10-build-delightful-fast-reliable-agentic-experiences/), [public index](https://github.com/a2ui-project/a2ui/blob/main/docs/public/index.md), [SDK table](https://a2ui.org/concepts/sdks/), [client setup guide](https://github.com/a2ui-project/a2ui/blob/main/docs/public/guides/client-setup.md), [renderer status matrix](https://a2ui.org/renderers/), [repository README](https://github.com/a2ui-project/a2ui/blob/main/README.md), [roadmap](https://github.com/a2ui-project/a2ui/blob/main/docs/public/roadmap.md), [published React package](https://www.npmjs.com/package/@a2ui/react) | Agent-described components/data, incremental surfaces, host catalogs and bidirectional function-call messages; the protocol index and launch post mark v1.0 current/stable. Current SDK and client-setup tables list React v1.0 support and stable status, while the renderer status matrix, older README/roadmap and last retrieved npm package documentation still describe v0.9.1 as production/current and v1.0 as planned/candidate | Optional adapter candidate, not an adopted LiteCowork runtime: ACP does not define an A2UI content type or guarantee its transport, and LiteCowork already owns canonical intent, RichPresentation, validation and view state. Reduce A2UI to the accepted display/local-interaction subset; no A2UI function call dispatches a Core operation. Official material conflicts on released renderer support; the npm page could not be fetched on 2026-11-13 (HTTP 403), so its last retrieved v0.9-era claims are historical. Before adoption, pin the exact published tarball/source tag, versioned imports and protocol conformance fixtures against the selected allowlisted catalog. This package conflict does not block the current custom validated adapter design | Mutable official pages rechecked 2026-11-13; npm last retrieved 2026-11-12 and unavailable on recheck; exact package/source conformance remains unverified |
| [AG-UI event concepts](https://github.com/ag-ui-protocol/ag-ui/blob/main/docs/concepts/events.mdx) | Distinguish lifecycle/text/tool-call/state snapshot/delta families; it can transport live interaction events | AG-UI is an agent/frontend event contract, not a component catalog or UI description. Keep the current Operator frame contract; no raw provider state authority, rawEvent/private reasoning leakage or AG-UI replacement transport | Mutable docs; version/SHA not pinned |
| [MCP Apps overview](https://apps.extensions.modelcontextprotocol.io/api/documents/Overview.html) | Explicit host capability negotiation, tool/resource separation and plain-result degradation | Apps are sandboxed HTML-resource guests with a bridge, separate from native declarative components; not proof of OS isolation; V1 app runtime stays PREVIEW_ONLY | Returned docs label ext-apps v1.1.2; exact source/package integrity still unpinned |
| [ACP v1 content](https://agentclientprotocol.com/protocol/v1/content), [tool calls and permission requests](https://agentclientprotocol.com/protocol/v1/tool-calls), [elicitation](https://agentclientprotocol.com/protocol/v1/elicitation), [ACP v2 draft](https://agentclientprotocol.com/protocol/v2/overview) | Content blocks carry text/Markdown, images, audio and resources; `tool_call` / `tool_call_update` report agent-owned activity; v1 defines permission requests and structured form/URL elicitation | ACP does not define a component tree or declarative interactive-view schema. Tool status, permission choices and elicitation responses are protocol UX, not LiteCowork authorization. Structured UI must use a separately qualified app contract and map to existing PresentationIntent. Keep v2 draft lifecycle/elicitation changes out of the v1 adapter. OpenCode HTTP support cannot be inferred from ACP. | v1 stable docs plus v2 draft checked 2026-11-12; exact provider/version capture required |
| [OpenCode ACP client reference](https://opencode.ai/docs/cli/acp/) | OpenCode documents stdio/HTTP MCP server injection on ACP session requests | Current docs state supplied servers live in the session directory for the lifetime of the `opencode acp` process and can be used by other sessions in that directory; same-name servers replace configured ones. This is not session isolation. Never apply latest-doc behavior to pinned 2.0.32 without a matching tagged-source or runtime test | Mutable official docs checked 2026-11-12; exact 2.0.32 behavior is unverified |

## 7. Product-interface comparisons (checked 2026-11-13)

These official product pages are evidence about visible workflows, not permission to copy private implementations or replace LiteCowork's architecture.

| Product evidence | Pattern worth using | LiteCowork decision |
|---|---|---|
| [ChatGPT desktop navigation](https://help.openai.com/en/articles/20001276-moving-to-the-new-chatgpt-desktop-app) | Groups Chat and Work under one destination, keeps their conversations together in Recents, and separates Codex as a distinct working surface | Keep one conversation-first workspace, put task work in Work and compact secondary destinations behind disclosure; do not copy the separate Codex app boundary because LiteCowork's BYOA Conversation is its core path |
| [Claude Artifacts](https://support.claude.com/en/articles/17153992-what-are-artifacts-and-how-do-i-use-them) | Current docs distinguish legacy chat artifacts from newer templates. New output can open beside chat, be edited directly or conversationally, and be found in a dedicated Artifacts area. Docs, Slides, Design, Data and beta Sheets/Whiteboards/Motion templates extend beyond a single code preview. App-connected data/actions request user permission; personal/shared storage is explicit and bounded | Keep a contextual Canvas and a Library destination. Preserve local-first ownership, immutable message-bound RichPresentation, and Artifact provenance/receipt owners. Cloud app actions, generated code, sharing and durable app state are separate capabilities and do not enter the V1 presentation runtime by inference |
| [ChatGPT writing blocks and code previews](https://help.openai.com/en/articles/20001246-working-with-writing-blocks-and-code-blocks-in-chatgpt), [Canvas launch design](https://openai.com/index/introducing-canvas/) | Drafts can remain editable in the chat, expand to a focused full-screen editor, and support selected-text changes and undo/redo. Supported code previews separate source from rendered result | Use progressive expansion and precise selection/context as UX references. Keep LiteCowork's semantic answer authoritative and its renderer host-owned; ChatGPT's supported HTML/React/SVG/code-preview sandbox is not permission to execute model markup in LiteCowork's main WebView |
| [ChatGPT Data Analysis tables and charts](https://help.openai.com/en/articles/9213685-extracting-insights-with-chatgpt-data-analysis) | Uploaded tables support visible row/column/cell selection and follow-up analysis; charts can expand/export and some chart types are interactive | Useful pattern for inspectable selection and progressive analysis. Feature 016 currently supports a chart bound to a semantic-message TABLE and one bounded calculator, not a general dataframe/query engine, arbitrary chart family, selected-cell prompt injection or CSV export. A broader file-analysis path needs an explicit Feature 003/007 owner for dataset identity, transformations, source receipts, user-confirmed selection/send and recovery; do not imply it from RichPresentation |
| [Gemini document editing in chat](https://blog.google/innovation-and-ai/products/gemini-app/edit-files-directly/) | The 2026-10-29 product update describes creating, previewing and editing documents from chat, expanding to full screen and selecting text for revisions; PDF editing is web/mobile while the other listed document/code formats are web-only | This confirms demand for an answer that can become an editable work surface. It does not change Feature 007's accepted V1 Markdown-only professional format or qualify PDF/Office edit engines in LiteCowork |
| [Anthropic computer-use data description](https://privacy.claude.com/en/articles/10030352-what-personal-data-will-be-processed-by-computer-use) | The visible model computer-use loop can rely on screen screenshots and explicit computer interaction; screen content is a distinct data flow | LiteCowork must make local preview, provider disclosure and input grants separate, visible permits. A product page about screenshots is not evidence of low-latency video, local-only processing or reliable takeover |
| [MCP Apps](https://apps.extensions.modelcontextprotocol.io/api/documents/Overview.html) | Tools can optionally provide an interactive embedded surface, with a host-controlled guest boundary | Keep this as a future isolated app/tool surface. It is not the default renderer for agent answers and cannot run in the main chat WebView until Feature 016/019 prove the separate runtime and fallback |

These patterns support a compact sidebar, contextual object tabs and progressive expansion rather than a permanent terminal, file tree, activity log and agent graph. The default remains ordinary Conversation; a meaningful chart/form/document can appear inline and expand into the same contextual Canvas with state preserved. A professional deliverable enters Library only through the Artifact owner path and its provenance/version rules. The protocol roles stay distinct: ACP carries native-agent sessions; AG-UI standardizes runtime/frontend event exchange; A2UI describes a declarative surface/data model; MCP Apps embeds a tool-owned HTML surface. They are not drop-in replacements for one another. A2UI can help an ACP-connected agent describe UI only when the provider emits that payload or a qualified tool/adapter carries it; ACP alone does not make ordinary agent text render as A2UI. Any A2UI ingress must be mapped into the canonical Feature 016 model with unknown kinds/functions rejected, and semantic text must survive mapping failure. They do not settle screen-capture APIs, safe MCP server scope, authorization, persistence or release qualification; LiteCowork's owner contracts still decide those semantics.

[Pipeline LLD](../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) records adoption decisions and missing pins as T20 gates. Native source cross-check: `acp_client.rs::session_new` sends an empty MCP list; `adapter.rs::parse_acp_session_update` retains text and tool title/status only; no structured UI ingress exists there. Current canonical intent has no calculator variant. No hidden second LLM pass is needed: selected native harness may propose through a provider-qualified structured path, Rust deterministically binds, registered React renders, and any malformed/unsupported proposal falls back to semantic text.

## 8. User-provided universal intelligent workspace proposal cross-check (2026-11-15)

The supplied product narrative and ChatGPT Work screenshots were checked against `ARCHITECTURE.md`, Product/V1 contracts, Feature 008, Feature 016, the ACP presentation pipeline LLD, and active UI/provider source. Screenshots and rival-product descriptions are design references, not authority about private implementation or evidence that a LiteCowork provider supports the same behavior.

| Proposal idea | LiteCowork disposition | Existing owner contract / consequence |
|---|---|---|
| Conversation, inline interactive answer, and expanded Canvas are views over one continuing work history | Retain as target; expansion preserves semantic message, presentation identity and local reducer state | Workspace Experience Revision and Feature 016 §19/AC-022 (IN_REVIEW); no new Conversation or runtime is created by expansion. |
| Right side is a contextual workspace for the current object and work, including live computer preview when authorized | Retain. Opening an object reauthorizes through its owner and cannot start capture, spawn a process or dispatch work. Computer preview needs a real provider and separate local-observation permit. | Feature 008 revision 1.1 and `docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md` are TARGET / IN_REVIEW; current UI has no live Computer provider. |
| Native components, declarative bindings and deterministic local calculations | Retain only through the canonical RichPresentation allowlist. The accepted first interactive component is bounded `SAFE_CALCULATOR`; its input state is view-local under B-40. | B-40 is accepted as proposed; the full Feature 016 baseline is still IN_REVIEW and AC-020..AC-027 need the separate T25 gate and canonical contract promotion. |
| Convert provider output to useful visual hierarchy | Retain as an optional host-side binder over committed semantic content. Prefer deterministic table handling and admit only qualified, correlated native intent. Invalid/unsupported presentation keeps the semantic answer intact. | Feature 016 §6/§19 and ACP presentation pipeline LLD; no production publisher exists. Hidden second-model calls and provider switching are excluded. |
| Save interactive state and revise an existing application conversationally | Keep separate from immutable presentation identity. Persistence is off and requires consent plus retention/deletion decisions; conversational changes are previewed and sent as a new ordinary turn with lineage. | Feature 016 AC-025..AC-027 are proposed under T22/T23/T25. B-40 calculator state is not persisted or sent to the agent. |
| Sandbox arbitrary generated mini-apps as an escape hatch | Exclude from V1. Interactive Artifact Runtime remains inert `PREVIEW_ONLY` on both release rows; model HTML/JavaScript does not execute in the chat WebView. | B-40, `docs/INTERACTIVE-ARTIFACT-RUNTIME.md`, and `AGENTS.md` §8. MCP Apps remain a distinct, tool-owned guest model. |
| Adopt assistant-ui, json-render, A2UI, AG-UI, Next.js, or a second schema wholesale | Do not adopt a stack or competing schema from this proposal. Existing React/Tauri and RichPresentation remain. A2UI is at most an optional future adapter after exact package and transport conformance; ACP alone guarantees no UI payload. | Interoperability research above, ACP presentation pipeline LLD, and Feature 016's single-schema rule. |
| Make hosted BYOK/model routing the product center | Exclude from this product direction. LiteCowork remains local-first, chat-first, BYOA-first; native harnesses own auth, models, reasoning, tools and private transcripts. | `ARCHITECTURE.md`, `docs/PRODUCT.md`, and `docs/V1-PRODUCT-RELEASE-CONTRACT.md`. A hosted router is a separate product/trust-boundary decision. |

This cross-check adds no implementation authorization, API/event/schema, dependency, model call, or qualification claim. The universal product direction and compact-navigation/contextual-pane review remain TARGET / IN_REVIEW. It does not pass any pending Feature 008 or 016 owner gate.
