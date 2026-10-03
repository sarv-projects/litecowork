# End-to-End Flows

Each flow's durable writes occur through the owning service. Domain events commit in the
same transaction as aggregate updates; UI is a projection and never drives truth
directly. The listed sequence is normative unless a linked owner contract is stricter.

## F00 — Workspace creation, replication policy, and archive

Actors: User, Operator UI, WorkspaceService, TrustService, RuntimeMesh.

1. The user creates a Workspace. If no replication policy is supplied, it is created as `LOCAL_ONLY`.
2. The UI explains each replication scope before the user explicitly enables cloud replication. `SELECTED_FOLDERS` requires one or more revision-pinned `workspace-folder://` ResourceRefs.
3. WorkspaceService commits the Workspace and `workspace.created` event.
4. A policy edit is a versioned prospective change; it does not erase already replicated bytes or grant permissions/secrets.
5. Archive is accepted only when every Task is terminal and every Automation is disabled. WorkspaceService commits `ARCHIVED`; reads and existing authorized artifact/resource downloads remain available, while every domain mutation is rejected, including Task changes, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel materialization.

Events: `workspace.created.v1`, `workspace.replication_policy.changed.v1`, `workspace.archived.v1`.

UI: show the selected policy and its scope; an archived Workspace is visibly read-only. No archive/delete animation occurs before the archive event is committed.

## F01 — Simple conversation, no Task

Actors: User, Operator UI, ConversationService, lead chat agent.

1. User sends factual/simple message.
2. message appended to Conversation.
3. the conversational agent produces an answer; no separate Core intent/planning model is invoked.
4. response appended as ConversationMessage.

Events: `conversation.message.added`.

No Task/Step/Attempt required unless the agent execution itself is represented internally as ephemeral chat work.

UI: ordinary conversation; no Task card.

## F02 — Conversation materializes a Task

1. User asks for outcome-oriented work or explicitly creates a Task from a message.
2. ConversationService persists the ConversationMessage.
3. TaskService creates Task + TaskSpecRevision(1) and binds the source message in one
   transaction; if the message and Task originate in one command, both commit together.
4. TaskService pins the selected lead AgentBinding and placement preference.
5. PlanningCoordinator starts a Task-scoped `LEAD_PLANNING` AgentSession without an Attempt,
   lease, Environment, or consequential capability grant.
6. After the session is ready, Task becomes RUNNING. The lead proposes PlanRevision(1);
   TaskService validates/promotes it and materializes Steps. Only then are Step Attempts
   admitted.
7. UI message expands into a Task card only after `task.created.v1` is durable. The planning
   phase may show a concise “Planning” status but creates no Live Desk work lane until a
   Step Attempt exists. Ambiguous intent remains a Conversation or gets a clarifying
   question; Core runs no hidden intent planner.

## F03 — Local Task happy path

1. Workspace is ACTIVE and local-only or otherwise permits the selected local resources.
2. A lead planning session proposes a plan; TaskService promotes PlanRevision and materializes Steps.
3. Step becomes READY.
4. placement selects local Runtime + Environment.
5. Attempt + lease + Step ownership commit atomically in the Hub transaction.
6. AgentSession starts after commit; Attempt becomes RUNNING only on adapter readiness.
7. agent executes.
8. Artifact/Effect/Evidence are recorded.
9. agent proposes finish.
10. verification runs.
11. Task COMPLETED.

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
9. activation lifecycle status becomes `ACTIVE`; a separate health observation records `HEALTHY`.
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
6. Later edit publishes version N+1 with expected Artifact aggregate version; prior versions remain addressable.
7. If a concurrent publisher wins, the stale publisher receives `STALE_VERSION`; preserve its draft and require explicit rebase or separate Artifact publication.

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

## F18 — Automation trigger and occurrence

1. Trigger host authenticates the source, normalizes any external payload, computes its digest, and resolves a bounded content-addressed ResourceRef; raw secret material is excluded.
2. Trigger host derives the revision-independent occurrence key from stable trigger identity.
3. TriggerCoordinator atomically claims `(automation_id, occurrence_key)`, stores the input ref/digest on first claim, and verifies the
   Automation is enabled, captures its current immutable AutomationRevision, and increments
   `claim_epoch`. If an edit races the claim, the transaction order decides the pinned
   revision; the claim is never reinterpreted afterward.
4. Same-key/same-digest duplicate returns the existing logical occurrence/Task. Same-key
   delivery with a different digest is rejected and audited; it cannot overwrite input.
   A reclaimed claim has a higher epoch; stale claimants cannot create or settle its Task.
5. Overlap policy is evaluated against that pinned revision.
6. A Task is created from the revision-pinned template and bounded input ref; the occurrence
   Task reference commits atomically with Task creation.
7. Task executes through the normal Task Runtime; the occurrence settles from Task outcome.
8. Result artifact/notification follows ordinary Artifact/Channel rules.

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
2. the activation remains `ACTIVE` unless its lifecycle actually fails; health is recorded as `DEGRADED` or `UNHEALTHY` independently.
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

1. User requests the new AgentBinding; TaskService validates Workspace ownership, enabled
   status, protocol compatibility, and placement availability.
2. TaskService records the requested lead binding and emits `task.lead_agent.changed.v1`.
   New planning/plan submissions use the new binding; existing Attempts retain their own
   agent identity and current lease while they drain or are cancelled at a safe boundary.
3. If a `LEAD_PLANNING` session is active, PlanningCoordinator closes it and starts a new
   planning session; no Attempt is fabricated.
4. TaskService writes a portable ResumePacket when execution context must be replaced.
   A replacement execution Attempt starts only after the prior lease is settled and open
   Effects are reconciled.
5. Task identity and prior session/Attempt history remain unchanged.

## F26 — Artifact Library archive

Precondition: the Workspace is active and the Artifact is SAVED. The user confirms the named Artifact and understands that archive removes it from the default Library view while preserving authorized version reads.

1. Operator sends `POST /v1/artifacts/{id}/archive` with Idempotency-Key and If-Match.
2. ArtifactStore authenticates/authorizes the owner, checks the aggregate version and SAVED state, then atomically commits ARCHIVED, increments Artifact.version, and appends `artifact.library.archived.v1`.
3. A replay with the same key returns its recorded result. A fresh command after archive with current If-Match returns the current representation without a second transition event; stale If-Match conflicts.
4. Existing immutable ArtifactVersions remain readable under current authorization. A linked Artifact archive does not mutate or delete the external provider object.
5. UI removes the item from the default Library projection after the committed event and retains it in archived/history views.

## F27 — Connection and channel binding setup, permissions, and revocation

Actors: User, provider-owned setup surface/ChannelAdapter, ConnectionService, ChannelService, TrustService.

1. The user starts setup through the selected integration's provider-owned flow. The
   Operator API does not prescribe OAuth, device-code, webhook-secret, or callback
   mechanics.
2. The provider authenticates its account and external identity. Credential bytes stay
   in the provider-owned secret mechanism; LiteCowork receives references and normalized
   status only.
3. ConnectionService records the Connection state. For a human-facing channel,
   ChannelService records the ChannelBinding using provider-attested identity and
   assurance with `allowed_actions = []`.
4. The owner reviews the binding and explicitly sets allowed actions. TrustService
   authorizes the change; an authority increase may require Approval. The versioned
   binding update and `channel.binding.changed.v1` event commit atomically.
5. On disconnect, ConnectionService blocks new capability use through that Connection;
   any linked ChannelBinding is non-authorizing even before its status projection
   updates. Binding revocation separately blocks inbound commands. Existing Conversations,
   Tasks, Artifacts, Effects, and audit history remain readable; no external account or
   credential is deleted.

Events: `connection.state.changed.v1`, `channel.binding.changed.v1`.

UI: show provider status, authenticated identity, assurance level, and granted actions.
Never show a binding as authorized while its allowed-action set is empty or its status
is REVOKED.

## Flow outputs and UI projection

| Flow | Durable result | Required UI projection / failure behavior |
|---|---|---|
| F00 | Workspace policy/lifecycle event | Show policy scope; archived Workspace is read-only |
| F01 | Conversation messages only | Ordinary exchange; no Task card |
| F02 | Task + initial spec linked to source message | Task card follows commit; planning is shown without a fake lane |
| F03 | Plan, Steps, lease, Attempt, outputs, verification | Lanes reflect persisted state; completion follows evaluator |
| F04 | Capability lock, grant, activation, invocation/Effect | Show capability only when active/used; unsafe path is unavailable |
| F05 | Proxy call, optional next-boundary attachment | Current turn continues without a forced restart |
| F06 | Approval, grant, resumed Attempt | Show exact scope and assurance; denial/expiry is not execution |
| F07 | Child Attempt and ResultEnvelope | Branch appears only after child creation; Inspector exposes ownership |
| F08 | Child failure/recovery event | Only affected lane stops; independent children continue |
| F09 | New TaskSpecRevision or steering message | Show revision impact and stale child results |
| F10 | Cancel intent, settled Attempts/Effects, terminal or needs-user state | Show Stopping until Effects reconcile; preserve outputs/history |
| F11 | Immutable ArtifactVersion and expected-version publication | Show only after blob digest and manifest commit; preserve stale concurrent drafts |
| F12 | VerificationRun/Evidence and Task result | Indicator follows a real verifier; no checkmark on fail/inconclusive |
| F13 | Handoff phases, old lease release, new Attempt | Show Cloud only after target lease; failure retains source location |
| F14 | Loss detection, lease expiry, reconciliation, eligibility | Show uncertainty/blocker until takeover is safe |
| F15 | Ambiguous Effect and reconciliation evidence | Never display success or resend while uncertain |
| F16 | Accepted/rejected events and updated cursor | Show reconnect/stale/conflict; no silent last-writer-wins |
| F17 | Paired Runtime identity and offers | Device appears only after one-use token validation/authentication |
| F18 | Revision-pinned occurrence and ordinary Task | One logical occurrence/Task; stale claim epochs are fenced |
| F19 | Completed Task with `NO_ACTION` result | Suppress notification only under saved policy |
| F20 | Shared Conversation/Task plus channel receipt | Same Task identity across surfaces; delivery failure is separate |
| F21 | Approval routed to stronger surface | Weak channel shows a link, not an enabled sensitive action |
| F22 | Existing CapabilityLock unchanged | In-flight Task stays pinned; no silent upgrade |
| F23 | Degraded activation health and SecretLease recovery | Show reconnect/reauth blocker; retry under Effect policy only |
| F24 | Isolated worktrees, diff Artifacts, integration result | Preserve branch provenance; conflicting edits block merge |
| F25 | New lead binding/session and Attempt history | Planning replacement creates no Attempt; execution replacement waits for lease safety |
| F26 | Artifact library archive event | Remove from default Library projection; preserve authorized version reads and linked source |
| F27 | Connection/ChannelBinding state and action changes | Show provider-attested identity, explicit authority, and preserved history on revocation |

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
