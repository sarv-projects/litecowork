# Experience Architecture

## Information architecture

Primary navigation:

```text
Home
Needs You
Coworkers
Work
Automations
Library
Discover
```

The desktop sidebar also contains search and a persistent Recent Conversations list. Work
is the user-facing label for durable Tasks and contains Active, Waiting, Scheduled, Done,
and All filters. Coworkers is an identity/context surface, not an execution container.
Needs You is a first-class destination with a pending count for approvals, expired logins,
required local Runtimes, ambiguous Effects, TaskSpec conflicts, user choices, and
verification requests. Advanced technical detail lives in Inspector/Settings/Discover
details. A Workspace selector is available in the app shell when more than one Workspace
exists.

**Current desktop/local implementation boundary:** The mounted page currently queries the
authenticated Task list for `WAITING_USER`, `NEEDS_USER`, and `BLOCKED`, then links each
persisted Task to its current detail view. This is a Task-attention view only. It is not
the full Needs You inbox: Approval/UserRequest aggregation, notification/deep-link
handling, blocker-specific actions, and resolve/dismiss mutations are not connected to
this page. It must say so and must not imply that opening a Task resolves anything.

## V1 client scope

V1 interaction implementation targets the desktop Operator. Responsive layouts inform
future clients and constrain desktop accessibility, but native mobile/web clients are not
release targets until after desktop, cloud Runtime, and remote Runtime qualification. Mobile
flows in domain continuity tests use an available desktop Operator or qualified channel;
they do not claim mobile rendering or app-store support.

## Workspace setup and selection

Workspace creation is part of first-run setup and Settings. Desktop/local V1 creates and
operates Workspaces with `LOCAL_ONLY` storage. Cloud continuation, Workspace replication,
and remote Runtimes are post-V1; their policy values remain architecture contracts but are
not selectable in this release.

The future replication values are:

- `LOCAL_ONLY`: stays on this Runtime.
- `METADATA_ONLY`: syncs Workspace and Task metadata, not file contents.
- `ACTIVE_TASK_INPUTS`: transfers only inputs and outputs required by cloud-eligible active Tasks.
- `SELECTED_FOLDERS`: transfers resources under the selected persistent WorkspaceRoots and Task outputs; newly observed revisions remain in scope while the root grant and policy remain active.
- `FULL_WORKSPACE`: explicit whole-Workspace replication with a clear storage/privacy summary.

Desktop V1 does not enable replication. If an existing Workspace contains a non-local saved
policy, Settings labels it inactive and offers an explicit return to `LOCAL_ONLY`; it does
not silently change the stored policy. Archive is a separate, confirmed action available
only after Tasks are terminal and Automations disabled. Quiescence also requires
Conversation turns and scoped Invocations to be settled, no active grants/SecretLeases/control
leases, and persistent Environments with no live workload. Authorized watchers/triggers stop
before the read-only transition. Retained Environment state may remain suspended under
storage/backup policy; archive never silently destroys it. Unknown provider quiescence
blocks archive with `WORKSPACE_NOT_QUIESCENT`. Archived Workspaces remain browsable and
show a persistent read-only banner.

States: first-run, create in progress, local-only storage, previously saved replication
policy inactive, policy reset in progress, archive blocked with named active
Tasks/Automations, archived/read-only, stale policy version/conflict.

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
[Alex ▾] What should we work on?                         [ + ] [Send →]
```

The default composer addresses the selected Coworker (normally the Workspace primary) and
keeps agent/runtime controls out of the first interaction. A Coworker selector is shown
only when the Workspace has more than one active Coworker. `Advanced` exposes Lead Agent,
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

## Conversation

Conversation uses the same presentation hierarchy for a simple answer and a durable Task:
answer/outcome first, concise real activity second, expandable work and source detail third,
Inspector last. There is no separate nontechnical/technical product mode. “Context used”
is an optional read-only disclosure listing the exact attachments and retrieved/instruction
sources resolved for that turn; it never claims that an Agent read every available file.
Transient streamed text is marked in progress until the ConversationMessage commits.
Reconnect replaces stale local state from the authorized projection and does not replay
old typing or activity animations. See [`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md).
The committed semantic answer appears as soon as the ConversationMessage is available.
An optional RichPresentation may upgrade that exact message after validation; while it is
fetching, unavailable, unsupported, or rejected, show the complete semantic Markdown and
a quiet non-error fallback. Rich blocks do not replace the message, create Task state,
perform actions, or delay turn completion. `Simple` suppresses model-composed decoration
but retains real Artifact, Task, UserRequest, Approval, and other required host controls.
`Rich` requests composition but cannot require a renderer or block completion.
Side conversations/branches are not part of v1. If introduced later, they must pin their
own context snapshot and cannot alter a parent Conversation or Task unless the user
explicitly applies a reviewed result.

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
- for a `READY`, unplanned Task, an objective editor that appends a TaskSpecRevision; it is not an inline mutation and is unavailable once planning or an accepted Plan exists
- for an unplanned Task, an explicit read-only **Check readiness** action may show the current Task-version's sanitized local planning blockers; it is not a Start action, never starts an AgentSession, and always states that dispatch is unavailable in this desktop slice
- offer a lazy, read-only Task specification history disclosure with exact objective, author, timestamp, and parent revision; do not expose restore/edit controls from history
- compare the history head with the loaded TaskSpec revision; label a newer head “Latest saved,” disclose an older/incomplete response, and offer explicit Task reload rather than mislabeling a revision “Current”
- disable that Task reload while an objective draft or unresolved save request is open, preserving user-authored edits
- keep the objective, current plain-language status, and whether a current plan is saved visible without expansion
- when a Task has Coworker origin, show the name from the exact immutable CoworkerRevision it pins; if that revision cannot be loaded, show the pinned revision identity and a clear unavailable state rather than the Coworker's current name
- put timestamps, current TaskSpec/Plan revision metadata, exact pinned Resource inputs, and persisted Plan Steps in a collapsed-by-default `Work details` disclosure
- use “inputs” for pinned Resource references; do not assume every Resource is a file
- show the current accepted PlanRevision and its materialized Step statuses only when present
- clear stale-plan notice when the plan pins an older TaskSpecRevision
- progress/workstreams
- outputs/artifacts
- approvals/blockers
- activity timeline
- execution location
- actions: steer, request a lead-agent change, pause/resume, cancel, recover a failed Step where valid

The saved-Task editor is intentionally narrow in the first desktop slice. Save creates a
new immutable revision using optimistic Task versioning. Unedited specification fields
remain pinned as-is. A conflict keeps the user's draft visible and offers a reload of the
current Task; the UI never claims a revision was saved until the Operator receipt and the
subsequent Task read agree. Saving does not start planning or execution.

The first desktop Task detail keeps its metadata, input list, and persisted Plan Steps
inside `Work details`, collapsed by default. Expanding it reveals only data loaded and
validated from the authenticated Task/Resource/Plan projections. The view does not infer
Attempts, Evidence, verification, or progress from a Plan Step. This disclosure state is a
local presentation choice and does not change Task state.

A pinned Task input may offer a lazy **Preview pinned text** disclosure. Opening it reads
only the exact selected Workspace/Resource/revision through the authenticated desktop
preview command, and renders valid UTF-8 text up to 1 MiB as escaped plain text. The UI
labels the exact revision and states that previewing does not attach content to an Agent or
change the saved Task. Unsupported media, invalid UTF-8, oversized content, unavailable
bytes, or a historical revision that is no longer the Resource's current head produces an
explicit error and retry for that same pin; the UI never substitutes the newer head. This
preview is a user-visible read, not agent context construction, semantic RAG, or execution.

The current saved outcome/activity panel uses a finite authenticated snapshot. It shows the
saved objective/status, committed output records, and a short activity preview; full source
IDs stay collapsed. It labels `CURRENT` only when the persisted source records were read
from one consistent SQLite snapshot; stale/unknown values remain distinct if a later
projection source reports them. Snapshot time is never called verified Evidence. The panel
cannot show result text or blocker details absent from the projection, and refreshing it
does not imply a live stream.

## Live Desk

Live Desk projects real work into outcome-oriented lanes.

Default hides agent names and protocol internals. A Task opens with its outcome, current
plain-language state, next required action, and newest committed output or blocker. A
compact activity summary shows a few recent real updates and offers `View activity`;
expanding it reveals persisted Step/Attempt lanes. `Details` exposes worker names and the
delegation tree; Inspector exposes protocols and runtime machinery. Users may keep activity
expanded for the current Task. This changes only local presentation state.

A planning status may appear before plan acceptance, but no work lane appears until real
Steps exist. Plan Steps may appear in Task details before Attempts exist; worker lanes
require real Step/Attempt state.

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

When an Artifact is selected, the header keeps its stable title, current immutable version,
source Task, verification state, and dirty-draft state visible. Version history is available
without leaving the work. Compare appears only for supported renderer types. Restoring an
older version publishes its content as a new version after expected-version validation;
history is never destructively rewound. A renderer/provider failure distinguishes unavailable
preview from unavailable content and retains any unsaved draft. Editing follows the owning
provider contract and publication creates an ordinary ArtifactVersion.

If publishing a text draft returns an ambiguous result, the Workbench keeps the original
request identity and exact payload. It locks the draft against editing and ordinary discard,
and offers **Retry unchanged**; retry sends the same request ID and expected Artifact versions
so the owner can resolve whether the immutable version committed without creating a duplicate.
A definitive version conflict instead keeps the draft for review and requires checking the
latest version and explicitly rebasing before a new publish request. If the owner explicitly
confirms closing a changed unpublished draft with no pending request, that draft is discarded.
Close is disabled while a publish request is pending. An application restart or other
external unmount can still discard the in-memory retry identity; after reopening, the owner
must inspect Artifact history/current head before composing another edit.

The current desktop Workbench remains narrower than this target: it selects exact
committed versions, previews supported bounded UTF-8 text as escaped text, copies selected
text, compares supported text with another explicitly selected committed version side by
side using the same renderer. A second comparison display aligns literal changed lines and
reports added/removed line counts for small inputs; its bounded diff has a side-by-side
fallback for larger or unusually long lines. It does not infer semantic changes or publish
anything. The Workbench also offers an authorized immutable
native Save As for the exact selected managed ArtifactVersion up to 10 MiB, plus
content/provenance metadata. Save As checks the selected version identity and digest through
the authenticated local bridge; the daemon verifies stored bytes and the native process
checks media type and length before writing. Cancel writes nothing, and linked
external/oversized content is not fetched or saved through this path. It also supports editing and restoring only managed
`text/plain` content up to 1 MiB through the mounted authenticated append path.
Restore asks for explicit confirmation, copies a selected historical text version into a
draft based on the freshly read current head, and requires a separate Publish action. It
never rewinds or overwrites history, and stale publication preserves the draft. The
current append contract records `user.text_edit` but does not persist a separate
restored-from relation. This is not a general document editor; external content, HTML,
office files, and other media remain read-only/download-only.

## Library

The local Workbench offers owner-confirmed **Save to Library** for TRANSIENT Artifacts
and **Archive Artifact** for SAVED Artifacts. Confirmation names the Artifact, explains
that history is preserved, and states that archive prevents future content publication.
Linked content remains with its provider; these actions neither fetch nor delete it.
The status changes only after a validated committed response. A stale head asks for refresh
and review, while an unconfirmed result offers the unchanged request for idempotent retry.
Library status changes are disabled while a text draft is open or publishing. Archive is
terminal and has no restore/unarchive action; restoring historical text applies only to
non-archived Artifacts through ordinary new-version publication.

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
For a current managed file Resource up to 10 MiB, Library offers **Save original…**. The
native dialog receives a sanitized suggested name; cancellation reads no content. Save
revalidates the exact current Resource revision, digest, byte length, media type, local
managed provider, and ContextDocument status before writing through the native atomic-save
path. The UI receives only Saved/Cancelled status, never file bytes or the selected
destination path. Stale, inactive, non-file, external, unavailable, or oversized content is
not saved. ZIP archives remain opaque and may be saved byte-for-byte as uploaded; Library
does not extract them. The action creates no new Resource revision or Task history.
In ContextDocument revision history, the owner can inspect the current status and request
`ACTIVE -> REVOKED` or `REVOKED -> ACTIVE` with the displayed Resource version. Confirmation
states that revocation blocks future LiteCowork reads and index updates while retaining
stored bytes, and cannot recall content already delivered to an agent. Ambiguous failures
offer a retry using the same idempotency key; a version conflict reloads metadata. This
control does not edit ordinary Resource metadata, delete or purge bytes, or invalidate
existing native sessions. Deletion controls remain unavailable. If a replacement file is
selected or the text draft differs from its loaded baseline, closing the revision editor
first asks whether to keep editing or discard and close. An unchanged text preview closes
without a prompt. Closing is unavailable while a revision upload is in progress; upload
commit/conflict behavior remains owned by the revision protocol. A ContextDocument
availability change is also blocked while a local draft exists; after an ambiguous status
response, editing and closing remain paused until the same request is resolved or retried.
The Library also offers **Write a Workspace note**. The owner supplies a title and up to
64 KiB of UTF-8 text; the desktop creates an immutable managed `FILE` Resource through the
normal resumable upload protocol with `WORKSPACE_NOTES` metadata pinned to the selected
Workspace. The note appears in the ordinary Resource catalog and uses the existing
revision-history/edit/revoke controls. V1 exposes no personal, Coworker, or Goal scope
selector. The UI explains that the note is scoped to the Workspace and is not automatically
attached to agent context; creating one does not claim semantic RAG.
Resource history offers **Compare text revisions** only for two explicitly selected,
committed revisions of this same Resource whose recorded media type is `text/plain` or
Markdown and whose individual size is at most 1 MiB. Both exact pins are read lazily through
the authenticated local content route. The desktop presents labeled side-by-side escaped
text, not a generated changed-line diff; it never renders HTML/SVG, chooses a head for the
owner, or substitutes newer bytes. A failed comparison leaves the existing revision
history/editor view intact and reports a retryable, revision-specific error. Selection is
limited to loaded history pages, which must be loaded explicitly for older entries.
Artifact detail exposes version history, provenance, and verification state; comparisons
use exact immutable versions and their source ResourceRevisions. Managed text supports an
optional bounded literal line comparison; unsupported or oversized input keeps the
side-by-side view and never implies semantic change analysis. Failure to read a prior
comparison version preserves the selected authorized preview and Artifact history; denial
for the selected version itself hides that version's metadata/content. Refresh is disabled while a
draft is open, and closing a dirty Artifact view requires explicit discard confirmation.
Restore-as-new-version uses the Workbench rules. Unsupported types remain downloadable when
authorized.

Small managed CSV/TSV Artifact previews use a bounded table view (500 rows, 32 columns,
8,192 characters per cell, and the existing 1 MiB transfer ceiling). Cells are rendered as
escaped text; malformed or out-of-bound content falls back to the original text, which is
also available from the table view. This does not imply spreadsheet formula evaluation or
editing support.

The desktop/local intake slice shows a keyset-paginated Resource catalog and
Workspace-wide metadata/content search. Users load the first page quickly and request
older rows as needed. Search results may be explicitly pinned as Task inputs; the action
records the exact Resource revision and does not fetch its content. The Home composer
shows selected names and allows removal before saving. A text Preview action requests the
selected current revision through the authenticated Operator API and renders only valid
UTF-8 text-like content up to 1 MiB as escaped plain text. HTML, SVG, PDF, Office
documents, ZIPs and other binary/active content are not rendered by this first preview.
Preview is Workspace-scoped and no-store. A pinned Task input is a reference, not a grant:
a future ContextPlanner must re-evaluate availability, policy and sensitivity before
content reaches an agent. The full Library contract continues to require authorized
download/renderers, durable indexing, revision history and root-aware freshness as their
implementation stories land.

The current desktop Library also exposes the existing exact `kind` and `freshness` search
filters. They are applied by the authenticated Resource search route before pagination;
changing either filter clears the previous page/cursor and starts a new query. They refine
the selected search mode but do not change its content-reading limits or attach results to
a Task. With metadata mode selected, a user can leave the query empty to browse by filters;
content-search modes still require search terms. This UI source is not yet built or verified.

For a selected Resource row, Library offers **Rebuild local text index** as an explicit
owner action. It submits the exact revision ID and content digest currently displayed by
the catalog, so a stale row cannot rebuild a newer revision. While a request is active the
button reports that state; a confirmed result announces either that the encrypted local
index was rebuilt or a plain-language reason the file cannot be indexed. If the response
is ambiguous, retry uses the same request ID; a changed Resource head instead asks the
owner to reload. The action returns no file text or search terms. ZIP and unsupported
formats remain intact and are reported as not indexable; this control does not imply ZIP
extraction, semantic RAG, or background indexing. The UI source has not been built or
verified.

## Automations

The page has three views: `Routines | Automations | Runs`.

Routine cards show immutable current revision, typed inputs, outputs/acceptance criteria,
capability requirements, placement eligibility, and actions to run, revise, duplicate,
archive, or create an Automation. Saving work as a Routine opens a redacted Operator
draft; task-specific paths, records, secrets, and incidental context are removed or
converted to typed inputs before the user commits it. An unsaved draft is not executable.
Manual Run renders bounded text/choice inputs and same-Workspace Resource selectors from
the pinned Routine schema. Selecting a Resource pins its exact immutable revision; the UI
never asks the owner to type Resource IDs. The action creates one READY Task only and
clearly says that planning and agent execution have not started.

Automation cards show all triggers, TriggerHost, next occurrence, independent execution
placement, timezone/misfire policy, dependency blockers (for example “Needs this computer
— Excel and D:\\Finance”), pause state, and last Task outcome. Runs is occurrence and Task
history; occurrence COMPLETED requires the linked Task to be COMPLETED. A schedule with
no future runs does not imply all of its Tasks succeeded. Due work that cannot
run is visibly `Waiting for dependency`, not silently skipped or left indistinguishable
from a future schedule.

Routine cards include revision-specific health, recent terminal-run outcomes, observed
duration/cost confidence, dependency freshness, and drift state. New or insufficiently
sampled definitions say “Not enough runs yet”. A drifted dependency names its evidence and
offers Review, Let worker prepare a repair proposal, or Disable; it never repairs or
publishes a Skill automatically. Updating a Skill or Routine requires the existing review
and revision flow.

Each Automation run creates an ordinary Task from the pinned RoutineRevision. Users must
confirm schedule creation and material updates. `Run now` is shown when an Automation has
a ManualTrigger and the local TriggerHost path is available. It works while the Automation
is PAUSED, creates one request-idempotent occurrence plus a saved READY Task, and never
enables recurring triggers. Otherwise the user can run its pinned Routine directly,
without creating an AutomationOccurrence.

## Discover

Embedded LiteSPM-oriented experience with user categories:

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
| Context used | Exact context sources resolved for a turn/Task | Open authorized source revision; navigate to context settings | No source content on permission loss; unknown provider provenance is stated; no claim that all history was read |
| Task list | Durable outcome, status, next required action, latest update | Filter, open, pause/resume, cancel, recover an eligible Step | Empty filter-specific state; cached/offline status is visibly stale |
| Task detail | Current spec revision, progress, artifacts, blockers, history | Steer, approve, cancel, request recovery, open artifact | Missing/archived resources are identified; no fabricated progress |
| Live Desk | Steps, active Attempts, inputs, capability activity, artifacts, verification | Inspect lane, respond, stop, open result | No lanes before Steps/Attempts exist; preserve last known state as stale when disconnected |
| Workbench | Selected versioned Artifact or actual provider UI | View/edit through owning provider, publish a new version | Unsupported preview offers download; provider loss does not imply artifact loss; stale publication keeps the draft and requires explicit rebase |
| Library | Saved/generated/uploaded/imported/linked resources | Search, open, promote, archive a linked reference, view archived resources | Empty state explains how to save/import; stale linked revisions are marked; archive removes the item from the default Library view without deleting its external source |
| Automations | Routines, triggers, next run, policy, recent occurrences and Task outcomes | Save/revise/run Routine; create/edit/run manual trigger/pause/resume/disable Automation | Missed/failed occurrence is explicit; duplicate trigger is shown once logically |
| Discover | LiteSPM-backed user-facing offers and compatible agents | Inspect, connect/enable, grant required scope | LiteSPM unavailable shows a dependency error; cached items are labeled stale |
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
- Workspace instructions are edited in Settings as plain UTF-8 text (64 KiB maximum) and committed as immutable revisions backed by a pinned Resource; the UI shows revision history. Updating them does not silently alter active Tasks, and TaskSpec pinning is required before this guidance reaches Task sessions.
- Adding a folder as an attachment is one-time. “Add to Workspace” creates a persistent WorkspaceRoot with a separate watch policy; root observation does not imply write access or cloud replication.
- Persistent-folder rows show WorkspaceRoot lifecycle status separately from the last committed ResourceLocation availability. Availability is an observed value, not a live Runtime probe; `PAUSED` must not hide an `OFFLINE`/`UNAVAILABLE` location, and the view never exposes a local path or raw file identity.
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

### Desktop implementation staging

The current desktop increment offers **Save Task** against the selected active Workspace
and its explicit current default lead binding. The owner can select existing Library
Resources as exact revision-pinned inputs; selections remain Workspace-scoped. Saving
commits the standalone Task envelope and inputs, then opens Work detail. `READY` with no
current PlanRevision is rendered as “Ready” with the note “No accepted plan is currently
saved for this Task.” This staged action does not start a
planning session, read Resource contents for an agent, or imply agent work; the normal
product flow above remains the target once planner/session admission is integrated. If
the local Operator response is ambiguous, an in-window retry reuses the same RequestId
only for the same Workspace, objective, lead binding, and ordered input references.
Changing any pinned field creates a new RequestId.

## Initial planning projection

While the lead planning session is active, show “Planning” with session status and stop/steer affordances permitted by Task policy. Do not show a fabricated worker lane, environment, runtime handoff, progress percentage, or artifact. Once PlanRevision and Step Attempts exist, Live Desk lanes may appear from their persisted state.

## Honesty rules

- connector/API use is shown as source activity, not fake GUI clicks.
- actual desktop/browser computer use may show the real surface.
- child branch appears only after child Attempt exists.
- artifact appears only after ArtifactVersion exists.
- verification checkmark appears only after verifier pass.
- cloud location changes only after new authoritative lease/Attempt exists.

## Coworkers, Work, and responsibility surfaces

### Coworker page

The page answers “who is helping, what is it responsible for, and what does it know?”
Its default order is identity/presence, Active/Scheduled/Done work, linked Goals,
user-editable context, connected access, autonomy summaries, and lead/worker setup.
Presence is derived from real Task/session/Runtime state and shows separate proactive
status (`ACTIVE`/`PAUSED`), activity (`AVAILABLE`, `PLANNING`, `WORKING`, `WAITING`,
`NEEDS_YOU`), and Runtime availability (`AVAILABLE`, `DEGRADED`, `OFFLINE`, `UNKNOWN`);
it is not a social simulation.

Create/edit fields are Coworker name, optional pinned avatar Resource revision, role
description, default lead binding, delegation strategy, enabled worker-profile allowlist,
lead-failover default, narrow interaction defaults, context policy, and notification
policy. Save uses expected version and creates an immutable
revision. Archive explains why active Coworker Automations/Tasks block it and retains
linked history. Pausing stops proactive/new scheduled admission only; the UI says active
Tasks continue under their own policy.

### Worker settings

Normal users choose how the Coworker delegates: “Let the lead use its own approach”,
“Balance native and available workers”, or “Prefer suitable lower-cost workers”. These
map to `NATIVE_DEFAULT`, `BALANCED`, and `COST_SAVER` in `DELEGATION.md`. The advanced
`HOST_DELEGATION_ONLY` choice appears only when the lead harness can enforce it. These are
preferences, not cost/speed/quality guarantees. Show “Cost unknown” when usage units are
not comparable; never imply an automatic model substitution. Exact model/profile rules
and `LATENCY_FIRST` remain in Advanced worker settings.

`Settings → Agents` has `Main`, `Subagents`, and `Installed` views. Installed shows
discovered profile/binding connectivity and staleness. Main selects only enabled,
lead-eligible bindings. Subagents lists every installed/bound agent, including the lead,
and one or more profiles under each binding. New profiles are disabled until explicitly
enabled; enabling a profile never starts its process. Enabling a profile makes it eligible
at Workspace scope only. Each Coworker separately allowlists which enabled profiles it may
use. Profile rows show `Available to: Alex, Researcher` or `Not assigned`; editing that
assignment creates a new CoworkerRevision. The enable flow asks whether to make a profile
available to the selected Coworker, so users can distinguish installed, enabled, and
Coworker-allowed states.

The Workspace default lead can be explicitly cleared from the Agent Catalog after a
review prompt. Clearing changes only the Workspace fallback: a selected Coworker's pinned
default lead still takes precedence, while future Tasks with no explicit or Coworker lead
are rejected as `AGENT_UNAVAILABLE` and preserve the composer draft. Existing Tasks and
Attempts keep their pinned leads. The clear prompt is scoped to the selected Workspace and
binding so a Workspace switch or concurrent default change cannot confirm a stale target.

The current Codex profile's expandable probe details show only bounded allowlisted
observations returned by the Operator: protocol initialization, account-read and
authentication observation, model-list/catalog status and count, whether session start
was tested, direct probe-process stop, and writer-quiescence status. A listed model is
never presented as inference entitlement. These details are diagnostics, not execution
readiness; the probe does not create a work session or prove safe switching.

The profile editor has sections for: status/name; short “When to use” description;
adapter-discovered model/reasoning/session options; worker instructions; capability
requirements; enforced security/limits; filesystem isolation; native delegation policy;
concurrency/depth; optimization/quality floor; budget; latency and warm preferences.
Renaming saves a new revision and shows the changed name in profile history. “Duplicate”
asks for a new name, previews copied settings, and creates a disabled profile; it never
copies execution history, grants, Environment state, or authentication. Portable export
and import are not available in v1.
Prompt guidance is labeled `Instructions`; actual enforced restrictions are labeled
`Security & limits`. Unsupported options are unavailable with an explanation, never
silently replaced. Advanced controls collapse by default. A profile's revision history
is inspectable, and changes affect future admissions only.

Coworker autonomy summaries use the narrow interaction defaults `Research`, `Create
drafts`, `External changes`, `Destructive actions`, and `Financial commitments`. They
explain `Standard Trust checks`, `Ask first`, or `Hand off`; they never present a numeric
autonomy level or imply that a preference is a Grant or approval.

### Work / Live Desk

Work filters are Active, Waiting, Scheduled, Done, and All. Each row shows outcome title,
plain-language state, `Task updated` from the saved Task summary timestamp, current blocker,
newest Artifact, and number of active workstreams only when real child Attempts exist. This
list timestamp is not observed runtime activity. Task detail defaults to
steps/outcomes and outputs. `Details` exposes the delegation tree and Inspector fields.
Lead, delegated worker, native-reported worker, capability, and verifier rows use distinct
labels so the product does not imply Core ownership of native subagents.

Task detail states include Planning, Preparing worker, Working, Checking the result,
Waiting for you/service/device, Needs your decision, Completed, Not finished, Failed,
Stopping, and Cancelled. Do not render numeric completion percentages or predicted ETAs
unless a measured and qualified projection contract is added. “No new activity for …”
is based on observed-activity timestamps. `Last verified evidence` is shown separately and
only when an Evidence record supports the label; an Invocation heartbeat is not described
as a verified action.

If the initial Task detail read fails or returns an identity that does not match the
selected Workspace and Task, show an unavailable state with **Retry Task details**. Retry
repeats only the same authenticated read; it does not revise, plan, or execute the Task.

### Lead changes and failover

When the lead is unavailable, `DISABLED` stops and explains the blocker. `ASK` creates a
Needs You item with the affected lead, observed trigger, eligible alternatives, and the
remaining Task state; no lead changes before the owner chooses. `ALLOW_LISTED` may select
only a listed binding after the trigger observation is fresh and all current binding,
endpoint, Runtime, auth, Trust, resource, budget, and deadline checks pass. The activity
timeline explains the committed change and its reason (for example, “Claude usage limit
reported”); it does not show the replacement as lead before `task.lead_agent.changed` and
new lead-session admission commit. Existing Attempts keep their original agent/profile and
lease provenance. If no listed lead qualifies, the Task remains blocked for the owner.

### Goals and Suggestions

Goals show the owner's objective, success criteria, horizon, linked active/completed Tasks,
linked Routine revisions, pinned Artifact versions, and evidence-backed contributions.
The owner can add or remove Task and Artifact links while editing a Goal. Removing a link
creates a new Goal revision; it does not alter or delete the Task or Artifact. Conflicting
or stale source records have explicit badges and do not count as verified progress. Only
the owner can complete or reopen a Goal.

Each linked Task in Goal details is an explicit **Open Task** action that navigates to that
Task's existing details in Work within the selected Workspace. The action is read-only: it
does not revise the Goal or Task, start planning or execution, or change link provenance.
Task creation, linking/unlinking, and Task execution remain separate owner actions.

When the local projection is partial, the Goal page labels the limitation and renders
unknown verified/stale/conflicted counts as unavailable, never zero. A Task marked
`COMPLETED` remains unverified until the current mandatory criteria and inputs are matched
to passing VerificationRuns. Artifact evidence is shown only when the exact pinned version's
Evidence IDs resolve to committed records in the selected Workspace.

At most one prominent Idea appears on Home and at most three in the Ideas drawer. Similar
open suggestions deduplicate. Each card offers Prepare/Accept, Remind me, Dismiss, and Why
this? as applicable. Remind me offers Later today, Tomorrow, and Next week, bounded by
`expires_at`; choosing one persists `snoozed_until`. The overflow menu offers “Don't
suggest this type,” which mutes the Workspace preference for its `SuggestionKind` and
clears currently proposed records of that kind as dismissed. The preference can be
reversed in Settings. Individual dismissal suppresses only the same `dedupe_key` for 30
days. `Why this?` lists source references and explains what will be created and what
authority may be requested. Acceptance always opens ordinary Task/Routine/Automation
flows. A suggestion is never a notification disguised as authorization.

The Ideas drawer has `For you` and `Snoozed` filters. A snoozed card can be restored with
“Show now”; it remains out of Home until its saved time or expiry. If it expires first,
it leaves the actionable list with its terminal history retained.

### Onboarding

First run is a short sequence with Skip/Back and no mandatory avatar/personalization:

1. Welcome and a one-sentence explanation that work can continue across enabled agents
   and devices.
2. Create/select Workspace; default is “This computer only”. Cloud/replication details
   are a separate explicit choice.
3. Discover/connect an agent, create its disabled binding, authenticate if needed, enable
   it, then choose a lead-eligible binding. Preserve drafts when setup is incomplete.
4. Create/select the primary Coworker with the neutral name “Assistant” and role
   “General-purpose assistant”; renaming, role changes, avatar, and personalization are
   optional and can be changed later. Do not invent a human-like personality or require
   naming before the user can start work.
5. Offer optional worker setup (“Use lower-cost workers for suitable work”) with per-
   profile descriptions, provider usage caveat, and no automatic enablement.
6. Suggest one concrete first Task from the available Workspace resources; do not auto-
   attach folders or grant app access.

After the first verified output, explain the visible delegation and verification result,
then offer “Make this reusable” through SkillProposal review. Avoid a setup tour of IDs,
protocols, tools, Runtime internals, or every setting.

## Cost, quota, offline, and error presentation

Money appears only when the provider reports or a named estimator produces a value with
currency, unit, confidence, and observation time. Native plan usage may be shown as
provider units with unknown monetary cost. Unknown is a first-class visual state, never
`$0.00`. Quota shows Normal/Low/Exhausted/Unavailable only at its observed freshness;
do not fabricate a remaining-use meter. An exhausted lead offers Continue with an
explicit eligible worker, Choose a lead, or Wait. Automatic handoff appears only when the
Task/Coworker policy explicitly allows it.

Offline surfaces show the last-known timestamp and disable actions that require authority
or unavailable Runtime state with a reason. Local work may continue under valid local
authority. Configuration drift offers Review changes/Revalidate; worker failures expose
recovery and evidence; ambiguous Effects expose Reconcile/Wait/Ask rather than Retry.
Unknown cost, quota, provider state, or page freshness remains textually Unknown.

## Responsive and accessible responsibility UI

On mobile, bottom navigation is Home, Work, Needs You, Library, More. More contains
Coworkers, Automations, Discover, and Settings. Mobile prioritizes steering, approving,
answering, pausing/cancelling, and opening Artifacts. The delegation tree becomes an
accessible nested list; the full Inspector is a separate diagnostics route. Browser
takeover is offered only when the device can own that EnvironmentControlLease; otherwise
the UI explains where takeover is available.

All worker/profile trees expose hierarchical list semantics, labels for verification and
authority, and keyboard-operable expansion. Live announcements are throttled to meaningful
state changes, especially worker start/settle, verification, and control ownership.
At 200% zoom content reflows; at 400% single-column surfaces remain usable. Touch targets
remain at least 44px. Reduced motion removes spatial expansion, pulse, and fades without
removing status, focus, or owner labels.
