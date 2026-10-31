# LiteCowork — Product Strategy

**Status:** active strategy derived from Revision 9 and the 30 October 2026 competitive engineering audit.  
**Authority:** subordinate to root `ARCHITECTURE.md`, `docs/PRODUCT.md`, the V1 Product Release Contract and owner-domain contracts.

## 1. Product definition

LiteCowork is a **local-first, chat-first AI workspace around Bring Your Own Agent (BYOA)**. It is not a model router, a replacement agent, a workflow-engine clone or a terminal wrapper. It gives the user one coherent desktop workspace around the native agents they already choose.

LiteCowork owns the durable user-facing workspace and cross-agent authority: Workspace, Conversation, Tasks, Trust, capability grants, mediated Effects, Artifacts, Evidence, recovery and presentation. Native agents retain their own reasoning, native authentication, models, modes, tools, private transcripts and private subagents. LiteSPM owns package discovery and package lifecycle. These ownership boundaries are load-bearing and are not to be redesigned for visual parity.

The ordinary user flow starts with a Conversation. A question, explanation or draft does not require a Task or Coworker. Outcome-oriented work becomes a durable Task only through explicit or clearly accepted promotion. Coworkers are optional persistent identities, not a mandatory onboarding step.

## 2. The promise to the user

> One workspace for the agents you trust, the work you actually do, and the files and applications you own — with continuity, visible control and truthful evidence.

The user should be able to:
1. Start a conversation with an explicitly selected qualified native agent and retain the human-facing history.
2. Switch the lead agent for a later turn without pretending to transfer private reasoning or hidden native transcripts.
3. Attach selected folders/resources, retrieve grounded content and see which exact sources were used.
4. Use the same workspace for research, coding, documents, apps, automations, Coworkers and artifacts.
5. Promote work into a durable Task with an accepted plan, bounded attempts, recoverable effects and evidence.
6. Discover and reuse compatible capabilities across agents through LiteSPM without copying credentials or duplicating package state.
7. Review approvals, blocked work, questions and failures in one actionable Needs You surface.
8. Return after a restart and see the actual persisted state, not a success animation unsupported by evidence.

## 3. Who it serves

**Primary user:** a technically capable desktop user who already uses one or more native AI agents and wants a single workspace for conversations, projects, files, automation and deliverables.

**Secondary user:** a nontechnical professional who wants a persistent AI workspace and does not need to understand ACP, MCP, OAuth, Runtime or AgentModule internals.

Both use the same product. Plain-language outcomes are the default. Technical provenance, model/effort/native options and system detail are progressively disclosed in the composer, Activity and Inspector. Do not create separate “Cowork” and “Code” universes with separate history or stores.

## 4. The moat — not agent count

The defensible difference is the combination of:

- **Agent Relay:** a durable Conversation can change lead agents without claiming private-session migration.
- **Workspace continuity:** one Workspace connects chats, roots, Resources, Tasks, Coworkers, Routines and Artifacts.
- **Portable capabilities:** LiteSPM manages packages once; compatible agents receive qualified bridges, while Core separately owns grants and actual use.
- **Evidence-bearing work:** Task completion is based on required criteria, reconciled effects and verification, not the worker saying “done.”
- **Native fidelity:** each AgentModule exposes the controls and behavior that exact agent/version/OS routes can actually qualify.
- **Local ownership:** V1 work runs on the user's desktop; local files, secrets and execution remain local unless the user explicitly connects external services.
- **One useful Workbench:** files, diffs, terminal, browser, computer and artifacts are available within the same workspace, with state and control surfaced truthfully.

“Supports 25 agents” is not meaningful if no native turn can be sent. “Hundreds of connectors” is not meaningful if no useful operation can complete safely.

## 5. Competitive bar

For V1 desktop, the non-cloud capability floor includes:
- persistent chat, multimodal inputs and rich output;
- research, workspace/project context and local file work;
- browser/computer interaction with truthful observation and takeover;
- connectors, capability discovery and account connections;
- professional deliverables and artifact versioning;
- persistent assistants/Coworkers, memory, goals and routines;
- automation and notifications;
- bounded multi-agent work, skills and plugins;
- usage, recovery, privacy and local ownership.

The excluded parity promise is competitor-owned cloud computers, laptop-off continuation on LiteCowork-hosted Runtime, cloud-only storage and collaboration, and cross-device continuation that depends on hosted Runtime. Normal model inference through a user's legitimate provider account and cloud SaaS connections are still allowed; they are not LiteCowork execution Runtime.

**Keep two release views separate:**
- **V1 Core completeness:** every promised local journey is safe, functional and qualified on the declared platform.
- **Competitive parity:** the product reaches the broader convenience and breadth expected from leading cowork products. A missing competitive convenience does not justify weakening Core invariants.

## 6. V1 / later boundaries

**V1:** local desktop Runtime, native Conversation, qualified agent routes, Workspace and Resource context, one safe Task path, LiteSPM integrated service, first-party connection/connector slices, core Artifacts, useful Workbench, Coworkers and bounded local Automations where qualified.

**V1.1:** convenience and refinement that do not require a new authority model, such as conversation rewind/fork or richer voice routes, once their prerequisites qualify.

**V2:** LiteCowork-hosted cloud Runtime, laptop-off continuation, remote Runtime, multi-device replication, mobile Dispatch and multi-user organisation/RBAC.

Do not present a sleeping/offline laptop as continuing local work. A local Runtime-bound task waits safely or is re-admitted on another eligible Runtime only after the relevant placement and recovery contracts exist.

## 7. Product UX rules

- One primary composer; actions such as Send, Run as Task, Schedule and Save as Routine are explicit and context-sensitive.
- A Conversation is not a Task; a Coworker is not a prerequisite.
- Needs You is an actionable inbox, not only a status page.
- A Workbench panel is visible only when its provider is real and its state is known; unsupported panels explain why.
- Agent/model/mode/commands remain native and agent-specific. There is no global Providers screen or hidden global router.
- Installation, observation, adoption, connection, assignment, grant, activation, invocation and verified outcome are separate facts.
- Presentation is a projection of committed semantic content and authorized state. A visual card or transient stream cannot create product truth.
- Unknown readiness, usage, cost, progress or quota stays Unknown.

## 8. Strategic non-goals

LiteCowork Core is not a universal model router, cognitive memory engine, Office suite, browser-agent implementation, computer-use model, code-intelligence platform, web-search engine, second package marketplace or generic visual workflow-graph engine. Integrate or qualify specialist providers behind the established owner contracts instead of absorbing their entire runtimes.

## 9. Source basis

This strategy reconciles Revision 9 §1–§2 and §30, the V7 deep competitive audit §§1–§14, the existing Product and Architecture contracts, and Revision 9 Appendices AI–AL. The pinned implementation baseline referenced by the supplied material is `c7e9c05ab09563a554515bdd7e0e7d86c99fffa0`; the strategy is not an assertion that the journeys are currently implemented or qualified.
