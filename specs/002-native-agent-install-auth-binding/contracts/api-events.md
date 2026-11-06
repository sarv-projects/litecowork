# API and events - Native Agent Install, Auth and Binding

Use existing Agent registry, lifecycle and binding operations in docs/API.md and docs/AGENT-CONTROL.md §12. Do not add a universal /v1/providers catalog.

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. This DRAFT does not declare new event types.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## Model and effort option boundary

No new global model-catalog or provider route is introduced. The UI reads model/session options from the exact AgentControlDescriptor owned by the selected AgentModule/profile and stores only a binding-scoped preference for a future admission. Any descriptor refresh operation remains the existing agent-control owner operation and must return source, version, observed-at and revision information. The native adapter revalidates the selected provider model/effort when a Conversation turn or Task Attempt is admitted. No new event is added by this UI contract; use existing Agent profile/binding/descriptor event ownership. Stale descriptors fail closed and do not silently switch provider or model.
