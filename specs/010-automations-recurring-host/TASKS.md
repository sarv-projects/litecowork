# Tasks - Automations and Recurring Host

**Feature:** 010 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 010-T01 - Lock trigger type and misfire policy

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Automation
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-26 and B-27.
- **Acceptance:** Trigger and notification rules written. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 010-T02 - Durable occurrence idempotency

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Automation and Storage
- **Allowed paths:** trigger host
- **Contract:** AUTOMATION.md
- **Change intent:** Exactly-one occurrence and Task link.
- **Acceptance:** AC-001, AC-002, AC-004. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p storage-sqlite --locked automation_occurrence`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 010-T03 - Sleep restart and notification status

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Runtime and Automation
- **Allowed paths:** lifecycle and notifications
- **Contract:** RUNTIME-LIFECYCLE and AUTOMATION.md
- **Change intent:** Misfire and distinct notification status.
- **Acceptance:** AC-003, AC-005, AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked automation_recovery`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 010-T04 - Automation history UI

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** automations
- **Contract:** ui-behavior
- **Change intent:** History shows occurrence, Task and notification separately.
- **Acceptance:** UI matches settlement. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
