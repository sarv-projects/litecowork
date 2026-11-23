# Error and Recovery Contract - Feature 015

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation.
Canonical codes: [docs/schemas/error-codes.schema.json](../../../docs/schemas/error-codes.schema.json). Recovery semantics: [docs/FAILURE-RECOVERY.md](../../../docs/FAILURE-RECOVERY.md).

## 1. Error mapping

| Condition | Code | HTTP | Exists today |
|---|---|---|---|
| Pause on PAUSED Task | `TASK_ALREADY_PAUSED` | 409 | yes |
| Pause at unsafe boundary | `TASK_PAUSE_UNSAFE` | 409 | yes |
| Control on terminal Task | `TASK_TERMINAL` | 409 | yes |
| Resume when not PAUSED | `TASK_NOT_PAUSED` | 409 | accepted target |
| Stale Task version | `STALE_TASK_VERSION` | 409 | yes |
| Stale spec revision | `STALE_SPEC_REVISION` | 409 | yes |
| Same idempotency key, different body | `CONFLICT` | 409 | yes |
| Late result from superseded Attempt or epoch | `STALE_FENCE` | 409 (internal) | yes |
| Handoff while prior lease is live | `HANDOFF_PRIOR_LEASE_ACTIVE` | 409 with retry-after | accepted target |
| Profile not in Coworker allowlist | `DELEGATION_PROFILE_NOT_ENABLED_FOR_COWORKER` | 403 | accepted target |
| Depth limit | `DELEGATION_DEPTH_EXCEEDED` | 409 | yes |
| Concurrency limit | `DELEGATION_CONCURRENCY_EXCEEDED` | 409 | yes |
| Profile disabled or archived | `DELEGATION_PROFILE_DISABLED`, `DELEGATION_PROFILE_ARCHIVED` | 409 | yes |
| Budget STOP_NEW_DELEGATION | `DELEGATION_BUDGET_EXHAUSTED` | 409 | accepted target |
| Shared writable Environment requested | `DELEGATION_ENVIRONMENT_SHARING_DENIED` | 403 | accepted target |
| No qualified Environment (Windows row) | `ENVIRONMENT_UNAVAILABLE` | 503 | yes |
| Foreign Workspace or Task | `NOT_FOUND` | 404 | yes |

`ChildAdmissionDenial` variants map in one exhaustive function: `StaleTask` to `STALE_TASK_VERSION`, `StalePlanOrSpec` to `STALE_SPEC_REVISION`, `StepNotReady` to `STEP_NOT_READY`, `ParentAttemptNotCurrent`, `ParentSessionNotCurrent`, `ParentLeaseNotCurrent` and `InvalidLineage` to `STALE_FENCE`, `DepthLimitReached` to `DELEGATION_DEPTH_EXCEEDED`, `ConcurrencyLimitReached` to `DELEGATION_CONCURRENCY_EXCEEDED`, `ProfileNotEnabledForCoworker` to `DELEGATION_PROFILE_NOT_ENABLED_FOR_COWORKER`, `ProfileUnavailable` and `ProfileRevisionMismatch` to `DELEGATION_PROFILE_INCOMPATIBLE`, `AuthorityPrerequisitesUnproven` to `POLICY_DENIED`, `WorkspaceMismatch` to `NOT_FOUND`, `InvalidInput` to `INVALID_ARGUMENT`.

## 2. Failure and crash matrix

| Window | Required behaviour |
|---|---|
| Crash after PAUSE_REQUESTED commit, before quiescence | Startup sweep: prior leases cannot be renewed; Attempts ABANDONED; Effects reconciled; then PAUSED commits from the durable control request |
| Crash before child admission commit, after private provider preparation | No child Attempt exists. Startup resolves the preparation journal against the exact admission receipt; an unbound scope is destroyed only after provider identity and positive quiescence, while unknown evidence keeps a blocker |
| Crash after child admission commit, before native launch handoff | Resolve the exact child-admission receipt. The durable Attempt/STARTING session is fenced to its old Runtime incarnation; stop or reconcile the held supervisor by exact identity, observe quiescence, then mark the Attempt ABANDONED before any replacement |
| Crash after native launch handoff, before worker readiness is committed | Treat the child as possibly running. Reconcile exact process scope, stop if owned, prove quiescence, settle child Attempt and Effects, then permit replacement; never replay the launch handoff blindly |
| Timeout after worker dispatch | Effect AMBIGUOUS; Feature 005 reconciler settles it before any replacement |
| Late ResultEnvelope after handoff or replacement | Fenced observation; no state change |
| Duplicate command replay | Stored receipt returned; no second effect |
| Cancel racing child completion | Earlier committed outcome wins; cancel settles the rest |
| Pause racing cancel | One CAS wins; cancel supersedes a pending pause |
| Quiescence never observed | Bounded wait then forceful stop of LiteCowork-owned processes only; Task stays in the requested state with a typed blocker until settled |
| Verifier timeout | VerificationRun INCONCLUSIVE |
| Resume of confirmed `INPUT_REQUIRED` | Same-Attempt continuation is permitted only under canonical TASK-RUNTIME same-incarnation/same-Environment, fresh-session, higher-lease and Effect-reconciliation conditions; after restart or resource/binding replacement, reconcile/cancel the prior operation and use a fresh Attempt |

## 3. Mandatory behaviour

UNKNOWN is never success. No retry resends a native prompt to repair a storage write. Recovery never replays recorded actions.

## 4. TaskSpec policy-resolution race and recovery (USER-DELEGATED direction, T22)

The TaskStore resolves the policy before writing the content-addressed aggregate-state blob. The SQLite transaction recalculates from its locked immutable/current owner rows and compares the exact canonical snapshot. If Workspace policy changed, the Coworker origin/source row is missing, a selected profile no longer lies within the referenced Workspace ceiling, or any resolved field differs, return a typed conflict/invalid-reference result and commit no TaskSpec, event or idempotency receipt. The prewritten blob has no public reference and remains eligible for normal BlobStore collection; do not perform BlobStore work inside SQLite. The caller may reload and submit a new explicit TaskSpec revision with a new idempotency identity. A retry of the original request only returns its original committed receipt if one exists; it never silently resolves a different policy. On restart, readers return the last committed TaskSpec snapshot. Pre-v28 null snapshots remain explicitly legacy/unavailable until an owner-controlled new TaskSpec revision resolves them.

Feature 005's bounded file-write admission/recovery path is not a generic native provider session manager. Its revision 1.1 records the owner-directed Task Runtime seam; cross-owner review and Feature 015 acceptance/implementation remain required before delegation, verifier or Task-level quiescence recovery is claimed. Native direct actions must not be represented as LiteCowork Effect receipts unless they actually cross the mediated Effect boundary.
