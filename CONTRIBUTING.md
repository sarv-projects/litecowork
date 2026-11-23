# Contributing to LiteCowork

## Repository layout

- Rust workspace and local daemon: apps/litecoworkd
- Tauri v2 / React desktop application: apps/litecowork-ui
- Domain, storage, runtime and IPC crates: crates/
- Normative product and domain contracts: docs/
- Feature SDD packages: specs/
- Specification templates and validators: .specify/

The desktop Operator is not the Runtime. The daemon owns durable operations and lifecycle; the native agent owns its own reasoning and private state; LiteSPM owns package lifecycle. A route, package listing, probe or UI affordance is not proof of a qualified product flow.

## Building and running

The Rust workspace is rooted at Cargo.toml. The desktop package uses pnpm and is under apps/litecowork-ui. Use the package scripts and the Rust workspace configuration in the repository; do not infer the supported build matrix from a developer machine alone.

Before claiming a feature works, trace its screen and control to the typed Tauri command, the authenticated Operator route, the service owner, the storage or effect transaction, the provider integration, the stream or projection and the recovery behavior. Record CODE, SERVED, INTEGRATED and QUALIFIED separately.

## Current delivery truth

Conversation CRUD and an OpenCode-only code-level dispatch path exist. Send eligibility currently checks dispatch conditions (draft, Workspace, Operator readiness and binding eligibility); it does not prove a fresh provider descriptor, authentication, session readiness, recovery readiness or a qualified release route (Feature 001 T09/A-24). Stop/retry and provider journeys remain unqualified. The local 30-second Automation TriggerHost settles supported schedules and materializes READY Tasks but does not execute them. Notification outcomes can create PENDING delivery records, but no outbound sender advances them to delivered states. Task planning/Attempt admission, a qualified mediated Effect path, LiteSPM lifecycle integration, a real process-backed terminal, a controlled browser, Computer use and the full Coworker/memory experience are not release-qualified. The precise evidence and the ordered blockers are maintained in [docs/IMPLEMENTATION-STATUS.md](docs/IMPLEMENTATION-STATUS.md); the capability-level coverage verdict is in [docs/V1-PRODUCT-FEATURE-COVERAGE.md](docs/V1-PRODUCT-FEATURE-COVERAGE.md).

## Architecture first

Read [`ARCHITECTURE.md`](ARCHITECTURE.md) and the focused contracts listed in
[`docs/COVERAGE-MATRIX.md`](docs/COVERAGE-MATRIX.md). Local archives are historical
copies, not implementation guidance. If a change alters an invariant, entity meaning,
ownership boundary, or protocol, update the authority and the relevant contract in the
same change; record a durable non-obvious tradeoff in `docs/adr/`.

Do not introduce implementation while required architecture decisions remain marked
open for the slice being built. Keep first implementation slices narrow and prove one
vertical workflow before adding broad providers or additional runtime roles.

## Documentation workflow

For every material feature or cross-domain change, follow
[docs/SPECIFICATION-STANDARD.md](docs/SPECIFICATION-STANDARD.md) and its package
templates under .specify/. Create or update a complete package under specs/ with
SPEC, PLAN, TASKS, DATA-MODEL, RESEARCH, WIRING, `contracts/ui-behavior.md`, and feature contract links.

Navigate the full active corpus (every indexed file, line counts): [docs/COMPLETE-ACTIVE-CORPUS-INVENTORY.md](docs/COMPLETE-ACTIVE-CORPUS-INVENTORY.md). Master delivery tasks: [docs/LITECOWORK-BUILD-TASKS.md](docs/LITECOWORK-BUILD-TASKS.md). Crosswalk: [docs/SPEC-TASK-TRACEABILITY.md](docs/SPEC-TASK-TRACEABILITY.md).

1. Identify the owner from the coverage matrix and read the complete authority chain:
   architecture, product/release, domain owner, accepted ADR, machine schema and
   dependent flows/ledgers.
2. Define the user outcome, goals/non-goals, LOCKED/FLEXIBLE decisions, HLD/LLD,
   exact interfaces, data ownership, wiring, security, errors, recovery, configuration,
   EARS acceptance and explicit out-of-bounds paths before implementation.
3. Trace every requirement to an owner contract, task and executable acceptance check.
   Keep one normative definition for each entity, state machine, API and event; link to
   it elsewhere instead of copying it.
4. Record source/version/date for research and label current implementation evidence
   separately from target design. An archive, mock, schema or transport helper is not
   product qualification.
5. Submit the full package for owner review. Keep it DRAFT/IN_REVIEW until authorized
   owners explicitly accept and lock it; a passing validator does not provide approval.
6. Update dependent contracts, machine schemas, traceability ledgers, UI projections and
   local links in the same reviewed documentation change. Keep CURRENT-RUN.md current.
7. Keep LiteSPM API/package details in LiteSPM's version-pinned authority; do not infer
   wire methods or auth behavior from a catalogue or CLI output.
8. Run the feature package validator, Markdown link/schema checks and git diff --check.
   Record exact commands/results and known limitations in CURRENT-RUN.md.

Implementation does not begin for a material feature while its spec is DRAFT/IN_REVIEW
or while a decision blocking the assigned task remains unresolved, except when the
product owner explicitly delegates the internal product/engineering decision and
implementation. In that case, record the direction in the owning package and alignment
register as USER-DELEGATED DIRECTION; keep the package lifecycle, cross-owner
ratification, external protocol facts, credentials, platform evidence and release
qualification gates separate. Delegation is not owner acceptance or qualification.

## Implementation workflow

1. Inspect Git status, the owning contracts, callers/dependents, and relevant tests.
2. Keep each domain behind its documented owner and port.
3. Add focused acceptance coverage for behavior changes.
4. Run the narrowest relevant formatting, lint, type, and test checks; report exact
   commands and outcomes.
5. Review the complete diff, stage intended paths only, and commit a clear
   software-focused message.

Do not add a dependency without documenting why existing options do not fit. Never
commit local archives, extracted reference files, caches, credentials, or generated
output.
