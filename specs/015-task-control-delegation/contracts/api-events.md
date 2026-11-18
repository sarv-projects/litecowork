# API and Event Contract - Feature 015

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation. Links to global owners are normative.

## 1. Existing served routes

| Method and path | Handler | Behaviour today |
|---|---|---|
| `POST /v1/tasks/{task_id}/lead-agent` | `owner_handoff` in `apps/litecoworkd/src/task_lead_handoff_operator.rs` | Body `{expected_task_spec_revision, target_agent_binding_id}`, If-Match (Task version), Idempotency-Key, 8 KiB limit. Commits lead CAS, `task.lead_agent.changed.v1` and receipt. Response `{task_id, workspace_id, from_agent_binding_id, to_agent_binding_id, task_spec_revision, aggregate_version, replayed}`. No Attempt is created. |
| `GET /v1/tasks/{task_id}/verification-runs` | `list_task_verification_runs` in `apps/litecoworkd/src/verification_runs_operator.rs` | Cursor and limit 1..200 (default 50), `no-store`. Read-only. |
| `/v1/delegation-profiles` family | `apps/litecoworkd/src/delegation_profiles_operator.rs` | Profile list, revisions, performance, duplicate, status. |

## 2. Accepted target routes (PROPOSED; not yet in source)

All use the authenticate middleware, Workspace-owner check (foreign Task is 404), If-Match with the Task version, Idempotency-Key and `Cache-Control: no-store`.

| Method and path | Body | Success | Notes |
|---|---|---|---|
| `POST /v1/tasks/{task_id}/pause` | `{reason?}` (8 KiB) | 202 with control receipt `{control_request_id, task_id, state, aggregate_version, replayed}` | PAUSED is reached asynchronously after quiescence |
| `POST /v1/tasks/{task_id}/resume` | `{}` | 200 receipt | Only from PAUSED |
| `POST /v1/tasks/{task_id}/steering` | `{class, expected_task_spec_revision, body}` (16 KiB) | 200 receipt; for TASK_SPEC_CHANGE includes `task_spec_revision` | Classes CHAT_ONLY, TASK_SPEC_CHANGE, PLAN_HINT, URGENT_INTERRUPT, CANCEL |
| Feature 005 `cancel_task` | as defined by Feature 005 | 202 receipt | Extended to fan out to child Attempts |
| `GET /v1/tasks/{task_id}/activity` | query `cursor`, `limit` 1..200 | page of entries `{entry_id, occurred_at, actor, kind, attempt_id?, lease_epoch?, state_from?, state_to?, correlation_id}` | Projection of owner events only |
| `GET /v1/tasks/{task_id}/workers` | none | `{lead, children[], verifiers[]}` each with binding, AgentModule and version, profile revision, Environment class, state, budget used, verification | Native sub-agent reports appear only as `reported_native_activity[]` |

The Task projection gains `allowed_commands[]` (subset of PAUSE, RESUME, STEER, CANCEL, CHANGE_LEAD) and `unavailable_reasons{}` computed by the daemon from the `TaskStatus` transition table and the release-row capability. CHANGE_LEAD calls the existing lead-binding operation: it creates a fresh planning session without an Attempt, while existing Attempts retain their binding/lease. It must not imply execution replacement. The distinct execution-replacement operation is unresolved under SPEC O-07 and requires the generic admission seam in O-06; `HANDOFF_PRIOR_LEASE_ACTIVE` belongs to that future operation, not the binding-CAS route.

Child admission is daemon-internal: the delegate request comes from the parent AgentSession through the adapter, not from the desktop. Feature 005 revision 1.1 records the owner-directed generic Task Runtime Environment-first admission/control seam, but cross-owner review and Feature 015 acceptance/implementation remain pending. Feature 005's `admit_file_write` contract remains a bounded file-write slice and is not implicitly the generic service.

## 3. Events

Existing: `task.lead_agent.changed.v1` (cause `OWNER_REQUEST` or `POLICY_FAILOVER`).

Accepted target: `task.pause.requested.v1`, `task.paused.v1`, `task.resumed.v1`, `task.steering.recorded.v1` (class and revision, never body text), `task.child_attempt.admitted.v1` (parent and child Attempt ids, profile revision, depth, lease epoch), `task.child_result.fenced.v1` (Attempt id, observed and live epoch, reason). Payload schemas are promoted in `docs/schemas/domain-event.schema.json` by T02.

## 4. Tauri commands (accepted target)

`pause_task`, `resume_task`, `steer_task`, `change_task_lead`, `list_task_activity`, `list_task_workers`, `list_task_verification_runs` in `apps/litecowork-ui/src-tauri/src/task_control_bridge.rs` (new). Identifiers are validated before the request; no host path crosses the bridge.

## 5. Security contract

No route returns provider handles, native transcripts, SecretLease material or steering text in events. The delegate request is authorized by the parent Attempt's current session and lease, never by the desktop principal alone.
