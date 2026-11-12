# Plan - Coworkers and Scoped Memory

**Feature:** 009 | **Spec:** SPEC.md revision 1.1 (amendment IN_REVIEW; revision 1.0 remains ACCEPTED/LOCKED)
**Status:** Existing revision 1.0 implementation scope remains accepted; revision 1.1 amendment IN_REVIEW. A-91 gates manual-memory schema and persistence. This is not a claim of implementation or qualification.
**Updated:** 2026-11-12. The authoritative ordered work list is [TASKS.md](TASKS.md) (T01 to T18).

## Phases

1. B-24 and B-25 were accepted by the repository owner on 2026-11-09 (SPEC §5): the first memory is owner-authored and Coworker-scoped, with B-24 naming the committed-record status lifecycle; Quick Create starts UNBOUND until Feature 002 qualifies a binding. LEARNING.md §7 defines its `MemoryRecordMetadata` struct as automatic-only with required candidate provenance, so A-91 must settle how the accepted manual-memory status is represented before schema or persistence is finalized. The automatic LearningCandidate pipeline belongs to Feature 020.
2. Promote accepted machine-schema fields in their canonical owner and `docs/schemas/` in the same reviewed change (TASKS.md T01). Register A-18 is owner-accepted: the current OpenAPI `ContextDocumentCreateMetadata.kind` enum is narrowed to PERSONAL_PROFILE and WORKSPACE_NOTES, matching `domain-workspace`; record/read types may retain all four values. T04 can add COWORKER_NOTES only alongside the authenticated Resource upload owner check for the same Workspace and a current, non-ARCHIVED Coworker. Keep the control unavailable until the complete T05 scoped memory path exists. A-91 must be resolved before T01 finalizes manual-memory metadata or the v38 migration.
3. Implement Conversation Coworker origin and first-chat flow separately from Task Coworker origin; then implement Coworker owner-scoped ContextDocument and Coworker-filtered retrieval with read/revoke/purge fences. Do not reuse Workspace-only retrieval as if it were Coworker scoped.
4. After A-91 is resolved and T01 promotes one manual-memory representation, implement the accepted minimal owner-authored memory record through ResourceService. Automatic learning is outside this slice; its target pipeline remains in `docs/LEARNING.md` and needs separate storage/provider/service/API/event/UI promotion.
5. Implement only work covered by the accepted revision in dependency order on a recovered source tree. Do not implement the revision 1.1 amendment until owner approval; A-91 continues to gate manual-memory schema and persistence.
6. Run the cargo filters in TASKS.md, package/contract validators and the desktop USER path in [ACCEPTANCE-AND-E2E-PLAN.md](../../docs/ACCEPTANCE-AND-E2E-PLAN.md).

## Dependency DAG

- T01 blocks later tasks.
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Coworkers, Conversation, Memory, Trust, Desktop UI.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require package acceptance, a clean selected worktree, and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

No migration has shipped for this package yet; the chain ends at `docs/schemas/sqlite-v32.sql`. The current build allocation reserves v38 for Feature 009 Conversation Coworker origin and memory fields, but no v38 migration exists and A-91 blocks finalizing its manual-memory fields. Any further accepted schema change (capability assignment, responsibility proposals) lands as the next free immutable `sqlite-vNN.sql` in the task that needs it. Rollback is revert to the previous committed revision and refuse new admission that needs the new field.

## Risks

- Implementing ahead of the TASKS.md dependency order or before the T01 contract promotion lands.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner review
- After lock: TASKS.md commands and denial, replay, restart and cancel cases
