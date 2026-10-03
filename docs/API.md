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
  context field. Only Workspace list/create omit it. When a route or body also names a
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
Credential bytes never appear in request bodies. `SecretRef` values are opaque references,
not credentials. Runtime pairing returns a short-lived bearer token once in the
`PairingToken` response; it is not exposed by list/read APIs or written to logs/events.

## Workspaces

```text
GET  /v1/workspaces?cursor=&limit=
POST /v1/workspaces
GET  /v1/workspaces/{id}
PATCH /v1/workspaces/{id}
POST /v1/workspaces/{id}/archive
```

The selected Workspace header is mandatory for all three item operations above and must
match the path ID. This lets the same authenticated API surface reject mismatched scope
before invoking a Workspace command.

Create defaults to `LOCAL_ONLY`. A client may send another policy only after the user explicitly selects it; enabling cloud defaults the UI to `ACTIVE_TASK_INPUTS`. `SELECTED_FOLDERS` requires one or more revision-pinned `workspace-folder://` ResourceRefs. A replication-policy update applies to future transfers and never silently deletes content already replicated to another Runtime. Archive is accepted only after every Task is terminal and every Automation is disabled. Archived Workspaces remain readable and preserve existing authorized Artifact/Resource downloads, but reject all domain mutations, including Task mutations, capability activation/grants, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel work.

## Conversations

```text
POST /v1/conversations
GET  /v1/conversations/{id}
POST /v1/conversations/{id}/messages
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
POST /v1/tasks/{id}/cancel
POST /v1/tasks/{id}/retry
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

## Runtime/devices

```text
GET  /v1/runtimes?cursor=&limit=
POST /v1/runtimes/pair-token
POST /v1/runtimes/{id}/revoke
GET  /v1/runtimes/{id}/offers
```

The pairing-token request names the Workspace and allowed initial Runtime roles. The Hub
sets expiry under its policy and returns the opaque one-use token only in that response.
The token can create one Runtime identity and is invalidated after successful pairing.

## Agents

```text
GET  /v1/agent-profiles?runtime_id=&cursor=&limit=
GET  /v1/agent-bindings?runtime_id=&enabled=&cursor=&limit=
POST /v1/agent-bindings
GET  /v1/agent-bindings/{id}
POST /v1/agent-bindings/{id}/enable
POST /v1/agent-bindings/{id}/disable
```

Profiles are expiring Runtime inventory; bindings are durable Workspace records. Create
requires an observed profile and always creates a disabled binding. Agent setup/auth
flows remain adapter-owned; the API accepts only an opaque `SecretRef` and non-secret
configuration. Enable/disable are versioned and idempotent. Disabling blocks new
admission immediately while already admitted sessions remain pinned and settle safely.

## Connections and channel bindings

```text
GET   /v1/connections?status=&cursor=&limit=
GET   /v1/connections/{id}
POST  /v1/connections/{id}/disconnect

GET   /v1/channel-bindings?status=&cursor=&limit=
GET   /v1/channel-bindings/{id}
PATCH /v1/channel-bindings/{id}
POST  /v1/channel-bindings/{id}/revoke
```

These routes expose normalized connection/binding metadata and owner controls only.
Provider-owned setup establishes the authenticated account/identity and creates the
records; the Operator API does not invent an OAuth, device-code, webhook-secret, or
provider callback flow. `PATCH` changes only the binding's allowed actions, uses
`If-Match`, and cannot increase authority without TrustService approval. Disconnecting
a Connection or revoking a ChannelBinding blocks new use/inbound commands but preserves
history and does not delete external accounts or credentials. Provider-specific setup
and reauthentication contracts remain deferred with the relevant integration.

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
PUT  /v1/resources/uploads/{id}/content     # stream bytes, enforce size/type policy
POST /v1/resources/uploads/{id}/commit      # verify digest and return ResourceRef
```

Create supplies the declared full byte size, media type, and optional expected SHA-256.
The returned session is size-limited and expires. `PUT` streams the complete byte body
for that upload ID; Content-Length and Content-Type must match the declaration. It is a
single content transfer, not a range/chunk protocol. `POST .../commit` computes the
digest, checks the declaration and policy, then returns a revision-pinned ResourceRef.
Uncommitted or expired uploads cannot be referenced by a Task. Commit returns a
`blob://<sha256>` ResourceRef with its digest pinned. ArtifactVersion creation is a
separate ArtifactStore operation.

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

Server sends projection events, not raw domain events unless Inspector/developer mode explicitly requests them.
The stream supports resume from an opaque cursor and sends a resync-required marker if
the cursor has expired or the client projection version is incompatible. On reconnect,
the client replaces stale projection state before applying later events; it does not
replay UI animations for historical changes.

Workspace errors include WORKSPACE_ARCHIVED for writes to an archived Workspace and WORKSPACE_NOT_QUIESCENT when archive is requested while a Task is nonterminal or an Automation is enabled. A stale policy update returns STALE_WORKSPACE_VERSION. SELECTED_FOLDERS without revision-pinned folder refs returns INVALID_ARGUMENT.

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

Error codes are canonical in `SCHEMAS.md`; the Operator API returns those codes without transport-specific renaming. Common responses include `NOT_FOUND`, `WORKSPACE_ARCHIVED`, `WORKSPACE_NOT_QUIESCENT`, `STALE_VERSION`, `INVALID_TRANSITION`, `APPROVAL_REQUIRED`, `FORBIDDEN`, and `DEPENDENCY_UNAVAILABLE`. Domain-specific commands return their owning domain's typed codes.

`retryable` is advice for transport/operation retry, not permission to repeat an external
Effect. Effect-specific reconciliation rules always take precedence. `details` contains
only safe field errors, blockers, or version metadata.
