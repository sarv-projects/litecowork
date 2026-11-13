# Plan - Native Agent Install, Auth and Binding

**Feature:** 002 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Spec revision:** 1.0

## Phases

SPEC section 5 blockers B-11 and B-12 and the A-35 screen split were owner-accepted on 2026-11-09. Phases now follow TASKS.md:

1. P1 prerequisite: make the OpenCode v2.0.32 profile truthfully bindable (002-T01), so Feature 001 Send is reachable from the UI.
2. Contract promotion: AGENT-CONTROL, OpenAPI, error codes and event schemas together (002-T02). The descriptor store reuses the table created by migration 16, so it does not need new descriptor DDL. The accepted durable auth-observation record and replay-safe credential receipt each need additive persistence; coordinate their next unused migration numbers with the global allocator before adding files.
3. Module registry, descriptor storage and descriptor producer (002-T03 to 002-T05), then the fresh-descriptor enable gate (002-T06).
4. Native sign-in observation and credential slots (002-T07, 002-T08).
5. Composer options/commands and Settings operations wired into the Feature 011 screen (002-T09, 002-T10), honest install/update unavailability (002-T11).
6. Claude Code CLI as the second module (002-T12), failure/recovery (002-T13), cross-layer trace (002-T14).
7. ACP client and advisory Matchmaker slices run on their own dependencies: 002-T17 follows T03; 002-T18 follows T05 and T17. Neither makes an agent dispatchable or replaces provider qualification.
8. Qualify the exact release-matrix journeys (002-T15), synchronize evidence and ledgers (002-T16), then record package VERIFIED only after 002-T19 owner sign-off. T15 is deferred by owner instruction; the package remains unverified until those gates are completed.

Run the commands named per task and the desktop USER path in [ACCEPTANCE-AND-E2E-PLAN.md](../../docs/ACCEPTANCE-AND-E2E-PLAN.md) (E2E-01 binding portion; E2E-02 with Feature 004).

## Dependency DAG

- The local DAG is in TASKS.md "Dependency order". T01 and T02 have no local predecessor; T01 is the first implementation change.
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Agent Runtime, Security/Trust, Desktop UI, Product.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require package acceptance, a clean selected worktree, and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

The implementation currently uses the existing `agent_control_descriptors` table (created by migration 16) and has no durable auth-observation table or credential-submission receipt. Do not create duplicate descriptor DDL. If durable auth observations or replay receipts are implemented, allocate the next unused immutable migration through `docs/LITECOWORK-BUILD-TASKS.md` and land it with the owner doc, schema and tests in one change. Rollback is revert to the previous committed revision and refuse new admission that needs the new field.

## Risks

- Confusing the exact OpenCode CODE offer predicate with a qualified native provider journey; the packaged bind/send/stop/restart path remains unproven.
- Duplicating the descriptor table or creating an unallocated migration because an old task row still names v33 for an already-existing table.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner acceptance recorded 2026-11-09; owner USER sign-off still required for VERIFIED
- TASKS.md commands and denial, replay, restart and cancel cases

## DeepSeek Harness proposals (PROPOSED, owner review required)

PROPOSED only; see the matching section in [TASKS.md](TASKS.md). Nothing here changes the accepted design of this package. Owner review is required first.

- **P-38 Login-shell PATH-class capture for agents launched from a Linux desktop entry** (owns the proposed row). Class **OWNER (open; not ADOPT, ADAPT or REJECT yet)**. Design impact if accepted: T07 and T13 of package 002; package 005 Environment admission. CONFLICT with environment scrubbing (A-58): stays open. Deferral: Blocked on the owner decision. Then a Linux desktop-entry launch test. Upstream: [src/login-shell-environment.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/login-shell-environment.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-38 and the conflict note).

