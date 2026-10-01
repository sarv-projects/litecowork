# End-to-End Flows

Each flow's durable writes occur through the owning service. Domain events commit in the
same transaction as aggregate updates; UI is a projection and never drives truth
directly. The listed sequence is normative unless a linked owner contract is stricter.

## F01 — Simple conversation, no Task

Actors: User, Operator UI, ConversationService, lead chat agent.

1. User sends factual/simple message.
2. message appended to Conversation.
3. the conversational agent answers; no separate Core intent/planning model is invoked.
4. agent answers.
5. response appended as ConversationMessage.

Events: `conversation.message.added`.

No Task/Step/Attempt required unless the agent execution itself is represented internally as ephemeral chat work.

UI: ordinary conversation; no Task card.

## F02 — Conversation materializes a Task

1. User asks for outcome-oriented work or explicitly creates a Task from a message.
2. ConversationService persists the ConversationMessage.
3. TaskService creates Task + TaskSpecRevision(1) and binds the source message in one
   transaction; if the message and Task originate in one command, both commit together.
4. lead AgentBinding selected from user/default policy.
5. plan/initial Attempt starts.
6. UI message expands into a Task card only after `task.created.v1` is durable. Ambiguous
   intent remains a Conversation or gets a clarifying question; Core runs no hidden
   intent planner.

## F03 — Local Task happy path

1. Task READY.
2. lead submits PlanRevision.
3. Steps materialized.
4. Step READY.
5. placement selects local Runtime + Environment.
6. Attempt + lease + Step ownership commit atomically in the Hub transaction.
7. AgentSession starts after commit; Attempt becomes RUNNING only on adapter readiness.
8. agent executes.
9. artifact/effect/evidence recorded.
10. agent proposes finish.
11. verification runs.
12. Task COMPLETED.

## F04 — Capability discovery and direct attachment

1. worker needs operation not currently available.
2. `litecowork.capabilities.search` -> CapabilityBroker.
3. Broker searches LitePSM normalized catalog.
4. worker calls describe/select.
5. Broker resolves exact package version/digest.
6. Trust evaluates grant/approval.
7. provider installed/started on eligible Runtime if needed.
8. direct attach only if the provider enforces the Task grant and required Effect/fence
   contract; otherwise select proxy.
9. activation becomes HEALTHY.
10. worker invokes the capability; a direct mutation cannot bypass authorization or
    recovery. If neither path preserves required guarantees, the operation is unavailable.

UI: source/capability appears only after activation/use.

## F05 — Capability proxy fallback

Same as F04 through grant, but agent cannot hot-attach.

1. Gateway keeps capability behind `litecowork.capabilities.invoke`.
2. current turn uses proxy.
3. at safe next session boundary, direct attach may occur if supported.
4. Task does not restart solely for capability discovery.

## F06 — Approval-gated capability

1. grant request requires sensitive permission.
2. TrustService returns Approval(PENDING).
3. Attempt -> WAITING_APPROVAL.
4. UI shows exact requested action and scope.
5. user approves on adequate-assurance surface.
6. TrustService atomically records the decision and issues the scoped grant; Attempt
   resumes only after activation and secret prerequisites are healthy.

Denied/expired approval produces policy failure or revised plan; no effect is executed.

## F07 — Host delegation

1. lead calls `litecowork.agents.delegate` with DelegateRequest.
2. Core validates depth, budget, capabilities, runtime/environment and isolation.
3. child Step/Attempt created.
4. bounded TaskPacket delivered to child AgentSession.
5. child runs independently.
6. child returns ResultEnvelope + refs.
7. parent is notified.
8. lead integrates/rejects/retries.

No parent full transcript is copied by default.

## F08 — Child failure

1. child AgentSession lost or Attempt fails.
2. other independent child Attempts continue.
3. recovery ladder chooses resume/replace/new Attempt.
4. if child objective no longer worth retrying, lead revises plan.
5. parent sees structured blocker/failure, not a fabricated result.

## F09 — Task steering during execution

1. user sends steering message.
2. classify as task-spec change, plan hint, interrupt or chat-only.
3. spec change creates TaskSpecRevision(n+1).
4. lead AgentSession receives new revision.
5. relevant child Attempts are updated/cancelled at safe boundary.
6. results produced against stale revision are marked with their source revision and require applicability check.

## F10 — Cancellation

1. User requests cancellation with the expected Task version.
2. Task -> CANCEL_REQUESTED.
3. signal all host-owned Attempts.
4. block new consequential Effects/grants.
5. interrupt/cancel agents.
6. reconcile open Effects.
7. terminate isolated environments where safe.
8. Task -> CANCELLED after authoritative work settles and Effects reconcile. If an Effect
   remains ambiguous, Task becomes NEEDS_USER with cancellation intent retained.

## F11 — Artifact production/revision

1. provider creates bytes/draft.
2. BlobStore commits content digest.
3. ArtifactStore creates immutable ArtifactVersion.
4. event emitted.
5. UI shows artifact only after version exists.
6. later edit creates version N+1; prior version remains addressable.

## F12 — Completion verification

1. worker calls finish proposal.
2. Task -> VERIFYING.
3. CompletionEvaluator enumerates mandatory criteria/outputs/effects/approvals.
4. VerifierRegistry selects deterministic verifier first.
5. evidence appended.
6. all mandatory criteria pass -> COMPLETED.
7. failed criterion -> INCOMPLETE or RUNNING with recovery.
8. uncertain semantic criterion -> NEEDS_USER or independent verifier agent.

## F13 — Graceful local -> cloud handoff

1. user closes laptop or requests cloud continuation.
2. source Attempt reaches a safe boundary; unsafe/unfenced work requires explicit review.
3. ResumePacket written.
4. events/artifacts required by Task replicated.
5. open Effects reconciled.
6. source lease RELEASED.
7. target cloud Runtime validates eligibility.
8. target acquires higher-epoch lease after eligibility is rechecked.
9. fresh Attempt/AgentSession starts; no live process is transferred.
10. Task continues.

UI sequence: `This computer -> Saving progress… -> Cloud`, only after actual handoff phases.

## F14 — Unexpected device loss

1. heartbeat stale.
2. wait for execution lease expiry.
3. Attempt becomes ABANDONED/uncertain.
4. reconcile open Effects.
5. evaluate FailoverClass + ContinuationEligibility.
6. SAFE_PORTABLE/REPLAYABLE -> fresh Attempt elsewhere only after lease expiry/skew
   guard, reconciliation, and provider-fence checks.
7. HANDOFF_REQUIRED -> wait/ask.
8. LOCAL_BOUND -> wait for local resource.

## F15 — Ambiguous external effect

Example: email send request left process, network failed before response.

1. Effect already STARTED.
2. no trustworthy result -> AMBIGUOUS.
3. Reconciler checks provider state/id/message digest.
4. found -> OBSERVED/VERIFIED.
5. confirmed absent -> same idempotency identity may retry according to policy.
6. cannot determine -> Task NEEDS_USER; do not blindly repeat.

## F16 — Offline Runtime reconnect

1. local Runtime was offline while Hub received new events.
2. reconnect authenticates identity.
3. exchange replication cursors.
4. upload local unseen events; download Hub unseen events.
5. Hub validates origin authority, expected revisions, transition owner, and fences.
6. entity-specific conflict rules apply; rejected stale events cannot change projections.
7. stale lease/fence cannot regain authority.

## F17 — Runtime pairing

1. strong operator creates short-lived pairing token.
2. new Runtime generates key and submits token + public key.
3. Hub binds runtime identity.
4. runtime capabilities/environments advertised.
5. UI displays new device/runtime.

## F18 — Scheduled automation

1. trigger host fires logical occurrence.
2. occurrence claim deduplicated.
3. Automation enabled/overlap policy checked.
4. ordinary Task created from template.
5. Task executes through normal runtime.
6. result artifact/notification produced.
7. occurrence terminal state recorded.
Task creation and recording its occurrence reference are one transaction; duplicate
deliveries of the same scheduled key return that Task.

## F19 — Conditional monitor with no change

1. Automation occurrence creates Task.
2. agent/capability checks condition.
3. condition false.
4. Task completes with a `NO_ACTION` result artifact/evidence as policy requires. `NO_ACTION`
   is a result value, not a Task status.
5. notification suppressed.

When condition becomes true, notify according to policy and optionally disable automation.

## F20 — Telegram/email Task

1. ChannelAdapter receives deduplicated external message.
2. external identity maps to ChannelBinding/Workspace.
3. same Conversation mapping used.
4. Task materializes normally if needed.
5. cloud Runtime executes.
6. result response sent to same channel.

No separate bot Task store.

## F21 — Sensitive approval from weak channel

1. task requests sensitive effect.
2. Approval requires higher assurance than channel provides.
3. channel receives "approval required" + deep link.
4. strong LiteCowork surface authenticates user and resolves approval.
5. Task resumes.

## F22 — Package update while Task running

1. LitePSM reports a newer package through its separately defined contract.
2. running Attempt remains pinned to CapabilityLock digest/version.
3. new Tasks may resolve newer version according to policy.
4. upgrade never mutates an in-flight capability silently.

## F23 — OAuth/secret expires mid-Task

1. capability invocation returns auth failure.
2. activation -> DEGRADED/FAILED.
3. Attempt WAITING_RESOURCE/approval as appropriate.
4. reconnect/re-auth creates/updates SecretRef outside transcript.
5. new SecretLease issued.
6. retry only if effect safety permits.

## F24 — Parallel code workers

1. lead delegates independent changes.
2. each child receives distinct GitWorktree Environment.
3. each produces diff/branch Artifact.
4. lead integrates in dedicated integration Environment.
5. tests verifier runs after merge.

Never permit two workers to mutate same checkout without explicit serialization.

## F25 — User changes lead agent

1. current Attempt reaches checkpoint/safe boundary when possible.
2. ResumePacket created.
3. old session closed/abandoned.
4. new AgentBinding + AgentSession starts as new Attempt where required.
5. Task identity/history unchanged.

## Flow outputs and UI projection

| Flow | Durable result | Required UI projection / failure behavior |
|---|---|---|
| F01 | Conversation messages only | Ordinary exchange; no Task card |
| F02 | Task + initial spec linked to source message | Inline Task appears only after commit; failure is explicit |
| F03 | Plan, Steps, lease, Attempt, outputs, verification | Lanes reflect persisted state; completion follows evaluator |
| F04 | Capability lock, grant, activation, invocation/Effect | Show capability only when active/used; unsafe path is unavailable |
| F05 | Proxy call, optional next-boundary attachment | Current turn continues without a forced restart |
| F06 | Approval, grant, resumed Attempt | Show exact scope and assurance; denial/expiry is not execution |
| F07 | Child Attempt and ResultEnvelope | Branch appears only after child creation; Inspector exposes ownership |
| F08 | Child failure/recovery event | Only affected lane stops; independent children continue |
| F09 | New TaskSpecRevision or steering message | Show revision impact and stale child results |
| F10 | Cancel intent, settled Attempts/Effects, terminal or needs-user state | Show Stopping until Effects reconcile; preserve outputs/history |
| F11 | Immutable ArtifactVersion | Show only after blob digest and manifest commit |
| F12 | VerificationRun/Evidence and Task result | Indicator follows a real verifier; no checkmark on fail/inconclusive |
| F13 | Handoff phases, old lease release, new Attempt | Show Cloud only after target lease; failure retains source location |
| F14 | Loss detection, lease expiry, reconciliation, eligibility | Show uncertainty/blocker until takeover is safe |
| F15 | Ambiguous Effect and reconciliation evidence | Never display success or resend while uncertain |
| F16 | Accepted/rejected events and updated cursor | Show reconnect/stale/conflict; no silent last-writer-wins |
| F17 | Paired Runtime identity and offers | Device appears only after one-use token validation/authentication |
| F18 | Deduplicated occurrence and ordinary Task | One occurrence/Task per scheduled key |
| F19 | Completed Task with `NO_ACTION` result | Suppress notification only under saved policy |
| F20 | Shared Conversation/Task plus channel receipt | Same Task identity across surfaces; delivery failure is separate |
| F21 | Approval routed to stronger surface | Weak channel shows a link, not an enabled sensitive action |
| F22 | Existing CapabilityLock unchanged | In-flight Task stays pinned; no silent upgrade |
| F23 | Degraded activation and SecretLease recovery | Show reconnect/reauth blocker; retry under Effect policy only |
| F24 | Isolated worktrees, diff Artifacts, integration result | Preserve branch provenance; conflicting edits block merge |
| F25 | New lead binding/session and Attempt history | Task identity stays stable; handoff appears after new Attempt exists |

## Shared flow invariants

- Every mutating command has an authenticated principal, RequestId, correlation ID, and
  expected aggregate version where concurrent edits are possible.
- Aggregate mutation and its event commit atomically. UI motion waits for the resulting
  projection event.
- Direct/native reports never receive stronger evidence than the adapter/provider
  actually proves.
- External content and trigger payloads are untrusted; they cannot add grants, approvals,
  or SecretLeases.
- Failure preserves committed Conversation, Task, Artifact, Effect, and Evidence history.
  Retries retain explicit provenance rather than rewriting history.
