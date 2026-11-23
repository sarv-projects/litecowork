# Plan - Professional Artifacts One Format

**Feature:** 007 | **Status:** Base revision 1.0 ACCEPTED/LOCKED; revision 1.1 Saved View amendment IN_REVIEW after owner direction 2026-11-17. The amendment does not authorize implementation before cross-owner review and contract promotion.
**Spec revision:** 1.0 | **Release target:** V1 (Windows x64 first; Linux x64 second)

## Phases

1. Contract promotion (TASKS T01): promote B-21 Markdown and Saved View media rules, B-30 source classes and atomic Saved View create, B-31 export receipt, the capability matrix and Conversation origin into ARTIFACTS-EVIDENCE, API, EVENTS and SCHEMAS with the OpenAPI contract, canonical Saved View schema, `artifact.exported.v1`, error taxonomy and next immutable migration. Saved View uses existing ArtifactVersion/ResourceRevision and event/receipt owners; no separate table or event is allocated.
2. Storage and format (T02-T06): atomic create, Markdown adapter, Markdown version publish, capability matrix producer, export receipt. T06 remains blocked on A-81 until owner review resolves the SPEC §7 versus HLD/API/T06 ordering conflict and local write-before-receipt recovery; do not promote its command/schema ahead of that review.
3. Operator routes and Tauri bridge (T07); Conversation association (T08); imported source class (T12).
4. UI (T09-T11): Library and Workbench create/edit/export, read-only/unsupported rendering for DOCX/XLSX/PPTX/PDF, stored-version compare.
5. Saved View extension (same create/version/route/UI/recovery tasks): after Feature 016 T01/T02 and Feature 007 cross-owner acceptance, create a self-contained Saved View Artifact in Library, reopen the exact version, append reviewed updates, and archive it. No hard Delete, export of its internal JSON, or permanent Saved Apps destination.
5. Failure/recovery (T13), cross-layer trace (T14), qualification of E2E-07 on both rows (T15), documentation sync (T16), package VERIFIED (T17).

## Dependency DAG

- T01 blocks every implementation task.
- T02 and T03 run in parallel after T01; T04 needs both. T05 needs only T01.
- Task-generated source class waits for Feature 005 Task output; the user-authored Markdown path does not.
- External side effects (format engine, native picker, local file write) never run inside the transactions declared in WIRING.yaml.

## Owners and file scope

Owners: Artifacts, Presentation, Storage, Desktop UI. Allowed implementation paths are listed per task in TASKS.md. The active source tree is in apps/ and crates/. Changes stay inside task-owned paths and require a clean selected worktree and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

T01 adds `artifact_export_receipts` and nullable `source_conversation_id` / `source_message_id` columns on artifacts (accepted target interface, not yet in source) in the immutable `docs/schemas/sqlite-v38.sql` (allocation in docs/LITECOWORK-BUILD-TASKS.md; v34 is wired in the 2026-11-20 checkout; take the next free number if another package merges first and update that table). The migration is additive. Rollback is revert to the previous committed revision and refuse create/export admission that needs the new fields; committed receipts and versions are never rewritten.

Rollout: the capability matrix ships first so the UI can hide unsupported controls before new write paths exist.

## Risks

- Two Markdown dialects (daemon validator versus the hand-written UI preview) drifting apart; T03 and T09 test both against one fixture set.
- Presenting a PDF or Office preview as editing.
- SPEC §7 says receipt before “any external call,” while HLD/API/T06 say format/write/verify before receipt. The local file can exist without a receipt, so A-81 requires one owner-approved order and explicit UNKNOWN/restart recovery without exposing the selected path.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- TASKS.md commands, including denial, replay, restart and crash cases
- CODE, SYSTEM and owner USER evidence for E2E-07 on each release-matrix row
