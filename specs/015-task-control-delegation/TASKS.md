# Tasks - Task Control and Heterogeneous Delegation

**Feature:** 015 | **Status:** IN_REVIEW - authored complete for owner review; implementation not authorized until owner acceptance (T01); tasks not started unless marked.
**Spec revision:** 0.1 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second). For this package the Linux x64 row qualifies first because Task execution needs an attested Environment (B-17 rider); the Windows row stays UNAVAILABLE until Feature 005 B-35 qualifies.
**Owners:** Task Runtime, Delegation, Storage, Operator/API, Desktop UI, Security/Trust, Product.

Cross-package task references inside task fields are written as "Feature NNN Txx" so that each hyphenated cross-package ID appears once in this file (the structural validator rejects a repeated task ID); the hyphenated IDs are listed once in "Cross-package dependencies" below.

## Current baseline

Source-checked 2026-11-12 at HEAD `64c435149a82ba952c6c3bf3049e26cf2d9344b1` plus shared working-tree changes on branch `main`; highest migration is wired `docs/schemas/sqlite-v32.sql`; Feature 015 migration v43 is allocated.

- **Domain (Rust, CODE, no production caller):** `crates/domain-task/src/delegation_admission.rs` (`decide_child_attempt_admission`, `decide_child_attempt_admission_from_durable_snapshot`, `preview_child_attempt_admission`, `ChildAdmissionDenial` with 15 variants including `ProfileNotEnabledForCoworker`); `crates/domain-task/src/execution.rs` (`StepAttemptCoordinator::preview_delegated_admission`, `admit`); `crates/domain-task/src/lead_failover.rs` (`FailoverPolicy`, `decide_lead_failover`), `lead_failover_service.rs` (`LeadFailoverService::evaluate`), `lead_handoff_packet.rs` (`prepare_lead_handoff_packet`); `crates/domain-task/src/delegation_recovery.rs` (`EscalationPolicy { max_worker_attempts, fallback_profile_ids }`, no `on_exhaustion`, retained TARGET-not-yet-wired per accepted A-37); `crates/domain-task/src/delegation_result_service.rs`.
- **Served:** `POST /v1/tasks/{task_id}/lead-agent` (`owner_handoff` in `apps/litecoworkd/src/task_lead_handoff_operator.rs`) commits only the lead-binding CAS, `task.lead_agent.changed.v1` and the receipt through `TaskStore::commit_task_lead_handoff`; it creates no Attempt and fences nothing. `GET /v1/tasks/{task_id}/verification-runs` (`list_task_verification_runs` in `apps/litecoworkd/src/verification_runs_operator.rs`) is read-only; `start_verification_run` and `settle_verification_run` in `crates/storage-sqlite/src/verification_runs.rs` have no route. Delegation profile routes in `apps/litecoworkd/src/delegation_profiles_operator.rs`.
- **Storage:** Task status is a `String`; `AttemptState` is typed in `crates/storage-core/src/execution.rs`; `execution_leases.epoch` with `UNIQUE(step_id, epoch)`; `task_spec_revisions.parent_revisions_json`; `coworker_revisions.enabled_delegation_profile_ids_json` read by the SQLite child-admission guard in `crates/storage-sqlite/src/execution.rs`; `delegation_results` (`sqlite-v24.sql`) checks Workspace/parent lineage, terminal Attempt/session, Runtime/incarnation and pinned TaskSpec revision, but not live Step Attempt + lease epoch. No ResumePacket or control-request storage.
- **Errors present:** `TASK_ALREADY_PAUSED`, `TASK_PAUSE_UNSAFE`, `TASK_TERMINAL`, `DELEGATION_DEPTH_EXCEEDED`, `DELEGATION_CONCURRENCY_EXCEEDED`, `DELEGATION_PROFILE_DISABLED`, `DELEGATION_PROFILE_ARCHIVED`, `STALE_FENCE`. Absent: a wire code for the Coworker allowlist denial, budget exhaustion, sharing denial, `TASK_NOT_PAUSED`.
- **UI:** `WorkPage`, `TaskListPanel`, `TaskDetailPanel` in `apps/litecowork-ui/src/App.tsx`; `apps/litecowork-ui/src/tasks/` (`TaskSpecRevisionHistory.tsx`, `task-stream.ts`, `planning-readiness.ts`); `apps/litecowork-ui/src/presentation/TaskPresentationPanel.tsx`. No timeline, Workers view or control buttons. No Tauri bridge for handoff, verification runs, pause, resume or steer.

Gaps to V1: B-39 is accepted but whole-SPEC owner acceptance is pending; no typed Task status, no pause/resume/steer, no child-Attempt dispatch, no live lease-epoch result fence, no independent native-agent verifier, no Work-screen controls or timeline, and no CODE/SYSTEM/USER evidence on either row. O-06's semantic direction is recorded in Feature 005 revision 1.1 (IN_REVIEW): Task Runtime owns the generic Environment-first Attempt/session seam; the 2026-11-19 refinement requires a write-ahead private preparation journal and trusted idle supervisor, with the native provider/harness starting only after the exact admission receipt resolves. Feature 005 stays bounded to file replacement. Cross-owner review, Feature 015 acceptance and implementation still block worker/verifier/pause/cancel session wiring. O-07 blocks conflated lead-change/Attempt-replacement behavior. O-08 keeps A-37 producer support with its accepted Feature 004/005 dispatcher owner.

## 015-T01 - Owner acceptance of SPEC and blockers

- **Status:** BLOCKED - B-39 accepted as proposed (2026-11-10); pending owner acceptance of this SPEC revision 0.1 (owner: repository owner, sarv-projects; Delegation and Product prepare the decision).
- **DEFERRED/BLOCKED note:** Why: whole-SPEC acceptance of revision 0.1 is pending although B-39 is accepted. Unblocks when: owner reviews and accepts the SPEC. Test later at: decision only.
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 005 accepted (done 2026-11-09); Feature 005 B-35 decision is separate and does not block acceptance of this package.
- **Owner:** Product, Delegation; decision by the repository owner.
- **Allowed paths:** `specs/015-task-control-delegation/` (status flip to ACCEPTED/LOCKED per the authoring brief section 2). Register rows are written by the orchestrator.
- **Contract:** SPEC sections 5 and 17; AC-021; SPECIFICATION-STANDARD section 7.
- **Change intent:** record the owner decision on B-39 (pair, Environment sharing, V1 bounds, A-37 default); flip SPEC and WIRING status to ACCEPTED/LOCKED, bump revision to 1.0, add the owner-acceptance metadata line, rename "PROPOSED" signature headings to "Accepted target".
- **Acceptance:**
  - [ ] Owner decision on B-39 recorded with date and any rider.
  - [ ] SPEC, WIRING, README, PLAN, DATA-MODEL, RESEARCH and contracts banners updated together; validator PASS.
  - [ ] Negative: no task below starts implementation while this task is BLOCKED.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** owner decision reference in SPEC section 5; register row for B-39 closed by the orchestrator.
- **Stop:** owner rejects or changes B-39; revise SPEC and re-run review before any other task.

## 015-T02 - Promote accepted interfaces into owner docs and machine schemas

- **Status:** NOT STARTED
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 005 contract promotion (its T01) merged first so Attempt, cancel and Environment names are canonical.
- **Owner:** Task Runtime, Delegation, API, Events, Storage owners.
- **Allowed paths:** `docs/TASK-RUNTIME.md`, `docs/DELEGATION.md`, `docs/STATE-MACHINES.md`, `docs/API.md`, `docs/EVENTS.md`, `docs/STORAGE.md`, `docs/SCHEMAS.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `docs/schemas/delegation.schema.json`, `docs/schemas/sqlite-v43.sql` (allocated by `docs/LITECOWORK-BUILD-TASKS.md`; use that allocation only if still free at implementation time), this package.
- **Contract:** SPEC sections 7.2, 8; AC-002, AC-009, AC-011; register A-37.
- **Change intent:** in one reviewed change promote routes `POST /v1/tasks/{task_id}/pause`, `/resume`, `/steering`, `GET /v1/tasks/{task_id}/activity`, `/workers`; errors `DELEGATION_PROFILE_NOT_ENABLED_FOR_COWORKER`, `DELEGATION_BUDGET_EXHAUSTED`, `DELEGATION_ENVIRONMENT_SHARING_DENIED`, `TASK_NOT_PAUSED`, `HANDOFF_PRIOR_LEASE_ACTIVE`; events `task.pause.requested.v1`, `task.paused.v1`, `task.resumed.v1`, `task.steering.recorded.v1`, `task.child_attempt.admitted.v1`, `task.child_result.fenced.v1`; tables `task_control_requests` and `task_resume_packets`. Keep `escalation_policy.on_exhaustion` TARGET-not-yet-wired and optional for legacy packets until the dispatcher owner implements its producer (accepted A-37); do not relabel or rewrite the canonical schema in this task. Each new interface is labelled "accepted target interface (not yet in source)" until its implementing task lands. Resolve O-06/O-07/O-08 before promoting any conflicting generic admission, execution-replacement or exhaustion-producer contract.
- **Acceptance:**
  - [ ] Every promoted name exists once in its owner doc and machine schema; no parallel definition in this package.
  - [ ] Migration is the next immutable `sqlite-vNN.sql`; older migrations byte-identical; fingerprint test passes.
  - [ ] Negative: schema validation rejects a pause body without `expected_task_version` and a steering body over 16 KiB.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py` and `cargo test -p storage-sqlite --locked schema`
- **Evidence:** PR link and command output in Completion evidence.
- **Stop:** a promoted name conflicts with an existing owner definition; resolve at the owner.

## 015-T03 - Typed TaskStatus enum and transition table (Rust)

- **Status:** NOT STARTED - Task status is a `String` in `TaskRecord`/`TaskView`; transitions are checked ad hoc (for example the `"READY" | "RUNNING"` safe boundary in `lead_failover_service.rs`).
- **Depends on:** T01, T02
- **Cross-package prerequisites:** coordinate with Feature 005 (its Attempt admission task) so both use the same enum.
- **Owner:** Task Runtime, Storage.
- **Allowed paths:** `crates/storage-core/src/task_status.rs` (new), `crates/storage-core/src/lib.rs`, `crates/domain-task/src/lib.rs`, `crates/domain-task/src/lead_failover_service.rs`, `crates/storage-sqlite/src/lib.rs` (status parse/serialize only), adjacent tests.
- **Contract:** SPEC section 7.2; STATE-MACHINES "Task"; AC-002, AC-003, AC-013.
- **Change intent:** add `TaskStatus` (13 states, SCREAMING_SNAKE_CASE serde) and `task_transition(from, &TaskControlCommand)`, generated as an exhaustive match so a missing transition is a compile error; replace string comparisons in domain-task with the enum; keep the stored column text-compatible (no migration). Reconcile its complete status/transition list against the current canonical Task state machine before coding; this target enum is not a second authority.
- **Acceptance:**
  - [ ] Table test covers every (state, command) pair against STATE-MACHINES, including PAUSED to CANCEL_REQUESTED and no VERIFYING to RUNNING.
  - [ ] Unknown stored status fails closed with a typed error, never defaults.
  - [ ] Existing lead-failover tests still pass.
- **Command:** `cargo test -p storage-core --locked task_status` and `cargo test -p domain-task --locked` (filter `task_status` to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** the stored data contains a status not in STATE-MACHINES; fix at the owner first.

## 015-T04 - Pause and resume: storage, domain and quiescence

- **Status:** NOT STARTED - no `request_pause`/`resume` in source; error codes exist.
- **Depends on:** T01, T03
- **Cross-package prerequisites:** O-06 owner-directed generic Task Runtime Attempt/session execution-control seam from Feature 005 revision 1.1; cross-owner review and Feature 015 acceptance are pending. Feature 005 T08 specifies `admit_file_write` only for one bounded file-write Effect and T11 cancels/quiesces that path. The generic provider-session interface and lifecycle must be reviewed and implemented before this task.
- **Owner:** Task Runtime, Storage.
- **Allowed paths:** `apps/litecoworkd/src/task_control_service.rs` (new), `crates/domain-task/src/task_control.rs` (new), `crates/domain-task/src/lib.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/task_control.rs` (new), `crates/storage-sqlite/src/task_control_tests.rs` (new).
- **Contract:** SPEC L-01, L-02, sections 7.2, 11; TASK-RUNTIME "Pause and resume", "ResumePacket"; AC-001, AC-002, AC-003.
- **Change intent:** `TaskControlService::request_pause` commits PAUSE_REQUESTED plus a `task_control_requests` row and receipt; outside SQLite it asks each active Attempt through the O-06 owner-approved execution-control interface to stop at a safe boundary, waits for observed quiescence, builds the ResumePacket (fields per TASK-RUNTIME, no grants or leases), stores it in `task_resume_packets` with a digest, releases leases and commits PAUSED. Feature 005's `admit_file_write` signature alone does not provide generic native-session admission/control. Resume normally marks interrupted Attempts ABANDONED and continues with a fresh Attempt and higher epoch; TASK-RUNTIME's narrow same-Attempt INPUT_REQUIRED exception applies only under its full same-incarnation/same-Environment and reconciliation conditions. A response arriving during PAUSE_REQUESTED returns `CONFLICT`.
- **Acceptance:**
  - [ ] Pause from each allowed state reaches PAUSED only after quiescence and lease release (AC-001).
  - [ ] PAUSED, terminal, unsafe and stale inputs are rejected with the mapped code and no write; exact replay returns the stored receipt; same key with a different body is `CONFLICT` (AC-002).
  - [ ] Resume restores the prior state; a later admission uses epoch + 1; resume on a non-PAUSED Task is `TASK_NOT_PAUSED` (AC-003).
  - [ ] Fault injection after each insert rolls back the whole transaction.
- **Command:** `cargo test -p storage-sqlite --locked task_control` and `cargo test -p litecoworkd --locked task_control_pause` (filters to be added by this task)
- **Evidence:** test names and output in Completion evidence; IMPLEMENTATION-STATUS Task control row.
- **Stop:** any path reports PAUSED before quiescence is observed.

## 015-T05 - Typed steering with sibling spec revisions

- **Status:** NOT STARTED - `revise_saved_spec` in `crates/domain-task/src/lib.rs` revises a saved spec but has no steering class and is not admitted while the Task runs.
- **Depends on:** T01, T03
- **Cross-package prerequisites:** NONE
- **Owner:** Task Runtime, Storage.
- **Allowed paths:** `crates/domain-task/src/task_control.rs`, `crates/domain-task/src/lib.rs`, `crates/storage-sqlite/src/task_control.rs`, `crates/storage-sqlite/src/lib.rs` (spec revision insert only), `apps/litecoworkd/src/task_control_service.rs`, adjacent tests.
- **Contract:** SPEC L-04, section 7.2 `SteerTaskRequest`; TASK-RUNTIME "Steering"; AC-004.
- **Change intent:** `steer(SteerTaskRequest)`: CHAT_ONLY records a steering note for the lead; TASK_SPEC_CHANGE inserts `TaskSpecRevision(n+1)` with `parent_revisions_json = [n]`; a concurrent change from the same base becomes a sibling; PLAN_HINT is delivered to the lead's next planning boundary; URGENT_INTERRUPT asks the active Attempt to stop at the next safe boundary; CANCEL delegates to T06. Running Attempts stay pinned; children receive only relevant changes or finish against their recorded revision with an applicability review flag.
- **Acceptance:**
  - [ ] Spec change creates n+1 and leaves the running Attempt's pinned revision unchanged (AC-004).
  - [ ] Two racing changes from revision n produce two siblings; neither is lost (AC-004).
  - [ ] Body over 16 KiB or unknown class is rejected; steering text never appears in logs.
- **Command:** `cargo test -p storage-sqlite --locked task_steering` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** any implementation overwrites a revision.

## 015-T06 - Task-level cancel fan-out to child Attempts

- **Status:** NOT STARTED - Feature 005 plans a single-Attempt cancel; no fan-out exists.
- **Depends on:** T01, T03, T04
- **Cross-package prerequisites:** Feature 005 T10 (`cancel_task` route and bridge) and Feature 005 T11 (cancel quiescence and Effect reconciliation).
- **Owner:** Task Runtime.
- **Allowed paths:** `apps/litecoworkd/src/task_control_service.rs`, `crates/domain-task/src/task_control.rs`, `crates/storage-sqlite/src/task_control.rs`, `crates/storage-sqlite/src/execution.rs` (child Attempt listing only), adjacent tests.
- **Contract:** SPEC L-03, section 11; TASK-RUNTIME "Cancellation"; AC-005, AC-013.
- **Change intent:** extend the Feature 005 cancel path: on CANCEL_REQUESTED fence completion and new VerificationRuns, list every nonterminal child Attempt by `parent_attempt_id`, request cancel for each through the Feature 005 service, reconcile open Effects, and commit CANCELLED only when all are quiescent or settled. Cancel supersedes PAUSE_REQUESTED and is valid from PAUSED.
- **Acceptance:**
  - [ ] Cancel with two active children settles CANCELLED only after both are quiescent (AC-005).
  - [ ] Cancel during PAUSE_REQUESTED supersedes the pause; pause request outcome recorded as superseded (AC-005).
  - [ ] Pause and cancel racing: one CAS wins, the other gets `STALE_TASK_VERSION` (AC-013).
- **Command:** `cargo test -p litecoworkd --locked task_control_cancel_fanout` (filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** a child is left RUNNING after the Task reports CANCELLED.

## 015-T07 - Execution replacement admission and late-result fencing (separate from lead binding change)

- **Status:** PARTIAL with blocked target - `POST /v1/tasks/{task_id}/lead-agent` and `TaskLeadHandoffService::execute` commit only the lead-binding change (`apps/litecoworkd/src/task_lead_handoff_operator.rs`, `crates/domain-task/src/lead_handoff.rs`, safe-boundary checks in `crates/storage-sqlite/src/lead_handoff_boundary.rs`); `prepare_lead_handoff_packet` exists. Missing: separately defined execution-replacement command/admission, prior-lease wait, and live lease-epoch fence. O-06/O-07 block implementation of that target; binding change must not create an Attempt.
- **Depends on:** T01, T03, T04
- **Cross-package prerequisites:** O-06 generic Attempt/session admission interface and O-07 separate execution-replacement command; Feature 005 T08 supplies only the bounded file-write admission slice unless explicitly extended by its owner.
- **Owner:** Task Runtime, Delegation, Storage.
- **Allowed paths:** `apps/litecoworkd/src/lead_handoff_attempt.rs` (new), `apps/litecoworkd/src/task_lead_handoff_operator.rs`, `crates/domain-task/src/lead_handoff.rs`, `crates/domain-task/src/lead_handoff_packet.rs`, `crates/domain-task/src/delegation_result_service.rs`, `crates/storage-sqlite/src/delegation_results.rs`, `crates/storage-sqlite/src/execution.rs`, adjacent tests.
- **Contract:** SPEC L-05, L-13, section 7.2; release contract section 6 item 3; TASK-RUNTIME "Lead-agent change"; AC-006, AC-007.
- **Change intent:** never chain Attempt replacement from `commit_task_lead_handoff`/`change_lead_agent`. Only after O-07 defines a distinct execution-replacement operation and O-06 provides its admission/session contract may the replacement path wait until the prior lease is released, expired or revoked and open Effects settle (otherwise `HANDOFF_PRIOR_LEASE_ACTIVE` with retry-after), then admit a new Attempt with a higher epoch from a bounded handoff projection (no grants or handles). In `DelegationResultService` and the Effect-receipt path, compare source Attempt id and live lease epoch with current Step/lease, plus `revision_at_start`; on mismatch write a fenced observation and return `STALE_FENCE`.
- **Acceptance:**
  - [ ] Binding change alone creates no Attempt and existing Attempts retain their binding and lease; a separately accepted execution-replacement operation creates a new Attempt id and higher epoch after the prior lease and Effects settle, with no grant, Approval, SecretLease or handle copied (AC-006).
  - [ ] A ResultEnvelope or Effect receipt from the old Attempt after replacement is fenced with no Task, Step, Effect or Artifact change (AC-007).
  - [ ] Two handoffs racing: one wins (AC-013).
- **Command:** `cargo test -p litecoworkd --locked lead_handoff_attempt` and `cargo test -p domain-task --locked delegation_result` (filters to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** any late result mutates state.

## 015-T08 - Child-Attempt admission service with allowlist and B-39 Environment rule

- **Status:** PARTIAL - decision logic and SQLite guard exist (`crates/domain-task/src/delegation_admission.rs`, `StepAttemptCoordinator::preview_delegated_admission` and `admit` in `crates/domain-task/src/execution.rs`, guard in `crates/storage-sqlite/src/execution.rs`); no production caller in `apps/litecoworkd`, no wire code for `ProfileNotEnabledForCoworker`, no worker dispatch.
- **Depends on:** T01, T02, T03
- **Cross-package prerequisites:** O-06 owner-directed generic Task Runtime Attempt/session admission and execution-control seam (revision 1.1 IN_REVIEW; cross-owner review pending); Feature 005 T04 (Linux bubblewrap provider); Feature 004 T05 (Claude Code dispatch arm); Feature 002 T12 (Claude Code AgentModule); Feature 009 Coworker revision with `enabled_delegation_profile_ids` editable.
- **Owner:** Delegation, Task Runtime, Storage.
- **Allowed paths:** `apps/litecoworkd/src/delegation_admission_service.rs` (new), `apps/litecoworkd/src/main.rs` (service wiring only), `crates/domain-task/src/delegation_admission.rs`, `crates/domain-task/src/execution.rs`, `crates/domain-task/src/task_packet_preparation.rs`, `crates/storage-sqlite/src/execution.rs`, adjacent tests.
- **Contract:** SPEC L-06 to L-08, B-39, sections 7.1, 7.2, 10; DELEGATION "Depth and concurrency"; AC-008, AC-009, AC-019.
- **Change intent:** `DelegationAdmissionService::admit_child` validates the delegate request came from the current parent session and lease (`ParentSessionProof`), loads the durable snapshot, calls `decide_child_attempt_admission_from_durable_snapshot`, and enforces B-39 bounds (depth 1, concurrency 2) and exact `ATTEMPT_PRIVATE` ownership. Following Feature 005 A-70, it durably journals the private preparation identity, starts a trusted idle supervisor in the fresh Environment and attests the exact held scope before storage; no native child harness, tools, credentials or user code start before admission. One transaction then commits Environment metadata, Attempt, Step binding, lease, budget reservation, `STARTING` AgentSession and `task.child_attempt.admitted.v1`. After the exact receipt resolves, a one-time private handoff starts the worker in that same scope. An uncertain transaction result is resolved by the original receipt before launch or cleanup; an unbound scope is destroyed only after exact provider identity and quiescence are observed. Preparation failure leaves no admitted child. No Trust grant, ApprovalUse, invocation intent or file-write Effect is part of generic Attempt admission. Each consequential file mutation later crosses Feature 005's separate mediated Effect/Invocation dispatch boundary. Map each `ChildAdmissionDenial` variant to a wire code in one exhaustive function.
- **Acceptance:**
  - [ ] Runtime writes a private preparation intent before provider creation; only a trusted idle supervisor probes the exact held scope before the transaction, and no native harness/tools/credentials/user code start before admission.
  - [ ] An allowed request commits Environment metadata, Attempt, Step binding, lease, budget reservation, STARTING AgentSession and event atomically; only a resolved receipt permits one-time native provider launch in the same held scope (AC-008).
  - [ ] Crash before admission cleans an unbound scope only after exact provider identity and positive quiescence; crash after admission resolves the receipt and reconciles the old incarnation/scope before any replacement; unknown evidence blocks.
  - [ ] One negative test per `ChildAdmissionDenial` variant, each with no partial write (AC-009).
  - [ ] Non-current session, grant refs beyond the parent's, foreign Workspace and shared writable scope are denied; logs show ids and class only (AC-019).
  - [ ] Two admissions at the concurrency limit: one wins, the other gets `DELEGATION_CONCURRENCY_EXCEEDED` (AC-013).
- **Command:** `cargo test -p domain-task --locked delegation_admission` and `cargo test -p litecoworkd --locked delegation_admission_service` (second filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** any path starts a worker before the admission commit.

## 015-T09 - Budgets and escalation with on_exhaustion (A-37)

- **Status:** PARTIAL - `EscalationPolicy` and `EscalationAttemptOutcome` exist in `crates/domain-task/src/delegation_recovery.rs`; `delegation_budget_policy_json` is read by the SQLite guard; no `on_exhaustion`, no threshold action executor.
- **Depends on:** T01, T02, T08
- **Cross-package prerequisites:** O-08 dispatcher-owner contract for any `on_exhaustion` producer behavior; Feature 020 Needs You producer only for NEEDS_YOU delivery.
- **Owner:** Delegation.
- **Allowed paths:** `crates/domain-task/src/delegation_recovery.rs`, `crates/domain-task/src/delegation_envelopes.rs`, `apps/litecoworkd/src/delegation_admission_service.rs`, `crates/storage-sqlite/src/delegation_profiles.rs`, `crates/storage-sqlite/src/execution.rs`, adjacent tests.
- **Contract:** SPEC L-10, B-39 item 4, section 12; DELEGATION "Verification and escalation", "Cost, budgets, and performance projections"; AC-010, AC-011.
- **Change intent:** implement Feature 015-owned budget threshold evaluation and settled-retry rules. Keep `on_exhaustion` TARGET-not-yet-wired, schema-ahead-of-code and optional on legacy packets as accepted by A-37 until the Feature 004/005 dispatcher owner defines and implements its producer. Do not add the field or claim producer support in this task without that owner decision. Once the owner interface exists, the dispatcher may route exhausted outcomes to the typed lead blocker or Feature 020 Needs You item. Budget evaluator resolves the strictest action across Attempt, Task and profile scopes.
- **Acceptance:**
  - [ ] Exhaustion with RETURN_TO_LEAD and with NEEDS_YOU each produce the right outcome; a legacy packet without the field reads as NEEDS_YOU (AC-011).
  - [ ] STOP_NEW_DELEGATION refuses new children with `DELEGATION_BUDGET_EXHAUSTED`; out-of-range profile values are rejected, never clamped (AC-010).
- **Command:** `cargo test -p domain-task --locked delegation_recovery`
- **Evidence:** test output in Completion evidence.
- **Stop:** a replacement worker starts while the prior Attempt's Effect is AMBIGUOUS.

## 015-T10 - Independent-agent verification

- **Status:** PARTIAL - `start_verification_run`/`settle_verification_run` in `crates/storage-sqlite/src/verification_runs.rs` and `VerificationRunStatus` in `crates/domain-effects/src/lib.rs` exist; no producer and no independence rule.
- **Depends on:** T01, T08
- **Cross-package prerequisites:** O-06 generic Attempt/session admission contract; Feature 005 T09 covers Effect reconciliation and an independent file verifier for its bounded file-write slice, not the independent native-agent verifier defined here.
- **Owner:** Task Runtime, Delegation.
- **Allowed paths:** `apps/litecoworkd/src/independent_verification_service.rs` (new), `crates/storage-sqlite/src/verification_runs.rs`, `crates/storage-core/src/verification_run.rs`, `crates/domain-effects/src/lib.rs`, adjacent tests.
- **Contract:** SPEC L-09, B-39 item 1; STATE-MACHINES "VerificationRun"; AC-012.
- **Change intent:** `verify_child_result` admits a verifier Attempt through the O-06 owner-approved seam, with an AgentBinding different from the producing worker's (checked in storage, not only in the service) and a read-only Environment pinned to the produced revision; settles PASSED, FAILED or INCONCLUSIVE (timeout); refuses to start during PAUSE_REQUESTED or CANCEL_REQUESTED; never moves the Task to COMPLETED.
- **Acceptance:**
  - [ ] Verifier on the producing worker's binding is rejected in storage (AC-012).
  - [ ] Timeout yields INCONCLUSIVE; a PASS leaves the Task non-terminal (AC-012).
  - [ ] No run starts while PAUSE_REQUESTED or CANCEL_REQUESTED.
- **Command:** `cargo test -p storage-sqlite --locked verification_run` and `cargo test -p litecoworkd --locked independent_verification` (second filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** the verifier shares a writable handle with the producer.

## 015-T11 - Operator routes, authorization and Tauri bridge commands

- **Status:** PARTIAL - `POST /v1/tasks/{task_id}/lead-agent` and `GET /v1/tasks/{task_id}/verification-runs` are served behind `authenticate`; no pause, resume, steering, activity or workers route; no Tauri bridge for any of them.
- **Depends on:** T01, T02, T04-T10
- **Cross-package prerequisites:** Feature 011 IPC profile (Windows named pipe and Linux socket) and Feature 005 T10 (`cancel_task` route).
- **Owner:** Operator/API, Desktop shell.
- **Allowed paths:** `apps/litecoworkd/src/operator.rs` (route merge only), `apps/litecoworkd/src/task_control_operator.rs` (new), `apps/litecoworkd/src/task_lead_handoff_operator.rs`, `apps/litecoworkd/src/verification_runs_operator.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs` (command registration), `apps/litecowork-ui/src-tauri/src/task_control_bridge.rs` (new), `apps/litecowork-ui/src/api/operator-api.ts` (generated), `docs/schemas/served-route-inventory.json` (regenerated).
- **Contract:** SPEC section 7.2; API; LOCAL-OPERATOR-IPC; AC-002, AC-013, AC-019.
- **Change intent:** mount pause, resume, steering, activity and workers routes with Workspace-owner checks, If-Match, Idempotency-Key, body limits (8 KiB control, 16 KiB steering), `Cache-Control: no-store`; include `allowed_commands` in the Task projection from the daemon's transition table. Add Tauri commands `pause_task`, `resume_task`, `steer_task`, `change_task_lead`, `list_task_activity`, `list_task_workers`, `list_task_verification_runs` that validate identifiers and never pass host paths.
- **Acceptance:**
  - [ ] Each route: unauthenticated 401, foreign Workspace 404, stale If-Match 409, replay returns stored receipt (AC-002, AC-019).
  - [ ] Racing commands through the route: one wins (AC-013).
  - [ ] Served-route inventory regenerated and matches the OpenAPI.
- **Command:** `cargo test -p litecoworkd --locked task_control_operator` (filter to be added by this task) and `cargo test -p litecowork-ui --locked task_control_bridge` (filter to be added by this task) and `python3 scripts/audit_served_routes.py`
- **Evidence:** test output; route inventory diff in the PR.
- **Stop:** a route admits a command the transition table forbids.

## 015-T12 - Work-screen controls: Pause, Resume, Steer, Cancel, Change lead

- **Status:** NOT STARTED - `TaskDetailPanel` in `apps/litecowork-ui/src/App.tsx` has no execution controls.
- **Depends on:** T01, T11
- **Cross-package prerequisites:** Feature 005 T13 (`TaskExecutionPanel.tsx` and `task-execution-state.ts`); Feature 011 app-wide keyboard and focus baseline.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/tasks/TaskControls.tsx` (new), `apps/litecowork-ui/src/tasks/task-control-state.ts` (new), `apps/litecowork-ui/src/tasks/TaskExecutionPanel.tsx`, `apps/litecowork-ui/src/tasks/task-execution-state.ts`, `apps/litecowork-ui/src/App.tsx` (`TaskDetailPanel` mount only), `apps/litecowork-ui/tests/task-control-state.test.ts` (new).
- **Contract:** [contracts/ui-behavior.md](contracts/ui-behavior.md); V1-UI-SCREEN-CONTRACT section 3 "Controls"; AC-016.
- **Change intent:** a toolbar on Task detail with Pause, Resume, Steer (dialog with class selector and body), Cancel (confirm dialog naming active children) and Change lead (binding picker limited to eligible bindings). Enabled state comes only from `allowed_commands`; pending shows "Pausing", "Cancelling"; 409 refetches and announces the change in an `aria-live="polite"` region. Pure state logic in `task-control-state.ts`.
- **Acceptance:**
  - [ ] Disabled controls expose an accessible reason; all reachable by Tab with a visible focus ring (AC-016).
  - [ ] 409 refetch announces "This Task changed; showing the latest state." (AC-016).
  - [ ] Offline and denied states render with repair text, no optimistic state change.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test output; screenshot set in the evidence bundle.
- **Stop:** the UI computes allowed transitions itself.

## 015-T13 - Activity timeline and Workers view

- **Status:** NOT STARTED - no timeline component; V1-UI-SCREEN-CONTRACT section 3 records the gap.
- **Depends on:** T01, T11
- **Cross-package prerequisites:** Feature 005 T13 (Task execution panel).
- **Owner:** Desktop UI, Presentation Operator.
- **Allowed paths:** `apps/litecoworkd/src/task_control_operator.rs` (activity and workers projections), `apps/litecowork-ui/src/tasks/TaskActivityTimeline.tsx` (new), `apps/litecowork-ui/src/tasks/TaskWorkersView.tsx` (new), `apps/litecowork-ui/src/tasks/task-activity.ts` (new), `apps/litecowork-ui/src/tasks/task-stream.ts`, `apps/litecowork-ui/tests/task-activity.test.ts` (new).
- **Contract:** contracts/ui-behavior.md; V1-UI-SCREEN-CONTRACT section 3 "Activity", "Workers", "Mission Control"; AC-015, AC-018, AC-020.
- **Change intent:** Activity renders bounded pages (cursor, limit 1..200) of owner events with actor, Attempt, lease epoch, timestamps and correlation; Workers lists lead, children and verifiers with AgentModule and version, DelegationProfile revision, Environment class, budget used, outcome and verification. Native sub-agent reports render as "Reported by the agent (not managed by LiteCowork)". Live updates through `task-stream.ts` with resync on gap. Use list virtualization only if AC-020 fails without it.
- **Acceptance:**
  - [ ] Only owner-recorded states render; no percent, ETA or invented presence (AC-015).
  - [ ] Native sub-agent activity labelled REPORTED and not listed as a child Attempt (AC-018).
  - [ ] 200-entry page renders with no main-thread task over 100 ms in the harness (AC-020).
  - [ ] Loading, empty, error, denied, offline, stale and recovery states each have a test.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `cargo test -p litecoworkd --locked task_activity_projection` (filter to be added by this task)
- **Evidence:** test output; performance trace in the evidence bundle.
- **Stop:** the projection needs data the owner does not record; fix the owner first.

## 015-T14 - Windows row: delegation and execution controls UNAVAILABLE until B-35

- **Status:** NOT STARTED
- **Depends on:** T01, T11
- **Cross-package prerequisites:** Feature 005 T14 (Windows Task execution UNAVAILABLE reason).
- **Owner:** Task Runtime, Desktop UI.
- **Allowed paths:** `apps/litecoworkd/src/task_control_operator.rs`, `apps/litecoworkd/src/delegation_admission_service.rs`, `apps/litecowork-ui/src/tasks/task-control-state.ts`, `apps/litecowork-ui/tests/task-control-state.test.ts`.
- **Contract:** SPEC section 12; Feature 005 B-17 rider and B-35; AC-017.
- **Change intent:** on Windows, the Task projection omits child admission, handoff Attempt, resume-with-continuation and verification from `allowed_commands` with the typed reason from Feature 005; pause and cancel of a Task without active Attempts remain. No text calls a Job Object a sandbox.
- **Acceptance:**
  - [ ] Windows projection reports the four operations UNAVAILABLE with the typed reason (AC-017).
  - [ ] Pause and cancel of a READY Task still succeed on Windows (AC-017).
- **Command:** `cargo test -p litecoworkd --locked task_control_windows_unavailable` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** Windows-row screenshot and projection JSON in the evidence bundle.
- **Stop:** any Windows path could admit a child Attempt.

## 015-T15 - Failure and recovery suite

- **Status:** NOT STARTED
- **Depends on:** T01, T04-T10
- **Cross-package prerequisites:** Feature 005 T11 (startup sweep for Attempts and Effects).
- **Owner:** Task Runtime, Storage.
- **Allowed paths:** `apps/litecoworkd/src/task_control_service.rs`, `apps/litecoworkd/src/runtime.rs` (startup sweep hook only), `apps/litecoworkd/tests/task_control_delegation.rs` (new), `crates/storage-sqlite/src/task_control_tests.rs`.
- **Contract:** SPEC section 11; contracts/errors-recovery.md; FAILURE-RECOVERY; AC-007, AC-013, AC-014.
- **Change intent:** startup sweep resumes `task_control_requests` that are not settled: new incarnation never renews prior leases, prior Attempts become ABANDONED, Effects reconcile before any replacement, then the pause or cancel completes from its durable record. Inject crash at each step of pause, cancel fan-out, handoff and child admission; inject timeout after worker dispatch; duplicate replay of every command; cancel racing a child completion.
- **Acceptance:**
  - [ ] Each crash point leaves a recoverable durable state and never reports PAUSED or CANCELLED without quiescence (AC-014).
  - [ ] Duplicate replay returns stored receipts; late results after restart are fenced (AC-007).
  - [ ] Cancellation racing a completing child: the earlier committed outcome wins (AC-013).
- **Command:** `cargo test -p litecoworkd --test task_control_delegation --locked recovery` (target and filter to be added by this task)
- **Evidence:** test output in Completion evidence.
- **Stop:** any UNKNOWN outcome is treated as success.

## 015-T16 - Cross-layer traces for every user-visible operation

- **Status:** NOT STARTED
- **Depends on:** T01, T11-T15
- **Cross-package prerequisites:** NONE
- **Owner:** Product, each component owner.
- **Allowed paths:** `specs/015-task-control-delegation/contracts/ui-behavior.md`, `specs/015-task-control-delegation/WIRING.yaml`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`.
- **Contract:** AGENTS.md section 4; AC-001 to AC-021.
- **Change intent:** for Pause, Resume, Steer, Cancel, Change lead, delegated Step, verification and timeline view record: user outcome, screen and control, Tauri command, route, authorization, service, transaction and fence, event and receipt, projection, restart path and evidence test name.
- **Acceptance:**
  - [ ] Every control has a route and every route has a control or is marked daemon-internal (child admission).
  - [ ] Each trace cites a passing test.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/audit_v1_contracts.py`
- **Evidence:** trace table in contracts/ui-behavior.md.
- **Stop:** a control has no owner route.

## 015-T17 - Linux-row qualification of the B-39 pair (CODE, SYSTEM, USER)

- **Status:** NOT STARTED
- **Depends on:** T01-T16
- **Cross-package prerequisites:** Feature 005 T15 (Linux E2E-04 qualified); Feature 019 release matrix rows and evidence bundle process; Feature 011 Linux socket profile qualified.
- **Owner:** Product, Release, repository owner (USER evidence).
- **Allowed paths:** `apps/litecoworkd/tests/task_control_delegation.rs`, `apps/litecowork-ui/tests/` (E2E driver files for this journey), evidence bundle path defined by Feature 019, `CURRENT-RUN.md`.
- **Contract:** SPEC section 15; ACCEPTANCE-AND-E2E-PLAN (new gate-E delegation row, id assigned by the orchestrator, extends E2E-04); AC-020, AC-021.
- **Change intent:** create the canonical test target `task_control_delegation`; run accepted E2E-15 on Linux x86_64 with OpenCode v2.0.32 lead and the pinned Claude Code CLI worker in this order: running Task, steer, pause, resume, delegate one Step, independent verification, cancel fan-out. Include denial (allowlist miss, sharing request), crash and restart, late-result fence and performance measurements. Test lead-binding change and any separately scoped execution replacement only after O-07 resolves them; do not make them a hidden part of the lead-binding route.
- **Acceptance:**
  - [ ] Canonical command passes on the Linux row with recorded build, OS, harness versions and auth routes (AC-021).
  - [ ] AC-020 measurements recorded on the release-row machine.
  - [ ] Owner USER evidence signed for the exact build.
- **Command:** `cargo test -p litecoworkd --test task_control_delegation --locked -- --nocapture`
- **Evidence:** evidence bundle entry and IMPLEMENTATION-STATUS row with class QUALIFIED for the Linux row only.
- **Stop:** any denial or recovery case fails; do not claim QUALIFIED.

## 015-T18 - Windows-row qualification after B-35

- **Status:** BLOCKED - Feature 005 B-35 is PROPOSED (owner: repository owner) and no Windows Environment provider exists.
- **DEFERRED/BLOCKED note:** Why: Windows row needs B-35 and a Windows Environment provider. Unblocks when: Feature 005 task T17 is done. Test later at: the owner's Windows laptop.
- **Depends on:** T01, T14, T17
- **Cross-package prerequisites:** Feature 005 T17 (Windows Environment provider) and Feature 005 T18 (Windows E2E-04 qualified); Feature 011 Windows named-pipe profile qualified.
- **Owner:** Product, Release, repository owner.
- **Allowed paths:** same as T17.
- **Contract:** SPEC section 12; AC-017, AC-021.
- **Change intent:** until B-35 qualifies, record Windows-row evidence that AC-017 UNAVAILABLE behaviour holds in the packaged app; after B-35 qualifies, rerun the T17 journey on Windows x64.
- **Acceptance:**
  - [ ] Pre-B-35: packaged Windows build shows the UNAVAILABLE reason and admits nothing (AC-017).
  - [ ] Post-B-35: canonical command and USER journey pass on Windows x64 (AC-021).
- **Command:** `cargo test -p litecoworkd --test task_control_delegation --locked -- --nocapture`
- **Evidence:** evidence bundle entry for the Windows row.
- **Stop:** a Job Object-only configuration is presented as qualified.

## 015-T19 - Documentation and ledger sync

- **Status:** NOT STARTED
- **Depends on:** T01, T16, T17
- **Cross-package prerequisites:** NONE (orchestrator owns shared docs and closes register rows).
- **Owner:** Documentation owner, orchestrator.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/V1-UI-SCREEN-CONTRACT.md` (section 3 current boundary only), `docs/ACCEPTANCE-AND-E2E-PLAN.md` (delegation row), `docs/schemas/served-route-inventory.json`, `CURRENT-RUN.md`.
- **Contract:** AGENTS.md sections 3 and 5; AC-015, AC-016.
- **Change intent:** update capability rows (pause, resume, steer, cancel, handoff, delegation, verification, Work timeline) with exact evidence class per row; regenerate the served-route inventory; record the gate-E E2E row.
- **Acceptance:**
  - [ ] No row claims more than its evidence (Windows row stays UNAVAILABLE until T18).
  - [ ] `scripts/audit_served_routes.py` output matches the OpenAPI.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** PR diff.
- **Stop:** a status upgrade lacks an evidence reference.

## 015-T20 - Mission Control canvas and read-only Compare Agents fan-out (P-14)

- **Status:** NOT STARTED - added under this package's IN_REVIEW status. Folds proposed task P-14 from docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md section 8 into this package. The Compare Agents half is BLOCKED by O-09 because its Conversation/provider owner path and failure/persistence contract are not defined; do not expose a nonfunctional control.
- **Depends on:** T13, T16, O-09 for Compare Agents
- **Cross-package prerequisites:** Feature 001 turn activity records and Feature 005 Attempt records as the runtime read sources.
- **Owner:** Task Control, Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/`, `apps/litecoworkd/src/`, `specs/015-task-control-delegation/contracts/ui-behavior.md`, adjacent tests.
- **Contract:** SPEC §12; AC-015, AC-022; V1-UI-SCREEN-CONTRACT; PRESENTATION-RUNTIME.
- **Change intent:** within Work > Task detail, add a secondary Mission Control view that shows delegation tree, Step status, live tool activity, approvals and evidence from owner records, and expose it only after the first bounded workflow is proven. It is a projection, not a second workflow engine. Keep the P-14 Compare Agents feature disabled until O-09 defines its exact screen/control, Conversation/Agent Runtime owner, typed command/route, result identity/retention, partial failure and recovery contract; then implement an explicitly requested read-only fan-out with no Task/Step/Attempt/grant mutation. An absent runtime record renders unavailable, never fabricated. The list itself defers agent swarms past V1; this task does not add swarm orchestration.
- **Acceptance:**
  - [ ] The canvas renders the delegation tree, step status, live tool activity, approvals and evidence from real runtime records only (AC-022).
  - [ ] Compare Agents has an owner-approved user control and typed path from O-09; it is explicitly requested, read-only, and never mutates a Task or creates authority (AC-022).
  - [ ] The canvas is shown only after a bounded workflow is proven; an absent record renders unavailable (AC-015, AC-022).
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** canvas and Compare Agents test output in Completion evidence.
- **Stop:** the canvas would show a value with no runtime record; render it unavailable rather than infer.

## 015-T22 - Resolve, persist and project the immutable delegation-policy snapshot

- **Status:** DONE (USER-DELEGATED implementation; focused CODE scope) - 2026-11-20. Feature 015 remains IN_REVIEW; this is not formal package acceptance, Task execution integration, or release qualification.
- **Depends on:** Feature 009 T19; canonical `ResolvedDelegationPolicySnapshot` in `docs/schemas/operator-api.openapi.yaml`; SQLite v28 delegation policy records.
- **Cross-package prerequisites:** Feature 005 Task creation/spec-revision transaction and aggregate snapshot owner path; Feature 009 Coworker source round-trip.
- **Owner:** Task Runtime, Delegation, SQLite Storage, Operator/API and Work UI.
- **Allowed paths:** `crates/storage-core/src/lib.rs` and adjacent tests; `crates/storage-sqlite/src/lib.rs`, `docs/schemas/sqlite-v34.sql` and migration tests; `apps/litecoworkd/src/` Task Operator mappings and tests; `apps/litecowork-ui/src-tauri/src/` Task projection mapping and tests; `apps/litecowork-ui/src/App.tsx`, `apps/litecowork-ui/src/tasks/` and adjacent tests; `docs/schemas/operator-api.openapi.yaml` only if the existing canonical snapshot shape must be corrected; `docs/SCHEMAS.md`, `docs/STORAGE.md` (global migration registry and current version), `docs/LITECOWORK-BUILD-TASKS.md` (global provisional migration allocation), `docs/DELEGATION.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SPEC-TASK-TRACEABILITY.md`, `CURRENT-RUN.md`, this Feature 015 package, and only the migration-number reservation references in the active Feature 002, 003, 004, 006, 007, 008, 009, 010, 013, 017, 018 and 020 packages.
- **Contract:** `ResolvedDelegationPolicySnapshot`, `TaskSpecRevision`, SQLite v28 fields, `docs/DELEGATION.md`, Feature 005 TaskSpec create/revise transaction and Feature 015 DATA-MODEL/API-events/errors-recovery/UI behavior.
- **Change intent:** Resolve the Task's source from the exact immutable Coworker origin revision (when present) and the exact current Workspace delegation settings revision; apply the Workspace profile set as a ceiling; pin exact profile revisions and `worker_model_selection_policy`; include Workspace revision, Coworker ID/revision or null, effective strategy, native child-model policy and canonical `resolved_at`. Carry one typed snapshot in `TaskSpecRevisionRecord`, persist and return it for every new TaskSpec revision, include the identical value in committed responses and aggregate-state blobs, and display the current pin in Task details. Resolve before external BlobStore writes and revalidate the exact result inside the SQLite commit transaction; a Workspace-settings-head race conflicts with no public commit, and any unreachable content-addressed blob remains GC-eligible. Initial Task admission validates the selected Coworker version; later Coworker head changes do not replace the Task's immutable origin revision. An explicit TaskSpec revision is the only operation that re-resolves policy; existing Attempts retain their pinned revision. Add migration v34 to enrich already non-null v28 snapshots from their immutable Task/Coworker/Workspace/profile revisions and tighten the canonical trigger; keep pre-v28 null history null. Do not introduce a second snapshot schema or a new capability gate.
- **Acceptance:**
  - [x] Task create and explicit spec revision resolve a closed snapshot from owner records, including exact Coworker source/revision, Workspace ceiling, profile revisions, per-profile model-selection policy and canonical timestamp.
  - [x] SQLite revalidates the resolved snapshot in the same transaction as TaskSpec commit; a concurrent Workspace settings-head change fails with a conflict and does not commit a mismatched revision. Initial Task admission rejects a stale selected Coworker version, while a later Coworker-head change cannot rewrite the Task's immutable origin source.
  - [x] TaskSpec record/read/list, Operator/Tauri projection, idempotent response and aggregate-state blob all carry the same canonical snapshot; no handle, secret or caller-supplied authority is included.
  - [x] Migration v34 preserves pre-v28 null records, upgrades non-null legacy snapshots using immutable source rows, and rejects malformed or cross-Workspace profile references without partial migration.
  - [x] At T22 implementation time (2026-11-20), the global migration allocator and active package reservation references agreed with the then-wired v34 chain; only unwritten reservations moved, and historical migration records remained unchanged. The current chain later advanced to v35 under Feature 002-T07.
  - [x] Work > Task detail shows the pinned source/revisions/strategy/profile policy as provenance and renders legacy null as unavailable; it does not suggest a worker has run or been qualified.
  - [x] Changing Workspace settings does not mutate an existing TaskSpec or Attempt pin; only an explicit TaskSpec revision can receive a fresh Workspace policy snapshot. The Task's Coworker origin revision remains fixed.
- **Command:** `cargo test -j 1 -p storage-sqlite --locked task_create_persists_and_projects_the_canonical_delegation_policy_snapshot -- --nocapture`; `cargo test -j 1 -p storage-sqlite --locked task_create_pins_ -- --nocapture`; `cargo test -j 1 -p storage-sqlite --locked explicit_task_spec_revision -- --nocapture`; `cargo test -j 1 -p storage-sqlite --locked v34_migration -- --nocapture`; `cargo test -j 1 --manifest-path apps/litecowork-ui/src-tauri/Cargo.toml --offline task_delegation_snapshot_projection_tests --lib`; `pnpm --dir apps/litecowork-ui exec node --test tests/task-delegation-policy-snapshot.test.mjs`; `pnpm --dir apps/litecowork-ui exec tsc --noEmit`; `pnpm --dir apps/litecowork-ui run api:check`; `python .specify/scripts/validate_feature_package.py`.
- **Evidence:** native-Windows storage tests passed: create/read/history/receipt/aggregate-blob **1/1**; Coworker-default/override source, exact worker-profile policy and stale Coworker version **2/2**; explicit revision with Coworker source preserved and Workspace-head race **2/2**; v34 null preservation/backfill, foreign-profile rollback and v28-v34 replay **3/3**. Tauri projection **2/2**, Work provenance UI **2/2**, TypeScript, API generation and 20-package structural validation passed. Exact run records are in `CURRENT-RUN.md`. This does not qualify Task execution or heterogeneous delegation.
- **Stop:** a caller can supply snapshot authority, a mutable head is silently reread without revisioning, BlobStore work moves inside SQLite, old history is rewritten from current settings, or an Attempt loses its prior pin.

## 015-T21 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T20, T22
- **Cross-package prerequisites:** Feature 019 release evidence bundle accepted.
- **Owner:** Product, repository owner.
- **Allowed paths:** `specs/015-task-control-delegation/`.
- **Contract:** SPECIFICATION-STANDARD lifecycle; AC-001 to AC-022.
- **Change intent:** assemble the evidence bundle (per-task revision, command, result, OS, harness, reviewer); move status ACCEPTED/LOCKED to IMPLEMENTING when T02 starts and to VERIFIED when every task closes and the owner signs off.
- **Acceptance:**
  - [ ] Every AC has a passing test reference on the Linux row; Windows row is either qualified or recorded UNAVAILABLE with B-35 open.
  - [ ] Owner sign-off recorded.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `cargo test -p litecoworkd --test task_control_delegation --locked -- --nocapture`
- **Evidence:** Completion evidence table below.
- **Stop:** any AC lacks evidence.

## Dependency order

T01 -> T02 -> T03 -> (T04, T05 in parallel) -> T06 and T07 (both after T04) -> T08 (after T02, T03) -> T09 and T10 (after T08) -> T11 (after T04-T10) -> (T12, T13, T14 in parallel) -> T15 (after T04-T10) -> T16 (after T11-T15) -> T17 -> T18 (blocked on B-35) and T19. (T13, T16) -> T20 (Mission Control canvas, P-14). (T18, T19, T20) -> T21 (VERIFIED).

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T04, T07, T08, T10 | 005-T08 plus O-06 | Feature 005 revision 1.1 records the owner-directed generic Task Runtime seam; it remains IN_REVIEW. Feature 005 T08/T11 still cover one bounded file-write Attempt only. Generic provider AgentSession/Attempt admission, control and verifier lifecycle require cross-owner review and implementation before reuse |
| T04, T06, T15 | 005-T11 plus O-06 | Feature 005 task T11 covers the single file-write Attempt path; the owner-directed generic seam is IN_REVIEW, and generic session fan-out/recovery requires cross-owner review and implementation |
| T06, T11 | 005-T10 | `cancel_task` route and bridge are extended, not duplicated |
| T08 | 005-T04 | Linux bubblewrap provider supplies `ATTEMPT_PRIVATE` Environments |
| T10 | 005-T09 plus O-06 | Feature 005 task T09 provides an independent file verifier and Effect reconciliation for Feature 005's bounded slice; it does not provide a native-agent verifier Attempt/session |
| T12, T13 | 005-T13 | Controls and timeline mount in `TaskExecutionPanel.tsx` |
| T14 | 005-T14 | Same typed Windows UNAVAILABLE reason |
| T17 | 005-T15 | Linux E2E-04 must pass before delegation qualifies |
| T18 | 005-T17 | Windows Environment provider (B-35) |
| T18 | 005-T18 | Windows E2E-04 qualified first |
| T08 | 004-T05 | Claude Code dispatch arm in `AgentSessionSupervisor` |
| T08 | 002-T12 | Claude Code CLI AgentModule (B-11) |
| T08 | Feature 009 Coworker revision editing of `enabled_delegation_profile_ids` | Allowlist must be owner-editable |
| T09 | Feature 020 Needs You producer | NEEDS_YOU destination for `on_exhaustion` |
| T11, T17, T18 | Feature 011 IPC profiles (Windows named pipe, Linux socket) | Packaged desktop transport |
| T12 | Feature 011 app-wide keyboard and focus baseline | Control accessibility |
| T17, T20 | Feature 019 release matrix rows and evidence bundle | Qualification is per release row |

Feature 009, 011, 019 and 020 task plans are being written in the same release-planning pass, so those rows name the outcome; the orchestrator resolves them to task IDs in the traceability index.

## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Linux x86_64 distribution or Windows 11/10 x64), harness and version (OpenCode v2.0.32 lead, Claude Code CLI version), auth route, evidence class reached (CODE, SYSTEM, USER), report path and reviewer. Statuses above are source-audit classifications at this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
| (none yet) | | | | | | | |
