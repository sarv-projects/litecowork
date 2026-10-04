# Security Architecture

## Trust boundaries

1. Human operator surfaces
2. Local LiteCowork Runtime
3. Other paired runtimes/cloud Hub
4. External agents
5. LitePSM/package sources
6. MCP/capability providers
7. Environments/sandboxes
8. External messaging channels
9. External content being processed
10. Embedded MCP App iframe

No boundary is implicitly trusted merely because it is local.

## Threats and required controls

### Malicious capability/MCP
Controls:
- exact package digest/version lock
- declared permission review
- grant matching the authenticated session scope; Conversation/planning grants are read-only and secret-free
- least-privilege resource scope
- process/network isolation where possible
- effect ledger for mutations
- revocation and health monitoring

### Prompt injection from web/email/doc
Controls:
- external content treated as data, not authority
- system/user policy precedence preserved by agent adapters where possible
- capabilities require independent grants regardless of content instructions
- sensitive effects require approval/policy even if agent requests them

### Compromised agent
Controls:
- agent cannot directly mutate Core domain state
- bounded TaskPacket
- capability grants and environment write scope
- lease/fence validation on Core effects
- short-lived AgentSession-scoped Gateway credentials and method allowlists
- separate EnvironmentControlLease epochs for human/agent input ownership
- output verification

### Credential solicitation through provider input

Ordinary UserRequest answers are non-secret and may replicate with Workspace data.
The Gateway rejects secret-typed or suspicious credential requests; provider credentials
must use a Connection/SecretStore flow or a supported out-of-band authorization handoff.
MCP form input is non-sensitive only. URL-mode state remains encrypted in the source
Runtime and reaches the authenticated Operator only after an explicit no-store handoff.
Unknown MCP input methods fail closed rather than becoming arbitrary user prompts.

### Compromised runtime/device
Controls:
- per-device identity keys
- runtime revocation
- no automatic credential replication
- secret placement rules
- artifact/event integrity checks
- sensitive cloud access policy

### Replay attacks
Controls:
- single-use pairing tokens
- request IDs/nonces for approvals and mutations
- short-lived authorization/secret leases
- fencing epochs
- deduplicated external channel events

### Stale executor/split brain
Controls:
- execution lease epoch + Runtime-private fencing credential (durable state stores only its digest)
- Core mutation validation
- explicit lease expiry before failover

### Channel spoofing
Controls:
- ChannelBinding to authenticated external identity
- provider event dedupe
- assurance levels
- sensitive approval escalation to stronger surface

### Artifact poisoning/tampering
Controls:
- content digest
- immutable ArtifactVersion
- provenance
- blob verification after replication

## Secrets

Secret bytes live in SecretStore implementation, not Task/Event payloads. Domain stores SecretRef and lease metadata only.

No browser cookies/native agent auth are silently uploaded to cloud.

## Network

Managed/default runtime connectivity is outbound authenticated TLS. Self-host options may use Tailscale/WireGuard/SSH but Core protocol is transport-independent.
Worker egress, DNS/redirect/SSRF protection, local IPC identity, filesystem TOCTOU,
upload/archive limits, and MCP App sandbox rules are normative in
[`NETWORK-SECURITY.md`](NETWORK-SECURITY.md). Sandbox-local firewall rules alone are not
the enforcement boundary.

MCP Apps have no host DOM, cookies, local storage, native IPC, local file, or SecretStore
access. CSP domains are checked by the host egress policy; every tool/resource request
passes the ordinary capability grant and Invocation checks. Consequential mutations also
require Effect recording before dispatch; audit records follow the operation's sensitivity
and Trust policy.

## Supply chain

LitePSM package verification metadata should include source, publisher, digest/signature, permissions, compatibility and last verification. LiteCowork consumes normalized lock metadata and independently applies task policy.
