# Experience Architecture

## Information architecture

Primary navigation:

```text
Home
Needs You
Tasks
Automations
Library
Discover
```

The desktop sidebar also contains search and a persistent Recent Conversations list. Needs
You is a first-class destination with a pending count for approvals, expired logins,
required local Runtimes, ambiguous Effects, TaskSpec conflicts, user choices, and
verification requests. Advanced technical detail lives in Inspector/Settings/Discover
details. A Workspace selector is available in the app shell when more than one Workspace
exists.

## Workspace setup and selection

Workspace creation is part of first-run setup and Settings. The user sees the replication scope in plain language before choosing a policy:

- `LOCAL_ONLY`: stays on this Runtime.
- `METADATA_ONLY`: syncs Workspace and Task metadata, not file contents.
- `ACTIVE_TASK_INPUTS`: transfers only inputs and outputs required by cloud-eligible active Tasks.
- `SELECTED_FOLDERS`: transfers resources under the selected persistent WorkspaceRoots and Task outputs; newly observed revisions remain in scope while the root grant and policy remain active.
- `FULL_WORKSPACE`: explicit whole-Workspace replication with a clear storage/privacy summary.

Creation defaults to `LOCAL_ONLY`; cloud enablement is a separate explicit action. To use `SELECTED_FOLDERS`, the setup flow first creates the Workspace, then creates the persistent WorkspaceRoots, then saves the selected-root policy. Changing policy applies prospectively and explains that copies already transferred are retained. Archive is a separate, confirmed action available only after Tasks are terminal and Automations disabled. Quiescence also requires Conversation turns and scoped Invocations to be settled, no active grants/SecretLeases/control leases, and persistent Environments with no live workload. Authorized watchers/triggers stop before the read-only transition. Retained Environment state may remain suspended under storage/backup policy; archive never silently destroys it. Unknown provider quiescence blocks archive with `WORKSPACE_NOT_QUIESCENT`. Archived Workspaces remain browsable and show a persistent read-only banner.

States: first-run, create in progress, policy saving, cloud unavailable, archive blocked with named active Tasks/Automations, archived/read-only, stale policy version/conflict.

First use also establishes an enabled default AgentBinding. The user chooses it from
discovered external agents; LiteCowork does not silently select a model or create an
AgentBinding. If no eligible default exists, the composer explains setup and preserves
the draft instead of creating a failed chat turn. Existing Workspaces may clear the
default; new chat turns and Task materialization remain unsubmitted until the user selects
and enables an eligible lead. A binding that becomes unavailable after Task creation is
shown as a named `Task.blocking_conditions[]` entry.

Agent setup is a three-step path: discover Runtime-reported AgentProfiles/Endpoints; create
a disabled Workspace AgentBinding; enable it after authentication/policy checks; then
explicitly choose it as the Workspace default or a Conversation override. Without a
selected binding, sending keeps the draft and opens setup guidance. “Auto placement” may
choose among eligible Runtimes/Environments for the selected binding; it never silently
switches the AgentBinding or its model.

## User interaction flows

- **Create Workspace:** explain local-only default and available replication scopes; create only after the user commits the choice.
- **Enable cloud:** show eligible content, storage implications, required Runtime/agent/secrets, and any unavailable inputs; save the policy only after explicit confirmation. If the update conflicts or fails, retain the old selection.
- **Archive:** show the exact nonterminal Tasks or enabled Automations that block archive. After successful archive, retain navigation/read access and show the read-only banner.
- **Connect a capability:** move from Discover details to requested scope/approval, then show it as ready only after activation health succeeds.
- **Load a server Skill:** show the host-assigned server identity, exact Skill URI, manifest digest, and requested authority. Approval applies to that exact content set; a changed manifest pauses its use and requests approval again. A dynamic Skill is labeled unsupported in v1, and a nested Skill requires its own approval.
- **Approve an action:** present exact action, target, scope, risk, and required assurance; stale/expired approvals require a fresh request rather than replaying approval.
- **Answer a provider question:** render bounded non-sensitive form requests as ordinary UserRequests; clearly state that passwords, API keys, recovery codes, and tokens must not be entered. Suspicious credential fields are rejected with a provider-connection action.
- **Continue provider sign-in:** show the capability publisher and parsed HTTPS destination origin, explain that the external site receives anything entered there, open the provider page in the system browser only after a user click, then offer Done/Decline/Cancel. Never embed the page or store its state-bearing URL in LiteCowork history; the system browser may retain its normal browsing history. The response sent back is action-only.
- **Change lead agent:** choose an eligible Workspace binding, submit a versioned request, show existing Attempts draining under their pinned identity, and show the new lead only after the assignment event.
- **Create or edit an Automation:** choose an active Routine revision, preview every trigger/timezone, TriggerHost, overlap/retry behavior, execution placement, dependencies, and notifications; save edits as a new immutable revision. Each occurrence detail displays the Routine and Automation revisions it pinned.
- **Pair a Runtime:** explain its role and advertised local resources, issue a short-lived one-use pairing action, and show it as available only after identity verification.
- **Backup and restore:** Settings lists only verified restore points, their event cursor/schema and key reference, and missing-object blockers. Creating a backup shows a busy state until the verified manifest is returned; it shows stage/percentage only when the snapshot provider reports real progress. Restore is exposed only during authenticated setup on an empty installation and clearly notes that connections may need reauthentication.

## Home

Purpose: start or resume work.

States:
- normal/recent Tasks
- first-use empty state
- no eligible default AgentBinding with preserved draft/setup action
- offline local-only
- reconnecting
- cloud unavailable

Home includes a persistent “Needs you” inbox entry/count and notification history as well
as the first-class Needs You destination.

Composer:

```text
[ + ] Ask something or describe work... [Agent: Auto ▾] [Run: Auto ▾] [Tools: Auto ▾] [▶]
```

`Agent: Auto` means the active Conversation override or Workspace default AgentBinding; it
does not invoke a model router or silently select different agent software. `Run: Auto`
lets placement select only among eligible Runtimes for that binding. `Tools: Auto` allows
progressive discovery, but does not skip grants, approvals, or capability availability
checks. Controls are compact progressive disclosures; placement labels include `This
computer`, `Cloud`, and a named Runtime. Selecting an Agent does not start its host.
The host starts when the user submits a turn or an Attempt is admitted; optional prewarming
is an explicit latency optimization. Changing an Agent during active work changes future
planning assignments only; existing Attempts remain pinned and visibly drain.

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

## Conversation

Conversation is the human continuity surface. Task cards appear inline when durable work materializes.

ConversationTurn `WAITING_USER` is labeled “Waiting for you”. After a user response is
committed, `WAITING_DEPENDENCY` is labeled “Waiting for service” with a concise provider or
agent-startup detail; it is not shown as another unanswered request. “Continue” appears
only after the exact provider input is accepted and the replacement session is ready.

Task card states:
- Planning (Task is durable; lead planning session is active; no Step lane exists yet)
- Ready
- Running
- Pausing safely
- Paused
- Waiting for you
- Blocked
- Verifying
- Needs your decision
- Not finished
- Completed
- Failed
- Stopping
- Cancelled

If the user answers a Task-scoped provider question while its Task is `PAUSED`, show the
response as saved/queued on the Task and Needs You item. Do not label the Task resumed or
show provider activity until explicit resume and scope-specific authorization succeeds.
If the provider task cannot resume on its owning Runtime/Attempt, keep the answer visible
while reconciliation or a newly authorized continuation is required.

Card shows plain-language execution location: `This computer`, `Cloud`, `2 workers active — Cloud + This computer`, or specific blocker such as `Waiting for your laptop — needs local Chrome`.

## Tasks

Task list filters:
- Active
- Needs you
- Scheduled/automation-created
- Completed
- Failed/blocked

Task detail sections:
- objective/current spec summary
- progress/workstreams
- outputs/artifacts
- approvals/blockers
- activity timeline
- execution location
- actions: steer, request a lead-agent change, pause/resume, cancel, recover a failed Step where valid

## Live Desk

Live Desk projects real work into outcome-oriented lanes.

Default hides agent names and protocol internals. A planning status may appear before plan acceptance, but no work lane appears until real Steps and Attempts exist. Each lane corresponds to real Step/Attempt state.

Possible lane elements:
- source/resource card
- capability activity
- child workstream
- artifact card
- approval card
- verification indicator
- blocker/failure

Inspector reveals actual Agent, Runtime, Environment, capabilities, effects and usage.

## Workbench

Right-side contextual editor/viewer shell.

Tabs may include:
- Document
- Spreadsheet
- Slides
- PDF
- Image
- Code
- Diff
- Browser
- Terminal
- MCP App
- Generated UI
- Task details

LiteCowork owns the shell, identity/provenance and display contract; domain editing logic belongs to capability/provider.

## Library

Filters/resources:
- Generated
- Uploaded
- Imported
- Linked
- Archived
- Templates
- Skills
- Saved workflows

Saving/promotion is explicit. Linked resources show provider and external revision/availability.

## Automations

The page has three views: `Routines | Automations | Runs`.

Routine cards show immutable current revision, typed inputs, outputs/acceptance criteria,
capability requirements, placement eligibility, and actions to run, revise, duplicate,
archive, or create an Automation. Saving work as a Routine opens a redacted Operator
draft; task-specific paths, records, secrets, and incidental context are removed or
converted to typed inputs before the user commits it. An unsaved draft is not executable.

Automation cards show all triggers, TriggerHost, next occurrence, independent execution
placement, timezone/misfire policy, dependency blockers (for example “Needs this computer
— Excel and D:\\Finance”), pause state, and last Task outcome. Runs is occurrence and Task
history; occurrence COMPLETED requires the linked Task to be COMPLETED. A schedule with
no future runs does not imply all of its Tasks succeeded. Due work that cannot
run is visibly `Waiting for dependency`, not silently skipped or left indistinguishable
from a future schedule.

Each Automation run creates an ordinary Task from the pinned RoutineRevision. Users must
confirm schedule creation and material updates. `Run now` is shown for an Automation only
when it has an enabled MANUAL trigger; otherwise the user can run its pinned Routine
directly, without creating an AutomationOccurrence.

## Discover

Embedded LitePSM-oriented experience with user categories:

```text
Apps
Capabilities
Skills
Agents
```

MCP-served Skills retain their originating server identity and URI through search,
approval, inspection, and loading. Same-named Skills appear as separate origin-qualified
entries. Their detail shows the content manifest and requested authority before approval;
manifest changes revoke the prior approval. Dynamic entries remain inspectable but cannot
be loaded as Skills in v1.

Default card shows user value and compatibility. Expanded detail shows provider/package source/version/digest/permissions/health/protocol.

## Runtime and local availability

Settings exposes Runtime startup policy (`Manual`, `Start at login`, `Always-on server`),
current incarnation/recovery status, and an explicit `Stop local Runtime` action. Closing
the UI does not stop a background Runtime. The read-only stop preview shows local Tasks,
Automations, and roots that depend on the device and offers `Cancel`, `Move eligible work
to Cloud`, or `Stop anyway`; moving eligible work is an explicit handoff, never assumed.
Sleep/wake, locked-session restrictions, offline state, and cloud/local eligibility are
plain-language states. Optional hardware wake is labeled experimental/best-effort.

Workspace Environment settings list persistent compute by name, Runtime, health,
retention/backup policy, resource/network bounds, budget, active consumers and current
blockers. Budget display separates the user's ceiling and enforcement choice from the
provider's actual enforcement status, and shows cumulative cost/time with confidence and
last observation; stale/unavailable values read Unknown, never $0. Create requires a
provider/placement and cost/time review. Suspend/resume/destroy
show which Tasks use the Environment; no action restores old Task authority or silently
deletes retained files. A resumed or reattached Environment remains unavailable until the
provider identity and current source Resources are checked. In v1 the budget cannot be
raised in place. At the limit, show the Environment as safely suspended and list blocked
Tasks/Steps. Offer `Choose another Environment` or `Provision replacement`; selecting an
existing Environment first refreshes the Step's placement preview and requires the current
candidate digest when recovery is submitted. A stale preview asks the user to review fresh
options. Explain that a replacement has a new identity and private provider state is not
silently cloned. Users can publish/export required state as Resources or Artifacts before
starting a new Attempt.

Preflight and Discover distinguish `AVAILABLE`, `STARTABLE`, `STARTING`, `READY`, `BUSY`,
`DEGRADED`, `OFFLINE`, `NEEDS_AUTH`, and `UNAVAILABLE`. For example, an Agent may be
`Installed · not running`, an MCP provider `Stopped · can start`, and Excel `Available on
this computer`. These states do not imply a grant or activation.

Quick Entry (global hotkey and tiny composer with optional current-app/screenshot context)
is a later Operator-shell feature. It never belongs to the headless Runtime daemon.
If Quick Entry includes a screenshot/current-app context, that content is previewed and
attached as a ResourceRef only after the user submits; opening the hotkey never captures
or sends context in the background.

## Inspector

Developer/advanced panel:

```text
Task ID
TaskSpec revision
Plan revision
Lead agent
Runtime
Environment
Capability locks/grants
Capability provider hosts (Runtime, health age, active LiteCowork Activation count)
Delegation tree
Effects and evidence levels
Artifacts/versions
Usage
Protocols/versions
Correlation IDs
```

## Global states

Every surface must define:
- loading
- empty
- partial data
- stale/offline
- reconnecting
- permission denied
- approval required
- conflict
- cancelled
- failure
- missing/default AgentBinding setup with draft preservation
- reduced-motion presentation

## Surface contracts

| Surface | Primary content | Primary actions | No-data/error behavior |
|---|---|---|---|
| Workspace selector/setup | Workspace name, replication scope, status | Create, select, update policy, archive when quiescent | Creation defaults to local-only; cloud scope is explicit; archive blockers name active Tasks/Automations |
| Home | Composer, recent Conversations/Tasks, Runtime availability | Start a Conversation, attach resources, reopen recent work | First-use guidance; offline local-only explanation; reconnect state with cached data labeled stale |
| Conversation | Ordered messages, attachments, inline Task cards | Reply, attach, steer linked Task, stop/cancel a running turn, retry a failed turn | Empty prompt; send failure preserves draft; message/task creation is idempotent; prior failed output remains provenance-tagged |
| Task list | Durable outcome, status, next required action, latest update | Filter, open, pause/resume, cancel, recover an eligible Step | Empty filter-specific state; cached/offline status is visibly stale |
| Task detail | Current spec revision, progress, artifacts, blockers, history | Steer, approve, cancel, request recovery, open artifact | Missing/archived resources are identified; no fabricated progress |
| Live Desk | Steps, active Attempts, inputs, capability activity, artifacts, verification | Inspect lane, respond, stop, open result | No lanes before Steps/Attempts exist; preserve last known state as stale when disconnected |
| Workbench | Selected versioned Artifact or actual provider UI | View/edit through owning provider, publish a new version | Unsupported preview offers download; provider loss does not imply artifact loss; stale publication keeps the draft and requires explicit rebase |
| Library | Saved/generated/uploaded/imported/linked resources | Search, open, promote, archive a linked reference, view archived resources | Empty state explains how to save/import; stale linked revisions are marked; archive removes the item from the default Library view without deleting its external source |
| Automations | Routines, triggers, next run, policy, recent occurrences and Task outcomes | Save/revise/run Routine; create/edit/run manual trigger/pause/resume/disable Automation | Missed/failed occurrence is explicit; duplicate trigger is shown once logically |
| Discover | LitePSM-backed user-facing offers and compatible agents | Inspect, connect/enable, grant required scope | LitePSM unavailable shows a dependency error; cached items are labeled stale |
| Inspector | IDs, revisions, Agent/Runtime/Environment, locks, grants, Effects, Evidence, protocols | Copy diagnostics, inspect provenance | Redact credentials and secret values; unavailable details are marked unknown |
| Settings | Workspace, Runtime/device, AgentBindings, connections, security, storage and preferences | Pair/revoke, enable/disable agents, configure, export/delete according to policy | Each setting shows whether it applies locally, to the Hub, or to the Workspace; profile availability may be stale when its Runtime is offline |

Provider `input_required` projection distinguishes a normal non-sensitive Form from an
External sign-in handoff. The Form UI shows a persistent “Do not enter passwords or
credentials here” note. Sensitive-looking field definitions and unsupported embedded MCP
methods stop with a typed blocker and a Connect/setup action. External sign-in displays
publisher identity and parsed destination origin before opening the system browser; after returning,
the user explicitly confirms completion. A successful click is not proof of authentication;
the provider task must confirm it.

## Task state language

Use one plain-language primary label mapped from the canonical Task status:

| Canonical state | User-facing label | User action |
|---|---|---|
| `READY` | Ready | Start, or wait for automatic admission |
| `RUNNING` | Working | Inspect or steer |
| `PAUSE_REQUESTED` | Pausing safely | Wait; inspect checkpoint, provider-invocation, verification, or Effect blocker |
| `PAUSED` | Paused | Resume or cancel |
| `WAITING_USER` | Waiting for you | Reply or approve |
| `BLOCKED` | Blocked | Resolve the named resource/policy blocker |
| `VERIFYING` | Checking the result | Wait; do not show completion yet |
| `NEEDS_USER` | Needs your decision | Review ambiguity or select a recovery action |
| `INCOMPLETE` | Not finished | See unmet acceptance criteria and revise or recover a Step |
| `COMPLETED` | Completed | Open verified outputs |
| `FAILED` | Failed | Review the terminal reason; recovery was exhausted, create follow-up Task if needed |
| `CANCEL_REQUESTED` | Stopping | Wait for Attempts, provider Invocations, VerificationRuns, and Effects to settle |
| `CANCELLED` | Cancelled | Review preserved work and effects |

The UI may combine state with blocker detail (`Waiting for your laptop — needs local
Chrome`) but does not define new domain state. `Planning` is a presentation of RUNNING when
a TASK_PLANNING session is active and no PlanRevision has been promoted; it is not a new
TaskStatus. Unsent composer content remains an Operator draft and is not a persisted Task.
Show `Getting ready` while the Task is READY and the planning session is not yet active.
Attempt and Step state appears in the Inspector or lane details.

`FAILED` is shown only after recoverable Step/Attempt options are exhausted. Recoverable
failures appear on the affected Step and expose a Step recovery action, never a Task retry
button. Pausing displays the stages actually underway: stopping admission, safe boundary,
checkpoint, provider-call settlement, active verification settlement when applicable,
Effect reconciliation, and lease release. Resume remains disabled until the pause is
settled and reports any blockers.

## Workspace and archive projection

- The active Workspace is explicit in the app shell; switching never changes ownership of a Conversation or Task.
- Replication policy is shown as scope, not as a security grant. Capability and secret access still require their own authorization.
- Policy updates do not imply remote deletion. The UI states that already-replicated copies remain until separately managed.
- Archive does not delete or hide history. Archived Workspaces are read-only; inbound channel messages are rejected with a clear channel-side response.
- Workspace instructions have a version history and a clear current revision. Updating them does not silently alter active Tasks.
- Adding a folder as an attachment is one-time. “Add to Workspace” creates a persistent WorkspaceRoot with a separate watch policy; root observation does not imply write access or cloud replication.
- Resource search results show freshness and available locations. A conflicted Resource exposes its revision branches; an unpinned reference is not resolved until a branch is explicitly pinned or a verified merge is created. Search matches do not silently enter agent context; the user/agent must attach selected ResourceRefs.

## Needs you inbox and notifications

The persistent Needs you surface projects open `UserRequest` and `Approval` records,
actionable Task blockers through distinct actions, and resolved history while source
records remain retained. The badge counts OPEN rows only. A blocker that already has a
linked UserRequest/Approval is shown once with that record's stable identity. Otherwise
use `(task_id, blocker_id)`; resolving the underlying record removes its active entry.
Counts include authorized unresolved items only, not notification-delivery retries or
nonactionable progress. Runtime-offline items link to device/dependency status; they
cannot imply that an approval wakes a machine. Cached counts are explicitly stale.
UserRequests contain originating
Conversation/Task/Attempt/session/invocation provenance, typed response schema or choices,
status, and expiry. A Task-originated request may have no Conversation and still appears
in the persistent Home “Needs you” inbox and its Task detail. A UserRequest response never
resolves an Approval; approval goes through TrustService's explicit assurance and decision
path. Notifications are preference-driven,
deduplicated deliveries with bounded retries and channel fallback. A sent notification is
only a transport acknowledgement and never a success/completion state.

When a channel has owner-granted `RESPOND` and supports exact reply-to references, a single
eligible FORM question may be answered by replying to its delivered prompt. Otherwise the
notification links to the Operator inbox. The channel never answers an implicitly selected
“latest” request, an Approval, or provider sign-in. Ambiguous delivery is shown as
unconfirmed and is not resent until reconciled.

Channel settings show the assigned Runtime and ingress status. During cursor recovery the
status reads “Reconnecting; checking missed messages” and the channel does not process new
ingress. A host move with confirmed replay/transfer shows continuous service; a move that
the owner explicitly accepts without continuity shows a persistent “Possible message gap
since [time]” history entry. A later rescan never changes that historical statement.

## Cross-device continuity

- A Conversation is opened by its stable ID on every surface; a channel thread is only a
  mapping to it.
- Task detail shows the current Runtime in ordinary language and a precise blocker if
  unavailable.
- A handoff progress label is driven by persisted `Handoff` phase. The UI changes the
  execution location only after the target Attempt holds the new authoritative lease.
- During disconnected operation, cached projections are read-only unless the Runtime has
  a valid, policy-eligible local authority. Staleness time is visible.
- Conflicting TaskSpec revisions are presented as unresolved versions; the UI never
  silently chooses a winner.

## Task creation from the composer

There is one composer and no chat-versus-task mode switch. A clearly outcome-oriented user
request or explicit create-Task action creates a structured Task through the normal
TaskService. The bound conversational agent may request Task materialization when intent
is clear. If the request is ambiguous about whether durable execution is wanted, continue
the Conversation or ask one concise question. Task creation becomes visible only after
its message, initial TaskSpecRevision, and event commit atomically. Core does not run a
separate hidden intent/planning model.
Before that commit, admission verifies an eligible selected AgentBinding. If missing or
unavailable, no ConversationTurn or Task is created; the complete draft remains in the
composer and the user is routed through explicit Agent setup.

## Initial planning projection

While the lead planning session is active, show “Planning” with session status and stop/steer affordances permitted by Task policy. Do not show a fabricated worker lane, environment, runtime handoff, progress percentage, or artifact. Once PlanRevision and Step Attempts exist, Live Desk lanes may appear from their persisted state.

## Honesty rules

- connector/API use is shown as source activity, not fake GUI clicks.
- actual desktop/browser computer use may show the real surface.
- child branch appears only after child Attempt exists.
- artifact appears only after ArtifactVersion exists.
- verification checkmark appears only after verifier pass.
- cloud location changes only after new authoritative lease/Attempt exists.
