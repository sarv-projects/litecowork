# OpenBot Reference Implementation Study

**Reference revision:** nightly-labs/openbot@6759fa3ee857f4f4966865e12de99ae84ab6de07  
**Checked:** 2026-11-06  
**Observed package version:** 0.62.0  
**License:** PolyForm Noncommercial 1.0.0 (package.json)  
**Status:** architecture and source comparison only; no code copied, no runtime build or security test performed.

This study records what the inspected OpenBot source actually implements, what is merely documented, and which ideas fit LiteCowork's existing owner contracts. It does not promote a new LiteCowork runtime, workflow engine, API, or release commitment.

## 1. Executive decision

OpenBot is a useful reference for a compact local-first agent desktop. Its strongest transferable ideas are:

- provider-specific model and reasoning controls scoped to an agent profile;
- a calm Tasks page with one composer, a narrow agent selector, grouped attention states and task detail beside the list;
- durable local usage records with provider-specific counter semantics and a separate provider-quota observation path;
- a visual workflow graph that can also be edited through constrained agent tools;
- an agent-change visualization that makes graph edits visible in the canvas;
- a persistent local agent/daemon architecture and separate activity, task and usage surfaces.

**LiteCowork does not adopt OpenBot's internal WorkflowGraph engine for V1.** LiteCowork's accepted product strategy explicitly rejects becoming a generic visual workflow-graph engine. The canonical Core model remains Conversation, Routine, AutomationOccurrence, Task, Plan/Step, Attempt, CapabilityInvocation, Effect and Evidence. Existing external workflow products remain optional WorkflowBackendAdapter providers. Their internal nodes and run statuses do not become LiteCowork domain records.

For workflow authoring, distinguish three separate paths:

1. **Manual provider authoring:** users create/edit the graph in the connected provider's own visual editor.
2. **Provider-native AI authoring:** where enabled by the provider, users can ask n8n Assistant or Activepieces Build by Asking/MCP to create or edit a provider-owned graph. The result remains a normal graph in that provider and can be inspected/refined/published there. This is available at the provider boundary, subject to exact product/version/permission gates; it is not a LiteCowork agent capability.
3. **LiteCowork-owned agent assistance:** a LiteCowork Coworker may help formulate a LiteCowork Routine and map inputs to an existing provider workflow. Direct LiteCowork-agent graph mutation through provider MCP/API is a future provider-specific capability requiring a separate owner/security decision. It is not current V1 functionality.

The target WorkflowBackendAdapter for discovering/validating/invoking existing external workflows is itself **not implemented in the active LiteCowork source tree**. Therefore no workflow selection/invocation feature is currently served by LiteCowork. The product pattern is hybrid authoring with provider-owned graphs, without a LiteCowork Core canvas or graph runtime.

## 2. Snapshot, source pin and trust caveat

The inspected source was the public main head at commit 6759fa3ee857f4f4966865e12de99ae84ab6de07, committed 2026-11-06 08:39:48 UTC. The package manifest at that revision reports version 0.62.0 and PolyForm-Noncommercial-1.0.0. Use this revision for reproducible source references; do not cite an unpinned moving main URL as the only evidence.

The current README labels OpenBot a **development preview** and states that agents currently run with danger-full-access and approvalPolicy: never; agents can read/write files, run commands, use the network and control the embedded browser without per-action confirmation after first-launch consent. This is a material security limitation, not a pattern to copy. Its UI and graph UX can be studied, but LiteCowork must preserve its own Trust/Grant/Environment admission and mediated-effect semantics.

The noncommercial license and private package metadata mean this repository is a **design reference**, not an assumed source of reusable components. Re-check file-level licensing and dependencies before any future code reuse; no code is copied in this work.

## 3. Product and runtime topology

OpenBot is a Bun workspace with an Electron desktop application, SolidJS renderer/shared UI, provider adapters, local SQLite persistence, account/API Workers, a Signal/relay service, mobile client and shared contracts. Its root README and architecture guide describe a local-first product that can also connect to external provider services; local-first does not mean offline-only.

Its centre is persistent agent profiles and provider sessions. It runs existing native/ACP coding agents, gives each agent a workspace and conversation, and exposes queue, file transfer, browser, agent-to-agent and task features around them. Provider-specific auth/session state remains with the native CLI/provider rather than becoming a single universal credential system.

**LiteCowork mapping:** borrow the separation between renderer, privileged host, persistent owner records and native adapters. Do not transplant Electron/SolidJS service assumptions into the Tauri/Rust daemon. LiteCowork's Runtime/Operator/Workspace/Trust/Task records remain canonical.

## 4. Agent model and effort picker

The screenshot's model picker is associated with a selected agent/profile. The inspected shared AgentSettingsPanel uses a provider model picker and profile/account helpers. Provider drivers and provider-runtime modules supply model lists, defaults, account information and provider-specific options.

The interaction pattern is compact:
- show the active agent/provider profile identity;
- search the model list;
- show one selected model and provider-declared default labels;
- expose reasoning effort when the provider/model supports it;
- keep other provider-specific controls separate;
- do not turn the picker into a universal model router.

OpenBot's model and effort choices are per-agent profile settings. This is not a guarantee that every provider exposes the same effort levels or that the same model identifier is portable across accounts.

**LiteCowork decision:** use a compact anchored popover scoped to the exact AgentProfile/AgentBinding and fresh AgentControlDescriptor. Preserve the exact provider model identifier and descriptor revision. Effort is optional and provider-declared. A stale descriptor blocks a new selection while preserving the draft. A setting applies to future admissions/new sessions; it cannot silently mutate an active native session or provider-wide native config. Task selection remains distinct: select the eligible Coworker/AgentBinding first, then show model/effort only if its module exposes those controls.

Current LiteCowork implementation remains target-only for this control: the Feature 002 audit found ControlDescriptor read/refresh returning 503 and no fully qualified multi-provider send path. A model catalogue in UI is not proof that a Conversation or Task can dispatch that model.

## 5. Tasks page and navigation

OpenBot's Tasks screen is separate from the ordinary agent Conversation view. The composer selects one agent for the next task; the choice is remembered per server as a convenience preference. Task rows show status and the agent identity. Opening a task presents its conversation in a right-hand panel so the list remains visible. The list is grouped into Needs You, Working, Waiting, Unread, Read and Closed sections. Its composer also supports attachments and optional voice input.

The UI code includes a short Undo interval before a new task is committed, a separate task list for all agents on a server, and per-task conversation snapshots/deltas. The status group labels are OpenBot's own projections; they are not interchangeable with LiteCowork's Task/Attempt/UserRequest enums.

**LiteCowork decision:** keep a compact icon-led main rail; one Work destination; a compact eligible Coworker/AgentBinding selector; visible group headings only when non-empty; and a side panel for Task detail where width permits. On narrow windows, detail may become a full-screen route with a real Back path. Needs You must derive from unresolved UserRequest/Approval/Blocker owner projections; Working must derive from an admitted nonterminal Attempt/Step; Ready/Queued must not be called Working. Closing the detail panel is not Stop Task. Any Undo UI requires a durable cancellable admission, not a timer-only projection.

## 6. Usage dashboard and accounting

OpenBot's docs/architecture/analytics.md describes a durable local usage subsystem separate from Conversation projections. Usage records survive Conversation deletion; agent deletion removes the related usage/checkpoint/activity rows. It aggregates by day/model/agent/provider and provides host-wide and agent-scoped views.

Provider semantics differ:
- Codex counters are cumulative and require checkpoints/last-value handling, not summing each cumulative total;
- Claude has per-model query totals and process-specific counter identity;
- ACP usage is optional and generally session-total, with provider-specific exceptions;
- missing fields remain unknown;
- provider account quotas are read through separate provider-specific readers where available.

The dashboard's cost is explicitly a USD **API-equivalent estimate**, not a subscription charge. Unknown/managed pricing can stay unpriced. Provider limits/quota and reset times are not inference spend. Tool/media fees may be outside the estimate. This is a useful warning against displaying a precise-looking total without provenance.

**LiteCowork decision:** retain the stricter two-ledger model in V1-TELEMETRY-USAGE-RESILIENCE-LLD.md: immutable inference-usage leaves with provenance/cost basis, and separate quota snapshots. PROVIDER_API billed amount, LIST_PRICE_ESTIMATE, SUBSCRIPTION_INCLUDED and UNKNOWN are distinct. No quota-to-money conversion, no zero for missing usage, and no prompt/completion content in usage storage. Use the reference layout—thin filter bar, compact headline metrics, provider/agent breakdown, daily trend, totals strip and detailed drilldown—without weakening LiteCowork's provenance and coverage requirements.

## 7. Workflow graph and agent editing

OpenBot has a real graph editor in packages/ui/src/features/workflows/ and a host-owned workflow feature. Its schema includes workflow, node, edge, version, trigger, webhook receipt, run, step and output-delivery records. A saved run carries the graph snapshot/revision it started from; later graph edits do not change that run.

Its graph node kinds in the inspected schema are:
- trigger;
- text, prompt and asset;
- LLM/model;
- agent;
- image;
- condition;
- approval;
- output;
- note and shape annotations.

Typed connection ports include text, image and file. **There is no dedicated code node kind in the inspected schema.** The LLM node selects provider/model/effort; the agent node delegates to an existing agent. Annotation nodes do not run. The agent editing tool omits the asset and image kinds from the set it may add. Therefore the screenshots should not be described as proof of arbitrary JavaScript/Python code nodes.

The agent tool bridge exposes list_workflows, read_workflow, edit_workflow, delete_workflow, run_workflow and read_workflow_runs. Edit operations are constrained (add/update/move/remove/connect/disconnect) and schema-validated. A human-request policy protects change/run tool use. The canvas has a build presentation path that compares before/after graph state and animates the nodes and edges changed by an agent.

**Important qualification:** source evidence shows the agent can edit/save a graph and the canvas can visualize that change. It does not establish that every agent edit is held in a separate owner-approved draft before commit. The separate draft/diff/accept step is a recommended LiteCowork safeguard, not a claim about OpenBot's exact behavior.

### 7.1 Why LiteCowork is not adopting that graph engine in Core V1

The inspected OpenBot graph is a self-contained workflow runtime. Importing its pattern as an in-Core feature would require LiteCowork to own a second graph schema, graph persistence/revisions, trigger registration, graph execution, run/step statuses, webhook receipts, external output delivery and cancellation/recovery. That would conflict with the existing LiteCowork decision that:
- Automation schedules/observes occurrences and materializes Tasks; it is not a workflow engine;
- Routine is a pinned reusable task instruction/input contract, not a graph runtime;
- a deterministic external workflow is invoked through WorkflowBackendAdapter as one bounded CapabilityInvocation inside a LiteCowork Task;
- provider node/run statuses are observations, not LiteCowork Steps or Task state;
- LiteCowork Task/Attempt/Effect/Evidence remain the execution and verification authority.

The target WorkflowBackendAdapter contract covers health, list/get, input validation, invoke, get/stream/cancel run and result normalization. It does **not** currently define external graph create/update/publish operations, and no production adapter implementation exists in the active LiteCowork source tree. Thus the current checkout cannot discover/invoke a provider workflow through LiteCowork, and it cannot expose graph mutation tools to a LiteCowork agent. Users may still author graphs manually or use provider-native AI/MCP tools directly inside n8n/Activepieces, when those features are enabled and permitted by the provider.

### 7.2 V1 and future authoring paths

**Provider-owned manual path:** the user creates/edits the graph in their connected workflow provider's own editor (for example, a user-owned n8n/Activepieces deployment where supported). The *target* LiteCowork integration is to discover an existing workflow, show provider identity and supported input/output metadata, validate the selected input, map it to the pinned Routine/Task, invoke it under Trust, and observe the external run through a qualified adapter. No production WorkflowBackendAdapter exists in this checkout, so none of those discovery/invocation steps is currently available in LiteCowork. The user may open the provider's own editor through a reviewed external link when such a provider connection/link exists; LiteCowork does not embed or clone that editor.

**LiteCowork Routine assistance target:** once the relevant Conversation/Coworker path and provider adapter are qualified, a Coworker may help formulate the desired Routine, map user-provided values to the selected workflow's input schema, explain validation errors and summarize run receipts. Today it is a product design target, not a served workflow-integration path. It cannot claim to have created/edited a provider graph.

**Future direct LiteCowork-Coworker authoring extension:** a later LiteCowork version may expose a selected provider's create/draft/update/validate/publish/read-version tools to a LiteCowork agent only through a separate WorkflowAuthoringAdapter or narrowly scoped mediated MCP Connection capability. This is not required for the user to use n8n Assistant or Activepieces Build by Asking/MCP directly in the provider today. If the LiteCowork-originated path is considered, it needs provider version/capability declaration, exact graph digest, stale revision checks, external effect summary, distinct read/mutate/test/publish/run/delete permissions, and owner confirmation. It must not add graph runtime or graph persistence to LiteCowork Core. A Coworker proposal must be staged as a provider-side draft and reviewed before provider publish. This requires an explicit owner ADR, API/Trust review, provider-specific tests and license review; it is not part of current V1 acceptance.

### 7.3 Related external workflow platforms: hybrid authoring and code nodes

The follow-up provider research confirms that manual visual editing and agent-generated
graphs are not mutually exclusive. Several external platforms now expose both paths:

**n8n**
- The n8n Assistant announcement (2026-09-09) describes plain-language workflow
  planning/building directly on the canvas, credential prompts, running and debugging,
  then leaving the result as a normal n8n graph that the user can inspect and edit by hand:
  https://blog.n8n.io/introducing-n8n-assistant/
- The official n8n MCP server describes an AI client creating and editing workflows,
  validating them, running tests, reading failures and iterating in the connected n8n
  instance: https://n8n.io/mcp/
- n8n's Code node supports JavaScript and Python:
  https://docs.n8n.io/code/code-node/
- n8n's Security Audit reports risky built-in nodes, including nodes that can fetch/run
  code on the host: https://docs.n8n.io/hosting/securing/security-audit/
- Security caveat: GitHub advisory GHSA-9pq8-m8gp-4p53 (published 2026-05-27) describes
  a Python Task Runner sandbox escape for workflows containing a Python Code Node when
  Python Task Runner is enabled. The advisory lists fixes in 1.123.48, 2.21.8 and 2.22.4:
  https://github.com/n8n-io/n8n/security/advisories/GHSA-9pq8-m8gp-4p53
  Any user-owned n8n instance connected to LiteCowork must be version/advisory checked;
  "Code node" or "sandbox" is not by itself a sufficient isolation guarantee.

**Activepieces**
- Its product documentation describes a hybrid: chat can draft flows, the visual builder
  gives full control, and TypeScript can be written inline for harder steps:
  https://www.activepieces.com/product/workflow-builder
- Its Agents documentation distinguishes a deterministic Flow (steps written down once)
  from an Agent (which chooses tools/steps at each run); agents can invoke flows and can
  be created from a plain-language brief:
  https://www.activepieces.com/docs/agents
- Its MCP tool reference includes `ap_build_flow` (create a flow from a trigger and
  sequential steps), `ap_add_step`, `ap_update_step`, `ap_validate_step_config`,
  `ap_read_step_code` and `ap_flow_structure`. CODE steps accept JavaScript/TypeScript
  `sourceCode` and `packageJson` dependency declarations:
  https://www.activepieces.com/docs/mcp/tools
- Its architecture documentation describes a worker, job queue, engine and sandbox
  responsible for process lifecycle/resource limits:
  https://www.activepieces.com/docs/install/architecture/overview
  That is the provider's own sandbox/worker boundary, not a LiteCowork Environment.

**Node-RED**
- Its Function node runs JavaScript against flow messages; its built-in palette also
  includes declarative Change/Switch/Template nodes:
  https://nodered.org/docs/user-guide/writing-functions
  This is another example of a manual flow canvas with optional code execution.

### 7.3.1 Provider documentation links and factual boundaries

The provider documentation was rechecked on 2026-11-06. The essential distinction is that the provider-native AI builder and the provider's external MCP interface are related but separate access paths.

**n8n sources and precise availability**
- Assistant product announcement (2026-09-09): https://blog.n8n.io/introducing-n8n-assistant/
- Instance-level MCP landing page: https://n8n.io/mcp/
- Instance-level MCP setup, permission and version documentation: https://docs.n8n.io/connect/connect-to-n8n-mcp-server/
- Code node: https://docs.n8n.io/code/code-node/
- Security Audit: https://docs.n8n.io/hosting/securing/security-audit/
- Python Task Runner advisory GHSA-9pq8-m8gp-4p53: https://github.com/n8n-io/n8n/security/advisories/GHSA-9pq8-m8gp-4p53

n8n Assistant (the provider's own AI canvas builder) is behind a preview flag. The announcement states: Cloud is on by default for new instances except Enterprise Cloud; self-hosted Docker requires version 2.36+ plus user-supplied keys/environment configuration; self-hosted npm is unsupported; the launch is single-instance and does not control the user's computer/browser. n8n states that credentials and workflow activation require explicit confirmation. Do not infer that enabling instance-level MCP enables or unlocks the separate Assistant feature.

The instance-level MCP server supports building/editing workflows (documented from n8n 2.13.0), validation, testing, test data and execution inspection. It has its own security model: the instance owner/admin enables it; each workflow is exposed individually; connected MCP clients receive explicit permission scopes (including read-only vs creation/execution); all clients share the same instance-level set of MCP-enabled workflows rather than per-client workflow subsets; search_workflows provides previews for workflows the current user can view, whereas full workflow read/edit/run requires MCP exposure. execute_workflow uses the published workflow by default and has a manual mode for the current unpublished version. The current MCP setup UI/version details have additional gates (for example, the newer per-client connection-details UI begins at 2.33.0); use the provider's current setup guide for the exact instance version.

**Activepieces sources and precise permissions**
- Workflow builder / Build by Asking: https://www.activepieces.com/product/workflow-builder
- Agent semantics: https://www.activepieces.com/docs/agents
- MCP server overview and per-project tool categories: https://www.activepieces.com/docs/mcp/overview
- MCP tools and input schemas: https://www.activepieces.com/docs/mcp/tools
- Execution architecture: https://www.activepieces.com/docs/install/architecture/overview
- Workers and sandbox/network boundary: https://www.activepieces.com/docs/install/architecture/workers

Activepieces' built-in MCP server is enabled in the provider's Settings and uses OAuth. Discovery tools are always available and read-only; other categories are enabled/disabled per project. Operations are scoped to the authenticated project, credentials/secrets are not returned by tools, and its setup-guide tool tells the user how to configure a connection in the provider UI rather than accepting secrets over MCP. The tool catalog includes flow creation/building, flow management/publish/enable/disable/delete, step/branch updates, testing and run inspection, and Tables CRUD. Some operations are destructive: ap_delete_flow deletes a flow and its versions and is documented as irreversible. The separate ap_run_action one-shot operation has a per-call credit charge; persistent flow runs are billed as normal flow runs. These details make it incorrect to treat MCP connection as implicit authority for every action.

**Code and security**
- n8n's Python Task Runner advisory GHSA-9pq8-m8gp-4p53 was published 2026-05-27. It applies where Python Task Runner is enabled and an affected workflow contains a Python Code Node. Published fixed versions are 1.123.48, 2.21.8 and 2.22.4. Do not generalize this to every Code node or infer that patching this one advisory proves an entire instance secure.
- Activepieces documents JavaScript/TypeScript CODE steps and a separate worker/queue/engine/sandbox architecture. Its own documentation describes process/resource management but also documents configurable execution and network isolation choices. It is not a LiteCowork Environment attestation.
- Provider documentation is a capability description, not proof that a specific user's instance has a feature enabled, is on a safe version, has sufficient project permissions, or that an arbitrary workflow is safe. Probe the exact instance/version/settings and required scopes before integration.


### 7.4 What this means for LiteCowork

The product answer is **hybrid authoring, provider-owned graph**:
1. A user can open the connected n8n/Activepieces editor and build visually by hand.
2. Where the provider offers its own AI Assistant, the user can describe the intended
   workflow, let that provider construct a normal editable graph, then inspect/refine it
   manually. The resulting graph, credentials and executions remain in that provider.
3. A future LiteCowork-to-provider authoring integration could expose the provider's
   official MCP/REST graph tools to a selected Coworker, but only after the tool set is
   registered as a qualified Connection/capability, exact graph mutators are declared,
   scopes and code-node effects are disclosed, and Trust gates publish/run appropriately.
   The agent's create/edit authority must be narrower than ordinary invocation authority.
4. LiteCowork Core still does not own the graph or its code execution. The parent Task
   records the one bounded external invocation, provider run reference/observations,
   effects and independent result verification. Provider internal steps are not
   LiteCowork Steps.

This is more capable than a manual-only experience, without building a second n8n-like
engine into LiteCowork. The currently documented WorkflowBackendAdapter remains read/
invoke-oriented: direct external graph-authoring tools are **not yet a LiteCowork V1
capability**. Provider-native AI builders can be used in the provider's own product; direct
LiteCowork agent editing is a future, explicitly gated extension.

## 8. UI and visual design lessons

The useful screenshot lessons are:
- a slim persistent rail with a small number of meaningful destinations;
- page-specific context in a compact header instead of widening the global sidebar;
- one focused Tasks composer;
- short status rows and subdued metadata;
- Task detail beside the list instead of replacing the whole screen on desktop;
- an Usage dashboard with a small set of filters and a detailed table below the trend chart;
- a model popover with search and effort at the bottom, not a full-screen model catalogue;
- a workflow canvas whose palette, inspector and run history stay contextual rather than always expanded.

For LiteCowork, those are presentation principles only. Domain states, affordances and action availability must continue to come from LiteCowork owner projections. Avoid decorative statuses, fake progress, inert dropdowns and duplicate status labels.

## 9. Exact source references

All links below pin the inspected commit:

- OpenBot README and development-preview warning: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/README.md
- OpenBot package manifest and license declaration: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/package.json
- Repository structure and state ownership: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/docs/architecture/structure.md
- Agent/workflow architecture: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/docs/architecture/agents.md
- Usage analytics design: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/docs/architecture/analytics.md
- Agent model/effort settings: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/packages/ui/src/features/conversation/AgentSettingsPanel.tsx
- Provider model picker: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/packages/ui/src/components/ProviderModelPicker.tsx
- Tasks page: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/packages/ui/src/features/tasks/TasksPage.tsx
- Usage report: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/src/renderer/src/features/usage/AgentUsagePanel.tsx
- Workflow graph schema: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/src/backend/workflows/workflow-schema.ts
- Agent workflow tools: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/src/backend/workflows/workflow-tools.ts
- Workflow canvas: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/packages/ui/src/features/workflows/WorkflowCanvas.tsx
- Workflow agent-change visualization: https://github.com/nightly-labs/openbot/blob/6759fa3ee857f4f4966865e12de99ae84ab6de07/packages/ui/src/features/workflows/workflow-build.ts

## 10. Inspection limitations

This was a source/document inspection at the pinned GitHub revision. OpenBot was not cloned, built, run or subjected to a security test in this pass. The public README's warning and the package license were reviewed; no live provider account, quota, remote/mobile or computer-use workflow was independently exercised. Do not treat this study as runtime qualification or a legal opinion.
