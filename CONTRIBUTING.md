# Contributing to LiteCowork

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
or while a decision blocking the assigned task remains unresolved.

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
