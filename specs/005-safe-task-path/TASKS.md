# Tasks - Safe Task Path

**Feature:** 005 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.2

## 005-T01 - Lock environment provider and edit class

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product, Environments, Trust
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-17 and B-18.
- **Acceptance:** Product, Environments and Trust select (1) the GA OS/build and exact first provider/version with evidence for filesystem, process-tree/quiescence, network and resource-limit enforcement (B-17), and (2) one exact edit class with pinned base digest, allowed path, byte bound, atomic-write and rollback/reconciliation/verifier semantics (B-18). Provider probes or Job Object presence alone do not satisfy B-17.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 005-T02 - Explicit Task and plan acceptance

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Task Runtime
- **Allowed paths:** task service
- **Contract:** TASK-RUNTIME create and plan acceptance
- **Change intent:** Commit Task without Attempt; accept plan atomically.
- **Acceptance:** Task creation from the current Home composer remains an envelope only (AC-001–AC-003); plan submission is exposed only after owner-authorized producer-scope validation and immutable TaskSpec-head checks; accepted PlanRevision and Steps commit atomically before any Attempt (AC-004). Rendered UI must show accepted vs absent/stale plan without implying execution.
- **Command:** `cargo test -p litecoworkd --locked task_explicit_plan`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 005-T03 - Isolated single edit and reconcile

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Environments and Trust
- **Allowed paths:** environment provider and effect reconciler
- **Contract:** TRUST and ENVIRONMENTS
- **Change intent:** Attest isolation; reconcile unknown writes.
- **Acceptance:** Exact provider attestation and scoped lease/grant/idempotency receipt commit before the external file operation; the operation runs outside SQLite; ambiguous writes reconcile before retry; quiescence is proven before releasing writer authority; independent verification evidence is persisted; restart/cancel cannot claim success without that evidence (AC-005–AC-007).
- **Command:** `cargo test -p litecoworkd --locked task_one_edit_reconcile`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 005-T04 - Promote and receipt UI

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** work task surfaces
- **Contract:** ui-behavior
- **Change intent:** Promotion is explicit; receipt shows verified versus unknown.
- **Acceptance:** Home “New task” remains explicit and distinct from Conversation send; any future Conversation promotion is a separate reviewed action. Work shows Task envelope, accepted plan, Attempt/Effect/Environment and verification as distinct owner-backed states. The first file change is visible only after its committed receipt and independent verification; UNKNOWN/AMBIGUOUS stays unresolved and is never rendered as success.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
