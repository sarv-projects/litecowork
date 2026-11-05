# Digest archive-02

Files: 40 read / 41 listed | Lines read: 8,530 of 18,170 | Bytes: ~520 KB of 1,236 KB

Bucket = `docs/archive/legacy-implementation-c7e9c05/` — the versioned historical docs of the
pre-reset implementation. Every file self-labels as evidence/handoff, not current truth.
Recurring status vocabulary matches the repo: **TARGET / CODE / SERVED / INTEGRATED /
QUALIFIED**, plus `BLOCKED` as a release verdict. Backlog enum: PLANNED/READY/IN_PROGRESS/
IN_REVIEW/BLOCKED/ACCEPTED. **No story is ACCEPTED in any snapshot in this bucket.**

## Per-file notes

### AGENT-PORTFOLIO-2026-10-21.md (136)
Roster revision declaring itself the supersession of `AGENT-PORTFOLIO-2026-10-16.md` (15 entries,
"kept as the historical snapshot"). 25 entries, checked 2026-10-21. Roster ladder: Discovered →
Profile observed → Module implemented → Protocol handshake → Qualified support. Qualification-first
set: Claude Code, Codex CLI, OpenCode (v2), Cline CLI + Antigravity CLI (Google slot). Gemini CLI
dropped (consumer serving ended 2026-06-18); OpenCode tracked at v2 `2.0.26`/`2.0.28` although the
ACP Registry still advertises `1.18.34`. Version pins worth keeping: Codex schema bundle `0.162.0`
vs observed host `0.163.0` ("observation, not a qualification"); copilot `@github/copilot@1.0.98
--acp`; Qwen registry `0.25.2` vs probed `0.25.1`; Goose `1.54.0`; Vibe `2.26.1` (`vibe-acp`);
Cursor `2026.10.14` (`cursor-agent acp`); Kimi `1.52.0` vs separate `kimi-code` 2.2.0 (ACP 0.23);
Junie `3659.4.0`; Auggie `@augmentcode/auggie 0.36.0`; MiniMax `0.2.7`; Amp ACP wrapper `0.9.0`;
Factory Droid "launch contract unresolved" (`droid@0.243.0 exec --output-format acp-daemon` vs
matrix `0.242.0`); Antigravity has no native ACP (`antigravity-cli#31` open). **Axis B** hosted
providers: "not yet qualified for any provider… no passing row". 2026-10-22 addendum adds
HorizonCode as a first-party candidate, explicitly "not a privileged Core planner".

### CURRENT-RUN.md (10,160) — PARTIAL, see Coverage check
Reverse-chronological handoff changelog, 2026-10 → 2026-10-25. Sections actually read:
- 2026-10-25: Conversation submit serialized from durable version (fixes
  `CONVERSATION_TURN_EVENT_REVISION_MISMATCH`/400); submit replay survives AgentBinding drift;
  **removal of `ConversationTurnStore::admit_conversation_turn`** (could create a durable STARTING
  session and mark a turn RUNNING from inventory with no observed ready handshake); session
  admission safety boundary → route persists user message, settles turn `FAILED`, returns **503
  `DEPENDENCY_UNAVAILABLE`**; `PATCH /v1/conversations/{conversation_id}/agent-binding`
  (If-Match + Idempotency-Key, `conversation.agent_binding.changed.v1`); Automation starter cards;
  Workbench Start screen (Browser/Terminal/Computer "unavailable with explicit provider reasons,
  no simulated tool output"), build 170 modules, JS 1,017.96 kB.
- 2026-10-24: Discover surfaces (Explore/Installed/native/Managed/Connections), reason-code typo
  fix `LITESPOM_…` → `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`.
- 2026-10-22: four competitive-gap designs (event-trigger delivery, connector breadth, learned/
  imported skills, cross-agent usage) = design only, functions F07/F08/F09/F11; competitor
  verification (Grok Bot 2026-08-11, Claude Cowork cloud-first, OpenAI dots, Meta Muse/Sentinel);
  second exhaustive doc reconciliation = **92 findings applied**; whole-UI audit = **48 claims:
  18 implemented, 15 partial, 15 absent**, UI suite 497/497; occurrence settlement producer
  `settle_started_occurrences_from_task_outcomes` (storage-sqlite 588/588, litecoworkd 526/526).
- 2026-10-21: hosted-provider "Axis B" gap (zero "cloud" hits in four owner docs), roster 15→25,
  E09-S04 first notification producer.
- Tail block dated 2026-10-20 (out of order relative to 10-25 blocks): backlog 67 stories
  (**1 IN_REVIEW, 22 IN_PROGRESS, 44 PLANNED**); desktop V1 60 stories **0 ACCEPTED**; 138 tracked
  modified / 103 untracked / 0 staged; validators 193 md files, 103 docs, 6,744 rows, 145 typed
  events; disk 849 GB Linux free, Windows C: 57 GB.
Validator counts drift across entries (145 events; 67 stories with 6,744 / 7,179 / 7,193 / 7,218
rows; 107 or 103 docs) — treat each as dated, not current.

### NEXT-SESSION-HANDOFF-2026-10-13.md (316)
Refreshed 2026-10-16. Pins `main = HEAD = origin/main = de73535cf27af347d9d5413ccaffc81bef2e78be`.
Active slice E02-S03 (Resource index/search/GC, ContextDocument purge, v17–v19 migrations):
storage-sqlite **303/303**, authenticated daemon purge restart/replay **1/1**, WSL2 x86_64, Rust
`1.98.1`. Backlog E01–E10+E13 = 60 stories: 1 IN_REVIEW, 19 IN_PROGRESS, 40 PLANNED, none
ACCEPTED; E11–E12 post-V1. Defect map A01–A13 / D01–D12 lives in
`V1-IMPLEMENTATION-REAUDIT-2026-10-13.md`. U15 = ContextDocument concurrent-edit/revoke/delete;
U16 = Goal progress from verified Task outcomes — neither is an automatic-learning test (that is
E08-S03, UIV1-120–UIV1-127). Gates G0–G3. OpenIntelligentUI reference pinned at
`f6e4388b26a64b9a0714943b08a1ce622b924eec`, license checked MIT; its inline/eval CSP is
explicitly **not** an approved RichPresentation boundary. Disk: `/` 903,853,826,048 B free;
`target/` 7,677,090,567 B; UI `dist/` 993,835 B; `node_modules/` 100,066,382 B.

### PROCESS.md (99)
Agent-and-one-reviewer agile process. WIP = 1 story awaiting review + ≤1 spike. Review-packet
template (story IDs, commit/base, contracts, commands, adapter versions, fixture digests, failure
checks, cost, rollback, owner decision). Change management: "A discovered contradiction is a
contract task before implementation invents behavior." 2026-10-13 addendum: no parallel status enum.

### ROADMAP.md (182)
Epic→gate table (E01/E02/E03 G0/G1, E04–E10 G2, E13 G3, E11/E12 post-V1). Second-pass audit matrix
using BOUNDED/PARTIAL/NOT DONE: ordinary Conversation list PARTIAL; **provider-backed chat send NOT
DONE**; Coworker-owned Conversation NOT DONE ("`owner_coworker_id` currently exists for Environment
ownership and does NOT establish Coworker-owned Conversations"); automatic quiet private memory NOT
DONE; StandingResponsibility group NOT DONE; scheduled/provider TriggerHost NOT DONE; unified right
Workbench shell NOT DONE. Agent-module order E03-S06→S11 then E04-S06; connector order
(SecretStore → one read-only connector → one reconciled write → …). "A grep hit is not
implementation proof."

### SCOPE.md (158)
V1 = production-qualified desktop/local, not "finished alpha". Two meanings of cloud separated:
moving **our Runtime** to cloud = post-V1; a **hosted agent provider** behind Connection +
CapabilityGrant + Task-scoped SecretLease = V1. 2026-10-22: ordinary New Chat must work without
Task/Coworker materialization; free local core without mandatory account is a release requirement.
Gate table G0–G3 with required evidence and owner decision. Per-OS qualification is separate; "No
OS receives a supported label without a real run on that OS." Native-agent completeness list
(discover/install/update/repair, native sign-in, model/session options, inputs, slash/reference,
config, extensions, cancel/resume/steer/quiescence).

### STACK.md (140)
Pins: Rust `1.98.1`, Python `3.13.12`, uv `0.12.23`, Node `24.21.0`, pnpm `12.10.1`; workspace MSRV
`1.88` (from `time`). Rusqlite `0.40.2` "provisional first adapter"; SQLx a "less-complete
prototype". Tauri 2 / React 19 / TS 5.8 / Vite 6; openapi-typescript `7.13.0` freshness checked in
every production build. Known gap: frozen-lockfile install stops at pnpm's ignored `esbuild`
lifecycle policy → dependency bootstrap "not yet qualified from a clean checkout". Spikes SP01–SP07
with pass/fail outputs; SP02 (driver), SP03 (IPC incl. Windows named pipe current-logon-SID DACL,
macOS security-scoped bookmarks).

### V1-CONTRACT-CHECK.md (41)
Generated by `scripts/audit_v1_contracts.py`; **0 blocking structural findings**; 229 OpenAPI
operations specified, 158 supplemental UIV1 acceptance definitions, 68 backlog stories. Operation
IDs present: `getAgentControlDescriptor`, `submitConversationTurn`, `retryConversationTurn`,
`getConversationPresentation`; 10 `/v1/browser/*` routes "NO: proposed". Explicit caveat: OpenAPI
count ≠ mounted handlers.

### V1-DESIGN-DEPTH-AUDIT.md (320)
Generated by `scripts/audit_v1_design_depth.py`. 243 md files / 92,947 lines / 3,528 headings;
20 HLD→domain→LLD→epic authority rows; 254 named signatures in F01–F14; **187 settings ops, 187
exact matches, 0 unmapped**; 229 OpenAPI methods; **107 baseline SQLite tables**; 0 conflicting
signatures; 0 blocking problems. Includes a full per-file table with SHA-256 prefixes (useful as a
dated corpus fingerprint). States plainly: matching names prove no semantics; no runtime/UI run.

### V1-EVERY-MD-AUDIT-2026-10-12.csv (143)
Machine twin of the md audit: 142 rows × (File, Kind{Architecture,ADR,Epic,Implementation,Other,
Review}, Lines, Bytes, full SHA256, Title, Headings, RelativeLinks, ExternalLinks, Flags, Samples).
Largest entries: `docs/FLOWS.md` 4,421 lines/303,613 B/150 headings; `implementation/CURRENT-RUN.md`
6,223 lines/451,050 B at that date (it has since roughly doubled).

### V1-EVERY-MD-AUDIT-2026-10-12.md (179)
142 files, 53,544 lines, 3,528,376 bytes, **0 broken relative links, 0 unclosed fences**. Flags are
"review candidates, not proven defects": AGENTS.md 2, API.md 2, BENCHMARKS 2, CURRENT-RUN 4,
AGENT-CONTROL 1, REFERENCE-IMPLEMENTATIONS 1, SCHEMAS 1, TESTING 1, UI.md 1, implementation/README 1,
ROADMAP 8, E01-S04 1. Candidate terms center on `PLACEHOLDER`, `UNKNOWN`, "not yet built".

### V1-PRODUCT-ACCEPTANCE-MATRIX.md (298)
The densest source-evidence file in the bucket. Truth labels mapped to the release contract
(TARGET=`TARGET_ONLY`, CODE=`CODE_IMPLEMENTED`, INTEGRATED=`INTEGRATION_PROVEN`). **All 11 user
journeys BLOCKED at 2026-10-22**: Chat, Native agent, Coworker, Computer/Workbench, LiteSPM, Task,
Multi-agent, Automation, Memory, Artifacts, Desktop/release ("BLOCKED for V1 GA"). Each area carries
Exists / Remains / Evidence-to-flip with file:line: `conversation_operator.rs:441-508`, `:766-772`;
`ConversationsPage.tsx:583` (unconditional `disabled` Send, no handler), `:162-174` (hard-coded null
descriptor → `NEEDS_REFRESH`); `crates/storage-sqlite/src/conversations.rs:398-400` and `:590-594`
(every `TurnCommand::Start` refused); only the USER message insert exists at `:269`; no
`submit_conversation_turn`/`stop_conversation_turn` Tauri command. `codex_profile_probe.rs` hard-sets
every control to `Unknown` → `NOT_ELIGIBLE`, so dispatch would refuse even after 1–8. Task:
`lib.rs:245-251` returns `TASK_PLANNING_ISOLATION_UNAVAILABLE`; Effect `STARTED` deliberately
disabled (`STORAGE.md:895-897`, `:1135-1140`). LiteSPM: real daemon `litespm v0.3.0`, JSON-RPC +
`daemon.handshake` with `SO_PEERCRED`, `tools.list`, `host.detect_config`, **22 registered methods**;
no `LiteSpmClient` exists; routes refuse `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`. 2026-10-25 device
observation: OpenCode `2.0.32` native-Windows smoke (200 on `/api/provider`, 9 model entries) —
"the CLI itself, not the LiteCowork adapter"; Codex absent. Automation settlement producer is CODE
with no USER evidence.

### epics/E07.md (302)
G2. E07-S00/S01 IN_PROGRESS, S02–S05 PLANNED. Source progress is unusually explicit:
`local-environment-staging` `PreparedOnly`; `linux-process-scope` cgroup v2 observer requiring
`populated 0` + Bubblewrap launcher (fresh tmpfs, ro inputs, rw outputs, network namespace denied);
`apps/litecoworkd/src/environment_isolation.rs` (`IsolationAttestation`, `QuiescenceObservation`,
`NOT_ENFORCED`/`UNKNOWN` both refuse), `environment_provider.rs`, `browser_control_service.rs`
(epoch fencing, atomic takeover), `browser_consent.rs` (refuses default Chrome profile, requires
managed profile + **Chrome 144+**), `environment_action_batch.rs`, `environment_office_qualification.rs`,
`environment_computer_use.rs`. Windows 11 build 26200 evidence: `windows-local-security` 36/0,
`local-environment-staging` 14/0, daemon E01 3/0, E02 WorkspaceRoot 3/0 — helper-crate CODE only;
Windows support cell stays blocked.

### epics/E11.md (98) / epics/E12.md (44)
Post-V1 cloud continuation (E11-S01–S05) and Remote Runtime (E12-S01–S02), all PLANNED, explicitly
outside the desktop V1 gate. E11 depends on E13-S04; acceptance language already fixes the hard
rules: "Only permitted domain events/blobs sync; no DB/native-private-state copy"; "Partition never
permits two consequential writers."

### epics/E13.md (110)
G3. All four stories `IN_REVIEW` as of 2026-10-20 with a what-exists/what-is-not-proven table:
backup `11 passed`; soak 745 iterations / 14 restarts; platform matrix 3 cells `PASSED`, every other
cell `BLOCKED`; packaging 8/8 + secret scan 0 findings. Rule stated: "a matrix cell is `PASSED` only
when the probe ran and observed success."

### reviews/E01-S01.md (244)
`IN_REVIEW`; commit `d50ab88`, base `a1fb178`, evidence 2026-10-06. `scripts/check.sh` stage order,
`--preflight`, `LITECOWORK_CARGO_JOBS`, exit 2 on unsupported arg. Exact messages:
`Required pinned tool uv was not found on PATH.`, `Expected uv 0.12.23, found 0.0.0`,
`Expected an isolated pinned Python environment…`. Tests: `tests/test_check_script.py` 5/5; guard
suite 7/7; root Python 38/38; `litecoworkd --test cli` 4/4 (incl. not echoing a secret-shaped arg).
`--version` → `litecoworkd 0.1.0`. Failures recorded honestly: preflight 47.8 s then stale generated
`V1-CONTRACT-CHECK.md`; full gate exit 2 at `git diff --check` from owner `audit-inventory.csv` CRLF;
2026-10-18 hosted CI (`25ea383`) failed Linux rustfmt and Windows `local-environment-staging`.

### reviews/E03-S03.md (108)
Partial CODE, stays `IN_PROGRESS`, base `b003e9e03a55ce2e019d0a006054df03931a8750`. Planning
service/storage paths exist; reservation/activation refuse with internal
`TASK_PLANNING_ISOLATION_UNAVAILABLE` before any write; "a caller-supplied AgentSession ID is not
authority"; no native tool invokes `submit_initial_plan`; plan-acceptance transaction recorded as
unbuilt/untested. A09 Task-create receipt fix in `5fb33ea` (receipt checked before resolving mutable
default binding; 200-byte idempotency key).

### reviews/E04-S01.md (203)
`PARTIAL`. TaskPlanning grants only, `workspace.resource.read` only, `APPROVE_SAFE`, ≤24 h, non-
disclosing `NotFound` for missing/foreign/mismatched/inactive refs. Conversation grants denied
(persisted scope cannot pin a live ConversationTurn). Counts: trust::tests 24/24 → 26/26 → **28/28
(391 filtered)**; route tests 3/3 (201/200, `If-Match`, `X-Correlation-ID`, `Cache-Control: no-store`);
daemon trust 27/27; effect_evidence denials 1/1 each. Missing: PolicyEngine context loader, approval
producer route, grant-specific ApprovalUse, Attempt grant issuance, CapabilityBroker admission,
credential-egress/SSRF-DNS-rebinding. SecretStore API is write-only by design.

### reviews/E04-S05.md (304)
`PARTIAL`. Verifier settlement: a PASSED run must carry ≥1 Evidence whose refs equal the Evidence IDs
exactly; stale subject head blocks PASSED; cancelled closes `INCONCLUSIVE/TASK_CANCELLED`. New served
route `GET /v1/tasks/{taskId}/verification-runs` (`listTaskVerificationRuns`), keyset pagination
`started_at DESC, verification_run_id DESC`, base64url cursor binding Workspace/Task/limit, 1–200,
no-store, projection excludes `failure_code` and `version`. Counts: verification_runs 6/7/8/9,
effect_evidence 14/14, domain-effects 13→**14**, artifact_append 3/3, artifact_tests 16/16, route
test 1/1. `assess_completion_with_evidence` is a pure evaluator **not consumed by TaskService**.

### reviews/E06-S01.md (286)
Partial. `POST /v1/resources/{resource_id}/archive/preview` (`previewResourceArchive`), closed request
`resource_revision_id` + `content_digest`, 16 MiB cap, metadata-only. Worker compiled **only** on
Linux behind non-default feature `source-zip-worker`, launches `/usr/bin/python3 -I` with cleared env,
24 MiB request / 8 MiB response caps, 15 s deadline; release sidecar builds without the feature, so
package preview stays unavailable. `create_derived_resource` + v25 append-only `resource_edges` exist
but are **not** wired to ZIP extraction; no extraction Effect type, no child deletion lifecycle, no
provenance shape in DATA-MODEL. Tests: zip suite **63/63**, pytest **80 passed + 61 subtests**,
Rust archive-worker 8/8, feature-off guard 1/1, route 6/6.

### reviews/E06-S02.md (211)
Partial but the strongest served slice. New crate `crates/domain-rag` (`provider.rs`, `scope.rs`,
`grounding.rs`, `limits.rs`, `local_lexical.rs`, `index_state.rs`, `local_model.rs`, `placement.rs`).
Routes: `POST /v1/resources/retrieval` (`groundedRetrieval`), `GET /v1/resources/{id}/index-state`
(`getResourceIndexState`), `GET /v1/capabilities/local-models`. Statuses `GROUNDED` /
`INSUFFICIENT_EVIDENCE` (`NO_AUTHORIZED_MATCH`, `PINNED_REVISION_NOT_INDEXED`) /
`PROVIDER_UNAVAILABLE`; citation binds resourceId+revision+digest+TEXT_SPAN; `EMBEDDED_INSTRUCTION_DETECTED`
poisons a claim; `PROVIDER_INDEX_GENERATION_INCOMPATIBLE` on parser-generation mix. Budgets: 20
results, 2 KiB excerpt, 16 KiB total, 2 KiB query. Tests 38/38, routes 8/8, whole daemon **504/504
(1 ignored, 315 s)**, UI 18/18. Provider is deterministic lexical only — "not embeddings, not vector
recall, not reranking"; only current head indexed; no Tauri command yet.

### reviews/E06-S04.md (106)
Inventory only, fail-closed: `NOT_CONFIGURED` / `LOCAL_MODEL_PROVIDER_NOT_QUALIFIED`,
`inventoryOnly: true`, `surface: AGENT_SCOPED`, `agent_binding_policy:
AGENT_MODULE_COMPATIBILITY_REQUIRED`, `models: []`. No probe, no URL accepted, no credential
returned. Admission gate in `NETWORK-SECURITY.md` **not satisfied**. 4/4 + 1/1 + 8 tests.

### reviews/E07-S03.md (90) / E07-S04.md (98) / E07-S05.md (98)
All stay `PLANNED` — policy/decision modules with no provider, no route, no dispatch. E07-S03:
`plan_batch` refusal reasons (`TooManyMembers`, `DuplicateMember`, `UnknownDependency`,
`DependencyNotAdmitted`, `StalePrecondition`, `InsufficientDeadlineBudget`, `DeadlinePassed`);
`BatchOutcome::{Completed, PartiallyApplied, Ambiguous, Aborted}`; `WarmHoldCarriesNoAuthority`; 13/13.
E07-S04: `ChecksNotIndependent`, `PinnedProvenance`, `ReviewStatus::Draft`, `OwnerReviewRequired`,
macro `/JavaScript` `/OpenAction` `/Launch` `/EmbeddedFile` refusal; all five formats reported
`NotEnforced`; 11/11. E07-S05: `QuickEntryCaptureRefused`, `ConsentMissing/Revoked/Expired`,
`ApplicationNotInScope`, `WindowNotInScope`, `CaptureNotPermitted`, `InputNotPermitted`,
`SecurityBypassRefused`, `ScreenLocked`, `HumanOwnsInput`, `SensitiveRegion`; no platform reports
`Available`; 10/10.

### reviews/E08-S02.md (200)
`IN_PROGRESS`, base `b003e9e…`. Goals read-only projection; Suggestion accept → one READY Task +
events + receipt atomically; events `goal.*.v1`, `suggestion.*.v1`, `suggestion.visibility.changed.v1`,
`suggestion.preference.changed.v1`. **Idempotency key bound 1–200 printable ASCII** (was wrongly
129–200); 200-byte commits, 201-byte → `INVALID_ARGUMENT`, changed payload → `CONFLICT`. Workspace
response fencing added for goals/suggestions/pagination. Test trajectory: 192/192 → 20/20 → 33/33 →
324/324; build 129 modules, 884.57 kB.

### reviews/E08-S05.md (109)
Stays `PLANNED`. Focus transfer only (Edit/Restore → editor, Discard → action, publish → status,
fallback → heading); dirty draft keeps values on Escape with message "Your unsaved changes are still
here. Use Cancel to discard them."; looping sprite spinners replaced with static busy + `aria-busy`.
Counts 4/4, 16/16, **439/439**, build 150 modules/941.29 kB. Explicit: helper tests "verify the
decisions, not the browser" — no screen reader, zoom, high contrast, packaged WebView.

### reviews/E08-S06.md (169)
Bounded transient streaming, semantic fallback, no HTML/JS execution. Reducer caps: 4 active turns,
8 retained records, 256 KiB per response, 512 KiB aggregate. 34/34 reducer; **14 rendered
Conversation checks on Chrome `149.0.7827.200`**; 12/12 fallback; `rich_presentation_read_tests`
10/10 including `reading_a_presentation_snapshot_journals_no_transient_item`. Honest wording fix:
"items could not be displayed safely" replaces "items were hidden". Missing: typed producer, so no
transient frame reaches a running app.

### reviews/E08-S07.md (134)
`IN_PROGRESS`, base `b003e9e…`. Immutable versions, bounded text edit, restore-as-new-version,
exact comparison, safe-subset Markdown/table preview, Tauri Save As ≤10 MiB with staged same-directory
write and Saved/Cancelled only. Epoch fencing per Workspace/Artifact visit; access-loss invalidation
on 401/403/404. Counts: save_as 4/4, 2/2, 9/9, access-state 2/2; builds 97→140 modules
(734.53→916.28 kB). Historical rows admit Node 22.23.1 runs occurred "below declared Node 24 range".

### reviews/E09-S03.md (212)
`IN_PROGRESS`, base `0b98c21`. Health projections `GET /v1/automations/{id}/health` /
`GET /v1/routines/{id}/health`; `DRIFTED` needs explicit incompatibility, failed Task alone =
`WARNING`, absence of observation = `UNKNOWN` never `HEALTHY`; success rate over 10 most recent
terminal Tasks, <3 → "not enough runs yet". Exact-revision test run: schema **v27**
`automation_test_runs` + `ALTER TABLE tasks ADD COLUMN automation_test_run_id`, 201/200/409,
no occurrence, no cursor advance, PAUSED/ENABLED allowed, DISABLED refused. Counts: 96/96, 22/22,
8/8, **581** storage, **523/523** daemon, 497/497 UI. Owner approved the design 2026-10-21.

### reviews/E13-S01.md (190)
Local CLI only (`litecoworkd backup create|verify|restore|upgrade-check`), deliberately not an
Operator route ("a restore … must never be reachable over the Operator socket"). 11/11 tests
including corrupt archive, wrong passphrase, non-empty destination, live-runtime lock, foreign schema,
passphrase-absent-from-archive. `chacha20poly1305` 0.10 XChaCha20-Poly1305 + hand-rolled
PBKDF2-HMAC-SHA256 (no PBKDF2/Argon2 dependency in repo). Blockers: no `org.freedesktop.secrets` →
`RUNTIME_OS_PRINCIPAL_UNAVAILABLE`, so SYSTEM is BLOCKED; blob scope is one Workspace while the DB
snapshot still carries all Workspaces; no request dedup/AuditRecord.

### reviews/E13-S02.md (164)
`scripts/v1_soak_harness.py`, `v1_resilience_matrix.py`, `v1_store_probe.py`. Soak: seed `20261020`,
**745 iterations, 14 restarts, 5,960 rows, 8,466.9 B/iteration, p50 7.39 ms, p95 21.5 ms, p99
104.7 ms**, integrity `ok`. Resilience 5/5 (`restart_mid_write`, `kill_during_drain`, `disk_full`,
`corrupt_blob`, `stale_lock`). Reports storage-sqlite **534 passed / 7 failed**, all outside
`backup::`, traced to `sqlite-v26.sql` changing the schema fingerprint pinned by v17/v19 migrations —
a regression reported rather than hidden. Core-vs-provider latency, million-row Task, provider
circuit storm: NOT RUN.

### reviews/E13-S04.md (125)
`scripts/v1_packaging_check.py` 8/8 (bundle config, icons, daemon version, backup surface, 0700 state
dir / 0600 DB, systemd unit accepted by `systemd-analyze verify`, "no updater endpoint → no unsigned
update", build inputs); `v1_secret_scan.py` "scanned 1 files, 0 findings" (debug binary only).
`scripts/packaging/systemd/litecoworkd.service`. Not done: installer, signature, notarization,
publishing, SBOM/licences, release-artifact scan, clean-host install/update/uninstall.

### spikes/SP04.md (474)
Codex→OpenCode handover PoC, `IN_PROGRESS`. Hosts: Codex `0.160.0`/`0.160.1` (`gpt-6-luna`, medium),
OpenCode `2.0.22`/`2.0.24`; models `opencode/mimo-v2.6-flash-free` (fell back to
`opencode/ling-3.1-flash-free` after HTTP 429 `FreeUsageLimitError`). Five cases HO-01…HO-05 all
reached task acceptance; HO-05 needed an independent-review correction (RFC 9110 §10.2.3 signed
`delay-seconds`, 5,000-digit int limit, rounding). **Key negative finding:** in HO-04
`turn/interrupt` returned `turn/completed` status `interrupted` yet the sandbox writer's heartbeat
advanced 1→198 over 20.12 s — turn status is not a stale-writer fence; only sender-host shutdown +
3 s observed quiet admitted the receiver. 42 spike tests; Linux/WSL2 bubblewrap `0.9.0` only.

### spikes/agent_switching_poc/process_supervisor.py (390)
Spike-only Linux PID-namespace supervisor (domain: test harness, not product). Constants
`CHECKPOINT_MOUNT_PATH="/tmp/litecowork/checkpoint"`, `MAX_CHECKPOINT_FILE_COUNT=4096`,
`MAX_CHECKPOINT_TOTAL_BYTES=512 MiB`. bwrap args `--unshare-pid --die-with-parent --ro-bind / / --dev
/dev --proc /proc --tmpfs /tmp --bind <cwd>`; checkpoint files copied into sealed `memfd`
(`F_ADD_SEALS` 1|2|4|8) and mounted `--ro-bind-data`. Identity = `(pid, start_time_ticks, boot_id)`
from `/proc/<pid>/stat` field 20 + `/proc/sys/kernel/random/boot_id`; zombies (`Z`,`X`) treated as
dead, unreadable members fail closed. `stop_and_confirm` = SIGTERM → SIGKILL → wait for empty group.
Refuses to start on non-Linux; docstring concedes the read-only-root wrapper "is not a CLI adapter".

### spikes/agent_switching_poc/tests/test_switch_coordinator.py (599)
13 tests against `implementation.spikes.agent_switching_poc.switch_coordinator.SafeSwitchCoordinator`:
pinned snapshot immune to later sender edits; tampered persisted snapshot → `SwitchBlocked`; digest
path traversal rejected; admit after ledger restart (epoch 2); receiver reads sealed mount read-only
and writes only its project dir; TOCTOU snapshot change between admission and launch →
`START_FAILED` + version 5; `ADMITTED → RUNNING` on successful spawn; orphaned-admission recovery only
after launch owner gone (retry at epoch 3); running-attempt recovery blocks while runtime/worker alive;
`worker_identity=None` never releases the Task slot; admission-owner-alive blocks; running crash
recovery → `ABANDONED`; full compose of quiescence → checkpoint → settlement → admission → receiver.
Uses `subprocess` + real `bwrap`; requires Linux.

### spikes/agent_switching_poc/tests/test_switch_ledger.py (404)
10 tests on `SwitchLedger` (SQLite): lead change rebinds only the Task, never the running Attempt, and
cannot admit a replacement; settlement requires quiescence + `effects_reconciled` + `lease_released`
(three sub-cases each leave status `RUNNING`); checkpoint-manifest digest mismatch blocks; handoff and
lease epoch survive reopen (version 4, epoch 2, status `ADMITTED`); `fail_attempt_start` →
`START_FAILED` then same-lead retry at epoch 3; restart keeps an unresolved admission blocking a
duplicate worker (2 attempts only); packet changed after settlement blocks; `None`/`{}`/`[]`/"bad"
packets rejected; crashed-but-quiescent sender replaceable; stale version → `StaleTaskVersion`.
Statuses observed: `RUNNING`, `SETTLED`, `ADMITTED`, `START_FAILED`, `ABANDONED`.

### spikes/sp02-driver-compare/Cargo.toml (22)
`sp02-driver-compare` 0.1.0, edition 2024, `publish = false`, own `[workspace]`. SQLx `0.9.0`
(`runtime-tokio`, `sqlite-unbundled`) plus `chacha20poly1305 0.10.1`, `getrandom 0.3`, `hex 0.4`,
`serde_json_canonicalizer 0.3.2`, `sha2 0.10`, `zeroize 1.8`, `tempfile 3.20`, `tokio 1`, and path
deps on real `storage-core` / `domain-workspace` — the comparison harness drives the product crates,
not a toy schema.

### spikes/sp02-driver-compare/rusqlite/Cargo.toml (14)
`sp02-rusqlite-qualification`; depends on `storage-sqlite` with `default-features = false, features =
["sqlite-rusqlite-defaults"]` — i.e. the shipped adapter, the reason SP02 could call rusqlite the
provisional winner.

### spikes/sp02-driver-compare/rusqlite/src/main.rs (490)
Benchmark binary. Constants `PRODUCERS=8`, `READERS=4`, `MIXED_WRITERS=40`,
`MAX_MIXED_UPDATES_PER_WORKSPACE=10_000`, `MAX_RECORDED_READ_SAMPLES_PER_READER=100_000`;
`SqliteConfig{writer_queue_capacity: 32, busy_timeout: 5 s}`, WAL + FULL. `TestKeys` returns
`[19u8; 32]` for every key (fixture only). Two modes: `run(writes)` (8 producers × N workspace
`change_replication_policy` updates) and `mixed <n>` (40 writers + 4 readers behind a `Barrier`,
reader checks monotonic version and policy consistency). Both reopen the store and compare
`get_workspace` against `rebuild_workspace_projection()`; mismatch is a hard error. Emits p50/p95,
`send_wait_*`, `outstanding_commands_peak`. 2 tests (`run(12)`; `run_mixed_load(2)`).

## Cross-cutting observations

- **Evidence discipline is the through-line.** Almost every file separates CODE from SYSTEM from
  owner USER, names the exact command, and refuses to promote on a green suite. Two independent
  counts of "ACCEPTED" stories are **zero** (60-story V1 snapshot, 67-story snapshot).
- **The dominant structural blocker is one gap repeated across epics:** no qualified native
  Conversation session admission. It disables Send, which blocks Chat → Coworker → Task → Delegation
  → Automation in the acceptance matrix. The 2026-10-25 entries show the team choosing honesty over
  a false 202: the inventory-only admission path was *deleted* rather than left reachable.
- **Fail-closed gates are real code, not prose:** `TASK_PLANNING_ISOLATION_UNAVAILABLE` before any
  write, `admit_step_attempt` closed, Effect `STARTED` disabled, `WarmHoldCarriesNoAuthority`,
  `NOT_ENFORCED`/`UNKNOWN` treated as refusal. Consistent with "Unknown is not success".
- **Platform asymmetry:** all sandbox/containment proof is Linux (cgroup v2 + Bubblewrap + `/proc`
  identity). Windows has DACL/reparse-point revalidation and named-pipe IPC only; macOS has nothing
  but a bookmark gate. Windows/macOS release cells are blocked *by design*, not by omission.
- **Doc-vs-code drift is the recurring defect class:** audits repeatedly find docs claiming LIVE for
  unserved routes (`API.md`, `EVENTS.md`, `STORAGE.md`), stale "not yet built" claims for shipped
  features, and three/four competing Work-filter and Task-state vocabularies. The 2026-10-22 pass
  logged 92 findings, the UI pass 48 claims (18/15/15).
- **Live numbers worth remembering as a trajectory, not a status:** storage-sqlite 188 → 303 → 534
  → 581 → 588; `litecoworkd` 504 → 523 → 526; UI helper suite 192 → 324 → 439 → 497; schema to v27;
  145 typed events; 229 OpenAPI operations; 107 baseline tables; 25 roster entries.
- **Divergence from ARCHITECTURE.md invariants is mostly in the honest direction** (Core refuses to
  fake admission, warm state carries no authority, transient frames are never journaled), but V1
  G1 ("installed app + real native Send + verified Artifact + kill/restart recovery") is unmet in
  every snapshot here — this archive documents a *pre-reset* tree that never reached V1.
- **Ordering anomaly:** `CURRENT-RUN.md` is reverse-chronological but contains a 2026-10-20 block
  *after* 2026-10-25 blocks, and a second 2026-10-25 cluster at EOF — appended, not re-sorted.

## Coverage check

- **`CURRENT-RUN.md`: 520 of 10,160 lines read (~5%).** Read head (lines 1–400: 2026-10-25 →
  2026-10-21) and tail (10,041–10,160: 2026-10-20 handoff + appended 2026-10-25 block). The remaining
  ~9,640 lines were **not read**: the paging read was cancelled at the task deadline. Everything
  asserted above about this file comes from those 520 lines plus cross-references in the other 40
  files (review packets, epics, handoffs), which quote its entries extensively. Treat any statement
  about mid-file entries (roughly 2026-10-05 → 2026-10-20 detail) as unverified.
- All other 40 files read to EOF, including the 599-line and 490-line source files.
- Not verified at all: whether any path in this archive still exists in the current checkout; whether
  the quoted test counts reproduce; the Revision 9 master specification referenced by
  `docs/RESET-AND-ARCHIVE-RECORD.md` (absent per repository instructions).
