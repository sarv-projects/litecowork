# Failure and Recovery

## Recovery ladder

Always prefer the smallest safe recovery:

```text
0 reconcile ambiguous Effect
1 retry same safe operation with same idempotency identity
2 resume Attempt from portable/native checkpoint
3 replace AgentSession
4 create new Attempt for same Step
5 ask lead agent for revised PlanRevision
6 ask user
```

Stop when the same failure signature recurs with no meaningful new state/evidence and configured recovery budget is exhausted.

## Failure signature

```text
FailureSignature = hash(
  component_kind,
  error_code,
  operation,
  capability/provider,
  target_class,
  normalized_root_cause
)
```

Do not include volatile timestamps/random IDs.

## Failure matrix

| Component | Failure | Detection | Default response |
|---|---|---|---|
| Lead AgentSession | process/session lost | adapter event/stream EOF | replace session from ResumePacket; new Attempt if ownership changed |
| Child agent | lost/fails | adapter event | retry child independently or let lead revise plan |
| Runtime | offline | presence + lease expiry | reconcile Effects; eligible failover only |
| Environment | provisioning fail | provider error | alternate provider/runtime or fail Step |
| Environment | corrupted/unhealthy | health/test | checkpoint if safe, recreate, new Attempt |
| MCP/provider | process dies | activation health/invoke error | restart activation; retry read/idempotent operations only |
| LitePSM | unavailable | client error | existing locked activations continue; new discovery waits/fails clearly |
| Secret/OAuth | revoked/expired | auth error | WAITING_RESOURCE; re-auth; new SecretLease |
| BlobStore | unavailable | storage error | pause Artifact publication; retry; never create version referencing missing blob |
| StateStore | unavailable | storage error | stop authoritative mutations; UI may show cached read-only state |
| Mesh partition | transport/heartbeat | retain local safe work under policy; no conflicting new cross-runtime lease |
| Hub unavailable | failed calls | cross-device coordination pauses; local policy governs existing attempts |
| Event projection | bug/corruption | checksum/invariant | rebuild projection from journal |
| External effect | timeout after send | missing definitive response | mark AMBIGUOUS; reconcile before retry |
| Verifier | crash | run timeout | retry verifier or alternate verifier; Task remains VERIFYING |
| Approval channel | unavailable | delivery error | approval stays PENDING; use another eligible surface |
| Capability update | incompatible | health/resolve | in-flight lock remains; rollback/choose old version for new activation if available |

## Retry classes

### Safe automatic retry
- read-only operation
- local deterministic computation
- failed-before-send confirmed
- idempotent provider operation with stable idempotency key

### Reconcile first
- email/message send
- payment/order mutation
- publish/deploy
- delete/move external resource
- calendar/CRM update if provider acknowledgement uncertain

### Never auto-retry without user/policy
- destructive operation without idempotency/reconciliation support
- native unfenced side effect after runtime loss
- action whose semantic target may have changed since checkpoint

## Recovery budgets

Every Task may define:

```text
RecoveryBudget {
  max_same_signature_retries
  max_step_attempts
  max_plan_revisions_due_to_failure
  max_recovery_wall_time
}
```

Budget exhaustion produces BLOCKED/NEEDS_USER with failure summary.

## Split-brain defense

- all Core-mediated conflicting mutations validate active fencing token.
- new lease epoch supersedes all older epochs.
- stale runtime reconnect cannot mutate using cached authority.
- native external effects outside Core are classified HANDOFF_REQUIRED/LOCAL_BOUND as appropriate and are not falsely fenced.

## Projection recovery

Durable event journal is source of truth for replayable projections. Projection schema changes may rebuild from event stream plus immutable records/blobs.
