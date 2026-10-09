# Competitive Product Research (Snapshot: 2026-10-10; earlier checks retained)

This is dated product research, not a normative competitor dependency. Product behavior
changes quickly; cited vendor pages are the evidence, while LiteCowork contracts are owned
by `ARCHITECTURE.md` and the domain documents.

## Verified patterns and corrections

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

**Low level / desktop surface:** Dot setup is available on desktop; local-computer access is optional and starts off. When connected, a dot can access files, create local Work/Codex tasks, use local skills and use the local browser when needed. Plugin/app permissions are shared with the wider ChatGPT app surface.

**Mid/high level:** A dot is a named persistent agent with memory, proactive connected-app review, scheduled tasks, background suggestions, customizable rules and pause/reset. Desktop profile exposes activity grouped as In progress, Scheduled and Completed and can open the dot's computer. Custom rules offer action modes roughly equivalent to act without asking, pre-approved, ask first, or hand off.

**UI:** Conversation remains central. Profile/customize areas carry Plugins, Memory, Custom rules, recent activity and computer access rather than crowding the composer. Scheduled/proactive updates come back into the conversation. Local computer tasks show separately in the sidebar.

**Take for LiteCowork:** Keep Coworker profile secondary to chat; use compact In progress/Scheduled/Completed summaries; separate connection availability from approval rules; support Pause at identity level. LiteCowork should make memory more inspectable/revocable per record than a reset-only experience and should not imply local work can continue while the machine is off.

Sources: [Getting started with your dot](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot), [Introducing dots](https://openai.com/index/introducing-dots/), [Dots privacy/security](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs).

### Meta Muse — conversation + goals + proactive suggestions

**Low level / desktop surface:** Muse has a Mac app and is documented as working with desktop files, apps and browser tabs. It also has a persistent isolated computer/browser experience and connected-app support. Credentials are kept in a secure store rather than exposed directly to the agent.

**Mid/high level:** Users give Muse goals/tasks; it builds plans, tracks progress, suggests ideas proactively, monitors things in the background, connects to email/calendar/Instagram and other services, and can create tools when a needed tool does not exist. Critical actions such as sends/purchases are reviewable and the product advertises an audit trail of completed/planned actions.

**UI:** Conversation-first across Mac/mobile/WhatsApp, backed by explicit Goals/Ideas tracking, approval cards and a shared browser/computer surface that a human can intervene in. Its product framing reduces workflow-builder exposure: the assistant carries the plan, while the user sees goals, ideas, approvals and outcomes.

**Take for LiteCowork:** Keep Goals and Suggestions/Ideas as visible but non-authoritative supporting surfaces, add concise audit/action history, and make Take control obvious for Browser/Computer. Generated tools remain proposals that pass LiteSPM/Skill review rather than self-installing.

Source: [Muse product page](https://ai.meta.com/muse/).

### Grok Bot — strongest current persistent-bot/routine desktop UX benchmark

**Low level / desktop surface:** Grok Bot has desktop apps and a persistent computer. The desktop client includes Bot chats, computer/live view, Marketplace/plugins, settings, app sign-in flows and local-computer execution controls where enabled. Model selection is managed by the product rather than exposed as a picker.

**Mid level:** Bots keep role-specific memory, can share files/context through explicit chats/groups, own reusable Skills and Routines, use connectors/logins, and expose a Library tab with shared pages/files/links. A task asking for an unconnected app can present connection UI and resumes automatically after sign-in. Teach-a-task can record a demonstrated browser workflow and generate a draft skill for review/testing.

**High level:** Routines can run on schedules or supported events; Test run performs real work and is explicitly risky. Routine UI supports pause/resume/test/edit/delete and run history. Primary Bot proactivity can spot work to offer. Approval cards quote the Bot/instruction; Custom Rules and Auto Review govern unattended actions. Long jobs, failure recovery, notifications and computer takeover are explicit product concepts.

**UI:** Bot roster/sidebar, primary Bot star, per-Bot details with Routines and Library tabs, Marketplace shortcut, chat approval cards, full-screen computer view/Take over, routine context actions, per-Bot notification switches and searchable prior work. This is the closest competitor to LiteCowork's named Coworker concept.

**Take for LiteCowork:** Add `Test responsibility/routine`, recent run history, Library/activity tab within Coworker, task-time Connect-and-resume, optional Teach workflow later, clear primary/featured Coworker without making it mandatory, and strong per-Coworker notification/routine controls. Unlike Grok Bot's shared cloud computer, LiteCowork must retain exact local Environment scopes and cannot let Coworkers implicitly share files/logins.

Sources: [Grok Bot overview](https://docs.x.ai/grok-bot/overview), [Skills and routines](https://docs.x.ai/grok-bot/skills-routines-and-automations), [Bots](https://docs.x.ai/grok-bot/bots), [Changelog](https://x.ai/changelog/bot), [approvals/security](https://docs.x.ai/grok-bot/approvals-security-and-privacy).

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
