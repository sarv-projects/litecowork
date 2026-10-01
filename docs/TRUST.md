# Trust, Authorization, Approvals and Secrets

## Principles

- least privilege
- explicit resource scope
- short-lived authority for risky actions
- user-visible approval for consequential escalation
- channel assurance matters
- package discovery is not trust
- credentials are not replicated as ordinary artifacts

## Core records

```text
Principal {
  principal_id
  kind: USER | SERVICE | RUNTIME | AGENT | CHANNEL_IDENTITY
  display_name
}

PolicyDecision {
  decision_id
  principal
  action
  resource
  decision: ALLOW | DENY | REQUIRE_APPROVAL
  reason_code
  policy_version
  evaluated_at
}

SecretRef {
  secret_ref
  provider
  placement: LOCAL_ONLY | CLOUD_AVAILABLE | RUNTIME_BOUND | EXTERNAL_AGENT_OWNED
  metadata
}

SecretLease {
  secret_lease_id
  secret_ref
  task_id
  attempt_id?
  capability_ref?
  runtime_id
  allowed_usage
  status: ACTIVE | REVOKED | EXPIRED
  issued_at
  expires_at
}
```

## TrustService

```text
interface TrustService {
  authorize(AuthorizationRequest) -> PolicyDecision
  request_approval(ApprovalRequest) -> Approval
  resolve_approval(ResolveApprovalRequest) -> Approval
  create_capability_grant(GrantRequest) -> CapabilityGrant
  revoke_grant(CapabilityGrantId) -> Ack
  lease_secret(SecretLeaseRequest) -> SecretLease
  revoke_secret_lease(SecretLeaseId) -> Ack
  audit(AuditRecord) -> Ack
}
```

## Authorization order

1. validate authenticated principal/runtime/channel identity.
2. validate Task/Attempt is current and fence is valid where required.
3. validate capability grant exists and is active.
4. validate requested operation is allowed.
5. validate resource scope.
6. validate secret placement/runtime constraints.
7. apply workspace policy.
8. validate effect class, idempotency/reconciliation support, and required approval.
9. if escalation is required, produce an approval request bound to exact action/target.
10. record the decision and issue only the minimum short-lived grant/lease.

Default is DENY when a required fact is unknown.

## Approval model

Approval request includes:

```text
ApprovalRequest {
  task_id
  attempt_id?
  requested_action
  target
  reason
  risk
  required_assurance
  expires_at?
  preview_ref?
}
```

Approval is tied to the exact action scope/digest. Material changes require new approval.
Approval is a one-time authorization decision, not a reusable capability grant. It is
consumed atomically with grant/effect admission and cannot be replayed against a changed
request digest or resource revision.

## Channel assurance

```text
AssuranceLevel =
  VIEW_ONLY
  STEER_SAFE
  APPROVE_SAFE
  APPROVE_SENSITIVE
  LOCAL_STRONG
```

A weak email identity may start low-risk Tasks and receive results but may not approve credential disclosure, destructive operations or external publication.

## Package/capability trust

Track:
- source provenance
- publisher identity
- version/digest
- signature if available
- declared permissions
- compatibility
- last package verification
- runtime health

Registry presence is not trust.

## Threat model requirements

Architecture must defend against:
- malicious MCP server
- malicious Skill/plugin instructions
- prompt injection from external content
- compromised worker agent
- compromised runtime
- stolen device key
- replayed pairing/approval request
- stale fencing token
- artifact poisoning/tampering
- spoofed messaging sender
- excessive privilege retained after Task completion

Mitigations are expanded in `SECURITY.md`.

## Secret lifecycle

SecretRef identifies a secret in an implementation-owned SecretStore. It may include
non-sensitive display/provider metadata, but never secret bytes. Placement values mean:

- `LOCAL_ONLY`: only the bound local Runtime may lease it.
- `CLOUD_AVAILABLE`: cloud use is allowed by the user/workspace policy, but still
  requires a Task/Attempt-scoped lease.
- `RUNTIME_BOUND`: only one explicitly identified Runtime may lease it.
- `EXTERNAL_AGENT_OWNED`: LiteCowork cannot copy or lease the credential; availability
  depends on the agent's own authenticated session.

A SecretLease binds one secret, Task, optional Attempt, optional Capability, Runtime,
allowed use, and expiry. Secret bytes are released only to the authorized provider
boundary and are redacted from logs, event payloads, agent-visible refs, and crash
reports. Revocation blocks new use immediately and terminates/revokes provider handles
where supported. A lease cannot be renewed after its Task/Attempt authority is revoked.

## Policy decisions and audit

PolicyDecision is immutable and records the policy version and reason code. AuditService
stores sensitive decisions and outcomes as append-only AuditRecords. Audit records may
reference a request digest and ResourceRef, but do not contain prompts, raw request
bodies, secrets, or unredacted provider output. Administrative retention does not erase
records needed to explain an active or disputed Effect.

An agent can request an operation, but it cannot grant itself permission, resolve an
Approval, change channel assurance, or mint a SecretLease. Only the authenticated
operator or a narrowly authorized service may exercise those transitions.
