# Plan - Automations and Recurring Host

**Feature:** 010 | **Status:** DRAFT

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

Owners: Automation, Task Runtime, Runtime lifecycle, Desktop UI.
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

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## External workflow scope

This package does not implement a graph schema, graph canvas or workflow execution engine. Its only V1-target integration task is the invocation of an existing qualified provider workflow as one CapabilityInvocation within an admitted Task; no production adapter is currently present. The provider owns graph authoring, graph revisions and internal execution states. Users may already use provider-native AI builders/MCP (for example n8n Assistant or Activepieces Build by Asking) inside the provider product, subject to that provider's feature/version/permission gates. What is deferred is direct LiteCowork-Coworker graph mutation through a provider API/MCP capability; that requires a separate owner ADR, exact permissions and a qualified provider-authoring adapter.
