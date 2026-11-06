# Research - Safe Task Path

**Feature:** 005 | **Checked:** 2026-11-06

## Sources

Authorities: docs/TASK-RUNTIME.md, docs/ENVIRONMENTS.md, docs/TRUST.md, docs/DELEGATION.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md §6. Reject worktree-as-sandbox. The current source/route/UI crosswalk was checked 2026-11-06.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Source, route and UI audit (2026-11-06)

Inspected `apps/litecoworkd/src/operator.rs`, `agents/planning.rs`, `task_lead_handoff_operator.rs`, `environment_provider.rs`, `environment_isolation.rs`, `attempt_process_scope.rs`, `trust_operator.rs`, `presentation_operator.rs`, `verification_runs_operator.rs`, `crates/domain-task/src/{lib.rs,planning.rs,execution.rs}`, `crates/storage-core/src/execution.rs`, `crates/storage-sqlite/src/execution.rs`, the composed route inventory, `apps/litecowork-ui/src/App.tsx`, `presentation/TaskPresentationPanel.tsx`, `artifacts/TaskArtifactOutputs.tsx`, `tasks/TaskSpecRevisionHistory.tsx`, `needs-you/TaskAttentionPage.tsx`, and `routines/RoutinesPage.tsx`.

### Served Task operations

- The composed authenticated Operator serves Task create/list/get, Task specification revision create/list, planning-readiness GET, plan-revision list GET, Step list GET and Conversation Task list GET.
- Read surfaces are also served for Task artifacts, capability invocations, presentation, progress and verification runs. These read committed records/projections; they do not admit execution.
- Suggestion acceptance can materialize a Task; the Task lead-agent owner handoff route commits a lead change. Neither action creates an Attempt or dispatches an agent.
- Task create is deliberately limited to a durable Task envelope and initial TaskSpecRevision. It uses an idempotency receipt, owner/Workspace checks and preferred lead resolution. The implementation comment explicitly says it never starts a native session or marks the Task as working.
- The Home “New task” composer creates a Task from a standalone objective, selected Coworker/default lead and pinned Resource revisions. The user can also materialize Tasks from Suggestions or Routine Manual Run. No explicit Conversation-to-Task promotion call site was found in `ConversationsPage`.

### Missing Task execution operations

- `POST /tasks/{taskId}/plan-revisions` (`submitTaskPlan`) is target-only. Only list plan revisions is served; no Operator plan-acceptance handler or UI acceptance action exists.
- Attempt list/admission, Effects list/dispatch, step recovery, Task pause/resume/cancel/steer, and Environment create/provision/suspend/resume/destroy/control operations are target-only in `served-route-inventory.json`.
- `crates/domain-task` and SQLite contain Attempt/ExecutionLease/PlanRevision admission types and transactional storage logic with tests, but repository search found no production caller of `admit_step_attempt` in `apps/litecoworkd/src`.
- `agents/planning.rs` is a read-only preflight builder. It prepares a bounded planning packet and returns dispatch blockers for unavailable isolation, unmediated native capabilities, unqualified protocol/process containment, unavailable planning context and missing session settlement. It sets `dispatch_available=false`. Its `require_dispatch()` method is explicitly unused/dead-code and always returns an error stating native Task planning dispatch is unavailable.
- `get_task_planning_readiness` is a no-side-effect diagnostic: it validates owner/Workspace and `If-Match`, calls the preflight builder, and hardcodes `dispatch_available`, `planning_started`, `agent_session_started` and `plan_created` to false. The endpoint does not start a provider or create plan/Attempt records.

### Environment, isolation, Trust and Effects

- `probe_environment_provider()` reports provider capability observations only. On Windows it reports a LocalWorkspace class and Job Object process-tree lifetime support; on Linux it reports a LocalWorkspace class only if bubblewrap is present and explicitly notes isolation is not qualified. The probe reports `budget_enforcement=Unavailable`, no checkpoint/restore and no workspace-persistent lifetime.
- `IsolationAttestation` and fail-closed comparison code model filesystem, process-tree, network and resource-limit enforcement, along with exact Workspace/Task/Runtime incarnation and root/input digests. Repository search found no production caller connecting these attestations to Task Attempt admission.
- `attempt_process_scope.rs` declares its process containment/journal staged until production admission wires exact Runtime scope evidence (`#![allow(dead_code)]`). Its tests correctly refuse to treat parent-process exit as quiescence. This is not a live Task execution scope.
- A Windows Job Object can support process-tree lifetime/termination/resource controls, but it does not by itself attest Workspace filesystem confinement, network denial or every requested resource limit. Those must be measured and enforced by the selected Environment provider, not inferred from the Job Object badge.
- `POST /capability-grants` and revoke are served, but the create route is narrowly limited to bounded, expiring Task-planning `workspace.resource.read` grants over explicit Resource refs. This is not an execution-write grant. No Effect dispatcher/reconciler is connected to an Attempt path.

### Desktop UI crosswalk

- Home is explicitly labelled “New task”. The composer saves the objective as a Task, pins selected Resource revisions and optional Coworker/lead defaults, preserves the same request identity on retry, and navigates to Work after the Task envelope is committed.
- Work lists and filters saved Tasks, paginates, opens Task details, edits objective/specification revisions and checks read-only planning readiness. It displays pinned input Resources, read-only persisted Steps, Task presentation/progress snapshots, Task artifacts and output previews.
- `TaskPresentationPanel` loads the persisted Task presentation/progress projections and a Task stream. It labels its section “Latest saved progress”; it does not invent percentage/ETA. The panel itself is not an execution service and can only reflect records that an owning service has committed.
- Task detail explicitly says the readiness check is read-only and does not start planning or an agent. The planning readiness response states planning dispatch is unavailable. No plan acceptance, Attempt start, Effect approval, Task cancel/recover, or Environment controls are rendered.
- Task output views are read-only committed Artifact projections. The UI does not show a successful file write merely because a Task record exists. A Task status badge or READY record must not be presented as proof of a running worker.

These findings are source/route/UI inspection, not an end-to-end Task USER test. No Task plan was accepted, no Attempt was admitted, no Environment was provisioned, no file was modified by a Task provider, and no Effect was independently verified.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) only for a future market-positioning decision.
- No end-to-end Task execution was run: no plan acceptance, Attempt admission, Environment provisioning, Task-provider file modification or independent Effect verification occurred.

## Unresolved questions

See SPEC section 5 blockers. Owners are named on the package README.
