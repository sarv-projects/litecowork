# Plan - Automations and Recurring Host

**Feature:** 010 | **Spec:** SPEC.md revision 1.0
**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Updated:** 2026-11-09. The authoritative ordered work list is [TASKS.md](TASKS.md) (T01 to T17 for V1; T18 and T19 are outside the V1 gate).

## Phases

1. B-26 (cron in a named timezone first, RUN_ONCE_WHEN_AVAILABLE default), B-27 (LOCAL_POLL channel, delivery status separate from Task outcome) and A-35 (quiet-hours control surface owned here) were accepted by the repository owner on 2026-11-09. The event-trigger kind required by E2E-10 is added after cron (TASKS.md T09).
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

No migration has shipped for this package; the current chain ends at `docs/schemas/sqlite-v32.sql`. v44 is a provisional reservation in `docs/LITECOWORK-BUILD-TASKS.md`, not permission to duplicate schema already present in `sqlite-v1.sql`: notification delivery status names, attempt Runtime/epoch/count, `next_attempt_at`, `settled_at`, and encrypted Runtime-local `automation_trigger_bindings` already exist. The required delivery row CAS/version field and whether v44 is needed at all remain unresolved under A-98. Do not add a migration until the owner defines the exact additive delta; take the next free number if another package merges first. Rollback is revert to the previous committed revision and refuse new admission that requires a newly promoted field.

## Risks

- Implementing ahead of the TASKS.md dependency order or before the T01 contract promotion lands.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner review
- After lock: TASKS.md commands and denial, replay, restart and cancel cases


## External workflow scope

This package does not implement a graph schema, graph canvas or workflow execution engine. Its only V1-target integration task is the invocation of an existing qualified provider workflow as one CapabilityInvocation within an admitted Task; no production adapter is currently present. The provider owns graph authoring, graph revisions and internal execution states. Users may already use provider-native AI builders/MCP (for example n8n Assistant or Activepieces Build by Asking) inside the provider product, subject to that provider's feature/version/permission gates. What is deferred is direct LiteCowork-Coworker graph mutation through a provider API/MCP capability; that requires a separate owner ADR, exact permissions and a qualified provider-authoring adapter.

## DeepSeek Harness proposals (PROPOSED, owner review required)

PROPOSED only; see the matching section in [TASKS.md](TASKS.md). Nothing here changes the accepted design of this package. Owner review is required first.

- **P-31 QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once)** (touches this package; row owned by package 011). Class **ADAPT**. Design impact if accepted: T12, T13, T14 of package 011; T11 of package 019; T10 of package 010. Deferral: Daemon half (`RuntimeLifecycleStore`, served `previewLocalRuntimeStop`) can be built and tested on Linux; the packaged quit path needs the Windows laptop. Not started. Upstream: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31).

