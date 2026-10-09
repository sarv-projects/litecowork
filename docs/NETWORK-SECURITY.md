# Network, IPC, and Resource Security

## Security boundary

Untrusted Agents, MCP servers, Apps, document content, browser pages, and Environments
must not control the authority of the LiteCowork Runtime. Network and filesystem policy
is enforced by a broker outside the worker sandbox wherever the platform allows. A
container's local firewall configuration alone is not a sufficient egress boundary.

## Egress and SSRF

All sandbox egress is deny-by-default. A Task or provider requests an explicit
destination/protocol exception; TrustService authorizes the scope and the external egress
broker enforces it. The broker applies these rules to every request:

1. Parse and normalize URL scheme, host, port, and user-info; reject unsupported schemes,
   embedded credentials, malformed hosts, and disallowed ports.
2. Resolve all A and AAAA answers using the configured resolver and normalize IPv4-mapped
   IPv6 and scoped IPv6 literals before policy evaluation. Reject loopback,
   unspecified, private, link-local, multicast, reserved, unique-local IPv6, and
   cloud-metadata destinations for worker egress. A Task-level grant never creates a
   general network exception to these address classes.
3. Pin an allowed resolved address for the connection. Re-resolve and revalidate on
   connection retry and every redirect; cap redirect count and reject a redirect that
   crosses an unauthorized origin or address class.
4. Prevent DNS rebinding by connecting only to a validated pinned address while retaining
   the original host for TLS SNI and certificate validation. Re-check the address when
   DNS TTL expires or a new connection is made.
5. Force traffic through the broker; remove or override worker proxy variables and block
   direct socket egress, alternate DNS, QUIC, and proxy bypass paths unless explicitly
   allowed by policy.
6. Enforce connection/read timeouts, response-header limits, compressed and decompressed
   body limits, download quotas, MIME policy, and per-Task/provider rate limits.
7. Require normal TLS certificate and hostname validation. No automatic downgrade or
   arbitrary certificate bypass is permitted.

Local files and local provider endpoints are accessed through a scoped ResourceResolver
or CapabilityBroker that performs the operation inside the trusted Runtime. They are
never exposed by allowing a sandboxed or internet-facing worker to reach loopback, private
network ranges, cloud metadata, or an arbitrary internal host. This default still permits
user-authorized access to a specific local intranet service: the Agent receives a typed
ResourceRef/capability, while the on-device broker makes the network request outside the
sandbox. The grant pins the registered service identity and exact scheme, host/address,
port, protocol, and operation scope; arbitrary Agent-supplied URLs, CIDR-wide access, and
redirects to unapproved destinations are rejected. DNS answers are pinned and revalidated
on reconnect/redirect. Loopback is available only to a registered local provider endpoint
with an exact service binding and authenticated local transport. Cloud metadata, unspecified,
multicast, broadcast, and unregistered link-local targets remain denied even when a Task
has a network grant. Cloud Environments never inherit a local Runtime's intranet grant.

## Local Operator IPC

Localhost TCP is not an identity boundary. The preferred local transport is OS IPC with
peer authentication and restrictive endpoint ACLs (for example Unix peer credentials or
Windows named-pipe ACL/client identity). If loopback HTTP is used, require both authenticated
OS peer identity where available and a high-entropy, short-lived client credential; bind
only to loopback and reject browser-origin requests/CSRF.

The normative desktop transport contract is [`LOCAL-OPERATOR-IPC.md`](LOCAL-OPERATOR-IPC.md).
It specifies per-logon Windows pipe ACLs, Unix peer-credential checks, a bounded framed
protocol, endpoint naming/lifecycle, Runtime-to-OS-principal binding, and qualification
gates. The local IPC adapter may attach an authenticated peer marker only after the OS
identity check; that marker is process-local and cannot be supplied by a remote request.

Every mutating request carries RequestId, correlation ID, selected Workspace scope, and
expected aggregate version where applicable. Remote Operator access uses authenticated
TLS and the same Principal/policy checks.

The current desktop source path uses owner-only Unix IPC plus kernel-reported peer UID
checks on Linux/macOS. The Runtime validates an installation-scoped OS-principal binding
before starting the endpoint, and Tauri authenticates the daemon peer before sending a
frame. The Operator application handlers remain the authorization boundary. Windows has
no named-pipe implementation and fails closed. This source integration is not yet
build-, system-, or OS-qualified; do not describe it as production-ready until the
qualification gates in [`LOCAL-OPERATOR-IPC.md`](LOCAL-OPERATOR-IPC.md) pass. Readiness
confirms only Operator API serving for the reported local incarnation, not Task execution
readiness or Mesh registration. No loopback HTTP/bearer fallback is present in the desktop
Tauri client or daemon Operator listener.

## LiteCowork Capability Gateway authentication

The Gateway authenticates each AgentSession with a short-lived, unguessable session
credential delivered over the authenticated AgentAdapter control channel or inherited
private descriptor, never as ordinary prompt text. The token is bound to:

- AgentSession ID and Runtime ID;
- the session scope (`CONVERSATION`, `TASK_PLANNING`, or `ATTEMPT_EXECUTION`);
- Workspace and optional Task/Attempt IDs;
- an explicit method allowlist and grant references;
- expiry and revocation epoch.

The Gateway rechecks the authenticated peer/session binding and TrustService on every
call. `CONVERSATION` allows only explicitly granted read-only invocation methods;
`TASK_PLANNING` allows task read, plan proposal, permitted clarification, and progressive
search/description; `ATTEMPT_EXECUTION` additionally requires active Attempt, Environment,
lease/fence, and scoped grants. `task.finish`, `user.ask`, `capabilities.invoke`, and
artifact publication each have distinct scope/permission checks. Expiry, session close,
Task pause/cancel, grant revocation, Runtime revocation, or policy change invalidates the
credential. A random process on the same machine cannot call the Gateway merely because
it can reach localhost.

A compromised Agent can still misuse authority intentionally granted to its authenticated
session; scopes, short lifetimes, operation allowlists, rate limits, Effect recording,
and audit bound that risk but do not make an authorized worker trustworthy.

## Rich response content and media

RichPresentation is untrusted structured input until the host validates its closed schema,
source bindings, Workspace scope, byte/count/depth limits, and semantic-message digest.
Renderers use text nodes and an allowlisted Markdown subset; raw HTML, scriptable SVG,
inline event handlers, CSS, executable components, and arbitrary React/JavaScript are never
accepted. Mermaid/diagram source is parsed with bounded node/edge/depth limits and rendered
in a non-scriptable sandbox or converted to a static host graph. Unknown block versions
fall back to semantic text.

Model-authored media URLs are never fetched by the desktop renderer. Media must resolve to
an authorized, digest-pinned Resource revision through the ResourceResolver. This prevents
tracking requests, local-network probes, and source substitution. External HTTPS navigation
is a user-triggered Operator action with origin display and normal network policy; it is
not an embedded fetch. A media Resource revoked or unavailable after the document was
published is reauthorized at read time and shown as unavailable if access no longer holds.
MCP App blocks use the existing isolated App host and Capability/Grant boundary; they do
not receive the desktop DOM, filesystem, local IPC, or SecretStore access.

## Provider-requested user input and external sign-in

Ordinary `UserRequest` forms are non-sensitive inputs. They are immutable Workspace data
and can replicate according to Workspace policy. Agents and MCP providers cannot use
`user.ask` or form elicitation to collect credentials. Credentials belong to a
provider-owned authentication/Connection flow or a SecretStore-backed secure entry
surface. No secret is accepted by serializing it into the ordinary UserRequest response.

For the negotiated MCP `2026-07-28` contract, `elicitation/create` form mode is limited to
non-sensitive values. LiteCowork validates the bounded schema and rejects suspicious
credential-like field names, descriptions, or secret/password formats. URL mode is the
out-of-band path: the URL and original provider input remain in the encrypted local
provider binding; the shared UserRequest exposes only a host-authored summary and
interaction mode. The raw URL is fetched from the source Runtime only after explicit
Operator authentication/action, sent with `Cache-Control: no-store`, and omitted from
logs, events, projections, analytics, crash reports, and backups. LiteCowork accepts only
HTTPS URLs without userinfo or IP-literal hosts, shows the parsed destination origin and
capability publisher, and opens the link in the system browser only after a user click.
The handoff is not a Runtime egress request: LiteCowork does not fetch the URL, embed the
page, automate form entry, or control the system browser's DNS resolution and redirects.
Standard browser security and the user's browser/network policy apply. The UI makes clear
that the external site receives anything entered there. The user's credentials go
directly to that provider site; the later MCP response contains only `accept`, `decline`,
or `cancel`.

These checks cannot infer every secret from arbitrary natural language. UI copy must say
not to enter credentials in an ordinary answer, and an input that the provider declares
or strongly indicates is sensitive fails closed. Unknown MCP embedded request methods
(including sampling requests) are unsupported; they are never silently converted into a
UserRequest or fulfilled by an Agent/model. This preserves the protocol's distinct trust
and disclosure semantics.

ExecutionLease and EnvironmentControlLease fencing credentials are distinct from Agent
session credentials. Their raw HMAC-derived values remain in issuer/provider process-private
memory and are sent only over authenticated control transport to an enforcing provider.
The versioned issuer key is held in an OS keystore/HSM, never in SQLite or Workspace
backup; loss of an issuer key fails closed by reconciling and ending all leases that require it.
Durable lease records store `fencing_token_digest`; the raw value is excluded from event
payloads, aggregate-state blobs, Operator projections, logs, and Workspace backups. Every
fenced operation also authenticates its caller and checks lease ID, Runtime incarnation,
Attempt, epoch, expiry, and current ownership, so a copied credential is not sufficient
authority. A new daemon incarnation discards old credentials and cannot renew its prior
incarnation's lease.

## Filesystem and archive handling

WorkspaceRoots are the only persistent indexing roots. Use handle-relative no-follow
traversal and stable file identity checks where supported; revalidate identity/revision
immediately before mutation and use atomic replace. Reject path traversal, symlink escapes,
device nodes, unexpected hard links, and race-swapped targets. If these checks are
unavailable, use an isolated private copy or read-only access.

Raw filesystem/volume/file identifiers and absolute locators remain in source-Runtime
bindings. Replicated FileIdentity values are keyed pseudonyms generated with a Runtime key
held in the OS keystore; that key is not part of Workspace backup. Losing it invalidates
cross-restart identity confidence and requires bounded re-indexing rather than content-only
deduplication.

At local Runtime startup, each persisted non-revoked WorkspaceRoot is reopened from its
prior private locator with no-follow directory-handle traversal before Operator IPC is
started. The held directory's raw identity must equal its prior local binding, and its
keyed Resource identity digest/projection must still match. Exact matches receive current-
incarnation private bindings atomically with an AVAILABLE ResourceLocation. Missing,
changed, mismatched or unsupported identities receive no replacement binding and make the
location UNAVAILABLE; an ACTIVE root becomes UNAVAILABLE while a PAUSED root remains
PAUSED. Exact later recovery can reactivate only a previously UNAVAILABLE root and never
resumes a PAUSED root. No path substitution or broader-root fallback is permitted. These
failures are root-local and do not block unrelated Runtime services. Current source support
is Linux/macOS only and is not OS-qualified. The separate
OS-principal credential validation occurs before storage open; if that key is unavailable,
startup fails closed before root revalidation, so this specific key-loss case does not yet
persist UNAVAILABLE root transitions.

File uploads and extracted archives have compressed size, expanded size, entry count,
nesting depth, path normalization, and processing-time limits. Reject absolute paths,
`..` traversal, duplicate/conflicting paths, symlink escapes, and archive bombs. Treat
MIME types and file extensions as hints; inspect bounded content before rendering or
dispatching to a parser. Renderers and text extractors run in isolated workers without
ambient network or filesystem authority and have CPU, memory, time, and output-size
budgets.

## MCP Apps

MCP Apps run only in an isolated iframe. The default sandbox permits scripts for the
AppBridge and omits `allow-same-origin`, forms, popups, downloads, and top navigation.
The host checks declared CSP `connectDomains`/`resourceDomains` against the host's network
policy and applies `default-src 'none'` with only the exact declared domains when they are
present. If declarations are absent or invalid, all external connections/resources are
blocked; the host never invents fallback domains. An App receives only the structured data and session capabilities
required for its view. It has no host DOM, cookies, storage, native IPC, local files,
SecretStore, or ungranted tools; external navigation goes through a host confirmation.
All tool/resource requests are mediated and reauthorized at the host.
App origin, server identity, resource digest, package version, grants, and Task/Workspace
scope are visible in Inspector and retained with invocation provenance.

## Threat coverage

This contract supplements `SECURITY.md` and `TRUST.md`. It covers DNS rebinding, redirects,
IPv4/IPv6 private-address access, metadata endpoints, proxy bypass, oversized responses,
malicious MIME/archive content, local IPC impersonation, Gateway token replay, and
filesystem symlink/TOCTOU races. Tests include direct and redirected private-IP probes,
dual-stack DNS answers, proxy environment poisoning, session-token reuse/expiry, and
root-relative race attempts.

## References

- [MCP Apps security overview](https://apps.extensions.modelcontextprotocol.io/api/documents/overview.html)
- [MCP Apps CSP and CORS](https://apps.extensions.modelcontextprotocol.io/api/documents/csp-and-cors.html)
