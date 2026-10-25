# Competitive Product Research (latest snapshot: 2026-10-25; earlier checks retained)

This is dated product research, not a normative competitor dependency. Product behavior
changes quickly; cited vendor pages are the evidence, while LiteCowork contracts are owned
by `ARCHITECTURE.md` and the domain documents.

## Earlier snapshot (2026-10-10): verified patterns and corrections

| Product | Verified behavior | LiteCowork implication |
|---|---|---|
| Claude Cowork | For Pro/Max, one Claude experience is rolling out; Team/Enterprise still retain separate chat and Cowork. A Scheduled page supports recurring and on-demand runs. Cloud work can continue while the computer is off, but work tied to local folders/apps runs locally and needs that computer; an active cloud session can reach local files/browser only through the connected Desktop app. Local MCP servers run in local desktop sessions, not cloud sessions. The cited page announces a Pro/Max change scheduled for 2026-10-06, the day after this snapshot; actual rollout and account eligibility remain time-sensitive. | Keep one composer; represent local-resource dependency separately from cloud continuation. Do not describe the announced migration as already complete or imply the local bridge is unnecessary. |
| ChatGPT Work | Chat and Work remain selectable experiences; chats appear together in Recents, while Codex remains separate. Cloud Work syncs across surfaces. Scheduled work supports one-time/recurring runs, change monitoring, and event triggers; approval-required actions can pause a run. Opt-in local work sync permits following the same conversation from web/mobile, but local steps still require the computer online and an active local turn does not automatically switch to cloud. Shared Team tasks use team service-account connections, saved instructions, and permissions, not the creator's personal memory/context. | Keep LiteCowork's one-composer direction, while preserving explicit Task/Routine ownership and placement. Runtime loss never teleports an Attempt; a later turn or replacement Attempt needs fresh placement, authorization, and environment checks. |
| OpenHands Agent Canvas | Browser client and execution backend are separate; backends include local, Docker, VM, Modal, and cloud. A local npm/npx Canvas process stops when its terminal process closes; Docker, VM, and cloud backends continue until stopped. ACP agents can be spawned by the selected backend. Official material also documents GitHub Action issue/PR triggers and API conversation trigger types including Slack/Jira/Linear; this pass did not verify a general cron/polling automation service. | Keep Operator separate from daemon/backend and preserve agent ownership. Backend lifetime, not browser-window lifetime, determines whether work continues. Treat event automation as integration-specific unless a broader scheduler contract is documented. |
| OpenClaw | Gateway owns durable schedules and run history; startup recovery applies explicit catch-up/defer rules, uses receipts, and avoids blindly replaying interrupted external delivery. OS service managers launch the Gateway. | Scheduler belongs to `litecoworkd`; OS manager starts the daemon. Misfire policy and Effect reconciliation must be explicit. |
| Agent Zero | Projects group instructions, files, memory/secrets, and project-scoped scheduled tasks. | Workspace identity needs explicit instructions, persistent roots, and credential references; LiteCowork keeps secrets separately scoped. |
| Manus Cloud Computer | A persistent cloud VM retains files, installed tools, and processes between sessions, distinct from a temporary task sandbox. | Environment lifetime can outlive a Task, but persistent compute must be explicit, costed, scoped, and reauthorized per Attempt. |
| Cursor Automations | One Automation can have multiple triggers; Cloud Agent runs can be started by schedules and supported product events. Cursor also documents desktop sharing/takeover on supported workers. | Normalize multiple triggers and define human control epochs; do not copy Cursor's trigger catalog wholesale. |
| Goose | Recipes package reusable instructions, extensions, parameters, and prompts and can be launched headlessly; Goose also documents MCP Apps as an interactive extension surface. | A user-facing Routine is useful as reusable “what”; keep it distinct from Skill “how” and Automation “when.” |
| GitHub Copilot | Repository/personal hooks execute shell commands around agent session and tool events in Copilot CLI and cloud agent. | Worker-native hooks remain worker-owned; LiteCowork host events cannot claim equivalent native interception. |
| Devin | The official indexed dependency-update template describes an RRULE schedule, reusable prompt and invocation limits. The direct page returned 404 on this pass; the indexed material may be stale. | Supports the reusable-work/trigger distinction as historical evidence; do not depend on a current Devin wire contract. |
| OpenWorker | Its public repository describes a local desktop coworker with a local agent server and standing automations; its scheduler tools gate automation creation behind an approval card. The inspected sources do not establish independent OS-service durability after the UI/server exits. | Treat local-first automations as a real use case, but specify daemon lifecycle and recovery instead of inferring them from a desktop scheduler. |
| OpenCowork (OpenCoworkAI) | A separate local desktop agent project. Its public roadmap describes a cron-like timer scheduler; an issue proposes condition-based polling as a future enhancement, so that monitoring behavior is not treated as shipped. | Similar names do not identify one product. Keep competitor claims tied to the specific repository and shipped behavior. |
| Zapier and n8n | Both provide explicit workflow runtimes, triggers, history, and human review. Zapier documents scheduled triggers, approval pauses, and trigger deduplication by source item ID; downstream action duplication depends on the connected app. n8n requires a published workflow and uses workflow/instance timezone settings for schedule triggers. | Borrow durable trigger identity, explicit timezone and review/approval semantics. Keep the deterministic graph engine external; do not imply deduplicating trigger delivery makes every downstream effect exactly-once. |

## Architectural synthesis

The recurring pattern is a persistent, lightweight control plane with durable schedules,
state, and history that starts worker processes only when work needs them. This is a
product-design synthesis, not a claim that every competitor implements the same internal
architecture.

LiteCowork adopts the lifecycle distinction as an invariant:

```text
Operator may disappear
litecoworkd owns durable coordination and trigger state
workers/services are demand-started and released when their references settle
```

Competitor feature lists are evidence for user needs, not a mandate to copy their
implementation. LiteCowork keeps its own invariants for process ownership, local resource
availability, no duplicate Effects, separate TriggerHost/ExecutionHost, and verified
Task outcomes.

## Primary sources checked

- Anthropic, [Use Claude Cowork on web, desktop, and mobile](https://support.claude.com/en/articles/15520349-use-claude-cowork-on-web-desktop-and-mobile), [Claude Cowork architecture overview](https://support.claude.com/en/articles/14479288-claude-cowork-architecture-overview), [Schedule recurring tasks in Claude Cowork](https://support.claude.com/en/articles/13854387-schedule-recurring-tasks-in-claude-cowork), and [Claude Cowork and chat are one Claude](https://support.claude.com/en/articles/16761823-claude-cowork-and-chat-are-one-claude).
- OpenAI, [Agent Security and local work sync](https://help.openai.com/en/articles/20001548-agent-security-and-local-work-sync-in-chatgpt), [ChatGPT Work and Codex](https://help.openai.com/en/articles/20001275-chatgpt-work-and-codex), [Scheduled tasks in ChatGPT](https://help.openai.com/en/articles/10291617-scheduled-tasks-in-chatgpt), and [Creating and managing team tasks](https://help.openai.com/en/articles/20001540-creating-and-managing-team-tasks-in-chatgpt).
- OpenHands, [Agent Canvas overview](https://github.com/OpenHands/docs/blob/main/openhands/usage/agent-canvas/overview.mdx), [ACP agents](https://github.com/OpenHands/docs/blob/main/openhands/usage/agent-canvas/acp-agents.mdx), [GitHub Action](https://docs.openhands.dev/openhands/usage/run-openhands/github-action), and [conversation API](https://docs.openhands.dev/api-reference/get-conversation).
- OpenClaw, [How automations work](https://docs.openclaw.ai/automation/cron-jobs/how-it-works), [Gateway runbook](https://docs.openclaw.ai/gateway), and [Automation schedules](https://docs.openclaw.ai/automation/cron-jobs/schedules).
- Agent Zero, [Usage guide](https://github.com/agent0ai/agent-zero/blob/main/docs/guides/usage.md).
- Manus, [What is the Cloud Computer?](https://help.manus.im/en/articles/15392111-what-is-the-cloud-computer).
- Cursor, [Automations](https://cursor.com/docs/cloud-agent/automations), [Capabilities](https://cursor.com/docs/cloud-agent/capabilities), and [Computer use and desktop sharing](https://prod.cursor.com/docs/cloud-agent/self-hosted/computer-use).
- Goose, [Recipe reference](https://github.com/aaif-goose/goose/blob/main/documentation/docs/guides/recipes/recipe-reference.md), [Using extensions](https://github.com/aaif-goose/goose/blob/main/documentation/docs/getting-started/using-extensions.md), and [Goose architecture](https://github.com/aaif-goose/goose/blob/main/documentation/docs/goose-architecture/goose-architecture.md).
- OpenWorker, [project overview](https://github.com/andrewyng/openworker) and [approval-gated scheduling tools](https://github.com/andrewyng/openworker/blob/main/coworker/automation/tools.py).
- OpenCoworkAI, [project overview](https://github.com/OpenCoworkAI/open-cowork) and [scheduler/condition-polling issue](https://github.com/OpenCoworkAI/open-cowork/issues/304).
- Zapier, [Schedule triggers](https://help.zapier.com/hc/en-us/articles/8496288648461-Schedule-Zaps-to-run-at-specific-intervals), [Human in the Loop approval](https://help.zapier.com/hc/en-us/articles/38731463206029-Request-approval-to-keep-your-workflow-running-with-Human-in-the-Loop), and [trigger deduplication](https://help.zapier.com/hc/en-us/articles/8496260269965-How-Zapier-handles-duplicate-data-in-Zap-workflows).
- n8n, [Schedule Trigger](https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.scheduletrigger/).

Sources were checked on 2026-10-04 and selectively rechecked on 2026-10-05, including the
current Agent Zero project/task guide, Cursor Automation triggers, OpenHands backend
lifecycle, OpenClaw run recovery, and Manus Cloud Computer behavior. Rollouts, plans, and
account/region availability are not generalized beyond what each cited source states.
Third-party/community reports are not used as authoritative facts in this snapshot.

### Additional source checks

- GitHub, [About hooks for Copilot](https://docs.github.com/en/copilot/concepts/agents/hooks).
- Devin, [Weekly Dependency Updates template](https://docs.devin.ai/automation-templates/weekly-dependency-updates): search-index evidence only on this pass; direct fetch returned 404.

OpenWorker and OpenCowork names refer to separate projects; their local UI/server startup
patterns are not evidence of independent OS-service durability. Unverified community
claims and the assertion that no competitor combines every LiteCowork feature are not
architecture facts. Zapier/n8n inspire trigger/history/review semantics, while their
workflow execution engines remain external integrations.

## Coworker autonomy reference set (reviewed target; implementation claims excluded)

The accepted consumer and open-source feature comparison is in [Coworker target](COWORKERS-TARGET.md), section 12, with source links and the distinction between claimed experience and qualified behavior. The engineering decisions are optional chat-first assistants, reviewed standing responsibilities, bounded proactive observation, provider-native agent reasoning, secure reuse of app/MCP connections, and local daemon/background limitations without cloud equivalence claims. Do not treat marketing descriptions of Dots, Grok Bot, Muse, Claude Cowork, Kimi Work, OpenClaw, Eigent or OpenWork as independent reliability test evidence.


## Desktop competitor deep benchmark — 2026-10-10

This pass intentionally compares the DESKTOP PRODUCT EXPERIENCE and user-visible capabilities. Cloud-only scale is noted only when it changes what a desktop control means; LiteCowork must not claim cloud-like availability for an offline local machine. Marketing claims are separated from verified repository/product documentation.

### Capability ladder

| Level | Capability | LiteCowork target |
|---|---|---|
| L0 | Ordinary conversation and persistent specialist identity | Ordinary chat with zero Coworkers; optional persistent Coworkers |
| L1 | Local files/workspace and generated documents | Authorized Resource roots, immutable Artifacts, Workbench |
| L2 | Browser, terminal, desktop/computer use | Qualified Environment providers in one Workbench shell |
| L3 | Connected apps, MCP, skills/plugins | Unlimited CONFIGURED catalog; bounded lazy discovery/activation |
| L4 | Permissions, approvals, isolation, audit | Trust + grants + leases + Effect reconciliation, consumer-friendly UI |
| L5 | Memory and knowledge continuity | Quiet scoped private memory plus source-backed Knowledge |
| L6 | Reusable skills/routines and schedules | Routine + Automation; reviewed StandingResponsibility grouping |
| L7 | Event/condition-driven proactivity and background work | Deterministic filters, durable Task admission, local daemon availability |
| L8 | Multi-agent/coworker collaboration and handoff | Native subagents plus explicit external worker/Coworker handoffs |
| L9 | Verification/recovery and rich work product UI | Evidence-bound completion, Needs You, common Workbench |

### OpenWork (different-ai/openwork) — local-first open source

**Low level / runtime:** Desktop app on macOS/Windows/Linux, powered by OpenCode. Local files remain local in desktop mode; cloud/control-plane use is optional. It supports model/provider choice, local models, skills/plugins/MCP servers, local browser automation, scheduled automations, connected services, and artifacts. The OpenWork MCP gateway deliberately collapses a large capability inventory into `search_capabilities` and `execute_capability`, a strong validation of LiteCowork's progressive-discovery design.

**Mid level / product:** Sessions/workspaces are chat-first. Connected tools can be used by workflows and artifacts; remote channel work can report back into normal task chats. Its design system explicitly says chat is the home surface, advanced state is progressive disclosure, tool activity is compact, consequential consent names action/data/risk, and a tool result must NOT auto-open a side panel or steal focus.

**UI:** Sidebar sessions + New task, central composer, model selector, built-in browser/artifact surfaces, settings for providers/extensions/connections. Design guidance prefers one focal surface, compact rows, no card-in-card, inline tool steps collapsed after completion, and side-panel escalation only when the user chooses iteration/editing.

**Take for LiteCowork:** Preserve chat-first layout, capability search/describe/execute, no auto-opening Workbench on background result, compact truthful tool activity, and blocked-with-reason states. LiteCowork should exceed OpenWork on durable Task/Effect verification, interchangeable full native harnesses, scoped persistent Coworkers, and recovery semantics.

Sources: [OpenWork repository](https://github.com/different-ai/openwork), [OpenWork DESIGN.md](https://github.com/different-ai/openwork/blob/dev/DESIGN.md), [releases](https://github.com/different-ai/openwork/releases).

### Open Cowork (OpenCoworkAI/open-cowork) — local desktop open source

**Low level / runtime:** Windows/macOS installers, workspace-scoped file access, WSL2/Lima VM isolation where available, multi-model APIs, Skills, MCP connectors, browser/GUI computer use, rich file/image input, and remote control through Feishu/Slack. The published roadmap still marks some capabilities such as memory optimization/computer-use maturation as ongoing, so roadmap items must not be treated as shipped reliability.

**Mid level / product:** Strong emphasis on document creation (PPTX/DOCX/XLSX/PDF), local workspace automation and explicit permission dialogs. The code layout exposes a ChatView, Sidebar, ContextPanel and TracePanel, which is useful evidence for the common desktop pattern: chat center, scoped context/trace as supporting panes.

**UI:** Traditional agent desktop: navigation sidebar, central chat, file/context panel, permission dialog, real-time trace/tool-call display, settings/config modal. It makes technical execution more visible than a consumer assistant, which is useful for debugging but too noisy as LiteCowork's default.

**Take for LiteCowork:** Keep VM/process isolation, document skills, drag/drop rich input and visible real tool execution. Move raw trace into optional Inspector/Technical details rather than the default. Do not require API-provider configuration for Coworker users when an existing native agent subscription/harness is already configured.

Sources: [Open Cowork repository](https://github.com/OpenCoworkAI/open-cowork), [README/component layout](https://github.com/OpenCoworkAI/open-cowork/blob/main/readme.md).

### Claude Cowork — desktop-first delegated knowledge work

**Low level / runtime:** Cowork runs on desktop and can read/write only folders the user explicitly grants. Anthropic documents a local VM/sandbox architecture with mounted workspace, host credential storage, and read-only/read-write/read-write-no-delete mount modes. Local MCP servers moved outside the VM for reliability and local-process access; remote MCP remains external. Cowork can use connectors and Skills and perform multi-step work across local files and connected apps.

**Mid level / product:** The product distinction is delegation rather than chat-only work: hand over multi-file/multi-app projects, let them run for minutes/hours, then review output. Anthropic also uses plugins/workflow packs and Microsoft 365 add-ins; sending/posting/paying remains behind approval in their small-business positioning.

**UI:** Desktop task-oriented experience with model selection, connectors, granted folders, long-running work/results, file/document outputs and review. The strongest UI/security lesson is that nontechnical users should not be forced to judge shell commands; absolute sandbox/mount boundaries are preferable to endless command-level prompts.

**Take for LiteCowork:** Add explicit root mount modes (`read only`, `read/write`, `read/write/no delete`) as a future Environment/access option; no background drive indexing; keep credentials host-side; use high-level approvals for meaningful external actions rather than showing raw command permission walls. Preserve ordinary chat alongside delegated durable Tasks instead of making every message a project.

Sources: [Anthropic containment architecture](https://www.anthropic.com/engineering/how-we-contain-claude), [finance agents / Cowork desktop workflows](https://www.anthropic.com/news/finance-agents), [Claude for Small Business](https://www.anthropic.com/news/claude-for-small-business).

### OpenAI dots — persistent named assistant with optional local-computer bridge

**Low level / desktop surface:** A dot has its own persistent cloud computer and browser. Optional local-computer access is a separate bridge, starts off, and requires the ChatGPT desktop app to remain open while the host is online. Local Work/Codex tasks run separately from cloud tasks. Connecting a computer to Codex or enabling Work Sync alone does not grant a dot local access; local skills also require a connected computer. Connected apps and the cloud browser have their own account/session boundaries.

**Mid/high level:** A dot is a named persistent agent with memory, proactive connected-app review, scheduled tasks, background suggestions, customizable rules and pause/reset. Desktop profile exposes activity grouped as In progress, Scheduled and Completed and can open the dot's computer. Custom rules offer action modes roughly equivalent to act without asking, pre-approved, ask first, or hand off.

**UI:** Conversation remains central. Profile/customize areas carry Plugins, Memory, Custom rules, recent activity and computer access rather than crowding the composer. Scheduled/proactive updates come back into the conversation. Local computer tasks show separately in the sidebar.

**Take for LiteCowork:** Keep Coworker profile secondary to chat; use compact In progress/Scheduled/Completed summaries; separate connection availability from approval rules; support Pause at identity level. LiteCowork should make memory more inspectable/revocable per record than a reset-only experience and should not imply local work can continue while the machine is off.

Sources: [Getting started with your dot](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot), [Introducing dots](https://openai.com/index/introducing-dots/), [Dots privacy/security](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs).

### Meta Muse — conversation + goals + proactive suggestions

**Low level / desktop surface:** Meta documents two surfaces: a Muse for Mac app that can work with local desktop files, apps and browser tabs with permission, and a persistent dedicated Muse Secure VM with its own browser for cloud work. Its product pages also list iOS, Android and web. The VM uses a separate Sentinel agent to gate egress; credentials are held in secure storage rather than exposed to Muse. Treat the local Mac bridge and hosted VM as separate execution loci and consent scopes.

**Mid/high level:** Users give Muse goals/tasks; it builds plans, tracks progress, suggests ideas proactively, monitors things in the background, connects to email/calendar/Instagram and other services, and can create tools when a needed tool does not exist. Critical actions such as sends/purchases are reviewable and the product advertises an audit trail of completed/planned actions.

**UI:** Conversation-first across Mac/mobile/WhatsApp, backed by explicit Goals/Ideas tracking, approval cards and a shared browser/computer surface that a human can intervene in. Its product framing reduces workflow-builder exposure: the assistant carries the plan, while the user sees goals, ideas, approvals and outcomes.

**Take for LiteCowork:** Keep Goals and Suggestions/Ideas as visible but non-authoritative supporting surfaces, add concise audit/action history, and make Take control obvious for Browser/Computer. Generated tools remain proposals that pass LiteSPM/Skill review rather than self-installing.

Sources: [Muse product page](https://ai.meta.com/muse/), [Muse download and platform page](https://ai.meta.com/muse/download/), [Meta introduction and security architecture](https://about.fb.com/news/2026/09/introducing-muse-personal-ai-agent/).

### Grok Bot — strongest current persistent-bot/routine desktop UX benchmark

**Low level / desktop surface:** Grok Bot has a persistent cloud computer shared by the user's Bots (files, browser sessions and app logins), plus a separate optional local-computer execution path through its desktop app. Local command/file access has its own per-computer policy and asks for approval by default; cloud-computer approvals do not grant local access. Bot model selection is product-managed, not exposed as a customer-facing picker.

**Mid level:** Bots keep role-specific memory, can share files/context through explicit chats/groups, own reusable Skills and Routines, use connectors/logins, and expose a Library tab with shared pages/files/links. A task asking for an unconnected app can present connection UI and resumes automatically after sign-in. Teach-a-task can record a demonstrated browser workflow and generate a draft skill for review/testing.

**High level:** Routines can run on schedules or supported events; Test run performs real work and is explicitly risky. Routine UI supports pause/resume/test/edit/delete and run history. Primary Bot proactivity can spot work to offer. Approval cards quote the Bot/instruction; Custom Rules and Auto Review govern unattended actions. Long jobs, failure recovery, notifications and computer takeover are explicit product concepts.

**UI:** Bot roster/sidebar, primary Bot star, per-Bot details with Routines and Library tabs, Marketplace shortcut, chat approval cards, full-screen computer view/Take over, routine context actions, per-Bot notification switches and searchable prior work. This is the closest competitor to LiteCowork's named Coworker concept.

**Take for LiteCowork:** Add `Test responsibility/routine`, recent run history, Library/activity tab within Coworker, task-time Connect-and-resume, optional Teach workflow later, clear primary/featured Coworker without making it mandatory, and strong per-Coworker notification/routine controls. Unlike Grok Bot's shared cloud computer, LiteCowork must retain exact local Environment scopes and cannot let Coworkers implicitly share files/logins.

Sources: [Grok Bot overview](https://docs.x.ai/grok-bot/overview), [computer and apps](https://docs.x.ai/grok-bot/computer-and-apps), [Skills and routines](https://docs.x.ai/grok-bot/skills-routines-and-automations), [Bots](https://docs.x.ai/grok-bot/bots), [approvals/security](https://docs.x.ai/grok-bot/approvals-security-and-privacy), [security FAQ](https://docs.x.ai/grok-bot/security-faq).

### Consolidated adoption decisions

1. **Chat is the home surface.** OpenWork, Dots, Muse and Grok Bot all reinforce that configuration should not replace conversation.
2. **Right Workbench is user-controlled.** Adopt a calm Start launcher when open; do not auto-open from background tool results. Browser/Computer/Terminal/Artifacts share one shell but distinct authority.
3. **Persistent specialist identities are optional.** Dots/Grok/Muse validate named assistants; Claude/OpenWork validate powerful one-off sessions. LiteCowork supports both instead of forcing a persona.
4. **Connections are huge inventories, not huge prompts.** OpenWork's search/execute gateway and Claude's MCP/Skill loading support lazy discovery. Keep configured count uncapped while activation/context remains bounded.
5. **Task-time connection repair should resume work.** Grok Bot's connect-in-chat/resume pattern is the target UX, gated by LiteCowork's durable Task/Effect checks.
6. **Routines need Test, Run history, pause/resume, edit and explicit owner.** Grok Bot is the clearest benchmark; LiteCowork adds stronger occurrence/Task/Effect identity.
7. **Automatic memory and proactive work remain independent.** Dots/Muse/Grok validate usefulness, but LiteCowork keeps source eligibility and action authority separate.
8. **Nontechnical security uses boundaries, not command spam.** Claude's local VM/mount model is the benchmark; advanced users can inspect technical details without making them mandatory approvals.
9. **Trace is optional.** OpenCowork's TracePanel is useful for technical users; LiteCowork keeps it behind Inspector/Technical details and foregrounds verified outcomes.
10. **Local desktop truth beats cloud imitation.** Cloud-backed products may work with laptop closed; LiteCowork desktop V1 says asleep/off means unavailable and reconciles on wake.

### UI benchmark summary

| Product | Primary surface | Secondary surfaces worth copying selectively | Avoid copying |
|---|---|---|---|
| OpenWork | Chat/session | Side panel for artifacts/apps, built-in browser, compact tool rail | Auto-open panels, verbose setup |
| Open Cowork | Chat | Context panel, trace, file/document tooling | Technical trace as default for nontechnical users |
| Claude Cowork | Delegated task + review | Explicit workspace roots, files/docs/connectors, model choice | Shell-command approval burden |
| Dots | Persistent assistant chat | Profile activity, plugins, memory, custom rules, computer | Reset-only coarse memory management |
| Muse | Conversation | Goals/ideas, approvals, audit trail, shared browser/computer | Self-installing generated tools without review |
| Grok Bot | Bot chat | Bot roster, Routines, Library, Marketplace, computer takeover, approval cards | Implicit shared-computer scope between Coworkers |

This benchmark changes LiteCowork's acceptance criteria and UI design; it does not prove those competitor capabilities are reliable under our workloads. Open-source README/roadmap claims remain claims until reproduced. Closed products' cloud availability is explicitly not used as evidence for local desktop durability.


## 2026-10-10 deep dive — learning coworkers, workflow backends, and visual orchestration

A source-level review of Agenta, Eigent/CAMEL, Activepieces, n8n, Nango, Arcade and
Composio changed implementation detail but not LiteCowork's control-plane ownership.
Exact pinned commits/files/licenses are recorded in
[Reference Implementations](REFERENCE-IMPLEMENTATIONS.md).

- **Agenta:** borrow the user-facing unification of scheduled and event-driven work,
  explicit run history, run-to-session continuation, and the separation between memory,
  skills, instructions and future triggers. LiteCowork keeps stricter revision/Trust
  control: a model proposes persistent changes; it does not receive a generic arbitrary
  configuration-commit authority.
- **Eigent/CAMEL:** borrow the calm visual desktop shell, coherent multi-agent activity
  timeline, decomposition/retry/replan test cases, and conditional browser/terminal/MCP
  toolkit lifecycle ideas. CAMEL does not become LiteCowork's planner or Task authority;
  native Codex/Claude/OpenCode/etc. remain first-class AgentBindings under the existing
  Plan/Step/Attempt model.
- **Activepieces:** investigate a qualified community-Piece compatibility provider and
  support a user-owned Activepieces deployment as a workflow backend.
- **n8n:** support a user-owned instance through a bounded WorkflowBackendAdapter. Do not
  embed or white-label the engine by default.
- **Nango / Arcade / Composio:** optional Auth/Tool/Connector backends only.
- **Product IA:** do not fork the product into Cowork versus Code. Code is a launch preset
  and Task intent inside the same Conversation/Task architecture.
- **Credentials and events:** local provider credentials and remote event reachability are
  separate concerns. A future EventRelay can carry minimal signed wake/event metadata
  without becoming the token vault.

Normative targets:
[Connectors & Workflows](CONNECTORS-WORKFLOWS.md) and
[Learning](LEARNING.md).

## V1 detailed design integration — 2026-10-12

Use V1-DESIGN-INDEX.md and implementation/V1-DESIGN-COVERAGE.md to distinguish current code gaps from target competitor parity: native / @ and session/model semantics; managed Chrome and optional explicit existing-session attach; verified work/cowork and rendering. Claims require an exact-date vendor source and own real feature tests; do not infer account availability or planned V1 parity from a marketing screenshot.

## 2026-10-14 Manus 2.0 and local UI comparison

The owner-provided `manusui/` folder contains 20 screenshots. They show a task-centered
workspace with visible agent activity, a Library and Creations area, separate connector
and Skills catalogs, account connection flows, and Automation setup for schedules and
triggers. Screenshot dates, account tier, rollout status, and product version are not
known, so these are visual references rather than proof that every shown path is generally
available.

Manus's current product pages describe Manus Studio as a shared workspace for creating and
editing websites, videos, games, and other project outputs alongside local files and tools
([Manus 2.0](https://manus.im/en/blog/introducing-manus-2-0),
[Desktop](https://manus.im/desktop?locale=en)). Its connector catalog exposes app
connectors and custom MCP/API setup, while its Skills page presents reusable abilities as a
separate surface ([Connectors](https://www.manus.com/features/connectors),
[Skills](https://manus.im/features/skills)). The Automation page describes scheduled and
event-triggered work, test runs, confirmation settings, updates, and failed-run repair
([Automations](https://www.manus.com/features/automations)). Plan Mode adds a user-editable
review step before execution ([Plan Mode](https://manus.im/blog/manus-plan-mode)). Its
Cloud Browser documentation describes live browser visibility, account sessions, and
user takeover during verification; its Wide Research help page describes automatic
parallel decomposition, paid-user availability, and a vendor claim of up to 20 concurrent
subtasks ([Cloud Browser](https://manus.im/docs/features/cloud-browser),
[Wide Research](https://help.manus.im/en/articles/11960169-what-is-wide-research)). These
are vendor claims, not independent reliability or performance measurements. Manus's
reported 2.0 performance numbers compare with its own prior system, not LiteCowork.

Against the current LiteCowork source, the main deficit is an unfinished end-to-end work
flow rather than an unusable visual shell. LiteCowork has a durable Task surface and real
Resource/Artifact browsing, but Home explicitly says saved Tasks are not sent to an agent,
Conversation sending is disabled until native turn execution is connected, and there is no
general Discover/connector/Skills catalog. Needs You is a real read-only projection of
persisted Approvals, UserRequests, and Task blockers. Task detail also has a live-updating,
read-only Outcome and activity panel derived from saved Task records. That panel can show
persisted progress and outputs; it does not show a native agent actively executing or prove
that a Task is running. Automation definitions can include schedules, but the desktop
Runtime does not yet host or execute recurring schedules. The UI uses a light color scheme
and its navigation is mostly ungrouped, although Settings is grouped under Manage; the
browser harness shows card surfaces, so “flat” would overstate the visual evidence. These
screenshots are not packaged Tauri acceptance evidence. See `implementation/UI.md` and
`docs/EXPERIENCE.md` for the product target.

Priority from this comparison: first complete native Conversation/Task execution and
connect its real Attempt progress, controls, verification, and recovery to Task detail.
Task detail already links committed Task outputs to the Workbench; this is a source-backed
flow, while Attempt-level Evidence and verification remain absent. Next expose only
qualified connectors and Skills with clear installed/connected/assigned/granted/ready
states; finish truthful Automation schedule hosting and run history; then refine navigation
grouping, icons, theme, and empty/loading/error states. Do not copy Manus's connection
model or terminal permission policy without review: its launch post and Help Center
currently describe different command-approval defaults
([launch post](https://manus.im/blog/manus-my-computer-desktop),
[Help Center](https://help.manus.im/en/articles/14178443-what-is-the-my-computer-feature-capable-of)).
LiteCowork's Trust and local authorization contracts remain authoritative.

The impression that Manus is quicker to understand comes from the connected task/activity
surface and visible setup paths; no comparative timed study or packaged-build usability
test was run.

## 2026-10-15 competitor recheck and LiteCowork priorities

Current official materials reinforce that the competitive bar is a single, outcome-first
entry into ordinary questions and larger work. Anthropic is gradually merging Claude chat
and Cowork into one conversation, where the system can answer a quick question or produce
editable work, while cloud work can continue away from the computer and local file access
depends on Desktop ([unified Claude experience](https://support.claude.com/en/articles/16761823-claude-cowork-and-chat-are-one-claude)). ChatGPT Work keeps Chat and Work in one desktop history and supports projects, reviewable progress, approvals, schedules and eligible event triggers; local Work runs on the computer, but messages and task context may still be stored in the cloud ([ChatGPT Work and Codex](https://help.openai.com/en/articles/20001275-chatgpt-work-and-codex)).

The desktop/work competitors also converge on persistent context and visible results. Grok
Bot offers named Bots, parallel work, routines and a shared cloud computer that continues
when the user closes the app; access to the user's local computer is a separate permission
([Grok Bot overview](https://docs.x.ai/grok-bot/overview), [approvals and local-computer
access](https://docs.x.ai/grok-bot/approvals-security-and-privacy)). Kimi Work is a Mac and
Windows local-agent beta with goal-driven work, parallel agents, Skills, plugins, widgets,
and a dashboard ([Kimi Work overview](https://www.kimi.com/en/help/kimi-work/overview)).
Manus combines cloud research and production, local Desktop computer access, connectors,
projects, and schedule/event-triggered Automations with run review ([Manus Desktop](https://manus.im/desktop?locale=en), [Automations](https://www.manus.im/features/automations), [Connectors](https://www.manus.im/features/connectors)). These are vendor-described product behaviors and availability remains account/platform dependent; they are not comparative reliability measurements.

### What LiteCowork must deliver to compete

1. **A useful first answer in one conversation.** New session should be the obvious primary
   action. The same composer should support quick questions and requests for larger work;
   selecting a Coworker should remain optional. This requires a real, qualified native
   Conversation turn and must not be faked with a saved Task or simulated response
   ([E03-S02](../implementation/epics/E03.md), [Native Composer LLD](NATIVE-COMPOSER-LLD.md)).
2. **Visible progress and steerable work.** A user should see what was accepted, what is
   running, where it is waiting for them, and what failed, then resume from durable state.
   Task/Attempt, approval, stop, retry, and recovery states must come from Core and the
   native provider, not decorative activity.
3. **Deliverables that are easy to inspect and continue editing.** Make completed files,
   source references, versions, and follow-up actions easy to find in the Workbench. Rich
   and interactive output must remain inside the approved renderer boundary; do not run
   competitor-style generated HTML as trusted LiteCowork UI.
4. **Useful local context with visible boundaries.** Add/focus on a chosen Workspace and
   selected files in everyday language. Explain exactly which local data is available,
   what will be sent to an external provider, and when the device must remain awake. Keep
   exact identity, revision, authorization, Effect, and Evidence details inspectable for
   advanced users.
5. **Breadth only when it works end to end.** Discover, connectors, Skills, browser work,
   recurring responsibilities, and proactive suggestions are important expectations, but
   each must have its actual provider, permission lifecycle, recovery, and trustworthy UI
   before it appears enabled.

The current source's largest competitive deficit is still real execution: Home explicitly
says a saved Task is not sent to an agent, Conversation Send is disabled, and schedule
hosting and a served Discover catalog remain absent. Therefore implementation order stays
native Conversation/Task execution and evidence first, followed by the accepted chat-first
shell and reliable Workbench outcomes; provider breadth and visual polish cannot compensate
for a nonfunctional primary action. LiteCowork can differentiate through local-first
ownership, explicit Workspace and file scope, native-agent parity, and durable trust/effect
records, but those are product claims only where the actual provider path enforces them.

## 2026-10-24 deep product and repository audit

This dated addendum supersedes earlier snapshots where rollout, packaging, or capability
claims differ. It separates official product surfaces from open-source repository
descriptions and from LiteCowork design requirements. Vendor pages establish what a
vendor presents or documents; they do not establish independent quality, latency, safety,
or reliability parity.

### Products and desktop-relevant capabilities

| Product | Surface and execution model documented in this audit | Boundary and LiteCowork implication |
|---|---|---|
| **Grok Bot** | X describes a persistent computer-use agent with its own cloud computer, Bot identity, goals/routines, task handoff/collaboration, approval/takeover, and desktop/mobile access. Its FAQ and computer-use docs describe distinct cloud and local-computer paths, including an Ask-every-time default for local control. Each Bot has a screen; the shared Bot environment means files, browser state, and credentials are not automatically isolated by Bot identity. | Treat identity, screen, account, computer, and permission scope as separate concepts. Make local-vs-cloud execution visible, require explicit local control, and never imply that separate worker profiles form a security boundary. A local V1 Task cannot promise cloud continuation. |
| **OpenAI Dots** | OpenAI presents Dots as persistent cloud agents with their own cloud computer, saved context/preferences, integrations, and cross-surface access through ChatGPT, desktop, Slack/Teams, and supported voice/call entry points. The launch documentation describes a distinct cloud-agent experience and separately identifies Codex environments and local computer connection. | Do not collapse Dots, ChatGPT Work, Codex, and a native OpenAI model into one “agent” route. A local computer connection is a placement dependency, not cloud execution. Model entitlement, connected account, local environment, and agent identity must remain distinct in LiteCowork. |
| **Claude Cowork / unified Claude** | Anthropic documents Cowork in the Claude experience across web, desktop, and mobile, with availability/rollout depending on account. Its desktop surface supports local files and integrations; cloud tasks can continue separately, while a local-folder/app task depends on the desktop bridge remaining available. Artifacts include documents, slides, designs, dashboards, and interactive experiences. Computer use is documented as a controlled capability. | The strongest comparison is the finished cowork experience: one conversation, visible work progress, reviewable outputs, and a useful artifact surface. Preserve the exact Claude Code/native harness when selected; do not emulate its subscription or pass Claude credentials to another worker. Keep local resource availability distinct from cloud continuation. |
| **ChatGPT Work and Dots** | The official materials checked describe ChatGPT Work as the user-facing work experience and Dots as a distinct persistent agent product. Work supports cloud work, scheduled work and an opt-in local-work path; local actions still depend on an available connected computer. Codex remains a distinct coding-agent surface. | “ChatGPT Cowork” is not treated as a separately verified official product name in this snapshot. Compare against the actual Work, Dots, and Codex surfaces separately. LiteCowork should make ordinary cowork tasks feel first-class without labeling the whole product as a coding-agent UI. |
| **Meta Muse** | Meta presents Muse as a personal agent using a secure virtual environment, browser/app access, goals, memory, scoped permissions, and an audit trail, with Muse app, WhatsApp, web, and staged regional/device availability described in its rollout material. | The important desktop pattern is a persistent objective with an inspectable action history and clear permission scope. Do not overstate region/platform availability or assume an agent's VM is equivalent to a user's local desktop. |
| **Microsoft 365 Copilot Cowork** | Microsoft documents Cowork for eligible Microsoft 365 Copilot tenants, with task creation across Microsoft 365 content/apps, approvals and enterprise governance. Published model choices are tenant/region/plan dependent; administrators separately govern model families and feature access. | A model picker is a qualified route catalog, not a promise that every account can use every model. Surface the actual signed-in account, plan/tenant eligibility, app permissions, and available model routes before execution. |
| **Eigent** | The public repository describes an Electron/React desktop client, a local FastAPI service, CAMEL-based multi-agent orchestration, model/provider configuration, MCP/skills, and browser/terminal work. Repository claims and release status can change; source review is not a production qualification. | Its architecture is evidence that desktop UI and an agent orchestration/runtime boundary can be separate. LiteCowork should support provider-native lead agents plus explicit host-managed workers, but retain its own admission, permission, effect, and verification authorities. |
| **OpenWork** | The public repository describes an Electron cowork surface built around OpenCode, with local workspaces, multiple model/provider paths, browser/tool access, schedules, artifacts, and optional remote/organization services. Its repository also exposes a separate server/control-plane path; exact features depend on release and configuration. | Study its polished cowork framing and local-first entry points. Keep local worker availability, remote control plane, schedule host, and execution environment as separate status facts. Do not infer an always-on OS service from the presence of a desktop scheduler. |
| **OpenCowork (name collision)** | Several unrelated public repositories use this or a near-identical name, including `LeonGaoHaining/opencowork`, `OpenCoworkAI/open-cowork`, and `opencowork-ai/opencowork`. They differ in ownership, runtime, maturity, and feature claims. | Always cite the owner/repository and commit or release. “OpenCowork” alone is not a stable product or architecture reference and must not be used as a dependency identifier. |
| **DeepSeek Harness** | `deepseek-ai/deepseek-harness` is the public developer-preview harness repository checked here, with a web entry point, a desktop app, and a plugin-oriented tool/runtime design. The project labels itself preview/developer-facing; its interfaces may change. | Treat this as an agent harness that can be launched and qualified, not as a model subscription route or a universal model endpoint. An adapter must report the exact executable, version, auth mode, features, OS, and supported control semantics. |
| **Devin** | Devin's official docs describe a cloud agent and desktop/IDE surfaces, reviewable work, computer use, and reusable automation templates. Devin is proprietary; no public Devin desktop source repository was verified. | Borrow reviewable task progress and clear handoff. Do not imply that a proprietary worker can be embedded, controlled, or delegated to without a documented supported interface. |
| **Cursor** | Cursor's official docs describe cloud agents, automations, repository/environment setup, tool/capability controls, and remote desktop/computer-use support for qualified workers. Cursor is proprietary; no public Cursor Desktop implementation repository was verified. | Learn from setup prerequisites, task artifacts, and automation lifecycle. Distinguish cloud-agent features from the local IDE and do not treat Cursor as an embeddable open-source harness. |

### Capability comparison translated into LiteCowork's V1 desktop bar

The goal is to match the best experience a user can achieve **on the user's currently
available desktop/laptop**, while preserving every selected agent's native behavior and
authorization. It is not a claim of equality with every competitor's cloud service,
mobile surface, model, paid plan, or private benchmark.

1. **Agent-native lead selection in the chat composer.** The user selects an installed,
   authenticated, eligible agent/harness and one of its supported models/options in the
   same composer. The visible choice is the exact binding and qualified route, not a
   generic model string. Switching agents preserves draft content but clears incompatible
   controls visibly. No unsupported route is silently substituted.
2. **Native subagents remain native.** A normal conversation turn goes to the selected
   lead agent. Its own child-agent mechanisms, tool semantics, auth, context, visibility,
   cancellation, and model controls are used only to the extent reported by that adapter.
   LiteCowork must not describe opaque native children as independently authorized or
   independently metered.
3. **Cross-agent subagents are a separate, explicit Task path.** Workspace Settings choose
   whether Task delegation uses native delegation, named LiteCowork worker profiles, or a
   supported combination. Coworkers may inherit or narrow that Workspace ceiling. Each
   host worker profile pins a qualified agent/harness, OS/runtime, independent auth,
   model-selection policy, allowed capability set, budget, and cancellation behavior.
   The user can choose “Run as Task” or accept a Task plan; ordinary Send is never silently
   promoted to a Task. Parent credentials/subscriptions never transfer.
4. **Task-grade progress and control.** A user can see what is happening, which worker and
   environment own each step, what needs approval, what changed, and what was verified.
   Pause, resume, cancel, take-control, and handoff have observable process/effect
   semantics; a UI toggle alone is not a stop receipt.
5. **Real desktop capabilities, qualified per route.** Browser, terminal, selected files,
   local applications, and computer-use are offered only when that agent/runtime/OS path is
   qualified. The screen identifies local resource dependencies, machine availability,
   and whether a step can continue after the app closes. No cloud continuation is inferred
   from a local daemon.
6. **Cowork outputs are first-class.** Semantic answers, files, rich presentations, and
   safe interactive artifacts have distinct contracts. Users can preview, compare,
   download, revise, and find outputs without reading raw logs or code-agent traces. A
   renderer failure must preserve the answer and downloadable verified files.
7. **Connectors and packages have honest boundaries.** LiteSPM is the inbuilt manager for
   its supported MCP/skill/plugin catalog and install lifecycle. Installing a package is
   not connecting an account, granting a Workspace permission, attaching it to an agent,
   or launching that agent as a worker. Unsupported agent/package combinations remain
   visible as unsupported.
8. **Automation starts as an explainable draft.** Coding and general cowork templates
   preview their requirements, accounts, permissions, outputs, and runtime placement.
   Choosing one creates an editable Routine draft only. Saving, connecting/assigning
   dependencies, selecting a local trigger host/timezone, enabling, and running are
   separate owner actions.
9. **Reliability is visible.** Durable Task/Event/Evidence history, idempotent admission,
   bounded replay/recovery, exact artifact versions, and Effect reconciliation protect
   against duplicate work and false success. Native session continuity is claimed only
   when the native agent adapter proves it.
10. **Usage and quality are route-specific.** Display model/agent identity, auth owner,
    plan/quota availability, feature gaps, and usage-confidence provenance per route.
    Compare LiteCowork mediation with the same agent/model's direct native path. Never
    claim that changing the wrapper makes a weaker model equal to ChatGPT Plus or Claude
    Pro.

### Requirements confirmed, missing-contract checks, and decisions

| Requirement | Current contract outcome | Required follow-through |
|---|---|---|
| V1 is the full desktop/laptop product; cloud/mobile continuation is later | Confirmed in Product Index, release contract, and UI contract | Keep local/remote placement and machine-online state visible; do not leave “cloud” wording in V1 promises |
| Switch lead agents/models in the central composer | Agent route matrix and native composer flow define route pinning and visible incompatibility | Every catalog row must expose install/auth/eligibility/features; qualify exact OS and harness |
| Use selected agent's native subagents, with optional LiteCowork workers from other agents | Two delegation mechanisms are defined; direct chat uses native only, host delegation requires an admitted Task | Settings/schema/storage/API/event must use one source-of-truth revision and pin it at Task admission |
| Inherit parent model or use a worker's configured model | Model policy is explicit and route-qualified | Disable inheritance unless the exact agent supports it and the worker independently authenticates that route |
| LiteSPM installs MCPs, skills, and plugins | Catalog/install lifecycle is separate from connections, grants, and worker launch | Never model LiteSPM as the multi-agent scheduler or claim every item supports every agent |
| Coding and general cowork automation examples | Automation gallery creates a draft only | Template detail must show inputs, account/connector needs, local host, output and approval points before save |
| Attractive cowork experience instead of coding-agent control panel | The shell is three-pane with conversation center and compact, visible Workbench | Default to useful end-user language and artifact views; expose logs, diffs and agent diagnostics progressively |
| “Plus/Pro-equivalent” artifacts, generative UI, and content | No product-wide parity can be promised from docs or vendor marketing | Define route-matched paired quality and responsiveness benchmarks; publish tested routes and limitations |
| Multi-provider auth without subscription proxying | Route matrix separates native sign-in, API keys, quotas, and agent-owned sessions | Each integration needs documented vendor-supported auth; no cookie/session/token copying or hidden fallback |

### First-party sources and repositories checked for this addendum

- xAI, [Introducing Grok Bot](https://x.ai/news/introducing-grok-bot), [overview](https://docs.x.ai/grok-bot/overview), [FAQ](https://docs.x.ai/grok-bot/faq), [computer and apps](https://docs.x.ai/grok-bot/computer-and-apps), [approvals, security and privacy](https://docs.x.ai/grok-bot/approvals-security-and-privacy), and [chat and collaboration](https://docs.x.ai/grok-bot/chat-and-collaboration).
- OpenAI, [Introducing Dots](https://openai.com/index/introducing-dots/), [Dots documentation](https://learn.chatgpt.com/docs/dots), and [Getting started with your Dot](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot).
- Anthropic, [Cowork on web, desktop and mobile](https://support.claude.com/en/articles/15520349-use-claude-cowork-on-web-desktop-and-mobile), [Cowork and chat are one Claude](https://support.claude.com/en/articles/16761823-claude-cowork-and-chat-are-one-claude), [Artifacts](https://support.claude.com/en/articles/17153992-what-are-artifacts-and-how-do-i-use-them), and [computer use in Cowork](https://support.claude.com/en/articles/14128542-let-claude-use-your-computer-in-cowork).
- Meta, [Introducing Muse, a personal AI agent](https://about.fb.com/news/2026/09/introducing-muse-personal-ai-agent/), [Muse availability](https://ai.meta.com/muse/download/), and [Muse Wonder models](https://about.fb.com/news/2026/10/introducing-muse-wonder-models/).
- Microsoft, [Copilot Cowork](https://learn.microsoft.com/en-us/microsoft-365/copilot/cowork/), [model availability](https://learn.microsoft.com/en-us/microsoft-365/copilot/cowork/cowork-models), [admin governance](https://learn.microsoft.com/en-us/microsoft-365/copilot/cowork/cowork-admin-governance), and [What's new](https://learn.microsoft.com/en-us/microsoft-365/copilot/cowork/whats-new).
- Open-source repositories: [Eigent](https://github.com/eigent-ai/eigent), [OpenWork](https://github.com/different-ai/openwork), [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness), [LeonGaoHaining/opencowork](https://github.com/LeonGaoHaining/opencowork), [OpenCoworkAI/open-cowork](https://github.com/OpenCoworkAI/open-cowork), and [opencowork-ai/opencowork](https://github.com/opencowork-ai/opencowork).
- Proprietary product documentation: [Devin introduction](https://docs.devin.ai/get-started/devin-intro), [Devin desktop](https://docs.devin.ai/desktop/getting-started), [Devin computer use](https://docs.devin.ai/work-with-devin/computer-use), [Cursor cloud agents](https://cursor.com/docs/cloud-agent), [Cursor automations](https://cursor.com/docs/cloud-agent/automations), [Cursor capabilities](https://cursor.com/docs/cloud-agent/capabilities), and [Cursor setup](https://cursor.com/docs/cloud-agent/setup).

Sources above were checked on 2026-10-24. Product availability, rollouts, repository heads,
plan entitlements, regions, and model lists are time-sensitive. For any future release
comparison, recheck the first-party page and a pinned repository revision; do not treat
this snapshot as a current adapter qualification or a promise of provider capability.


## LiteSPM dependency verification — 2026-10-24

This is a dated dependency check, not a LiteCowork wire contract. I opened the LiteSPM
public catalog, its developer and security pages, and the linked
[upstream repository README](https://github.com/sarv-projects/LiteSPM). The pages describe
a local CLI that syncs/searches a catalog, targets supported agent hosts, previews
configuration changes, backs up files, records managed ownership and supports removal
or restore. The developer guide documents a stdio MCP bridge and says an agent can search
and prepare a `/marketplace` plan but cannot approve that plan itself.

The upstream README showed **69,376 catalog listings** and **26,554 one-command
installable listings** at the time checked. It reported 50 MCP host targets, 81 skill
targets, and that some hosts do not support remote-URL MCP entries. These values are
dynamic. The critical design finding is the category difference: a catalog result may
be discovery-only, and host compatibility varies by package type, OS, target and scope.
The product must display those states instead of treating the catalog as a universal
install list.

The README also describes package-plan review, a human approval step, backups and
managed-entry ownership, while the public security page warns that installed capabilities
run with the user's machine permissions and are not sandboxed. A source badge, install
success, or LiteSPM's package verification does not prove that third-party code is safe or
that LiteCowork has granted it an action capability. Keep install/connection/auth/assignment/
grant/activation/invocation as separate product states.

The linked
[CLI product-surface specification](https://github.com/sarv-projects/LiteSPM/blob/main/ARCH/38-CLI-PRODUCT-SURFACE.md)
labels several areas as designed rather than shipped (including parts of the orient,
profiles and expanded generic plan/apply command families), even though the current
individual install flow has plan/approval behavior. Do not promote a documented target or
command tree into an available integration promise without release-pinned implementation evidence.
Our in-app Discover UX, LiteSPM interoperability LLD and route qualification gates now
preserve this distinction. Recheck upstream release evidence before enabling the adapter.


## 2026-10-25 recheck — route terms, harness reality, and desktop control

This dated addendum records facts that affect the V1 bar and corrects any reading of repository roadmaps as shipped capability. Vendor and repository claims are evidence to inspect, not LiteCowork release qualification.

- **OpenAI SIWC is a real supported app route with boundaries.** The current terms require a user-controlled runtime, user-originated or expressly authorized requests, explicit consent before background use, connected-app-only use, no general-purpose API, plan limits, no charge for the SIWC feature, and no pooling or transfer of account usage. This does not make arbitrary ChatGPT/Codex subscription access a universal adapter. Source: [SIWC Terms](https://openai.com/policies/sign-in-with-chatgpt-terms/).
- **Claude Code binary and Claude Agent SDK are separate integration choices.** Anthropic permits an unmodified Claude Code binary inside a product only under its Commercial Terms and stated conditions, with each user authenticating and paying under their own eligible agreement. For a product built on the Agent SDK, Anthropic says to use API-key or supported-provider authentication and prohibits offering Claude.ai login or intermediating subscription credentials in the third-party app. Source: [Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance), [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview).
- **OpenCode is a powerful native harness, not a safe permission layer by default.** Current OpenCode docs say most permissions default to allow; edit, shell, subagent, web and external-directory access must be explicitly controlled for the chosen mode. A UI that launches OpenCode and reports an approval badge does not prove LiteCowork-mediated Trust, isolation, or Effect evidence. Sources: [permissions](https://opencode.ai/docs/permissions/), [server API](https://docs.opencode.ai/docs/server/).
- **OpenCoworkAI's roadmap must be separated from its current product.** Its roadmap describes the current desktop as a single-session, single-turn Claude Agent SDK wrapper; headless operation, configurable agent routing, and richer subagent orchestration are planned gaps in that roadmap. Do not credit roadmap items as shipped features. Source: [repository](https://github.com/OpenCoworkAI/open-cowork), [roadmap](https://github.com/OpenCoworkAI/open-cowork/blob/main/ROADMAP.md).
- **OpenWork, Eigent, and OpenCowork solve different slices.** OpenWork combines an OpenCode desktop experience with local project work and a separate control-plane/service story; its repository/docs are useful for local-first entry, provider breadth and lazy capability discovery. Eigent demonstrates a CAMEL-based multi-agent desktop/runtime approach with MCP, skills, browser and terminal toolkits. OpenCoworkAI is a smaller single-turn wrapper today. Compare exact releases and runtime boundaries rather than treating any of the three as a complete local Cowork reference. Sources: [OpenWork](https://github.com/different-ai/openwork), [Eigent](https://github.com/eigent-ai/eigent), [OpenCoworkAI](https://github.com/OpenCoworkAI/open-cowork).
- **Computer use is an OS-control capability, not a browser panel.** The [open-computer-use MCP project](https://github.com/ifuryst/open-computer-use) exposes OS-level controls across desktop platforms, while the [OpenAI CUA sample](https://github.com/openai/openai-cua-sample-app) shows browser/desktop observe-and-act loops. Both are implementation references only: LiteCowork still needs a qualified provider, explicit OS consent, scoped execution, takeover/return state, and truthful activity evidence.

### 2026-10-25 local/cloud execution-path corrections

- **OpenAI dots:** each dot has its own persistent cloud computer. The optional local bridge is separately granted and depends on the host desktop app being open and the computer online; local tasks appear separately. Work Sync or a Codex connection alone does not grant local access. Sources: [Dots help](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot), [Dots privacy and security](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs).
- **Meta Muse:** the Mac product can access permitted local files/apps/browser tabs while Muse Secure VM is a distinct persistent hosted computer with its own browser. The public product pages advertise Mac, iOS, Android and web; the launch article describes a Sentinel egress gate, separate secure credential storage, approval for sensitive actions and an audit trail. Sources: [Muse](https://ai.meta.com/muse/), [Muse download](https://ai.meta.com/muse/download/), [Meta launch/security architecture](https://about.fb.com/news/2026/09/introducing-muse-personal-ai-agent/).
- **Grok Bot:** its persistent cloud computer is shared among a user's Bots, including files, browser sessions and app logins. Optional local desktop execution is a distinct path with its own per-computer permission policy (Ask every time by default); cloud approvals do not grant local execution. Sources: [Grok Bot overview](https://docs.x.ai/grok-bot/overview), [computer and apps](https://docs.x.ai/grok-bot/computer-and-apps), [approvals/security](https://docs.x.ai/grok-bot/approvals-security-and-privacy).
- **LiteCowork contract impact:** expose the actual execution location and authority for every route, treat each local host bridge and cloud Environment as separate consent/capability scopes, and never infer capability from app presence, account connection, or a connected agent profile. Competitor availability is not proof that a LiteCowork provider is implemented or qualified.

### Product consequences

1. The required V1 differentiator is the *finished desktop work experience*: selected native lead agent, that agent's actual models/options, native subagents when supported, and separately configured host-managed workers for durable Tasks. Selection must pin the route and never imply that switching a model is switching harnesses.
2. Conversation-specific AgentBinding set/clear now has a served, version-fenced and idempotent route plus a draft-preserving selector. This only chooses the binding for future turns; no native session adapter is qualified for a user-visible Send, so Send remains disabled.
3. Preserve distinct states for native child agents and LiteCowork-hosted child Attempts. Parent-model inheritance is available only when the native harness supports/enforces it; cross-harness delegation needs its own pinned worker route and Task admission.
4. No reviewed competitor source makes a full OpenCode/Codex/Claude/Cursor/Devin feature set automatically available through LiteCowork. Route qualification, permissions, account terms, filesystem scope, result recovery, and UI parity remain per-harness work.
