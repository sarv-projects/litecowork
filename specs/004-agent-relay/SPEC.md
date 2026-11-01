# 004 - Agent Relay on One Conversation

**ID:** 004
**Status:** DRAFT
**Owners:** Conversation, Agent Runtime, Product
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

A second native agent is useless if switching agents either shares hidden harness state or drops committed LiteCowork history. Relay is not Task delegation and not a universal router.

Observable outcome: On one Conversation the owner runs a committed turn on agent A, a later turn on agent B that sees only portable context, then returns to A without copying private transcripts or credentials.

## 2. Goals

- Qualify a second AgentModule using feature 002 rules.
- Execute A then B then A as separate turns on one Conversation.
- Build each turn's native input from committed messages, selected Resources and durable references only.
- Show the exact producer agent on every turn.

## 3. Non-goals

- Transfer private native transcripts, hidden reasoning or credentials between harnesses.
- Create a Task or child Attempt merely because the agent changed.
- Silently substitute a model when the selected binding is stale.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Conversation and Task stay separate.
- Native handles never enter public projections.
- HOST_DELEGATED work belongs to Task Runtime (DELEGATION.md), not to ordinary relay.
- Each turn uses the binding selected for that turn.

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- Portable context budget and which resource kinds the second adapter accepts.

### Blockers

- B-15: portable context schema and size bound.
- B-16: second AgentModule identity, depends on 002.

## 6. High-level design

UI selects a different enabled binding between turns. Operator admits the next turn only through the feature 001 supervisor with a new session for the newly selected binding. Context materializer exports a portable package and never a native handle.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Turn N+1 input is a pure function of committed Conversation messages, explicit resource refs and artifact refs. Each admitted Conversation turn uses a fresh Conversation-scoped AgentSession. Returning to A does not replay A's prior AgentSession or native handle. Host reuse is a host-supervisor choice and is not reuse of the prior AgentSession. No DelegationProfile is created.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Export filter strips secret refs, native handles and harness-private transcripts. Each binding is revalidated. Cross-workspace refs are rejected.

## 11. Errors and recovery

If B is unavailable, turn admission fails and A's history remains. Partial stream on B settles or fails through 001 rules before another relay. Restart reloads committed turns only.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Requires two qualified modules from 002. Relay flag is per turn selection, not a workspace mode that forces delegation.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner switches from agent A to agent B on the same Conversation, THE SYSTEM SHALL admit B only with a fresh binding revalidation.
- **AC-002:** WHEN B's turn is dispatched, THE SYSTEM SHALL supply portable context from committed messages and selected refs and SHALL NOT supply A's native handle or private transcript.
- **AC-003:** WHEN the owner returns to A, THE SYSTEM SHALL preserve B's committed messages in semantic history.
- **AC-004:** IF B's binding is stale, THEN THE SYSTEM SHALL fail admission and keep the draft without calling A as a fallback.
- **AC-005:** WHEN history reloads, THE SYSTEM SHALL show the producer binding id for each committed turn.
- **AC-006:** IF the user did not create a Task, THEN THE SYSTEM SHALL NOT create an Attempt, lease or grant because the agent changed.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/004-agent-relay
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not implement HOST_DELEGATED Task workers here.
- Do not merge native sessions across harnesses.

## 18. Assumptions and open questions

- Depends on 001 turn settlement and 002 second module.
- Default resume policy is new session until an owner qualifies harness resume.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 004-T01, 004-T02, 004-T03, 004-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from DELEGATION, STATE-MACHINES, and CONTEXT.

- Ordinary relay is not NATIVE_INTERNAL, HOST_DELEGATED, or CAPABILITY_EXECUTOR. It creates no child Attempt, DelegationProfile, Environment, lease, grant, ResultEnvelope, or capability invocation.
- Relay does not rewrite native instruction files, hooks, skills, plugins, MCP settings, permissions, subagents, or memory.
- enabled=false denies a new Conversation session. lead_eligible=false cannot be the Conversation lead. A disabled binding leaves an already admitted session to ordinary settlement.
- A stale session option returns AGENT_SESSION_OVERRIDE_UNSUPPORTED or AGENT_NATIVE_CONFIG_CHANGED. Quota exhaustion returns AGENT_QUOTA_EXHAUSTED. Every other unavailable binding returns AGENT_UNAVAILABLE with no substitute.
- Switching the Conversation binding does not emit task.lead_agent.changed.v1 and does not build a LeadHandoffPacket.
- Cite setConversationAgentBinding, submitConversationTurn, retryConversationTurn. Events: conversation.agent_binding.changed.v1, conversation.turn.created.v2, agent.session.starting.v1, agent.session.started.v1.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 004-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 004-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 004-T03 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 004-T03 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 004-T04 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 004-T03 | AC-006 |
