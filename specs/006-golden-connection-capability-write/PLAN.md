# Plan - Golden Connection and One Capability Write

**Feature:** 006 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

## Phases

1. Owner decision on the authentication model and effective provider permissions (T02), followed by contract promotion (T01); both are blocked until B-19/A-82 receive renewed owner approval.
2. Foundations (T03, T04, T07): keystore custody with a broker-only resolver, brokered HTTP egress policy, Invocation dispatch schema opened only through the combined admission transaction.
3. Connection (T05, T06): ConnectionService and refresh, after T02.
4. Operations (T08-T11): golden read, golden write with exact approval, UNKNOWN reconciliation, revoke.
5. Surfaces (T12, T13): Operator routes, Tauri bridge, Discover Connections UI.
6. Recovery (T14), then E2E-05 qualification (T15) with the desktop USER path in [ACCEPTANCE-AND-E2E-PLAN.md](../../docs/ACCEPTANCE-AND-E2E-PLAN.md). **The Task-write row order is OPEN under A-80:** Connection/read evidence may be collected per available OS row, but the write journey follows Feature 005's Environment qualification order if the write requires an admitted Environment. Do not mark the combined journey VERIFIED until the owner decision is reflected in the platform rows. Then documentation sync (T16) and VERIFIED (T17).

## Dependency DAG

- T02 (owner) blocks T01 and ConnectionService; T01 then blocks contract-dependent implementation tasks. The full DAG is in [TASKS.md](TASKS.md#dependency-order).
- 005 Trust wiring and Attempt admission precede dispatch (Wave 3 after 005).
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Connectors, Capability Fabric, Security, LiteSPM boundary, Desktop UI.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require package acceptance, a clean selected worktree, and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

No migration has shipped for this package yet. T01 promotes one forward-only immutable migration (`docs/schemas/sqlite-v40.sql` per the Migration allocation in docs/LITECOWORK-BUILD-TASKS.md; take the next free number if another package merges first): a `connections` table rebuild adding `REVOKED`, an `auth_attempts` table and the remote-revocation field; T07 replaces the `sqlite-v15.sql` dispatch triggers with admission-bound guards. Rollback is revert to the previous committed revision and refuse new admission that needs the new field.

## Risks

- Writing ConnectionService code before the T02 recipe decision.
- Letting the LiteSPM install path leak into this package (013 owns it).
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner review recorded 2026-11-09 for B-19/B-20; renewed T02 review for authentication and effective provider permissions pending
- TASKS.md commands and denial, replay, restart and cancel cases
