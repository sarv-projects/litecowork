# Operator API

This is a logical API contract. Transport binding may be local IPC for desktop and HTTP/WebSocket remotely.

The remote HTTP representation is described by `schemas/operator-api.openapi.yaml`.
Local IPC maps to the same commands and schemas; it is not allowed to call storage or
domain internals directly. Mesh, Agent, LitePSM, and Environment-provider methods are not
Operator API methods.

## Conventions

- IDs are opaque strings.
- every mutating HTTP call requires `Idempotency-Key`; local IPC uses the equivalent
  `request_id` field.
- mutable resources expose `version`; HTTP commands use `If-Match`, local IPC uses the
  equivalent `expected_version`.
- conflict returns `409/CONFLICT` semantics.
- authorization failures never reveal secret/resource existence beyond policy.
- every call is scoped to an authenticated Principal and Workspace; an opaque ID is not
  authority.
- command results include the committed aggregate version and correlation ID.
- pagination uses opaque cursors bound to the query/filter; cursors are not offsets.

Authentication is deployment-specific, but authorization is not: local OS identity or
remote credentials resolve to a Principal, then TrustService authorizes each command.
Credentials and secret bytes never appear in request bodies except via a dedicated
one-time secret entry flow outside this API.

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

## Approvals

```text
GET  /v1/approvals?status=pending
GET  /v1/approvals/{id}
POST /v1/approvals/{id}/resolve
```

Resolution body:

```text
{
  decision: APPROVE | DENY
  expected_version
  confirmation_digest?
}
```

## Library / artifacts

```text
GET  /v1/artifacts/{id}
GET  /v1/artifacts/{id}/versions/{version}
POST /v1/artifacts/{id}/promote
GET  /v1/library?cursor=
```

Blob transfer uses signed/authorized streaming endpoint rather than embedding binary in JSON.
The content endpoint checks Workspace membership, Artifact visibility, and current
authorization on every transfer. A signed URL, if used by a deployment, is short-lived
and scoped to one immutable blob digest.

## Automations

```text
POST /v1/automations
GET  /v1/automations
GET  /v1/automations/{id}
PATCH /v1/automations/{id}
POST /v1/automations/{id}/pause
POST /v1/automations/{id}/resume
POST /v1/automations/{id}/disable
GET  /v1/automations/{id}/occurrences
```

## Runtime/devices

```text
GET  /v1/runtimes
POST /v1/runtimes/pair-token
POST /v1/runtimes/{id}/revoke
GET  /v1/runtimes/{id}/offers
```

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

Uncommitted uploads are temporary and cannot be referenced by a Task. The commit
response is an immutable ResourceRef or ArtifactVersion reference after integrity
verification.

## Live event stream

```text
WS /v1/stream
```

Client subscribes:

```text
Subscribe {
  workspace_id
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

Canonical codes:

```text
NOT_FOUND
UNAUTHORIZED
FORBIDDEN
CONFLICT
STALE_VERSION
INVALID_ARGUMENT
INVALID_TRANSITION
APPROVAL_REQUIRED
RESOURCE_UNAVAILABLE
CAPABILITY_UNAVAILABLE
RUNTIME_UNAVAILABLE
RATE_LIMITED
TIMEOUT
INTERNAL
```

`retryable` is advice for transport/operation retry, not permission to repeat an external
Effect. Effect-specific reconciliation rules always take precedence. `details` contains
only safe field errors, blockers, or version metadata.
