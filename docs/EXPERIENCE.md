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

Advanced technical detail lives in Inspector/Settings/Discover details.

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
- actions: steer, cancel, retry/recover where valid

## Live Desk

Live Desk projects real work into outcome-oriented lanes.

Default hides agent names and protocol internals. Each lane corresponds to real Step/Attempt state.

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
- Templates
- Skills
- Saved workflows

Saving/promotion is explicit. Linked resources show provider and external revision/availability.

## Automations

Screens:
- list
- create/edit
- run/occurrence history
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
| Home | Composer, recent Conversations/Tasks, Runtime availability | Start a Conversation, attach resources, reopen recent work | First-use guidance; offline local-only explanation; reconnect state with cached data labeled stale |
| Conversation | Ordered messages, attachments, inline Task cards | Reply, attach, steer linked Task, cancel where allowed | Empty prompt; send failure preserves draft; message/task creation is idempotent |
| Task list | Durable outcome, status, next required action, latest update | Filter, open, cancel, retry/recover if legal | Empty filter-specific state; cached/offline status is visibly stale |
| Task detail | Current spec revision, progress, artifacts, blockers, history | Steer, approve, cancel, request recovery, open artifact | Missing/archived resources are identified; no fabricated progress |
| Live Desk | Steps, active Attempts, inputs, capability activity, artifacts, verification | Inspect lane, respond, stop, open result | No lanes before Steps/Attempts exist; preserve last known state as stale when disconnected |
| Workbench | Selected versioned Artifact or actual provider UI | View/edit through owning provider, publish a new version | Unsupported preview offers download; provider loss does not imply artifact loss |
| Library | Saved/generated/uploaded/imported/linked resources | Search, open, promote, archive, remove link | Empty state explains how to save/import; stale linked revisions are marked |
| Automations | Trigger, next run, policy, recent occurrences | Create, edit, pause, resume, disable | Missed/failed occurrence is explicit; duplicate trigger is shown once logically |
| Discover | LitePSM-backed user-facing offers and compatible agents | Inspect, connect/enable, grant required scope | LitePSM unavailable shows a dependency error; cached items are labeled stale |
| Inspector | IDs, revisions, Agent/Runtime/Environment, locks, grants, Effects, Evidence, protocols | Copy diagnostics, inspect provenance | Redact credentials and secret values; unavailable details are marked unknown |
| Settings | Workspace, Runtime/device, connections, security, storage and preferences | Pair/revoke, configure, export/delete according to policy | Each setting shows whether it applies locally, to the Hub, or to the Workspace |

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
Chrome`) but does not define new domain state. Attempt and Step state appears in the
Inspector or lane details.

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

There is one composer and no Chat/Cowork mode switch. A clearly outcome-oriented user
request or explicit create-Task action creates a structured Task through the normal
TaskService. The bound conversational agent may request Task materialization when intent
is clear. If the request is ambiguous about whether durable execution is wanted, continue
the Conversation or ask one concise question. Task creation becomes visible only after
its message, initial TaskSpecRevision, and event commit atomically. Core does not run a
separate hidden intent/planning model.

## Honesty rules

- connector/API use is shown as source activity, not fake GUI clicks.
- actual desktop/browser computer use may show the real surface.
- child branch appears only after child Attempt exists.
- artifact appears only after ArtifactVersion exists.
- verification checkmark appears only after verifier pass.
- cloud location changes only after new authoritative lease/Attempt exists.
