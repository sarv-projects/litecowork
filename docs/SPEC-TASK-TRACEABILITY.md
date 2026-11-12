# Spec, Build TASKS, and USER Test Traceability

**Updated:** 2026-11-12

**Purpose:** one crosswalk between **global build tasks** ([LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md)), **feature SDD tasks** (`specs/NNN-*/TASKS.md`), **acceptance IDs** (SPEC EARS), and **competitor USER cases** ([COMPETITOR-PROMOTED-USE-CASES.md](COMPETITOR-PROMOTED-USE-CASES.md)).

## Feature 001 — Conversation Session Supervisor

| Build task | Spec task | Primary AC-* | USER UC-* |
|---|---|---|---|
| BT-0.6, BT-1.* | 001-T01 | AC-001–004 blockers | UC-CHAT-01–05 |
| BT-1.2 | 001-T02, T06 | AC-009 | UC-CHAT-01 |
| BT-1.1 | 001-T03, T04 | AC-005–008 | UC-CHAT-01 |
| BT-1.1 | 001-T05 | AC-013–016 | UC-CHAT-06 |
| BT-1.3 | 001-T07 | AC-010–011, 017–019 | UC-CHAT-01, UIV1-016–022 |
| BT-1.4 | 001-T08 | AC-012 | FL01–FL05 |
| BT-1.5, BT-12.* | 001-T09 | AC-001–019 all | UC-CHAT-01 + full 001 EARS |

> The `UIV1-016–022` citation above is a provenance pointer to the archived legacy catalog only; the legacy `UIV1-xxx` namespace is retired as an active acceptance authority (register [A-39](CONTRADICTION-AND-ALIGNMENT-REGISTER.md)). The live acceptance obligation is the referenced flow/AC IDs, per [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md).

**Spec package files (all required for lock):**

| File | Lines (approx) | Role |
|---|---:|---|
| [specs/001-.../README.md](../specs/001-conversation-session-supervisor/README.md) | 28 | Scope, owners, links |
| [SPEC.md](../specs/001-conversation-session-supervisor/SPEC.md) | 354 | EARS, HLD/LLD, blockers B-01–B-10 |
| [PLAN.md](../specs/001-conversation-session-supervisor/PLAN.md) | 106 | Phases, DAG |
| [TASKS.md](../specs/001-conversation-session-supervisor/TASKS.md) | see file | 001-T01..T23 V1 task plan |
| [DATA-MODEL.md](../specs/001-conversation-session-supervisor/DATA-MODEL.md) | 78 | Records, B-04 boundary |
| [RESEARCH.md](../specs/001-conversation-session-supervisor/RESEARCH.md) | 101 | Evidence, rivals |
| [WIRING.yaml](../specs/001-conversation-session-supervisor/WIRING.yaml) | 236 | Components, transactions |
| [contracts/api-events.md](../specs/001-conversation-session-supervisor/contracts/api-events.md) | 63 | OpenAPI/event binding |
| [contracts/errors-recovery.md](../specs/001-conversation-session-supervisor/contracts/errors-recovery.md) | 78 | Typed failures |
| [contracts/ui-behavior.md](../specs/001-conversation-session-supervisor/contracts/ui-behavior.md) | 93 | UI flows/states/commands |

**Validator:** `python3 .specify/scripts/validate_feature_package.py` (the validator walks every package under specs/; it takes no package argument)

## All packages: V1 task plans (2026-11-09)

Owner acceptance for 001-012 was recorded on 2026-11-09. Each TASKS.md now runs to a final "Package VERIFIED for V1" task, and 019-T23 is the V1 go/no-go that lists every package's VERIFIED task. Cross-package dependencies are written as "Feature NNN Txx" so the validator does not read them as local tasks. Feature 010 renumbered its WorkflowBackend items: the external Routine mapping is now 010-T18 and the provider-authoring evaluation is 010-T19; both sit outside the V1 gate. Always read the current TASKS.md rather than assume a fixed upper bound.

On 2026-11-09 the owner also approved folding the proposed tasks P-05 and P-11..P-19 (and the P-00..P-09 rendering/shell set) into their owning packages, including the LOCKED ones, under the ACP native-agent transport direction (register [A-54](CONTRADICTION-AND-ALIGNMENT-REGISTER.md)). Those additions raised several packages' task counts and pushed each package's "Package VERIFIED for V1" task to a new final id: 001 now runs to T28 (added T23..T27 for P-00/P-01/P-02/P-04/P-09, VERIFIED T28), 002 to T19 (ACP client P-11 = 002-T17, Matchmaker P-12 = 002-T18, VERIFIED T19), 003 to T18 (Context Debugger P-15 = 003-T17, VERIFIED T18), 008 to T21 (iframe guard P-06 = 008-T19, Git worktree/PR P-13 = 008-T20, VERIFIED T21), 011 to T23 (shell P-05 = 011-T19, decomposition P-07 and P-19 = 011-T20, Settings merge P-08 = 011-T21, Workspace Dashboard P-16 = 011-T22, VERIFIED T23), 013 to T21 (LiteSPM pin P-17 = 013-T20, VERIFIED T21), 015 to T21 (Mission Control/Compare P-14 = 015-T20, VERIFIED T21), 016 to T19 (per-block Markdown P-03 = 016-T18, VERIFIED T19), and 018 to T19 (operator-api.ts codegen + drift gate P-18 = 018-T18, VERIFIED T19). 019-T23 re-synced its VERIFIED-id list to those new ids. The full capability-to-task mapping is in [V1-PRODUCT-FEATURE-COVERAGE.md](V1-PRODUCT-FEATURE-COVERAGE.md).

| SDD | Status | Tasks | Qualification / VERIFIED | E2E | USER themes | Wave |
|---|---|---|---|---|---|---|
| 001 | ACCEPTED/LOCKED 2026-11-09 (B-01..B-10; B-02 rider: OpenCode v2.0.32 first, Windows x64 first row; folded P-00/P-01/P-02/P-04/P-09) | T01..T28 | 001-T09, 001-T28 | E2E-01 | UC-CHAT-01..06 | W1 |
| 002 | ACCEPTED/LOCKED 2026-11-09 (B-11, B-12; folded ACP client P-11, Matchmaker P-12) | T01..T19 | 002-T15, 002-T19 | E2E-01, E2E-02 | UC agent install/auth | W1 |
| 003 | ACCEPTED/LOCKED 2026-11-09 (B-13, B-14 with Windows scanner rider; folded Context Debugger P-15) | T01..T18 | 003-T15, 003-T18 | E2E-03 | UC files and citations | W1 |
| 004 | ACCEPTED/LOCKED 2026-11-09 (B-15, B-16) | T01..T12 | 004-T10, 004-T12 | E2E-02 | UC relay A to B to A | W4 |
| 005 | ACCEPTED/LOCKED 2026-11-09 (B-17 Linux-first rider, B-18); B-35 Windows provider PROPOSED; A-70/A-87/A-88/A-90 block execution sequencing and T08 pending T01 | T01..T20 | 005-T15, 005-T18, 005-T20 | E2E-04 | UC one safe task edit | W2 |
| 006 | ACCEPTED/LOCKED 2026-11-09 (B-19, B-20); T01/T02 BLOCKED on authentication/effective permissions (A-82); Task-write row order OPEN (A-80) | T01..T17 | 006-T15, 006-T17 | E2E-05 | UC one connection write | W3 |
| 007 | ACCEPTED/LOCKED 2026-11-09 (B-21, B-30, B-31); T01/T06 BLOCKED on export receipt ordering/local recovery (A-81); destination-class decision closed | T01..T17 | 007-T15, 007-T17 | E2E-07 | UC one professional format | W2 |
| 008 | Revision 1.0 ACCEPTED/LOCKED 2026-11-09 (B-22, B-23, B-32 deferred; P-06/P-13); revision 1.1 IN_REVIEW (selected laptop Computer, compact Workbench, takeover) | T01..T27 | 008-T17, 008-T21 (rev 1.0); 008-T22..T27 (rev 1.1 review/promotion/qualification gates) | E2E-08; revision 1.1 E2E mapping pending owner/schema promotion | UC browser/terminal; selected-screen Computer proposal | W3 |
| 009 | ACCEPTED/LOCKED 2026-11-09 (B-24, B-25) | T01..T18 | 009-T16, 009-T18 | E2E-09 | UC optional Coworker chat and memory | W2 |
| 010 | ACCEPTED/LOCKED 2026-11-09 (B-26, B-27); CONNECTOR_EVENT over LOCAL_POLL confirmed 2026-11-10 (010-T09 still needs a qualified 006 Connection; 010-T19 deferred, out of V1) | T01..T19 (T18/T19 outside the V1 gate) | 010-T15, 010-T17 | E2E-10 | UC schedule exactly once | W5 |
| 011 | ACCEPTED/LOCKED 2026-11-09 (B-28 Windows-first rider, B-29, B-33; folded shell P-05, decomposition P-07/P-19, Settings merge P-08, Workspace Dashboard P-16) | T01..T23 | 011-T17, 011-T23 | E2E-01 shell portion | UC settings honesty and health | W0 |
| 012 | ACCEPTED/LOCKED 2026-11-09 (B-34 model; implementation waits on the 001 Stop fix) | T01..T11 | 012-T09, 012-T11 | E2E-01 extension | UC co-edit / take-control | W1 (after 001-T07) |
| 013 | IN_REVIEW (new 2026-11-09); B-36, B-37 DEFERRED by the owner 2026-11-10 (LiteSPM parts deferred); folded LiteSPM pin P-17 | T01..T21 | 013-T18, 013-T21 | E2E-06 | UC discover and install a package | W2 observe, W3 plan/apply |
| 014 | IN_REVIEW (new); B-38 ACCEPTED as proposed 2026-11-10 | T01..T19 | 014-T17, 014-T19 | E2E-14 (adopted) | UC find, rename, archive, delete chats | W1 |
| 015 | IN_REVIEW (new); B-39 ACCEPTED as proposed 2026-11-10; folded Mission Control/Compare P-14 | T01..T21 | 015-T17, 015-T18, 015-T21 | E2E-15 (adopted) | UC steer, pause and delegate a Task | W4 |
| 016 | IN_REVIEW; baseline AC-001..AC-019 acceptance pending T01; extension AC-020..AC-027 separately gated at T25 (A-84); A-85 blocks MCP proposal exposure | T01..T25 | 016-T16, 016-T19, 016-T24 | E2E-11 | UC rich answer with fallback | W2 |
| 017 | IN_REVIEW (new); B-41 ACCEPTED as proposed 2026-11-10 | T01..T18 | 017-T16, 017-T18 | E2E-12 | UC back up and restore | W6 |
| 018 | IN_REVIEW (new); B-42 PROPOSED (open, BLOCKED); folded operator-api.ts codegen + drift gate P-18 | T01..T19 | 018-T16, 018-T19 | E2E-16 (adopted) | UC usage and support bundle | W6 |
| 019 | IN_REVIEW (new); B-43, B-44 DEFERRED 2026-11-10 (release dependent) | T01..T23 | 019-T22, 019-T23 (V1 go/no-go) | all rows | UC install, update, sign-off | W0 skeleton, W6 qualification |
| 020 | IN_REVIEW (new); D-01, B-45 ACCEPTED as proposed 2026-11-10 | T01..T17 | 020-T15, 020-T17 | E2E-17 (adopted) | UC quiet learning on a Coworker | W4 |

### Build task to package task crosswalk

| Build task | Package tasks |
|---|---|
| BT-00.1..BT-00.6 | 019-T03, 019-T04, 019-T05, 001-T22 and each package's documentation-sync task |
| BT-01.1, BT-01.2 | 001-T10 (outbox), 001-T11 (single-flight), 001-T08 (restart recovery) |
| BT-01.3 | 001-T04 (OpenCode v2.0.32 adapter), 001-T24 (OpenCodeEventNormalizer, P-01), 002-T17 (ACP client adapter, P-11), 002-T01 (bindable profile), 001-T09 |
| BT-01.4 | 001-T19, 002-T06 |
| BT-01.5 | 001-T13, 016-T08, 016-T09 |
| BT-01.6 | 001-T01, 001-T21 |
| BT-02.1..BT-02.5 | 003-T02..003-T08, 003-T13 |
| BT-03.1, BT-03.4 | 002-T03..002-T09 |
| BT-03.2 | 002-T12 (Claude Code second module) |
| BT-03.3 | 004-T03..004-T10 |
| BT-04.1..BT-04.6 | 005-T03..005-T15; Windows row 005-T16..005-T18 |
| BT-05.1..BT-05.3 | 013-T02..013-T11 (moved from 006 to the new LiteSPM package) |
| BT-05.4, BT-05.5 | 006-T03..006-T15, 013-T14 |
| BT-06.1, BT-06.2 | 007-T02..007-T15 |
| BT-06.3..BT-06.5 | 016-T03..016-T12 (moved from 007 to the new presentation package) |
| BT-07.1..BT-07.6 | 008-T02..008-T17 |
| BT-08.1..BT-08.5 | 009-T02..009-T16 |
| BT-09.1..BT-09.5 | 010-T03..010-T15 |
| BT-10.1..BT-10.4 | 011-T05..011-T15 |
| BT-10.5 | 019-T14, 019-T15, 019-T23 |
| BT-11.* (history) | 014-T03..014-T17 |
| BT-12.* (Task control) | 015-T03..015-T18 |
| BT-13.* (backup, usage, diagnostics) | 017-T03..017-T16, 018-T03..018-T16 |
| BT-14.* (proactivity, learning) | 020-T03..020-T15 |
| BT-15.* (release engineering) | 019-T03..019-T22 |

## Implementation snapshot (2026-11-10)

A task is traced to a test only when its TASKS.md status line says so. Snapshot at tip e9abe46:

| Package | Implemented so far (evidence class CODE or SERVED unless stated) | Not implemented, deferred or blocked |
|---|---|---|
| 001 | Durable turn outbox and single-flight (v29), Stop and Retry, restart sweep, transient frames, usage records, sliding context window, REPORTED activity (v30, served read, UI cards), ResourceRef text materialization, ACP adapter and normalizer, Runtime DEGRADED on Operator endpoint failure | T12 BLOCKED, T09 and T28 NOT STARTED (live OpenCode), binary attachments, permission answer channel, O-05 real input budget |
| 002 | OpenCode AgentModule manifest, descriptor store and routes, enable gate, ACP client, auth observation service, served begin-authentication and open-native-config (fail-closed), model picker view-model, Matchmaker | T08 and T12 BLOCKED, T15 DEFERRED, T19 NOT STARTED, no live run, no Tauri commands |
| 003 | Linux workspace root lifecycle storage tests (T02 partial) | Windows tasks DEFERRED (T03, T04 Windows part, T05, T06); T01, T07, T09 to T18 NOT STARTED |
| 011 | IPC bounds, runtime health rows, local preferences, Workspace rename (served), shortcut registry, SettingsPage merge, read-only stop preview (served), Dashboard, keep-awake state machine | T02, T03, T17 DEFERRED; T13 request half, T23 not done |
| 012 | Migration v32 storage (T05 partial) | Routes, service, UI NOT STARTED |
| 013 | Nothing | Whole package DEFERRED by the owner |
| 014 | Migration v31 side table and `ConversationHistoryStore` (T01 to T03 partial) | Routes, search, purge, UI NOT STARTED |
| 019 | CI skeleton, CODEOWNERS, secret scan, CSP audit, release schemas, matrix, evidence bundle, Linux deb config and scripts, updater UI reducer | B-43 and B-44 items DEFERRED; CI never ran on GitHub; no deb built |
| 004 to 010, 015 to 018, 020 | Pre-existing foundations only | Not started; see each TASKS.md |

Test baselines at this snapshot: `cargo test -p storage-sqlite --lib` 624 pass; `cargo test -p litecoworkd --bin litecoworkd` 656 pass, 1 ignored; UI `pnpm test` 5 pre-existing failures; served routes 178 / 1 local-only / 53 target-only; `SCHEMA_VERSION` 32. Full per-task detail: [CURRENT-RUN.md](../CURRENT-RUN.md) "Handoff status by package".

## .specify templates (copy source for new packages)

| Template | Use |
|---|---|
| [.specify/templates/SPEC.md](../.specify/templates/SPEC.md) | Full SPEC skeleton |
| [.specify/templates/TASKS.md](../.specify/templates/TASKS.md) | Task ID discipline |
| [.specify/templates/contracts/ui-behavior.md](../.specify/templates/contracts/ui-behavior.md) | UI behavior contract |
