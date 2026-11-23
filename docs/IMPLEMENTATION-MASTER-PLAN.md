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

- Enforce canonical AgentHostInstance shutdown order on every orderly close path: persist STOPPING after CLOSED/LOST and before native process shutdown, then persist STOPPED only on observed exit. The user-delegated T03 correction is CODE-only. A-104 host process identity and prior-incarnation recovery are wired before Operator admission; native-Windows helper tests cover fail-closed identity handling, but not SQLite-backed startup reconciliation, live process termination, Linux process-group cleanup or packaged crash/restart. StopUnconfirmed now retains the exact process handle and AgentSession identity in the Runtime-local host owner as CODE; same-incarnation stream/session reconciliation, reusable host leases, and those integration/qualification gates remain open.

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
- Use source-bound metrics, deterministic host calculations and accessible chart/table fallbacks. Inputs remain local and non-submitting under accepted B-40; any remembered state or reviewed ordinary-turn send is a separately proposed host action under B-46 and remains gated until owner acceptance.
- The owner decided on 2026-11-17 to keep Saved Views in V1 and amend Feature 007 B-21. Feature 007 revision 1.1 now has a detailed TARGET contract: one non-executable Artifact kind/media type, exact source lineage and semantic fallback, immutable update from a new committed presentation, standard Artifact receipts/events, archive-only removal, source-deletion and recovery behavior. Cross-owner review and canonical API/schema promotion remain open. Preserve Feature 016 B-40: session-modified calculator values are transient and are neither saved nor sent.
- When generated media is supported, publish it as an explicit provenance-bearing Artifact through Feature 007's owner path; do not treat a media preview, model output or temporary presentation block as a committed deliverable.
- Keep arbitrary HTML/JS, eval, script imports and host API calls out of chat WebView. Do not add a second UI schema, hosted BYOK router, implicit provider substitution or general-purpose generated-code runtime.
- Qualify inline rendering, Canvas continuity, persistence/reopen, denial, race, crash/restart and supported provider/OS routes as one packaged journey. Keep the isolated Interactive Artifact Runtime separately PREVIEW_ONLY until its own gate qualifies.

**Current gate:** Feature 016 revision 0.4 is TARGET/IN_REVIEW. T25 records the Saved View direction and drafted contract; cross-owner review is still required. B-46 remains open for separate remembered-input and input-sharing behavior. Record whole-scope T01 and promote owner contracts/schema at T02 before implementation. Existing CODE is limited to validator parity and safe fallback foundations; no Saved View route or control is authorized by this plan text.

**Exit:** packaged user can obtain a complete semantic answer, safely inspect and interact with a validated companion view, and expand it without losing state during the active view. A Saved View reopens its exact committed presentation/defaults and updates only from a new committed answer under its own Artifact receipt/recovery path. Any remembered-input or state-sharing capability requires its own owner decision and receipt. Denial and recovery remain truthful, and malformed content cannot mint host state or authority.

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

## 6. Current implementation and immediate work

The current release posture is pre-release and not qualified. The current implementation record, exact route inventory, test outcomes and release blockers live in IMPLEMENTATION-STATUS.md. Durable Conversation first-send admission/outbox and prior-incarnation recovery have CODE/SERVED evidence; the next Conversation gaps are claim ambiguity and packaged OpenCode send/stream/stop/retry/restart qualification. User-delegated Feature 015 T22 adds SQLite v34 resolution, transaction revalidation, backfill and canonical guards for immutable TaskSpec policy snapshots. Focused native-Windows storage tests now cover Workspace-default, Coworker-default and Coworker-override source resolution, exact worker profile/model policy, TaskSpec revision, Workspace-head race, legacy null preservation, invalid cross-Workspace migration rollback and v28-v34 replay; Tauri projection (2/2), Work UI (2/2), TypeScript, API and 20-package structural checks pass. The task remains CODE-only and does not implement Task execution or qualify heterogeneous delegation. Other critical release priorities remain native Windows WorkspaceRoot support, one integrated Task execution slice, real Workbench Terminal/Diffs/Browser providers, and the feature gates listed in their task ledgers.
