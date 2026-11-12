# Tasks - Safe Task Path

**Feature:** 005 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.0 | **Release target:** V1 (Task execution qualifies first on Linux x64; Windows x64 Task execution stays visibly UNAVAILABLE until B-35 is accepted and qualified; Task create/read remain available)

Task execution follows the B-17 rider: it qualifies first on the Linux x64 row. On the Windows x64 row, Task create and read views stay available while execution stays visibly UNAVAILABLE until B-35 (PROPOSED, pending owner) is accepted and its provider qualifies.

## Current baseline

Source baseline originally checked 2026-11-09; refreshed against `main` at `c892a567` plus the current working tree on 2026-11-11. The Environment consolidation evidence below reflects the current tree; the other baseline statements remain source observations unless their task status says otherwise.

- **Served Task envelope:** `create_task` in `apps/litecoworkd/src/operator.rs` commits Task + TaskSpecRevision + idempotency receipt. `validate_task_create_commit` in `crates/storage-sqlite/src/lib.rs` refuses any `conversation_id` or `source_message_refs` ("Conversation-origin Task creation is unavailable until its ConversationMessage can be appended atomically"). Home "New task" (`HomePage`, form `aria-label="Save a Task"` in `apps/litecowork-ui/src/App.tsx`) calls the Tauri `create_task` command in `apps/litecowork-ui/src-tauri/src/lib.rs`. `ConversationsPage.tsx` has no promotion control.
- **Planning is closed at two layers:** `apps/litecoworkd/src/agents/planning.rs` `prepare_local_task_planning` hardcodes `dispatch_available: false` with six `PlanningDispatchBlocker` values, and `LocalPlanningPreflight::require_dispatch()` is `#[allow(dead_code)]` and always returns an error. In storage, `require_task_planning_isolation_admission()` always returns `TASK_PLANNING_ISOLATION_UNAVAILABLE`, which closes `start_task_planning_session`.
- **Plan acceptance storage exists, no route:** `TaskStore::accept_initial_plan(PlanAcceptanceCommit) -> PlanAcceptance` (`crates/storage-core/src/lib.rs`, SQLite `accept_initial_plan_transaction`) is tested. `submitTaskPlan` is in `docs/schemas/operator-api.openapi.yaml` but is target-only. `parse_initial_plan_output` and `initial_plan_output_schema` exist in `crates/domain-task/src/planning.rs`.
- **Attempt admission has no caller:** `StepAttemptCoordinator::admit` / `decide_attempt_admission` in `crates/domain-task/src/execution.rs` and `admit_step_attempt` in `crates/storage-sqlite/src/execution.rs` have no caller in `apps/litecoworkd/src`.
- **Isolation evidence is dead code:** `apps/litecoworkd/src/main.rs` compiles `environment_isolation`, `environment_provider` and `trust` under `#[allow(dead_code)]`. `attempt_process_scope.rs` is `#![allow(dead_code)]`. `IsolationAttestation`, `evaluate_isolation_admission`, `evaluate_quiescence` and `RuntimeLocalIsolationStore` exist but nothing produces a real attestation.
- **Linux substrate:** `crates/linux-process-scope/src/bubblewrap.rs` `spawn_bubblewrapped` / `prepare_bubblewrap_command` already unshares user/pid/net/ipc/uts, mounts inputs read-only and one output root writable, under a systemd transient scope (`ManagedScope`). It has no attestation, egress broker or aggregate quota. `crates/local-environment-staging` stages pinned inputs (`stage_task_spec_inputs`, `PreparedEnvironment`) and measures output (`measure_output_root`, `check_output_quota`).
- **Windows substrate:** `crates/windows-local-security` has `spawn_kill_on_drop_job` (Job Object) and `replace_file_write_through`; no AppContainer, restricted token or WFP code.
- **Effects and verification:** `domain-effects` (`EffectRecord::transition`, `authorize_retry`, `EvidenceRecord`) and `crates/storage-sqlite/src/effect_evidence.rs` (`propose_effect`, `transition_effect`, `append_evidence`) exist with no Attempt caller. `verification_runs_operator.rs` serves list only.
- **Environment consolidation status after 2026-11-11:** `EnforcementStatus` now has one definition in `domain-environment::environment` and is re-exported by `local-environment-staging`; `EnvironmentLifetime`, `EnvironmentSharingScope`, `BudgetEnforcement` and `BudgetEnforcementPolicy` are imported from the domain owner. The daemon uses `linux-process-scope::probe_local_provider` for both capability and isolation probes, and only a trusted Bubblewrap candidate permits the LocalWorkspace class. The daemon now depends on the owner crates. T01 remains NOT STARTED; this code slice does not promote the execution contracts or authorize an admitted Task write.

Gaps to V1: Conversation promotion, planning dispatch, plan acceptance route, a real Linux provider and attestation, Attempt admission, the B-18 write, Trust/grant/approval for that write, reconciliation, verification, cancel/restart recovery, Work execution UI, the Windows-row UNAVAILABLE posture, B-35, and qualification evidence.

## 005-T01 - Promote accepted 005 contracts into owner docs and machine schemas

- **Status:** NOT STARTED
- **Depends on:** NONE
- **Cross-package prerequisites:** 011 IPC and served-route conventions (Wave 0); coordinate the next migration number with the orchestrator because several packages promote migrations in parallel.
- **Owner:** Task Runtime with Environments, Trust, Storage and the schema owner
- **Allowed paths:** `docs/TASK-RUNTIME.md`, `docs/ENVIRONMENTS.md`, `docs/TRUST.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/STORAGE.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `docs/schemas/sqlite-v36.sql` (allocated in `docs/LITECOWORK-BUILD-TASKS.md`; if another package merges first, use the next free number and update the allocation table), `specs/005-safe-task-path/`
- **Contract:** SPEC §5 (B-17, B-18 accepted; B-35 PROPOSED), §7.2, §13; AC-004, AC-005, AC-008, AC-009, AC-013
- **Change intent:** Promote the accepted target interfaces in one reviewed change: `AdmitFileWriteAttempt`, `ApprovedFileReplacement`, `FileWriteGrant`, `FileWriteVerifier`, `AttemptAdmissionError` and `FileWriteError` into TASK-RUNTIME/ENVIRONMENTS/TRUST prose; the Attempt admission, Effect list, `cancelTask` and `recoverStep` operations already present in the OpenAPI as target are confirmed with exact request/response bodies; error codes for attestation-not-enforced, path escape, over-bound and environment-provider-unavailable reuse existing codes where they fit (`DEPENDENCY_UNAVAILABLE`, `INTEGRITY_FAILURE`, `STALE_FENCE`) and add only what is missing; pin the bubblewrap minimum version and launch profile; record the Conversation-origin rule for `createTask` (`conversation_id` + `source_message_refs` admitted atomically with the origin message reference). The migration adds only columns/tables required for file-write grants, the Attempt-to-Environment link and the attestation digest reference; it never stores host paths or native handles.
- **Acceptance:**
  - [ ] Owner prose and OpenAPI/JSON Schema/migration change in the same commit; no parallel schema authority.
  - [ ] No new error code duplicates an existing canonical code.
  - [ ] Migration is immutable, forward-only and applied by `SqliteWorkspaceStore` startup; a database at the prior version upgrades and a downgrade attempt is refused.
  - [ ] B-35 remains marked PROPOSED in every promoted document.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`; `cargo test -p storage-sqlite --locked migration` (existing migration tests plus the new version)
- **Evidence:** commit SHA recorded in this package's completion table; register row for B-17/B-18 updated by the orchestrator.
- **Stop:** stop if a promoted field conflicts with an existing owner definition; resolve at the owner first.

## 005-T02 - Consolidate Environment types and provider probes into the owning crates (Rust)

- **Status:** PARTIAL 2026-11-11 (CODE; T01 owner-contract promotion still precedes integration). Canonical Environment enums and enforcement status are reused; the daemon discovery path delegates to the trust-checked `linux-process-scope` probe. The untrusted-candidate negative test passes.
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Environments
- **Allowed paths:** `apps/litecoworkd/Cargo.toml`, `apps/litecoworkd/src/environment_isolation.rs`, `apps/litecoworkd/src/environment_provider.rs`, `crates/domain-environment/src/environment.rs`, `crates/domain-environment/src/lib.rs`, `crates/linux-process-scope/src/provider_probe.rs`, `crates/local-environment-staging/src/output_budget.rs`
- **Contract:** SPEC §4 additional facts, §6 ownership; ENVIRONMENTS; AC-015
- **Change intent:** Make one definition per concept before wiring attestation: keep `EnforcementStatus` in one crate (prefer `domain-environment`) and re-export it; replace the daemon copies of `EnvironmentLifetime`, `EnvironmentSharingScope`, `BudgetEnforcement` and `BudgetEnforcementPolicy` with the `domain-environment` types; make `linux_process_scope::probe_local_provider` and `BUBBLEWRAP_CANDIDATES` the only bubblewrap discovery path, so `probe_environment_provider` uses the trust-checked candidate set; remove the duplicate `operating_system_build` and `cgroup_v2_available`. Pure Rust refactor with no behavior change except that an untrusted bwrap binary no longer reports a LocalWorkspace class.
- **Acceptance:**
  - [x] `rg` finds one `enum EnforcementStatus`, one `enum EnvironmentLifetime`, and one Bubblewrap candidate list (2026-11-11).
  - [x] An injected untrusted probe candidate yields no LocalWorkspace availability (daemon negative test). The Linux permission-mode predicate still requires execution on Linux for OS-level evidence.
  - [x] Existing environment-isolation, provider-probe, domain-environment and staging tests pass unchanged in meaning on Windows. Linux live Bubblewrap/systemd/cgroup qualification remains open.
- **Command:** `cargo test -p domain-environment --locked` (22 unit tests, plus 23 browser-control and 9 browser-pointer integration tests); `cargo test -p linux-process-scope --locked provider_probe` (5 passed); `cargo test -p litecoworkd --locked environment_` (64 passed); `cargo test -p local-environment-staging --locked` (24 passed). `cargo check -p litecoworkd --locked` passes with unrelated existing warnings.
- **Evidence:** test output in CURRENT-RUN.md; diff reviewed for zero semantic change.
- **Stop:** stop if consolidation changes any serialized enum spelling used by a schema.

## 005-T03 - Plan acceptance domain and storage with CAS and idempotency

- **Status:** PARTIAL (`accept_initial_plan` / `PlanAcceptanceCommit` / `accept_initial_plan_transaction` exist with tests in `crates/storage-core/src/lib.rs` and `crates/storage-sqlite/src/lib.rs`; `parse_initial_plan_output` in `crates/domain-task/src/planning.rs`; no producer-scope check against a live planning session because sessions cannot start)
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Task Runtime, Storage
- **Allowed paths:** `crates/domain-task/src/planning.rs`, `crates/domain-task/src/lib.rs`, `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/lib.rs`, `crates/storage-sqlite/src/execution.rs`, `crates/storage-sqlite/src/tests.rs`
- **Contract:** SPEC §7a (If-Match, `STALE_TASK_VERSION`, `STALE_SPEC_REVISION`, producer scope); TASK-RUNTIME plan acceptance; AC-004
- **Change intent:** Complete `accept_initial_plan` so one IMMEDIATE transaction checks Task version, TaskSpec head, the active TASK_PLANNING session/leased lead as producer, the plan digest and the request key, then writes PlanRevision + Steps + `task.plan.revised.v1` + `step.created.v1` + receipt. Same key and digest replays the original `PlanAcceptance`; same key with a different digest is `CONFLICT`.
- **Acceptance:**
  - [ ] Accepted plan commits PlanRevision and every Step atomically and creates no Attempt (AC-004).
  - [ ] Stale Task version returns 409 `STALE_TASK_VERSION`; stale spec head returns 409 `STALE_SPEC_REVISION`; producer that is not the active planning session returns 403 `FORBIDDEN` or 409 `CONFLICT`; each writes nothing.
  - [ ] Duplicate exact replay returns the original receipt; changed payload under the same key conflicts.
  - [ ] Two concurrent acceptances for the same Task: exactly one commits, the other gets a stale-version error.
  - [ ] Injected failure after Step insert rolls back the PlanRevision.
- **Command:** `cargo test -p storage-sqlite --locked plan_acceptance` (filter to be added by this task); `cargo test -p domain-task --locked planning`
- **Evidence:** test names listed in completion table.
- **Stop:** stop if acceptance would need a Task state not in STATE-MACHINES.

## 005-T04 - Linux bubblewrap Environment provider with four ENFORCED, attested dimensions

- **Status:** PARTIAL (launch primitive `spawn_bubblewrapped` / `prepare_bubblewrap_command` and live tests `live_bubblewrap_exposes_pinned_inputs_read_only_and_only_output_writable`, `live_bubblewrap_network_namespace_cannot_reach_host_loopback` exist in `crates/linux-process-scope/src/bubblewrap.rs`; `build_attestation`, `RuntimeLocalIsolationStore` exist in `apps/litecoworkd/src/environment_isolation.rs` as dead code; no provider produces an attestation)
- **Depends on:** T02
- **Cross-package prerequisites:** 011 Runtime incarnation identity and Linux 0600/0700 socket row (B-28 second row)
- **Owner:** Environments, Security
- **Allowed paths:** `crates/linux-process-scope/src/bubblewrap.rs`, `crates/linux-process-scope/src/lib.rs`, `crates/local-environment-staging/src/lib.rs`, `crates/local-environment-staging/Cargo.toml`, `crates/local-environment-staging/src/output_budget.rs`, `apps/litecoworkd/src/environment_isolation.rs`, `apps/litecoworkd/src/environment_provider.rs`, `apps/litecoworkd/src/linux_environment_provider.rs` (new), `apps/litecoworkd/src/main.rs`
- **Contract:** SPEC §5 B-17 (accepted, with rider), §10; ENVIRONMENTS provision/attest; AC-008, AC-015
- **Change intent:** Add a `LinuxBubblewrapProvider` that implements the `domain-environment` `EnvironmentLifecycle` provision/destroy for the LocalWorkspace class. After launch it runs observed probes inside the sandbox and fills `IsolationAttestation`: filesystem = write outside the output root denied and inputs read-only (observed); process-tree = systemd transient scope with recursive `populated 0` quiescence (observed via `ManagedScope`); network = `--unshare-net` plus failed connect to host loopback and to a public address (observed); resource-limit = cgroup v2 memory/pids/cpu limits read back from the scope plus `check_output_quota` on the output root. Bind to the current `runtime_incarnation_id` and the `digest_root_set` / `digest_pinned_inputs` digests. Store through `RuntimeLocalIsolationStore`. Remove `#[allow(dead_code)]` from `environment_isolation` once callers exist. The Runtime never sees a host path, only digests.
- **Acceptance:**
  - [ ] On a qualified Linux host, attestation has all four fields `ENFORCED` from observation (AC-015).
  - [ ] Attestation built from `probe_local_provider` output alone is refused by `evaluate_isolation_admission` (AC-015, negative).
  - [ ] Missing bwrap, untrusted bwrap, no user namespaces or no cgroup v2 delegation each yields `UNKNOWN` or `NOT_ENFORCED` on the affected dimension and admission fails closed (AC-008).
  - [ ] Attestation from a previous Runtime incarnation or past `expires_at_ms` is refused.
  - [ ] Symlink/hardlink in inputs and output-root reuse are rejected (existing test kept).
  - [ ] Destroy requires `QuiescenceResult` QUIESCENT; UNKNOWN keeps the Environment held.
- **Command:** `cargo test -p linux-process-scope --locked bubblewrap`; `cargo test -p litecoworkd --locked linux_environment_provider` (filter to be added by this task); live tests run on the Linux x64 qualification host
- **Evidence:** attestation JSON (redacted, digests only) attached to the Linux-row evidence bundle; host kernel, bwrap and systemd versions recorded.
- **Stop:** stop if any dimension can only be asserted from configuration rather than observed; never relax to "probably enforced".

## 005-T05 - Native lead planning dispatch behind the isolation gate (Linux row)

- **Status:** NOT STARTED (`require_dispatch()` always errors; `dispatch_available=false` hardcoded in `apps/litecoworkd/src/agents/planning.rs`; `require_task_planning_isolation_admission()` closes storage)
- **Depends on:** T03, T04
- **Cross-package prerequisites:** 001 AgentSessionSupervisor bounded lifecycle and settlement (the planning session reuses it); 002 creatable OpenCode binding with real probe compatibility
- **Owner:** Task Runtime, Agent Runtime
- **Allowed paths:** `apps/litecoworkd/src/agents/planning.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecoworkd/src/operator.rs` (planning-readiness handler only), `crates/domain-task/src/planning.rs`, `crates/storage-sqlite/src/lib.rs` (`require_task_planning_isolation_admission`, `start_task_planning_session`, `activate_task_planning_session`, `mark_starting_task_planning_session_lost`)
- **Contract:** SPEC §6 actor sequence steps 2-3, §7a (`DEPENDENCY_UNAVAILABLE` without attestation); TASK-RUNTIME planning session tools; AC-012, AC-014
- **Change intent:** Replace the always-failing `require_dispatch()` with a real gate that clears each `PlanningDispatchBlocker` only on evidence: TaskIsolationUnavailable clears when a current attestation exists; ProcessContainmentUnqualified clears with the T04 scope; ProtocolUnqualified and SessionSettlementUnavailable clear only for the harness/version the Linux row declares; NativeCapabilitiesUnmediated stays reported as a disclosed limitation. Replace `require_task_planning_isolation_admission()` with a check of a Runtime-held admission proof passed into `start_task_planning_session`. Start one planning AgentSession with Task read, plan-proposal and clarification tools only, send the bounded `TaskPlanningPacket`, parse with `parse_initial_plan_output`, and hand the proposal to T03. `dispatch_available` is computed, not hardcoded.
- **Acceptance:**
  - [ ] Without a current attestation, planning returns `DEPENDENCY_UNAVAILABLE` and creates no session, PlanRevision, Step, Attempt or Environment.
  - [ ] On the Windows row (no provider), readiness lists the provider blocker with reason "no qualified Windows Environment provider (B-35)" (AC-014).
  - [ ] Preflight packet build is bounded in bytes and items and does not scan unbounded Task history (AC-012).
  - [ ] A planning session lost during STARTING is marked LOST on restart and no plan is fabricated.
  - [ ] Malformed or oversized plan output is rejected with no write.
- **Command:** `cargo test -p litecoworkd --locked task_planning_dispatch` (filter to be added by this task); `cargo test -p storage-sqlite --locked task_planning_session`
- **Evidence:** test names; one recorded Linux-row planning run with harness version.
- **Stop:** stop if the planning session would receive write tools, an Environment handle or credentials.

## 005-T06 - B-18 atomic whole-file replacement primitive (Rust)

- **Status:** NOT STARTED (`replace_file_write_through` exists for Windows in `crates/windows-local-security/src/lib.rs`; Linux path confinement helpers exist in `bubblewrap.rs` `validate_input_tree`; no write primitive)
- **Depends on:** T01
- **Cross-package prerequisites:** 003 pinned ResourceRevision digests and WorkspaceRoot identity
- **Owner:** Environments, Storage
- **Allowed paths:** `crates/local-environment-staging/src/lib.rs`, `crates/local-environment-staging/src/file_replace.rs` (new), `crates/windows-local-security/src/lib.rs` (reuse only)
- **Contract:** SPEC §5 B-18 (accepted), §7.2 `ApprovedFileReplacement`; AC-009, AC-016
- **Change intent:** Add `replace_approved_file(target: &WorkspaceConfinedPath, base: &ContentDigest, new_content: &BoundedBlob) -> Result<ReplaceOutcome, FileWriteError>` in a dependency-light crate so both providers share it. It opens the root by handle (`openat`-style on Linux, `open_directory_path_without_reparse` on Windows), refuses symlink, hardlink and reparse escapes, re-reads and digests the current content, refuses if it differs from `base`, writes a same-directory temp file, fsyncs file and directory, then renames atomically (`renameat` on Linux, `replace_file_write_through` on Windows). Bound is 1 MiB checked before any write. Temp names are derived from the Effect id so recovery can find them.
- **Acceptance:**
  - [ ] Write inside the root with the matching base digest succeeds and the observed digest equals the approved digest.
  - [ ] Path traversal, absolute path, symlink, hardlink and Windows reparse targets are refused with `PathEscape` and no temp file remains (AC-009).
  - [ ] 1 MiB + 1 byte is refused with `OverBound` before any filesystem write (AC-009).
  - [ ] Base digest mismatch returns `IntegrityFailure` and leaves the target unchanged.
  - [ ] Process kill between temp write and rename leaves the target at the base digest and a findable temp file (AC-016 input).
- **Command:** `cargo test -p local-environment-staging --locked file_replace` (filter to be added by this task)
- **Evidence:** test names; fuzzed path cases count.
- **Stop:** stop if atomic rename semantics cannot be guaranteed on the target filesystem; report the filesystem as unsupported instead.

## 005-T07 - Trust wiring, file-write grant and exact Effect approval

- **Status:** PARTIAL (`apps/litecoworkd/src/trust.rs` evaluator and approval producer exist under `#[allow(dead_code)]`; `trust_operator.rs` `create_read_grant` serves only `workspace.resource.read`; approval resolve with exact `confirmation_digest` is served via `approval_operator.rs`)
- **Depends on:** T01
- **Cross-package prerequisites:** NONE (006 reuses the same wiring later; keep the evaluator generic)
- **Owner:** Trust
- **Allowed paths:** `apps/litecoworkd/src/trust.rs`, `apps/litecoworkd/src/trust_operator.rs`, `apps/litecoworkd/src/main.rs`, `crates/storage-core/src/trust.rs`, `crates/storage-sqlite/src/trust.rs`
- **Contract:** SPEC §5 LOCKED "Grants are explicit and scoped", B-18; TRUST; AC-005, AC-008
- **Change intent:** Remove `#[allow(dead_code)]` from `trust` by giving it a caller. Add a `FileWriteGrant` issued only from an accepted PlanRevision Step: one confined path, base digest, max 1 MiB, expiry, Attempt-bound. The Trust evaluator classifies the replacement as a consequential local write and produces an Approval bound to the action digest (path + base digest + new-content digest + plan digest). The grant never authorizes a second path or a shell command.
- **Acceptance:**
  - [ ] A grant for path A cannot authorize path B or a changed content digest (negative).
  - [ ] Approval resolved for one digest cannot be used for another; expired or revoked grant blocks admission.
  - [ ] An agent principal cannot approve its own write.
  - [ ] Read-grant behavior of `create_read_grant` is unchanged.
- **Command:** `cargo test -p litecoworkd --locked trust_file_write_grant` (filter to be added by this task); `cargo test -p storage-sqlite --locked trust`
- **Evidence:** test names.
- **Stop:** stop if the grant would need to be reusable across Attempts.

## 005-T08 - Attempt admission service: attestation gate, lease, grant, receipt, then Environment call

- **Status:** NOT STARTED (`StepAttemptCoordinator::admit`, `decide_attempt_admission`, SQLite `admit_step_attempt` exist with no daemon caller)
- **Depends on:** T03, T04, T06, T07
- **Cross-package prerequisites:** NONE
- **Owner:** Task Runtime
- **Allowed paths:** `apps/litecoworkd/src/task_attempt_service.rs` (new), `apps/litecoworkd/src/attempt_process_scope.rs`, `apps/litecoworkd/src/environment_isolation.rs`, `apps/litecoworkd/src/main.rs`, `crates/domain-task/src/execution.rs`, `crates/storage-sqlite/src/execution.rs`, `crates/storage-sqlite/src/effect_evidence.rs`
- **Contract:** SPEC §6 steps 4-5, §7.2 `AdmitFileWriteAttempt`, §7a; AC-005, AC-008
- **Change intent:** Add `TaskAttemptService::admit_file_write(AdmitFileWriteAttempt)`: attach the Environment first, load the Runtime-held attestation, run `evaluate_isolation_admission`, then one transaction via `StepAttemptCoordinator::admit` committing Attempt + ExecutionLease + FileWriteGrant use + PROPOSED Effect + idempotency receipt. Only after commit, call the provider's `replace_approved_file` outside SQLite and journal the process scope through `AttemptProcessScopeJournal::record_active` (removing its `#![allow(dead_code)]`). Map refusals to `AttemptAdmissionError`.
- **Acceptance:**
  - [ ] Admission-ordering test proves no filesystem call before commit and no external call inside the transaction (AC-005).
  - [ ] Any attestation field `UNKNOWN` or `NOT_ENFORCED`, wrong incarnation, wrong Workspace/Task, or digest mismatch refuses admission and writes nothing (AC-008).
  - [ ] Caller-supplied attestation in the request is rejected; only the Runtime-held one is used.
  - [ ] Duplicate admission with the same key returns the original Attempt; a different payload conflicts.
  - [ ] Two concurrent admissions for one Step: one lease, the other `LeaseConflict`.
- **Command:** `cargo test -p litecoworkd --locked task_attempt_admission` (filter to be added by this task); `cargo test -p domain-task --locked execution`; `cargo test -p storage-sqlite --locked execution`
- **Evidence:** test names.
- **Stop:** stop if admission requires any Environment call inside a SQLite transaction.

## 005-T09 - Effect reconciliation and independent verification

- **Status:** PARTIAL (`EffectRecord::transition` / `authorize_retry`, `EvidenceRecord` in `crates/domain-effects/src/lib.rs`; `transition_effect` / `append_evidence` in `crates/storage-sqlite/src/effect_evidence.rs`; `verification_runs_operator.rs` read-only; no reconciler or verifier caller)
- **Depends on:** T06, T08
- **Cross-package prerequisites:** NONE
- **Owner:** Trust, Task Runtime
- **Allowed paths:** `apps/litecoworkd/src/task_effect_reconciler.rs` (new), `apps/litecoworkd/src/task_attempt_service.rs`, `crates/domain-effects/src/lib.rs`, `crates/storage-sqlite/src/effect_evidence.rs`, `crates/storage-sqlite/src/verification_runs.rs`, `crates/storage-core/src/verification_run.rs`
- **Contract:** SPEC §7.2 `FileWriteVerifier`, §11; release contract §6 (unknown is reconciled); AC-006, AC-007
- **Change intent:** After the provider returns (or times out), the reconciler reads the target digest through a fresh confined handle and settles the Effect: equals approved digest -> COMMITTED; equals base -> NOT_APPLIED; anything else -> AMBIGUOUS held for the owner. A separate `FileWriteVerifier` implementation that does not share the writer's handle recomputes the digest and writes a VerificationRun + Evidence pinned to TaskSpec, PlanRevision and the Effect. Retry is allowed only through `authorize_retry` after reconciliation.
- **Acceptance:**
  - [ ] Timeout after the provider call reconciles before any retry; no second write while AMBIGUOUS (AC-006).
  - [ ] Worker exit code or provider "ok" alone never produces PASSED verification (AC-007).
  - [ ] Verifier mismatch records FAILED evidence and the Task does not report success.
  - [ ] Duplicate reconcile is idempotent.
- **Command:** `cargo test -p litecoworkd --locked task_effect_reconcile` (filter to be added by this task); `cargo test -p domain-effects --locked`
- **Evidence:** test names.
- **Stop:** stop if verification would read the content through the writer's own handle or process.

## 005-T10 - Operator routes and Tauri bridge commands for plan, Attempt, Effect, cancel and recover

- **Status:** NOT STARTED (target-only in `docs/schemas/served-route-inventory.json`: `submitTaskPlan`, Attempt list/admit, Effect list, `cancelTask`, `recoverStep`)
- **Depends on:** T05, T08, T09
- **Cross-package prerequisites:** 011 authenticated IPC on both rows
- **Owner:** Operator, Desktop UI (bridge)
- **Allowed paths:** `apps/litecoworkd/src/operator.rs`, `apps/litecoworkd/src/task_execution_operator.rs` (new), `apps/litecoworkd/src/verification_runs_operator.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/src-tauri/src/task_execution_bridge.rs` (new), `apps/litecowork-ui/src/api/` (typed client)
- **Contract:** SPEC §7.1-7.2; `docs/schemas/operator-api.openapi.yaml` operations promoted by T01; AC-004, AC-005, AC-011
- **Change intent:** Mount `POST /v1/tasks/{task_id}/plan-revisions` (`submitTaskPlan`, owner accept with If-Match), Attempt list/admit, Effect list, `cancelTask` and `recoverStep` behind the existing authenticate middleware with Workspace-owner checks, `no-store` and idempotency keys. Add Tauri commands `accept_task_plan`, `start_task_attempt`, `list_task_attempts`, `list_task_effects`, `cancel_task`, `recover_task_step` that validate identifiers like `discover_bridge.rs` does and never pass host paths. Pause/resume/steer are not in this package.
- **Acceptance:**
  - [ ] Each route rejects a foreign Principal, wrong Workspace and missing idempotency key.
  - [ ] Route responses never contain native handles, host paths or attestation internals beyond digests and enforcement statuses.
  - [ ] Served-route inventory regeneration lists exactly the new operations.
  - [ ] Bridge commands reject malformed ids before IPC.
- **Command:** `cargo test -p litecoworkd --locked task_execution_operator` (filter to be added by this task); `cargo test -p litecowork-ui --locked task_execution_bridge` (filter to be added by this task); `python3 scripts/audit_served_routes.py`
- **Evidence:** regenerated `docs/schemas/served-route-inventory.json` diff.
- **Stop:** stop if any route can start execution on a platform with no qualified provider.

## 005-T11 - Cancel, quiescence, crash and restart recovery

- **Status:** NOT STARTED (`evaluate_quiescence`, `QuiescenceObservation`, `ManagedAttemptScope::wait_for_quiescence`, `AttemptProcessScopeJournal::incomplete_records` exist as dead code; `decide_lease_expiry` exists in `crates/domain-task/src/execution.rs`)
- **Depends on:** T08, T09, T10
- **Cross-package prerequisites:** 011 Runtime startup recovery ordering
- **Owner:** Task Runtime, Environments
- **Allowed paths:** `apps/litecoworkd/src/task_attempt_service.rs`, `apps/litecoworkd/src/task_effect_reconciler.rs`, `apps/litecoworkd/src/attempt_process_scope.rs`, `apps/litecoworkd/src/runtime.rs` (startup sweep hook only), `crates/domain-task/src/execution.rs`, `crates/storage-sqlite/src/execution.rs`, `crates/local-environment-staging/src/file_replace.rs`
- **Contract:** SPEC §7a (CANCEL_REQUESTED, recover_step, new incarnation cannot renew), §11-§12; AC-007, AC-010, AC-016
- **Change intent:** Cancel moves the Task to CANCEL_REQUESTED, fences the lease, reconciles STARTED/ACKNOWLEDGED/AMBIGUOUS Effects, waits for observed QUIESCENT, then commits CANCELLED. Startup sweep: for every journaled Attempt from a prior incarnation commit the lease EXPIRED or REVOKED and the Attempt ABANDONED, find temp files by Effect id, reconcile the target digest, delete orphans, never reissue. `recover_step` creates a new Attempt id after reconciliation.
- **Acceptance:**
  - [ ] Kill the daemon between temp write and rename: restart shows a durable receipt, Effect AMBIGUOUS then reconciled, temp file removed, no automatic rewrite (AC-016, AC-007).
  - [ ] Releasing the lease or destroying the Environment with quiescence UNKNOWN or timed out keeps the Attempt fenced; a stale fence returns `STALE_FENCE` (AC-010).
  - [ ] Cancel racing a completing write: either the write is reconciled COMMITTED and cancel settles after it, or it was never started; never both "cancelled" and an unreported write.
  - [ ] New incarnation cannot renew the old lease.
- **Command:** `cargo test -p litecoworkd --locked task_attempt_recovery` (filter to be added by this task); `cargo test -p litecoworkd --locked attempt_process_scope`
- **Evidence:** crash-injection test names; Linux-row restart recording.
- **Stop:** stop if recovery would infer success from an intent record.

## 005-T12 - Conversation to Task promotion (storage, route and UI)

- **Status:** NOT STARTED (`validate_task_create_commit` refuses Conversation origin; `ConversationsPage.tsx` has no control; Home "New task" works for standalone objectives)
- **Depends on:** T01
- **Cross-package prerequisites:** 001 committed AGENT message identity and message version (promotion needs a committed message ref); 011 keyboard/focus baseline
- **Owner:** Task Runtime, Conversation, Desktop UI
- **Allowed paths:** `crates/storage-sqlite/src/lib.rs` (`validate_task_create_commit` and the Task create transaction), `crates/storage-core/src/lib.rs`, `apps/litecoworkd/src/operator.rs` (`create_task`), `apps/litecowork-ui/src-tauri/src/lib.rs` (`create_task`), `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`, `apps/litecowork-ui/src/conversations/conversation-view.ts`, `apps/litecowork-ui/src/conversations/task-promotion.ts` (new), `apps/litecowork-ui/tests/conversation-task-promotion.test.ts` (new)
- **Contract:** SPEC §7.2 promotion note; TASK-RUNTIME `CreateTaskRequest` (`conversation_id`, `source_message_refs[]`); contracts/ui-behavior.md; AC-001, AC-002, AC-003, AC-013
- **Change intent:** In storage, accept `conversation_id` + `source_message_refs` only when each ref names a committed message in the same Workspace and Conversation, and append the origin reference with the Task in the same IMMEDIATE transaction; then remove the refusal branch. In the UI, add a "Create Task from this reply" action on committed AGENT messages that opens a prefilled Task draft (objective from the user's prior message, editable), shows Workspace, lead and pinned inputs, and saves via `create_task` with the same request-id retry rule Home uses. Put the pure draft/state logic in a TypeScript module (`task-promotion.ts`) so it is unit-testable without React.
- **Acceptance:**
  - [ ] Ordinary Send creates no Task (AC-001).
  - [ ] Promotion commits Task + TaskSpecRevision + origin ref with no Attempt (AC-002, AC-013).
  - [ ] Lead unavailable returns `AGENT_UNAVAILABLE`, creates no Task and keeps the draft (AC-003).
  - [ ] Control is absent on streaming, failed, cancelled or uncommitted turns; a ref to another Conversation or Workspace is refused.
  - [ ] Lost response then retry with the same request id returns the original Task; changed draft makes a new request.
  - [ ] Control is reachable by keyboard, has an accessible name, and focus moves to the draft then back on cancel.
- **Command:** `cargo test -p storage-sqlite --locked task_conversation_origin` (filter to be added by this task); `cargo test -p litecoworkd --locked create_task`; `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test names; screenshot pair (Linux and Windows rows) in the evidence bundle.
- **Stop:** stop if promotion would need to mutate the Conversation turn state.

## 005-T13 - Work screen execution UI with full state coverage

- **Status:** PARTIAL (`TaskDetailPanel` in `apps/litecowork-ui/src/App.tsx` shows read-only planning readiness via `apps/litecowork-ui/src/tasks/planning-readiness.ts`, Steps, `TaskPresentationPanel` and outputs; no execution controls)
- **Depends on:** T10, T11
- **Cross-package prerequisites:** 007 Task output Artifact projection; 011 app keyboard/focus baseline
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/App.tsx` (`TaskDetailPanel` only), `apps/litecowork-ui/src/tasks/` (new modules `task-execution-state.ts`, `TaskExecutionPanel.tsx`), `apps/litecowork-ui/src/presentation/TaskPresentationPanel.tsx`, `apps/litecowork-ui/tests/task-execution-state.test.ts` (new), `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`
- **Contract:** contracts/ui-behavior.md; SPEC §6; AC-011, AC-014
- **Change intent:** Move execution UI out of the monolithic `App.tsx` into `TaskExecutionPanel.tsx` with a pure reducer in `task-execution-state.ts` (no UI-owned Task state machine; it maps owner records to view states). Render separately: proposed plan with Accept (If-Match), Attempt state, Environment class and the four enforcement statuses, grant and Approval (deep link to Needs You `ApprovalDetail`), Effect PROPOSED/STARTED/AMBIGUOUS/COMMITTED/NOT_APPLIED, Evidence and verification PASSED/FAILED, Cancel and Recover. States: loading, empty (no plan yet), error, denied, offline (Runtime unreachable), stale (409 refetch), UNAVAILABLE (no provider), recovery (AMBIGUOUS awaiting reconcile).
- **Acceptance:**
  - [ ] A saved READY Task or progress snapshot never renders as a running worker (AC-011).
  - [ ] AMBIGUOUS never renders as success or offers Retry; Recover appears only after reconciliation.
  - [ ] On a platform with no provider, Accept-and-run and Start are replaced by an UNAVAILABLE notice naming B-35; no Job Object badge (AC-014).
  - [ ] Every control has an accessible name, `aria-live` status for state changes, and visible focus; the panel works at the narrow layout from `responsive-layout.test.ts`.
- **Command:** `pnpm --dir apps/litecowork-ui test`; `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; UI ledger rows updated.
- **Stop:** stop if any control lacks a served owner route.

## 005-T14 - Windows row: Task execution visibly UNAVAILABLE until B-35 qualifies

- **Status:** PARTIAL (2026-11-11 CODE: the Windows provider probe now advertises no `LocalWorkspace` class or supported Task lifetime and explains that Job Objects manage process lifetime only; the planning-readiness contract carries `WINDOWS_ENVIRONMENT_PROVIDER_UNAVAILABLE` and the Work diagnostic has a plain-language B-35 label. Windows provider tests now pass (10/10), and the typed blocker is represented in the generated UI contract; a packaged Work screenshot/projection record remains to close this task.)
- **Depends on:** T05, T13
- **Cross-package prerequisites:** 019 release matrix row definitions; 008 shares the same UNAVAILABLE reason for the daemon-owned terminal
- **Owner:** Environments, Desktop UI
- **Allowed paths:** `apps/litecoworkd/src/environment_provider.rs`, `apps/litecoworkd/src/agents/planning.rs`, `apps/litecowork-ui/src/tasks/task-execution-state.ts`, `apps/litecowork-ui/src/tasks/planning-readiness.ts`, `apps/litecowork-ui/tests/planning-readiness.test.ts`
- **Contract:** SPEC §5 B-17 rider, §13 release matrix; AC-014
- **Change intent:** On Windows, report no Task-execution-capable class until a B-35 provider exists; reword the note to "Job Object process lifetime only; not an isolation boundary". Add a `PlanningDispatchBlocker` reason (or reuse TaskIsolationUnavailable with a typed `provider_unavailable_reason`) that the UI renders as "Task execution is not available on Windows yet: no qualified Windows Environment provider." Task create, spec edit and read views remain enabled.
- **Acceptance:**
  - [ ] Windows readiness projection reports execution UNAVAILABLE with the B-35 reason (AC-014).
  - [ ] No string in the Windows projection or UI calls the Job Object isolation or a sandbox.
  - [ ] Task create and spec edit still succeed on Windows.
- **Command:** `cargo test -p litecoworkd --locked environment_provider`; `pnpm --dir apps/litecowork-ui test`
- **Evidence:** Windows-row screenshot and projection JSON in the evidence bundle.
- **Stop:** stop if any Windows path could admit an Attempt.

## 005-T15 - Linux-row qualification: E2E-04 with cross-layer traces

- **Status:** NOT STARTED
- **Depends on:** T12, T13, T14
- **Cross-package prerequisites:** 001 and 002 Linux-row native route qualified; 019 evidence bundle process and Linux row; 011 packaged desktop on Linux
- **Owner:** Task Runtime, QA, repository owner (USER evidence)
- **Allowed paths:** `apps/litecoworkd/tests/` (new E2E harness files), `apps/litecowork-ui/tests/`, `scripts/v1_platform_matrix.py` (row entry only), `specs/005-safe-task-path/`
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-04; AGENTS.md §4; AC-001 to AC-016
- **Change intent:** Record the AGENTS.md §4 trace for each user-visible operation: (1) promote ("Create Task from this reply" -> `create_task` command -> `POST /v1/tasks` -> Workspace owner -> TaskService -> IMMEDIATE txn with receipt -> `task.created.v1` -> Work list -> restart reload -> test); (2) accept plan (Accept button -> `accept_task_plan` -> `submitTaskPlan` If-Match -> producer scope -> `accept_initial_plan` -> PlanRevision/Steps -> Work plan view -> replay); (3) approve and run write (Approve in Needs You + Start -> `start_task_attempt` -> Attempt admit -> attestation gate -> Attempt/lease/grant/receipt txn -> provider replace outside SQLite -> Effect/Evidence -> Work execution panel -> crash recovery); (4) cancel; (5) recover. Run packaged desktop E2E-04 on Linux x64 with denial (bad grant, stale plan, non-enforced attestation), duplicate replay, timeout-after-dispatch, cancel race, crash and restart.
- **Acceptance:**
  - [ ] CODE, SYSTEM and owner USER evidence on the exact Linux build, kernel, bwrap, systemd and harness version.
  - [ ] Every negative and recovery case in AC-005 to AC-010 and AC-016 exercised in the packaged app.
  - [ ] Trace table for the five operations stored with the evidence.
- **Command:** `cargo test -p litecoworkd --locked e2e_04_safe_task` (filter to be added by this task); `pnpm --dir apps/litecowork-ui test`; `python3 scripts/v1_platform_matrix.py`
- **Evidence:** evidence bundle path recorded in CURRENT-RUN.md and IMPLEMENTATION-STATUS.md Area 6 row.
- **Stop:** stop and keep the row unqualified if any USER step needs manual database or filesystem intervention.

## 005-T16 - Owner decision on B-35 Windows Environment provider

- **Status:** BLOCKED (B-35 PROPOSED; owner: repository owner, with Environments and Security)
- **DEFERRED/BLOCKED note:** Why: B-35 (Windows Environment provider) is PROPOSED and not owner-accepted. Unblocks when: owner decision on B-35. Test later at: decision only; no test.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Repository owner; Environments; Security
- **Allowed paths:** `specs/005-safe-task-path/` (SPEC §5 row and revision bump), `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md` (orchestrator)
- **Contract:** SPEC §5 B-35, §7.3; SPECIFICATION-STANDARD §7
- **Change intent:** Present B-35 (AppContainer + restricted token + Job Object + WFP egress, four dimensions ENFORCED and attested) for decision. If accepted, bump SPEC to 1.1 and mark the row ACCEPTED; if deferred, record that the Windows row ships with Task execution UNAVAILABLE.
- **Acceptance:**
  - [ ] Owner decision recorded with date and the exact Windows builds in scope.
  - [ ] If rejected or deferred, T17 and T18 are marked owner-deferred and T14 is the shipped Windows posture.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** SPEC revision and register row.
- **Stop:** no Windows provider code before this decision.

## 005-T17 - Windows Environment provider: AppContainer, restricted token, Job Object, WFP egress (Rust)

- **Status:** BLOCKED (needs T16 acceptance; `spawn_kill_on_drop_job` and `replace_file_write_through` exist in `crates/windows-local-security/src/lib.rs`, nothing else)
- **DEFERRED/BLOCKED note:** Why: needs T16 acceptance and is Windows-only (AppContainer, restricted token, Job Object, WFP egress). Unblocks when: T16 accepted, then Windows implementation. Test later at: the owner's Windows laptop.
- **Depends on:** T02, T15, T16
- **Cross-package prerequisites:** 011 Windows named-pipe IPC and Runtime incarnation on the Windows row
- **Owner:** Environments, Security
- **Allowed paths:** `crates/windows-local-security/src/lib.rs`, `crates/windows-local-security/src/appcontainer.rs` (new), `crates/windows-local-security/src/wfp.rs` (new), `crates/windows-local-security/Cargo.toml`, `apps/litecoworkd/src/windows_environment_provider.rs` (new), `apps/litecoworkd/src/environment_provider.rs`, `apps/litecoworkd/src/main.rs`
- **Contract:** SPEC §5 B-35, §7.3; ENVIRONMENTS; AC-008, AC-017
- **Change intent:** Implement `WindowsEnvironmentLaunch` in Rust using the `windows-sys` crate (0.61) already used by `windows-local-security`, adding only the needed feature flags: create a per-Environment AppContainer profile, grant its package SID read ACLs on staged inputs and write ACL on one output root only, launch with a restricted token inside a Job Object with KILL_ON_JOB_CLOSE and process/memory/CPU limits, and install a WFP filter blocking outbound for the package SID. Attest from observed probes: denied write outside the root, denied outbound connect, Job accounting read back, quiescence via Job active-process count 0. Reuse the T06 replace primitive. Clean up the profile and WFP filter on destroy and on startup sweep.
- **Acceptance:**
  - [ ] All four fields `ENFORCED` from observation on the exact Windows build (AC-017).
  - [ ] Job Object alone, or any missing mechanism, yields NOT_ENFORCED/UNKNOWN and admission fails closed (AC-008).
  - [ ] Leftover WFP filters and AppContainer profiles from a crashed incarnation are removed on startup.
  - [ ] Admin rights are not required at run time, or the requirement is recorded as a blocker.
- **Command:** `cargo test -p windows-local-security --locked appcontainer` (filter to be added by this task); `cargo test -p litecoworkd --locked windows_environment_provider` (filter to be added by this task)
- **Evidence:** Windows build number, attestation (digests only), WFP filter dump in the evidence bundle.
- **Stop:** stop if WFP filters need elevation the installer cannot provide; return to the owner.

## 005-T18 - Windows-row qualification of Task execution (E2E-04)

- **Status:** BLOCKED (needs T17; B-35 pending owner)
- **DEFERRED/BLOCKED note:** Why: Windows row of Task execution (E2E-04) depends on T17. Unblocks when: T17 done. Test later at: the owner's Windows laptop.
- **Depends on:** T17
- **Cross-package prerequisites:** 019 Windows installer and evidence bundle process; 001 Windows OpenCode v2.0.32 route qualified
- **Owner:** Task Runtime, QA, repository owner (USER evidence)
- **Allowed paths:** `apps/litecoworkd/tests/`, `apps/litecowork-ui/tests/`, `scripts/v1_platform_matrix.py` (row entry only), `specs/005-safe-task-path/`
- **Contract:** E2E-04; AC-014, AC-017
- **Change intent:** Repeat the T15 E2E and traces on the packaged Windows build, then remove the UNAVAILABLE posture from T14 for that exact build only.
- **Acceptance:**
  - [ ] CODE, SYSTEM and owner USER evidence on Windows 11/10 `x86_64-pc-windows-msvc` with OpenCode v2.0.32, including denial and recovery.
  - [ ] Builds not in the evidence still render UNAVAILABLE.
- **Command:** `cargo test -p litecoworkd --locked e2e_04_safe_task` (same filter as the Linux run, Windows host); `python3 scripts/v1_platform_matrix.py`
- **Evidence:** Windows evidence bundle; IMPLEMENTATION-STATUS.md Area 6 row.
- **Stop:** stop if WSL2 is the only evidence; it does not count.

## 005-T19 - Documentation and ledger sync

- **Status:** NOT STARTED
- **Depends on:** T15
- **Cross-package prerequisites:** orchestrator owns the register and specs index edits
- **Owner:** Task Runtime, documentation owner
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `CURRENT-RUN.md`, `specs/005-safe-task-path/`
- **Contract:** AGENTS.md §4 steps 5-8
- **Change intent:** Update the Area 6 and Safe Task Path sections of IMPLEMENTATION-STATUS.md with exact evidence level per row (Linux row, Windows row separately); regenerate the served-route inventory with `scripts/audit_served_routes.py`; update UI ledgers for the promotion control and execution panel; close B-17/B-18 register rows as implemented; keep B-35 open unless T18 is done; refresh 005 SPEC §4 current-state matrix.
- **Acceptance:**
  - [ ] No document claims Windows Task execution unless T18 evidence exists.
  - [ ] Route inventory matches mounted routes.
- **Command:** `python3 scripts/audit_served_routes.py`; `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of the listed docs.
- **Stop:** stop if a doc would upgrade an evidence level without a recorded run.

## 005-T20 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T19
- **Cross-package prerequisites:** 019 release qualification gate G
- **Owner:** Repository owner; Task Runtime
- **Allowed paths:** `specs/005-safe-task-path/`
- **Contract:** SPECIFICATION-STANDARD lifecycle; all AC IDs in SPEC
- **Change intent:** Assemble the evidence bundle (per-task revision, command, result, OS, harness, reviewer), move the package ACCEPTED/LOCKED -> IMPLEMENTING (at first code task) -> VERIFIED, and get owner sign-off. If the owner deferred B-35 in T16, VERIFIED is scoped to the Linux row and records the Windows row as visibly UNAVAILABLE.
- **Acceptance:**
  - [ ] Every AC has passing evidence on the row it claims.
  - [ ] Owner sign-off recorded with date.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** completion table below; SPEC and WIRING status VERIFIED.
- **Stop:** stop if any AC lacks evidence on its claimed row.

## Dependency order

T01 -> T02 -> T04; T01 -> T03; T01 -> T06; T01 -> T07; T01 -> T12.
T03 + T04 -> T05. T03 + T04 + T06 + T07 -> T08 -> T09.
T05 + T08 + T09 -> T10 -> T11. T10 + T11 -> T13. T05 + T13 -> T14.
T12 + T13 + T14 -> T15 (Linux row qualified) -> T19.
T16 (owner, can be decided any time) + T02 + T15 -> T17 -> T18.
T01-T19 -> T20.

Parallel lanes after T01: storage/plan (T03), provider (T02, T04), write primitive (T06), Trust (T07), promotion (T12).

## Cross-package dependencies

| This task | Other package work | Reason |
|---|---|---|
| T01 | 011 IPC/route conventions; orchestrator migration numbering | Shared schema and next migration number |
| T04 | 011 Runtime incarnation and Linux socket row | Attestation binds to the incarnation |
| T05 | 001 session supervisor lifecycle and settlement; 002 OpenCode binding creatable | Planning session reuses the native session path |
| T06 | 003 pinned revisions and WorkspaceRoot identity | Base digest and confined root |
| T10 | 011 authenticated IPC | Route transport |
| T11 | 011 startup recovery ordering | Startup sweep placement |
| T12 | 001 committed AGENT message identity; 011 focus baseline | Promotion needs a committed message ref |
| T13 | 007 Task output Artifacts; 011 focus baseline | Output view and accessibility |
| T14 | 019 release matrix rows; 008 terminal UNAVAILABLE wording | Same B-35 reason across Area 4 and Area 6 |
| T15 | 001/002 Linux native route; 019 evidence process; 011 packaged Linux desktop | Full E2E prerequisites |
| T17 | 011 Windows named-pipe IPC | Windows Runtime identity |
| T18 | 019 Windows installer; 001 Windows OpenCode route | Windows E2E prerequisites |
| T20 | 019 gate G | Release qualification |

Downstream consumers (they depend on this package): 006 golden write admission reuses T07/T08 Trust and Attempt wiring; 008 terminal needs an admitted Environment from T04/T17; 010 Task execution claims; 015 control and delegation; 020 accept-task.

## DeepSeek Harness proposals (PROPOSED, owner review required)

Status: PROPOSED. None of the items below is an accepted task. They come from the DeepSeek Harness studies ([desktop deep-dive](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) register A-63, proposals P-31 to P-46; [plugin deep-dive](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) register A-58). Owner review is required before any item becomes a numbered task in this package. They are labelled `PROP-Pnn` so they do not collide with the stable task IDs that `.specify/scripts/validate_feature_package.py` checks. On acceptance: assign the next free task ID in this file, copy the upstream permalink, class and reuse statement into the task, and update the [reuse map](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md). Upstream pin `fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159` (MIT, Copyright (c) 2026 DeepSeek). No upstream code is imported (AGENTS.md section 7).

### Proposals owned elsewhere that touch this package (pointer only)

- **PROP-P38 (P-38) Login-shell PATH-class capture for agents launched from a Linux desktop entry** [PROPOSED, not accepted, owner review required]: row owned by package 002; class **OWNER (open; not ADOPT, ADAPT or REJECT yet)**; would touch here: T07 and T13 of package 002; package 005 Environment admission. Upstream permalinks: [src/login-shell-environment.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/login-shell-environment.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-38 and the conflict note). Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). No reuse is allowed before the owner decision.

### Plugin-architecture proposal (P-27)

- **PROP-P27 (P-27) Durable run record plus process-group liveness fence for recovery** [PROPOSED, not accepted, owner review required]
  - Class: **ADAPT** (the liveness check itself is an **ADOPT** candidate, algorithm only).
  - Upstream permalinks: [operations.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/operations.ts) (`activeRecordedRun`), [run-tree.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/run-tree.ts) (`treeAlive`, `awaitTreeGone`).
  - Study: [plugin study](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) Sections 2, 3 and 8.
  - Why: it is the reference design for durable dispatch intent, atomic rollback and liveness-fenced restart recovery. The restart sweep in package 001 (T08) is already done independently; this is a cross-check for package 005 Attempt recovery and later package 013 apply recovery.
  - Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). For the liveness probe only: If any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
  - Deferral note: package 013 is DEFERRED, so the apply-recovery half waits; the Attempt-recovery half belongs to the package 005 owner review.


## Completion evidence

For each task record, in the table below and in CURRENT-RUN.md: git revision, exact command, result (pass/fail with counts), release-matrix row (OS name, build, architecture), harness and version where used, provider versions (bwrap, systemd, kernel or Windows build), evidence level reached (CODE, SYSTEM, USER), reviewer and date.

| Task | Revision | Command | Result | Row / OS build | Harness | Level | Reviewer | Date |
|---|---|---|---|---|---|---|---|---|
| T02 | c892a567 + working-tree | `cargo check -p litecoworkd --locked`; `cargo test -p litecoworkd --locked environment_`; `cargo test -p local-environment-staging --locked`; `cargo test -p domain-environment --locked`; `cargo test -p linux-process-scope --locked provider_probe` | check passed; 64 + 24 + 22/23/9 + 5 tests passed | Windows 11 Home Single Language build 26300 x64 | None | CODE | No owner review recorded | 2026-11-11 |
