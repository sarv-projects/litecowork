# Local Operator IPC

This document defines the authenticated desktop transport between the native Tauri
process and the local `litecoworkd` Runtime. It is a transport adapter for the logical
Operator operations in `API.md`; it does not change their authorization, idempotency,
Workspace scoping, or response semantics.

## Implementation status (2026-10-10)

The daemon and Tauri source share bounded framed IPC dispatch on Linux/macOS and Windows.
Unix continues to compare kernel peer credentials with the validated installation
binding. Windows now has a local named-pipe adapter whose protected DACL grants the current
logon SID and SYSTEM, rejects remote clients, and is installed before frame parsing; the
Runtime persists a keyed logon-SID binding. Both platform adapters feed the same Axum
handlers, authorization middleware, response allowlist, body budget, and bounded drain.
The Tauri bearer/loopback descriptor client has been removed. Windows support is present
in source and cross-compiles for `operator-ipc` and `windows-local-security`. The daemon's
Windows-target Rust source also passes `cargo check` and Clippy with warnings denied when
`libsqlite3-sys` is directed to host pkg-config metadata; this type-checks the Windows Rust
configuration but does not build or link Windows SQLite. The normal bundled-SQLite daemon
cross-check stops because MinGW GCC is unavailable, and the Tauri cross-check stops in
`tauri-build` because MinGW `windres` is unavailable. No Windows host
has exercised pipe ACLs, connect/disconnect behavior, or the desktop-to-daemon path. These
results establish only partial CODE evidence; they do not establish packaged or OS
qualification. Linux/macOS provider behavior and real desktop-to-daemon qualification
remain release requirements; this is not a production-safety claim.

The focused auth-boundary test also exercises same-process Unix socket dispatch through
`dispatch_ipc_exchange` into an auth-protected readiness handler. Separate cases verify
that the middleware rejects a request without its internal authenticated-peer marker and
that IPC frame validation rejects a caller-supplied bearer header. These are code-level
tests only; they do not test a second OS identity, a packaged desktop/daemon pair, or
cross-platform peer authentication.

## Goals and limits

- Authenticate a local caller through operating-system IPC controls before accepting an
  Operator request body.
- Remove the per-incarnation bearer token and TCP port descriptor from the desktop path.
- Keep native credentials, paths, and IPC handles out of WebView JavaScript.
- Reuse the same Operator application handlers and domain authorization as other
  transports.
- Fail closed when the platform identity or pipe/socket protection cannot be established.
- Keep transport connection identity separate from Workspace grants and Task authority.

This authenticates a process running as the authorized OS user principal on Unix-like
systems, and as the authorized logon-session principal on Windows. Unix peer credentials
normally expose a UID, so separate sessions that share the same UID are treated as the
same OS user; this transport does not promise per-session isolation there. Windows uses a
logon-SID-restricted pipe ACL to distinguish sessions. Neither platform distinction
protects against malicious code already running with the authorized principal.

## High-level shape

```text
Tauri native command
        │
        │ bounded local IPC request
        ▼
OS-authenticated local socket / named pipe
        │
        │ authenticated peer principal attached internally
        ▼
Operator transport adapter
        │
        ▼
existing Operator route/application handlers
        │
        ├── Workspace-owner checks
        ├── RequestId / If-Match checks
        └── Resource and storage operations
```

The WebView cannot choose an IPC endpoint, add arbitrary headers, or read native files.
Tauri derives the endpoint from its trusted application-data directory using the shared
transport library. The daemon independently derives the same endpoint from its
`--data-dir`.

## Platform authentication

### Linux and macOS

- Use a local Unix-domain socket in a verified private Runtime directory, or an
  OS-namespaced local socket when the platform mapping and lifecycle are qualified.
- Socket filesystem permissions are owner-only (`0600`); its parent directory is
  owner-only (`0700`). Reject symlinks and non-socket stale paths. Remove a stale socket
  only after checking its type and owner.
- On accept, obtain peer credentials from the connected socket and compare the peer UID
  with the validated installation binding's UID. The binding is fingerprinted with a
  random key in the platform credential store (Linux Secret Service/keyring backend;
  macOS Keychain). Missing credentials
  or a mismatch rejects the connection before frame parsing. A second session using the
  same UID is within the same OS-user trust boundary; per-session isolation is not
  promised by this Unix transport contract.
- macOS sandbox/security-scoped Resource access is a separate grant lifecycle; socket
  authentication does not confer filesystem authority.

### Windows

- Use a local named pipe with remote clients rejected.
- Create a protected DACL granting access only to the current logon SID and SYSTEM. A
  user-account SID alone is insufficient because a second logon session for the same
  account must not inherit this Runtime's endpoint.
- Derive and persist the Runtime's OS logon identity through a narrowly isolated,
  reviewed Windows identity adapter. `interprocess` exposes a peer PID on Windows but
  PID lookup is race-prone and is not an authorization check.
- The workspace currently forbids unsafe Rust globally. If Windows token APIs require
  unsafe calls, place them in one small platform-security crate with audited unsafe
  blocks and change the workspace lint from `forbid` to `deny`, so other crates remain
  unable to opt in accidentally. Never scatter `allow(unsafe_code)` through daemon or
  UI modules.
- If the per-logon DACL cannot be created, do not start an unauthenticated pipe or fall
  back to loopback bearer auth.

## Runtime identity binding

`local_principal_id` is a durable application identity, not an OS identity. Add a separate
installation-local record containing:

```text
RuntimeOsPrincipalBinding {
    runtime_id
    platform
    principal_fingerprint
    binding_version
}
```

`principal_fingerprint` is a keyed or OS-protected digest of the stable OS principal
material; do not store raw Windows tokens, logon SID bytes, or other credential material
in ordinary Runtime state. The binding is created at first trusted startup and must be
validated on every IPC server start. A changed OS principal requires explicit local
re-pair/recovery; it must not silently adopt a new owner. The current Linux/macOS source
uses an installation-scoped OS-keyring account and keyed fingerprint; credential-provider
behavior remains a qualification dependency and must be recorded in the OS test matrix
before this binding is treated as production-ready.

## Endpoint naming and lifecycle

- Endpoint identity is deterministic for one private application-data directory and
  independent of the Runtime incarnation. Use a domain-separated hash; do not embed the
  full path, Workspace name, or user-provided text in the socket/pipe name.
- Endpoint naming, normalization, and platform namespace mapping live in one shared
  `operator-ipc` library used by both daemon and Tauri.
- At startup, the daemon acquires the existing single-instance lock before creating the
  endpoint. A second daemon must fail without replacing or unlinking the active endpoint.
- Readiness is an authenticated IPC operation that returns Runtime ID, local incarnation
  ID, Operator contract version, and serving state. The desktop verifies these against
  native Runtime status before normal requests.
- On graceful shutdown, stop admission, finish or reject bounded in-flight requests,
  close the listener, and remove only the endpoint instance created by this process. The
  Runtime may persist a clean stop only after all admitted request tasks complete without
  an ambiguous handler timeout and the endpoint thread exits successfully; a failed or
  expired drain leaves the Runtime unclean.
- Runtime supervises the endpoint thread after readiness. Unexpected listener/thread exit
  signals active streams, settles the bounded request drain, and causes a nonzero Runtime
  exit. It never turns endpoint failure into a clean stop; startup failures leave the
  already-committed incarnation `DEGRADED`, while failures during graceful shutdown retain
  the current unclean lifecycle state.
- On crash recovery, inspect endpoint ownership and type before stale cleanup. Never
  unlink an active endpoint based only on a stale status file.

## Versioned frame protocol

The initial protocol is request/response, one exchange per connection. It preserves the
logical HTTP operation shape internally to avoid changing existing route semantics, but
the transport is not HTTP and has no Host, Origin, cookie, or bearer headers.

```text
Request:
  u32-be header_length
  JSON RequestHeader
  exactly body_length raw bytes

Response:
  u32-be header_length
  JSON ResponseHeader
  exactly body_length raw bytes
```

`RequestHeader`:

```text
protocol_version: u16
request_id: bounded opaque correlation ID
method: allowlisted HTTP method
path_and_query: relative Operator path only
headers: allowlisted logical headers
body_length: u64
```

`ResponseHeader`:

```text
protocol_version: u16
request_id: same request ID
status: u16
headers: allowlisted response headers
body_length: u64
```

The frame `request_id` identifies one transport exchange and is echoed only to correlate
its response. A reconnect retry uses a new frame `request_id`. Mutating Operator commands
carry the separate logical `Idempotency-Key` header; the caller preserves that same key
and payload across retries so the durable command receipt can resolve an ambiguous prior
commit. The adapter never derives the idempotency key from the frame correlation ID.

Initial limits:

```text
header JSON: 16 KiB maximum
request body: 100 MiB maximum
response body: 10 MiB maximum in the current Operator adapter
concurrent accepted requests: 8 maximum per Runtime
combined in-flight request and response bodies: 128 MiB maximum per Runtime
connect + header-read deadline: 3 seconds
body-read deadline: 30 seconds, refreshed only by bounded progress
handler deadline: existing per-operation limit, at most 60 seconds
```

The shared framing crate exposes `InFlightBodyBudget`; each Runtime listener/client must
create one shared 128 MiB budget and retain each frame's RAII reservation for as long as
its body is retained or written. Inbound reads reserve before allocation. Constructed
outbound frames reserve when admitted, after their caller has produced the body Vec, so
the caller must enforce operation-specific allocation limits before producing large bodies.
The current daemon IPC adapter reserves up to 10 MiB of response capacity before reading
or dispatching each admitted request, then caps the collected response at that size.
Resource-content storage also checks indexed size before blob decryption, so large Resources
are not materialized only to be rejected by the transport. The listener separately enforces
the eight-connection admission limit and the deadlines above;
the framing crate alone does not enforce connection counts or timeouts. Any failed
or cancelled read or write leaves the one-exchange stream unusable; close it and retry a
mutation only with the original `Idempotency-Key`.
Lengths are checked before allocation. Reject unsupported protocol versions, absolute
URLs, path traversal, duplicate/forbidden headers, mismatched request IDs, short frames,
and bodies exceeding the operation's own stricter limit. Each unary connection carries
exactly one request/response exchange; bytes after the declared body are never interpreted
as another request, and the server closes the connection after its response. A failed or
ambiguous request is returned as a transport error; the client retries mutations only
with the original `Idempotency-Key`.

Logical headers initially permitted are `X-Workspace-ID`, `Idempotency-Key`, `If-Match`,
`Content-Type`, `Content-Range`, and `X-Chunk-SHA256`. Pagination and search values are
carried only in `path_and_query`, not promoted into additional headers.
`Authorization`, `Host`, `Origin`, cookies, proxy headers, and hop-by-hop headers are
forbidden on IPC. Response headers are independently allowlisted as `Content-Type`,
`Content-Length`, `Cache-Control`, `ETag`, `X-Content-Type-Options`,
`X-Resource-Media-Type`, `X-Resource-Revision-Id`, and `X-Correlation-ID`. The exact
Resource revision header is required by the desktop's pinned content preview and must
survive the frame boundary; arbitrary response headers remain filtered and rejected.

### Authenticated local stream session

The logical `WS /v1/stream` subscription maps to a dedicated long-lived local session;
ordinary one-request/one-response connections keep the protocol above unchanged. The
desktop opens the same OS-authenticated Unix socket or named pipe and sends one bounded
`RequestFrame` with `POST /v1/stream`, the selected `X-Workspace-ID`, and the JSON
`Subscribe` body. The daemon authenticates the OS peer before reading the body and applies
the same Operator middleware and Workspace-owner authorization. `Subscribe.workspace_id`
must equal the selected Workspace header. The subscription body is limited to 32 KiB;
this accommodates a bounded 14 KiB multi-Conversation resume cursor with up to 20 message
frontiers while keeping the subscription small relative to ordinary Operator requests.

After the route accepts the subscription, the daemon sends one correlated `ResponseFrame`
with status 200, an empty body, and content type
`application/vnd.litecowork.operator-stream+json`. It then writes each
`operator-stream.schema.json` object as a separate `u32-be length` followed by exactly
that many UTF-8 JSON bytes. The client reads one frame at a time; the server never
concatenates an unbounded stream in memory. Each JSON message is at most 1 MiB. EOF ends
the session. A subscription starts with `stream.ready`, followed by an authorized
projection snapshot and then changes committed after that snapshot's captured frontier.
The initial snapshot is the first bounded message page; later committed messages arrive as
single-message Conversation upserts. The opaque cursor binds the durable event sequence and
private message frontier, so reconnect resumes missed message upserts even when catch-up
spans multiple 100-message pages. Manual history pagination keeps its own forward cursor.
Reconnect sends the last fully applied opaque cursor; the fresh snapshot replaces stale
client state before later updates are applied. If the cursor binding or retained history
is invalid, the server sends `stream.resync_required` and a fresh snapshot, then catches up
from its last safe message frontier. Domain event payloads are never sent on this feed.

Long-lived sessions share OS peer authentication, the existing eight-connection cap and
the Runtime body budget, with a separate limit of two simultaneous stream sessions so
they cannot occupy every Operator connection. Each write has a five-second deadline; a
slow consumer is disconnected when that deadline expires. The client sends exactly one
request frame and keeps the duplex connection open while consuming response frames; it
sends no further request-side bytes and does not half-close the connection to cancel. While
waiting for the next projection frame, the daemon concurrently monitors the request side:
EOF or a transport reset ends the stream and releases its session slot, while any byte after
the subscription is rejected as an invalid exchange. Thus an idle disconnected client is
detected without an application heartbeat frame. The stream client never retries a mutation;
it cancels by closing the duplex connection and reconnects with a fresh transport request
ID and its last completely applied stream cursor. Shutdown cancels stream sessions before
joining request workers, so a disconnected subscriber cannot hold Runtime shutdown open.

The native Tauri bridge carries at most one IPC frame that has not yet been reduced by the
WebView. Each emitted frame has a session-scoped `frameId`; after validating and applying
or safely discarding that frame, the UI acknowledges the exact ID through a native command.
The native reader does not read another IPC frame until this acknowledgement arrives. A
stale, duplicate, or foreign acknowledgement is rejected and cannot release another
frame's credit. Unsubscribe cancels an acknowledgement wait and drops the duplex
connection; an unacknowledged frame also closes the stream after five seconds. This
one-frame bridge bound lets a stalled WebView apply pressure back to the daemon's bounded
IPC writer rather than queueing an unbounded sequence of Tauri events. The acknowledgement
is local Tauri flow control and does not change the Operator stream wire schema.

Artifact text publication uses the ordinary authenticated Operator route
`POST /v1/artifacts/{artifact_id}/text-version`; its expected content and Resource heads
are carried in the strict bounded JSON body, while `If-Match` and `Idempotency-Key` use
the allowlisted logical headers above. The Tauri bridge exposes only this finite command
and the corresponding edit-head read; it does not accept arbitrary URLs, methods, or
headers. The daemon verifies Workspace ownership again at blob staging and rechecks the
exact heads during atomic publication.

### Native persistent-folder selection (source-integrated, unqualified)

The Tauri `add_workspace_root` command opens the operating-system folder chooser itself;
the WebView supplies no path. After selection, the native Tauri process sends the selected
path bytes as URL-safe base64 only over the authenticated local IPC operation
`POST /__litecowork_local/workspace-root-selection`. This reserved path is not part of the
public Operator/OpenAPI API and is rejected by the shared frame path allowlist on other
transports. The daemon treats the path as untrusted, verifies Workspace owner/status and
expected version, opens the folder through directory handles with no-follow flags, derives
a keyed file-identity projection, and commits the Resource, location, private locator,
file-identity binding, WorkspaceRoot, events and idempotency receipt atomically. The
response and persisted events omit the absolute path and raw operating-system identity.

The current IPC credential proves the OS user principal, not the Tauri binary's code
identity. A different process already running as that principal can call the local IPC
endpoint directly; the native chooser is the supported product path, not a cryptographic
proof of a picker interaction. This source path currently targets the existing Linux/macOS
Unix transport and fails closed elsewhere. It has not been built, tested, or OS-qualified.
The current source can create/list roots and revoke a root grant; revocation deletes local
locator and raw file-identity bindings and removes the root from future replication
selection. It does not delete copies already transferred to another Runtime. Pause/resume,
restart identity revalidation, directory watching, content indexing, and RAG remain
unimplemented. The UI must describe this honestly and must not present a saved root as
indexed or currently observed content.

## Server dispatch

After OS-peer authentication and bounded frame validation, the transport adapter converts
the request into the existing Operator request type and attaches a process-local,
unforgeable `AuthenticatedLocalPeer` extension. The Operator middleware accepts that
extension only when installed by the IPC adapter. The current desktop daemon does not
expose a TCP/HTTP Operator listener; a future remote Operator transport must use a separate
explicit authentication path and cannot create this extension.

Do not duplicate Workspace-owner checks, idempotency handling, Resource access, or
authorization in the IPC adapter. Those remain in the shared Operator application path.
The adapter must not interpret socket authentication as approval for consequential
Effects or as a Workspace grant.

## Client behavior

- The Tauri native layer creates a fresh IPC connection for each request initially. This
  keeps framing, cancellation, and recovery simple while the API is local and bounded.
- The Tauri readiness operation applies a 400 ms overall IPC exchange deadline without
  changing the wire protocol's general request/response limits. Startup probes further
  clamp that deadline to the remaining five-second startup budget.
- Reconnection always performs authenticated readiness first when the daemon incarnation
  changes. A request is never silently replayed under a new incarnation unless it is
  idempotent and carries the original request key.
- Tauri maps transport errors to safe user-facing messages. It does not expose raw OS
  paths, socket names, pipe names, frame bytes, or peer credentials to the WebView.
- No transparent HTTP/bearer fallback is allowed.

## Failure behavior

| Condition | Behavior |
|---|---|
| Peer identity unavailable or mismatched | Reject before reading request body; audit a bounded reason code |
| Unsafe/missing private directory | Do not create endpoint; Runtime reports Operator unavailable |
| Endpoint already active | Preserve incumbent endpoint; new daemon fails startup |
| Stale endpoint with wrong type/owner | Do not unlink; require recovery action |
| Malformed/oversized frame | Close connection; do not dispatch a route |
| Client disconnect before dispatch | Discard incomplete frame |
| Client disconnect during mutation | Return ambiguous transport result; caller reconciles by original idempotency key |
| Daemon restarts during request | Report unavailable/ambiguous; re-handshake with new incarnation |
| Windows logon SID/DACL setup fails | Fail closed; no pipe and no fallback |

## Qualification gates

Before shipping the source-integrated transport in a release build, verify on supported Linux,
macOS, and Windows versions:

- another OS user cannot connect on Unix-like systems or Windows;
- a second Unix session using the same UID is treated as the same OS user, while a second
  Windows logon session for the same account cannot connect;
- endpoint replacement/symlink attacks do not redirect a client;
- restart and stale-endpoint cleanup preserve the active daemon;
- disconnects, partial headers/bodies, oversized lengths, and concurrent clients fail
  safely;
- a mutation response lost after commit can be reconciled with the same idempotency key;
- Tauri never exposes endpoint or OS identity material to WebView code;
- folder grants use the separate native picker/bookmark/handle flow and cannot be
  inferred from IPC peer authentication.

Until those gates pass, the source-integrated Unix IPC path is not production-qualified.
There is no desktop loopback HTTP/bearer fallback. Unsupported platforms remain unavailable
until their authenticated transport is implemented and qualified.


## Confidential agent credential submission

An adapter-declared LITECOWORK_SECRET_STORE credential slot is one of the few Operator
operations whose request body may contain secret bytes. It therefore has a stricter local
transport profile than ordinary JSON configuration:

1. only the authenticated local desktop principal may invoke it;
2. the WebView receives no SecretRef target metadata or stored secret value;
3. the native Tauri command accepts a bounded one-shot secret buffer and forwards it only
   to the local authenticated Operator;
4. request/response body logging, debug snapshots, retries by generic query middleware,
   persistence, crash breadcrumbs and analytics are disabled;
5. daemon validates the current AgentControlDescriptor, slot ID, storage owner and secret
   kind before SecretStore write;
6. the input buffer is zeroized/released as soon as the SecretStore operation settles;
7. the response contains only configured status / AgentCredentialBinding SecretRef;
8. remote Operator transports do not inherit this operation by default.

Native-agent-owned sign-in/API keys do not use this route; LiteCowork launches/surfaces the
adapter-declared native flow instead.

Windows named-pipe support must demonstrate the same no-fallback peer-authentication and
secret-buffer behavior before this path is enabled there.
