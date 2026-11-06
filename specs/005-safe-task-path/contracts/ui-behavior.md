# UI behavior - Safe Task Path

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Target user journey

1. The owner explicitly creates a Task from a standalone objective or a separately reviewed Conversation promotion action. Ordinary Conversation Send never creates a Task.
2. Home shows the exact selected Workspace, lead/Coworker, pinned input Resource revisions and the durable Task-save action. A successful save confirms only Task + TaskSpecRevision creation.
3. Work shows Task objective/spec history and planning readiness. The owner reviews an immutable proposed plan and explicitly accepts a version-pinned PlanRevision before any Attempt can start.
4. The execution view distinguishes plan Steps, Attempt state, selected Environment and its exact isolation attestation, scoped grants/approvals, proposed/started/ambiguous/reconciled Effects, Evidence and verification runs.
5. A file change is not shown as successful until the exact admitted write is reconciled and independently verified. UNKNOWN/AMBIGUOUS remains visibly unresolved. Restart does not replay an ambiguous write automatically.
6. Cancel/pause/resume/recover controls exist only when their owning Operator handlers are served and their state-transition preconditions can be confirmed. UI does not encode its own Task state machine.

## Current UI implementation crosswalk (not the target)

- Home is labelled “New task” and its form is explicitly “Save a Task”. It saves a standalone objective with optional Coworker/default lead and selected Resource revision refs, preserves an unchanged request ID for retry after an uncertain response, then opens Work.
- No Conversation-page control or source call site promotes a committed chat outcome directly into a Task. Conversation Send stays separate.
- Work provides Task list/filter/pagination and Task detail. Detail supports objective/spec revision editing, displays pinned input Resources, any persisted Steps, Task status, read-only planning-readiness result, Task presentation/progress snapshots and committed output Artifacts.
- Planning readiness UI explicitly says it is read-only and does not start planning or an agent. It reports blockers and says planning dispatch is unavailable; it must not be styled as a Start/Run action.
- Task presentation reads saved projections and a stream. “Latest saved progress” is not a live process guarantee, invented percentage or ETA. A Task with status READY is a saved Task state, not an executing worker.
- Plan acceptance, Attempt start, Effect approval, Task cancel/recover, Environment selection/provisioning and isolation detail controls are not present in the current Work UI because their execution routes are target-only.

## Required rendered regression cases after implementation

- Ordinary Conversation Send does not create a Task; explicit Task save creates only Task + TaskSpecRevision.
- Lost Task-create response and same-payload retry replay the original receipt; changed payload does not create a duplicate.
- Lead unavailable or Coworker/Workspace binding stale: preserve draft and fail without silent lower-precedence substitution.
- Planning readiness blockers and stale Task version are visible and never presented as plan acceptance.
- Plan acceptance is version-pinned and precedes Attempt admission; stale TaskSpec/producer scope is rejected.
- Environment status distinguishes capability probe, provisioned state, exact enforcement evidence and unavailable/unknown dimensions.
- During a write, cancellation and restart require exact quiescence and Effect reconciliation. AMBIGUOUS is never presented as success or safe-to-retry.
- Output/result labels distinguish committed Artifact, Evidence, and independently PASSED verification.

## Validation

Source-level reducer tests and mounted read routes do not prove a working Task execution journey. Success requires receipt-backed USER acceptance on the declared OS/build, exact Environment provider, native agent/auth route, and file-write class.