
# Product strategy

**Status:** active product strategy. Normative scope is governed by PRODUCT.md and V1-PRODUCT-RELEASE-CONTRACT.md. Current delivery status is governed by IMPLEMENTATION-STATUS.md.

## 1. Product definition

LiteCowork is a **local-first, chat-first AI workspace around Bring Your Own Agent (BYOA)**. It is not a universal model router, a replacement agent, a workflow-engine clone or a terminal wrapper. It gives the user one coherent desktop workspace around the native agents they choose.

LiteCowork owns the durable workspace and cross-agent authority: Workspace, Conversation, Tasks, Trust, capability grants, mediated Effects, Artifacts, Evidence, recovery and presentation. Native agents retain their own reasoning, native authentication, models, modes, tools, private transcripts and private subagents. LiteSPM owns package discovery and package lifecycle. These ownership boundaries are load-bearing.

The ordinary user flow starts with a Conversation. A question, explanation or draft does not require a Task or Coworker. Outcome-oriented work becomes a durable Task only through explicit or clearly accepted promotion. Coworkers are optional persistent identities, not a mandatory onboarding step.

## 2. The promise to the user

> One workspace for the agents you trust, the work you actually do, and the files and applications you own — with continuity, visible control and truthful evidence.

The user should be able to:

1. Start a conversation with an explicitly selected, qualified native agent and retain the human-facing history.
2. Switch the lead agent for a later turn without pretending to transfer private reasoning or hidden native transcripts.
3. Attach selected folders/resources, retrieve grounded content and see which exact sources were used.
4. Use the same workspace for research, coding, documents, apps, automations, Coworkers and artifacts.
5. Promote work into a durable Task with an accepted plan, bounded attempts, recoverable Effects and evidence.
6. Discover and reuse compatible capabilities across agents through LiteSPM without copying credentials or duplicating package state.
7. Review approvals, blocked work, questions and failures in one actionable Needs You surface.
8. Return after a restart and see the actual persisted state, not a success animation unsupported by evidence.

## 3. Who it serves

**Primary beachhead:** technically capable desktop users, developers and technical founders who already use one or more native AI agents and want a single workspace for conversations, projects, files, tools and deliverables.

**Secondary audience:** nontechnical professionals and researchers who want a persistent AI workspace and do not need to understand ACP, MCP, OAuth, Runtime or AgentModule internals.

Both use the same product. Plain-language outcomes are the default. Technical provenance, model/effort/native options and system detail are progressively disclosed in the composer, Activity and Inspector. Do not create separate Cowork and Code universes with separate histories or stores.

The "$100–$300 per month AI tax" is not an established market statistic and must not be used as a factual claim without primary customer/pricing evidence. Validate willingness to pay, switching costs, retention and target-segment demand through interviews and product telemetry.

## 4. The moat — not agent count

The defensible difference is the combination of:

- **Native fidelity:** each AgentModule exposes controls and behavior that the exact agent/version/OS route can actually qualify.
- **Conversation continuity:** one durable human-facing Conversation can change lead agents without claiming private-session migration.
- **Workspace continuity:** one Workspace connects chats, roots, Resources, Tasks, Coworkers, Routines and Artifacts.
- **Portable capabilities:** LiteSPM manages packages once; compatible hosts receive qualified bridges, while Core separately owns grants and actual use.
- **Evidence-bearing work:** Task completion is based on required criteria, reconciled Effects and verification, not the worker saying "done."
- **Local ownership:** V1 work runs on the user's desktop; local files, secrets and execution remain local unless the user explicitly connects external services.
- **One useful Workbench:** files, diffs, terminal, browser, computer and artifacts are progressively integrated, with state and control surfaced truthfully.

"Supports 25 agents" is not meaningful if no native turn can be sent. "Hundreds of connectors" is not meaningful if no useful operation can complete safely.

## 5. Competitive bar

The local desktop target must become useful across:

- persistent chat, multimodal input and rich output;
- research, Workspace/project context and local file work;
- browser/computer interaction with truthful observation and takeover;
- connector and capability discovery with account Connections;
- professional deliverables and Artifact versioning;
- persistent assistants/Coworkers, scoped memory, goals and routines;
- Automations and notifications;
- bounded multi-agent work, skills and plugins;
- usage, recovery, privacy and local ownership.

The excluded local V1 promise is competitor-owned cloud computers, laptop-off continuation on a LiteCowork-hosted Runtime, cloud-only storage, hosted collaboration and cross-device continuation that depends on a hosted Runtime. Normal model inference through a user's legitimate provider account and cloud SaaS Connections are allowed; they are not LiteCowork execution Runtime.

Keep two release views separate:

- **V1 Core completeness:** every promised local journey is safe, functional and qualified on the declared platform.
- **Competitive parity:** the product reaches the broader convenience and breadth expected from leading cowork products. A missing competitive convenience does not justify weakening Core invariants.

## 6. V1 and later boundaries

**V1 target:** local desktop Runtime, native Conversation, qualified agent routes, Workspace and Resource context, one safe Task path, LiteSPM integrated service, first-party Connection/connector slices, core Artifacts, useful Workbench, Coworkers and bounded local Automations where qualified.

**V1.1 candidates:** convenience and refinement that do not require a new authority model, such as Conversation rewind/fork or richer voice routes, once their prerequisites qualify.

**V2:** LiteCowork-hosted cloud Runtime, laptop-off continuation, remote Runtime, multi-device replication, mobile Dispatch and multi-user organization/RBAC.

Cloud continuation is Task rehydration, not live process migration. It requires portable committed state, effect reconciliation, Hub-owned fencing, a new Attempt and a new Runtime/Environment. Local resources, OS credentials, native sessions and local-only capabilities remain placement blockers.

Remote mobile control is a separate authenticated remote-gateway integration. Read-only status and artifact access should precede actions; approval must be bound to an exact operation, target, digest, scope, expiry and owner identity. A generic MCP server is not itself a secure remote-control architecture.

## 7. Product UX rules

- One primary composer; Send, Run as Task, Schedule and Save as Routine are separate, reviewable actions.
- A Task is created only by explicit promotion or a qualified Automation occurrence; a chat message is not silently a Task.
- A Coworker is optional and is not a resident process.
- Agent, model, auth, effort, mode and command controls remain native to the selected AgentModule.
- Installation, Connection, assignment, grant, activation, invocation and verified outcome are separate facts.
- Needs You is an actionable inbox, not a notification count.
- Workbench is contextual; terminal/browser/computer tiles do not imply a real provider.
- Presentation is a projection of committed semantic content and authorized state. A visual card or transient stream cannot create product truth.
- Unknown readiness, usage, cost, progress or quota stays Unknown.
- Motion reflects real state transitions. Do not simulate workers, progress, verification or success.
- Security promises are precise: Job Objects constrain process lifetime but are not by themselves a full filesystem/network/credential sandbox.

## 8. Architecture non-goals

LiteCowork Core is not a universal model router, cognitive memory engine, Office suite, browser-agent implementation, computer-use model, code-intelligence platform, web-search engine, second package marketplace or generic visual workflow-graph engine. Integrate or qualify specialist providers behind established owner contracts instead of absorbing their entire runtimes.

## 9. Research discipline

Competitor repository claims are tied to exact repository commits in REFERENCE-IMPLEMENTATIONS.md. A README, demo, issue, screenshot or alpha template is not evidence of a production capability. Record what was inspected, its maturity, dependencies, trust boundary, adopted idea, rejected assumption and the LiteCowork owner contract affected.

Market claims require primary customer evidence. Provider subscription reuse requires current provider terms and an exact supported authentication route. Do not promise that any third-party desktop wrapper can use a flat-rate provider subscription without additional cost or restrictions.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## OpenBot reference and workflow-engine boundary — 2026-11-06

The pinned OpenBot study is documented in [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md). Its compact agent/task/usage UX and its combination of a manual graph canvas with agent-edit tools are valuable reference patterns. They do not change LiteCowork's product boundary: LiteCowork is not a generic visual workflow-graph engine. In V1, manual workflow editing and provider-native AI authoring remain inside the connected external provider's own product. LiteCowork's target integration is to discover, validate, map, invoke and observe an existing workflow through a future qualified WorkflowBackendAdapter as one bounded CapabilityInvocation inside a Task. **That adapter is not implemented in the active source tree**, so no workflow selection or invocation is currently served. Agent assistance may draft the LiteCowork Routine and map inputs, but no production adapter authorizes graph creation/update/publish. A provider-specific authoring extension requires a separate owner decision, provider-side draft/validation/publish semantics and explicit user approval. No Core graph runtime or arbitrary code node is added.
