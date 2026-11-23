# 006 - Golden Connection and One Capability Write

**ID:** 006
**Status:** IN_REVIEW
**Owners:** Connectors, Capability Fabric, Security, LiteSPM boundary, Desktop UI
**Updated:** 2026-11-20
**Revision:** 1.1
**Revision 1.0 history:** B-19 and B-20 were accepted on 2026-11-09 by the repository owner (sarv-projects) via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md) (PR #1). The provider documentation check later disproved B-19's claim that a public GitHub desktop OAuth web flow needs no client secret and found its OAuth-scope mapping inaccurate.
**Revision 1.1 direction:** GitHub App user-to-server device authorization, user-access tokens with expiring access/refresh tokens, the app's fine-grained Issues permission, and a separately scoped LiteCowork grant. This is **USER-DELEGATED DIRECTION RECORDED**, not domain-owner ratification, ACCEPTED/LOCKED status, implementation evidence or provider qualification. The new revision is IN_REVIEW until the owning reviewers ratify it.
**Change control:** material changes to the previously locked B-19 decision are recorded as a new revision. The explicit product-owner delegation permits internal implementation planning against revision 1.1 while external owner review remains visible as a separate gate; it does not authorize false acceptance or qualification claims.
**Package boundary:** 006 does NOT own the LiteSPM package lifecycle (observe, adopt, install, update, repair, unmanage, remove, inventory, Discover Managed/Installed tabs, E2E-06). That belongs to new package 013 (litespm-package-lifecycle-bridge). `POST /v1/capabilities/{capability_id}/connect-or-install` stays a fail-closed 503 owned by 013 for its install half; 006 owns only the account Connection, the scoped read/write, approval, reconciliation and revoke.
**Target release:** V1 local desktop. The standalone Connection/auth and provider-read evidence rows are Windows 11/10 `x86_64-pc-windows-msvc` first, Linux x86_64 second, with OS keystore qualification per row (Windows Credential Manager, Linux Secret Service). The Task-scoped write's row order remains OPEN under A-80: if it requires Feature 005's admitted Environment, Linux qualifies first and Windows Task execution stays unavailable until B-35; otherwise its exact mediated scope must be documented. Do not merge Connection/read evidence into the Task-write qualification claim.
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Capabilities are not installed merely by appearing in a catalog. Token bytes must not reach the agent. A write with an unknown provider result must not be retried blindly.

Observable outcome: The owner completes one real account Connection, performs one authorized read, then one consequential write with approval, idempotency and reconciliation, and revoke fences later use.

## 2. Goals

- Finish one first-party Connection with SecretStore custody.
- Execute one scoped read and one scoped write through the capability broker.
- Require explicit approval on the write and reconcile UNKNOWN before retry.
- Revoke so later invocations fail closed.

## 3. Non-goals

- Embedding Activepieces, Nango or n8n runtimes.
- Generating operations from OpenAPI without review.
- Multiple providers in this slice.
- Coworker assignment as a substitute for a grant.
- LiteSPM package discovery, install, adopt, update, repair, unmanage or remove (package 013). Installation is not Connection authentication, grant or invocation authority.

## 4. Current evidence

The active checkout contains a Discovery catalog/read projection, a truthful fail-closed package connect/test surface, durable Approval decisions, a narrow Task-planning Resource-read grant, and read-only CapabilityInvocation history. It does not contain a served account Connection lifecycle or a live provider capability-invocation/Effect dispatch path. The following source/route/UI crosswalk is not owner acceptance or release qualification.

| Capability | Current source/route evidence | Limitation / release meaning |
|---|---|---|
| Discover catalog | CODE/SERVED: `GET /v1/discover/search` and `GET /v1/discover/items/{item_id}`; Workspace-scoped, paginated, `no-store` projections with install/auth/compatibility/availability fields and `authoritative=false` | LiteSPM package service is not qualified; public package discovery and installation are not currently available. A catalog row never grants authority. |
| Connect/install action | CODE/SERVED: `POST /v1/capabilities/{capability_id}/connect-or-install` requires local owner Workspace, runtime check and idempotency key | Handler always returns `503 DEPENDENCY_UNAVAILABLE` / `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`, creates no receipt and grants no requested scope. This is not OAuth or Connection creation. |
| Capability health test | CODE/SERVED: `POST /v1/capabilities/{capability_id}/test` validates owner/runtime and returns `health=UNKNOWN` with a reason | It deliberately contacts no provider because package/service contract is unqualified. Never display this as connected/healthy. |
| Account Connections | OpenAPI target-only: `GET /connections`, `GET /connections/{connectionId}`, `POST /connections/{connectionId}/disconnect` | No composed daemon handlers were found. No served device-authorization begin/poll/cancel, refresh, list/get or disconnect Connection workflow exists. |
| SecretStore | CODE: `OsSecretStore` and write-only `SecretStore` trait exist. A credential-slot route for AgentProfiles is mounted as a different Feature 002 surface | The credential route verifies a current AgentControlDescriptor then deliberately returns 503 before SecretStore write because exact durable replay protection is not ready. This is not the Feature 006 account-Connection OAuth/token lifecycle. |
| Capability grants | CODE/SERVED: `POST /v1/capability-grants` and revoke commit a narrow bounded, expiring Task-planning `workspace.resource.read` grant over explicit Resource refs | No connection-backed provider operation or generic user-approved write grant is served. The route is not the AC-003 golden connector read/write grant lifecycle. |
| Capability invocations | CODE/SERVED read-only `GET /v1/tasks/{task_id}/invocations` and `GET /v1/capability-invocations/{invocation_id}`; SQLite contains transition/reconciliation primitives | The Operator module explicitly says it does not create, transition or dispatch Invocations. Storage rejects transition to DISPATCHED with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`. No CapabilityBroker dispatch/Effect/ApprovalUse transaction is connected. |
| Approval | CODE/SERVED: list/get and `POST /v1/approvals/{approval_id}/resolve`; CAS version, idempotency key, owner/local assurance and exact action-digest confirmation on APPROVE | This is a real exact-Approval state change, not the provider action itself. UI copy explicitly says the decision records the action decision only and does not create a reusable grant or run the action. |
| Trust evaluator and Effect | CODE: pure Trust decision primitive and durable approval/invocation/effect schemas/tests exist | `apps/litecoworkd/src/main.rs` compiles Trust evaluator as `allow(dead_code)` and states the daemon does not wire it; `trust.rs` says CapabilityBroker admission is fail-closed until combined Trust/Effect/audit transaction exists. No actual external write or reconciliation is connected. |
| Desktop Discover/Needs You UI | Discover shows Explore, Installed/native Runtime observations, Managed, and Connections tabs. `connectActionOutcome` disables every row action; Managed says not available; Connections says account connections cannot be read. Needs You ApprovalDetail performs a real exact-digest approval decision | No device-authorization panel, connected account list, connection details, golden read/write operation chooser, grant-binding UI or connected→granted→invoked lifecycle is present. Approval UI does not execute the approved provider operation. |

Revision 1.0 remains ACCEPTED/LOCKED for its original B-19/B-20 scope. Revision 1.1 changes the B-19 authentication recipe and is IN_REVIEW under change control; the product owner has recorded a USER-DELEGATED direction for internal implementation planning, while formal Connectors/Security ratification remains pending. The current-state matrix above is unchanged by either decision. There is no served golden account Connection, no provider-backed read/write, and no capability invocation dispatch. Existing catalog, grant, approval and invocation-record pieces must not be presented as the full connection/capability lifecycle.

Additional current-state facts checked 2026-11-20 against the active source for task grounding:

- `SecretStore` (`crates/storage-core/src/trust.rs`) is write-only by design: one `write(workspace_id, secret_kind, secret_bytes) -> SecretRef` method, no read and no delete. `OsSecretStore` (`crates/storage-sqlite/src/trust.rs`) writes through the `keyring` crate (Windows native, Linux Secret Service, macOS keychain features in `crates/storage-sqlite/Cargo.toml`); its private `SecretCredentialBackend` has `set_secret` and `remove_secret`. `SecretKind` is `ApiKey | Token | Password | Other`. `SecretLeaseRecord` and `SecretLeaseStore::issue_secret_lease` are Task/Attempt-scoped.
- `SqliteCapabilityDispatchStore::admit_capability_dispatch` (`crates/storage-sqlite/src/capability_dispatch.rs`) implements the combined Trust + ApprovalUse + Invocation + Effect admission transaction, but its module comment states it fails closed on the current schema because `sqlite-v15.sql` still guards Invocation creation and the first `DISPATCHED` transition. No daemon code constructs it.
- `apps/litecoworkd/src/capability_invocations_operator.rs` mounts only `get_invocation` and `list_task_invocations` and states it does not create, transition or dispatch an Invocation. `crates/storage-sqlite/src/invocations.rs` returns `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE` on a `DISPATCHED` transition. `domain_invocations::CapabilityInvocationRecord::reconcile_ambiguous_terminal` exists.
- `apps/litecoworkd/Cargo.toml` already depends on `reqwest` 0.12 with `rustls-tls` (no default features), so the brokered HTTP client does not need a new TLS stack.
- The OpenAPI Connection status enum (`docs/schemas/operator-api.openapi.yaml`, `listConnections` filter and the Connection schema) is `CONNECTING, CONNECTED, DEGRADED, REAUTH_REQUIRED, DISCONNECTED`; `REVOKED` is missing.

## 5. Current and flexible decisions

### LOCKED

- LiteCowork remains the control plane (CONNECTORS-WORKFLOWS §1).
- Installed, connected, assigned, granted, activated and invoked are distinct.
- Tokens stay in SecretStore; the model never sees them.
- TLS verification stays on; SSRF, redirects, timeouts and byte limits apply (AGENTS.md).
- Package approval is bound to an exact plan digest; agents cannot self-approve.

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### RECORDED DECISIONS AND ARCHITECTURAL CONTRACTS

B-19 and B-20 were accepted by the repository owner on 2026-11-09 via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md). The B-19 recipe is superseded for current implementation planning by the product-owner's explicit delegated direction recorded below; B-20 remains the accepted operation. The register of record is [docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md](../../docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md).

| ID | Topic | Status, resolution and architectural contract | Canonical authority and consequence |
|---|---|---|---|
| B-19 | First Connection/provider | **Revision 1.0 historical acceptance (2026-11-09):** GitHub OAuth App authorization-code + PKCE loopback. This recipe is superseded for current implementation planning. **USER-DELEGATED DIRECTION RECORDED (2026-11-20):** use a GitHub App user-to-server device flow with expiring user access and refresh tokens; no embedded client secret, App private key, callback listener or hosted token broker. Enable the GitHub App device flow. Request only the repository `Issues: write` permission required by the complete golden comment read/write slice; that permission also supports its issue-comment read. Keep the LiteCowork grant narrower and bound to the exact Connection, repository, operation and expiry. The user's GitHub access and the GitHub App's installation/repository access must both permit a target. Use the GitHub device-flow repository restriction where the exact repository ID is already known and the provider supports retaining that restriction through refresh; otherwise enforce repository scope at the LiteCowork broker on every operation. Tokens are stored in SecretStore before `status` becomes `CONNECTED`. No `repo:status`, `read:user` or OAuth-scope claim authorizes issue comments. Remote authorization revocation is a user action in GitHub Settings; local disconnect immediately removes local credentials and fences future use. | [CONNECTORS-WORKFLOWS.md](../../docs/CONNECTORS-WORKFLOWS.md), [NETWORK-SECURITY.md](../../docs/NETWORK-SECURITY.md), [RESEARCH.md](RESEARCH.md), and [docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md A-82/B-19](../../docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md). Formal owner ratification remains open; no auth route or provider journey is yet implemented or qualified. |
| B-20 | First capability write operation | **ACCEPTED (2026-11-09): the one golden write is "create an issue comment" (`POST /repos/{owner}/{repo}/issues/{number}/comments`, GitHub REST v3).** `ConnectorActionDescriptor.operation_class = WRITE`; grant scope is the single comment-write scope distinct from the read grant; approval binds to the immutable plan/action digest (comment target + body hash), not a display name. Provider idempotency is `UNKNOWN` for this endpoint, so the operation blocks and reconciles by listing the issue's comments for a matching client-tagged body before any second call; `UNKNOWN` is never permission to retry. Success evidence is the reconciled comment id. | [CAPABILITY-FABRIC.md](../../docs/CAPABILITY-FABRIC.md), [CAPABILITY-INVOCATIONS.md](../../docs/CAPABILITY-INVOCATIONS.md), [TRUST.md](../../docs/TRUST.md). No generic write permission or model-selected endpoint is acceptable; the golden write operationId must be promoted into the OpenAPI and schema owner together. |

B-20 remains owner-accepted. B-19's prior recipe remains a historical accepted decision, but the revision 1.1 direction is pending domain-owner ratification. Feature 006 is therefore IN_REVIEW; implementation and qualification status are tracked separately in §4 and [docs/IMPLEMENTATION-STATUS.md](../../docs/IMPLEMENTATION-STATUS.md).

**Provider source check and delegated decision 2026-11-20.** GitHub's [GitHub App user access token documentation](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app) explicitly supports device flow for desktop applications. Device flow uses the public GitHub App client ID, fixed device-code and token endpoints, a user-entered one-time code, provider-supplied expiry/poll interval, and `slow_down` handling; user access tokens from device flow do not require a client secret for refresh. Request `offline_access` to obtain expiring access/refresh tokens; keep both only in SecretStore, rotate the refresh token atomically, and require a fresh device flow if refresh expires. GitHub warns that device authorization can be abused for impersonation/phishing. LiteCowork therefore starts the flow only from an explicit in-app Connect action, shows the registered app identity and exact GitHub destination, warns the user to enter only the code generated by the current LiteCowork flow, never places the code in Conversation/agent context, and offers immediate cancellation.

GitHub App user access tokens have the intersection of the user's access and the App's permissions and resource access. GitHub's issue-comment read endpoint requires Issues read; create issue comment requires Issues write. The App's `Issues: write` permission is the minimum single permission covering this feature's read/write pair. It is broad enough to permit other issue operations at GitHub's boundary, so LiteCowork must enforce its own exact operation, repository, target, approval and expiry grant on every mediated call. Use App installation configuration to select only intended repositories where applicable; a catalog selection, account authorization or installation alone never creates LiteCowork invocation authority. The provider's optional `repository_id` device-flow restriction may be used only when its lifecycle and refresh semantics are verified; the broker must independently enforce the LiteCowork repository scope regardless.

GitHub allows the user to revoke a GitHub App authorization in account Settings → Applications → Authorized GitHub Apps. LiteCowork local revoke is immediate and does not claim that GitHub authorization was remotely revoked. Current facts were checked against the official device flow, user-token refresh, user authorization, permission and revocation documentation linked in [RESEARCH.md](RESEARCH.md). This choice resolves the internal design direction for A-82 under user delegation; it does not itself record the GitHub App registration, produce credentials, prove an authenticated connection, or satisfy owner review.

## 6. High-level design

UI starts Connect. ConnectionService owns the GitHub App device authorization and polling outside SQLite. SecretStore writes tokens. CapabilityBroker loads only a grant-scoped invocation. The provider HTTP call is outside the invocation transaction. Effect settlement records the reconciled receipt.

### Feature-specific component diagram

```mermaid
flowchart TD
  UI[Desktop Discover/Needs You UI] --> OP[Authenticated Operator]
  OP --> CN[ConnectionService device flow outside SQLite]
  CN --> GH[GitHub fixed device and token endpoints]
  GH --> SB[System browser at github.com/login/device]
  CN --> SS[(SecretStore tokens only)]
  CN --> DB[(SQLite Connection/grant/approval/invocation)]
  OP --> BR[CapabilityBroker grant-scoped invocation]
  BR --> TR[Trust evaluator + ApprovalUse]
  BR --> DB
  BR --> HTTP[Provider HTTP call outside invocation txn]
  HTTP --> RC[Reconciler read-back]
  RC --> DB
```

The ConnectionService OAuth/loopback path, the CapabilityBroker dispatch transaction and the reconciler read-back are the feature-specific components; Connection records, operation classes and Trust decisions remain owned by CONNECTORS-WORKFLOWS/CAPABILITY-FABRIC/TRUST and are not redefined.

### Trust boundaries

The desktop UI is untrusted; the authenticated Operator is the only command path. Provider token bytes live only in SecretStore and never enter Conversation, events, logs or the agent-visible context; only an opaque Connection id is visible. TLS verification stays on; SSRF, redirect, timeout and byte limits apply; worker egress rejects loopback, private, link-local, multicast and cloud-metadata destinations even if a grant names them. External dispatch waits until one transaction rechecks owner, lease, grant, activation, Trust decision and exact ApprovalUse. SQLite transactions make no network or process call.

### Actor sequence (target)

1. Owner explicitly starts Connect. ConnectionService requests a device/user code from the fixed GitHub endpoint and the UI displays only the user code, exact GitHub verification URI, permission summary and expiry.
2. The owner authorizes the named GitHub App in the system browser. ConnectionService polls no faster than the provider interval, honors `slow_down`, and stops on cancellation, denial, expiry or shutdown. Device code remains daemon-memory-only and is never persisted, logged, streamed or exposed to agents.
3. ConnectionService stores the access and refresh tokens in SecretStore, then commits Connection CONNECTED + AuthAttempt SUCCEEDED + event + receipt. A failed commit purges newly written secrets. On process restart, any pending authorization expires and the owner starts a fresh flow.
4. The broker verifies the authenticated user, App permission, repository accessibility and the exact LiteCowork grant before any read or write; the user and App must both have access to the repository.
5. Owner requests the golden write; a fresh approval binds to the exact plan/action digest.
6. The broker commits the invocation admission transaction, then makes the provider HTTP call outside the transaction.
7. On UNKNOWN, the reconciler reads back before any retry; on local revoke or provider revocation, later reads/writes fail closed.

### Ownership

| Owner | Responsibility | Excluded responsibility |
|---|---|---|
| Desktop UI | Connect flow, read/write chooser, approval decision | Executing the provider operation |
| Operator | Auth, Workspace scope, route validation | Holding token bytes |
| ConnectionService | GitHub App device flow and rate-limited polling, SecretStore custody, refresh/revoke | Capability invocation authority |
| SecretStore | Token custody write-only | Echoing secret bytes |
| CapabilityBroker | Grant-scoped invocation admission, dispatch, reconcile | Self-approval |
| Trust evaluator | ApprovalUse and operation-class decision | UI boolean shortcuts |
| Storage | Atomic Connection/grant/approval/invocation commits | External process/network calls |

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design and interfaces

Follow CONNECTORS-WORKFLOWS Connection record and ConnectionService. One read operation and one write operation are named only after B-20. Approval is a Trust decision, not a UI boolean. LiteSPM may supply the package; it does not grant use.

### 7.1 Existing signatures (CODE)

- Served: `GET /v1/discover/search`, `GET /v1/discover/items/{item_id}`, `POST /v1/capabilities/{capability_id}/connect-or-install` (returns 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`), `POST /v1/capabilities/{capability_id}/test` (returns `health=UNKNOWN`), Approval list/get/`POST /v1/approvals/{approval_id}/resolve`, `POST /v1/capability-grants` + revoke (`workspace.resource.read` only), `GET /v1/tasks/{task_id}/invocations`, `GET /v1/capability-invocations/{invocation_id}`.
- `OsSecretStore` + write-only `SecretStore` trait. Trust evaluator compiled `allow(dead_code)`; SQLite invocation store rejects transition to `DISPATCHED` with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`.
- Event: `connection.state.changed.v1`. Connection list/get/disconnect are target-only.

### 7.2 Target signatures (not yet in source)

Every signature below is a target interface (not yet in source). It becomes source only when its canonical owner document and the OpenAPI/schema change are promoted together (task 006-T01). Revision 1.1 replaces the obsolete loopback callback interface with an explicit device authorization lifecycle.

```rust
// Target (not yet in source): GitHub App device authorization; secrets never enter SQLite.
pub struct BeginConnection {
    pub principal_id: String,
    pub workspace_id: String,
    pub provider_id: String,                 // fixed first provider: "github"
    pub requested_repository: Option<RepositoryRef>, // only a user-selected target
}

pub struct DeviceAuthorizationView {
    pub auth_attempt_id: String,
    pub verification_uri: FixedGithubVerificationUri, // https://github.com/login/device
    pub user_code: RedactedUserCode,                 // UI-only; never an agent/event/log field
    pub expires_at: Timestamp,
    pub poll_interval_seconds: u32,
    pub requested_permissions: Vec<GitHubAppPermission>,
}

pub enum DeviceAuthorizationState { Pending, Authorized, Cancelled, Denied, Expired }

pub trait ConnectionService: Send + Sync {
    fn begin_connection(&self, begin: BeginConnection) -> Result<DeviceAuthorizationView, ConnectError>;
    fn poll_device_authorization(&self, auth_attempt_id: &str, expected_version: u64) -> Result<DeviceAuthorizationState, ConnectError>;
    fn cancel_device_authorization(&self, auth_attempt_id: &str, expected_version: u64) -> Result<DeviceAuthorizationState, ConnectError>;
    fn disconnect(&self, principal_id: &str, connection_id: &str) -> Result<(), ConnectError>;
}

// Accepted target (not yet in source): one golden write, approval bound to the action digest.
pub struct GoldenWriteInvocation {
    pub connection_id: String,               // opaque
    pub operation_class: OperationClass,      // WRITE
    pub action_digest: ActionDigest,          // target + body hash
    pub idempotency: ProviderIdempotency,     // UNKNOWN for this endpoint
    pub approval_use: ApprovalUse,
}
```

Typed errors (accepted target, not yet in source): `ConnectError::{Cancelled, StateReplay, SecretCommitFailed, HostNotAllowlisted}`; `InvocationError::{SelfApprovalDenied, UnknownResultBlocked, GrantMissing, ConnectionRevoked}`.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Payload fields stay accepted target interfaces until the canonical owner document and machine schema change together.

Secret custody (accepted target interface, not yet in source): the public `SecretStore` stays write-only. A daemon-internal `ConnectionSecretBroker` in `apps/litecoworkd` resolves token bytes only inside the brokered HTTP call for an admitted Invocation, using a new non-public `SecretResolver` port implemented beside `OsSecretStore`, and zeroizes after the request. A `SecretStore` delete operation (`remove(secret_ref)`) is added for revoke. Neither the resolver nor the bytes are reachable from an Operator route, a Tauri command, an event or an agent tool.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Opaque Connection id only in agent-visible context. No token bytes in events, logs or prompts. Redirect and host allowlists. Revocation fences refresh and invoke.

## 11. Errors and recovery

Cancelled OAuth leaves no Connection. Refresh serialization avoids stampeding the provider. UNKNOWN write blocks retry until reconciliation. Restart reloads Connection status without printing tokens.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

One provider for the golden slice: a GitHub App user-to-server device authorization flow, with device flow enabled and expiring user tokens. Use the public App client ID; do not embed an OAuth client secret or GitHub App private key and do not add a hosted token broker. The app registration must request only repository Issues permission needed by this slice. The user-facing summary explains that `Issues: write` covers the golden issue-comment read/write and that LiteCowork separately narrows each mediated operation through its grant and approval.

Exact network allowlist: invocation calls use only `https://api.github.com`; device-code requests use only `https://github.com/login/device/code`; device authorization polling and refresh use only `https://github.com/login/oauth/access_token`. These fixed auth paths are callable only by ConnectionService, never by an Invocation. Open `https://github.com/login/device` in the system browser; reject any provider-supplied verification URI other than this exact HTTPS origin/path. Disable redirects for auth requests or revalidate the full destination against the fixed allowlist. No arbitrary URL, scheme, port or redirect target is reachable. `offline_access` is the only device-flow `scope` value; it requests token refreshability and does not grant repository permissions.

Connection/auth and provider-read rows qualify per native OS, with Windows x64 first (Windows Credential Manager through `keyring` `windows-native`) and Linux x64 second (Secret Service through `linux-native-sync-persistent`). A row whose keystore is unavailable shows Connect as UNAVAILABLE; there is no plaintext fallback. The Task-scoped write row order is unresolved under A-80. If that write requires a Task Attempt with an admitted Environment, E2E-05 follows Feature 005's Linux-first B-17 rider and Windows Task execution remains UNAVAILABLE until B-35; see §18. A Connection/read row alone never qualifies the complete E2E-05 write journey.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

Positive:
- **AC-003:** WHEN a read runs, THE SYSTEM SHALL require an explicit grant distinct from the Connection record. Verify: grant-required read test.
- **AC-004:** WHEN a write is requested, THE SYSTEM SHALL require a fresh approval bound to the exact plan digest before invocation. Verify: approval-digest binding test.

Security:
- **AC-001:** WHEN the owner starts connect, THE SYSTEM SHALL keep provider token bytes out of Conversation, events and logs. Verify: token-redaction assertion across events/logs/prompt.
- **AC-007:** IF an agent requests install or approval of its own write, THEN THE SYSTEM SHALL deny self-approval. Verify: self-approval denial test.
- **AC-008:** WHEN a worker egress target is loopback, private, link-local, multicast or cloud-metadata, THE SYSTEM SHALL reject it and SHALL NOT let a grant exempt those classes; TLS downgrade is refused and every redirect is revalidated. Verify: SSRF/redirect/TLS egress matrix.

Negative:
- **AC-002:** IF the owner cancels auth, THEN THE SYSTEM SHALL persist no usable Connection. Verify: cancelled-OAuth negative test (AuthAttempt CANCELLED, no Connection).
- **AC-006:** WHEN the owner revokes the Connection, THE SYSTEM SHALL fail later reads and writes for that Connection id. Verify: revoke-fencing test.

Recovery:
- **AC-005:** IF the provider result is UNKNOWN, THEN THE SYSTEM SHALL reconcile before retry and SHALL NOT emit a second consequential call meanwhile. Verify: UNKNOWN reconcile-before-retry test.

Concurrency:
- **AC-009:** WHILE a token refresh is in flight, THE SYSTEM SHALL hold a per-connection lock so concurrent refreshes do not stampede the provider, mapping `invalid_grant` to `REAUTH_REQUIRED` and other failures to `DEGRADED`. Verify: concurrent-refresh lock test.

Boundary:
- **AC-010:** WHEN the golden write body or target exceeds the declared bound, THE SYSTEM SHALL reject the invocation before any provider call. Verify: payload-bound test.

UI and accessibility:
- **AC-011:** WHEN connection and capability states are shown, THE SYSTEM SHALL present connected, granted, approved and invoked as distinct states and SHALL NOT present a catalog row or `health=UNKNOWN` as connected or healthy. Verify: Discover/Needs You render test.

Performance:
- **AC-012:** WHEN a device authorization is pending, THE SYSTEM SHALL enforce the provider's finite expiry, single active poll schedule, and expected AuthAttempt version; late, replayed, cancelled or stale polls SHALL NOT create or replace a Connection. Verify: expiry, duplicate poll, cancellation-race and restart test.

V1 gap coverage (revision 1.0):
- **AC-013:** WHEN a Connection completes, THE SYSTEM SHALL write access and refresh tokens only to the OS keystore through `OsSecretStore` before status becomes CONNECTED, SHALL refuse to connect when the keystore is unavailable, and SHALL resolve token bytes only inside the daemon broker for an admitted Invocation. Verify: keystore-unavailable negative test, no-plaintext-fallback test and resolver reachability test.
- **AC-014:** WHEN the broker or ConnectionService makes an HTTP request, THE SYSTEM SHALL allow only `https://api.github.com` for Invocations, only `https://github.com/login/device/code` and `https://github.com/login/oauth/access_token` for ConnectionService device authorization/poll/refresh, and only `https://github.com/login/device` as the system-browser verification destination. It SHALL refuse any other host, scheme, port, redirect target or provider-returned verification URI. Verify: exact endpoint, redirect and verification-URI allowlist tests.
- **AC-015:** WHEN the owner revokes a Connection, THE SYSTEM SHALL commit REVOKED, delete the keystore entries, fence in-flight and later Invocations, and SHALL show remote revocation as either confirmed or "owner action required" with the GitHub settings link, never as done when unconfirmed. Verify: revoke test with remote-revoke unavailable and keystore-delete failure cases.
- **AC-016:** WHEN the owner connects GitHub, THE SYSTEM SHALL NOT install, adopt or mutate any LiteSPM package, and `connect-or-install` SHALL keep returning its fail-closed 503 until package 013 qualifies. Verify: route regression test and no-LiteSPM-call assertion.

Revision 1.1 device-authorization and permission requirements:
- **AC-017:** WHEN Connect begins, THE SYSTEM SHALL use only GitHub's fixed device-code endpoint and SHALL show the named GitHub App, exact verification URI, permission summary, user code, expiry and cancel control; it SHALL never place the device code or token in UI projections, events, logs, Conversation or agent context. Verify: URL allowlist and secret/code redaction tests.
- **AC-018:** WHEN the owner authorizes the GitHub App, THE SYSTEM SHALL request only the repository `Issues: write` App permission needed by the golden comment read/write pair, explain its provider-level breadth, require both user and App resource access to the repository, and separately enforce an exact repository/operation/expiry CapabilityGrant. Verify: least-permission manifest assertion, missing-App-permission denial, user-access denial and cross-repository grant denial.
- **AC-019:** WHILE device authorization is pending, THE SYSTEM SHALL honor the provider poll interval, `authorization_pending`, `slow_down` (including the added wait), denial and expiry responses, and stop polling immediately on cancel or terminal state. Verify: deterministic poll-clock and state-transition tests.
- **AC-020:** WHEN the owner completes device authorization, THE SYSTEM SHALL request expiring user access and refresh tokens with `offline_access`, persist token bytes only in the OS SecretStore, rotate refresh credentials safely, and require fresh authorization after refresh expiry/revocation. Verify: SecretStore-only custody, refresh rotation/replay, expired-refresh and restart recovery tests.
- **AC-021:** WHEN local Disconnect occurs, THE SYSTEM SHALL immediately fence invocations and remove local tokens; it SHALL describe remote GitHub App authorization revocation as a separate user action in GitHub Settings and SHALL NOT claim remote revocation unless independently observed. Verify: local fence/delete and truthful remote-revocation UI tests.
- **AC-022:** WHEN connection tokens are finalized or refreshed, THE SYSTEM SHALL persist opaque preallocated SecretRef cleanup intent before writing token bytes, SHALL atomically transfer/replace the references with the Connection state, and SHALL reconcile staged references on restart without deleting references owned by a committed Connection. IF a refresh outcome is ambiguous after dispatch, THE SYSTEM SHALL NOT reuse the possibly consumed refresh token and SHALL require reauthorization unless the exact replacement pair is durably reconciled. Verify: crash injection between each SQLite/keystore boundary, partial-write cleanup failure, committed-ref preservation, refresh timeout and restart tests.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/006-golden-connection-capability-write
```

After formal ACCEPTED/LOCKED review, the remaining gates are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not enable arbitrary URLs from the model.
- Do not ship a workflow-builder canvas in this slice.

## 18. Assumptions and open questions

- Provider is GitHub. The old GitHub OAuth App loopback recipe is superseded for implementation planning by revision 1.1's user-delegated GitHub App device flow. Formal domain-owner ratification is pending; do not describe revision 1.1 as ACCEPTED/LOCKED.
- The registered GitHub App's repository Issues permission is broad enough for other issue operations. LiteCowork must state this before authorization and enforce its exact per-Connection, per-repository, per-operation grant, plus exact write approval. Neither an App authorization nor App installation is LiteCowork invocation authority.
- GitHub's user-to-server token is limited by the intersection of the user's resource access and the App's resource access. Repositories must be explicitly confirmed as accessible before any capability action. Device-flow `repository_id` may be used only when the restriction remains valid after refresh; broker-side repository grant checks are always required.
- Device-flow authorization has phishing/impersonation risk. Mitigations are explicit user initiation, exact GitHub verification URI, current App identity and permission display, one active expiring code, no code in chat/agent/logs, clear anti-phishing guidance, and immediate cancel. Residual risk and exact user wording must be tested before qualification.
- Open: does the golden write run inside a Task Attempt? CONVERSATION and TASK_PLANNING Invocations are read-only (§7a), so the write needs a Task. If Task Attempt admission requires an admitted Environment even for a broker-only STRUCTURED_API Step, E2E-05 on the Windows row is UNAVAILABLE until 005's B-35 qualifies. Owners: Trust and Environments. Recorded in 005 SPEC §18 as well.
- Historical connector code in the archive is not served evidence.

Accepted decisions are recorded in §5. Implementation tasks: 006-T01 through 006-T17 in [TASKS.md](TASKS.md).

## 7a. Owner facts and implementation crosswalk checked 2026-11-06 (historical; auth recipe superseded by revision 1.1)

Read from CONNECTORS-WORKFLOWS, CAPABILITY-INVOCATIONS, CAPABILITY-FABRIC, and NETWORK-SECURITY.

- Connection.status in CONNECTORS-WORKFLOWS is CONNECTING, CONNECTED, DEGRADED, REAUTH_REQUIRED, REVOKED, or DISCONNECTED. The OpenAPI Connection status enum omits REVOKED. Promote that value into the schema and API owner together before implementation. account_display_hint is not authenticated identity.
- Historical revision 1.0 used a loopback listener on 127.0.0.1, single-use state and a token-free callback. Revision 1.1 supersedes that recipe with device-flow AuthAttempt identity/version/expiry checks; no loopback listener, callback or PKCE state is used. Both revisions retain the rule that SecretStore succeeds before the Connection becomes CONNECTED and a failed Connection commit must clean up newly written secrets without plaintext fallback.
- Cancelled, denied or expired device authorization records a terminal AuthAttempt state and persists no usable Connection. A terminal attempt cannot be restarted or replaced by a late provider response.
- Refresh holds a per-connection lock. invalid_grant or provider revocation becomes REAUTH_REQUIRED. Other refresh failures become DEGRADED. Refresh does not delete historical Effects and does not start a consequential retry.
- ConnectorActionDescriptor.operation_class is READ, WRITE, SEND, DELETE, PURCHASE_LIKE, or ADMIN. A package READ label cannot override Trust classification. Idempotency is PROVIDER_KEY, CLIENT_KEY, READ_ONLY, or UNKNOWN.
- CapabilityInvocation.execution_method is STRUCTURED_API, STRUCTURED_BROWSER, ACCESSIBILITY_BROWSER, SCREEN_COMPUTER_USE, DETERMINISTIC_LOCAL, NATIVE_AGENT_TOOL, or UNKNOWN. Status includes AMBIGUOUS, which is non-terminal. A timeout is not completion. CONVERSATION and TASK_PLANNING invocations are read-only.
- Credential-like elicitation fails with SENSITIVE_INPUT_UNSUPPORTED. Unknown MCP methods, including sampling, fail with PROVIDER_INPUT_UNSUPPORTED and do not become a UserRequest.
- Worker egress rejects loopback, private, link-local, multicast, and cloud-metadata destinations. A Task grant cannot exempt those classes. Every redirect is revalidated. TLS downgrade is forbidden.
- External dispatch waits until one transaction rechecks owner, lease, grant, activation, Trust decision, and exact ApprovalUse. A PROPOSED Effect is not a dispatch permit.
- Cite listConnections, getConnection, disconnectConnection, createCapabilityGrant, revokeCapabilityGrant, resolveApproval. Event: connection.state.changed.v1. No golden-provider write operationId exists until B-20.


## 7b. Current source and UI boundary checked 2026-11-06

- The composed router serves Discover search/detail, capability connect-or-install, capability test, Approval list/get/resolve, capability-grant create/revoke, Task invocation list and individual invocation get. The served route inventory does not mount the Connection list/get/disconnect operations; those remain target-only.
- `connect_or_install_capability` returns `503 DEPENDENCY_UNAVAILABLE` with `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`, does not create a durable receipt and explicitly returns `requested_scope_granted=false`. `test_capability_connection` returns `health=UNKNOWN` and does not contact a provider.
- `DiscoverPage` presents Explore, Installed/native, Managed and Connections. Explore labels Workspace-local skills and states public package discovery unavailable. Installed/native shows only Runtime-reported capability hosts, not a complete native installation scan. Managed and Connections both render explicit not-available panels. Every row action is disabled by `connectActionOutcome`; it never enables Connect/Use/Open.
- `CapabilityInvocation` API source declares this module does not create, transition or dispatch Invocations. It serves GET only. The SQLite invocation store rejects transition to DISPATCHED with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`; it can record/reconcile local observations, but not admit provider dispatch.
- `apps/litecoworkd/src/trust.rs` explicitly says the policy evaluator does not dispatch providers, issue grants/SecretLeases or create an independently reusable permit; CapabilityBroker admission remains fail-closed until the combined authorization/Effect/audit transaction is implemented. `main.rs` compiles `trust` behind `allow(dead_code)` and says it is not wired.
- A local OS `OsSecretStore` implementation and write-only SecretStore trait exist. The mounted AgentProfile credential-slot route is Feature 002, not an account Connection flow; after checking the current descriptor it returns 503 before writing because exact replay protection is incomplete. No device-authorization/Connection refresh/Connection SecretStore path is served.
- Approval list/get/resolve are real. Approval resolution requires current version, idempotency key, local owner assurance and exact `confirmation_digest` for APPROVE, then commits `approval.resolved.v1`. Needs You ApprovalDetail uses this route and preserves the original decision request ID for replay. The UI says an Approval decision records the exact-action decision only; it neither grants reusable capability authority nor runs the provider operation.
- The only served CapabilityGrant creation route is the Task-planning Resource-read grant path (`workspace.resource.read`) over explicitly enumerated Resource refs. It does not issue the Feature 006 connector read/write scope or a general invocation grant.
- No provider-backed connector read or write was executed in this audit. No Connection was created, no device authorization was started, no token was stored through a Connection flow, no CapabilityInvocation was dispatched, and no external Effect was reconciled.
## 19. Requirement traceability

| Requirement | Canonical contract | Task | Acceptance | Verification method |
|---|---|---|---|---|
| Tokens out of chat/events/logs | NETWORK-SECURITY, CONNECTORS-WORKFLOWS | 006-T05 | AC-001 | Token-redaction assertion |
| Cancelled auth persists no Connection | CONNECTORS-WORKFLOWS | 006-T05 | AC-002 | Cancelled-OAuth test |
| Read requires explicit grant | CAPABILITY-FABRIC, TRUST | 006-T08 | AC-003 | Grant-required read test |
| Write requires fresh digest approval | TRUST, CAPABILITY-INVOCATIONS | 006-T09 | AC-004 | Approval-digest test |
| UNKNOWN reconcile before retry | CAPABILITY-INVOCATIONS, TRUST | 006-T10 | AC-005 | Reconcile-before-retry test |
| Revoke fences later use | CONNECTORS-WORKFLOWS | 006-T11 | AC-006 | Revoke-fencing test |
| Deny self-approval | TRUST | 006-T09 | AC-007 | Self-approval denial test |
| SSRF/redirect/TLS egress guard | NETWORK-SECURITY | 006-T04 | AC-008 | Egress matrix |
| Per-connection refresh lock | CONNECTORS-WORKFLOWS | 006-T06 | AC-009 | Concurrent-refresh test |
| Golden write payload bound | CAPABILITY-FABRIC | 006-T09 | AC-010 | Payload-bound test |
| Distinct connection/capability states | PRESENTATION-RUNTIME, contracts/ui-behavior.md | 006-T13 | AC-011 | Render test |
| Device authorization expiry + stale-attempt fencing | CONNECTORS-WORKFLOWS, NETWORK-SECURITY | 006-T05 | AC-012, AC-019 | Poll interval, duplicate/stale attempt, expiry, cancel and restart tests |
| OS-keystore custody, broker-only resolve | TRUST, NETWORK-SECURITY | 006-T03 | AC-013 | Keystore-unavailable + resolver reachability tests |
| GitHub host allowlist | NETWORK-SECURITY, CONNECTORS-WORKFLOWS | 006-T04 | AC-014 | Allowlist and redirect matrix |
| Local revoke with explicit remote-revoke state | CONNECTORS-WORKFLOWS | 006-T11 | AC-015 | Revoke failure-case tests |
| No LiteSPM lifecycle in 006 | LITESPM-LITECOWORK-INTEROP-LLD, AGENTS.md §6 | 006-T12 | AC-016 | Route regression + no-LiteSPM-call assertion |
| Fixed device endpoints and verification URI | NETWORK-SECURITY, CONNECTORS-WORKFLOWS | 006-T04, T05 | AC-017 | Exact endpoint/URI allowlist and redaction tests |
| GitHub Issues permission vs LiteCowork exact grant | CAPABILITY-FABRIC, TRUST | 006-T01, T08, T09 | AC-018 | Permission manifest, user/App access denial and cross-repository grant tests |
| Device polling, interval and `slow_down` behavior | CONNECTORS-WORKFLOWS | 006-T05 | AC-019 | Deterministic polling clock and terminal-state tests |
| Expiring access/refresh token custody and rotation | TRUST, CONNECTORS-WORKFLOWS | 006-T03, T05, T06 | AC-020, AC-022 | Keyring-only custody, atomic rotation, expiry and restart tests |
| Local disconnect vs GitHub authorization revoke | CONNECTORS-WORKFLOWS | 006-T11, T13 | AC-021 | Local fence/delete and truthful owner-action UI tests |
| Durable staged SecretRef cleanup and ambiguous-refresh recovery | TRUST, CONNECTORS-WORKFLOWS | 006-T01, T03, T05, T06, T14 | AC-022 | Crash boundary, cleanup retry and no-refresh-replay tests |
