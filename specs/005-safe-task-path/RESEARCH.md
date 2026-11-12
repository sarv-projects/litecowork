# Research - Safe Task Path

**Feature:** 005 | **Checked:** 2026-11-06; source crosswalk re-checked 2026-11-12. The older pinned rival-source review remains dated 2026-11-06.

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

## Sources

Authorities: docs/TASK-RUNTIME.md, docs/ENVIRONMENTS.md, docs/TRUST.md, docs/DELEGATION.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md §6. Reject worktree-as-sandbox. The original source/route/UI crosswalk below is historical; current provider behavior is refreshed in the 2026-11-12 note below.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Borrowable-design sources (pinned)

| Source | Exact pinned URL | License | What informs the safe task path | Verdict |
|---|---|---|---|---|
| OpenWork `packages/automations` engine/ports doctrine (`engine.ts`, `ports.ts`, `contracts.ts`) | https://github.com/different-ai/openwork/blob/5493b18a6af64d56454658a53da00eabbc6e033d/packages/automations/src/engine.ts | MIT except `/ee` (Enterprise Edition License) | Receipts/events/results stay readable after the admitting process stops. This supports the restart-reload-receipt and independent-verification requirement (AC-007) and the reconcile-before-retry doctrine (AC-006). | Prototype the durable-readable engine/ports pattern against TASK-RUNTIME/FAILURE-RECOVERY; do not vendor the runtime or import `/ee` paths. |
| OpenCowork agent-runner loop guard (`src/main/agent/agent-runner-loop-guard.ts`) | https://github.com/opencoworkai/open-cowork/blob/e94ee2c50239e64ba0e18921e339d49b93dae31f/src/main/agent/agent-runner-loop-guard.ts | MIT at checked root LICENSE | Non-progress/loop guarding informs bounded Attempt admission and the refusal to retry blindly on an ambiguous result. | Prototype the guard logic; LiteCowork keeps TRUST/TASK-RUNTIME as the admission authority, not the loop guard. |
| OpenCowork path guard + VM bridges (`src/main/sandbox/path-guard.ts`, `lima-bridge.ts`, `wsl-bridge.ts`) | https://github.com/opencoworkai/open-cowork/blob/e94ee2c50239e64ba0e18921e339d49b93dae31f/src/main/sandbox/path-guard.ts | MIT at checked root LICENSE | Path-traversal/zip-slip guarding and VM isolation map directly to the ENFORCED filesystem-root dimension of `IsolationAttestation` and the confined-path write (AC-008/AC-009). | Prototype the path-guard checks against ENVIRONMENTS attestation; do not adopt the Electron sandbox as a second authority. |

No OpenWork or OpenCowork code is imported; all three are prototype/reference inputs that reinforce the isolation-attestation and reconcile-before-retry contracts this slice already requires.

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

### Environment, isolation, Trust and Effects (historical source snapshot, 2026-11-06)

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

### Current source correction (2026-11-12)

The original crosswalk above is retained as dated research history and must not be used as current provider status. `apps/litecoworkd/src/environment_provider.rs` now reports no Windows `LocalWorkspace` class or Task lifetime; planning readiness returns `WINDOWS_ENVIRONMENT_PROVIDER_UNAVAILABLE`. The Windows Job Object code manages process-tree lifetime and is not an Environment or filesystem/network boundary. On Linux, the daemon uses `linux-process-scope::probe_local_provider`; a trusted Bubblewrap candidate enables only a capability observation, not provisioning, attestation or Task execution. Current route/UI/source evidence is recorded in `specs/005-safe-task-path/SPEC.md` §4/§7b, `TASKS.md` T02/T14 and `docs/IMPLEMENTATION-STATUS.md`.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) only for a future market-positioning decision.
- No end-to-end Task execution was run: no plan acceptance, Attempt admission, Environment provisioning, Task-provider file modification or independent Effect verification occurred.

## Windows provider research input (B-35, PROPOSED)

- AppContainer gives per-profile filesystem and object-namespace confinement keyed by a package SID; resources must be explicitly ACL-granted to that SID.
- A restricted token removes privileges and sets user and administrator groups deny-only.
- A Job Object gives process-tree lifetime (KILL_ON_JOB_CLOSE), active-process, memory and CPU limits. It confines nothing on the filesystem or network by itself (B-09 caveat).
- Windows Filtering Platform filters can block outbound traffic for an AppContainer package SID.
- Only the combination, with each dimension observed after launch, can satisfy the four-dimension attestation. Exact Windows builds, required privileges for WFP filter install and cleanup after crash are open items for the owner decision task.

## Unresolved questions

- B-35 (Windows Environment provider) is PROPOSED and needs the owner.
- The Linux-row planning harness and version (SPEC §18).
- Whether a broker-only connector Step needs an admitted Environment (SPEC §18, shared with 006).

## DeepSeek Harness proposals (PROPOSED, owner review required)

Research note, not a decision. Source: [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159) at the pinned SHA (MIT, Copyright (c) 2026 DeepSeek). Local read-only clone `repo_take/deepseek-harness` is gitignored and never staged. Upstream paths below were checked to exist at that SHA on 2026-11-10.

Read-depth limits from the study (do not over-trust): `scripts/` about 58% read, tests about 5% read, installer UI text partly unread. See [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 0 and 15 before turning any item into a design.

- **P-38 Login-shell PATH-class capture for agents launched from a Linux desktop entry.** Class **OWNER (open; not ADOPT, ADAPT or REJECT yet)**. Upstream: [src/login-shell-environment.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/login-shell-environment.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-38 and the conflict note). Lesson: CONFLICT, stays open: capturing the login-shell environment merges ambient variables over the inherited environment, which conflicts with environment scrubbing (A-58, Section 9 of the desktop study and the Env scrubbing lesson in the plugin study) and with the T07 and T13 environment rules of package 002. Only a PATH-class allowlist could be considered, and only after an owner decision. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). No reuse is allowed before the owner decision. Where it applies: T07 and T13 of package 002; package 005 Environment admission. Deferral: Blocked on the owner decision. Then a Linux desktop-entry launch test. **Open conflict with environment scrubbing (A-58); owner decision required.**

Plugin-architecture lessons (see [plugin study](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) Section 8):
  - Capability seam: Definition, Provider, Consumer (ADR P-20): [docs/capability-seams.md](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/docs/capability-seams.md). Class **ADAPT**. Environments and Effects as consumers of a provider seam. Reuse: pattern only, reimplement; no upstream code imported.
  - Durable run record plus process-group liveness fence (P-27): [packages/boot/plugin-manager/src/run-tree.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/run-tree.ts). Class **ADAPT**. Attempt recovery cross-check. Reuse: pattern only, reimplement; no upstream code imported.
  - Atomic restore on a rejected mutation: [packages/boot/plugin-manager/src/operations.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/operations.ts). Class **ADAPT**. Effect rollback cross-check. Reuse: pattern only, reimplement; no upstream code imported.

