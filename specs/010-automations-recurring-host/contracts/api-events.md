# API and events - Automations and Recurring Host

Automation and Routine routes in docs/API.md and AUTOMATION.md. No new webhook ingress in this DRAFT.

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. This DRAFT does not declare new event types.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## External workflow adapter boundary

Feature 010 does not define WorkflowGraph CRUD, node/edge editing, graph versioning, graph run/step APIs or AI workflow-authoring tools. A Routine selects an existing workflow through the owner contract in [CONNECTORS-WORKFLOWS.md](../../../docs/CONNECTORS-WORKFLOWS.md). The target adapter contract's health/list/get/validate/invoke/get-run/stream/cancel/normalize operations are provider-scoped; this adapter is not currently implemented or served in the active source tree. Consequential invocation is recorded under the existing CapabilityInvocation/Task/Attempt/Effect owners; external provider run identifiers and node status do not become a new local event family.

No new event is introduced by the OpenBot comparison. Use current Task, CapabilityInvocation, Effect and Automation occurrence events. Any future provider authoring API requires a separate contract, schema and owner review before a route or agent tool is exposed.
