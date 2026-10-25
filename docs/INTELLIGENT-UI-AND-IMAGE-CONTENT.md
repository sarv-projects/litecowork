# Intelligent UI and Image Content Contract

**Owning story:** E08-S08 — Semantic-first RichPresentation and Host Guidance.

**Status:** Normative V1 desktop product target; the current implementation audit below is factual as of 2026-10-25. This document does not claim the target feature is implemented.

## Product meaning

Intelligent UI means a response can combine its readable answer with a visual, interactive interface that helps the user understand or act on the answer in the conversation. It is a response mode, not an image model and not a synonym for a generated website.

Keep these outputs separate:

| Output | User sees | Authority |
|---|---|---|
| Semantic response | Complete readable answer in the chat | The committed ConversationMessage |
| RichPresentation v1 | Validated static text, layout, card, callout, or table blocks | Optional display enhancement; no actions |
| Generated image | A stored image with preview, alt text, and provenance | Exact ArtifactVersion or Resource reference |
| Intelligent UI | A bounded interactive view, such as tabs, a calculator, a chart explorer, or a small form | A new versioned declarative presentation contract, or a separately sandboxed interactive Artifact |
| Interactive Artifact | A generated app bundle opened from the Workbench and run after the owner chooses Run | The isolated Artifact Runtime |

An image is visual content. Intelligent UI adds state and interactions. A rich response can be visual without being interactive. An interactive Artifact is a larger executable app and remains a separate security boundary.

## Current implementation audit

The user-facing Conversation pipeline does not yet deliver Intelligent UI:

- Conversation message content parsed by the desktop currently accepts TEXT and RESOURCE blocks. It has no inline image input block or agent-generated image attachment path.
- The Conversation page loads an optional immutable RichPresentation and renders it through RichResponseView. The current desktop block allowlist is TEXT_SLICE, LAYOUT, CARD, CALLOUT, and TABLE. Those blocks are display-only. There are no tabs, inputs, client state, action buttons, or image blocks in this renderer.
- The general PresentationRuntime has IMAGE and MCP_APP cases, but IMAGE shows an artifact/resource fallback placeholder and MCP_APP says interactive app content is unavailable in that renderer. These are not generated inline image or app rendering.
- The presentation stream reducer and Tauri stream bridge exist. The operator has no native Conversation turn producer for TURN_DELTA, TURN_SETTLED, or RICH_DRAFT. Rich draft state is not displayed. Reconnection code alone is not live agent streaming.
- Conversation submit persists the user message and turn, then fails admission with 503 because no qualified native-session supervisor is connected. Retry and cancel are unavailable. The composer Send button is disabled.
- INTERACTIVE-ARTIFACT-RUNTIME.md defines an isolated Artifact Runtime target and explicitly does not claim code or operating-system qualification. Its contract must not be described as a current Conversation renderer.

Therefore current source contains a safe static rich-response foundation, not ChatGPT-style Intelligent UI. The exact evidence and turn-lifecycle blockers are tracked in NATIVE-COMPOSER-LLD.md and implementation/V1-PRODUCT-ACCEPTANCE-MATRIX.md.

## V1 response contract

The semantic answer is committed and readable first. Rich or visual output may improve that answer, but may not replace it or delay settlement. If an optional output cannot be generated, validated, stored, fetched, or rendered, preserve the answer and show a useful plain-language fallback.

Intelligent UI is a LiteCowork product layer above every selected AgentBinding. The renderer
and normalized presentation contract do not belong to a coding-agent harness; switching
Claude Code, Codex, Cursor CLI, OpenCode, or another qualified adapter must not change the
chat's output format contract. Every adapter qualified for full Conversation use must map
responses into the common semantic-content/presentation-intent envelope. It may use the
agent's native structured output or make a clearly accounted follow-up through that same
selected AgentBinding; it must never switch providers to create UI. The common renderer,
schema, lifecycle, and fallback belong to LiteCowork. Provider-specific capabilities still
govern image input/output and executable interactive-app packages, but do not gate this
agent-agnostic declarative UI layer. If a route cannot produce a valid intent, preserve its
complete semantic answer and report that route's presentation limitation.

If an adapter cannot supply a valid presentation intent, or the document fails validation,
show the complete semantic answer with a useful fallback. Do not silently switch agent,
model, account, or credentials to generate the UI. An additional presentation-planning
inference is an ordinary, reviewable invocation through the selected binding and its cost
belongs to the same Conversation and turn. UNKNOWN, missing, stale, NOT_EXPOSED, and
UNSUPPORTED capability observations never authorize provider-specific image or app output.

### Inline declarative interface

Add a new, versioned schema for interactive response documents. Do not add behavior to RichPresentation v1 or weaken its closed renderer contract. The new schema may support bounded layout, text, tables, charts, images by immutable local reference, tabs, disclosures, local form inputs, and safe local calculations. Every node has a stable ID, accessible label, bounded data, explicit source references, and deterministic fallback text.

The schema contains declarative data only. It does not accept HTML, JavaScript, CSS, arbitrary URLs, event handlers, eval, remote modules, or code supplied by an agent. Client-side interactions may update local view state or calculate from the validated document. Actions that send another prompt, open a stored Artifact, or change host state must use explicit host-owned controls and their normal authorization path. A visual element cannot grant capabilities, approve requests, write files, invoke tools, or create a Task by itself.

Bind the document to the exact Conversation, message, turn, AgentSession, binding, and immutable source references. Validate schema version, canonical digest, byte and node limits, nesting, identifiers, source ownership, and accessible names before rendering. Use a distinct versioned event and API contract for publication and recovery. Partial streaming may show text and validated sealed blocks only; an incomplete or invalid interactive control is never activated.

### Generated images

Image input and image output are independent route capabilities. An input image must be imported through the desktop's explicit file picker, size/type checks, and immutable Workspace-scoped attachment reference; it must remain visible if the selected native agent cannot accept it. The adapter sends it only through that agent's documented native input path.

An output image must be produced by the selected route's supported native image capability or by a separately selected image-generation integration. Store the bytes as an immutable ArtifactVersion or Resource, verify digest and media type, and render from an authorized local reference. Do not persist arbitrary provider URLs, base64 blobs, or executable SVG as chat HTML. The message keeps alt text, origin, exact artifact/resource version, and a fallback link when inline decoding or preview is unavailable.

### Interactive Artifact

Use the separately specified Interactive Artifact Runtime for a full app bundle, code execution, persistent app data, or tool-backed behavior. Validate and store the package as an immutable ArtifactVersion. Static preview and execution are separate. Opening a result never runs it; the user chooses Run. The app receives no ambient filesystem, shell, credential, browser, network, or native-MCP access. User-approved capabilities remain brokered and revocable. If the runtime is not qualified on the current OS, keep the result as source/static preview and say why Run is unavailable.

## Chat-panel behavior

- Keep the Conversation as the main surface. Render compact, trustworthy visual content inline with its answer; open larger artifacts in the right Workbench without replacing chat history.
- Keep native-agent setup visible during onboarding and in Settings. In ordinary use, show the selected agent/model and a clear cost or usage affordance, while keeping protocol names and debug detail out of the primary response.
- Preserve native slash commands and @ references exactly as advertised by the selected harness. Host actions stay in their own menu and never impersonate native commands.
- Show concise progress, tool activity, waiting-for-you states, and stop controls only from real operator events. Reconnect restores persisted content without replaying typing animations.
- Preserve keyboard navigation, screen-reader descriptions, focus, zoom, high contrast, and reduced-motion behavior. Motion indicates a real state change and never delays Stop or approval.
- Keep source and semantic text reachable if the visual pane fails, is unsupported, or is hidden at narrow desktop widths.

## Competitor research and product implications

OpenAI describes GPT-6 Intelligent UI as fully interactive answer interfaces, with examples that combine explanatory text and selectable visual sections. ChatGPT Images 2.5 is a separate image creation and editing capability. See https://openai.com/index/gpt-6-for-everyone/ and https://openai.com/index/introducing-chatgpt-images-2-5/.

Anthropic's Cowork presents task work for review, shows work as it happens, supports a built-in browser, and separates interactive custom Artifacts that open beside the conversation. See https://claude.com/product/cowork and https://academy.claude.com/tutorials/build-custom-artifacts-with-claude.

xAI documents Grok Imagine image generation and editing as an explicit model capability. This is useful for image workflow comparison but does not establish an interactive in-chat UI contract. See https://docs.x.ai/developers/model-capabilities/images/generation.

The product implication is to compete on the whole desktop experience: truthful live progress, a polished semantic conversation, safe inline visuals, reviewable files, and optional interactive results. A provider/model switcher alone does not supply these user-facing capabilities.

## Acceptance evidence

Do not mark this feature complete until one exact, pinned native route passes all applicable gates:

1. Send, native readiness, dispatch, streaming, stop, retry, and daemon-restart recovery complete as a real owner-visible Conversation flow.
2. Semantic text remains useful when rich generation, validation, or rendering fails.
3. A supported route publishes and restores a valid inline interactive document; unsupported and stale routes fall back without hidden provider switching.
4. An image input reaches only a route that declares image-input support. An output image is stored, authorized, previewed, and exportable by exact immutable reference.
5. An app bundle remains inert until the owner selects Run in a qualified isolated Artifact Runtime.
6. Keyboard, screen reader, high contrast, zoom, and reduced-motion behavior are verified on the packaged desktop build.
7. Cost, route, output capability, and failure state are truthful and reviewable.

CODE evidence uses hostile schema and storage fixtures. SYSTEM evidence exercises exact native harness/model/version and recovery on the pinned OS. USER evidence verifies the complete desktop flow, including review and accessible fallback. A renderer unit test or static design draft alone is not acceptance evidence.
