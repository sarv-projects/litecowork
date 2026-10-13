# Next-session handoff — LiteCowork desktop/local V1 — 2026-10-13

This is the current-worktree handoff for continuing the active LiteCowork implementation
objective. It is navigation and evidence, not a replacement for the current contracts,
source, tests, or backlog. Re-check all listed state at session start.

## Current branch and preservation boundary

- Workspace: `/home/sarvesh/business_Dev/litecowork`.
- Branch: `main`; at handoff inspection `HEAD` and `origin/main` point to
  `c66817b77c114dc1db8f4c0fc8b24ef5b498e177` (commit subject `commit`), following
  `86e0d80` (`fix(tauri): route workspace instruction revisions correctly`). The new commit
  records the owner architecture/documentation/source pass: 296 files, 68,758 insertions and
  3,109 deletions. It includes owner `.playwright-mcp/` data. Preserve that commit and data;
  do not reset, rewrite, or delete them. At current inspection, the handoff/current-run evidence and deterministic coverage/Markdown
  inventories are the only uncommitted paths. Recheck `git status --short` before mutation.
- Never reset, clean, stash, checkout away, overwrite, or broadly stage the owner work. Stage
  exact paths or hunks for the current agent's work and inspect the staged diff. The owner
  has authorized coherent milestone commits and pushes, but this does not authorize staging
  unrelated work.
- The E01-S02 test `idempotent_workspace_policy_busy_leaves_no_receipt_and_retries_once`
  is already present in the pushed `c66817b` owner commit. Do not duplicate or stage it again.
  The review addendum and handoff are agent-authored. `implementation/CURRENT-RUN.md` now
  has only the E01-S02 handoff entry as an uncommitted change; verify with `git diff` before
  staging.
- Do not change E01-S02 to `DONE`/`ACCEPTED`. E01-S01 is still `IN_REVIEW`, and owner
  acceptance/system/provider gates remain open.

## Work completed immediately before handoff

A single new regression was added to
[`crates/storage-sqlite/src/tests.rs`](../crates/storage-sqlite/src/tests.rs):
`idempotent_workspace_policy_busy_leaves_no_receipt_and_retries_once`. It uses an actual
on-disk SQLite adapter and `WorkspaceService`, holds a second-connection `BEGIN IMMEDIATE`
lock, observes `StoreError::Busy`, and proves no partial Workspace/event/sequence/replay or
idempotency receipt. Releasing the lock and retrying the identical request commits version
2 / origin sequence 2; exact replay returns the same receipt without a second event, and the
saved response digest is checked. No production behavior changed.

Evidence is in the [E01-S02 busy retry CODE addendum](reviews/E01-S02-BUSY-RETRY-2026-10-13.md)
and the new entry in `CURRENT-RUN.md`. On the working tree before the owner committed `c66817b`, the focused test passed 1/1 and
`cargo test --locked -p storage-sqlite` passed 190/190 with 0 doc tests. The test is now
part of the pushed `c66817b` source tree; the 190-test suite was not rerun after that commit. A detached
clean-HEAD worktree at `86e0d80` with an isolated target also passed the focused test 1/1;
that worktree and its 1,508,110,871-byte target were removed. Do not describe the dirty full
suite as a clean-HEAD full-suite run. The focused storage fixture reported bundled SQLite
3.53.2. All exact commands, versions, evidence limits, and cleanup measurements are in the
addendum.

The test demonstrates local SQLite busy-timeout rollback and retry. Physical filesystem
exhaustion, OS key stores, other operating systems, packaged product/System integration,
cryptographic review, and owner USER acceptance remain unverified. The existing E01-S02
review packet remains the owner decision record; its owner section must not be filled by an
agent.

Previous pushed slice: E02-S02 Tauri instruction bridge transport CODE, commit `86e0d80`.
It covers five bridge tests, 59 Tauri library tests, Clippy (with 26 existing warnings), and
separate daemon IPC coverage. It does not prove a single Settings→Tauri→daemon→authorization→
SQLite flow, packaged SYSTEM behavior, cross-platform qualification, or USER acceptance.
See `implementation/reviews/E02-S02.md` and `CURRENT-RUN.md`.

## Immediate implementation frontier

The authoritative current status is `implementation/backlog.json`; read it again with the
source because the owner pass at `c66817b` changed the implementation and contracts broadly.
The latest read-only frontier audit identified E01-S02 as the current foundational continuation. The test above
adds CODE evidence only and does not finish E01-S02. Continue through the remaining
transactional persistence acceptance/recovery gaps, while keeping the E01-S01 prerequisite
and owner decision explicit. Then select the next dependency-ready story from the live
source/backlog. Do not jump to an unrelated PLANNED story such as E03-S06 solely because it
is an obvious new feature.

For E01-S02, reread at minimum: `implementation/epics/E01.md` (exact story),
`implementation/reviews/E01-S02.md`, `docs/STORAGE.md`, `docs/SERVICES.md`, `docs/EVENTS.md`,
`docs/DATA-MODEL.md`, `docs/IMPLEMENTATION.md`, relevant API/protocol/security contracts,
`docs/schemas/sqlite-v1.sql`, and actual storage source/tests. Current acceptance explicitly
requires atomic Workspace/event/per-origin-sequence commits; stale concurrent writes;
rollback/replay; restart; migration/schema drift and key/blob fail-closed behavior; bounded
writer Busy/SQLITE_FULL rollback and retry; CODE/SYSTEM/USER evidence. Controlled SQLite
`max_page_count` is not physical filesystem exhaustion.

## Complete desktop/local V1 backlog snapshot

Status and dependency columns below reproduce the current `implementation/backlog.json`
read during handoff preparation. The backlog remains the status authority; a PLANNED or
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

## Worktree, resource, and verification state

- One worktree remains (`main`); the temporary clean-HEAD worktree was removed and
  `git worktree prune` completed. Branch count is 26; this slice created no branch.
- The 1,508,110,871-byte isolated clean-HEAD target and its temporary worktree were removed.
  At a later current-state inspection, both shared root and Tauri `target` directories were
  absent and free space was 924,286,369,792 bytes; do not assume prior caches still exist.
  Owner `.playwright-mcp` data (133,783 bytes) remains and is committed in `c66817b`; preserve it.
- One main worktree and 26 branches remain. At the latest process check, 21 Node processes
  were present and no Cargo/rustc process was listed; do not kill unclassified Node processes.
  Recheck ownership before cleanup. No E01 temporary SQLite fixture or agent worktree remains.
- Do not rerun the slow full storage suite merely to recreate already recorded evidence.
  Run the focused test or the next story's required checks as appropriate.
- `.venv/bin/python scripts/validate_architecture.py` passed across schemas, events, error
  codes, OpenAPI, SQLite, names, and repository Markdown links. A scoped link/fence check and
  `git diff --check` for this evidence also passed. `python3 scripts/validate_architecture.py`
  could not import `openapi_spec_validator`; the configured `.venv` has the dependency.
- The first plan validation found `coverage.csv` stale while the owner pass was not yet
  committed. Once that pass landed at `c66817b`, the documented sequence was run: `.venv/bin/python
  scripts/audit_v1_contracts.py --strict --write-report` reported 0 errors (206 routes, 158
  UI cases, 67 stories); `.venv/bin/python scripts/audit_v1_markdown.py` reported 159 files,
  57,218 lines, 0 broken links; coverage generation/read-only validation passed for 103 docs,
  1,668 sections, 6,340 rows; implementation-plan validation passed for 67 stories; and
  architecture validation passed for 142 typed events and current machine contracts. The
  contract audit notes that Conversation native message submission is still unavailable and
  read-only/disabled controls need release review; these are not pass claims for product
  functionality. The generated Markdown CSV retains its established CRLF format; use
  `git -c core.whitespace=cr-at-eol diff --check` when checking those generated rows.
  Validators prove consistency, not runtime behavior.

## Copy/paste prompt for the next session

```text
You are the lead implementation agent for LiteCowork. Continue the active desktop/local V1 implementation objective from implementation/NEXT-SESSION-HANDOFF-2026-10-13.md. Work autonomously through dependency-ready stories until the complete V1 release evidence is actually proven or an explicit stop condition in AGENTS.md applies. Do not ask whether to continue ordinary reversible work.

First inspect git status, branch, staged diff, current diff, worktrees, and owner changes. Preserve the committed owner pass at c66817b and .playwright-mcp data. Read AGENTS.md, implementation/CURRENT-RUN.md, implementation/README.md, SCOPE.md, AUDIT.md, PROCESS.md, ROADMAP.md, backlog.json, the next exact epic/story, ARCHITECTURE.md, COVERAGE-MATRIX.md, ARCHITECTURE-COVERAGE.md, and all story-specific authority documents and machine contracts before editing. Search and read actual source/tests. Treat backlog.json as status/dependency authority; this handoff is evidence/navigation only.

The focused E01-S02 test idempotent_workspace_policy_busy_leaves_no_receipt_and_retries_once in crates/storage-sqlite/src/tests.rs is included in pushed commit c66817b. Its review addendum is implementation/reviews/E01-S02-BUSY-RETRY-2026-10-13.md. Verify current history; do not duplicate or stage the test again. E01-S02 remains IN_PROGRESS and E01-S01 remains IN_REVIEW. Continue the highest-priority dependency-ready foundational work, do not jump to unrelated PLANNED E03 agent registry work. Keep Task planning isolation, durable idempotency/Effect/Evidence/Trust, native agent behavior, and platform gates fail-closed.

For every slice: update CURRENT-RUN and truthful evidence, run narrow then required integration/system/security/USER gates, inspect the whole diff, measure worktrees/branches/processes/regenerable paths at start/end, remove only confirmed agent-created temporary outputs, selectively stage only your paths/hunks, inspect the staged diff, make a Conventional Commit and push the authorized milestone to origin. Preserve owner data/caches and do not publish/release/provision/spend or change external production state. Keep E11/E12 out of the V1 critical path. V1 release is E01-E10 plus E13 and must meet G0-G3 in SCOPE.md and RELEASE.md; it is not complete until production-qualified desktop/local evidence and owner approval exist.

When implementing E08-S08, use the pinned CopilotKit/OpenIntelligentUI revision and exact comparison/test plan in docs/REFERENCE-IMPLEMENTATIONS.md. Run the viability and matched renderer tests then; preserve it only as UX/test reference, not as the RichPresentation runtime/CSP. Record measured speed and suitability only after matched runs.

Continue working; a handoff/status summary is not completion. Update the next session's evidence and continue to the next dependency-ready story automatically.
```
