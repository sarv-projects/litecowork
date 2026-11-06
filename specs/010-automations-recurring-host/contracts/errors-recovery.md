# Errors and recovery - Automations and Recurring Host

Restart replays the cursor and does not double-create. Overlap policy is explicit. Missed fire during sleep is recorded as misfire or skipped according to B-26, never as silent success.

Denial, duplicate replay, timeout-after-dispatch, cancel, crash and restart are all required before QUALIFIED. Unknown external results reconcile before retry.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## External workflow invocation

External workflow timeout after dispatch is UNKNOWN until the provider observation or idempotency receipt resolves it. Reconcile the existing external run before retry; never issue a new idempotency identity merely because the UI timed out. A cancel request is not rollback and does not prove the provider stopped. A provider-side graph revision change does not rewrite the Routine pin; a new Routine revision is required for a changed workflow reference/version.

Provider-native AI builder availability, MCP permissions, enabled workflow set and Code-node runtime are separate from LiteCowork connection state. If the provider disables MCP, removes a workflow exposure, changes a permission scope or revokes OAuth, the local adapter must surface the exact unavailable/denied state; never retry using a broader provider token. If a connected provider is below a security-advisory fixed version or has an affected execution configuration (for example the cited n8n Python Task Runner advisory condition), disable graph/code authoring and consequential invocation until the provider owner resolves the advisory and the exact instance is re-probed. A provider's own “test succeeded” result is an observation, not LiteCowork Task verification.

Provider graph nodes and step states remain provider-owned. Map only normalized progress/result observations to the LiteCowork CapabilityInvocation and parent Task/Step. Do not synthesize local child Steps from provider node names.
