# Plan - Workbench Browser Terminal and Computer

**Feature:** 008 | **Status:** DRAFT

## Phases

1. Resolve SPEC section 5 blockers with the named owners. No code.
2. Promote any missing machine-schema field in the canonical owner and `docs/schemas/` together if the locked design needs it.
3. Implement tasks in dependency order on a recovered source tree after ACCEPTED/LOCKED.
4. Run the cargo filters in TASKS.md and the desktop USER path in [ACCEPTANCE-AND-E2E-PLAN.md](../../docs/ACCEPTANCE-AND-E2E-PLAN.md).

## Dependency DAG

- T01 blocks later tasks.
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Desktop shell, Environments, Browser provider, Trust, Desktop UI.
Allowed implementation paths are listed per task. This docs-first checkout has no `apps/` or `crates/` tree; those paths are the future scope, not files to create here.

## Migration, rollout, rollback

No migration ships in DRAFT. A later schema change uses the next immutable SQLite migration. Rollback is revert to the previous committed revision and refuse new admission that needs the new field.

## Risks

- Implementing from this DRAFT before blockers close.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner review
- After lock: TASKS.md commands and denial, replay, restart and cancel cases
