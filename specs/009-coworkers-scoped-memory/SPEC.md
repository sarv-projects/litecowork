# 009 - Coworkers and Scoped Memory

**ID:** 009
**Status:** DRAFT
**Owners:** Coworkers, Conversation, Memory, Trust, Desktop UI
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Coworkers are optional and must not be required for ordinary chat. Memory must not silently grant permissions, Connections, packages or schedules. Automatic learning waits on consent and deletion contracts.

Observable outcome: The owner optionally creates a Coworker, chats, attaches and revokes scoped context, restarts, and proves another Coworker or Workspace scope cannot read it.

## 2. Goals

- Quick Create to a first usable Coworker chat.
- Scope Conversation ownership and ContextDocuments to that Coworker.
- Support inspect, correct, revoke and purge of owner-authored memory.
- Prove restart and cross-scope denial.

## 3. Non-goals

- Mandatory Coworker on first run.
- Automatic learning in this slice.
- Coworker-to-Coworker authority sharing.
- Channels and remote control.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Ordinary Conversation does not require a Coworker (ARCHITECTURE, release contract).
- A memory change cannot grant permissions, Connections, packages or schedules (release contract §6.4).
- Goals and Suggestions do not grant authority (COWORKERS-TARGET).
- ADR-0024 remains the decision record for optional chat-first Coworkers.
- Paused versus archived Coworker rules in TASK-RUNTIME apply when a Task names a Coworker.

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- Which memory record is in the minimal slice: owner-authored ContextDocument only, with embeddings deferred.

### Blockers

- B-24: minimal memory record and purge semantics.
- B-25: whether Quick Create pins a default lead binding or starts unbound until 002.

## 6. High-level design

Coworker service commits identity and revision. Chat uses the Conversation model with an explicit coworker origin. Retrieval filters by coworker and workspace scope. Purge fences future assembly but keeps audit of the deletion event without the deleted bytes.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Records proposed in COWORKERS-TARGET stay PROPOSED until promoted. This package uses only owner-authored context documents and Coworker revision pins already described in TASK-RUNTIME and CONTEXT.md. No new grant path.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Scope checks on every read. Cross-coworker ids fail. Purge is authorized by the owner. Memory text is not a Trust policy document.

## 11. Errors and recovery

Restart reloads Coworker head and does not restore purged bytes. Stale expected_coworker_version conflicts. PAUSED blocks proactive and scheduled admission, and an explicit owner Task may still name that Coworker. ARCHIVED cannot be a new Task origin at all.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

One Coworker, owner-authored memory, no learning pipeline, no multi-coworker handoff.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner uses New Chat without a Coworker, THE SYSTEM SHALL create an ordinary Conversation.
- **AC-002:** WHEN the owner Quick Creates a Coworker, THE SYSTEM SHALL open a chat scoped to that Coworker id.
- **AC-003:** WHEN context is attached to a Coworker, THE SYSTEM SHALL exclude it from other Coworkers and from Coworker-less retrieval.
- **AC-004:** WHEN the owner revokes or purges memory, THE SYSTEM SHALL fence future prompts and SHALL NOT grant any capability.
- **AC-005:** IF a Task names a stale Coworker version, THEN THE SYSTEM SHALL conflict and SHALL NOT switch Coworker implicitly.
- **AC-006:** WHEN the app restarts, THE SYSTEM SHALL restore the Coworker chat without cross-scope leakage.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/009-coworkers-scoped-memory
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not enable schedules from memory text.
- Do not auto-learn from chat in this slice.

## 18. Assumptions and open questions

- Promotion of proposed Coworker tables needs owner schema review before implementation.
- Depends on Conversation send from 001 for a usable chat.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 009-T01, 009-T02, 009-T03, 009-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from RESPONSIBILITIES, COWORKERS-TARGET, and LEARNING boundaries those docs already lock.

- Coworker.status is ACTIVE, PAUSED, or ARCHIVED. Archive is allowed only when no Coworker-owned Automation is active and every Task is terminal. Archive does not delete Tasks, Artifacts, ContextDocuments, or external data.
- A Coworker-origin Task stores immutable origin_coworker_id and origin_coworker_revision. Later Coworker edits do not mutate that Task.
- Presence projection keeps proactive_status, activity_status, and runtime_status separate on GET /v1/coworkers/{id}/presence. That projection is not an admission input and is not synthetic typing.
- CoworkerInteractionPolicy is STANDARD_TRUST_POLICY, REQUIRE_OWNER_APPROVAL, or HANDOFF_TO_OWNER. No setting weakens Workspace or provider requirements or creates a Grant.
- Goal.status is ACTIVE, PAUSED, COMPLETED, or ARCHIVED. Only an owner command sets COMPLETED, and that does not change linked Tasks or disable linked Routines. Mutating an archived Goal returns GOAL_ARCHIVED. GoalProgressProjection COMPLETE means readers were readable, not that criteria passed.
- Suggestion.status is PROPOSED, ACCEPTED, DISMISSED, or EXPIRED. Accepting a TASK suggestion commits one READY Task and does not create a Plan or start an AgentSession. A suggestion does not install, schedule, authorize, or send.
- owner_coworker_id is immutable. Quick Create uses two idempotency keys. A failed first chat leaves the Coworker created. Add Memory writes a user-authored ContextDocument and does not forge a LearningCandidate.
- Cite createCoworker, reviseCoworker, changeCoworkerStatus, getCoworkerRevision, setPrimaryCoworker, setContextDocumentStatus, requestContextDocumentDeletion.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 009-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 009-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 009-T02 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 009-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 009-T02 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 009-T03 | AC-006 |
