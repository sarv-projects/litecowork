# Assumption-free documentation re-audit (2026)

Status: audit record. This document is a Phase 1 verdict table, not an owner contract. It
does not redefine any concept. Where a doc claim is wrong, the correction is applied at the
owning document in the same change and this table links to it; where a claim is right, the
verdict is `true` and the owner is cited as evidence.

This re-audit treats every documentation claim as unverified and checks it against the
active source (`apps/`, `crates/`, `capabilities/`) and the source-backed status authority
[IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md). It complements, and does not replace,
[REPOSITORY-DOCUMENTATION-AUDIT.md](REPOSITORY-DOCUMENTATION-AUDIT.md) (consistency and
validation status) and the adjudicated
[CONTRADICTION-AND-ALIGNMENT-REGISTER.md](CONTRADICTION-AND-ALIGNMENT-REGISTER.md)
(A-01..A-51, B-11..B-33). Most ground-truth divergences are already encoded as register
rows A-01..A-50; those rows remain the yardstick and this table cites them rather than
restating them. Genuinely new findings are filed from A-51.

## Verdict vocabulary

- **true** — the doc claim matches current source and the repo evidence vocabulary.
- **stale** — the claim described earlier source that has since changed; it is now wrong.
- **aspirational-unlabeled** — the claim describes TARGET behavior but reads as shipped
  without a TARGET/CODE/SERVED label.
- **contradicted** — the claim conflicts with current source or another owner.

Evidence vocabulary (TARGET / CODE / SERVED / INTEGRATED / QUALIFIED) is defined in
[docs/README.md](README.md#evidence-vocabulary) and
[IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md#evidence-vocabulary).

## Cluster A — Product, Architecture, ADRs

| Doc + section | Claim (quoted/paraphrased) | Code reality (file-path evidence) | Verdict | Register |
|---|---|---|---|---|
| [ARCHITECTURE.md](../ARCHITECTURE.md) execution model | Exactly one real native agent execution path exists and other harnesses are not turn-capable merely because discovery/probe/binding exist. | One `provider_key=opencode` branch in `apps/litecoworkd/src/agents/opencode_server.rs` (enum `V2_0_32`) and `opencode_profile_probe.rs` (pins `"2.0.32"`); other providers fail closed. | true | A-04 |
| [PRODUCT.md](PRODUCT.md) / [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md) release posture | V1 is pre-release and not release-qualified; a route/mock/tile is not a shipped journey. | Matches IMPLEMENTATION-STATUS.md "Release posture" and the 12 release blockers. | true | A-02, A-03 |
| [adr/](adr/) decisions vs current authority | ADRs explain rationale and never override current authority. | COVERAGE-MATRIX.md authority order §7 states the same; no ADR redefines an owner. | true | — |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) "Active documentation corpus" | 258 files / 54,473 lines; Markdown audit 255 / 54,385. | The old counts were superseded on 2026-11-10, and the current 2026-11-13 generators report 372 design-depth files / 77,064 lines and 369 Markdown files / 77,085 lines. The corpus grew as Phase 2/3 added docs, the spec-upgrade pass deepened packages 002-011, package 012 was added, the v1-review cleanup added EARS categories to 007/009/011, V1 planning added TASKS and packages 013-020, and later review updates expanded Feature 005/009 contracts and audit evidence. | stale | corrected in current status; see CURRENT-RUN.md |

## Cluster B — Domain contracts (DATA-MODEL, STATE-MACHINES, SCHEMAS, TASK-RUNTIME, TRUST, ENVIRONMENTS, DELEGATION, LEARNING)

| Doc + section | Claim | Code reality | Verdict | Register |
|---|---|---|---|---|
| [TASK-RUNTIME.md](TASK-RUNTIME.md) execution pipeline | Safe Task path: planning, Attempt admission, Environment, Trust, Effects, verification. | `apps/litecoworkd/src/agents/planning.rs` `require_dispatch()` is dead code and always errors; no production `admit_step_attempt` caller. Planning readiness returns `dispatch_available=false`. | aspirational-unlabeled (owner is TARGET; status-side already labels it) | A-06, A-07 |
| [STORAGE.md](STORAGE.md) migrations §"current schema version is 28" | The current schema version is 28; migration 28 is applied. | `crates/storage-sqlite/src/lib.rs:218` `SCHEMA_VERSION = 27`; last `include_str!` is `sqlite-v27.sql`; `sqlite-v28.sql` is not referenced. | contradicted | A-40 |
| [EVENTS.md](EVENTS.md) / [DELEGATION.md](DELEGATION.md) v28 references | "migration v28 seeds…" / "A pre-v28 TaskSpecRevision…" read as applied. | v28 unwired (above). | stale | A-40 |
| [DATA-MODEL.md](DATA-MODEL.md) / [STATE-MACHINES.md](STATE-MACHINES.md) learning vocabulary | **At the original audit baseline:** `MemoryCandidate`, `LearningCandidate` and `MemoryRecordMetadata.status` coexisted without an owner decision. | The source still has typed gates only and no store/service/route/producer. **Rechecked 2026-11-12:** A-20/A-34 owner-accepted `LearningCandidate` as the sole candidate lifecycle; implementation is absent and A-91 separately gates manual Coworker-memory metadata. | baseline finding corrected by owner decision; implementation gap remains | A-34, A-20, A-91 |
| [schemas/delegation.schema.json](schemas/delegation.schema.json) escalation_policy | Requires `escalation_policy.on_exhaustion` (RETURN_TO_LEAD\|NEEDS_YOU). | Rust `EscalationPolicy` carries only `max_worker_attempts`/`fallback_profile_ids`; no non-test producer. A-37 was owner-accepted 2026-11-09: this is an intentional TARGET-ahead-of-code field and packets may omit it until the dispatcher exists. | baseline finding resolved by owner decision; implementation gap remains | A-37 |
| [ENVIRONMENTS.md](ENVIRONMENTS.md) / [TRUST.md](TRUST.md) isolation | Trust/Environment/isolation modules mediate Task execution. | Modules compile as dead code; not wired into a qualified admission path (`attempt_process_scope.rs` staged). | aspirational-unlabeled (TARGET owner) | A-07 |
| [DATA-MODEL.md](DATA-MODEL.md) ContextDocument kinds | **At the original audit baseline:** four ContextDocument kinds were described as usable. | At that baseline the validator accepted only PERSONAL_PROFILE / WORKSPACE_NOTES and the OpenAPI create enum listed all four. **Rechecked 2026-11-12:** A-18 is owner-accepted; the create enum is now correctly narrowed to two while record/read and policy types retain all four. Coworker/Goal owner operations remain unserved. | baseline finding corrected at contract | A-18 |

## Cluster C — UI, Flows, Ledgers

| Doc + section | Claim | Code reality | Verdict | Register |
|---|---|---|---|---|
| [FLOWS.md](FLOWS.md) "Flow outputs and UI projection" index | Index enumerates the defined flows; F128 is distinct. | Index omitted F115-F124 and F136-F143 and duplicated F119's title under F128. | contradicted | A-38 (resolved here) |
| [FLOWS.md](FLOWS.md) / [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md) / [V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md) UIV1 citations | UIV1-001..158 are acceptance anchors. | Those IDs are defined only in `archive/legacy-implementation-c7e9c05/V1-DESIGN-COVERAGE.md`. | contradicted (dangling namespace) | A-39 (resolved here) |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) "Terminal current truth" | Terminal fabricates `exit code 0`, prints sample commands, labels pwsh/ConPTY/Job Object. | `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx` `TerminalPane` now renders "Terminal execution is unavailable"; fabrication removed. | stale | A-51 (new) |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) "Diff current truth" | Diffs synthesize additions/deletions and a fixed `+4/-1` badge. | `ReviewPane` renders "Nothing to review yet"; synthetic transform removed. | stale | A-51 (new) |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) "Browser current truth" | Browser iframe defaults to `http://127.0.0.1:4242/v1/health`. | `WorkbenchPanel.tsx` browser URL `useState("")` starts empty; comment "claims no containment". | stale | A-51 (new) |
| [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md) empty-chat canvas | Honest empty-state copy. | `conversations/ConversationsPage.tsx` now renders a truthful LiteCowork-branded empty state (no "Antigravity" branding, no unsupported capability claims); guarded by `tests/conversation-empty-state-truthfulness.test.mjs`. | resolved (code fix) | A-26 |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) Send gate | Send stays disabled until qualified. | `ConversationsPage` now gates Send via a `sendEligible` predicate that requires a qualified (selectable) AgentBinding plus draft/Operator/no-in-flight; the UI disabled-until-qualified gate is enforced (full OS/provider/auth/USER qualification still pending, tracked under A-04). | resolved (code fix; UI gate) | A-24 |

## Cluster D — Capability, LiteSPM, Connectors

| Doc + section | Claim | Code reality | Verdict | Register |
|---|---|---|---|---|
| [CAPABILITY-INVOCATIONS.md](CAPABILITY-INVOCATIONS.md) dispatch | CapabilityInvocation lifecycle dispatches providers. | Operator module is read-only GET; SQLite rejects DISPATCHED with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`; Trust evaluator dead code. | aspirational-unlabeled (TARGET owner) | A-33 |
| [CONNECTORS-WORKFLOWS.md](CONNECTORS-WORKFLOWS.md) §10.2 workflow graph | Core does not own a generic WorkflowGraph engine; invocation is provider-owned via a future adapter. | No in-Core graph schema/editor/run engine; no production `WorkflowBackendAdapter`. | true | A-17 |
| [CAPABILITY-FABRIC.md](CAPABILITY-FABRIC.md) / Discover connect | Connect/install creates a receipt and grants scope. | Discover `connect-or-install` returns `503 LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`; capability-test reports UNKNOWN; row actions disabled. | true | A-13, A-33 |
| [LITESPM-LITECOWORK-INTEROP-LLD.md](LITESPM-LITECOWORK-INTEROP-LLD.md) lifecycle | LiteSPM is the package-lifecycle authority; LiteCowork does not build a second inventory. | Managed/Connections tabs report unavailability; no second inventory writer found. | true | A-13 |

## Cluster E — Automation, Runtime, Mesh

| Doc + section | Claim | Code reality | Verdict | Register |
|---|---|---|---|---|
| [AUTOMATION.md](AUTOMATION.md) trigger host | Local TriggerHost polls cursors and settles recurring schedules; recurring hosting/`/resume` served. | `OperatorServer::start` runs `AutomationTriggerHostWorker` (30s poll). The `automation_operator.rs` header saying hosting/`/resume` are unavailable is stale. | stale (source comment) / true (behavior) | A-29 |
| [AUTOMATION.md](AUTOMATION.md) execution | A due/manual occurrence materializes a READY Task; it does not execute. | Manual Run / Create test Task create READY Tasks only; no planning/agent start. | true | A-12 |
| [NOTIFICATIONS / RESPONSIBILITIES] delivery | Notifications are delivered. | Settlement inserts PENDING `notification_deliveries` rows; no production sender/state-transition writer. | aspirational-unlabeled | A-27, A-28 |
| [RUNTIME-LIFECYCLE.md](RUNTIME-LIFECYCLE.md) / [LOCAL-OPERATOR-IPC.md](LOCAL-OPERATOR-IPC.md) readiness | Runtime READY / operatorReady describe startup/API health, not Task execution readiness. | Matches IMPLEMENTATION-STATUS.md "Runtime and Operator readiness". | true | A-02 |
| [AGENT-FABRIC.md](AGENT-FABRIC.md) relay | Heterogeneous A→B→A relay. | One OpenCode dispatch branch; `CodexAppServer` not wired to the Conversation supervisor; desktop message types drop producer identity. | aspirational-unlabeled | A-04 (relay subset) |

## SDD completeness gaps (specs/001-011)

Checked against [SPECIFICATION-STANDARD.md](SPECIFICATION-STANDARD.md) §4 (required SPEC
sections), §5 (blockers) and §10 (dangling references). The structural validator passes
11/11 packages (required files and all 19 section headers present); the gaps below are
depth and consistency gaps, not missing files. No DRAFT package is marked accepted by this
audit.

- **Thin HLD/LLD in every package except 001.** Packages 002-011 keep sections 6 (HLD),
  7 (LLD), 8-14 to a few sentences that defer to owner docs via links (for example
  `specs/004-agent-relay/SPEC.md` §6/§7 are two to three sentences each). This satisfies
  the "link to the owner, do not duplicate" rule but leaves no feature-specific component
  diagram, actor sequence or proposed-signature LLD. Each remains DRAFT; none is ready for
  IN_REVIEW on HLD/LLD depth alone. Package 001 (354 lines, 33 AC IDs) is the depth bar.
- **Unresolved blockers gate acceptance.** Every DRAFT package carries open §5 blockers
  that are owner decisions, not authoring gaps: B-11/B-12 (002), B-13/B-14 (003),
  B-15/B-16 (004), B-17/B-18 (005), B-19/B-20 (006), B-21/B-30/B-31 (007),
  B-22/B-23/B-32 (008), B-24/B-25 (009), B-26/B-27 (010), B-28/B-29/B-33 (011). None may
  be filled by implementation guesswork (SPECIFICATION-STANDARD §5, §7).
- **Dangling UIV1 reference in `specs/README.md`.** Its closing paragraph instructs that
  "UI contracts must cite the relevant UIV1 IDs." With the UIV1 namespace retired as an
  active acceptance authority (A-39), that instruction points at an archived catalog. The
  owning packages' UI contracts (e.g. `specs/001-.../contracts/ui-behavior.md`) should cite
  live flow IDs and feature AC IDs; the UIV1 citation is provenance only.
- **A-35 owner decision accepted (2026-11-09).** Feature 011 owns the Settings → Agents
  screen; Feature 002 owns only the agent operations it invokes. Feature 010 owns the
  notification quiet-hours control; Feature 011 links to it. The duplicate-control
  conflict is resolved at the owner level; implementation status stays separate.
- **A-20/A-34 vocabulary accepted (2026-11-09); implementation absent.** LearningCandidate
  is the one candidate lifecycle. No learning store/service/route/UI exists. A-91 is a
  separate open owner decision for manual Coworker-memory metadata; see the current
  contradiction register and Feature 009 package.

## What this re-audit changed

- Reconciled the known danglers at their owners: FLOWS.md flow index (A-38), the UIV1
  namespace (A-39) and the unwired `sqlite-v28.sql` (A-40), plus the CURRENT-RUN.md and
  IMPLEMENTATION-STATUS.md line-count drift.
- Filed one new contradiction, A-51 (Workbench Terminal/Diff/Browser status docs stale
  after the UI was remediated), and corrected the owner (IMPLEMENTATION-STATUS.md) and the
  affected rows A-08/A-09/A-10/A-30.
- No new file under `reading/` or `.zwork/`; no generated artifact was hand-edited.
