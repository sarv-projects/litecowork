# Tasks - Workspace Roots and Context Ingestion

**Feature:** 003 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 003-T01 - Lock ingestion formats and watcher policy

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Resource
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-13 and B-14.
- **Acceptance:** Owner decision recorded. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 003-T02 - Qualify root admission and identity rebind

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Resource and Storage
- **Allowed paths:** resource service and sqlite root tests
- **Contract:** WORLD-RESOURCES Workspace roots
- **Change intent:** Implement create, pause, resume, revoke and startup rebind as specified.
- **Acceptance:** AC-001, AC-002, AC-003, AC-004, AC-005. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p storage-sqlite --locked workspace_root_lifecycle`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 003-T03 - Bound ingestion and retrieval receipts

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Resource and Conversation
- **Allowed paths:** context assembly
- **Contract:** CONTEXT.md and WORLD-RESOURCES indexing
- **Change intent:** Attach only explicit revisions under ACTIVE roots.
- **Acceptance:** AC-006 and AC-007. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked context_attachment_receipts`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 003-T04 - Root and Context Used UI

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** desktop roots and context used
- **Contract:** contracts/ui-behavior.md
- **Change intent:** Show unavailable, paused and revoked distinctly.
- **Acceptance:** UI states match root status. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
