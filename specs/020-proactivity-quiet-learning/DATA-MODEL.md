# Data Model - Proactivity and Quiet Learning

**Status:** IN_REVIEW - not owner-accepted; authorizes no implementation; not a claim of implementation or qualification.
**Feature:** 020 | **Authority:** RESPONSIBILITIES.md (Suggestion, Goal), LEARNING.md (sections 3 to 7, 19, 20), DATA-MODEL.md, STORAGE.md, EVENTS.md.

This feature reuses the canonical Suggestion, SuggestionPreference, Goal and Needs You records and adds the learning records LEARNING.md already defines. It must not create a second Suggestion store, a parallel memory transcript or a second candidate vocabulary.

## 1. Existing records (CODE)

| Record | Identity/scope | Owner | Feature use |
|---|---|---|---|
| Suggestion | `suggestion_id`; Workspace; optional Coworker; `dedupe_key`; `version` | SuggestionService | Proposals; one PROPOSED per Workspace and dedupe key (`uq_suggestions_open_dedupe`) |
| SuggestionPreference | (`workspace_id`, `kind`); virtual default `muted=false, version=0` | SuggestionService | Mute per kind |
| SuggestionTaskAcceptanceReceipt | acceptance request id | storage-sqlite | Atomic READY Task + ACCEPTED Suggestion |
| Goal, GoalRevision | `goal_id`, `revision` | GoalService | Producer input (GOAL_NEXT_STEP); progress projection |
| NeedsYou item | (kind, source id) projection | storage-core read store | Read-only inbox |
| notification_preferences.quiet_hours_json | (`workspace_id`, `event_class`) | Feature 010 | Quiet-hours window |

## 2. New records (PROPOSED; promoted by T02)

| Record | Identity | Fields | Notes |
|---|---|---|---|
| LearningCandidate | `candidate_id`; Workspace; Coworker | as `LearningCandidate` in `crates/storage-core/src/coworker_learning.rs` and LEARNING section 3: kind, source_kind, source ids, source_digest, proposed_payload_ref, redaction_report_ref, classifier_version, confidence, state, reason_codes, created_at, updated_at, version | Payload and report are Resource refs; no body column |
| CoworkerLearningPolicy | (`coworker_id`, `revision`) | as `CoworkerLearningPolicy` (memory_mode, eligible_source_kinds, eligible_conversation_classes, excluded_resource_labels, candidate_retention_days, inferred_confidence_threshold, review modes, notify_on_memory_commit, created_at) | Missing row reads as REVIEW_ALL; V1 rejects the inferred auto mode |
| MemoryRecordMetadata | `memory_id`; Workspace; Coworker | as `MemoryRecordMetadata` (context_document_id, source_candidate_id, memory_class, certainty, scope, source_digest, committed_at, expires_at, superseded_by, status, version) plus `normalized_key` | Unique ACTIVE (`coworker_id`, `normalized_key`, source lineage); text lives in a Coworker-scoped ContextDocument revision (Feature 009 COWORKER_NOTES) |
| LearningWorkItem | `work_item_id`; (`coworker_id`, `turn_id` or `feedback_request_id`) unique | state (QUEUED, CLAIMED, DONE, SKIPPED), attempts, claimed_at, skip_reason | Bounded queue; 1,000 per Workspace |
| SuggestionProducerCursor | `producer_id` | last_event_sequence, updated_at, version | Advanced after a processed batch |

## 3. Lifecycles

Suggestion (existing, RESPONSIBILITIES): `PROPOSED -> ACCEPTED | DISMISSED | EXPIRED`; snooze changes only `snoozed_until`.

LearningCandidate (LEARNING section 20, A-20/A-34):

```text
NEW -> CLASSIFIED -> AUTO_COMMIT_ELIGIBLE -> COMMITTED
                  -> NEEDS_REVIEW -> COMMITTED | DISMISSED | REJECTED
                  -> REJECTED
NEW | CLASSIFIED | NEEDS_REVIEW | AUTO_COMMIT_ELIGIBLE -> SUPERSEDED
```

Terminal: COMMITTED, DISMISSED, REJECTED, SUPERSEDED. A changed proposal is a new candidate.

MemoryRecordMetadata.status: `ACTIVE -> SUPERSEDED | REVOKED`; `REVOKED -> PURGE_PENDING` on purge (receipt retained without bytes).

LearningWorkItem: `QUEUED -> CLAIMED -> DONE | SKIPPED`; a CLAIMED item older than its lease returns to QUEUED on restart.

## 4. Events

Existing: `suggestion.proposed.v1` (first producer here), `suggestion.resolved.v1`, `suggestion.visibility.changed.v1`, `suggestion.preference.changed.v1`, `goal.created.v1`, `goal.revised.v1`, `goal.status.changed.v1`.

New (LEARNING section 19, promoted by T02): `learning.candidate.created.v1`, `learning.candidate.status.changed.v1`, `learning.memory.committed.v1`, `learning.memory.revoked.v1`, `coworker.learning_policy.revised.v1`. Payloads follow `learning_candidate_event_payload`: ids, digests, reason codes, bounded metadata; never candidate text.

## 5. Retention, deletion and privacy

Candidates are retained for `candidate_retention_days` (default 30) after a terminal state, then their payload Resources are purged and the row keeps ids, digests and reason codes. REJECTED candidates never store a body. A source message deleted under Feature 014 fences any non-terminal candidate from it (REJECTED). Revoked memory is excluded from context assembly immediately and purged on request with a receipt. Suppressed Suggestion candidates leave only a reason counter.

## 6. Migration

One forward-only migration (allocated v44 in docs/LITECOWORK-BUILD-TASKS.md; take the next free number if another package merges first). No backfill: existing Coworkers have no policy row and read as REVIEW_ALL; no existing Suggestion changes.
