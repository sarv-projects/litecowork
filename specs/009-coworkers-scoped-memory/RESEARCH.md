# Research - Coworkers and Scoped Memory

**Feature:** 009 | **Checked:** 2026-11-12 against active source checkpoint `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`
**Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 amendment IN_REVIEW. This file is a source-backed implementation crosswalk (current-state evidence only); A-91 remains open and gates manual-memory schema/persistence.

## Canonical authorities

- [COWORKERS-TARGET.md](../../docs/COWORKERS-TARGET.md) owns the accepted Coworker product/UX target, including Coworker-owned Conversations, optional Quick Create chat-first semantics, scoped Memory & Knowledge and status distinctions. It is target design, not implementation proof.
- [RESPONSIBILITIES.md](../../docs/RESPONSIBILITIES.md) and the Responsibility domain own persisted Coworker definition/revision/status behavior.
- [CONTEXT.md](../../docs/CONTEXT.md) owns ContextDocument/resource scope, retrieval and deletion/revocation semantics.
- [LEARNING.md](../../docs/LEARNING.md) owns the target LearningCandidate, CoworkerLearningPolicy and private-memory pipeline.
- [ADR-0024](../../docs/adr/0024-optional-chat-first-coworkers.md) keeps Coworkers optional and preserves ordinary chat.
- [IMPLEMENTATION-STATUS.md](../../docs/IMPLEMENTATION-STATUS.md) owns the detailed current source/route/UI crosswalk.

## Source files inspected

- Coworker Operator: `apps/litecoworkd/src/coworker_operator.rs`.
- Coworker persistence: `crates/storage-sqlite/src/coworkers.rs`; Coworker model/validation: `crates/domain-responsibility/src/models.rs`, `service.rs`.
- Home/Settings UI: `apps/litecowork-ui/src/coworkers/HomeCoworkerOnboarding.tsx`, `CoworkerSettings.tsx`, `coworker-api.ts`.
- Conversation record/operator/UI: `crates/storage-core/src/conversation.rs`, `apps/litecoworkd/src/conversation_operator.rs`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx`.
- ContextDocument owner validation: `crates/domain-workspace/src/lib.rs::valid_context_document_metadata`; resource upload/status/purge routes in `apps/litecoworkd/src/operator.rs`.
- Retrieval: `apps/litecoworkd/src/rag_retrieval_operator.rs`, `crates/storage-sqlite/src/rag_retrieval.rs`.
- Deterministic learning gates: `crates/storage-core/src/coworker_learning.rs`; SkillProposal redaction reuse: `crates/storage-core/src/skill.rs`.
- Storage module inventory: `crates/storage-sqlite/src/lib.rs`.

## Borrowable-design sources (pinned)

| Source | Exact pinned URL | License | What informs this feature | Verdict |
|---|---|---|---|---|
| Rowboat living knowledge-graph-as-context (`rowboatlabs/rowboat`) | https://github.com/rowboatlabs/rowboat/tree/9052e28b9ede46478e04f9cf600c7ebaf5895dc7 | Apache-2.0 at checked root LICENSE | A local desktop coworker that indexes work into a living knowledge graph used as retrieval context (`apps/x` desktop + `apps/harbor` agent runtime). It is a reference for how Coworker-scoped Memory & Knowledge can be surfaced as retrieval context, and for keeping provenance/deletion owner explicit on each entry. | REJECT the auto-indexing crawler model for V1. MAP the pattern to quiet, owner-authored, scoped Memory + Knowledge only: Add Memory writes an owner-authored Coworker-scoped ContextDocument (`MemoryRecordMetadata.status`), never an automatic `LearningCandidate`; the automatic-learning pipeline stays deferred (B-24). Do not adopt harbor as LiteCowork Task authority and import no Rowboat code. |

No Rowboat code is imported; it is read at the pinned SHA as a design reference. LiteCowork's V1 memory is owner-authored and quiet, not an ambient indexer; the single automatic-learning candidate vocabulary (`LearningCandidate`) is owned by [LEARNING.md](../../docs/LEARNING.md) per register A-34/A-20. `MemoryRecordMetadata.status` applies to its defined automatic-record contract; the matching manual-memory representation required by B-24 remains open under A-91.

## Findings from the active source tree

1. Coworker list/create/get/revision/status and Workspace primary selection are served and stored. These operations create/update Coworker configuration; they do not start an agent or create a Coworker Conversation.
2. Home Quick Create creates a general Assistant and may set it as Workspace primary. The UI explains that primary only prefills future Task settings. It does not open a chat.
3. Conversation records and create requests have no Coworker origin/revision. Task records do pin `origin_coworker_id` and `origin_coworker_revision`; that Task provenance must not be described as Conversation ownership.
4. Coworker Settings persists `CoworkerContextPolicy` as part of CoworkerRevision. The domain validator checks its shape/ranges and forces `require_user_confirmation_for_memory=true`. No production retrieval caller enforces `allowed_context_kinds`, `max_retrieved_items` or `retain_task_summaries`. This is not the separate `CoworkerLearningPolicy`.
5. ContextDocument creation is narrower than its generic schema/UI: `valid_context_document_metadata` accepts USER-owned `PERSONAL_PROFILE` and WORKSPACE-owned `WORKSPACE_NOTES`; it rejects `COWORKER_NOTES` and `GOAL_NOTES`. Coworker Settings still lists both unsupported options.
6. `POST /v1/resources/retrieval` is Workspace-authorized and accepts Workspace scope or explicit Resource references. Its contract has no Coworker ID/revision or Coworker context-policy input. It is lexical/read-only and is not wired to Coworker Conversation assembly.
7. `GET /v1/coworkers/{id}/presence` is mounted but responds 503 `DEPENDENCY_UNAVAILABLE` after validating Coworker access because Task, UserRequest, Attempt and Runtime projections are not yet available.
8. `storage-core/src/coworker_learning.rs` defines deterministic data types, source eligibility, redaction, sensitivity/conflict and auto-commit gates plus a bounded event-payload builder. Its module documentation says it does not classify free text, call a model or persist candidates. `redact_sensitive_content` is reused by SkillProposal extraction in `storage-core/src/skill.rs`; this does not feed a LearningCandidate pipeline.
9. No production caller of `evaluate_learning_candidate`, `classify_learning_conflict` or the Coworker learning policy was found. No SQLite LearningCandidate/MemoryRecord table/migration/store, LearningService, learning Operator route/event producer, Coworker-private Memory list/editor, Learning Inbox or Coworker-scoped context attachment path was found.
10. No runtime was started and no owner USER flow was executed during this source audit. Presence, Conversation ownership, scoped retrieval, memory creation/revocation and restart isolation remain unqualified.
11. Coworker Settings also persists default lead, delegation strategy, worker-profile allowlist, interaction defaults, lead-failover preferences and notification preferences. Default lead is consumed by Task creation; lead-failover is copied into TaskSpec but no production fallback dispatcher is wired; interaction, notification and delegation strategy have no production consumer. Context policy remains unconsumed by retrieval.
12. enabled_delegation_profile_ids are validated against same-Workspace enabled DelegationProfiles and read by a SQLite child-Attempt admission guard; delegation_budget_policy is also read by that storage primitive. No production daemon/Operator caller invokes child-Attempt admission, so these checks do not constitute live worker dispatch.
13. Coworker PAUSED is only partially enforced. Automation enable checks Coworker active at enable time, but a Coworker can be paused while an Automation remains enabled and the TriggerHost does not re-check Coworker status at occurrence settlement. The origin-status helper has no production caller. Task creation storage rejects ARCHIVED, not PAUSED; the Task composer separately blocks PAUSED Coworkers for owner Tasks. The UI copy that pause stops new proactive/scheduled work is therefore stronger than the implemented fence.
14. The Coworker editor exposes profile assignment and interaction/failover/context/notification controls, but not avatar selection, delegation-budget editing or delegation-policy-source selection. The presence view is unavailable; there is no Coworker-owned chat, memory browser or Learning Inbox.

## Architecture implications

- Preserve the product target: ordinary chat remains available without a Coworker; Coworker navigation should open the Coworker's last-active Conversation, and Quick Create should end in its first chat.
- Keep Task Coworker origin distinct from Conversation Coworker origin. A Task pin cannot substitute for the latter.
- Keep user-authored ContextDocuments distinct from inferred/automatically learned MemoryRecords. Do not create a LearningCandidate for a manual Resource edit.
- Do not silently weaken the COWORKER_NOTES/GOAL_NOTES owner boundary or infer that a persisted context-policy setting filters retrieval.
- Do not enable automatic learning until a qualified extraction provider, current CoworkerLearningPolicy revision, candidate persistence/Resource payload path, atomic commit/reconcile, revocation/purge, events, review UI and cross-scope tests are implemented together.
- Coworker presence must remain visibly unavailable rather than synthesizing activity from Coworker configuration.
- Memory and context changes never create Grants, Connections, package assignments, schedules, Task dispatch or new agent authority.

## Owner decisions and unresolved contracts

- **B-24 is ACCEPTED (2026-11-09):** first Coworker memory is owner-authored Coworker-scoped ContextDocument data; automatic LearningCandidate processing is deferred. Personal Profile or Workspace Notes cannot be relabelled as Coworker-private memory. The accepted decision does not mean the create, retrieval, revoke/purge or UI path is implemented.
- **B-25 is ACCEPTED (2026-11-09):** Quick Create creates the Coworker and first Conversation as separate idempotent commits and leaves the Coworker UNBOUND until Feature 002 provides a qualified binding. The opened Conversation may exist before Send is enabled.
- **A-18 is ACCEPTED (2026-11-09):** the create enum stays aligned with the current two-kind domain validator until the scoped Coworker/Goal owner paths exist. COWORKER_NOTES remains a Feature 009 target; do not describe it as currently creatable.
- **A-91 remains OPEN:** B-24 names MemoryRecordMetadata.status for manual Coworker memory, while LEARNING.md §7 reserves MemoryRecordMetadata for automatic entries with a real LearningCandidate source id. The owner must settle one manual-memory provenance/status representation before T01 finalizes its migration or T05 implements storage. Never invent a candidate id or add a second memory lifecycle.

## Verification

- Source inspection and targeted recursive symbol search refreshed 2026-11-12 against active source checkpoint 6adadab539a62ca1c8b70621c7f88e1785cfe8c2 on main.
- No runtime, Cargo tests, desktop USER flow or real-provider behavior was exercised in this Feature 009 research pass.
- Structural package, architecture, route and Markdown validators are run after the documentation changes; those validators do not establish runtime or owner acceptance.
