# Contributing

## Architecture first

Read [ARCHITECTURE.md](ARCHITECTURE.md) and the focused documents under `docs/` that
cover the change. The ignored `archive_code/` and `archives_docs/` trees are historical
copies and are not active implementation or authority.

If a change alters an ownership boundary or invariant, update `ARCHITECTURE.md` and add
or amend an ADR in the same change. Detailed docs may explain the contract but must not
create a competing authority. Keep the first implementation slices narrow and prove a
vertical workflow before adding providers or broad domain features.

## Implementation workflow

1. Inspect Git status, current files, dependencies, and relevant tests.
2. Identify the Task, Agent, Capability, Runtime, Environment, Effect, Artifact, or
   Evidence owner involved.
3. Make the smallest coherent change at that boundary.
4. Add or update focused acceptance coverage for behavior changes.
5. Run relevant formatting, lint, type, and test checks; state any check that could not
   run and why.
6. Review the full diff, stage intended paths only, and commit a clear software-focused
   message.

Do not add a new dependency without checking whether an existing equivalent is
available. Do not commit local archive folders, caches, credentials, or generated output.
