# Whole-repository audit — 2026-10-21

**Scope:** every Rust crate (`crates/**`), the Operator daemon (`apps/litecoworkd/**`), the
desktop and Tauri bridge (`apps/litecowork-ui/**`), all of `docs/**`, and all of
`implementation/**` (epics, backlog, reviews, coverage, machine inventory).
**Method:** five independent read-only audit lanes (Rust core, daemon/API, desktop UI,
docs↔code cross-check, remaining-work map), each verifying claims against source rather
than prose. Every finding below cites `path:line`.
**Baseline:** `main` @ `ed2e1c8` (pushed to `origin/main`).

The machine contracts (OpenAPI, SQLite DDL, event registry, error-code schema) are
internally consistent and the validators pass. The drift is in three places: the prose
layer, the gap between *published* and *served* API surface, and whole domain modules that
no production path calls.

---

## 1. The single largest finding: published ≠ served

`docs/schemas/operator-api.openapi.yaml` publishes **225 operations over 200 paths**.
The composed daemon router (`apps/litecoworkd/src/operator.rs`, 36 `.merge()` calls)
mounts **154 distinct paths**. **47 published operation groups have no handler**, and
`scripts/audit_v1_contracts.py` never parses the router, so every green gate hides this.

Representative target-only groups (all published with `operationId`, request bodies and
success responses):

| Group | OpenAPI | Served |
|---|---|---|
| Task control — `steer`, `cancel`, `pause`, `resume`, `steps/{id}/recover`, `execution-dependencies` | `yaml:605-697` | none |
| Task reads — `attempts`, `timeline`, `effects`, `usage` | `yaml:812-855`, `4268` | none |
| Plan submission — `POST /tasks/{id}/plan-revisions` | `yaml:764` | GET only (`operator.rs:1610`) |
| Environments — all 8 `workspaces/{id}/environments*` operations | `yaml:3730-3855` | none |
| Browser/Computer — control, takeover, return | `docs/CHROME-BROWSER-LLD.md` | none (`browser_*.rs` are `#[allow(dead_code)]`) |
| Connections / channel bindings — 9 operations | `yaml:2601-2730` | none |
| Runtimes — list, pair-token, revoke, incarnations | `yaml:1637-1685` | none |
| Agent install/update, binding configuration, native auth, native config | `yaml:1798-1966` | none |
| Backups — list, create, get, restore | `yaml:187-232` | none (CLI-only by design, `backup_operator.rs:3-8`) |
| Workspace roots — `POST`, `PATCH` | `yaml:3400-3418` | none (prose marks them target-only) |

Conversely, `POST /v1/stream` (`presentation_operator.rs:31`) is served and documented in
prose but is not an OpenAPI operation.

`docs/API.md` repeats the problem: 56 routes sit in "served" code blocks (`:61-71`,
`:165-172`, `:270-280`, `:317-339`, `:736-746`, `:1193-1200`) while being unimplemented.

**Fix (top of the local queue):** mark every unpublished-but-unmounted operation with a
machine-readable `x-litecowork-status: TARGET_ONLY`, generate a served-route manifest by
parsing the composed router, and assert in a validator that the two sets agree. This
converts the largest class of hidden drift into a red/green signal.

## 2. Domain modules that no production path calls

| Module | Evidence | Consequence |
|---|---|---|
| `domain-responsibility::notifications` (`decide_notification`, `NotificationFacts`, `QuietHours`) | called only from `crates/domain-responsibility/src/notification_tests.rs` | `notification_deliveries` has **no writer**; `GET /v1/notifications` is permanently empty |
| `domain-responsibility::occurrences::{decide_occurrence_transition, settle_from_task_outcome}` | called only from `occurrence_tests.rs:141-158` | every materialized occurrence stays `STARTED` forever; `max_concurrent_occurrences` eventually deadlocks the Automation and Workspace archive is blocked permanently (`workspace_archive.rs:25-27`) |
| `domain-invocations::capability_bridge` (4-way route qualification) | no route imports it | E04-S06 has a classifier but no qualification path |
| `domain-task::delegation_benchmark` (1 380 lines) | no benchmark maps to it | `docs/BENCHMARKS.md` has no E05-S05 benchmark |
| `storage-sqlite::capability_dispatch::admit_capability_dispatch` (atomic CREATED→DISPATCHED + ApprovalUse + Effect) | only test callers | `invocations.rs:48-52` blanket-rejects dispatch with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`; `docs/CAPABILITY-INVOCATIONS.md:127-141` still claims the gate does not exist |

## 3. Quiet-hours contract is mutually unreachable

| Shape | Where |
|---|---|
| `{timezone, weekdays[], start_local_time, end_local_time}` | persisted (`crates/storage-core/src/notification.rs:41-48`), OpenAPI `QuietHours` (`yaml:4652-4663`), validated by `storage-sqlite/src/notification.rs:534-547` |
| `{start, end, timezone}` | domain parser (`crates/domain-responsibility/src/notifications.rs:129-170`) — `object.len() != 3` → `InvalidDefinition` |

A stored preference can never be evaluated, and the domain has no weekday concept, so the
persisted `weekdays` restriction would be ignored even after key alignment. Additionally
`notifications.rs:287` swallows an invalid window with `unwrap_or(false)`, i.e. **fails
open** and delivers during an unparsable quiet window.

## 4. Occurrence and Task lifecycles are mostly unimplemented

- Occurrence writes exist in exactly three production statements: create PENDING
  (`lib.rs:13809`), claim (hard-coded `version=2`, `claim_epoch=1`, `lib.rs:13829`),
  STARTED (`lib.rs:13874`), and SKIP (`automation_host.rs:561`). **Missing:**
  `CLAIMED→PENDING` requeue, `→WAITING_DEPENDENCY`, and every terminal settlement.
- Task transitions implemented: 2 of ~25 (`READY→RUNNING` behind the isolation gate at
  `lib.rs:13081`, `RUNNING→BLOCKED` at `execution.rs:607`). No writer for VERIFYING,
  COMPLETED, FAILED, NEEDS_USER, INCOMPLETE, WAITING_USER, PAUSE_REQUESTED, PAUSED,
  CANCEL_REQUESTED, CANCELLED.
- `admit_step_attempt`, lease renew and lease release are unconditional `Err`
  (`execution.rs:94-98`, `:123`, `:135`); there is **no production `INSERT INTO attempts`**
  (all attempt inserts are inside `#[cfg(test)]` modules).
- `settle_from_task_outcome` maps `INCOMPLETE` to terminal FAILED
  (`occurrences.rs:276`), contradicting `docs/STATE-MACHINES.md:97-99` and `:1337-1344`
  ("recoverable failure does not falsely settle an occurrence").
- Automation cursors are never deleted: `workspace_archive.rs:21-23` blocks archive while
  any `automation_cursors` row exists, and no `DELETE FROM automation_cursors` exists
  anywhere — so **any** Workspace that ever enabled an Automation can never be archived.

## 5. Contract drift that breaks generated clients

- **16 daemon error codes** are absent from `docs/schemas/error-codes.schema.json`
  (`BUSY`, `CAPACITY_EXCEEDED`, `CONSENT_REQUIRED`, `IDEMPOTENCY_CONFLICT`,
  `LIMIT_EXCEEDED`, `PROVIDER_UNAVAILABLE`, `RESOURCE_IDENTITY_CHANGED`,
  `RESOURCE_IDENTITY_UNAVAILABLE`, `RESOURCE_INDEX_UNAVAILABLE`, `ROUTINE_ARCHIVE_BLOCKED`,
  `SCAN_UNAVAILABLE`, `SCAN_UNSUPPORTED_PLATFORM`, `STORE_BUSY`, `UNAVAILABLE`,
  `UNSUPPORTED_PROJECTION`, `USER_REQUEST_RESPONSE_TOO_LARGE`) although `docs/API.md:2181`
  states the schema is canonical and returned without renaming.
- `Idempotency-Key` is declared but **not enforced** on
  `POST /v1/capabilities/{id}/connect-or-install` and `.../test`
  (`apps/litecoworkd/src/discover_operator.rs:33`, `:37`) — the only two of 225.
- Occurrence/health blocker objects violate the `Blocker` schema: storage persists
  `[{"code": reason}]` (`automation_host.rs:475`, `:524`, `:563`) while `Blocker`
  (`yaml:7100`) requires five fields with `additionalProperties: false`; the codes used
  (`MISFIRED_SLOT_SKIPPED`, `ROUTINE_NOT_ACTIVE`, `ROUTINE_REVISION_DRIFTED`,
  `SCHEDULE_PLAN_UNAVAILABLE`, `CLAIM_WINDOW_UNAVAILABLE`) are not in the error-code enum.
- `limit` maximum is 200 in the shared `Limit` component but 100 in the daemon for
  `/v1/agent-registry`, `/v1/conversations`, `/v1/user-requests`.
- OpenAPI path parameters are camelCase while every daemon route is snake_case (108 paths
  differ), so no mechanical router↔spec diff is possible today.
- `GET /v1/user-requests` and `GET /v1/needs-you` omit the `X-Workspace-ID` parameter
  their sibling routes declare.

## 6. Desktop UI drift (all locally fixable)

- **Library shows a false empty state after a failed read**: `App.tsx:3839` renders
  "No files in this Workspace yet" while `App.tsx:1041-1043` swallowed a failed
  `list_resources` and cleared the list — `docs/EXPERIENCE.md:38` forbids inferring an
  empty result from a failed read.
- **Green "Completed" pill** on Task detail (`App.tsx:2827`) asserts verified completion
  while the same screen says verification is not shown, contradicting `docs/API.md:1113`.
- **"Sent" notification is styled as success** (`NotificationsPage.tsx:51`) although
  `docs/EXPERIENCE.md:708` says a sent notification is only a transport acknowledgement.
- **Discover and SkillProposal pages are built but unreachable**: `src/discover/DiscoverPage.tsx`
  and `src/skills/SkillProposalsPage.tsx` are never mounted even though
  `GET /v1/discover/search` and `/v1/skill-proposals` are served; the Discover "Connect"
  button has no handler at all.
- **Inert chrome buttons**: `App.tsx:2050-2073` (account "···" and "?" Help) are enabled
  with no handler.
- **No reduced-motion app setting and no dark/high-contrast theme**, both required by
  `docs/DESKTOP-SHELL-LLD.md:41,44,132,145`.
- **Tauri bridge gaps**: `commit_resource_revision_upload` never compares the committed
  Resource id with the edited Resource (`src-tauri/src/lib.rs:3266-3297`);
  `import_resource`/`create_resource_upload` forward an unvalidated `media_type` and a
  weakly validated `workspace_id` (`:3323-3341`, `:3367-3411`).
- `src/resources/local-model-inventory.ts` is orphaned although
  `GET /v1/capabilities/local-models` is served; `PlaceholderPage` (`App.tsx:3155`) is
  unreachable dead code that would render a false "Nothing in X yet" for any future route.

## 7. Docs, backlog and review-packet integrity

- **44 of 77 epic stories** have an `Implement`/`Acceptance` obligation that no longer
  appears in `implementation/backlog.json`; the epics are the "read first" surface, so a
  new agent inherits stale obligations.
- **E13-S03 requires** a PASS/BLOCKED/NOT_APPLICABLE verdict for every flow and
  benchmark; `docs/FLOWS.md` (144 flows) and `docs/BENCHMARKS.md` (90 benchmarks) carry
  **zero** verdicts, while `implementation/coverage.csv` labels every row "heuristic for
  initial planning".
- **Backlog dependency graph is inconsistent** in 14 stories (e.g. `E13-S01 IN_REVIEW`
  depends on `E10-S02 IN_PROGRESS`; `E09-S05 IN_PROGRESS` depends on `E08-S05 PLANNED`).
- **Status drift**: E03-S08/S09/S10 are `PLANNED` but have code and review packets;
  E09-S01 is `PLANNED` but has served routes.
- **15 stories have no review packet**, including every E11/E12 story and the four
  contract-gated stories that most need one.
- **Broken references in review packets** (the artifacts a reviewer trusts):
  `implementation/reviews/E13-S04.md:25` cites a non-existent `docs/RELEASE.md`;
  `implementation/reviews/E10-S02.md:128` and `implementation/epics/E10.md:138` cite
  `crates/ports/skill_test_runner.rs` (the `crates/ports/` directory does not exist);
  `implementation/reviews/E03-S01.md:447` cites `docs/en/reference/kimi-command.md` (an
  upstream URL fragment, not a repo path); `implementation/reviews/E13-S02.md:11` still
  says the harness reads up to `sqlite-v26.sql` while v27 is live.
- **`docs/CONNECTORS-WORKFLOWS.md:768-800` and `docs/LEARNING.md:612-630` describe a
  crate layout that does not exist** (`crates/domain-connection/`, `crates/application/`,
  `crates/ports/`, `crates/adapters/*`, `crates/domain-automation/`, `crates/domain-learning/`).
  A team following them builds the wrong module graph.
- **`implementation/V1-SERVED-CAPABILITY-INVENTORY.md` is stale**: it claims 19 merged route
  factories (actual 36) and every cited line anchor is wrong; it also claims
  `GET /v1/approvals` is target-only while it is mounted
  (`approval_operator.rs:79`).
- **44 SQLite tables are never touched by production code** (all `channel_*`, `connections`,
  `pairing_tokens`, `handoffs`, `capability_locks`, `budget_reservations`,
  `provider_circuit_states`, `agent_lifecycle_operations`,
  `agent_installation_observations`, `agent_binding_configurations`,
  `agent_control_descriptors`, `replication_*`, `event_archive_segments`, …), and
  **77 of 145 registered events have no producer**, yet `docs/STORAGE.md:495-520` and
  `docs/EVENTS.md:178-280` read as if they are live.
- `docs/FLOWS.md` names 18 flows' events that are never emitted (for example F30 names
  `task.pause.requested.v1`, `task.paused.v1`, `task.resumed.v1` — none exist).
- `docs/LEARNING.md:520-533` and `docs/COWORKERS-TARGET.md:220-227` publish route families
  that do not exist and disagree with each other about being "target".

## 8. Ranked local work queue (no owner, provider, or new contract)

**Closed in the same pass as this report (2026-10-21):** queue item 1 (served-vs-target
drift gate, `scripts/audit_served_routes.py` + `implementation/served-route-inventory.json`
+ a `scripts/check.sh` stage), item 6 (31 missing ErrorCode values added to
`docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json` and
`docs/SCHEMAS.md`, plus a validator rule that every `operator_error` code and every
AutomationOccurrence blocker code is registered), item 7 (`Idempotency-Key` is now enforced
on `POST /v1/capabilities/{id}/connect-or-install` and `.../test`), and the first four
truthfulness items of item 8 (the Library no longer claims an empty Workspace after a failed
read, the Task "Completed" pill no longer implies verification, a transport-acknowledged
notification is no longer styled as a success, and the two inert chrome buttons are gone).

1. **Served-vs-target drift gate** — **DONE** — mark unserved OpenAPI operations
   `x-litecowork-status: TARGET_ONLY`, generate a served-route manifest from the router,
   assert agreement in a validator, and regenerate `V1-SERVED-CAPABILITY-INVENTORY.md`.
   Files: `docs/schemas/operator-api.openapi.yaml`, `scripts/audit_v1_contracts.py`,
   `scripts/validate_architecture.py`, `implementation/machine-inventory.json`,
   `implementation/V1-SERVED-CAPABILITY-INVENTORY.md`.
2. **Occurrence settlement from a Task outcome** — one storage writer reusing
   `settle_from_task_outcome`, emitting `automation.occurrence.status.changed.v1`; unblocks
   occurrence terminal state, `max_concurrent_occurrences`, Runs history and Workspace
   archive. Files: `crates/storage-sqlite/src/automation_host.rs`,
   `crates/storage-sqlite/src/lib.rs`, `crates/domain-responsibility/src/occurrences.rs`.
3. **Automation cursor removal on disable** — restores Workspace archive for any Workspace
   that enabled an Automation. Files: `crates/storage-sqlite/src/automation_host.rs`,
   `crates/storage-sqlite/src/coworkers.rs`, `crates/storage-sqlite/src/workspace_archive.rs`.
4. **Quiet-hours shape alignment** — teach the domain the persisted shape including
   weekdays, and fail closed on an unparsable window.
   Files: `crates/domain-responsibility/src/notifications.rs`. **DONE** — the domain now
   parses the contract shape (`timezone`, `weekdays`, `start_local_time`, `end_local_time`),
   scopes the window to its start weekday for overnight intervals, and rejects an
   unparsable window instead of treating it as "not quiet".
5. **`notification_deliveries` writer + `decide_notification` wiring** — closes a served
   but always-empty endpoint. Files: `crates/storage-sqlite/src/notification.rs`,
   `crates/storage-sqlite/src/automation_host.rs`, `apps/litecoworkd/src/notification_operator.rs`.
6. **Error-code enum completion + a validator rule** that every emitted code is in the enum.
   **DONE** (31 codes added; the rule now fails the build on a new unregistered code).
   Files: `docs/schemas/error-codes.schema.json`, `scripts/validate_architecture.py`.
7. **`Idempotency-Key` enforcement on the two capability commands.** **DONE.**
   Files: `apps/litecoworkd/src/discover_operator.rs`.
8. **Library false-empty-state fix, "Completed" pill, "Sent" styling, inert chrome buttons,
   Discover/SkillProposal mounts, bridge media_type/workspace validation.** The first four
   are **DONE**; Discover/SkillProposal mounts and the bridge validation remain.
   Files: `apps/litecowork-ui/src/App.tsx`,
   `apps/litecowork-ui/src/notifications/NotificationsPage.tsx`,
   `apps/litecowork-ui/src-tauri/src/lib.rs`.
9. **Backlog/epic/status truthfulness pass** and repair the broken review-packet
   references. Files: `implementation/backlog.json`, `implementation/epics/E*.md`,
   `implementation/reviews/*.md`.
10. **Flow/benchmark verdict columns** for the E13-S03 gate.
    Files: `docs/FLOWS.md`, `docs/BENCHMARKS.md`, `implementation/QUALIFICATION-MATRIX.md`.

## 9. Owner decisions that unblock multiple stories

- **D1 — launch harness (corrected by owner direction, 2026-10-21).** My first phrasing
  ("Codex CLI only, the other 14 become unsupported rows") was wrong: `implementation/SCOPE.md`
  requires the complete framework and module integration for **every** roster entry, and
  `docs/AGENT-CONTROL.md` forbids Core branching on agent brand names. The owner revised the
  roster to **25 entries** in
  [`AGENT-PORTFOLIO-2026-10-21.md`](AGENT-PORTFOLIO-2026-10-21.md): Gemini CLI is dropped
  (Google moved the consumer path to **Antigravity CLI**), OpenCode is tracked on its **v2**
  line, and Command Code, Aider, Pi, Oh My Pi, Muse Code, Grok Build, MiMo Code, Devin CLI,
  MiniMax Code, and Amp are added. The qualification-first set stays the four agents named in
  `SCOPE.md` (Claude Code, Codex CLI, OpenCode, Cline CLI) plus Antigravity CLI. What I decide
  is the **build order and the matrix**, not the scope: every roster entry gets an adapter
  path and a per-exact-version/OS matrix row, and an entry with no adapter is reported as
  unqualified rather than removed. Unblocks E03-S01..S11, E05-S01, E05-S05, E07-S00,
  E13-S03.
- **D2 — Environment isolation mode.** Linux Bubblewrap only for V1
  (`crates/linux-process-scope`, `crates/local-environment-staging` already pass 107
  tests); Windows/macOS marked unsupported. Unblocks E07-S00/S01, E02-S03, E03-S03/S04.
- **D3 — backup API surface.** Keep `create`/`restore` CLI-only (a restore replaces
  installation state and must run with the daemon stopped) and remove those two OpenAPI
  operations; optionally serve the two GETs. Unblocks E13-S01.
- **D4 — LiteSPM authority.** If no LiteSPM endpoint/credentials exist, publish an
  explicit "LiteSPM authority unavailable in V1" section in `docs/CAPABILITY-FABRIC.md`
  and ship every LiteSPM-dependent surface truthfully unavailable. Unblocks E04-S02/S04/S06,
  E10-S01/S02.
- **D5 — notification delivery shape.** In-app delivery history plus one local OS
  notification bridge; no email/Telegram (that is E11-S04, post-V1). Unblocks E09-S04 and
  the Needs You deep-link gap in E02-S04.

## 10. Verdict

**Roster correction (2026-10-21).** The agent portfolio was revised by owner direction to
25 entries; `implementation/QUALIFICATION-MATRIX.md` now carries one row per roster entry,
and 17 of them are observed on this host (aider, antigravity, cline, codex, command-code,
copilot, droid, grok-build, junie, kiro-cli, mimocode, mistral-vibe, muse-code, oh-my-pi,
opencode, pi, qwen). An observation is still not a qualification.

The repository is structurally sound: every machine contract validates, the daemon is
authenticated and owner-scoped on every route, and the local storage layer is
transactional with real rollback evidence. The failures are **coverage and honesty**
failures, not correctness failures: 47 published operations are not served, 44 tables and
77 events have no producer, five domain modules are dead, one persisted contract
(quiet hours) cannot be parsed, and 15 review packets cite paths that do not exist. The
single highest-value move is to make the served surface machine-checkable, then to close the
occurrence/notification lifecycles that the rest of the epics depend on.
