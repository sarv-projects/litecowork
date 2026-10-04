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

`GrantRequest.scope` is one of `CONVERSATION {conversation_id}`, `TASK_PLANNING
{task_id}`, or `ATTEMPT_EXECUTION {task_id, attempt_id}`. Scope identifiers must match
the AgentSession's parent Conversation/Task/Attempt and Workspace; a Conversation-scoped
AgentSession additionally binds one exact `conversation_turn_id`. Conversation- and planning-scoped grants
are read-only, contain no SecretRefs, and expire; an Attempt grant is pinned to the
admitted Attempt and cannot outlive its execution authority. Scope may be narrowed for an
Invocation but never widened.

## Authorization order

1. validate authenticated principal/runtime/channel identity.
2. validate the Conversation/Task/Attempt scope matches the authenticated AgentSession's
   parent scope; validate the current ConversationTurn or live Task/Attempt lease as
   required.
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
  target_ref
  scope_digest
  action_digest
  reason
  risk
  required_assurance
  expires_at?
  preview_ref?
}
```

Approval persists target_ref, scope_digest, and action_digest. The action digest binds the
exact operation, target revision, requested scope, Task, and Attempt when present.
Material changes require new approval. `ApprovalUse.request_digest` is the canonical
digest of the actual operation request retained for audit and deduplication. During
consumption, TrustService recomputes the approved action binding from that request and
requires it to equal `Approval.action_digest`; matching only a request ID or a similar
summary is insufficient.

Approval is a one-time authorization decision, not a reusable capability grant. An
approval for capability escalation is consumed by the transaction that creates the
scoped CapabilityGrant, so its `ApprovalUse` names that grant. An approval for a
consequential operation is consumed by the transaction that admits its Effect, so its
`ApprovalUse` names that Effect. These are distinct authorization uses and never share
one approval record. Approval resolution alone does not consume it.

The internal `consume_approval(approval_id, request_digest, target, transaction)` port
validates the approved action binding and atomically appends `ApprovalUse` with its
single target. The enclosing application command creates the target and use in one
authoritative transaction and emits the corresponding domain events together.
Consumption also rechecks the owning Task is active in that transaction. After
`CANCEL_REQUESTED` commits, a previously approved but unused Approval remains immutable
audit state but can no longer authorize a grant or Effect. The cancellation consumer may
move a still-PENDING Approval to `CANCELLED` through TrustService; it cannot rewrite an
already resolved decision.

Consumption is represented by immutable `ApprovalUse`, committed in the same authoritative
transaction as the approved Effect admission or CapabilityGrant issuance. Exactly one
target (`effect_id` or `capability_grant_id`) is recorded; a unique constraint on
`approval_id` permits at most one use. Duplicate delivery of the same command returns its
deduplicated prior result, while any second use is rejected as
`APPROVAL_ALREADY_CONSUMED`. Approval resolution alone does not consume it.

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

SecretRef identifies an object in an implementation-owned SecretStore. Its `provider_ref`
is a stable opaque, non-secret lookup key that cannot grant access by itself; raw
provider-native handles stay inside the SecretStore adapter. A SecretRef may include
non-sensitive display/provider metadata, but never secret bytes or a filesystem path.
Placement values mean:

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
The Agent receives no SecretRef store locator or secret bytes; a host/provider receives a
short-lived lease through the narrowly scoped SecretStore delivery port. AgentBinding
configuration contains declarative non-secret options only, and native agent credentials
remain owned by that agent when `EXTERNAL_AGENT_OWNED` is selected.

Ordinary `UserRequest` responses are non-secret Workspace input and may be replicated
under the Workspace policy; they are never a credential-entry channel. Agent `user.ask`
and MCP form elicitation must not solicit passwords, API keys, recovery codes, tokens, or
other credentials. Credential setup uses the owning Connection/provider authentication
flow or an explicitly supported out-of-band URL handoff. MCP form elicitation is treated
as non-sensitive input; suspicious credential-like schema fields are rejected as a
defense-in-depth check, not assumed to make arbitrary prose safe. An MCP URL-mode request
keeps its potentially state-bearing URL encrypted in the source Runtime's private
provider binding. The Operator receives it only after an authenticated explicit action,
with no-store handling; LiteCowork does not log, replicate, or back it up. Credentials
entered at the provider's HTTPS origin do not pass through a UserRequest response or
LiteCowork's ordinary API request body.

## Policy decisions and audit

PolicyDecision is immutable and records the policy version and reason code. AuditService
stores sensitive decisions and outcomes as append-only AuditRecords. Audit records may
reference a request digest and ResourceRef, but do not contain prompts, raw request
bodies, secrets, or unredacted provider output. Administrative retention does not erase
records needed to explain an active or disputed Effect.

An agent can request an operation, but it cannot grant itself permission, resolve an
Approval, change channel assurance, or mint a SecretLease. UserRequest answers are
ordinary user input and are never interpreted as Approval without this explicit Trust
transition. Only the authenticated
operator or a narrowly authorized service may exercise those transitions.
