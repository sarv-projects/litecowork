# Plan - Coworkers and Scoped Memory

**Feature:** 009 | **Status:** DRAFT

## Phases

1. Resolve SPEC §5 blockers B-24/B-25 with Product, Coworkers, Conversation, Memory and Trust owners. No code before the selected ContextDocument/memory record, purge model and Quick Create binding rule are explicit.
2. Promote any required machine-schema field in its canonical owner and `docs/schemas/` in the same reviewed change. The current OpenAPI four-kind ContextDocument enum must not be mistaken for current domain acceptance: `domain-workspace` currently accepts only USER/PERSONAL_PROFILE and WORKSPACE/WORKSPACE_NOTES.
3. Implement Conversation Coworker origin and first-chat flow separately from Task Coworker origin; then implement Coworker owner-scoped ContextDocument and Coworker-filtered retrieval with read/revoke/purge fences. Do not reuse Workspace-only retrieval as if it were Coworker scoped.
4. Implement the accepted minimal owner-authored memory record first. Automatic learning is explicitly outside this slice; its target pipeline remains in `docs/LEARNING.md` and needs separate storage/provider/service/API/event/UI promotion.
5. Implement tasks in dependency order on a recovered source tree after ACCEPTED/LOCKED.
6. Run the cargo filters in TASKS.md, package/contract validators and the desktop USER path in [ACCEPTANCE-AND-E2E-PLAN.md](../../docs/ACCEPTANCE-AND-E2E-PLAN.md).

## Dependency DAG

- T01 blocks later tasks.
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Coworkers, Conversation, Memory, Trust, Desktop UI.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require package acceptance, a clean selected worktree, and source/contract/UI/test reconciliation.

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
