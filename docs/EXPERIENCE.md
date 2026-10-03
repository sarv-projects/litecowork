# Experience Architecture

## Information architecture

Primary navigation:

```text
Home
Tasks
Library
Automations
Discover
```

Advanced technical detail lives in Inspector/Settings/Discover details. A Workspace selector is available in the app shell when more than one Workspace exists; it does not add a sixth primary navigation destination.

## Workspace setup and selection

Workspace creation is part of first-run setup and Settings. The user sees the replication scope in plain language before choosing a policy:

- `LOCAL_ONLY`: stays on this Runtime.
- `METADATA_ONLY`: syncs Workspace and Task metadata, not file contents.
- `ACTIVE_TASK_INPUTS`: transfers only inputs and outputs required by cloud-eligible active Tasks.
- `SELECTED_FOLDERS`: transfers selected, revision-pinned folders and Task outputs.
- `FULL_WORKSPACE`: explicit whole-Workspace replication with a clear storage/privacy summary.

Creation defaults to `LOCAL_ONLY`; cloud enablement is a separate explicit action. Changing policy applies prospectively and explains that copies already transferred are retained. Archive is a separate, confirmed action available only after Tasks are terminal and Automations disabled. Archived Workspaces remain browsable and show a persistent read-only banner.

States: first-run, create in progress, policy saving, cloud unavailable, archive blocked with named active Tasks/Automations, archived/read-only, stale policy version/conflict.

## User interaction flows

- **Create Workspace:** explain local-only default and available replication scopes; create only after the user commits the choice.
- **Enable cloud:** show eligible content, storage implications, required Runtime/agent/secrets, and any unavailable inputs; save the policy only after explicit confirmation. If the update conflicts or fails, retain the old selection.
- **Archive:** show the exact nonterminal Tasks or enabled Automations that block archive. After successful archive, retain navigation/read access and show the read-only banner.
- **Connect a capability:** move from Discover details to requested scope/approval, then show it as ready only after activation health succeeds.
- **Approve an action:** present exact action, target, scope, risk, and required assurance; stale/expired approvals require a fresh request rather than replaying approval.
- **Change lead agent:** choose an eligible Workspace binding, submit a versioned request, show existing Attempts draining under their pinned identity, and show the new lead only after the assignment event.
- **Create or edit an Automation:** preview trigger/timezone, Task template, overlap/retry behavior, placement, and notifications; save edits as a new immutable revision. Each occurrence detail displays the revision it pinned.
- **Pair a Runtime:** explain its role and advertised local resources, issue a short-lived one-use pairing action, and show it as available only after identity verification.

## Home

Purpose: start or resume work.

States:
- normal/recent Tasks
- first-use empty state
- offline local-only
- reconnecting
- cloud unavailable

Composer:

```text
[ + ] [Auto ▾ optional] Ask or describe a task... [send]
```

`+` menu:
- Add files
- Add folder
- Screenshot
- From Library
- Connect an app
- More

Advanced popover may expose Agent, Model (when agent exposes it), Reasoning, Execution preference, Budget, Access and Capabilities.

## Conversation

Conversation is the human continuity surface. Task cards appear inline when durable work materializes.

Task card states:
- Planning (Task is durable; lead planning session is active; no Step lane exists yet)
- Ready
- Running
- Waiting for you
- Blocked
- Verifying
- Completed
- Failed
- Cancelled

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
- actions: steer, request a lead-agent change, cancel, retry/recover where valid

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

Screens:
- list
- create/edit; each definition edit creates an immutable AutomationRevision
- run now for ManualTrigger; run/occurrence history with the pinned revision for each run
- next scheduled run
- status/paused/disabled
- last result

Automation UI explains that each run creates a normal Task.

## Discover

Embedded LitePSM-oriented experience with user categories:

```text
Apps
Capabilities
Skills
Agents
```

Default card shows user value and compatibility. Expanded detail shows provider/package source/version/digest/permissions/health/protocol.

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
- reduced-motion presentation

## Surface contracts

| Surface | Primary content | Primary actions | No-data/error behavior |
|---|---|---|---|
| Workspace selector/setup | Workspace name, replication scope, status | Create, select, update policy, archive when quiescent | Creation defaults to local-only; cloud scope is explicit; archive blockers name active Tasks/Automations |
| Home | Composer, recent Conversations/Tasks, Runtime availability | Start a Conversation, attach resources, reopen recent work | First-use guidance; offline local-only explanation; reconnect state with cached data labeled stale |
| Conversation | Ordered messages, attachments, inline Task cards | Reply, attach, steer linked Task, cancel where allowed | Empty prompt; send failure preserves draft; message/task creation is idempotent |
| Task list | Durable outcome, status, next required action, latest update | Filter, open, cancel, retry/recover if legal | Empty filter-specific state; cached/offline status is visibly stale |
| Task detail | Current spec revision, progress, artifacts, blockers, history | Steer, approve, cancel, request recovery, open artifact | Missing/archived resources are identified; no fabricated progress |
| Live Desk | Steps, active Attempts, inputs, capability activity, artifacts, verification | Inspect lane, respond, stop, open result | No lanes before Steps/Attempts exist; preserve last known state as stale when disconnected |
| Workbench | Selected versioned Artifact or actual provider UI | View/edit through owning provider, publish a new version | Unsupported preview offers download; provider loss does not imply artifact loss; stale publication keeps the draft and requires explicit rebase |
| Library | Saved/generated/uploaded/imported/linked resources | Search, open, promote, archive a linked reference, view archived resources | Empty state explains how to save/import; stale linked revisions are marked; archive removes the item from the default Library view without deleting its external source |
| Automations | Trigger, next run, policy, recent occurrences | Create, edit, run manual trigger, pause, resume, disable | Missed/failed occurrence is explicit; duplicate trigger is shown once logically |
| Discover | LitePSM-backed user-facing offers and compatible agents | Inspect, connect/enable, grant required scope | LitePSM unavailable shows a dependency error; cached items are labeled stale |
| Inspector | IDs, revisions, Agent/Runtime/Environment, locks, grants, Effects, Evidence, protocols | Copy diagnostics, inspect provenance | Redact credentials and secret values; unavailable details are marked unknown |
| Settings | Workspace, Runtime/device, AgentBindings, connections, security, storage and preferences | Pair/revoke, enable/disable agents, configure, export/delete according to policy | Each setting shows whether it applies locally, to the Hub, or to the Workspace; profile availability may be stale when its Runtime is offline |

## Task state language

Use one plain-language primary label mapped from the canonical Task status:

| Canonical state | User-facing label | User action |
|---|---|---|
| `DRAFT` | Draft | Review or start |
| `READY` | Ready | Start, or wait for automatic admission |
| `RUNNING` | Working | Inspect or steer |
| `WAITING_USER` | Waiting for you | Reply or approve |
| `BLOCKED` | Blocked | Resolve the named resource/policy blocker |
| `VERIFYING` | Checking the result | Wait; do not show completion yet |
| `NEEDS_USER` | Needs your decision | Review ambiguity or select a recovery action |
| `INCOMPLETE` | Not finished | See unmet acceptance criteria and retry/revise |
| `COMPLETED` | Completed | Open verified outputs |
| `FAILED` | Failed | Review reason; create a new attempt only if recovery permits |
| `CANCEL_REQUESTED` | Stopping | Wait for attempts/effects to settle |
| `CANCELLED` | Cancelled | Review preserved work and effects |

The UI may combine state with blocker detail (`Waiting for your laptop — needs local
Chrome`) but does not define new domain state. `Planning` is a presentation of RUNNING when
a LEAD_PLANNING session is active and no PlanRevision has been promoted; it is not a new
TaskStatus. Show `Getting ready` while the Task is READY and the planning session is not
yet active. Attempt and Step state appears in the Inspector or lane details.

## Workspace and archive projection

- The active Workspace is explicit in the app shell; switching never changes ownership of a Conversation or Task.
- Replication policy is shown as scope, not as a security grant. Capability and secret access still require their own authorization.
- Policy updates do not imply remote deletion. The UI states that already-replicated copies remain until separately managed.
- Archive does not delete or hide history. Archived Workspaces are read-only; inbound channel messages are rejected with a clear channel-side response.

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

## Initial planning projection

While the lead planning session is active, show “Planning” with session status and stop/steer affordances permitted by Task policy. Do not show a fabricated worker lane, environment, runtime handoff, progress percentage, or artifact. Once PlanRevision and Step Attempts exist, Live Desk lanes may appear from their persisted state.

## Honesty rules

- connector/API use is shown as source activity, not fake GUI clicks.
- actual desktop/browser computer use may show the real surface.
- child branch appears only after child Attempt exists.
- artifact appears only after ArtifactVersion exists.
- verification checkmark appears only after verifier pass.
- cloud location changes only after new authoritative lease/Attempt exists.
