# Archived Task-first Home UX (non-normative, 2026-10-22)

Historical interim implementation description. **Final V1 defaults to New Chat.**
Canonical [V1 screen contract](../../docs/V1-UI-SCREEN-CONTRACT.md).

## Home — legacy Task-first source surface during transition

This section documents the current interim Home/Task-draft functionality. The accepted final default is ordinary New session/Conversation with optional chat-first Coworker navigation, not a mandatory primary-Coworker Home. The current Conversation page does not yet admit native agent turns; do not relabel its disabled composer as functional. See [Coworker target](COWORKERS-TARGET.md).

Purpose of the interim Home: start or resume work.

States:
- normal/recent Tasks
- first-use empty state
- no eligible default AgentBinding with preserved draft/setup action
- offline local-only
- reconnecting
- cloud unavailable

Home includes a persistent “Needs you” inbox entry and the first-class Needs You
destination. The pending count and notification history are not connected yet: the Needs
You page reports its own loading, unavailable, and empty states, and no count is inferred
from an unread inbox.

Composer:

```text
[Alex ▾] What should we work on?                         [ + ] [Send →]
```

The default new-session composer is an ORDINARY conversation, not a required
Coworker. An optional Coworker can be selected explicitly, and opening one from the
sidebar opens its last active chat. The chosen native chief agent, native model and
reasoning effort are accessible without an Agent model router. A legacy Workspace
primary-Coworker preference can prefill existing Task-origin UI only; it does not
require creation or automatically attach a new ordinary chat. `Advanced` exposes Lead Agent,
Model/Reasoning when adapter-supported, Run location, Tools, Budget, and Access. The
selected Coworker's default lead and enabled worker allowlist are pinned into Task origin
and TaskSpec at admission; changing them affects future Tasks. A direct lead override is
explicit and does not mutate Coworker settings. It does not invoke a model router or
silently select different agent software. Run placement selects only among eligible
Runtimes for that binding. Tool discovery never skips grants, approvals, or capability
availability checks. Selecting an Agent does not start its host. The host starts when the
user submits a turn or an Attempt is admitted; optional prewarming is an explicit latency
optimization. Changing an Agent during active work changes future planning assignments
only; existing Attempts remain pinned and visibly drain.

Home's normative content order is Composer; Needs You when non-empty; Being handled (at
most four Tasks); Coming up (at most three responsibilities); one prominent Idea; Recent
outputs (at most four); then Recent Conversations. A section may be hidden when it has no
useful content; Home never expands into an operations dashboard.

`+` menu:
- Add files
- Add folder
- Screenshot
- From Library
- Connect an app
- More

Send menu actions:
- Send (ordinary intent detection)
- Run as Task (explicit durable Task)
- Schedule… (reviewed Routine/trigger/placement summary, then confirmation)
- Save as Routine (reviewable draft; no Automation is created)

Natural-language timing may propose a schedule, but it never silently creates or enables a
long-lived Automation. A schedule preview names timezone, trigger host, misfire behavior,
required local resources/apps, eligible execution locations, cost/permission bounds, and
the Routine revision that will be pinned.

Advanced popover may expose Agent, Model (when agent exposes it), Reasoning, Execution preference, Budget, Access and Capabilities.
Response style is a separate preference from execution optimization. It defaults to
`Auto`; users may choose `Simple` or `Rich` from a low-friction composer menu or Settings.
Do not prompt for this choice on every turn, and do not label it as Agent/model selection.

