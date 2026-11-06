# API and events - Safe Task Path

## Currently served Task operations

- `POST /v1/tasks` (`createTask`): creates the durable Task envelope and initial TaskSpecRevision with an idempotency receipt. It does not start planning, an AgentSession, an Attempt or an Environment.
- `GET /v1/tasks` (`listTasks`) and `GET /v1/tasks/{task_id}` (`getTask`): Task list/detail projections.
- `POST /v1/tasks/{task_id}/spec-revisions` (`reviseTaskSpec`) and `GET` on the same path (`listTaskSpecRevisions`): objective/specification revision write and history read.
- `GET /v1/tasks/{task_id}/planning-readiness` (`getTaskPlanningReadiness`): read-only, version-checked preflight; `dispatch_available`, `planning_started`, `agent_session_started` and `plan_created` are all false.
- `GET /v1/tasks/{task_id}/plan-revisions` and `GET /v1/tasks/{task_id}/steps`: read-only plan/Step projections. The Task lead owner-handoff route `POST /v1/tasks/{task_id}/lead-agent` changes the selected lead but does not start work.
- Additional served read projections: Conversation Tasks, Task Artifacts, Task capability invocations, Task presentation/progress and verification runs. Capability grant create/revoke currently serves only bounded, expiring Task-planning Resource-read grants.

## Target-only operations

The composed route inventory does not register plan submission `POST /tasks/{taskId}/plan-revisions`, Attempt list/admission, Effect list/dispatch, step recovery, Task pause/resume/cancel/steer, or Environment provision/control. The exact method/path/operationId list is maintained in `docs/schemas/served-route-inventory.json`. Do not describe these target operations as mounted or clickable UI.

## Event contracts

Use only canonical event names and schemas from `docs/EVENTS.md` and `docs/schemas/domain-event.schema.json`. The target Task lifecycle includes `task.created.v1`, `task.spec.revised.v1`, `task.plan.revised.v1`, `step.created.v1`, `attempt.created.v1`, `lease.acquired.v1`, the applicable AgentSession lifecycle events, `effect.*`, `evidence.created.v1`, and `verification.*`. A Task record/spec event is not proof of planning, Attempt, Effect or verification.

## Admission and write ordering

Target ordering is: authenticate Task/Workspace/producer scope; compare Task and TaskSpec versions; validate current-incarnation Environment attestation and exact pinned input/root digests; commit the accepted plan or Attempt/lease/grant/invocation-intent/idempotency receipt transaction; invoke the Environment outside SQLite; reconcile the external result; persist Evidence and verification. No filesystem/process/provider call is permitted inside the SQLite transaction.

## Error contract

Reuse canonical errors from `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`, including `AGENT_UNAVAILABLE`, `STALE_TASK_VERSION`, `STALE_SPEC_REVISION`, `FORBIDDEN`, `CONFLICT`, `DEPENDENCY_UNAVAILABLE`, `STALE_FENCE` and `INTEGRITY_FAILURE` where their owner conditions apply. This DRAFT declares no new ErrorCode or API operation.
