# API and events - Golden Connection and One Capability Write

**Status:** IN_REVIEW - revision 1.1 records USER-DELEGATED direction; formal domain-owner ratification pending. API/event behavior below is TARGET unless explicitly marked served.

## Currently served operations

- `GET /v1/discover/search` and `GET /v1/discover/items/{item_id}`: Workspace-scoped catalog/Runtime-observation read projections. Catalog items are `authoritative=false`; a package row is not a Connection, installation receipt, grant or permission.
- `POST /v1/capabilities/{capability_id}/connect-or-install`: mounted, but always returns `503 DEPENDENCY_UNAVAILABLE` (`LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`), creates no receipt and grants no requested scope.
- `POST /v1/capabilities/{capability_id}/test`: mounted, but performs no provider call and returns `health=UNKNOWN` with an explicit reason.
- `GET /v1/approvals`, `GET /v1/approvals/{approval_id}` and `POST /v1/approvals/{approval_id}/resolve`: served. Resolution uses current Approval `If-Match`, `Idempotency-Key`, authenticated local owner assurance and exact `confirmation_digest` for APPROVE; commits `approval.resolved.v1`.
- `POST /v1/capability-grants` and `POST /v1/capability-grants/{grant_id}/revoke`: served only for the current bounded Task-planning `workspace.resource.read` grant over explicit Resource refs. They do not implement this feature's connector account/read/write grant.
- `GET /v1/tasks/{task_id}/invocations` and `GET /v1/capability-invocations/{invocation_id}`: served read-only projections. They do not create, transition to DISPATCHED or dispatch an Invocation.

## Target-only operations and missing lifecycle

`listConnections`, `getConnection` and `disconnectConnection` remain target-only (`GET /connections`, `GET /connections/{connectionId}`, `POST /connections/{connectionId}/disconnect`). No composed account-Connection device-authorization begin/poll/cancel or refresh route is served. No served golden-provider read or consequential-write invocation operation exists. `docs/schemas/served-route-inventory.json` is the composed-route authority for this checkout; generated TypeScript operation types alone do not prove a mounted handler.

## Target Connection flow

Revision 1.1 target: the authenticated owner explicitly starts a GitHub App user-to-server device authorization. ConnectionService sends requests only to the fixed GitHub device-code endpoint, returns the user code and fixed verification URI to the settings UI, and polls only the fixed token endpoint at the provider-supplied interval. It honors `authorization_pending`, `slow_down` (including the added interval), `access_denied`, expiry and cancellation. No loopback listener, redirect callback, client secret or App private key is used. `device_code` is daemon-memory-only; AuthAttempt persists status, expiry, expected version, non-secret correlation metadata and private preallocated SecretRef IDs only while credentials are staged. Restart expires pending authorization and cleans staged refs. Device-flow `offline_access` requests an expiring access/refresh pair; tokens move directly to the owner SecretStore boundary, refresh is serialized and token rotation is atomic. Connection state becomes CONNECTED only after durable Connection commit and SecretStore reference persistence succeed. The GitHub user's resource access and App installation/permission access must both include a target repository. The exact LiteCowork CapabilityGrant still gates each operation and repository. Local revoke fences future refresh/invocation and deletes local tokens; the user separately revokes GitHub App authorization in GitHub Settings. No API/event schema is promoted until T01 updates the canonical owners and machine contracts together.

## Target capability and Effect flow

After B-20 closes, the golden read requires an exact-scope CapabilityGrant. The one write operation must use its provider API operation ID, frozen request/response schema and operation class. Admission rechecks owner, runtime/lease, grant, activation, Trust result and exact `ApprovalUse` in one durable transaction before provider dispatch. Provider HTTP runs outside SQLite. The resulting Invocation/Effect remains AMBIGUOUS after an uncertain response until the exact target is reconciled. Retry is allowed only under the selected provider/client idempotency contract.

## Canonical events

Use existing owner event schemas only: `connection.state.changed.v1`, `approval.resolved.v1`, `capability.invocation.created`, `capability.invocation.dispatched`, `capability.invocation.status.changed`, and the relevant `effect.*` and `evidence.created.v1` events. This package adds no event beyond these unless T01 finds one missing; new request bodies are accepted target interfaces (not yet in source) promoted by T01. Preserve exact event version/payload names from `docs/EVENTS.md`.

## Compatibility and error semantics

Use canonical errors from `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`. In particular, unqualified LiteSPM/Connection/provider services fail `DEPENDENCY_UNAVAILABLE`; unavailable capabilities are not healthy; sensitive credential elicitation fails `SENSITIVE_INPUT_UNSUPPORTED`; unsupported MCP/provider input fails `PROVIDER_INPUT_UNSUPPORTED`; unknown external outcomes remain ambiguous and are reconciled before retry. No feature-local error codes are declared.
