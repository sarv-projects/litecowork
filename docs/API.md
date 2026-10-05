# Operator API

This is a logical API contract. Transport binding may be local IPC for desktop and HTTP/WebSocket remotely.

The remote HTTP command representation is described by
`schemas/operator-api.openapi.yaml`; the WebSocket stream contract is defined below.
Local IPC maps to the same commands and schemas; it is not allowed to call storage or
domain internals directly. Mesh, Agent, LitePSM, and Environment-provider methods are not
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
  context field. Only Workspace list/create and locally authenticated recovery bootstrap
  operations omit it. When a route or body also names a
  Workspace, it must match the selected context. The service resolves an opaque resource
  ID to its Workspace and checks owner authority; an ID is never authority by itself.
  Missing or mismatched scope is rejected as `FORBIDDEN` without revealing whether the
  named Workspace/resource exists.
- `GET`, `PATCH`, and `POST .../archive` on `/v1/workspaces/{id}` also carry
  `X-Workspace-ID`, and it must equal `{id}`. Workspace list/create are the only
  unscoped HTTP operations.
- command results include the committed aggregate version and correlation ID.
- pagination uses opaque cursors bound to the query/filter; cursors are not offsets.

Authentication is deployment-specific, but authorization is not: local OS identity or remote credentials resolve to a Principal, then TrustService authorizes each command. V1 Workspaces have one owner principal and no membership model; every Workspace-scoped call verifies that the authenticated Principal is that owner.
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

## Workspace-persistent Environments

```text
GET  /v1/workspaces/{workspace_id}/environments?cursor=&limit=
POST /v1/workspaces/{workspace_id}/environments/provision-preview
POST /v1/workspaces/{workspace_id}/environments
GET  /v1/workspaces/{workspace_id}/environments/{environment_id}
POST /v1/workspaces/{workspace_id}/environments/{environment_id}/suspend
POST /v1/workspaces/{workspace_id}/environments/{environment_id}/resume
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

Restore verifies the key and every required object before making the Workspace writable,
restores the exact snapshot/event history through its recorded cursors, rebuilds projections,
and creates a new Runtime identity. Missing secrets require reauthentication; secret bytes
and old execution leases are never restored.

The selected Workspace header is mandatory for Workspace item and settings operations
and must match the path ID. This lets the same authenticated API surface reject mismatched
scope before invoking a Workspace command.

Agent selection is explicit and Workspace-scoped. The user enables a discovered
AgentBinding, then sets it as the Workspace default. A Conversation may select an
enabled binding override; when absent, new Conversation turns use the Workspace default.
An invalid/disabled explicit override does not silently fall back. Changing or clearing
the default affects newly admitted turns and Tasks only; existing sessions and Attempts
remain pinned. Without an eligible binding, chat/Task admission returns
`AGENT_UNAVAILABLE` and creates no partial turn or Task. The composer retains the unsent
draft while first-use setup selects and enables an AgentBinding. Other resource/runtime
blockers after Task creation are represented by `Task.blocking_conditions[]`.

Create defaults to `LOCAL_ONLY`; `SELECTED_FOLDERS` is unavailable in the create request because roots are Workspace-owned records created afterward. A client may send another supported policy only after the user explicitly selects it; enabling cloud defaults the UI to `ACTIVE_TASK_INPUTS`. The user adds persistent roots, then updates policy with one or more active same-Workspace `replication_scope_root_ids`. Root IDs follow each selected folder's future observed revisions; they do not pin one content snapshot. Root-level replication settings intersect with Workspace policy and cannot broaden it. A replication-policy update applies to future transfers and never silently deletes content already replicated to another Runtime. Archive is accepted only after every Task is terminal and every Automation is disabled. Quiescence also requires Conversation turns and scoped Invocations to be settled, no active grants/SecretLeases/control leases, and persistent Environments with no live workload. Authorized watchers/triggers stop before the read-only transition. Retained Environment state may remain suspended under storage/backup policy; archive never silently destroys it. Unknown provider quiescence blocks archive with `WORKSPACE_NOT_QUIESCENT`. Archived Workspaces remain readable and preserve existing authorized Artifact/Resource downloads, but reject all domain mutations, including Task mutations, capability activation/grants, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel work.

## Conversations

```text
POST /v1/conversations
GET  /v1/conversations/{id}
POST /v1/conversations/{id}/messages
POST /v1/conversations/{id}/turns
POST /v1/conversations/{id}/turns/{turn_id}/retry
POST /v1/conversations/{id}/turns/{turn_id}/cancel
GET  /v1/conversations/{id}/messages?cursor=
GET  /v1/conversations/{id}/tasks?cursor=
```

## Tasks

```text
POST /v1/tasks
GET  /v1/tasks/{id}
GET  /v1/tasks?status=&cursor=
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
GET  /v1/tasks/{id}/plan-revisions
GET  /v1/tasks/{id}/steps
GET  /v1/tasks/{id}/attempts
GET  /v1/tasks/{id}/timeline?cursor=
GET  /v1/tasks/{id}/artifacts
GET  /v1/tasks/{id}/effects
```

The lead-agent command body is `{ agent_binding_id}`. The response is accepted asynchronously; current Attempts remain pinned while work drains, and a replacement execution Attempt waits for old lease settlement and Effect reconciliation.

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
POST /v1/artifacts/{id}/promote
POST /v1/artifacts/{id}/archive
GET  /v1/library?cursor=&limit=
```

Promotion/archive commands require `If-Match` with the current Artifact aggregate version. Promotion accepts TRANSIENT artifacts; archive transitions SAVED artifacts and returns the current representation for an already archived Artifact when If-Match names its current version, without emitting another event; stale If-Match returns STALE_VERSION. Neither changes the immutable content version. `STALE_VERSION`, `ARTIFACT_ARCHIVED` (for content publication after archive), and `INVALID_ARTIFACT_TRANSITION` are returned as applicable.

`GET /v1/library` returns SAVED artifacts; `GET /v1/artifacts?library_status=ARCHIVED` backs the Archived filter. Archived Artifacts remain readable but cannot receive new content versions.

`GET /v1/artifacts/{id}/versions/{version}/content` streams the immutable version bytes after rechecking Workspace authorization; the response Content-Type is the stored media type. A deployment may redirect through a short-lived URL scoped to that blob digest, but the logical resource and authorization check stay the same.
The content endpoint checks Workspace owner authorization, Artifact visibility, and current
authorization on every transfer. A signed URL, if used by a deployment, is short-lived
and scoped to one immutable blob digest.

## Automations

```text
POST /v1/routines
GET  /v1/routines?cursor=&limit=
GET  /v1/routines/{id}
GET  /v1/routines/{id}/revisions?cursor=
POST /v1/routines/{id}/revisions
POST /v1/routines/{id}/run
POST /v1/routines/{id}/archive

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

Trigger placement (`HUB`, `SPECIFIC_RUNTIME`, `AUTO`) is independent of Task execution
placement. `AUTO` resolves and persists one owner per trigger; changes require fenced cursor
handoff. Schedule and one-shot triggers require an explicit `MisfirePolicy` (`SKIP`,
`RUN_ONCE_WHEN_AVAILABLE`, or bounded catch-up). Wake attempts are optional and best-effort.

## Local Runtime lifecycle

```text
GET   /v1/runtime-lifecycle
PATCH /v1/runtime-lifecycle/startup-policy
GET   /v1/runtime-lifecycle/stop-preview
POST  /v1/runtime-lifecycle/stop
```

These are local-Operator operations authenticated by the local OS identity; they do not
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
LiteCowork scope references; it is not LitePSM's process-wide client count. The response
never exposes a LitePSM provider handle, process identity, credentials, or package-private
fields. Expired observations are returned as `UNKNOWN`/stale and cannot satisfy admission.

## Agents

```text
GET  /v1/agent-profiles?runtime_id=&cursor=&limit=
GET  /v1/agent-bindings?runtime_id=&enabled=&cursor=&limit=
POST /v1/agent-bindings
GET  /v1/agent-bindings/{id}
POST /v1/agent-bindings/{id}/enable
POST /v1/agent-bindings/{id}/disable
```

AgentProfile and AgentEndpoint are stable identities; the profile response joins them to
per-endpoint RuntimeOffer observations (Runtime/incarnation, readiness, compatibility,
and expiry) without exposing endpoint locators. Bindings are durable Workspace records.
Create requires an observed compatible endpoint and always creates a disabled binding. Agent setup/auth
flows remain adapter-owned; the API accepts only an opaque `SecretRef` and non-secret
configuration. Enable/disable are versioned and idempotent. Disabling blocks new
admission immediately while already admitted sessions remain pinned and settle safely.
Creation may pin an exact discovered endpoint or save required features and preferred
topologies for later compatible selection; protocol choice is not a universal ranking.

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
checks provider compatibility, secret placement, source-lease settlement/expiry and
clock-skew margin, then increments the host epoch. A continuity-safe move requires provider
cursor transfer or replay from the last Hub-replicated receipt. If that is unavailable, the
request must explicitly confirm `accept_ingress_gap`; the resulting assignment records
`GAP_ACCEPTED` and a timestamp shown in channel history. The API never returns the fencing
credential, cursor, or digest. A moved binding cannot use old Runtime-local reply references.

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

These are user-facing wrappers around LitePSM/CapabilityBroker; they do not expose package-manager internals by default.
They never invoke LitePSM from the client directly. The API implementation calls
CapabilityBroker; the detailed LitePSM wire contract remains deferred in
`CAPABILITY-FABRIC.md`.

## Resource intake

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

Create supplies the declared full byte size, media type, and optional expected SHA-256.
The returned session is size-limited and expires and includes a fixed chunk size. The
client uploads indexed chunks with `Content-Range`, per-chunk SHA-256, and idempotent
chunk identity. The server reports the committed offset/ranges; an identical retry is
accepted, while conflicting bytes for an existing range return `UPLOAD_OFFSET_CONFLICT`.
HTTP `Content-Range` end offsets are inclusive. Persisted `ResourceUploadChunk`
`end_offset_exclusive` and response `UploadRange.end_offset_inclusive` make the conversion
explicit. `chunk_index` determines the only accepted start offset; all non-final chunks
have the negotiated chunk size. `GET` returns the current session and derived ranges.
Commit verifies complete coverage, total size, media policy, and full-content digest,
then creates a stable Resource and committed ResourceRevision and returns a pinned
ResourceRef. Uncommitted/expired uploads cannot be referenced by a Task. ArtifactVersion
creation is a separate ArtifactStore operation.

## Workspace roots and deterministic resource search

```text
GET    /v1/resources/search?q=&kind=&freshness=&cursor=
GET    /v1/resources/{resource_id}
GET    /v1/resources/{resource_id}/locations
GET    /v1/resources/{resource_id}/revisions
POST   /v1/workspace-roots
GET    /v1/workspace-roots?status=&cursor=
PATCH  /v1/workspace-roots/{id}
POST   /v1/workspace-roots/{id}/pause
POST   /v1/workspace-roots/{id}/resume
POST   /v1/workspace-roots/{id}/revoke
```

Root creation requires an explicit user grant naming one selected folder Resource and
location. A one-time attachment does not create a WorkspaceRoot. `PATCH` updates only
watch/replication policy under expected-version checks. Pause, resume, and revoke are
separate versioned actions; Runtime offline status maps to `UNAVAILABLE`, not user pause.
Search is deterministic,
metadata/text-index based, consumes no model tokens, applies Resource and root grants,
and returns stable ResourceRefs with location and freshness metadata. A result is not an
implicit Task/Agent context attachment.

The revision endpoint returns immutable revisions in ancestry order with parent IDs and
head markers. If multiple heads exist, the Resource projection has a null
`current_revision_id`; clients must present a pinned ResourceRef or create a verified
merge before an unpinned reference can resolve. Choosing a branch is not a merge.

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

Error codes are canonical in `schemas/error-codes.schema.json`; the Operator API returns
those codes without transport-specific renaming. Common responses include `NOT_FOUND`,
`WORKSPACE_ARCHIVED`, `WORKSPACE_NOT_QUIESCENT`, `STALE_VERSION`, `INVALID_TRANSITION`,
`APPROVAL_REQUIRED`, `FORBIDDEN`, and `DEPENDENCY_UNAVAILABLE`. Domain-specific commands
return their owning domain's typed codes.

`retryable` is advice for transport/operation retry, not permission to repeat an external
Effect. Effect-specific reconciliation rules always take precedence. `details` contains
only safe field errors, blockers, or version metadata.
