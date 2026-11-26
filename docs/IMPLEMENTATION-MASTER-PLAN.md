# LiteCowork implementation master plan

**Authority:** This plan orders delivery. Root ARCHITECTURE.md, PRODUCT.md, V1-PRODUCT-RELEASE-CONTRACT.md, domain owner contracts, accepted ADRs and docs/schemas remain normative. Current source evidence is maintained in IMPLEMENTATION-STATUS.md.

## 1. Delivery rule

Build complete vertical user journeys, not isolated capability counts. A design, schema, type, mock, unit test, catalogue item, mounted route or visible screen is not a qualified user journey.

Each delivery slice identifies the user outcome, owning domains, request/response/event schemas, state preconditions, authorization, transaction owner, durable event/receipt, projection, UI component, error/recovery behavior, tests and exact OS/provider qualification.

The active implementation is in apps/ and crates/. Do not start by recovering the old archive or creating duplicate source trees. Inspect the current revision/worktree, preserve ignored/local material, and make a narrowly scoped branch/worktree choice for each implementation slice.

## 2. Product sequence

The sequence below is dependency-aware, not a claim that work is absent or already complete. Current implementation state for each step is in IMPLEMENTATION-STATUS.md.

### P0 — Evidence and release baseline

- Keep TARGET, CODE, SERVED, INTEGRATED and QUALIFIED separate.
- Maintain schemas/served-route-inventory.json from the composed production router.
- Preserve the six passing Windows WorkspaceRoot route regressions; separately qualify Windows folder scanning, which remains unsupported/not implemented.
- Run architecture/schema, documentation, frontend, daemon, packaged desktop and owner acceptance gates.
- Select the exact GA OS/build and first native agent/version/auth route.
- Reconcile the 2026-11-23 superset design package against current owners and code using [the dated disposition](audit/SUPERSET-DESIGN-DISPOSITION-2026-11-23.md); do not carry forward its stale baseline, duplicate task IDs, unverified provider claims or migration numbers.

**Current evidence:** the latest completed full desktop UI run (2026-11-25) is red at **627 passed / 22 failed / 5 skipped (654 total)** on Windows, using Node 22.23.1 and pnpm 12.10.1. A prior run recorded **641/23/5 (669 total)**; the 15-test inventory discrepancy remains unreconciled, so the totals are not a trend. All eleven previously timed-out Vite SSR files pass focused runs on pinned Node 24.21.0 in isolation (**81 assertions passed**), including quota observation, profile history and Needs You after scoped test-runner timeout/cleanup updates. A subsequent 2026-11-26 serial full-suite attempt was interrupted before its final TAP summary; the retained log had a **674 passed / 10 failed / 5 skipped checkpoint**, not a completed suite result. Its remaining failures include one CSS fetch timeout, mounted Needs You and Resource cases, indexed search, narrow navigation and renderer-v2; DevTools startup caused five dependent skips. Resolve and classify those failures, then finish a full run and reconcile the differing test inventories. Exact commands, names, timings and limitations are in CURRENT-RUN.md and IMPLEMENTATION-STATUS.md.

**Exit:** reproducible green baseline and explicit release matrix. The current baseline is not green; see IMPLEMENTATION-STATUS.md.

### P1 — Durable native Conversation

- Close the first-click New Chat split before qualification: one Send must atomically create the Conversation, first USER message, OPEN turn, events, exact receipt and durable dispatch intent; preserve the exact payload and selected binding through ambiguous response/restart. Feature 001 T29 is user-delegated implementation work, not external domain-owner ratification; T09 qualifies the packaged provider journey only after this slice.
- Close the existing-Conversation lost-acknowledgement gap too: journal each exact request before dispatch, replay its original key/body after ambiguity/restart, return an existing receipt before mutable Resource checks, and display only committed transcript messages. Feature 001 T30 is DONE for its declared CODE/SERVED scope under USER-DELEGATED implementation direction; revision 1.2 remains IN_REVIEW and the change adds lifecycle completeness without narrowing the V1 product. T09 still owns packaged provider qualification.
- Gate fresh New Chat first-send, existing-Conversation Send and Retry on the authenticated daemon's exact current Runtime/incarnation being ONLINE/READY inside the same SQLite admission transaction, after exact replay and read-only validation. Feature 001 T32 is DONE for this operational CODE/SERVED scope under USER-DELEGATED A-119; UI lifecycle state is preflight only, exact replay and Stop/cancel/read paths retain their existing rules, and Feature 001 revision 1.2 remains IN_REVIEW. This does not qualify a native provider or package; T09 still owns real OpenCode auth, permission/continuation evidence where supported, packaged lifecycle/recovery and Windows/Linux USER evidence.
- The durable dispatch intent/outbox is already committed with turn admission and replay is idempotent (T10, CODE). Startup recovery is wired and settles prior-incarnation unsettled work without blindly resending a prompt (T08, CODE). Exact same-incarnation claim acknowledgement reconciliation is now implemented under USER-DELEGATED A-116 as T20 CODE: a point read of the exact attempt distinguishes local ownership, another owner, settled work, retryable uncertainty and malformed/missing state. The full commit/claim/dispatch crash matrix and packaged recovery evidence remain (T20/T09); do not describe the outbox itself as absent.
- Keep admission replay separate from execution evidence: an exact replay returns the original admission receipt, while the durable claim/settlement state and recovered projection show execution progress. The claim-only retry now also has a coordinator-to-SQLite fault-injection regression (CODE) proving it waits for storage recovery and opens no AgentSession before ownership is established. It still does not exercise a real provider prompt; verify no duplicate prompt in the real route and qualify the packaged path (T20/T09).
- Qualify the OpenCode v2.0.32 route end-to-end through packaged desktop, local IPC, daemon, provider, persistence and reconnect.
- Preserve native auth/private session semantics; add other AgentModules independently and only when their exact adapter/OS/auth matrix is ready.
- Keep unsupported bindings non-sendable with preserved drafts and honest repair copy.

- Enforce canonical AgentHostInstance shutdown order on every orderly close path: persist STOPPING after CLOSED/LOST and before native process shutdown, then persist STOPPED only on observed exit. The user-delegated T03 correction is CODE-only. A-104 host process identity and prior-incarnation recovery are wired before Operator admission; native-Windows helper tests cover fail-closed identity handling, but not SQLite-backed startup reconciliation, live process termination, Linux process-group cleanup or packaged crash/restart. StopUnconfirmed retains the exact process handle and AgentSession identity in the Runtime-local host owner. A-124 CODE now validates the exact OpenCode prompt receipt and performs a bounded same-incarnation durable-log reconciliation before that handle transfers; the recovered outcome uses the shared production settlement router, with a focused SQLite test proving one exact Stop settlement and the unresolved no-write path. Malformed, missing, shutdown, over-budget or late evidence remains unresolved. These tests do not prove one authenticated provider observation through transport, reducer, storage and retained-host retirement, restart recovery or qualification. Reusable host leases and those integration/qualification gates remain open.
- **Next Conversation gate — T09/T23 provider lifecycle:** the T03/T04 SQLite settlement seam is now covered at CODE level. Next, qualify the pinned OpenCode v2.0.32 path through packaged desktop, local IPC, daemon, authenticated provider, persistence, Stop/retry/restart and recovery on the declared Windows/Linux rows. Do not treat helper tests as provider or USER evidence; keep T03/T04 PARTIAL and T09/T23 open until the packaged journey and recovery evidence pass.

**Exit:** send, stream/resync, stop, retry, restart and crash-after-commit tests on the declared OS/provider build with no duplicate prompt or stranded OPEN turn.

### P2 — Workspace roots and context

- Complete the WorkspaceRoot lifecycle and platform path contract.
- Qualify root select, pause, resume, rescan, revoke, resource invalidation and safe deletion.
- Pin exact Resource revisions and digests; validate same-Workspace access at use time.
- Finish bounded ingestion/index/retrieval for a first explicit format set; preserve source provenance in Context Used.
- Treat ZIP preview/readiness as separate from extraction, child-Resource publication and Task context.

**Exit:** owner attaches a selected root/resource, receives grounded retrieval with exact source references, revokes access and proves future retrieval is fenced.

### P3 — Native Agent completeness and relay

- Complete the AgentModule contract for install observation, auth state, descriptor freshness, native options, session admission and turn support.
- Qualify a second native agent route only after provider terms/auth/OS/version checks.
- Implement A→B→A over one human-facing Conversation using committed messages and explicitly selected Resources/Artifacts, not private transcript/session migration.
- Preserve per-turn binding, model/mode/effort, provenance and failure state.

**Exit:** multi-agent continuity test with exact producing agent and no hidden context or credential transfer.

### P4 — One safe Task vertical slice

- Explicitly promote an outcome to a Task.
- Produce and accept an immutable PlanRevision with pinned Steps and criteria.
- Admit one Attempt with fenced lease, eligible agent and exact Environment.
- Wire Trust evaluation into the actual execution boundary.
- Execute one bounded file mutation through the Core Gateway with idempotency, Effect and durable receipt.
- Reconcile timeout-after-effect, cancellation, restart and unknown results; independently verify the result.

**Exit:** one user-approved file edit in a qualified isolated environment, survives restart/cancel/ambiguous receipt and has independent evidence. A working directory or git worktree alone is not a sandbox.

**Current gate:** Feature 005 revision 1.1 records the owner-directed Environment lifecycle, immutable ResourceRevision proposal, mediated dispatch and Trust-owned grant decisions (A-70/A-87/A-88/A-90); revision 1.1 remains IN_REVIEW pending cross-owner review. T01 remains IN_PROGRESS pending cross-owner review. T02 is DONE (CODE) for type/probe consolidation only; Linux live enforcement qualification belongs to T04/T15. T03 is DONE (CODE) for initial-plan SQLite acceptance, with stale-version/spec/producer denial, exact/conflicting replay, concurrency and rollback coverage. T04 has lower-level CODE for a capability-dropped Bubblewrap command plan, trusted candidate runner, fixed-path reader and bounded frame parser; the WSL suite passed 36 library and 4 runner tests. Two opt-in direct Bubblewrap tests passed 1/1 each: the built runner as PID 1 blocked frame forgery and waited for a delayed descendant; a second probe runs the production Bubblewrap command builder directly, reads its fixed 1 MiB `/workspace` cap, writes through output and the in-sandbox `/tmp` symlink on the same quota, and observes `ENOSPC`; it exposed and fixed an invalid host-side bind. Both bypass `ManagedScope` and systemd/cgroup; the second does not validate EnvironmentConfig mapping. Seven pre-existing live Bubblewrap/systemd checks remain ignored by default. The staging crate passed 38 tests. A daemon-private launch mapper passed 5 focused tests for Attempt-private/current-incarnation identity, exact input pins and supported limits. A private capture session now connects the mapped plan to Bubblewrap, preserves native agent stdin/stdout, drains the separate candidate channel and withholds bytes until recursive quiescence plus launcher reap. Three fake-scope tests pass, including terminal denial after nonzero exit is retried and retry after a quiescence timeout. The helper has no production Task Runtime caller and does not attest, admit an Attempt or persist a Resource; managed native Linux qualification remains open. T06 has CODE for the B-18 bounded file-replacement primitive on native Windows and WSL Linux; the native-Windows suite now includes a missing optional replacement-handle denial before staging and the crash-before-rename recovery shape. Linux POSIX ACL preservation remains unverified because available WSL filesystems reject ACL xattrs. T07 is DONE (CODE) for the deterministic-local Core replacement policy and Trust-only durable Attempt grant storage foundation; the native-Windows focused regression now also rejects a candidate with absent or foreign `provenance.created_by_attempt` (4/4 focused tests). T08 is PARTIAL (CODE) for closed FileReplacementTarget validation and dispatch-time exact Core grant-scope revalidation in the atomic SQLite store (14/14 focused dispatch tests). No Task proposal producer, Attempt admission caller, production dispatch route, filesystem call, receipt/reconciliation pipeline or Work journey is integrated. Plan submit and Attempt routes remain target-only under T10. Continue from the dependency graph in Feature 005 TASKS and current evidence in CURRENT-RUN.md; preserve the Linux-first row and keep Windows Task execution unavailable until B-35 qualifies.
**Conversation promotion gate:** Feature 001-T31 and Feature 005-T12 now implement the read-only completed-turn lineage projection and explicit Conversation-to-Task path as CODE/SERVED work. A-114/A-115 are user-delegated implementation directions; Feature 001 revision 1.2 and Feature 005 revision 1.1 remain IN_REVIEW. Promotion creates only a READY Task and references already committed messages; it does not mutate Conversation state or start planning. The native-Windows storage regression now covers the source-lineage denial matrix, but the exact request cache is memory-only, so rendered packaged UI, restart retry recovery, screenshot evidence and packaged denial/race/crash-recovery E2E still gate completion. This does not reduce or replace the safe Task execution scope below.

**2026-11-19 architecture cross-check:** the existing Linux capture helper launches the native harness and cannot attest before Attempt reservation. The target requires a private write-ahead preparation journal and trusted idle supervisor that probes and holds the exact scope before admission, then launches the harness only after the admission receipt resolves. CODE now includes a held-runner launch-order primitive and a separate durable `RuntimeLocalPreparationJournal`; native-Windows tests pass 6/6 for bounded no-follow/canonical journal reads, transition/recovery rules and cross-process phase serialization. Neither primitive is called by a Task Runtime. Same-scope probes, receipt query, attestation, admission and startup reconciliation remain absent. The complete scope-continuity/recovery path remains unimplemented and IN_REVIEW; T01 is IN_PROGRESS and T04 PARTIAL.

**2026-11-25 T04 source update:** the trusted held runner performs filesystem quota/read-only observation in the exact Bubblewrap scope before it reports readiness. The launch builder rounds the configured scratch maximum down to page size and passes the same effective cap to the mount and runner; `fstatfs`, same-device writable aliases, root/input `EROFS`, and bounded create/unlink checks fail closed. Direct WSL tests cover the held gate and a separate small-cap `ENOSPC` check. This corrects the earlier live-full-cap exhaustion requirement because consuming production tmpfs pages could exhaust the Attempt's memory budget. `ManagedScope`/`HeldEnvironmentScope` expose CODE-only process-start/cgroup identity capture and recheck. The snapshot binds the pinned PID's boot/start identity, membership, cgroup inode/population, executable inode, UID and exact memory/pids/cpu limits, then rechecks those mutable values. No provider/Task Runtime caller records or rechecks that identity before admission, and no attestation is produced. T04 remains PARTIAL (CODE); namespace/network probes, receipt lookup, journal wiring/recovery, real managed Linux evidence and packaged qualification remain. T05 stays blocked by T03/T04 and provider/session prerequisites.
**2026-11-25 verification correction:** a full post-identity `linux-process-scope` test run and the daemon's filtered `linux_environment_provider` tests are recorded in CURRENT-RUN. These checks are WSL CODE evidence only and do not replace managed systemd/cgroup behavior, native Linux USER evidence or any packaged release journey. Do not reuse the earlier 45-test count from the quota-probe checkpoint as the current count.

### P5 — LiteSPM and a golden Connection

- Pin LiteSPM service version, bridge schema, authentication and host compatibility.
- Use one LiteSPM package inventory/configuration writer; do not add a duplicate package manager.
- Implement exact plan preview, digest-bound approval, apply, durable receipt, rollback and UNKNOWN reconciliation.
- Keep install, native Connection auth, Coworker assignment, Task grant, activation and invocation separate.
- Qualify one first-party Connection: a bounded read first, then one approved write with idempotency and reconciliation.

**Exit:** a package lifecycle operation and one account operation each complete with correct scope, receipt and revocation behavior.

### P6 — Professional Artifact

- Qualify Artifact identity, immutable versions, provenance and Workspace scope.
- Deliver one genuinely editable professional format with round-trip, save, export, reopen and validation.
- Compare exact revisions; preserve conflict and recovery behavior.
- Treat interactive applications as a separate isolated Runtime, not as rich chat markup.

**Exit:** user creates/revises/exports/reopens one professional deliverable and verifies source provenance and version differences.

### P7 — Real Workbench providers

- Keep the default right Workbench visible in compact Start state.
- Replace simulated terminal with daemon-owned PTY/ConPTY sessions, process identity, bounded output/backpressure, detach/reattach and stop/recovery.
- Replace iframe-only Browser label with a truthful preview until a real isolated browser provider exists.
- Qualify real Browser profile/session, site grants, epochs, takeover/return and stale-handle revocation.
- Add Computer use only with exact OS permission, display/session identity and human takeover.
- Compute diffs from real VCS or exact revision pairs; remove demo/sample success states.

**Exit:** real terminal/browser work from the UI with audit/effect semantics, safe takeover and crash/restart proof.

### P8 — Coworkers, learning and responsibilities

- Keep Coworker optional and distinct from Conversation, Task and a resident process.
- Qualify Coworker-owned Conversation admission and exact Task origin provenance.
- Complete scoped Memory/Knowledge source lifecycle, policy, provenance, correction, revocation and deletion.
- Implement responsibilities only as proposals/admitted routines or Automations under their owners.
- Ensure worker assignment and connection state never confer grants.

**Exit:** create optional Coworker, chat, attach/revoke scoped context, restart, and prove no cross-Workspace leakage or hidden standing authority.

### P9 — Automations and notifications

- Preserve the current local TriggerHost as scheduler/materializer, not a Task executor.
- Qualify due slot dedupe, cursor/occurrence atomicity, host restart, schedule revision, timezone/DST, misfire and overlap.
- Only claim a Task is executing after Task admission and worker dispatch actually occur.
- Qualify notification eligibility, delivery provider, dedupe and ambiguous delivery separately.
- Local recurrence requires awake/available Runtime; cloud/laptop-off continuation is post-V1.
- Do not add a native WorkflowGraph schema, canvas, graph-run engine or generic Code-node runtime to Core. Automation/Routine remains the reusable schedule-to-Task path.
- External workflow invocation is an optional future provider integration, not a prerequisite for Automations V1. The current WorkflowBackendAdapter is target-only and no production implementation/route exists in apps/ or crates/.
- Where a user connects a supported external provider, the user may use the provider's own visual editor and, when available/enabled, the provider-native AI builder/MCP. These create/edit provider-owned graphs; they are not LiteCowork Coworker authoring features.
- LiteCowork Coworkers may help author a Routine and map inputs. Direct LiteCowork-agent graph mutation requires a separately approved provider-specific authoring capability with distinct read/draft/test/publish/run/delete scopes, provider version/digest, owner confirmation and ambiguous-result recovery. This is deferred and not V1 authorization.

**Exit:** one occurrence produces exactly one Task with accurate history; subsequent Task execution and notification claims require their own proof. Optional external workflow integration has a separate exit only after Feature 005/006, provider adapter, Trust/Effect/reconciliation and exact provider USER E2E qualify.

### P10 — Intelligent UI and media

- Preserve each committed semantic ConversationMessage as the authoritative answer; present rich output as a validated, source-bound companion.
- Use one canonical, versioned RichPresentation contract and a closed host-registered component set. Native providers may supply structured intent only through a qualified capability; ACP itself does not guarantee a presentation tree or subagent stream. Unsupported or invalid intent falls back to complete semantic content.
- Keep the primary experience conversation-first. Render compact answers inline, then open the same view and controller in a contextual Workbench Canvas without adding permanent sidebar tabs. Keep diagnostics and provider traces out of the default answer surface.
- Use source-bound metrics, deterministic host calculations and accessible chart/table fallbacks. The component remains local and non-submitting under B-40. Under A-84, a separate explicit Saved View Artifact action may opt in to a bounded numeric snapshot, while Ask a follow-up may place selected values only in visible ordinary composer text after exact preview and user Send; no hidden context channel is used. Device-local Remember/Forget remains its own consented storage/recovery path under T22.
- The owner decided on 2026-11-17 to keep Saved Views in V1 and amend Feature 007 B-21. Feature 007 revision 1.1 now has a detailed TARGET contract: one non-executable Artifact kind/media type, exact source lineage and semantic fallback, optional explicit numeric input snapshot inside immutable ArtifactVersion content, immutable update from a new committed presentation, standard Artifact receipts/events, archive-only removal, source-deletion and recovery behavior. Cross-owner review and canonical API/schema promotion remain open. Saving a snapshot is not component persistence; no event, receipt, agent request or separate state table carries input values.
- When generated media is supported, publish it as an explicit provenance-bearing Artifact through Feature 007's owner path; do not treat a media preview, model output or temporary presentation block as a committed deliverable.
- Keep arbitrary HTML/JS, eval, script imports and host API calls out of chat WebView. Do not add a second UI schema, hosted BYOK router, implicit provider substitution or general-purpose generated-code runtime.
- Qualify inline rendering, Canvas continuity, persistence/reopen, denial, race, crash/restart and supported provider/OS routes as one packaged journey. Keep the isolated Interactive Artifact Runtime separately PREVIEW_ONLY until its own gate qualifies.

**2026-11-25 prior implementation delta (superseded):** Feature 016-T03 added Rust CODE for canonical v1/v2 document validation, exact source-bound metric/chart data and SAFE_CALCULATOR initial-result evaluation. The date reflects the earlier host-local handoff snapshot. The current client-date update below records the later work in this session.

**2026-11-24 implementation delta:** Feature 016-T04 now has CODE for a shared Rust/TypeScript golden corpus and matching renderer-v1/v2 contract validation, including source-bound METRIC/CHART, initial SAFE_CALCULATOR value/result matching, exact boundary limits, unsafe integer-value rejection in integer and exponent forms, and negative cases. TypeScript snapshots own data descriptors before validation; transparent Proxy detection is not a browser capability, while throwing reflection traps fail closed. V2 passes contract validation but the response parser keeps semantic fallback until UI renderers ship. The focused Rust and TypeScript suites and production build passed. The broad UI test command hit an unrelated Vite transport timeout and was interrupted; the latest full completed UI suite remains red. See CURRENT-RUN for exact commands and limits.

**2026-11-24 implementation delta (016-T05):** Rust CalcAst v1 now has a focused structural rejection matrix and exact node/depth boundary coverage. The desktop has one iterative, bounded TypeScript `evaluateCalc` reducer with typed errors, a one-time own-data-descriptor AST snapshot, finite Map input snapshot, and seeded property coverage; the TypeScript SAFE_CALCULATOR initial-result validator reuses it instead of retaining a second evaluator. Focused Rust passed 15/15, desktop reducer/parser/corpus passed 34/34, and `pnpm build` passed with the existing >500 KiB main-chunk warning. This is CODE only; no calculator component, v2 renderer, production publisher, provider ingress or qualification is established. The full UI suite remains red/incomplete as recorded in CURRENT-RUN.

**2026-11-24 implementation delta (016-T06 semantic-context extension):** renderer-v2 METRIC and CHART have CODE/UI in `RichResponseView`. The canonical schema, Rust validator, TypeScript mirror and shared corpus now require a closed bounded `unit`/`scale`/`time_interval`/`aggregation` context, exact source-derived measure labels and an exact committed semantic line grounding source labels and declarations. The UI displays the unchanged raw value with that context. It does not convert or aggregate data, fact-check provider-authored semantics, or implement the derived-metric fixtures owned by T20/T24. CHART uses host-generated static SVG and an exact keyboard-toggle source table. Focused Rust domain tests passed 15/15, focused SQLite RichPresentation tests passed 28/28 (615 filtered), the model/parser/corpus suite passed 36/36, TypeScript and the Chrome rendered group passed, and the production build passed. The main Vite bundle is 1,166.12 kB and still emits its >500 kB warning. At the T06 checkpoint, no production publisher/provider ingress existed; the later T08 table-only binder does not make v2 provider intents reachable. These are CODE/UI checks only; at that 2026-11-24 checkpoint, the full UI suite was red at 658 passed / 9 failed / 5 skipped; this historical result is superseded by the 2026-11-25 full run recorded in §2 above.

**2026-11-24 implementation delta (016-T07 SAFE_CALCULATOR):** the desktop parser now returns an immutable render model only for a fully validated, source-bound calculator. The component renders labelled finite/range/step-bounded numeric fields, inert AST-derived formula text, synchronous local recomputation, a polite result announcement, keyboard reset and always-visible semantic fallback. Empty, invalid, off-step, divide-by-zero and non-finite states show a generic bounded error; no value is clamped, persisted, submitted or sent through a host/provider call. Focused parser/state/reducer tests passed 38/38, the rendered Chrome presentation group passed with zero serious/critical axe findings and zero host calls, TypeScript passed, and `pnpm run build` passed API generation, TypeScript and Vite. Vite reports the existing >500 KiB main chunk at 1,170.75 kB. These are T07 CODE/UI checks only; the later T08 binder emits TABLE/TEXT_SLICE but no calculator intent, and no packaged journey, provider integration or qualification exists. The full UI suite remains red/incomplete as recorded in CURRENT-RUN.

**2026-11-24 implementation result (016-T08):** the daemon now calls the publisher after the committed semantic response and emits only exact GFM TABLE blocks plus UTF-8-bound TEXT_SLICE spans from that message. It does not infer chart type, units, aggregation, timeframe or a calculator from number-shaped tables or prose. CHART and SAFE_CALCULATOR publication remain in the full V1 scope and require canonical validated ChartIntent/CalcIntent through T20. Publish identity is deterministic from the exact committed Workspace/message; the immutable row and event act as the replay receipt, so no new migration or parallel dedup store was introduced. Binder failures increment a content-free in-process counter and are swallowed after settlement; durable telemetry, Conversation-plus-presentation integration fault coverage, provider ingress and packaged qualification remain open.

**2026-11-25 implementation result (011-T07 / 016-T09):** the mounted Settings > General screen now owns a local `Show formatted answers` preference in the existing `litecowork.local-preferences.v1` snapshot. It defaults on for older snapshots, selects the complete semantic response when off, prevents new automatic/manual RichPresentation reads, and ignores a response that was already in flight when the user disabled the feature. It does not modify the turn-captured presentation preference, send body, provider input or domain state. A failed localStorage write is disclosed as session-only. Focused preference/policy tests passed 12/12, the TypeScript check passed, and the rendered Conversation browser group passed **17 checks** on Chrome 158.0.8127.38. The rendered journey reopened a saved Conversation with formatted answers off, confirmed the full semantic reply remained visible and observed zero `get_rich_presentation` calls; keyboard, same-storage reload and simulated storage-failure feedback also passed. These are CODE/UI checks; packaged restart and E2E-11 remain open. Feature 011/016 remain at their existing lifecycle status; no provider integration or release qualification is claimed.

**Current gate:** Feature 016 revision 0.4 remains TARGET/IN_REVIEW. T25 records the Saved View direction and draft, with cross-owner ratification still open. A-84 directs implementation of the full current proposal without reducing capability scope while preserving formal owner lifecycle and evidence boundaries. T02 has promoted the scoped metric-context contract; T03/T04/T05 are DONE at their scoped CODE levels; T06 is DONE for METRIC/CHART component CODE/UI; T07 is DONE for SAFE_CALCULATOR CODE/UI with focused parser, bounded-input, local-state, zero-host-call and serious/critical axe evidence. T20/T24 still own derived metric calculations and their reordered/missing-period, scale and denominator fixtures. Production publication/provider ingress, Canvas continuity, Saved View lifecycle and packaged evidence remain incomplete; therefore no renderer-v2 provider journey is integrated or qualified. T01 formal whole-scope acceptance remains a separate lifecycle and release gate. T18 is PARTIAL (CODE): bounded display-only Markdown rendering, shared TEXT_SLICE rendering, local copy and lazy highlighting have focused unit and rendered Chrome evidence; the required corpus of actual native-agent answers remains open. Device-local Remember/Forget still needs its durable owner contract. Ask-a-follow-up uses only the ordinary composer/Send path with visible user-reviewed text. Saved View routes and controls remain unavailable until Feature 007 serves their owner operations.

**Exit:** packaged user can obtain a complete semantic answer, safely inspect and interact with a validated companion view, and expand it without losing state during the active view. A Saved View reopens its exact committed presentation with declared defaults or a separately opted-in, validated numeric snapshot; saving changed values appends an immutable ArtifactVersion with its own receipt and recovery path. Device-local remembered inputs and ordinary-turn state sharing require their own contracts and receipts. Formal owner acceptance, cross-owner ratification and exact release-row evidence remain required before V1 release. Denial and recovery remain truthful, and malformed content cannot mint host state or authority.

### P11 — Platform release and portability

- Complete backup/export/import/restore, deletion and revocation UX, diagnostics, accessibility, performance and crash/upgrade/rollback testing.
- Run exact OS/provider/auth conformance for every marketed feature.
- WSL2 is not evidence for native Windows or native Linux desktop behavior.
- Unsupported combinations remain explicitly unavailable.
- Cloud Runtime, laptop-off continuation, remote/mobile control and multi-user organization are post-V1.

**Exit:** declared local-desktop release matrix is green with CODE/SYSTEM/USER evidence and owner approval.

## 3. LiteSPM integration lane

The LiteSPM lane may proceed alongside independent Conversation and Resource work, but cannot substitute for it:

1. Re-read pinned LiteSPM source/version, current bridge schema and conformance tests.
2. Freeze protocol version, peer identity, service lifecycle and typed IPC.
3. Implement/review the client against generated/published contracts, not guessed CLI output.
4. Add one qualified host compatibility profile and inventory observation.
5. Add consent-controlled plan/apply/reconcile for one package kind/host.
6. Preserve LiteSPM-owned package state; Core owns assignment, grants, invocation and mediated effects.
7. Add repair/update/unmanage/remove only after receipt and rollback semantics are proven.

docs/LITESPM-LITECOWORK-INTEROP-LLD.md and docs/LITESPM-INTEGRATION-PLAN.md own this boundary. A Marketplace screen or mounted connect-or-install route does not qualify integration.

## 4. Execution discipline

- Preserve unrelated user files, archives, local configuration and ignored worktrees.
- Do not reset, clean, stash, delete archives or stage local secrets.
- Implement the smallest complete vertical slice and update owner prose, machine schemas, UI, Tauri bridge, route inventory, feature SDD and tests together.
- The UI is not an authority. No local React state may masquerade as a persisted permission, install, connection, process or policy.
- Do not claim an action succeeded until the owner returns a committed receipt or qualified external observation.
- Run narrow tests for each change, then cross-contract, frontend, workspace and packaged desktop tests.
- Inspect the full diff and generated outputs. Do not commit or push unless requested.

## 5. Superset design additions and sequencing (2026-11-23)

The product direction remains a complete, local-first, chat-first and BYOA-first workspace.
This section preserves the supplied superset capabilities in the V1 roadmap while keeping
unqualified providers unavailable and the existing V2 remote-runtime boundary intact. The
line-by-line design decisions and rejected assumptions are recorded in
[the Superset Design Disposition](audit/SUPERSET-DESIGN-DISPOSITION-2026-11-23.md). That
document is an implementation design input, not a substitute for owner contracts or release
evidence.

Delivery follows the existing P0–P11 gates and feature dependencies:

1. **P0 hygiene and verified defects:** ZIP-SUP-01 now fails closed on JSON recursion with a
   pre-parse depth/size budget and portable regression coverage; keep its Linux-only containment
   tests scoped to Linux. 017-T19 now bounds backup creation and untrusted header work before
   PBKDF2 under A-122; the local optimized Rust sample is recorded, while T14 still needs the
   full backup-operation/peak-memory baseline and slowest-row evidence before locking the ceiling.
   019-T25 repairs stale root contract fixtures. These code fixes do not close their platform or
   release qualification gates.
2. **P1/P3 Conversation and provider breadth:** finish the exact OpenCode first-send, permission,
   continuation, stop, retry, restart and recovery route; then integrate native Codex App Server,
   Claude and negotiated ACP adapters as independent harness-specific paths. Keep a conformance
   matrix by exact version/transport/OS/auth/capability and require full route evidence. A generic
   ACP client is not a universal provider promise.
3. **P4/P5 execution and Connections:** complete admitted Attempt → attested Environment →
   Trust-mediated bounded Effect → durable receipt/reconcile → independent verification with
   cancellation, timeout and restart recovery. Finish credential replay-safe sign-in and
   Connection egress only through their owning service, then connect notifications and
   Automations to actual send/settle paths.
4. **P2/P7 context and Workbench:** finish root ingestion/retrieval and durable Context Used;
   implement compact navigation and contextual object tabs; qualify local Computer preview and
   Take over, managed Browser settings and non-secret import, real Browser sessions, and Terminal
   only in an admitted Environment. Opening/restoring a surface never auto-starts it.
5. **P6/P10 presentation and deliverables:** reconcile the single RichPresentation schema,
   validators and renderer; implement deterministic charts/calculators, source provenance,
   semantic fallback and a clear inline-to-Canvas path. Deliver Markdown author/publish/export
   receipts and Saved Views in Library. Add DOCX/XLSX/PPTX/PDF authoring as separate, format-
   qualified capabilities after engine, fidelity, macro, formula, package and recovery decisions.
6. **Complete V1 roadmap additions:** add full Feature 021 Voice and Feature 022 Visual
   Annotation/Teach-by-Demo SDDs. Voice starts with user-held push-to-talk and transient editable
   transcription; annotations bind to immutable render/artifact versions and are sent only after
   review; Teach-by-Demo is explicit, redaction-first and draft-only. Implement these only after
   Conversation, Artifact, Browser/Computer consent and privacy dependencies are in place.
7. **P11 release:** native Windows x64/OpenCode v2.0.32 first, native Linux x64/OpenCode v2.0.32
   second, with the full packaged journeys, exact-provider compatibility, denial, duplicate,
   timeout-after-dispatch, cancellation-race, crash, restart and reconnect evidence. Remote
   Runtime/mobile continuity remains V2 under the current release contract.

Do not assign the supplied task IDs or SQLite v49/v50 slots. Existing 007/008/011/016 tasks
already use many of those IDs, and the migration allocator is v36 wired with v37–v47 reserved
or provisional. Each extension must add owner-specific tasks, allowed paths, cross-layer traces,
acceptance and migration changes in the owning SDD before implementation.

**First authorized implementation slice (2026-11-23; 011-T19, A-123):** complete the compact
shell navigation using the already loaded `list_workspaces` projection for a Workspace-backed
Projects directory. Keep New Chat, Recent Chats, Projects and Needs You directly visible; move
Work, Coworkers, Library, Automations, Routines, Discover, Dashboard, Goals, Notifications,
Ideas and explicit New Task into More; keep Settings in the footer. Projects selection follows
the dirty-Library guard, changes active Workspace context, clears the selected Conversation and
opens a fresh blank Conversation; it must never create a Project record, query another
Workspace's contents or imply a new grant. Add LOADING, READY-empty, READY and ERROR states,
distinct Recent Chats idle/loading/empty/failure/unavailable states, keyboard/current-route
behavior and focus restoration. No backend route,
schema, event, migration, unified Recents aggregation or composite Needs You count is in this
slice. The existing accepted navigation contract remains formally unchanged; this slice is
authorized by user direction and does not establish package acceptance or qualification. Add
failing component/reachability tests first, implement only within 011-T19's allowed UI paths,
then run focused UI tests, type/API checks, the feature-package validator and Markdown/design
consistency checks. Record exact results in CURRENT-RUN and IMPLEMENTATION-STATUS before
choosing the next dependency-ready owner task.

**Checkpoint:** focused shell, Conversation and source/component tests pass **56/56**; `pnpm exec
tsc --noEmit`, `pnpm run api:check` and rendered-script syntax check pass. The rendered-test launcher
discovers common Windows Chrome/Edge paths when `LITECOWORK_CHROME` is unset. In Chrome
**158.0.8127.38**, the shell-reachability group passes **4/4** and the Conversation group passes
**15/15** against the production-built harness. The shell journey covers Workspace-backed Projects,
More, narrow-drawer keyboard/focus and dirty-Library Stay/Discard; Conversation covers blank/draft
truthfulness, Runtime-unready Send, transient stream recovery, integrated shell navigation and
RichPresentation invalidation/retry. Keep 011-T19 PARTIAL: this is rendered browser CODE, not a
packaged Tauri journey, formal package acceptance, INTEGRATED or QUALIFIED evidence. Feature 010/014
dependencies and native platform qualification remain open. The full UI suite remains red at its
At this 2026-11-24 checkpoint, the full UI suite was red at **658/9/5**; this historical result is superseded by the 2026-11-25 full run recorded in the current evidence section above. Feature 002-T08 now wires the
authenticated credential route to v36 receipt lookup/reservation/settlement and idempotent keyring
write (CODE/SERVED); route tests and native/provider qualification gates are recorded in
CURRENT-RUN.md and Feature 002 TASKS.

## 6. Current implementation and immediate work

The current release posture is pre-release and not qualified. The current implementation record, exact route inventory, test outcomes and release blockers live in IMPLEMENTATION-STATUS.md. Durable Conversation first-send admission/outbox and prior-incarnation recovery have CODE/SERVED evidence; the next Conversation gaps are claim ambiguity and packaged OpenCode send/stream/stop/retry/restart qualification. User-delegated Feature 015 T22 adds SQLite v34 resolution, transaction revalidation, backfill and canonical guards for immutable TaskSpec policy snapshots. Focused native-Windows storage tests now cover Workspace-default, Coworker-default and Coworker-override source resolution, exact worker profile/model policy, TaskSpec revision, Workspace-head race, legacy null preservation, invalid cross-Workspace migration rollback and v28-v34 replay; Tauri projection (2/2), Work UI (2/2), TypeScript, API and 20-package structural checks pass. The task remains CODE-only and does not implement Task execution or qualify heterogeneous delegation. Feature 002-T08 now has CODE/SERVED authenticated credential-route wiring for Runtime-keyed replay, reservation, idempotent keyring mutation and settlement. T08 remains IN PROGRESS: no current V1 module declares a LiteCowork-owned credential slot, the seeded-PENDING test is not a daemon restart, and native OS-keyring/provider sign-in/bind/send qualification remains open. Other critical release priorities remain native Windows WorkspaceRoot support, one integrated Task execution slice, real Workbench Terminal/Diffs/Browser providers, and the feature gates listed in their task ledgers.
