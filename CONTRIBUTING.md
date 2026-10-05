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

1. Identify the owning document from the coverage matrix.
2. Update all affected contracts, schemas, flows, failure cases, and UI projections
   together.
3. Preserve one normative definition for each entity, state machine, API, and event;
   other documents link to that definition.
4. Keep LiteSPM API/package details in LiteSPM's authority. LiteCowork records only the
   chosen endpoint and its integration boundary until that contract is supplied.
5. Run Markdown link, schema, and diff checks appropriate to the edit.

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
