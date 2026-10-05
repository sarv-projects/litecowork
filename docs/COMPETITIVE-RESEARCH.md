# Competitive Product Research (Snapshot: 2026-10-05)

This is dated product research, not a normative competitor dependency. Product behavior
changes quickly; cited vendor pages are the evidence, while LiteCowork contracts are owned
by `ARCHITECTURE.md` and the domain documents.

## Verified patterns and corrections

| Product | Verified behavior | LiteCowork implication |
|---|---|---|
| Claude Cowork | For Pro/Max, one Claude experience is rolling out; Team/Enterprise still retain separate chat and Cowork. A Scheduled page supports recurring and on-demand runs. Cloud work can continue while the computer is off, but work tied to local folders/apps runs locally and needs that computer; an active cloud session can reach local files/browser only through the connected Desktop app. Local MCP servers run in local desktop sessions, not cloud sessions. The Pro/Max move for new Cowork tasks is scheduled for 2026-10-06, two days after this snapshot. | Keep one composer; represent local-resource dependency separately from cloud continuation. Do not describe the announced migration as already complete or imply the local bridge is unnecessary. |
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
