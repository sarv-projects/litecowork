# Tasks - Intelligent Workspace Presentation Runtime

**Feature:** 016 | **Status:** IN_REVIEW - revision 0.4 V1 proposal (AC-001..AC-041). The owner directed Saved Views to remain in V1 and Feature 007 B-21 to be amended; T25 has a drafted cross-owner contract but review remains open before T01/T02. No implementation is authorized by either task's existence.
**Spec revision:** 0.3 (proposed) | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** Presentation Runtime, Conversation, Artifacts, Storage, Operator/API, Desktop UI, Security, Accessibility, Product.

Cross-package task references inside task fields are written as "Feature NNN Txx" so that each hyphenated cross-package ID appears once in this file (the structural validator rejects a repeated task ID); the hyphenated IDs are listed once in "Cross-package dependencies" below.

## Current baseline

Source cross-check: active source at `main` HEAD `f146bb3a187449d5348c5bf2e380c122c6d290c0` was rechecked on 2026-11-17. The validator/store/read route and Markdown/RichResponseView are partial CODE/SERVED evidence only. This design review adds no Feature 016 code. The revised proposal includes a Library Saved View only through the Feature 007 contract; it preserves B-40 transient-only changed inputs. Separate Remember/Forget persistence remains gated by B-46/T22; neither behavior exists today.

- **Desktop validator:** `apps/litecowork-ui/src/presentation/rich-presentation.ts` with `MAX_DOCUMENT_BYTES = 512 * 1024`, `MAX_BLOCKS = 200`, `MAX_DEPTH = 8`, `MAX_JSON_VALUES = 30_000`, `MAX_JSON_DEPTH = 32`; accepts TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE; `renderer_contract_version` must be 1; `parseRichPresentationResponse`, `resolveRichResponseContent`.
- **Rust validator:** `validate_metadata`, `validate_document`, `validate_block` in `crates/storage-sqlite/src/rich_presentations.rs` accept the same five V1 kinds as the desktop validator (TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE) with the same closed keys, and hold CARD titles, CALLOUT summaries and TABLE headers and cells to the committed semantic text; `MAX_RICH_PRESENTATION_BYTES = 524_288` in `crates/storage-core/src/rich_presentation.rs` matches the desktop cap and `document_size_bytes.maximum` in `docs/schemas/operator-stream.schema.json` (the doc comment no longer names a DIVIDER kind neither side accepts). `publish_rich_presentation` is called only from tests in `apps/litecoworkd/src/presentation_operator.rs`.
- **Served reads:** `GET /v1/rich-presentations/{presentation_id}` (`get_rich_presentation`), `GET /v1/conversations/{conversation_id}/presentation`, `POST /v1/stream`; Tauri `get_rich_presentation` in `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs`.
- **Renderer:** `apps/litecowork-ui/src/presentation/RichResponseView.tsx` (used by `ConversationsPage.tsx`) with the "Read the original response" disclosure; no chart, media or Intelligent UI. Semantic fallback uses the bounded `apps/litecowork-ui/src/artifacts/StructuredTextPreview.tsx` Markdown subset; it still has whole-message fallback for unsupported syntax, lacks GFM tables/horizontal rules/nested lists and code label/copy/highlighting, and TEXT_SLICE remains plain text. `PresentationRuntime.tsx` shows MCP_APP as unavailable.
- **Tests:** `apps/litecowork-ui/tests/presentation-runtime-fallback.test.mjs`, `rich-presentation.test.ts`, `rich-response-view-policy.test.ts`; Rust `crates/storage-sqlite/src/rich_presentation_tests.rs`.
- **Absent:** a shared corpus, a Rust presentation crate, CalcAst, chart and calculator components, a production binder, media ingestion, `PRESENTATION_*` error codes, an Intelligent UI schema in `docs/schemas`, an Artifact create route (Feature 007 B-30), any Interactive Artifact Runtime code. CSP in `apps/litecowork-ui/src-tauri/tauri.conf.json` is `script-src 'self'` with `img-src 'self' asset: http://asset.localhost data:`.

Gaps to proposed V1: revised-scope owner review is pending at T25/T01. The validator parity fix is present, but shared validation authority, production publisher, chart/calculator and local-control renderers, media provenance, preference wiring, Canvas controller, Saved View lifecycle and packaged evidence remain open. The product owner has selected Saved Views; Feature 007 revision 1.1 technical review and API/schema promotion remain open; no save route exists. All AC-001..AC-041 remain TARGET until accepted.

## 016-T01 - Owner acceptance of the revised V1 scope

- **Status:** BLOCKED - owner requested a scope revision; B-40 remains the accepted calculator/state boundary and does not accept revision 0.4. Saved View product direction is now decided, but T25 cross-owner review must close before the repository owner accepts/revises the complete V1 AC set at T01.
- **DEFERRED/BLOCKED note:** Why: no whole-feature V1 scope is accepted. Unblocks when: T25 records the Feature 007 Saved View decision; SPEC/TASKS/related contracts reflect any exclusion; then the owner records a dated decision on every included AC and rider at T01. Test later at: decision only.
- **Depends on:** T25
- **Cross-package prerequisites:** Feature 007 Artifact/Library owner review through T25 if Saved View remains in proposed V1.
- **Owner:** Product, Presentation Runtime, Security; decision by the repository owner.
- **Allowed paths:** `specs/016-rich-presentation-intelligent-ui/`, `docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`.
- **Contract:** SPEC §20 and AC-001..AC-041 as revised after T25; B-40; SPECIFICATION-STANDARD §7; A-84/A-103.
- **Change intent:** record an explicit accept/revise/defer decision for every proposed V1 capability and rider, including B-46's component-versus-host Remember/Send boundary: semantic-first presentation, canonical registered components, safe calculations/metrics, optional qualified ingress, contextual Canvas, local remembered-input state, Feature 007 Saved View lifecycle if approved, explicit conversational modification, media, preview-only executable Artifacts and exact platform qualification. If any capability is deferred, update its AC/task/UI promise before marking the package ACCEPTED/LOCKED. B-40 alone is not acceptance of this package.
- **Acceptance:**
  - [ ] Dated owner decision records the exact accepted ACs, explicit exclusions, cross-owner riders and release rows for revision 0.4; no unreviewed item is implied accepted.
  - [ ] Package banners, plan, UI/API/error contracts, wiring and acceptance packet agree with the decision; validator PASS.
  - [ ] Negative: no implementation task starts while this task is BLOCKED.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** dated owner decision in OWNER-ACCEPTANCE-REVIEW-PACKET Part 6, A-84/A-103 updates, and the exact included/excluded AC set in SPEC/TASKS.
- **Stop:** T25 lacks an Artifact owner decision, B-46 is unaddressed, any included item lacks its owner/rider, or a deferred item remains in the V1 promise.

## 016-T02 - Promote accepted interfaces into owner docs and machine schemas

- **Status:** NOT STARTED
- **Depends on:** T01, T25
- **Cross-package prerequisites:** Feature 007/008/011/001 and Privacy/Storage owners resolve all included Saved View, Canvas, state, deletion, send and preference contracts.
- **Owner:** Presentation Runtime, API, Events, Schemas owners.
- **Allowed paths:** `docs/RICH-RESPONSE.md`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/INTERACTIVE-ARTIFACT-RUNTIME.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/ACCEPTANCE-AND-E2E-PLAN.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/schemas/rich-presentation.schema.json`, `docs/schemas/calc-ast.schema.json` (new), `docs/schemas/presentation-intent.schema.json`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `specs/007-professional-artifacts/`, `specs/008-workbench-browser-terminal-computer/`, `specs/011-desktop-shell-ipc-settings-runtime/`, this package.
- **Contract:** SPEC §20 and accepted ACs; canonical Rich Response, Artifact, Workbench, state, event, API and UI owners.
- **Change intent:** promote the accepted revision's one closed RichPresentation profile, metric semantics and source bindings, B-40 calculation AST, any B-46 local input types, policy/fallback, producer provenance, Saved View representation if T25/T01 include it, Canvas ownership, Remember/Forget contracts if included, owner errors/events and UI trace. Reconcile global Rich Response, Artifact, Workbench, privacy/retention and release contracts in the same change; do not create a parallel schema or route.
- **Acceptance:**
  - [ ] Each accepted interface has one normative owner and canonical machine schema; dependent UI/API/event/error/wiring and coverage maps match.
  - [ ] Negative corpus rejects unknown fields, unbound metrics, unregistered kernels and out-of-profile components; valid fallback remains semantic text.
  - [ ] Migration needs, receipt, retention, deletion, backup and rollback are explicit; do not claim no migration if T22 is included.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py`
- **Evidence:** PR link and output in Completion evidence.
- **Stop:** an owner doc conflicts with the B-40 decision; resolve at the owner.

## 016-T03 - Single Rust validator crate and drift fix

- **Status:** PARTIAL - the drift is fixed: `crates/storage-sqlite/src/rich_presentations.rs` now accepts TEXT_SLICE, LAYOUT, CARD, CALLOUT and TABLE under the desktop's 512 KiB cap, holding CARD titles, CALLOUT summaries and TABLE headers and cells to the semantic text, and the stale DIVIDER comment is gone. A TABLE cell is compared with ECMAScript `Number::toString` (so `1e-27` matches, not `0.000000000000000000000000001`) and row collection mirrors the desktop's clear-on-wrong-width rule, because either difference would refuse a table the desktop renders. Still open: no shared `domain-presentation` crate, and CHART and SAFE_CALCULATOR are not validated.
- **Depends on:** T01, T02
- **Cross-package prerequisites:** NONE
- **Owner:** Presentation Runtime, Storage.
- **Allowed paths:** `crates/domain-presentation/` (new crate: `Cargo.toml`, `src/lib.rs`, `src/blocks.rs`, `src/calc.rs`, `src/tests.rs`), `Cargo.toml` (workspace members), `Cargo.lock`, `crates/storage-sqlite/Cargo.toml`, `crates/storage-sqlite/src/rich_presentations.rs`, `crates/storage-sqlite/src/rich_presentation_tests.rs`, `crates/storage-core/src/rich_presentation.rs` (cap constant and doc comment).
- **Contract:** SPEC G-01, G-02, section 7.2; AC-001, AC-002, AC-006.
- **Change intent:** move the closed-object, canonical-bytes, digest, slice, depth and provenance checks from `rich_presentations.rs` into `domain_presentation::validate_document`, add CARD, CALLOUT, TABLE (exact semantic Markdown table match), CHART and SAFE_CALCULATOR checks, set `MAX_DOCUMENT_BYTES` to 512 KiB, and have `SqliteRichPresentationStore::publish_rich_presentation` call it. Remove the stale DIVIDER comment. Dependencies limited to `serde_json`, `serde_json_canonicalizer`, `sha2` (already in the lockfile); no `jsonschema` crate.
- **Acceptance:**
  - [x] The cap is 512 KiB on every surface and a document above it is rejected (AC-001): `rich_presentation_document_cap_agrees_with_the_desktop_and_the_wire_schema` pins the constant against the desktop validator and the served wire schema, and the existing over-cap case still fails. The exact-512-KiB-accepted publish case moves with the shared crate.
  - [ ] Every non-allowlisted kind rejected with `UnsupportedBlock`; actions, citations, HOST_PROJECTION and MCP_APP rejected (AC-002, AC-006).
  - [ ] Existing `rich_presentation_tests.rs` cases still pass after the move.
- **Command:** `cargo test -p domain-presentation --locked` and `cargo test -p storage-sqlite --locked rich_presentation`
- **Evidence:** test output in Completion evidence.
- **Stop:** the crate needs a storage dependency; keep it pure.

## 016-T04 - Shared golden corpus and TypeScript parity

- **Status:** PARTIAL - `rich-presentation.ts` and `tests/rich-presentation.test.ts` exist with inline fixtures; no shared corpus.
- **Depends on:** T01, T03
- **Cross-package prerequisites:** NONE
- **Owner:** Presentation Runtime, Desktop UI.
- **Allowed paths:** `crates/domain-presentation/tests/corpus/` (new JSON files), `crates/domain-presentation/src/tests.rs`, `apps/litecowork-ui/src/presentation/rich-presentation.ts`, `apps/litecowork-ui/tests/rich-presentation-corpus.test.ts` (new).
- **Contract:** SPEC G-01; RENDERING-CONFORMANCE-LLD; AC-002, AC-003.
- **Change intent:** one corpus directory of documents with an expected verdict file (accept or reject plus reason); both test suites iterate it. Update `rich-presentation.ts` to accept renderer contract version 2 and the CHART and SAFE_CALCULATOR kinds with the same bounds; keep its defensive checks (proxies, getters, cycles). Corpus includes boundary sizes, deep nesting, hostile strings, non-finite numbers and every rejected kind.
- **Acceptance:**
  - [ ] Both validators agree on every corpus file (AC-003).
  - [ ] Unknown kinds render semantic text in the desktop (AC-002).
- **Command:** `cargo test -p domain-presentation --locked corpus` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** both outputs in Completion evidence.
- **Stop:** a disagreement is resolved by loosening either side; tighten instead.

## 016-T05 - Restricted CalcAst validator (Rust) and pure reducer (TypeScript)

- **Status:** NOT STARTED
- **Depends on:** T01, T03
- **Cross-package prerequisites:** NONE
- **Owner:** Presentation Runtime.
- **Allowed paths:** `crates/domain-presentation/src/calc.rs`, `apps/litecowork-ui/src/presentation/calc-reducer.ts` (new), `apps/litecowork-ui/tests/calc-reducer.test.ts` (new), `crates/domain-presentation/tests/corpus/`.
- **Contract:** SPEC L-03, B-40; INTELLIGENT-UI section 3; AC-007.
- **Change intent:** Rust `validate_calc_ast` checks node kinds, at most 64 nodes, depth at most 12, finite literals, arity of MIN, MAX, ROUND, ABS, CLAMP and that every VARIABLE names a declared input. TypeScript `evaluateCalc` is an iterative, allocation-light evaluator over a frozen state map, returning `DIVIDE_BY_ZERO` or `NON_FINITE` instead of NaN or Infinity. No string parsing of formulas anywhere.
- **Acceptance:**
  - [ ] Every rule has a Rust negative test (AC-007).
  - [ ] Reducer property test: for random valid ASTs and finite inputs the result is finite or a typed reason; never throws (AC-007).
- **Command:** `cargo test -p domain-presentation --locked calc` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** any path evaluates a string expression.

## 016-T06 - CHART block with host SVG and table fallback

- **Status:** NOT STARTED - PRESENTATION-RUNTIME records CHART as a text placeholder today.
- **Depends on:** T01, T03, T04
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Accessibility.
- **Allowed paths:** `apps/litecowork-ui/src/presentation/ChartBlock.tsx` (new), `apps/litecowork-ui/src/presentation/chart-model.ts` (new), `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-response.css`, `apps/litecowork-ui/tests/chart-block.test.ts` (new), `crates/domain-presentation/src/blocks.rs`.
- **Contract:** SPEC L-04, G-04; RICH-RESPONSE "Tables and charts"; AC-008, AC-010.
- **Change intent:** `chart-model.ts` turns the bound TABLE slice into series (pure, tested); `ChartBlock.tsx` draws BAR, LINE and AREA with React `<svg>`, `<rect>`, `<path>` elements and computed coordinates only (no model markup), `role="img"` with the accessible summary, and a "Show data table" toggle that renders the existing TABLE renderer. PIE and SCATTER are rejected in V1 profile unless the owner widens it. No chart library dependency.
- **Acceptance:**
  - [ ] Chart and its table show identical values (AC-008).
  - [ ] Binding mismatch or non-finite cell renders the table fallback (AC-008).
  - [ ] Table toggle reachable by keyboard; summary exposed to screen readers (AC-010).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test output; screenshot in the evidence bundle.
- **Stop:** a design needs model-supplied SVG or style.

## 016-T07 - SAFE_CALCULATOR Intelligent UI component (B-40)

- **Status:** NOT STARTED
- **Depends on:** T01, T05, T06
- **Cross-package prerequisites:** Feature 011 app-wide keyboard and focus baseline.
- **Owner:** Desktop UI, Accessibility, Presentation Runtime.
- **Allowed paths:** `apps/litecowork-ui/src/presentation/SafeCalculatorBlock.tsx` (new), `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-response.css`, `apps/litecowork-ui/tests/safe-calculator-block.test.ts` (new), `crates/domain-presentation/src/blocks.rs`.
- **Contract:** SPEC B-40, G-03; INTELLIGENT-UI section 3; canonical SAFE_CALCULATOR promotion; AC-007, AC-009, AC-010.
- **Change intent:** labelled `<input type="number">` fields (initial values from the bound table cells), output in an `aria-live="polite"` region formatted per `output_format`, "Reset to original values" button, fallback text always visible below. State lives in a `useReducer` inside the component; no store, no bridge call, no persistence.
- **Acceptance:**
  - [ ] A bridge-call spy records zero calls during interaction (AC-009).
  - [ ] Divide by zero shows "Cannot calculate with these values" (AC-007).
  - [ ] Inputs labelled; result announced; axe clean at serious and critical (AC-010).
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output and accessibility report in the evidence bundle.
- **Stop:** the component needs to send values to the agent; that is a different, unaccepted feature.

## 016-T08 - PresentationBinder and publish after semantic commit

- **Status:** NOT STARTED - `publish_rich_presentation` has no production caller.
- **Depends on:** T01, T03, T05
- **Cross-package prerequisites:** Feature 001 T06 (final response settlement with stable identity in the supervisor).
- **Owner:** Presentation Runtime, Conversation, Storage.
- **Allowed paths:** `apps/litecoworkd/src/presentation_binder.rs` (new), `apps/litecoworkd/src/agents/supervisor.rs` (one call after the commit returns), `apps/litecoworkd/src/main.rs` (wiring only), `crates/storage-sqlite/src/rich_presentations.rs`, adjacent tests.
- **Contract:** SPEC L-01, G-05, section 6 sequence; AC-004, AC-014, AC-017.
- **Change intent:** after `commit_conversation_agent_response` returns, `bind_after_commit` derives a document from the committed semantic text (Markdown tables to TABLE, eligible numeric tables to CHART, calculation intent only after canonical T02/T20 promotion to SAFE_CALCULATOR), validates it, and publishes in its own transaction with a trusted host-allocated retained publish identity so replays are idempotent and the one-record-per-message uniqueness is preserved. Any error is counted and dropped; the turn is already settled.
- **Acceptance:**
  - [ ] Binder fault injection leaves the turn and message unchanged (AC-004).
  - [ ] Replay returns the stored record; a different digest under the same id conflicts (AC-014).
  - [ ] Only committed text and validated intent are used (AC-017).
- **Command:** `cargo test -p litecoworkd --locked presentation_binder` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** any binder path runs inside the final-commit transaction.

## 016-T09 - Disable preference and semantic fallback (E2E-11)

- **Status:** PARTIAL - `RichResponseView.tsx` already renders semantic text when no valid presentation exists and offers "Read the original response"; no preference to disable rich presentation.
- **Depends on:** T01, T04
- **Cross-package prerequisites:** Feature 011 T07 (applied and persisted local display preferences).
- **Owner:** Desktop UI, Feature 011 shell.
- **Allowed paths:** `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-presentation.ts` (`resolveRichResponseContent` gains the preference), `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (pass the preference only), `apps/litecowork-ui/src/settings/` (toggle row in the display preferences section owned by Feature 011), `apps/litecowork-ui/tests/rich-response-view-policy.test.ts`.
- **Contract:** SPEC G-06; release contract section 4; AC-004, AC-018.
- **Change intent:** a local preference `rich_presentation_enabled` (default true); when false, `resolveRichResponseContent` returns `primary: "SEMANTIC"` without fetching documents. The toggle is a labelled switch in Settings display preferences.
- **Acceptance:**
  - [ ] With the preference off, no `get_rich_presentation` call happens and the full semantic text shows (AC-004, AC-018).
  - [ ] Preference survives restart; toggle keyboard operable (AC-018).
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; E2E-11 step evidence in T16.
- **Stop:** the preference is stored outside the Feature 011 preference owner.

## 016-T10 - Generated media as provenance Artifacts

- **Status:** NOT STARTED - no ingestion pipeline; no Artifact create route today.
- **Depends on:** T01, T02
- **Cross-package prerequisites:** Feature 007 T02 (atomic Artifact create store, B-30) and Feature 007 T08 (Conversation association of Artifacts); Feature 011 decision on image display scheme (CSP).
- **Owner:** Artifacts, Presentation Runtime, Security.
- **Allowed paths:** `apps/litecoworkd/src/media_ingestion.rs` (new), `apps/litecoworkd/src/agents/supervisor.rs` (hand off harness file outputs only), `apps/litecowork-ui/src/artifacts/ArtifactWorkbench.tsx` (image view only), `apps/litecowork-ui/tests/artifact-image-view.test.ts` (new), adjacent Rust tests.
- **Contract:** SPEC L-05, G-07, section 10; INTELLIGENT-UI section 4; AC-011, AC-012, AC-015.
- **Change intent:** `ingest_harness_file` reads an admitted Workspace path, sniffs magic bytes (PNG, JPEG, WebP, GIF), enforces at most 20 MiB and 16384 pixels per side, hashes to `blobs/<sha256>`, then asks Feature 007 to create an Artifact version with provenance (Conversation, turn, AgentSession, digest). SVG and URL-only references are rejected. The Workbench image view shows alt text from the harness or a "No description provided" label, never autoplays, and loads bytes only through the authorized Artifact content read.
- **Acceptance:**
  - [ ] Valid PNG produces one Artifact with full provenance (AC-011).
  - [ ] SVG, oversize, mismatch and URL-only inputs are rejected with no Artifact and no network call (AC-012).
  - [ ] Crash between blob write and Artifact commit: sweep removes the orphan blob (AC-015).
- **Command:** `cargo test -p litecoworkd --locked media_ingestion` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** any path fetches a URL.

## 016-T11 - Interactive Artifact Runtime kept PREVIEW_ONLY and inert

- **Status:** PARTIAL - no runtime code exists and `PresentationRuntime.tsx` renders MCP_APP as unavailable; the Library/Workbench has no explicit inert preview for interactive Artifacts.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 007 T10 (declared format capability matrix rendering).
- **Owner:** Artifacts, Desktop UI, Security.
- **Allowed paths:** `apps/litecowork-ui/src/artifacts/ArtifactWorkbench.tsx`, `apps/litecowork-ui/src/artifacts/interactive-preview.ts` (new), `apps/litecowork-ui/tests/interactive-artifact-preview.test.ts` (new).
- **Contract:** SPEC L-06, B-40 item 3; INTERACTIVE-ARTIFACT-RUNTIME "Lifecycle" PREVIEW_ONLY; AC-013.
- **Change intent:** for an interactive Artifact show title, exact version, provenance, package validation result and a source view (text only); no Run control; the text "Running interactive apps is not available in this version." No iframe, no `srcdoc`, no script.
- **Acceptance:**
  - [ ] No Run control and no iframe in the rendered tree (AC-013).
  - [ ] Source shown as escaped text (AC-013).
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; packaged check on both rows in T16.
- **Stop:** any proposal enables Run before E08-S09 evidence and a new accepted revision.

## 016-T12 - No-executable-content gate, hostile corpus and CSP check

- **Status:** PARTIAL - `presentation-runtime-fallback.test.mjs` covers escaping and unsupported kinds for `PresentationRuntime.tsx`; no static gate for the presentation directory; CSP is not tested.
- **Depends on:** T01, T06, T07
- **Cross-package prerequisites:** Feature 019 CSP audit of `tauri.conf.json`.
- **Owner:** Security, Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/tests/presentation-no-exec-gate.test.mjs` (new), `apps/litecowork-ui/tests/presentation-hostile-corpus.test.ts` (new), `crates/domain-presentation/tests/corpus/`.
- **Contract:** SPEC L-07, L-08, G-09, section 10; AC-005, AC-006.
- **Change intent:** a test that scans `apps/litecowork-ui/src/presentation/` for `dangerouslySetInnerHTML`, `eval(`, `new Function`, `innerHTML`, `srcdoc`; a hostile corpus (script tags, `javascript:` text, event-handler strings, forged APPROVED states) rendered through every block; a check that `tauri.conf.json` keeps `script-src 'self'` and no `unsafe-eval`; a bridge-call spy proving no rendered element invokes a state-changing command.
- **Acceptance:**
  - [ ] Gate test fails on an injected forbidden token and passes on the clean tree (AC-005).
  - [ ] Hostile strings render as text; no bridge call (AC-006).
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** any exception is requested for a forbidden API.

## 016-T13 - Failure and recovery suite

- **Status:** NOT STARTED
- **Depends on:** T01, T08, T10
- **Cross-package prerequisites:** NONE
- **Owner:** Presentation Runtime, Storage.
- **Allowed paths:** `apps/litecoworkd/tests/rich_presentation_intelligent_ui.rs` (new), `crates/storage-sqlite/src/rich_presentation_tests.rs`, `apps/litecowork-ui/tests/presentation-stream.test.ts`.
- **Contract:** SPEC section 11; contracts/errors-recovery.md; AC-014, AC-015.
- **Change intent:** concurrency test of two publishes for one message; replay test; crash injection between blob write and record commit for presentations and media; stream gap followed by resync from `GET /v1/conversations/{conversation_id}/presentation` with no duplicate messages; daemon restart with a published presentation still readable.
- **Acceptance:**
  - [ ] Exactly one of two racing publishes commits (AC-014).
  - [ ] Orphan blobs removed; stream resync has no duplicates (AC-015).
- **Command:** `cargo test -p litecoworkd --test rich_presentation_intelligent_ui --locked recovery` (target and filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** a recovery path rewrites a semantic message.

## 016-T14 - Performance budgets

- **Status:** NOT STARTED
- **Depends on:** T01, T03, T04, T06, T07
- **Cross-package prerequisites:** Feature 019 release-row reference machines.
- **Owner:** Presentation Runtime, Release.
- **Allowed paths:** `crates/domain-presentation/tests/perf.rs` (new; plain timed test, no benchmark crate added), `apps/litecowork-ui/tests/rich-presentation-perf.test.ts` (new).
- **Contract:** SPEC section 14; AC-016.
- **Change intent:** benchmark the Rust validator on the largest corpus file and a 512 KiB synthetic file; measure TypeScript validation time and the longest main-thread task while rendering 200 blocks; record results per release row. If the TypeScript budget fails, move slice digest checks to a single pass over the encoded bytes before considering a Web Worker.
- **Acceptance:**
  - [ ] Rust p95 at most 50 ms; TypeScript p95 at most 150 ms; no task over 100 ms on render (AC-016).
- **Command:** `cargo test -p domain-presentation --release --locked --test perf -- --nocapture` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** numbers per row in the evidence bundle.
- **Stop:** budgets are met only by skipping validation.

## 016-T15 - Cross-layer traces for every user-visible operation

- **Status:** NOT STARTED
- **Depends on:** T01, T06-T13
- **Cross-package prerequisites:** NONE
- **Owner:** Product, component owners.
- **Allowed paths:** `specs/016-rich-presentation-intelligent-ui/contracts/ui-behavior.md`, `specs/016-rich-presentation-intelligent-ui/WIRING.yaml`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`.
- **Contract:** AGENTS.md section 4; AC-004, AC-018.
- **Change intent:** trace rich answer view, chart table toggle, calculator use, disable preference, image Artifact view and interactive Artifact preview from control to owner record and evidence test.
- **Acceptance:**
  - [ ] Every control maps to an owner route or is documented as view-local (calculator, chart toggle).
  - [ ] Each trace cites a passing test.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/audit_v1_contracts.py`
- **Evidence:** trace table in contracts/ui-behavior.md.
- **Stop:** a control has no owner and is not view-local.

## 016-T16 - Release-row qualification (E2E-11) on Windows x64 then Linux x64

- **Status:** NOT STARTED
- **Depends on:** T01-T15
- **Cross-package prerequisites:** Feature 001 T09 (qualified native Conversation route; a real answer is needed); Feature 011 T17 (packaged qualification of the shell); Feature 019 release matrix and evidence bundle.
- **Owner:** Product, Release, repository owner (USER evidence).
- **Allowed paths:** `apps/litecoworkd/tests/rich_presentation_intelligent_ui.rs`, `apps/litecowork-ui/tests/` (E2E driver for this journey), evidence bundle path defined by Feature 019, `CURRENT-RUN.md`.
- **Contract:** ACCEPTANCE-AND-E2E-PLAN E2E-11; SPEC section 15; AC-004, AC-005, AC-010, AC-013.
- **Change intent:** create the canonical test target; on the packaged desktop with OpenCode v2.0.32, ask a question whose answer has a numeric table: verify table, chart and calculator; disable rich presentation and verify full semantic text; inject a malicious presentation and verify fallback with no host-state change; open an interactive Artifact and verify inert preview; ingest a generated image and verify the Artifact provenance. Windows x64 first, then Linux x64.
- **Acceptance:**
  - [ ] Canonical command passes on each row with recorded build, OS and harness version.
  - [ ] Owner USER evidence for E2E-11 on each row, including the malicious-presentation and disabled cases.
- **Command:** `cargo test -p domain-presentation --locked && cargo test -p litecoworkd --test rich_presentation_intelligent_ui --locked -- --nocapture && pnpm --dir apps/litecowork-ui test`
- **Evidence:** evidence bundle entries; IMPLEMENTATION-STATUS rows per row.
- **Stop:** any malicious case changes host state.

## 016-T17 - Documentation and ledger sync

- **Status:** NOT STARTED
- **Depends on:** T01, T15, T16
- **Cross-package prerequisites:** NONE (orchestrator owns shared docs).
- **Owner:** Documentation owner, orchestrator.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/PRESENTATION-RUNTIME.md` (current-state lines only, for example the CHART placeholder note), `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/schemas/served-route-inventory.json`, `CURRENT-RUN.md`.
- **Contract:** AGENTS.md sections 3 and 5; AC-013.
- **Change intent:** update the five-layer output rows (semantic, rich presentation, generated media, Intelligent UI, interactive Artifact) with exact evidence class per row; keep Interactive Artifact Runtime TARGET with PREVIEW_ONLY posture; regenerate the route inventory (no new public route expected).
- **Acceptance:**
  - [ ] No row claims more than its evidence.
  - [ ] Route inventory unchanged or regenerated consistently.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** PR diff.
- **Stop:** a status upgrade lacks evidence.

## 016-T18 - Per-block Markdown degradation with code and TEXT_SLICE rendering (P-03)

- **Status:** NOT STARTED - task remains gated by the package's IN_REVIEW status and T01. Adjacent Artifact Markdown preview code exists, but does not satisfy AC-019: it uses whole-message fallback for unsupported syntax, lacks GFM tables/horizontal rules/nested lists and code label/copy/highlighting, and RichResponseView renders TEXT_SLICE as plain text. Folds proposed task P-03 from docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md section 9 (findings R-05, R-06) into this package.
- **Depends on:** T06
- **Cross-package prerequisites:** Feature 001 committed AGENT message text is the input the renderer consumes.
- **Owner:** Presentation, Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/artifacts/StructuredTextPreview.tsx`, `apps/litecowork-ui/src/artifacts/structured-text-preview.css`, `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-response.css`, `apps/litecowork-ui/tests/`, adjacent tests. The earlier `src/conversations/` paths did not match the active source tree; the canonical UI paths are corrected here for future authorized work.
- **Contract:** SPEC §14; AC-019; INTELLIGENT-UI-AND-IMAGE-CONTENT; AGENTS.md section 8.
- **Change intent:** replace the all-or-nothing Markdown parse with per-block degradation so GFM tables, horizontal rules and nested lists render without a whole-message raw fallback; give each code block a language label, a copy button and lazy highlighting that returns React nodes (never HTML strings); render TEXT_SLICE through the same Markdown renderer; and keep the no-executable-content posture (no `dangerouslySetInnerHTML`, https-only links). Add a golden corpus of real agent answers.
- **Acceptance:**
  - [ ] The golden corpus of real agent answers renders with no whole-message raw fallback and no HTML strings (AC-019).
  - [ ] Code blocks show a language label, a copy button and lazy highlighting returning React nodes (AC-019).
  - [ ] TEXT_SLICE renders through the same Markdown renderer; links stay https-only and no executable content is emitted (AC-019).
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** golden-corpus render test output in Completion evidence.
- **Stop:** a block cannot degrade safely; render that block as plain text, never the whole message raw.

## 016-T19 - Package VERIFIED for the accepted V1 scope

- **Status:** NOT STARTED
- **Depends on:** T01-T18, T20-T25
- **Cross-package prerequisites:** Feature 019 release evidence bundle accepted.
- **Owner:** Product, repository owner.
- **Allowed paths:** `specs/016-rich-presentation-intelligent-ui/`.
- **Contract:** SPECIFICATION-STANDARD lifecycle; every AC included in the dated T01 V1 acceptance, currently proposed as AC-001..AC-041.
- **Change intent:** assemble evidence for every accepted V1 AC and cross-owner journey; move ACCEPTED/LOCKED to IMPLEMENTING only when T02 starts, and to VERIFIED only after each implementation/qualification task closes and the owner signs off. Deferred ACs must be removed from release promise and traceability before this task closes.
- **Acceptance:**
  - [ ] Every accepted AC has exact test, SYSTEM and USER evidence required by its row on each release platform; a unit test or route alone is insufficient.
  - [ ] Owner sign-off and release matrix evidence recorded; no deferred AC is described as delivered.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** Completion evidence table below.
- **Stop:** any AC lacks evidence.

## Dependency order

T25 cross-owner Saved View decision -> T01 revised whole-scope acceptance -> T02 canonical contract promotion -> T03 validator -> T04/T05 parity and calculation -> T06 chart -> T07 calculator -> T08 publisher -> T09 preference. T10 media and T11 inert Artifact preview follow the exact Feature 007 owner gates. T12-T14 security/recovery/performance -> T15 traces -> T16 base E2E-11 -> T17 documentation; T06 -> T18 Markdown degradation. In parallel after T02/T20 provider input is either exactly qualified and promoted or deliberately left text-only; T20 -> T21 shared Canvas controller -> T22 optional remembered inputs and T23 Saved View/ordinary-turn modification. T16 plus T20-T23 -> T24 revised journeys and packaged qualification. T17/T18/T24 -> T19 VERIFIED. T23 does not depend on T22, but does require Feature 007 to accept the Saved View representation. If Saved View is deferred, revise T01, T23, AC-028/033-036 and the release plan before implementation.

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T08 | 001-T06 | Binder runs after the final response commit with stable identity |
| T16 | 001-T09 | A qualified native Conversation route produces the real answers E2E-11 uses |
| T10 | 007-T02 | Atomic Artifact create with provenance (B-30) |
| T10 | 007-T08 | Conversation association of Artifacts |
| T11 | 007-T10 | Declared format capability matrix and read-only rendering |
| T09 | 011-T07 | Local display preference owner |
| T16 | 011-T17 | Packaged shell qualification on Windows x64 then Linux x64 |
| T07 | Feature 011 app-wide keyboard and focus baseline | Calculator and chart toggle accessibility |
| T10 | Feature 011 image display scheme in CSP | Media display without weakening CSP |
| T12, T14, T16, T18 | Feature 019 CSP audit, reference machines, release matrix and evidence bundle | Security and qualification are per release row |

Feature 019 task plans are being written in the same release-planning pass, so those rows name the outcome; the orchestrator resolves them to task IDs in the traceability index.

## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), WebView (WebView2 or WebKitGTK version), harness and version (OpenCode v2.0.32), evidence class reached (CODE, SYSTEM, USER), report path and reviewer. Statuses above are source-audit classifications at this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
| (none yet) | | | | | | | |

## 016-T20 - Provider ingress and canonical declarative contract

- **Status:** BLOCKED - revision 0.4 TARGET proposal; requires T25/T01 acceptance and T02 canonical contract promotion; no implementation authorized.
- **Depends on:** T01, T02, T25
- **Cross-package prerequisites:** Feature 001 native provider adapter/turn correlation; Feature 002 current AgentControlDescriptor.
- **Owner:** AgentModule, Presentation Runtime, Security, Product.
- **Allowed paths:** docs/ACP-PRESENTATION-PIPELINE-LLD.md; docs/RICH-RESPONSE.md; docs/HOST-GUIDANCE.md; docs/schemas/presentation-intent.schema.json; docs/schemas/rich-presentation.schema.json; owning schemas/events/OpenAPI and AgentModule conformance fixtures.
- **Contract:** SPEC §20; pipeline LLD; AC-020/021/023/024/029-031/038/039.
- **Change intent:** Pin exact supported provider/version/auth route and prove whether it can carry a correlated presentation proposal. If not, complete deterministic host composition and text fallback without requiring provider support. If yes, resolve A-85's model-visible input, trusted sequence/replay identity, typed acknowledgement and canonical mapping; never assume ACP capability. Promote metric semantics and registered local controls only in the canonical schema. A tool observation cannot execute.
- **Acceptance:**
  - [ ] Provider ignores guidance, lacks injection, emits foreign/replayed intent or unknown calculation: safe semantic answer; metric latest/total/change are distinct and provenance-bound.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `python .specify/scripts/validate_feature_package.py; future AgentModule fixture command recorded when implemented`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** Missing exact provider evidence, canonical contract promotion or owner acceptance.

## 016-T21 - One controller for inline and contextual Workbench Canvas

- **Status:** BLOCKED - revision 0.4 TARGET proposal; T25/T01/T02 gates must close; no implementation authorized.
- **Depends on:** T01, T02, T04, T05, T07, T08, T20
- **Cross-package prerequisites:** Feature 011 Workbench shell and keyboard/focus owner.
- **Owner:** Presentation Runtime, Desktop UI, Accessibility.
- **Allowed paths:** apps/litecowork-ui/src/presentation/; existing Workbench shell; adjacent UI tests; UI ledgers.
- **Contract:** SPEC §20; pipeline LLD; Feature 008 Canvas/UI contract; AC-022/041.
- **Change intent:** Register host Open in Canvas/Return controls and one digest-bound controller in the existing contextual right Workbench; preserve inline input revision, focus, Conversation scroll and composer state without calling an agent or invoking app runtime. Do not add a permanent sidebar destination or start computer capture/processes.
- **Acceptance:**
  - [ ] Repeated expand/collapse preserves exact message/document/input revision and controller; focus, scroll and composer restore; Workspace switch/revocation clears protected content.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** A second controller, hidden agent call, persistent write or unqualified component is required.

## 016-T22 - Review and implement optional remembered answer inputs

- **Status:** BLOCKED - revision 0.4 TARGET proposal; requires T25/T01 acceptance and a separately approved T02 storage/privacy contract; no implementation authorized.
- **Depends on:** T01, T02, T20, T21
- **Cross-package prerequisites:** Workspace/Conversation retention and deletion owners; Storage migration allocation; Feature 011 authenticated IPC.
- **Owner:** Storage, Presentation Runtime, Security, Privacy, Product.
- **Allowed paths:** owning storage core/sqlite; authenticated presentation Operator and Tauri bridge; presentation UI; canonical OpenAPI/event/state schemas; migration allocation; retention/backup docs and tests.
- **Contract:** SPEC §20; pipeline LLD; Feature 014/017 retention and backup; AC-025/026/040.
- **Change intent:** Obtain explicit owner approval for consent, quota, backup/encryption/deletion/receipt lifetime and migration allocation. Promote exact record/API/events before code. Implement host-only expected-revision writes, receipts, forget fences and exact namespace; never browser storage. Keep this state separate from Saved View ArtifactVersion and executable app data.
- **Acceptance:**
  - [ ] Off by default; no agent exposure; replay/body-conflict, two-window conflict, timeout-after-commit, quota, forget/save/deletion race, restart/backup and revocation fail safely.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `future focused storage/Operator/UI test commands recorded before implementation; python .specify/scripts/validate_feature_package.py`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** B-40 is used as persistence authorization, migration unallocated, or privacy/deletion/backup decision unresolved.

## 016-T23 - Saved View lifecycle and explicit conversational modification

- **Status:** BLOCKED - revision 0.4 TARGET proposal; the product direction is decided, while Feature 007 cross-owner review and T01/T02 contract promotion remain open; no implementation authorized. Conversation modification remains independent of T22 persistence.
- **Depends on:** T01, T02, T20, T21, T25
- **Cross-package prerequisites:** Feature 007 Artifact/Library create, immutable version/update, source-deletion and receipt contracts; Feature 001 ordinary new-turn dispatch and persisted lineage contracts.
- **Owner:** Conversation, Presentation Runtime, Desktop UI, Security.
- **Allowed paths:** Feature 007 Artifact/Library owner paths included in the accepted task, Feature 001 context/provenance paths, presentation host chrome, canonical API/event/state schemas, UI ledgers and adjacent tests.
- **Contract:** SPEC §20, Feature 007 owner contract, Feature 001 send contract, AC-028/033-036/040/041.
- **Change intent:** Add capability-gated Save/Open/Update/Archive via Feature 007 using exact immutable ArtifactVersion, source lineage and durable receipts; create/update append a version and Archive is the only removal action. Saved View preserves declared RichPresentation defaults; session-modified inputs remain transient under B-40 and are not automatically sent. Update requires a new committed eligible presentation and explicit review. Conversational modification uses ordinary Conversation Send with user-authored text, without silently attaching local calculator values or mutating the source message/version.
- **Acceptance:**
  - [ ] Saved View create/open/update/archive follows Feature 007 receipt/version/recovery contract; no control appears without capability. No send on input change; cancel/retry does not mutate original; stale base prompts review; new message identity and host-assigned lineage.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `future focused Conversation/desktop fixture commands recorded before implementation`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** Feature 007 cannot represent this without violating Artifact invariants, owner receipt is missing, automatic input transmission, original message/version rewrite or inferred approval.

## 016-T24 - Revised V1 negative/recovery/accessibility and packaged evidence

- **Status:** BLOCKED - revision 0.4 TARGET proposal; requires accepted scope, implemented owner contracts and completed prerequisite journeys; no implementation authorized.
- **Depends on:** T01, T16, T20, T21, T22, T23
- **Cross-package prerequisites:** Feature 001 qualified provider route and Feature 019 release evidence; existing Windows/release deferrals resolved.
- **Owner:** Security, Accessibility, Release, Product, repository owner.
- **Allowed paths:** presentation/AgentModule/storage test fixtures; release evidence bundle; CURRENT-RUN.md; implementation status and Feature 016 contracts.
- **Contract:** SPEC §20; pipeline LLD; all accepted AC-001..AC-041, especially denial, Saved View, state and Canvas behavior; canonical E2E ID and steps assigned by T02.
- **Change intent:** Exercise E2E-11 fallback plus the rich-answer-to-Canvas continuity journey proposed in SPEC §20.11 (candidate E2E-18; T02 verifies/assigns the global ID). Exercise semantic source-to-render, provider fallback/ingress, scope fencing, misleading metrics, unknown component/kernel, controller identity, consent, quotas, receipts, timeout/crash/restart, forget/save/archive race, revocation, stale version, scroll/focus recovery and explicit new-message modification on exact packaged release rows. Include the Saved View create/open/update/archive path only after T25 cross-owner review and T01/T02 promotion; include remembered-state steps only if their separate B-46/T22 behavior is accepted.
- **Acceptance:**
  - [ ] Record build/OS/WebView/provider/model/version/auth/ingress, fixture counts, SYSTEM captures and owner USER review; no V1 qualification claim until every accepted AC has exact evidence.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `python .specify/scripts/validate_feature_package.py; exact future packaged E2E command supplied by release owner`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** Any authority leak, content resurrection, stale overwrite, hidden native/model dispatch or missing owner evidence.

## 016-T25 - Resolve Saved View cross-owner design boundary

- **Status:** PARTIAL - product owner direction recorded 2026-11-17 to keep Saved Views in V1 and amend Feature 007 B-21; the bounded Artifact contract is drafted in `specs/007-professional-artifacts/contracts/saved-view.md`. Cross-owner review and canonical API/schema promotion remain open. This is a design gate, not implementation authorization.
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 007 Artifact/Library owner review; Storage, Conversation, Workspace deletion, Backup, Privacy and Product review of provenance, exact-version reopen/update, source deletion, declared-default snapshot, receipts, retention and recovery.
- **Owner:** Artifact, Product, Presentation Runtime, Security, Storage, Privacy and Conversation; decision by the repository owner.
  - **Allowed paths:** `specs/016-rich-presentation-intelligent-ui/`, `specs/007-professional-artifacts/`, `docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/ACP-PRESENTATION-PIPELINE-LLD.md`, `docs/ARTIFACTS-EVIDENCE.md`, `docs/SCHEMAS.md`, `docs/schemas/saved-view-artifact.schema.json`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/V1-PRODUCT-RELEASE-CONTRACT.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/SPEC-TASK-TRACEABILITY.md`, `docs/V1-FEATURE-SDD-REGISTER.md`, `docs/V1-PRODUCT-FEATURE-COVERAGE.md`, `docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md`, `CURRENT-RUN.md`.
- **Contract:** SPEC §20.6/§20.9 and AC-028/033-036; Feature 007 Artifact/Library contract; SPECIFICATION-STANDARD §7; A-84/A-85/A-103.
  - **Change intent:** record the product owner's selection to keep Saved Views in V1 and amend B-21, then specify a bounded, non-executable Artifact snapshot without weakening other Artifact format rules. Feature 007 revision 1.1 remains in review: Markdown remains the first professional document format; one exact Saved View media type is proposed as a separate Artifact kind, backed by the canonical RichPresentation plus semantic fallback and exact Conversation/message/presentation lineage. Only the RichPresentation's declared defaults are retained; session-modified calculator values are excluded under B-40. The proposed contract uses ArtifactVersion/ResourceRevision and existing create/version receipts/events; it adds no SavedView table, event, side store, hard-delete route, arbitrary executable content or permanent navigation destination. Archive is the V1 removal action. Source deletion, replay, version conflict, Workspace fencing, backup, retention and restart recovery are specified in `specs/007-professional-artifacts/contracts/saved-view.md`. Do not disguise structured JSON as Markdown or treat it as `INTERACTIVE_APP`. This direction does not accept Feature 016 as a whole, promote or serve routes, or authorize code. T01 remains the separate whole-scope owner acceptance; T02 remains contract promotion before implementation. T22 remembered-input persistence and T20 provider ingress remain separate; A-85 is not implied by T25.
- **Acceptance:**
    - [x] A dated product-owner direction records “Keep Saved Views in V1; amend Feature 007 B-21” in the acceptance packet and both feature packages; this records the scope choice, not approval of unreviewed technical riders.
    - [ ] Artifact, Storage, Conversation, Privacy, Backup and Product owners review the Saved View contract; promoted API/schema, dependency map and implementation task scopes are consistent before Feature 016 T01 review.
    - [ ] A-103 and all V1/UI traceability records state the decision and its remaining gates; no permanent navigation destination is added.
    - [x] No code, route, tool or persistent state behavior is enabled by this decision task alone.
- **Command:** `python .specify/scripts/validate_feature_package.py`
- **Evidence:** owner decision record and validator output.
  - **Stop:** cross-owner review or canonical contract promotion is absent, source/deletion/receipt/version behavior drifts from the Saved View contract, or this decision is mistaken for T01 whole-feature acceptance.
