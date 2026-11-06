# LiteCowork build tasks and release checklist

This is the working delivery checklist for the active source tree. It is not a Git commit plan. Feature-specific work items belong in specs/NNN-*/TASKS.md. Product semantics remain in owner contracts; current evidence is in IMPLEMENTATION-STATUS.md.

## BT-00 — Current baseline and documentation integrity

| ID | Task | Exit evidence |
|---|---|---|
| BT-00.1 | Record exact branch/SHA/worktree and preserve ignored local archives | CURRENT-RUN.md; no destructive cleanup |
| BT-00.2 | Keep route inventory generated from composed production router | scripts/audit_served_routes.py --report matches schemas/served-route-inventory.json |
| BT-00.3 | Reconcile source/status/UI owners and stale references | IMPLEMENTATION-STATUS.md, semantic map, current inventory |
| BT-00.4 | **Validated** — Windows WorkspaceRoot private-locator route regressions | Six route tests pass on the declared Windows target; production locator validation remains fail-closed |
| BT-00.5 | Run documentation link and owner-contract guards | Markdown audit and cross-contract validator pass with reviewed exceptions only |
| BT-00.6 | Run frontend type/build/unit checks and packaged desktop smoke | Exact commands and results recorded |

## BT-01 — Conversation dispatch durability (Feature 001)

| ID | Task | Exit evidence |
|---|---|---|
| BT-01.1 | Make submit admission and dispatch intent recoverable across daemon crash | Durable state/outbox; restart finds committed undispatched turns |
| BT-01.2 | Make idempotent replay resolve original dispatch state | Same key returns receipt; no duplicate provider prompt |
| BT-01.3 | Qualify OpenCode v2.0.32 adapter on packaged Windows | Send/stream/cancel/retry/restart USER flow |
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

## BT-05 — LiteSPM and Connections (Feature 006)

| ID | Task | Exit evidence |
|---|---|---|
| BT-05.1 | Pin LiteSPM bridge service/schema/auth version | Published contract and compatibility fixtures |
| BT-05.2 | Use one LiteSPM package writer and source of inventory | No second package DB/config writer |
| BT-05.3 | Plan → digest approval → apply → receipt → reconcile one package | Crash/retry/rollback/UNKNOWN tests |
| BT-05.4 | Keep install, auth, assignment, grant and invocation separate | UI and service state matrix |
| BT-05.5 | Qualify one first-party Connection read then one write | Secret custody, scopes, approval, idempotency, revoke |

## BT-06 — Artifacts and intelligent presentation (Feature 007)

| ID | Task | Exit evidence |
|---|---|---|
| BT-06.1 | Qualify one professional Artifact format | Create/edit/save/export/reopen/compare round trip |
| BT-06.2 | Verify exact source/version and Artifact provenance | Hash and revision consistency |
| BT-06.3 | Validate typed RichPresentation document | Size/depth/node/schema/digest bounds and semantic fallback |
| BT-06.4 | Add one accessible Intelligent UI component | Restricted state reducer, table fallback, malicious input tests |
| BT-06.5 | Keep Interactive Artifact in isolated runtime | Separate process, authority and OS qualification |

## BT-07 — Workbench providers (Feature 008)

| ID | Task | Exit evidence |
|---|---|---|
| BT-07.1 | Remove simulated terminal success and sample output | No false success label or exit code |
| BT-07.2 | Implement daemon-owned PTY/ConPTY session | Stream/resize/detach/reattach/stop/backpressure/restart |
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
| BT-10.5 | Run full declared OS/provider release matrix | CODE/SYSTEM/USER and owner sign-off |

## Parallel documentation and validation

- Update owner documents, schemas, UI contract, action ledger and SDD wiring together.
- Keep the route inventory current after every Operator router/OpenAPI change.
- Run the served-route validator, Markdown audit and cross-contract validator.
- Run the feature package validator.
- Run Rust, frontend, packaged desktop and owner tests for the affected slice.
- Do not mark a story/SDD VERIFIED because a static script or unit suite passes.
