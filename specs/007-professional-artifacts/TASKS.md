# Tasks - Professional Artifacts One Format

**Feature:** 007 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.2

## 007-T01 - Lock the one professional format

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Artifacts
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-21.
- **Acceptance:** Product/Artifacts/Desktop UI record one exact format ID, editor/parser/export engine and version, OS/build requirements, allowed operations, bounded input/output sizes, fidelity and reopen/round-trip checks, deterministic validator, output digest and explicit unsupported behavior. A Markdown/CSV preview or text/plain textarea does not satisfy this acceptance.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 007-T02 - Versioned artifact store

- **Status:** NOT STARTED
- **Depends on:** T01 and B-30 accepted
- **Owner:** Artifacts and Storage
- **Allowed paths:** artifact store
- **Contract:** ARTIFACTS-EVIDENCE
- **Change intent:** Immutable versions and provenance.
- **Acceptance:** A new Artifact can be created from the selected authorized source (user-authored/imported/Task-generated path as accepted) through a served owner command. One SQLite transaction commits the Artifact, ARTIFACT Resource, ResourceRevision, immutable version 1, provenance/input dependency edges, snapshots, canonical creation events and owner-scoped idempotency receipt. Identical replay returns that receipt; changed payload conflicts. Existing direct-SQL test fixtures are not production create support.
- **Command:** `cargo test -p storage-sqlite --locked artifact_versions`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 007-T03 - Format edit export reopen

- **Status:** NOT STARTED
- **Depends on:** T01, T02 and B-31 accepted
- **Owner:** Artifacts
- **Allowed paths:** format adapter
- **Contract:** SPEC LLD
- **Change intent:** Real edit and export for the chosen format.
- **Acceptance:** The selected professional format can be opened, edited with the selected engine, validated, saved as a new immutable version, exported through the accepted export flow, closed and reopened after application/runtime restart. Independent format validation and a content digest confirm the round trip. Unsupported formats remain explicitly read-only/unsupported.
- **Command:** `cargo test -p litecoworkd --locked artifact_one_format`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 007-T04 - Library UI honesty

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** library
- **Contract:** ui-behavior and PRESENTATION-RUNTIME
- **Change intent:** Unsupported actions stay honest; semantic fallback remains.
- **Acceptance:** Library has an explicit supported creation/import/generation entry point only after B-30 is served; version history/compare/provenance identify exact Artifact/ResourceRevision; native Save As is labelled as a local file save and is not confused with an export receipt; format operations are individually enabled/disabled based on the accepted engine; semantic source remains readable if rendering fails.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
