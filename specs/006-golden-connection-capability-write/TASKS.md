# Tasks - Golden Connection and One Capability Write

**Feature:** 006 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 006-T01 - Lock golden provider and write operation

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Security
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-19 and B-20.
- **Acceptance:** Provider, auth recipe and write operation named. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 006-T02 - Connection custody and revoke

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Connectors
- **Allowed paths:** connection service and secret store
- **Contract:** CONNECTORS-WORKFLOWS §4–§6
- **Change intent:** OAuth and revoke without token leakage.
- **Acceptance:** AC-001, AC-002, AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked golden_connection_custody`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 006-T03 - Granted read and approved write

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Capability Fabric and Trust
- **Allowed paths:** broker and reconciler
- **Contract:** CAPABILITY-FABRIC and NETWORK-SECURITY
- **Change intent:** Approval, idempotency and UNKNOWN reconciliation.
- **Acceptance:** AC-003, AC-004, AC-005, AC-007. Criteria discussed include AC-001 AC-002.
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
