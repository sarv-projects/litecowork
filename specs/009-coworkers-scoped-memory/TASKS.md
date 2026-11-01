# Tasks - Coworkers and Scoped Memory

**Feature:** 009 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 009-T01 - Lock minimal memory and quick-create binding

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Coworkers
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-24 and B-25.
- **Acceptance:** Record and binding rule written. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 009-T02 - Coworker identity and scoped documents

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Coworkers and Storage
- **Allowed paths:** coworker service
- **Contract:** COWORKERS-TARGET and ADR-0024
- **Change intent:** Create, scope and purge without grants.
- **Acceptance:** AC-002, AC-003, AC-004, AC-005. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p storage-sqlite --locked coworker_scope`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 009-T03 - Scoped retrieval on restart

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Conversation and Memory
- **Allowed paths:** retrieval
- **Contract:** CONTEXT.md
- **Change intent:** Filter by scope across restart.
- **Acceptance:** AC-003 and AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked coworker_retrieval_fence`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 009-T04 - Optional Coworker UI

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** coworkers and new chat
- **Contract:** ui-behavior
- **Change intent:** New Chat stays available without a Coworker.
- **Acceptance:** AC-001. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
