# Research - Golden Connection and One Capability Write

**Current decision research:** see [GitHub App device-flow review (2026-11-20)](#github-app-device-flow-review-2026-11-20). The older OAuth App PKCE research below is retained as historical evidence for revision 1.0 and no longer defines the revision 1.1 target.

**Feature:** 006 | **Checked:** 2026-11-06 (source, composed routes and desktop UI); provider docs re-checked 2026-11-12

**Status:** IN_REVIEW - revision 1.1 includes USER-DELEGATED direction; formal domain-owner ratification remains pending. Research records evidence and alternatives, not implementation or qualification.

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

## Borrowable-design sources (pinned)

| Source | Exact pinned URL | License | What informs the golden connection/write | Verdict |
|---|---|---|---|---|
| OpenWork lazy capability gateway (`search_capabilities`/`execute_capability`) | https://github.com/different-ai/openwork/tree/5493b18a6af64d56454658a53da00eabbc6e033d | MIT except `/ee` (Enterprise Edition License) | The lazy gateway admits a named capability only on demand, which maps to the broker loading only a grant-scoped invocation rather than exposing a standing provider surface. | Prototype the lazy-admission shape against CAPABILITY-FABRIC; do not import `/ee` paths or adopt the OpenCode loop. |
| LiteSPM published package/host contract | https://github.com/sarv-projects/LiteSPM/tree/9f42b25ddfb1acedce0f2dbf9964f4a7724d622e | Apache-2.0 at checked root LICENSE | LiteSPM is the selected package lifecycle authority; its published package/host contract is the boundary LiteCowork links to for install, not reimplements. The checked root does not yet settle the LiteCowork-facing wire contract, so no API is inferred. | Reference/boundary only; use the published contract when available, do not invent or duplicate its API. |
| Arcade MCP authorized tool declarations | https://github.com/ArcadeAI/arcade-mcp/blob/79a2dc90d347e4cd1a96366a2d94c8ddc356f1c3/README.md | MIT | Study authorized MCP tool declarations and secret-injection boundaries: tokens are injected by the host, never exposed to the model, which matches the SecretStore custody and token-redaction requirement (AC-001). | Reference only; Arcade MCP is an optional provider, not a required dependency of this slice. |

No OpenWork, LiteSPM or Arcade MCP code is imported; all three are prototype/reference/boundary inputs. OpenBot is not cited here; if it were, it would be reference/UX only with no code reuse under its PolyForm Noncommercial license.

## Source, composed-route and UI audit (2026-11-06)

Inspected `apps/litecoworkd/src/discover_operator.rs`, `capability_invocations_operator.rs`, `approval_operator.rs`, `trust_operator.rs`, `trust.rs`, `agent_credentials_routes.rs`, `main.rs`, `crates/storage-sqlite/src/invocations.rs`, `crates/storage-sqlite/src/trust.rs`, `apps/litecowork-ui/src/discover/DiscoverPage.tsx`, `discover-api.ts`, `discover-state.ts`, `needs-you/ApprovalDetail.tsx`, `needs-you/approval-state.ts`, and the composed `docs/schemas/served-route-inventory.json`.

### Current route truth

- Served read/write operations include Discover search/detail; capability `connect-or-install` and `test`; Approval list/detail/resolve; Task-planning read-grant create/revoke; and CapabilityInvocation task-list/detail GETs.
- The `connect-or-install` route requires authenticated owner Workspace/runtime scope and a bounded idempotency key, but it always returns `503 DEPENDENCY_UNAVAILABLE` with reason `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`. It creates no durable package/install receipt and explicitly reports `requested_scope_granted=false`.
- The `test` route is a no-provider probe. It returns `health=UNKNOWN` and a reason saying the LiteSPM package service is not qualified and no provider was contacted.
- Connection list/get/disconnect operations in the OpenAPI contract are target-only: `GET /connections`, `GET /connections/{connectionId}`, `POST /connections/{connectionId}/disconnect`. No corresponding composed daemon routes exist. No account-Connection device-auth begin/poll/cancel or refresh lifecycle is served.
- CapabilityInvocation Operator exposes only GET list/detail. Its module-level comment says it does not create, transition or dispatch Invocations. The SQLite transition store explicitly rejects transition to DISPATCHED with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`. No production invocation-creation/dispatch caller was found in `apps/litecoworkd/src`.
- The Trust evaluator module is compiled with `allow(dead_code)` in `main.rs`; `trust.rs` itself says it does not dispatch providers, issue grants/SecretLeases or create reusable permits, and that CapabilityBroker admission remains fail-closed until the combined Trust/Effect/audit transaction is implemented.
- `OsSecretStore` is a real local OS keychain adapter and the SecretStore API is write-only, but no Feature 006 account Connection service uses it. The mounted AgentProfile credential-slot route is Feature 002 and currently fails 503 after descriptor validation, before SecretStore write, because exact durable replay protection is not ready.

### Approval, grants and UI

- Approval list/get/resolve are served. Approve requires the current Approval version, an idempotency key, local authenticated owner assurance and an exact `confirmation_digest`; the store commits `approval.resolved.v1` with replay protection. Deny does not need the approval digest.
- `ApprovalDetail` has an actual load/resolve UI. It preserves the same decision request identity after an uncertain response, refreshes status, and says explicitly that approving records the exact-action decision only—it does not grant reusable capability access or run the external action.
- The served CapabilityGrant route currently creates a bounded, expiring Task-planning `workspace.resource.read` grant over explicit Resource refs. It does not establish OAuth/account Connection custody or a connector write grant.
- Discover has Explore, Installed/native, Managed and Connections tabs. Explore identifies Workspace-local skills and states public package discovery unavailable. Installed/native shows only Runtime-reported capability hosts (not all native installations). Managed says package management is not connected. Connections says account connections cannot be read in this build.
- `connectActionOutcome()` returns `allowed=false` for every row: availability-not-ready rows show their truthful state/reason; Workspace-local skill Open/Run is unavailable; package Connect/Use is unavailable until the LiteSPM contract is qualified. All row action buttons are disabled.
- No provider device-auth panel, account Connection list/detail, provider read/write operation picker, grant assignment, or connected→granted→invoked UI exists. Approval detail is a separate, real state transition and is not proof that an invocation ran.

These are source/route/UI findings. No LiteCowork account Connection was created, device authorization was not started, no token was stored through a Connection flow, no connector provider was called, no CapabilityInvocation was dispatched and no provider Effect was reconciled.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; the LiteSPM bridge pin was resolved 2026-11-09 to HEAD `84d5fab` (v0.6.0, [LITESPM-INTEGRATION-STUDY-2026-11.md](../../docs/LITESPM-INTEGRATION-STUDY-2026-11.md)), superseding the earlier `111fd9b8e84573f8fce76b6374b4ff9639502539` as checked on 2026-11-05, not as proof of LiteCowork integration. GitHub provider documentation was rechecked on 2026-11-12; see the current supplemental check below.
- No LiteCowork runtime or end-to-end bridge integration was executed. The upstream source review is static and commit-pinned; it does not qualify a local LiteCowork process launch, IPC identity, host mutation, or receipt/recovery path.

## Historical provider documentation check (2026-11-09; revision 1.0)

| Source | Finding | Effect on this package |
|---|---|---|
| [GitHub: Authorizing OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps) | PKCE supported with `S256` only; loopback `redirect_uri` on any port at `127.0.0.1` is accepted; `client_secret` is listed as required for the web-flow code exchange and for refresh unless the token came from the device flow; the device flow needs no client secret; token exchange is at `github.com/login/oauth/access_token`. | This was the evidence that invalidated revision 1.0's loopback recipe. The old T02 stop state is superseded by the 2026-11-20 USER-DELEGATED revision 1.1 direction below. The OAuth token endpoint remains a separate allowlisted auth host. |
| [GitHub REST: OAuth authorizations](https://docs.github.com/en/rest/apps/oauth-applications) | `DELETE /applications/{client_id}/token` requires Basic auth with the client secret. | Remote revocation is not possible without a client secret; revoke is local first and remote revoke may be an owner action (AC-015). |

## Historical provider documentation check (2026-11-12; revision 1.0)

The official GitHub OAuth pages were re-read after the 2026-11-09 check. The authentication finding is unchanged: the web-application code exchange requires `client_secret` even when PKCE is used, while the device-flow token request does not; refreshing device-flow tokens also omits the secret. Token revocation by `DELETE /applications/{client_id}/token` requires Basic authentication with the client id and secret. This historical evidence invalidated the accepted loopback web-flow recipe; the former T02 stop state and undecided-choice conclusion were superseded by the USER-DELEGATED revision 1.1 direction recorded below.

The provider's permission model also invalidated revision 1.0's operation-specific scope wording. GitHub documents `repo:status` as commit-status access, `read:user` as profile read access, and `repo`/`public_repo` as broader OAuth App access for repository issues; OAuth Apps cannot request fine-grained Issues permissions. The `list issue comments` read and `create issue comment` write therefore cannot be described as separate least-privilege OAuth scopes. Revision 1.1 now records the delegated GitHub App permission direction and separates that provider permission from LiteCowork's exact operation grant. See the current provider facts and limits below; this does not represent formal owner ratification or a qualified provider flow.

Primary sources checked 2026-11-12: [GitHub OAuth authorization](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps), [OAuth scopes](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps), [OAuth App vs GitHub App permissions](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/differences-between-github-apps-and-oauth-apps), [issue-comment REST operations](https://docs.github.com/en/rest/issues/comments), and [OAuth token revocation](https://docs.github.com/en/rest/apps/oauth-applications).

Content was rephrased for compliance with licensing restrictions.

## GitHub App device-flow review (2026-11-20)

### Current documented provider facts

- GitHub's official [user access token guide](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app) explicitly recommends device flow for desktop applications. The flow uses the GitHub App **client ID** (public identifier), `POST https://github.com/login/device/code`, and then `POST https://github.com/login/oauth/access_token`; it does not use a loopback callback. The fixed verification URI is `https://github.com/login/device`.
- The device response supplies a `device_code`, user-facing `user_code`, expiry and polling interval. Current documentation lists a default 900-second expiry and 5-second interval; the client must obey the returned values, stop when terminal, and add five seconds on `slow_down`. These provider defaults are not hardcoded product guarantees.
- GitHub App device flow must be enabled in the App registration. `offline_access` is the only supported device-flow scope value; it requests an expiring access/refresh pair and does not grant repository permissions. Current docs describe an eight-hour access-token lifetime and six-month refresh-token lifetime. Refreshing rotates both tokens; the consumed pair must be replaced atomically. Refresh of a user token generated by device flow does not require a client secret. GitHub notes expiration is an optional App feature and behavior can change, so T03/T05 must validate returned lifetimes and fail closed on malformed/unsupported responses.
- GitHub App user access tokens use fine-grained permissions rather than OAuth App scopes. Access is constrained by the user's own access and the App's permissions/resource access. GitHub's [user authorization guide](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-with-a-github-app-on-behalf-of-a-user) documents that the App must have Issues permission and access to the account/repository, while the user must also be able to access the resource.
- GitHub's [permission reference](https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps) says `GET /repos/{owner}/{repo}/issues/{issue_number}/comments` requires Issues read and `POST /repos/{owner}/{repo}/issues/{issue_number}/comments` requires Issues write for GitHub App user access tokens. For the full accepted read/write slice, App permission `Issues: write` is the smallest single permission that covers both. It remains broader than LiteCowork's one mediated operation; per-operation and per-repository authorization must remain in LiteCowork's exact grant and approval boundary.
- The device-flow request accepts an optional `repository_id` restriction. It is a useful defense in depth only after its behavior through refresh and selected-repository changes is verified against the pinned API behavior; it does not replace a LiteCowork grant check on every call. GitHub App installation selection is also separate from user authorization and LiteCowork grants.
- GitHub's [token refresh guide](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens) says refreshed access and refresh tokens rotate, and the old credentials stop working. GitHub's [authorization review guide](https://docs.github.com/en/apps/using-github-apps/reviewing-and-revoking-authorization-of-github-apps) lets the authorizing user revoke the App from account Settings → Applications → Authorized GitHub Apps. LiteCowork should not claim that its local Disconnect remotely revoked GitHub authorization.
- Device authorization has a documented phishing/impersonation risk. The [OAuth App best-practices page](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/best-practices-for-creating-an-oauth-app) specifically cautions about device flow; the GitHub App user-token guide itself establishes desktop support. The choice is justified by the local desktop, no-confidential-secret constraint, and exact App permission model, with user guidance and explicit cancellation required. This residual risk must be included in UI review and E2E denial evidence.

### Alternatives and decision

| Option | Provider facts | Decision |
|---|---|---|
| GitHub OAuth App web authorization-code + PKCE/loopback | PKCE does not remove the documented `client_secret` requirement for token exchange/refresh; OAuth scopes are too broad or semantically wrong for the issue-comment operations. | Rejected for a native desktop binary because embedding a confidential secret is not secure and the scope semantics are false. |
| GitHub OAuth App device flow | Avoids the loopback callback and client secret, but retains coarse OAuth App scope limitations and has device-code phishing risk. | Not selected; GitHub App offers the fine-grained permission model the capability needs. |
| GitHub App user-to-server device flow | Desktop-supported, uses a public client ID, no client secret/private key in the local client, expiring/refreshable tokens, and App-level Issues permissions. User and App access intersect. Device flow has phishing risk and revocation is user-managed. | **USER-DELEGATED DIRECTION RECORDED 2026-11-20.** Use for the first GitHub Connection; only request Issues write for the golden comment read/write pair; keep LiteCowork grants narrower and exact. Formal Connectors/Security review remains pending. |
| Hosted token broker | Could hide credentials but moves auth/token custody off-device and violates the local-first, local-runtime boundary. | Rejected. |
| Different provider or operation | Not necessary to preserve the accepted GitHub issue-comment outcome; would add provider scope without resolving an architectural blocker. | Not selected; the full V1 connector capability remains. |

### Implementation constraints carried into revision 1.1

1. App client ID is public configuration, not a secret. No App private key or OAuth client secret is shipped. App registration and its exact permission manifest are external setup prerequisites and must be pinned/documented before a real sign-in test.
2. Auth requests use fixed hosts and exact paths only. No arbitrary auth URL, redirect following, user-supplied endpoint, or API-host confusion is allowed. Device authorization and refresh are ConnectionService calls, never Invocation calls.
3. `device_code` is transient daemon-only input; the user code is returned only to the explicitly authorized settings UI. Neither enters logs, events, agent prompts or Conversation. On cancel, expiry or process restart, the attempt is terminal and the user starts a new flow.
4. The UI names the App, displays the exact verification URI, explains the Issues permission breadth, includes anti-phishing guidance, and exposes immediate cancellation. Never silently start an auth flow as a side effect of package install, action execution or agent request.
5. Before enabling a golden read/write, verify authenticated user identity, App permission, App/user repository access, exact repository grant, Connection status and keystore readiness. Provider access never substitutes for LiteCowork authorization or Trust.
6. User revocation, refresh expiration, refresh-token rotation, denied permissions, missing App installation/resource access and unavailable keyring all have explicit states. Unknown auth/refresh outcomes never become CONNECTED or a retry permission.

This is pinned research, not provider registration or qualification. Documentation was reviewed on 2026-11-20; implementation must recheck the exact endpoints, error shapes, intervals and token-expiration configuration before shipping.

## Implementation placement notes

- Keep OAuth, token custody and brokered HTTP in the Rust daemon (`apps/litecoworkd`), not in TypeScript or the Tauri shell, so verifiers and tokens never reach the WebView process. The daemon already has `reqwest` with rustls.
- Reuse `OsSecretStore` and its `keyring` backends; add a crate-private resolver and a delete rather than a second secret store.
- Reuse `SqliteCapabilityDispatchStore::admit_capability_dispatch` rather than writing a second admission path.
- Keep UI state logic in small TypeScript modules (`connections-state.ts`) so it is testable with the existing `node --test` runner.

## Unresolved questions

- Formal Connectors/Security ratification of the USER-DELEGATED revision 1.1 direction (A-82); T01 contract promotion also retains its explicit Feature 005 shared Trust/Effect prerequisite. This is a review and dependency gate, not a reason to restore the superseded OAuth App recipe or shrink the V1 Connection capability.
- Whether the Task-scoped golden write needs an admitted Environment (SPEC §18; affects the Windows row through 005 B-35).
- Confirmation of the golden read operation (proposed: list issue comments), recorded in T01.

## DeepSeek Harness proposals (PROPOSED, owner review required)

Research note, not a decision. Source: [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159) at the pinned SHA (MIT, Copyright (c) 2026 DeepSeek). Local read-only clone `repo_take/deepseek-harness` is gitignored and never staged. Upstream paths below were checked to exist at that SHA on 2026-11-10.

Read-depth limits from the study (do not over-trust): `scripts/` about 58% read, tests about 5% read, installer UI text partly unread. See [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 0 and 15 before turning any item into a design.


Plugin-architecture lessons (see [plugin study](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) Section 8):
  - Fail-closed reachability probe before network operations: [packages/boot/plugin-manager/src/git-transport.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/git-transport.ts). Class **ADAPT**. Probe before a golden connection write. Reuse: pattern only, reimplement; no upstream code imported.
  - Credential refusal in approvals and install specs: [packages/boot/plugin-manager/src/install-spec.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/install-spec.ts). Class **ADOPT (small rule)**. Tokens never appear in config, specs or prompts. Rule only, reimplement; MIT notice if text is carried over. Reuse: pattern only, reimplement; no upstream code imported.

