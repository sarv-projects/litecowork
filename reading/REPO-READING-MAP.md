# LiteCowork — repository reading map

**Read date:** 2026-11-04 · **Snapshot:** `main` @ `f22ec11` (2026-11-04), clean tree, 223 commits
**Scope:** every one of the 1,056 tracked files (23.87 MB, 514,413 lines) — method and residual gaps in `COVERAGE.md`.

---

## 1. Verdict in one paragraph

LiteCowork is a **documentation-first specification of a local-first, chat-first AI cowork desktop**, with a large, real Rust + Tauri implementation that has been re-admitted into the tree **one day after** the repo declared itself docs-only. The documentation is unusually rigorous: 31 load-bearing invariants, a frozen V1 scope of ten product areas, 229-route OpenAPI contract, 148 versioned domain events, 28 immutable SQLite migrations, 25 ADRs, and 11 SDD feature packages that all pass a structural validator. The code is real but **deliberately fail-closed at almost every consequential boundary**: conversation send, task planning, attempt admission, environment isolation, effect dispatch, package service. The result is a system that is honest about being incomplete — except in one place: `CURRENT-RUN.md` declares the whole thing "PRODUCTION-READY", which the repository's own evidence contract (TARGET / CODE / SERVED / INTEGRATED / QUALIFIED) does not support.

---

## 2. What the product is meant to be

- One desktop workspace: left navigation (New Chat, Coworkers, Needs You, Work, Automations, Library, Discover, Settings), centre conversation + composer, right Workbench (Files / Terminal / Browser / Diffs / Activity).
- Chat-first: an ordinary Conversation needs **no Task and no Coworker**; a Task is created only when work is explicitly made durable.
- BYOA: the user's native agent harness (Codex, Claude Code, OpenCode, Gemini…) keeps its own auth, models, tools and private transcripts. LiteCowork coordinates it and owns the durable record.
- Core owns Workspace, Conversation, Task/TaskSpecRevision/PlanRevision/Step/Attempt, ExecutionLease, Effect, Artifact, Evidence, Trust and recovery. LiteSPM (separate product) owns package discovery/install/lifecycle; LiteCowork owns scoped authorization to use.
- V1 = complete **local desktop** product. Cloud Runtime, multi-device continuation and mobile are V2.

## 3. Repository shape (measured)

| Area | Files | Lines | Role |
|---|---:|---:|---|
| `docs/` | 303 | 143,474 | Canonical product, architecture, domain owners, ADRs, machine schemas, ledgers, archive |
| `apps/litecoworkd/` | 86 | 84,956 | Headless Rust Runtime: 181 routes, 537 tests |
| `apps/litecowork-ui/` | 289 | 86,847 | Tauri 2 + React/TS Operator: 106 `#[tauri::command]`s, 123 TS/TSX files |
| `crates/` | 196 | 174,736 | 15 crates: domain-\*, storage-core/sqlite, operator-ipc, OS containment |
| `specs/` | 122 | 5,631 | 11 SDD packages (README, SPEC, PLAN, TASKS, DATA-MODEL, RESEARCH, WIRING, contracts/) |
| `scripts/` `tests/` `.specify/` `capabilities/` | 51 | 13,658 | Validators, pytest suite, SDD templates, ZIP-intake capability |

Text-level totals: 7,270 `fn`, 1,877 tests, 181 `.route()` calls, 673 exports, 143 `CREATE TABLE`, 282 `CREATE TRIGGER`, 1,438 `.unwrap()`.

## 4. Authority chain (and how to navigate)

1. **`ARCHITECTURE.md`** — 31 invariants, canonical concepts table, Core/adapter boundary, protocols, release gates. This is the constitution; everything else is subordinate.
2. **`docs/PRODUCT.md`** + **`docs/V1-PRODUCT-RELEASE-CONTRACT.md`** — promise, ten V1 areas, hard V1/V2 boundary.
3. **Domain owners** — `DATA-MODEL`, `STATE-MACHINES`, `STORAGE`, `SERVICES`, `API`, `EVENTS`, `TASK-RUNTIME`, `AGENT-FABRIC`, `AGENT-CONTROL`, `DELEGATION`, `ENVIRONMENTS`, `TRUST`, `RUNTIME-LIFECYCLE`, `RUNTIME-MESH`, `AUTOMATION`, `ROUTINES`, `CHANNELS`, `ARTIFACTS-EVIDENCE`, `PRESENTATION-RUNTIME`, `CONNECTORS-WORKFLOWS`, `LEARNING`, `WORLD-RESOURCES`, plus 25 ADRs.
4. **`docs/schemas/`** — the only machine authority: OpenAPI 3.1, domain-event JSON Schema, operator-stream, rich-presentation, error codes, 28 SQLite migrations.
5. **Ledgers** — `COVERAGE-MATRIX`, `V1-FUNCTION-CONTRACT-LEDGER` (226 functions), `V1-SETTINGS-CONTRACT-LEDGER` (187 controls), `V1-UI-ACTION-STATE-LEDGER`, `SEMANTIC-SERVED-UI-MAP` (traceability only; no authority).
6. **`docs/IMPLEMENTATION-MASTER-PLAN.md`** — P0–P11 delivery order; orders work, never semantics.
7. **`docs/archive/legacy-implementation-c7e9c05/`** — historical evidence; explicitly not current truth.

## 5. Architecture, compressed

**Identity discipline.** Conversation, Task, Step, Attempt, Agent, AgentSession, AgentHostInstance, Runtime, RuntimeIncarnation, Environment, CapabilityInvocation, Capability, Resource, Effect, Artifact, Evidence and ExecutionLease are all distinct. Runtime is installation-scoped (not workspace-scoped); a restart creates a new RuntimeIncarnation; workspace access needs an explicit, revocable `RuntimeWorkspaceBinding`.

**Authority discipline.** Installation ≠ connection ≠ assignment ≠ grant ≠ activation ≠ invocation. Agent-model-account routes are qualified only for an exact AgentModule + auth route + OS + version (`AGENT-ACCOUNT-ROUTE-MATRIX`). There is deliberately **no global provider/model router**; local models are the only separate inventory.

**Effect discipline.** Core governs only mediated calls. Every consequential operation needs a durable CapabilityInvocation, idempotency where retry matters, an Effect with `PROPOSED → STARTED → ACKNOWLEDGED → OBSERVED → VERIFIED | FAILED | AMBIGUOUS`, and reconciliation before retry. Unknown stays unknown; a worker saying "done" is a proposal.

**Recovery discipline.** Task state is the recovery floor; native sessions are optimizations. Leases are epoch-fenced; live processes never "teleport"; continuation starts a new Attempt from portable state after effect reconciliation.

**Presentation discipline.** Committed `ConversationMessage` is semantic truth; `RichPresentation` is optional, immutable and bound to the message digest; streamed frames cannot create or settle domain state.

**Protocol separation.** Operator (local IPC), Mesh (Runtime↔Runtime), Agent (ACP/A2A/SDK/CLI), Capability (Gateway MCP / Broker / LiteSPM), Environment (EnvironmentProvider), Human channels (Telegram/Slack/…). Six protocols, six authorities.

## 6. Machine contracts (exact, verified by parsing)

- **Operator API** (`operator-api.openapi.yaml`, 10,045 lines): OpenAPI 3.1.0 — **203 paths, 229 operations, 405 component schemas**, 2 security schemes (`workspaceContext`, `operatorAuth`), 8 shared parameters (`WorkspaceId`, `WorkspaceContextHeader`, `IdempotencyKey`, `IfMatch`, `Cursor`, `Limit`, `ConversationId`, `TaskId`). Methods: 108 GET, 107 POST, 8 PATCH, 6 PUT, **0 DELETE** (lifecycle is state-based). Largest tag groups: `resources` 29, `tasks` 27, `agents` 22, `conversations` 18, `workspaces` 15, `automations` 14, `capabilities` 13. The YAML parses cleanly; one earlier "invalid YAML" claim was a false positive caused by output redaction.
- **Domain events** (`domain-event.schema.json`, 12,309 lines): **148 event types**, 15 required envelope fields (`event_id`, `workspace_id`, `entity_type`, `entity_id`, `origin_runtime_id`, `origin_sequence`, `entity_revision`, `hlc_timestamp`, `correlation_id`, `schema_version`, `type`, `payload`, `aggregate_state_ref`, `recorded_at`, `payload_digest`), 5 `.v2` types coexisting with their v1 siblings. `entity_type` is a free string, so event↔entity binding is not enforced by schema.
- **Storage**: 28 immutable migrations (v1 superseded-down, v2–v28 forward), **143 tables, 282 triggers, 8,061 SQL lines**; WAL, foreign keys, `json_valid` checks, version CAS, `request_dedup` receipts, blob store with AEAD. Full inventory in `contracts-inventory.md`.
- **Errors**: 130 codes in `error-codes.schema.json` (owner prose cites 131 — off by one).
- **Stream**: typed Operator frames (`STREAM_READY`, `PROJECTION`, `TURN_DELTA`, `TURN_SETTLED`, `RICH_DRAFT`, `RESYNC_REQUIRED`), `no-store`, resync on gap.

## 7. What the code actually does

**Transport and trust root.** `litecoworkd` has **no HTTP listener**. It serves an authenticated local IPC frame protocol and drives the Axum router through `oneshot`. Peer identity is a kernel fact: Unix peer-UID match (socket `0600` in a `0700` dir, inode-identity rebind refusal) or Windows named pipe with a protected DACL; the authenticated peer is a private request extension, unforgeable by header. There are 106 Tauri commands on the UI side, each a fixed operation with validated ids — the WebView never receives a socket path, PID or token.

**Durability.** One writer thread, `IMMEDIATE` transactions, version CAS, receipt-before-effect idempotency (`request_dedup(principal_id, request_id)` + `response_digest`), events written with an immutable `AggregateStateRef` blob verified by read-back. Exact retries return byte-identical bodies across process restart; changed payloads yield `409 CONFLICT`; stale `If-Match` writes no receipt. Real child-process tests cover lost-ACK, unread response and TERM drain.

**Deliberate fail-closed gates (verified in code).** `CONVERSATION_SESSION_ADMISSION_UNAVAILABLE` (503 when no supervisor) · `TASK_PLANNING_ISOLATION_UNAVAILABLE` (planning sessions cannot activate: `storage-sqlite/src/lib.rs:249`) · `TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED` · `ENVIRONMENT_DESTROY_ADMISSION_UNAVAILABLE` · `DispatchAdmissionUnavailable` for effect `STARTED` · `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` · isolation attestation that reports every dimension `Unknown` (so admission always refuses) · `PACKAGE_SERVICE_NOT_QUALIFIED`.

**What is thinner than the prose.** `local_filesystem.rs` is read-only (no write/rename/delete); `environment_action_batch.rs` only plans; `opencode_server.rs` is transport-only; capability invocation create/dispatch has no writer; approval-use consumption is absent. Effects, environments and delegation are specified far beyond what is enforced today.

**Trust is a surface, not yet a gate.** `apps/litecoworkd/src/trust.rs` (74 KB policy engine, two rules: `workspace.resource.read` ALLOW, `external.message.send` REQUIRE_APPROVAL) compiles but has **zero callers** outside itself; `trust_operator.rs` is mounted (`operator.rs:1538`), so trust/approval reads are served while automation run/test-run admission and other mutations perform no policy evaluation. The mounted routes make the surface look governed; the enforcement path is not connected.

**UI.** `src/api/operator-api.ts` (13,832 lines) is **types only** — no client, no auth injection; transport is Tauri IPC. `App.tsx` is a 4,499-line god module (routing, Home, TaskList, TaskDetail, Settings, Library inline) with dead navigation config and an unused `SettingsView` import. Some UI strings still fabricate state ("Showing the 5 most recent Tasks" ignoring `limit`, hard-coded "now", static "Windows Job Object Active" badges, `SENDING_NOT_AVAILABLE` first-use guidance). No CSP was visible in `index.html`; that must be asserted from `tauri.conf.json` before any qualification claim.

**Crates.** Clean hexagonal layering: `domain-*` pure decision functions (serde only), `storage-core` ports/records (no rusqlite/tokio), `storage-sqlite` all SQL/transactions/blobs/keyring, `windows-local-security` the only crate allowed `unsafe`. No dependency cycle — but only because `domain-effects` has zero internal deps. Duplicated primitives across ≥5 crates (`valid_id`, `valid_digest`, `sha256:{hex}`), state names hard-coded in SQL strings, and a second plan-contract authority inside `planning.rs` (imperative validator + JSON Schema, no generation link).

## 8. Where the repository actually stands

**Timeline (git-verified).**

| Date | Event |
|---|---|
| 2026-10-30 | Docs-first reset: source moved to `archives/legacy-codebase-c7e9c05/`, old docs to `docs/archive/…` |
| 2026-10-31 | `e888ac0` "updates" — docs-only tree; `ARCHITECTURE.md` note says the executable snapshot is *only* in the archive |
| 2026-11-01 | `fd53630` "production release implementation and qualification on Windows" re-adds **615 files / +358,326 lines** (`apps/`, `crates/`, `capabilities/`, `scripts/`, `tests/`) |
| 2026-11-02 → 11-04 | 9 more commits: runtime bootstrap, pipe DACL fix, fail-closed dispatch, workbench overhaul, sidebar/settings |
| 2026-11-04 | `f22ec11` — HEAD |

**So the tree is hybrid:** the prose constitution still says "no current executable implementation in this checkout", while the code is tracked, builds, and `CURRENT-RUN.md` claims a qualified Windows production release (830+ tests, `litecowork-ui.exe` 15.9 MB + `litecoworkd.exe` 33.4 MB built 2026-11-03, both present but gitignored).

**Evidence reading against the repo's own ladder:**
- **TARGET** — extremely thorough, internally mostly consistent.
- **CODE** — credible and broad: 1,877 tests, real child-process replay tests, real Windows DACL/IPC tests (some `#[ignore]`d and env-gated).
- **SERVED** — partial: many routes mounted, but `API.md` itself records routes "declared in OpenAPI, no handler", and the seven hard gates above mean whole families answer 503 by design.
- **INTEGRATED** — not demonstrated: no qualified native session path; Send is fail-closed without a supervisor, and with one it returns 202 *before* dispatch (dispatch failure is only `eprintln!` at `conversation_operator.rs:577`, with no compensating settlement — a durable `OPEN` turn can outlive a failed dispatch).
- **QUALIFIED** — no dated SYSTEM/USER evidence exists in the repo for any OS/build/agent tuple. There is no `evidence/` tree; test results live in prose.

**Therefore:** the honest label is "CODE-complete in slices, SERVED partially, INTEGRATED/QUALIFIED unproven". "PRODUCTION-READY" in `CURRENT-RUN.md` is a claim the repository's own rules forbid without fresh, dated, owner-signed evidence.

## 9. Verified contradictions (worth resolving before any release claim)

| # | Conflict | Where |
|---|---|---|
| 1 | "Docs-only, executable snapshot is only in the archive" vs a tracked, building source tree | `ARCHITECTURE.md` status note, `docs/README.md`, `AGENTS.md` §7 vs `fd53630` + HEAD |
| 2 | "PRODUCTION-READY, all phases complete" vs fail-closed Send/planning/effects and `API.md`'s own target-vs-served ledger | `CURRENT-RUN.md` §1–3 vs `docs/API.md`, code gates |
| 3 | Spec 001 status: **DRAFT** (index) vs **ACCEPTED/LOCKED v1.0** (001 README/SPEC/PLAN/TASKS) vs **DRAFT, `spec_revision: 0.1`** (001 DATA-MODEL/RESEARCH/WIRING) vs the register's own row saying LOCKED *and* "none are ACCEPTED/LOCKED" | `specs/README.md`, `specs/001-*/`, `docs/V1-FEATURE-SDD-REGISTER.md`, `docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md` |
| 4 | Workbench default: §1 says visible at launch in compact Start state; §14 (dated 2026-10-10, labelled "Final Workbench decision") says **not** always open by default, new chats start at maximum conversational width | `COWORKERS-TARGET.md` §1 vs §14; `V1-PRODUCT-RELEASE-CONTRACT.md` §4 sides with §1 |
| 5 | Planning sessions described as reachable vs hard-closed in storage | `docs/STATE-MACHINES.md` / plan docs vs `TASK_PLANNING_ISOLATION_UNAVAILABLE` |
| 6 | 131 error codes in prose vs 130 in the schema | `docs/SCHEMAS.md`/`API.md` vs `docs/schemas/error-codes.schema.json` |
| 7 | Archived docs cited as live authority | `COWORKER-FLOWS.md` → `archive/…/UI.md`; `OBSERVABILITY.md` UAT gate |
| 8 | Event payload shape enforced twice: `docs/schemas/` and `json!` equality inside `storage-sqlite/src/lib.rs` | parallel schema authority risk |
| 9 | UI fabrications (counts, "now", static isolation badges) vs the "never style absent data as success" rule | `App.tsx`, settings badges, `first-use-guidance.ts` |
| 10 | `check.sh` gate never runs the SDD validator or the ZIP-intake suite | `scripts/check.sh` vs `specs/README.md`, `capabilities/zip_intake/` |
| 11 | Trust policy engine compiled but never called; automation admission does not evaluate policy | `apps/litecoworkd/src/trust.rs` (0 external callers) vs `ARCHITECTURE.md` "Core owns Trust decisions" |
| 12 | No `LICENSE`/`NOTICE` file in the active checkout, while `CURRENT-RUN.md` declares a production release | repo root; `repo_take/README.md` warns about outbound licensing |

## 10. Risks, in the order they would bite

1. **Posture ambiguity** — a contributor reading `AGENTS.md` believes there is no source; a contributor reading `CURRENT-RUN.md` believes it ships. Both are wrong. Pick one and update the owner docs.
2. **Release claim without evidence** — "production-ready" is currently prose, not a dated matrix. Publishing on it would violate the repo's own §5 vocabulary.
3. **Dispatch after 202** — a supervisor failure leaves an `OPEN` turn with no operator-visible signal; the exact "timeout-after-dispatch" case the acceptance plan demands.
4. **Isolation-by-unknown** — because every isolation dimension reports `Unknown`, all native Task execution is blocked; that is safe today but means the P4 gate cannot be passed without implementing attestation.
5. **Trust not enforced** — the policy engine is dead code; automation and other mutations admit work without evaluating policy. Any claim that Core "governs approvals" is currently a projection, not a gate.
6. **No licence** — no `LICENSE`/`NOTICE` at the repo root, which blocks distribution regardless of build quality.
7. **Parallel authorities** — event payload shapes, plan bounds, error codes and state literals each exist twice (schema + code string).
8. **UI truthfulness regressions** — hard-coded counts and static badges undermine the strongest product rule (`unknown ≠ success`).
9. **Docs drift inside the owner set** — `COMPLETE-ACTIVE-CORPUS-INVENTORY` counts are stale; `VIABILITY` marks research items "LOCKED DESIGN" that owners contradict; archive links survive in normative sentences.

## 11. Next safe actions (no code changes, in order)

1. Decide the posture and make it true in one place: either restore the docs-only claim (untrack source) or update `ARCHITECTURE.md`, `docs/README.md`, `AGENTS.md` §7 to "source present, evidence ladder applies".
2. Downgrade `CURRENT-RUN.md` from "PRODUCTION-READY" to the evidence actually held (CODE + partial SERVED), or attach the dated matrix that justifies it.
3. Resolve spec 001 status to a single value in `specs/README.md`, `specs/001-*/README.md`, and its DATA-MODEL/RESEARCH/WIRING headers.
4. Fix the 202-then-dispatch hole with a compensating settlement on supervisor failure.
5. Promote one machine authority for event payload shape (generate the Rust check from `docs/schemas/`).
6. Run `python .specify/scripts/validate_feature_package.py` (passes today) *and* `scripts/check.sh` in the release gate; add the ZIP-intake suite.
7. Only then pick the first GA OS/agent tuple and produce dated SYSTEM + USER evidence under `docs/ACCEPTANCE-AND-E2E-PLAN.md`.

## 12. Where the detail lives

- `reading/COVERAGE.md` — what was read, how deep, what remains.
- `reading/contracts-inventory.md` — all 229 operations and every migration/table/trigger.
- `reading/digest-*.md` — 32 per-bucket digests (per-file notes, exact identifiers, coverage checks).
- `reading/_coverage.md` — every reader's own statement of what it did not finish.
- `reading/inventory.tsv` — the 1,056-file manifest with reader assignment.
