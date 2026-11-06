[Reading 81 lines from start (total: 81 lines, 0 remaining)]

[Reading 53 lines from start (total: 53 lines, 0 remaining)]

# Tasks - Automations and Recurring Host

**Feature:** 010 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.2

## 010-T01 - Lock trigger type and misfire policy

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Automation
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-26 and B-27.
- **Acceptance:** Trigger and notification rules written. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 010-T02 - Durable occurrence idempotency

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Automation and Storage
- **Allowed paths:** trigger host
- **Contract:** AUTOMATION.md
- **Change intent:** Exactly-one occurrence and Task link.
- **Acceptance:** AC-001, AC-002, AC-004. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p storage-sqlite --locked automation_occurrence`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 010-T03 - Sleep restart and notification status

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Runtime and Automation
- **Allowed paths:** lifecycle and notifications
- **Contract:** RUNTIME-LIFECYCLE and AUTOMATION.md
- **Change intent:** Misfire and distinct notification status.
- **Acceptance:** AC-003, AC-005, AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked automation_recovery`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 010-T04 - Automation history UI

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** automations
- **Contract:** ui-behavior
- **Change intent:** History shows occurrence, Task and notification separately.
- **Acceptance:** UI matches settlement. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 010-T05 - Qualified external WorkflowBackend Routine mapping

- **Status:** BLOCKED / DEPENDS ON FEATURES 005 AND 006
- **Depends on:** T01; qualified Task admission; qualified CapabilityInvocation and WorkflowBackendAdapter
- **Owner:** Automation, Capability Fabric, Task Runtime, Desktop UI
- **Allowed paths:** Routine-to-workflow reference mapping, adapter integration, invocation projection and tests; provider adapter remains owned by CONNECTORS-WORKFLOWS
- **Contract:** SPEC §19, ROUTINES.md, CONNECTORS-WORKFLOWS.md, TASK-RUNTIME.md, Feature 005 and Feature 006
- **Change intent:** Allow a Routine to pin a discovered external workflow reference, validate mapped inputs and invoke it as one bounded CapabilityInvocation inside the admitted Task.
- **Acceptance:** provider/Connection identity and workflow reference are pinned; input validation occurs before invocation; external graph state remains provider-owned; provider run IDs remain scoped/private; provider step observations do not create LiteCowork Steps; timeout/unknown result reconciles before retry; Trust and Effect policy cover declared external effect classes.
- **Command:** owner-specific adapter and Task/Capability integration tests after those owners are ACCEPTED/LOCKED.
- **Stop:** do not create a Core WorkflowGraph schema, canvas, local graph revision/run store or second Task/Attempt state machine.

## 010-T06 - Direct LiteCowork Coworker provider-graph authoring evaluation

- **Status:** DEFERRED / NOT V1 / NO IMPLEMENTATION AUTHORIZATION
- **Depends on:** T05; provider-specific workflow authoring API; explicit owner ADR
- **Owner:** Product, Capability Fabric, Trust, Automation
- **Allowed paths:** research and provider adapter proposal only until owner promotion
- **Contract:** SPEC §19, CONNECTORS-WORKFLOWS.md, TRUST.md
- **Change intent:** Evaluate whether LiteCowork may expose a selected provider's graph-mutation MCP/API tools to a LiteCowork Coworker with narrower authority than normal workflow invocation. This task is not about using the provider's own Assistant/MCP directly in that provider; users may already do that outside LiteCoworker.
- **Acceptance:** no acceptance for V1; any future proposal must include provider revision/digest, schema validation, draft preview/diff, separate read/draft/mutate/test/publish/run/delete scopes, explicit owner publish confirmation, scope/effect summary, idempotency, ambiguous publish reconciliation, audit receipt and provider-specific security tests.
- **Command:** none until the owner ADR and machine/API contracts are promoted.
- **Stop:** no generic agent tool that directly writes or publishes arbitrary external workflows; no new in-Core graph data model or arbitrary code execution node.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]