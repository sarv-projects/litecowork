# Plan - Workspace Roots and Context Ingestion

**Feature:** 003 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Spec revision:** 1.0 | **Release target:** V1 (Windows x64 + OpenCode v2.0.32 first; Linux x64 second)

## Phases

1. Contract promotion (TASKS T01): promote ContextAttachment, retrieval receipt, Windows root-selection behavior and the platform-availability signal into WORLD-RESOURCES, CONTEXT, STORAGE, API, EVENTS and SCHEMAS together with `docs/schemas/operator-api.openapi.yaml`, `domain-event.schema.json`, `error-codes.schema.json` and the next immutable `docs/schemas/sqlite-vNN.sql`.
2. Storage and domain (T02, T09): root lifecycle qualification on both rows; ContextAttachment and retrieval-receipt stores with CAS, idempotency and negative tests.
3. Windows GA row (T03-T06): Windows root selection, startup identity revalidation, Windows bounded rescan and the symlink/reparse/path-identity matrix.
4. Content ingestion (T07): bounded allowlisted UTF-8 text ingestion from ACTIVE roots into Resource revisions (B-13).
5. Send integration and UI (T08, T10-T12): platform-honest folder controls, retrieval receipt from Send, @-reference picker in the 001 composer, Context Used.
6. Failure and recovery (T13), cross-layer trace (T14), qualification on the release rows (T15), documentation sync (T16), package VERIFIED (T17).

## Dependency DAG

- T01 blocks every implementation task.
- Windows tasks T03-T06 run in parallel with Send-path tasks T09-T11 once T01 and T02 land.
- Feature 001 composer attachment handler and B-08 materialization gate T10 and T11; Feature 011 Windows Operator IPC gates the Windows row in T15.
- External side effects (filesystem traversal, native picker, agent dispatch) never run inside the transactions declared in WIRING.yaml.

## Owners and file scope

Owners: Resource, Conversation, Storage, Security/Trust, Desktop UI. Allowed implementation paths are listed per task in TASKS.md. The active source tree is in apps/ and crates/. Changes stay inside task-owned paths and require a clean selected worktree and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

T01 adds the `context_attachments` and `retrieval_receipts` tables (accepted target interface, not yet in source) in the immutable `docs/schemas/sqlite-v37.sql` (allocation in docs/LITECOWORK-BUILD-TASKS.md Migration allocation; v33 is wired by 005-T01 and v34 by 015-T22; if another package merges first, take the next free number and update that table in the same change). The migration is additive; no existing table is rewritten. Rollback is revert to the previous committed revision and refuse new admission that needs the new tables; committed receipts are retained and are never reinterpreted.

Rollout order: Linux row behavior is already compiled for add/scan, so Windows add/scan ships behind the platform-availability signal and is enabled only after T06 passes on the Windows row.

## Risks

- Claiming Windows add/scan before the reparse-point and identity matrix passes on native Windows (WSL2 is not evidence).
- Treating a bare search hit or an ACTIVE root as an implicit attachment.
- Two Feature 001 and 003 authors changing turn admission at the same time; T10 coordinates through the 001 owner.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- TASKS.md commands, including denial, replay, restart and revoke-race cases
- CODE, SYSTEM and owner USER evidence for E2E-03 on each release-matrix row
