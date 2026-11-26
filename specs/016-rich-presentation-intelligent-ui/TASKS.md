# Tasks - Intelligent Workspace Presentation Runtime

**Feature:** 016 | **Status:** IN_REVIEW - revision 0.4 V1 proposal (AC-001..AC-041). USER-DELEGATED implementation direction is recorded in A-84 for the complete current V1 proposal. This does not complete T01/T25, accept or lock the package, or replace cross-owner review, protocol evidence or release qualification.
**Spec revision:** 0.4 (proposed) | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** Presentation Runtime, Conversation, Artifacts, Storage, Operator/API, Desktop UI, Security, Accessibility, Product.

Cross-package task references inside task fields are written as "Feature NNN Txx" so that each hyphenated cross-package ID appears once in this file (the structural validator rejects a repeated task ID); the hyphenated IDs are listed once in "Cross-package dependencies" below.

## Current baseline

Source cross-check: active source at `main` HEAD `8af299d8b2e305b8411a704ff4e9f0152801b09f` plus the uncommitted worktree. The pure Rust validator and TypeScript mirror validate renderer-v1/v2, including source-bound METRIC/CHART/SAFE_CALCULATOR and the closed source-grounded metric context. SQLite uses the Rust validator on publish/read. The desktop parser maps fully validated renderer-v2 METRIC/CHART/SAFE_CALCULATOR into registered render models; T07 is CODE/UI for local calculator interaction. T08 now wires a post-commit production publisher for exact semantic Markdown TABLE and byte-bound TEXT_SLICE blocks, with deterministic identity and tested exact storage replay/conflict. It does not infer or publish CHART/SAFE_CALCULATOR and does not provide provider structured-intent ingress. This remains CODE, not provider integration or qualification. A-84 records user direction to implement the complete current proposal while preserving IN_REVIEW lifecycle and separate cross-owner, protocol, platform and qualification gates. Saved View remains TARGET and has no route; its proposed envelope may include a user-consented numeric `input_snapshot`, omitted by default. Calculator state remains local/non-submitting under B-40. Device-local Remember/Forget remains separate under T22; ordinary-turn sharing is a user-reviewed visible message draft sent only through Feature 001 ordinary Send (T23).

- **Desktop validator:** `apps/litecowork-ui/src/presentation/rich-presentation.ts` with `MAX_DOCUMENT_BYTES = 512 * 1024`, `MAX_BLOCKS = 200`, `MAX_DEPTH = 8`, `MAX_JSON_VALUES = 30_000`, `MAX_JSON_DEPTH = 32`; its contract validator accepts the shared renderer-v1/v2 profile. V2 adds validated METRIC, CHART and SAFE_CALCULATOR with Rust-aligned bounds. METRIC/CHART require the closed source-grounded context and preserve raw source values. Both validators reject non-finite and integer-valued numbers outside the exact binary64 safe-integer range, including exponent notation. The renderable response path exposes METRIC/CHART; SAFE_CALCULATOR and unsupported future blocks preserve the complete semantic fallback.
- **Validator owners:** `crates/domain-presentation/` owns Rust closed document/block checks, canonical bytes, digests, source bindings, provenance, metric context, CalcAst limits and profile allowlists. `SqliteRichPresentationStore` delegates to it on publish and read; `storage-sqlite` retains record/semantic authorization and the 512 KiB cap. The TypeScript contract validator mirrors renderer-v1/v2 through the shared corpus. `publish_rich_presentation` now has a daemon binder caller after semantic settlement; the caller currently produces TABLE/TEXT_SLICE only.
- **Served reads:** `GET /v1/rich-presentations/{presentation_id}` (`get_rich_presentation`), `GET /v1/conversations/{conversation_id}/presentation`, `POST /v1/stream`; Tauri `get_rich_presentation` in `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs`.
- **Renderer:** `apps/litecowork-ui/src/presentation/RichResponseView.tsx` (used by `ConversationsPage.tsx`) renders v1 blocks plus validated v2 METRIC/CHART/SAFE_CALCULATOR. CHART is host-generated SVG with an accessible table toggle; METRIC displays the unchanged bound number beside its source-derived label and declared context; SAFE_CALCULATOR uses bounded numeric inputs, the pure T05 evaluator, transient `useReducer` state, readable formula text, polite result announcements, reset and always-visible semantic fallback. These are local CODE/UI components only; the table-only post-commit publisher is CODE, while provider structured-intent ingress, media, Canvas and the sanitized real-agent answer corpus remain open. The 2026-11-24 T18 source uses bounded block-local Markdown parsing for semantic fallback and `TEXT_SLICE`, with GFM tables, horizontal rules, nested lists, code label/copy/lazy highlighting and inert hostile content. `PresentationRuntime.tsx` still shows MCP_APP as unavailable.
- **Tests:** `apps/litecowork-ui/tests/rich-presentation-corpus.test.ts` and `crates/domain-presentation/tests/corpus/cases.json` now provide shared cross-language vectors; the existing Markdown, RichPresentation, fallback, response-view and T18 browser checks remain. Rust `crates/storage-sqlite/src/rich_presentation_tests.rs` covers owner storage validation.
- **Absent:** provider structured-presentation ingress, canonical ChartIntent/CalcIntent binding through T20, media ingestion, `PRESENTATION_*` error codes, Artifact create route (Feature 007 B-30), and any Interactive Artifact Runtime code. SAFE_CALCULATOR UI exists as CODE/UI but is not published by the current table-only binder. Canonical RichPresentation and CalcAst schemas exist. CSP in `apps/litecowork-ui/src-tauri/tauri.conf.json` is `script-src 'self'` with `img-src 'self' asset: http://asset.localhost data:`.

Gaps to proposed V1: formal revised-scope acceptance remains open at T01 and cross-owner Saved View ratification remains open at T25. Rust/TypeScript contract parity and its shared corpus are CODE; the repository-wide UI test command timed out in an unrelated Vite module-fetch path, and the latest completed suite remains red. T08's table-only after-settlement publisher is now CODE; provider presentation-intent ingress, canonical chart/calculator publication, binder/storage integration failure tests, media provenance, preference wiring, Canvas controller, Saved View lifecycle and packaged evidence remain open. A-84 authorizes user-directed implementation of scoped tasks; it does not accept or lock the package. Feature 007 revision 1.1 remains IN_REVIEW and no save route exists. All AC-001..AC-041 remain TARGET until formally accepted and qualified.

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
  - [ ] Negative: no implementation task starts solely because this package or task exists. Explicit USER-DELEGATED tasks may proceed only within their recorded scope and without claiming ACCEPTED/LOCKED status, cross-owner ratification, external protocol evidence or qualification.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** dated owner decision in OWNER-ACCEPTANCE-REVIEW-PACKET Part 6, A-84/A-103 updates, and the exact included/excluded AC set in SPEC/TASKS.
- **Stop:** T25 lacks an Artifact owner decision, B-46 is unaddressed, any included item lacks its owner/rider, or a deferred item remains in the V1 promise.

## 016-T02 - Promote accepted interfaces into owner docs and machine schemas

- **Status:** PARTIAL - canonical target contracts are being promoted under A-84; formal whole-scope acceptance remains open at T01, and cross-owner ratification remains open at T25.
- **Depends on:** A-84 user-delegated direction and T25's recorded product direction. T01 formal acceptance is a package-lifecycle gate, not a prerequisite for implementation explicitly directed by the user.
- **Cross-package prerequisites:** Identify exact Feature 007/008/011/001 owner operations and preserve their accepted invariants. Under A-84, internal design choices may be resolved and implemented; cross-owner ratification remains a separate release gate.
- **Owner:** Presentation Runtime, API, Events, Schemas owners.
- **Allowed paths:** `docs/RICH-RESPONSE.md`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/INTERACTIVE-ARTIFACT-RUNTIME.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/ACCEPTANCE-AND-E2E-PLAN.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/schemas/rich-presentation.schema.json`, `docs/schemas/calc-ast.schema.json` (new), `docs/schemas/presentation-intent.schema.json`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `specs/007-professional-artifacts/`, `specs/008-workbench-browser-terminal-computer/`, `specs/011-desktop-shell-ipc-settings-runtime/`, this package.
- **Contract:** SPEC §20 and accepted ACs; canonical Rich Response, Artifact, Workbench, state, event, API and UI owners.
- **Change intent:** promote one user-directed TARGET RichPresentation profile, metric semantics and source bindings, B-40 calculation AST, B-46 local input types and host-owned state actions, policy/fallback, producer provenance, Saved View representation, Canvas ownership, owner errors/events and UI trace. Reconcile global Rich Response, Artifact, Workbench, privacy/retention and release contracts in the same change; do not create a parallel schema or route. Keep formal acceptance and cross-owner review status visible and unresolved until the responsible owners record them.
- **Acceptance:**
  - [ ] Each user-directed interface has one normative owner and canonical machine schema; dependent UI/API/event/error/wiring and coverage maps match. Renderer-v2 METRIC/CHART semantics require an exact source measure, `unit`, closed `scale`, bounded `time_interval`, closed `aggregation` and an exact semantic summary that contains those labels; visible UI displays the declarations and the unchanged source value.
  - [ ] Negative corpus rejects unknown fields, unbound metrics, unregistered kernels and out-of-profile components; valid fallback remains semantic text.
  - [ ] Migration needs, receipt, retention, deletion, backup and rollback are explicit; do not claim no migration if T22 is included.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py`
- **Evidence:** PR link and output in Completion evidence.
- **Stop:** an owner doc conflicts with B-40 or another higher-authority contract; record the conflict and implement only the user-directed choice without claiming owner ratification. External provider facts remain gated on evidence.

## 016-T03 - Single Rust validator crate and drift fix

- **Status:** DONE (Rust CODE for this task's validator scope). The pure `crates/domain-presentation` crate owns the canonical document/block validator for renderer-v1 and renderer-v2; SQLite calls it for both publish and read after owning record, Workspace, semantic-message and blob checks. METRIC/CHART now require a closed, bounded context object whose unit, scale, interval and aggregation are repeated with the exact source labels in an exact committed semantic line. The host preserves raw numeric values and performs no conversion or aggregation. Renderer-v2 remains unavailable to provider answers because production publication is a separate task.
- **Depends on:** T02
- **Cross-package prerequisites:** NONE
- **Owner:** Presentation Runtime, Storage.
- **Allowed paths:** `crates/domain-presentation/` (new crate: `Cargo.toml`, `src/lib.rs`, `src/blocks.rs`, `src/calc.rs`, `src/tests.rs`), `Cargo.toml` (workspace members), `Cargo.lock`, `crates/storage-sqlite/Cargo.toml`, `crates/storage-sqlite/src/rich_presentations.rs`, `crates/storage-sqlite/src/rich_presentation_tests.rs`, `crates/storage-core/src/rich_presentation.rs` (cap constant and doc comment).
- **Contract:** SPEC G-01, G-02, section 7.2; AC-001, AC-002, AC-006.
- **Change intent:** move the closed-object, canonical-bytes, digest, slice, depth and provenance checks from `rich_presentations.rs` into `domain_presentation::validate_document`; add CARD, CALLOUT, TABLE (exact semantic Markdown table match), source-bound METRIC/CHART semantic-context validation, and SAFE_CALCULATOR checks; set `MAX_DOCUMENT_BYTES` to 512 KiB; and have `SqliteRichPresentationStore::publish_rich_presentation` call it. The context checker must require exact source labels and context tokens in an exact committed semantic line, reject controls/bidi overrides, and never transform source values. Remove the stale DIVIDER comment. Dependencies limited to `serde_json`, `serde_json_canonicalizer`, `sha2` (already in the lockfile); no `jsonschema` crate.
- **Acceptance:**
  - [x] The cap is 512 KiB on every surface: the exact-cap Rust document is accepted, the existing over-cap publish case is rejected, and `rich_presentation_document_cap_agrees_with_the_desktop_and_the_wire_schema` pins storage to the desktop and served wire schema (AC-001).
  - [x] Non-allowlisted known kinds and unknown kinds return `UnsupportedBlock`; non-empty actions/citations are rejected; HOST_PROJECTION and MCP_APP cannot enter either profile (AC-002, AC-006). Covered by `unsupported_components_and_unavailable_document_capabilities_fail_closed`.
  - [x] SAFE_CALCULATOR's declared initial inputs, formula result and result binding agree with the exact semantic answer; mismatched result binding is refused. Calculator evaluator tests cover positive/negative ties-away-from-zero rounding, negative precision, division by zero and finite-input overflow.
  - [x] METRIC/CHART require a closed semantic-context object; missing fields, context not repeated by the exact semantic summary, missing source measure/row labels, forbidden controls and invalid enum values fail closed.
  - [x] Existing storage publication, read, provenance, semantic-binding, cap, and rollback cases pass with the shared validator and initial-result check: current `cargo test -p storage-sqlite --locked rich_presentation` passed 28/28, 0 failed, 615 filtered.
- **Command:** `cargo test -p domain-presentation --locked` and `cargo test -p storage-sqlite --locked rich_presentation`
- **Evidence:** current `cargo test -p domain-presentation --locked` passed **15/15**; `cargo test -p storage-sqlite --locked rich_presentation` passed **28/28**, 0 failed, 615 filtered, including the updated renderer-v2 chart publication fixture. This verifies the focused persistence boundary, not the production provider publication journey. Exact outcomes and Windows checkout boundary are recorded in CURRENT-RUN.md.
- **Stop:** the crate needs a storage dependency; keep it pure.

## 016-T04 - Shared golden corpus and TypeScript parity

- **Status:** DONE (CODE for shared-contract parity). Shared Rust/TypeScript corpus parity covers renderer-v1/v2 contract validation, exact bounds, semantic source binding, source-grounded METRIC/CHART context, and the initial SAFE_CALCULATOR result. Both validators reject integer-valued JSON numbers above the exact binary64 safe-integer range, including exponent notation. This is validator parity, not factual verification of the underlying semantic answer.
- **Depends on:** T03
- **Cross-package prerequisites:** NONE
- **Owner:** Presentation Runtime, Desktop UI.
- **Allowed paths:** `crates/domain-presentation/tests/corpus/` (new JSON files), `crates/domain-presentation/src/tests.rs`, `crates/domain-presentation/src/lib.rs`, `apps/litecowork-ui/src/presentation/rich-presentation.ts`, `apps/litecowork-ui/tests/rich-presentation-corpus.test.ts` (new), `specs/016-rich-presentation-intelligent-ui/SPEC.md`, `docs/schemas/rich-presentation.schema.json`, `docs/RICH-RESPONSE.md`.
- **Contract:** SPEC G-01, G-02; RENDERING-CONFORMANCE-LLD; AC-002, AC-003.
  - **Change intent:** one shared JSON corpus of named canonical-document scenarios with an expected verdict and reason; both Rust and TypeScript suites construct/parse the same vectors and enforce renderer-v1/v2 profile parity. Add a contract validator for renderer-v2 METRIC, CHART and SAFE_CALCULATOR with the exact Rust/schema bounds, including source slices, closed source-grounded metric context and initial calculator result matching. All integer-valued numbers whose magnitude exceeds `9007199254740991` must be rejected in both languages, including exponent-form JSON values; Rust and JavaScript must agree on this before accepting a canonical presentation. The response parser may return renderer-v2 only when every contained block is supported by the registered React renderer; currently that means METRIC and CHART, while SAFE_CALCULATOR and unknown blocks preserve the complete semantic answer. Corpus covers exact/over byte and block boundaries, nesting, hostile semantic strings, non-finite and unsafe integer-valued numbers, source/context/result binding mismatches, and unsupported kinds. Add separate TypeScript-only tests for accessors, cycles and hostile Proxy reflection: standard browser JavaScript cannot identify a transparent Proxy, so validation must snapshot bounded own data descriptors once, never invoke getters or later property reads on the supplied object, and fail closed on reflective exceptions. T05 now extracts the initial-value calculation into the shared bounded reducer; no competing CalcAst schema is created.
- **Acceptance:**
  - [x] Rust and TypeScript agree on every shared corpus case, including valid and missing/ungrounded/unsafe/invalid-enum renderer-v2 METRIC/CHART context, initial SAFE_CALCULATOR value/result binding and unsafe integer-valued JSON numbers in integer and exponent forms (AC-003, AC-023).
  - [x] The response parser returns v2 only when every block has a registered renderer; unsupported or unknown kinds keep the complete semantic answer (AC-002).
  - [x] Supplied getters are not invoked, cyclic/shared-object graphs are rejected, Proxy reflection exceptions fail closed, and all subsequent checks run against the bounded descriptor snapshot. Transparent Proxy detection is not promised by the browser runtime.
- **Command:** `cargo test -p domain-presentation --locked corpus` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** `cargo test -p domain-presentation --locked` passed **15/15**; `node --experimental-transform-types --test tests/chart-model.test.ts tests/rich-presentation.test.ts tests/rich-presentation-corpus.test.ts` passed **36/36**; `pnpm exec tsc --noEmit` passed; the shared corpus includes missing, ungrounded, unsafe-control, invalid-enum and substring-boundary cases. Production build and the rendered Chrome presentation group passed. See CURRENT-RUN.md for exact results. This is CODE only; there is no provider integration or qualification.
- **Stop:** a disagreement is resolved by loosening either side; tighten instead.

## 016-T05 - Restricted CalcAst validator (Rust) and pure reducer (TypeScript)

- **Status:** DONE - CODE for the bounded CalcAst v1 validator/evaluator parity and reusable TypeScript reducer; no renderer, publishing, provider integration or qualification is implied.
- **Depends on:** T03
- **Cross-package prerequisites:** NONE
- **Owner:** Presentation Runtime.
- **Allowed paths:** `crates/domain-presentation/src/calc.rs`, `crates/domain-presentation/src/tests.rs`, `crates/domain-presentation/tests/corpus/`, `apps/litecowork-ui/src/presentation/calc-reducer.ts` (new), `apps/litecowork-ui/src/presentation/rich-presentation.ts`, `apps/litecowork-ui/tests/calc-reducer.test.ts` (new), `specs/016-rich-presentation-intelligent-ui/SPEC.md`, `specs/016-rich-presentation-intelligent-ui/README.md`, `specs/016-rich-presentation-intelligent-ui/RESEARCH.md`, this TASKS.md, `docs/IMPLEMENTATION-MASTER-PLAN.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/SCHEMAS.md`, `docs/audit/SUPERSET-DESIGN-DISPOSITION-2026-11-23.md`, `CURRENT-RUN.md`.
- **Contract:** SPEC L-03, B-40; INTELLIGENT-UI section 3; AC-007.
- **Change intent:** Complete Rust `validate_calc_ast`'s negative matrix for node kinds, closed fields, at most 64 nodes, depth at most 12, finite literals, declared VARIABLE inputs, supported operators and function arity; cover ROUND's literal integer precision from -6 through 6 and static CLAMP bound order. Keep Rust initial-value evaluation and its committed-result binding. Add one exported TypeScript `evaluateCalc(ast, inputValues)` in `calc-reducer.ts`: an iterative, bounded postorder evaluator over a snapshot of the provided finite numeric input map, returning `{ ok: true, value }` or `{ ok: false, reason }` where `reason` is one of `DIVIDE_BY_ZERO`, `NON_FINITE`, `INVALID_AST`, `INVALID_BOUNDS`, `MISSING_INPUT` or `INVALID_INPUTS`. Implement only CalcAst v1 and match Rust binary64 operations, including ties-away-from-zero ROUND for negative and positive precision. Reuse this evaluator in `rich-presentation.ts` for SAFE_CALCULATOR's initial result check instead of keeping a second TypeScript AST evaluator. Property tests use a deterministic seeded generator and verify every generated valid AST returns a finite value or typed failure without throwing. No string parsing of formulas, generated code, IPC, persistence or provider action.
- **Acceptance:**
  - [x] Every Rust AST rejection rule has a focused negative test, including closed fields, malformed nodes, operators, identifiers, arity, ROUND precision, CLAMP bounds, node/depth limits and non-finite arithmetic (AC-007).
  - [x] TypeScript unit cases cover every operator/function, negative and positive ROUND ties/precision, divide by zero, overflow, invalid AST, missing/non-finite inputs and complexity limits.
  - [x] Reducer property test: for deterministic generated valid ASTs and finite inputs the result is finite or a typed reason; never throws or mutates its AST/input map (AC-007).
- **Command:** `cargo test -p domain-presentation --locked`; from `apps/litecowork-ui`, `node --experimental-transform-types --test tests/calc-reducer.test.ts tests/rich-presentation.test.ts tests/rich-presentation-corpus.test.ts`; `pnpm build`.
- **Evidence:** Rust domain-presentation passed **15/15**; focused TypeScript reducer/parser/corpus passed **34/34**; production build passed Node/API checks, `tsc --noEmit` and Vite. Vite reports the existing main bundle over 500 KiB (1,154.89 KiB); Node reports its expected experimental type-transform warning. See CURRENT-RUN.md. This is CODE only; renderer-v2 remains unavailable until the component renderers and publishing path exist.
- **Stop:** any path evaluates a string expression.

## 016-T06 - Source-bound METRIC and CHART blocks with table fallback

- **Status:** DONE (CODE/UI for METRIC/CHART rendering). The metadata contract for METRIC/CHART is enforced and displayed: source measure labels come from bound table headers/rows, and the declared unit/scale/interval/aggregation must be grounded in the exact committed semantic answer. The host shows the original value without conversion or aggregation. This does not verify the truth of provider-authored claims or implement the broader AC-023 derived-metric fixtures owned by T20/T24. SAFE_CALCULATOR rendering is owned by T07. Production publication/provider ingress, a real native-agent answer corpus, Canvas continuity and packaged qualification remain open; PresentationRuntime is not a chart provider.
- **Depends on:** T03, T04
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Accessibility.
- **Allowed paths:** `apps/litecowork-ui/src/presentation/MetricBlock.tsx` (new), `apps/litecowork-ui/src/presentation/ChartBlock.tsx` (new), `apps/litecowork-ui/src/presentation/chart-model.ts` (new), `apps/litecowork-ui/src/presentation/rich-presentation.ts`, `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-response.css`, `apps/litecowork-ui/tests/chart-model.test.ts` (new), `apps/litecowork-ui/tests/rich-presentation.test.ts`, `apps/litecowork-ui/tests/rich-presentation-corpus.test.ts`, `apps/litecowork-ui/tests/browser/harness.tsx`, `apps/litecowork-ui/scripts/check-rendered-ui.mjs`, `crates/domain-presentation/src/blocks.rs`, `specs/016-rich-presentation-intelligent-ui/TASKS.md`, `specs/016-rich-presentation-intelligent-ui/SPEC.md`, `specs/016-rich-presentation-intelligent-ui/README.md`, `specs/016-rich-presentation-intelligent-ui/WIRING.yaml`, `specs/016-rich-presentation-intelligent-ui/contracts/ui-behavior.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/RICH-RESPONSE.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/SCHEMAS.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/audit/SUPERSET-DESIGN-DISPOSITION-2026-11-23.md`, `CURRENT-RUN.md`.
- **Contract:** SPEC L-04, G-04; RICH-RESPONSE "Tables and charts"; AC-008, AC-010.
- **Change intent:** Extend the existing bounded desktop parser so its branded renderable type can carry renderer-v2 METRIC and CHART only when every block in the document is supported by the current renderer. A valid SAFE_CALCULATOR or unknown future block remains contract-valid where its Rust/schema rules allow it but stays on the complete semantic-answer path until its renderer ships. `chart-model.ts` reads only the exact UTF-8 semantic table slice already checked against the message and slice digest; it builds a bounded table, source-derived labels, registered semantic context and deterministic SVG coordinates. METRIC requires an exact finite source cell, derives measure from the source header and row label (or one-column header plus one-based row number), and displays the original value with explicit unit, scale, interval and aggregation; it performs no conversion, rescaling or arithmetic. CHART requires 2–60 rows, one zero-based category column plus 1–6 distinct in-range finite numeric series columns, source-derived category/series labels, one shared context and preserved source row order. The exact semantic summary must contain the relevant source labels and all declared context. It draws BAR, LINE and AREA with React `<svg>`, `<rect>` and `<path>` generated from the bounded coordinate model (never model-supplied markup). Scale coordinates through a finite normalization that handles negative, constant and very large finite values without overflow. `role="img"` exposes the semantic accessible summary; a keyboard-operable "Show data table" / "Show chart" toggle reveals the identical bound values. The SVG itself is decorative to assistive technology. If the verified source slice exists but display-only metadata/geometry is invalid, show its exact semantic table; if digest/message binding cannot be verified, use the complete semantic answer. PIE and SCATTER remain unsupported. No chart library, provider call, IPC, action or persistence is added.
- **Acceptance:**
  - [x] Chart and its exact source table show identical categories, labels and numeric values in source order (AC-008).
  - [x] A display-model rejection with an independently verified slice shows the exact source table; a digest, message or binding-integrity failure leaves the complete semantic answer visible and renders no chart (AC-008).
  - [x] METRIC label and value come only from the bound source header/row/cell; ambiguous labels and unsupported values use the exact source table or complete semantic fallback without guessed meaning.
  - [x] METRIC/CHART context is present, closed, bounded, repeated in the exact semantic summary with source labels, visible to users, and does not change the source numeric value; invalid context uses semantic fallback.
  - [x] Table toggle is reachable by keyboard, has accurate expanded state, restores chart mode by keyboard, and summary is exposed to screen readers; no SVG animation is required (AC-010).
  - [x] All v1 behavior and fallback remain unchanged; after T07, SAFE_CALCULATOR uses its registered local renderer, while unknown or otherwise unsupported future blocks do not leak raw objects into React.
- **Command:** from `apps/litecowork-ui`, `node --experimental-transform-types --test tests/chart-model.test.ts tests/rich-presentation.test.ts tests/rich-presentation-corpus.test.ts`; `pnpm exec tsc --noEmit`; `pnpm run build`; `$env:LITECOWORK_RENDERED_CHECK_GROUP='presentation'; pnpm run test:browser`.
- **Evidence:** the focused model/parser/corpus suite passed **36/36**; `pnpm exec tsc --noEmit` and `pnpm run build` passed; rendered-browser checks passed on Chrome 158.0.8127.38 for chart SVG/table identity and keyboard toggle, metric value/context/source binding, unsupported-calculator semantic fallback, and zero host calls. `cargo fmt --all -- --check` passed. Exact commands are in CURRENT-RUN.md. These are T06 CODE/UI checks only; the later T08 task adds table-only publication, and no provider integration or platform qualification is implied.
- **Stop:** a design needs model-supplied SVG or style.

## 016-T07 - SAFE_CALCULATOR Intelligent UI component (B-40)

- **Status:** DONE (CODE/UI) - the desktop parser maps only fully validated calculator inputs, source-bound initial values, CalcAst and closed output format into an immutable render model. React renders bounded labelled inputs, inert formula text, local recalculation/reset and a persistent semantic fallback. A rendered test exercises invalid arithmetic, keyboard reset, no host calls and axe serious/critical checks. No production publisher/provider path serves calculators.
- **Depends on:** T05, T06
- **Cross-package prerequisites:** Feature 011 app-wide keyboard and focus baseline.
- **Owner:** Desktop UI, Accessibility, Presentation Runtime.
- **Allowed paths:** `apps/litecowork-ui/src/presentation/SafeCalculatorBlock.tsx` (new), `apps/litecowork-ui/src/presentation/safe-calculator-state.ts` (new; pure local input reducer/evaluation/formatting helpers), `apps/litecowork-ui/src/presentation/rich-presentation.ts` (map only fully validated blocks to immutable render models), `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-response.css`, `apps/litecowork-ui/tests/safe-calculator-block.test.ts` (new), `apps/litecowork-ui/tests/browser/harness.tsx`, `apps/litecowork-ui/scripts/check-rendered-ui.mjs`, `apps/litecowork-ui/package.json`, `apps/litecowork-ui/pnpm-lock.yaml` (only if needed for an accessibility audit dependency), `crates/domain-presentation/src/blocks.rs` (only if a source-confirmed validator defect is found), `specs/016-rich-presentation-intelligent-ui/TASKS.md`, `specs/016-rich-presentation-intelligent-ui/contracts/ui-behavior.md`, `specs/016-rich-presentation-intelligent-ui/contracts/api-events.md`, `docs/schemas/rich-presentation.schema.json` (descriptive renderer-evidence text only; no shape change), `docs/ACP-PRESENTATION-PIPELINE-LLD.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/RICH-RESPONSE.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/SCHEMAS.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/V1-PRODUCT-FEATURE-COVERAGE.md`, `docs/V1-FEATURE-SDD-REGISTER.md`, `docs/LITECOWORK-BUILD-TASKS.md`, `docs/audit/SUPERSET-DESIGN-DISPOSITION-2026-11-23.md`, and `CURRENT-RUN.md`.
- **Contract:** SPEC B-40, G-03; INTELLIGENT-UI section 3; canonical SAFE_CALCULATOR promotion; AC-007, AC-009, AC-010.
- **Change intent:** after parser-side binding and AST validation, map only the bounded input IDs/labels/initial cells/ranges, CalcAst and closed output format into an immutable renderer model. Render labelled `<input type="number">` fields (initial values from the bound table cells), the registered formula in readable text, and output in an `aria-live="polite"` region formatted per `output_format`; provide a "Reset to original values" button and keep `fallback_text` visible below. Component state lives in a local `useReducer`; inputs are rechecked for finite/range/step validity before the existing pure T05 evaluator runs. No store, bridge call, provider call, persistence, or automatic value submission. Run a focused rendered-browser flow to assert recalculation, divide-by-zero fallback, reset, accessible names/live output and a zero host-call count; use axe-core for serious/critical accessibility findings.
- **Acceptance:**
  - [x] A rendered bridge-call spy records zero calls during input, reset and result updates (AC-009).
  - [x] Divide by zero and non-finite arithmetic show "Cannot calculate with these values" while the semantic fallback remains visible (AC-007).
  - [x] Inputs are explicitly labelled and bounded, result changes are announced, keyboard reset works, and axe reports no serious/critical issue (AC-010).
- **Completion evidence:** `node --experimental-transform-types --test tests/safe-calculator-block.test.ts tests/rich-presentation.test.ts tests/calc-reducer.test.ts` passed **38/38**; `node node_modules/typescript/bin/tsc --noEmit` passed; `LITECOWORK_RENDERED_CHECK_GROUP=presentation node scripts/check-rendered-ui.mjs` passed on Chrome 158.0.8127.38, including interaction, reset, semantic fallback, zero host calls and no serious/critical axe findings. On 2026-11-25, the equivalent `pnpm run test:browser` rendered `presentation` group passed **3/3** in Chrome 158.0.8127.38. A regression was added to keep `axe-core` out of the Vite browser harness's static UMD import path: the harness now dynamically imports axe only when `auditRichCalculatorA11y` runs; `node --experimental-transform-types --test tests/safe-calculator-block.test.ts` passed **5/5** after the change, `pnpm run build` passed, and the rendered presentation group passed **3/3** again. The rendered fixture is a test harness, not a production publisher/provider path.
- **Command:** focused `safe-calculator-block.test.ts`, rendered `presentation` browser group, and TypeScript check.
- **Evidence:** exact focused test output and axe report recorded in `CURRENT-RUN.md`.
- **Stop:** the component needs to send values to the agent; that is a different, unaccepted feature.

## 016-T08 - PresentationBinder and publish after semantic commit

- **Status:** PARTIAL (CODE) - the daemon calls `publish_rich_presentation` after the semantic response commit. The first deterministic binder profile publishes only exact semantic Markdown tables plus surrounding byte-bound text slices. Storage returns the existing immutable record/event only for exact replay and rejects changed content/bindings. It never guesses CHART/SAFE_CALCULATOR intent; those blocks require the exact canonical validated-intent path owned by T20. Provider presentation ingress, complete task-level fault coverage, packaged integration and qualification remain open.
- **Depends on:** T02, T03, T05
- **Cross-package prerequisites:** Feature 001 T06 (final response settlement with stable identity in the supervisor).
- **Owner:** Presentation Runtime, Conversation, Storage.
- **Allowed paths:** `apps/litecoworkd/Cargo.toml` (direct dependency on the canonical pure domain validator), `Cargo.lock` (workspace dependency edge only), `apps/litecoworkd/src/presentation_binder.rs` (new), `apps/litecoworkd/src/agents/supervisor.rs` (one call after the commit returns), `apps/litecoworkd/src/main.rs` (module wiring only), `crates/domain-presentation/src/blocks.rs` (shared exact Markdown-table extraction helper), `crates/domain-presentation/src/tests.rs`, `crates/storage-sqlite/src/rich_presentations.rs`, `crates/storage-sqlite/src/rich_presentation_tests.rs`, adjacent daemon tests, this package, A-100 alignment and current implementation status.
- **Contract:** SPEC L-01, G-05, section 6 sequence; AC-004, AC-014, AC-017.
- **Change intent:** after `commit_conversation_agent_response` returns, `bind_after_commit` reads only the returned committed message, derives an immutable document containing exact semantic Markdown TABLE blocks and byte-bound TEXT_SLICE blocks for all surrounding content, validates it through the shared domain validator, and publishes in its own transaction. Allocate `presentation_id` and event identity deterministically from the exact committed Workspace/message identity; the existing immutable one-record-per-message row is the replay receipt, so no separate idempotency table or migration is introduced. Storage returns the existing record only for an exact identity/metadata/document/event replay and conflicts on any different digest or cross-message collision. Binder failure increments a content-free counter and is dropped after settlement; it never retries the provider turn or changes the committed message/turn. Do not infer CHART from numeric shape or SAFE_CALCULATOR from prose: these require a canonical, validated PresentationIntent and exact context promoted by T20. This task wires the deterministic semantic-table baseline; CHART/SAFE_CALCULATOR publication remains a dependent T20 completion gate, not a scope deletion.
- **Acceptance:**
  - [ ] Binder failure injection after settlement leaves the committed turn, message, and Conversation events unchanged; failure metrics contain no answer content (AC-004). The call is after commit and the content-free counter is covered, but a Conversation-plus-publication storage integration assertion remains open.
  - [ ] Exact publication replay returns the stored record/event; changed document digest, metadata, message binding, or event under the same deterministic identity conflicts without another row/blob reference/event (AC-014). Focused storage tests cover exact replay, archived replay, changed-document conflict and single row/event; exhaustive metadata/event collision and crash-window coverage remain open.
  - [ ] Only the committed message content and its resource-ref list contribute to the semantic digest; no pre-commit stream buffer, provider-private session handle, credential, or inferred calculation/chart intent is used (AC-017). The current binder hashes the committed resource-ref list but does not claim resource-backed provenance for a table extracted from semantic text.
  - [ ] An answer with a valid GFM pipe table publishes a v1 TABLE companion while preserving every non-table UTF-8 source span as TEXT_SLICE; an answer without a supported table produces no presentation and remains fully readable.
  - [ ] Numeric-looking tables remain TABLE until an exact validated ChartIntent with the required closed semantic context is available through T20; SAFE_CALCULATOR remains absent until canonical calculation intent is promoted and validated.
- **Command:** `cargo test -p litecoworkd --locked presentation_binder` (filter to be added by this task)
- **Evidence:** focused daemon binder tests, domain table-extraction tests, SQLite exact replay/conflict/fault tests and command output in Completion evidence. This is CODE; T20 provider ingress and packaged native qualification remain separate.
- **Focused evidence (2026-11-24):** `cargo test -p domain-presentation --locked` passed 17/17; `cargo test -p litecoworkd --locked presentation_binder -- --nocapture` passed 3/3; `cargo test -p storage-sqlite --locked rich_publication_ -- --nocapture` passed 21/21; `cargo fmt --all -- --check` passed. These establish CODE only; no packaged Conversation/provider journey or qualification was run.
- **Stop:** any binder path runs inside the final-commit transaction.

## 016-T09 - Disable preference and semantic fallback (E2E-11)

- **Status:** DONE (CODE/UI); E2E-11 remains open. `Settings > General > Show formatted answers` is persisted as Feature 011's local display preference, defaults on for legacy snapshots, selects the complete semantic answer when off, and prevents new automatic/manual `get_rich_presentation` reads. A previously dispatched read cannot be cancelled; disabling clears visible rich state and late responses are ignored. The preference does not alter the turn-captured presentation policy or provider request. The rendered Conversation group reopened a saved Conversation with the preference off, verified semantic text remained visible and confirmed zero optional presentation reads; focused preference/policy tests, keyboard behavior, same-storage reload and simulated storage-failure disclosure also pass. This remains CODE/UI evidence, not packaged E2E.
- **Depends on:** T02, T04
- **Cross-package prerequisites:** Feature 011 T07 (applied and persisted local display preferences).
- **Owner:** Desktop UI, Feature 011 shell.
- **Allowed paths:** `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-presentation.ts` (`resolveRichResponseContent` gains render policy), `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (read/render gate only), `apps/litecowork-ui/src/settings/local-preferences.ts`, `apps/litecowork-ui/src/settings/SettingsView.tsx`, `apps/litecowork-ui/src/App.tsx` (preference load/save/pass-through only), `apps/litecowork-ui/tests/local-preferences.test.mjs`, `apps/litecowork-ui/tests/rich-response-view-policy.test.ts`, `apps/litecowork-ui/tests/conversation-view.test.ts` (binding/dependency regression only), `apps/litecowork-ui/scripts/check-rendered-ui.mjs`, `scripts/audit_v1_contracts.py` (served-surface consistency guard), and directly affected UI trace/status documents. No API, event, schema, migration or provider contract is in scope. This test-scope addition is USER-DELEGATED implementation direction; it is not package acceptance.
- **Contract:** SPEC G-06; release contract section 4; AC-004, AC-018; Feature 011 `LocalPreferenceStore` and T07.
- **Change intent:** a local preference `richPresentationEnabled` (wire/docs concept `rich_presentation_enabled`, default true); when false, `resolveRichResponseContent` returns `primary: "SEMANTIC"`, automatic/manual reads are suppressed, existing in-flight results are discarded, and the complete semantic answer remains visible. The labelled checkbox switch is in Settings > General. This local display choice is distinct from per-turn `PresentationPreference` and does not change provider input.
- **Acceptance:**
  - [x] With the preference off, render policy selects the full semantic text and the Conversation read effects are gated before starting automatic/manual `get_rich_presentation` calls (AC-004, AC-018).
  - [x] The setting is keyboard-operable and persists on reload when local storage is available; old snapshots default on.
  - [ ] Packaged E2E-11 proves saved Conversation behavior, restart, late-read recovery and the exact supported Windows/Linux desktop builds.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** `local-preferences.test.mjs`, `rich-response-view-policy.test.ts`, `conversation-view.test.ts`'s exact cache-effect dependency regression, and rendered `conversation` browser-group output recorded in CURRENT-RUN, including saved-Conversation no-read behavior. These establish CODE/UI only; packaged restart and E2E-11 evidence belong to T16.
- **Stop:** do not move the preference outside Feature 011, add it to provider request metadata, or claim packaged persistence/provider integration from local-storage tests.

## 016-T10 - Generated media as provenance Artifacts

- **Status:** NOT STARTED - no ingestion pipeline; no Artifact create route today.
- **Depends on:** T02
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
- **Depends on:** T02
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

- **Status:** PARTIAL - `presentation-runtime-fallback.test.mjs` covers escaping and unsupported kinds for `PresentationRuntime.tsx`; on 2026-11-26 its Node 24.21.0 static SSR suite passed **7/7** after moving to Vite's exported module-runner transport with a test-local 120-second timeout and early cleanup. No static gate for the presentation directory; CSP is not tested.
- **Depends on:** T02, T06, T07
- **Cross-package prerequisites:** Feature 019 CSP audit of `tauri.conf.json`.
- **Owner:** Security, Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/tests/presentation-no-exec-gate.test.mjs` (new), `apps/litecowork-ui/tests/presentation-hostile-corpus.test.ts` (new), `apps/litecowork-ui/tests/presentation-runtime-fallback.test.mjs` (existing fallback coverage and its SSR runner), `crates/domain-presentation/tests/corpus/`.
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
- **Depends on:** T02, T08, T10
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
- **Depends on:** T03, T04, T06, T07
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
- **Depends on:** T02, T06-T13
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
- **Depends on:** T15, T16
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

- **Status:** PARTIAL (CODE) — the bounded parser, per-block degradation, shared TEXT_SLICE path, code label/copy/lazy token highlighting, focused tests and rendered Chrome checks are implemented. The real native-agent answer corpus required by AC-019 is still absent; authored test vectors are not provider evidence. USER-DELEGATED implementation direction is recorded 2026-11-24 in A-84. Feature 016 remains IN_REVIEW and T01 remains the formal whole-scope owner-acceptance gate. This renderer consumes committed semantic Markdown and adds no schema, route, persistence, provider ingress or authority. It does not claim package acceptance, provider integration or qualification. Folds proposed task P-03 from docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md section 9 (findings R-05, R-06) into this package.
- **Depends on:** Feature 001 committed AGENT message text only. T06 chart rendering is independent and not a prerequisite. This renderer's semantic-text input does not promote RichPresentation blocks or provider ingress.
- **Cross-package prerequisites:** Feature 001 committed AGENT message text is the input the renderer consumes.
- **Owner:** Presentation, Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/artifacts/StructuredTextPreview.tsx`, `apps/litecowork-ui/src/artifacts/markdown-document.ts`, `apps/litecowork-ui/src/artifacts/markdown-highlighter.ts`, `apps/litecowork-ui/src/artifacts/structured-text-preview.css`, `apps/litecowork-ui/src/presentation/RichResponseView.tsx`, `apps/litecowork-ui/src/presentation/rich-response.css`, `apps/litecowork-ui/tests/`, `apps/litecowork-ui/scripts/check-rendered-ui.mjs`, adjacent test fixtures, and owner/status/UI trace documentation needed to record evidence. The earlier `src/conversations/` paths did not match the active source tree; the canonical UI paths are corrected here for future authorized work.
- **Contract:** SPEC §14; AC-019; INTELLIGENT-UI-AND-IMAGE-CONTENT; AGENTS.md section 8.
- **Change intent:** replace the all-or-nothing Markdown parse with per-block degradation so GFM tables, horizontal rules and nested lists render without a whole-message raw fallback; give each code block a language label, a copy button and lazy highlighting that returns React nodes (never HTML strings); render TEXT_SLICE through the same Markdown renderer; and keep the no-executable-content posture (no `dangerouslySetInnerHTML`, https-only links). Add a golden corpus of real agent answers.
- **Acceptance:**
  - [ ] A sanitized corpus of actual native-agent answers renders with no whole-message raw fallback and no HTML strings (AC-019). Authored adversarial fixtures cover behavior but do not satisfy this provider-evidence criterion.
  - [x] Code blocks show a language label, a copy button and lazy highlighting returning React nodes (AC-019); unavailable/denied copy reports failure and preserves visible source.
  - [x] TEXT_SLICE renders through the same Markdown renderer; links stay HTTPS-only and no executable content is emitted.
- **Command:** `node --experimental-transform-types --test tests/markdown-document.test.ts tests/markdown-highlighter.test.ts tests/rich-presentation.test.ts`
- **Evidence:** 36/36 focused unit tests passed; in PowerShell, `$env:LITECOWORK_RENDERED_CHECK_GROUP='markdown'; pnpm run test:browser` passed the Chrome 158 rendered checks for GFM table, nested list, horizontal rule, lazy highlighting, hostile text, copy success/denial and no IPC/provider calls. The first browser run exposed a Windows temp-profile lock during cleanup after the checks; the harness now terminates its own process tree, and a serial rerun exited successfully. `pnpm run build` passed; see CURRENT-RUN. Actual native-agent corpus remains open.
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

A-84 + T25 user direction -> T02 canonical target-contract promotion -> T03 validator -> T04 corpus parity and T05 calculations -> T06 chart -> T07 calculator -> T08 publisher -> T09 preference. T10 media and T11 inert Artifact preview follow the exact Feature 007 owner paths. T12-T14 security/recovery/performance -> T15 traces -> T16 E2E-11 -> T17 documentation. T18 Markdown degradation is PARTIAL (CODE) and consumes committed semantic text without changing the RichPresentation schema. T01 formal whole-scope acceptance remains a package-lifecycle and release gate but does not block tasks explicitly authorized by A-84. T20 deterministic host fallback may proceed; provider ingress remains blocked on A-85 evidence. T21 Canvas can proceed without provider proposal support, then T22 Remember/Forget and T23 Saved View/ordinary-turn modification follow their distinct storage and Artifact owner paths. T16 plus T20-T23 -> T24 packaged qualification. T17/T18/T24/T01 -> T19 VERIFIED. T23 does not depend on T22; it requires Feature 007 create/version operations to be served and recoverable.

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

- **Status:** PARTIAL - the canonical target contract and deterministic host fallback are USER-DIRECTED under A-84. Provider proposal ingress remains BLOCKED on A-85's exact model-input/sequence/replay contract and pinned provider evidence; do not claim any provider supports it.
- **Depends on:** T02, T25
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
- **Stop:** Missing exact provider evidence, canonical contract promotion or correlation/replay semantics. Unsupported routes remain semantic-only; user-directed host fallback work may proceed without asserting provider support.

## 016-T21 - One controller for inline and contextual Workbench Canvas

- **Status:** NOT STARTED - inline-to-Canvas behavior is USER-DIRECTED under A-84; implementation may use the deterministic host presentation path and does not depend on provider proposal support. Feature 008 still owns the panel, tab and object reauthorization contracts.
- **Depends on:** T02, T04, T05, T07, T08
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
- **Stop:** A second controller, hidden agent call, implicit capture/process start, persistent write from the component or unqualified component is required.

## 016-T22 - Review and implement optional remembered answer inputs

- **Status:** NOT STARTED - A-84 records user direction to implement the complete proposal, including a separately owned, off-by-default Remember/Forget path. The exact persistence, quota, backup, encryption-at-rest and deletion/recovery contracts and migration must be specified in T02 before storage code.
- **Depends on:** T02, T21
- **Cross-package prerequisites:** Workspace/Conversation retention and deletion owners; Storage migration allocation; Feature 011 authenticated IPC.
- **Owner:** Storage, Presentation Runtime, Security, Privacy, Product.
- **Allowed paths:** owning storage core/sqlite; authenticated presentation Operator and Tauri bridge; presentation UI; canonical OpenAPI/event/state schemas; migration allocation; retention/backup docs and tests.
- **Contract:** SPEC §20; pipeline LLD; Feature 014/017 retention and backup; AC-025/026/040.
- **Change intent:** Under the user's delegated direction, resolve the full host-owned state design in T02: one per-device record keyed by exact Workspace/Conversation/message/presentation/semantic digest, closed numeric values tied to declared input IDs/digests, off by default, bounded quota and retention, explicit Remember/Forget, expected-revision CAS, idempotent receipts, deletion/revocation fences, restart recovery, backup/restore policy and a newly allocated migration if persistence requires DDL. Then implement through authenticated Operator/Tauri commands; never browser storage. Keep this state separate from Saved View ArtifactVersion and executable app data. Formal Storage/Privacy/Backup owner ratification remains a release gate.
- **Acceptance:**
  - [ ] Off by default; no agent exposure; replay/body-conflict, two-window conflict, timeout-after-commit, quota, forget/save/deletion race, restart/backup and revocation fail safely.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `future focused storage/Operator/UI test commands recorded before implementation; python .specify/scripts/validate_feature_package.py`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** the component itself gains persistence/submit authority, a database migration is used before allocation, a deletion/backup/recovery path can resurrect forgotten values, or input data can cross Workspace/provider boundaries.

## 016-T23 - Saved View lifecycle and explicit conversational modification

- **Status:** NOT STARTED - implementation is USER-DIRECTED by A-84; Feature 007 route/store integration depends on its owner tasks and promoted cross-layer contracts. Formal ratification and release qualification remain open. Conversation modification remains independent of T22 persistence.
- **Depends on:** T02, T21, T25
- **Cross-package prerequisites:** Feature 007 Artifact/Library create, immutable version/update, source-deletion and receipt contracts; Feature 001 ordinary new-turn Send, permission/continuation, idempotency, single-flight and recovery. There is no presentation-specific send route or hidden parent-lineage field; selected values are visible ordinary message content.
- **Owner:** Conversation, Presentation Runtime, Desktop UI, Security.
- **Allowed paths:** Feature 007 Artifact/Library owner paths included in the accepted task, Feature 001 context/provenance paths, presentation host chrome, canonical API/event/state schemas, UI ledgers and adjacent tests.
- **Contract:** SPEC §20, Feature 007 owner contract, Feature 001 send contract, AC-028/033-036/040/041.
- **Change intent:** Add capability-gated Save/Open/Update/Archive via Feature 007 using exact immutable ArtifactVersion, source lineage and durable receipts; create/update append a version and Archive is the only removal action. Default save preserves declared RichPresentation defaults. A separate unchecked-by-default user choice may snapshot up to eight current declared numeric inputs after the Feature 016 validator proves declaration identity/digest, finiteness, range and step. The component remains transient under B-40 and values are never automatically sent. Ask a follow-up opens an editable ordinary composer draft; selected inputs are included only as visible plain message text after exact preview and explicit user selection. The user must press Feature 001 Send; no hidden metadata, side-channel or component callback exists. Update from a new answer clears prior values unless compatible declarations are explicitly reviewed and saved again. No original message or ArtifactVersion is mutated.
- **Acceptance:**
  - [ ] Saved View create/open/update/archive follows Feature 007 receipt/version/recovery contract; no control appears without capability. No send on input change; cancel/retry does not mutate original; stale base prompts review; new message identity and host-assigned lineage.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `future focused Conversation/desktop fixture commands recorded before implementation`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** Feature 007 cannot represent this without violating Artifact invariants, owner receipt is missing, automatic input transmission, original message/version rewrite or inferred approval.

## 016-T24 - Revised V1 negative/recovery/accessibility and packaged evidence

- **Status:** NOT STARTED - security, recovery and accessibility work is USER-DIRECTED under A-84; packaged qualification cannot be claimed until whole-scope acceptance, owner ratification and exact platform evidence are recorded.
- **Depends on:** T01, T16, T20, T21, T22, T23
- **Cross-package prerequisites:** Feature 001 qualified provider route and Feature 019 release evidence; existing Windows/release deferrals resolved.
- **Owner:** Security, Accessibility, Release, Product, repository owner.
- **Allowed paths:** presentation/AgentModule/storage test fixtures; release evidence bundle; CURRENT-RUN.md; implementation status and Feature 016 contracts.
- **Contract:** SPEC §20; pipeline LLD; all accepted AC-001..AC-041, especially denial, Saved View, state and Canvas behavior; canonical E2E ID and steps assigned by T02.
- **Change intent:** Exercise E2E-11 fallback plus the rich-answer-to-Canvas continuity journey proposed in SPEC §20.11 (candidate E2E-18; T02 verifies/assigns the global ID). Exercise semantic source-to-render, provider fallback/ingress, scope fencing, misleading metrics, unknown component/kernel, controller identity, consent, quotas, receipts, timeout/crash/restart, forget/save/archive race, revocation, stale version, scroll/focus recovery and explicit new-message modification on exact packaged release rows. Implemented owner paths may be exercised as CODE/SYSTEM before ratification; USER qualification and release claims still require exact owner sign-off. Saved View create/open/update/archive depends on the Feature 007 served owner path; remembered-state steps remain separate under B-46/T22.
- **Acceptance:**
  - [ ] Record build/OS/WebView/provider/model/version/auth/ingress, fixture counts, SYSTEM captures and owner USER review; no V1 qualification claim until every accepted AC has exact evidence.
  - [ ] Canonical owners, machine contracts and UI trace updated together; evidence class limited to actual checks.
- **Command:** `python .specify/scripts/validate_feature_package.py; exact future packaged E2E command supplied by release owner`
- **Evidence:** exact commands/results, revision and boundary in Completion evidence; design-only checks do not close implementation tasks.
- **Stop:** Any authority leak, content resurrection, stale overwrite, hidden native/model dispatch or missing owner evidence.

## 016-T25 - Resolve Saved View cross-owner design boundary

- **Status:** PARTIAL - product owner direction recorded 2026-11-17 to keep Saved Views in V1 and amend Feature 007 B-21; A-84 records user-delegated implementation direction for an optional explicit numeric snapshot in immutable ArtifactVersion content. The bounded Artifact contract is drafted in `specs/007-professional-artifacts/contracts/saved-view.md`. Cross-owner ratification and canonical API/schema promotion remain open. This task records design and lineage; it cannot mark Feature 007 or 016 accepted.
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 007 Artifact/Library owner review; Storage, Conversation, Workspace deletion, Backup, Privacy and Product review of provenance, exact-version reopen/update, source deletion, declared-default snapshot, receipts, retention and recovery.
- **Owner:** Artifact, Product, Presentation Runtime, Security, Storage, Privacy and Conversation; decision by the repository owner.
  - **Allowed paths:** `specs/016-rich-presentation-intelligent-ui/`, `specs/007-professional-artifacts/`, `docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/ACP-PRESENTATION-PIPELINE-LLD.md`, `docs/ARTIFACTS-EVIDENCE.md`, `docs/SCHEMAS.md`, `docs/schemas/saved-view-artifact.schema.json`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/V1-PRODUCT-RELEASE-CONTRACT.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/SPEC-TASK-TRACEABILITY.md`, `docs/V1-FEATURE-SDD-REGISTER.md`, `docs/V1-PRODUCT-FEATURE-COVERAGE.md`, `docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md`, `CURRENT-RUN.md`.
- **Contract:** SPEC §20.6/§20.9 and AC-028/033-036; Feature 007 Artifact/Library contract; SPECIFICATION-STANDARD §7; A-84/A-85/A-103.
  - **Change intent:** record the product owner's selection to keep Saved Views in V1 and amend B-21, then specify a bounded, non-executable Artifact snapshot without weakening other Artifact format rules. Feature 007 revision 1.1 remains in review: Markdown remains the first professional document format; one exact Saved View media type is proposed as a separate Artifact kind, backed by the canonical RichPresentation plus semantic fallback and exact Conversation/message/presentation lineage. By default, declared values are retained. Under A-84, a distinct explicit Save choice may include at most eight current numeric values after exact declared-input identity/digest, finite, range and step validation. This is an immutable ArtifactVersion content action, not component persistence, device-local Remember/Forget, ordinary-turn input sharing or agent input. The contract uses ArtifactVersion/ResourceRevision and existing create/version receipts/events; it adds no SavedView table, event, side store, hard-delete route, arbitrary executable content or permanent navigation destination. Archive is the V1 removal action. Source deletion, replay, version conflict, Workspace fencing, backup, retention and restart recovery are specified in `specs/007-professional-artifacts/contracts/saved-view.md`. Do not disguise structured JSON as Markdown or treat it as `INTERACTIVE_APP`. A-84 authorizes scoped implementation; this does not accept Feature 016 as a whole, complete T01/T25, or establish Feature 007 cross-owner ratification. T02 promotes the canonical contract before serving; T22 remembered-input persistence and T20 provider ingress remain separate, and A-85 is not implied by T25.
- **Acceptance:**
    - [x] A dated product-owner direction records “Keep Saved Views in V1; amend Feature 007 B-21” in the acceptance packet and both feature packages; this records the scope choice, not approval of unreviewed technical riders.
    - [ ] Artifact, Storage, Conversation, Privacy, Backup and Product owners review the Saved View contract; promoted API/schema, dependency map and implementation task scopes are consistent before Feature 016 T01 review.
    - [ ] A-103 and all V1/UI traceability records state the decision and its remaining gates; no permanent navigation destination is added.
    - [x] The direction is explicitly separated from formal package acceptance, cross-owner ratification and qualification; implementation authority comes from A-84 and each active task scope, not from this task alone.
- **Command:** `python .specify/scripts/validate_feature_package.py`
- **Evidence:** owner decision record and validator output.
  - **Stop:** cross-owner review or canonical contract promotion is absent, source/deletion/receipt/version behavior drifts from the Saved View contract, or this decision is mistaken for T01 whole-feature acceptance.
