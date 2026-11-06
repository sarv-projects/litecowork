[Reading 154 lines from start (total: 154 lines, 0 remaining)]

# 010 - Automations and Recurring Host

**ID:** 010
**Status:** DRAFT
**Owners:** Automation, Task Runtime, Runtime lifecycle, Desktop UI
**Updated:** 2026-11-06
**Revision:** 0.2
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
- An in-Core visual workflow graph, graph database, graph execution engine or generic code-node runtime.
- Importing external workflow node/step state as LiteCowork Task Step state.

## 4. Current evidence

The active source tree is present in apps/ and crates/. The behavior below is the normative feature target; source, served-route and UI evidence for this checkout is recorded in docs/IMPLEMENTATION-STATUS.md. Code or a route alone does not qualify the feature. A recursive 2026-11-06 scan of active apps/ and crates/ found no WorkflowBackendAdapter, WorkflowAuthoringAdapter, WorkflowGraph, edit_workflow, list_workflows or invokeWorkflow symbols. The WorkflowBackendAdapter described in CONNECTORS-WORKFLOWS.md is therefore a target contract, not an implemented production integration in this checkout. This package remains DRAFT until its named owners accept the decisions and supporting contracts.

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

The future external-workflow adapter is a provider boundary, not a trust shortcut. Provider workflow metadata, node descriptions, prompts, code, comments, schemas, sample data, test outputs and run errors are untrusted input. Provider MCP access is not one boolean: discover/read, draft/mutate, validate/test, publish/activate, production run and delete/disable/revoke are separate permissions. Provider credentials remain in provider custody and are never copied into Routine text, Task packets, public events or model context.

n8n instance-level MCP requires owner/admin enablement, per-workflow exposure and client scopes; Activepieces separates read-only discovery from per-project tool categories. The exact provider instance/version and permission configuration must be probed before integration. Provider Code nodes execute under the provider's own host, credentials, filesystem/network and runner isolation; they are not LiteCowork Environment processes, and Core Trust cannot claim to mediate their internal effects. See [SECURITY.md](../../docs/SECURITY.md) and [CONNECTORS-WORKFLOWS.md](../../docs/CONNECTORS-WORKFLOWS.md) §10.2. No production workflow adapter exists in the active source tree, so these are future qualification requirements only.

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

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 010-T01 through 010-T06. T05 is blocked on qualified Task/CapabilityInvocation/WorkflowBackendAdapter ownership; T06 is deferred future provider-authoring evaluation and is not V1 implementation authorization.

## 7a. Owner facts incorporated 2026-11-01

Read from AUTOMATION and ROUTINES.

- Only a terminal Task settles an occurrence. COMPLETED maps to COMPLETED, terminal FAILED maps to FAILED, and CANCELLED maps to SKIPPED with an explicit reason. INCOMPLETE, PAUSED, WAITING_USER, BLOCKED, NEEDS_USER, VERIFYING, READY, RUNNING, PAUSE_REQUESTED, and CANCEL_REQUESTED leave the occurrence STARTED and still occupying its concurrency slot.
- MisfirePolicy is SKIP, RUN_ONCE_WHEN_AVAILABLE, or CATCH_UP_BOUNDED. ambiguous_local_time is EARLIER or LATER. nonexistent_local_time is SKIP or NEXT_VALID. A timezone-rule update does not rewrite a claimed occurrence.
- Occurrence states move PENDING to CLAIMED to STARTED to COMPLETED or FAILED, with WAITING_DEPENDENCY and SKIPPED. CLAIMED returns to PENDING only when its lease expires before Task materialization. A stable delivery id reused with a different payload digest is OCCURRENCE_CONFLICT.
- Saving an Automation creates it PAUSED and admits no Task. POST /v1/automations/{id}/run is allowed while PAUSED and does not enable recurring hosting. DISABLED is terminal. Resume enables only a PAUSED Automation whose triggers are locally reconcilable Manual or Schedule triggers and does not fire them at enable time.
- POST /v1/routines/{id}/run requires Idempotency-Key and the exact routine_revision, rejects a non-null conversation_id until Conversation and Task can commit atomically, and accepts at most 64 top-level TEXT or RESOURCE_REF bindings. Success creates one READY Task and TaskSpecRevision 1 and no Plan, Step, session, Attempt, lease, Environment, Effect, or grant. Routine text cannot grant capabilities or weaken approvals.
- A SIGNATURE webhook is verified with constant-time HMAC over the raw bytes before parsing. Local default hosting is LOCAL_POLL. This slice still does not add a public webhook listener.
- Cite createAutomation, updateAutomation, pauseAutomation, resumeAutomation, runAutomationNow, settleAutomationTriggers, runRoutine. Events: automation.occurrence.created.v1, automation.occurrence.claimed.v1, automation.occurrence.settled.v1, automation.cursor.changed.v1, notification.delivery.changed.v1.



## 19. External WorkflowBackend integration and authoring boundary — 2026-11-06

This feature owns Automation revision, trigger hosting, occurrence identity, cursor, misfire/overlap, Task materialization and notification status. It does not own a general-purpose visual workflow graph or workflow execution runtime.

The canonical boundary is defined by [CONNECTORS-WORKFLOWS.md](../../docs/CONNECTORS-WORKFLOWS.md), [AUTOMATION.md](../../docs/AUTOMATION.md), [ROUTINES.md](../../docs/ROUTINES.md), [TASK-RUNTIME.md](../../docs/TASK-RUNTIME.md) and [V1-WORKFLOW-EVENT-LLD.md](../../docs/V1-WORKFLOW-EVENT-LLD.md):
- A Routine is a pinned reusable Task instruction/input contract.
- An Automation determines when/why a Routine is invoked and produces a canonical Occurrence/Task.
- The target external-workflow integration invokes a provider-owned deterministic workflow through a future qualified WorkflowBackendAdapter as one bounded CapabilityInvocation inside a Task; no production adapter is present in this checkout.
- The external engine owns its graph schema, internal nodes, graph revisions, execution engine and internal run/step state.
- LiteCowork owns Task/Attempt/CapabilityInvocation/Effect/Evidence, Trust, Workspace scope, user approvals and user-visible reconciliation.
- Provider node status is an observation. It does not create a LiteCowork Task Step or settle a Task by itself.

### V1 authoring modes

**Manual authoring:** the user creates or edits an existing flow in the connected workflow provider's own editor (for example, a user-owned n8n or Activepieces instance). The target LiteCowork experience is to discover the workflow, inspect its supported input/output metadata, validate user-provided input, pin the selected external workflow reference in a Routine, invoke it through the adapter and display provider run observations. This is not implemented in the current checkout: the adapter contract is target-only and no production WorkflowBackendAdapter was found in apps/ or crates/. LiteCowork does not embed or clone the provider canvas.

**Provider-native AI authoring:** n8n documents an Assistant that builds a normal n8n graph on the provider's canvas from a natural-language brief, then lets the owner inspect and edit the same graph manually; its MCP server also documents provider-side create/edit/validate/test operations. Activepieces documents a Build-by-Asking path and MCP tools such as `ap_build_flow`, `ap_add_step`, `ap_update_step`, validation and publish. These are authoring capabilities of the user's external provider, not capabilities of LiteCowork Core. The user may use them in the connected provider's own UI/MCP environment, subject to that provider's permissions and security boundary. LiteCowork currently has no production provider connection/WorkflowBackendAdapter path in the active source tree, so this is external-provider capability research rather than an available LiteCowork integration.

**LiteCowork agent assistance:** in V1, a LiteCowork Coworker/agent may help the user formulate a Routine, map values to a selected workflow's input schema, explain validation errors and summarize run receipts. It may not claim to create or modify an external graph. The WorkflowBackendAdapter described in CONNECTORS-WORKFLOWS.md is a target read/validate/invoke/observe/cancel contract; no corresponding production adapter symbols/routes were found in the active source tree. It has no qualified create/update/publish operation. An agent tool that only lists/reads/invokes is not an authoring API.

These paths are complementary: manual and provider-native AI graph authoring stay in the external provider; LiteCowork V1 owns the Routine/input mapping and will invoke only once a qualified adapter and Task/CapabilityInvocation/Trust path exist. Direct LiteCowork agent graph writes are future, provider-specific and separately gated—not V1.

### Future direct LiteCowork-Coworker provider-graph authoring extension

A future direct LiteCowork-agent capability may be evaluated through an optional provider-specific WorkflowAuthoringAdapter or tightly scoped mediated MCP Connection capability, with provider-owned create/draft/update/validate/publish/read-version operations. This is distinct from users using n8n Assistant or Activepieces Build by Asking/MCP directly inside those provider products, which does not require LiteCowork to author the graph. Direct LiteCowork-agent authoring must be an explicit provider capability, not an assumption shared by all WorkflowBackendAdapter implementations. Before enabling it, the owner must define:
- provider identity, API version, workflow reference and exact graph/version digest;
- draft-versus-published lifecycle, expected revision and stale-update conflict behavior;
- validation response and supported node/edge catalog;
- external effects, required scopes, execution identity and visibility;
- idempotency, timeout, ambiguous publish reconciliation and rollback/restore behavior;
- owner review/confirmation before a draft is published or a consequential run is started;
- audit receipts, retention, secret boundaries and per-provider compatibility tests.

A graph proposed by a LiteCowork Coworker must be created as a provider-side draft, schema-validated by that provider, shown to the owner as a provider-backed diff/preview, and explicitly confirmed before publish. LiteCowork must not create a second local graph database or execute the graph itself. This is a future direct-agent adapter proposal, not V1 acceptance and not an implementation authorization.

### Code-based nodes and execution

The inspected OpenBot graph schema has trigger, text, prompt, asset, LLM, agent, image, condition, approval, output, note and shape node kinds; it has no dedicated code-node kind. Its agent tool omits asset and image nodes from the kinds it may add. The reference therefore does not establish a JavaScript/Python workflow code-node feature.

n8n documents a Code node with JavaScript and Python support: https://docs.n8n.io/code/code-node/. Activepieces documents CODE steps accepting JavaScript/TypeScript source and package dependencies through its provider MCP tools: https://www.activepieces.com/docs/mcp/tools. Node-RED documents a JavaScript Function node over flow messages: https://nodered.org/docs/user-guide/writing-functions. These are external provider capabilities, not LiteCowork code-node types. They do not inherit LiteCowork's Environment/Trust guarantees. Code runs under the external engine's host configuration, credentials, filesystem/network policy and isolation. LiteCowork must disclose the provider execution boundary and required scopes; if effects cannot be declared/reconciled, the workflow remains unavailable for consequential production use. n8n also disclosed the condition-specific Python Task Runner advisory GHSA-9pq8-m8gp-4p53; where Python Task Runner is enabled, verify affected/patched version and configuration before allowing provider-side graph editing or execution.

### UI boundary

The V1 Automation screen remains a list/editor for Routine + trigger + owner + schedule/policy + history. It does not gain a permanent Workflow canvas section or an in-Core node palette.

Where a qualified external WorkflowBackend exists, a Routine may select an existing workflow and show:
- provider/Connection identity and exact workflow reference/version;
- supported input schema and validation errors;
- mapped Routine values and pinned source references;
- expected effect classes and Trust/approval requirements;
- test versus production invocation mode where the adapter supports both;
- external run identifier/progress observation and normalized result;
- a link to open the provider's own editor, when safe and supported.

A backend without a qualified listing/validation/invocation adapter is not selectable. A provider's internal graph, nodes and statuses are never represented as LiteCowork-owned Steps.

### OpenBot comparison

The source comparison and limitations are recorded in [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](../../docs/OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md). OpenBot demonstrates visual graph editing plus constrained agent tools that can save graph changes and animate the result. That is useful design evidence, but its graph runtime and direct agent editing are not promoted into LiteCowork Core. The reference README describes a development preview with danger-full-access/approvalPolicy-never behavior, and the repository declares PolyForm Noncommercial 1.0.0; this is not a security or code-reuse template.

## 19a. Current source and UI boundary — 2026-11-06

The following distinctions are source-verified and must remain explicit in downstream UI/feature ledgers:

- `AutomationTriggerHostWorker` is started by `OperatorServer::start` and polls local trigger cursors every 30 seconds. The `automation_operator.rs` module header is stale where it says recurring hosting and `/resume` remain unavailable. The Automation page's Pause success message is also stale where it says trigger execution is unavailable in this build.
- Definition pagination has a UI Load more action; occurrence history does not. `listOccurrences` returns a page with `next_cursor`, but the current page reads only the first page and discards the cursor.
- `retry_policy` and `wake_policy` are persisted editor fields with no production daemon consumer. The local TriggerHost does not run Task retry attempts or wake the machine. `max_concurrent_occurrences`, `overlap_policy` and schedule misfire decisions are consumed by the host. Placement/budget are materialized into Task inputs but do not imply a Task executor or cloud placement.
- Notification preferences store `preferred_channels`, but the outcome producer never reads that field. It reads Workspace event-class policy and quiet hours, then treats the first active ChannelBinding or any active Runtime Workspace binding as a local channel. Free-text channel names do not select a delivery route.
- `ON_CONDITION` means only a `Blocked` / `DEPENDENCY_BLOCKED` outcome in the domain evaluator; it is not a user-authored named condition. The Notifications page has no condition editor. It displays quiet hours but has no controls to create/change them; saves preserve the existing value or null.
- Occurrence/Task settlement inserts `PENDING` notification delivery rows. No production sender/consumer or status-transition writer was found for `SENDING`, `SENT`, `FAILED`, `AMBIGUOUS` or `SUPPRESSED`. The current Notifications Operator exposes reads and preference writes only; delivery history is not proof of transport delivery.
- Manual Run and Create test Task are separate: the former commits an occurrence and READY Task; the latter creates a READY Task without an occurrence/cursor. Neither starts planning or agent execution. The Routine and Automation pending-request recovery registries are process-local and are lost on app exit.
- External WorkflowBackendAdapter remains target-only; there is no provider selector/invocation UI or Core graph runtime.

## 20. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T03 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 010-T03 | AC-006 |

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]