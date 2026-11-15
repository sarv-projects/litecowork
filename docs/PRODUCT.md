# Product Contract

## Promise

LiteCowork lets people **start with native-agent conversation** and, when useful,
complete managed durable work with controlled permissions, visible progress,
scoped knowledge and real deliverables. A durable Task survives eligible
worker or process replacement; ordinary chat does **not** require a Task or
Coworker. The ten-part V1 scope and free-core constraint are frozen in
[V1 Product Release Contract](V1-PRODUCT-RELEASE-CONTRACT.md), with
[Final Screen Contract](V1-UI-SCREEN-CONTRACT.md) and
[LiteSPM interoperability](LITESPM-LITECOWORK-INTEROP-LLD.md).
Provider-owned costs remain separate; LiteCowork Runtime cloud continuation,
cross-device support and teamwork remain V2.

## Long-term product direction

LiteCowork's north star is to supersede app switching for supported everyday work:
people should be able to ask, inspect, decide, create and complete work in one
coherent workspace instead of moving between separate chat, code, browser, file,
analysis and automation products. The conversation is the common entry point;
Projects, Recents and the current job's contextual work surface keep related
messages, source material, live sessions, results and deliverables together.
Specialized capabilities remain available as contextual objects and workflows,
not as separate product modes that make users choose a tool before explaining
their goal. This is a product direction, not a claim that V1 replaces every
desktop application or that every provider is already integrated. The detailed
navigation and right-panel proposal remains under review in the
[Workspace Experience Revision](WORKSPACE-EXPERIENCE-REVISION-2026-11.md), and
V1 availability continues to be limited by this contract and each owner's
qualification evidence.

## Core user expectations

1. Simple questions remain conversation; outcome-oriented work becomes durable Task state.
2. User does not need to understand MCP, ACP, providers or runtime topology for ordinary use.
3. User can inspect internals when desired through Inspector.
4. V1 work runs on the local desktop. Post-V1 cloud continuation is available only when
   inputs, secrets, capabilities and effect safety permit it.
5. LiteCowork never claims completion solely because a worker says "done".
6. LiteCowork does not fake visual interaction when structured/API capabilities were used.
7. Connected capabilities are discovered progressively instead of flooding every agent context.
8. Parallel/heterogeneous agents may collaborate but lead reasoning remains agent-owned.
9. User can steer, pause/resume, cancel, and see what is blocked or waiting for them.
10. Reusable work definitions are Routines; reusable agent procedure may separately be
    proposed as Skills and then become LiteSPM-managed packages.
11. Closing the Operator does not stop a background Runtime when its startup policy keeps
    it alive. Local work still requires the specific Runtime/resources to remain available;
    sleep/offline creates a visible wait or a safe new Attempt elsewhere.
12. Installed Agents, MCP servers, and desktop applications are cold by default and start
    only when admitted work needs them, except explicitly configured shared services.
13. A named Coworker can keep user-facing preferences and responsibilities across lead
    agent changes; Coworker state never replaces Task truth.
14. A lead can use native subagents or explicitly enabled LiteCowork worker profiles.
    Host-delegated work is bounded by accepted plan Steps, child-scoped authority, budget,
    isolation, and verification.
15. Cost, quota, progress, and warm readiness are shown only at the confidence supported
    by their source. Unknown values remain unknown.
16. Goals are passive user intent and Suggestions are proposals. Neither can authorize or
    start work without the normal user and Task admission path.
17. Technical and nontechnical users share one product flow. Plain-language outcomes are
    the default; progressively disclosed work detail and Inspector retain technical
    provenance without creating separate product modes. The primary surface speaks in
    terms of work, updates, decisions and deliverables. Agent and model selection stays
    explicit in the composer, while internal Attempt/lease/bridge mechanics stay under
    Activity or Inspector unless a user needs them to resolve a blocker.
18. Presentation is derived from authorized projections. Transient streamed output is not
    durable until its ConversationMessage commits; UI frames cannot authorize or settle work.

## Primary navigation

```text
New Chat / Recent chats
Needs You (compact attention inbox)
Work
Coworkers
Library
Automations (Routines inside)
More (Discover, Dashboard, Goals, Notifications, Ideas, explicit New Task)
Settings (footer)
```

Recent Conversations remain visible in the persistent sidebar. Technical surfaces such as
Agents, MCP, Providers, Environments, Runtime details and protocols live under
Settings/Inspector/Discover details.

First use establishes a Workspace and an enabled, explicitly selected native lead
AgentBinding; creating a Coworker is OPTIONAL. Ordinary sessions use the Workspace
lead/model/effort without a mandatory Coworker or connection. A Coworker may be
created later using Quick Create or Customize; worker profiles stay disabled until
authorized. See [Coworker target](COWORKERS-TARGET.md).
Conversations may override that selection. If the selected binding is unavailable,
LiteCowork preserves the unsent draft and requests agent setup; admission does not create
a partial ConversationTurn or Task. If an assigned binding becomes unavailable after a
Task is created, the Task records a visible blocking condition and never silently routes
work to another agent.

## Conversation and Task behavior

The composer accepts questions and outcomes in the same place. Ordinary conversation
does not need a Task. A clear request for durable, outcome-oriented work may materialize
a Task; an explicit user request always may. The conversational lead may request
materialization, but ambiguous intent is clarified instead of being sent through a
separate hidden planner. A committed Task is linked to the originating Conversation and
message. See `EXPERIENCE.md` and `FLOWS.md` for the UI and command sequence.

The single composer may expose explicit send actions (`Send`, `Run as Task`, `Schedule…`,
`Save as Routine`). Scheduling or saving reusable work is never silently inferred from
natural language: LiteCowork previews the Routine, trigger(s), timezone, TriggerHost,
execution dependencies, permissions, and cost policy, then requires confirmation before
persisting the Routine or Automation. `Save as Routine` first opens a redacted, editable
Operator draft; only the user's Save command creates an active immutable-revision Routine.
Discarding the draft leaves no durable Routine. A future implementation may persist
explicit drafts, but they are not runnable or schedulable.

Conversation preserves the human-facing exchange. Task records the durable work and
requirements. The user can steer, revise, pause, cancel, or inspect a Task without
depending on one agent transcript. A Task that continues on another Runtime starts a new
Attempt from portable state; no process migration is promised.

Conversation has its own AgentSession scope and can answer simple questions without
creating a Task. A Conversation session is read-only with respect to Task state and
consequential operations. Workspace instructions are durable, versioned context; each
Task pins one revision and receives later instruction changes only through an explicit
TaskSpecRevision.

## Runtime choices

V1 is a local-only desktop installation and does not require a cloud account. Cloud
continuation and remote Runtime are post-V1 releases. Local and future remote Runtime
instances use the same domain contracts. Cross-device availability is conditional on
replicated inputs, eligible agents/capabilities, secrets, Environment support, policy,
budget, and safe Effects. The UI names the actual blocker rather than presenting cloud
continuation as unconditional.

The initial product is a single-user Workspace coordinated by one authoritative Hub.
Additional Runtimes may execute eligible Attempts. Multi-tenant collaboration, automatic
Hub consensus, and transparent migration of native processes are not v1 promises.

The Operator, Runtime daemon, and worker processes have separate lifecycles. Runtime
startup policy is `MANUAL`, `LOGIN_BACKGROUND`, or `ALWAYS_ON_SERVICE`; it determines
whether the daemon starts independently of its UI. Boot recovery does not launch all
Agents/providers/apps; demand-driven supervisors start only prerequisites for admitted
work. Runtime sleep/offline blocks local-bound work but does not stop eligible cloud work.
An optional WakeProvider may be attempted later as best effort; it is never a correctness
assumption or a way to grant remote access to local resources.

Desktop installation provides a separate LiteCowork Operator and `litecoworkd` daemon.
The Operator may close while the Runtime remains available. In `MANUAL` mode, closing the
last Operator may stop the daemon only after safe drain and only when no active local Task,
local TriggerHost, explicit watcher duty, or configured shared service still depends on it.
Agents and local capability packages normally remain cold until admitted work needs them.

## Capability and trust expectations

Discover is the user-facing path to compatible agents and external capabilities. LiteSPM
is the selected external package ecosystem; LiteCowork adds scope-matched compatibility,
grants, activation, effect handling, and verification. Conversation/planning grants are
read-only; Attempt grants are tied to admitted work. The selected base URL and deferred
integration details are in `CAPABILITY-FABRIC.md`. Installing or connecting something
does not by itself authorize it for every Task.

The user sees the requested operation and scope when approval is needed. A message
channel may start or steer permitted work, but a weakly authenticated channel cannot
approve a high-impact action. Credentials are not copied to another Runtime without
explicit placement and secret-lease policy.

## Output and completion expectations

Providers may create documents, spreadsheets, code changes, research, or other
domain-specific outputs. LiteCowork owns Artifact identity, version, provenance, and
availability. Publishing/saving to Library is explicit. A completion label means the
Task's mandatory criteria met their declared evidence requirements; a worker's
completion claim alone is never enough.

## Product limits

LiteCowork reports what it can observe. It does not claim control over an external
agent's private tools, credentials, hidden usage, or unmediated side effects. Where an
action cannot be fenced or reconciled, continuation may require a handoff or user
decision. Those limits are shown as state, not hidden behind a success animation.

The World Index records facts about explicitly selected WorkspaceRoots: resources,
locations, observed revisions, freshness, and deterministic search results. It never scans
the whole machine or acts as an AI reasoning model. A one-time folder attachment is not a
persistent root grant.

Workspace Routines are immutable-revision templates for repeated work, distinct from
Skills (how an Agent performs a procedure), Automations (when/why a Routine runs), and
Tasks (one durable execution). Each Routine run re-resolves Agent, capabilities, secrets,
Runtime, Environment, and approvals. A persistent Environment is a separately visible,
explicitly authorized resource with cost/retention policy, not an automatic side effect of
Task resume.

## Product non-goals for Core

LiteCowork Core is not:
- a universal model router
- a cognitive memory engine
- an Office suite
- a browser-agent implementation
- a computer-use model
- a code intelligence platform
- a web-search engine
- a second plugin marketplace
- a general workflow-engine competitor

## Optional chat-first Coworkers — accepted target

A new ordinary conversation is available with zero Coworkers. The optional Coworkers section lists persistent assistants. Selecting one resumes the most recently active Coworker-owned chat; New chat starts another conversation and retains only authorized Coworker memory, not private native-agent transcripts. Quick Create requires a name and purpose and inherits eligible defaults. Customize may select native chief/model/effort, external worker profiles, knowledge, connections and standing responsibilities; every optional stage can be skipped. Connect apps, MCP, skills and local providers during setup OR while chatting, with no arbitrary configured integration-count limit. Consumer setup never forces per-operation permission checkboxes; consequential actions still follow Trust and scoped approval. A user-confirmed Responsibility can admit scheduled, event or bounded monitor work, while merely creating a Coworker or saving automatic memory never enables autonomous work. The right Workbench and truthful activity are available within a Coworker conversation. Implemented UI/API gaps remain unavailable, not simulated.


## Native-agent completeness guarantee

LiteCowork's product promise is **use your agent through a better cowork shell without
losing the agent you chose**. The native chief is not merely a backend model selector.

For every supported agent/version, LiteCowork SHOULD expose every safely supportable
user-facing capability that the AgentModule can qualify:

- installation and update lifecycle;
- native sign-in and account state;
- provider/API-key configuration through the correct native or secure adapter path;
- agent-owned model/session options;
- reasoning/effort/mode controls when supported;
- text/image/file/resource inputs;
- native slash commands, mentions/references and other input affordances;
- native configuration launch points;
- native extensions/subagents/harness behavior, clearly distinguished from
  LiteSPM-managed capabilities.

The product has **no standalone global Providers screen**. Agent-specific provider
configuration lives in the selected Agent's Registry panel. Local models are the one
separate model inventory because they are machine resources; they appear in an agent's
Model picker only when that AgentModule reports compatibility.

The Agent Registry is the single setup surface:

**discover -> install/update -> sign in/configure -> probe -> bind -> optionally set default**

None of those steps silently authorizes work. See AGENT-CONTROL.md and
archive/legacy-implementation-c7e9c05/UI.md.


## Connector breadth, learning, and unified-mode product contract

LiteCowork should feel integrated without becoming a workflow-engine clone. Highest-value
apps can have direct first-party ConnectorPackages; MCP/community packages extend breadth;
qualified Activepieces compatibility may add connectors; a future qualified
WorkflowBackendAdapter may invoke an existing external workflow as one bounded capability.
No production workflow adapter is currently present in the active source tree. None is mandatory.

Credential custody is visible. Supported desktop public-client flows keep tokens in local
SecretStore; a hosted auth provider is named when selected. Event delivery while the
machine sleeps is a separate relay/hosting concern.

Coworkers may improve from corrections, but learn means a typed durable change with
source/provenance. Private memory can be quiet under policy; behavioral instructions,
Skills, Routines, Automations and permissions use their own review owners.

The product remains one cowork shell. Code task is a convenience preset; users do not
enter a separate Cowork/Code universe and lose history, files, Workbench, Automations or
native agent controls when task type changes.

## V1 interaction and implementation crosswalk

The product promise and release boundary are translated into screens and journeys by [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md) and [V1-DESIGN-INDEX.md](V1-DESIGN-INDEX.md). The active source and qualification state are recorded separately in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md). Native agent controls, real browser/computer use, safe Task execution, professional Artifact workflows and end-to-end Automations must be qualified before being marketed as supported. A simple Conversation never requires Coworker creation.


## External workflow authoring boundary — 2026-11-06

The OpenBot source comparison demonstrates a visual graph editor plus agent tools for graph editing. n8n Assistant/MCP and Activepieces Build by Asking/MCP further demonstrate that provider-native AI authoring can produce a normal provider-owned graph that remains visually inspectable and manually editable. LiteCowork's product boundary remains: manual and provider-native AI authoring happen inside the connected provider; no native graph editor/runtime is added to LiteCowork Core. The target LiteCowork integration is to discover, validate, map, invoke and observe existing workflows through a future qualified WorkflowBackendAdapter as one bounded CapabilityInvocation inside a Task. That adapter is not implemented in the active source tree, so no workflow selection or invocation is currently served. Coworkers may draft LiteCowork Routines and map inputs; direct LiteCowork-agent graph mutation requires a future provider-specific authoring capability and explicit owner/security approval. Provider Code nodes remain provider-side execution and do not inherit LiteCowork Environment/Trust guarantees. See [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md) and [CONNECTORS-WORKFLOWS.md](CONNECTORS-WORKFLOWS.md) §10.

## Workspace experience revision - 2026-11-12

The requested product scope includes a live selected computer screen/window in the right panel, separately consented agent disclosure and input, and explicit manual takeover. A managed-browser viewport is a distinct view and does not fulfill the live-computer requirement. Runtime capture/input remains unavailable until its exact provider, OS permissions, revocation, input fencing and recovery qualify. The prior B-32 milestone deferral is historical; the requested design revision and its unresolved provider gates are recorded in [Workspace experience revision](WORKSPACE-EXPERIENCE-REVISION-2026-11.md) and Feature 008.

The current accepted navigation baseline exposes four workspace destinations and keeps secondary modules reachable without permanent sidebar rows. The 2026-11-14 universal-workspace revision proposes a smaller top level; see below. Conversation, interactive answer and expanded Canvas are presentation states in one application. Native registered components and deterministic calculations are the default; saved interactive state and conversational revisions require the separately governed Feature 016 revision. No global BYOK router, hosted account, hidden presentation model or architecture replacement is authorized by importing the supplied design essay.

### Universal workspace direction - 2026-11-14 (user-confirmed product north star; TARGET)

The product ambition is one adaptable, AI-centered workspace for varied work, not a collection of separate chat, coding, browser or document products. The proposed compact navigation has one workspace home, **New Chat**, **Projects**, and **Recents** as the frequent left-side actions; **Needs You** is a compact attention count and **Settings** sits at the bottom. A single **More** entry provides access to less frequent domain views such as Work, Coworkers, Library, Automations and Discover. These views remain available without occupying permanent top-level tabs. This is a user-directed product-level TARGET; the exact navigation is a UI proposal, and the accepted four-destination navigation remains the release contract until its review is recorded.

The conversation is the place to ask and direct work; a contextual right-side object workspace holds the files, diffs, agent status, browser/computer views, terminal sessions, and rendered or authored outputs relevant to that work. Users can switch among open objects or add one through a searchable **+ / New tab** chooser. The open objects, not a permanent list of every tool, shape the tab strip. Conversation and work history appear together in Recents and can be filtered by type without being split into separate Chat/Work/Code products. Projects group related material but do not create a new security boundary. The exact entry point for each secondary domain is contextual when possible and available through More otherwise.

“Supersede all” is the user-confirmed product north star: make LiteCowork the user's primary AI work environment and remove the need to bounce among separate AI chat, coding, browser, file and task products for supported everyday work. The supplied ChatGPT Work screenshots clarify the right side: it is a contextual object workspace, with open work objects in tabs and a searchable add/open action, alongside the conversation. The product should own the coherent workflow and user experience: one conversation-led starting point, shared project context, contextual live work objects, and finished outputs that can be reopened and continued. Native capabilities should grow to complete useful work in that environment; integrations extend user choice and reach rather than defining LiteCowork as a directory of specialist tools or a thin launcher. This is broader than a specialist chat or coding client. The north star is confirmed; the specific V1 navigation revision remains subject to the release contract and feature review gates below.

This ambition does not make every specialist capability a V1 promise. V1 does not claim to replace every desktop application, own native agents, or safely control every provider. Where a complete workflow depends on an external application or native harness, LiteCowork must say what remains outside its control and treat any handoff as an explicit boundary, not as completion inside LiteCowork. A view appears as available only when its owning domain, authorization, provider and recovery path exist. A tab is a view reference; selecting, restoring, or opening it never starts a process, grants access, begins capture, or dispatches agent work. Native-agent reasoning, sign-in, private transcripts and native tools remain with their harness. Actions outside LiteCowork mediation remain outside LiteCowork Trust and Effect guarantees.

The reference pattern is ChatGPT Work's contextual right-side tabs and object opening, as shown in the user's 2026-11-14 screenshots. Adopt the general workspace pattern, not its private implementation or exact controls. “Open with” a desktop application may be considered as an explicit, owner-reviewed handoff for an exact file/version; it must disclose that the edit occurs outside LiteCowork's mediation and must not imply that changes or permissions are tracked. This is not a V1 capability promise.
