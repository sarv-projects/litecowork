# Test Architecture

## Test layers

### Unit
Pure domain invariants, schema validation, state transitions, conflict rules, policy evaluation helpers.

### State-machine/property tests
Generate legal/illegal sequences for Task, Attempt, Effect, Approval, Lease and AutomationOccurrence. Invariants must hold under arbitrary command ordering.

### Contract tests
Every AgentAdapter, EnvironmentProvider, Capability/LiteSPM adapter, StateStore, BlobStore and ChannelAdapter implementation runs a common conformance suite.

### Integration
Task Runtime + storage + one agent adapter + one capability + one environment.

### Fault injection
Kill processes, sever network, expire leases, corrupt provider responses, duplicate events, reorder event batches, expire secrets, fail blob uploads.

### Cross-runtime
At least local + cloud runtime with real event/artifact replication and lease handoff.

### Security
Permission bypass attempts, stale execution/control fencing tokens, Gateway token replay
and expiry, spoofed channel events, malicious capability metadata, secret leakage checks,
prompt-injection scenarios, SSRF/private-IP/DNS-rebinding/redirect probes, MCP App sandbox
and CSP violations, filesystem symlink/TOCTOU races, archive/decompression bombs, and
ApprovalUse replay with changed digests. MCP Skill conformance covers origin+URI identity,
complete manifest, SHA-256 and exact byte-size checks, frontmatter consistency, per-Skill
limits, manifest-change revocation, nested Skill consent, `allowed-tools` not widening
grants, and cross-origin resource-read isolation. Also test Skills referenced by exact URI
but omitted from a partial listing, rejection of unverified scheme-only Skills,
`resources/directory/read` negotiation, and that directory results never expand an
approved manifest; dynamic Skills are not activated in v1. MCP App tests distinguish the
host-computed UI content digest from publisher authenticity and reject undeclared domains.

### Schema and API conformance
Validate every JSON Schema; validate every current DomainEvent registry entry against its
typed payload branch; assert an invalid/missing event payload fails; compare the error
registry with the documented enum; parse OpenAPI with duplicate-key rejection, validate
it as OpenAPI 3.1, and resolve all local references; verify LiteCowork Gateway names use
the frozen `litecowork.*` namespace; execute SQLite DDL and inspect foreign keys/indexes.

### E2E
Real user workflows from BENCHMARKS.md.

### Performance/soak
Long Tasks, many events, large artifacts, large file/data corpora, repeated reconnects, hours-long automation/channel operation.

## Mandatory vertical slice gates

### Gate 1 — durable local Task
Desktop -> local runtime -> external ACP agent -> LiteCowork Gateway -> LiteSPM -> MCP -> Artifact/Effect -> Verifier.

Kill worker mid-Task; replacement must continue from ResumePacket and complete.

### Gate 2 — heterogeneous delegation
Lead agent delegates bounded child to different external agent; no shared full transcript; child result integrates and verifies.

### Gate 3 — cloud
Local Task replicates; explicit handoff creates higher-epoch cloud Attempt; reconnect shows same final state. Kill local runtime mid-effect and prove no duplicate external action.

### Gate 4 — channels
Telegram/email creates/steers the same Conversation/Task; a reply to one delivered FORM
UserRequest resolves only that exact request; sensitive approval and external sign-in stay
on the stronger Operator surface. Reassign the ChannelBinding between Runtimes during a
pending prompt and during an inbound claim; verify higher host epoch, no copied reply
references, stale-owner rejection, ambiguous outbound reconciliation, and Operator inbox
fallback for replies to old prompts.

### Gate 5 — automation
Recurring trigger creates ordinary Task and deduplicates duplicate trigger delivery.

## Non-negotiable invariant tests

- worker cannot set Task COMPLETED directly
- old fence rejected after new epoch
- Attempt, AgentSession, Environment and ExecutionLease composite ownership keys reject a different Task, selected AgentBinding, Runtime incarnation, or Environment; a daemon restart requires a fresh Attempt and higher lease epoch
- EnvironmentControlLease cannot control an Environment other than the one pinned by its Attempt, even when both Environments share one Runtime
- raw fencing credentials never appear in SQLite, events, aggregate blobs, API projections, logs, or backups; only the digest is durable
- lease acquire/renew/release retries with the same RequestId are idempotent; conflicting request reuse fails, renewal never extends execution past the last provider-confirmed expiry, and issuer-key loss fails closed
- ArtifactVersion references committed blob digest
- archived Artifact rejects new content versions and remains readable by immutable version
- stale concurrent Artifact publication does not overwrite the winning version
- archive replay does not append a second transition event
- duplicate external channel event creates no duplicate message/task
- exact channel reply-to correlation resolves only its one active UserRequest target; an ordinary message in the same thread never selects the latest open request
- channel response rejects another sender, revoked/low-assurance binding, missing RESPOND action, expired/answered request, duplicate event, ambiguous/missing target, attachments, nested schema, and credential-like content without consuming a valid target
- channel response records the exact provider event provenance, atomically accepts its receipt and consumes only the matched target; replay cannot answer a sibling request or resolve an Approval
- channel host assignment is unique per binding; a new Runtime cannot claim/settle receipts or use reply targets at an old epoch, and no new host lease is granted before source settlement or authoritative expiry plus skew margin
- a receipt is durably replicated before cursor/deferred-ack advancement; reclaim after expired/fenced claim increments claim_epoch, preserves origin provenance, and rejects a conflicting digest
- host restart marks stale cursor incarnation for reconciliation; host move resumes after last Hub-replicated receipt, and a move without replay/transfer requires explicit gap acceptance that remains visible
- provider-accepted notification timeout remains AMBIGUOUS and cannot create a reply target or retry until reconciliation
- duplicate automation trigger creates no duplicate occurrence task
- package update does not alter in-flight CapabilityLock
- Conversation-scoped AgentSession works without Task/Attempt and has read-only grants
- A Conversation-scoped UserRequest is linked to one exact turn and originating session; responding to it cannot resume another concurrent/waiting turn, and non-Conversation UserRequests cannot resume a chat turn
- each settled or user-waiting ConversationTurn closes its AgentSession and releases its host binding; user response continues the same turn in a fresh session with bounded durable context, while every response retains its original session provenance
- a pending durable CapabilityInvocation continues after its AgentSession closes and keeps its Activation/provider host retained; delivery to a replacement session requires the same ConversationTurn or the same source Attempt, Runtime incarnation, valid current lease, and reconciled checkpoint. A replacement Attempt/Runtime cannot inherit an old MCP task/input key; reconcile or cancel first, then issue newly authorized work with the saved response as context. An unmediated live tool call cannot be detached as if durable
- a Task-scoped UserRequest answered during `PAUSE_REQUESTED` is rejected without resolving it; answered while `PAUSED`, its immutable response and local outbox stay queued, with no provider `tasks/update` until explicit resume and successful continuation authorization
- UserRequest resolution is one-way from PENDING; an ANSWERED state must match the immutable response digest, responder, and timestamp, and response rows cannot be updated or deleted
- EXTERNAL_URL UserRequests require EXTERNAL_AUTHORIZATION, have no form schema/choices, expose no raw URL in projections/events, and accept only an action-only response object; extra content is rejected by storage
- credential-like MCP form fields and unknown embedded input methods fail closed without persisting a response or dispatching `tasks/update`; normal non-sensitive elicitation form input still follows the immutable response path
- external handoff requires an authenticated explicit Operator action, returns no-store data, rejects unsafe URL destinations, and never logs or replicates the state-bearing URL
- the Gateway `litecowork.user.ask` surface cannot create EXTERNAL_AUTHORIZATION requests, resolve Approvals, or turn a free-text response into SecretStore data
- after pause, a Conversation input can continue only its exact current ConversationTurn; Task-planning input requires a fresh planner on the unchanged lead/TaskSpec; Attempt input requires the same current Attempt, Runtime incarnation, and Environment with a fresh session and a strictly higher lease epoch
- provider-input delivery requires the originating AgentSession to be `CLOSED` or `LOST`; Task-planning/Attempt continuation uses a distinct active replacement session, and the Conversation turn does not replace its historical session pointer until provider acceptance
- a Task-planning continuation planner on a different Runtime is eligible only when that Runtime incarnation is current; it never transfers the provider task/input key from its original Runtime
- a replacement Attempt, Runtime, or Runtime incarnation cannot receive an old provider input key; reconcile/cancel the old Invocation and Effects first, then use the saved answer only as bounded context for newly authorized work
- a daemon restart changes RuntimeIncarnation and makes old input/task handles reconciliation-only; a synced answer stays queued until the origin Runtime can reconcile or a new invocation is explicitly admitted, and no new incarnation sends the old key
- an ambiguous provider-input retry requires an authenticated observation of the same outstanding provider key, an adapter contract proving the exact response digest duplicate-safe, and a persisted local proof digest; the retry keeps the same key/digest and increments dispatch count, while missing/stale proof is rejected
- cancelling before provider-input dispatch atomically withdraws the outbox and later delivery is impossible; cancellation after dispatch cannot rewrite it as withdrawn and waits for provider/effect reconciliation
- PlanningAssignment is a transient envelope, not persisted state; Task-planning sessions are durable AgentSessions, at most one active per Task, and close when planning waits for user input or the Task pauses/cancels. A later envelope/session pins the current TaskSpec revision
- Pause/cancel racing with plan acceptance is serialized by Task version: a plan committed first remains valid history; a transition committed first rejects the late proposal and creates no Steps
- zero live AgentSession bindings permits owned-host idle cleanup after TTL, and the stop transaction rechecks references so it cannot kill a newly acquired session; external/shared processes remain unowned
- Conversation/planning grant cannot include a SecretRef or invoke a mutation
- each CapabilityInvocation records the exact grant that authorized it
- pause and cancel remain distinct; PAUSED requires checkpoint, provider Invocation quiescence, Effect reconciliation, and lease settlement
- pause/cancel from VERIFYING fences completion; active VerificationRuns settle against pinned inputs and a late result cannot win after the Task transition
- Task cancellation remains CANCEL_REQUESTED until every Task/Attempt Invocation has terminal provider state; cancel acknowledgement alone is insufficient
- loss of provider state from WAITING/INPUT_REQUIRED records AMBIGUOUS and cannot be mistaken for a terminal result
- no old EnvironmentControlLease epoch command is accepted or replayed after takeover
- criterion changes require fresh VerificationRun even if criterion ID is reused
- a ResourceInput digest that differs from its pinned ResourceRevision digest is rejected and cannot produce passing Evidence
- ArtifactVersion input refs exactly match provenance source/transformation inputs, including linked external content refs, or publication is rejected
- one-time Approval has at most one matching ApprovalUse
- MCP task handles resume independently of LiteCowork Task identity
- loss of the initial MCP `CreateTaskResult` leaves the Invocation ambiguous and never blindly reissues the `tools/call`
- direct native tool attachment records Invocation before dispatch and enforces grant/effect/fence checks; an unmediated agent tool remains agent-reported
- notification delivery state never mutates Task state
- a Task can use an explicitly registered local intranet capability through the on-device broker, while sandbox sockets, arbitrary Agent URLs, unauthorized redirects, cloud Metadata endpoints, and cloud-runtime use of local grants remain blocked
- Resource search is deterministic, scoped, freshness-aware, and does not consume model usage
- ArtifactVersion/VerificationRun inputs pin exact Resource revisions and produce one rebuildable DependencyEdge per input
- dependency invalidation links the exact edge to the changed revision, is idempotent, and marks downstream projections stale without mutating immutable records
- a conflicted Resource revision graph never projects a dependency as current
- Workspace restore registers a new Runtime and cannot revive old fencing authority
- resumable upload duplicate chunks are idempotent only for identical bytes/digest
- native agent reported action never becomes VERIFIED without verifier evidence
- SQLite databases are never synchronized across runtimes

### Delegation, responsibility, and context invariants

- native harness files/options remain intact; unsupported overrides fail before invocation
- multiple DelegationProfiles on one AgentBinding have independent immutable revisions and
  selection constraints
- a host delegation admission targets only a READY Step in the current accepted
  PlanRevision under the current parent Attempt session/lease; no runtime-created Step
- child Attempt grants are freshly evaluated and scoped; parent grants, ApprovalUses,
  SecretLeases, and native credentials are never inherited
- profile REQUIRE selection never substitutes; PREFER may substitute only under the
  request's explicit selection semantics and policy
- a disabled/revised profile blocks future admission while existing children retain their
  pinned revision; explicit cancellation follows Effect reconciliation
- unknown cost/quota/readiness never becomes zero/available/exhausted; unknown native cost
  cannot satisfy an unenforced hard monetary ceiling
- prewarm creates no Attempt, AgentSession, model invocation, grant, lease, or lead change
- every escalation is a new Attempt with bounded recovery and a new VerificationRun
- DelegateRequest, TaskPacket, and ResultEnvelope reject unpinned Resource/Artifact
  references, oversized envelopes, extra fields, invalid profile-selection combinations,
  and fallback lists exceeding their attempt budget
- Coworker pause blocks proactive/scheduled admission but leaves existing Tasks governed
  by their own state/policy; archived Coworkers cannot remain Workspace primary
- Goal progress references accepted Task outcomes/Evidence; worker summaries cannot
  complete Goals; Suggestions cannot execute or authorize their own proposal
- Reopening a completed Goal preserves prior status events, Task state, Evidence, and
  derived progress; an archived Goal remains terminal
- Suggestion snooze respects version and expiry; mute atomically dismisses open items of
  the kind, prevents future proposals, and unmute does not revive dismissed items
- Suggestion dismissal suppresses only the exact dedupe key for 30 days; expiry and
  acceptance do not create a dismissal cooldown
- ContextDocument kind/owner pairs are validated, Resources are revisioned, and concurrent
  edits never last-write-wins
- Environment sharing scope/owner/lifetime constraints reject cross-scope attachments;
  USER_SHARED does not cross Workspace boundary in v1
- EnvironmentControlLease admits one input owner/epoch at a time and never substitutes
  for the Task ExecutionLease
- deadline preflight runs before Effects; fallback preserves operation semantics/authority;
  ActionBatch suboperations preserve individual Effect/Evidence identity
- DemonstrationSession traces are semantic, bounded, secret-redacted, and convert only to
  a reviewable SkillProposal

## Runtime, Routine and trigger conformance

- Cold boot starts authorized metadata observation and scheduler duties without launching
  installed agents, capability servers, browser or Office applications.
- Restart creates a fresh incarnation; PID reuse cannot authorize cleanup or attachment.
- A RuntimeIncarnation is registered before its presence, offers, or domain events can be
  accepted; replayed/stale state updates and altered reuse of an incarnation ID are rejected.
- Replicas retain the compact RuntimeIncarnation record needed by durable references;
  local OS boot IDs and diagnostics are absent from event state and Workspace backups.
- Durable AgentSessions expose Runtime/incarnation provenance without any host/process or
  native-session handle. A local `AgentSessionHostBinding` with the wrong Runtime,
  incarnation, endpoint, or profile is rejected; takeover on another Runtime creates a new
  session and cannot reuse the old native handle.
- AgentEndpoint records contain stable protocol/topology identity only. Reject stale
  `AgentEndpointBinding` incarnations and deletion while a host is live; verify
  Operator/event/backup state omits endpoint command paths, sockets, URLs, and auth material.
- Environment rows contain no provider locator and checkpoint rows contain no provider
  handle. Local bindings with stale incarnations or a Runtime/provider mismatch are
  rejected; provider-only checkpoint digests do not qualify as portable state. Portable
  snapshot BlobRefs must match the checkpoint digest, and their bytes are copied only when
  both Environment and Workspace policies permit it. Backup/restore omits all local
  provider bindings and requires fresh provider reattachment.
- ResourceLocation aggregate state contains only a stable non-secret resolver key; raw
  paths, connector locators, and browser handles exist only in an incarnation-scoped local
  binding. Reject a binding with a stale incarnation or a different location key, and
  verify that backup, replication, and Operator views contain no private locator values.
- Durable FileIdentity contains Runtime-keyed pseudonyms; raw OS filesystem/volume/file
  identifiers and the HMAC key remain local. A lost key lowers identity confidence and
  must not cause cross-Resource deduplication based only on matching content.
- AgentHost session-use counts are derived from nonterminal sessions joined through local
  bindings and fall once after CLOSED/LOST settlement; active bindings cannot be deleted.
- Concurrent `ensure_ready` requests share only a compatible isolated host; an idle-stop
  race cannot terminate a host whose session reference has just been acquired.
- Selecting an agent default changes configuration without spawning it. Changing the lead
  drains planning context while existing execution Attempts retain their binding.
- Runtime keeps only a lightweight, incarnation-scoped installed-application inventory warm; it launches an app only for a selected dependency, never records unrequested window/document activity, and never closes a pre-existing user process
- Multiple compatible Activations may share one declared-safe CapabilityHostInstance;
  LiteCowork's displayed `active_activation_count` is derived from live Activation references and falls
  exactly once on settlement, pause/deactivation, revocation, or Runtime recovery.
- CapabilityActivation must persist a valid CONVERSATION/TASK_PLANNING/ATTEMPT_EXECUTION
  scope. Invocation admission rejects scope, Workspace, Runtime/incarnation, or
  CapabilityRef disagreement among AgentSession, Grant, Activation, and Invocation.
- A provider host with mismatched capability/configuration digests or Trust isolation
  partition is never shared. Invocation grants/fences remain independent on shared hosts.
- Expired provider health is UNKNOWN and cannot satisfy placement; a re-probe is required.
- LiteSPM remains authoritative for provider process start/stop and global process counts;
  LiteCowork releases its own host-use references and never terminates external services.
- Replicating a durable CapabilityActivation never requires a remote Runtime to possess
  the source Runtime's HostBinding/provider handle; failover creates a new local Activation
  and host binding, and Workspace restore re-probes rather than restoring handles.
- Saving a Routine requires explicit review; cancelling a draft creates no durable job.
  Routine edits cannot alter an already pinned Task or claimed occurrence.
- Stable trigger IDs retain cursors across unrelated edits; changed sources require fresh
  identity. Host reassignment fences old observers, and duplicate delivery creates one run.
- A Hub schedule with offline local dependencies enters WAITING_DEPENDENCY without worker
  startup. Wake/reconnect reevaluates authorization and lease authority before execution.
- Missed slots, DST ambiguity, timezone changes, bounded catch-up and condition observation
  gaps obey pinned recurrence semantics without duplicate occurrences.
- Runtime drain preserves unsolved Effects and local dependencies; UI exit alone does not
  stop background operation. Only owned applications are eligible for automatic cleanup.
- Persistent Environments reattach with fresh authorization and observation after restart;
  stale handles and queued actions cannot resume authority.
- Provision preview and create bind to the same canonical request, principal, Runtime
  incarnation, pinned Resource revisions, provider offer and policy/quote basis. Any
  request mismatch, expired preview, changed input head, changed offer or restarted target
  Runtime is rejected before budget reservation/provider allocation; an idempotent replay
  returns the original result even after preview expiry.
- Persistent Environment cost ceilings are labeled provider-enforced only when the provider
  confirms enforcement. Host-monitored budgets require explicit selection and are never
  described as hard caps; unavailable estimates cannot be presented as a guaranteed cost.
- Persistent Environment usage accumulates from provision time without implicit reset. A
  Task using that Environment is checked against its Task and Environment ceilings through
  separate reservations; the same observation is not double-counted within either owner.
  Currency mismatch or an observation gap becomes UNKNOWN, and a reached ceiling stops new
  admission before safe suspension/reconciliation.
- V1 exposes no in-place Environment budget update/reset operation. At `LIMIT_REACHED`,
  affected Steps receive `BUDGET_EXCEEDED` after settlement; a replacement Environment
  requires its own preview/confirmation. Placement preview candidates are digest-bound;
  stale digest, changed Runtime incarnation, changed resource revision, or newly exhausted
  candidate creates no Attempt. A valid explicit candidate creates a new Attempt bound to
  that Environment without assuming the old provider's private state was cloned.
- `UsageObservation.confidence=UNKNOWN` requires `quantity=null`; zero is a measured amount,
  never a placeholder for missing telemetry. Exact/estimated COST observations and cost
  reservations require an explicit matching currency; unlike currencies are not summed.
- Needs You item IDs are stable across projection rebuilds and label/status edits, differ
  across distinct source records, and deduplicate a linked blocker to its Approval or
  UserRequest. Notification retry/delivery state never creates another inbox item.

These are required conformance cases, not claims that an implementation already passes.

- Stop preview is read-only; POST stop rejects an old incarnation after restart and
  rechecks work admitted after preview. `CANCEL` does not enter DRAINING.
- Routine/Automation/Occurrence/Cursor storage rejects references to another Workspace;
  trigger provenance remains mandatory in occurrence events.
- Specific-runtime triggers reject missing/null runtime IDs; resource triggers reject
  both missing targets and ambiguous simultaneous root/resource targets.
- Occurrence settlement cannot bypass Task cancellation, Effect reconciliation or
  completion evaluation; a paused Task does not produce a completed run.

- Workspace archive rejects active Conversation turns, scoped Invocations, live persistent
  Environment workloads and authority leases even when all Tasks are terminal. It stops
  observation/trigger duties and preserves suspended state without destructive cleanup.
