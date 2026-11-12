# Research and Decision Log - Rich Presentation, Intelligent UI and Generated Media

**Feature:** 016 | **Status:** IN_REVIEW - authored for owner review; does not authorize implementation.
**Source cross-check:** 2026-11-12, current checkout at `8432397befc69bafd3335a0c30b2f1e9e82bef5e`. The presentation/storage/bridge source paths in the evidence table have no source diff from the prior `64c435149a82ba952c6c3bf3049e26cf2d9344b1` check; no source implementation changed in this documentation review. **Upstream pin:** `f6e4388b26a64b9a0714943b08a1ce622b924eec` from the reference index; the upstream source was not re-fetched for this revision.

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
| [A2UI 1.0 protocol](https://github.com/a2ui-project/a2ui/blob/main/specification/v1_0/docs/a2ui_protocol.md), [official launch post](https://developers.googleblog.com/a2ui-10-build-delightful-fast-reliable-agentic-experiences/), [public index](https://github.com/a2ui-project/a2ui/blob/main/docs/public/index.md), [README](https://github.com/a2ui-project/a2ui/blob/main/README.md), [GitHub Releases](https://github.com/a2ui-project/a2ui/releases) | Agent-described components/data, incremental surfaces, local data binding and bidirectional function-call messages; Google's 2026-11-11 post calls v1.0 stable and lists a React renderer | Candidate adapter only: no direct renderer or parallel state/schema authority; A2UI functions are untrusted proposals, never Core operations. ACP does not define an A2UI content type or guarantee its transport. The public index and launch post say 1.0 is released, while README/roadmap still call 0.9.1 the production release and 1.0 a candidate. GitHub Releases lists CLI/core/agent SDK packages, but exact React renderer package/version and its v1.0 conformance remain unverified. Pin protocol, renderer package, license and conformance before adoption | Mutable project docs checked 2026-11-12; exact repository commit and renderer package still require an immutable pin |
| [AG-UI event concepts](https://github.com/ag-ui-protocol/ag-ui/blob/main/docs/concepts/events.mdx) | Distinguish lifecycle/text/tool-call/state snapshot/delta families; it can transport live interaction events | AG-UI is an agent/frontend event contract, not a component catalog or UI description. Keep the current Operator frame contract; no raw provider state authority, rawEvent/private reasoning leakage or AG-UI replacement transport | Mutable docs; version/SHA not pinned |
| [MCP Apps overview](https://apps.extensions.modelcontextprotocol.io/api/documents/Overview.html) | Explicit host capability negotiation, tool/resource separation and plain-result degradation | Apps are sandboxed HTML-resource guests with a bridge, separate from native declarative components; not proof of OS isolation; V1 app runtime stays PREVIEW_ONLY | Returned docs label ext-apps v1.1.2; exact source/package integrity still unpinned |
| [ACP v1 content](https://agentclientprotocol.com/protocol/v1/content), [tool calls and permission requests](https://agentclientprotocol.com/protocol/v1/tool-calls), [session setup](https://agentclientprotocol.com/protocol/v1/session-setup) | Content blocks carry text/Markdown, image, audio and resource forms; `tool_call` / `tool_call_update` report agent-owned tool activity, and `session/request_permission` carries an agent request for a user choice | ACP does not define a component tree or declarative interactive-view schema. Tool status and permission options are observations/request UX, not LiteCowork authorization. Structured UI must travel through a separately qualified application contract and be mapped to the existing presentation intent; unknown content remains semantic text | v1 docs; exact schema/provider capture pin required |
| [OpenCode ACP client reference](https://opencode.ai/docs/cli/acp/) | OpenCode documents stdio/HTTP MCP server injection on ACP session requests | Current docs state supplied servers live in the session directory for the lifetime of the `opencode acp` process and can be used by other sessions in that directory; same-name servers replace configured ones. This is not session isolation. Never apply latest-doc behavior to pinned 2.0.32 without a matching tagged-source or runtime test | Mutable official docs checked 2026-11-12; exact 2.0.32 behavior is unverified |

## 7. Product-interface comparisons (checked 2026-11-12)

These official product pages are evidence about visible workflows, not permission to copy private implementations or replace LiteCowork's architecture.

| Product evidence | Pattern worth using | LiteCowork decision |
|---|---|---|
| [ChatGPT desktop navigation](https://help.openai.com/en/articles/20001276-moving-to-the-new-chatgpt-desktop-app) | Groups Chat and Work under one destination, keeps their conversations together in Recents, and separates Codex as a distinct working surface | Keep one conversation-first workspace, put task work in Work and compact secondary destinations behind disclosure; do not copy the separate Codex app boundary because LiteCowork's BYOA Conversation is its core path |
| [Claude Artifacts](https://support.claude.com/en/articles/17153992-what-are-artifacts-and-how-do-i-use-them) | Current docs distinguish legacy chat artifacts from newer templates. New output can open beside chat, be edited directly or conversationally, and be found in a dedicated Artifacts area. Docs, Slides, Design, Data and beta Sheets/Whiteboards/Motion templates extend beyond a single code preview. App-connected data/actions request user permission; personal/shared storage is explicit and bounded | Keep a contextual Canvas and a Library destination. Preserve local-first ownership, immutable message-bound RichPresentation, and Artifact provenance/receipt owners. Cloud app actions, generated code, sharing and durable app state are separate capabilities and do not enter the V1 presentation runtime by inference |
| [ChatGPT writing blocks and code previews](https://help.openai.com/en/articles/20001246-working-with-writing-blocks-and-code-blocks-in-chatgpt), [Canvas launch design](https://openai.com/index/introducing-canvas/) | Drafts can remain editable in the chat, expand to a focused full-screen editor, and support selected-text changes and undo/redo. Supported code previews separate source from rendered result | Use progressive expansion and precise selection/context as UX references. Keep LiteCowork's semantic answer authoritative and its renderer host-owned; ChatGPT's supported HTML/React/SVG/code-preview sandbox is not permission to execute model markup in LiteCowork's main WebView |
| [Gemini document editing in chat](https://blog.google/innovation-and-ai/products/gemini-app/edit-files-directly/) | The 2026-10-29 product update describes creating, previewing and editing documents from chat, expanding to full screen and selecting text for revisions; PDF editing is web/mobile while the other listed document/code formats are web-only | This confirms demand for an answer that can become an editable work surface. It does not change Feature 007's accepted V1 Markdown-only professional format or qualify PDF/Office edit engines in LiteCowork |
| [Anthropic computer-use data description](https://privacy.claude.com/en/articles/10030352-what-personal-data-will-be-processed-by-computer-use) | The visible model computer-use loop can rely on screen screenshots and explicit computer interaction; screen content is a distinct data flow | LiteCowork must make local preview, provider disclosure and input grants separate, visible permits. A product page about screenshots is not evidence of low-latency video, local-only processing or reliable takeover |
| [MCP Apps](https://apps.extensions.modelcontextprotocol.io/api/documents/Overview.html) | Tools can optionally provide an interactive embedded surface, with a host-controlled guest boundary | Keep this as a future isolated app/tool surface. It is not the default renderer for agent answers and cannot run in the main chat WebView until Feature 016/019 prove the separate runtime and fallback |

These patterns support a compact sidebar, contextual object tabs and progressive expansion rather than a permanent terminal, file tree, activity log and agent graph. The default remains ordinary Conversation; a meaningful chart/form/document can appear inline and expand into the same contextual Canvas with state preserved. A professional deliverable enters Library only through the Artifact owner path and its provenance/version rules. The protocol roles stay distinct: ACP carries native-agent sessions; AG-UI standardizes runtime/frontend event exchange; A2UI describes a declarative surface/data model; MCP Apps embeds a tool-owned HTML surface. They are not drop-in replacements for one another. A2UI can help an ACP-connected agent describe UI only when the provider emits that payload or a qualified tool/adapter carries it; ACP alone does not make ordinary agent text render as A2UI. Any A2UI ingress must be mapped into the canonical Feature 016 model with unknown kinds/functions rejected, and semantic text must survive mapping failure. They do not settle screen-capture APIs, safe MCP server scope, authorization, persistence or release qualification; LiteCowork's owner contracts still decide those semantics.

[Pipeline LLD](../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) records adoption decisions and missing pins as T20 gates. Native source cross-check: `acp_client.rs::session_new` sends an empty MCP list; `adapter.rs::parse_acp_session_update` retains text and tool title/status only; no structured UI ingress exists there. Current canonical intent has no calculator variant. No hidden second LLM pass is needed: selected native harness may propose through a provider-qualified structured path, Rust deterministically binds, registered React renders, and any malformed/unsupported proposal falls back to semantic text.
