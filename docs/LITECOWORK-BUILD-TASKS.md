# LiteCowork build tasks and release checklist

This is the working delivery checklist for the active source tree. It is not a Git commit plan. Feature-specific work items belong in specs/NNN-*/TASKS.md. Product semantics remain in owner contracts; current evidence is in IMPLEMENTATION-STATUS.md.

## Status snapshot (refreshed 2026-11-12, review tip 8432397)

The tables below are the plan and carry no per-row status. Per-task status lives in specs/NNN-*/TASKS.md; this snapshot only indexes it. Nothing is INTEGRATED with a live agent or QUALIFIED.

| Group | State | Detail |
|---|---|---|
| BT-00 | Partly done | Route inventory generated and matches (178 served / 1 local-only / 53 target-only); CURRENT-RUN.md and IMPLEMENTATION-STATUS.md synced 2026-11-10 |
| BT-01 | CODE and SERVED for BT-01.1, 01.2, 01.4; BT-01.3 CODE (ACP client, OpenCode HTTP adapter kept); BT-01.5 and 01.6 partial | 001 DONE/PARTIAL list in CURRENT-RUN.md; live OpenCode run (001-T09) NOT STARTED |
| BT-02 | Linux storage tests only; Windows tasks DEFERRED to the owner's Windows laptop | 003-T02 PARTIAL |
| BT-03 | Registry, descriptors, auth observation, Matchmaker CODE/SERVED; Claude Code module BLOCKED; relay (004) not started | 002 statuses |
| BT-04 | Not started | 005; Windows Environment provider B-35 still PROPOSED |
| BT-05 | 013 part: whole package DEFERRED by the owner (A-62). 006 part: BLOCKED on B-19/A-82 owner decisions; E2E-05 Task-write row order remains OPEN under A-80 | See the DEFERRED note in specs/013-litespm-package-lifecycle-bridge/TASKS.md and A-80/A-82 in the contradiction register |
| BT-06 to BT-09 | Existing foundations only; 010-T19 deferred out of V1. Feature 016 baseline acceptance is pending; A-84 separates its revision 0.2 extension decision and A-85 blocks MCP proposal exposure pending a canonical tool-input/sequence contract. | 007, 016, 008, 009, 010 |
| BT-10 | Partly done for Linux; Windows and packaged rows DEFERRED | 011 DONE T01, T04, T05, T07, T08, T09, T10, T21 |
| BT-11 | Storage layer only (014-T03 partial, migration v31) | Routes, search, purge, UI NOT STARTED |
| BT-12 to BT-14 | Not started; B-39, B-40, B-41, D-01, B-45 accepted as proposed; B-42 open | 015 to 018, 020 |
| BT-15 | CI and release machinery skeleton (CODE, never run on GitHub); B-43 and B-44 items DEFERRED | 019 |
| Co-edit (012) | Storage layer only (012-T05 partial, migration v32) | Routes, service, UI NOT STARTED |

## BT-00 — Current baseline and documentation integrity

| ID | Task | Exit evidence |
|---|---|---|
| BT-00.1 | Record exact branch/SHA/worktree and preserve ignored local archives | CURRENT-RUN.md; no destructive cleanup |
| BT-00.2 | Keep route inventory generated from composed production router | scripts/audit_served_routes.py --report matches schemas/served-route-inventory.json |
| BT-00.3 | Reconcile source/status/UI owners and stale references | IMPLEMENTATION-STATUS.md, semantic map, current inventory |
| BT-00.4 | **Validated** — Windows WorkspaceRoot private-locator route regressions | Six route tests pass on the declared Windows target; production locator validation remains fail-closed |
| BT-00.5 | Run documentation link and owner-contract guards | Markdown audit and cross-contract validator pass with reviewed exceptions only |
| BT-00.6 | Run frontend type/build/unit checks and packaged desktop smoke | Exact commands and results recorded |
| BT-00.7 | Release evidence and release-row schemas, release docs index | docs/schemas/release-evidence-bundle.schema.json, docs/schemas/release-matrix-row.schema.json, docs/release/README.md; 019-T02 |

## BT-01 — Conversation dispatch durability (Feature 001)

| ID | Task | Exit evidence |
|---|---|---|
| BT-01.1 | Make submit admission and dispatch intent recoverable across daemon crash | Durable state/outbox; restart finds committed undispatched turns |
| BT-01.2 | Make idempotent replay resolve original dispatch state | Same key returns receipt; no duplicate provider prompt |
| BT-01.3 | Qualify the native-agent adapter as an ACP client (Feature 002 002-T17, consumed by Feature 001), keeping OpenCode v2.0.32 HTTP as one non-ACP adapter until its ACP route qualifies | Send/stream/cancel/retry/restart USER flow through the ACP client; OpenCode event mapping confirmed against the 001-T23 fixture |
| BT-01.4 | Ensure unsupported agent bindings cannot appear Send-eligible | Exact descriptor + adapter/auth/OS eligibility matrix |
| BT-01.5 | Verify semantic message, rich presentation and stream resync | Saved transcript authoritative after gap/restart |
| BT-01.6 | Complete Feature 001 API/UI/error/recovery contract review | Supporting contracts accepted by owners |

## BT-02 — Workspace roots and Resources (Feature 003)

| ID | Task | Exit evidence |
|---|---|---|
| BT-02.1 | Audit root selection/create/pause/resume/scan/revoke route parity | Operation inventory + Tauri command + screen control matrix |
| BT-02.2 | Qualify Windows paths, identity and symlink/reparse-point behavior | Positive and negative platform tests |
| BT-02.3 | Pin Resource revisions/digests in context and retrieval | Grounded output names exact source revision |
| BT-02.4 | Qualify first explicit ingestion formats | Format parser, bounds, unsupported behavior, restart and deletion tests |
| BT-02.5 | Keep ZIP preview separate from extraction/publication | Current unavailable reason and no fake extracted files |

## BT-03 — Native agent fidelity and relay (Features 002/004)

| ID | Task | Exit evidence |
|---|---|---|
| BT-03.1 | Align AgentModule installation/auth/descriptor/session states | No collapsed “connected” flag |
| BT-03.2 | Select second agent only after auth/OS/provider research | Pinned exact version and terms route |
| BT-03.3 | Qualify A→B→A Conversation continuity from committed state | Explicit per-turn producer and no transcript/credential migration |
| BT-03.4 | Preserve native model/mode/effort/options and unsupported behavior | Descriptor freshness/conformance tests |

## BT-04 — Safe Task execution (Feature 005)

| ID | Task | Exit evidence |
|---|---|---|
| BT-04.1 | Implement planning admission and immutable accepted PlanRevision | CAS/version/digest and failure recovery |
| BT-04.2 | Implement one Environment with real isolation attestation | Filesystem/network/process/resource limits verified on OS |
| BT-04.3 | Implement one Attempt and fenced execution lease | Duplicate/stale worker denied |
| BT-04.4 | Wire Trust into the actual mediated execution boundary | Real policy decision in path, not dead module |
| BT-04.5 | Execute one bounded file edit with CapabilityInvocation/Effect receipt | Timeout-after-effect and idempotent reconciliation |
| BT-04.6 | Verify independently and present actual evidence | Model self-report is not accepted as verification |

## BT-05 — LiteSPM and Connections (Features 013/006)

BT-05.1..BT-05.3 are owned by Feature 013 (LiteSPM package lifecycle bridge); BT-05.4 and BT-05.5 stay with Feature 006.

| ID | Task | Exit evidence |
|---|---|---|
| BT-05.1 | Pin LiteSPM bridge service/schema/auth version | Published contract and compatibility fixtures |
| BT-05.2 | Use one LiteSPM package writer and source of inventory | No second package DB/config writer |
| BT-05.3 | Plan → digest approval → apply → receipt → reconcile one package | Crash/retry/rollback/UNKNOWN tests |
| BT-05.4 | Keep install, auth, assignment, grant and invocation separate | UI and service state matrix |
| BT-05.5 | Qualify one first-party Connection read then one write | Secret custody, scopes, approval, idempotency, revoke |

## BT-06 — Artifacts and intelligent presentation (Features 007/016)

BT-06.1 and BT-06.2 are owned by Feature 007; BT-06.3..BT-06.5 are owned by Feature 016 (rich presentation and Intelligent UI).

| ID | Task | Exit evidence |
|---|---|---|
| BT-06.1 | Qualify one professional Artifact format | Create/edit/save/export/reopen/compare round trip |
| BT-06.2 | Verify exact source/version and Artifact provenance | Hash and revision consistency |
| BT-06.3 | Validate typed RichPresentation document | Size/depth/node/schema/digest bounds and semantic fallback |
| BT-06.4 | Add the accepted first accessible Intelligent UI component, SAFE_CALCULATOR | Restricted state reducer, table fallback, malicious-input tests; exact feature acceptance still pending at 016-T01 |
| BT-06.5 | Keep Interactive Artifact in isolated runtime | Separate process, authority and OS qualification |

## BT-07 — Workbench providers (Feature 008)

| ID | Task | Exit evidence |
|---|---|---|
| BT-07.1 | Remove simulated terminal success and sample output | No false success label or exit code |
| BT-07.2 | Implement daemon-owned PTY/ConPTY session | Stream/resize/detach/reattach/stop/backpressure/restart; resolve A-83 before claiming per-command exit receipts or using PTY output as Task/Effect evidence |
| BT-07.3 | Replace sample diffs with exact VCS/revision diff | Source identity and actual hunk tests |
| BT-07.4 | Keep Browser iframe labeled preview until real provider | No Chrome/automation/sandbox claims |
| BT-07.5 | Implement isolated browser provider and takeover only after security design | Profile, site grants, lease epoch, stale handle, egress tests |
| BT-07.6 | Add Computer provider only with real OS permissions/consent | Display identity, active window/focus, takeover tests |

## BT-08 — Coworker, memory and responsibility (Feature 009)

| ID | Task | Exit evidence |
|---|---|---|
| BT-08.1 | Implement Coworker-owned Conversation admission | Server derives exact origin and immutable revision |
| BT-08.2 | Define first scoped Memory source/record lifecycle | Provenance, eligibility, correction, revocation, deletion |
| BT-08.3 | Implement responsibility proposals and reviewed activation | No model text or UI grants authority |
| BT-08.4 | Qualify Coworker pause/archive/resume with active Task/Automation checks | Conflict/retention/recovery tests |
| BT-08.5 | Verify no cross-Workspace leakage | USER/SYSTEM privacy fixtures |

## BT-09 — Automation and notifications (Feature 010)

| ID | Task | Exit evidence |
|---|---|---|
| BT-09.1 | Preserve TriggerHost as occurrence scheduler/materializer | One occurrence receipt per source slot |
| BT-09.2 | Qualify due cursor/occurrence/task-link atomicity | Duplicate, crash, revision update, resume tests |
| BT-09.3 | Qualify timezone/DST/misfire/overlap/sleep/offline | Explicit blocker and no laptop-off claim |
| BT-09.4 | Implement notification provider and ambiguity handling | Dedupe and receipt/reconcile |
| BT-09.5 | Connect Task execution only after Feature 005 is qualified | Materialized READY Task is not shown as completed work |

## BT-10 — Shell, Settings and release (Feature 011)

| ID | Task | Exit evidence |
|---|---|---|
| BT-10.1 | Remove or implement inert Browser/Terminal/Security Settings controls | No fake persisted settings or success badges |
| BT-10.2 | Map every visible Settings control to exact owner and receipt | Control-level semantic map |
| BT-10.3 | Verify Runtime vs Operator vs Task readiness copy | No false “execution ready” indication |
| BT-10.4 | Complete shell keyboard/focus/reduced-motion tests | Packaged desktop acceptance |
| BT-10.5 | Run full declared OS/provider release matrix | CODE/SYSTEM/USER and owner sign-off; owned by 019-T14, 019-T15 and 019-T22 |

## BT-11: Conversation history management (Feature 014)

| ID | Task | Exit evidence |
|---|---|---|
| BT-11.1 | Rename, archive and unarchive with CAS and idempotency | 014-T03, 014-T07, 014-T12 |
| BT-11.2 | Conversation search with in-transaction index maintenance | 014-T04, 014-T08, 014-T11 |
| BT-11.3 | Delete as tombstone, fenced purge and startup resume (B-38) | 014-T05, 014-T06, 014-T09, 014-T13 |
| BT-11.4 | Qualify on both release rows | 014-T17 and proposed E2E-14 |

## BT-12: Task control and delegation (Feature 015)

| ID | Task | Exit evidence |
|---|---|---|
| BT-12.1 | Typed TaskStatus transitions, pause/resume with quiescence | 015-T03, 015-T04 |
| BT-12.2 | Steering, cancel fan-out and handoff with late-result fencing | 015-T05..015-T07 |
| BT-12.3 | Child-Attempt admission, budgets and escalation (B-39, A-37) | 015-T08..015-T10 |
| BT-12.4 | Linux-row qualification of the delegation pair; Windows after B-35 | 015-T17, 015-T18 and proposed E2E-15 |

## BT-13: Backup, usage and diagnostics (Features 017/018)

| ID | Task | Exit evidence |
|---|---|---|
| BT-13.1 | Workspace-scoped snapshot that cannot leak other Workspaces | 017-T03 with canary scan |
| BT-13.2 | Portable export/import and restore correctness fixes | 017-T04, 017-T05, 017-T08, 017-T09 |
| BT-13.3 | Usage ledger, projections and export without fabricated zeros | 018-T03..018-T06 |
| BT-13.4 | Structured redacted log, crash records and support bundle (B-42) | 018-T07..018-T10 |
| BT-13.5 | Qualify E2E-12 and proposed E2E-16 on both rows | 017-T16, 018-T16 |

## BT-14: Proactivity and quiet learning (Feature 020)

| ID | Task | Exit evidence |
|---|---|---|
| BT-14.1 | Fix the deterministic learning gate before first use | 020-T03 |
| BT-14.2 | Deterministic Suggestion producers with budgets (D-01) | 020-T04..020-T07 |
| BT-14.3 | Learning storage, service and review UI (B-45) | 020-T08..020-T11 |
| BT-14.4 | Qualify proposed E2E-17 on both rows | 020-T15 |

## BT-15: Release engineering and V1 go/no-go (Feature 019)

| ID | Task | Exit evidence |
|---|---|---|
| BT-15.1 | Native Linux and Windows CI runners; secret scan and dependency audit | 019-T03..019-T05 |
| BT-15.2 | Signed Windows installers and Linux deb with checksums (B-43) | 019-T08..019-T10 |
| BT-15.3 | Updater gated on Feature 011 quiescence | 019-T11, 019-T12, 019-T19 |
| BT-15.4 | Licensing ADR and third-party notices (B-44) | 019-T13 |
| BT-15.5 | Row-aware qualification matrix, accessibility, performance, resilience | 019-T14..019-T18 |
| BT-15.6 | V1 go/no-go over every package VERIFIED task | 019-T23 |

## V1 release waves (2026-11-09)

The owner accepted packages 001-012 on 2026-11-09. Packages 013-020 are IN_REVIEW and each wave item for them starts only after that package's T01 records acceptance. A wave starts when its inputs below are met, not on a date.

| Wave | Packages | Inputs | Exit |
|---|---|---|---|
| W0 | 011, 019 skeleton (019-T03..019-T05) | Acceptance recorded | Native CI runs `check.sh` plus the `src-tauri` workspace on Linux and Windows; IPC profiles qualified (011-T02, 011-T03) |
| W1 | 001, 002, 003, 014; then 012 after 001-T07 | W0 CI green | OpenCode v2.0.32 bindable (002-T01) and dispatching through a durable, single-flight, recoverable turn; E2E-01 and E2E-03 on the Windows row |
| W2 | 005 (Linux), 007, 009, 013 observe (013-T02..013-T06), 016 | 001 commit and recovery tasks done | E2E-04 on Linux, E2E-07, E2E-09, E2E-11 |
| W3 | 006, 013 plan/apply (013-T07..013-T18), 008 | 005 Environment and Trust wiring on Linux | E2E-05, E2E-06, E2E-08 |
| W4 | 004, 015, 020 | 002-T12 Claude Code module; 005 Attempt admission; 009 Coworker chat | E2E-02, proposed E2E-15 and E2E-17 |
| W5 | 010 | 005 execution handoff | E2E-10 |
| W6 | 017, 018, 019 qualification (019-T06..019-T23) | All earlier waves VERIFIED or explicitly descoped by the owner | E2E-12, proposed E2E-16, signed packages, V1 go/no-go |

Windows Task execution, the daemon terminal and Windows delegation stay UNAVAILABLE until B-35 is accepted and 005-T17/005-T18 qualify. That path runs in parallel with W3 to W5 and does not block the Linux row.

Feature 006's Connection/read qualification is distinct from its Task-scoped golden write. A-80 leaves open whether the write requires an admitted Environment; if it does, E2E-05's write row follows Feature 005's Linux-first qualification and Windows stays unavailable until B-35. A-82 also blocks provider contract promotion until the OAuth/App permission model and actual user-visible scopes are owner-approved. Do not describe Windows Connection/read evidence alone as a fully qualified E2E-05 journey.

Folded tasks (owner-accepted 2026-11-09, register A-55) land in the wave of their owning package. The ACP native-agent client (002-T17, consumed by Feature 001) is the W1 adapter spine and carries the OpenCode fixture and normalizer (001-T23, 001-T24) plus the activity/plan cards, autoscroll and native permission prompts (001-T25, 001-T26, 001-T27); the Agent Matchmaker (002-T18) is W1. Per-block Markdown (016-T18) and the Context Debugger (003-T17) are W2. The LiteSPM pin (013-T20) is W3; the iframe sandbox guard and Git worktree/diff/PR (008-T19, 008-T20) are W3. Mission Control and read-only Compare (015-T20) is W4. The shell reachability, decomposition, SettingsPage merge and Workspace Dashboard tasks (011-T19..011-T22) ride the W0 shell package, with the Dashboard deliverable landing once its inputs exist. The operator-api.ts codegen and drift gate (018-T18) is W6. The V1 go/no-go (019-T23) re-synced its VERIFIED-id list to the new final task ids.

## Migration allocation (provisional)

The wired schema is v32 (v30 adds REPORTED turn activity records, 001-T25 storage layer; v31 adds Conversation history lifecycle state, 014-T03; v32 adds Conversation control events and owner side notes, 012-T05): v28 (register A-40) and v29 (Feature 001 durable turn) are applied by `crates/storage-sqlite/src/lib.rs`. Many packages wrote "sqlite-v29.sql or the next free number" and Feature 012 named v30, so the numbers collide. This table assigns numbers in wave merge order. Rules: the first package to bump `SCHEMA_VERSION` also wires v28, because migrations apply in order; a package that merges out of order takes the next free number and updates this table in the same change; a merged migration is never renumbered or edited.

| Migration | Package and task | Content (from the package DATA-MODEL) |
|---|---|---|
| v28 (existing) | wired by 001 (done) | Workspace delegation settings revisions (A-40) |
| v29 (wired) | 001 (W1, done) | Dispatch outbox, single-flight triggers, commit identity table, usage records |
| v30 (wired) | 001-T25 (done) | REPORTED Conversation turn activity records |
| v31 (wired) | 014-T03 (partial: storage only) | `conversation_history_state` side table (status, archived_at, deleted_at); search index and purge manifest are NOT in v31 and take a later number |
| v32 (wired) | 012-T05 (partial: storage only) | `conversation_control_events`, `conversation_owner_side_notes` (replaces the v30 named in 012) |
| v33 | 002-T07 (W1) | Runtime-local durable auth observations. The descriptor store already exists in migration 16; no descriptor DDL is needed. |
| v34 | 002-T08 (W1) | Runtime-local credential-submission replay receipts only if the reviewed SecretStore crash-reconciliation contract requires durable DDL; otherwise release the reservation before implementation. Never store secret bytes. |
| v35 | 003-T09 (W1) | `context_attachments`, `retrieval_receipts` (renumbered from v32) |
| v36 | 005-T01 (W2) | Plan acceptance, Attempt admission and Effect records |
| v37 | 007-T01 (W2) | `artifact_export_receipts`, source Conversation columns |
| v38 | 009 (W2) | Conversation Coworker origin and memory fields |
| v39 | 013 (W2/W3) | Package approvals and operation receipts |
| v40 | 006-T01 (W3) | `connections` rebuild with `REVOKED` and `remote_revocation`, Connection records, and `auth_attempts` (state/expiry/digests only; no verifier or token bytes) |
| v41 | 008-T01 (W3) | Browser session/lease/receipt and terminal session/receipt |
| v42 | 015 (W4) | `task_control_requests`, `task_resume_packets` |
| v43 | 020-T02 (W4) | `learning_candidates`, `coworker_learning_policies`, `memory_records` |
| v44 | 010-T01 (W5) | Delivery lease and status transitions, SUPPRESSED rows |
| v45 | 017 (W6) | `backup_operations` |
| v46 (if needed) | 018 (W6) | Optional diagnostics table; the usage table comes from 001 |

Packages 004, 011, 016 and 019 plan no migration.

## Cross-package dependency graph

Each edge "A -> B" means B needs A's named outcome. Edges are taken from each package's PLAN.md and TASKS.md "Depends on" lines.

```text
011 (IPC, shell)           -> every package with UI or Operator routes
019 skeleton (CI)         -> every package qualification task
002-T01 (bindable OpenCode) -> 001-T04, 001-T09
001 (turn commit, Stop)   -> 012, 014, 016-T08, 018-T03, 004
001-T15 (usage records)   -> 018-T05
002-T12 (Claude Code)     -> 004-T05, 015-T08
003 (retrieval)           -> 009-T07, 014-T13
005 (Environment, Attempt) -> 006-T07, 008-T09, 010-T08, 015
005-T17 (Windows, B-35)   -> 008-T12, 015-T18, 005-T18
013-T14 (bridge handoff)  -> 006 package-backed capabilities
007 (Artifact create)     -> 016-T10
009 (Coworker chat)       -> 020-T09
010 quiet hours           -> 020-T06
011-T13/T14 (quiescence)  -> 019-T11, 019-T19
all package VERIFIED tasks -> 019-T23 (V1 go/no-go)
```

## Parallel documentation and validation

- Update owner documents, schemas, UI contract, action ledger and SDD wiring together.
- Keep the route inventory current after every Operator router/OpenAPI change.
- Run the served-route validator, Markdown audit and cross-contract validator.
- Run the feature package validator.
- Run Rust, frontend, packaged desktop and owner tests for the affected slice.
- Do not mark a story/SDD VERIFIED because a static script or unit suite passes.
