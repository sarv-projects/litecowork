# Central Conversation Rendering, Workbench Viewers and Native Output Conformance LLD

Status NORMATIVE V1 TARGET. Authority: PRESENTATION-RUNTIME, RICH-RESPONSE, ARTIFACTS-EVIDENCE, TRUST and EXPERIENCE. Owning work E03-S11, E08-S04/S06/S07/S08 and E07-S04. Never treat a display operation as Evidence, Effect success or Task completion.

## Render architecture and precedence

~~~text
Agent specific structured event / raw native payload
  -> harness-specific verified decoder(version, schema, capability)
  -> bounded redaction / normalization / provenance / correlation
  -> durable semantic ConversationMessage + typed PresentationItem stream
  -> stable central renderer registry (safe HTML-free renderers)
       -> text/Markdown/code/diff/table/metrics/logs/permissions/errors/links/artifacts
       -> qualified native renderer extension if supported
       -> bounded readable structured/raw fallback with explicit label
  -> context Workbench viewer for large/editable/live artifacts
       -> Document/Spreadsheet/Slides/PDF/Image/Code/Diff/Browser/Terminal/MCP/GeneratedUI
~~~

The complete semantic response is always first-class, committed and accessible. RichPresentation is an optional display improvement that never blocks turn completion. Native provenance is pinned: provider/harness/adapter version, run/turn/task/attempt, event seq and source object. No raw HTML execution, JavaScript, untrusted iframe, terminal escape injection, network asset loading, arbitrary links or CSS from provider payload. Rich schema allowlist remains authoritative; unsupported types never crash overall conversation.

## Render message types and action ownership

| Kind | Central-pane display | Workbench | Ownership |
|---|---|---|---|
| USER_TEXT / AGENT_TEXT | Accessible Markdown, inline code, math where supported, citations | optional source opening | committed Conversation |
| THINKING/REASONING_SUMMARY | Only provider-exposed allowed summary, labeled; never fabricate private traces | no | provider emitted + policy |
| TOOL_CALL / TOOL_RESULT | Name, real method, status, duration/elapsed, bounded structured preview | inspector/log/resource | actual Invocation/Attempt |
| COMMAND / TERMINAL_OUTPUT | escaped monospace log, exit status, truncation/continue | interactive terminal only when qualified | environment + owner |
| FILE_EDIT / PATCH | file names, status and grounded +/- counts | exact diff viewer | Artifact and workspace changes |
| APPROVAL / USER_REQUEST | actionable typed card, exact target/scope/expiry | details | Core authorization |
| TASK/STEP/ATTEMPT | truthful state and progress summary | Work/Inspector | canonical Task state |
| ARTIFACT | exact immutable version, title, verification status | media-specific viewer/editor | Artifact service |
| TABLE / CSV / JSON | accessible table/structured safe viewer with row cap | full paginated if qualified | parser + provenance |
| IMAGE / VIDEO / AUDIO | safe media preview from authorized source, no invented thumbnail | full viewer where supported | Resource/Artifact revision |
| PDF / DOCX / XLSX / PPTX | file metadata + grounded thumbnail only if real | qualified provider preview/editor | external qualified provider |
| BROWSER_OBSERVATION | snapshot with time and nonlive badge if not live | BrowserSession surface | browser observation service |
| ERROR / CANCEL / RETRY | compact actual state; accessible explanation | detail/diagnostics | owning operation |
| UNKNOWN_NATIVE | formatted text or capped JSON tree + provider/source | raw/diagnostic disclosure | adapter unsupported |

Do not strip provider-native distinct semantics merely to force visual uniformity. Render stable shared cards and allow an explicit native detail view or fallback. No raw provider DOM injection into main React root.

## Required typed pipeline contracts

~~~ts
type NativeEventEnvelope = {
  harness:string; harnessVersion:string; adapterVersion:string;
  sessionId:string; messageId?:string; providerSequence?:string;
  observedAt:string; kind:string; payload:unknown;
};
type NormalizedPresentation = {
  key:string; workspaceId:string; conversationId:string; turnId:string;
  messageId?:string; itemKind:string; source:{provider:string;sessionId:string;eventRef:string};
  revision:number; phase:"TRANSIENT"|"COMMITTED"|"SUPERSEDED"|"ERROR";
  payload:unknown; contentDigest?:string; createdAt:string;
};
type RendererRegistration = {
  kind:string; supportedSchemaVersions:number[]; maxBytes:number;
  validate:(data:unknown)=>ValidationResult;
  render:(validated:unknown,ctx:SafeRenderContext)=>SafeReactNode;
  fallback:(data:unknown)=>PlainTextProjection;
};
~~~

Named type aliases are candidate client-level projections over existing canonical PresentationItem/OperatorStream; do not fork their schemas. New durable variants require EVENT/OpenAPI/storage migrations and feature-qualified tests.

## Required functions, constraints and edge cases

| Function | Algorithm/invariant |
|---|---|
| decodeNativeEvent | Validate exact harness version/event; reject invalid size/depth/cross-context reference; keep safe raw fallback |
| normalizeNativeEvent | Map to typed semantic display event; preserve native name, source, sequence, status; never infer success |
| redactUntrustedPayload | Remove credentials and private URLs/secret fields using known allowlist; safe truncation/redaction receipt |
| mapProviderSequence | Coalesce duplicates/out-of-order only by provider session and stream cursor; detect gaps for refetch |
| projectCommittedMessages | Replace transient frames with durable semantic messages by stable ID, preserving viewport anchor |
| compileRichPresentation | Check validated optional layout structure and exact message ownership; unsupported version => semantic fallback |
| resolveRenderer | Prefer exact safe built-in; then qualified extension; else bounded readable fallback |
| renderMarkdownSafe | Disallow raw HTML, scripts, event handlers, javascript/data URLs; escape user/provider text; code copy safe |
| renderCodeBlock | Syntax highlighting bounded, line numbers optional, copy original code, no execution |
| renderDiff | Preserve change semantics/file identity, search and collapse; very large diff uses bounded fallback |
| renderTable | Header labels, accessible captions, truncation indicators, virtual rows and horizontal local scroll |
| renderJsonTree | Escape keys and primitive values, bounded depth/count/bytes; no prototype execution |
| renderTerminalOutput | Strip or safely interpret ANSI allowlist, no OSC8 unsafe links, no terminal control sequences affecting UI |
| renderToolActivity | Distinguish proposed/queued/running/observed/verified; no "done" from only native text |
| renderArtifactReference | Require exact ArtifactVersion, permission and supported format; fallback explains no editor |
| renderCitation | Exact source identity and revision; never link guessed path or document |
| renderErrorCard | Stable code, safe user-readable reason, retry only if operation permits |
| renderApprovalCard | Typed Core UserRequest and short intent/scope, no actionable model-authored controls |
| renderNativeFallback | Safe JSON/text with known provider and unsupported kind; visibility without loss of transcript |
| restoreStreamAfterReconnect | Load authoritative message projection, resume cursor, dedup provisional items |
| preserveScrollAnchor | Avoid auto-scroll while reading history; offer jump-to-new |
| loadLargeOutputSegment | Bounded range with stable identity; explicit total/truncated/next cursor, cancellation |
| openExactWorkbenchVersion | Resolve owner access + current digest, select viewer, dirty-state guard |
| compareArtifactVersions | Pin both version IDs, avoid comparing stale heads; line-diff fallback when too large |
| handleRendererFailure | Isolate failed block, keep committed plain semantic content, log redacted diagnostic |
| auditRendererCoverage | For each upstream sample assert rendered/fallback and no schema-lost events |

## Display specifics

- Long streaming responses: use incremental chunk batching with immutable source cursor, not markdown reparsing every character; do not block typing or left rail. Cancel stream subscriptions when route changes while durable backend work continues.
- Markdown: headings/lists/links/tables/code/math/footnotes if supported; safe link confirmation for external schemes, no external image fetch unless authorized; highlighted code wrap/scroll with copy; gigantic fenced blocks stay responsive.
- Table: heading labels, empty cells, semantic numbers, copy/download only with real permission; zero rows explain empty; wide table scroll inside itself, not the entire chat.
- Diffs: file path escape, syntax coloring without semantic loss, additions/removals status, hunk collapsed/expanded; binary file/renamed path/missing base/merge conflict have explicit fallbacks. Exact version IDs and verification status visible.
- Media: loading, broken source, revoked bytes, unsupported codec, source privacy/size cap; no "perfect" media without qualified provider. Original media metadata accessible.
- Charts/diagrams: require schema-qualified, sandboxed implementation; no executed model JS/CSS/SVG payload. Fall back to accessible table/text.
- Tool calls: tool name, exact event phase, current actor and actual time; nested calls collapse safely, unknown tool JSON shown sanitized.
- Interactive MCP/Generated UI: sandbox each app with allowlisted host actions and granted capability bindings; no untrusted iframe origin as implicit authority; denials retain a usable static fallback.
- Approvals: only real Core request object can create actionable Approve/Deny. Text asking user to approve is just text.
- Native messages: retain a "View native details" disclosure if normalized projection loses unfamiliar fields. Absence of rich support must never hide committed output.

## Pane composition contracts

Conversation main remains chat/Task-focused; Workbench opens only by user intent or explicit restoration. Exact ArtifactVersion displayed, not generic latest after switch. Dirty editor with publish in flight locks conflicting close; Save As verifies version digest. Browser view uses CHROME-BROWSER-LLD live/external distinction. Code and terminal are distinct: output log does not imply interactive shell access. Document/XLSX/PDF presentation uses qualified external provider; never label text preview a full editor.

## Golden conformance corpus

For each supported agent and representative native session, capture with redaction: short Markdown, long Markdown, empty result, code + ANSI, huge log, code diff, JSON and tools, nested agent delegation, permission request, canceled tool, provider-specific native command, @ references, image/file attachments, failed auth, persisted Artifact, reconnect during stream, timed-out task, unknown event kind, and runtime upgrade. Compare normalized semantic meaning, order, status, source and action availability with upstream native client. Include screenshot baselines on Windows/Linux/macOS where claimed, accessibility tree snapshots, reduced motion and performance budgets (measured rather than invented numbers).

Reference authority: docs/PRESENTATION-RUNTIME.md and docs/RICH-RESPONSE.md, existing src/presentation/* and artifact renderer. Rich outputs are optional; Core task status must never be derived from a renderer's optimistic state.
