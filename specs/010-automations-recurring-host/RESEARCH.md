# Research - Automations and Recurring Host

**Feature:** 010 | **Checked:** 2026-11-06

## Canonical authorities

- [AUTOMATION.md](../../docs/AUTOMATION.md) owns triggers, occurrences, cursor, overlap, misfire, settlement and notification behavior.
- [ROUTINES.md](../../docs/ROUTINES.md) owns reusable Routine inputs, revision pins and external workflow-backed Routine steps.
- [CONNECTORS-WORKFLOWS.md](../../docs/CONNECTORS-WORKFLOWS.md) defines the target WorkflowBackendAdapter contract, provider connections, workflow references, external invocation and provider effect boundaries; no production adapter implementation is present in the active source tree.
- [TASK-RUNTIME.md](../../docs/TASK-RUNTIME.md), [TRUST.md](../../docs/TRUST.md) and Feature 005 own Task/Attempt execution, grants, approvals, effects and verification.
- Historical plans under docs/superpowers/plans/ are not authoritative.

## OpenBot source comparison

- **Repository:** https://github.com/nightly-labs/openbot
- **Pinned revision:** 6759fa3ee857f4f4966865e12de99ae84ab6de07
- **Package version at pin:** 0.62.0
- **Declared license:** PolyForm Noncommercial 1.0.0. Exact files and LiteCowork mapping are in [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](../../docs/OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md).

Findings from source:
- OpenBot has a visual workflow canvas backed by a host-owned typed graph, node/edge/version/run/step records and immutable graph snapshots per run.
- Its agent tool bridge can list/read/edit/delete/run workflows and read run results. Edit uses constrained add/update/move/remove/connect/disconnect operations; the UI has a build animation for changes made by an agent.
- Its schema has trigger, text, prompt, asset, LLM, agent, image, condition, approval, output, note and shape kinds. There is no dedicated code node in the inspected schema.
- Its graph is not part of LiteCowork; it is an independent workflow runtime. The current README labels OpenBot a development preview and warns that agents use danger-full-access with approvalPolicy never. Do not adopt its security mode.
- The source comparison is a design reference only; its noncommercial license is not a code-reuse permission for LiteCowork.

## Decision for LiteCowork

Do not add OpenBot's graph database, visual node canvas or graph execution engine to LiteCowork Core V1. The existing accepted design says Automation is a scheduler/materializer and Routine is a pinned Task input/instruction contract. The target external-workflow path is one CapabilityInvocation inside a Task, but no production WorkflowBackendAdapter has been implemented in the active source tree.

Manual authoring remains in the connected external provider's own editor. The target LiteCowork experience is to discover and validate existing workflows, map Routine inputs, invoke under Trust, and observe the normalized run result once a qualified adapter exists. No production adapter is currently present; do not describe workflow selection/invocation as an available LiteCowork feature. V1 agent assistance may help draft the Routine, map input values and explain validation; it must not claim to create/edit a provider graph.

The target WorkflowBackendAdapter contract is read/validate/invoke/observe/cancel oriented and has no workflow create/update/publish operations. No production adapter is implemented in the current source. Future direct LiteCowork agent graph authoring requires a separate provider-specific authoring contract, a provider-side draft, exact version/digest, validation, effect summary, explicit owner approval before publish, idempotent publish and ambiguous-result reconciliation. It is not part of current V1 acceptance.

## Code-based workflow nodes

OpenBot's inspected graph schema has no dedicated code node. n8n's official documentation describes a Code node supporting JavaScript and Python: https://docs.n8n.io/code/code-node/. This is a capability of the external n8n environment, not a LiteCowork node. It remains subject to the connected provider's own execution isolation, credentials and filesystem/network permissions. LiteCowork must not represent it as running inside a LiteCowork Environment or imply that Core Trust automatically mediates internal provider node execution.

## Alternatives

- Build a native n8n-style graph engine inside LiteCowork. **Rejected for V1** because it conflicts with PRODUCT-STRATEGY.md and CONNECTORS-WORKFLOWS.md, duplicates Task/Attempt/Effect execution ownership and materially expands Core scope.
- Make workflow integration mandatory. **Rejected**; a user can use Routines/Automations without connecting a workflow backend.
- Allow any provider workflow/code node to execute with Task authority implicitly. **Rejected**; adapter scope, provider identity, effects, approval and reconciliation are explicit.
- Allow an agent to create/publish workflows through tools without a provider-specific authoring API. **Rejected**.

## Limitations

This was a source/document inspection at the pinned revision. OpenBot was not cloned, built or run, and no live provider/account, quota, mobile, remote or computer-use flow was exercised. The current adapter contract and LiteCowork owner docs were inspected; no workflow provider authoring API was qualified. No runtime qualification is claimed.

## Current source and UI audit — 2026-11-06

- `AutomationTriggerHostWorker` is real: `OperatorServer::start` starts it before the IPC listener reports ready, owns it for the server lifetime and joins it on shutdown. It polls persisted local trigger cursors every 30 seconds. This contradicts the `automation_operator.rs` module header, which still says recurring hosting and `/resume` are unavailable; both are stale source comments, not current behavior.
- The Automations page has a stale success message after Pause: “trigger execution is not available in this build.” The local schedule host is in fact running. The message misrepresents the operation; pause changes persisted status and future due-schedule admission is filtered by the host's current schedule query. The separate Coworker-pause fence defect is documented in Feature 009.
- Automation occurrence history is only loaded once with `listOccurrences(selectedId)`; the API returns `next_cursor`, but the page stores only `page.items` and renders no Load more action. History beyond the first API page is not reachable from this surface. Automation definitions themselves do have cursor pagination and Load more.
- The editor persists retry policy and wake policy, but a source scan of the daemon shows no production consumer of either field. The trigger host uses concurrency/overlap and schedule/misfire rules; it does not execute Task retries or wake the Runtime. Placement/budget are carried into the READY Task contract, not proof of an executor or cloud placement.
- Workspace notification preferences are persisted, but `preferred_channels` is not read by the outcome producer. The producer reads only policy and quiet-hours, then chooses the first active ChannelBinding or treats any active Runtime Workspace binding as a local channel. The free-text channel list shown in Notifications is therefore descriptive stored data, not routing.
- `ON_CONDITION` is not a named-condition evaluator: the domain maps it specifically to `NotificationOutcome::Blocked` / `DEPENDENCY_BLOCKED`. Notifications UI copy says “When a named condition holds”, but no condition editor or expression is stored. Quiet hours are displayed but not editable in the current UI; `savePreference` resubmits the stored quiet-hours object unchanged or `null`.
- The notification outcome producer inserts `PENDING` delivery rows transactionally with occurrence/Task outcome settlement. The source tree has no production delivery worker/consumer or status-transition writer for `SENDING`, `SENT`, `FAILED`, `AMBIGUOUS` or `SUPPRESSED`; `notification_operator.rs` only reads delivery history and writes preferences. The Notifications page is read-only for delivery and correctly says it never sends one. A `PENDING` row is not a delivered notification.
- The current Automation page exposes templates, definition/revision, Manual Run, Create test Task, pause/resume/disable, health and first-page Runs. Manual Run creates an occurrence plus READY Task; Test Task creates a READY Task without an occurrence/cursor. Neither starts planning/agent execution. Routines separately expose immutable definition/revision, JSON advanced fields, archive, exact pinned inputs, Run now, and process-local ambiguous-request retry/discard. Pending Routine/Automation request envelopes are lost when the app process closes.
- External WorkflowBackendAdapter remains target-only; no provider workflow selector, mapping, invocation, graph canvas or graph mutation UI is mounted.

## Unresolved questions

- B-26: first trigger type and misfire policy for GA.
- B-27: local notification channel and delivery guarantee.
- Future, not V1 blocker: whether a particular external backend should receive a qualified WorkflowAuthoringAdapter pilot. Any such decision requires a separate ADR and provider/security review.

## External provider hybrid authoring research — checked 2026-11-06

Manual graph editing and AI-assisted graph authoring coexist in established external workflow platforms. This means the user need not choose between a visual editor and an AI builder, but it does **not** mean LiteCowork should own another workflow engine.

### n8n
- n8n Assistant announcement (2026-09-09): https://blog.n8n.io/introducing-n8n-assistant/
  - A natural-language description is planned and built on the n8n canvas; the assistant asks for credentials, runs/debugs/iterates, and leaves a normal editable n8n workflow.
  - The user can refine that same graph manually. The provider states credential access and activation require explicit confirmation.
- Official n8n MCP: https://n8n.io/mcp/
  - An external MCP client can create/edit workflows, validate, test, inspect logs and iterate within the connected n8n instance.
- Code node: https://docs.n8n.io/code/code-node/
  - Supports JavaScript and Python. Self-hosted and Cloud module behavior differs.
- Security audit: https://docs.n8n.io/hosting/securing/security-audit/
- GHSA-9pq8-m8gp-4p53 (published 2026-05-27): https://github.com/n8n-io/n8n/security/advisories/GHSA-9pq8-m8gp-4p53
  - Condition-specific Python Task Runner sandbox escape for a workflow with a Python Code Node when Python Task Runner is enabled. Published fixed versions: 1.123.48, 2.21.8, 2.22.4. Do not generalize this to all Code Nodes; verify exact instance version and runner configuration.

### Activepieces
- Workflow builder: https://www.activepieces.com/product/workflow-builder — visual builder, "Build by Asking", inline TypeScript steps, per-step testing and versioned draft/publish.
- Agents: https://www.activepieces.com/docs/agents — distinguishes deterministic Flow logic from agent-selected actions; agents can operate within a Flow and can be created from a natural-language brief.
- MCP tool catalog: https://www.activepieces.com/docs/mcp/tools
  - `ap_build_flow` builds a trigger and sequential steps with validation.
  - `ap_add_step` / `ap_update_step` configure PIECE, CODE, LOOP_ON_ITEMS and ROUTER steps.
  - `ap_validate_step_config` / `ap_validate_flow` validate; `ap_read_step_code` reads the full code/dependencies/mappings; `ap_lock_and_publish` publishes the draft.
  - CODE steps accept JavaScript/TypeScript source and package dependency declarations.
- Execution architecture: https://www.activepieces.com/docs/install/architecture/overview — App/Worker/queue/engine/sandbox with resource limits. This is the provider's own boundary, not a LiteCowork Environment.

### Node-RED
- Function node: https://nodered.org/docs/user-guide/writing-functions — JavaScript over flow messages alongside declarative Change/Switch/Template nodes.

### LiteCowork decision
1. **Manual:** user authors/refines graphs in the connected provider's own editor.
2. **Provider-native AI:** if enabled by that provider, user can describe a graph to its Assistant/MCP feature; the provider creates a normal editable provider-owned graph, which the user inspects and publishes there.
3. **LiteCowork Routine assistance:** Coworker can help define the Routine objective and map values to an existing provider workflow input schema. It cannot create/update/publish the graph in the current design.
4. **Future direct LiteCowork authoring:** a selected Coworker could call provider-specific graph tools only after a qualified authoring capability, narrow tool scopes, exact provider/version/digest, draft/validate/effect summary, owner confirmation, audit/idempotency and ambiguous-publish recovery are accepted by owners.
5. Provider Code nodes execute under provider-owned credentials, filesystem/network, host and isolation policy. They are not LiteCowork Environment processes, and LiteCowork Trust does not automatically mediate internal provider steps.

### Implementation boundary verification
A recursive symbol scan of active `apps/` and `crates/` source found no `WorkflowBackendAdapter`, `WorkflowAuthoringAdapter`, `WorkflowGraph`, `edit_workflow`, `list_workflows` or `invokeWorkflow` symbols (2026-11-06). The WorkflowBackendAdapter in CONNECTORS-WORKFLOWS.md is a target contract, not an implemented production integration in this checkout. No external provider was connected or exercised.

The full source pin, OpenBot workflow graph comparison, provider links and license/security limitations are in [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](../../docs/OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md).
