# Tasks - Conversation Session Supervisor and Native Turn Execution

**Feature:** 001 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.1 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** domain owners named in SPEC.md. Owner acceptance: 2026-11-09 via docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md.

## Historical baseline (2026-11-09; superseded)

Verified against the checkout on branch `docs/audit-competitor-superset` (after commit 8129420).

- **Operator:** `apps/litecoworkd/src/conversation_operator.rs` mounts `POST/GET /v1/conversations`, `POST /v1/conversations/{conversation_id}/turns` (`submit_conversation_turn`), `.../turns/{turn_id}/retry` (`retry_conversation_turn`), `.../turns/{turn_id}/cancel` (`cancel_conversation_turn`), `PATCH .../agent-binding` and `GET /v1/conversations/{conversation_id}`. Submit commits USER message + OPEN turn + receipt through `create_conversation_turn`, then starts `AgentSessionSupervisor::dispatch_turn` in a detached `tokio::spawn`; a dispatch error is only printed with `eprintln!`. The module header comment (lines 1-2) still says turn dispatch is "intentionally absent", which is stale.
- **Supervisor:** `apps/litecoworkd/src/agents/supervisor.rs` `AgentSessionSupervisor::dispatch_turn` runs the whole turn inside `spawn_blocking`: `resolve_conversation_session_host_for_turn`, creates a `LOCAL_PER_SESSION` `AgentHostInstanceRecord` if missing, reserves STARTING with `start_conversation_agent_session`, then `execute_agent_turn` matches only `provider_key == "opencode"`. That branch calls `OpenCodeServer::spawn` per turn, `create_session`, `activate_conversation_agent_session`, `load_conversation_context_for_turn`, `send_conversation_context_async`, polls `next_event` every 500 ms up to a hard-coded 60 s deadline, calls `server.stop()` and commits with `commit_conversation_agent_response`. The commit request id is `req-commit-{final_message_id}` with a freshly generated message id, so a repeated commit attempt is not idempotent; `expected_session_version: 2` and the message event `entity_revision: 3` are hard-coded.
- **Adapter:** `apps/litecoworkd/src/agents/opencode_server.rs` pins `OpenCodeServerProtocol::V2_0_32`, caps a request at `MAX_REQUEST_BYTES` (512 KiB) and has `abort_session` (`/api/session/{session_id}/interrupt`) with no caller.
- **Storage:** `crates/storage-core/src/conversation.rs` defines `CommitConversationAgentResponse` without retry ordinal or provider-response identity, plus `build_conversation_context_package`. `crates/storage-sqlite/src/conversations.rs` `create_conversation_turn` does not check for another nonterminal turn (the test `conversation_turn_creation_serializes_appends_from_the_same_stale_snapshot` in `crates/storage-sqlite/src/conversation_turn_tests.rs` still admits two OPEN turns). `transition_conversation_turn` accepts `TurnCommand::Retry` only when an ACTIVE session with the new id already exists (`CONVERSATION_SESSION_NOT_READY`), while `retry_conversation_turn` generates a brand-new session id, so the retry route cannot succeed; `crates/domain-conversation/src/lib.rs` moves Retry straight to RUNNING. `list_starting_conversation_agent_sessions` and `mark_starting_conversation_agent_session_lost` exist (tests in `crates/storage-sqlite/src/agent_binding_tests.rs`) but no daemon startup caller exists; `recover_and_persist_startup_blockers` in `apps/litecoworkd/src/runtime.rs` is the startup recovery hook that does not call them. No dispatch outbox, no usage record table, and no `CONVERSATION_TURN_IN_PROGRESS` code in `docs/schemas/error-codes.schema.json`. Migration chain ended at `docs/schemas/sqlite-v28.sql` when this baseline was written (2026-11-09); it now ends at `sqlite-v32.sql` (v29 outbox and usage records, v30 turn activity, v31 and v32 from Features 014 and 012). The outbox, usage-record and `CONVERSATION_TURN_IN_PROGRESS` absences above are historical; each task status below records what exists now.
- **Tauri:** `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs` has `submit_conversation_turn`, `retry_conversation_turn` and `cancel_conversation_turn` (both send `If-Match` from `expected_version`) and `get_conversation_presentation`; `apps/litecowork-ui/src-tauri/src/conversation_stream_bridge.rs` has `subscribe_conversation_stream` and `acknowledge_conversation_stream_frame` for the durable projection stream (no native TURN_DELTA producer).
- **UI:** `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`. Commit 8129420 added the `sendEligible` / `bindingEligibleForSend` gate (register A-24) and the truthful empty state (register A-26 empty-state part), guarded by `apps/litecowork-ui/tests/conversation-empty-state-truthfulness.test.mjs`. Still broken: `isTurnRunning` checks only RUNNING plus transient STREAMING/RECONNECTING turns; `cancelTurn` searches `transientTurns` and sends `expectedVersion: 1`; no `retry_conversation_turn` call site; submit always sends `resourceRefs: []`; the "+" (Add context or file), Voice Input, More options and Turbo controls have no handlers; the agent `<select>` falls back to the fabricated label "Gemini 3.8 Flash Low" and the header falls back to "AI Capabilities Overview". `apps/litecowork-ui/src/conversations/conversation-view.ts` types `active_turn` as `{ status }` only (no version or retry ordinal).
- **Tests today:** `apps/litecoworkd/src/conversation_operator_tests.rs` (for example `conversation_turn_submit_returns_accepted_receipt_with_active_session_supervisor`), `crates/storage-sqlite/src/agent_binding_tests.rs` (`conversation_activation_commits_session_turn_and_local_host_binding_atomically`, `conversation_agent_response_commit_settles_turn_and_session_atomically`), `crates/storage-sqlite/src/conversation_turn_tests.rs`, UI `tests/conversation-stream.test.ts`, `tests/conversation-view.test.ts`. `apps/litecoworkd/tests/` contains only `cli.rs`; the canonical target `--test conversation_session_supervisor` does not exist.

Gaps to V1: Stop without quiescence, broken Retry, no outbox or startup sweep, no storage single-flight, B-04 identity fields, attachments, usage records, composer fencing and inert controls, and no CODE/SYSTEM/USER qualification on either release row. Cross-package: the Feature 002 OpenCode probe records `compatible=false`, so the desktop cannot create the OpenCode binding this package needs.

The preceding audit is preserved for provenance only. It must not be used as current source status: several listed absences were implemented after this snapshot. At the 2026-11-12 checkout (`6adadab`), inspect each task's status below and `docs/IMPLEMENTATION-STATUS.md`. Current examples: the dispatch outbox, usage records, single-flight admission, Stop/Retry UI paths, and text ResourceRef picker/materialization have CODE-level coverage; pinned OpenCode Form and permission APIs exist, but live event evidence and end-to-end answer/deny handling remain incomplete. T12 remains BLOCKED; T09 and T28 remain NOT STARTED; T27 remains PARTIAL. The packaged desktop journey and release qualification remain outstanding.

## 001-T01 - Record owner acceptance and promote accepted contracts into owner docs and schemas

- **Status:** PARTIAL - owner acceptance of B-01 to B-10 recorded 2026-11-09. Promoted and implemented in this slice: `CONVERSATION_TURN_IN_PROGRESS` (error-codes and domain-event schemas, SCHEMAS.md, API.md), migration `docs/schemas/sqlite-v29.sql` (dispatch intents, response commit identity, usage records, single-flight triggers; STORAGE.md migrations), v28 wired with it. Still not promoted: `retry_ordinal`/`provider_response_id` on the receipt schema, `active_turn.version`/`retry_ordinal` in the presentation snapshot, `turn.delta` correlation fields, Adapter 1.0 handshake names (T13/T16 own those).
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE (Feature 018 must agree the usage record shape it will read; record its review in the PR).
- **Owner:** Conversation, Storage, API, Events owners; Product for B-02 rider.
- **Allowed paths:** `docs/API.md`, `docs/EVENTS.md`, `docs/STORAGE.md`, `docs/SCHEMAS.md`, `docs/STATE-MACHINES.md`, `docs/SERVICES.md`, `docs/AGENT-CONTROL.md`, `docs/OBSERVABILITY.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `docs/schemas/operator-stream.schema.json`, `docs/schemas/sqlite-v29.sql` (new; use the next free number at implementation time), this package.
- **Contract:** SPEC sections 5, 7, 10, 12; AC-016, AC-023; SPECIFICATION-STANDARD section 7.
- **Change intent:** in one reviewed change promote: error `CONVERSATION_TURN_IN_PROGRESS` (409) for B-01; `retry_ordinal` and `provider_response_id` on the final-response commit and its receipt (B-04); `active_turn.version` and `active_turn.retry_ordinal` in the Conversation presentation snapshot; the durable dispatch-intent record and its states (UNCLAIMED, CLAIMED, SETTLED) for the outbox; the usage record (turn id, attribution_source PROVIDER_REPORTED / ESTIMATED_LOCAL / UNKNOWN, input/output token counts); the transient `turn.delta` frame correlation fields for B-05; the Adapter 1.0 handshake names from B-03 as the target contract the OpenCode adapter is wrapped behind. Each is labelled "accepted target interface (not yet in source)" until its implementing task lands. One forward-only migration with checksum/fingerprint update.
- **Acceptance:**
  - [x] B-01 to B-10 owner acceptance recorded with the B-02 rider and B-04/B-07/B-09 caveats (SPEC section 5).
  - [ ] Every promoted name exists exactly once in its owner doc and machine schema; no parallel definition in this package.
  - [x] The new migration is the next immutable `sqlite-vNN.sql`, older migrations are byte-identical, and the schema fingerprint test passes.
  - [ ] Negative: schema validators reject a commit receipt missing `retry_ordinal` and a frame missing turn/retry/session correlation.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py` and `cargo test -p storage-sqlite --locked schema` (filter to be added by this task for the fingerprint check)
- **Evidence:** PR link and command output in Completion evidence below; register rows for B-01 to B-10 closed by the orchestrator.
- **Stop:** a promoted name conflicts with an existing owner definition; resolve at the owner, not here.

## 001-T02 - Harden the atomic final Agent response commit (B-04 caveat)

- **Status:** DONE (storage) - `CommitConversationAgentResponse` now carries `retry_ordinal`, `provider_response_id` and optional usage; the IMMEDIATE transaction verifies the ordinal, the AGENT author pinned to the session binding, derives the receipt identity from `(turn_id, retry_ordinal)` with a content digest (exact repeat replays the stored receipt under a different request id, any other repeat is `CONVERSATION_RESPONSE_IDENTITY_CONFLICT`), settles the dispatch intent and writes the usage record. Evidence: `conversation_agent_response_commit_*` tests in `crates/storage-sqlite/src/agent_binding_tests.rs` and `conversation_durable_turn_tests.rs`.
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Conversation.
- **Allowed paths:** `crates/storage-core/src/conversation.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/agent_binding_tests.rs`, `crates/storage-sqlite/src/conversation_turn_tests.rs`.
- **Contract:** SPEC 7.3, B-04, AC-009, AC-026; DATA-MODEL section 5.
- **Change intent:** add `retry_ordinal: u32` and `provider_response_id: String` to `CommitConversationAgentResponse`; verify `retry_ordinal` equals the turn's current ordinal inside the transaction; derive the receipt identity from (turn_id, retry_ordinal, provider_response_id) instead of a caller-generated message id; validate `message.author` is `{kind: AGENT, agent_binding_id}` matching the turn-pinned binding; derive event revisions from loaded aggregate versions rather than trusting caller-supplied `entity_revision`.
- **Acceptance:**
  - [x] Typed request/result and SQLite operation exist and are invoked by the OpenCode supervisor.
  - [x] Exact replay with the same turn, retry ordinal and provider-response id returns the stored receipt and message (AC-026).
  - [x] Same identity with different content digest, different retry ordinal or different author fails with no partial write (AC-026).
  - [x] Foreign owner, wrong Workspace, wrong or non-ACTIVE session and stale versions fail with no partial write (AC-009).
  - [x] Fault injection after each insert (message, turn, session, host binding delete, events, receipt) rolls back everything.
  - [x] Two concurrent commits for one turn: exactly one wins, the other gets a conflict or the stored receipt.
- **Command:** `cargo test -p storage-sqlite --locked conversation_agent_response_commit`
- **Evidence:** test names and output recorded below; IMPLEMENTATION-STATUS.md Conversation final-commit row.
- **Stop:** any path exposes a final answer as complete without the commit receipt.

## 001-T03 - Complete bounded AgentSessionSupervisor and AgentHostSupervisor lifecycle

- **Status:** PARTIAL - per-turn `AgentSessionSupervisor` in `apps/litecoworkd/src/agents/supervisor.rs` reserves STARTING and activates; it also creates the host record and starts/stops `OpenCodeServer` itself, so host and session ownership are not separate, there is no idle TTL, and the Job Object/cgroup lifetime control from B-09 is absent.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 011 Runtime incarnation/readiness foundation (Wave 0).
- **Owner:** Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecoworkd/src/agents/mod.rs`, `apps/litecoworkd/src/agents/host_supervisor.rs` (new), `apps/litecoworkd/src/attempt_process_scope.rs` (read for reuse only unless a shared helper is extracted), `crates/windows-local-security/`, `crates/linux-process-scope/`, adjacent tests.
- **Contract:** SPEC sections 5 (B-03, B-09), 6, 7.1-7.2, 8; L-04 to L-07; AC-003, AC-004, AC-005, AC-012, AC-013; SERVICES; STATE-MACHINES.
- **Change intent:** introduce `AgentHostSupervisor` (new module) owning `AgentHostInstanceRecord` lifecycle, the OpenCode server process under a Windows Job Object or Linux cgroup (process lifetime only, never described as isolation), live use references and the 10-minute idle TTL with graceful stop, 5 s wait, then kill of LiteCowork-owned processes only. `AgentSessionSupervisor` keeps STARTING reservation, readiness check, `activate_conversation_agent_session`, quiescence and CLOSED/LOST. Replace the hard-coded `expected_session_version: 2` with the committed session version. Move the blocking per-turn flow off a single long `spawn_blocking` call (see optimization notes).
- **Acceptance:**
  - [ ] Host process ownership and session ownership are separate types; no turn aggregate write from either supervisor (AC-004).
  - [ ] No native call occurs inside a SQLite transaction (code review plus test with a store that panics if called re-entrantly).
  - [ ] Only an observed native session handshake activates; Runtime/AgentHost READY alone never reports RUNNING (AC-005).
  - [ ] The turn-pinned binding is used even after the selector changes mid-dispatch (AC-003).
  - [ ] Handle never serialized, logged or returned; host binding deleted only after CLOSED/LOST (AC-013).
  - [ ] Foreign, stale, degraded and wrong-incarnation hosts fail closed; an externally owned process is never killed.
  - [ ] Idle TTL stops the owned process after the last reference with no zombie child (Windows and Linux).
- **Command:** `cargo test -p litecoworkd --locked conversation_session_supervisor_lifecycle` (filter to be added by this task)
- **Evidence:** test output; Windows and Linux process-tree check recorded in Completion evidence.
- **Stop:** unknown provider lifecycle or an unbounded queue halts admission.

## 001-T04 - Qualify the OpenCode v2.0.32 AgentAdapter behind the Adapter 1.0 contract

- **Status:** PARTIAL - the provider-neutral `AgentAdapter` trait (probe_version, create_session, send_context, drive_turn, interrupt, shutdown), the OpenCode v2.0.32 `OpenCodeAdapter` (wraps `OpenCodeServer`, `interrupt` calls `abort_session`), the ACP client spine and per-provider launch catalog exist in `apps/litecoworkd/src/agents/adapter.rs`; the supervisor drives the OpenCode turn through them with a line-resetting idle watchdog instead of the old 60 s cap, and tests `selected_adapter_conversation_*` pass. Not done: the Stop route does not yet call `interrupt` (T07), a timeout after dispatch is not yet reconciled from native session state, and no Windows or USER evidence exists.
- **Depends on:** T01, T03
- **Cross-package prerequisites:** Feature 002 OpenCode probe fix so the binding is creatable (probe currently records `compatible=false`).
- **Owner:** Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/src/agents/opencode_server.rs`, `apps/litecoworkd/src/agents/conversation_prompt.rs`, `apps/litecoworkd/src/agents/adapter.rs` (new), `apps/litecoworkd/src/agents/supervisor.rs`, adapter fixtures and tests.
- **Contract:** SPEC 7.4, 8, 10; B-02 rider, B-03, B-05; AC-005, AC-008, AC-015; AGENT-FABRIC; NATIVE-COMPOSER-LLD.
- **Change intent:** define an `AgentAdapter` trait (version probe, create session, send context, bounded event stream, interrupt, shutdown) matching the B-03 phases and implement it for OpenCode v2.0.32 by wrapping the existing helper; wire `abort_session` as the interrupt; replace the fixed 60 s deadline and 500 ms poll with named finite configuration values; correlate every SSE event to the native session and drop uncorrelated frames.
- **Acceptance:**
  - [ ] Exact session and host identity match the STARTING reservation; prompt dispatch only after activation commit.
  - [ ] Duplicate, out-of-order, malformed, oversized (`MAX_SSE_EVENT_BYTES`) and foreign-session events cannot mutate state or projection (AC-008).
  - [ ] Timeout after dispatch is reconciled through the native session state, never resent blindly.
  - [ ] Missing executable, unsupported version, auth failure or startup failure produce a typed FAILED turn with the USER message kept and no fallback (AC-015).
  - [ ] Interrupt via `abort_session` is observed and reported as a stop observation, not as settlement.
- **Command:** `cargo test -p litecoworkd --locked selected_adapter_conversation` (filter to be added by this task) and `cargo test -p litecoworkd --locked opencode`
- **Evidence:** fixture-based test output plus one recorded live OpenCode v2.0.32 run on Windows x64 in Completion evidence.
- **Stop:** if the native API cannot prove readiness or safe stop, the route is not qualified.

## 001-T05 - Complete bounded context dispatch and confirm the first adapter budget (B-07 caveat)

- **Status:** PARTIAL (window implemented; real OpenCode budget NOT confirmed) - the B-07 window now exists in two layers. `build_conversation_context_package` no longer refuses a long Conversation: it keeps the newest contiguous messages ending at the boundary within 1,024 messages and 4 MiB and records the older count in `omitted_message_count` (refusing with `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` only when the boundary message alone exceeds 4 MiB); the SQLite loader reads only the newest 1,024 committed messages. `window_conversation_context` in `conversation_prompt.rs` then keeps the package unchanged when it encodes within the native input cap, otherwise keeps the newest suffix within 80% of that cap, adds the visible `[Notice: Messages 1..N omitted due to model context window budget]` to the encoded transcript and returns a `ContextReduction` receipt (counts only), or refuses with `InputTooLarge` when the current message cannot fit. The supervisor applies it before dispatch. Budget used is LiteCowork's own 256 KiB text-input cap (`MAX_NATIVE_CONVERSATION_INPUT_BYTES`) under the 512 KiB HTTP request cap; neither was measured against what OpenCode v2.0.32 accepts. The Windows host has the pinned binary. `opencode auth list` currently completes and reports OpenCode Console and OpenRouter entries as `stored`, which does not verify credential validity or the product route; O-05 stays open until the pinned build accepts a measured input-boundary probe. The reduction receipt is returned to the supervisor but not yet persisted or shown in the UI. Pinned Workspace instructions and selected ResourceRefs are not part of the package yet (T14). Evidence: `conversation_context_dispatch_*` (4), `context_window_boundary_at_1024_and_1025_messages_omits_only_the_oldest`, `context_window_byte_bound_keeps_the_boundary_and_refuses_when_it_alone_is_too_large` (storage-core), storage-sqlite context tests (29 pass). No SQLite test seeds more than 1,024 committed messages through the loader.
- **Depends on:** T01, T03, T04
- **Cross-package prerequisites:** NONE
- **Owner:** Conversation, Agent Runtime.
- **Allowed paths:** `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, `apps/litecoworkd/src/agents/conversation_prompt.rs`, `apps/litecoworkd/src/agents/opencode_server.rs`, adjacent tests.
- **Contract:** SPEC 7.5, 8, 10, B-07; AC-006, AC-007; CONTEXT; SCHEMAS.
- **Change intent:** measure and record the OpenCode v2.0.32 accepted request size; implement the B-07 window (pinned preamble, selected refs, recent contiguous messages to 80% of the confirmed budget) with the explicit omission notice and a recorded reduction receipt; keep refusal when even the pinned part does not fit.
- **Acceptance:**
  - [ ] Only the committed same-Workspace prefix through the submitted USER message is sent; prior AGENT messages keep source binding provenance (AC-006).
  - [ ] Over-budget context is reduced only by the B-07 rule with a visible notice, or refused with a typed error; never silent truncation (AC-007).
  - [ ] Boundary tests at exactly the budget, one byte over, 1,024 and 1,025 messages.
  - [ ] Confirmed OpenCode budget value and measurement method recorded in RESEARCH.md and Completion evidence.
- **Command:** `cargo test -p litecoworkd --locked conversation_context_dispatch` (filter to be added by this task) and `cargo test -p storage-core --locked context`
- **Evidence:** test output; measured budget in Completion evidence.
- **Stop:** budget cannot be measured on the pinned version.

## 001-T06 - Wire final response settlement with stable identity in the supervisor

- **Status:** DONE (without a live native adapter) - the supervisor derives the commit request id, message id and `provider_response_id` deterministically from `(turn_id, retry_ordinal, native session/message)`, takes expected versions from the activation result instead of hard-coded 2/3, and retries only transient store failures through `retry_transient_store_commit`; every retry repeats the identical commit so a lost acknowledgement is answered from the stored receipt (T02 replay) and the prompt is never resent. Failed and cancelled turns settle through `settle_conversation_turn` and leave no AGENT message. Evidence: `conversation_final_settlement` helper test `conversation_final_settlement_retries_only_transient_store_failures`, storage `conversation_agent_response_commit_*` tests. A full supervisor run against a fake or live native agent is not tested here (no such harness in this checkout).
- **Depends on:** T02, T03, T04, T05
- **Cross-package prerequisites:** NONE
- **Owner:** Conversation, Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecoworkd/src/agents/opencode_server.rs`, adjacent tests.
- **Contract:** SPEC 7.3, B-04; AC-009, AC-026; EVENTS; STATE-MACHINES.
- **Change intent:** derive the commit request id and `provider_response_id` deterministically from turn id, retry ordinal and the native message id; on an uncertain commit outcome, re-read the stored receipt before any retry of the local commit; never resend the prompt to repair a DB write; failed/cancelled turns never promote partial deltas to a durable message.
- **Acceptance:**
  - [ ] Repeated provider final or a repeated commit attempt returns one committed message (AC-026).
  - [ ] Injected commit failure is reconciled from the stored receipt without provider resend (AC-009).
  - [ ] Failed or cancelled turns leave no AGENT message.
- **Command:** `cargo test -p litecoworkd --locked conversation_final_settlement` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** a final answer is visible as complete before commit.

## 001-T07 - Make Stop effective with observed quiescence and fix the Retry lifecycle (daemon)

- **Status:** DONE (daemon + storage; native interrupt covered by adapter fixtures, not a live agent) - Cancel on RUNNING commits only CANCEL_REQUESTED and signals the attempt through `AgentSessionSupervisor::request_cancel` (per `(turn_id, retry_ordinal)` `CancelSignal`); the OpenCode drive loop sends one `abort_session` interrupt and reports `Stopped` only on observed quiescence, otherwise `StopUnconfirmed`, and only then `settle_attempt_cancelled` commits CANCELLED and closes the session. A completion observed before the interrupt stays COMPLETED (commit accepts a CANCEL_REQUESTED turn for Completed only) and a failure stays FAILED. A repeated cancel never settles. Retry is now `FAILED -> OPEN` with `retry_ordinal + 1`, no session, an UNCLAIMED intent for the new ordinal and `conversation.turn.retried.v1` without `agent_session_id` (domain, storage, schema, EVENTS and STATE-MACHINES updated); the dispatcher then reserves a fresh STARTING session. Duplicate retry/cancel keys replay via `find_conversation_turn_command_replay`; stale If-Match is 412, non-FAILED retry is 409, retry during another in-flight turn is 409 `CONVERSATION_TURN_IN_PROGRESS`. Known limit: OpenCode v2 cannot distinguish a completion that raced the interrupt after the interrupt was sent, so that case reports Stopped; failures during a failed settlement also now end the session LOST so Retry is not blocked. Tests: `conversation_stop_and_retry_*`, `conversation_cancel_commits_only_the_request_*`, `conversation_retry_is_refused_*` (litecoworkd), `selected_adapter_conversation_stop_*` (adapter), `conversation_retry_reopens_*`, `conversation_cancel_settles_*`, `conversation_turn_settlement_*` (storage), domain-conversation tests.
- **Depends on:** T03, T04, T06
- **Cross-package prerequisites:** NONE (Feature 012 take-control waits on this task).
- **Owner:** Conversation, Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/conversation_operator_tests.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, `crates/domain-conversation/src/lib.rs`, `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/conversation_turn_tests.rs`.
- **Contract:** SPEC L-11, L-12, 7.2; AC-010, AC-011; STATE-MACHINES; API.
- **Change intent:** cancel on RUNNING commits only CANCEL_REQUESTED and signals the session supervisor (per-turn cancellation handle keyed by turn id and retry ordinal); the supervisor calls the adapter interrupt, waits for observed quiescence with a finite deadline, then settles CANCELLED, or keeps the winning COMPLETED/FAILED. Retry changes FAILED to OPEN with incremented `retry_ordinal` and no session (new domain transition), then dispatch reserves a fresh STARTING session; storage `Retry` precondition updated to match.
- **Acceptance:**
  - [ ] Cancel returns 202 with CANCEL_REQUESTED; CANCELLED is committed only after observed quiescence (AC-010).
  - [ ] Race: completion arriving during interrupt stays COMPLETED with its message; failure stays FAILED (AC-010).
  - [ ] Quiescence deadline exceeded leaves CANCEL_REQUESTED with a typed recovery state, never a fabricated CANCELLED.
  - [ ] Retry with current version and new key: fresh session, retry ordinal + 1, earlier provenance kept, no duplicate USER message (AC-011).
  - [ ] Stale If-Match returns 412; retry of a non-FAILED turn returns 409; duplicate retry key replays the receipt.
- **Command:** `cargo test -p litecoworkd --locked conversation_stop_retry` (filter to be added by this task) and `cargo test -p domain-conversation --locked`
- **Evidence:** test output; register A-25 daemon part closed in IMPLEMENTATION-STATUS.md by T22.
- **Stop:** the adapter cannot report quiescence; keep Stop marked unavailable for that route.

## 001-T08 - Implement daemon restart and orphan recovery sweep

- **Status:** DONE (restart sweep; crash windows covered with seeded rows, no live agent kill) - `agents/conversation_recovery.rs` runs from `recover_and_persist_startup_blockers` before the Operator accepts commands. It settles every UNCLAIMED or CLAIMED dispatch intent's turn FAILED with reason `RUNTIME_RESTARTED` (RUNNING, CANCEL_REQUESTED and never-claimed OPEN turns alike; no same-incarnation resume in V1, so unknown native state is never reported completed and Retry is offered), ends the AgentSession LOST, and releases any STARTING session no intent explains. A failing row raises the `CONVERSATION_SESSION_RECOVERY_FAILED` startup blocker. Legacy turns from before the v29 outbox are only reached through their STARTING session. Evidence: `conversation_session_recovery_settles_prior_incarnation_attempts_idempotently` (RUNNING, CANCEL_REQUESTED, OPEN/UNCLAIMED, a COMPLETED turn untouched, no AGENT message invented, second sweep a no-op). Not exercised: a real process kill mid-turn.
- **Depends on:** T03, T06, T07, T10
- **Cross-package prerequisites:** Feature 011 Runtime incarnation and startup-blocker reporting.
- **Owner:** Agent Runtime, Storage, Conversation.
- **Allowed paths:** `apps/litecoworkd/src/runtime.rs`, `apps/litecoworkd/src/agents/conversation_recovery.rs` (new), `apps/litecoworkd/src/runtime_recovery_tests.rs`, `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/lib.rs`.
- **Contract:** SPEC 7.2, 9; AC-012, AC-022; STATE-MACHINES; FAILURE-RECOVERY.
- **Change intent:** at startup, before Operator admission, page prior-incarnation STARTING sessions to LOST, reconcile ACTIVE/INTERRUPTING/CLOSING sessions (no same-incarnation resume in V1, so mark LOST and settle the turn FAILED with a typed reason), settle CANCEL_REQUESTED turns, and hand UNCLAIMED dispatch intents (T10) to the dispatcher or settle them FAILED. Report counts as startup recovery facts.
- **Acceptance:**
  - [ ] Prior-incarnation STARTING becomes LOST (AC-012).
  - [ ] Unknown ACTIVE state is never reported as completed; the turn shows a typed failure and Retry is offered.
  - [ ] Crash in each window (after admission, after STARTING, after activation, after final commit) recovers to a single consistent state (AC-022).
  - [ ] Running the sweep twice is idempotent; host bindings released only after LOST/CLOSED.
- **Command:** `cargo test -p litecoworkd --locked conversation_session_recovery` (filter to be added by this task)
- **Evidence:** test output; restart scenario recorded in Completion evidence.
- **Stop:** a crash window produces two dispatches or a lost USER message.

## 001-T09 - Qualify the end-to-end native Conversation route on the release matrix and gate Send

- **Status:** NOT STARTED - no `apps/litecoworkd/tests/conversation_session_supervisor.rs`; no Windows or Linux SYSTEM/USER evidence.
- **DEFERRED/BLOCKED note:** Why: needs a real OpenCode v2.0.32 binary and a native desktop run, none available in the sandbox. Unblocks when: an installed pinned OpenCode and a machine that can run the packaged app. Test later at: Windows laptop first, then native Linux; run the packaged desktop UI against a real session.
- **Depends on:** T01-T08, T10-T18
- **Cross-package prerequisites:** Feature 002 OpenCode binding creatable; Feature 011 Windows named-pipe IPC and packaged sidecar; Feature 019 release matrix rows and evidence bundle process.
- **Owner:** Product, Agent Runtime, Conversation, UI, Security.
- **Allowed paths:** `apps/litecoworkd/tests/conversation_session_supervisor.rs` (new), `apps/litecowork-ui/tests/` (new E2E driver files), `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (eligibility only), this package.
- **Contract:** SPEC 7-15; AC-001 to AC-026; G-09; docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-01.
- **Change intent:** create the canonical target with daemon + SQLite + adapter fixture; run packaged desktop E2E-01 (fresh install, Workspace, select OpenCode lead, new Conversation, send, stream, stop, retry, reload) on Windows x64 with OpenCode v2.0.32 first, then on Linux x64; Send eligibility becomes true only for the qualified tuple.
- **Acceptance:**
  - [ ] `cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture` exists and passes (AC-001 to AC-016).
  - [ ] CODE, SYSTEM and owner USER evidence on Windows x64 + OpenCode v2.0.32 covers positive, denial (foreign owner, unauthenticated pipe peer), timeout-after-dispatch, duplicate replay, cancel race, crash, restart and reconnect.
  - [ ] Same evidence on Linux x64 (0600/0700 socket) recorded as a separate row; WSL2 is not accepted for either row.
  - [ ] No prompt, handle or secret in logs, events or projections (AC-013); no fallback (AC-015).
  - [ ] An unqualified tuple keeps Send disabled with a named reason (AC-016).
- **Command:** `cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** dated evidence bundle per row (build SHA, OS build, OpenCode version, auth route, reviewer) referenced from IMPLEMENTATION-STATUS.md.
- **Stop:** any unqualified OS/agent/auth tuple stays unavailable even if another tuple passes.

## 001-T10 - Add a durable dispatch outbox for admitted turns

- **Status:** DONE - `conversation_dispatch_intents` (v29) is written in the same transaction as the turn by `create_conversation_turn`, claimed by compare-and-swap (`claim_conversation_dispatch_intent`) before the STARTING reservation in `AgentSessionSupervisor::dispatch_turn`, settled by the final-response commit and by any terminal turn transition, never deleted. Exact replay inserts no second intent and a losing dispatcher (including the replay re-spawn) returns without reserving a session. The Retry path inserting a new-ordinal intent and the startup sweep over UNCLAIMED/CLAIMED intents belong to T07 and T08.
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Conversation.
- **Allowed paths:** `docs/schemas/sqlite-v29.sql` (new, as promoted by T01), `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/lib.rs`, `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, adjacent tests.
- **Contract:** SPEC DATA-MODEL section 6; AC-022; STORAGE; FAILURE-RECOVERY.
- **Change intent:** insert one dispatch-intent row (turn id, retry ordinal, UNCLAIMED) in the same transaction as `create_conversation_turn` and as the Retry transition; the dispatcher claims it by CAS (UNCLAIMED to CLAIMED with Runtime incarnation) before `start_conversation_agent_session`; settlement marks it SETTLED. Submit and retry both enqueue instead of spawning ad hoc.
- **Acceptance:**
  - [x] Intent row commits atomically with the turn; rollback test removes both.
  - [x] Two dispatchers racing on one intent: exactly one claim wins.
  - [ ] Crash after admission and before claim leaves an UNCLAIMED intent that the T08 sweep handles (AC-022).
  - [x] Exact submit replay does not create a second intent.
- **Command:** `cargo test -p storage-sqlite --locked conversation_dispatch_intent` (filter to be added by this task)
- **Evidence:** test output; migration checksum recorded.
- **Stop:** the claim cannot be made atomic with STARTING reservation ordering.

## 001-T11 - Enforce single-flight admission in storage (B-01)

- **Status:** DONE - `create_conversation_turn` rejects a second turn while another is OPEN, RUNNING, WAITING_USER, WAITING_DEPENDENCY or CANCEL_REQUESTED with `CONVERSATION_TURN_IN_PROGRESS` after the receipt replay check; v29 triggers back it at the database level (triggers rather than a partial unique index so legacy duplicate rows still migrate). The Operator maps it to 409. Tests: `conversation_turn_creation_rejects_a_second_nonterminal_turn_and_admits_after_settlement`, `conversation_turn_creation_two_concurrent_submits_admit_exactly_one`, `conversation_turn_submit_returns_409_while_another_turn_is_in_flight_and_writes_nothing`.
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Conversation, API.
- **Allowed paths:** `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/conversation_turn_tests.rs`, `docs/schemas/sqlite-v29.sql` (shared with T10), `apps/litecoworkd/src/conversation_operator.rs` (`map_store_error`), `apps/litecoworkd/src/conversation_operator_tests.rs`.
- **Contract:** SPEC B-01; AC-023; STATE-MACHINES; API.
- **Change intent:** inside the IMMEDIATE transaction reject when any turn of the Conversation is OPEN, RUNNING, WAITING_USER, WAITING_DEPENDENCY or CANCEL_REQUESTED, backed by a partial unique index on nonterminal turns per Conversation; map to `CONVERSATION_TURN_IN_PROGRESS` 409; update `conversation_turn_creation_serializes_appends_from_the_same_stale_snapshot` to assert one admission and one rejection.
- **Acceptance:**
  - [x] Second submit during an active turn returns 409 and writes no message, turn, event or receipt (AC-023).
  - [x] Two concurrent submits from the same stale snapshot: one 202, one 409.
  - [x] Exact replay of the admitted submit still returns its receipt (receipt check runs before the single-flight check).
  - [x] Submit after the active turn settles succeeds.
- **Command:** `cargo test -p storage-sqlite --locked conversation_turn_creation` and `cargo test -p litecoworkd --locked conversation_turn_submit`
- **Evidence:** test output in Completion evidence.
- **Stop:** the index conflicts with existing turn rows during migration; repair forward, never delete user messages.

## 001-T12 - Wire Conversation-scoped UserRequest continuation (B-06)

- **Status:** BLOCKED (runtime input behavior not evidenced) - pinned v2.0.32 source defines Form create/reply/cancel routes and form lifecycle events, but LiteCowork has not captured a provider-generated `form.created` event from its active per-turn server or established which forms qualify as B-06 UserRequests. The normalizer currently has no Form event mapping, and the old fixture is not a live capture. No speculative code was added. See RESEARCH 8.1.
- **DEFERRED/BLOCKED note:** Why: source contracts prove the mechanism exists, not that the current per-turn route emits the needed input request or that its answer can be reconciled under AC-018/019. Unblocks when: an exact v2.0.32 event/request/reply capture and owner-approved mapping establish the Runtime-local request key and acceptance/rejection behavior. Test on the pinned OpenCode native Windows x64 row; WSL2 and containers never count.
- **Depends on:** T04, T07, T10
- **Cross-package prerequisites:** NONE
- **Owner:** Conversation, Agent Runtime, Trust.
- **Allowed paths:** `apps/litecoworkd/src/user_request_operator.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecoworkd/src/agents/opencode_server.rs`, `crates/storage-sqlite/src/user_request_details.rs`, `crates/domain-conversation/src/lib.rs`, adjacent tests.
- **Contract:** SPEC B-06; AC-017, AC-018, AC-019; STATE-MACHINES; TRUST.
- **Change intent:** map an OpenCode input request to a Conversation-scoped UserRequest correlated by a Runtime-local provider input key; the answer commits the immutable response and moves the turn to WAITING_DEPENDENCY; acceptance by the provider allows a fresh STARTING session; rejection or ambiguity follows AC-019. If OpenCode v2.0.32 exposes no input request, record that and keep the path unavailable for this route.
- **Acceptance:**
  - [ ] Valid answer is immutable and moves WAITING_USER to WAITING_DEPENDENCY before continuation (AC-017).
  - [ ] Acknowledgement alone is not acceptance; RUNNING only after readiness (AC-018).
  - [ ] Rejection returns to WAITING_USER with a new request; ambiguity keeps WAITING_DEPENDENCY with no new session (AC-019).
  - [ ] No Task, Attempt or Coworker is created (AC-002).
- **Command:** `cargo test -p litecoworkd --locked conversation_user_request_continuation` (filter to be added by this task)
- **Evidence:** test output; provider capability finding in RESEARCH.md.
- **Stop:** the provider cannot correlate an input key; mark unavailable for the route.

## 001-T13 - Emit bounded transient TURN_DELTA frames with resync (B-05)

- **Status:** DONE (daemon; UI reducer already handled the frames) - `apps/litecoworkd/src/transient_frames.rs` holds a fixed 256-frame broadcast ring (`transient_bus()`); `TurnFramePublisher` is attached to the OpenCode adapter as a `TurnObserver` and coalesces answer text into `turn.delta` frames (flush at 1 KiB or 100 ms, at most 8 KiB per frame, 256 KiB per attempt, then it stops), never includes reasoning, streams only the unseen suffix of the terminal text part and stops on a divergent terminal text. The supervisor publishes `turn.settled` (with `committed_message_id` only after a successful commit). The single-Conversation stream in `presentation_operator.rs` forwards only frames for its Workspace and Conversation while polling; publishing never blocks the native read loop. The Operator schema `turn_delta` has no session field, so correlation is (Workspace, Conversation, turn, retry ordinal, sequence); the UI reducer drops stale retry ordinals. The batch (2 to 20 Conversation) stream does not forward transient frames (the Desktop UI subscribes to one). Evidence: `conversation_turn_delta_*` (8 tests: coalescing, suffix-only terminal text, divergence, chunking and cap, slow subscriber lag, settled frame, fixture stream equals committed answer, route-level scoping). The Tauri bridge forwards frames generically. The crate now compiles on the connected Windows host (Feature 002 focused Windows Rust tests); this does not qualify the packaged stream UI or a live provider run.
- **Depends on:** T04, T06
- **Cross-package prerequisites:** NONE
- **Owner:** Agent Runtime, Presentation Operator, Desktop bridge, UI.
- **Allowed paths:** `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecoworkd/src/presentation_operator.rs`, `apps/litecowork-ui/src-tauri/src/conversation_stream_bridge.rs`, `apps/litecowork-ui/src/conversations/conversation-stream.ts`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/tests/conversation-stream.test.ts`.
- **Contract:** SPEC B-05, 7.6; AC-008, AC-014; PRESENTATION-RUNTIME.
- **Change intent:** publish correlated (Workspace, Conversation, turn, retry ordinal, session, sequence) coalesced deltas through a bounded in-memory channel to the Operator stream; never persist them; the UI drops the buffer and re-reads `get_conversation_presentation` on a sequence gap or reconnect; the committed AGENT message replaces the transient text.
- **Acceptance:**
  - [ ] Gap or reconnect triggers resync and no duplicated or concatenated text (AC-014).
  - [ ] Frames from a stale retry ordinal or another session are dropped (AC-008).
  - [ ] Slow consumer: channel stays bounded and the daemon never blocks the native read loop.
  - [ ] Committed message supersedes transient text on reload.
- **Command:** `cargo test -p litecoworkd --locked conversation_turn_delta` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** frames would need persistence to work; keep durable-only progress (F-04).

## 001-T14 - Composer attachments and B-08 resource materialization

- **Status:** PARTIAL (text attachments CODE-level; binary UNAVAILABLE; no Windows row; no live run) - `agents/resource_materializer.rs` reauthorizes every `resource_refs` entry and `RESOURCE` content block at dispatch (same-Workspace identity, Resource exists, pinned revision still the head, not RESTRICTED, allowlisted text, at most 128 KiB each and 16 per message, valid UTF-8) and replaces them with a delimited TEXT block `[Attached resource BEGIN id= revision= name= media_type= bytes= digest=...]` carrying the provider-verified `sha256:` digest; any refusal fails the turn before dispatch with a typed reason (`RESOURCE_*`). The same rules run at submit (`check_turn_resources` in `conversation_operator.rs`) so a bad attachment is refused with 404/409/422 while the draft and chips are still on screen. The prompt encoder still refuses unmaterialized refs. UI: `conversation-attachments.ts` (bounded, de-duplicated chips pinned to the revision, accessible remove names, refusal notices) and the "+" picker in `ConversationsPage.tsx` (lists only Workspace Resources via `list_resources`, text and size filtered, Escape closes, enabled only with a selected Conversation and no running turn; chips and draft stay on a refused send). Not done and stated honestly: binary attachments stay UNAVAILABLE (the OpenCode text transport has no verified file input, per the task Stop rule); WorkspaceRoot files are not attachable until 003-T07 ingests them, so the "canonicalized read-only path inside an admitted root" and symlink or traversal rows have no code path to test; the first message of a not-yet-created Conversation cannot carry attachments; ContextAttachment persistence and the Context Used surface are 003-T09 and 003-T12; the Windows WorkspaceRoot row is deferred. `src-tauri` is unchanged (it reuses `list_resources` and `submit_conversation_turn`). Evidence: `conversation_resource_materialization_*` (3, real SQLite store: inline header and digest, block and ref de-duplication, foreign Workspace, missing, stale revision, binary, restricted, non-UTF-8, oversized, submit-time check), `tests/conversation-attachments.test.ts` (4), updated AC-025 source test.
- **Depends on:** T05
- **Cross-package prerequisites:** Feature 003 ResourceRef authority, persisted ContextAttachment and Context Used surface; Feature 003 Windows WorkspaceRoot add for the Windows row.
- **Owner:** Conversation, Resource, Agent Runtime, UI.
- **Allowed paths:** `apps/litecoworkd/src/agents/conversation_prompt.rs`, `apps/litecoworkd/src/agents/resource_materializer.rs` (new), `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/conversation-attachments.ts` (new), `apps/litecowork-ui/tests/conversation-attachments.test.ts` (new), adjacent Rust tests.
- **Contract:** SPEC B-08, L-09; AC-006, AC-020; STORAGE; TRUST.
- **Change intent:** in Rust, reauthorize each ref at dispatch, then materialize text inline with a delimited checksummed header and binary as a canonicalized read-only path inside the admitted WorkspaceRoot passed through the adapter's file input; reject symlink escape, `..` traversal and non-admitted roots. In the UI, wire "+" to a picker that lists only Feature 003 ResourceRefs and shows chips with remove.
- **Acceptance:**
  - [ ] Attached text resource reaches the prompt with header and checksum; binary passes as an admitted path (AC-020).
  - [ ] External path, traversal, symlink escape, revoked root and changed revision are rejected before dispatch; the draft and chips stay (AC-020).
  - [ ] Picker is keyboard operable and each chip has an accessible name.
  - [ ] No attachment control renders on a route whose adapter declares no file input.
- **Command:** `cargo test -p litecoworkd --locked conversation_resource_materialization` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** the OpenCode adapter has no verified file input; keep binary attachments unavailable.

## 001-T15 - Write usage records at turn settlement (B-10)

- **Status:** DONE (OpenCode path; table was v29) - every attempt settlement writes exactly one `conversation_turn_usage_records` row inside its own transaction: the final commit (T02) and now `settle_conversation_turn` (FAILED and CANCELLED, `INSERT OR IGNORE`, `SettleConversationTurn.usage`). The supervisor attributes PROVIDER_REPORTED when the native `step-finish` frames carried non-zero `tokens.input` or `tokens.output` (summed by `UsageCollector`), else ESTIMATED_LOCAL `max(1, bytes / 4)` per direction from the encoded prompt and the answer, else UNKNOWN. A zero-token report is treated as not-provider-reported (the captured fixture reports 0/0). No cost is stored. Restart-recovery settlements record UNKNOWN. Evidence: `conversation_usage_attribution_prefers_reported_then_estimate_then_unknown`, `conversation_usage_collector_sums_reported_tokens_across_steps`, `conversation_usage_record_written_at_settlement_keeps_the_reported_attribution_once`, the UNKNOWN assertion in `conversation_turn_settlement_fails_turn_*`, existing commit tests. Whether real OpenCode v2.0.32 reports tokens in this shape is unconfirmed (fixture only).
- **Depends on:** T01, T06
- **Cross-package prerequisites:** Feature 018 owns the reader and dashboard.
- **Owner:** Conversation, Observability.
- **Allowed paths:** `docs/schemas/sqlite-v29.sql` (shared with T10), `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, adjacent tests.
- **Contract:** SPEC B-10; AC-021; OBSERVABILITY; docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md.
- **Change intent:** write one usage row in the final-response transaction (and for FAILED/CANCELLED settlements with whatever was observed): PROVIDER_REPORTED when OpenCode reports tokens, else ESTIMATED_LOCAL `max(1, utf8_bytes / 4)`, else UNKNOWN; no cost field is inferred.
- **Acceptance:**
  - [ ] Reported, estimated and unknown cases each write the right attribution (AC-021).
  - [ ] Missing telemetry never blocks or fails settlement.
  - [ ] Replay of a final commit writes no second usage row.
- **Command:** `cargo test -p storage-sqlite --locked conversation_usage_record` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** usage write would need to leave the settlement transaction.

## 001-T16 - Bind Stop to the projected active turn and real aggregate version

- **Status:** DONE (prerequisite of T17/T18, done with them; rendered DOM not exercised) - the projected `active_turn` now carries `version` and the snapshot gains `retryable_turn` (OpenAPI `ConversationTurnPresentation` and `ConversationRetryableTurn`, stream schema, generated `operator-api.ts`). `conversation-turn-state.ts` `stopTarget` binds Stop to the projected turn id and version; `cancelTurn` sends them to `cancel_conversation_turn`, shows `Stopping` for CANCEL_REQUESTED (button disabled, named `Stopping the response`), and on any failure refreshes the snapshot and shows a recoverable message. Focus returns to the composer when the fence lifts. Evidence: `conversation-turn-state.test.ts` (10 tests), storage `conversation_presentation_snapshot_is_bounded_paged_and_keeps_semantic_truth` (version, retryable reason). The Tauri Rust crate now compiles on the connected Windows host (Feature 002 focused Windows Rust tests); no React render test exists for this page.
- **Depends on:** T01, T07
- **Cross-package prerequisites:** NONE
- **Owner:** UI, Presentation Operator.
- **Allowed paths:** `apps/litecoworkd/src/presentation_operator.rs`, `apps/litecowork-ui/src/conversations/conversation-view.ts`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/conversation-turn-state.ts` (new), `apps/litecowork-ui/tests/conversation-view.test.ts`, `apps/litecowork-ui/tests/conversation-turn-state.test.ts` (new).
- **Contract:** SPEC AC-024; contracts/ui-behavior.md; NATIVE-COMPOSER-LLD.
- **Change intent:** add `turn_id`, `version` and `retry_ordinal` to the projected `active_turn`; `cancelTurn` uses them through `cancel_conversation_turn`; show "Stopping" for CANCEL_REQUESTED until the terminal state is projected; on 412 refresh the snapshot and let the owner retry Stop.
- **Acceptance:**
  - [ ] Stop on an OpenCode RUNNING turn sends the projected turn id and version (AC-024).
  - [ ] Stale version shows a recoverable message and refreshes; no silent failure.
  - [ ] Stop has an accessible name and is reachable by keyboard; focus returns to the composer after settlement.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `cargo test -p litecoworkd --locked conversation_snapshot`
- **Evidence:** test output; register A-25 UI part closed by T22.
- **Stop:** projection version cannot be read in the same snapshot as status.

## 001-T17 - Add the failed-turn Retry UI

- **Status:** DONE (logic and source-level guards; no rendered test) - the latest FAILED turn renders an inline card with a fixed safe reason (`retryReasonLabel`; unknown codes never leak raw text), says the message was kept, and a Retry button whose accessible name includes the reason. It invokes `retry_conversation_turn` with the projected `retryable_turn.version` and a fresh request id, is disabled while pending (double click sends one request), and a failure refreshes the snapshot. Retry is hidden once any turn is in flight. Evidence: `conversation-turn-state.test.ts`, `conversation-empty-state-truthfulness.test.mjs` (source guards: no hard-coded `expectedVersion: 1`, retry command present).
- **Depends on:** T07, T16
- **Cross-package prerequisites:** NONE
- **Owner:** UI.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/conversation-turn-state.ts`, `apps/litecowork-ui/tests/conversation-turn-state.test.ts`.
- **Contract:** SPEC AC-011, AC-025; contracts/ui-behavior.md.
- **Change intent:** render Retry on the latest FAILED turn with its typed reason; invoke `retry_conversation_turn` with current version and a fresh idempotency key; disable while pending; keep the original USER message visible once.
- **Acceptance:**
  - [ ] Retry appears only for a FAILED latest turn and sends the current version (AC-025).
  - [ ] Double click sends one request; stale version refreshes (AC-011).
  - [ ] Accessible name includes the failure reason.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** the daemon retry path (T07) is not green.

## 001-T18 - Fence the composer for every nonterminal state and remove inert or fabricated composer controls

- **Status:** DONE (logic and source-level guards; no rendered test) - `isSendEligible` and `isTurnOccupyingComposer` in `conversation-turn-state.ts` fence OPEN, RUNNING, WAITING_USER, WAITING_DEPENDENCY and CANCEL_REQUESTED plus a visible streamed turn and the in-flight submit (so the gap after the 202 is covered). Attachments, Voice input and the header More options are disabled with `(unavailable)` accessible names; the Turbo pill and the `@`/`/` placeholder hint were removed; the fabricated `Gemini 3.8 Flash Low` and `AI Capabilities Overview` fallbacks became `No agent selected` and `Untitled conversation`. The A-24 `disabled={!sendEligible}` guard still passes. Evidence: `conversation-turn-state.test.ts`, extended `conversation-empty-state-truthfulness.test.mjs`. `pnpm run build` was not run (api:check, tsc and tests were).
- **Depends on:** T16
- **Cross-package prerequisites:** Feature 002 descriptor-declared commands and model/effort (render only when declared); Feature 003 @-reference picker; Feature 011 app-wide keyboard and focus baseline.
- **Owner:** UI.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/conversation-turn-state.ts`, `apps/litecowork-ui/src/conversations/conversations-page.css`, `apps/litecowork-ui/tests/conversation-turn-state.test.ts`, `apps/litecowork-ui/tests/conversation-empty-state-truthfulness.test.mjs`.
- **Contract:** SPEC AC-016, AC-024, AC-025; V1-UI-SCREEN-CONTRACT; NATIVE-COMPOSER-LLD.
- **Change intent:** move `isTurnRunning`, `sendEligible` and `statusLabel` into a pure `conversation-turn-state.ts` module covering every nonterminal status; keep the local sending flag until the projected turn leaves OPEN; hide or disable-with-reason Voice Input, More options, Turbo and the placeholder hints until an owner route exists; replace the fabricated fallback labels with the real Workspace default name or "No agent selected" and the Conversation title or "Untitled conversation".
- **Acceptance:**
  - [ ] Send is fenced for OPEN, RUNNING, WAITING_USER, WAITING_DEPENDENCY and CANCEL_REQUESTED, including the gap after 202 (AC-024).
  - [ ] No control without an owner route is clickable; each disabled control has an "unavailable" accessible name (AC-025).
  - [ ] No fabricated model or title string remains (extend the truthfulness test).
  - [ ] Existing A-24 guard still passes (AC-016).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test output; register A-26 remaining part updated by T22.
- **Stop:** NONE beyond test failure.

## 001-T19 - Keep the Send disabled-until-qualified UI gate and truthful empty state

- **Status:** DONE (UI gate only) - commit 8129420: `sendEligible` / `bindingEligibleForSend` in `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` gate the `sendTurn` guard and `disabled={!sendEligible}`; the empty state is LiteCowork-branded with no unsupported claims; guarded by `apps/litecowork-ui/tests/conversation-empty-state-truthfulness.test.mjs`. This is not G-09 route qualification, which stays in T09.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** UI.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/tests/conversation-empty-state-truthfulness.test.mjs`.
- **Contract:** SPEC G-09; AC-016; register A-24, A-26 (empty-state part).
- **Change intent:** none now; regression ownership only. T09 later ties eligibility to the qualified tuple.
- **Acceptance:**
  - [x] Send is disabled without a qualified binding, draft, Operator readiness or while sending/running (AC-016).
  - [x] Empty state has no third-party branding or capability claims.
  - [ ] Guard test is part of the T23 evidence run.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** commit 8129420 and the guard test.
- **Stop:** guard test fails; revert the regressing change.

## 001-T20 - Fix the stale Operator header and extract the shared dispatch admission path

- **Status:** DONE - the module header now describes the OpenCode-only dispatch path; `agents/turn_coordinator.rs` adds `AgentTurnCoordinator::dispatch_admitted_turn`, the single entry both submit and retry use (no handler calls `dispatch_turn` or `tokio::spawn` directly). The coordinator drives the supervisor, which claims the durable intent (T10) before reserving a session; admission still commits first and the T08 sweep owns crash leftovers. The copied `eprintln!` blocks are replaced by `dispatch_diagnostic`, which emits only turn id, correlation id and a coarse error class (text before the first `:`, `{` or `(`, at most 48 safe characters). Evidence: `conversation_turn_dispatch_diagnostic_carries_ids_and_class_but_no_detail` and the 25 existing `conversation_turn*`/`conversation_stop*`/`conversation_retry*`/`conversation_cancel*` tests pass; `cargo clippy -p litecoworkd` reports no warnings in the touched modules (the crate-wide `-D warnings` run was not completed). Diagnostics are still plain `eprintln!`, not a structured logger; there is no log-capture harness.
- **Depends on:** T10
- **Cross-package prerequisites:** NONE
- **Owner:** Conversation, Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecoworkd/src/agents/turn_coordinator.rs` (new), `apps/litecoworkd/src/agents/mod.rs`, `apps/litecoworkd/src/conversation_operator_tests.rs`.
- **Contract:** SPEC 6 (ownership), 7.2, L-07; AC-001, AC-022.
- **Change intent:** correct the module header to describe the OpenCode-only dispatch path; add `AgentTurnCoordinator` that consumes claimed dispatch intents (T10) and drives `AgentSessionSupervisor`, so the Operator only admits and returns 202; replace `eprintln!` with structured redacted diagnostics carrying correlation ids only.
- **Acceptance:**
  - [ ] Operator handlers contain no direct `dispatch_turn` call.
  - [ ] Admission still commits before any native work (AC-001); the outbox path covers crash recovery (AC-022).
  - [ ] Diagnostics contain no prompt text or handle (log capture test).
- **Command:** `cargo test -p litecoworkd --locked conversation_turn` and `cargo clippy -p litecoworkd --locked -- -D warnings`
- **Evidence:** test output in Completion evidence.
- **Stop:** NONE beyond test failure.

## 001-T21 - Record the cross-layer trace for each user-visible operation

- **Status:** PARTIAL (trace recorded for Send, Stop, Retry, Reload, Activity and Attach; Answer UserRequest and native permission rows are recorded as gaps) - `contracts/ui-behavior.md` section 9 holds the rows with the exact control, Tauri command, route, fence, recovery and evidence test. Per the Stop rule the missing hops are open tasks (T12 BLOCKED, T27 PARTIAL), not documented as present. WIRING edges were not flipped to served because T09 evidence does not exist.
- **Depends on:** T07, T12-T18
- **Cross-package prerequisites:** NONE
- **Owner:** Conversation, UI, Agent Runtime.
- **Allowed paths:** `specs/001-conversation-session-supervisor/contracts/ui-behavior.md`, `specs/001-conversation-session-supervisor/WIRING.yaml`.
- **Contract:** AGENTS.md section 4; AC-001 to AC-026.
- **Change intent:** for Send, Stop, Retry, Reload, Answer UserRequest and Attach, record: user outcome, exact control in `ConversationsPage.tsx`, Tauri command (`submit_conversation_turn`, `cancel_conversation_turn`, `retry_conversation_turn`, `get_conversation_presentation`, `subscribe_conversation_stream`), Operator route, authorization, service owner, transaction/version/fence, aggregate/event/receipt, projection/stream, restart/recovery behavior and the evidence test.
- **Acceptance:**
  - [ ] Every operation has a complete row and no backend route lacks a UI path or vice versa.
  - [ ] WIRING edges are marked served only where T09 evidence exists.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** updated contracts/ui-behavior.md.
- **Stop:** a hop is missing; open the gap as a task instead of documenting it as present.

## 001-T22 - Synchronize status documents and inventories

- **Status:** PARTIAL (rows synced to current CODE evidence; closure waits on T09) - the served-route inventory was regenerated (176/1/55 at that time; current 178/1/53) and `audit_served_routes.py` matches it; `docs/IMPLEMENTATION-STATUS.md` has a 2026-11-10 Conversation status paragraph, register A-25 and A-26 carry dated updates but stay open (no live run, no rendered DOM test), and the UI action ledger attachment row states the text-only boundary. No row claims QUALIFIED. A-24 stays as previously recorded. SPEC section 4 was not rewritten because T09 evidence does not exist; the SPEC/AC statuses are unchanged.
- **Depends on:** T09, T21
- **Cross-package prerequisites:** NONE (orchestrator owns shared ledgers; this task supplies the edits for review).
- **Owner:** Conversation, Documentation owners.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `CURRENT-RUN.md`, this package.
- **Contract:** AGENTS.md sections 4-5; SPEC section 4.
- **Change intent:** regenerate the served-route inventory, update the Conversation rows (Send, Stop, Retry, attachments, usage, recovery) with exact evidence class per release row, close register A-24, A-25 and the A-26 remainder where evidence exists, and refresh SPEC section 4.
- **Acceptance:**
  - [ ] `python3 scripts/audit_served_routes.py` output matches the committed inventory.
  - [ ] No row claims QUALIFIED without the T09 bundle for that exact row.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of the status documents.
- **Stop:** evidence and claim disagree; downgrade the claim.

## 001-T23 - Capture a redacted OpenCode v2.0.32 turn fixture (P-00)

- **Status:** PARTIAL - `apps/litecoworkd/src/agents/fixtures/opencode_v2_0_32_turn.jsonl` replays deterministically and `opencode_turn_fixture_*` tests assert no handle, credential or prompt body survives. R-01 is not yet confirmed or refuted in RESEARCH.md. OpenCode v2.0.32 is installed on the Windows host. `opencode auth list` now completes with stored-provider metadata, but no authenticated live turn or input-request event was captured. Re-capture remains BLOCKED on producing a real provider turn and recording only redacted envelope keys.
- **Depends on:** T04
- **Cross-package prerequisites:** Feature 002 OpenCode probe fix so a live binding is creatable for the capture.
- **Owner:** Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/src/agents/fixtures/` (new), `specs/001-conversation-session-supervisor/RESEARCH.md`, adjacent tests.
- **Contract:** SPEC 7.4, 10; AC-008, AC-027; AGENT-FABRIC; RESEARCH pinned sources.
- **Change intent:** capture one real OpenCode v2.0.32 turn (event stream for a text answer, a tool call, a file edit, a permission ask and an error) as a committed redacted fixture, and record the exact event envelope shape in RESEARCH.md so finding R-01 (top-level `delta`/`content`/`done` versus the v2 `session.text.delta`/`session.text.ended`/`session.idle` names) is confirmed or refuted with evidence before any normalizer change.
- **Acceptance:**
  - [ ] The fixture replays through the parser deterministically with handles, tokens and prompt text redacted.
  - [ ] R-01 is confirmed or refuted in RESEARCH.md with the exact captured envelope keys (AC-027).
  - [ ] No credential, native handle or raw prompt body survives in the committed fixture (AC-008).
- **Command:** `cargo test -p litecoworkd --locked opencode_turn_fixture` (filter to be added by this task)
- **Evidence:** fixture path and RESEARCH.md envelope note in Completion evidence.
- **Stop:** the pinned OpenCode v2.0.32 server cannot be reached to capture a real turn; record the blocker rather than fabricate an envelope.

## 001-T24 - OpenCodeEventNormalizer with correlated internal events (P-01)

- **Status:** DONE (fixture-driven) - `apps/litecoworkd/src/agents/normalizer.rs` (`OpenCodeEventNormalizer`) maps events to correlated internal events, counts and drops unknown, foreign-session and uncorrelated frames, never stores reasoning in the answer, and the supervisor consumes it through `drive_turn` with an idle/heartbeat budget. Evidence: `opencode_event_normalizer_*` (11) and `normalizer::` (10) tests pass. Live-capture confirmation remains with T23.
- **Depends on:** T04, T23
- **Cross-package prerequisites:** NONE
- **Owner:** Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/src/agents/opencode_server.rs`, `apps/litecoworkd/src/agents/normalizer.rs` (new), `apps/litecoworkd/src/agents/supervisor.rs`, adjacent tests.
- **Contract:** SPEC 7.4, 8; AC-008, AC-028; AGENT-CONTROL; AGENT-FABRIC.
- **Change intent:** add an `OpenCodeEventNormalizer` beside `opencode_server.rs` that maps v2.0.32 events to the internal enum in audit section 7 (`TextDelta`, `TextEnded`, `ReasoningStarted`/`ReasoningEnded` kept separate and never stored or shown unless the harness marks it user-visible, `ToolCalled`/`Progress`/`Succeeded`/`Failed`, `PermissionAsked`, `FileChanged`, `TodoUpdated`, `StepStarted`/`StepEnded`, `UsageRecorded`, `ExecutionSucceeded`/`Failed`, `Idle`). Correlate on the session id carried in the envelope `data` or `location`, never on a top-level guess. Replace the hard 60 s read cap with an idle or heartbeat timeout. Count and drop unknown events.
- **Acceptance:**
  - [ ] Fixture tests cover each event kind from T23; unknown events are counted and dropped, never projected (AC-008).
  - [ ] Reasoning content never appears in the committed AGENT message (AC-028).
  - [ ] Correlation uses the envelope session id; an uncorrelated or foreign-session frame cannot mutate state (AC-008).
  - [ ] An idle or heartbeat timeout, not a fixed 60 s cap, bounds a quiet stream.
- **Command:** `cargo test -p litecoworkd --locked opencode_event_normalizer` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** an event cannot be correlated to a session; drop it rather than guess.

## 001-T25 - Transient turn.activity frames and REPORTED activity records with UI cards (P-02)

- **Status:** PARTIAL (CODE + SERVED; no live OpenCode run, no rendered UI test) - migration v30 stores REPORTED records (`append_conversation_turn_activity`, `list_conversation_turn_activity`). The supervisor now attaches a `TurnActivityRecorder` observer next to the delta publisher and usage collector: for tool start and settle, permission asked, file change, todo update and step start and end it first persists a REPORTED record (bounded label of safe characters only, at most 200 per attempt) and then publishes a `turn.activity` frame (`operator-stream.schema.json` `turn_activity`; a failed persist publishes nothing). Tool progress is deliberately not recorded. Read path: `GET /v1/conversations/{id}/turns/{turn_id}/activity?retry_ordinal=` (`listConversationTurnActivity`, route inventory was 176/1/55 at that time; current 178/1/53) plus Tauri command `list_conversation_turn_activity`. UI: `turn-activity.ts` (pure parse, merge, cards, plan line), `TurnActivityView` (collapsed `details`, copy "Activity reported by the agent"), reducer support for `turn.activity`, page wiring that reads stored records for the active or failed turn and merges live frames, and the activity inside the inline FAILED card. Limits, stated honestly: the plan checklist is a count line ("Plan reported by the agent: N items") because the normalizer keeps only the todo count, not item text; stored records are re-read only for the active or retryable turn, so activity of older completed turns is not rehydrated in the transcript; the Tauri command was added by following the existing bridge pattern; the Rust crate now compiles on the connected Windows host (Feature 002 focused Windows Rust tests), but no real OpenCode run produced these events here. Evidence: `turn_activity_records_publish_frames_and_persist_reported_rows_without_content`, `turn_activity_records_are_bounded_and_a_failed_persist_publishes_nothing`, `turn_activity_records_route_returns_reported_rows_scoped_to_the_turn_attempt` (litecoworkd), `turn_activity_records_are_reported_ordered_idempotent_scoped_and_immutable` (storage-sqlite), `tests/turn-activity.test.ts` (5). Original note: added by owner approval on 2026-11-09. Folds proposed task P-02 (audit C-1, C-2; findings R-04, R-07) into this LOCKED package.
- **Depends on:** T13, T24
- **Cross-package prerequisites:** NONE
- **Owner:** Agent Runtime, Conversation, UI.
- **Allowed paths:** `apps/litecoworkd/src/agents/supervisor.rs`, `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/presentation-stream.ts`, adjacent tests.
- **Contract:** SPEC 7.2, 7.3, 11; AC-029; PRESENTATION-RUNTIME; STATE-MACHINES.
- **Change intent:** emit a transient `turn.activity` frame for tool, file, permission and todo events from the normalizer (T24) using the frame shape the UI reducer already accepts, and persist them as REPORTED activity observation records linked to the turn, never as LiteCowork Effects. Render activity cards (collapsed by default), a plan checklist from `TodoUpdated`, and an inline FAILED card with the safe reason. Copy says "reported by the agent".
- **Acceptance:**
  - [ ] Activity is visible during a turn and reconstructable after restart from the stored REPORTED records (AC-029).
  - [ ] Tool and file activity are stored as REPORTED observations, never as Effect receipts; no fabricated success (AC-029).
  - [ ] A failed turn shows an inline FAILED card with a safe reason; UI copy says "reported by the agent".
- **Command:** `cargo test -p litecoworkd --locked turn_activity_records` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; screenshot or DOM assertion of an activity card in Completion evidence.
- **Stop:** an activity record would read as a LiteCowork Effect; keep it REPORTED.

## 001-T26 - Stick-to-bottom autoscroll and new-messages indicator (P-04)

- **Status:** DONE (logic and source wiring; no rendered scroll test) - `isAtBottom` (48 px threshold), `scrollFollowDecision` and `contentSignature` are pure and tested. The page follows new messages and streamed text only while the reader is at the bottom and not selecting text; otherwise it keeps the scroll position and shows a `New messages` indicator that clears on return to the bottom or on the scroll-to-bottom button. A different Conversation starts at the bottom. Evidence: `conversation-turn-state.test.ts` autoscroll tests. jsdom scroll behavior is not tested.
- **Depends on:** T13
- **Cross-package prerequisites:** NONE
- **Owner:** UI.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/conversation-turn-state.ts` (new per the optimization notes), adjacent tests.
- **Contract:** SPEC 11; AC-030; NATIVE-COMPOSER-LLD; V1-UI-SCREEN-CONTRACT.
- **Change intent:** add stick-to-bottom autoscroll that follows new frames only when the view is already at the bottom, and a "new messages" indicator when the user has scrolled up while new content arrives; keep scroll position stable while the user reads history.
- **Acceptance:**
  - [ ] Scroll stays put when the user is reading history and a new frame arrives (AC-030).
  - [ ] The view follows new content when the user is at the bottom (AC-030).
  - [ ] The "new messages" indicator appears only while scrolled up and clears on return to the bottom.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** UI test output in Completion evidence.
- **Stop:** autoscroll would steal focus from a text selection; prefer the indicator.

## 001-T27 - Surface native permission prompts inline and in Needs You (P-09)

- **Status:** PARTIAL (display only; answer channel unimplemented) - `PermissionAsked` is persisted and framed as a REPORTED PERMISSION activity (T25); UI copy accurately says the agent is asking. Pinned v2.0.32 source defines a session-scoped permission reply route that verifies session/request ownership, so “no API exists” was inaccurate. LiteCowork has not yet aligned its event normalizer/fixture to the pinned permission payload or called that exact reply route. Not done: answer/deny UI and bridge, denied response reaching the same native request, Needs You decision, unanswered-prompt timeout/default behavior, and restart/reconnect reconciliation. A native permission remains separate from a LiteCowork UserRequest, grant, or Effect. Evidence: `turn-activity.test.ts` only covers display; pinned source contract is summarized in RESEARCH 8.1, with no live request/reply evidence.
- **Depends on:** T24, T25
- **Cross-package prerequisites:** NONE for the OpenCode v2.0.32 HTTP permission route; ACP `request_permission` is a separate provider path and remains governed by Feature 002.
- **Owner:** Agent Runtime, Conversation, UI.
- **Allowed paths:** `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/needs-you/`, adjacent tests.
- **Contract:** SPEC 7.2, 8; AC-031; TRUST; AGENT-CONTROL.
- **Change intent:** when the harness emits `PermissionAsked`, surface the prompt inline in the transcript and in Needs You, and send the owner answer back through the harness permission API; this is a native permission answered through the harness, never a LiteCowork grant or an Effect. Copy must state that the agent, not LiteCowork, is asking.
- **Acceptance:**
  - [ ] A denied prompt reaches the harness and no LiteCowork grant is created (AC-031).
  - [ ] The prompt appears both inline and in Needs You with copy that says the agent is asking.
  - [ ] An unanswered prompt times out to the harness default without fabricating an answer (AC-031).
- **Command:** `cargo test -p litecoworkd --locked native_permission_prompt` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output in Completion evidence.
- **Stop:** the answer channel is not the harness; do not convert the prompt into a LiteCowork grant.

## 001-T28 - Package VERIFIED for V1

- **Status:** NOT STARTED.
- **DEFERRED/BLOCKED note:** Why: depends on T09 live matrix and release evidence. Unblocks when: T09 evidence plus Feature 019 evidence bundle tooling (done) and release rows. Test later at: packaged build on the owner's native Windows x64 laptop (release matrix row 1); WSL2 and containers never count, then native Linux.
- **Depends on:** T01-T27
- **Cross-package prerequisites:** Feature 019 evidence bundle process and release qualification matrix.
- **Owner:** Product (sign-off), Conversation, Agent Runtime, Security.
- **Allowed paths:** this package; `docs/IMPLEMENTATION-STATUS.md`.
- **Contract:** SPEC sections 12-13; AC-001 to AC-026; SPECIFICATION-STANDARD lifecycle.
- **Change intent:** assemble the evidence bundle (per-task revision, command, result, OS, harness, reviewer), move the package ACCEPTED/LOCKED to IMPLEMENTING at the first implementation merge and to VERIFIED after owner sign-off on both release rows.
- **Acceptance:**
  - [ ] Canonical command passes on Windows x64 + OpenCode v2.0.32 and on Linux x64.
  - [ ] Owner USER sign-off recorded for E2E-01 on each row, including denial and recovery.
  - [ ] SPEC and WIRING status equal VERIFIED and the validator passes.
- **Command:** `cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle path referenced from IMPLEMENTATION-STATUS.md.
- **Stop:** any row lacks USER evidence; that row stays unqualified.

## Implementation and optimization notes (Rust and TypeScript)

These are findings from reading the source, folded into the tasks above. They do not add scope.

- **Per-turn process spawn (Rust, T03):** `execute_agent_turn` calls `OpenCodeServer::spawn` and `server.stop()` for every turn, so each message pays full server startup and health checks. `AgentHostSupervisor` with host reuse (F-02) and the B-09 idle TTL removes that cost.
- **Blocking thread per turn (Rust, T03/T04):** the whole turn runs in one `spawn_blocking` call for up to 60 s while polling `next_event` every 500 ms. Prefer a dedicated reader thread feeding a bounded `tokio::sync::mpsc` channel (or an async HTTP/SSE client) so cancellation (T07) can interrupt promptly and deltas (T13) can be forwarded without blocking.
- **Hand-built events (Rust, T02/T06):** supervisor and cancel handler build many `EventDraft` JSON payloads by hand with hard-coded revisions (`entity_revision: 3`, `expected_session_version: 2`). Typed event constructors in `crates/storage-core` that derive revisions from loaded aggregates remove a class of version bugs and shrink `supervisor.rs` and `cancel_conversation_turn` (three near-identical branches).
- **Single-flight in SQL (Rust/SQLite, T11):** a partial unique index on nonterminal turns per Conversation enforces B-01 in the database itself and costs one index lookup per insert.
- **Duplicated dispatch spawn (Rust, T20):** submit and retry copy the same spawn block; the coordinator plus outbox replaces both.
- **UI state predicates (TypeScript, T16/T18):** turn-state logic lives inline in the 1,040-line `ConversationsPage.tsx`. A pure `conversation-turn-state.ts` module makes it unit-testable with the existing `node --test` runner. Keep this UI logic in TypeScript; authority (single-flight, versions, materialization, usage estimation) stays in the Rust daemon, never in the WebView.

## Dependency order

T19 is done and independent. T01 -> (T02, T03, T10, T11 in parallel) -> T04 (after T03) -> T05 -> T06 (after T02-T05) -> T07 (after T03, T04, T06) -> T08 (after T07, T10). T10 -> T20. T12 after T04, T07, T10. T13 after T04, T06. T14 after T05. T15 after T01, T06. T16 after T01, T07 -> T17 and T18. T21 after T07 and T12-T18. T09 after T01-T08 and T10-T18. T22 after T09 and T21. T23 after everything.

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T03, T08 | Feature 011 Runtime incarnation and startup-blocker foundation (Wave 0) | Recovery sweep and host ownership need the Runtime incarnation identity |
| T04, T09 | Feature 002 OpenCode probe fix (binding creatable) | The probe records `compatible=false`, so the OpenCode binding cannot be created and Send is unreachable |
| T09 | Feature 011 Windows named-pipe IPC and packaged sidecar | E2E-01 runs on the packaged desktop |
| T09, T23 | Feature 019 release matrix rows and evidence bundle | Qualification is per release row |
| T14 | Feature 003 ResourceRef authority, ContextAttachment, Context Used, Windows WorkspaceRoot add | Attachments need admitted roots and authorized refs |
| T15 | Feature 018 usage dashboard | 018 reads the usage records this task writes |
| T18 | Feature 002 descriptor commands and model/effort; Feature 003 @-reference picker; Feature 011 keyboard baseline | Composer controls render only when an owner route declares them |
| T07 | Feature 012 take-control (downstream) | 012 implementation waits on effective Stop with observed quiescence (B-34 caveat) |

Cross-package task IDs are named by outcome because the other packages' task plans are being renumbered in the same release-planning pass; the orchestrator resolves them to IDs in the traceability index.

## Completion evidence

For each task append a row here when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), harness and version (OpenCode v2.0.32), auth route, evidence class reached (CODE, SYSTEM, USER), report path and reviewer. No commit or push is implied by task completion. Statuses above are source-audit classifications at the date of this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
| T19 | 8129420 | `pnpm --dir apps/litecowork-ui test` (guard test) | recorded in docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md A-24/A-26 | not recorded | n/a | CODE (UI gate) | repository owner |
