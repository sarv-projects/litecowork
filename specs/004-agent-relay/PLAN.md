# Plan - Agent Relay on One Conversation

**Feature:** 004 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Spec revision:** 1.0

## Phases

B-15 and B-16 were owner-accepted on 2026-11-09. Phases follow TASKS.md:

1. Contract promotion of the portable-context bounds, truncation receipt and relay errors (004-T01).
2. Producer identity through the desktop types, which needs no new contract and can start in Wave 1 (004-T02).
3. Portable export and relay admission in storage/domain (004-T03, 004-T04).
4. Claude Code dispatch arm in `AgentSessionSupervisor` (004-T05) after Feature 002 delivers the second module.
5. Picker gating and relay UI states (004-T06, 004-T07).
6. Failure/recovery, cross-layer trace, E2E-02 qualification, documentation sync and VERIFIED (004-T08 to 004-T12).

Run the commands named per task and the desktop USER path E2E-02 in [ACCEPTANCE-AND-E2E-PLAN.md](../../docs/ACCEPTANCE-AND-E2E-PLAN.md).

## Dependency DAG

- The local DAG is in TASKS.md "Dependency order". T01 and T02 have no local predecessor.
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Conversation, Agent Runtime, Product.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require package acceptance, a clean selected worktree, and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

No migration is authorized by the current design crosswalk: the portable package is derived, and producer fields already exist on Conversation messages. A-66/A-96 leave the durable refusal/truncation receipt's record and storage placement for resolution in T01/T09. Only if that decision requires new persistence should a migration be allocated after the then-current highest wired schema (v34 in the 2026-11-20 checkout; v35 is now wired, with v36-v47 still provisional in `docs/LITECOWORK-BUILD-TASKS.md`). Rollback is revert to the previous committed revision and refuse new admission that needs any newly promoted field.

## Risks

- Adding the Claude Code arm with a string match, repeating the `OPENCODE`/`opencode` mismatch; dispatch must go through Feature 002's closed provider key.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner acceptance recorded 2026-11-09; owner USER sign-off still required for VERIFIED
- TASKS.md commands and denial, replay, restart and cancel cases
