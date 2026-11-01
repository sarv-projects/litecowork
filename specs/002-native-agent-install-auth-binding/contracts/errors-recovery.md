# Errors and recovery - Native Agent Install, Auth and Binding

Stale descriptor, expired auth, missing binary, unsupported version and cancelled native login are distinct typed failures. A failed probe does not delete a prior binding. Retry re-probes; it does not replay secret bytes. Crash mid-probe leaves the last committed binding and a failed observation.

Denial, duplicate replay, timeout-after-dispatch, cancel, crash and restart are all required before QUALIFIED. Unknown external results reconcile before retry.
