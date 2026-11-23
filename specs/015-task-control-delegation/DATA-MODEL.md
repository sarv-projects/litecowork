# Data Model - Task Control and Heterogeneous Delegation

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation. No migration ships until the package is accepted and TASKS T02 promotes it.

Canonical entity definitions live in [docs/DATA-MODEL.md](../../docs/DATA-MODEL.md), [docs/STATE-MACHINES.md](../../docs/STATE-MACHINES.md), [docs/TASK-RUNTIME.md](../../docs/TASK-RUNTIME.md) and [docs/DELEGATION.md](../../docs/DELEGATION.md). This file maps them to this feature and names the new records.

## 1. Existing durable records (reused)

| Record | Location | Fields used here |
|---|---|---|
| `tasks` | `docs/schemas/sqlite-v1.sql` | status (text), version, lead agent binding, current spec revision, Coworker revision pin |
| `task_spec_revisions` | `sqlite-v1.sql`, delegation snapshot column in `sqlite-v28.sql` | immutable TaskSpec fields, sibling `parent_revisions_json`, non-null resolved delegation snapshot for post-v28 admissions; exact read schema is `ResolvedDelegationPolicySnapshot` |
| `plan_revisions`, `steps` | `sqlite-v1.sql` | accepted plan pin, Step status READY |
| `attempts` | `sqlite-v1.sql` | `parent_attempt_id`, `delegation_profile_id`, `delegation_profile_revision`, `execution_lease_id`, state (`AttemptState`) |
| `execution_leases` | `sqlite-v1.sql` | `epoch` with `UNIQUE(step_id, epoch)` and immutable identity trigger |
| `agent_sessions`, Runtime and Environment identities | canonical owner schemas | Session is distinct from Attempt, Runtime incarnation and Environment; opaque native handles stay outside public records |
| `delegation_profiles`, `delegation_profile_revisions` | `sqlite-v1.sql` | status, `max_concurrency`, `max_host_delegation_depth`, policies |
| `coworker_revisions` | `sqlite-v1.sql`, `sqlite-v28.sql` | `enabled_delegation_profile_ids_json`, `delegation_budget_policy_json` |
| `delegation_results` | `sqlite-v24.sql` | ResultEnvelope per child Attempt |
| `verification_runs` | `sqlite-v1.sql` | status PENDING, RUNNING, PASSED, FAILED, INCONCLUSIVE |
| `event_journal`, `request_dedup` | `sqlite-v1.sql` | events and idempotency receipts |

### Resolved delegation policy snapshot (USER-DELEGATED direction, T22)

Each post-v28 TaskSpec revision carries the canonical `ResolvedDelegationPolicySnapshot`: policy source, Workspace settings revision, Coworker ID/revision or null, effective strategy, native child-model policy, exact selected profile revisions with each `worker_model_selection_policy`, and `resolved_at`. Workspace settings form the ceiling even for Coworker overrides. The persisted value is server-resolved; a Task caller cannot submit authority or choose historical revisions. The Task's immutable Coworker origin revision remains the source for that Task; a later Coworker head does not replace it. An explicit TaskSpec revision re-resolves the snapshot against that fixed origin and the current Workspace settings head; existing revisions and Attempts remain pinned.

The canonical OpenAPI schema already defines this closed shape. Pre-v28 TaskSpec rows may remain null and render as legacy/unavailable. Migration v34 upgrades only non-null reduced v28 snapshots using the immutable Task, Coworker, Workspace settings and profile revision records; it never reconstructs from current mutable heads. Feature 015 migration v43 remains separate and owns Task-control rows.

## 2. New records (accepted target after T02, not yet in source)

| Record | Key | Fields | Rules |
|---|---|---|---|
| `task_control_requests` | `control_request_id` | workspace_id, task_id, command (PAUSE, RESUME, STEER, CANCEL), class (for STEER), requested_by, expected_task_version, idempotency key digest, state (ACCEPTED, WAITING_QUIESCENCE, SETTLED, SUPERSEDED, FAILED), outcome code, created_at, settled_at | One nonterminal PAUSE or CANCEL per Task; CANCEL may supersede PAUSE; rows are append-then-settle, never deleted while the Task exists |
| `task_resume_packets` | `resume_packet_id` | task_id, task_spec_revision, current_plan_revision, source_attempt_id, packet digest, packet blob ref, generated_at | Immutable; contains no grant, lease, SecretLease or handle; digest checked on resume |
| escalation `on_exhaustion` | owner-defined location in the delegation packet/profile contract | TARGET-not-yet-wired enum `RETURN_TO_LEAD` or `NEEDS_YOU`; no separate Feature 015 column is assumed | Optional legacy field remains valid and resolves to `NEEDS_YOU` when the owning dispatcher is implemented (accepted A-37/B-39); storage location and producer remain open under SPEC O-08 |

## 3. Lifecycles

- **Task (control subset):** `READY|RUNNING|WAITING_USER|BLOCKED|VERIFYING -> PAUSE_REQUESTED -> PAUSED -> (prior state)`; any nonterminal including PAUSE_REQUESTED and PAUSED `-> CANCEL_REQUESTED -> CANCELLED`. No STEER state: steering produces spec revisions or notes.
- **Attempt:** `CREATED -> PREPARING -> RUNNING -> ... -> COMPLETED|FAILED|ABANDONED|CANCEL_REQUESTED -> CANCELLED`. Replacement execution and ordinary interrupted continuation use a new Attempt id with a higher lease epoch. Only the canonical confirmed `INPUT_REQUIRED` same-Attempt continuation may retain the Attempt on the same Runtime incarnation and Environment, after Effect reconciliation, with a fresh AgentSession and higher lease; it is unavailable after restart, Runtime handoff, Environment replacement or plan invalidation.
- **Control request:** `ACCEPTED -> WAITING_QUIESCENCE -> SETTLED | SUPERSEDED | FAILED`.
- **VerificationRun:** `PENDING -> RUNNING -> PASSED | FAILED | INCONCLUSIVE`; none starts while the Task is PAUSE_REQUESTED or CANCEL_REQUESTED.

## 4. Identity and fence invariants

1. A ResultEnvelope, Effect receipt or completion claim is accepted only if its Attempt id is current for the Step, its lease epoch equals the live lease epoch and its `revision_at_start` matches the pinned revision; otherwise it is stored as a fenced observation.
2. Child depth is computed from immutable `parent_attempt_id` links; V1 depth is 1 (B-39).
3. `delegation_profile_id` and `delegation_profile_revision` are set together or not at all.
4. The verifier Attempt's AgentBinding differs from the producing worker Attempt's AgentBinding (checked in storage).

## 5. Atomic writes

| Operation | Records in one transaction |
|---|---|
| Request pause | Task PAUSE_REQUESTED, control request, `task.pause.requested.v1`, receipt |
| Confirm paused | Task PAUSED, ResumePacket row, lease releases, control request SETTLED, `task.paused.v1` |
| Resume | Task prior state, normally interrupted Attempts ABANDONED (or eligible INPUT_REQUIRED Attempt renewed under the narrow TASK-RUNTIME exception), control request SETTLED, `task.resumed.v1`, receipt |
| Steer (spec change) | TaskSpecRevision(n+1), control request, `task.steering.recorded.v1`, receipt |
| Child admission | child Attempt CREATED, Step binding, ExecutionLease, budget reservation, `task.child_attempt.admitted.v1`, receipt |
| Fence late result | fenced observation, `task.child_result.fenced.v1` |

No transaction contains a native, Environment or network call.

## 6. Privacy, retention and deletion

Steering text is user content stored with the Task and deleted with it; it is never copied into events or logs (events carry ids and class only). ResumePackets follow the Task's retention. Fenced observations keep ids, digests and the denial class only.

## 7. Migration and compatibility

Additive migration only. The allocated Feature 015 migration is v43 and covers control requests and resume packets; T22 separately uses global migration v34 to complete and constrain the already-existing v28 TaskSpec snapshot. Neither migration assumes a new `on_exhaustion` column because the DELEGATION owner owns its contract/storage shape. Existing Tasks have no control requests and no resume packets. Stored statuses are parsed by `TaskStatus`; an unknown value fails closed. Legacy delegation packets without `on_exhaustion` remain valid and the accepted field stays TARGET-not-yet-wired until the owning dispatcher is implemented (A-37).
