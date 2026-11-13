# API and events - Automations and Recurring Host

**Feature:** 010 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Automation and Routine routes in docs/API.md and AUTOMATION.md. No new public webhook ingress in this package (V1 non-goal).

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. `notification.delivery.changed.v1` and its current payload shape already exist; its JSON Schema currently permits arbitrary nonempty `from`/`to` strings instead of constraining them to delivery-state tokens. No production delivery presenter or status-transition writer is served. T01 must add or trace the missing transition-token validation and event-trigger observation fields without redefining the existing event or status vocabulary.

SQLite v1 already contains the notification status/attempt columns and Runtime-local encrypted Automation trigger cursor table. The proposed settle route still needs a precise `If-Match`/lease fencing value; a delivery-row version column is absent. Do not claim an additive migration until A-98 resolves the exact delta.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.


## External workflow adapter boundary

Feature 010 does not define WorkflowGraph CRUD, node/edge editing, graph versioning, graph run/step APIs or AI workflow-authoring tools. A Routine selects an existing workflow through the owner contract in [CONNECTORS-WORKFLOWS.md](../../../docs/CONNECTORS-WORKFLOWS.md). The target adapter contract's health/list/get/validate/invoke/get-run/stream/cancel/normalize operations are provider-scoped; this adapter is not currently implemented or served in the active source tree. Consequential invocation is recorded under the existing CapabilityInvocation/Task/Attempt/Effect owners; external provider run identifiers and node status do not become a new local event family.

No new event is introduced by the OpenBot comparison. Use current Task, CapabilityInvocation, Effect and Automation occurrence events. Any future provider authoring API requires a separate contract, schema and owner review before a route or agent tool is exposed.
