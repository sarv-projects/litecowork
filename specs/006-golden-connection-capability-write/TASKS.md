# Tasks - Golden Connection and One Capability Write

**Feature:** 006 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.2

## 006-T01 - Lock golden provider and write operation

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Security
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-19 and B-20.
- **Acceptance:** Product/Connectors/Security select one provider and exact public OAuth/PKCE/loopback recipe with minimum scopes, refresh/revoke, host allowlist and supported OS (B-19); Capability Fabric/Trust select one provider operation ID, operation class, HTTP schema, exact grant/approval binding, idempotency type and reconciliation observation (B-20). No implementation is authorized by this selection alone.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 006-T02 - Connection custody and revoke

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Connectors
- **Allowed paths:** connection service and secret store
- **Contract:** CONNECTORS-WORKFLOWS §4–§6
- **Change intent:** OAuth and revoke without token leakage.
- **Acceptance:** Cancelled OAuth persists no usable Connection; loopback/PKCE state is single-use and callback carries no token; provider secrets are written only to the OS SecretStore and never appear in logs/events/prompts; Connection becomes CONNECTED only after token custody and durable Connection commit; refresh is serialized; `invalid_grant` becomes REAUTH_REQUIRED; revoke fences future refresh/invocation; lost responses replay the original idempotent receipt.
- **Command:** `cargo test -p litecoworkd --locked golden_connection_custody`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 006-T03 - Granted read and approved write

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Capability Fabric and Trust
- **Allowed paths:** broker and reconciler
- **Contract:** CAPABILITY-FABRIC and NETWORK-SECURITY
- **Change intent:** Approval, idempotency and UNKNOWN reconciliation.
- **Acceptance:** One provider-backed READ requires an explicit exact-scope CapabilityGrant. One selected WRITE/SEND/DELETE-class operation requires a fresh Approval bound to the exact immutable plan/action digest; one atomic admission transaction rechecks owner, lease, grant, activation, Trust decision and exact ApprovalUse before the provider call. Provider dispatch occurs outside SQLite. Timeout/UNKNOWN becomes AMBIGUOUS and must reconcile before retry; revoke blocks later invocations; agent/package cannot self-approve or grant itself authority.
- **Command:** `cargo test -p litecoworkd --locked golden_capability_write`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 006-T04 - Connect and approval UI

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** discover and needs-you
- **Contract:** ui-behavior
- **Change intent:** Separate connected from granted from invoked.
- **Acceptance:** UI states match the six-way distinction. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
