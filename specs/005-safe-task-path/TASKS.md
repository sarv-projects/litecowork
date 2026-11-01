# Tasks - Safe Task Path

**Feature:** 005 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 005-T01 - Lock environment provider and edit class

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product, Environments, Trust
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-17 and B-18.
- **Acceptance:** Provider and edit class written. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 005-T02 - Explicit Task and plan acceptance

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Task Runtime
- **Allowed paths:** task service
- **Contract:** TASK-RUNTIME create and plan acceptance
- **Change intent:** Commit Task without Attempt; accept plan atomically.
- **Acceptance:** AC-001, AC-002, AC-003, AC-004. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked task_explicit_plan`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 005-T03 - Isolated single edit and reconcile

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Environments and Trust
- **Allowed paths:** environment provider and effect reconciler
- **Contract:** TRUST and ENVIRONMENTS
- **Change intent:** Attest isolation; reconcile unknown writes.
- **Acceptance:** AC-005, AC-006, AC-007. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked task_one_edit_reconcile`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 005-T04 - Promote and receipt UI

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** work task surfaces
- **Contract:** ui-behavior
- **Change intent:** Promotion is explicit; receipt shows verified versus unknown.
- **Acceptance:** UI does not imply Task from send. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
