# UI behavior - Intelligent workspace presentations (Feature 016)

**Status:** IN_REVIEW, revision 0.4 TARGET proposal. The owner directed Saved Views to remain in V1 on 2026-11-17; the Feature 007 amendment is drafted but cross-owner review and whole-scope T01/T02 remain open. No implementation or route is authorized here.
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md).
**Wire owners:** [LOCAL-OPERATOR-IPC.md](../../../docs/LOCAL-OPERATOR-IPC.md), [operator-api.openapi.yaml](../../../docs/schemas/operator-api.openapi.yaml), and the Feature 007/008/011/001 owner contracts.

The semantic Conversation answer remains readable and authoritative. A valid, immutable RichPresentation may add registered visual structure. The same presentation can open in the contextual Workbench Canvas; it does not create a new conversation, Task, process or agent run. Save, remember, modify and navigation actions are host-owned controls routed to their existing domain owners. A renderer block cannot invoke host commands.

## 1. User journey and transitions (TARGET)

| Step | Screen / exact control | Expected transition |
|---|---|---|
| Read answer | Conversation message | Show semantic answer while optional presentation loads; replace no semantic record. Render only after committed message and accepted document validate. |
| Inspect sources and meaning | Host-owned `Details` / provenance disclosure beside an eligible metric or chart | Show bound semantic source, measure, units/currency, scale, interval and aggregation. Hide or fall back when any required meaning is ambiguous. |
| Explore a bounded view | Registered NUMBER, RANGE or SELECT control | Update declared local state and deterministic derived display. No model/provider call, IPC or domain write occurs. `Reset` restores the initial values. |
| Expand | Host-owned `Open in Canvas` control on the view | Open that object in Feature 008's right-side Workbench Canvas. Preserve message ID, document digest, selected inputs and controller revision; do not add a permanent sidebar destination. |
| Return to conversation | Canvas `Close` / `Return to answer` control | Close Canvas, restore focus to the originating control and preserve the current transient input revision. Preserve Conversation scroll and composer state. |
| Remember inputs | Optional host-owned `Remember on this device` control | Show consent and scope before the first write. Only exact-message primitive inputs persist. This stays unavailable until T22 and all storage/privacy/backup contracts are accepted and served. |
| Forget inputs | Host-owned `Forget remembered inputs` control | Remove exact namespace and advance the deletion fence. Show completion only after the owner's durable receipt; reconcile ambiguous timeouts before retry. |
| Save view | Host-owned `Save to Library` control in Canvas | Show only when Feature 007 authorizes the Saved View type and provides its create capability. Preview the exact source and disclose that the semantic answer and declared defaults become durable Library content; show Saved only after the durable receipt. Do not capture session-modified calculator values. |
| Reopen saved view | Library item selected by the user | Reauthorize and load the exact immutable ArtifactVersion. Never follow a moving head silently and never execute app code. |
| Update saved view | Host-owned `Update from answer` after selecting a new committed eligible presentation in the same Conversation | Append a new immutable ArtifactVersion using Feature 007 expected-version/idempotency behavior. Conflict preserves the review draft and requires confirmation. Cross-Conversation content creates a new Saved View. |
| Change answer conversationally | Ordinary Conversation composer/send | Sends only user-authored message text and normal Conversation context. Current calculator values are not automatically included; only explicit Update from answer changes the Library Artifact. |

The product owner has selected Saved Views for V1 and directed a narrow Feature 007 B-21 amendment. Its technical contract remains IN_REVIEW pending Feature 007/Storage/Conversation/Privacy/Backup/Product review and Feature 016 T01/T02. Until serving handlers and receipts exist, Save/Open/Update/Archive controls remain absent. Remember/Forget is a separate decision and cannot create a Library item. The live computer preview, takeover, browser, terminal, files, changes and activity panels remain Feature 008-owned contextual objects; this document does not claim those providers are implemented.

## 2. UI states

| Surface | States | Meaning and recovery |
|---|---|---|
| Conversation message | semantic-streaming, semantic-committed, presentation-loading, rich, semantic-fallback, disabled | Never flash a partial component tree. A read/presentation failure leaves the full semantic answer available; retry reads the projection and never resends the agent turn. |
| Structured block | valid, locally-degraded, rejected | Degrade the individual supported block when safe; otherwise retain semantic text. Do not render unknown HTML, JavaScript, CSS, URL actions or handler fields. |
| Metric / chart | bound, data-table, ambiguous/fallback | Provide an equivalent accessible table. Missing or conflicting source semantics produce no guessed figure. |
| Local controls | original, edited, invalid, reset | Invalid values show a bounded explanation. Result recalculates deterministically; reset returns to declared original inputs. |
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
| Feature 007 or persistence owner denies / is unavailable | Keep view usable in memory where possible; label save/remember unavailable and preserve unsaved draft. |
| Save/update response times out after possible commit | Keep `Saving`/`Unknown`; reconcile the same RequestId and digest before offering retry. |
| Two windows update one version or remembered-state revision | Preserve the local draft and show the latest owner version for explicit comparison. |
| Forget, deletion, revocation or Workspace switch races a write | Clear protected live values immediately, fence late writes, and do not restore from stale acknowledgement or backup. |
| Exact Saved View version is missing, corrupt or unauthorized | Show unavailable; never silently use the latest version. |
| Stream reconnect/gap occurs | Resync the Conversation projection and load only committed message-bound presentation records. |

## 4. Commands and ownership (TARGET unless explicitly marked SERVED)

| User control | UI-local behavior | Owner command/evidence |
|---|---|---|
| Read rich answer | Validate and render projection | Existing `get_rich_presentation` Tauri/Operator read is CODE/SERVED; no production publisher is wired. |
| Details / data table / calculator input / Reset | Local view/controller only | No IPC, event or record. |
| Open/close Canvas | Host navigation and controller transfer | Feature 008 Workbench owner; no standalone Feature 016 route. |
| Remember / Forget | Consent and local state display | Candidate Feature 016 authenticated command, storage transaction, receipt and event; no route exists. T22 must promote canonical contracts first. |
| Save / Reopen / Update / Archive Saved View | Host-owned capability-gated Library controls | Feature 007 Artifact/Library command and durable receipt. Exact type and operation contract is drafted in Feature 007 `contracts/saved-view.md`; API promotion is pending T01/T02 and no route is served. |
| Ask to change / Send | Preview selected bounded snapshot; normal composer send | Feature 001 ordinary Conversation command, idempotency, turn admission, lineage and recovery; no Feature 016 send route. |
| Display preference | Render policy preference | Feature 011 preference owner; current control is not served unless its owner contract says otherwise. |

## 5. Stream and projection behavior

The Conversation stream remains the turn authority. RichPresentation is a separate immutable projection bound to a committed semantic message. A presentation reference can prompt an exact read, but stream delta, provider tool observation or candidate intent cannot make an uncommitted component visible. On a sequence gap, discard transient presentation candidates and rehydrate the committed Conversation/presentation projections. Provider identity and source bindings remain visible to host validation; native subagents are not LiteCowork Task workers.

## 6. Accessibility and interaction quality

Use semantic headings and landmarks. Tables expose scoped headers and keyboard-accessible horizontal scrolling. A chart has a text summary and a data-table alternative. Inputs have visible labels, units, bounds and keyboard behavior; live calculation output uses a polite announcement. Canvas movement returns focus to the initiating control, retains Conversation scroll/composer state and supports keyboard, screen reader, high zoom/reflow and reduced motion. Color is not the only chart signal. No permanent row of specialist tabs is added for this presentation capability; the view appears inline and expands into the existing contextual Workbench.

## 7. Required UI tests and acceptance mapping

The future implementation must cover semantic-first loading and malformed-block fallback; exact message/digest/controller identity across expansion; source/metric visibility; safe input bounds/reset/keyboard use; policy preferences; Save/Open/Update/Archive capability hiding and receipt states; optional state consent/quota/replay/conflict/forget races only if T22 is accepted; Conversation source-deletion races; explicit modification through ordinary Send without hidden value sharing; scroll/composer preservation; and reconnect/restart recovery. See SPEC §20.10 and TASKS T15/T16/T20-T25. No route-presence or component-unit test alone qualifies the user journey.

## 8. Cross-layer trace (TARGET; sources remain owner-controlled)

| Operation | Control | Command | Owner | Durable record / evidence | Recovery |
|---|---|---|---|---|---|
| Read answer presentation | Conversation message | `get_rich_presentation` | Feature 016 read projection | Existing `rich_presentations` row/blob; CODE/SERVED read, no production publisher | Semantic fallback and exact reread |
| Change bounded input | Registered field | local reducer | Desktop presentation controller | none | Reset / semantic fallback |
| Expand answer | Open in Canvas | local Workbench navigation | Feature 008 | none | restore focus and original inline location |
| Remember / Forget | explicit host controls | future authenticated typed commands | Feature 016 + Storage + 011/014/017 | proposed exact-namespace state + owner receipt; not served | same-receipt reconciliation and deletion fence |
| Save / open / update | Library host controls | future Feature 007 Artifact commands | Feature 007 | immutable ArtifactVersion and receipt, if approved | exact-version reauthorization; preserve draft on conflict |
| Ask to change | preview then ordinary Send | Feature 001 send command | Feature 001 | new semantic message/turn with parent presentation lineage | request reconciliation; original remains immutable |

## 9. Scope trace to owner review

The previous revision 0.2 controls are superseded by revision 0.4. The product owner selected Saved Views for V1; T25 records the selected direction and detailed Feature 007 contract, while cross-owner review remains open. T01 still accepts or revises the complete V1 Feature 016 scope. Nothing in this proposal upgrades the current read-only/fallback UI into a supported rich-answer journey.
