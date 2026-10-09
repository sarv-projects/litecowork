# Operator API

This is a logical API contract. Transport binding may be local IPC for desktop and HTTP/WebSocket remotely.

The remote HTTP command representation is described by
`schemas/operator-api.openapi.yaml`; the WebSocket stream contract is defined below.
Local IPC maps to the same commands and schemas; it is not allowed to call storage or
domain internals directly. Mesh, Agent, LiteSPM, and Environment-provider methods are not
Operator API methods.

## Conventions

- IDs are opaque strings.
- every mutating HTTP call requires `Idempotency-Key`; local IPC uses the equivalent
  `request_id` field. The adapter maps both to one normalized RequestId used by deduplication
  and manual Automation occurrence identity.
- mutable resources expose `version`; HTTP commands use `If-Match`, local IPC uses the
  equivalent `expected_version`.
- conflict returns `409/CONFLICT` semantics.
- authorization failures never reveal secret/resource existence beyond policy.
- every Workspace-scoped HTTP call carries `X-Workspace-ID`; local IPC carries the same
  context field. Workspace list/create, authenticated local agent-installation inventory,
  and locally authenticated recovery bootstrap operations omit it. When a route or body also names a
  Workspace, it must match the selected context. The service resolves an opaque resource
  ID to its Workspace and checks owner authority; an ID is never authority by itself.
  Missing or mismatched scope is rejected as `FORBIDDEN` without revealing whether the
  named Workspace/resource exists.
- `GET`, `PATCH`, and `POST .../archive` on `/v1/workspaces/{id}` also carry
  `X-Workspace-ID`, and it must equal `{id}`. Workspace list/create,
  agent-installation inventory, local Operator readiness, and locally authenticated
  recovery bootstrap are the unscoped HTTP operations.
- command results include the committed aggregate version and correlation ID.
- The Workspace-create response returns the Workspace body and its correlation ID in
  `X-Correlation-ID`, including an idempotent replay of the original committed result.
- pagination uses opaque cursors bound to the query/filter; cursors are not offsets.

Authentication is deployment-specific, but authorization is not: a trusted local or remote
identity resolves to a Principal, then TrustService authorizes each command. The desktop
source now uses OS-peer-authenticated Unix IPC on Linux/macOS and retains inline
Workspace-owner checks; Windows is fail-closed pending named-pipe identity/ACL support.
The IPC transport has not yet passed build, system, or OS qualification and is not a
production release claim. V1 Workspaces have one owner principal and no membership model;
every Workspace-scoped call verifies that the authenticated Principal is that owner.
The desktop local transport is specified in
[`LOCAL-OPERATOR-IPC.md`](LOCAL-OPERATOR-IPC.md). It maps the same logical operations to a
bounded IPC frame and supplies an authenticated local peer only after OS identity checks;
it does not confer Workspace authority or replace command-level owner/policy checks.
Ordinary Operator command bodies and UserRequest responses are not credential-entry
channels; free-text schemas cannot prove arbitrary text contains no secret, so this is a
contract backed by bounded sensitive-field checks and clear UI warnings. Provider
authentication uses an owning Connection/SecretStore flow or an explicitly supported
out-of-band handoff; `SecretRef` values are opaque references, not credentials. Runtime pairing returns a short-lived bearer token once in the
`PairingToken` response; it is not exposed by list/read APIs or written to logs/events.

## Workspaces

```text
GET  /v1/workspaces?cursor=&limit=
POST /v1/workspaces
GET  /v1/workspaces/{id}
PATCH /v1/workspaces/{id}
PATCH /v1/workspaces/{id}/default-agent-binding
POST /v1/workspaces/{id}/archive
GET  /v1/workspaces/{id}/backups?cursor=&limit=
GET  /v1/workspaces/{id}/backups/{backup_id}
POST /v1/workspaces/{id}/backups
GET  /v1/workspaces/{id}/instructions/revisions
POST /v1/workspaces/{id}/instructions/revisions
```

The domain/API contract defines future Workspace replication policies, but the desktop/local
V1 client operates with `LOCAL_ONLY`. It does not initiate cloud continuation or remote
Runtime transfer. If a Workspace has a previously saved non-local policy, the local UI
reports it as inactive and allows an explicit reset to `LOCAL_ONLY`; it never implies that
the saved policy is currently transferring data.

Instruction revision creation requires the selected Workspace header to match the path,
the current Workspace `If-Match` version, an idempotency key, and a pinned ResourceRef for
a same-Workspace UTF-8 text Resource no larger than 64 KiB. The server verifies the
Resource revision digest before atomically committing the immutable instruction revision,
Workspace version/current-revision projection, domain event, and idempotency receipt.
Instruction history returns immutable revisions in ascending revision order through the
standard opaque Workspace-bound `cursor`/`limit` page contract (`limit` defaults to 50 and
is capped at 200).

Backup listing exposes only integrity-verified immutable manifests. The create command
returns a manifest only after the consistent database snapshot, event cursors, blob set,
encryption-key reference, and final integrity check are committed. Restore is a separate
locally authenticated bootstrap operation on an empty installation; it does not require a
live Workspace header and cannot overwrite a populated installation.

```text
POST /v1/recovery/backups/{backup_id}/restore
```

## Needs You inbox

```text
GET /v1/needs-you?status=OPEN&cursor=&limit=
```

This is a read-only, rebuildable Workspace projection over pending Approvals,
UserRequests, and actionable Task blockers. If a blocker links to an Approval or
UserRequest, the linked record supplies the single inbox item; otherwise the item uses
its stable Task/blocker identity. Answer, approve, dismiss, and resolve operations remain
on the owning resource routes. Notifications and delivery retries are not inbox items.

This remains the target `GET /v1/needs-you` contract; the desktop/local Operator does not
currently implement this route. The current Needs You page uses the existing authenticated
Task-list route for a clearly labeled, narrower view of `WAITING_USER`, `NEEDS_USER`, and
`BLOCKED` Tasks. It does not claim Approval/UserRequest coverage or provide inbox actions.

## Workspace-persistent Environments

```text
GET  /v1/workspaces/{workspace_id}/environments?cursor=&limit=
POST /v1/workspaces/{workspace_id}/environments/provision-preview
POST /v1/workspaces/{workspace_id}/environments
GET  /v1/workspaces/{workspace_id}/environments/{environment_id}
POST /v1/workspaces/{workspace_id}/environments/{environment_id}/suspend
POST /v1/workspaces/{workspace_id}/environments/{environment_id}/resume
POST /v1/workspaces/{workspace_id}/environments/{environment_id}/sharing-scope
POST /v1/workspaces/{workspace_id}/environments/{environment_id}/destroy
```

Provision preview is read-only and expires within five minutes, or earlier if the provider
offer/quote expires; it reports eligible Runtime choices, pinned
resources/dependencies, cost-estimate confidence, and whether the provider enforces the
budget. The response returns `preview_digest`, binding the normalized request, authenticated
preview principal, selected Runtime incarnation, input Resource revisions, provider offer,
and policy/quote basis. Create must send that digest with the same normalized request.
EnvironmentManager rechecks it atomically with admission; mismatch returns `CONFLICT`, and
expiry or a changed Runtime/provider/resource/policy basis returns `STALE_VERSION`. The
same Idempotency-Key returns its recorded create result even if the original preview later
expires. Preview is not a reservation and never guarantees future capacity. A requested
hard ceiling requires an eligible provider-enforced limit; monitor-only behavior is labeled
as such and requires explicit user choice.

Create provisions only an explicit `WORKSPACE_PERSISTENT` Environment. The request pins
source Resource revisions, preferred Runtime, resource/network limits, retention and
backup policies, and an observable cost/time budget; it cannot include credentials or
reuse Task grants. Provisioning returns `202` and a current Environment view. Suspend and
destroy are blocked while Attempts, capability Invocations, unresolved Effects, or
checkpoint holds use the Environment. Resume revalidates provider identity, health,
resources and Workspace policy; each later Task receives new authority. The Operator
view does not expose provider-private locator/credential data. Destroy requires an
explicit confirmation body and retains provenance/history after substrate deletion.
The Environment view separates the selected enforcement policy from the provider's
current enforcement capability and includes cumulative usage with confidence and
observation time. Missing or stale provider measurements are shown as `UNKNOWN`, never
as zero. Cost is compared only in the pinned budget currency; no implicit exchange-rate
conversion is performed. In v1 the budget ceiling and enforcement policy are immutable
after provisioning; there is no in-place top-up/reset route. At the limit, the Environment
is safely suspended and dependent Steps expose `BUDGET_EXCEEDED`. The user may select
another eligible Environment or provision a replacement using the ordinary preview and
confirmation routes. Replacement creates a new Environment and Attempt; it never silently
clones private provider state or changes an existing Attempt's binding.

`EnvironmentSpec` includes `sharing_scope`, independent of lifetime. New Environments
default to `ATTEMPT_PRIVATE`; persistent Environment preview/create requests explicitly
name the desired scope. V1 provision requests support `COWORKER_PRIVATE` and
`WORKSPACE_SHARED`; `USER_SHARED` remains reserved and is rejected until its user-level
ownership and cross-Workspace attachment contract is defined. The view reports sharing
scope without provider identifiers.

Changing a persistent Environment between `COWORKER_PRIVATE` and `WORKSPACE_SHARED`
requires explicit owner confirmation, `If-Match`, and an idempotency key. The Environment
must be suspended and have no active Attempts, Invocations, control leases, or unresolved
Effects. A Coworker-private target must name an active or paused Coworker in this
Workspace; a Workspace-shared target must clear the Coworker owner. The command changes
reuse eligibility only: it does not copy provider data, change grants, resume the
Environment, or attach it to a Task. `USER_SHARED` cannot be selected.

Restore verifies the key and every required object before making the Workspace writable,
restores the exact snapshot/event history through its recorded cursors, rebuilds projections,
and creates a new Runtime identity. Missing secrets require reauthentication; secret bytes
and old execution leases are never restored.

The selected Workspace header is mandatory for Workspace item and settings operations
and must match the path ID. This lets the same authenticated API surface reject mismatched
scope before invoking a Workspace command.

Agent selection is explicit and Workspace-scoped. The user enables a discovered
AgentBinding, then sets an enabled, lead-eligible binding as the Workspace default. A
Conversation may select an enabled, lead-eligible binding override; when absent, new
Conversation turns use the Workspace default.
`PATCH /v1/workspaces/{workspace_id}/default-agent-binding` accepts an explicit
`agent_binding_id` or JSON `null` to clear it. The command requires the selected
Workspace header, `If-Match`, and `Idempotency-Key`; its response is the committed
versioned Workspace. The storage transaction enforces same-Workspace, enabled,
lead-eligible binding membership. Settings reflects only the committed response.
An invalid/disabled explicit override does not silently fall back. Changing or clearing
the default affects newly admitted Conversation turns and Tasks that have no higher-
precedence Task or Coworker selection; existing sessions and Attempts remain pinned.
Without an eligible binding, chat/Task admission returns
`AGENT_UNAVAILABLE` and creates no partial turn or Task. The composer retains the unsent
draft while first-use setup selects and enables an AgentBinding. Other resource/runtime
blockers after Task creation are represented by `Task.blocking_conditions[]`.

Task creation may include a selected `coworker_id`; the Operator normally prefills the
Workspace primary Coworker in the composer and sends that exact ID. The service pins the
Coworker's current revision atomically with Task creation. Lead precedence for a Task is
explicit Task lead, selected Coworker revision default, then Workspace default. The first
configured binding is validated and never skipped in favor of a lower-precedence binding.
The desktop composer also sends the observed Coworker aggregate version when available;
the current revision is pinned by the service and rechecked in the Task transaction. A
direct Operator caller may omit that expected version, but a supplied version must be
positive and current. Omitting `coworker_id` deliberately creates a Task without Coworker
origin; the client does not submit an origin revision. Conversation turns remain governed
by Conversation override and Workspace default, independently of Coworker Task defaults.

Create defaults to `LOCAL_ONLY`; `SELECTED_FOLDERS` is unavailable in the create request because roots are Workspace-owned records created afterward. A client may send another supported policy only after the user explicitly selects it; enabling cloud defaults the UI to `ACTIVE_TASK_INPUTS`. The user adds persistent roots, then updates policy with one or more active same-Workspace `replication_scope_root_ids`. Root IDs follow each selected folder's future observed revisions; they do not pin one content snapshot. Root-level replication settings intersect with Workspace policy and cannot broaden it. A replication-policy update applies to future transfers and never silently deletes content already replicated to another Runtime. Archive is accepted only after every Task is terminal and every Automation is disabled. Quiescence also requires Conversation turns and scoped Invocations to be settled, no active grants/SecretLeases/control leases, and persistent Environments with no live workload. Authorized watchers/triggers stop before the read-only transition. Retained Environment state may remain suspended under storage/backup policy; archive never silently destroys it. Unknown provider quiescence blocks archive with `WORKSPACE_NOT_QUIESCENT`. Archived Workspaces remain readable and preserve existing authorized Artifact/Resource downloads, but reject all domain mutations, including Task mutations, capability activation/grants, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel work.

## Conversations

```text
POST /v1/conversations
GET  /v1/conversations?cursor=&limit=
GET  /v1/conversations/{id}
POST /v1/conversations/{id}/messages
POST /v1/conversations/{id}/turns
POST /v1/conversations/{id}/turns/{turn_id}/retry
POST /v1/conversations/{id}/turns/{turn_id}/cancel
GET  /v1/conversations/{id}/messages?cursor=
GET  /v1/conversations/{id}/presentation?cursor=&limit=
GET  /v1/rich-presentations/{presentation_id}
GET  /v1/conversations/{id}/tasks?cursor=
```

Conversation creation requires the selected Workspace, a bounded optional title, and an
`Idempotency-Key`. It commits the Conversation row, `conversation.created.v1` event,
aggregate-state reference, and request receipt atomically. The authenticated local catalog
is newest-first and uses a Workspace-bound opaque cursor. Conversation reads are owner
scoped and remain separate from provider/session state. The desktop Conversation page can
create, list, and read persisted semantic messages; it does not expose message submission
as available until E03-S01 native session admission and E03-S02 turn execution are
connected.

`POST /v1/conversations/{id}/turns` accepts optional
`presentation_preference: AUTO | SIMPLE | RICH`, default `AUTO`. The preference is pinned
to the ConversationTurn, returned in its receipt, and reused by retry. New turn creation
uses `conversation.turn.created.v2`; earlier v1 turns migrate/read as `AUTO`. Preference
controls optional response layout only and cannot suppress trusted UserRequest, Task, or
Artifact controls.

The Conversation presentation route returns a bounded authenticated snapshot of committed
messages, linked trusted items, active-turn state, optional immutable RichPresentation refs,
and the snapshot cursor. Rich documents are fetched separately and returned `no-store`
only after same-Workspace authorization and digest verification. The client validates
message/document binding, schema version, size, and exact host-bound source refs before
rendering. Semantic ConversationMessage content is always rendered first and remains
complete if rich data is missing, invalid, or unsupported. See
[`RICH-RESPONSE.md`](RICH-RESPONSE.md) and
[`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md).

## Tasks

```text
POST /v1/tasks
GET  /v1/tasks/{id}
GET  /v1/tasks/{id}/planning-readiness
GET  /v1/tasks/{id}/presentation
GET  /v1/tasks?status=&conversation_id=&cursor=&limit=
POST /v1/tasks/{id}/steer
POST /v1/tasks/{id}/lead-agent
POST /v1/tasks/{id}/pause
POST /v1/tasks/{id}/resume
POST /v1/tasks/{id}/cancel
POST /v1/tasks/{id}/steps/{step_id}/recover
GET  /v1/tasks/{id}/steps/{step_id}/execution-dependencies
POST /v1/tasks/{id}/attempts/{attempt_id}/take-control
POST /v1/tasks/{id}/attempts/{attempt_id}/return-control
POST /v1/tasks/{id}/spec-revisions
GET  /v1/tasks/{id}/spec-revisions
POST /v1/tasks/{id}/plan-revisions
GET  /v1/tasks/{id}/plan-revisions
GET  /v1/tasks/{id}/steps
GET  /v1/tasks/{id}/attempts
GET  /v1/tasks/{id}/timeline?cursor=
GET  /v1/tasks/{id}/artifacts
GET  /v1/tasks/{id}/effects
```

`POST /v1/tasks` and `GET /v1/tasks/{id}` return a `TaskView` containing the Task
projection and its current immutable `TaskSpecRevision`. The list route returns a
`TaskPage`; each item is a `TaskSummary` with `task_id`, `status`, `objective`,
`created_at`, and `updated_at`, matching the storage page projection. The optional
`status` and `conversation_id` filters are applied before cursor pagination, and a cursor
is bound to the Workspace and filter set used to create it. `status` accepts only
`READY`, `RUNNING`, `WAITING_USER`, `BLOCKED`, `VERIFYING`, `NEEDS_USER`, `INCOMPLETE`,
`PAUSE_REQUESTED`, `PAUSED`, `COMPLETED`, `FAILED`, `CANCEL_REQUESTED`, or `CANCELLED`.

`GET /v1/tasks/{id}/planning-readiness` is an authenticated, Workspace-scoped,
read-only local preflight. It requires `If-Match` with the current positive Task
aggregate version; a stale version returns `409 STALE_TASK_VERSION`. It returns the Task
and specification revisions, observed status/time, a bounded list of typed blocker codes,
and literal `false` values for `dispatch_available`, `planning_started`,
`agent_session_started`, and `plan_created`. The response is `Cache-Control: no-store`.
It never starts planning, creates an AgentSession/Plan/Step/Attempt/ExecutionLease or
Environment, invokes a provider, or mutates Task state. It deliberately omits the planning
packet, objective/model context, provider handles, and endpoint identifiers. A clear
preflight does not imply dispatch is enabled: this desktop slice always reports
`dispatch_available: false` until the full planning admission and lifecycle contract is
implemented.

`GET /v1/tasks/{id}/presentation` returns a bounded read-only
`TaskPresentationSnapshot` for the selected Workspace. It projects only the committed
Task head, current Plan Steps, and Artifact versions that the owning Artifact store can
resolve. Storage reads the Task/current spec, Steps for the pinned current PlanRevision,
Task Artifacts, and each resolvable current ArtifactVersion in one SQLite read
transaction. The read is capped at 100 Steps and 200 Task Artifacts; if either source
exceeds its cap, the endpoint returns `413 INVALID_ARGUMENT` without a partial snapshot.
Artifact rows without a resolvable current ArtifactVersion are omitted. Each included
item carries an exact source reference; renderers validate the item shape and
source-opening routes reauthorize independently. `CURRENT` freshness means only that the
persisted source records were read from one consistent SQLite snapshot. It does not
assert worker liveness, progress, verification, or freshness of external/provider state.
The endpoint does not report transient agent output, a live stream, or an external action
result. Responses use `Cache-Control: no-store`.

`POST /v1/tasks/{id}/spec-revisions` appends an owner-authored immutable revision. It
requires `Idempotency-Key`, `If-Match` with the current Task aggregate version, and a
`ReviseTaskSpecRequest` whose `parent_revisions` names the exact current spec head. The
current local Operator permits this edit only for a `READY` Task with no accepted plan and
no live Task-planning session. Omitted fields inherit the current revision; the desktop
currently exposes objective editing and preserves every other field. The Task version and
spec pointer, immutable revision, `task.spec.revised.v1` event/state snapshot, and
idempotency receipt commit atomically. A repeated identical request returns the original
revision; a changed request with the same key or a stale parent/version returns `409`.
Invalid fields return `422 INVALID_ARGUMENT`; a Task outside this edit lifecycle returns
`409 CONFLICT`. This command creates no AgentSession, Plan, Step, Attempt, lease, Environment, or
Invocation. Once planning or an accepted Plan exists, intent changes must use the Task
steering/replanning lifecycle. `GET /v1/tasks/{id}/spec-revisions` returns the immutable
history in revision order.

The lead-agent command body is `{ agent_binding_id}`. The response is accepted asynchronously; current Attempts remain pinned while work drains, and a replacement execution Attempt waits for old lease settlement and Effect reconciliation.

`POST /v1/tasks/{id}/plan-revisions` accepts a `SubmitPlanRequest` and returns a
`PlanAcceptance` with HTTP `201`. It requires the authenticated Workspace context,
`Idempotency-Key`, and `If-Match` containing the expected Task aggregate version. The
body's `task_spec_revision` must equal the current TaskSpec head. The authenticated
request context must be bound to the active current-lead `TASK_PLANNING` AgentSession or
to a current lead execution Attempt's active AgentSession and valid lease. Producer IDs
are derived from that trusted context and are not accepted as caller-supplied authority;
an owner credential cannot impersonate a planner by naming its AgentSession or Attempt.

The request includes `task_spec_revision`, a non-empty `steps` array of `PlannedStep`
values, and optional `reason_for_revision`. Each proposed step has a unique `logical_key`;
TaskService allocates durable Step IDs and maps dependencies from logical keys in the
same transaction. Stale Task versions return `409 CONFLICT`/`STALE_TASK_VERSION`; a stale
spec returns `409 STALE_SPEC_REVISION`. Invalid DAGs or producer authority create no
PlanRevision, Steps, or events.

Idempotency is scoped to authenticated producer, Workspace, route, and key. An identical
retry (same normalized body and `If-Match`) returns the original committed PlanAcceptance,
including its PlanRevision and Step IDs, without emitting events or creating records
again; this replay is checked after authentication and Workspace authorization, before
current-version/spec checks. The receipt must match the authenticated idempotency subject,
but producer eligibility is not re-evaluated for a committed replay, so a planner can
recover a response after its session closes. Reusing the key with a different route,
precondition, or request digest returns `409 CONFLICT`. A request with a new key must
pass current preconditions and producer revalidation. Successful acceptance atomically emits `task.plan.revised.v1`, one
`step.created.v1` per materialized Step, and applicable Step status-change events for
superseded/cancellation-requested Steps.

Implementation status: the POST contract is specified but the current Operator router
does not expose it yet. `operatorAuth` currently authenticates the Workspace owner but
does not establish a producer-scoped AgentSession/Attempt assertion. Until the trusted
agent-session gateway supplies that context, the route must not be wired to accept a
body-supplied producer ID. The current Rust work is an internal initial-plan
TaskService/SQLite seam only.

TaskSpec creation accepts an optional non-null `lead_failover_policy`; omission resolves
the selected CoworkerRevision default or an explicit `DISABLED` policy. A TaskSpec
revision that omits it inherits the prior pinned policy. Automatic continuation is
restricted to provider-confirmed triggers and ordered, currently eligible lead bindings.
A `task.lead_agent.changed.v1` event records `cause=OWNER_REQUEST` or
`cause=POLICY_FAILOVER`, the acting Principal/Service, the pinned TaskSpec revision, and
the trigger observation when applicable. Lead failover never transfers a Grant, Approval,
SecretLease, native session handle, or active Attempt.

`POST /v1/conversations/{id}/turns` first resolves and checks the Conversation override or
Workspace default AgentBinding/endpoint. If none is eligible, it returns
`AGENT_UNAVAILABLE` and persists no message or turn. Otherwise it atomically appends the
user message and creates the ConversationTurn, then starts a fresh Conversation-scoped
AgentSession. It returns `202` with the turn ID and command receipt;
the agent response is appended with AgentSession/AgentBinding provenance. This path does
not create a Task unless ordinary product materialization rules determine the request
requires durable outcome work. Conversation-scoped capability invocations are read-only.
The retry route applies only to a FAILED turn in the same Conversation; it creates a new
Conversation-scoped AgentSession and preserves messages from earlier failed sessions.
The cancel route requests interruption and returns `202` while the agent is stopping. It
returns a settled outcome when stop, failure, or a racing completion is observed. A
structured UserRequest response that belongs to a ConversationTurn first moves that exact
turn from `WAITING_USER` to `WAITING_DEPENDENCY` after the immutable response commits. The
prior session is closed and its message provenance remains unchanged. For MCP
`input_required`, an update acknowledgement is not provider acceptance; the Runtime polls
the existing provider task until the exact input key is resolved. The Runtime starts a
fresh Conversation session only after provider acceptance. If the provider rejects the
response but remains `INPUT_REQUIRED`, the old UserRequest closes and a new one is created
for its current request key; the turn returns to `WAITING_USER`. If acceptance is
ambiguous, it stays `WAITING_DEPENDENCY` until reconciliation. The turn returns to
`RUNNING` only after provider readiness and session readiness are confirmed. The status
projection distinguishes “Waiting for you” from “Waiting for service”.

Pause is separate from cancel. `/pause` stops new Attempt admission and asynchronously
coordinates safe-boundary interruption, ResumePacket creation, provider Invocation and
active VerificationRun settlement, Effect reconciliation, and lease release. It may return
`PAUSE_REQUESTED` until those steps settle. `/resume`
revalidates inputs, resources, grants, capabilities, secrets, budget, and placement.
Restarted work normally gets a fresh Attempt/lease; a quiescent provider-input Attempt may
continue in place only on its original Runtime incarnation/Environment with a fresh
higher-epoch lease and reconciled checkpoint/Effects. Recoverable failure uses the Step-scoped
`/steps/{step_id}/recover` command after Effect reconciliation and retry-budget checks.
There is no Task-level retry; a terminal FAILED Task remains terminal.
The execution-dependencies read returns a short-lived placement plan with eligible
existing Environment candidates. Recovery may include
`placement_override: { candidate_id, plan_digest }`; the pair must be supplied together.
TaskService rechecks the plan digest and current eligibility at commit, returning
`STALE_VERSION` without creating an Attempt if inputs, policy, offers, Runtime incarnation,
or Environment state changed. The request's `If-Match` header carries the expected Task
version and its body carries `expected_step_version`; both must match the preview basis.
Omitting the override leaves placement automatic.

## Approvals

```text
GET  /v1/approvals?status=PENDING&cursor=&limit=
GET  /v1/approvals/{id}
POST /v1/approvals/{id}/resolve
```

Resolution body:

```text
{
  decision: APPROVE | DENY
  confirmation_digest?
}
```

## Library / artifacts

```text
GET  /v1/artifacts?library_status=ARCHIVED&cursor=&limit=
GET  /v1/artifacts/{id}
GET  /v1/artifacts/{id}/versions/{version}
GET  /v1/artifacts/{id}/versions/{version}/content
GET  /v1/artifacts/{id}/edit-head
POST /v1/artifacts/{id}/text-version
POST /v1/artifacts/{id}/promote
POST /v1/artifacts/{id}/archive
GET  /v1/library?cursor=&limit=
```

Promotion/archive commands require `If-Match` with the current Artifact aggregate version. Promotion accepts TRANSIENT artifacts; archive transitions SAVED artifacts and returns the current representation for an already archived Artifact when If-Match names its current version, without emitting another event; stale If-Match returns STALE_VERSION. Neither changes the immutable content version. `STALE_VERSION`, `ARTIFACT_ARCHIVED` (for content publication after archive), and `INVALID_ARTIFACT_TRANSITION` are returned as applicable.

`GET /v1/library` returns SAVED artifacts; `GET /v1/artifacts?library_status=ARCHIVED` backs the Archived filter. Archived Artifacts remain readable but cannot receive new content versions.

The local Operator mounts both Library commands with empty request bodies, Workspace owner
authorization, If-Match and principal-scoped Idempotency-Key. Responses are the committed
Artifact representation, use `Cache-Control: no-store`, and preserve `current_version`,
Resource identity and all immutable metadata. A changed action, Artifact, Workspace or
expected version under the same key returns `IDEMPOTENCY_CONFLICT`. Fresh writes require
an ACTIVE Workspace; identical recorded retries remain readable after later archival.
Linked Artifacts use these commands for local Library metadata only, without provider I/O.

The desktop Workbench's recent-history panel uses the existing exact-version metadata
route, requesting at most the latest ten committed version numbers from the loaded
Artifact head. It is a bounded client projection, not a new server-side list endpoint or
a live history stream; omitted older versions remain directly addressable by their exact
version number.

`GET /v1/artifacts/{id}/versions/{version}/content` streams the immutable version bytes after rechecking Workspace authorization; the response Content-Type is the stored media type. A deployment may redirect through a short-lived URL scoped to that blob digest, but the logical resource and authorization check stay the same.

The desktop text editor may publish a new immutable version only for a current managed
`text/plain` Artifact no larger than 1 MiB. `GET .../edit-head` returns the Artifact
aggregate version, content version, backing Resource version, and exact parent revision.
`POST .../text-version` requires that head snapshot in a strict JSON body, `If-Match` for
the Artifact aggregate version, and `Idempotency-Key`; it accepts no provider-backed,
HTML, or other media type. The body is limited to 1 MiB of UTF-8 text; line breaks and
tabs are allowed, while other Unicode control characters are rejected. A new
ArtifactVersion and ResourceRevision, both domain
events, dependency updates, and the replay receipt commit atomically. An identical retry
returns the original committed `{artifact, version, replayed}` receipt, including after
later archival; a new request against an archived Workspace or Artifact is rejected.
Stale heads return `STALE_VERSION` and require reload/review before republishing. Both
initial and replayed successful responses carry the committed event's
`X-Correlation-ID` and `Cache-Control: no-store`.

The desktop may offer a text-only restore by placing the exact verified content of a
historical managed `text/plain` version into a user-reviewed draft and publishing it with
this same endpoint against a freshly loaded current edit head. Confirmation plus the
explicit Publish action are required. This creates a new immutable version and preserves
the existing stale-head conflict behavior; it adds no restore endpoint. The current route
records `user.text_edit` provenance and does not persist a distinct `restored_from` link.

The content endpoint checks Workspace owner authorization, Artifact visibility, and current
authorization on every transfer. A signed URL, if used by a deployment, is short-lived
and scoped to one immutable blob digest. Artifact list, metadata, version, and content
responses use `Cache-Control: no-store` so local clients and intermediaries do not retain
Workspace artifact metadata or bytes as reusable cache entries.

## Automations

```text
POST /v1/routines
GET  /v1/routines?cursor=&limit=
GET  /v1/routines/{id}
GET  /v1/routines/{id}/revisions?cursor=
POST /v1/routines/{id}/revisions
POST /v1/routines/{id}/run
POST /v1/routines/{id}/archive
GET  /v1/routines/{id}/health

POST /v1/automations
GET  /v1/automations?cursor=&limit=
GET  /v1/automations/{id}
PATCH /v1/automations/{id}
POST /v1/automations/{id}/pause
POST /v1/automations/{id}/resume
POST /v1/automations/{id}/disable
GET  /v1/automations/{id}/revisions?cursor=
GET  /v1/automations/{id}/occurrences?cursor=&limit=
POST /v1/automations/{id}/run
```

RoutineRevision is the reusable work definition; each revision is immutable. A manual
Routine run creates a normal Task and records `routine_id`/`routine_revision`. An
AutomationRevision references one RoutineRevision and one or more independent TriggerSpecs
with ANY semantics. Updating a Routine does not silently change an Automation; explicitly
revise the Automation to pin the new RoutineRevision. A manual Automation run is accepted
only when that Automation has a ManualTrigger. Occurrences pin AutomationRevision,
RoutineRevision, `trigger_id`, and TriggerHost. A due occurrence may already have a Task
while that Task is waiting for a required local Runtime/resource; report it as
`WAITING_DEPENDENCY`, separately from a not-yet-due occurrence.

For local manual Automation Run now, `POST /v1/automations/{id}/run` requires the selected
`X-Workspace-ID`, `Idempotency-Key`, `If-Match` Automation version, and body
`{ "automation_revision": n, "inputs": {} }`. `automation_revision` must be the exact
current revision on a new command. `inputs` is the bounded Routine input object and is
validated against the exact RoutineRevision pinned by that Automation. The owner command
requires a ManualTrigger and an active local TRIGGER_HOST binding. It is valid while the
Automation is PAUSED and never enables recurring triggers. The daemon atomically creates
the occurrence at version 1, claims it at version 2/claim epoch 1, and commits an ordinary
Task in `READY` plus occurrence `STARTED` at version 3. It creates no Plan, Attempt, or
AgentSession. Exact same-key retries return the original response; changed inputs,
revision, expected Automation version, or trigger conflict. A disabled Automation,
stale revision/version, inactive Coworker/Routine, unsupported host placement, or missing
ManualTrigger fails without a Task or occurrence.

The first successful response is `201`; an exact receipt replay is `200`. Both responses
describe a saved READY Task and an occurrence whose STARTED state means materialized, not
that planning or agent execution began. Authentication and immutable AutomationRevision
lookup still precede receipt resolution; mutable Runtime binding, Resource, Coworker, and
Automation-head admission checks do not.

For local manual Routine Run now, `POST /v1/routines/{id}/run` requires the selected
`X-Workspace-ID`, `Idempotency-Key`, and body `{ "routine_revision": n, "inputs": {} }`.
It returns `201 TaskView` only after atomically committing an ordinary `READY` Task pinned
to that exact active/current revision and validating all supplied input/resource refs.
The route rejects non-null `conversation_id`; no Plan or execution starts. Same-key,
same-command retries return the original committed Task; changed command content conflicts.
The desktop exposes schema-generated TEXT fields and disables Run when a required Resource
input cannot be selected safely. Routine `required_capabilities` and `verification_policy`
remain on the pinned RoutineRevision for future planner/Trust/verifier admission; this
save-only endpoint does not claim those policies were executed.

`GET /v1/automations/{id}/revisions` is Workspace-scoped and returns immutable revisions
in descending revision order; its first page contains the current revision for exact
edit-form reconstruction. The opaque cursor is bound to Workspace and Automation. Desktop
create/revise requests pin a Routine ID and exact revision loaded from the selected
Workspace. Both request bodies include `coworker_ref`, either an exact
`{ coworker_id, revision }` reference loaded from that Workspace or `null`. Omitting the
field is invalid, so a revision explicitly preserves, changes, or clears its Coworker pin.
The daemon rechecks the referenced Coworker revision and Workspace in the same write
boundary as the Automation revision. Creation persists `PAUSED`; revision requires the current Automation to be
`PAUSED` and preserves that state. Revising an `ENABLED` Automation returns
`AUTOMATION_NOT_PAUSED` (409), and a `DISABLED` Automation remains terminal. This route does not expose Automation resume,
manual run, or trigger execution in the current local build.

Routine revisions require non-empty objective and instruction text. The saved Routine name
is aggregate metadata and remains unchanged when appending a content revision; the current
local Operator API does not expose a separate Routine rename command.

An AutomationRevision may pin a `coworker_ref` (Coworker ID and exact revision). The
occurrence Task uses that revision's lead, worker allowlist, budget, context, and
interaction defaults while admission separately requires the Coworker to remain `ACTIVE`.
Pause/archive prevents new scheduled Task admission; already-admitted Tasks retain their
TaskSpec/origin pins. Revise an Automation to adopt a later CoworkerRevision.

Trigger placement (`HUB`, `SPECIFIC_RUNTIME`, `AUTO`) is independent of Task execution
placement. `AUTO` resolves and persists one owner per trigger; changes require fenced cursor
handoff. Schedule and one-shot triggers require an explicit `MisfirePolicy` (`SKIP`,
`RUN_ONCE_WHEN_AVAILABLE`, or bounded catch-up). Wake attempts are optional and best-effort.

## Local Runtime lifecycle

The desktop first performs an authenticated `GET /v1/operator/readiness` handshake. It
returns `operator_state=SERVING`, the local bootstrap Runtime/incarnation IDs from the
current local connection metadata. `SERVING` proves only that this Operator API is serving
for that local daemon incarnation; it does not mean Runtime execution is ready. The daemon currently remains `DEGRADED` with
Task recovery unavailable. This local handshake does not publish or register a Mesh
Runtime.

```text
GET   /v1/operator/readiness
```

```text
GET   /v1/runtime-lifecycle
PATCH /v1/runtime-lifecycle/startup-policy
GET   /v1/runtime-lifecycle/stop-preview
POST  /v1/runtime-lifecycle/stop
```

These are the planned local-Operator operations. In the current source, only
`GET /v1/operator/readiness` is mounted; the four `/runtime-lifecycle` routes listed
above are not implemented. In particular, no authenticated Operator stop command is
exposed. Do not emulate one with a signal, process kill, or direct socket shortcut.
Implementing it requires a RuntimeLifecycleService that inventories and drains admitted
Attempts, reconciles Effects, fences/releases leases, and settles local trigger/provider
references before process exit. Those owners are not integrated, so the current Runtime
must fail closed rather than claim a safe stop preview or accepted drain.

When implemented, these are local-Operator operations. Desktop Tauri calls use the OS-authenticated local
IPC contract in [`LOCAL-OPERATOR-IPC.md`](LOCAL-OPERATOR-IPC.md); Windows currently fails
closed. Source integration is unqualified pending verification and OS testing. They do not
configure or stop a remote Runtime. Startup policy is `MANUAL`, `LOGIN_BACKGROUND`, or
`ALWAYS_ON_SERVICE`. The read-only stop preview returns dependent Tasks,
Automations, WorkspaceRoots, and eligible handoffs. The UI confirms `Cancel`, `Move
eligible work to Cloud`, or `Stop anyway`; it cannot silently terminate active local work.
After confirmation, the stop command must include the previewed `expected_incarnation_id`.
The Runtime rechecks current dependencies before beginning drain; a preview is not a
reservation, and a changed incarnation rejects the request as stale. `CANCEL` makes no
change; `HANDOFF_ELIGIBLE` moves only work that passes ordinary handoff checks;
`STOP_ANYWAY` never authorizes duplicate Effects or stale lease reuse. A `202` receipt
means drain was accepted, not that the process has exited.
The OS service manager owns daemon start/stop. Closing the Operator window is a separate
operation. Runtime descriptors expose the current RuntimeIncarnation and expiring offer
readiness; no worker is started just by listing an Agent or capability.

## Runtime/devices

Runtime descriptors are filtered through the selected Workspace's active
`RuntimeWorkspaceBinding`; the descriptor itself is installation-scoped and carries no
single `workspace_id`. A Workspace can enroll the local installation without pairing it
to Mesh. That enrollment does not create presence or enable replication.

```text
GET  /v1/workspaces/{workspace_id}/runtime-bindings
GET  /v1/workspaces/{workspace_id}/runtime-bindings/current-local
POST /v1/workspaces/{workspace_id}/runtime-bindings/local-enrollment
POST /v1/workspaces/{workspace_id}/runtime-bindings/{binding_id}/revoke
```

Local enrollment is available only over authenticated same-installation IPC. Mesh
pairing creates a separate `MESH_PAIRING` binding after the Hub consumes a valid token.
Revoking one binding is Workspace-scoped; before revocation, the owner must clear a
Workspace Mesh-hub pointer and drain ChannelHost leases/assignments and enabled TriggerHost
cursors on that binding. RuntimeMesh either commits a move with the target lease or safely
clears the released ChannelHost assignment when no target is eligible; the ChannelBinding
then remains visible as DEGRADED/unassigned and cannot accept or send channel events until
an owner assigns a new host. This clear is an internal part of Runtime-binding revocation,
not a public assignment-delete route. Receipt admission and DRAINING serialize per
ChannelBinding: a receipt inserted first is included in source reconciliation; a drain
committed first means the source does not insert or acknowledge new provider delivery.
Quiescent release requires all source-epoch RECEIVED/PROCESSING receipts settled, outbound
Effects reconciled, and Hub durability confirmed. Installation-wide device revocation
remains a separate Runtime Mesh operation. Every route requires the matching selected Workspace,
owner Principal, idempotency key for mutations, and expected binding/Workspace version.
The desktop Settings screen uses `current-local`, which returns only the active enrollment
for the exact current Runtime incarnation (zero or one row). It does not represent the
full Workspace binding inventory; that remains the responsibility of the unfiltered
`runtime-bindings` operation.
The no-target state is a nullable host-assignment projection with ChannelBinding status
`DEGRADED` or `REVOKED`. The owner assignment command fences both the ChannelBinding
aggregate version through `If-Match` and the assignment snapshot through
`expected_assignment_version` (null only when currently unassigned). It can assign or move,
but cannot clear/delete an assignment. RuntimeMesh performs safe clearing internally as
part of RuntimeWorkspaceBinding revocation; assignment history remains in the event journal.


```text
GET  /v1/runtimes?cursor=&limit=
POST /v1/runtimes/pair-token
POST /v1/runtimes/{id}/revoke
GET  /v1/runtimes/{id}/offers
GET  /v1/runtimes/{id}/incarnations?cursor=
GET  /v1/runtimes/{id}/capability-hosts?cursor=&limit=
```

The pairing-token request names the Workspace and allowed initial Runtime roles. The Hub
sets expiry under its policy and returns the opaque one-use token only in that response.
The token can create one Runtime identity and is invalidated after successful pairing.

`capability-hosts` returns normalized provider instance state, health freshness, and the
count of active LiteCowork Activations using each instance. This count is derived from
LiteCowork scope references; it is not LiteSPM's process-wide client count. The response
never exposes a LiteSPM provider handle, process identity, credentials, or package-private
fields. Expired observations are returned as `UNKNOWN`/stale and cannot satisfy admission.

## Agents

```text
GET  /v1/agent-installations
GET  /v1/agent-profiles
POST /v1/agent-profiles/probe
GET  /v1/agent-bindings
POST /v1/agent-bindings
GET  /v1/agent-bindings/{id}
POST /v1/agent-bindings/{id}/enable
POST /v1/agent-bindings/{id}/disable
GET  /v1/agent-bindings/{id}/session-options
GET  /v1/agent-bindings/{id}/harness-capabilities
GET  /v1/agent-bindings/{id}/quota-observation
```

`agent-installations` is an authenticated runtime-local snapshot with no Workspace
scope because it reports installed software. It invokes only each supported
executable's `--version`, accepts a bounded version-shaped value, and discards
arbitrary process output. The response is cached for at most five seconds and does not
persist executable paths. `authentication=UNKNOWN` and
`session_readiness=NOT_PROBED` are permanent inventory values; later negotiated status
belongs to AgentProfile/RuntimeOffer observations. An installation row cannot be selected
as a lead or delegated worker; that requires a negotiated AgentProfile and an explicitly
created AgentBinding.

`agent-profiles/probe` is a separate, explicit owner action. V1 accepts
`provider_key=CODEX` or `provider_key=OPENCODE`, uses the daemon's admitted local
executable/environment binding, and returns a sanitized AgentProfileView plus its
RuntimeOffer observation. It does not start a Task or establish that a model is entitled
to perform inference. A successful Codex `model/list` or OpenCode `/config/providers`
response is catalog discovery only. OpenCode reads only `/provider` and
`/config/providers` and returns bounded provider/model IDs and display names plus
explicitly named reported connected-provider IDs; the result never includes native
provider/model objects, options, headers, keys, URLs, or raw responses. OpenCode's
reported connected list is not an authentication or entitlement claim. Its model catalog
is display-only, `session_model_selection=NOT_QUALIFIED`, and its offer is always
`compatible=false`, so it cannot create or enable an AgentBinding. Authentication is
reported only from each adapter's bounded evidence; account identifiers, native protocol
payloads, endpoint locators, and private configuration are never returned. The probe is
bounded and stops its owned host, but does not prove descendant-writer quiescence;
therefore neither probe is a production-safe AgentSession or switching path. The selected
Workspace must first have an ACTIVE `LOCAL_ENROLLMENT` RuntimeWorkspaceBinding for this
exact local Runtime incarnation with both `EXECUTOR` and `OPERATOR_ENDPOINT` roles. This
is a distinct owner action; Operator readiness alone is not Workspace authorization.
Every explicit probe call runs a fresh bounded observation and refreshes an expiring
RuntimeOffer, so the probe operation does not accept `Idempotency-Key` and must not be
automatically retried by a client.

AgentProfile and AgentEndpoint are stable identities; the profile response joins them to
per-endpoint RuntimeOffer observations (Runtime/incarnation, readiness, compatibility,
and expiry) without exposing endpoint locators. Bindings are durable Workspace records.
Create requires an observed compatible endpoint and always creates a disabled binding. Agent setup/auth
flows remain adapter-owned; the API accepts only an opaque `SecretRef` and non-secret
configuration. Enable/disable are versioned and idempotent. Disabling blocks new
admission immediately while already admitted sessions remain pinned and settle safely.
Creation may pin an exact discovered endpoint or save required features and preferred
topologies for later compatible selection; protocol choice is not a universal ranking.
Each profile observation includes sanitized `constraints` for that current offer. A
Codex probe may include bounded model-option metadata and protocol/authentication
observations there, but no raw App Server messages, account identity, paths, configuration
contents, or credentials. An OpenCode probe includes only its bounded sanitized display
catalog and labeled `/provider.connected` observation. That connection summary is not
treated as authentication evidence. OpenCode remains incompatible and cannot be bound or
enabled until its session/model option semantics and execution lifecycle are qualified.
Model discovery does not prove inference entitlement. The offer is expiring
Runtime-operational state, not an AgentSession or evidence that a Task can execute.
`lead_eligible` is distinct from `enabled`: a worker-only binding cannot be selected as a
lead and returns `AGENT_NOT_LEAD_ELIGIBLE` when explicitly selected. Session-option and harness-capability routes return fresh adapter-negotiated
metadata only; they never return native configuration files, credentials, or local
endpoint locators.

Quota observation returns the latest source-named observation, including `UNKNOWN` and
its expiry, or JSON `null` when no observation exists. An expired observation is
projected as `UNKNOWN`; clients must not display stale `LOW`/`EXHAUSTED` as current state.

## Delegation profiles

```text
GET    /v1/delegation-profiles?agent_binding_id=&status=&cursor=&limit=
POST   /v1/delegation-profiles
GET    /v1/delegation-profiles/{id}
POST   /v1/delegation-profiles/{id}/duplicate
GET    /v1/delegation-profiles/{id}/revisions?cursor=&limit=
POST   /v1/delegation-profiles/{id}/revisions
POST   /v1/delegation-profiles/{id}/status # ENABLED | DISABLED | ARCHIVED
GET    /v1/delegation-profiles/{id}/performance?task_category=
```

Create binds a profile to an enabled same-Workspace AgentBinding and creates revision 1
disabled. The current desktop implementation accepts only an empty `session_options`
object because it has no current-descriptor validation path yet. The name is part of the
immutable revision; a rename therefore creates a revision. Names are trimmed,
NFC-normalized, and Unicode case-folded for uniqueness within one binding among
non-archived profiles. Duplicate requires the source version in `If-Match`, copies its
current non-secret revision into revision 1 of a new disabled profile on the same binding,
and copies no runtime or authority state. Its idempotency key makes retries return the
same created profile. Export/import is deferred; future portable templates must define
destination compatibility and owner review. The current desktop Operator fails closed
with `DEPENDENCY_UNAVAILABLE` for `ENABLED` until adapter descriptor, Trust, and
Environment admission validation are integrated; disabling and archiving remain
available. Enablement must validate session options, required features, Environment
policy, and Trust ceiling, and does not start an agent host. Revisions are immutable and
use `If-Match` on the profile version. Existing Attempts keep
their pinned revision. Delegation selection is `AUTOMATIC`, `PREFER`, or `REQUIRE`:
`AUTOMATIC` ranks eligible profiles; `PREFER` ranks the named profile first and may use
another profile only when Workspace policy allows fallback; `REQUIRE` fails without
substitution. A user explicitly choosing a profile uses `REQUIRE`. Delegation targets a
READY Step in the accepted PlanRevision and requires the current parent
Attempt/session/lease. See `DELEGATION.md` for the exact data and admission algorithm.

`performance` is a rebuildable projection by bounded TaskCategory; it reports sample
count and confidence and never includes prompt/output content. Small samples cannot
override an explicit profile choice. Cost fields retain compatible provider units and
confidence; the underlying Task Usage route remains the source-attributed record.

## Coworkers, Goals, and Suggestions

```text
GET    /v1/coworkers?status=&cursor=&limit=
POST   /v1/coworkers
GET    /v1/coworkers/{id}
GET    /v1/coworkers/{id}/revisions/{revision}
GET    /v1/coworkers/{id}/presence
POST   /v1/coworkers/{id}/revisions
POST   /v1/coworkers/{id}/status     # ACTIVE | PAUSED | ARCHIVED
POST   /v1/workspaces/{workspace_id}/primary-coworker

GET    /v1/goals?status=&coworker_id=&cursor=&limit=
POST   /v1/goals
GET    /v1/goals/{id}
POST   /v1/goals/{id}/revisions
POST   /v1/goals/{id}/status        # ACTIVE | PAUSED | COMPLETED | ARCHIVED

GET    /v1/suggestions?status=PROPOSED&cursor=&limit=
POST   /v1/suggestions/{id}/resolve # DISMISSED only
POST   /v1/suggestions/{id}/accept-task # atomically create a READY Task; returns a bounded acceptance receipt
POST   /v1/suggestions/{id}/snooze
GET    /v1/workspaces/{workspace_id}/suggestion-preferences
PUT    /v1/workspaces/{workspace_id}/suggestion-preferences/{kind}
```

All routes require Workspace context and owner authorization. Create/revision commands
are idempotent; status commands use `If-Match`. Primary Coworker selection is a versioned
Workspace command and accepts `coworker_id: null` to clear it. Goal completion is always
owner-authorized. The exact Coworker revision read is owner- and selected-Workspace-scoped,
returns the immutable `{ coworker_id, revision, definition, authored_by, created_at }`
record, and uses `Cache-Control: no-store`; it never substitutes the Coworker's current
revision for the requested historical revision. A missing Coworker or revision returns
the same non-disclosing unavailable result. `POST /v1/suggestions/{id}/accept-task` requires the current Suggestion
version in `If-Match` and an `Idempotency-Key`. It copies the exact proposed objective,
constraints, inputs, outputs, acceptance criteria, budget, and deadline into a normal
TaskSpecRevision; pinned source Resource revisions remain exact Task inputs. If the
Suggestion has a Coworker origin, the Task pins the current Coworker revision and expected
head version. SQLite commits the READY Task, its event/snapshot/idempotency receipt, the
accepted Suggestion, and `suggestion.resolved.v1` in one transaction. The `201` response
contains `SuggestionTaskAcceptanceReceipt` with `disposition=CREATED`; the receipt is
bounded to Workspace identity, Suggestion ID/status/result Task/version, and Task
ID/Workspace/status/version. Clients require both Workspace IDs to match the selected
Workspace, Suggestion status `ACCEPTED`, Suggestion result Task ID equal to the Task ID,
and Task status `READY`. A lost-response retry with the same Idempotency-Key returns the
same original committed Task link in a `200` receipt with `disposition=REPLAYED`; storage
validates the accepted Suggestion and immutable Task request receipt together, and does
not rebuild the Task from a newer Coworker revision. Reusing a different request key is a
conflict and does not reveal the linked Task. The endpoint never creates a plan, starts an agent,
or begins execution. Suggestion acceptance for Routine/Automation actions only opens the
owning editor; saving reusable work remains a separate explicit command. Suggestions
cannot grant authority or run work.

Goal mutations require `Idempotency-Key`; revision and status mutations also require
`If-Match` with the current Goal aggregate version. Related Task, Routine revision,
Artifact-version, and optional Coworker references are validated in the selected Workspace; Artifact links pin
`{workspace_id, artifact_id, version}`. Adding or removing a link creates a new immutable
Goal revision, so old revisions preserve their original grouping. Goals remain
passive and never create Tasks or start Automations. List/get responses include a
read-only projection of current linked Task status, bounded committed Task Evidence IDs,
and Evidence references resolvable from each exact Goal-pinned Artifact version.
Until VerificationRun and dependency-freshness readers are integrated, the projection is
`PARTIAL`, reports typed limitations, and returns null for verified/stale/conflicted
counts it cannot prove. A `COMPLETED` Task remains `UNVERIFIED` without current criterion-
and-input-bound passing VerificationRuns; worker summaries and synthetic zero counts are
never substituted.
Snooze is an idempotent owner command with a requested `snoozed_until` no later than the
Suggestion's expiry; it changes visibility while the Suggestion remains `PROPOSED`.
Workspace preferences mute one `SuggestionKind`. Muting atomically dismisses currently
proposed items of that kind and prevents new proposals; unmuting affects only future
proposals. The preference list returns all kinds, including virtual defaults with
`muted=false`, `version=0`, and `updated_at=null`; persisted preference versions return
their committed timestamp. `PUT` uses `If-Match` and `Idempotency-Key`; `If-Match: 0`
selects an absent virtual default. Muting atomically stores the new preference and
resolves all unexpired `PROPOSED` items of that kind as `DISMISSED` with reason
`MUTED_KIND`, including snoozed items. The preference event, each Suggestion resolution
event/snapshot, and the idempotency receipt commit in one SQLite transaction. A retry
with the same key and request returns the original preference result. Individual
dismissal suppresses the same dedupe key for 30 days. Preference changes do not run or
authorize work.

Suggestion producers are registered deterministic rules or separately authorized
read-only capabilities. SuggestionService records non-secret `proposed_by` provenance and
validates exact same-Workspace source/Goal revisions, mute/cooldown/dedupe rules, and
expiry. Producers cannot commit Suggestions or Tasks directly. V1 has no provider-generated
memory-proposal path.

```text
GET /v1/tasks/{id}/progress
```

The desktop/local route is authenticated and scoped to the selected Workspace owner. It
returns a `no-store` read model from one bounded persisted Task-presentation snapshot;
over-cap Step, Artifact, or blocker collections fail rather than returning silently
truncated progress. `last_activity_at` currently comes from the latest committed Task,
current-plan Step, or persisted current-Attempt event, including a terminal current
Attempt. `active_workstreams` include only nonterminal current Attempts. `last_evidence_at`
is an independent Evidence timestamp. CapabilityInvocation, provider-progress, and
Environment-observation sources are not yet integrated in this route and must not be
presented as live observations. This endpoint does not mutate Task state or establish
process/provider liveness.

Coworker presence, Task progress, Goal contributions, and worker performance are
read-only rebuildable projections. Presence keeps proactive status, current Task activity,
and Runtime availability as separate axes. Task progress distinguishes latest observed
activity from latest Evidence and contains no synthetic percentage or ETA. Goal
contributions identify linked Task outcomes and Evidence; they do not infer that a Goal's
free-text success criteria are satisfied.

## Demonstrations

```text
POST /v1/demonstrations
POST /v1/demonstrations/{id}/complete
POST /v1/demonstrations/{id}/pause
POST /v1/demonstrations/{id}/resume
POST /v1/demonstrations/{id}/abort
```

Capture requires explicit semantic consent, a supported Environment, and an immutable
`DemonstrationCapturePolicy` (maximum duration, action count, trace bytes, Environment
class, and sensitive-region behavior). Pause/resume is owner-controlled; detection of a
sensitive region either pauses capture or omits sensitive fields according to the selected
policy. Reaching a cap stops further observation. A completed trace creates a reviewable
SkillProposal; it never installs or publishes a package.

Capture requires an explicit user-controlled Environment and consent. The semantic trace
is redacted and stored as a Resource. Completion creates a SkillProposal for review; it
never installs or publishes a package.

## Connections and channel bindings

```text
GET   /v1/connections?status=&cursor=&limit=
GET   /v1/connections/{id}
POST  /v1/connections/{id}/disconnect

GET   /v1/channel-bindings?status=&cursor=&limit=
GET   /v1/channel-bindings/{id}
PATCH /v1/channel-bindings/{id}
POST  /v1/channel-bindings/{id}/revoke
POST  /v1/channel-bindings/{id}/host-assignment
```

These routes expose normalized connection/binding metadata and owner controls only.
Provider-owned setup establishes the authenticated account/identity and creates the
records; the Operator API does not invent an OAuth, device-code, webhook-secret, or
provider callback flow. `PATCH` changes only the binding's allowed actions, uses
`If-Match`, and cannot increase authority without TrustService approval. Disconnecting
a Connection or revoking a ChannelBinding blocks new use/inbound commands but preserves
history and does not delete external accounts or credentials. Provider-specific setup
and reauthentication contracts remain deferred with the relevant integration.
`GET /channel-bindings/{id}` includes a redacted current host-assignment projection.
The host-assignment command requests an explicit move to an eligible Runtime; the Hub
checks provider compatibility, secret placement, source drain proof or the pinned
expiry-plus-skew release record, then increments the host epoch. A continuity-safe move
requires provider cursor transfer or replay from the last Hub-replicated receipt and records
its evidence reference. If that is unavailable, the request must explicitly confirm
`accept_ingress_gap`; the authenticated owner decision is audited and the v2 assignment
history records its provenance with `GAP_ACCEPTED`. Existing v1 assignment events remain
unchanged. The API never returns the fencing credential or opaque cursor. A moved binding
cannot use old Runtime-local reply references.
Every successful move releases the source lease and commits the target ACTIVE assignment
with exactly one fresh lease in the same Hub transaction. Lease IDs are never reused across
host epochs; fencing credentials are freshly derived from the ChannelBinding, Workspace,
Runtime, host epoch, and unique lease ID. Reusing a lease identity or observing a credential
digest collision fails closed. No committed ACTIVE assignment is exposed without its
matching lease.

`RESPOND` allows only a reply-to-message correlation for an exact delivered FORM
UserRequest on that binding; it does not grant general Task steering or Approval
authority. The response is checked against the pinned schema, sender identity, assurance,
expiry, and current binding actions. External sign-in, Approval decisions, and unsupported
structured schemas remain in the Operator's Needs You surface.

## Discover/capabilities

```text
GET  /v1/discover/search?q=
GET  /v1/discover/items/{id}
POST /v1/capabilities/{id}/connect-or-install
POST /v1/capabilities/{id}/test
```

These are user-facing wrappers around LiteSPM/CapabilityBroker; they do not expose package-manager internals by default.
They never invoke LiteSPM from the client directly. The API implementation calls
CapabilityBroker; the detailed LiteSPM wire contract remains deferred in
`CAPABILITY-FABRIC.md`.

## Resource intake

The desktop can query ZIP extraction readiness without disclosing archive bytes:

```text
GET /v1/capabilities/zip-intake
```

This is an authenticated, Workspace-scoped capability observation. The current local
response is `UNAVAILABLE` with `resource_behavior=OPAQUE_RESOURCE_ONLY` and reason
`ISOLATED_WORKER_NOT_QUALIFIED`. It distinguishes an integrated provider and enabled
extraction from the current unavailable state. It never previews or extracts a Resource. ZIP upload
remains an ordinary resumable Resource upload and preserves the exact archive bytes. The
endpoint is read-only, returns `Cache-Control: no-store`, and creates no event, Grant,
CapabilityActivation, Invocation, Effect, or Artifact. Do not use a cached status to admit
an extraction request; extraction has no Operator command until the isolated worker and
Core publication path are qualified.

The local desktop uses resumable uploads for file intake:

```text
POST /v1/resources/quick-import             # bounded compatibility import for small local files
POST /v1/resources/uploads
GET  /v1/resources/uploads/{id}
PUT  /v1/resources/uploads/{id}/chunks/{chunk_index}
POST /v1/resources/uploads/{id}/commit
GET  /v1/resources?limit=100&cursor=...    # requires X-Workspace-ID; keyset-paginated
GET  /v1/resources/{resource_id}/content?revision_id=...&max_bytes=...
                                         # optional exact immutable revision pin and read bound
```

When `revision_id` is supplied, the endpoint resolves that exact immutable revision only
after proving it belongs to the selected Resource and Workspace. It never substitutes the
current head. Omitting the pin reads the current head. `max_bytes` is an optional positive
per-read limit no greater than the 10 MiB local Operator ceiling; storage checks the
selected revision's indexed length before BlobStore access. Historical bytes are available
only when the selected revision resolves through the local managed encrypted BlobStore;
external provider revisions return `RESOURCE_CONTENT_EXTERNAL`, while unavailable local
content returns `RESOURCE_LOCATION_UNAVAILABLE`. Every returned object is verified against
the immutable revision's exact byte length and SHA-256 digest. Archived Workspaces remain
readable under the existing owner authorization contract. Desktop text preview/comparison
requests `max_bytes=1048576` and allow only UTF-8 plain text or Markdown. Resource intake
may accept a 100 MiB file, but this endpoint is not a general large-file download path. A
future large-file reader must use bounded range/chunk transport rather than increasing the
IPC response allocation cap. Storage rejects a new content read before BlobStore access
when the Resource is a non-`ACTIVE` ContextDocument. The owner receives
`CONTEXT_DOCUMENT_NOT_ACTIVE` with a status-specific message; Resource metadata remains
available to show whether content is retained as `REVOKED`, being purged as
`DELETION_PENDING`, or already `DELETED`. A read admitted while `ACTIVE` may finish if the
status changes after admission, but derived-state commits must recheck status and cannot
publish after that transition. If a BlobStore read fails, the daemon rechecks the current
ContextDocument status: a proven transition returns `CONTEXT_DOCUMENT_NOT_ACTIVE`; if the
status remains active or cannot be proven, it preserves the original content failure:
BlobStore/location unavailability returns `RESOURCE_LOCATION_UNAVAILABLE` (503), while
verified-content integrity failures return `INTEGRITY_FAILURE` (500).

The authenticated local owner route
`PATCH /v1/resources/{resource_id}/context-document/status` accepts only `ACTIVE` or
`REVOKED`, requires `X-Workspace-ID`, `If-Match` with the current Resource version, and
`Idempotency-Key`. It commits the Resource status, aggregate snapshot, domain event, and
replay receipt in one SQLite transaction. Only `ACTIVE -> REVOKED` and `REVOKED -> ACTIVE`
are legal; same-state requests and stale versions conflict. Revocation blocks subsequent
content-read admissions and derived-index publication while retaining the bytes. It does
not recall content already delivered to a native AgentSession, and this route does not
claim to invalidate such sessions. `DELETION_PENDING`, purge receipts, and `DELETED` are
not writable through the local desktop Operator yet.

The desktop Library offers **Save original…** only for catalog entries at or below the
same 10 MiB bound. This is a native Tauri command, not a new logical Operator route: it
re-reads Resource detail and bounded revision history over authenticated local IPC, checks
Workspace identity, `kind=FILE`, the local-upload managed provider, current head, exact
revision digest/size/media type, and ACTIVE ContextDocument status before opening the
native save dialog. After selection it calls the content route with that exact
`revision_id`; the daemon rechecks Workspace ownership, active status, current head, and
stored-blob integrity. The native process checks returned media type and exact size, then
uses the existing same-directory atomic write. The command returns only `SAVED` or
`CANCELLED`; neither bytes nor the destination path cross into the WebView. Cancel fetches
no bytes and writes nothing. Linked/external, unavailable, non-file, stale, inactive, and
oversized content is refused. ZIP content is saved as its original opaque bytes; this
action does not inspect or extract archives, increase IPC bounds, or create Resource,
Artifact, Task, or Event state.

`POST /v1/resources/quick-import` remains as a bounded compatibility route for small local
attachments; the desktop Library uses the resumable protocol above.

The desktop accepts up to 100 files and 100 MiB per selection; each initial Resource is
limited to 100 MiB. The upload session negotiates fixed 4 MiB chunks and expires after 24
hours. The local daemon scans at most 100 expired sessions every 30 seconds and persists
`EXPIRED` with a `resource.upload.status.changed.v1` event using a progress-version check. Until the sweep runs, a
read may still show the prior status together with an elapsed `expires_at`; chunk and
commit mutations reject an elapsed session. Session creation is journaled with
`resource.upload.created.v1`; the final accepted chunk emits OPEN -> CONTENT_RECEIVED, and
Resource commit emits CONTENT_RECEIVED -> COMMITTED. Each lifecycle transition and its
aggregate-state snapshot commit atomically with the corresponding session/Resource update.
Definite stored-content integrity failure emits CONTENT_RECEIVED -> FAILED before returning
`INTEGRITY_FAILURE`; the upload cannot be resumed and must be recreated. Transient storage
or database failures leave the session retryable.
Chunk acceptance increments `progress_version`; lifecycle events use `version`, so chunk
traffic does not create gaps in aggregate event revisions. Chunk bytes are encrypted under
the distinct `RESOURCE_UPLOAD_CHUNK` BlobPurpose;
the SQLite chunk ledger stores only offsets, digests, and encrypted-blob references. An
identical chunk replay is accepted, conflicting content for an accepted index is rejected,
and `GET` returns durable progress. A zero-byte file has no chunk rows and proceeds
directly to commit. Commit concatenates verified chunks, verifies declared size and the
required whole-content digest, and atomically writes Resource metadata, its initial
revision/location, the versioned `resource.created.v1` or `resource.created.v2` event, the committed Resource ID on the upload
session, and the idempotency receipt. Session creation itself is idempotent. ZIP files are
still stored as opaque Resources. The separate content route rechecks active Workspace ownership,
current revision/location and encrypted-blob digest before returning bytes as
`application/octet-stream` with `no-store` and `nosniff`. The desktop text preview accepts
only text-like media types and limits the response to 1 MiB; it does not render active HTML
or execute imported content. The resumable upload contract is the normal desktop file
intake path.
The current local catalog is keyset-paginated in descending `(created_at, resource_id)`
order. `limit` defaults to 100 and must be between 1 and 100. `next_cursor` is opaque,
Workspace-bound, and resumes strictly after the last returned row; a cursor from another
Workspace is rejected. The desktop requests one page at a time and appends unseen Resource
IDs; the Library exposes an explicit “Load older files” action and rejects repeated cursors.
This list remains metadata-only; it does not provide deterministic content search or
implicitly attach Resource content.
Desktop's current in-memory name/type filter still applies only after catalog loading.

```text
POST /v1/resources/uploads                 # create bounded upload session
GET  /v1/resources/uploads/{id}             # read resumable progress
PUT  /v1/resources/uploads/{id}/chunks/{chunk_index} # resumable bounded chunk
POST /v1/resources/uploads/{id}/commit      # verify digest and return ResourceRef
```

Each Artifact response carries its stable `resource_id`; each ArtifactVersion carries its
`resource_revision_id`. Clients form a pinned ResourceRef from those values and the
Artifact's `workspace_id` to reuse the exact immutable output as a later Task input.
External ArtifactContent still retains its separate pinned source ResourceRef.

Create supplies the declared full byte size, media type, and required expected SHA-256.
The desktop computes it from the selected file before creating the session. Resume is
allowed only when the reselected file's digest matches the session; name, size, timestamp,
and previously accepted chunk digests alone do not identify the remaining bytes.
For one-time folder attachments, Create may also supply `folder_import.relative_path`.
The daemon validates normalized slash-separated segments, rejects absolute/drive/UNC paths,
dot segments, backslashes and control characters, and requires this value to match the
current compatibility `display_name`. The upload session pins it; commit copies that pinned
metadata to `Resource.provenance.folder_import`. It participates in idempotency and resume
matching. It is not a persistent WorkspaceRoot, file locator, or filesystem grant, and the
absolute selected folder path is never sent to the Operator.
Folder-derived Resource creation emits `resource.created.v2`; other Resource creation
retains `resource.created.v1`.
First-party user-authored context documents may additionally supply typed
`context_document` metadata; ResourceService validates that the metadata kind matches its
USER/Workspace/Coworker/Goal owner and that the owner belongs to the selected Workspace.
The upload session pins this metadata and applies it to the Resource created at commit.
The upload-session response echoes `ContextDocumentCreateMetadata` (owner and kind only);
the committed Resource response exposes full `ContextDocumentMetadata`, initially
`ACTIVE` without purge fields.
Omitting the field creates an ordinary Resource.
The local desktop V1 Library currently creates only `WORKSPACE_NOTES` owned by the selected
Workspace, through this same resumable upload contract. It does not expose USER, Coworker,
or Goal ownership until their owner-selection and authorization flows are implemented in
the desktop. The metadata classifies the Workspace-scoped Resource; it does not attach the
note to an AgentSession or enable semantic RAG.
The returned session is size-limited and expires and includes a fixed chunk size. The
client uploads indexed chunks with `Content-Range`, per-chunk SHA-256, and idempotent
chunk identity. The server reports the committed offset/ranges; an identical retry is
accepted, while conflicting bytes for an existing range return `UPLOAD_OFFSET_CONFLICT`.
HTTP `Content-Range` end offsets are inclusive. Persisted `ResourceUploadChunk`
`end_offset_exclusive` and response `UploadRange.end_offset_inclusive` make the conversion
explicit. `chunk_index` determines the only accepted start offset; all non-final chunks
have the negotiated chunk size. `GET` returns the current session and derived ranges.
Commit verifies complete coverage, total size, media policy, full-content digest, and any
pinned context owner, then creates a stable Resource and committed ResourceRevision and
returns a pinned ResourceRef. Uncommitted/expired uploads cannot be referenced by a Task. ArtifactVersion
creation is a separate ArtifactStore operation.

## Workspace roots and deterministic resource search

```text
GET    /v1/resources/search?q=&mode=METADATA|ON_DEMAND_CONTENT|INDEXED_CONTENT&kind=&freshness=&cursor=
GET    /v1/resources/{resource_id}
GET    /v1/resources/{resource_id}/locations
GET    /v1/resources/{resource_id}/revisions
POST   /v1/resources/{resource_id}/text-index/rebuild
POST   /v1/resources/{resource_id}/revision-uploads
PATCH  /v1/resources/{resource_id}/context-document/status
GET    /v1/resources/{resource_id}/context-document/deletion
POST   /v1/workspace-roots
GET    /v1/workspace-roots?status=&cursor=
PATCH  /v1/workspace-roots/{id}
POST   /v1/workspace-roots/{id}/pause
POST   /v1/workspace-roots/{id}/resume
POST   /v1/workspace-roots/{id}/revoke
```

`POST /v1/resources/{resource_id}/text-index/rebuild` is an owner-triggered local
maintenance command. It requires the selected Workspace header, `Idempotency-Key`, and
the exact `resource_revision_id` plus `content_digest` currently shown by Library. The
store verifies Workspace ownership and ACTIVE state and compares both pins with the current
Resource head. Eligible text sources are read through the at-most-1-MiB managed-byte path,
which verifies the digest, then the encrypted lexical projection and typed result receipt
are committed atomically. Unsupported/oversized inputs are declined from pinned metadata
without reading their bytes. Replaying the same key and payload returns the original outcome;
reusing the key for another Resource/revision/digest conflicts. A changed or conflicted
head and a mismatched idempotency-key reuse both return `CONFLICT`; no newer revision is
indexed implicitly.

An inactive ContextDocument is rejected before BlobStore bytes are read. The final index
transaction still rechecks active status and the exact Resource pin: a read admitted while
`ACTIVE` may finish after a concurrent status transition, but its prepared index is not
committed. The owner receives `CONTEXT_DOCUMENT_NOT_ACTIVE` with a message that
distinguishes retained revoked content from deletion in progress or completed.

Success is `INDEXED` or `NOT_INDEXABLE` with one of
`UNSUPPORTED_TYPE`, `OVER_SIZE_LIMIT`, `INVALID_UTF8`, `CONTROL_CHARACTERS`, or
`TERM_LIMIT_EXCEEDED`. The response contains only the operation identity and status; it
never returns source bytes, extracted text, or terms. `NOT_INDEXABLE` is a committed
idempotent outcome and removes any stale projection for the exact current revision. The
operation changes only rebuildable local index state: it emits no domain event, creates no
ResourceRevision, changes no Task, and grants no agent access. It does not extract ZIP or
Office content, perform OCR, create embeddings, or run in a background job.

`POST /v1/resources/{resource_id}/revision-uploads` is an authenticated owner operation
with required `If-Match` (current Resource version) and `Idempotency-Key` headers. Its body
contains `media_type`, `size_bytes` (0–100 MiB), a whole-content `expected_digest`, and
1–16 unique `parent_revision_ids`. Admission requires those parents to equal the
Resource's complete current head set and requires an available managed local content
location. The upload session pins the exact Resource version and parent set. Generic chunk
upload and commit then verify range coverage, chunk/content digests, total size and media
type; the atomic commit appends `resource.revision.created.v1`, advances the Resource head
with compare-and-swap, updates the managed location and derived index/invalidation state,
and commits the upload status and idempotency receipt. If the version or head set changed,
the commit returns `RESOURCE_CONFLICT` without publishing. A retry after a lost successful
response returns the original committed receipt. This route does not provide a merge editor
or ContextDocument revoke/delete/session-invalidation UI.

`GET /v1/resources/{resource_id}/revisions` returns at most `limit` rows after a
storage-side `limit + 1` keyset query (`limit` defaults to 50 and is capped at 200). The opaque cursor names the last revision ID
and is bound to the selected Workspace and Resource. Storage returns immutable append order;
because every ancestry edge references an already inserted revision, this order always
places parents before children. A page does not materialize or sort the Resource's complete
history in the daemon. The current SQLite schema has no indexed append ordinal, so the
database may still scan matching revision rows internally to satisfy append-rowid ordering;
an indexed bounded-I/O cursor would require a later additive schema migration.

The authenticated, owner-scoped list returns each root with `location_availability`,
joined from its canonical `ResourceLocation` in the same bounded SQLite read query. Values
are `AVAILABLE`, `OFFLINE`, `PLACEHOLDER`, `REVOKED`, `UNKNOWN`, or `UNAVAILABLE`. This is
the last committed location observation, not a live Runtime probe; root `status` and
location availability are separate fields (for example, a `PAUSED` root may have an
`OFFLINE` location). The projection contains no filesystem path, raw file identity, or
Runtime-local locator binding. Pagination, Workspace-owner authorization, and the
Operator's `no-store` response policy are unchanged.

The Operator API above is the logical root-management contract. The current desktop source
implements owner-scoped listing, pause/resume/revoke on the authenticated local Operator;
native folder creation uses a reserved Tauri-only IPC operation because a public request
must not accept an absolute path or caller-asserted filesystem identity. General
ResourceRef-based root creation and root-policy `PATCH` remain unimplemented.

Root creation requires an explicit user grant naming one selected folder Resource and
location. A one-time attachment does not create a WorkspaceRoot. `PATCH` is reserved for
updating watch/replication policy under expected-version checks. Pause, resume, and revoke
are separate idempotent versioned actions: pause retains local identity bindings and the
user's selected replication scope while root status suppresses observation/search/exposure/
replication; resume is accepted only for a PAUSED root whose location and locator/identity
bindings are AVAILABLE in this current Runtime incarnation; revoke is terminal and removes
those bindings and the selected-root relation without deleting already replicated copies.
Runtime offline or failed identity revalidation maps to `UNAVAILABLE`, not user pause. An
UNAVAILABLE root cannot be resumed through the user action; exact Runtime identity
revalidation is required first. The stable idempotency digest contains the action, Workspace,
root ID, and expected version; current Runtime identity is a separate resume commit
precondition. On a fresh resume request, the current handler reopens the saved directory
without following symlinks, compares the live file identity and keyed Resource projection,
retains the verified directory handle through the atomic resume commit, and refreshes the
current-incarnation locator/identity bindings. An exact replay returns its committed receipt
without re-opening the path. This is a point-in-time check only: future filesystem consumers
must independently validate/use a qualified handle-based provider. No watcher or content
reader currently consumes the root, and this source remains unbuilt and OS-unqualified.
`mode=METADATA` (the default) searches the authenticated Workspace's current managed
Resource catalog by literal display-name or media-type ASCII-case-insensitive substring,
with optional exact Resource-kind and supported freshness filters. `q` may be omitted or
empty to browse the catalog using only those filters; content modes require a non-empty
query. Supported freshness filters are `CURRENT`, `STALE`, `UNKNOWN`, and `UNAVAILABLE`.
`mode=ON_DEMAND_CONTENT` requires a
non-empty `q`; it scans at most 20 current managed candidates, reads no Resource larger
than 1 MiB, and reads at most 8 MiB total per request. Only allowlisted UTF-8 text
(`txt`, Markdown, CSV, JSON/JSONL, and selected source/config extensions or text media
types) is examined. ZIPs, PDFs, office documents, images, ungranted WorkspaceRoots, and
semantic embeddings are not searched. `ON_DEMAND_CONTENT` decrypts and scans bounded
bytes in request memory and persists no extracted text, terms, or snippets. The new
`INDEXED_CONTENT` mode matches all normalized query terms using the Workspace-keyed local
index for supported current managed text revisions. Extracted snapshots are encrypted;
SQLite stores only versioned HMAC term tokens. The candidate revision/digest and active
ContextDocument state are rechecked after decryption. A bounded snippet may be returned
with `CONTENT_ON_DEMAND` or `CONTENT_INDEXED`, respectively. Content mode also retains
explicit name/type matches and labels match reasons separately. The response's
`content_scan` reports limits only for `ON_DEMAND_CONTENT`; it is null for indexed search.
The cursor is bound to Workspace, query, mode and filters. A result is never an implicit
Task/Agent context attachment. This is deterministic lexical retrieval, not semantic RAG.

Every `ResourceSearchResult` carries a `PinnedResourceRef` and the exact
`source_content_digest` of that revision. Its required `source_matches` array is empty
except for `INDEXED_CONTENT` lexical hits; there it contains one first-occurrence record
per distinct normalized query term, with zero-based half-open UTF-8 byte offsets into the
original revision bytes. The offsets are meaningful only with that result's exact
`resource_ref.revision_id` and `source_content_digest`; they do not address a newer head
or the normalized display snippet. On-demand snippets currently have no offset provenance.

The mounted daemon route serializes the pinned digest and spans and fails closed if the
storage result's Resource revision, digest, matched-term count, or span bounds disagree.
The desktop validates the result against the normalized query, then fetches the exact
revision and verifies its digest before applying highlights. If verification fails, it
shows the exact-revision preview without highlights and reports that they could not be
verified. The Library uses spans only for preview highlighting; it does not expose durable
citation anchors or present these search hits as citations.

The revision endpoint returns immutable revisions in ancestry order with parent IDs and
head markers. If multiple heads exist, the Resource projection has a null
`current_revision_id`; clients must present a pinned ResourceRef or create a verified
merge before an unpinned reference can resolve. Choosing a branch is not a merge.

Revision upload creation pins `If-Match` and the exact current Resource head set before
accepting bytes. It also requires the whole-content SHA-256 to bind resumed chunks to the
same selected content. Stale Resource versions/parents return `RESOURCE_CONFLICT`; commit
verifies digest and atomically appends a ResourceRevision. ContextDocument creation always
starts `ACTIVE`. Revocation fences future resolution and Task attachments; deletion writes
a sealed, immutable purge target manifest and replicated tombstone in one transaction, then
blocks reads immediately. The deletion-status response exposes the manifest digest and
required/acknowledged target counts, not provider locators. A registered Core-owned
replica receipt must match its exact plan target and Runtime incarnation. An explicit empty
manifest proves there were no content/index replicas to purge. The Resource becomes
`DELETED` only after the acknowledged receipt set exactly equals the manifest. Historical
Task/Evidence IDs and digests remain without content bytes.

An unknown, cross-Resource, or duplicated parent revision is malformed input and returns
`RESOURCE_REVISION_PARENT_MISMATCH`; a valid but stale parent-head set or Resource version
returns `RESOURCE_CONFLICT`.

## User requests and notifications

```text
GET  /v1/user-requests?status=PENDING&cursor=
GET  /v1/user-requests/{id}
POST /v1/user-requests/{id}/respond
POST /v1/user-requests/{id}/external-handoff
POST /v1/user-requests/{id}/dismiss
GET  /v1/notifications?cursor=
GET  /v1/notification-preferences
PUT  /v1/notification-preferences/{event_class}
GET  /v1/skill-proposals?status=&cursor=
POST /v1/skill-proposals
POST /v1/skill-proposals/{id}/approve
POST /v1/skill-proposals/{id}/reject
```

UserRequest responses are validated against the pinned response schema and appended as
immutable records. Ordinary `FORM` answers are non-secret Workspace input and can
replicate under Workspace policy; credentials must never be entered there. `EXTERNAL_URL`
requests return only `{action: "accept" | "decline" | "cancel"}` from the response route.
The separate `external-handoff` command is available only for a pending external
authorization request after explicit owner authentication/action; the source Runtime
returns the provider URL only for that request with `Cache-Control: no-store`, without
storing it in an idempotency receipt. The URL is omitted from the
ordinary UserRequest projection, event journal, logs, analytics, and backups. For a Task-scoped request, `PAUSE_REQUESTED` returns `CONFLICT` and
leaves it pending; while the Task is `PAUSED`, a valid response is stored but provider-input
delivery waits for explicit resume and fresh owner authorization. Task-planning continuation
requires a fresh planner on the unchanged lead/TaskSpec; Attempt continuation requires the
same source Attempt/Runtime incarnation/Environment with a newly issued lease. A replacement
Attempt/Runtime cannot receive the old provider key; the old Invocation is reconciled/cancelled
and the answer may become bounded context for new authorized work. An Approval may be resolved
while paused, but cannot be consumed for an Effect or grant until resume. Cancellation
winning first closes pending requests and rejects later responses. An expired request
returns `USER_REQUEST_EXPIRED`; a dismissed,
cancelled, or already-answered request returns `CONFLICT` with its current status; a stale
version returns the applicable version conflict. None can resume a ConversationTurn or
Attempt. If a provider-backed request expires, its ConversationTurn remains `WAITING_DEPENDENCY` through provider
cancellation/reconciliation, then settles retryably as `FAILED`. A Task/Attempt request
blocks only its Step and exposes `NEEDS_USER` only when no independent work can proceed.
Responses do not implicitly approve an Effect or grant. Approval is a
separate Trust entity; the Needs you projection can combine pending UserRequests and
Approvals without conflating their response paths. Provider input keys and provider task
handles/cursors stay Runtime-private; the Operator projection exposes only sanitized
provider status, latest TTL/expiry, and poll guidance. Notification delivery is
deduplicated and transport acknowledgement is distinct from Task completion.

`respond` returns `SENSITIVE_INPUT_UNSUPPORTED` for a sensitive-typed or suspicious
credential request and `PROVIDER_INPUT_UNSUPPORTED` for an unsupported embedded provider
method. It does not write a response or dispatch input in either case. The external
handoff route rejects non-HTTPS URLs, URLs with userinfo, and IP-literal hosts. Its
no-store response is confidential even though no credential is posted to LiteCowork;
LiteCowork does not control system-browser DNS resolution or redirects.

Human control routes require the current `expected_control_epoch`. Takeover atomically
increments the epoch and fences queued or late agent input. Returning control requires a
fresh observation reference after drift/Effect reconciliation and creates a new Agent
control epoch; previously queued actions are discarded, never replayed. The Operator
receives an `EnvironmentControlLeaseView` containing owner and epoch only; it never
receives a raw fencing credential or digest.

## Invocation, usage, and dependency inspection

```text
GET /v1/tasks/{id}/invocations?cursor=
GET /v1/capability-invocations/{id}
GET /v1/tasks/{id}/usage?cursor=
GET /v1/resources/{id}/dependents?cursor=
GET /v1/dependency-edges/{dependencyEdgeId}/invalidations?cursor=
GET /v1/tasks/{id}/verification-runs?cursor=
```

These are read-only projections for Inspector and recovery. Provider-native task handles
and resume cursors remain Runtime-private; the projection never exposes them as LiteCowork
Task IDs or as directly reusable provider credentials. Usage displays source and
confidence, including UNKNOWN native-agent usage. Dependency inspection returns exact
consumed Resource revisions, each dependent ArtifactVersion or VerificationRun, its
current/stale/conflicted/unknown projection, and the latest invalidation. The separate
invalidation history route is paginated. It does not infer dependency from matching content
digests.

## Live event stream

```text
WS /v1/stream
```

Client subscribes:

```text
Subscribe {
  workspace_id # must equal X-Workspace-ID
  conversation_ids[]?
  task_ids[]?
  projection_types[]
  resume_cursor?
}
```

`projection_types` may include `runtime_inventory`; those notifications identify the
Runtime and projection version/change kind, not a raw provider handle. The Operator
re-reads `/runtimes/{id}/offers` or `/runtimes/{id}/capability-hosts` for current bounded
state. Server sends projection events, not raw domain events unless Inspector/developer
mode explicitly requests them.
The stream supports resume from an opaque cursor and sends a resync-required marker if
the cursor has expired or the client projection version is incompatible. On reconnect,
the client replaces stale projection state before applying later events; it does not
replay UI animations for historical changes.

`projection_types` may include `conversation_presentation` and `task_presentation` as
defined by [`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md). A presentation snapshot
establishes its projection revision/cursor before later updates are applied. Item identity
and source revision, not network arrival order, determine replacement and ordering.
After a valid subscription the server sends `stream.ready` with accepted projection version,
initial opaque cursor, and finite `max_frame_bytes`; projection updates and transient frames
are accepted only after this message. A resync-required marker invalidates the cursor and
requires a fresh snapshot.

While a Conversation turn is active, the stream may also deliver bounded transient
`turn.delta` frames containing `conversation_id`, `turn_id`, `retry_ordinal`, a monotonic
`sequence` within that retry, and coalesced text delta. The server advertises/enforces a
finite maximum frame size and applies backpressure; it does not emit one frame per model
token. These frames are not domain events, durable messages, or replicated content. A committed
`ConversationMessage` supersedes them. Failed turns do not promote partial deltas to a saved
answer; reconnect obtains the current projection and ignores duplicate, stale-sequence, or
already-settled turn frames. They never contain hidden reasoning, SecretStore bytes,
authorization material, or arbitrary executable UI payloads; standard output safety and
redaction policy still applies.
If the adapter cannot replay a sequence gap, the Operator discards the partial text and
waits for the committed message rather than concatenating across missing content.
Optional `rich.draft` frames use a separate bounded ephemeral frame schema and bind the
current `(turn_id, retry_ordinal, agent_session_id, draft_id)`. They are never
`projection_types`, DomainEvents, or persisted messages. If the Operator cannot replay a
draft gap, it discards the rich draft and continues with `turn.delta`/committed-message
handling. Rich compilation/publication runs after semantic-message commit and cannot hold
turn settlement or Task completion. Frame shapes are defined in
[`schemas/operator-stream.schema.json`](schemas/operator-stream.schema.json).

Workspace errors include WORKSPACE_ARCHIVED for writes to an archived Workspace and WORKSPACE_NOT_QUIESCENT when archive is requested while a Task is nonterminal or an Automation is enabled. A stale policy update returns STALE_WORKSPACE_VERSION. SELECTED_FOLDERS without one or more active same-Workspace root IDs returns INVALID_ARGUMENT; a missing, revoked, or unavailable selected root returns the corresponding typed resource error.

## Error envelope

```text
ApiError {
  code
  message
  retryable
  correlation_id
  details?
}
```

`ApiError` is the top-level JSON response body, not nested under an `error` property.
`X-Correlation-ID` repeats `correlation_id` for log and support workflows; an absent
`details` value is serialized as `null` by the local Operator.

Error codes are canonical in `schemas/error-codes.schema.json`; the Operator API returns
those codes without transport-specific renaming. Common responses include `NOT_FOUND`,
`WORKSPACE_ARCHIVED`, `WORKSPACE_NOT_QUIESCENT`, `STALE_VERSION`, `INVALID_TRANSITION`,
`APPROVAL_REQUIRED`, `FORBIDDEN`, and `DEPENDENCY_UNAVAILABLE`. Domain-specific commands
return their owning domain's typed codes.

`retryable` is advice for transport/operation retry, not permission to repeat an external
Effect. Effect-specific reconciliation rules always take precedence. `details` contains
only safe field errors, blockers, or version metadata.

## Coworker target API additions — NOT implemented routes

Proposed Workspace-authorized routes: list/create Coworker-owned Conversations with same-Workspace ownership; read last-active; list/assign/revoke Coworker capability assignments; get/update memory-learning policy and view filtered Resource-backed memory; create/review/enable/pause/resume StandingResponsibilities; read run history/health and actual activity projections. Every route must use the existing authenticated Operator boundary, exact request identity/idempotency semantics and cursor/version policy, not invent a second server. The concrete OpenAPI paths and components remain canonical; these route families are requirements pending specification, implementation and tests. No UI button may call a nonexistent route or claim success from a local mock.
