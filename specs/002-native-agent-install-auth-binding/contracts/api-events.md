# API and events - Native Agent Install, Auth and Binding

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Use existing Agent registry, lifecycle and binding operations in docs/API.md and docs/AGENT-CONTROL.md §12. Do not add a universal /v1/providers catalog.

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. Revision 1.0 declares no new event type as existing. Any event needed for descriptor refresh or auth observation is an accepted target interface (not yet in source) and is promoted by 002-T02 into EVENTS.md and its schema in the same change; Runtime-local observations stay non-Workspace events unless the owner promotes them.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.


## Model and effort option boundary

No new global model-catalog or provider route is introduced. The UI reads model/session options from the exact AgentControlDescriptor owned by the selected AgentModule/profile and stores only a binding-scoped preference for a future admission. Any descriptor refresh operation remains the existing agent-control owner operation and must return source, version, observed-at and revision information. The native adapter revalidates the selected provider model/effort when a Conversation turn or Task Attempt is admitted. No new event is added by this UI contract; use existing Agent profile/binding/descriptor event ownership. Stale descriptors fail closed and do not silently switch provider or model.


## Operations used by revision 1.0 tasks

| Operation (OpenAPI operationId) | Current source state (rechecked 2026-11-13) | Task |
|---|---|---|
| `probeLocalAgentProfile` | SERVED; exact OpenCode 2.0.32 + complete probe + observed host stop yields a CODE-compatible `STARTABLE` offer; no native provider journey is qualified | 002-T01 |
| `createAgentBinding`, `enableAgentBinding`, `disableAgentBinding` | SERVED; enable applies descriptor-revision CAS when its module declares a descriptor; no live auth/bind/send qualification | 002-T06 |
| `getAgentControlDescriptor`, `refreshAgentProfileControl` | SERVED for the trusted OpenCode module; module-less profiles return `503`; descriptor writes reuse the migration-16 table | 002-T05 |
| `submitAgentCredential` | authenticated/no-store route is wired to v36 owner-scoped receipts and the idempotent OS-keyring path. New requests require a fresh descriptor with a unique `LITECOWORK_SECRET_STORE` slot; exact committed replay returns 200, a changed fingerprint returns 409, and a pending request resumes the reserved reference. CODE/SERVED only: current OpenCode and Claude Code descriptors declare no LiteCowork-owned slot, so neither exposes a credential UI or provider sign-in path. | 002-T08 |
| `beginAgentAuthentication` | SERVED (fail-closed: declared methods only; OpenCode declares none; production launcher is unqualified; redacted observations persist in v35) | 002-T07 |
| `getAgentSessionOptions`, `getAgentHarnessCapabilities` | SERVED for a binding with a fresh OpenCode descriptor; capabilities also require one matching current Runtime endpoint/offer. No Tauri picker bridge or Send-admission consumer is wired. | 002-T05, 002-T09 |
| `openAgentNativeConfiguration` | SERVED (fail-closed: declared targets only; V1 modules declare none; unqualified opener) | 002-T10 |
| install / update under `/agent-registry/{registryAgentId}` | OpenAPI target-only, not served; V1 shows them unavailable | 002-T11 |

`submitAgentCredential` uses the existing endpoint and `Idempotency-Key`; it adds no event. The v36 receipt and audit settlement are internal SQLite records, and the receipt is excluded from Workspace backups. The authenticated daemon route now performs owner-scoped receipt lookup, Runtime-keyed fingerprinting, reservation, idempotent SecretStore write and settlement; exact committed replay is 200, first commit is 201, and changed request content is 409. The route tests cover a fresh descriptor-backed slot, replay/conflict, and resumption from a seeded PENDING receipt. A seeded receipt retry is not a daemon-process restart test. Current OpenCode and Claude Code modules declare no LiteCowork-owned credential slot, so the new path is not an integrated provider sign-in. Actual OS credential-store restart behavior, platform qualification and provider bind/send remain separate gates. See `docs/AGENT-CONTROL.md` §9.5 and Feature 002 SPEC §7.2.1.
