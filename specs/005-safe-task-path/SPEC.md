# 005 - Safe Task Path

**ID:** 005
**Status:** DRAFT
**Owners:** Task Runtime, Environments, Trust, Storage, Desktop UI
**Updated:** 2026-11-06
**Revision:** 0.2
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Ordinary chat must not become a Task by inference. A working directory is not a sandbox. One consequential edit needs plan acceptance, isolation attestation, lease, grant and independent verification.

Observable outcome: The owner promotes one Conversation outcome into a Task, accepts an immutable plan, and one isolated Attempt edits one approved file with a receipt that survives cancel and restart.

## 2. Goals

- Create a Task explicitly from a committed Conversation outcome or a standalone objective.
- Accept one immutable PlanRevision and Steps atomically before any Attempt.
- Run one Environment provider that attests read/write roots, network, process tree and limits.
- Reconcile an ambiguous write before retry and verify the edit independently.

## 3. Non-goals

- Multi-step heterogeneous delegation beyond one edit.
- Coworker-required Tasks.
- Treating git worktree alone as isolation.
- Treating omission of LeadFailoverPolicy as DISABLED when a Coworker revision already supplies ASK or ALLOW_LISTED.

## 4. Current evidence

The active checkout contains a partial Task record/specification/read model, a read-only planning preflight, and compiled domain/storage execution primitives. It does not contain an integrated Task planning → accepted PlanRevision → Attempt → isolated Environment → mediated Effect → independent verification path. The following matrix records the current source/route/UI boundary; it is not owner acceptance or release qualification.

| Capability | Current source/route evidence | Limitation / release meaning |
|---|---|---|
| Explicit Task creation | CODE/SERVED: `POST /v1/tasks` persists a durable Task and initial TaskSpecRevision with idempotency receipt. The Home “New task” composer submits a standalone objective, optional Coworker/default lead, and pinned Resource revision refs. Suggestion acceptance and Routine Manual Run also materialize Task records. | Task creation deliberately does not start planning, an AgentSession, an Attempt or an Environment. No explicit “promote this Conversation outcome” action was found in `ConversationsPage`; current Home creation is a separate Task objective flow. |
| Task/specification read and edit | CODE/SERVED: Task list/get, `POST /v1/tasks/{task_id}/spec-revisions`, plan-revision list, Step list, Conversation Task list, Task artifacts, presentation/progress, capability-invocation and verification-run read views | These are durable records and read projections. Reading existing Steps/Attempt/Evidence is not an Attempt admission path. |
| Task lead selection | CODE/SERVED: Task create resolves explicit preferred binding, then selected Coworker default, then Workspace default; missing lead fails `AGENT_UNAVAILABLE`. Owner Task lead handoff is served via `POST /v1/tasks/{task_id}/lead-agent` and commits a Task lead change. | Changing the lead does not start planning or create an Attempt. There is no substitute-agent execution fallback. |
| Planning readiness | CODE/SERVED: `GET /v1/tasks/{task_id}/planning-readiness` validates Workspace ownership and `If-Match`, prepares a bounded internal planning packet, and returns a read-only projection | Response hardcodes `dispatch_available=false`, `planning_started=false`, `agent_session_started=false`, `plan_created=false`. Current blockers include isolation, unmediated native capabilities, protocol/process qualification, planning context materialization and session settlement. No provider is invoked. |
| Plan acceptance | Only `GET /v1/tasks/{task_id}/plan-revisions` is served; `POST /v1/tasks/{task_id}/plan-revisions` (`submitTaskPlan`) is target-only | No authenticated Operator plan-acceptance handler or user acceptance flow. `PlanRevision` and `Step` types/storage exist but are not produced by the current Task UI flow. |
| Attempt admission and recovery | Attempt/lease admission state types and SQLite storage methods exist in domain/storage crates with unit tests | No Task daemon route or production caller of `admit_step_attempt` was found. Attempt list, Effect list, recovery, pause/resume/cancel/steer and environment control routes are target-only. `attempt_process_scope.rs` explicitly says its containment/journal remains staged and is `allow(dead_code)`. |
| Environment provider | CODE: `probe_environment_provider` reports platform/provider observations. Windows reports a local workspace class and Job Object process-tree lifetime capability; Linux only reports a local workspace class if bubblewrap exists and explicitly notes isolation is not qualified. | Provider probe is not provisioning or Task invocation. Budget enforcement is `Unavailable`; checkpoint/restore and persistent Workspace lifetime are false. No Task API provisions or attaches an Environment. |
| Isolation/quiescence evidence | CODE: `IsolationAttestation`, enforcement dimensions and fail-closed admission checks exist in `environment_isolation.rs`; process-scope journal/proof types exist | Search found no production caller wiring those attestations into Task Attempt admission or recovery. A Windows Job Object is process-tree containment evidence; by itself it does not prove filesystem-root confinement, network denial or resource-budget enforcement. |
| Trust and grants | CODE/SERVED: `POST /v1/capability-grants` and revoke route accept bounded, expiring Task-planning `workspace.resource.read` grants scoped to exact Resource refs | These are read grants for planning context, not an execution grant for the proposed file write. The Trust evaluator is not connected to a live Attempt/tool invocation path. |
| Effects and verification | CODE/SERVED read views for Task verification runs, Artifacts, capability invocations and presentation/progress | No production Task Effect dispatcher/reconciler/verification pipeline is connected to the Attempt path. No file write is issued through an Environment by this feature. |
| Desktop Work UI | `App.tsx` Home composer explicitly saves a Task; Work lists Tasks, opens detail, edits objective revisions, checks planning readiness, displays pinned inputs, read-only Steps, presentation/progress snapshots and committed outputs | No plan acceptance, Attempt start, cancellation, retry/recover, approval, Environment detail or real execution controls. Task presentation reads saved records and does not prove an execution worker is live. |

Feature 005 remains DRAFT. The code has useful domain/storage primitives and a truthful read-only readiness screen, but the V1 outcome of one independently verified isolated file edit is not implemented.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Task creation does not imply Step, Attempt, lease or Environment (TASK-RUNTIME).
- No persisted Task DRAFT; unsent composer stays in the Operator.
- Unavailable lead fails with AGENT_UNAVAILABLE and does not substitute another binding.
- Unknown external effect is reconciled before retry (release contract §6).
- Grants are explicit and scoped.

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- B-17: the first V1 Environment provider must be selected for the declared GA OS and exact Windows build/provider version (or another explicitly selected OS). Decide whether the first target is the Windows Job Object-backed local provider or a separately qualified container/OS sandbox provider. The current `probe_environment_provider()` output is discovery evidence only: Windows reports process-tree lifetime capability; it does not prove filesystem confinement, network enforcement or budget enforcement. On Linux, bubblewrap presence is not a qualification result.
- B-18: choose the one write primitive for the first vertical slice: atomic replacement of one exact approved file, or a bounded patch against a pinned base revision. The choice must define the input/base digest, maximum bytes, allowed path/root, temporary-file semantics, atomic commit, rollback/reconciliation behavior, and independent post-write verifier. Do not accept arbitrary shell commands or general workspace writes as an equivalent first slice.

### Blockers

- B-17: Product, Environments and Trust must name the first GA OS/build, exact Environment provider/version, and evidence for all enforced isolation dimensions: filesystem roots, process-tree lifetime/quiescence, network policy and resource limits. Provider capability probes and Job Object presence alone do not close this blocker.
- B-18: Product, Environments and Trust must accept one exact file-write class and its digest/path/size/atomicity/recovery/verifier contract. No implementation should infer the allowed write class from an available OS API.

## 6. High-level design

UI promotes work with an explicit command. TaskService commits Task and TaskSpecRevision. Planning uses a native lead session. Acceptance commits PlanRevision. Attempt admission is a separate transaction that never contains the filesystem call. Effect reconciler and verifier run after the external write returns.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

CreateTaskRequest fields follow TASK-RUNTIME. Plan acceptance is atomic. Attempt, lease, grant, invocation intent and idempotency receipt commit together before the Environment is invoked. The write happens outside SQLite. Reconciliation updates Effect and Evidence after the external result is known.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Workspace and owner checks on every command. Environment cannot widen roots. Network default deny unless the plan's grant allows it. Secret material is not placed in the TaskPacket.

## 11. Errors and recovery

Cancel fences the lease. Timeout-after-dispatch reconciles. Restart reloads Task state and does not reissue a write whose receipt is ambiguous. Stale plan version conflicts.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

One Environment provider. One file edit. LeadFailoverPolicy is pinned from an explicit Task request, otherwise the selected Coworker revision, otherwise DISABLED with empty triggers, empty fallbacks, and max_lead_changes 0.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner sends an ordinary Conversation message, THE SYSTEM SHALL NOT create a Task.
- **AC-002:** WHEN the owner explicitly promotes an outcome, THE SYSTEM SHALL commit Task and TaskSpecRevision without an Attempt.
- **AC-003:** IF the preferred lead binding is unavailable, THEN THE SYSTEM SHALL return AGENT_UNAVAILABLE and create no Task.
- **AC-004:** WHEN the owner accepts a plan, THE SYSTEM SHALL commit an immutable PlanRevision and Steps before any Effect.
- **AC-005:** WHEN the Attempt runs, THE SYSTEM SHALL invoke the Environment only after lease, grant and idempotency receipt are committed, and the external call SHALL stay outside that transaction.
- **AC-006:** IF the external write result is unknown, THEN THE SYSTEM SHALL reconcile before any retry.
- **AC-007:** WHEN the Runtime restarts after cancel or timeout, THE SYSTEM SHALL show a durable receipt and SHALL NOT report success without independent verification.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/005-safe-task-path
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not run browser automation.
- Do not install connectors.
- Do not enable standing automations.

## 18. Assumptions and open questions

- First Environment provider is undecided.
- Verification may be a hash compare of the approved file for this slice.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 005-T01, 005-T02, 005-T03, 005-T04.

## 7a. Owner facts incorporated and implementation crosswalk checked 2026-11-06

Read from TASK-RUNTIME, ENVIRONMENTS, DELEGATION, and STATE-MACHINES.

- If the lead binding is unavailable, creation returns AGENT_UNAVAILABLE, creates no Task or turn, does not fall through to a lower-precedence binding, and leaves the unsent composer draft.
- Until a current-incarnation IsolationAttestation is revalidated, planner admission returns DEPENDENCY_UNAVAILABLE and creates no session, PlanRevision, Step, Attempt, or Environment.
- Plan acceptance uses If-Match and the current TaskSpec head. A Task-version mismatch returns 409 STALE_TASK_VERSION. A spec mismatch returns 409 STALE_SPEC_REVISION. A producer who is no longer the active TASK_PLANNING session or leased lead is 403 FORBIDDEN or 409 CONFLICT.
- The Runtime attaches an Environment before the transaction that commits Attempt, ExecutionLease, grant, and receipt, starts the AgentSession only after that commit, and runs the file mutation outside SQLite.
- Admission accepts only a Runtime-held IsolationAttestation whose filesystem, process-tree, network, and resource-limit fields are ENFORCED. UNKNOWN, NOT_ENFORCED, or a caller-supplied attestation fails closed.
- Releasing a writer lease or destroying the Environment requires QuiescenceObservation QUIESCENT. UNKNOWN, timeout, or restart leaves the Attempt fenced.
- Cancel moves the Task to CANCEL_REQUESTED, reconciles STARTED, ACKNOWLEDGED, and AMBIGUOUS Effects, and commits CANCELLED only after attempts, invocations, verification, and Effects have settled.
- There is no Task retry command. Continuation is recover_step, which creates a new Attempt id. A changed placement plan returns STALE_VERSION.
- A new Runtime incarnation cannot renew the prior lease. Recovery commits that lease EXPIRED or REVOKED and the Attempt ABANDONED. A stale fence returns STALE_FENCE.
- The provider exposes the pinned revision read-only, writes through a revalidated handle and atomic replace, and returns INTEGRITY_FAILURE when the observed digest differs.



## 7b. Current source and UI boundary checked 2026-11-06

- Task create/spec edit and planning-readiness read are served; plan submission, Attempt admission/listing, Effect list/dispatch, recovery, Task pause/resume/cancel/steer and Environment provision/control remain target-only.
- The current Task composer is a standalone Home “New task” form. It creates a Task envelope and pins selected Coworker/default lead plus Resource revision refs. It is not an in-Conversation promotion control; ordinary Conversation messages do not create Tasks.
- Task planning readiness only builds an internal bounded preflight packet. It hardcodes dispatch/planning/session/plan booleans false. `LocalPlanningPreflight::require_dispatch()` remains unused and always refuses dispatch. No production `admit_step_attempt` call site exists in the daemon.
- `probe_environment_provider()` reports provider capabilities, not an attached Environment. On Windows it reports a LocalWorkspace class and Job Object process-tree capability; on Linux it reports a local class only when bubblewrap exists and explicitly says isolation is not qualified. It reports no budget enforcement, checkpoint/restore, or workspace-persistent lifetime.
- IsolationAttestation and quiescence proof types exist with strict comparisons, but they are not connected to production Task admission/recovery. The attempt process-scope journal is compiled as staged/dead code until exact Runtime scope evidence is wired.
- The only currently served capability grant is the bounded Task-planning Resource-read grant. It does not authorize a file write. Trust evaluation and Effect reconciliation are not connected to a live Attempt execution path.
- The desktop Work surface renders persisted Task envelope/spec/plan/Step/Artifact/presentation records and read-only planning readiness. It does not provide plan acceptance, Attempt start, Effect approval, cancel/recover, or Environment control. Task presentation is not a live execution proof.
- The current Windows Job Object process-tree observation is not an attestation of filesystem root enforcement, network deny or every resource limit. Do not show it as a full Environment isolation badge.
## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T02 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T03 | AC-007 |
