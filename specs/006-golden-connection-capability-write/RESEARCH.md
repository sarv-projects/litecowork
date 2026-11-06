# Research - Golden Connection and One Capability Write

**Feature:** 006 | **Checked:** 2026-11-06 (source, composed routes and desktop UI)

## Sources

Authorities: docs/CONNECTORS-WORKFLOWS.md, docs/CAPABILITY-FABRIC.md, docs/NETWORK-SECURITY.md, docs/LITESPM-LITECOWORK-INTEROP-LLD.md. Reject embedding third-party automation runtimes. Upstream LiteSPM main was checked at `111fd9b8e84573f8fce76b6374b4ff9639502539` on 2026-11-05; its 13-tool local stdio bridge is WIRED upstream. LiteCowork's adapter remains TARGET / not qualified.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Upstream `STATUS.md` marks `internal/bridge` WIRED: `request_install` reaches supported skill/MCP/pinned-plugin installation paths; `get_invocation` and `cancel_invocation` reach the daemon invocation registry. These are upstream claims at the pinned commit, not LiteCowork integration proof.
- The bridge runs as local MCP stdio via `litespm bridge stdio --host <host-id>` and can cold-start/reconnect the daemon. Exact MCP tool schemas and host mapping still require a LiteCowork conformance fixture.
- Upstream's host UX is version-dependent: OpenCode v2.2.5 exposes `/litespm:marketplace`; Qwen Code 0.14.4 exposes tools through `/mcp` and does not use the same slash command.
- Windows peer identity is not independently verified by the current LiteSPM bridge (`peerAuth.verified=false`, owner-only Named Pipe DACL is the stated OS boundary). Do not label this application-level verified identity.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Source, composed-route and UI audit (2026-11-06)

Inspected `apps/litecoworkd/src/discover_operator.rs`, `capability_invocations_operator.rs`, `approval_operator.rs`, `trust_operator.rs`, `trust.rs`, `agent_credentials_routes.rs`, `main.rs`, `crates/storage-sqlite/src/invocations.rs`, `crates/storage-sqlite/src/trust.rs`, `apps/litecowork-ui/src/discover/DiscoverPage.tsx`, `discover-api.ts`, `discover-state.ts`, `needs-you/ApprovalDetail.tsx`, `needs-you/approval-state.ts`, and the composed `docs/schemas/served-route-inventory.json`.

### Current route truth

- Served read/write operations include Discover search/detail; capability `connect-or-install` and `test`; Approval list/detail/resolve; Task-planning read-grant create/revoke; and CapabilityInvocation task-list/detail GETs.
- The `connect-or-install` route requires authenticated owner Workspace/runtime scope and a bounded idempotency key, but it always returns `503 DEPENDENCY_UNAVAILABLE` with reason `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`. It creates no durable package/install receipt and explicitly reports `requested_scope_granted=false`.
- The `test` route is a no-provider probe. It returns `health=UNKNOWN` and a reason saying the LiteSPM package service is not qualified and no provider was contacted.
- Connection list/get/disconnect operations in the OpenAPI contract are target-only: `GET /connections`, `GET /connections/{connectionId}`, `POST /connections/{connectionId}/disconnect`. No corresponding composed daemon routes exist. No account-Connection OAuth start/callback/refresh lifecycle is served.
- CapabilityInvocation Operator exposes only GET list/detail. Its module-level comment says it does not create, transition or dispatch Invocations. The SQLite transition store explicitly rejects transition to DISPATCHED with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`. No production invocation-creation/dispatch caller was found in `apps/litecoworkd/src`.
- The Trust evaluator module is compiled with `allow(dead_code)` in `main.rs`; `trust.rs` itself says it does not dispatch providers, issue grants/SecretLeases or create reusable permits, and that CapabilityBroker admission remains fail-closed until the combined Trust/Effect/audit transaction is implemented.
- `OsSecretStore` is a real local OS keychain adapter and the SecretStore API is write-only, but no Feature 006 account Connection service uses it. The mounted AgentProfile credential-slot route is Feature 002 and currently fails 503 after descriptor validation, before SecretStore write, because exact durable replay protection is not ready.

### Approval, grants and UI

- Approval list/get/resolve are served. Approve requires the current Approval version, an idempotency key, local authenticated owner assurance and an exact `confirmation_digest`; the store commits `approval.resolved.v1` with replay protection. Deny does not need the approval digest.
- `ApprovalDetail` has an actual load/resolve UI. It preserves the same decision request identity after an uncertain response, refreshes status, and says explicitly that approving records the exact-action decision only—it does not grant reusable capability access or run the external action.
- The served CapabilityGrant route currently creates a bounded, expiring Task-planning `workspace.resource.read` grant over explicit Resource refs. It does not establish OAuth/account Connection custody or a connector write grant.
- Discover has Explore, Installed/native, Managed and Connections tabs. Explore identifies Workspace-local skills and states public package discovery unavailable. Installed/native shows only Runtime-reported capability hosts (not all native installations). Managed says package management is not connected. Connections says account connections cannot be read in this build.
- `connectActionOutcome()` returns `allowed=false` for every row: availability-not-ready rows show their truthful state/reason; Workspace-local skill Open/Run is unavailable; package Connect/Use is unavailable until the LiteSPM contract is qualified. All row action buttons are disabled.
- No provider OAuth form/callback, account Connection list/detail, provider read/write operation picker, grant assignment, or connected→granted→invoked UI exists. Approval detail is a separate, real state transition and is not proof that an invocation ran.

These are source/route/UI findings. No LiteCowork account Connection was created, OAuth was not started, no token was stored through a Connection flow, no connector provider was called, no CapabilityInvocation was dispatched and no provider Effect was reconciled.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; LiteSPM upstream bridge evidence remains pinned to commit `111fd9b8e84573f8fce76b6374b4ff9639502539` as checked on 2026-11-05, not as proof of LiteCowork integration.
- No LiteCowork runtime or end-to-end bridge integration was executed. The upstream source review is static and commit-pinned; it does not qualify a local LiteCowork process launch, IPC identity, host mutation, or receipt/recovery path.

## Unresolved questions

See SPEC section 5 blockers. Owners are named on the package README.
