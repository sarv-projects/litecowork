# Motion and Transition System

Motion communicates domain change; it never invents work.

## Tokens

Recommended baseline tokens (implementation may tune visually without changing semantics):

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

## Semantic transitions

### Workspace creation, policy change, and archive
Workspace creation appears only after `workspace.created.v1` commits. A replication-policy change updates its displayed label after `workspace.replication_policy.changed.v1`; it must not imply that previously replicated data was deleted. Archive enters a persistent read-only presentation only after `workspace.archived.v1` commits. Blocked archive requests show active Tasks/Automations without an archive transition.

### Initial planning
Show a quiet “Planning” status when a real LEAD_PLANNING session becomes active. Do not create an animated work lane before PlanRevision promotion and Step/Attempt creation. When the first Attempt is created, add the lane using the normal dispatch transition.

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
New ArtifactVersion enters with fast fade and subtle 0.98 -> 1.0 scale. Never animate before blob/version commit.

### Library archive
After `artifact.library.archived.v1`, fade the item out of the default Library projection. Do not animate deletion of the external source or immutable versions.

### Verification
Indicator stages:

```text
○ -> ◔ -> ◑ -> ✓
```

Progress states correspond to actual verification run status. Failed/inconclusive does not end with checkmark.

### Needs user
Relevant lane pauses; approval/blocker card may use low-frequency subtle emphasis. Avoid continuous distracting pulse.

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

## Interrupted animations

UI state is always derived from latest projection. If a new state arrives mid-animation:
- cancel obsolete exit/enter sequence.
- animate from current computed visual state to newest state.
- never queue stale semantic animations.

## Reduced motion

When enabled:
- replace transforms with opacity/state changes.
- remove pulses/continuous progress motion.
- retain textual status and icons.
