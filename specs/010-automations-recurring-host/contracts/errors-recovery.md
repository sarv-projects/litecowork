# Errors and recovery - Automations and Recurring Host

**Feature:** 010 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Restart replays the cursor and does not double-create. Overlap policy is explicit. Missed fire during sleep is recorded as misfire or skipped according to B-26, never as silent success.

Denial, duplicate replay, timeout-after-dispatch, cancel, crash and restart are all required before QUALIFIED. Unknown external results reconcile before retry.

Current CODE recovery is narrower: the local TriggerHost polls persisted Schedule cursors while its Runtime is alive, and repeated schedule-slot settlement looks up the existing occurrence/Task. The worker does not wake a sleeping machine. Notification outcomes remain PENDING because no production claim/settle worker exists; restart recovery for an in-flight notification is not implemented. A paused pinned Coworker currently causes admission to defer the same slot rather than record and advance a `COWORKER_PAUSED` skip. Invalid persisted quiet-hours timezone data can evaluate as outside the window; save-time IANA validation remains required.

The source schema already contains notification status/attempt fields and encrypted Runtime-local trigger cursor bindings. A-98 holds the exact delivery lease/version migration decision; A-97 holds quiet-hours atomic multi-class persistence. These are contract gaps, not recovery guarantees supplied by the existing schema.


## External workflow invocation

External workflow timeout after dispatch is UNKNOWN until the provider observation or idempotency receipt resolves it. Reconcile the existing external run before retry; never issue a new idempotency identity merely because the UI timed out. A cancel request is not rollback and does not prove the provider stopped. A provider-side graph revision change does not rewrite the Routine pin; a new Routine revision is required for a changed workflow reference/version.

Provider-native AI builder availability, MCP permissions, enabled workflow set and Code-node runtime are separate from LiteCowork connection state. If the provider disables MCP, removes a workflow exposure, changes a permission scope or revokes OAuth, the local adapter must surface the exact unavailable/denied state; never retry using a broader provider token. If a connected provider is below a security-advisory fixed version or has an affected execution configuration (for example the cited n8n Python Task Runner advisory condition), disable graph/code authoring and consequential invocation until the provider owner resolves the advisory and the exact instance is re-probed. A provider's own “test succeeded” result is an observation, not LiteCowork Task verification.

Provider graph nodes and step states remain provider-owned. Map only normalized progress/result observations to the LiteCowork CapabilityInvocation and parent Task/Step. Do not synthesize local child Steps from provider node names.
