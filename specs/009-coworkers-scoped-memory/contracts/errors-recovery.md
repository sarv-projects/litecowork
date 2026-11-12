# Errors and recovery - Coworkers and Scoped Memory

**Feature:** 009 | **Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 amendment IN_REVIEW. A-91 gates manual-memory schema/persistence. | **Source cross-check:** 2026-11-12 at source checkpoint 6adadab539a62ca1c8b70621c7f88e1785cfe8c2

## Currently served Coworker configuration operations

- Workspace mismatch and malformed identity/body fail closed.
- Stale Coworker/Workspace version or conflicting idempotency replay returns a conflict; reload the current head before preparing a new command. An exact retry reuses the original idempotency key and request payload.
- Target: PAUSED must fence new proactive/scheduled admission while an explicit owner Task may name a PAUSED Coworker when Task admission permits it. Current code does not fully enforce that target: already-enabled Automations are not rechecked by TriggerHost at occurrence settlement, while the Home composer blocks owner Tasks that the target allows. See register A-22; do not report PAUSED as a complete admission fence.
- ARCHIVED Coworkers cannot become new Task origin; archive is rejected while the owning aggregate reports blocking active work.
- GET `/v1/coworkers/{id}/presence` currently returns 503 `DEPENDENCY_UNAVAILABLE` after confirming the Coworker exists. The UI must show live availability unavailable; retrying the same GET cannot create the missing Task/UserRequest/Attempt/Runtime projections.

## ContextDocument and retrieval boundary

- Current Resource ContextDocument creation accepts only USER/PERSONAL_PROFILE and WORKSPACE/WORKSPACE_NOTES. The canonical OpenAPI create enum matches that validator; record/read types and Coworker Settings still list COWORKER_NOTES and GOAL_NOTES. Treat those unavailable create kinds as unsupported, not as a transient provider outage.
- Current retrieval is Workspace/explicit-Resource scoped and has no Coworker ID/revision. It cannot prove cross-Coworker isolation because a Coworker-owned context path is not yet implemented.
- For supported ContextDocuments, REVOKED, DELETION_PENDING and DELETED block new content reads. A read admitted while ACTIVE may finish after a concurrent revoke; revocation is not a barrier for bytes already delivered to an external AgentSession.
- Current local deletion fails closed before tombstoning if exact revisions use external/provider-backed locations. Accepted local purge is a recoverable saga; keep DELETION_PENDING until every sealed local target is acknowledged. Do not fabricate DELETED after timeout or process restart.

## Learning boundary

- No current LearningCandidate observation/classification/commit API exists. Do not retry against a guessed endpoint or create a manual candidate record when the learning backend is unavailable.
- Deterministic `storage-core` gate functions are not persistence. A process restart cannot restore a candidate or memory record that was never committed.
- The production SkillProposal redactor call is limited to secret-shaped scanning of a proposed procedure body. It does not establish a LearningCandidate state transition or automatic memory capability.
- Any future timeout after a candidate or Resource effect must reconcile against the original candidate/request/Resource revision identity before retry. Never generate a fresh idempotency key to escape an unknown outcome.

## Restart and scope fencing

Restart reloads Coworker head/revision, Workspace primary, Task origin pins, Resource ContextDocument status and purge receipts from their existing owners. It must not synthesize Coworker Conversation history, Coworker memory, learning candidates or Coworker-scoped retrieval receipts that are not persisted.

Future Coworker-owned Conversation and ContextDocument services must reauthorize Workspace and Coworker identity on every read, compare immutable owner/revision pins, and fence new context assembly after revoke/purge. A stale Coworker revision cannot silently rebind a Conversation or Task to a new Coworker.
