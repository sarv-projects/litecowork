# Failure and Recovery

## Recovery ladder

Always prefer the smallest safe recovery:

```text
0 reconcile ambiguous Effect
1 retry same safe operation with same idempotency identity
2 restore Task state from a portable checkpoint
3 replace AgentSession inside the same Runtime incarnation and valid lease
4 create a new Attempt for the same Step after incarnation/lease loss
5 ask lead agent for revised PlanRevision
6 ask user
```

Restoring a portable checkpoint restores Task inputs and progress; it does not transfer a
live Attempt or its authority. The same Attempt may continue only when its owning Runtime
incarnation and lease are still current and the Environment is verified. A daemon restart
creates a new incarnation: reconcile Effects and the old lease, settle the old Attempt,
then create a fresh Attempt with a new AgentSession and a higher lease epoch when policy
allows. Provider-native session or Environment snapshots are optional optimizations and
cannot override this ownership rule.

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
| Linux service stop deadline | Operator drain or final lifecycle persistence exceeds systemd `TimeoutStopSec` | service manager sends SIGKILL after the 90-second bound | treat the incarnation as unclean; keep the prior local state non-clean where possible; next startup performs ordinary recovery and does not infer Effects or work settled from process exit |
| Runtime device identity | OS credential store unavailable, identity malformed, or RuntimeId/key mismatch | identity-provider startup error | fail closed before Operator readiness; do not create replacement key or Runtime identity; preserve only a truthful DEGRADED local status if the local store is available |
| Environment | provisioning fail | provider error | alternate provider/runtime or fail Step |
| Environment | corrupted/unhealthy | health/test | checkpoint if safe, recreate, new Attempt |
| MCP/provider | process dies or becomes unhealthy | LiteSPM/process-health result or invocation error | open LiteCowork call circuit; ask LiteSPM/provider owner to recover process under its lifecycle policy; retry reads/idempotent operations only |
| Async CapabilityInvocation | cancellation acknowledgement without terminal provider state | provider task remains working or unavailable | keep Task PAUSE_REQUESTED/CANCEL_REQUESTED; reconcile provider state; never claim stopped |
| Async CapabilityInvocation | initial MCP task handle response lost after dispatch | transport timeout before the opaque handle is committed to its encrypted Runtime-local binding | keep Invocation AMBIGUOUS; recover only by provider lookup using a precommitted idempotency identity; never blindly redispatch a potentially effectful `tools/call` |
| Provider input request | UserRequest expires before response delivery | expiry event while provider task may still be waiting | mark local input binding EXPIRED; cancel/reconcile provider task; keep ConversationTurn WAITING_DEPENDENCY until quiescent, then fail retryably; block only the affected Task Step |
| LiteSPM | unavailable | client error | existing locked activations continue; new discovery waits/fails clearly |
| Secret/OAuth | revoked/expired | auth error | WAITING_RESOURCE; re-auth; new SecretLease |
| BlobStore | unavailable | storage error | pause Artifact publication; retry; never create version referencing missing blob |
| StateStore | unavailable | storage error | stop authoritative mutations; UI may show cached read-only state |
| Mesh partition | transport/heartbeat | retain local safe work under policy; no conflicting new cross-runtime lease |
| Hub unavailable | failed calls | cross-device coordination pauses; local policy governs existing attempts |
| Event projection | bug/corruption | checksum/invariant | rebuild projection from journal |
| External effect | timeout after send | missing definitive response | mark AMBIGUOUS; reconcile before retry |
| Verifier | crash | run timeout | retry verifier or alternate verifier; Task remains VERIFYING |
| Verification during pause/cancel | run does not settle | bounded run deadline | keep PAUSE_REQUESTED/CANCEL_REQUESTED; settle as INCONCLUSIVE on timeout; late evidence cannot complete a fenced Task |
| Approval channel | unavailable | delivery error | approval stays PENDING; use another eligible surface |
| Capability update | incompatible | health/resolve | in-flight lock remains; rollback/choose old version for new activation if available |
| DelegationProfile | disabled/archived before admission | profile version check | reject new child with typed error; never substitute unless selection mode/policy permits |
| DelegationProfile | disabled/revised after admission | pinned revision on Attempt | current child keeps its pin; explicit stop uses ordinary safe cancellation |
| Worker option | model/reasoning override unsupported | adapter schema validation | fail before model invocation; no silent default |
| Agent harness | normalized native config digest changed | descriptor reprobe | revalidate new admissions; active session finishes only if adapter proves frozen config |
| Quota | no provider observation | adapter returns UNKNOWN/expiry | show unavailable/unknown; do not estimate remaining quota or claim exhaustion |
| Prewarm | host/auth/config/environment becomes unavailable | fresh admission probe | discard warm hint and run ordinary admission or show blocker; Task state is unchanged |
| Worker cost | monetary usage unknown under hard ceiling | BudgetService comparison | reject candidate unless provider/host enforces the cap; never treat unknown as zero |
| Escalation | quality floor fails after bounded candidates | verifier + policy limit | stop, preserve Attempts/Evidence, return unmet criteria to lead/Needs You |
| Shared Environment | sharing owner/scope mismatch | EnvironmentManager owner check | reject attachment before session start; do not fall back to broader scope |
| Browser/computer | another actor owns EnvironmentControlLease | epoch check | reject/queue no input and show current controller; never replay stale commands |
| Coworker | paused/archived at admission | Coworker revision/status check | block new proactive/scheduled work; existing Tasks continue by their own policy |
| Goal/Suggestion source | stale/conflicted or expired | Resource/Goal/Suggestion version check | preserve proposal/history; no acceptance/task creation from stale state |
| ContextDocument | concurrent ResourceRevision edit | expected Resource head | keep both branches and request explicit rebase/merge; no last-writer-wins |
| Demonstration | secret detected or capture interrupted | redaction/Environment observation | abort or remove unsafe content under retention policy; no SkillProposal publication |
| Pinned Skill or Routine dependency | explicit semantic incompatibility | provider preflight or verifier evidence against the pinned revision | set `DRIFTED`, block unsafe replay with `SKILL_DRIFT_DETECTED`, and offer a reviewed repair proposal; never rewrite published Skill/Routine/Automation revisions |
| Environment sharing change | live Attempt, Invocation, control lease, or unresolved Effect | EnvironmentManager and storage guard | reject while retaining the old scope; suspend and reconcile before an explicit retry |

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

- all Core-mediated conflicting mutations validate authenticated Runtime/incarnation,
  active lease ID/epoch/expiry and the runtime-private fencing credential; only its digest
  is durable.
- new lease epoch supersedes all older epochs.
- stale runtime reconnect cannot mutate using cached authority.

## Provider call circuit breaker

Persist a call-admission circuit keyed by provider identity and Runtime. A configurable
rolling failure window opens the circuit with bounded backoff; only one half-open probe
may run after `open_until`. Success closes the circuit and clears the failure window;
failed probes reopen it. While OPEN, reject new invocations and report the reason; existing
results remain readable. A fallback provider is used only when an explicit
compatibility/permission decision selects it. Agent model failures use the AgentAdapter's
own quota/backoff policy and do not share this provider circuit.

This circuit is not a process supervisor and does not issue restart/stop commands. LiteSPM
owns package/provider launch, sharing, health, restart, and idle-stop lifecycle under its
future contract. Environment and Channel adapters are supervised by their owning Runtime
services. LiteCowork may report observed provider health and stop sending calls while the
circuit is open.

Circuit state is operational runtime health, not Task outcome or a CapabilityGrant. Its
transition is versioned and emits `provider.circuit.changed.v1` so all projections report
consistent availability after restart or replication.
- native external effects outside Core are classified HANDOFF_REQUIRED/LOCAL_BOUND as appropriate and are not falsely fenced.

## Projection recovery

Durable event journal is source of truth for replayable projections. Projection schema changes may rebuild from event stream plus immutable records/blobs.
