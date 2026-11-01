# Tasks - Professional Artifacts One Format

**Feature:** 007 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 007-T01 - Lock the one professional format

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Artifacts
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-21.
- **Acceptance:** Format and engine named. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 007-T02 - Versioned artifact store

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Artifacts and Storage
- **Allowed paths:** artifact store
- **Contract:** ARTIFACTS-EVIDENCE
- **Change intent:** Immutable versions and provenance.
- **Acceptance:** AC-001, AC-002, AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p storage-sqlite --locked artifact_versions`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 007-T03 - Format edit export reopen

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Artifacts
- **Allowed paths:** format adapter
- **Contract:** SPEC LLD
- **Change intent:** Real edit and export for the chosen format.
- **Acceptance:** AC-003 and AC-005. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked artifact_one_format`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 007-T04 - Library UI honesty

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** library
- **Contract:** ui-behavior and PRESENTATION-RUNTIME
- **Change intent:** Unsupported actions stay honest; semantic fallback remains.
- **Acceptance:** AC-004. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
