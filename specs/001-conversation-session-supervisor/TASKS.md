# Tasks - Conversation Session Supervisor and Native Turn Execution

**Feature:** 001 | **Status:** ACCEPTED/LOCKED - ready for execution.  
**Spec revision:** 1.0 | **Owners:** domain owners named in SPEC.md.

## 001-T01 - Resolve first-provider and Conversation admission decisions

- **Status:** COMPLETED.
- **Depends on:** NONE.
- **Owners:** Product, Conversation, Agent Runtime, Security.
- **Allowed paths:** this feature package; owner docs only after decision.
- **Contract:** SPEC sections 5, 6, 7.2, 7.4, 7.6, 9, 10, 15.
- **Change intent:** resolve B-01, B-02, B-03, B-05, B-07, B-08, B-09 and B-10; identify one exact OS/build/harness/adapter/auth route; freeze concurrency, readiness, input/attachment, event and process lifecycle contract. Apply the existing Conversation-scoped UserRequest continuation contract; do not create a competing flow.
- **Acceptance:**
  - [x] Every decision has an owner-authored resolution and source evidence (B-01 through B-10 resolved).
  - [x] First target provider handshake and exact version are testable (Windows MSVC + Claude Code / Codex).
  - [x] Context over-limit policy and process ownership are explicit (sliding window + Job Object).
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** any handshake/host identity ambiguity leaves feature blocked.

## 001-T02 - Specify and approve atomic final Agent response commit

- **Status:** READY.
- **Depends on:** T01.
- **Owner:** Storage and Conversation.
- **Allowed paths:** after approval, `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, related typed event/schema/docs/tests.
- **Contract:** SPEC 7.3, DATA-MODEL, contracts/api-events.md.
- **Change intent:** define one typed storage port and SQLite IMMEDIATE transaction for AGENT message + terminal turn + snapshots + events + receipt. Exact public payload/version must match canonical event and schema.
- **Acceptance:**
  - [x] Exact request/result types and idempotency identity are approved (commit_conversation_agent_response designed).
  - [ ] Foreign owner, wrong Workspace, wrong active session, stale version and wrong retry ordinal fail with no partial writes.
  - [ ] Duplicate exact final result returns one stable receipt/message; conflicting replay fails.
  - [ ] Crash/rollback tests prove atomicity.
- **Command:** `cargo test -p storage-sqlite --locked conversation_agent_response_commit` (test filter to be added).
- **Stop:** never expose final answer as complete without commit receipt.

## 001-T03 - Implement bounded AgentSessionSupervisor and AgentHostSupervisor lifecycle

- **Status:** NOT STARTED.
- **Depends on:** T01.
- **Scheduling:** Independent of T02; T02 and T03 may proceed in parallel after T01 locks shared interfaces. Both are prerequisites for T06.
- **Owner:** Agent Runtime.
- **Allowed paths:** focused `apps/litecoworkd/src/agents/` and dedicated tests.
- **Contract:** SPEC 5, 6, 7.1-7.2, 8; SERVICES; STATE-MACHINES; AGENT-CONTROL.
- **Change intent:** introduce supervisor interfaces, bounded startup, exact host/Runtime pin revalidation, STARTING reservation, activation, normalized lifecycle, quiescence, CLOSE/LOST, local handle lifetime, startup recovery enumeration.
- **Acceptance:**
  - [ ] Host process ownership and session ownership remain separate.
  - [ ] No native call occurs inside SQLite.
  - [ ] Only observed exact readiness activates.
  - [ ] Handle is never returned/serialized/logged; binding is deleted only after CLOSED/LOST.
  - [ ] Foreign, stale, degraded and wrong-incarnation host cases fail closed.
- **Command:** `cargo test -p litecoworkd --locked conversation_session_supervisor_lifecycle` (filter to be added).
- **Stop:** unknown provider lifecycle or unbounded queue halts admission.

## 001-T04 - Qualify one native AgentAdapter

- **Status:** BLOCKED - B-02/B-03/B-05.
- **Depends on:** T01, T03.
- **Owner:** Agent Runtime.
- **Allowed paths:** one selected adapter module, adapter contracts and fixtures.
- **Contract:** SPEC 7.4, 8, 10; AGENT-FABRIC; NATIVE-COMPOSER-LLD.
- **Change intent:** wrap one version-pinned harness transport behind AgentAdapter; implement startup/readiness, context prompt dispatch, bounded events, interrupt and stop/quiescence.
- **Acceptance:**
  - [ ] Exact session and host identity matches reservation.
  - [ ] Prompt dispatch occurs only after activation commit.
  - [ ] Duplicate/out-of-order/malformed/oversized events cannot mutate state.
  - [ ] Provider timeout-after-dispatch is reconciled, not blindly resent.
  - [ ] Auth/model/setup failure never auto-falls back.
- **Command:** `cargo test -p litecoworkd --locked selected_adapter_conversation` (filter to be added).
- **Stop:** if native API can't prove readiness or safe stop, do not qualify.

## 001-T05 - Dispatch bounded context and supported resources

- **Status:** NOT STARTED.
- **Depends on:** T01, T03, T04.
- **Owners:** Conversation, Resource, Agent Runtime.
- **Allowed paths:** focused context materializer and adapter tests.
- **Contract:** SPEC 7.5, 8, 10; STORAGE; SCHEMAS; TRUST.
- **Change intent:** consume ConversationContextPackage from immutable turn boundary; reauthorize ResourceRefs and project supported content to selected adapter under its exact bound.
- **Acceptance:**
  - [ ] Only committed same-Workspace transcript prefix is provided.
  - [ ] Prior AGENT messages retain source binding provenance.
  - [ ] Resource revision/root/location checked before bytes materialize.
  - [ ] Unsupported/oversize content fails visibly or uses owner-approved reviewed reduction.
  - [ ] Never silently truncate or cross provider private context.
- **Command:** `cargo test -p litecoworkd --locked conversation_context_dispatch` (filter to be added).

## 001-T06 - Implement final response and terminal settlement

- **Status:** BLOCKED - T02.
- **Depends on:** T02, T03, T04, T05.
- **Owners:** Conversation, Storage.
- **Allowed paths:** Conversation result writer, storage ports/SQLite adapter and adjacent tests.
- **Contract:** SPEC 7.3, AC-009; EVENTS; STORAGE; STATE-MACHINES.
- **Change intent:** persist one final AGENT message with exact session/binding/retry provenance and turn terminal state in the same transaction.
- **Acceptance:**
  - [ ] Message, terminal turn, snapshots, events and receipt commit atomically.
  - [ ] Repeated provider final or retry returns one committed message only.
  - [ ] Failed commit is reconciled by stable result identity without provider resend.
  - [ ] Failed/cancelled turns do not promote partial deltas to durable answer.
- **Command:** `cargo test -p storage-sqlite --locked conversation_agent_response_commit` (filter to be added).

## 001-T07 - Complete stop, retry, provider input and stream integration

- **Status:** NOT STARTED.
- **Depends on:** T01, T03, T04, T06.
- **Owners:** Conversation, Agent Runtime, UI.
- **Allowed paths:** Conversation Operator, supervisor, Tauri stream bridge, `apps/litecowork-ui/src/conversations/` and relevant tests.
- **Contract:** SPEC 7.2, 7.6, AC-010/011/014/017/018/019; API; EVENTS; STATE-MACHINES; STORAGE; TRUST.
- **Change intent:** connect cancel/retry and Conversation-scoped UserRequest continuation; reuse durable projection cursor and optionally bounded transient frames. No separate UI event transport.
- **Acceptance:**
  - [ ] Cancel waits for observed adapter stop/quiescence.
  - [ ] Winning completion/failure is preserved.
  - [ ] Retry uses new session and retry ordinal; no message duplication.
  - [ ] A valid Conversation-scoped response is immutable and moves the exact WAITING_USER turn to WAITING_DEPENDENCY before continuation is enqueued.
  - [ ] MCP update acknowledgement is not acceptance; reconcile the exact request key through the provider task, and do not start a fresh AgentSession before provider acceptance and session readiness.
  - [ ] Rejection that remains INPUT_REQUIRED closes the old request, creates a new one for the current key and returns the turn to WAITING_USER; ambiguity remains WAITING_DEPENDENCY without a new session.
  - [ ] Stream reconnect/resync uses durable snapshot and never concatenates across gaps.
- **Command:** `pnpm test -- conversation-stream` plus daemon lifecycle integration (exact workspace command to be confirmed from source).

## 001-T08 - Implement daemon restart and orphan recovery

- **Status:** NOT STARTED.
- **Depends on:** T03, T04, T06, T07.
- **Owners:** Agent Runtime, Storage, Conversation.
- **Allowed paths:** bounded startup recovery scanner, supervisor and storage tests.
- **Contract:** SPEC 7.2, 9, AC-012; STATE-MACHINES; FAILURE-RECOVERY.
- **Change intent:** reconcile STARTING/ACTIVE/INTERRUPTING/CLOSING records against exact Runtime incarnation/process/adapter evidence; mark LOST or finish close safely.
- **Acceptance:**
  - [ ] Prior-incarnation STARTING becomes LOST unless safe same-incarnation resume is positively proved.
  - [ ] Unknown ACTIVE session state cannot be reported as completed.
  - [ ] Orphan handles and host-use references are released only after safe close/loss settlement.
  - [ ] Restart and repeated recovery sweeps are idempotent.
- **Command:** `cargo test -p litecoworkd --locked conversation_session_recovery` (filter to be added).

## 001-T09 - Qualify one end-to-end native Conversation route and gate Send

- **Status:** NOT STARTED.
- **Depends on:** T01-T08.
- **Owners:** Product, Agent Runtime, Conversation, UI, Security.
- **Allowed paths:** targeted tests, qualification report, eligibility gates and docs.
- **Contract:** SPEC 7-15; AC-001/002/003/004/005/006/007/008/009/010/011/012/013/014/015/016/017/018/019; ACCEPTANCE-AND-E2E-PLAN; TESTING.
- **Change intent:** exercise the exact pinned OS/build/harness/version/auth path; enable Send only for a qualified tuple.
- **Acceptance:**
  - [ ] The future e2e target `cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture` exists and passes.
  - [ ] CODE, SYSTEM and owner USER evidence covers positive, denial, timeout, duplicate, cancel race, crash, restart and reconnect.
  - [ ] No prompt/handle/secret leaks or automatic fallback.
  - [ ] UI and API eligibility match actual qualified capability.
  - [ ] Current evidence report is dated and version-pinned.
- **Command:** cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture (target to be created).
- **Stop:** any unqualified OS/agent/auth tuple remains unavailable even if another tuple passes.

## Dependency order

T01 -> T02 -> T03 -> T04 -> T05 -> T06 -> T07/T08 -> T09. T03 may be developed in parallel with T02 only after T01 locks interfaces and with no conflicting storage mutation. T04 may not dispatch before T03 readiness/activation contract exists.

## Completion evidence

Record per-task code revision, exact command, result, target OS/harness/auth, test report and reviewer in this file. No commit or push is implied by task completion.

