# UI behavior - Safe Task Path

**Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 IN_REVIEW with owner-directed implementation authorization; not a claim of implementation or qualification.

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Target user journey

1. The owner explicitly creates a Task from a standalone objective or a separately reviewed Conversation promotion action. Ordinary Conversation Send never creates a Task.
2. Home shows the exact selected Workspace, lead/Coworker, pinned input Resource revisions and the durable Task-save action. A successful save confirms only Task + TaskSpecRevision creation.
3. Work shows Task objective/spec history and planning readiness. The owner reviews an immutable proposed plan and explicitly accepts a version-pinned PlanRevision before any Attempt can start.
4. The execution view distinguishes plan Steps, Attempt state, Environment role (read-only planning versus fresh Attempt-private), the four enforcement statuses and a sanitized evidence summary, scoped grants/approvals, proposed/started/ambiguous/reconciled Effects, Evidence and verification runs. Planning shows “Preparing read-only workspace” until the provider is ready. The private attestation body and provider handles stay Runtime-local.
5. Before approval, Work presents an inert, bounded preview loaded from the exact managed ResourceRevision pinned by the Effect target. It shows the WorkspaceRoot name, normalized root-relative path, pinned base digest, proposed content digest/size and plan action. Approval binds all of those values. If the base changes, the exact ResourceRevision is unavailable/revoked/external-only, or its digest/size cannot be verified, approval is unavailable and no write starts. The ResourceRevision and Effect each have their own durable receipt and replay identity. Work displays only owner-projected grant/approval scope; FileWriteScopeRequest is untrusted request data and cannot create or assert authority.

Before that preview is available, show “Finishing the isolated attempt…” while the trusted runner or any child remains in the process scope. The candidate frame may arrive while the runner is still alive, but the UI must not say the file was captured or show an approval preview until the Runtime observes recursive process-scope quiescence and reaps the launcher. Only then show “Collecting the proposed file…” while the bounded bytes enter the managed Resource lifecycle. If the provider, incarnation, frame, byte ceiling or quiescence proof is unavailable, show the specific unavailable/recovery reason and keep Approve disabled. Do not treat ACP text updates, a direct ACP Client filesystem write, or a worker-supplied ResourceRef as captured candidate bytes.

## Attempt admission control (TARGET, no current UI route)

On Work > Task detail > ready Step row, expose **Run step** only after an accepted plan exists and the owner projection says that Step is READY, the Task version is current, the Task has no active writer lease, an eligible lead is assigned, and the current release row has a supported Environment provider. Do not require a previously created or attested execution Environment: this action prepares and attests a fresh Attempt-private Environment before admission. Do not use a global Run button or show a progress percentage. Windows continues to show a specific unavailable reason while B-35 remains unqualified.

Activating **Run step** sends `admitTaskAttempt` with `plan_revision` and `step_id`; the UI supplies the required Task version and idempotency key through trusted request metadata. It cannot select an Environment, AgentBinding, grant, Approval or provider handle. During preparation, show “Preparing an isolated attempt…”; this means a trusted idle scope is being set up and checked, not that an agent is running. No native harness, agent tools or user code may start before the durable admission receipt is resolved. Stop remains unavailable until a registered owner cancellation operation exists. On success, open the Task activity/detail view for the returned Attempt and show its persisted status. On an attestation or eligibility denial, keep the Step READY and show the canonical reason. On stale Task/plan or replay conflict, refetch the Task and Activity before offering another Run step. If storage outcome is uncertain, keep the progress state and retry with the same idempotency key while the owner resolves its receipt; do not claim admission, launch another worker or clean up an uncertain scope. A confirmed rejection must wait for observed cleanup before offering another Run step.

This is a TARGET interaction only: no button, route, Environment provider or user journey is currently implemented or qualified. The current Work screen must continue to present planning readiness as read-only and Task execution as unavailable.
6. A file change is not shown as successful until the exact admitted write is reconciled and independently verified. UNKNOWN/AMBIGUOUS remains visibly unresolved. Restart does not replay an ambiguous write automatically.
7. Cancel/pause/resume/recover controls exist only when their owning Operator handlers are served and their state-transition preconditions can be confirmed. UI does not encode its own Task state machine.

## Current UI implementation crosswalk (not the target)

- Home is labelled “New task” and its form is explicitly “Save a Task”. It saves a standalone objective with optional Coworker/default lead and selected Resource revision refs, preserves an unchanged request ID for retry after an uncertain response, then opens Work.
- Current CODE/SERVED behavior (005-T12, 2026-11-23): eligible committed AGENT replies expose **Continue as Task**. The accessible dialog uses the same-turn USER prompt as its initial objective, shows the Workspace, offers enabled same-Workspace lead-eligible bindings, allows only exact pinned source inputs, and visibly excludes unresolved revisions. It uses the existing Tauri `create_task` command and `POST /v1/tasks`, validates the returned provenance, then opens that Task in Work. Cancel returns focus to the exact reply action. Ordinary Send remains separate.
- After an ambiguous save response, the open dialog locks its objective, lead and input selection and retries the same request key/body. The cache is currently process-local; restarting the desktop while the result is ambiguous is not recovered by this slice and remains an explicit acceptance gap. A known typed nonretryable pre-commit refusal may unlock the draft for correction. Success creates only a READY Task and never starts planning or execution.
- A-114 / Feature 001-T31 adds the per-message nullable `turn_status` projection. A-114's status is a visibility predicate only; this page must never infer it from the latest turn, activity frames or neighboring messages. T12's Task-create transaction repeats the source check.
- Work provides Task list/filter/pagination and Task detail. Detail supports objective/spec revision editing, displays pinned input Resources, any persisted Steps, Task status, read-only planning-readiness result, Task presentation/progress snapshots and committed output Artifacts.
- Work Task detail includes the originating Conversation ID and exact ordered source-message refs in its Task projection; **Open source Conversation** returns to that Conversation. This is provenance navigation, not a copied transcript or new Conversation authority.
- Planning readiness UI explicitly says it is read-only and does not start planning or an agent. It reports blockers and says planning dispatch is unavailable; it must not be styled as a Start/Run action.
- Task presentation reads saved projections and a stream. “Latest saved progress” is not a live process guarantee, invented percentage or ETA. A Task with status READY is a saved Task state, not an executing worker.
- Plan acceptance, Attempt start, Effect approval, Task cancel/recover, Environment selection/provisioning and isolation detail controls are not present in the current Work UI because their execution routes are target-only.

## Owner-accepted V1 additions (revision 1.0)

- Conversation page: a "Create Task from this reply" action on each committed AGENT message (not on streaming, failed, cancelled or uncommitted turns). It opens an editable Task draft showing Workspace, lead and pinned inputs and saves through `create_task` with the origin `conversation_id` and `source_message_refs`. Keyboard reachable, accessible name, focus returns to the message on cancel.
- The Task draft is rooted in the nearest preceding committed USER message with the same immutable `turn_id` as the selected AGENT reply. Its objective starts from that USER message's semantic text. Show the exact Conversation and Workspace lineage, a selectable lead from the current same-Workspace lead-eligible descriptor choices, and only exact pinned `ResourceRef`s from the source USER message. A source ref without a revision is shown as unresolved and excluded until the user explicitly resolves it; never replace it with the current Resource head implicitly. Save creates only READY Task + initial TaskSpecRevision + `task.created.v1` + idempotency receipt. Success opens that exact Task in Work. It creates no Attempt, Environment, AgentSession or provider dispatch.
- Work execution panel (`TaskExecutionPanel`): plan Accept, Attempt, Environment with the four enforcement statuses, grant and Approval link, Effect, Evidence, verification, Cancel and Recover. States: loading, empty, error, denied, offline, stale, UNAVAILABLE, recovery.
- Windows row until B-35 qualifies: execution controls are replaced by "Task execution is not available on Windows yet: no qualified Windows Environment provider." No Job Object isolation badge.

## Owner-directed V1 additions (revision 1.1, cross-owner review pending)

- Planning readiness and planning dispatch identify a Task-scoped read-only Environment. A planning session is never shown as an Attempt and its Environment is never reused as a writable execution Environment.
- Attempt admission first shows preparation of a fresh Attempt-private Environment. A trusted idle supervisor may be running for bounded probes, but the native provider/harness starts only after the admission receipt is committed and resolved. Failure before commit leaves no Attempt; failed or ambiguous preparation is shown as unavailable/recovering, not as a running worker.
- Work presents a Resource-backed exact file diff with loading, stale-base, revoked/missing-content, digest-failure, too-large, denied, approval-pending and ambiguous states. The user action is **Approve and apply** or **Reject** against the exact preview digest.
- After **Approve and apply**, the UI shows the committed Approval decision separately from dispatch admission. It shows “Applying” only after the dispatch receipt; success requires Effect `VERIFIED` plus the required passing VerificationRun. A crash after Approval but before dispatch resumes by replaying the same command identity; a crash or timeout after dispatch reconciles and never reissues the write automatically. Render canonical Effect states only. A target digest matching approved content is `OBSERVED` until independently verified; a base-digest match is a `NOT_APPLIED` outcome in durable reconciliation evidence, not a state, and retry requires explicit authorization; other results stay `AMBIGUOUS`.
- No new permanent navigation destination is added. Task and write review remain in the existing Work Task detail surface.

## Required rendered regression cases after implementation

- Ordinary Conversation Send does not create a Task; explicit Task save creates only Task + TaskSpecRevision.
- Lost Task-create response and same-payload retry replay the original receipt; changed payload does not create a duplicate.
- Lead unavailable or Coworker/Workspace binding stale: preserve draft and fail without silent lower-precedence substitution.
- Planning readiness blockers and stale Task version are visible and never presented as plan acceptance.
- Plan acceptance is version-pinned and precedes Attempt admission; stale TaskSpec/producer scope is rejected.
- Environment status distinguishes capability probe, provisioned state, exact enforcement evidence and unavailable/unknown dimensions.
- During a write, cancellation and restart require exact quiescence and Effect reconciliation. AMBIGUOUS is never presented as success or safe-to-retry.
- Approval is bound to the exact preview digest; a changed base or content invalidates it, and unavailable/stale preview states expose no approval action.
- Output/result labels distinguish committed Artifact, Evidence, and independently PASSED verification.

## Validation

Source-level reducer tests and mounted read routes do not prove a working Task execution journey. Success requires receipt-backed USER acceptance on the declared OS/build, exact Environment provider, native agent/auth route, and file-write class.
