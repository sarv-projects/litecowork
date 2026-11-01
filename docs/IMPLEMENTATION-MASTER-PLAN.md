# LiteCowork — Implementation Master Plan

**Status:** active dependency-aware delivery plan derived from Revision 9 §30, Appendices AI/AJ and the V7 competitive engineering audit.  
**Authority:** this plan orders work; owner-domain contracts, accepted ADRs and docs/schemas remain authoritative.

## 1. Delivery rule

**Master task list:** [LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md). **USER parity tests:** [COMPETITOR-PROMOTED-USE-CASES.md](COMPETITOR-PROMOTED-USE-CASES.md). **Viability:** [VIABILITY-VS-COMPETITORS.md](VIABILITY-VS-COMPETITORS.md).

Build complete vertical user journeys, not isolated capability count. Freeze load-bearing invariants now; promote only the contracts needed by the next slice. Do not invent missing contract edges locally. A design, schema, stub, mock, unit test, catalog entry or mounted route is not a qualified user journey.

Every slice names its user outcome, owner contracts, request/response/event schemas, state preconditions, authority, transaction owner, durable event, projection, UI, failure/recovery path, fixtures and exact OS/provider qualification.

## 2. Two coordinated delivery lanes

**Product critical path:** native Conversation → local Resources/context → cross-agent continuity → safe Task/Environment → golden Connection/capability → professional Artifact → Workbench/browser/computer → Coworker/memory → Automation → intelligent presentation/media → platform release.

**LiteSPM lane:** current-source reconciliation → stable IPC contract → shared daemon lifecycle → ACSR/consent → health/doctor → Marketplace → plan/apply lifecycle → bridges/invocation → portability/provenance → per-host conformance. It may proceed alongside independent Conversation work; package install or a Marketplace page is not LiteSPM integration acceptance.

## 3. Product sequence and exit gates

### P0 — Re-establish implementation truth
Record exact main SHA, active branch, dirty state, registered worktrees and ignored local data. Reconcile root ARCHITECTURE.md, docs/PRODUCT.md, V1 Product Release Contract, owner contracts, ADRs and docs/schemas. Re-run acceptance, served-route and qualification inventories; c7e9c05 reports are historical, not fresh evidence. Choose the first native desktop OS for GA and one first native agent route with exact version/auth/session support.

**Exit:** reviewed baseline/gap matrix with exact SHA, OS, agent route and TARGET/CODE/SERVED/INTEGRATED/QUALIFIED evidence.

### P1 — Real native Conversation
Implement fresh Workspace → selected AgentBinding → Conversation → send → stream → stop/cancel → persist → reload. Use a per-Conversation serialized turn lane, idempotent admission and durable USER/AGENT messages. Keep native session handles Runtime-local and preserve semantic history. Show the selected agent/model/effort and only supported native controls. Preserve drafts when a binding is unavailable.

**Exit:** owner sends through the desktop UI, streams and stops, reloads and restarts the daemon mid-turn; no lost/duplicated messages or false completion.

### P2 — Local files and context
Implement explicit WorkspaceRoot selection/revocation, bounded Resource ingestion, exact revisions/digests and deterministic retrieval for qualified formats. Expose Context Used from actual ContextAttachments and retrieval receipts. Search snippets are not implicit attachments; the machine is never scanned wholesale.

**Exit:** attach a folder, ask a grounded question with exact citations, revoke the root and verify that future retrieval is fenced.

### P3 — Agent Relay
Qualify a second native AgentModule. Implement A → B → A on the same Conversation using bounded portable context from committed messages, selected Resources and durable Task/Artifact/Evidence references. Never transfer private transcripts, provider credentials or hidden reasoning.

**Exit:** three-turn A → B → A owner journey with exact per-turn producer and no continuity loss.

### P4 — One safe Task path
Promote an outcome explicitly from Conversation. Qualify isolated planning; accept immutable PlanRevision and Steps atomically. Implement one Environment provider with real isolation attestation for read/write roots, network, process tree and resource limits. Atomically admit Attempt, lease, grant, invocation/effect intent and idempotency receipt. Execute one bounded file edit, reconcile ambiguous writes and independently verify.

**Exit:** one real Task edits one approved file inside qualified isolation and survives timeout/restart/cancel with a durable receipt and independent verification. A working directory or git worktree alone is not a sandbox.

### P5 — Golden Connection and capability
Complete one first-party account Connection. GitHub is a natural first developer workflow; Google Workspace or Microsoft 365 follows target-user demand. Implement SecretStore custody, supported OAuth/PKCE, exact scopes, serialized refresh and revoke/fence behavior. Start with one read, then one consequential write with approval, idempotency and reconciliation. Add progressive discovery and a qualified LiteSPM agent bridge.

**Exit:** real account read succeeds; one write uses exact approval and a reconciled receipt; revoke blocks future use.

### P6 — Professional Artifacts
Implement Artifact identity, immutable versions, provenance, Library visibility and one genuinely editable professional format with round-trip/export/reopen. Validate layout, formulas and content independently. Executable interactive artifacts remain a separately isolated runtime.

**Exit:** owner creates/revises/exports/reopens a deliverable and reviews source provenance and version differences.

### P7 — Useful Workbench and browser/computer
Ship one context-aware right Workbench integrated with Conversation/Task. Add Activity, Files, Diffs, Terminal, Browser, Computer and Artifact panels as providers qualify. Browser/computer require explicit consent, session epochs, takeover/return and stale-handle revocation. Terminal binds to admitted Environment/process-tree lifecycle. Needs You links directly to approvals, questions, blockers and failed work.

**Exit:** owner completes real browser and terminal work through UI, takes over/returns safely and proves stale handles cannot act.

### P8 — Coworkers, memory and responsibilities
Coworkers remain optional. Deliver Quick Create → first usable chat; scope Conversation ownership, ContextDocuments, memory, capability assignments and Connections. Start with owner-authored context and deterministic retrieval. Automatic learning waits for provenance, consent eligibility, third-party privacy, contradiction, revocation and deletion contracts. Goals are passive intent; Suggestions are proposals; neither grants authority.

**Exit:** create optional Coworker, chat, attach/revoke scoped context, restart and prove no cross-scope leakage.

### P9 — Automations and notifications
Implement durable schedule/event/poll/monitor AutomationOccurrences. TriggerHost observes an occurrence; ordinary Task admission selects execution Runtime. Cover timezone/DST, misfire, overlap, dedupe, cursor durability, offline/sleep, provider outage, notification ambiguity and recovery.

**Exit:** scheduled and event-triggered runs create exactly one Task each and settle with truthful history/notification status after restart.

### P10 — Intelligent UI, media and persistent surfaces
Render semantic-first safe rich presentation: facts/citations/warnings/actions remain in committed semantic content. Add bounded tables, charts, forms, diff summaries, cards and host actions over authorized projections. Add image/audio/voice routes only when qualified. Dashboard/widgets are projections over existing state, not new aggregates.

**Exit:** Conversation remains complete with rich rendering disabled; untrusted presentation cannot forge host state or actions.

### P11 — Portability, privacy and release
Complete export/import/backup/restore, deletion/revocation UX, diagnostics, accessibility, performance, crash recovery, upgrade/rollback and exact OS/agent/provider conformance for every marketed feature row. WSL2 is not native Windows or native Linux evidence. Unsupported cells remain explicitly unavailable.

**Exit:** declared V1 release matrix is green on declared platforms; only then schedule cloud Runtime/mobile/remote V2.

## 4. LiteSPM integrated-service sequence

Follow Revision 9 Appendix AI.19:
0. Re-read current LiteSPM HEAD/STATUS/ARCH/schemas/methods/tests and current LiteCowork routes/migrations.
1. Freeze and publish IPC handshake, generated client contract and protocol negotiation.
2. Implement typed LiteSpmClient, peer authentication and compatibility fixtures.
3. Implement one shared per-OS-user daemon: signed install/update/rollback, cold start, attach, health, restart and Windows stale-lock recovery.
4. Implement ACSR after agent discovery/install/update/remove, gated by one-time explicit auto-connect consent.
5. Add doctor, host/provider health, Marketplace on the same canonical catalog.
6. Qualify preview → exact planHash approval → single-use token → apply → receipt → reconcile for install/update/remove.
7. Preserve the 12 MCP bridge tools and /marketplace affordance; agents request but never self-approve.
8. Add grants/invocation, schema drift re-review, lockfile portability and per-host conformance.

The detailed LSI-001–LSI-040 scenarios are in LITESPM-INTEGRATION-PLAN.md. LiteSPM remains owner of package state; Core remains owner of authorization and mediated use.

## 5. Do not build before the gates

Do not build a universal model router, hidden lead switch, 25+ unqualified agent logos, generic workflow-graph engine, embedded Activepieces/Nango runtime, broad arbitrary native plugins, second package manager, cloud Runtime/laptop-off continuation, mobile collaboration, duplicate Project aggregate, separate coding universe or memory database. Avoid badges that infer readiness from installation, route registration, tests or catalog presence.

## 6. Execution discipline

Phase numbers express product dependency, not a requirement to serialize every file. Independent UI shell, schemas, adapter fixtures, accessibility and test harness work may run in parallel after owner contracts freeze. Every story records dependency edges and an exit bundle. An epic is not complete because one representative screen exists.

## 7. Immediate next sprint

1. Keep main docs-first. When implementation is authorized, create a clean isolated worktree from the chosen Git baseline (c7e9c05) rather than building from this docs-only checkout.
2. Inspect the preserved feature/conversation-session-supervisor worktree as a separate dirty candidate. Reconcile its Conversation activation changes before merging or recovering any part of it.
3. Execute the focused Conversation activation test recorded in the preserved 2026-10-26 plan; it was written but not run. Follow with storage, event rollback, stale-version, host/incarnation and recovery tests. A cargo check is compile evidence, not execution evidence.
4. Complete the missing native supervisor path: observed adapter readiness outside SQLite → atomic activation → bounded context load → native prompt dispatch → streamed/committed result. Keep Send fail-closed until the full route is proven.
5. Select the first GA OS and first native agent route; deliver one real Send/Stop/reload UI journey with actual daemon, IPC, storage, provider and owner USER evidence.
6. Run LiteSPM current-source/protocol reconciliation in parallel without blocking native Conversation work.

## 8. Owner contracts referenced

Root ARCHITECTURE.md; PRODUCT.md; V1-PRODUCT-RELEASE-CONTRACT.md; AGENT-FABRIC.md; AGENT-CONTROL.md; PROTOCOLS.md; CONTEXT.md; TASK-RUNTIME.md; ENVIRONMENTS.md; TRUST.md; STATE-MACHINES.md; CONNECTORS-WORKFLOWS.md; LITESPM-LITECOWORK-INTEROP-LLD.md; DESKTOP-SHELL-LLD.md; V1-UI-SCREEN-CONTRACT.md; AUTOMATION.md; ARTIFACTS-EVIDENCE.md; PRESENTATION-RUNTIME.md; TESTING.md; and schemas under docs/schemas/.
