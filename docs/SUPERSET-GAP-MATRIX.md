# Superset gap matrix (Phase 3 cross-cutting view)

**Status:** cross-cutting gap view assembled from existing authorities — not a new owner contract and not qualification evidence.
**Assembled:** 2026-11-08 (Phase 3; follows the Phase 1 doc re-audit and the Phase 2 rival re-verification).
**Scope:** one matrix with one row per capability/feature, crossing LiteCowork's **real state** × **doc accuracy** × **per-competitor coverage**, plus the three derived lists the audit asked for.

This document does **not** redefine any concept and holds **no** parity authority. It is a reading view that links to the owners:

- **Parity verdicts** (TARGET / PARTIAL / DEFER-V2 / RESEARCH / NO) and the RV research gates stay in [VIABILITY-VS-COMPETITORS.md](VIABILITY-VS-COMPETITORS.md). This matrix cites those verdicts; it never restates or overrides them.
- **Concern → owner** routing stays in [COVERAGE-MATRIX.md](COVERAGE-MATRIX.md).
- **Doc-claim → code-reality → verdict** rows stay in [DOCS-REAUDIT-2026.md](DOCS-REAUDIT-2026.md); the "Doc accuracy" column links to its clusters and to the adjudicated [CONTRADICTION-AND-ALIGNMENT-REGISTER.md](CONTRADICTION-AND-ALIGNMENT-REGISTER.md) rows.
- **Source-backed real state** stays in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md).
- **Per-rival evidence** (pinned SHAs, feature surface, borrowable-file URLs, LICENSE) stays in [COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md) and [REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md) / [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## How to read the columns

**LiteCowork done?** uses the evidence vocabulary from [docs/README.md](README.md#evidence-vocabulary) / [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md#evidence-vocabulary) — TARGET / CODE / SERVED / INTEGRATED / QUALIFIED — compressed into three honest buckets:

- **Done** = SERVED or better and usable as a product journey today. Almost nothing reaches this bar because V1 is pre-release; where it applies it is a read/record surface, not a qualified end-to-end journey.
- **Partial** = CODE/SERVED exists but fails closed, is read-only, or is not a qualified journey (the common LiteCowork state).
- **Not-done** = TARGET only; no wired code path.

The cell carries the precise evidence state in parentheses so a SERVED-but-fail-closed route is never read as shipped.

**Doc claims it?** — does any LiteCowork owner doc assert the capability. **Doc accuracy** — whether that claim is `true` / `stale` / `over-claiming` (aspirational-unlabeled) / `contradicted`, linking the [DOCS-REAUDIT-2026.md](DOCS-REAUDIT-2026.md) cluster and the A-xx register row.

**Competitor columns.** `has` = the rival ships it to users today; `partial` = present but roadmap/limited/consent-gated; `no` = not in its first-release surface; `cloud` = the rival does it only as cloud execution (out of scope for local parity, DEFER-V2). Rival facts are **not** re-derived here — each is the Phase 2 evidence in [COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md) and [VIABILITY-VS-COMPETITORS.md](VIABILITY-VS-COMPETITORS.md).

### Resolved rival set (Phase 2) and column keys

The feature axis is the **union** of LiteCowork's own capability set and every resolved rival's first-release surface, so the matrix expresses a **max superset**. The resolved rivals (columns, keyed to keep the table readable) are:

| Key | Rival | Phase 2 evidence |
|---|---|---|
| CCD | Claude Code Desktop (Code tab) | [VIABILITY §Claude Code Desktop](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §2026-11-08 Claude Code Desktop vs Cowork](COMPETITIVE-RESEARCH.md) |
| CCo | Claude Cowork / Claude chat | [VIABILITY §Claude Cowork](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §Claude Cowork](COMPETITIVE-RESEARCH.md) |
| CDu | Claude Duet (co-edit; first-party URL unresolved, RV-14) | [VIABILITY §Claude Duet](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §Claude Duet (2026-11-08)](COMPETITIVE-RESEARCH.md) |
| Dots | OpenAI dots (GPT-6.1 Terra) | [VIABILITY §OpenAI dots](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §OpenAI dots](COMPETITIVE-RESEARCH.md) |
| CGW | ChatGPT Work / Codex | [VIABILITY §OpenAI dots / ChatGPT Plus / Work](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §ChatGPT Work](COMPETITIVE-RESEARCH.md) |
| Muse | Meta Muse assistant | [VIABILITY §Muse](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §Meta Muse](COMPETITIVE-RESEARCH.md) |
| Grok | Grok Bot (xAI; Cursor marketplace bundling) | [VIABILITY §Grok Bot](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §Grok Bot](COMPETITIVE-RESEARCH.md) |
| OWk | OpenWork (different-ai/openwork) | [VIABILITY §OpenWork](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §OpenWork](COMPETITIVE-RESEARCH.md), REF-OPENWORK |
| OCo | Open Cowork (opencoworkai/open-cowork) | [VIABILITY §OpenWork / OpenCowork](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §Open Cowork](COMPETITIVE-RESEARCH.md), REF-OPENCOWORK |
| ZC | ZCode desktop (zai-org/ZCode) | [VIABILITY §ZCode / DeepSeek Harness](VIABILITY-VS-COMPETITORS.md), REF-ZCODE |
| DSH | DeepSeek Harness desktop (deepseek-ai/deepseek-harness) | [VIABILITY §ZCode / DeepSeek Harness](VIABILITY-VS-COMPETITORS.md), REF-DEEPSEEK-HARNESS |
| OBot | OpenBot (nightly-labs/openbot) | [VIABILITY §OpenBot](VIABILITY-VS-COMPETITORS.md), [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md), REF-OPENBOT |
| Rbt | Rowboat (rowboatlabs/rowboat) | [VIABILITY §Rowboat / OpenLoomi](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §Rowboat (2026-11-08)](COMPETITIVE-RESEARCH.md), REF-ROWBOAT |
| OLm | OpenLoomi (melandlabs/openloomi) | [VIABILITY §Rowboat / OpenLoomi](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §OpenLoomi (2026-11-08)](COMPETITIVE-RESEARCH.md), REF-OPENLOOMI |
| ODot | OpenDot (athulsreekumar/opendot, opendot.live) | [VIABILITY §OpenDot](VIABILITY-VS-COMPETITORS.md), [COMPETITIVE-RESEARCH §OpenDot (opendot.live)](COMPETITIVE-RESEARCH.md), REF-OPENDOT |

Non-resolved look-alikes (`itsperini/opendot`, `CopilotKit/OpenDots`, `composio-community/open-dot`, `CopilotKit/openbot`) are intentionally excluded; see the disambiguation tables in [COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md).

---

## The superset matrix

Feature axis rows are grouped by the capability ladder L0–L9 defined in [VIABILITY-VS-COMPETITORS.md §Capability ladder](VIABILITY-VS-COMPETITORS.md). Competitor cells: `has` / `partial` / `no` / `cloud`.

| Feature (ladder) | LiteCowork done? (evidence) | Doc claims it? | Doc accuracy (re-audit / A-xx) | CCD | CCo | CDu | Dots | CGW | Muse | Grok | OWk | OCo | ZC | DSH | OBot | Rbt | OLm | ODot |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Ordinary chat without a Coworker/Task (L0) | **Partial** (SERVED; Send violates disabled-until-qualified gate) | yes | contradicted ([Cluster C](DOCS-REAUDIT-2026.md#cluster-c--ui-flows-ledgers), A-24) | has | has | has | has | has | has | has | has | has | has | has | has | has | has | has |
| Persistent named specialist identity (Coworker/Bot/Dot) (L0) | **Partial** (SERVED CRUD/revision/status; no Coworker-owned Conversation, no scoped memory consumer) | yes | true (RV-12; status-side labels partial) | has | has | no | has | has | has | has | partial | partial | no | no | has | has | partial | has |
| One native agent turn actually executes (L0/L8) | **Partial** (INTEGRATED code path: OpenCode v2.0.32 only; not QUALIFIED) | yes | over-claiming at relay ([Cluster A](DOCS-REAUDIT-2026.md#cluster-a--product-architecture-adrs), A-04) | has | has | has | has | has | has | has | has | has | has | has | has | has | partial | has |
| BYO / interchangeable native harnesses (multi-provider) (L0/L8) | **Partial** (SERVED registry/probe/binding; only OpenCode dispatches, Codex not wired) | yes | over-claiming (relay subset, A-04) | partial | no | no | no | no | no | no | has | has | partial | partial | has | partial | partial | partial |
| Local files / Workspace roots + grounded retrieval (L1) | **Partial** (SERVED roots/resources; Linux-only scan, retrieval not wired to send) | yes | true (Cluster D owners) | has | has | no | has (local bridge) | has | has | partial | has | has | partial | partial | partial | has | partial | has |
| Immutable Artifacts / generated documents (L1) | **Partial** (SERVED text preview/edit; no create route, no professional-format engine) | yes | true (status labels target-only; B-21/B-30/B-31) | has | has | no | has | has | has | partial | has | has | partial | partial | partial | has | partial | partial |
| Professional doc formats (PPTX/DOCX/XLSX/PDF out) (L1) | **Not-done** (TARGET; B-21) | yes (as target) | true | partial | has | no | has | has | has | no | partial | has | no | no | no | partial | no | no |
| Browser / computer use (real automation) (L2) | **Not-done** (iframe preview only; RV-04, B-22) | yes (target) | true (status: iframe not Chrome; A-09/A-51) | has | has | no | has (cloud browser) | has | has | has | partial | has | no | no | partial | partial | partial | partial |
| Integrated terminal with real process receipts (L2) | **Not-done** (display-only stub; RV-OpenHands row, B-23) | yes (target) | true (status: unavailable; A-08/A-30/A-51) | has | partial | no | partial | partial | no | partial | partial | has | has | has | partial | partial | partial | no |
| Workbench / panes in one shell (chat/diff/browser/terminal/editor) (L2/L9) | **Partial** (SERVED Files projection; other panes honest-unavailable) | yes | true (A-51 remediation recorded) | has | has | no | partial | partial | partial | partial | partial | has | has | has | has | has | partial | partial |
| Connected apps / MCP / skills / plugins (L3) | **Partial** (SERVED Discover projections; connect/install 503, invocation read-only) | yes | true (Cluster D, A-13, A-33) | has | has | no | has | has | has | has | has | has | partial | partial | has | has | partial | has |
| In-Core visual workflow graph + graph runtime (L3) | **Not-done (non-goal for Core)** | documented as NO / external-only | true (Cluster D, A-17) | no | no | no | no | no | no | no | partial | no | no | no | has | no | no | no |
| Permissions / approvals / isolation / audit (Trust) (L4) | **Partial** (CODE modules dead; not on a qualified admission path; RV-05) | yes | over-claiming (Cluster B/D, A-06/A-07/A-33) | has | has | no | has | has | has | partial | partial | has | partial | partial | partial | partial | partial | has |
| RO/RW/RW-no-delete root mount modes (L4) | **Not-done** (TARGET; RV-05 detail) | yes (target) | true | has | has | no | partial | partial | has | no | partial | has | no | no | no | partial | no | partial |
| Memory + knowledge continuity (L5) | **Not-done** (typed records only; no learning service/store/UI) | yes | target accepted; manual metadata gate A-91 | has | has | no | has | has | has | has | partial | partial | no | no | partial | has | partial | has |
| Reusable Routines/Skills + teach-to-skill (L6) | **Not-done** (TARGET; RV-06) | yes (target) | true | partial | has | no | has | partial | partial | has | has | has | no | no | partial | partial | no | has |
| Scheduled tasks / automations with honest local/cloud columns (L6/L7) | **Partial** (SERVED local 30s TriggerHost; materializes READY only, no execution) | yes | true (Cluster E, A-12, A-29; RV-13) | has | has | no | has | has | partial | has | has | partial (cron roadmap) | no | no | has | partial | partial | has |
| Event/condition-driven proactivity + background work (L7) | **Partial** (filters/admission CODE; no execution; ON_CONDITION blocked) | yes | aspirational-unlabeled (Cluster E, A-27/A-28) | partial | has | no | has | has | has | partial | partial | partial | no | no | partial | has | has | has |
| Multi-agent / Coworker delegation + handoff (L8) | **Partial** (storage admission CODE; no production dispatch caller) | yes | over-claiming (relay, A-04; RV-07) | partial | has | no | has | has | partial | has | partial | partial | no | no | has | has | partial | has |
| Synchronous human+agent co-edit / take-control (L8) | **Not-done** (map to Conversation UX; RESEARCH RV-14; now owned by DRAFT [specs/012](../specs/012-conversation-coedit-take-control/README.md)) | target (DRAFT spec 012, B-34 PROPOSED) | n/a (gap now authored as DRAFT target in [specs/012](../specs/012-conversation-coedit-take-control/README.md)) | partial | partial | has | no | no | no | no | no | no | no | no | no | partial | no | no |
| Verification / recovery / evidence-bound completion (L9) | **Not-done** (no accepted-plan→Attempt→Effect→verify pipeline) | yes | over-claiming (Cluster B, A-06/A-07) | partial | partial | no | partial | partial | partial | partial | partial | partial | no | partial | partial | partial | partial | partial |
| "Needs You" attention/triage projection (L9) | **Partial** (projection target; presence route 503) | yes | true (status: presence target-only) | partial | partial | no | has | has | has | partial | partial | partial | no | no | has | has | has | has |
| Local-first daemon owns durable coordination (runtime split) (L7) | **Done (pattern)** (SERVED Operator vs daemon; READY ≠ task-ready) | yes | true (Cluster E, A-02) | partial | partial | no | no | no | no | no | has | has | has | has | has | has | has | has |
| Cloud computer runs while laptop is off (out-of-scope) | **Not-done (documented non-goal)** | documented as DEFER-V2 / NO | true | cloud | cloud | no | cloud | cloud | cloud | cloud | no | no | no | no | no | cloud | no | cloud |
| Cross-device chat/task sync (out-of-scope) | **Not-done (documented non-goal)** | DEFER-V2 | true | cloud | cloud | no | cloud | cloud | no | no | no | no | no | no | no | no | no | partial |
| Mobile / WhatsApp as first-class agent surface (out-of-scope) | **Not-done (documented non-goal)** | DEFER-V2 | true | partial | partial | no | partial | partial | has | no | no | partial | no | no | no | no | no | no |
| Voice / realtime audio agent (L2/L3) | **Not-done** (DEFERRED; RV-08) | yes (deferred) | true | no | no | no | partial | no | partial | partial | no | partial | no | no | no | partial | partial | partial |
| Screen-understanding / audio capture triage | **Not-done (consent-gated RESEARCH/NO-default)** | yes (research) | true (RV / OpenLoomi row) | no | no | no | no | no | partial | no | no | partial | no | no | no | no | has | no |

Reading note: no LiteCowork row is **Done** as a *qualified product journey*; the single "Done (pattern)" entry is the architectural runtime split, which is SERVED and real but is an invariant, not a user feature. Every other LiteCowork cell is **Partial** or **Not-done**, consistent with the pre-release posture in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md). This matrix therefore understates nothing: a `has` against a LiteCowork **Partial/Not-done** is a real gap, tracked in list 3 below.

---

## List 1 — ✓ Done / ✗ Not done (LiteCowork's real state)

Grounded in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) and [DOCS-REAUDIT-2026.md](DOCS-REAUDIT-2026.md). "Done" here means *a real, served, source-backed surface exists* — never that the journey is release-qualified (V1 is pre-release).

**✓ Real, served surfaces (Done at SERVED/CODE evidence; not QUALIFIED):**

- Local Operator vs daemon runtime split; RuntimeState READY / operatorReady health (SERVED). Runtime health ≠ Task-execution readiness.
- Conversation create / snapshot / message pagination / drafts / agent-binding selection / Send route; one real OpenCode v2.0.32 server/session/turn branch (SERVED + code-level INTEGRATED for OpenCode only).
- Conversation-level agent-binding switch `PATCH .../agent-binding` with `conversation.agent_binding.changed.v1` (SERVED).
- Workspace + Resource routes: quick import, chunked upload, revision commit, content read, list/detail/search, text-index rebuild, grounded retrieval (read-only), WorkspaceRoot list/pause/resume/revoke/scan (Linux) (SERVED).
- Coworker CRUD / immutable revision / status / Workspace primary selection (SERVED).
- Task create/list/get, TaskSpec revision, read-only planning-readiness (`dispatch_available=false`) (SERVED).
- Capability Discover projections and durable Approval decisions (SERVED).
- Artifact managed text/plain edit/append and safe read-only previews (SERVED).
- Local 30-second AutomationTriggerHost that settles supported recurring schedules and materializes READY Tasks (SERVED).
- Agent Registry list/refresh, local installation inventory, Codex/OpenCode profile probes, AgentBinding CRUD (SERVED).

**✗ Not done (TARGET only or fails closed; not a usable journey):**

- Safe Task execution pipeline: planning dispatch, accepted plan, Attempt admission, Environment assignment, Trust-mediated Effect, reconciliation, independent verification. `require_dispatch()` is dead code; no production `admit_step_attempt` caller.
- Heterogeneous A→B→A agent relay (only OpenCode dispatches; Codex is not wired to the supervisor; producer identity is now preserved and rendered, but no qualified second dispatch path/cross-layer relay exists) — B-15/B-16.
- Coworker-owned Conversation, Coworker-scoped retrieval, private memory browser / Learning Inbox; presence returns 503 — B-24.
- Learning/memory service: no store, schema/migration, service, route, event producer, or UI (typed pure-function gates only). A-20/A-34 vocabulary is owner-accepted; implementation remains open. A-91 gates manual Coworker-memory metadata.
- Real browser/computer use (iframe preview only) — RV-04, B-22; real integrated terminal (display-only stub) — B-23; Computer-use decision — B-32.
- Professional Artifact format engine and Artifact create route / export receipt — B-21, B-30, B-31.
- LiteSPM package connect/install/lifecycle (connect-or-install returns 503) and CapabilityInvocation dispatch — A-13, A-33, RV-01.
- Notification delivery (settlement inserts PENDING rows; no sender) — A-27, A-28, B-27.
- Trust/Environment isolation on a qualified admission path (modules compile as dead code) — A-06, A-07, RV-05.
- Settings scope: four mounted categories vs the 16-category target; several controls ephemeral/inert — B-29.
- Send remains enabled against the disabled-until-qualified gate — A-24 (a defect, not a feature).

---

## List 2 — 📝 What is wrong in the docs (stale / over-claiming / contradicted / missing)

Each row cross-references the [DOCS-REAUDIT-2026.md](DOCS-REAUDIT-2026.md) verdict cluster and the adjudicated [CONTRADICTION-AND-ALIGNMENT-REGISTER.md](CONTRADICTION-AND-ALIGNMENT-REGISTER.md) row. Rows below preserve the original audit findings and record whether a later owner decision or correction resolved them; “baseline” does not mean the issue remains open.

**Contradicted (now corrected at owner):**

- Baseline `STORAGE.md` "current schema version is 28" vs source `SCHEMA_VERSION = 27`; v28 unreachable — Cluster B, **A-40** (resolved and later implemented: v28 Workspace settings are wired; the current chain reaches v36, including v34 Task-policy snapshot enrichment, v35 Runtime-local auth observations and v36 credential-submission recovery receipts. Earlier v27/v34 labels are historical and are corrected in current owner/status docs).
- `FLOWS.md` flow index omitted F115–F124, F136–F143 and duplicated F119 as F128 — Cluster C, **A-38** (resolved: index now covers F00–F143).
- UIV1-001..158 cited as acceptance anchors but defined only in the archived legacy coverage doc — Cluster C, **A-39** (resolved: UIV1 retired as active authority, provenance-only).
- Baseline A-18 finding: ContextDocumentKind has four values while creation accepted only PERSONAL_PROFILE / WORKSPACE_NOTES. Owner accepted narrowing the create enum; current OpenAPI create metadata now has two kinds while record/read and policy types retain four. Coworker/Goal owner paths remain unserved.
- Baseline A-37 finding: `on_exhaustion` is in the accepted delegation target schema but is not yet represented by the Rust producer; omission remains valid until the dispatcher is implemented.
- Baseline A-20/A-34 finding: multiple learning names coexisted. The candidate vocabulary is now owner-accepted as LearningCandidate; no learning service/store/UI is implemented, and A-91 separately gates manual Coworker-memory metadata.
- `V1-UI-SCREEN-CONTRACT`/`ConversationsPage` empty canvas still renders "I am Antigravity…" branding — Cluster C, **A-26** (still open).
- Send stays disabled until qualified vs UI enabling Send for any non-empty draft — Cluster C, **A-24** (still open; code defect).

**Stale (described earlier source; now corrected):**

- IMPLEMENTATION-STATUS "Terminal / Diff / Browser current truth" described the old fabricating Workbench after the UI was remediated — Cluster C, **A-51** (new; corrected, and rows A-08/A-09/A-10/A-30 updated).
- EVENTS.md / DELEGATION.md v28 references read as applied — Cluster B, **A-40** (stale consequence of the v28 contradiction).
- `automation_operator.rs` header saying recurring hosting/`/resume` are unavailable while the worker actually runs — Cluster E, **A-29** (resolved by T02 at CODE level on 2026-11-13; UI copy and test are aligned, with broader AC-019 pagination still open under T11).
- IMPLEMENTATION-STATUS "Active documentation corpus" line counts (258/54,473; 255/54,385) vs generators — Cluster A (corrected in place); the matching CURRENT-RUN.md drift reconciled.

**Over-claiming (aspirational-unlabeled — TARGET reading as shipped):**

- `TASK-RUNTIME.md` safe Task pipeline reads as operative; `require_dispatch()` is dead code — Cluster B, **A-06**, **A-07**.
- `ENVIRONMENTS.md` / `TRUST.md` isolation "mediate Task execution" while modules are dead code — Cluster B, **A-07**.
- `CAPABILITY-INVOCATIONS.md` dispatch lifecycle while the module is read-only GET and dispatch is admission-blocked — Cluster D, **A-33**.
- `AGENT-FABRIC.md` heterogeneous A→B→A relay vs one OpenCode branch — Cluster E, **A-04** (relay subset).
- Notification delivery presented as delivered vs PENDING-only rows — Cluster E, **A-27**, **A-28**.

**Missing (capability a rival ships but no LiteCowork owner documents as even a target):**

- Synchronous human+agent **co-edit / take-control** surface (Claude Duet). Tracked as research **RV-14**; it was the one matrix row with "Doc claims it? = no". **Now owned** by the DRAFT feature package [specs/012-conversation-coedit-take-control](../specs/012-conversation-coedit-take-control/README.md) (owner decision **B-34**, PROPOSED), which maps co-edit onto an ordinary Conversation take-control / side-chat UX and creates no new durable-Task type. Claude Duet stays cited as RESEARCH (first-party URL unresolved), not asserted as fact.

---

## List 3 — 🎯 Missing to be a max superset of all competitors' first releases

Each item names the rival(s) that have it, the LiteCowork parity verdict (owned by [VIABILITY-VS-COMPETITORS.md](VIABILITY-VS-COMPETITORS.md)), the owner decision / research gate id, and — where a borrowable implementation exists — the Phase 2 exact-file URL and LICENSE note from [REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md). No new tracker ids are invented; each gap reuses an allocated **B-xx** / **RV-xx**.

1. **Real integrated terminal with ordered command receipts (exit status, timeout, truncation).** Rivals: ZC, DSH, OWk (`has`); CCD. Verdict TARGET. Gate **B-23**. Borrow: OpenHands shell receipt doctrine (REF-OPENHANDS, `OpenHands/software-agent-sdk` MIT); DeepSeek Harness `apps/desktop` typed-IPC quit-drain (REF-DEEPSEEK-HARNESS, MIT); ZCode `packages/desktop` thin-shell/Host split (REF-ZCODE, Apache-2.0 + NOTICE). Read-first paths in [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).
2. **Real browser / computer use (CDP automation, isolated profile, takeover lease).** Rivals: CCD, CCo, Grok, OCo (`has`); Dots (cloud browser). Verdict TARGET. Gate **RV-04**, **B-22**; Computer-use decision **B-32**. Borrow: `browser-use/browser-harness` `src/browser_harness/{daemon,helpers,_ipc,admin}.py` (MIT) and `browser-use/browser-use` `dom/serializer/*`, `watchdogs/recording_watchdog.py` (MIT) — ambient-CDP discovery and profile-copy must be disabled per the adapter rules in [REFERENCE-IMPLEMENTATIONS.md §Browser Use](REFERENCE-IMPLEMENTATIONS.md).
3. **Professional document formats out (PPTX/DOCX/XLSX/PDF).** Rivals: CCo, Dots, CGW, Muse, OCo (`has`). Verdict TARGET. Gate **B-21** (format engine), **B-30** (create route), **B-31** (export receipt). Borrow: Open Cowork document-skills surface (REF-OPENCOWORK, MIT) as a UX/skill reference — keep LiteCowork Artifact/Evidence identity.
4. **Memory + knowledge continuity (scoped private memory + source-backed Knowledge; Learning Inbox).** Rivals: CCD, CCo, Dots, CGW, Muse, Grok, Rbt, ODot (`has`). Verdict TARGET/PARTIAL. B-24 is accepted; manual-memory metadata remains gated by **A-91**. Automatic candidate vocabulary **A-20/A-34** is accepted, but Feature 020's service/storage/UI remain unimplemented. Borrow: Rowboat living knowledge-graph-as-context (REF-ROWBOAT, Apache-2.0) — map to quiet scoped Memory + Knowledge, not an auto-indexing crawler (RV-06/RV-12 area).
5. **Qualified native-agent turn + interchangeable harnesses + A→B→A relay.** Rivals: all ship a working turn; OWk/OCo/OBot interchangeable providers (`has`). Verdict PARTIAL. Gate **RV-02** (first native tuple), **RV-03** (atomic turn write), **RV-07** / **B-15** / **B-16** (relay/portable-context). Borrow: OpenWork `search_capabilities`/`execute_capability` lazy gateway and `packages/automations` readable-after-stop doctrine (REF-OPENWORK, MIT except `/ee`).
6. **LiteSPM package connect/install/lifecycle + CapabilityInvocation dispatch (MCP/skills/connectors).** Rivals: CCD, CCo, Dots, CGW, OWk, OCo, OBot, ODot (`has`). Verdict TARGET. Gate **RV-01**, **A-13**, **A-33**. Borrow: LiteSPM upstream contract (REF-LITESPM, Apache-2.0); Arcade MCP tool declarations (REF-ARCADE-MCP, MIT) — optional provider only.
7. **Trust / approvals / isolation / audit on a qualified admission path, with RO/RW/RW-no-delete mount modes.** Rivals: CCD, CCo, Dots, CGW, Muse, OCo, ODot (`has`/mount modes). Verdict PARTIAL. Gate **RV-05**. Borrow: Claude Cowork mount-mode UX (COMPETITIVE-RESEARCH §Claude Cowork; Anthropic containment, vendor doc — reference/UX study, not code reuse).
8. **Scheduled tasks with honest local/cloud/session truth columns + per-run worktree isolation.** Rivals: CCD, Dots, CGW, OWk, Grok, ODot (`has`). Verdict TARGET (local) / DEFER-V2 (cloud). Gate **RV-13**. Borrow: OpenCowork schedule/local-condition-checker (REF-OPENCOWORK, MIT; cron is still roadmap, not shipped).
9. **Reusable Routines/Skills + teach-to-skill from recording.** Rivals: CCo, Dots, Grok, OWk, OCo, ODot (`has`). Verdict RESEARCH. Gate **RV-06**. Borrow: OpenWork `codemode` code-step package (REF-OPENWORK, MIT) as a reusable-work reference.
10. **Event/condition-driven proactivity + "Needs You" triage.** Rivals: CCo, Dots, CGW, Muse, Rbt, OLm, ODot (`has`). Verdict TARGET (UX)/PARTIAL. Gate **RV-07** (relay), ON_CONDITION owner decision in Automations (**B-26**). Borrow: OpenLoomi attention/triage surface and lifecycle Rust modules (REF-OPENLOOMI, Apache-2.0, same Tauri+Rust stack); OpenDot urgent/update/quiet notification levels (REF-OPENDOT, MIT).
11. **Notification delivery (actually send, honor channel/quiet-hours).** Rivals: Dots, CGW, Muse, ODot (`has`). Verdict TARGET. Gate **B-27**; ties to **A-27**/**A-28**.
12. **Synchronous human+agent co-edit / take-control.** Rival: CDu (`has`); CCD side-chat (`partial`). Verdict RESEARCH. Gate **RV-14** — first-party Duet URL unresolved 2026-11-08; map onto a Conversation co-edit UX, do not assert as fact or create a new Task type. **Now owned** by the DRAFT package [specs/012-conversation-coedit-take-control](../specs/012-conversation-coedit-take-control/README.md) (owner decision **B-34**, PROPOSED), mapping take-control onto the Feature 001 cancel/quiescence path and a correlated side-chat. Borrow: OpenLoomi attention/presence Rust modules (REF-OPENLOOMI, Apache-2.0, same Tauri+Rust stack) for the truthful driver indicator; OpenDot notification levels (REF-OPENDOT, MIT); Claude Duet / Claude Code Desktop side-chat are vendor UX references only (no code reuse).
13. **Screen-understanding / audio-capture triage (consent-gated).** Rival: OLm (`has`), Muse (`partial`). Verdict RESEARCH / NO-as-default. Needs a privacy/permission owner before any target; never a default capability.
14. **Voice / realtime audio agent.** Rivals: Dots, Muse, Grok, ODot (`partial`). Verdict DEFERRED. Gate **RV-08**.

Explicitly **not** pursued for parity (documented non-goals, not gaps): cloud computer while the laptop is off, cross-device sync as primary UX, mobile/WhatsApp first-class surface, shared cloud filesystem between Coworkers, universal subscription-agnostic model router, a hidden second planner, an embedded workflow engine as Core, and any qualification claim without OS/harness/provider USER evidence. See [VIABILITY-VS-COMPETITORS.md §LiteCowork cannot match](VIABILITY-VS-COMPETITORS.md) and §8 of the non-goals list.

---

## Headline gap count

- **14** capability gaps to reach a max superset of all resolved rivals' first releases (List 3), of which **8** are already TARGET-owned (B-xx / RV-xx allocated) and **6** are RESEARCH/owner-decision gates (RV-06, RV-07, RV-08, RV-13, RV-14, plus the consent-gated screen/audio decision).
- **~18** doc-accuracy findings (List 2) across stale / over-claiming / contradicted / missing, every one cross-linked to a DOCS-REAUDIT-2026 cluster and an A-xx row; 2 remain open code-side defects (A-24 Send gate, A-26 empty-canvas branding).
- **0** LiteCowork rows reach **Done** as a qualified product journey; the lone "Done (pattern)" is the runtime split invariant. This is consistent with the pre-release posture in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md).

No parity verdict, RV gate, A-xx adjudication, or evidence state is created here. Corrections live at the owners; this matrix only reads across them.
