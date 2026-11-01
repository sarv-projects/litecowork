# 010 - Automations and Recurring Host

**ID:** 010
**Status:** DRAFT
**Owners:** Automation, Task Runtime, Runtime lifecycle, Desktop UI
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Timers in the UI are not occurrences. Closing the window is not the same as the machine sleeping. Duplicate fires must not duplicate consequential Tasks.

Observable outcome: A saved schedule or event creates exactly one Task per occurrence, including across sleep and restart, with truthful history and notification status.

## 2. Goals

- Persist Automation revisions and occurrence identity.
- Let TriggerHost observe and Task admission execute.
- Cover timezone, DST, misfire, overlap, dedupe and offline.
- Record notification status separately from Task success.

## 3. Non-goals

- Inbound public webhooks as the default reachability story.
- Unattended Coworker autonomy beyond an explicit Routine.
- Cloud Runtime execution.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- TriggerHost observes; ordinary Task admission selects the execution Runtime (master plan P9, AUTOMATION.md).
- At-least-once input becomes exactly-once occurrence.
- Polling is the default local reachability, not a fallback excuse for open webhooks.
- Logical occurrence and idempotency are durable.
- Window close may leave Runtime up; sleep and shutdown are blockers (release contract §6.7).

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- First trigger type: cron in a named timezone versus a local file poll.

### Blockers

- B-26: first trigger type and misfire policy for GA.
- B-27: notification channel (local only in this slice).

## 6. High-level design

UI saves a Routine revision. TriggerHost commits an occurrence once. TaskService materializes one Task linked to that occurrence. A second observation with the same identity returns the original Task id. Notifications update a separate status row.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Follow AUTOMATION.md occurrence lifecycle, cursor durability and concurrency rules. Manual run and scheduled run share idempotency rules. Execution still uses feature 005 admission once that package exists; until then this spec's acceptance is the occurrence-to-Task identity contract.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Triggers cannot widen grants. Event payloads are untrusted. Secrets are not stored in the automation revision.

## 11. Errors and recovery

Restart replays the cursor and does not double-create. Overlap policy is explicit. Missed fire during sleep is recorded as misfire or skipped according to B-26, never as silent success.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Local notifications only. No public webhook listener in this slice.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN a scheduled occurrence is observed once, THE SYSTEM SHALL create exactly one Task linked to that occurrence id.
- **AC-002:** WHEN the same occurrence is observed again, THE SYSTEM SHALL return the original Task and SHALL NOT create another.
- **AC-003:** IF the machine was asleep across a fire time, THEN THE SYSTEM SHALL record misfire or skip according to the locked policy and SHALL NOT claim the run succeeded.
- **AC-004:** WHEN DST or timezone changes, THE SYSTEM SHALL compute the next fire from the stored timezone, not from a fixed UTC offset baked into the revision.
- **AC-005:** WHEN the desktop window closes and the Runtime remains up, THE SYSTEM SHALL still observe; WHEN the Runtime stops, THE SYSTEM SHALL not claim runs that did not start.
- **AC-006:** WHEN notification delivery is unknown, THE SYSTEM SHALL keep Task settlement and notification status distinct.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/010-automations-recurring-host
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not expose an unauthenticated inbound port.
- Do not run the Task body inside TriggerHost.

## 18. Assumptions and open questions

- Depends on Task creation from 005 for end-to-end execution; occurrence identity can be tested first.
- Legacy docs/superpowers plans are not this package.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 010-T01, 010-T02, 010-T03, 010-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from AUTOMATION and ROUTINES.

- Only a terminal Task settles an occurrence. COMPLETED maps to COMPLETED, terminal FAILED maps to FAILED, and CANCELLED maps to SKIPPED with an explicit reason. INCOMPLETE, PAUSED, WAITING_USER, BLOCKED, NEEDS_USER, VERIFYING, READY, RUNNING, PAUSE_REQUESTED, and CANCEL_REQUESTED leave the occurrence STARTED and still occupying its concurrency slot.
- MisfirePolicy is SKIP, RUN_ONCE_WHEN_AVAILABLE, or CATCH_UP_BOUNDED. ambiguous_local_time is EARLIER or LATER. nonexistent_local_time is SKIP or NEXT_VALID. A timezone-rule update does not rewrite a claimed occurrence.
- Occurrence states move PENDING to CLAIMED to STARTED to COMPLETED or FAILED, with WAITING_DEPENDENCY and SKIPPED. CLAIMED returns to PENDING only when its lease expires before Task materialization. A stable delivery id reused with a different payload digest is OCCURRENCE_CONFLICT.
- Saving an Automation creates it PAUSED and admits no Task. POST /v1/automations/{id}/run is allowed while PAUSED and does not enable recurring hosting. DISABLED is terminal. Resume enables only a PAUSED Automation whose triggers are locally reconcilable Manual or Schedule triggers and does not fire them at enable time.
- POST /v1/routines/{id}/run requires Idempotency-Key and the exact routine_revision, rejects a non-null conversation_id until Conversation and Task can commit atomically, and accepts at most 64 top-level TEXT or RESOURCE_REF bindings. Success creates one READY Task and TaskSpecRevision 1 and no Plan, Step, session, Attempt, lease, Environment, Effect, or grant. Routine text cannot grant capabilities or weaken approvals.
- A SIGNATURE webhook is verified with constant-time HMAC over the raw bytes before parsing. Local default hosting is LOCAL_POLL. This slice still does not add a public webhook listener.
- Cite createAutomation, updateAutomation, pauseAutomation, resumeAutomation, runAutomationNow, settleAutomationTriggers, runRoutine. Events: automation.occurrence.created.v1, automation.occurrence.claimed.v1, automation.occurrence.settled.v1, automation.cursor.changed.v1, notification.delivery.changed.v1.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T03 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T03 | AC-006 |
