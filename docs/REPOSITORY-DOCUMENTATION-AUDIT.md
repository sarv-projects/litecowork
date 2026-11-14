
# Repository documentation and source audit

This document defines the present documentation/source alignment and the work required to keep it coherent. It is not a historical reset report.

## Repository structure

The active repository has these first-party working areas:

- apps/litecoworkd: Rust local Runtime/Operator daemon.
- apps/litecowork-ui: Tauri v2 desktop shell, React UI and Rust Tauri bridge.
- crates/: domain, persistence, IPC, environment and shared Rust libraries.
- docs/: product, architecture, owner contracts, ADRs, schemas, UI contracts, coverage, plans and evidence.
- specs/: 20 feature SDD packages (001–020). Package lifecycle and owner acceptance are separate; see `specs/README.md` and `docs/V1-FEATURE-SDD-REGISTER.md`.
- .specify/: package templates and structural validators.
- scripts/: source/route/documentation audits and validators.
- repo_take/: pinned reference repositories used only for research.

Historical snapshots under docs/archive and local ignored archives are not substitutes for this active source. They must not be deleted or described as the only implementation.

## Audit rules

1. Normative behavior has one owner. Supporting docs link to that owner.
2. Source implementation is recorded separately from target design.
3. Route declaration, service/store integration, provider integration, UI wiring and qualification are separate evidence points.
4. A UI control is not complete without a typed command, owner operation, persistence/effect semantics, error path and recovery.
5. Every screen contract has corresponding loading, empty, stale/offline, denied, unavailable, pending, ambiguous, success and error behavior where relevant.
6. A mock, demo, sample diff, local component state or preview iframe never masquerades as real work.
7. All generated manifests declare their scope, exclusions and source revision.
8. Dated addenda are not used to contradict old prose. Reconcile the owning document and remove superseded status assertions from the active contract.

## Reconciliation completed in the active owner set

The current owner/index set now distinguishes:

- V1 product target from actual implementation status.
- Runtime lifecycle READY from Task execution readiness.
- Route registration from integration and qualification.
- OpenCode dispatch from discovery/probing of other native agents.
- Coworker configuration persistence from Coworker chat/memory/responsibility execution.
- Task record creation from plan/Attempt admission.
- Local TriggerHost occurrence materialization from Task execution.
- Resource preview from live filesystem editing.
- Browser iframe preview from browser automation.
- Simulated terminal/sample diff from real process/evidence.
- LiteSPM catalogue read paths from package lifecycle ownership.
- Rich presentation code from full Intelligent UI and isolated Artifact Runtime qualification.

The detailed current facts are in IMPLEMENTATION-STATUS.md, the screen contract in V1-UI-SCREEN-CONTRACT.md and unresolved choices in CONTRADICTION-AND-ALIGNMENT-REGISTER.md.

## Validation paths and limitations

- scripts/audit_served_routes.py compares the composed Operator route registrations with the OpenAPI contract and writes docs/schemas/served-route-inventory.json.
- .specify/scripts/validate_feature_package.py checks the current 20 feature packages for required structure. A passing structural check is not owner acceptance or implementation evidence.
- scripts/validate_design_owner_consistency.py checks adjudicated cross-owner rules. Its output must remain under docs/audit/ rather than a removed implementation/ directory.
- scripts/audit_v1_markdown.py inventories first-party active Markdown and relative links; it must exclude caches, generated run artifacts, archives, the ignored staging_tar checkout and repo_take clones.
- scripts/validate_architecture.py validates machine contracts and requires the declared Python dependencies.
- cargo test --workspace --locked exercises Rust unit/integration test targets but is not a substitute for packaged UI/provider E2E.
- Frontend type/build/test commands are defined by apps/litecowork-ui/package.json.

The latest recorded full serial frontend run (2026-11-13) reports 626 passed, 5 failed and 5 skipped of 636 tests; three failures followed CDP startup timeouts and two Needs You tests failed because browser state was unavailable. Focused Conversation/responsive and production-build results are recorded separately in IMPLEMENTATION-STATUS.md. Packaged Tauri/provider USER E2E remains unqualified. Rust workspace and validator results are historical dated evidence and must be checked against the exact current revision before reuse. This audit is a documentation/source-structure review, not a replacement for those qualification gates.

## Corpus inventory

COMPLETE-ACTIVE-CORPUS-INVENTORY.md is the human-readable active file manifest. Machine-generated line/hash/link reports belong in docs/audit and are excluded from the normative document inventory. Generated schemas and route inventories belong in docs/schemas. Preserve archives and reference repositories but exclude them from the active first-party documentation count.
