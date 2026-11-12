# Tasks - Golden Connection and One Capability Write

**Feature:** 006 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.0 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)

Boundary: this package owns one GitHub account Connection, one scoped read, one approved write (create an issue comment), UNKNOWN reconciliation and revoke. It does not own the LiteSPM package lifecycle; that is package 013. Nothing here installs, adopts or mutates a LiteSPM package.

## Current baseline

Historical source crosswalk checked 2026-11-09 on branch `docs/audit-competitor-superset`; current route and storage status re-checked 2026-11-12 against `docs/IMPLEMENTATION-STATUS.md` and the generated served-route inventory. The source boundary remains unchanged: no Connection service/storage writer, OAuth journey or external provider dispatch is served; B-19 remains the recipe blocker.

- **Discover and connect:** `apps/litecoworkd/src/discover_operator.rs` `connect_or_install_capability` always returns 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` with no receipt; `test_capability_connection` returns `health=UNKNOWN`. `apps/litecowork-ui/src/discover/DiscoverPage.tsx` renders the Connections tab as "Connections cannot be read here"; `connectActionOutcome` in `discover-state.ts` disables every row action.
- **Connection storage:** `connections` table exists in `docs/schemas/sqlite-v1.sql` with `status CHECK (... 'DISCONNECTED')` (no `REVOKED`) and `secret_refs_json`; no Rust store reads or writes it. OpenAPI `listConnections`, `getConnection`, `disconnectConnection` are target-only.
- **Secrets:** `SecretStore` (`crates/storage-core/src/trust.rs`) is write-only (`write` only, no read, no delete). `OsSecretStore` (`crates/storage-sqlite/src/trust.rs`) uses `keyring` 3.6 with `windows-native`, `linux-native-sync-persistent` and `apple-native`; its private `SecretCredentialBackend` has `remove_secret`. `SecretLeaseStore::issue_secret_lease` is Task/Attempt-scoped.
- **Dispatch:** `SqliteCapabilityDispatchStore::admit_capability_dispatch` (`crates/storage-sqlite/src/capability_dispatch.rs`) implements the combined Trust + ApprovalUse + Invocation + Effect transaction but fails closed because `docs/schemas/sqlite-v15.sql` triggers `capability_invocation_creation_admission_closed` and `capability_invocation_status_transition_guard` reject Invocation INSERT and any `DISPATCHED` transition. No daemon caller. `apps/litecoworkd/src/capability_invocations_operator.rs` is read-only (`get_invocation`, `list_task_invocations`). `crates/domain-invocations` has `InvocationStatus`, `CapabilityInvocationRecord::transition` and `reconcile_ambiguous_terminal`; `capability_bridge.rs` has `classify_agent_capability_route`.
- **Trust and approval:** `apps/litecoworkd/src/trust.rs` is compiled `#[allow(dead_code)]` in `main.rs`. Approval list/get/resolve with exact `confirmation_digest` is served (`approval_operator.rs`) and used by `apps/litecowork-ui/src/needs-you/ApprovalDetail.tsx`. `trust_operator.rs` `create_read_grant` issues only `workspace.resource.read` grants.
- **HTTP:** `apps/litecoworkd/Cargo.toml` already has `reqwest` 0.12 (`rustls-tls`, no default features); no brokered egress policy exists.
- **Provider finding:** GitHub documentation (checked 2026-11-09) requires `client_secret` on the web-flow code exchange and on token revocation; see SPEC §5 source check. This blocks the accepted loopback-PKCE recipe under B-19's own stop clause until the owner picks an option (T02).

Gaps to V1: recipe decision, Connection aggregate and service, keystore custody with a broker-only resolver, egress policy, refresh, Invocation dispatch schema, golden read and write, reconciliation, revoke, routes and bridge, UI, recovery, qualification on both rows.

## 006-T01 - Promote accepted 006 contracts into owner docs and machine schemas

- **Status:** NOT STARTED
- **Depends on:** NONE
- **Cross-package prerequisites:** 005 contract promotion (shared Trust/Effect wording); orchestrator assigns the migration number because several packages promote migrations in parallel
- **Owner:** Connectors, Capability Fabric, schema owner
- **Allowed paths:** `docs/CONNECTORS-WORKFLOWS.md`, `docs/CAPABILITY-INVOCATIONS.md`, `docs/CAPABILITY-FABRIC.md`, `docs/NETWORK-SECURITY.md`, `docs/TRUST.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `docs/schemas/sqlite-v40.sql` (new, allocated in docs/LITECOWORK-BUILD-TASKS.md; v29 is already taken by Feature 001), `specs/006-golden-connection-capability-write/`
- **Contract:** SPEC §5 B-19/B-20, §7.2, §7a, §13; AC-006, AC-013, AC-014, AC-015
- **Change intent:** Add `REVOKED` to the OpenAPI Connection status enum and the `connections` table CHECK (SQLite needs a table rebuild migration that preserves rows). Add Connection begin, callback-complete (internal), cancel and list/get/disconnect bodies; the golden read operation (proposed: list issue comments, `GET /repos/{owner}/{repo}/issues/{number}/comments`, recorded for owner confirmation) and the golden write operationId for "create issue comment"; an `auth_attempts` table (state, expiry, PKCE/state digests only, never the verifier in clear after use); `remote_revocation` status on Connection (`CONFIRMED | OWNER_ACTION_REQUIRED | NOT_ATTEMPTED`). Reuse `connection.state.changed.v1`; add events only if missing. The recipe-specific fields wait for T02.
- **Acceptance:**
  - [ ] Owner prose and OpenAPI/JSON Schema/migration change in the same commit.
  - [ ] Migration rebuild keeps existing `connections` rows and foreign keys from `channel_bindings` and later tables; a test opens a v28-era database and upgrades.
  - [ ] No secret material column is added; only SecretRef ids.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`; `cargo test -p storage-sqlite --locked migration`
- **Evidence:** commit SHA in the completion table.
- **Stop:** stop if `REVOKED` conflicts with another owner's Connection state machine; resolve at CONNECTORS-WORKFLOWS first.

## 006-T02 - Owner decision on the GitHub OAuth recipe (client-secret finding)

- **Status:** BLOCKED (GitHub docs require `client_secret` for web-flow exchange and revocation; B-19 stop clause applies; owner: repository owner with Connectors and Security)
- **DEFERRED/BLOCKED note:** Why: GitHub web-flow exchange and revocation require a client_secret, which a desktop app cannot hold; the B-19 stop clause applies and the B-19 follow-up is open. Unblocks when: owner decision on the OAuth recipe (device flow, PKCE-only app, or other). Test later at: decision only; then a real GitHub account on any native machine.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Repository owner; Connectors; Security
- **Allowed paths:** `specs/006-golden-connection-capability-write/` (SPEC §5 and revision bump), `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md` (orchestrator)
- **Contract:** SPEC §5 B-19 and the 2026-11-09 source check; SPECIFICATION-STANDARD §7
- **Change intent:** Present options (a) device flow with public client id, (b) embedded non-confidential client secret with the risk recorded, (c) another provider. Re-verify the GitHub pages at decision time and pin the retrieval date. Record the decision as SPEC revision 1.1 with renewed approval.
- **Acceptance:**
  - [ ] Decision recorded with date, option, and the exact GitHub documentation revision checked.
  - [ ] AC-002, AC-012 wording adjusted in the new revision if the device flow removes the loopback listener.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** SPEC revision 1.1 and register row.
- **Stop:** no ConnectionService OAuth code (T05) before this decision.

## 006-T03 - OS-keystore secret custody with a broker-only resolver and delete (Rust)

- **Status:** PARTIAL (`OsSecretStore::write` and `keyring` backends exist; no resolve or delete on the public trait)
- **Depends on:** T01
- **Cross-package prerequisites:** 011 per-row keystore qualification scripts (`scripts/qualify-os-credential-store.sh` exists without an owner)
- **Owner:** Security, Storage
- **Allowed paths:** `crates/storage-core/src/trust.rs`, `crates/storage-sqlite/src/trust.rs`, `crates/storage-sqlite/Cargo.toml`, `apps/litecoworkd/src/connection_secret_broker.rs` (new)
- **Contract:** SPEC §7.2 secret custody paragraph; NETWORK-SECURITY; AC-001, AC-013
- **Change intent:** Add `SecretStore::remove(&self, secret_ref: &SecretRef)` (idempotent) and a separate crate-private `SecretResolver` trait with `with_secret<R>(&self, secret_ref: &SecretRef, f: impl FnOnce(&[u8]) -> R) -> Result<R, StoreError>` so bytes live only inside the closure and are zeroized after. Only `ConnectionSecretBroker` in `apps/litecoworkd` holds a resolver; it is not exported to Operator handlers or Tauri. Add `SecretKind::OAuthRefreshToken` only if `Token` cannot distinguish access and refresh tokens (decide in T01). Add a keystore availability probe used by the UI to show Connect as UNAVAILABLE.
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
- **Change intent:** One `BrokeredHttpClient` built on `reqwest` with redirects disabled at the client and revalidated manually, TLS verification always on with rustls, HTTPS only, a fixed allowlist (`api.github.com` for Invocations; the GitHub OAuth token endpoint for ConnectionService only), DNS resolution checked against loopback, private, link-local, multicast and cloud-metadata ranges before connect (pin the resolved address for the request), request and response byte limits, total timeout, and header redaction for `Authorization` in every log path.
- **Acceptance:**
  - [ ] Requests to any non-allowlisted host, `http://`, a non-443 port, an IP literal or a DNS name resolving to a private or metadata address are refused before connect (AC-008, AC-014).
  - [ ] A 3xx to a non-allowlisted host is refused; a 3xx within the allowlist is revalidated (AC-014).
  - [ ] Oversized response is cut at the limit and reported as a bounded failure, not success.
  - [ ] No log line contains an `Authorization` value.
- **Command:** `cargo test -p litecoworkd --locked brokered_http` (filter to be added by this task)
- **Evidence:** test names; egress matrix table.
- **Stop:** stop if any path needs TLS verification disabled.

## 006-T05 - ConnectionService: AuthAttempt, OAuth flow per T02, Connection commit after keystore write

- **Status:** BLOCKED (needs T02 recipe decision; no ConnectionService exists)
- **DEFERRED/BLOCKED note:** Why: depends on the T02 recipe decision; no ConnectionService exists. Unblocks when: T02 decided. Test later at: a real GitHub account on native Linux then the owner's native Windows x64 laptop (release matrix row 1); WSL2 and containers never count.
- **Depends on:** T01, T02, T03, T04
- **Cross-package prerequisites:** 011 system-browser open via Tauri and Runtime readiness
- **Owner:** Connectors
- **Allowed paths:** `apps/litecoworkd/src/connection_service.rs` (new), `crates/storage-core/src/connection.rs` (new), `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/connections.rs` (new), `crates/storage-sqlite/src/lib.rs`
- **Contract:** SPEC §6 actor sequence 1-2, §7.2 `ConnectionService`, §7a; CONNECTORS-WORKFLOWS; AC-001, AC-002, AC-012, AC-013
- **Change intent:** Implement `begin_connection`, `finish_connection` and `cancel` as `ConnectionService` in Rust in the daemon (not in TypeScript or the Tauri shell, so the verifier and tokens never enter the WebView process). For the loopback-PKCE option: bind `127.0.0.1` on an OS-chosen port, S256 challenge, single-use state, finite deadline, first terminal callback wins, callback response page carries no token. For the device-flow option: poll with the provider interval and `slow_down` handling. Tokens go to the keystore first; then one transaction commits Connection CONNECTED + AuthAttempt SUCCEEDED + event + receipt; if that commit fails the new secrets are removed.
- **Acceptance:**
  - [ ] Cancel or `access_denied` records AuthAttempt CANCELLED and no usable Connection (AC-002).
  - [ ] Replayed or late callback, wrong state, or callback after deadline is refused (AC-012).
  - [ ] Keystore write failure leaves no Connection; commit failure after keystore write removes the secrets (AC-013).
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
- **Contract:** SPEC §7a refresh rules; AC-009
- **Change intent:** If the recipe yields expiring tokens, refresh under a per-Connection async mutex; `invalid_grant` or `bad_refresh_token` -> REAUTH_REQUIRED; other failures -> DEGRADED; refresh writes the new pair to the keystore, then commits the Connection version, then removes the old secret refs. Refresh never starts a consequential retry.
- **Acceptance:**
  - [ ] Ten concurrent Invocations needing refresh cause one provider refresh call (AC-009).
  - [ ] `bad_refresh_token` maps to REAUTH_REQUIRED; network error maps to DEGRADED; neither deletes Effect history.
  - [ ] Crash between keystore write and commit leaves the old pair valid and the new refs orphaned for T14 cleanup.
- **Command:** `cargo test -p litecoworkd --locked connection_refresh` (filter to be added by this task)
- **Evidence:** test names.
- **Stop:** stop if refresh would need to run inside a SQLite transaction.

## 006-T07 - Open Invocation creation and DISPATCHED through the combined admission transaction only

- **Status:** PARTIAL (`admit_capability_dispatch` implemented and tested; `sqlite-v15.sql` triggers keep it closed)
- **Depends on:** T01
- **Cross-package prerequisites:** 005 Trust wiring (removal of `trust` dead code) and Attempt admission, because Invocation dispatch rechecks lease and Trust decision
- **Owner:** Storage, Capability Fabric
- **Allowed paths:** `docs/schemas/sqlite-v40.sql` (same migration as T01), `crates/storage-sqlite/src/capability_dispatch.rs`, `crates/storage-sqlite/src/invocations.rs`, `crates/storage-sqlite/src/invocation_dispatch_migration_tests.rs`, `crates/storage-core/src/capability_invocation.rs`
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
- **Contract:** SPEC §5 source check (revocation needs client secret), §10; AC-006, AC-015
- **Change intent:** `disconnect` commits REVOKED and revokes all grants for the Connection in one transaction (version CAS), then removes keystore entries, then attempts remote revocation only if the T02 option allows it; otherwise records `OWNER_ACTION_REQUIRED` and returns the GitHub settings URL. In-flight Invocations are fenced at their next state transition; a DISPATCHED request already sent is reconciled, not cancelled silently.
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
- **Contract:** SPEC §11-§12; AC-002, AC-005, AC-013
- **Change intent:** Startup sweep: pending AuthAttempts from a prior incarnation become EXPIRED and their listener is not reopened; keystore entries with no committed Connection reference are removed; DISPATCHED or AMBIGUOUS Invocations go to the reconciler; CONNECTED Connections are reloaded without reading token bytes.
- **Acceptance:**
  - [ ] Kill during callback handling leaves no usable Connection and no orphan secret after restart (AC-002, AC-013).
  - [ ] Kill after POST sent reconciles on restart without a second POST (AC-005).
  - [ ] Restart logs contain ids and statuses only.
- **Command:** `cargo test -p litecoworkd --locked connection_recovery` (filter to be added by this task)
- **Evidence:** crash-injection test names.
- **Stop:** stop if recovery would need to read token bytes.

## 006-T15 - Qualification: E2E-05 on the Windows row, then the Linux row, with cross-layer traces

- **Status:** NOT STARTED
- **Depends on:** T13, T14
- **Cross-package prerequisites:** 005 Attempt admission on the same row (and B-35 on Windows if the SPEC §18 question resolves that the write needs an Environment); 019 evidence bundle process and release rows; 011 packaged desktop
- **Owner:** Connectors, QA, repository owner (USER evidence)
- **Allowed paths:** `apps/litecoworkd/tests/` (new E2E harness files), `apps/litecowork-ui/tests/`, `scripts/v1_platform_matrix.py` (row entries only), `specs/006-golden-connection-capability-write/`
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-05; AGENTS.md §4; AC-001 to AC-016
- **Change intent:** Record the AGENTS.md §4 trace for each user operation: (1) connect (Connect GitHub -> `begin_connection` -> begin route -> owner check -> ConnectionService -> browser -> callback or device poll -> keystore write -> Connection txn -> `connection.state.changed.v1` -> Connections list -> restart reload); (2) read (Read action -> `request_golden_read` -> grant check -> broker -> Invocation -> detail view); (3) write (Write preview -> `request_golden_write` -> Approval -> Needs You approve -> admission txn -> POST -> reconcile -> Evidence -> detail view); (4) revoke. Run packaged E2E on Windows x64 first and Linux x64 second against a dedicated test repository, including denial (no grant, stale approval, self-approval, non-allowlisted host), duplicate replay, timeout-after-dispatch, revoke race, crash and restart.
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
- **Change intent:** Assemble the evidence bundle, move the package ACCEPTED/LOCKED -> IMPLEMENTING (at first code task) -> VERIFIED, and get owner sign-off per row.
- **Acceptance:**
  - [ ] Every AC has passing evidence on each row it claims.
  - [ ] Owner sign-off recorded with date.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** completion table below; SPEC and WIRING status VERIFIED.
- **Stop:** stop if any AC lacks evidence on its claimed row.

## Dependency order

T01 -> T03, T04, T07. T02 (owner) -> T05.
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
