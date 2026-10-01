# Failure and Recovery

Recovery preserves Task truth and avoids repeating uncertain external effects. A
portable Task checkpoint is authoritative; a process, AgentSession, and Environment
snapshot are replaceable.

## Recovery ladder

1. Reconcile every ambiguous Effect.
2. Retry the same safe, idempotent operation with the same idempotency identity.
3. Resume the Attempt from the latest portable ResumePacket.
4. Replace the AgentSession while keeping Task and Attempt provenance explicit.
5. Create a new Attempt for the same Step.
6. Ask the lead agent for a revised PlanRevision.
7. Ask the user or stop.

Repeated failure with no state/evidence change must stop rather than loop indefinitely.

## Effect uncertainty

If an operation starts but its response is lost, mark the Effect `AMBIGUOUS`; do not
blindly repeat it. Reconcile through the provider's state, idempotency key, message ID,
artifact digest, or another suitable observation. Resolve as observed/verified, safe to
retry, or needs-user. An agent's statement alone is `REPORTED` evidence.

## Continuation classes

- `SAFE_PORTABLE`: only mediated/fenced or disposable-isolated consequential effects;
  automatic continuation can be allowed.
- `REPLAYABLE`: read-only or provably idempotent operations; reconcile before retry.
- `HANDOFF_REQUIRED`: native/unfenced effects exist; require explicit user/agent handoff.
- `LOCAL_BOUND`: a required device/session/resource is available only on a missing local
  Runtime; wait for it or offer a separate user-directed alternative.

Eligibility also requires compatible agent and Environment offers, available Task inputs,
proper secret placement, policy approval, reconciled effects, and remaining budget.
