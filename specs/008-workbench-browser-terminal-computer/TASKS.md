# Tasks - Workbench Browser Terminal and Computer

**Feature:** 008 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 008-T01 - Lock browser mode and shell profile

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Security
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-22 and B-23.
- **Acceptance:** Mode and profile written. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 008-T02 - Browser epoch takeover return

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Browser provider and Trust
- **Allowed paths:** browser gateway
- **Contract:** CHROME-BROWSER-LLD
- **Change intent:** Consent, epoch, takeover, stale reject.
- **Acceptance:** AC-001, AC-002, AC-003, AC-004 and AC-007 for browser. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked browser_epoch`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 008-T03 - Terminal bound to Environment

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Environments
- **Allowed paths:** pty host
- **Contract:** V1-INTEGRATED-TERMINAL-LLD
- **Change intent:** Discovery without startup execution; one process tree per tab.
- **Acceptance:** AC-005 and AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked terminal_environment_bind`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 008-T04 - Workbench views

- **Status:** NOT STARTED
- **Depends on:** T02-T03
- **Owner:** Desktop UI
- **Allowed paths:** right workbench
- **Contract:** DESKTOP-SHELL-LLD and ui-behavior
- **Change intent:** Show real availability only.
- **Acceptance:** No fake browser or terminal state. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
