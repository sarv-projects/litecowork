# API and events - Safe Task Path

**Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 IN_REVIEW with owner-directed implementation authorization; not a claim of implementation or qualification.

## Currently served Task operations

- `POST /v1/tasks` (`createTask`): creates the durable Task envelope and initial TaskSpecRevision with an idempotency receipt. It does not start planning, an AgentSession, an Attempt or an Environment.
- `GET /v1/tasks` (`listTasks`) and `GET /v1/tasks/{task_id}` (`getTask`): Task list/detail projections.
- `POST /v1/tasks/{task_id}/spec-revisions` (`reviseTaskSpec`) and `GET` on the same path (`listTaskSpecRevisions`): objective/specification revision write and history read.
- `GET /v1/tasks/{task_id}/planning-readiness` (`getTaskPlanningReadiness`): read-only, version-checked preflight; `dispatch_available`, `planning_started`, `agent_session_started` and `plan_created` are all false.
- `GET /v1/tasks/{task_id}/plan-revisions` and `GET /v1/tasks/{task_id}/steps`: read-only plan/Step projections. The Task lead owner-handoff route `POST /v1/tasks/{task_id}/lead-agent` changes the selected lead but does not start work.
- Additional served read projections: Conversation Tasks, Task Artifacts, Task capability invocations, Task presentation/progress and verification runs. Capability grant create/revoke currently serves only bounded, expiring Task-planning Resource-read grants.

## Target-only operations

The composed route inventory does not register plan submission `POST /tasks/{taskId}/plan-revisions`, Attempt list/admission, file-write proposal preview/apply, Effect list/dispatch, step recovery, Task pause/resume/cancel/steer, or Environment provision/control. The exact method/path/operationId list is maintained in `docs/schemas/served-route-inventory.json`. Do not describe these target operations as mounted or clickable UI.

## Event contracts

Use only canonical event names and schemas from `docs/EVENTS.md` and `docs/schemas/domain-event.schema.json`. The target Task lifecycle includes `task.created.v1`, `task.spec.revised.v1`, `task.plan.revised.v1`, `step.created.v1`, `attempt.created.v1`, `lease.acquired.v1`, the applicable AgentSession lifecycle events, `effect.*`, `evidence.created.v1`, and `verification.*`. A Task record/spec event is not proof of planning, Attempt, Effect or verification.

## Admission and write ordering

Revision 1.1 ordering is: authenticate Task/Workspace/producer scope and compare Task/TaskSpec/Plan versions; durably write a private Runtime preparation intent; provision and attest a Task-scoped read-only planning Environment or fresh ATTEMPT_PRIVATE execution Environment using a trusted idle supervisor that probes and holds the exact live scope without starting a native harness, credentials, tools or user code; atomically persist Environment metadata with the appropriate AgentSession reservation and, for execution, Attempt/Step/lease; resolve the admission receipt; then use a one-time private handoff to launch the native provider inside that same held scope. An ambiguous storage result is resolved by the original idempotency receipt before launch or cleanup. Startup recovery reconciles journal entries against exact storage receipts and provider/process identity; unbound cleanup requires observed quiescence, while unknown evidence blocks replacement admission. After the worker process tree is QUIESCENT, a Runtime-private provider channel returns only the accepted Step's bounded candidate bytes. The Runtime computes the digest and creates an immutable managed ResourceRevision through the existing Resource lifecycle, setting `provenance.created_by_attempt`; exact Attempt/Step replay returns the same Resource receipt and changed content conflicts. This internal capture is TARGET only and is not an HTTP route. ACP `fs/write_text_file` is not repurposed because the existing Client handler performs a Workspace write. A closed FileReplacementTarget in `Effect.target_json` pins WorkspaceRoot, normalized relative path, base digest, exact ResourceRef/digest/size and plan-action digest. Work previews that exact revision. Before a file mutation, the existing atomic mediated Invocation dispatch transaction rechecks the current active Attempt/session and lease fence, grant, activation, Trust decision, Effect/action digests and Approval; it commits Invocation DISPATCHED, Effect STARTED, ApprovalUse, events, snapshots and receipt. The external write runs after commit and outside SQLite. Reconcile an unknown result before retry; persist Evidence and verification at their owner transaction boundaries.

Target-only Attempt admission operation: `POST /v1/tasks/{task_id}/attempts` (`POST /tasks/{taskId}/attempts` in OpenAPI), with authenticated `X-Workspace-ID`, required `If-Match` Task version, and required `Idempotency-Key`. The closed body is `{ plan_revision: u64, step_id: string }`. The service resolves the current Task lead, Runtime and other owner-controlled identities; the caller cannot supply an AgentBinding, Environment, attestation, grant, lease, provider handle or transcript. Before the admission transaction, Runtime journals a private preparation intent, creates a fresh ATTEMPT_PRIVATE scope with a trusted idle supervisor, and derives attestation from probes in that exact held scope. The transaction rechecks Workspace, Task version/status, accepted plan and READY Step, current Runtime incarnation and idempotency digest, then commits public Environment metadata, Attempt, Step binding, lease, STARTING AgentSession and receipt atomically. A fresh request returns 201; exact replay returns the same Attempt/receipt (200). Only a resolved receipt permits the one-time native-harness launch handoff. If storage may have committed, resolve the original receipt before any launch or destruction; an unbound scope is destroyed only after exact provider identity and quiescence are observed. This operation is TARGET only; it has no served Operator route or UI dispatch yet.

Public requests never carry `IsolationAttestation`, Environment handles, provider observations, CapabilityGrant or Trust decisions. The internal `CapabilityGrantStore::issue_attempt_file_write_grant` operation accepts only `FileWriteScopeRequest` (Attempt/Step selectors, root/path/base/size and exact pinned candidate ResourceRef); it is not an HTTP route or a replacement for `create_read_grant`. Storage derives Task/Plan identity from the admitted Attempt and atomically rechecks the current accepted Step, active session/current Runtime incarnation/live lease, root and managed candidate whose `provenance.created_by_attempt` matches that same Attempt before recording the short-lived `CORE_BUILTIN` grant with the existing `capability.grant.created.v1` event and receipt. It consumes no ApprovalUse. Proposal capture, Resource creation, grant issue, Approval resolution, Attempt admission and mediated dispatch have distinct replay identities and crash boundaries. The revision 1.1 contract remains TARGET/IN_REVIEW; the new internal storage contract has CODE coverage only and no production caller until T08.

### Internal candidate-capture interface (TARGET, not a public route)

`TaskFileProposalService::capture(attempt_id, step_id)` is invoked only by the Runtime's
post-quiescence Attempt lifecycle. The provider side derives the exact target and byte cap
from the accepted Step and the current Attempt-private Environment, then returns candidate
bytes through a private Runtime/Environment/incarnation-bound channel. The service accepts
no host path, candidate ResourceRef, digest, plan-action digest, grant, Approval or
attestation from the worker. It verifies the identity binding and cap, computes SHA-256,
and calls the existing Resource creation transaction with `provenance.created_by_attempt`.
The output is a pinned ResourceRef plus Resource receipt; repeated Attempt/Step capture of
the same bytes replays that receipt, while changed bytes conflict. This is an internal
service interface and reuses `resource.created.v1`, the Resource aggregate and managed
encrypted content store. It does not create an Effect or write a WorkspaceRoot. If the
provider channel or quiescence proof is unavailable, the call returns a typed unavailable
result and the approval/dispatch path remains closed.

For the Linux Bubblewrap path, the current design proposal uses the packaged scope-gate
binary's explicit `--attempt-runner` mode: it launches the harness with native stdin/stdout intact, captures harness stderr as log
frames, waits for the harness and reparented descendants, then reads only the fixed
`/workspace/outputs/proposal` file and emits one bounded versioned candidate frame over the
scope stderr pipe. The Runtime may parse only the bounded frame while draining the pipe; it
does not return bytes to `TaskFileProposalService` until recursive cgroup-v2 `populated 0`
is observed and the launcher is reaped. Worker-supplied paths, ResourceRefs, digests and
scope identities are never accepted. This adds no HTTP operation, event or durable record.
Lower-level writer/parser code now exists in `crates/linux-process-scope/src/output_channel.rs`,
and the daemon contains a private launch/capture-session helper that owns the reader and
enforces post-quiescence return in code. Three fake-scope tests cover the boundary. A
separate opt-in WSL test starts the built runner as PID 1 under direct Bubblewrap, blocks
child frame forgery, and waits for a delayed descendant; it bypasses `ManagedScope` and
systemd/cgroup supervision. No production Environment/Task Runtime caller invokes the
helper, and no Resource-creation transaction was exercised. This remains CODE-only helper
and isolated runner evidence; the internal interface above remains TARGET, not a served
operation or an integrated journey. The private frame format is described in Feature 005 SPEC under
“Linux candidate export inside the Environment”; it is not a second public API/event
schema.

## Error contract

Reuse canonical errors from `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`, including `AGENT_UNAVAILABLE`, `STALE_TASK_VERSION`, `STALE_SPEC_REVISION`, `FORBIDDEN`, `CONFLICT`, `DEPENDENCY_UNAVAILABLE`, `STALE_FENCE` and `INTEGRITY_FAILURE` where their owner conditions apply. This package declares no new operation outside the OpenAPI target set; task T01 promotes the accepted error codes and request bodies (accepted target interfaces, not yet in source). Conversation promotion reuses `createTask` with `conversation_id` and `source_message_refs`.
