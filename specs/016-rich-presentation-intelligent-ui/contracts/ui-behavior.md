# UI behavior - Intelligent workspace presentations (Feature 016)

**Status:** IN_REVIEW, revision 0.4 TARGET proposal. A-84 user-delegated direction authorizes implementation of the complete current proposal while preserving T01/T25 lifecycle and cross-owner review gates; it does not mark the package ACCEPTED/LOCKED or qualified.
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md).
**Wire owners:** [LOCAL-OPERATOR-IPC.md](../../../docs/LOCAL-OPERATOR-IPC.md), [operator-api.openapi.yaml](../../../docs/schemas/operator-api.openapi.yaml), and the Feature 007/008/011/001 owner contracts.

The semantic Conversation answer remains readable and authoritative. A valid, immutable RichPresentation may add registered visual structure. The same presentation can open in the contextual Workbench Canvas; it does not create a new conversation, Task, process or agent run. Save, remember, modify and navigation actions are host-owned controls routed to their existing domain owners. A renderer block cannot invoke host commands.

**Current implementation evidence (2026-11-25):** T06's React path can render source-bound renderer-v2 METRIC and CHART blocks when supplied a valid document; the chart's `Show data table` / `Show chart` button is keyboard-operable and exposes the exact bound values, while the metric shows the unchanged bound value, source-derived measure label and declared unit/scale/interval/aggregation, with `View source data` disclosure for its exact semantic table. T07 now renders validated SAFE_CALCULATOR blocks with labelled bounded number inputs, a formula derived as inert text from the frozen validated AST, local bounded recomputation/reset, polite result announcement, divide-by-zero/non-finite error text and always-visible semantic fallback. Input changes cause no host/provider call or durable write. Focused model and rendered-Chrome checks pass, including serious/critical axe rules and zero bridge-call count. Rust, TypeScript and SQLite publication/read validation require the closed context and exact semantic summary grounding; invalid/missing context falls back to the complete semantic answer. T08 now publishes exact semantic TABLE blocks with byte-bound TEXT_SLICE spans after settlement. T09 adds the device-local `Show formatted answers` switch in Settings > General; when disabled, `RichResponseView` selects the complete semantic response, future automatic/manual `get_rich_presentation` reads are blocked, and a response to a read already in flight is discarded. The switch does not alter immutable per-turn `PresentationPreference`, send content or provider input. Old local snapshots default to enabled; a rejected localStorage write is reported as session-only. Focused tests and the rendered Settings keyboard/persistence/failure-state group provide CODE/UI evidence, not packaged E2E-11. The T08 binder does not provide structured metric/chart/calculator intent, so those v2 views remain unreachable from ordinary provider answers. Context integrity does not fact-check provider-authored claims or complete the derived metric fixtures in T20/T24.

T18's display-only Markdown subset applies equally to the semantic answer, TEXT_SLICE and managed Markdown previews: headings, paragraphs, quotes, nested ordered/unordered lists, GFM pipe tables, horizontal rules, fenced code, inline emphasis/code and owner-confirmed HTTPS links. Bounds are 1 MiB source, 20,000 lines, 16,384 characters per line, 2,000 rendered blocks, list depth 8 / 2,000 items / ordered start ≤2,147,483,647, table 500 data rows / 32 columns / 8,192 characters per cell, and highlighting 100,000 code characters / 20,000 tokens. Byte and line limits are checked before line-array allocation. Unsupported or over-bound list/table syntax is literal text for that block; global source limits display the original as one literal block without parsing. Unknown code languages remain plain. No HTML parsing or remote image fetch is allowed.

## 1. User journey and transitions (TARGET)

| Step | Screen / exact control | Expected transition |
|---|---|---|
| Read answer | Conversation message | Show semantic answer while optional presentation loads; replace no semantic record. Render only after committed message and accepted document validate. |
| Read Markdown answer / TEXT_SLICE | Conversation message or validated TEXT_SLICE | Use the same bounded Markdown renderer for headings, paragraphs, quotes, nested lists, GFM tables, horizontal rules and fenced code. Unsupported syntax degrades only its block to literal text; the full semantic answer remains reachable. Never interpret raw HTML or load remote images. |
| Copy code | `Copy` button on a fenced code block | Copy only that block's original source after an explicit click. Show `Copied` only after the clipboard operation succeeds; on unavailable or denied clipboard access, retain the code and show `Copy unavailable`. No provider, IPC or durable write occurs. |
| Inspect sources and meaning | Host-owned `Details` / provenance disclosure beside an eligible metric or chart | Show bound semantic source, measure, units/currency, scale, interval and aggregation. Hide or fall back when any required meaning is ambiguous. |
| Explore a bounded view | Registered NUMBER, RANGE or SELECT control | Update declared local state and deterministic derived display. No model/provider call, IPC or domain write occurs. `Reset` restores the initial values. |
| Expand | Host-owned `Open in Canvas` control on the view | Open that object in Feature 008's right-side Workbench Canvas. Preserve message ID, document digest, selected inputs and controller revision; do not add a permanent sidebar destination. |
| Return to conversation | Canvas `Close` / `Return to answer` control | Close Canvas, restore focus to the originating control and preserve the current transient input revision. Preserve Conversation scroll and composer state. |
| Remember inputs | Optional host-owned `Remember on this device` control | Show consent and scope before the first write. Only exact-message primitive inputs persist. This stays unavailable until T22 and all storage/privacy/backup contracts are accepted and served. |
| Forget inputs | Host-owned `Forget remembered inputs` control | Remove exact namespace and advance the deletion fence. Show completion only after the owner's durable receipt; reconcile ambiguous timeouts before retry. |
| Save view | Host-owned `Save to Library` control in Canvas | Show only when Feature 007 authorizes the Saved View type and provides its create capability. Preview the exact source and disclose that the semantic answer and declared defaults become durable Library content; an unchecked option may include validated numeric values after explicit confirmation. Show Saved only after the durable receipt. |
| Reopen saved view | Library item selected by the user | Reauthorize and load the exact immutable ArtifactVersion. Never follow a moving head silently and never execute app code. |
| Update saved view | Host-owned `Update from answer` after selecting a new committed eligible presentation in the same Conversation | Append a new immutable ArtifactVersion using Feature 007 expected-version/idempotency behavior. Conflict preserves the review draft and requires confirmation. Cross-Conversation content creates a new Saved View. |
| Ask a follow-up | Host-owned action opens the existing ordinary composer with an editable draft; values omitted by default. If the user selects `Include these values`, show the exact declared labels and values as ordinary text in the preview. | The existing Feature 001 Send is the only dispatch path; no presentation route, hidden payload or component callback. Normal request identity, permissions, single-flight and recovery apply. |

The product owner has selected Saved Views for V1 and directed a narrow Feature 007 B-21 amendment. Its technical contract remains IN_REVIEW pending Feature 007/Storage/Conversation/Privacy/Backup/Product review and Feature 016 T01/T02. Until serving handlers and receipts exist, Save/Open/Update/Archive controls remain absent. Remember/Forget is a separate decision and cannot create a Library item. The live computer preview, takeover, browser, terminal, files, changes and activity panels remain Feature 008-owned contextual objects; this document does not claim those providers are implemented.

## 2. UI states

| Surface | States | Meaning and recovery |
|---|---|---|
| Conversation message | semantic-streaming, semantic-committed, presentation-loading, rich, semantic-fallback, disabled | Never flash a partial component tree. A read/presentation failure leaves the full semantic answer available; retry reads the projection and never resends the agent turn. |
| Structured block | valid, locally-degraded, rejected | Degrade the individual supported block when safe; otherwise retain semantic text. Do not render unknown HTML, JavaScript, CSS, URL actions or handler fields. |
| Markdown block | rendered, literal-fallback | Parsing limits or unsupported syntax affect only the current block. The unsupported source is emitted as React text; sibling blocks continue to render. Oversized documents keep the bounded original-text fallback. |
| Code block | unhighlighted, highlighting, highlighted, copy-failed | Source is visible immediately. Highlighting is lazy, bounded, and rendered as React text/token nodes; unsupported languages and oversized blocks stay plain text. Clipboard denial never hides or mutates the source. |
| Metric / chart | bound, data-table, ambiguous/fallback | Provide an equivalent accessible table. Missing or conflicting source semantics produce no guessed figure. |
| Local controls | original, edited, invalid, reset | Invalid values show a bounded explanation. Result recalculates deterministically; reset returns to declared original inputs. |
| SAFE_CALCULATOR | original, editing, invalid-input, calculated, arithmetic-error, reset | Inputs are bounded number fields with visible minimum/maximum/step and `aria-invalid` on empty, non-finite, out-of-range or off-step text. Valid edits recalculate synchronously in the local reducer; invalid inputs or arithmetic show the generic “Cannot calculate with these values” output, while the declared semantic fallback remains visible. Reset restores the exact validated initial table values and result. No loading state, provider request or host command exists. |
| Canvas | opening, open, closing, unavailable | Same object and controller as inline. Restore keyboard focus and Conversation scroll/composer. Opening does not start computer capture or a process. |
| Remembered inputs | off, consent-review, saving, saved, conflict, unavailable, forgetting, forgotten, unknown | No value appears saved/forgotten until a durable receipt resolves. On conflict keep the user's draft; on denial show current values and a plain reason. |
| Saved View | not-saved, save-review, saving, saved-version, opening, updating-from-answer, conflict, archived, denied, unknown | Owner receipt defines durable success. Open the requested exact version or show unavailable; never substitute a newer version or stale protected content. No hard Delete control; Archive uses Feature 007. |
| Conversational change | idle, preview, sending, cancelled, submitted, unresolved | Cancel leaves source answer and Library version unchanged. An unresolved send uses Feature 001's exact request identity/reconciliation. |

## 3. Validation and failure presentation

| Failure | UI result |
|---|---|
| Desktop validator rejects a document or sees an unknown renderer version | Show semantic answer; no raw object or error-looking replacement. |
| A component is malformed but sibling blocks remain valid | Render only blocks accepted by the canonical degradation rule; keep the complete semantic answer reachable. |
| Metric unit, interval, aggregation or source binding is missing | Show the data table or ask for clarification; do not guess. |
| Calculation is invalid/non-finite or kernel is unknown | Show the declared fallback and a bounded message; no host call. |
| SAFE_CALCULATOR input is empty, non-finite, out of range or off step | Mark that field invalid, keep its declared bounds visible, show the generic calculation message, and leave the semantic fallback visible. Do not clamp, round or submit the value. |
| Feature 007 or persistence owner denies / is unavailable | Keep view usable in memory where possible; label save/remember unavailable and preserve unsaved draft. |
| Save/update response times out after possible commit | Keep `Saving`/`Unknown`; reconcile the same RequestId and digest before offering retry. |
| Two windows update one version or remembered-state revision | Preserve the local draft and show the latest owner version for explicit comparison. |
| Forget, deletion, revocation or Workspace switch races a write | Clear protected live values immediately, fence late writes, and do not restore from stale acknowledgement or backup. |
| Exact Saved View version is missing, corrupt or unauthorized | Show unavailable; never silently use the latest version. |
| Stream reconnect/gap occurs | Resync the Conversation projection and load only committed message-bound presentation records. |
| Markdown block uses unsupported syntax or exceeds a table/list bound | Render that block's exact source as inert text and continue with neighboring blocks; never replace the whole message with a raw dump unless the document itself exceeds the global parser bound. |
| Code highlighting fails or clipboard access is denied | Leave the exact source visible without highlighting; report copy failure without claiming it copied. |

## 4. Commands and ownership (TARGET unless explicitly marked SERVED)

| User control | UI-local behavior | Owner command/evidence |
|---|---|---|
| Read rich answer | Validate and render projection | Existing `get_rich_presentation` Tauri/Operator read is CODE/SERVED; T08 has a CODE-only table/text publisher after Conversation settlement. |
| Details / data table / calculator input / Reset | Local view/controller only | No IPC, event or record. |
| Open/close Canvas | Host navigation and controller transfer | Feature 008 Workbench owner; no standalone Feature 016 route. |
| Remember / Forget | Consent and local state display | Candidate Feature 016 authenticated command, storage transaction, receipt and event; no route exists. T22 must promote canonical contracts first. |
| Save / Reopen / Update / Archive Saved View | Host-owned capability-gated Library controls | Feature 007 Artifact/Library command and durable receipt. Exact type and operation contract is drafted in Feature 007 `contracts/saved-view.md`; API promotion is pending T01/T02 and no route is served. |
| Ask a follow-up / Send | Prepare an editable, visible ordinary composer draft; optionally include selected values as plain message text only after explicit preview and user selection. | Existing Feature 001 ordinary Conversation command and idempotency/recovery; no Feature 016 send route or hidden provider metadata. |
| Display preference (`pref-rich-presentation`) | Settings > General checkbox; local callback into App-owned validated preference state | Feature 011 T07 local store; default-on for old snapshots. OFF shows semantic content and prevents new optional projection reads. It never changes turn intent or provider request; storage failure is reported as session-only. |

## 5. Stream and projection behavior

The Conversation stream remains the turn authority. RichPresentation is a separate immutable projection bound to a committed semantic message. A presentation reference can prompt an exact read, but stream delta, provider tool observation or candidate intent cannot make an uncommitted component visible. On a sequence gap, discard transient presentation candidates and rehydrate the committed Conversation/presentation projections. Provider identity and source bindings remain visible to host validation; native subagents are not LiteCowork Task workers.

## 6. Accessibility and interaction quality

Use semantic headings and landmarks. Tables expose scoped headers and keyboard-accessible horizontal scrolling. A chart has a text summary and a data-table alternative. Inputs have visible labels, units, bounds and keyboard behavior; live calculation output uses a polite announcement. Canvas movement returns focus to the initiating control, retains Conversation scroll/composer state and supports keyboard, screen reader, high zoom/reflow and reduced motion. Color is not the only chart signal. No permanent row of specialist tabs is added for this presentation capability; the view appears inline and expands into the existing contextual Workbench.

## 7. Required UI tests and acceptance mapping

The future implementation must cover semantic-first loading and malformed-block fallback; exact message/digest/controller identity across expansion; source/metric visibility; safe input bounds/reset/keyboard use; policy preferences; Save/Open/Update/Archive capability hiding and receipt states; optional state consent/quota/replay/conflict/forget races only if T22 is accepted; Conversation source-deletion races; explicit modification through ordinary Send without hidden value sharing; scroll/composer preservation; and reconnect/restart recovery. See SPEC §20.10 and TASKS T15/T16/T20-T25. No route-presence or component-unit test alone qualifies the user journey.

## 8. Cross-layer trace (TARGET; sources remain owner-controlled)

| Operation | Control | Command | Owner | Durable record / evidence | Recovery |
|---|---|---|---|---|---|
| Read answer presentation | Conversation message | `get_rich_presentation` | Feature 016 read projection | Existing `rich_presentations` row/blob; CODE/SERVED read; T08 table/text publisher is CODE only | Semantic fallback and exact reread |
| Change formatted-answer display | Settings > General `pref-rich-presentation` | LOCAL_PREF callback; no Tauri/Operator command | Feature 011 shell preference + Feature 016 render/read policy | `litecowork.local-preferences.v1` localStorage snapshot only; legacy default ON; CODE/UI. Not a domain record or provider request. | Failed storage write announces session-only; switching OFF clears visible rich state and ignores a late read result; switching ON allows only a subsequent authorized exact projection read. Packaged restart/E2E-11 remains open. |
| Copy fenced code | Code-block `Copy` button | UI-local Clipboard API | Desktop UI only | None; explicit per-click clipboard effect, no IPC/provider call | Source remains visible; show failure status if denied/unavailable |
| Change bounded input | Registered field | local reducer | Desktop presentation controller | none | Reset / semantic fallback |
| Expand answer | Open in Canvas | local Workbench navigation | Feature 008 | none | restore focus and original inline location |
| Remember / Forget | explicit host controls | future authenticated typed commands | Feature 016 + Storage + 011/014/017 | proposed exact-namespace state + owner receipt; not served | same-receipt reconciliation and deletion fence |
| Save / open / update | Library host controls | future Feature 007 Artifact commands | Feature 007 | immutable ArtifactVersion and receipt, if approved | exact-version reauthorization; preserve draft on conflict |
| Ask to change | preview then ordinary Send | Feature 001 send command | Feature 001 | new semantic message/turn with parent presentation lineage | request reconciliation; original remains immutable |

## 9. Scope trace to owner review

The previous revision 0.2 controls are superseded by revision 0.4. The product owner selected Saved Views for V1; T25 records the selected direction and detailed Feature 007 contract, while cross-owner review remains open. T01 still accepts or revises the complete V1 Feature 016 scope. Nothing in this proposal upgrades the current read-only/fallback UI into a supported rich-answer journey.
