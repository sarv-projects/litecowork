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

No boundary is implicitly trusted merely because it is local.

## Threats and required controls

### Malicious capability/MCP
Controls:
- exact package digest/version lock
- declared permission review
- Task-scoped grant
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
- output verification

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
- execution lease epoch + fencing token
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

## Supply chain

LitePSM package verification metadata should include source, publisher, digest/signature, permissions, compatibility and last verification. LiteCowork consumes normalized lock metadata and independently applies task policy.
