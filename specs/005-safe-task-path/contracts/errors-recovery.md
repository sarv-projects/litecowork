# Errors and recovery - Safe Task Path

## Target recovery rules

- A Task-create retry with the exact same idempotency identity/payload returns the original Task receipt. Reusing the same request identity with a different payload is a conflict.
- A stale Task aggregate or TaskSpec head blocks plan acceptance. A producer who is no longer the authorized planning session/leased lead cannot commit a plan.
- Plan acceptance is distinct from Task creation and Attempt admission. A Task record, readiness preflight or READY status does not start execution.
- Isolation attestation must be from the current Runtime incarnation and match the exact Environment, Workspace, Task, normalized policy, pinned input set and readable/writable roots. Any UNKNOWN/NOT_ENFORCED enforcement field fails closed.
- Attempt/lease/grant/invocation intent/receipt must commit before an external write. The Environment call runs outside SQLite. An unknown external result is reconciled against the exact target digest before any retry.
- Cancellation fences late work. Writer lease release and Environment destruction require a positive QUIESCENT observation bound to the exact Runtime incarnation and process scope. Unknown quiescence leaves the Attempt fenced.
- Runtime restart must recover durable receipts and reconcile unfinished Effects. A new Runtime incarnation cannot renew an old lease. An ambiguous write is never automatically reissued.
- Verification success requires independently generated Evidence pinned to the exact TaskSpec/plan/write subject. A worker response or process exit is not verification.

## Current implementation gaps

- Task create/spec revision are served; plan submission and Attempt admission are not served.
- The planning preflight always reports dispatch unavailable and its `require_dispatch()` always fails closed.
- Environment provider detection is not provisioning. Windows Job Object support is only process-tree evidence; Linux bubblewrap presence is not attestation. Budget enforcement is unavailable.
- IsolationAttestation and QuiescenceObservation types/checks are not wired into a production Task Attempt path; the process-scope journal is staged/dead code.
- Capability grants are presently limited to Task-planning Resource reads. There is no admitted write grant, Effect dispatcher/reconciler or independent write-verification pipeline connected to an Attempt.
- UI only exposes Task creation, spec editing, read-only readiness, Task/Step/progress/Artifact read views. No plan/Attempt/Environment/cancel/recover actions are live.

## Canonical errors

Use the error codes already owned by `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`, including `AGENT_UNAVAILABLE`, `STALE_TASK_VERSION`, `STALE_SPEC_REVISION`, `FORBIDDEN`, `CONFLICT`, `DEPENDENCY_UNAVAILABLE`, `STALE_FENCE` and `INTEGRITY_FAILURE` where applicable. Do not create a feature-local error vocabulary.

## Qualification gate

Exercise denial, stale versions, idempotent receipt replay, changed-payload conflict, grant expiry/revocation, provider refusal, isolation mismatch, timeout-before/after-write, AMBIGUOUS effect, cancellation, quiescence unknown/confirmed, runtime crash/restart, recovery and independent verification on the exact declared OS/provider/agent/auth route. No current end-to-end Task execution was run in this audit.