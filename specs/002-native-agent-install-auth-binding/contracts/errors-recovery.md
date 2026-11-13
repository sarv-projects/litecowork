# Errors and recovery - Native Agent Install, Auth and Binding

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Stale descriptor, expired auth, missing binary, unsupported version and cancelled native login are distinct typed failures. A failed probe does not delete a prior binding. Retry re-probes; it does not replay secret bytes. Crash mid-probe leaves the last committed binding and a failed observation.

Denial, duplicate replay, timeout-after-dispatch, cancel, crash and restart are all required before QUALIFIED. Unknown external results reconcile before retry.

Current source and target distinction (checked 2026-11-13): OpenCode offer admission is implemented in CODE; only exact 2.0.32 + complete probe + observed host stop yields `compatible=true`, and other observations carry a refusal reason (AC-013). Descriptor GET/refresh is served for the trusted OpenCode module; profiles without a module return `503 DEPENDENCY_UNAVAILABLE` and must never receive a catalog-manufactured descriptor (AC-014). Auth-begin rejects undeclared methods, but the production launcher is unqualified and no current module declares a method; observations are in-memory, restart to `UNKNOWN`, and durable observation/replay behavior remains incomplete (AC-003, AC-016). Recovery tests cover descriptor restart/failed refresh, not the complete provider lifecycle; see T13.
