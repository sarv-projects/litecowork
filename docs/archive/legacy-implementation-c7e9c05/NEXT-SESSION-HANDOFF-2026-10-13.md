# Next-session handoff — LiteCowork desktop/local V1 — refreshed 2026-10-16

This is a current-worktree handoff for continuing the active LiteCowork desktop/local V1
objective. It is navigation and evidence, not a replacement for current contracts, source,
tests, or `implementation/backlog.json`. Recheck live branch/status and the latest
`implementation/CURRENT-RUN.md` entries before acting; older dated records below remain
historical where explicitly stated.

## Current branch and preservation boundary

- Workspace: `/home/sarvesh/business_Dev/litecowork`.
- At this refresh, `main`, `HEAD`, and `origin/main` are all
  `de73535cf27af347d9d5413ccaffc81bef2e78be`. The working tree is heavily modified and
  contains both staged and unstaged work. The staged E02-S03 index is a distinct patch;
  many source, contract, UI, documentation, test, and generated-file changes remain
  unstaged. Preserve the index/worktree split. Do not reset, clean, stash, checkout away,
  overwrite, or broadly stage any owner work. The owner has authorized exact-path milestone
  commits and pushes; inspect both diffs and stage only reviewed paths.
- E02-S03 is the active integration slice. Its staged patch currently records Resource
  index/search/GC and ContextDocument purge/migration work; the surrounding working tree
  includes independent UI, Runtime, agent-catalog and audit changes. Reconcile ownership
  and evidence before staging. Keep E02-S03 `IN_PROGRESS` until all CODE, SYSTEM and owner
  USER gates in its story are met.
- Read [CURRENT-RUN](CURRENT-RUN.md), the exact backlog/epic and review packet for the
  latest per-slice commands/results. Never infer that dirty working-tree evidence is in
  `HEAD` or already pushed.

## Current audit and resume priority — 2026-10-16

The source-backed [implementation re-audit](V1-IMPLEMENTATION-REAUDIT-2026-10-13.md)
remains the defect and qualification map. Its A01–A13 findings have source-level repairs
and regression coverage in the current repository history/worktree, but inspect each
current staged/unstaged patch and its exact test record before treating it as integrated.
Recent fixes include the Resource editor's retained-draft fencing, RichPresentation
invalidation/loading recovery, Workspace-fenced Conversation creation and durable
create-request identity, Conversation pagination, stale Resource response fencing, Needs
You pagination recovery, Task-create replay after default-agent removal, 200-byte
idempotency validation, and receipt-conflict error classification. The current tree also
contains a RichPresentation invalidation change awaiting lead review/integration. None of
these source repairs alone qualifies packaged transport, native provider behavior, OS
support, or owner acceptance.

Resume the staged E02-S03 slice first, then re-evaluate the dependency-ready frontier from
`implementation/backlog.json`, the exact E02 story and current source. Do not return to the
old storage-only E01-S02 frontier from the original 2026-10-13 handoff. The active E02-S03
work includes ContextDocument purge/recovery, v17–v19 Resource recovery/index migrations,
and bounded Resource-index search. Its latest integrated storage run passed **303/303**
`storage-sqlite` tests; the authenticated daemon purge restart/replay test passed **1/1**;
targeted Clippy passed with the repository's recorded `too_many_arguments` and dead-code
allowances. Linux WSL2 x86_64, Rust/Cargo `1.98.1`. These are local CODE/Unix daemon
restart evidence only. Packaged Tauri, production key provider, non-Linux OS, abrupt crash or
power-loss, external-provider deletion, and owner USER acceptance remain pending.

Documentation validation needs one final rerun against the staged contract snapshot. The
strict contract and Markdown audits passed, but the first generated-coverage run rejected
two target-only WorkspaceRoot OpenAPI operations as missing served-route prose. The staged
coverage validator now recognizes explicit `Target only operation` markers and has a
regression test; coverage regeneration plus plan, architecture, design-depth, owner
consistency and diff checks must be rerun sequentially and their results recorded. A
validator pass proves contract consistency, not product behavior.

The U15/U16 memory traceability is clarified in `WORKFLOWS.md`: U15 is the
ContextDocument concurrent-edit/revoke/delete case; U16 is Goal progress based only on
verified Task outcomes. Neither is an automatic-learning test. The accepted quiet,
scoped private-learning target belongs to E08-S03 and its separate supplemental
UIV1-120–UIV1-127 acceptance family; it remains unimplemented. See
[Learning](../docs/LEARNING.md), [E08](epics/E08.md), and
[TESTING](TESTING.md). Do not claim that U15/U16 or the scoped-memory design crosswalk
proves LearningCandidate/MemoryRecord behavior.

## Complete desktop/local V1 backlog snapshot

Status and dependency columns below reproduce `implementation/backlog.json` as read on
2026-10-16. The backlog remains the status authority; a PLANNED or
IN_PROGRESS label is not proof of source implementation, provider qualification, or
acceptance. Desktop/local V1 comprises E01–E10 and E13 (60 stories: 1 `IN_REVIEW`, 19
`IN_PROGRESS`, 40 `PLANNED`, and none `ACCEPTED`). E11–E12 are post-V1.

### E01 — Development foundation

| Story | Status | Depends on | Story |
|---|---|---|---|
| E01-S01 | IN_REVIEW | — | Contract/toolchain baseline |
| E01-S02 | IN_PROGRESS | E01-S01 | Transactional persistence |
| E01-S03 | IN_PROGRESS | E01-S02 | Independent Runtime lifecycle |
| E01-S04 | IN_PROGRESS | E01-S03 | Authenticated Operator boundary |

### E02 — Desktop shell and Workspace foundation

| Story | Status | Depends on | Story |
|---|---|---|---|
| E02-S01 | IN_PROGRESS | E01-S04 | Desktop shell/onboarding |
| E02-S02 | IN_PROGRESS | E02-S01 | Workspace policy and instructions |
| E02-S03 | IN_PROGRESS | E02-S02 | Resource intake and factual index |
| E02-S04 | IN_PROGRESS | E02-S03 | Needs You and truthful UI state |

### E03 — Native agents, conversations, and execution

| Story | Status | Depends on | Story |
|---|---|---|---|
| E03-S01 | IN_PROGRESS | E02-S04 | Harness qualification and bindings |
| E03-S02 | PLANNED | E03-S01 | Conversation turns |
| E03-S03 | IN_PROGRESS | E03-S02 | Planning and atomic plan acceptance |
| E03-S04 | PLANNED | E03-S03, E07-S00 | Attempt execution and recovery |
| E03-S05 | PLANNED | E03-S04 | Native feature conformance |
| E03-S06 | PLANNED | E03-S01 | AgentModule registry and ACP discovery |
| E03-S07 | PLANNED | E03-S06 | Agent install, update and repair lifecycle |
| E03-S08 | PLANNED | E03-S07 | AgentControlDescriptor and versioned binding configuration |
| E03-S09 | PLANNED | E03-S08 | Native authentication, API-key slots and native configuration |
| E03-S10 | PLANNED | E03-S09, E03-S02 | Agent-adaptive composer and native input affordances |
| E03-S11 | PLANNED | E03-S05, E03-S10 | Per-agent lifecycle and native-surface conformance packs |

### E04 — Trust, Gateway, effects, and evidence

| Story | Status | Depends on | Story |
|---|---|---|---|
| E04-S01 | PLANNED | E03-S05 | Trust evaluation and credential boundary |
| E04-S02 | PLANNED | E04-S01 | Gateway and progressive discovery |
| E04-S03 | PLANNED | E04-S02 | Invocation/Effect reconciliation |
| E04-S04 | PLANNED | E04-S03 | Real LiteSPM and MCP conformance |
| E04-S05 | PLANNED | E04-S04 | Artifact/Evidence/verifier completion |
| E04-S06 | PLANNED | E04-S04, E03-S11 | Cross-agent LiteSPM capability bridges |

### E05 — Delegation

| Story | Status | Depends on | Story |
|---|---|---|---|
| E05-S01 | IN_PROGRESS | E04-S05 | DelegationProfile lifecycle and UI |
| E05-S02 | PLANNED | E05-S01 | Child admission and isolation |
| E05-S03 | PLANNED | E05-S02 | Routing and measured cost |
| E05-S04 | PLANNED | E05-S03 | Verification escalation and lead failover |
| E05-S05 | PLANNED | E05-S04 | Harness breadth and efficiency benchmark |

### E06 — Knowledge and grounded retrieval

| Story | Status | Depends on | Story |
|---|---|---|---|
| E06-S01 | PLANNED | E04-S05 | Bounded archive and extraction provider |
| E06-S02 | PLANNED | E06-S01 | Hybrid retrieval and grounded attachments |
| E06-S03 | PLANNED | E06-S02 | Incremental update/delete recovery |
| E06-S04 | PLANNED | E06-S03 | Local-model harness UX and performance |
| E06-S05 | PLANNED | E06-S04 | Knowledge UI and placement |

### E07 — Environments and computer use

| Story | Status | Depends on | Story |
|---|---|---|---|
| E07-S00 | IN_PROGRESS | E02-S03 | Task-scoped local Environment isolation |
| E07-S01 | IN_PROGRESS | E05-S05, E06-S05 | Persistent environment management |
| E07-S02 | PLANNED | E07-S01 | Structured browser and control leases |
| E07-S03 | PLANNED | E07-S02 | ActionBatch/deadline/warm lifecycle |
| E07-S04 | PLANNED | E07-S03 | Office/data/research capability qualification |
| E07-S05 | PLANNED | E07-S04 | Desktop computer use and optional observation |

### E08 — Coworker, Workbench, and presentation surfaces

| Story | Status | Depends on | Story |
|---|---|---|---|
| E08-S01 | PLANNED | E05-S05, E06-S05 | Coworker identity and Home |
| E08-S02 | IN_PROGRESS | E08-S01 | Goals and suggestion producers |
| E08-S03 | IN_PROGRESS | E08-S02 | Editable context lifecycle |
| E08-S04 | IN_PROGRESS | E08-S03 | Live Work surface and activity projection |
| E08-S05 | PLANNED | E08-S04, E08-S06, E08-S07 | Interaction completeness and accessibility |
| E08-S06 | PLANNED | E03-S02, E08-S04 | Typed Presentation Runtime and reconnect-safe streaming |
| E08-S07 | IN_PROGRESS | E04-S05, E06-S05, E08-S04 | Adaptive Artifact Workbench and version history |
| E08-S08 | IN_PROGRESS | E03-S02, E04-S05, E08-S04, E08-S06 | Semantic-first RichPresentation and Host Guidance |

### E09 — Routines and Automations

| Story | Status | Depends on | Story |
|---|---|---|---|
| E09-S01 | PLANNED | E07-S05, E08-S05 | Routine creation/revision and reusable work |
| E09-S02 | IN_PROGRESS | E09-S01 | Automation and occurrences |
| E09-S03 | PLANNED | E09-S02 | Automation test and health/drift UX |
| E09-S04 | PLANNED | E09-S03 | Notifications and settings |
| E09-S05 | IN_PROGRESS | E07-S05, E08-S05, E09-S01 | Manual Routine run creates a READY Task |

### E10 — Skills and discovery

| Story | Status | Depends on | Story |
|---|---|---|---|
| E10-S01 | PLANNED | E09-S04 | MCP Apps/Skills and Discover |
| E10-S02 | PLANNED | E10-S01 | Demonstration capture and SkillProposal |

### E13 — Desktop/local V1 production release

| Story | Status | Depends on | Story |
|---|---|---|---|
| E13-S01 | PLANNED | E10-S02 | Backup/restore and safe upgrades |
| E13-S02 | PLANNED | E13-S01 | Performance/soak and resilience |
| E13-S03 | PLANNED | E13-S02 | Real workflow and platform release suite |
| E13-S04 | PLANNED | E13-S03 | Packaging/security/runbooks and launch |

### Explicitly post-V1

| Epic/story | Status | Depends on | Scope |
|---|---|---|---|
| E11-S01 | PLANNED | E13-S04 | Cloud deployment and operations |
| E11-S02 | PLANNED | E11-S01 | Pairing, sync and conflict authority |
| E11-S03 | PLANNED | E11-S02 | Handoff, failure and fencing |
| E11-S04 | PLANNED | E11-S03 | Human channel integration |
| E11-S05 | PLANNED | E11-S04 | Cloud capability and security qualification |
| E12-S01 | PLANNED | E11-S05 | Remote installation and enrollment |
| E12-S02 | PLANNED | E12-S01 | Remote placement/continuation tests |

## V1 path to release and hard gates

`implementation/SCOPE.md`, `implementation/ROADMAP.md`, `implementation/RELEASE.md`, and
`implementation/V1-CONTRACT-RECONCILIATION-2026-10-13.md` are authoritative alongside the
epics and contracts. V1 is production-qualified desktop/local, not merely a finished alpha.
The dependency graph governs execution; the release sequence is:

1. **G0 — Contract/development readiness:** pinned tools, validators and qualification
   spikes; E01 foundation. E01-S01 requires clean-checkout and hosted Actions evidence plus
   owner review. Continue safe E01-S02 implementation without pretending that it clears
   E01-S01.
2. **G1 — Local durable work:** installed app, real native provider Conversation Send,
   verified Artifact, and kill/restart recovery. Current roadmap adds hard gates: E03-S01/
   S02 must prove provider-backed Send/Stop/reconnect; E07-S00 must qualify Runtime-owned
   isolation and Windows WorkspaceRoot identity/rebind/restart before those Windows paths
   can be advertised; E03-S03 must use an authenticated planner producer and atomically
   accept its versioned plan. Keep the planning isolation gate fail-closed.
3. **G2 — Desktop feature complete:** all in-scope local stories and representative actual
   workflows. This includes Trust/Gateway/Effects/Evidence, native-agent parity, delegation,
   real LiteSPM/MCP and provider qualification, retrieval, browser/desktop takeover, private
   automatic-learning lifecycle and user controls, Workbench/presentation, Automations,
   accessibility, and Discover. Unsupported combinations must remain visibly
   `UNAVAILABLE`/`BLOCKED`, never inferred as supported from mocks or design documents.
4. **G3 — Production V1 release:** accepted stories and linked executable evidence; exact
   supported OS/agent/provider matrix; local real-use corpus, security/fault/recovery tests,
   benchmark and soak gates; signed installers on clean hosts; upgrade, uninstall, restore,
   rollback and incident drills; user documentation; architecture/plan validators on the
   release commit; and owner approval of evidence and release claims. See `RELEASE.md` for
   the full checklist.

Cloud continuation E11 and remote Runtime E12 follow V1 and do not block local V1. Do not
remove finalized local feature scope to make the schedule look complete.

## OpenIntelligentUI reference commitment for E08-S08

The user's reference is [CopilotKit/OpenIntelligentUI](https://github.com/CopilotKit/OpenIntelligentUI),
reviewed at commit
[`f6e4388b26a64b9a0714943b08a1ce622b924eec`](https://github.com/CopilotKit/OpenIntelligentUI/tree/f6e4388b26a64b9a0714943b08a1ce622b924eec).
It is a UX and test-design reference for semantic-first prose/native/interactive selection,
streamed preview-to-final lifecycle, local iframe controls, explicit follow-up, resize
handling, and bridge/failure tests. Its arbitrary generated HTML/CSS/JavaScript sandbox and
CSP allowing inline/eval and selected CDN scripts are **not** an approved LiteCowork
RichPresentation security boundary. The root license at that revision was checked as MIT.

At E08-S08, run the pinned repository viability check and matched scenarios/measurements
specified in `docs/REFERENCE-IMPLEMENTATIONS.md` and the E08 story. The current speed
comparison is a hypothesis: OpenIntelligentUI is likely faster for a standalone generated-UI
prototype; LiteCowork's native RichPresentation path is likely faster to integrate into this
existing Tauri/React/Rust application. No measured relative speed claim is complete until
matched runs record exact commands, versions, raw timings, failures, and limits in the
E08-S08 review packet and CURRENT-RUN. The user explicitly asked to run and finalize this
comparison when E08-S08 is implemented.

## Worktree, resource, and verification state — measured 2026-10-16

- One `main` worktree; 26 local branches. No worktree or branch was created for the
  current integration. At measurement, `HEAD` and `origin/main` were `de73535` and the
  worktree/index contained extensive owner changes. Free space on `/` was
  903,853,826,048 bytes. Root `target/` measured 7,677,090,567 bytes; UI `dist/` measured
  993,835 bytes; UI `node_modules/` measured 100,066,382 bytes. The shared Cargo target,
  single UI build output, and dependency cache are retained as active reusable outputs.
- Playwright/Chrome MCP processes were visible during the measurement. Treat them as
  potentially owner-managed sessions; do not terminate them without establishing ownership.
  No new browser, Cargo, or server process was started for this documentation slice.
- Current evidence is intentionally split among `CURRENT-RUN.md`, the E02-S03 review
  packet, and the audit. The E02-S03 package run and daemon restart test listed above are
  CODE/restart evidence, not production key-provider, full packaged SYSTEM, or USER
  acceptance. Finish the staged-snapshot documentation gates described above before
  refreshing the review packet or committing this milestone.

## Copy/paste prompt for the next session

```text
You are the lead implementation agent for LiteCowork. Continue the authorized desktop/local
V1 objective using implementation/backlog.json as status/dependency authority and the exact
current contracts/source/tests as implementation authority. Do not ask whether to continue
ordinary reversible work. Read AGENTS.md, CURRENT-RUN, the next exact story and owning
contracts before editing.

At handoff refresh, main/HEAD/origin were de73535cf27af347d9d5413ccaffc81bef2e78be, but
the worktree contained substantial staged and unstaged owner work. First inspect branch,
status, index/worktree diffs, worktrees, active processes and disk measurements. Preserve
all existing owner changes; never broad-stage. The staged E02-S03 integration is active and
must be reviewed against its source, migrations, tests, review packet and exact acceptance.
A recent integrated storage run passed 303/303 and the authenticated daemon purge
restart/replay case passed 1/1; recheck their latest evidence and rerun required gates.
E02-S03 remains IN_PROGRESS; packaged Tauri, production key provider, non-Linux OS, abrupt
crash/power-loss, external-provider deletion and owner USER evidence remain open.

The first staged contract validation found target-only WorkspaceRoot operations missing from
served-route prose. A staged validator/test fix now accounts for explicit target-only
markers; rerun coverage generation, plan/architecture, strict design-depth, owner-consistency,
and diff checks in the prescribed order and record exact outputs. Refresh the review packet
and CURRENT-RUN only with proven evidence. Commit and push coherent reviewed milestones
using exact paths, then continue to the next dependency-ready V1 work.

The 2026-10-13 re-audit is still the source map for A01–A13 and D01–D12. Distinguish source
repairs, executed CODE tests, packaged/provider/SYSTEM evidence, and owner USER acceptance.
U15 is ContextDocument conflict/revoke/delete; U16 is Goal progress from verified Task
outcomes. Accepted automatic quiet private learning is a separate E08-S03 target tracked by
UIV1-120–UIV1-127 and docs/LEARNING.md; it is not implemented or proven by U15/U16. Do not
promote backlog status from tests or validators alone.

Continue through E01–E10 and E13 desktop/local V1, then complete E13 release gates: backup
and restore, upgrade/rollback, measured performance/soak, real workflows and platform
qualification, signed clean-host installers, security/accessibility/runbooks, and owner G3
approval. E11 cloud continuation and E12 remote Runtime are post-V1. At E08-S08, run the
pinned CopilotKit/OpenIntelligentUI viability/performance comparison from the exact plan in
docs/REFERENCE-IMPLEMENTATIONS.md; do not claim which route is faster until matched runs
record commands, versions, raw timings, failures and limits.
```
