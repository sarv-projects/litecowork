# Product Contract

## Promise

LiteCowork lets a user describe work once and have the Task survive agent replacement, process failure, device changes and capability changes while remaining understandable and controllable.

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
    provenance without creating separate product modes.
18. Presentation is derived from authorized projections. Transient streamed output is not
    durable until its ConversationMessage commits; UI frames cannot authorize or settle work.

## Primary navigation

```text
New session / Conversations
Coworkers (optional)
Needs You
Work
Automations
Library
Discover
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
implementation/UI.md.
