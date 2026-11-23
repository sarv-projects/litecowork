# Tasks - Golden Connection and One Capability Write

**Feature:** 006 | **Status:** IN_REVIEW - revision 1.1 records USER-DELEGATED direction; owner ratification remains a separate gate.
**Spec revision:** 1.1 | **Release target:** V1 (release matrix rows remain subject to the platform dependencies and evidence gates below)

Boundary: this package owns one GitHub account Connection, one scoped read, one approved write (create an issue comment), UNKNOWN reconciliation and revoke. It does not own the LiteSPM package lifecycle; that is package 013. Nothing here installs, adopts or mutates a LiteSPM package.

## Current baseline

Historical source crosswalk checked 2026-11-09 on branch `docs/audit-competitor-superset`; Connection-route, service and storage boundaries were rechecked against current source on 2026-11-20. No Connection service/storage writer, device-authorization journey or external provider dispatch is served. The delegated recipe direction is recorded in revision 1.1; T01 canonical contract/schema promotion remains NOT STARTED pending its listed Feature 005 Trust/Effect prerequisite.

- **Discover and connect:** `apps/litecoworkd/src/discover_operator.rs` `connect_or_install_capability` always returns 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` with no receipt; `test_capability_connection` returns `health=UNKNOWN`. `apps/litecowork-ui/src/discover/DiscoverPage.tsx` renders the Connections tab as "Connections cannot be read here"; `connectActionOutcome` in `discover-state.ts` disables every row action.
- **Connection storage:** `connections` table exists in `docs/schemas/sqlite-v1.sql` with `status CHECK (... 'DISCONNECTED')` (no `REVOKED`) and `secret_refs_json`; no Rust store reads or writes it. OpenAPI `listConnections`, `getConnection`, `disconnectConnection` are target-only.
- **Secrets:** `SecretStore` (`crates/storage-core/src/trust.rs`) is write-only (`write` only, no read, no delete). `OsSecretStore` (`crates/storage-sqlite/src/trust.rs`) uses `keyring` 3.6 with `windows-native`, `linux-native-sync-persistent` and `apple-native`; its private `SecretCredentialBackend` has `remove_secret`. `SecretLeaseStore::issue_secret_lease` is Task/Attempt-scoped.
- **Dispatch:** `SqliteCapabilityDispatchStore::admit_capability_dispatch` (`crates/storage-sqlite/src/capability_dispatch.rs`) implements the combined Trust + ApprovalUse + Invocation + Effect transaction but fails closed because `docs/schemas/sqlite-v15.sql` triggers `capability_invocation_creation_admission_closed` and `capability_invocation_status_transition_guard` reject Invocation INSERT and any `DISPATCHED` transition. No daemon caller. `apps/litecoworkd/src/capability_invocations_operator.rs` is read-only (`get_invocation`, `list_task_invocations`). `crates/domain-invocations` has `InvocationStatus`, `CapabilityInvocationRecord::transition` and `reconcile_ambiguous_terminal`; `capability_bridge.rs` has `classify_agent_capability_route`.
- **Trust and approval:** `apps/litecoworkd/src/trust.rs` is compiled `#[allow(dead_code)]` in `main.rs`. Approval list/get/resolve with exact `confirmation_digest` is served (`approval_operator.rs`) and used by `apps/litecowork-ui/src/needs-you/ApprovalDetail.tsx`. `trust_operator.rs` `create_read_grant` issues only `workspace.resource.read` grants.
- **HTTP:** `apps/litecoworkd/Cargo.toml` already has `reqwest` 0.12 (`rustls-tls`, no default features); no brokered egress policy exists.
- **Historical provider finding (2026-11-12):** GitHub OAuth App loopback exchange needs a client secret and its OAuth scopes do not map to the issue-comment permissions. `repo:status` is for commit statuses and `read:user` is for profile data; this explains why revision 1.0 could not safely proceed. The current direction is recorded in T02 and the 2026-11-20 RESEARCH supplement.

Gaps to V1: canonical contract/schema promotion; Connection aggregate and service; keystore custody with a broker-only resolver; egress policy; refresh; Invocation dispatch schema; golden read and write; reconciliation; revoke; routes and bridge; UI; recovery; and qualification on supported rows. The B-19 implementation direction is recorded under user delegation; formal Connectors/Security ratification remains pending and is not a substitute for any implementation or qualification gate.

## 006-T01 - Promote the directed 006 contracts into owner docs and machine schemas

- **Status:** NOT STARTED (T02 direction recorded; canonical contract promotion is next).
- **Depends on:** T02
- **Cross-package prerequisites:** 005 contract promotion (shared Trust/Effect wording); orchestrator assigns the migration number because several packages promote migrations in parallel
- **Owner:** Connectors, Capability Fabric, schema owner
- **Allowed paths:** `docs/CONNECTORS-WORKFLOWS.md`, `docs/CAPABILITY-INVOCATIONS.md`, `docs/CAPABILITY-FABRIC.md`, `docs/NETWORK-SECURITY.md`, `docs/TRUST.md`, `docs/STORAGE.md` (logical storage and recovery summary), `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `docs/schemas/sqlite-v41.sql` (new, provisionally allocated in docs/LITECOWORK-BUILD-TASKS.md; v29 is already taken by Feature 001), `specs/006-golden-connection-capability-write/`
- **Contract:** SPEC §5 B-19/B-20, §7.2, §7a, §13; AC-006, AC-013, AC-014, AC-015; A-82
- **Change intent:** Promote revision 1.1's device-authorization lifecycle into canonical owner docs and OpenAPI/schema together. Add `REVOKED` to the Connection status enum and a forward-only `connections` rebuild preserving rows. Define begin, poll, cancel, list/get/disconnect operations without a browser callback route; add an `auth_attempts` table with purpose (`DEVICE_AUTH | TOKEN_REFRESH`), optional Connection identity, state, expiry, version, non-secret correlation metadata and private preallocated SecretRef IDs for token finalization/cleanup (never device/user codes, token bytes, client secret, App private key or PKCE verifier). Define exact state transitions for `TOKEN_CUSTODY_PENDING`, `SUCCEEDED`, `CLEANUP_PENDING`, `FAILED`, `CANCELLED`, `DENIED`, and `EXPIRED`; AuthAttempt-to-Connection transfer occurs in one transaction. Define safe refresh ambiguity recovery: after uncertain dispatched refresh, never reuse the possibly consumed refresh token; move the Connection to REAUTH_REQUIRED unless the exact replacement pair was durably staged and reconciled. Define the exact GitHub App permission summary (`Issues: write` for this golden read/write pair), accessible-repository verification and exact LiteCowork grant target. Preserve `remote_revocation` truth (`CONFIRMED | OWNER_ACTION_REQUIRED | NOT_ATTEMPTED`): local disconnect fences and deletes credentials, while GitHub authorization revocation is a separate user action in account Settings. Reuse `connection.state.changed.v1`; add events only if missing. Do not encode provider-wide Issues permission as per-operation LiteCowork authority.
- **Acceptance:**
  - [ ] Owner prose and OpenAPI/JSON Schema/migration change in the same commit.
  - [ ] Migration rebuild keeps existing `connections` rows and foreign keys from `channel_bindings` and later tables; a test opens a v28-era database and upgrades.
  - [ ] No secret material column is added; private AuthAttempt staging stores only opaque SecretRef IDs and never exposes them through public events/projections.
  - [ ] Startup can identify and remove partially written keyring slots after a crash without enumerating arbitrary user keyring entries or deleting refs owned by committed Connections.
  - [ ] Canonical docs and machine contracts reflect T02's USER-DELEGATED direction, display provider Issues permission separately from LiteCowork's exact grant, and do not claim owner acceptance or qualification.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`; `cargo test -p storage-sqlite --locked migration`
- **Evidence:** commit SHA in the completion table.
- **Stop:** stop if `REVOKED` conflicts with another owner's Connection state machine; resolve at CONNECTORS-WORKFLOWS first.

## 006-T02 - GitHub authentication and effective-permission decision

- **Status:** DONE (USER-DELEGATED DIRECTION RECORDED 2026-11-20; formal owner ratification remains pending)
- **Decision boundary:** The product owner explicitly delegated internal product/engineering decisions and implementation. This records the selected direction for contract work; it does not claim domain-owner approval, package ACCEPTED/LOCKED status, external App registration, credentials, integration or qualification.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Repository owner; Connectors; Security
- **Allowed paths:** `specs/006-golden-connection-capability-write/`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/audit/EXTERNAL-AI-AUDIT-DISPOSITION-2026-11-19.md` (orchestrator audit reconciliation)
- **Contract:** SPEC §5 B-19, A-82, the provider checks in RESEARCH.md and SPECIFICATION-STANDARD §7
- **Change intent:** Compare GitHub OAuth App loopback web flow, OAuth device flow, GitHub App user-to-server device flow, and switching provider/operation. Re-verify official documentation for desktop suitability, secret custody, authorization and installation/resource boundaries, exact Issues permissions, token lifetime and rotation, polling/expiry behavior, phishing risk, and revocation. Record the selected GitHub App user-to-server device flow with `offline_access`, expiring tokens, app permission `Issues: write`, repository-access intersection, a narrow LiteCowork operation grant, fixed provider endpoints, and truthful local-versus-remote revoke UX. Keep the old OAuth App recipe historical; do not shrink the V1 connector capability. Record the direction in SPEC 1.1, RESEARCH, UI/API/recovery contracts and A-82/B-19. Formal Connectors/Security review remains due and is not implied by this delegated task.
- **Acceptance:**
  - [x] Decision recorded 2026-11-20 with selected option and official GitHub documentation links/current behavior in RESEARCH.md.
  - [x] Internal design direction resolves A-82 using App permission semantics; it makes no operation-specific OAuth-scope claim.
  - [x] AC-002, AC-012 and lifecycle contracts describe device-flow cancel, expiry and polling without a loopback listener.
  - [ ] Connectors/Security domain owners ratify revision 1.1; this governance gate does not erase or defer the user's selected V1 capability.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** SPEC revision 1.1, current-source RESEARCH entry, A-82/B-19 register rows and documentation validation. This task provides design evidence only.
- **Stop:** stop only if canonical Connection, Security or API contracts make the directed flow incompatible; record and reconcile that conflict. Do not mark the package accepted, integrated or qualified from this task.

## 006-T03 - OS-keystore secret custody with a broker-only resolver and delete (Rust)

- **Status:** PARTIAL (`OsSecretStore::write` and `keyring` backends exist; no resolve or delete on the public trait)
- **Depends on:** T01
- **Cross-package prerequisites:** 011 per-row keystore qualification scripts (`scripts/qualify-os-credential-store.sh` exists without an owner)
- **Owner:** Security, Storage
- **Allowed paths:** `crates/storage-core/src/trust.rs`, `crates/storage-sqlite/src/trust.rs`, `crates/storage-sqlite/Cargo.toml`, `apps/litecoworkd/src/connection_secret_broker.rs` (new)
- **Contract:** SPEC §7.2 secret custody paragraph; NETWORK-SECURITY; AC-001, AC-013
- **Change intent:** Extend SecretStore with create-only `write_at(secret_ref, workspace_id, secret_kind, secret_bytes)` and idempotent `remove(secret_ref)`. T01 persists caller-generated opaque SecretRef IDs in the private AuthAttempt before any keyring write. `write_at` must never silently replace an occupied reference; an uncertain/duplicate create keeps the staged reference for cleanup and requires explicit reconciliation or a fresh auth attempt. Add a separate crate-private `SecretResolver` trait with scoped `with_secret` so bytes live only inside the closure and are zeroized after. Only `ConnectionSecretBroker` in `apps/litecoworkd` holds a resolver; it is not exported to Operator handlers or Tauri. Add `SecretKind::OAuthRefreshToken` only if `Token` cannot distinguish access and refresh tokens. Add a keystore availability probe used by the UI to show Connect as UNAVAILABLE.
- **Acceptance:**
  - [ ] Keystore unavailable (locked Secret Service, no Credential Manager access) makes Connect fail closed with no plaintext fallback (AC-013).
  - [ ] Resolver is unreachable from any route or command (compile-time visibility test plus route inventory check) (AC-013).
  - [ ] No token bytes in logs, events or `Debug` output; `SecretRef` has no byte field (AC-001).
  - [ ] `remove` on an already-removed ref succeeds; on keystore error returns a typed error the revoke path can surface.
- **Command:** `cargo test -p storage-sqlite --locked secret_store` (filter to be added by this task); `cargo test -p litecoworkd --locked connection_secret_broker` (filter to be added by this task)
- **Evidence:** test names; per-row keystore probe output.
- **Stop:** stop if the resolver would need to be public to satisfy a caller.

## 006-T04 - Brokered HTTP egress policy: allowlist, TLS, SSRF, redirects, bounds, redaction (Rust)

- **Status:** NOT STARTED (`reqwest` with `rustls-tls` is a dependency; no egress policy module)
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Security, Connectors
- **Allowed paths:** `apps/litecoworkd/src/brokered_http.rs` (new), `apps/litecoworkd/src/main.rs`, `apps/litecoworkd/Cargo.toml`
- **Contract:** SPEC §6 trust boundaries, §13 host allowlist; NETWORK-SECURITY; AC-008, AC-014
- **Change intent:** One `BrokeredHttpClient` built on `reqwest` with redirects disabled at the client and revalidated manually, TLS verification always on with rustls, HTTPS only, a fixed allowlist (`api.github.com` for Invocations; `github.com/login/device/code` and `github.com/login/oauth/access_token` for ConnectionService only), DNS resolution checked against loopback, private, link-local, multicast and cloud-metadata ranges before connect (pin the resolved address for the request), request and response byte limits, total timeout, and header redaction for `Authorization` in every log path.
- **Acceptance:**
  - [ ] Requests to any non-allowlisted host, `http://`, a non-443 port, an IP literal or a DNS name resolving to a private or metadata address are refused before connect (AC-008, AC-014).
  - [ ] A 3xx to a non-allowlisted host is refused; a 3xx within the allowlist is revalidated (AC-014).
  - [ ] Oversized response is cut at the limit and reported as a bounded failure, not success.
  - [ ] No log line contains an `Authorization` value.
- **Command:** `cargo test -p litecoworkd --locked brokered_http` (filter to be added by this task)
- **Evidence:** test names; egress matrix table.
- **Stop:** stop if any path needs TLS verification disabled.

## 006-T05 - ConnectionService: GitHub App device authorization, Connection commit after keystore write

- **Status:** NOT STARTED (T01 contract promotion is prerequisite; no ConnectionService exists)
- **Decision boundary:** The selected device-flow direction is recorded in T02. Formal owner ratification remains pending; do not claim acceptance or qualification. Qualify the standalone Connection path on native Windows x64 then native Linux x64 as required by platform evidence; WSL2 and containers never count. E2E-05 Task-write ordering remains separately tracked under A-80.
- **Depends on:** T01, T02, T03, T04
- **Cross-package prerequisites:** 011 system-browser open via Tauri and Runtime readiness
- **Owner:** Connectors
- **Allowed paths:** `apps/litecoworkd/src/connection_service.rs` (new), `crates/storage-core/src/connection.rs` (new), `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/connections.rs` (new), `crates/storage-sqlite/src/lib.rs`
- **Contract:** SPEC §6 actor sequence 1-3, §7.2 `ConnectionService`, §7a; CONNECTORS-WORKFLOWS; AC-001, AC-002, AC-012, AC-013, AC-017, AC-019, AC-020, AC-022
- **Change intent:** Implement the device-authorization begin, poll and cancel lifecycle as `ConnectionService` in Rust in the daemon, not in TypeScript or the Tauri shell, so device codes and tokens never enter an untrusted renderer. Use only the fixed GitHub App device-code and token endpoints; show the user code and exact verification URI; poll no faster than the provider interval; apply `slow_down`; and stop on cancel, denial, expiry or daemon shutdown. Keep `device_code` in bounded daemon memory only. Before writing token bytes, persist preallocated opaque access/refresh SecretRef IDs and `TOKEN_CUSTODY_PENDING`; use `SecretStore::write_at` for those refs. One transaction commits Connection CONNECTED + AuthAttempt SUCCEEDED + event + receipt and transfers the refs. If that commit fails, persist/retain cleanup state and remove staged refs idempotently; restart resumes cleanup, not authorization.
- **Acceptance:**
  - [ ] Cancel or `access_denied` records AuthAttempt CANCELLED and no usable Connection (AC-002).
  - [ ] Replayed/stale poll, expired device code, wrong AuthAttempt version or poll after cancel/restart cannot create or replace a Connection (AC-012, AC-019).
  - [ ] Keystore write failure leaves no Connection; crash between intent/write/Connection commit removes staged refs before new auth; a committed Connection's refs survive recovery (AC-013, AC-022).
  - [ ] Token bytes absent from events, logs, Conversation and agent context (AC-001).
  - [ ] Two concurrent begin requests for the same Workspace and provider produce two independent AuthAttempts; only one Connection results per completed attempt.
- **Command:** `cargo test -p litecoworkd --locked connection_service` (filter to be added by this task); `cargo test -p storage-sqlite --locked connections` (filter to be added by this task)
- **Evidence:** test names; recorded live connect on each row with token redaction check.
- **Stop:** stop if any token would be passed to the WebView or written outside the keystore.

## 006-T06 - Token refresh with per-connection lock and state mapping

- **Status:** NOT STARTED
- **Depends on:** T05
- **Cross-package prerequisites:** NONE
- **Owner:** Connectors
- **Allowed paths:** `apps/litecoworkd/src/connection_service.rs`, `crates/storage-sqlite/src/connections.rs`
- **Contract:** SPEC §7a refresh rules; AC-009, AC-020, AC-022
- **Change intent:** Refresh under a per-Connection async mutex. Persist a refresh attempt and preallocated replacement SecretRefs before sending the single-use refresh token. `invalid_grant`/`bad_refresh_token` -> REAUTH_REQUIRED; a timeout, response loss or restart after dispatch is ambiguous and must not retry the consumed refresh token; resolve to REAUTH_REQUIRED and require a new explicit device flow unless the exact replacement pair was durably staged and reconciled. On a definite response, write the new pair idempotently, atomically swap Connection SecretRefs and mark refresh success, then remove old refs. Other failures before dispatch -> DEGRADED. Refresh never starts a consequential retry.
- **Acceptance:**
  - [ ] Ten concurrent Invocations needing refresh cause one provider refresh call (AC-009).
  - [ ] `bad_refresh_token` maps to REAUTH_REQUIRED; network error maps to DEGRADED; neither deletes Effect history.
  - [ ] Crash before refresh dispatch preserves the old pair; crash or timeout after dispatch never blindly retries the single-use refresh token, leaves Connection REAUTH_REQUIRED unless the new pair is durably reconciled, and cleans unowned staged refs (AC-020, AC-022).
- **Command:** `cargo test -p litecoworkd --locked connection_refresh` (filter to be added by this task)
- **Evidence:** test names.
- **Stop:** stop if refresh would need to run inside a SQLite transaction.

## 006-T07 - Open Invocation creation and DISPATCHED through the combined admission transaction only

- **Status:** PARTIAL (`admit_capability_dispatch` implemented and tested; `sqlite-v15.sql` triggers keep it closed)
- **Depends on:** T01
- **Cross-package prerequisites:** 005 Trust wiring (removal of `trust` dead code) and Attempt admission, because Invocation dispatch rechecks lease and Trust decision
- **Owner:** Storage, Capability Fabric
- **Allowed paths:** `docs/schemas/sqlite-v41.sql` (same migration as T01), `crates/storage-sqlite/src/capability_dispatch.rs`, `crates/storage-sqlite/src/invocations.rs`, `crates/storage-sqlite/src/invocation_dispatch_migration_tests.rs`, `crates/storage-core/src/capability_invocation.rs`
- **Contract:** CAPABILITY-INVOCATIONS; SPEC §6 ("External dispatch waits until one transaction rechecks owner, lease, grant, activation, Trust decision, and exact ApprovalUse"); AC-004, AC-005, AC-007
- **Change intent:** Replace the two v15 triggers with guards that allow INSERT and the `CREATED -> DISPATCHED` transition only when the same transaction writes the matching ApprovalUse/Effect rows (for example a trigger that checks a per-transaction admission marker row written by `admit_transaction`). Direct `transition` to DISPATCHED through `SqliteCapabilityInvocationStore` stays refused with `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`.
- **Acceptance:**
  - [ ] `admit_capability_dispatch` commits Invocation + Effect + ApprovalUse + events + receipt atomically; a rejected admission writes nothing.
  - [ ] Direct INSERT or DISPATCHED update outside the admission path still aborts.
  - [ ] ApprovalUse is single-use: a second admission with the same Approval fails.
  - [ ] Duplicate admission with the same idempotency key returns the original.
- **Command:** `cargo test -p storage-sqlite --locked capability_dispatch`; `cargo test -p storage-sqlite --locked invocation_dispatch_migration`
- **Evidence:** test names.
- **Stop:** stop if the guard can be bypassed by any single-statement write.

## 006-T08 - Golden read through the broker with an explicit connector read grant

- **Status:** NOT STARTED
- **Depends on:** T04, T05, T07
- **Cross-package prerequisites:** 005 Trust wiring for grant evaluation
- **Owner:** Capability Fabric, Connectors
- **Allowed paths:** `apps/litecoworkd/src/capability_broker.rs` (new), `apps/litecoworkd/src/trust_operator.rs`, `crates/storage-sqlite/src/trust.rs`, `crates/domain-invocations/src/lib.rs`
- **Contract:** SPEC §6 step 3; CAPABILITY-FABRIC; AC-003
- **Change intent:** Add a connector read grant kind (Connection id + operation id + repository scope + expiry) distinct from the Connection record; the broker classifies the read via `ConnectorActionDescriptor.operation_class = READ`, admits it, calls `api.github.com` with the token from `ConnectionSecretBroker`, bounds and normalizes the response as untrusted, and records Invocation SUCCEEDED/FAILED with Evidence.
- **Acceptance:**
  - [ ] Read without a grant is refused even with a CONNECTED Connection (AC-003).
  - [ ] Grant for repository A cannot read repository B.
  - [ ] Provider response content is treated as untrusted data in any agent-visible projection.
- **Command:** `cargo test -p litecoworkd --locked capability_broker_read` (filter to be added by this task)
- **Evidence:** test names; live read on each row.
- **Stop:** stop if the agent could supply an arbitrary URL or operation.

## 006-T09 - Golden write: create issue comment with exact approval, bounds and self-approval denial

- **Status:** NOT STARTED
- **Depends on:** T07, T08
- **Cross-package prerequisites:** 005 Attempt admission (the write runs under a Task; see SPEC §18 open question on Environment need)
- **Owner:** Capability Fabric, Trust
- **Allowed paths:** `apps/litecoworkd/src/capability_broker.rs`, `apps/litecoworkd/src/trust.rs`, `apps/litecoworkd/src/approval_operator.rs`, `crates/domain-invocations/src/lib.rs`
- **Contract:** SPEC §5 B-20, §7.2 `GoldenWriteInvocation`; TRUST; AC-004, AC-007, AC-010
- **Change intent:** The broker builds the action digest from (connection id, owner, repo, issue number, body SHA-256, client tag); the Trust evaluator classifies WRITE and produces an Approval bound to that digest; the owner resolves it in Needs You `ApprovalDetail`; then `admit_capability_dispatch` commits and only after commit the broker POSTs the comment. The body carries a client tag (an HTML comment `<!-- litecowork:{invocation_id} -->`) so reconciliation can find it. Body bound: GitHub's 65,536-character limit and a LiteCowork byte bound checked before any call.
- **Acceptance:**
  - [ ] Write without a fresh Approval for the exact digest is refused; a changed body after approval is refused (AC-004).
  - [ ] An agent principal requesting approval of its own write is denied (AC-007).
  - [ ] Over-bound body or invalid target is refused before any provider call (AC-010).
  - [ ] No provider call happens before the admission transaction commits.
- **Command:** `cargo test -p litecoworkd --locked capability_broker_write` (filter to be added by this task)
- **Evidence:** test names.
- **Stop:** stop if the write could be issued from a CONVERSATION or TASK_PLANNING Invocation.

## 006-T10 - UNKNOWN reconciliation for the comment write

- **Status:** PARTIAL (`reconcile_ambiguous_terminal` in `crates/domain-invocations/src/lib.rs`; AMBIGUOUS transitions allowed by the v15 guard; no reconciler)
- **Depends on:** T09
- **Cross-package prerequisites:** NONE
- **Owner:** Capability Fabric
- **Allowed paths:** `apps/litecoworkd/src/capability_reconciler.rs` (new), `apps/litecoworkd/src/capability_broker.rs`, `crates/storage-sqlite/src/invocations.rs`, `crates/storage-sqlite/src/effect_evidence.rs`
- **Contract:** SPEC §5 B-20 (idempotency UNKNOWN), §11; AC-005
- **Change intent:** Timeout, connection reset or 5xx after send marks the Invocation AMBIGUOUS. The reconciler lists the issue's comments (paged, bounded, newest first, since the dispatch time) for the exact client tag and body hash: one match -> SUCCEEDED with the comment id as Evidence; none after a bounded window -> stays AMBIGUOUS for the owner with an explicit "post again" that creates a new Invocation and a new Approval; more than one -> FAILED integrity with both ids. No automatic second POST.
- **Acceptance:**
  - [ ] Timeout-after-send never triggers a second POST (AC-005).
  - [ ] Reconcile finds the comment by tag and records its id.
  - [ ] Daemon restart with an AMBIGUOUS Invocation resumes reconciliation, not dispatch.
  - [ ] Read-back uses the read path's allowlist and bounds.
- **Command:** `cargo test -p litecoworkd --locked capability_reconcile` (filter to be added by this task)
- **Evidence:** test names; fault-injection recording.
- **Stop:** stop if reconciliation would need a write scope.

## 006-T11 - Revoke and disconnect with local fencing and explicit remote-revoke state

- **Status:** NOT STARTED
- **Depends on:** T03, T05
- **Cross-package prerequisites:** NONE
- **Owner:** Connectors, Security
- **Allowed paths:** `apps/litecoworkd/src/connection_service.rs`, `crates/storage-sqlite/src/connections.rs`, `crates/storage-sqlite/src/trust.rs`
- **Contract:** SPEC §5 device-flow authorization/revocation behavior and §10; AC-006, AC-015, AC-021
- **Change intent:** `disconnect` commits REVOKED and revokes all grants for the Connection in one transaction (version CAS), then removes keystore entries. It does not claim remote GitHub App authorization revocation: it records `OWNER_ACTION_REQUIRED` and directs the owner to GitHub account Settings → Applications → Authorized GitHub Apps. In-flight Invocations are fenced at their next state transition; a DISPATCHED request already sent is reconciled, not cancelled silently.
- **Acceptance:**
  - [ ] Reads and writes after revoke fail closed for that Connection id (AC-006).
  - [ ] Keystore delete failure leaves REVOKED committed and reports the cleanup failure; it does not reopen the Connection (AC-015).
  - [ ] Remote revoke unavailable shows OWNER_ACTION_REQUIRED, never "revoked at GitHub" (AC-015).
  - [ ] Historical Approval, Invocation, Effect and Evidence rows remain readable.
  - [ ] Revoke racing a refresh: refresh loses (stale version) and writes no new secret.
- **Command:** `cargo test -p litecoworkd --locked connection_revoke` (filter to be added by this task)
- **Evidence:** test names.
- **Stop:** stop if revoke would delete Effect history.

## 006-T12 - Operator routes, authorization and Tauri bridge commands

- **Status:** NOT STARTED (`listConnections`, `getConnection`, `disconnectConnection` target-only; `connect_or_install_capability` 503)
- **Depends on:** T05, T08, T09, T10, T11
- **Cross-package prerequisites:** 011 authenticated IPC on both rows; 013 owns the install half of `connect-or-install`
- **Owner:** Operator, Desktop UI (bridge)
- **Allowed paths:** `apps/litecoworkd/src/connection_operator.rs` (new), `apps/litecoworkd/src/operator.rs` (router mount only), `apps/litecoworkd/src/discover_operator.rs` (regression test only), `apps/litecowork-ui/src-tauri/src/connection_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/src/api/` (typed client)
- **Contract:** OpenAPI operations promoted in T01; SPEC §7.1; AC-011, AC-016
- **Change intent:** Mount begin, cancel, list, get and disconnect Connection routes, the golden read and write request routes, with the existing authenticate middleware, Workspace-owner checks, `no-store` and idempotency keys. Responses carry only the opaque Connection id, status, provider account display hint and scope labels. Tauri commands `begin_connection`, `cancel_connection`, `list_connections`, `get_connection`, `disconnect_connection`, `request_golden_read`, `request_golden_write` validate ids like `discover_bridge.rs`. Keep `connect_or_install_capability` unchanged and add a regression test that it still returns 503 and calls no LiteSPM client.
- **Acceptance:**
  - [ ] Foreign Principal, wrong Workspace and missing idempotency key are rejected on every route.
  - [ ] No response or bridge payload contains a token, verifier, code or SecretRef provider locator.
  - [ ] `connect-or-install` still returns 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` with no receipt (AC-016).
  - [ ] Served-route inventory lists exactly the new routes.
- **Command:** `cargo test -p litecoworkd --locked connection_operator` (filter to be added by this task); `cargo test -p litecowork-ui --locked connection_bridge` (filter to be added by this task); `python3 scripts/audit_served_routes.py`
- **Evidence:** regenerated `docs/schemas/served-route-inventory.json` diff.
- **Stop:** stop if a route would let an agent choose the endpoint or URL.

## 006-T13 - Discover Connections UI, connection detail and golden operation flow

- **Status:** NOT STARTED (`DiscoverPage.tsx` Connections tab shows "Connections cannot be read here")
- **Depends on:** T12
- **Cross-package prerequisites:** 011 keyboard/focus baseline; 013 keeps the Managed and Installed tabs
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/discover/DiscoverPage.tsx`, `apps/litecowork-ui/src/discover/discover-state.ts`, `apps/litecowork-ui/src/discover/connections-state.ts` (new), `apps/litecowork-ui/src/discover/ConnectionDetail.tsx` (new), `apps/litecowork-ui/src/needs-you/ApprovalDetail.tsx`, `apps/litecowork-ui/tests/connections-state.test.ts` (new), `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`
- **Contract:** contracts/ui-behavior.md; AC-011, AC-015
- **Change intent:** Replace the Connections not-available panel with a list and a "Connect GitHub" action (disabled with reason when the keystore probe or T02 decision is missing). Connection detail shows status (CONNECTING, CONNECTED, DEGRADED, REAUTH_REQUIRED, REVOKED), provider scopes in plain words, grants, recent Invocations, and Disconnect with confirmation and the remote-revoke state. The golden read and write appear as two separate actions; the write opens a preview (target issue and exact body) and routes to Needs You for approval. Pure state logic lives in `connections-state.ts`. States: loading, empty, error, denied, offline, stale (409), UNAVAILABLE (no keystore), awaiting browser, AMBIGUOUS awaiting reconcile.
- **Acceptance:**
  - [ ] Connected, granted, approved and invoked render as distinct states; catalog rows and `health=UNKNOWN` never render as connected or healthy (AC-011).
  - [ ] Remote revoke OWNER_ACTION_REQUIRED shows the GitHub settings link (AC-015).
  - [ ] All controls have accessible names, keyboard order and `aria-live` status updates.
- **Command:** `pnpm --dir apps/litecowork-ui test`; `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; UI ledger rows updated.
- **Stop:** stop if any control lacks a served route.

## 006-T14 - Crash and restart recovery for AuthAttempts, orphan secrets and in-flight Invocations

- **Status:** NOT STARTED
- **Depends on:** T06, T10, T11
- **Cross-package prerequisites:** 011 Runtime startup recovery ordering
- **Owner:** Connectors, Capability Fabric
- **Allowed paths:** `apps/litecoworkd/src/connection_service.rs`, `apps/litecoworkd/src/capability_reconciler.rs`, `apps/litecoworkd/src/runtime.rs` (startup sweep hook only), `crates/storage-sqlite/src/connections.rs`
- **Contract:** SPEC §11-§12; AC-002, AC-005, AC-013, AC-020, AC-022
- **Change intent:** Startup sweep: pending device AuthAttempts from a prior incarnation become EXPIRED and their flow is not reopened; TOKEN_CUSTODY_PENDING attempts reconcile their preallocated refs against committed Connections and remove unowned slots; CLEANUP_PENDING attempts retry exact removal; ambiguous refresh attempts become REAUTH_REQUIRED without reusing a possibly consumed refresh token; DISPATCHED or AMBIGUOUS Invocations go to the reconciler; CONNECTED Connections reload without projecting token bytes.
- **Acceptance:**
  - [ ] Kill during device authorization, token receipt, intent persistence, either keyring write, Connection commit or cleanup leaves no usable partial Connection; pending attempts expire, exact staged refs are removed before new auth, and committed Connection refs remain (AC-002, AC-013, AC-017, AC-020, AC-022).
  - [ ] Kill after POST sent reconciles on restart without a second POST (AC-005).
  - [ ] Restart logs contain ids and statuses only.
- **Command:** `cargo test -p litecoworkd --locked connection_recovery` (filter to be added by this task)
- **Evidence:** crash-injection test names.
- **Stop:** stop if recovery would need to read token bytes.

## 006-T15 - Qualification: E2E-05 after A-80 resolves Task-write row ordering

- **Status:** NOT STARTED; Task-write platform order is OPEN under register A-80.
- **Depends on:** T13, T14
- **Cross-package prerequisites:** 005 Attempt admission on the same row (and B-35 on Windows if the SPEC §18 question resolves that the write needs an Environment); 019 evidence bundle process and release rows; 011 packaged desktop
- **Owner:** Connectors, QA, repository owner (USER evidence)
- **Allowed paths:** `apps/litecoworkd/tests/` (new E2E harness files), `apps/litecowork-ui/tests/`, `scripts/v1_platform_matrix.py` (row entries only), `specs/006-golden-connection-capability-write/`
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-05; AGENTS.md §4; AC-001 to AC-016
- **Change intent:** Record the AGENTS.md §4 trace for each user operation: (1) connect (Connect GitHub -> `begin_connection` -> authenticated begin route -> owner check -> ConnectionService -> fixed GitHub device-code request -> system browser at the fixed verification URI -> bounded poll honoring interval/`slow_down` -> SecretStore write -> Connection transaction -> `connection.state.changed.v1` -> Connections list -> restart reload); (2) read (Read action -> `request_golden_read` -> accessible-repository and exact grant check -> broker -> Invocation -> detail view); (3) write (Write preview -> `request_golden_write` -> Approval -> Needs You approve -> admission transaction -> POST -> reconcile -> Evidence -> detail view); (4) local revoke and separate GitHub authorization revoke. Against a dedicated test repository, qualify Connection/read operations per available OS row and qualify the Task-scoped write in the row order established by A-80. Do not mark E2E-05 fully qualified while its write row is unavailable or unresolved. Include missing permission/repository access, no grant, stale approval, self-approval, non-allowlisted API/auth host, poll denial/slow_down/expiry, duplicate replay, timeout-after-dispatch, revoke race, crash and restart.
- **Acceptance:**
  - [ ] CODE, SYSTEM and owner USER evidence per row with exact OS build, keystore backend and GitHub API version header.
  - [ ] Token redaction verified across daemon logs, events and UI state dumps on both rows.
  - [ ] Trace table stored with the evidence.
- **Command:** `cargo test -p litecoworkd --locked e2e_05_golden_connection` (filter to be added by this task); `pnpm --dir apps/litecowork-ui test`; `python3 scripts/v1_platform_matrix.py`
- **Evidence:** evidence bundle path in CURRENT-RUN.md and IMPLEMENTATION-STATUS.md Golden Connection row.
- **Stop:** stop and keep the row unqualified if the test repository or account is shared with real user data.

## 006-T16 - Documentation and ledger sync

- **Status:** NOT STARTED
- **Depends on:** T15
- **Cross-package prerequisites:** orchestrator owns register and specs index edits
- **Owner:** Connectors, documentation owner
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `CURRENT-RUN.md`, `specs/006-golden-connection-capability-write/`
- **Contract:** AGENTS.md §4 steps 5-8
- **Change intent:** Update the "Golden Connection and one capability write" section of IMPLEMENTATION-STATUS.md per row; regenerate the route inventory with `scripts/audit_served_routes.py`; update UI ledgers for the Connections tab; close B-19/B-20 register rows; refresh 006 SPEC §4.
- **Acceptance:**
  - [ ] No document claims LiteSPM install or package lifecycle from 006 evidence.
  - [ ] Route inventory matches mounted routes.
- **Command:** `python3 scripts/audit_served_routes.py`; `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of the listed docs.
- **Stop:** stop if a doc would upgrade an evidence level without a recorded run.

## 006-T17 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T16
- **Cross-package prerequisites:** 019 release qualification gate G
- **Owner:** Repository owner; Connectors
- **Allowed paths:** `specs/006-golden-connection-capability-write/`
- **Contract:** SPECIFICATION-STANDARD lifecycle; all AC IDs in SPEC
- **Change intent:** Assemble the evidence bundle only after formal owner acceptance, then move ACCEPTED/LOCKED -> IMPLEMENTING (at first code task) -> VERIFIED under the feature lifecycle and obtain owner sign-off per row. Revision 1.1 remains IN_REVIEW until that acceptance is recorded; T02 does not advance the lifecycle.
- **Acceptance:**
  - [ ] Every AC has passing evidence on each row it claims.
  - [ ] Owner sign-off recorded with date.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** completion table below; SPEC and WIRING status VERIFIED.
- **Stop:** stop if any AC lacks evidence on its claimed row.

## Dependency order

T02 (owner; auth model and exact provider scopes) -> T01 -> T03, T04, T07. T02 -> T05.
T01 + T02 + T03 + T04 -> T05 -> T06.
T04 + T05 + T07 -> T08 -> T09 -> T10.
T03 + T05 -> T11.
T05 + T08 + T09 + T10 + T11 -> T12 -> T13.
T06 + T10 + T11 -> T14.
T13 + T14 -> T15 -> T16. T01-T16 -> T17.

Parallel lanes after T01: custody (T03), egress (T04), dispatch schema (T07); T02 can be decided at any time and gates only connection code.

## Cross-package dependencies

| This task | Other package work | Reason |
|---|---|---|
| T01 | 005 contract promotion; orchestrator migration numbering | Shared Trust/Effect prose and migration number |
| T03 | 011 keystore qualification per row | Connect must fail closed without a keystore |
| T05 | 011 system browser open and Runtime readiness | OAuth or device-flow UX |
| T07, T08 | 005 Trust wiring | Grant and Trust decision evaluation |
| T09 | 005 Attempt admission | The write is Task-scoped |
| T12 | 011 authenticated IPC; 013 owns install half of connect-or-install | Transport and boundary |
| T13 | 011 focus baseline; 013 Managed/Installed tabs | Shared Discover screen |
| T14 | 011 startup recovery ordering | Sweep placement |
| T15 | 005 Attempt admission per row (and B-35 on Windows if an Environment is required); 019 evidence process | E2E prerequisites |
| T17 | 019 gate G | Release qualification |

Boundary with 013: 013 owns LiteSPM inventory, plan, apply, receipt and rollback; if the GitHub capability later arrives as a LiteSPM package, 013 installs it and 006 still owns Connection, grant and invocation.

## Completion evidence

For each task record, in the table below and in CURRENT-RUN.md: git revision, exact command, result (pass/fail with counts), release-matrix row (OS name, build, architecture), keystore backend, GitHub API version header used, evidence level reached (CODE, SYSTEM, USER), reviewer and date.

| Task | Revision | Command | Result | Row / OS build | Keystore | Level | Reviewer | Date |
|---|---|---|---|---|---|---|---|---|
| (none yet) | | | | | | | | |
