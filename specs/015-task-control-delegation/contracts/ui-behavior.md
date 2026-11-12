# UI behavior - Work-screen Task controls, Activity and Workers (Feature 015)

**Status:** IN_REVIEW - authored for owner review; binds UI to owner contracts; not pixels.
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md) section 3, [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md).
**Wire:** [LOCAL-OPERATOR-IPC.md](../../../docs/LOCAL-OPERATOR-IPC.md) and [operator-api.openapi.yaml](../../../docs/schemas/operator-api.openapi.yaml).

The UI renders daemon projections and sends typed commands. It does not compute allowed transitions and does not parse English error strings.

## 1. User flow and transitions

| Step | Screen / surface | Transition |
|---|---|---|
| Open Task | Work > Task detail (`TaskDetailPanel`, execution area from Feature 005 `TaskExecutionPanel`) | Loads Task projection, first Activity page, Workers |
| Pause | Controls toolbar > Pause | Button shows "Pausing"; status PAUSE_REQUESTED then PAUSED from the stream |
| Resume | Controls toolbar > Resume | Enabled only when PAUSED; status returns to prior state |
| Steer | Controls toolbar > Steer opens dialog (class radio group, text area, Send) | TASK_SPEC_CHANGE shows the new spec revision number; CHAT_ONLY shows "Note sent to lead" |
| Cancel | Controls toolbar > Cancel opens confirm dialog naming active workers | "Cancelling" until CANCELLED is projected |
| Change lead | Controls toolbar > Change lead opens binding picker (eligible bindings only) | Confirms the assigned lead binding changed. Existing Attempts continue under their original binding/lease; the new lead is used for a fresh planning session. Do not show it as execution replacement. The separate execution-replacement operation is blocked under SPEC O-07. |
| Inspect worker | Workers view row > Details | Opens Attempt detail with profile revision, Environment class, verification |
| Browse history | Activity view > Load older | Next cursor page |
| Open Mission Control | Work > Task detail > Mission Control (secondary view) | Available only after the first bounded workflow has produced records; nodes open their real Step/Attempt/Effect/Artifact/Evidence detail |
| Compare Agents | No control is exposed until O-09 defines the owning screen and command | After O-09, an explicit action runs a read-only comparison; it never becomes a Task or delegated Attempt |

## 2. UI states

| Surface | States | User-visible meaning |
|---|---|---|
| Controls | enabled, disabled-with-reason, pending, denied, offline, stale | Disabled reason is the daemon's `unavailable_reasons` text |
| Activity | loading, empty ("No activity yet"), page, error, offline, stale (resync), recovery ("Recovering after restart") | Only owner events; no percent or ETA |
| Workers | loading, empty ("Only the lead is working"), list, UNAVAILABLE (Windows row), error | Native sub-agent reports in a separate "Reported by the agent" list |
| Windows row | UNAVAILABLE | "Task execution is not available on Windows yet: no qualified Windows Environment provider." |
| Mission Control (T20) | hidden until a bounded workflow has real owner records; then opens the Task's read-only canvas | No runtime records means UNAVAILABLE, never synthesized activity |
| Compare Agents (T20) | BLOCKED; no control until O-09 defines exact owner surface and request/response/recovery contract | After owner approval, read-only fan-out outside Task authority; no Step, Attempt, grant, or Task mutation |

## 3. Validation and failure presentation

| Validation | Rule | Typed failure / UI state |
|---|---|---|
| Steering body | 1..16 KiB | Inline error before send |
| Stale version | 409 `STALE_TASK_VERSION` | Refetch, announce "This Task changed; showing the latest state." |
| Allowlist denial (shown in Workers) | `DELEGATION_PROFILE_NOT_ENABLED_FOR_COWORKER` | "This Coworker does not allow that worker" with link to Coworker settings |
| Budget stop | `DELEGATION_BUDGET_EXHAUSTED` | Banner with budget scope |
| Fenced result | `task.child_result.fenced.v1` | Activity entry "Late result ignored (replaced worker)" |

## 4. Commands (UI to core)

| UI control | Local reducer action | Tauri command / operationId |
|---|---|---|
| Pause | `controlRequested(PAUSE)` | `pause_task` / pauseTask |
| Resume | `controlRequested(RESUME)` | `resume_task` / resumeTask |
| Steer > Send | `controlRequested(STEER)` | `steer_task` / steerTask |
| Cancel > Confirm | `controlRequested(CANCEL)` | Feature 005 `cancel_task` / cancelTask |
| Change lead > Confirm | `controlRequested(CHANGE_LEAD)` | `change_task_lead` / existing lead-agent route |
| Activity > Load older | `activityPageRequested(cursor)` | `list_task_activity` |
| Workers refresh | `workersRequested` | `list_task_workers`, `list_task_verification_runs` |

## 5. Events and stream (core to UI)

| Source | Type | UI use |
|---|---|---|
| Operator stream | Task projection frames via `task-stream.ts` | Update status, `allowed_commands`, append Activity entries; on gap, resync |
| Read APIs | Task, activity page, workers | Hydrate on open and after 409 |

## 6. Accessibility

Toolbar is a `role="toolbar"` with arrow-key navigation; each control has an accessible name and, when disabled, `aria-describedby` pointing at the reason. Dialogs trap focus and return it to the invoking control. Status changes are announced in one `aria-live="polite"` region. Activity is a list with each entry's time in a `<time>` element. Colour is never the only state signal. Target: no axe violations at serious or critical level.

Task-level Pause/Cancel controls govern Task lifecycle. A Computer “Take over” control, when delivered by Feature 008, transfers desktop input authority only; it does not pause or cancel a Task. A user message is not a takeover or a substitute for Stop. Keep those controls in their owning surfaces and link to the [Computer observation/control contract](../../../docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md). Mission Control is a secondary view within Task detail; do not add another permanent left-rail destination. Compare Agents remains hidden pending O-09.

## 7. Tests

AC-015, AC-016, AC-017, AC-018, AC-020 through `apps/litecowork-ui/tests/task-control-state.test.ts` and `apps/litecowork-ui/tests/task-activity.test.ts` (both new, TASKS T12 and T13), run by `pnpm --dir apps/litecowork-ui test`.

## 8. Cross-layer trace (filled by TASKS T16)

| Operation | Control | Command | Route | Service | Transaction | Event | Recovery | Evidence |
|---|---|---|---|---|---|---|---|---|
| Pause | Pause button | `pause_task` | `POST /v1/tasks/{task_id}/pause` | TaskControlService | tx-request-pause, tx-confirm-paused | `task.paused.v1` | startup sweep | to be recorded |
| Delegate Step | none (lead-initiated) | daemon-internal | adapter delegate request | DelegationAdmissionService | tx-child-admission | `task.child_attempt.admitted.v1` | abandon unstarted child | to be recorded |
