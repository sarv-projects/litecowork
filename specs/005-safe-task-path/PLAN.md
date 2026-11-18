# Plan - Safe Task Path

**Feature:** 005 | **Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 IN_REVIEW with owner-directed implementation authorization; not a claim of implementation or qualification.

## Phases

1. Contract promotion (T01): owner docs, OpenAPI, event contract, Environment/session binding and the next immutable SQLite migration in one change. Revision 1.1 is owner-directed for implementation and awaits cross-owner review.
2. Foundations in parallel (T02-T07, T12): type/probe consolidation, plan acceptance storage, Linux bubblewrap provider and attestation, planning dispatch, B-18 replace primitive, Trust/grant wiring, Conversation promotion.
3. Execution path (T08-T11): Attempt admission, reconciliation and verification, Operator routes and Tauri bridge, cancel/crash/restart recovery.
4. UI (T13-T14): Work execution panel and the Windows-row UNAVAILABLE posture.
5. Linux-row qualification (T15) with the desktop USER path in [ACCEPTANCE-AND-E2E-PLAN.md](../../docs/ACCEPTANCE-AND-E2E-PLAN.md) E2E-04.
6. B-35 Windows provider (T16 owner decision, T17 implementation, T18 Windows-row qualification), after the Linux slice.
7. Documentation sync (T19) and VERIFIED (T20).

## Dependency DAG

- T01 contracts are being promoted under the owner's 2026-11-17 implementation direction. T03-T08 retain their explicit task dependencies; T16 is a separate owner decision that only blocks T17 and T18. The full DAG is in [TASKS.md](TASKS.md#dependency-order).
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Task Runtime, Environments, Trust, Storage, Desktop UI.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require revision-specific review, a clean selected worktree, and source/contract/UI/test reconciliation. User authorization permits implementation of revision 1.1 while cross-owner review remains pending; it does not make that revision ACCEPTED or QUALIFIED.

## Migration, rollout, rollback

T01 wires the forward-only immutable migration v33 at the first unmerged version assigned by `docs/LITECOWORK-BUILD-TASKS.md`. It binds newly reserved Task AgentSessions to Environment identity; it does not provision or attest an Environment, open CapabilityInvocation creation or dispatch, or serve Task admission. Revision 1.1 remains IN_REVIEW pending cross-owner review. A merged migration is never renumbered. Rollback is revert to the previous committed revision and refuse new admission that needs the new field.

## Risks

- Implementing B-35 work before the owner accepts B-35.
- Shipping a Windows build that admits an Attempt with a Job Object as the only boundary.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner review recorded 2026-11-09 for revision 1.0 B-17/B-18; revision 1.1 cross-owner review pending; B-35 pending
- TASKS.md commands and denial, replay, restart and cancel cases

## DeepSeek Harness proposals (PROPOSED, owner review required)

PROPOSED only; see the matching section in [TASKS.md](TASKS.md). Nothing here changes the accepted design of this package. Owner review is required first.

- **P-38 Login-shell PATH-class capture for agents launched from a Linux desktop entry** (touches this package; row owned by package 002). Class **OWNER (open; not ADOPT, ADAPT or REJECT yet)**. Design impact if accepted: T07 and T13 of package 002; package 005 Environment admission. CONFLICT with environment scrubbing (A-58): stays open. Deferral: Blocked on the owner decision. Then a Linux desktop-entry launch test. Upstream: [src/login-shell-environment.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/login-shell-environment.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-38 and the conflict note).
- **P-27 durable run record plus process-group liveness fence** (ADAPT): see TASKS.md PROP-P27 / plugin lessons. Study: [plugin study](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) Sections 2, 3 and 8. Upstream: [operations.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/operations.ts), [run-tree.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/run-tree.ts).

