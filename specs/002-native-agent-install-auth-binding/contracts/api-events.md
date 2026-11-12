# API and events - Native Agent Install, Auth and Binding

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Use existing Agent registry, lifecycle and binding operations in docs/API.md and docs/AGENT-CONTROL.md §12. Do not add a universal /v1/providers catalog.

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. Revision 1.0 declares no new event type as existing. Any event needed for descriptor refresh or auth observation is an accepted target interface (not yet in source) and is promoted by 002-T02 into EVENTS.md and its schema in the same change; Runtime-local observations stay non-Workspace events unless the owner promotes them.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.


## Model and effort option boundary

No new global model-catalog or provider route is introduced. The UI reads model/session options from the exact AgentControlDescriptor owned by the selected AgentModule/profile and stores only a binding-scoped preference for a future admission. Any descriptor refresh operation remains the existing agent-control owner operation and must return source, version, observed-at and revision information. The native adapter revalidates the selected provider model/effort when a Conversation turn or Task Attempt is admitted. No new event is added by this UI contract; use existing Agent profile/binding/descriptor event ownership. Stale descriptors fail closed and do not silently switch provider or model.


## Operations used by revision 1.0 tasks

| Operation (OpenAPI operationId) | Current source state (rechecked 2026-11-12) | Task |
|---|---|---|
| `probeLocalAgentProfile` | SERVED; exact OpenCode 2.0.32 + complete probe + observed host stop yields a CODE-compatible `STARTABLE` offer; no native provider journey is qualified | 002-T01 |
| `createAgentBinding`, `enableAgentBinding`, `disableAgentBinding` | SERVED; enable applies descriptor-revision CAS when its module declares a descriptor; no live auth/bind/send qualification | 002-T06 |
| `getAgentControlDescriptor`, `refreshAgentProfileControl` | SERVED for the trusted OpenCode module; module-less profiles return `503`; descriptor writes reuse the migration-16 table | 002-T05 |
| `submitAgentCredential` | mounted, returns `503`; no SecretStore write | 002-T08 |
| `beginAgentAuthentication` | SERVED (fail-closed: declared methods only; OpenCode declares none; production launcher is unqualified; observations are in-memory) | 002-T07 |
| `getAgentSessionOptions`, `getAgentHarnessCapabilities` | OpenAPI target-only, not served | 002-T05, 002-T09 |
| `openAgentNativeConfiguration` | SERVED (fail-closed: declared targets only; V1 modules declare none; unqualified opener) | 002-T10 |
| install / update under `/agent-registry/{registryAgentId}` | OpenAPI target-only, not served; V1 shows them unavailable | 002-T11 |
