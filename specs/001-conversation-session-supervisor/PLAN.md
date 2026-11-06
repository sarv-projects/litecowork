# Implementation Plan - Conversation Session Supervisor and Native Turn Execution

**Feature:** 001 | **Status:** IN_REVIEW | **Spec:** SPEC.md revision 1.0
**Updated:** 2026-11-01  
**Rule:** This plan is under owner review. Implementation evidence is recorded separately from qualification; no feature-level acceptance is implied.

## 1. Delivery strategy

Deliver one qualified native chat vertical slice for one pinned OS/build, one selected harness/version, one adapter version and one auth route. Do not enable general multi-agent support or all platforms by inference. The active main branch contains source. Before implementation, inspect the current working tree and ignored worktrees, preserve user changes, and select the intended clean task worktree. Do not reset or mutate unrelated branches.

The durable transaction path is built before UI enablement. Adapter launch is outside SQLite. Readiness activation and final response settlement are separate atomic storage operations. Native events are normalized and never become canonical Conversation state directly.

## 2. Dependency graph

```mermaid
flowchart TD
  T1[Resolve blocking product/adapter decisions] --> T2[Freeze final response storage contract]
  T1 --> T3[Define host/session supervisor lifecycle]
  T2 --> T6[Implement response commit + receipt]
  T3 --> T4[Qualify adapter readiness/input/events]
  T4 --> T5[Context/resource dispatch]
  T5 --> T6
  T3 --> T7[Stop/retry/provider input/stream]
  T6 --> T7
  T3 --> T8[Daemon restart and orphan recovery]
  T4 --> T8
  T6 --> T8
  T7 --> T8
  T8 --> T9[System and owner qualification]
```

## 3. Phases and exit gates

| Phase | Tasks | Outcome | Exit gate |
|---|---|---|---|
| A - contract lock | T01 | Blocking decisions B-01 to B-05 and B-07 to B-10 resolved or explicitly scoped; inherited B-06 owner contract confirmed | Owner sign-off; SPEC accepted/locked; validator/link/schema pass |
| B - durable completion boundary | T02, T06 design | Typed result commit, message+turn transaction and event/receipt semantics frozen | Storage review; transaction/rollback/idempotency tests specified |
| C - lifecycle supervisor | T03 | Bounded supervisor/host/session lifecycle, STARTING -> activation -> quiescence/close/recovery | State transition and crash-window matrix passes |
| D - one adapter | T04 | Version-pinned adapter handshake, prompt transport, normalized terminal/delta/error/stop observations | Provider conformance fixtures and exact readiness proof |
| E - context dispatch | T05 | Same-Workspace bounded transcript, supported resource preparation and explicit oversize behavior | Context/resource boundary and prompt delivery tests |
| F - durable result | T06 | Atomic AGENT message + terminal turn + event/snapshot/receipt | Failure injection at every transaction boundary passes |
| G - user lifecycle and recovery | T07, T08 | Stop/retry/UserRequest continuation, stream/reconnect, daemon restart and orphan recovery | UI/daemon/DB integration, race and restart tests |
| H - qualification | T09 | One actual OS/build/harness/auth route accepted | Full CODE/SYSTEM/USER/recovery report; only then enable Send |

## 4. Scope and file ownership (candidate paths, re-inspect before implementation)

| Area | Candidate paths | Owner | Guardrail |
|---|---|---|---|
| Operator/request admission | `apps/litecoworkd/src/conversation_operator.rs` and focused tests | Operator/Conversation | Auth, receipt, If-Match and fail-closed behavior |
| Session/host lifecycle | focused `apps/litecoworkd/src/agents/` modules | Agent Runtime | No direct turn aggregate writes from Supervisor |
| Storage API | `crates/storage-core/src/conversation.rs`, `crates/storage-core/src/lib.rs` | Storage | Exact request/result contract; handle redaction |
| SQLite operations | `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/lib.rs` | Storage | Immediate transaction; immutable event prefix; atomic event/snapshot/receipt |
| Native adapter | one selected `apps/litecoworkd/src/agents/<adapter>.rs` | Agent Runtime | Transport only; version pin; bounded untrusted frames |
| Desktop UI | `apps/litecowork-ui/src/conversations/` | UI | Keep disabled until end-to-end qualification |
| Tauri bridge | `apps/litecowork-ui/src-tauri/src/conversation_stream_bridge.rs` | Desktop bridge | Existing projection cursor/backpressure |
| Contracts/docs | API, EVENTS, STORAGE, SERVICES, STATE-MACHINES, SCHEMAS, AGENT-CONTROL, NATIVE-COMPOSER | Domain owners | Promote global contracts atomically |
| Tests | adjacent unit/integration/contract plus one feature e2e target | Each owner | Fail closed, crash/race/replay coverage |

Do not edit a candidate path just because it is listed. Only files needed for the accepted task are in scope. No unrelated formatting sweep.

## 5. Persistence, migration, rollout

- First inspect current sqlite-v1 through sqlite-v28 and schema validators in the chosen implementation worktree.
- Reuse existing Conversation and AgentSession identities, event streams, snapshots and idempotency patterns.
- The activation storage transaction is already present in the preserved dirty worktree; do not reimplement or cherry-pick it without review.
- The final result commit may require new local schema fields/indexes or no migration if existing tables can satisfy the semantics. Decide only after exact storage model review.
- Any migration is forward-only, numbered, immutable after release, checksummed and covered by upgrade/rollback-to-old-binary compatibility tests.
- Do not enable UI Send until operator, supervisor, adapter, final result writer, stream/reload and failure recovery all qualify. A feature flag must not bypass missing server-side capability.
- Rollout starts with one explicitly pinned target environment; no broad capability claim until each OS/harness/auth tuple is qualified.

## 6. Operational risks

| Risk | Mitigation / gate |
|---|---|
| Host READY confused with session readiness | Exact adapter SESSION_READY handshake; activation transaction rechecks all pins |
| Turn pinned selector drift | Immutable turn-created event is sole authority |
| Duplicate prompt on timeout | Distinguish pre-dispatch from timeout-after-dispatch; reconcile by stable native request/session identity |
| Duplicate final message | Idempotent atomic result transaction keyed to turn/session/retry/response identity |
| Stale or hostile provider frames | Bound, normalize, correlate, order, redact and treat as untrusted |
| Daemon crash between reservation and activation | STARTING recovery sweep and LOST settlement; no turn falsely RUNNING |
| Daemon crash after activation | Query/reconcile adapter/host for exact same incarnation; never replay across incarnations |
| Stop/final race | Observe adapter quiescence; persist whichever terminal observation wins |
| Resource oversize or unsupported attachment | Explicit typed rejection; no silent omission |
| UI transient gap | Drop transient content and resync durable projection |
| Native direct Effect misrepresented | Clearly label assurance boundary; don't claim Core mediation |

## 7. Validation command matrix

| Gate | Exact command/target | Expected result |
|---|---|---|
| Storage readiness foundation | `cargo test -p storage-sqlite --locked conversation_activation_commits_session_turn_and_local_host_binding_atomically -- --nocapture` | Existing focused test passes in preserved worktree; not final feature qualification |
| Storage package regression | `cargo test -p storage-sqlite --locked` | Entire suite passes after final changes, with documented platform-only ignored tests |
| Daemon supervisor unit/contract | `cargo test -p litecoworkd --locked conversation_session_supervisor` | Bounded lifecycle/error/race tests pass |
| Feature end-to-end (PROPOSED target) | `cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture` | Actual daemon+SQLite+adapter fake/fixture e2e passes; test target must be created |
| Documentation contracts | `python3 .specify/scripts/validate_feature_package.py` | Package structure, links, traceability and wiring pass |
| Docs/schema validation | owning repository validator commands from `docs/TESTING.md` | OpenAPI/event/SQL/schema and coverage consistent |
| Target OS qualification | pinned GUI/daemon/native agent test harness | Send, stop, retry, reconnect and recovery pass with owner USER evidence |

## 8. Stop and rollback criteria

Stop immediately on unresolved binding, missing handshake proof, unexpected provider fallback, secret/handle leakage, ambiguous dispatch, unbounded buffer, missing result receipt, or failure to reconcile stop/restart. Keep Send fail-closed. Do not restore a prior session handle to a new Runtime incarnation. For schema work, use forward repair/reconciliation; never delete the user's committed USER message to hide a failed agent attempt.

## 9. Handoff

The first implementation handoff is blocked until SPEC status is ACCEPTED/LOCKED, blocking decisions B-01 through B-05 and B-07 through B-10 are resolved or scoped with named exclusions, the inherited B-06 owner contract is confirmed, the storage owner approves B-04 and the first harness/OS tuple is fixed. The implementation task must name the accepted spec revision and SHA, exact clean worktree, first task ID and exact acceptance command. No commit/push is included in this plan.

