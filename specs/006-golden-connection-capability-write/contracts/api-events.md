# API and events - Golden Connection and One Capability Write

## Currently served operations

- `GET /v1/discover/search` and `GET /v1/discover/items/{item_id}`: Workspace-scoped catalog/Runtime-observation read projections. Catalog items are `authoritative=false`; a package row is not a Connection, installation receipt, grant or permission.
- `POST /v1/capabilities/{capability_id}/connect-or-install`: mounted, but always returns `503 DEPENDENCY_UNAVAILABLE` (`LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`), creates no receipt and grants no requested scope.
- `POST /v1/capabilities/{capability_id}/test`: mounted, but performs no provider call and returns `health=UNKNOWN` with an explicit reason.
- `GET /v1/approvals`, `GET /v1/approvals/{approval_id}` and `POST /v1/approvals/{approval_id}/resolve`: served. Resolution uses current Approval `If-Match`, `Idempotency-Key`, authenticated local owner assurance and exact `confirmation_digest` for APPROVE; commits `approval.resolved.v1`.
- `POST /v1/capability-grants` and `POST /v1/capability-grants/{grant_id}/revoke`: served only for the current bounded Task-planning `workspace.resource.read` grant over explicit Resource refs. They do not implement this feature's connector account/read/write grant.
- `GET /v1/tasks/{task_id}/invocations` and `GET /v1/capability-invocations/{invocation_id}`: served read-only projections. They do not create, transition to DISPATCHED or dispatch an Invocation.

## Target-only operations and missing lifecycle

`listConnections`, `getConnection` and `disconnectConnection` remain target-only (`GET /connections`, `GET /connections/{connectionId}`, `POST /connections/{connectionId}/disconnect`). No composed account-Connection create/start-OAuth/callback/refresh route is served. No served golden-provider read or consequential-write invocation operation exists. `docs/schemas/served-route-inventory.json` is the composed-route authority for this checkout; generated TypeScript operation types alone do not prove a mounted handler.

## Target Connection flow

After B-19 closes, the accepted public OAuth/PKCE flow must use an OS-chosen loopback listener on 127.0.0.1, single-use `state`, PKCE, exact redirect/host allowlists, and a callback containing no token. Provider token bytes move directly to the owner SecretStore boundary. Connection state becomes CONNECTED only after durable Connection commit and SecretStore reference persistence succeed. Cancelled/failed auth persists no usable Connection. Refresh serializes per Connection; invalid_grant/revocation becomes REAUTH_REQUIRED; revoke fences future refresh and invocation.

## Target capability and Effect flow

After B-20 closes, the golden read requires an exact-scope CapabilityGrant. The one write operation must use its provider API operation ID, frozen request/response schema and operation class. Admission rechecks owner, runtime/lease, grant, activation, Trust result and exact `ApprovalUse` in one durable transaction before provider dispatch. Provider HTTP runs outside SQLite. The resulting Invocation/Effect remains AMBIGUOUS after an uncertain response until the exact target is reconciled. Retry is allowed only under the selected provider/client idempotency contract.

## Canonical events

Use existing owner event schemas only: `connection.state.changed.v1`, `approval.resolved.v1`, `capability.invocation.created`, `capability.invocation.dispatched`, `capability.invocation.status.changed`, and the relevant `effect.*` and `evidence.created.v1` events. No new event or schema is introduced by this DRAFT. Preserve exact event version/payload names from `docs/EVENTS.md`.

## Compatibility and error semantics

Use canonical errors from `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`. In particular, unqualified LiteSPM/Connection/provider services fail `DEPENDENCY_UNAVAILABLE`; unavailable capabilities are not healthy; sensitive credential elicitation fails `SENSITIVE_INPUT_UNSUPPORTED`; unsupported MCP/provider input fails `PROVIDER_INPUT_UNSUPPORTED`; unknown external outcomes remain ambiguous and are reconciled before retry. No feature-local error codes are declared.