# Motion and Transition System

Motion communicates domain change; it never invents work.

## Tokens

These v1 timing/easing values are normative. Changing them requires a design-system update;
semantic triggers and reduced-motion behavior remain invariant.

```text
duration.instant = 80ms
duration.fast    = 120ms
duration.normal  = 180ms
duration.slow    = 280ms

ease.standard = cubic-bezier(0.2, 0, 0, 1)
ease.enter    = cubic-bezier(0, 0, 0.2, 1)
ease.exit     = cubic-bezier(0.4, 0, 1, 1)
```

No essential information depends on motion.

Interactive color/border/focus transitions use `duration.fast`; content/status transitions
use only the table below. Avoid spring physics, overshoot, parallax, continuous shimmer,
and looping progress animation. Indeterminate work is shown with a static label/icon and
an accessible busy state, not a decorative animation.

## Semantic transitions

Live text may show a nonanimated insertion caret only while new `turn.delta` frames are
actually arriving. It disappears immediately on a stream gap, stall, settled turn, or
reduced-motion preference. Heartbeats and elapsed time do not animate the caret or imply
new work. The Activity label remains static unless its source projection changes.

| Domain/UI change | Commit or observation required before motion | Motion |
|---|---|---|
| Conversation message appears | `conversation.message.added.v1` projection | Fade in 120ms, standard easing; no slide-in |
| Live response text extends | Current monotonic `turn.delta` received for an active turn | Append real text; optional nonanimated caret while frames arrive; never animate fabricated characters |
| Stream reconnects/resyncs | Snapshot/cursor accepted by Operator | Replace stale projection immediately; do not replay historical message, activity, or artifact entrances |
| Context-used disclosure opens | User action | Expand/collapse 120ms; reduced motion changes immediately; no retrieval animation |
| Artifact version history opens | User action against current Artifact projection | Expand/collapse 120ms; version rows remain ordered by committed version |
| Artifact compare/restore result | Exact adjacent-version responses validated / new ArtifactVersion committed | Clear prior comparison before loading; reveal both exact panes together after validation, or show fallback immediately; restore card appears only after new version commit |
| Artifact Save As | Owner selects exact managed version and native save completes | Native dialog owns selection; cancel changes no UI/domain state; show “Saved” only after verified bytes are written; no download animation or Artifact event |
| Renderer fallback appears | Renderer resolution returns unsupported/disabled | Static fallback with reason; no failed-renderer shake or success-like transition |
| Message expands to Task card | `task.created.v1` projection | Height + opacity, 180ms, enter easing |
| Default AgentBinding changes | `workspace.default_agent_binding.changed.v1` projection | Update selected label with 120ms fade; no automatic agent-switch animation |
| Planning status appears | `agent.session.started.v1` for TASK_PLANNING | Opacity 120ms; no lane, spinner, or percentage |
| Workstream lane appears | persisted Step/Attempt projection reaches RUNNING | Height + opacity, 180ms, enter easing |
| Capability source attaches | activation/invocation reaches STARTING/DISPATCHED | Opacity 120ms; source text uses actual operation |
| Delegation branch appears | child Attempt committed | Height + opacity, 180ms; native subagent is labeled reported |
| Artifact version appears | ManagedBlob digest verified or ExternalResource pin validated, then ArtifactVersion committed | Opacity + 0.98→1 scale, 120ms, standard easing |
| Blocker/UserRequest enters view | corresponding durable record appears | Opacity 180ms; one border emphasis for 280ms, then static |
| Verification state advances | VerificationRun projection changes | Icon/fill 120ms; no continuous intermediate animation |
| Task pauses/resumes | committed pause/resume/lease events | Text/status change 180ms; old process is never depicted as restarting |
| Human/Agent control changes | control lease owner/epoch projection changes | Badge/label 120ms; never animate queued pointer input |
| Resource freshness changes | location/revision projection changes | Label + linked freshness icon 120ms |
| Imported Resource appears | Resource + initial revision/location and idempotency receipt commit | Add the catalog row with a 120ms fade; never animate on file selection or while bytes are being sent |
| Text preview appears | Authenticated current-revision bytes pass digest/size checks and Tauri accepts a text-like UTF-8 response within 1 MiB | Reveal the inert plain-text panel with opacity 120ms; never render active content or animate a preview before verified response |
| Markdown preview appears | Authenticated exact-version bytes pass digest/size checks and the bounded parser accepts the supported subset | Reveal the inert preview with opacity 120ms; fallback/raw-source changes are immediate and no source content is animated as executable markup |
| Saved Task presentation refreshes | A new finite authenticated snapshot for the same Task identity is validated | Keep existing rows stable; update only fields that changed. Freshness label may change at 120ms. Initial load, reconnect, or snapshot replacement does not replay historical item animations |
| Workspace instruction revision appears | `workspace.instructions.revision.created.v1` commits | Add the revision-history row with opacity 120ms; keep the editor's unsaved text unchanged until the user reloads or switches Workspaces |
| Instruction conflict comparison appears | stale-version response followed by successful latest-revision read | Reveal the latest saved text with opacity 120ms beside the preserved draft; never imply a merge or overwrite occurred |
| Saved Task objective editor opens/closes | Owner action on an eligible `READY`/unplanned Task | Show/hide immediately; no draft is represented as committed state |
| Task `Work details` disclosure opens/closes | User action on the loaded Task detail | Native disclosure, immediate; no Task/plan/status transition is implied, and reduced-motion behavior is identical |
| TaskSpec history disclosure opens | User action on the loaded Task | Load exact revisions lazily; show rows only after identity/order validation; stale cached history gets a text notice with no row replay |
| TaskSpec revision appears | `task.spec.revised.v1` committed and the subsequent Task read confirms the current revision | Update objective/revision label immediately; on conflict keep the draft static and show Reload latest Task |
| Task lane fails | affected lane projects failure | Border/status update 180ms; no screen-wide flash |
| Handoff phase/location changes | each Handoff phase committed; target lease required for final location | Current phase label 180ms; “Saving progress…” only during checkpoint/transfer, target only after lease acquisition |
| Archived Workspace banner appears | `workspace.archived.v1` projection | Banner fade 180ms; persistent read-only state |
| Primary Coworker label changes | `workspace.primary_coworker.changed.v1` projection | Text/icon 120ms; no execution identity animation |
| Coworker is created or revised | committed `coworker.created.v1` / `coworker.revised.v1` | Card fade 180ms; revision history appears only after commit |
| Coworker pauses/resumes | `coworker.status.changed.v1` projection | Presence/status text 120ms; current Tasks do not disappear |
| Worker profile enabled/disabled | `delegation_profile.status.changed.v1` projection | Toggle label 120ms; never animate a host/session starting |
| Worker preparing | committed `delegation.admitted.v1` and child Attempt projection | Add branch at 180ms with “Preparing”; no activity spinner before session state |
| Worker becomes active | AgentSession ACTIVE observation/event | Change “Preparing” to “Working” at 120ms |
| Verification escalation | next Attempt committed after failed VerificationRun | Prior lane settles; new lane enters at 180ms; do not morph one worker into another |
| Goal evidence contribution changes | verified Task/Evidence projection update | Text/check state 120ms; no animated percentage unless a true quantitative definition exists |
| Goal status changes | `goal.status.changed.v1` | Status label 120ms; linked Task lanes do not transition |
| Suggestion proposed/resolved | `suggestion.proposed.v1` / `suggestion.resolved.v1` | Fade 180ms in; fade 120ms out after resolution commit |
| Suggestion snoozed/shown now | `suggestion.visibility.changed.v1` | Fade 120ms after the event commits; it may re-enter only when the persisted time arrives or owner clears snooze |
| Suggestion kind muted | `suggestion.preference.changed.v1` plus resolution events | Remove matching cards after the preference and dismissals commit; Settings label updates at 120ms |
| Warm preflight succeeds/fails | operational readiness observation | No prominent motion; concise static readiness label |
| Cost ceiling blocks new admission | committed Task/profile blocker projection | One 280ms border emphasis, then static blocker |
| Deadline preflight advances | actual check result | Update one check row at 120ms; never animate a made-up countdown/percent |
| Environment control changes | committed `EnvironmentControlLease` owner/epoch | Existing owner badge changes at 120ms after the epoch commits |

Historical hydration, cursor resync, and initial page load render the current state without
replaying transitions. An animation is never queued from an earlier event after the latest
projection has advanced.

### Workspace creation, policy change, and archive
Workspace creation appears only after `workspace.created.v1` commits. A replication-policy change updates its displayed label after `workspace.replication_policy.changed.v1`; it must not imply that previously replicated data was deleted. Archive enters a persistent read-only presentation only after `workspace.archived.v1` commits. Blocked archive requests show active Tasks/Automations without an archive transition.

### Initial planning
Show a quiet “Planning” status when a real TASK_PLANNING session becomes active. Do not create an animated work lane before PlanRevision promotion and Step/Attempt creation. When the first Attempt is created, add the lane using the normal dispatch transition.

### Automation revision
Editing an Automation may animate its revision-history entry after the immutable revision event. Existing pending/running occurrences keep their pinned revision; do not restart, retitle, or visually replay them because the current definition changed.

### Conversation -> Task
After `task.created.v1`, message area may expand/morph into Task card using normal duration + fade. If Task creation fails, do not animate materialization.

### Step/Attempt dispatch
Lane enters after `attempt.created.v1` and its RUNNING projection. Fade/height transition only; no decorative spinner before authoritative creation.

### Capability activity
Source card attaches to lane after activation/invocation starts. Text reflects actual operation (`Reading 14 issues`, `Updating B3:F22`).

### Delegation
Child branch expands only after host child Attempt is durable. Native subagent may show `reported child` style if merely reported.

### Artifact
New ArtifactVersion enters with fast fade and subtle 0.98 -> 1.0 scale. Never animate before content validation and version commit; a linked external Artifact
does not require a local blob.

### Library archive
After `artifact.library.archived.v1`, fade the item out of the default Library projection. Do not animate deletion of the external source or immutable versions.

### Verification
Indicator stages:

```text
○ -> ◔ -> ◑ -> ✓
```

Partial-fill states require actual verifier stage/progress observations; elapsed time
never invents fractional progress. A verifier exposing only RUNNING shows a static checking
icon/label until a result exists. Failed/inconclusive does not end with a checkmark, and a
passing individual check does not itself certify the whole Task.

### Needs user
Relevant lane pauses. A new approval/blocker card may use the one-time 280ms border
emphasis in the table, then stays static. Never pulse continuously.

### Task pause/resume
On `PAUSE_REQUESTED`, show “Pausing safely” and update the visible stage only from
committed checkpoint/reconciliation/lease events. On `PAUSED`, settle to a static paused
state. If an Effect blocks pause, keep that blocker visible and do not run a completion
transition. Resume motion begins after TaskService accepts resume. Planning can resume
without an Attempt. An execution lane appears only after TaskService re-admits a retained
same-incarnation Attempt under a fresh higher-epoch lease or commits a new Attempt/lease.
Do not animate an old process waking up.

### Human control takeover
Show the current controller label and the committed control epoch change. During takeover,
indicate that queued agent input is being discarded; enable human controls only after the
new HUMAN epoch is active. Returning to the Agent shows fresh observation/reconciliation
before a new Agent epoch. Never replay cursor/input animations from an earlier epoch.

### Resource freshness and notifications
A Resource location changing to STALE/UNKNOWN updates its label and linked downstream
outputs after the projection event. A UserRequest arrival may use a brief badge/fade; a
NotificationDelivery acknowledgement never animates a Task into completion.

### Failure
Only affected lane transitions to failure state. No global red flash unless workspace-level catastrophic failure affects all work.

### Runtime handoff
Only show:

```text
This computer
Saving progress…
Cloud
```

when handoff phases actually progress. Never depict process teleportation.

### Worker/profile transitions

Enabling a DelegationProfile means “eligible for future selection”, not “running”. The
profile toggle changes after its committed status event without creating a branch. A
branch enters only after TaskService commits child Attempt admission; its initial text is
“Preparing worker”. Change to “Working” only after AgentSession ACTIVE is observed. If
startup fails, transition that branch to its typed failed/blocked state; do not show an
intermediate success pulse. A native subagent appears only as a reported child when its
harness emits that information, and is visually marked as harness-owned.

A profile rename updates the visible name only after its revision commits. A duplicate
profile card appears only after `delegation_profile.created.v1` is projected and carries
the committed `DISABLED` state; the card entrance never implies that a worker started.

When verification fails and policy admits another profile, preserve the failed Attempt in
history and add a separate next-Attempt lane. Do not animate the old card changing name,
model, or vendor. Quota-low prewarm and idle eviction are operational optimizations and
do not animate in the task canvas. Cost policy changes appear as a blocker/admission state
only when the Task/profile projection records that impact.

### Coworker, Goal, and Suggestion transitions

Coworker avatar/name setup may fade in after `coworker.created.v1`; presence is a quiet
text/icon update from current underlying Task/Runtime state. A paused Coworker does not
visually pause/cancel already running Task cards. Goal progress changes only after
accepted Task outcome/Evidence projections update; show a changed source count or
criterion state rather than interpolating a synthetic percentage. `goal.status.changed.v1`
updates the owner-controlled badge. Suggestion entry/removal follows committed proposal
and resolution events; accepting it transitions the proposal card to its resulting Task
card only after both states commit.

### Deadline preflight and control transfer

Deadline preflight checks appear as a short textual list whose items change only when a
check result is observed. Do not animate a countdown as proof that a site/action remains
available. For computer handoff, disable the outgoing controller until the control epoch
commits, then enable the new owner; returning to the agent includes a static “Checking
what changed” state until fresh observation and reconciliation complete.

## Runtime, Routine, and trigger presentation

Runtime recovery, AgentHost startup, and dependency waiting update static labels from
observed lifecycle state using `duration.fast`. Installation does not animate worker
startup. Routine creation/revision appears only after its event commits; saving an Operator
draft does not create a running lane. A due occurrence waiting for a laptop shows its
actual dependency blocker. Reconnecting or waking refreshes current state without replaying
missed worker animations. Quick Entry opens with a 120ms fade, with immediate display under
reduced motion; opening it never sends or captures context automatically.

RoutineHealth updates only after a terminal Task outcome, fresh dependency observation, or
drift Evidence is committed/projected. Change the outcome summary and state label directly;
never animate a synthetic health score. A `DRIFTED` badge appears with its blocker and
evidence link. Environment sharing-scope labels change only after
`environment.sharing_scope.changed.v1`; the Environment remains visibly SUSPENDED and does
not animate into use.

## Interrupted animations

UI state is always derived from latest projection. If a new state arrives mid-animation:
- cancel obsolete exit/enter sequence.
- animate from current computed visual state to newest state.
- never queue stale semantic animations.

## Reduced motion

When enabled:
- apply state changes immediately without transforms, fades, pulses, or continuous progress.
- retain textual status and icons.
