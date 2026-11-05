# Digest archive-03 — legacy `reviews/` and `epics/`

Files: 42 full / 48 listed (6 partial — see Coverage) | Lines read: ~11,500 / 22,385 | Bytes: ~790 KB / 1,236 KB

Bucket: `docs/archive/legacy-implementation-c7e9c05/`. **Historical**, not current qualification. "Verdict" = the packet's own status line, not a re-review. Packets are rolling: dates below are the first→last dated slice.

## Reviews, grouped by epic

| Story | Dates | Scope reviewed | Verdict | Defects found | Evidence cited |
|---|---|---|---|---|---|
| **E01-S02** | 10-10→10-20 | `StateStore`/`EventStore`/`BlobStore`, bounded SQLite writer, v1 DDL, per-origin sequence, JCS, encrypted `FileBlobStore` + `OsWorkspaceBlobKeyProvider`, rotation/recovery, SP02 | `IN_PROGRESS`; owner decision PENDING; not acceptable while E01-S01 unaccepted | No production defects in most slices; a mutation test caught a premature commit; test compile/borrow errors corrected; Linux Secret Service **unqualified** (`org.freedesktop.secrets` absent, runner exit **3**) | `SQLITE_FULL` via `max_page_count` → rollback + retry at next sequence; `os_key_provider` 14/14; BlobStore 19/19; storage 175/177/534 (7 foreign failures); Windows Credential Manager 1/1 on build 26200; AEAD AAD binds Workspace+purpose |
| **E01-S04** | 10-09→10-20 | Unix same-UID IPC + Windows pipes, framing/deadlines, `/v1/stream` (Conversation/Task/Runtime-inventory), locations, dependents, invalidations, offers, bindings, capability-hosts, Tauri one-frame ACK | `IN_PROGRESS` | Adversarial review: **idle peer disconnect undetected** (no client-side monitor); **multi-Conversation resync advanced cursors to latest frontier while emitting one message per Conversation** → skipped messages; Task retention race → stale snapshot; A09/A11/A13 receipt-vs-stale-version and replay-after-disable corrections | Unix admission cap 8 → `CapacityExceeded` → slot release; stream contract 5/5; daemon 160 unit; **Windows daemon 208 passed / 0 failed / 1 ignored**; spawned-daemon lost-ACK replay; foreign/future/prior-incarnation cursor resync |
| **E02-S02** | 10-11→10-17 | Workspace policy, instructions (immutable revisions, Resource pin), WorkspaceRoot create/overlap/revalidation, archive fail-closed, Tauri bridge, Settings UX | `IN_PROGRESS` | Bridge rejected contract-valid **empty `parent_revisions`**; allowlist `ACTIVE_TASKS` vs `ACTIVE_TASK_INPUTS`; **policy canonical payload omitted `workspace_id` + `expected_version`** → same-key replay across Workspaces/changed `If-Match` (200 where 409); no-follow canonicalized before traversal (intermediate symlink); BlobStore MIME `...litecow.resource+json` vs `...litecowork...` | Instruction replay after Workspace+Resource advance; TaskSpec pin retention; `WORKSPACE_NOT_QUIESCENT` / `ACTIVE_CHANNEL_HOSTS: 1`; 14 root-overlap SQLite tests; storage 181, daemon 145; audit 206 routes / 158 UIV1 / 67 stories |
| **E03-S01** | 10-16 | Agent Catalog status copy: offer compatibility vs process readiness vs Codex Conversation eligibility | `IN_PROGRESS`; CODE only, no API/event/schema/Rust change | Missing independent status text; overclaim "eligible for new work"; copy omitted lead precedence; stale generated API types | 14/14 focused; 275/275 UI; 1/1 rendered Chrome; Vite build |
| **E03-S02** | 10-15→10-25 | Conversation create/turn, journal, idempotency, turn persistence, AgentBinding selection, context package, provider wire | **PLANNED** (safety remediation only) | **202 RUNNING + AgentSession STARTING from Runtime inventory alone, no adapter/`SESSION_READY`, masked by an idempotency-key collision** (create vs admit same public key); storage `admit_conversation_turn` removed as second unsafe path; replay after binding clear returned 422 not 503; two submits from one snapshot → `400 CONVERSATION_TURN_EVENT_REVISION_MISMATCH`; OpenCode 2.0.32 unqualified | 503 `DEPENDENCY_UNAVAILABLE` + FAILED turn + zero AgentSession/Task/Attempt/lease/Environment; storage 9/9; daemon 9/9; context 3/3 + 1/1 (1,024 msgs / 4 MiB; 256 KiB prompt); prompt wire 5/5 |
| **E03-S08** | 10-18→10-19 | `AgentControlDescriptor` closed parsing/validation, digest, freshness, config defaults | CODE only; no producer/route | Value-bearing descriptor could not derive `Eq`; serde enum `LITE_COWORK_*` vs canonical `LITECOWORK_*`; remote `$ref` accepted; config default type mismatch | Descriptor 16→17/17; wider `agent` filter 98 passed / 1 ignored; UI panel 5/5; 256 KiB pre-decode cap |
| **E03-S09** | — | Descriptor→credential snapshot, write-only SecretStore, no-store route | CODE only; route always `DEPENDENCY_UNAVAILABLE` | No trusted module producer; no durable replay receipt for keyring mutation → successful route write disabled | Converter 13/13; helper tests for stale/mismatched/native-owned slots, zero secret echo |
| **E03-S10** | — | Composer surface freshness + native token compatibility | CODE only; Send disabled | Stale/broken/expired installation projected `READY`; session options retained across descriptor-digest change; token accepted after native syntax change; stale token range still `READY` | 9/9 + `tsc --noEmit` |
| **E05-S01** | 10-16→10-18 | DelegationProfile CRUD/revisions/duplicate/status, revision history, no-store, blocking-pool reads | `IN_PROGRESS`; profiles disabled by default, enablement fails closed | Exact route/bridge/UI commands absent; full SQLite run interrupted at 60 s (not green); revision-history storage test never returned; lockfile pending | Lifecycle 1/1; history UI 3/3; header 4/4 |
| **E05-S03** | 10-19→10-20 | Ranking, performance aggregation, quota observation, provider-enforced ceiling | `IN_PROGRESS` | **LOW-confidence metrics outranked qualified results and a LOW 10,000-bp rate passed a quality floor**; fractional `UsageQuantity` unknown; verifier metrics null (no Attempt FK); `failure_categories` empty; wrong SQLite path in fixture | Selector 15/15; domain-task 85→100→122; performance adapter 1/1; quota 3/3; UI 28/28; Tauri 2/2 |
| **E05-S05** | 10-20 | B83 sanitized benchmark ingestion + released-threshold benefit | **PLANNED**; `automatic_selection_qualified` always false | None in scope; no corpus, no persistence, no live measurements | Benchmark 6→12/12; domain-task 122/122; profile duplicate 1/1 |
| **E06-S03** | — | Index state `READY/STALE/ABSENT/NOT_INDEXABLE`, serve-time invalidation, crash recovery | `IN_PROGRESS` | **No writer purges superseded projection rows** (encrypted snapshot retained; needs lead-owned writer + maybe DDL); no auto re-index; no provider replica registration/purge receipt; no watcher | `GET /v1/resources/{id}/index-state`; storage **485 passed** (708 s); daemon **504 passed / 1 ignored**; rag_ 8; domain-rag 5; UI 10 |
| **E07-S01** | 10-20 | Environment lifecycle domain + SQLite, provider-class/budget/archive guards | `IN_PROGRESS`; decision module only | Provisions/reserves/routes nothing; prior-incarnation binding never reattached | `environment_provider` 9/9; domain 22; SQLite 11; `BudgetEnforcement::Unavailable`; `may_overshoot` |
| **E07-S02** | 10-17→10-20 | Browser control lease, pointer mapping, consent/profile policy, control service | **PLANNED** | Capture time **equal to takeover** accepted (ms ambiguity) → strictly later; hydrated USER lease with **no original AgentSession** accepted; equal observation revision accepted; over-age frame accepted; **expired row occupies unique ACTIVE slot, no eventful expiry** | Linux 24 unit + 18 control + 9 pointer; Windows 24 + 23 control; Chrome ≥144 attach, default profile refused, credential import refused |
| **E08-S03** | 10-14→10-20 | Resource/ContextDocument editor, draft safety, local purge saga, deterministic learning gates | `IN_PROGRESS` | No native-session invalidation; automatic learning unimplemented; proposed DDL not applied; learning module placed in `storage-core` not `domain-learning`; added `inferred_confidence_threshold` needs contract confirmation | Purge storage 14/14; route 1/1; decoder 2/2; UI 241/241; learning 30/30; storage-core 35/35 |
| **E09-S04** | 10-21 | Notification decision layer, read/preference routes, desktop surface, producer | `IN_PROGRESS` | Persisted quiet hours `{timezone,weekdays,start_local_time,end_local_time}` **unreachable** by domain `{start,end,timezone}` (`len != 3`); `unwrap_or(false)` **failed open**; earlier packet proposed duplicate `notification_deliveries` tables — "must not be applied" | domain-responsibility 96/96; notification 17/17; storage 560/560; daemon 521/521; UI 488/488 |
| **E09-S05** | 10-21 | `POST /v1/routines/{id}/run` → one READY Task | `IN_PROGRESS`; storage+route CODE settled | No defect recorded; storage deferred tests by owner instruction then run; ResourceRef body not exercised in route test | `routine_admission_tests` 9/9; `routine_run_operator_tests` 1/1 (401 before replay, 403, 400, 422, 201, 409, replay after ARCHIVED) |
| **E10-S02** | — | Demonstration capture, redaction, placeholders, SkillProposal review, publication | Blocked at **LiteSPM** (`LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`) | `crates/ports/skill_test_runner.rs` is illustrative and does not exist; capture producer owned by E07 | storage-core 103; skill 19; operator 4; UI 20/20; broader 534 (7 foreign) / 512 (2 foreign) |
| **E13-S03** | — | Generated release qualification matrix | Only **3 PASSED** cells; all daemon capabilities `BLOCKED` | Windows/macOS columns never run; 9 agent rows **NOT INSTALLED**; "WSL2 is not a native Linux desktop" | backup 11/11; packaging 8/8; secret scan 1 file/0 findings; `platform-matrix-evidence.json` |
| **SP04-HANDOVER** | 10-07 | Codex→OpenCode switch PoC, HO-01..HO-05 | "Reviewed to the extent retained evidence permits; product-safe switching is not implemented" | HO-05: **signed delta-seconds clamped to 0**; long decimals hit Python int-string limit; fractional-ms delay rounded down (early retry); HO-01..04 source trees absent | HO-05 fixture 20 tests; PoC `effects_reconciled`/`lease_released` are test inputs only |

## Epics and audits

**epics/E01.md** (799) — G0; S01 `IN_REVIEW`, S02–S04 `IN_PROGRESS`. `OWNER RIGHTS`+SYSTEM DACL; systemd `Delegate=yes`, `KillMode=control-group`, `TimeoutStopSec=90s` > 60 s drain; `linux-process-scope` READY/GO_CONSUMED/`PendingCleanup`; unclean restart; `OPERATOR_API_START_FAILED`; lease fence on old incarnation. 506 passed / 8 failed / 1 ignored (6 Cline, 2 automation).

**epics/E03.md** (615) — G1. Exact seam: `require_task_planning_isolation_admission()` inside `start_task_planning_session`/`activate_task_planning_session`, `TASK_PLANNING_ISOLATION_UNAVAILABLE`; `POST /tasks/{id}/plan-revisions` deliberately unserved; Codex eligibility fail-closed over six controls → `NOT_ELIGIBLE`; installed `codex-cli 0.163.0`, `opencode v2.0.32`.

**epics/E09.md** (312) — local trigger host (`CRON_5`, `RFC5545_RRULE`, DST fold/gap, misfire policies); schema v27 `automation_test_runs`; health `UNKNOWN` ≠ `HEALTHY`; `PENDING→CLAIMED→STARTED`; ambiguous-run registry volatile.

**epics/E10.md** (149) — MCP App sandbox (`allow-scripts`, `default-src 'none'`, host nonce); MCP_SKILL conformance; Skill DRAFT-only + revision narrowing; no migration.

**CODEBASE-AUDIT-2026-10-21** (284, baseline `ed2e1c8`) — 225 operations/200 paths vs **154 mounted paths**, 47 groups unserved; `POST /v1/stream` outside OpenAPI; **44 tables / 77 of 145 events with no producer**; five dead modules; **no production `INSERT INTO attempts`**; 2 of ~25 Task transitions; `automation_cursors` never deleted → archive permanently blocked; 16 unregistered error codes; `Idempotency-Key` unenforced on 2/225; `limit` 200 vs 100; 108 path-casing mismatches; UI false empty state, "Completed"/"Sent" styling, unreachable Discover/SkillProposal; 15 stories without packets; 44/77 epic obligations missing from backlog. Owner decisions D1–D5 (25-agent roster; Bubblewrap-only; backup CLI-only; LiteSPM unavailable; in-app+OS notifications).

**AGENT-MODULE-AUDIT** (532, baseline 10-10) — dispose of `CODEX|OPENCODE` unions, `probeCodex/probeOpenCode`, provider-key branching; target crates `agent-module-core`/`agent-registry`/`agent-lifecycle`; `AgentModule`/`AgentLifecycleAdapter`; `validate_control_descriptor`; atomic `configure_agent_binding`; deletion gates.

**V1-SERVED-CAPABILITY-INVENTORY** (86) — **172 served / 53 target-only / 1 outside OpenAPI**; 36 merged factories; `audit_served_routes.py` → `served-route-inventory.json`, enforced in `check.sh`.

**ARCHITECTURE-COVERAGE** (185) — 111 docs/1,893 sections, 108 plan docs, 68 stories, 144 flows, 91 benchmarks, 229 operations, 405 schemas, 131 error codes, 148 events, 107 tables/140 triggers. "Coverage is not implementation evidence."

**DOC-AUDIT-2026-10-10** (173) — 125 files / 45,217 lines / 2,845,812 bytes; 59 updated, 3 new, 0 broken links; per-file SHA-256 + line counts.

**V1-SEMANTIC-MD-FINDINGS** (73) — U01–U158 collided with workflow U01–U20 → `UIV1-001..158`; CREATED/TURN_RUNNING/COMMITTED pseudo-states replaced by canonical machines; `RequestId` vs IPC `request_id` corrected; regeneration PENDING (WSL timeout).

**V1-DOC-RECONCILIATION-20261022** (46) — docs-only; native Send disabled, Attempt admission fails closed, LiteSPM unqualified, `INCOMPLETE`→FAILED settlement, no CODE+SYSTEM+USER matrix.

**NEXT-SESSION-HANDOFF-2026-10-20** (149) — `HEAD 25ea3830`; 139 changed + 104 untracked, 0 staged; **67 stories: 1 IN_REVIEW / 22 IN_PROGRESS / 44 PLANNED / 0 ACCEPTED**; per-worker stop points (E01 Windows symlink test unverified, E02 bridge 5/5, E04-S05 14/14, E05 domain-task 100/100).

**RELEASE-RUNBOOK** (256) — `backup create|verify|restore|upgrade-check`; no key material in archive; restore refuses populated dir/lock, revokes grants, fences incarnations, new Runtime identity; limitations: passphrase-only, snapshot spans all Workspaces, local-only, no installer/signature.

**RAG.md** (120) — today only bounded lexical index over small UTF-8 text; no semantic RAG/parsing/embeddings; local-model backends are candidates only.

**WORKFLOWS.md** (70) — Cowork/Muse/ChatGPT replacement assessment, U01–U20; E11/E12 post-V1; vendor-reported caveats.

**SOURCES.md** (134) — pinned catalogue (R-CODEX, R-OPENCODE, R-ACP-REGISTRY `4d1729af…`, R-RFC8785, R-MUSE); navigation only.

**AGENT-PORTFOLIO-2026-10-16** (161) — **superseded by the 10-21 25-entry roster**; status ladder Discovered → Profile observed → Module implemented → Protocol handshake → Qualified support; Claude ACP children survive cancellation; Droid launch contract 0.242/0.243 `acp-daemon` mismatch.

**licensingandpkacgingresearch** (196) — deferred; BSL/ELv2/PolyForm unverified, not legal advice; do not move durable Task truth behind IPC for licensing.

**V1-CROSS-CONTRACT-GUARD.json** — 244 docs / 93,370 lines / 34 assertions / 0 issues.

**Spikes** — `SP02.md`: rusqlite 327.4 vs SQLx 322.9 median updates/s, overlapping → **no driver decision**; mixed 40 writers/4 readers, 44 outstanding vs 32 slots, coherent replay. `SP03.md`: local IPC plan (`interprocess` 2.4.x, peer creds, DACL/logon SID, macOS bookmark); source partial. `sp02-driver-compare/README.md`: separate workspaces (`libsqlite3-sys` conflict). `src/main.rs`: `#[path=…blob.rs] mod blob`, `TestKeys` 32×`19u8`, `PRODUCERS=8`, DDL from `sqlite-v1.sql`. `switch_ledger.py`: `SwitchError/StaleTaskVersion/SwitchBlocked`, `TaskView`/`AttemptView`. `Cargo.lock`: v4, `aead 0.5.2`.

## Cross-cutting observations

1. **The archive's thesis is "published ≠ served ≠ qualified."** Every packet separates CODE / SYSTEM / USER; none claims acceptance.
2. **Idempotency identity is the dominant defect class** — three independent fixes (receipt-before-mutable-state; missing `workspace_id`/`expected_version` in the policy digest; turn-create vs admit key collision that masked a false 202). The pattern: bind every input into the canonical payload and resolve the owner-scoped receipt first.
3. **Cursor/frontier pagination is the second** — multi-Conversation resync skipping pages, Task snapshot vs replay confirmation, tied `created_at` needing `(created_at, id)`.
4. **Test-first physical-safety work is unusually well documented** — symlink TOCTOU, FIFO, same-millisecond observation, over-age frame, stale descriptor; red runs recorded before fixes.
5. **Path drift from the live repo:** the archive uses `implementation/…`; `crates/ports/`, `domain-learning/`, `application/`, `adapters/*` are referenced but **do not exist**. Current `ARCHITECTURE.md` must not inherit these paths.
6. **Native execution was never qualified in this bucket:** Codex `NOT_ELIGIBLE` on all six controls, OpenCode pinned to 2.0.26/2.0.28, all E13 agent cells NOT INSTALLED, E03-S02 production path returns 503 by design.
7. **Bootstrap blockers are environmental:** missing Secret Service, MinGW `windres`/`icon.ico`, no Windows/macOS host, no signing identity; many shared-tree failures are labelled as other epics'.

## Coverage check

- `machine-inventory.json` — 45/8,031 lines (generated digest index; totals taken from ARCHITECTURE-COVERAGE).
- `V1-MARKDOWN-INVENTORY.csv` — 10/232 rows (`file,lines,bytes,sha256,categories,has_heading`).
- `audit-inventory.csv` — 18/119 rows (`path,baseline_lines,baseline_sha256,audit_method,headings_or_sections`).
- `sp02-driver-compare/src/main.rs` — 60/757 (SQLx prototype; head identifiers only).
- `agent_switching_poc/switch_ledger.py` — 55/483 (PoC types only).
- `sp02-driver-compare/Cargo.lock` — 22/1,454 (generated lockfile).
- All 16 review files and 4 epic files read completely (paged `E01-S04.md`, `E03.md`).

## Out of scope / suggestions

- 33 further packets exist but are not on this plan (`E02-S03`, `E04-S05`, `E13-S01`, `E13-S04`, `E03-S06` …); several are cited above.
- `AGENT-PORTFOLIO-2026-10-21.md` and `QUALIFICATION-MATRIX.md` are referenced but absent from this bucket.
