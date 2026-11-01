# Tasks - Desktop Shell IPC Settings and Runtime Health

**Feature:** 011 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 011-T01 - Lock GA OS and first settings groups

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Runtime
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-28 and B-29.
- **Acceptance:** OS and enabled groups written. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 011-T02 - Authenticated IPC gate

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Operator IPC
- **Allowed paths:** ipc adapters and tests
- **Contract:** LOCAL-OPERATOR-IPC
- **Change intent:** Reject unauthenticated peers on the qualified OS.
- **Acceptance:** AC-001 and AC-002. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p operator-ipc --locked ipc_peer_auth`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 011-T03 - Runtime health projection

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Runtime
- **Allowed paths:** runtime lifecycle
- **Contract:** RUNTIME-LIFECYCLE
- **Change intent:** Incarnation, ready and drain are distinct; window close is not stop.
- **Acceptance:** AC-005 and AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked runtime_health_projection`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 011-T04 - Settings gate and semantic fallback

- **Status:** NOT STARTED
- **Depends on:** T02-T03
- **Owner:** Desktop UI
- **Allowed paths:** shell and settings
- **Contract:** V1-SETTINGS-CONTRACT-LEDGER and SEMANTIC-SERVED-UI-MAP
- **Change intent:** Disable unserved controls; keep semantic text.
- **Acceptance:** AC-003, AC-004, AC-007, AC-008. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
