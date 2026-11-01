# 005 - Safe Task Path

**ID:** 005
**Status:** DRAFT
**Owners:** Task Runtime, Environments, Trust, Storage, Desktop UI
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Ordinary chat must not become a Task by inference. A working directory is not a sandbox. One consequential edit needs plan acceptance, isolation attestation, lease, grant and independent verification.

Observable outcome: The owner promotes one Conversation outcome into a Task, accepts an immutable plan, and one isolated Attempt edits one approved file with a receipt that survives cancel and restart.

## 2. Goals

- Create a Task explicitly from a committed Conversation outcome or a standalone objective.
- Accept one immutable PlanRevision and Steps atomically before any Attempt.
- Run one Environment provider that attests read/write roots, network, process tree and limits.
- Reconcile an ambiguous write before retry and verify the edit independently.

## 3. Non-goals

- Multi-step heterogeneous delegation beyond one edit.
- Coworker-required Tasks.
- Treating git worktree alone as isolation.
- Treating omission of LeadFailoverPolicy as DISABLED when a Coworker revision already supplies ASK or ALLOW_LISTED.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Task creation does not imply Step, Attempt, lease or Environment (TASK-RUNTIME).
- No persisted Task DRAFT; unsent composer stays in the Operator.
- Unavailable lead fails with AGENT_UNAVAILABLE and does not substitute another binding.
- Unknown external effect is reconciled before retry (release contract §6).
- Grants are explicit and scoped.

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- Which Environment provider is first (container, OS sandbox, or other) on the GA OS.

### Blockers

- B-17: first Environment provider and attestation evidence.
- B-18: the single allowed edit class (one file replace versus patch).

## 6. High-level design

UI promotes work with an explicit command. TaskService commits Task and TaskSpecRevision. Planning uses a native lead session. Acceptance commits PlanRevision. Attempt admission is a separate transaction that never contains the filesystem call. Effect reconciler and verifier run after the external write returns.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

CreateTaskRequest fields follow TASK-RUNTIME. Plan acceptance is atomic. Attempt, lease, grant, invocation intent and idempotency receipt commit together before the Environment is invoked. The write happens outside SQLite. Reconciliation updates Effect and Evidence after the external result is known.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Workspace and owner checks on every command. Environment cannot widen roots. Network default deny unless the plan's grant allows it. Secret material is not placed in the TaskPacket.

## 11. Errors and recovery

Cancel fences the lease. Timeout-after-dispatch reconciles. Restart reloads Task state and does not reissue a write whose receipt is ambiguous. Stale plan version conflicts.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

One Environment provider. One file edit. LeadFailoverPolicy is pinned from an explicit Task request, otherwise the selected Coworker revision, otherwise DISABLED with empty triggers, empty fallbacks, and max_lead_changes 0.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner sends an ordinary Conversation message, THE SYSTEM SHALL NOT create a Task.
- **AC-002:** WHEN the owner explicitly promotes an outcome, THE SYSTEM SHALL commit Task and TaskSpecRevision without an Attempt.
- **AC-003:** IF the preferred lead binding is unavailable, THEN THE SYSTEM SHALL return AGENT_UNAVAILABLE and create no Task.
- **AC-004:** WHEN the owner accepts a plan, THE SYSTEM SHALL commit an immutable PlanRevision and Steps before any Effect.
- **AC-005:** WHEN the Attempt runs, THE SYSTEM SHALL invoke the Environment only after lease, grant and idempotency receipt are committed, and the external call SHALL stay outside that transaction.
- **AC-006:** IF the external write result is unknown, THEN THE SYSTEM SHALL reconcile before any retry.
- **AC-007:** WHEN the Runtime restarts after cancel or timeout, THE SYSTEM SHALL show a durable receipt and SHALL NOT report success without independent verification.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/005-safe-task-path
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not run browser automation.
- Do not install connectors.
- Do not enable standing automations.

## 18. Assumptions and open questions

- First Environment provider is undecided.
- Verification may be a hash compare of the approved file for this slice.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 005-T01, 005-T02, 005-T03, 005-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from TASK-RUNTIME, ENVIRONMENTS, DELEGATION, and STATE-MACHINES.

- If the lead binding is unavailable, creation returns AGENT_UNAVAILABLE, creates no Task or turn, does not fall through to a lower-precedence binding, and leaves the unsent composer draft.
- Until a current-incarnation IsolationAttestation is revalidated, planner admission returns DEPENDENCY_UNAVAILABLE and creates no session, PlanRevision, Step, Attempt, or Environment.
- Plan acceptance uses If-Match and the current TaskSpec head. A Task-version mismatch returns 409 STALE_TASK_VERSION. A spec mismatch returns 409 STALE_SPEC_REVISION. A producer who is no longer the active TASK_PLANNING session or leased lead is 403 FORBIDDEN or 409 CONFLICT.
- The Runtime attaches an Environment before the transaction that commits Attempt, ExecutionLease, grant, and receipt, starts the AgentSession only after that commit, and runs the file mutation outside SQLite.
- Admission accepts only a Runtime-held IsolationAttestation whose filesystem, process-tree, network, and resource-limit fields are ENFORCED. UNKNOWN, NOT_ENFORCED, or a caller-supplied attestation fails closed.
- Releasing a writer lease or destroying the Environment requires QuiescenceObservation QUIESCENT. UNKNOWN, timeout, or restart leaves the Attempt fenced.
- Cancel moves the Task to CANCEL_REQUESTED, reconciles STARTED, ACKNOWLEDGED, and AMBIGUOUS Effects, and commits CANCELLED only after attempts, invocations, verification, and Effects have settled.
- There is no Task retry command. Continuation is recover_step, which creates a new Attempt id. A changed placement plan returns STALE_VERSION.
- A new Runtime incarnation cannot renew the prior lease. Recovery commits that lease EXPIRED or REVOKED and the Attempt ABANDONED. A stale fence returns STALE_FENCE.
- The provider exposes the pinned revision read-only, writes through a revalidated handle and atomic replace, and returns INTEGRITY_FAILURE when the observed digest differs.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T02 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 005-T03 | AC-007 |
