# Services and Ownership

This file fixes service ownership and dependency direction. Domain method/value contracts
are defined in their owner documents; implementations return typed results and do not
expose database models directly.

Every mutating service command receives the `CommandContext` defined in `SCHEMAS.md`;
the HTTP adapter maps `Idempotency-Key` and `X-Workspace-ID` into it, while local IPC
provides the equivalent fields. `expected_version` remains a command-specific
precondition. Read commands receive authenticated scope without a mutation request ID.

`OperatorTransportAdapter` owns transport framing, deadlines, peer authentication, and
mapping into the shared Operator request/response path. The target desktop implementation
uses the bounded frame definitions in the `operator-ipc` crate; OS-peer authentication must
complete before the adapter reads a body or attaches its internal authenticated-peer
marker. The adapter does not implement Workspace policy or Trust decisions. TCP/HTTP and
local IPC use separate authentication paths; the production desktop client must have no
HTTP fallback.

### WorkspaceService

```text
create(CreateWorkspaceRequest) -> Workspace
get(WorkspaceId) -> Workspace
list(WorkspaceQuery) -> Page<WorkspaceSummary>
update_replication_policy(UpdateWorkspacePolicyRequest) -> Workspace
set_default_agent_binding(WorkspaceId, AgentBindingId?, expected_version, RequestId) -> Workspace
set_primary_coworker(WorkspaceId, CoworkerId?, expected_version, RequestId) -> Workspace
archive(ArchiveWorkspaceRequest) -> Workspace
```

The local Operator create adapter requires a `RequestId`. It canonicalizes the normalized
create payload, scopes the receipt to the authenticated local Principal, and uses the
`IdempotentWorkspaceStore` port so the Workspace projection, `workspace.created.v1` event,
origin sequence, and committed response receipt share one SQLite transaction. Reusing the
same key and payload returns the original committed Workspace; reusing it with a different
payload returns `CONFLICT` and creates no second Workspace.

The selected default must be a binding in the same Workspace with both `enabled=true`
and `lead_eligible=true`; clearing it is explicit. Binding/endpoint availability is rechecked for
each new turn or Task admission, and no alternate binding is selected silently.
`set_default_agent_binding` is idempotent by authenticated Principal and RequestId. The
Workspace version, default binding, event, and original response commit atomically; the
SQLite adapter rechecks binding eligibility in the same transaction as the Workspace
write. A lost-response retry with the same key replays the committed Workspace, while a
stale new command conflicts.
The primary Coworker must belong to the Workspace and may be ACTIVE or PAUSED; this is a
UX/context default only and does not select an AgentBinding or confer authority. The
Workspace default binding and primary Coworker are independent settings.

The authenticated owner is the only Workspace principal in v1. Policy updates are prospective; they do not erase already replicated data. Archive requires all Tasks to be terminal and all Automations disabled, then makes the Workspace read-only. Quiescence also requires Conversation turns and scoped Invocations to be settled, no active grants/SecretLeases/control leases, and persistent Environments with no live workload. Authorized watchers/triggers stop before the read-only transition. Retained Environment state may remain suspended under storage/backup policy; archive never silently destroys it. Unknown provider quiescence blocks archive with `WORKSPACE_NOT_QUIESCENT`. The service rejects every domain mutation for archived Workspaces, including Task changes, capability grants/activation, Artifact/Library changes, connection changes, Runtime pairing, Automation occurrences, and inbound channel work. Authorized reads and Artifact/Resource downloads remain available.

### BackupService

```text
create_workspace_backup(WorkspaceId) -> WorkspaceBackupManifest
list_workspace_backups(WorkspaceId, Cursor?, Limit) -> Page<WorkspaceBackupManifest>
get_backup(WorkspaceId, BackupId) -> WorkspaceBackupManifest
restore_backup(RestoreWorkspaceBackupRequest) -> RestoreReceipt
```

Creates a consistent snapshot and publishes an immutable manifest only after all required
objects verify. Restore is available only through a locally authenticated recovery
surface on a new installation; it restores Workspace state and the included event history
through the manifest cursors, rebuilds projections, and registers a new Runtime identity.
It never restores old lease/fencing authority or secret bytes. Missing encryption keys or required
blobs fail restore before the target becomes writable.

Backup manifests are recovery-control metadata local to the backup target, not replicated
Workspace domain events. The create/restore request-dedup result and AuditRecord commit
with their respective control metadata; failed capture creates no restorable manifest.

The authenticated principal and request ID come from `CommandContext`. The
`RestoreWorkspaceBackupRequest` body contains `confirm_empty_installation: true`; the
backup ID is in the recovery route and the request is accepted only before the local
installation has a writable Workspace. Restore returns a non-persistent receipt with the
restored Workspace ID, new Runtime ID, restored cursors, and Connections that require
reauthentication.

### RuntimeWorkspaceBindingService

```text
list(WorkspaceId, Principal, Cursor?, Limit) -> Page<RuntimeWorkspaceBinding>
enroll_local(WorkspaceId, Principal, expected_workspace_version, RequestId) -> RuntimeWorkspaceBinding
activate_mesh_pairing(PairingReceipt) -> RuntimeWorkspaceBinding
revoke(WorkspaceId, BindingId, Principal, expected_binding_version, RequestId) -> RuntimeWorkspaceBinding
require_active(WorkspaceId, RuntimeId, RequiredRuntimeRole) -> RuntimeWorkspaceBinding
```

This service owns Workspace-scoped Runtime authorization. Local enrollment binds the
current installation to a locally owned Workspace but never registers Mesh presence.
Pairing activation accepts only an authenticated, single-use Hub receipt. `require_active`
is called before Workspace-scoped execution, resource-root access, replication, or
Runtime-host assignment; TrustService still checks the caller and action. Revoking one
binding stops new admissions and Workspace replication for that Runtime/Workspace pair,
but is admitted only after clearing a Mesh hub pointer and draining ChannelHost leases,
enabled TriggerHost cursors, and nonterminal AutomationOccurrences pinned to that
TriggerHost. Disabling an Automation prevents new occurrences but does not settle existing
ones. It then requests ordinary lease/Effect reconciliation for
active Task work. Installation-wide device revocation remains RuntimeMesh-owned. Binding
records and decisions are retained
as security-control metadata/AuditRecords, not Task-domain events.
Before revocation, this service asks RuntimeMesh to atomically move each ChannelHost to an
eligible target or safely clear its assignment when no target is available. A cleared host
leaves its ChannelBinding DEGRADED/unassigned and unable to accept or send events until an
owner assigns a new host. Revocation waits if the source cannot prove quiescence or reach
the authoritative expiry-plus-skew boundary. It never revokes first and attempts to repair
ChannelHost ownership later.

## Public application ports

### ConversationService

```text
create_conversation(CreateConversationRequest) -> Conversation
append_message(AppendMessageRequest) -> ConversationMessage
submit_turn(SubmitConversationTurnRequest) -> ConversationTurnReceipt
publish_rich_presentation(PreparedRichPresentation) -> RichPresentation
retry_turn(turn_id, expected_version, request_id) -> ConversationTurnReceipt
cancel_turn(turn_id, expected_version, request_id) -> ConversationTurnReceipt
get_conversation(ConversationId) -> ConversationView
list_messages(ConversationId, Cursor?, Limit) -> Page<ConversationMessage>
map_channel_thread(MapChannelThreadRequest) -> ConversationId
create_user_request(CreateUserRequest) -> UserRequest
resolve_user_request(ResolveUserRequest) -> UserRequestResponse
open_external_handoff(UserRequestId, Principal, expected_version) -> ExternalAuthHandoff
authorize_input_continuation(InvocationId, UserRequestId, expected_turn_version) -> InvocationContinuationAuthorization | Denied
```

Owns Conversation, ConversationMessage, and channel-thread mapping. Append and
Task-materialization requests originating from one user message use one transaction so
the Task cannot exist without its source message or vice versa.
`submit_turn` first resolves and rechecks an enabled compatible AgentBinding/endpoint. If
none is eligible, it returns `AGENT_UNAVAILABLE` before persisting a message or turn, so
the Operator can preserve the unsent draft for setup. Once admitted, it atomically persists
the user message and ConversationTurn before asynchronously starting the selected binding.
An adapter failure after commit settles the durable turn as FAILED; it does not erase the
user message. Agent responses carry AgentSession/AgentBinding provenance.
The AgentTurnCoordinator uses a Conversation-scoped session when no Task is materialized.
`retry_turn(turn_id, expected_version, request_id)` is allowed only for a FAILED turn; it
creates a fresh session, increments `retry_ordinal`, and leaves earlier response messages
and their provenance intact.
`publish_rich_presentation` is an optional post-message commit. It accepts only a
compiler-prepared, digest-verified document bound to an existing same-Workspace AGENT
message from a settled/committed turn. It revalidates the message digest, source identities,
schema/size bound, uniqueness, and presentation policy, then atomically inserts the
immutable RichPresentation aggregate, snapshot, event, and blob reference. Failure does
not alter the ConversationMessage or turn outcome.
`cancel_turn(turn_id, expected_version, request_id)` requests adapter interruption and
settles only after stop is observed; a completion/failure that wins the stop race remains
the recorded outcome.

ConversationService owns structured UserRequest creation/resolution. A response is
append-only, validated against the request schema, and cannot stand in for an Approval.
Only the trusted InvocationRunner may materialize `EXTERNAL_AUTHORIZATION` requests from
supported provider URL elicitation. Agent Gateway `user.ask` can create only ordinary
non-sensitive form requests. `open_external_handoff` verifies owner identity, pending
request/version, the source Runtime binding, HTTPS syntax and destination display policy,
then returns the confidential URL with no-store semantics for one explicit user action; it does not
write the URL to an event or projection. Ordinary response validation never accepts a
credential as an authorized SecretStore write.
For TASK_PLANNING or ATTEMPT_EXECUTION scope, response admission checks the parent Task is
in a response-eligible state in the same authoritative transaction. `PAUSE_REQUESTED`
returns `CONFLICT` and leaves the UserRequest pending; after the Task reaches `PAUSED`, an
answer may be stored immutably, but its provider-input delivery is queued and cannot start
work until explicit Task resume revalidates authority. TrustService may record an Approval
decision while paused, but cannot create its `ApprovalUse` or authorize an Effect/grant
until resume. A Task cancellation that commits first rejects the response and closes
pending Task-scoped requests. It atomically withdraws any undispatched local
ProviderInputBinding (`AWAITING_RESPONSE` or `PENDING`); a dispatched/ambiguous input is
reconciled and cannot be declared cancelled by changing the local outbox state.
Conversation-scoped requests remain bound to their exact
ConversationTurn and do not inherit Task pause/cancellation.
Resolving a Conversation-scoped UserRequest atomically records the response and moves its
exact still-WAITING_USER turn to `WAITING_DEPENDENCY` before enqueuing continuation.
AgentTurnCoordinator admits a fresh session only after any linked provider input is
confirmed accepted (an MCP `tasks/update` acknowledgement alone is insufficient) and
records `conversation.turn.resumed.v1` only after the new session is ready. Repeating the
same response request is idempotent; conflicting response reuse is rejected. A provider
rejection requiring new input creates a new UserRequest and returns the turn to
`WAITING_USER`. If bounded reconciliation or session startup fails, the response remains
durable and the turn settles FAILED with a retryable reason; the turn never resumes before
the exact provider request and ConversationTurn linkage are confirmed.

### AgentTurnCoordinator

```text
continue_from_user_request(UserRequestId, UserRequestResponseId) -> ConversationTurnReceipt
```

Consumes a committed ConversationTurn dispatch request, waits for any provider-input
dependency to become ready, asks AgentSessionSupervisor to start a fresh
Conversation-scoped AgentSession, normalizes its outcome stream,
and returns lifecycle outcomes to ConversationService. It does not write Conversation/Turn
aggregates or append messages directly. Retries always use a fresh AgentSession and
preserve earlier message provenance. On final response, failure, cancellation, or a
structured user-input wait, it observes adapter quiescence and closes the session before
releasing the host-use reference. A user response continues the same durable turn through a
new session and bounded Conversation projection. It resumes only the still-waiting turn
linked to the resolved UserRequest; stale or cancelled requests are rejected.

### TaskService

Defined in `TASK-RUNTIME.md`. Owns Task, TaskSpecRevision, PlanRevision, Step and high-level Attempt orchestration.

### AgentAdapter

Defined in `AGENT-FABRIC.md`. Protocol boundary to external agents.

### AgentBindingService

```text
list_profiles(WorkspaceId, RuntimeId?, Cursor?, Limit) -> Page<AgentProfileView>
list_bindings(WorkspaceId, AgentBindingQuery, Cursor?, Limit) -> Page<AgentBindingView>
get_binding(AgentBindingId) -> AgentBindingView
create_binding(CreateAgentBindingRequest) -> AgentBinding
enable_binding(AgentBindingId, expected_version) -> AgentBinding
disable_binding(AgentBindingId, expected_version) -> AgentBinding
set_lead_eligibility(AgentBindingId, bool, expected_version) -> AgentBinding
```

Profiles are Runtime-discovered inventory; bindings are durable Workspace authorization
records. Creation requires a currently observed profile and starts disabled. Enablement
checks TrustService and Runtime availability. Disablement blocks new admission without
rewriting already admitted Attempts or sessions.
Creation persists an `EndpointSelectionPolicy`; omission selects `AUTO_COMPATIBLE` with
no required features or topology preference. A pinned endpoint must belong to the profile,
remain unexpired, and satisfy all required features when a session starts.
Disabling a binding or removing its lead eligibility while it is currently the Workspace
default is rejected with `CONFLICT` until the owner explicitly clears or changes that
default. The default is therefore never left pointing at a non-lead or disabled binding.
Runtime or endpoint unavailability does not disable
the binding, but admission rechecks availability and returns `AGENT_UNAVAILABLE`.

### DelegationProfileService

```text
list_profiles(WorkspaceId, AgentBindingId?) -> Page<DelegationProfileView>
get_profile(DelegationProfileId) -> DelegationProfileView
list_revisions(DelegationProfileId, Cursor?, Limit) -> Page<DelegationProfileRevisionView>
create_profile(CreateDelegationProfileRequest) -> DelegationProfile
revise_profile(ReviseDelegationProfileRequest) -> DelegationProfile
duplicate_profile(source_id, new_name, expected_version, RequestId, Principal) -> DelegationProfile
set_profile_status(SetDelegationProfileStatusRequest) -> DelegationProfile
```

Owns worker profiles and immutable revisions. Creation requires an enabled same-Workspace
AgentBinding, a valid adapter-discovered option schema, and a disabled initial status.
It trims and NFC-normalizes names, derives a Unicode case-folded uniqueness key, and
rejects duplicate non-archived names within the binding with `CONFLICT`.
Enablement rechecks the binding, required features, enforced policy, and current revision.
Revision creation atomically updates the head name and current revision. Names are
trimmed using Unicode whitespace rules, normalized with v1 key algorithm
`NFC(NFC(trim(name)).casefold())`, and stored on head/revision.
Name uniqueness is per binding for every non-archived profile.
Duplication requires a non-archived source and a matching `If-Match`, then copies its
current non-secret revision into revision 1 of a new disabled profile on the same binding.
Idempotency lookup for the same principal/RequestId/request digest precedes rechecking the
source `expected_version`, so a retry returns the committed result. It copies no runtime
or authority state. Revising appends a revision; active Attempts keep their pinned revision. Disabling or
archiving prevents future admission and does not cancel already admitted children.
Archive is terminal. This service never starts a host or invokes a model.

### DelegationCoordinator and WorkerSelectionService

```text
delegate(DelegateRequest) -> ChildAttemptRef
eligible_candidates(WorkerRequirement) -> CandidateSet
```

These are internal Task Runtime collaborators, not separately deployed microservices.
WorkerSelectionService filters by hard eligibility first, then deterministically ranks
the eligible candidates using the requested optimization policy and recorded
observations. It preserves the candidate-set digest for audit without including prompts
or provider secrets. DelegationCoordinator asks TaskService to admit a child only for a
READY Step in the current accepted PlanRevision, then obtains a new AgentSession,
child-scoped grants, budget reservation, Environment, and lease. TaskService commits the
Attempt and admission provenance only after mutable versions and fences are rechecked;
adapter/provider calls happen after commit. If the selected profile becomes ineligible,
admission returns a typed error. It never silently substitutes another profile.

The coordinator enforces profile concurrency, Task fanout, ancestry depth, budget
ceilings, isolation, and bounded escalation. A fallback creates a new Step Attempt via
the same admission path. It cannot create arbitrary Steps, inherit parent grants,
Approvals, or SecretLeases, mark verification passed, or mutate an accepted PlanRevision.
A lead proposes plan changes only through TaskService.

### Warm lifecycle ownership

There is no cross-domain WarmthManager. AgentHostSupervisor owns agent-process reuse;
AgentSessionSupervisor owns native-session reuse; CapabilityHostSupervisor owns
capability-host use; EnvironmentManager owns Environment retention; the local model
backend owns model memory. Each owner interprets its axis of `WarmPolicy`, reports
observed readiness, and may reject prewarm under resource pressure. Warm state is never
sufficient for admission without fresh auth, configuration, Runtime-incarnation,
Environment, Trust, and lease checks.

### CoworkerService, GoalService, and SuggestionService

```text
CoworkerService.create/revise/set_status -> Coworker state
WorkspaceService.set_primary_coworker -> Workspace state
GoalService.create/revise/set_status -> Goal revisions and status
SuggestionService.propose/accept/dismiss/snooze/expire -> Suggestion lifecycle
SuggestionService.set_kind_preference -> Workspace-scoped preference and atomic mute cleanup
```

These services own only their aggregates and projections. WorkspaceService owns the
primary Coworker reference and clears/changes it before the target Coworker can archive.
CoworkerService pins Coworker
revision provenance during Task creation but does not own Task execution. GoalService
validates same-Workspace Tasks, Routine revisions, and exact ArtifactVersionRefs. Link and
unlink are ordinary immutable revisions, not a separate command; Artifact links pin an
existing exact version. A read-only GoalProgressProjector assembles current linked Task
status, committed Task Evidence IDs, and resolvable Evidence references from exact pinned
Artifact versions. It reports unavailable VerificationRun/dependency-freshness dimensions
explicitly and never infers a verified outcome from Task status alone. Only an owner command
changes Goal completion status. Mutating an archived
Goal returns `GOAL_ARCHIVED`. SuggestionService validates exact same-Workspace pinned
Resource/Goal provenance, deduplicates open suggestions, enforces muted kinds and 30-day exact-key
dismissal cooldown, expires by injected Clock, and atomically converts an accepted Task
proposal into an ordinary Task. Snooze is an optimistic, expiry-bounded visibility update.
Muting a kind atomically updates Workspace preference and dismisses current proposals of
that kind with `MUTED_KIND`. It never issues authority or bypasses Task admission and
Trust. A Routine/Automation proposal opens its existing editor; the owner must save there.
Proposal admission returns `SUPPRESSED_MUTED` or `SUPPRESSED_COOLDOWN` without persisting
candidate content or a Suggestion event; Settings can explain the active kind preference.

TaskService accepts a Coworker origin only from the same Workspace and pins the selected
CoworkerRevision atomically with Task creation. An explicit owner command may originate a
Task from an ACTIVE or PAUSED Coworker; proactive and scheduled admission requires ACTIVE.
Archived Coworkers are rejected for new Task origin. Existing Tasks retain their pinned
origin and are not stopped by pause/archive.

`LeadFailoverService` watches only typed provider/Runtime observations and the current
TaskSpecRevision. It admits no change under `DISABLED`; under `ASK` it opens a Needs You
decision; under `ALLOW_LISTED` it tests ordered fallback bindings against current
Workspace, lead eligibility, endpoint, Runtime/incarnation, auth, Trust, resource,
deadline, and budget state. The service asks TaskService to fence new admissions from the
old lead, settle old planning work, build a bounded handoff from durable Task state, and
create a fresh planning AgentSession. Each successful lead change is a Task event with
cause and trigger provenance. It never transfers Grants, Approvals, SecretLeases, native
session handles, or existing Attempt ownership.

```text
evaluate(TaskId, LeadFailoverTriggerObservation) -> NO_ACTION | NEEDS_OWNER | Candidate
request_owner_decision(TaskId, trigger, RequestId) -> UserRequest
admit(TaskId, candidate_binding_id, expected_task_version) -> LeadChangeReceipt
```

```text
interface SuggestionProducer {
  service_ref() -> ServiceRef
  evaluate(trigger: SuggestionTrigger, context: BoundedSuggestionContext) -> SuggestionCandidate[]
}
```

Only registered producers may supply candidates. Producers are deterministic event rules
or separately authorized read-only capabilities; they cannot commit Suggestions or Tasks.
SuggestionService validates their identity, exact source revisions, visibility, scope,
and admission policy, then records `proposed_by` on accepted proposals. Suppressed
candidates retain no content.

`ProjectionService` exposes `task_progress(TaskId)`,
`coworker_presence(CoworkerId)`, `goal_progress(GoalId)`, and
`worker_performance(DelegationProfileId, TaskCategory)`. Each is rebuilt from committed
domain state and eligible fresh observations, reports its computation time, and exposes
source references where needed. A stale/missing observation remains unknown. Projection
updates do not append domain events or change aggregate versions.

`AgentHostSupervisor.quota_observation(AgentBindingId)` returns a time-bounded
adapter/provider observation or no observation. Expired values project as `UNKNOWN`; this
read path does not probe by spending model tokens. Quota remains a routing hint and never
authorizes a lead switch.

### DemonstrationSessionService and PersonalContextService

DemonstrationSessionService captures bounded semantic observations in an Environment and
persists the trace as a Resource; conversion creates a SkillProposal through the existing
proposal/review path. It stores neither raw credentials nor coordinate-only authority.
PersonalContextService is a Core authorization/provenance adapter over user-authored
ContextDocument Resources and optional PersonalContextProvider capabilities. Retrieval
and indexing remain provider-owned; revocation, deletion, and source scope remain
Core-owned.

`PersonalContextProvider` v1 does not expose `propose_memory`; derived memory proposals
remain deferred until Suggestion/Needs You review can pin content, sources, expiry,
redaction, and owner resolution. `ResourceService.revise_context_document` uses the normal
Resource revision DAG. The current local V1 implementation of
`set_context_document_status` accepts only `ACTIVE -> REVOKED` and `REVOKED -> ACTIVE`;
it atomically commits the Resource metadata/version, aggregate snapshot, status event, and
idempotency receipt. It does not invalidate content already delivered to an agent session,
and no session-revocation integration is currently wired. Deletion remains a separate
target contract: the purge worker first seals a `ContextDocumentPurgePlan` over the exact registered Core-managed
blob/index replicas, then marks `DELETION_PENDING` and creates one pending receipt per
target in the same transaction. It marks `DELETED` only after the acknowledged receipt set
equals that plan, including a verified empty plan. A provider-backed replica remains
pending until its registered adapter confirms removal; unrelated context providers do not
become deletion authorities.

```text
ResourceService.create_revision_upload(ResourceId, parent_revision_ids, if_match, RequestId) -> ResourceUploadSession
ResourceService.commit_revision_upload(ResourceUploadId, RequestId) -> ResourceRevision
ResourceService.set_context_document_status(ResourceId, ACTIVE|REVOKED, if_match, RequestId) -> Resource
ContextDocumentPurgeReconciler.seal_plan(ResourceId, exact_targets, if_match, RequestId) -> ContextDocumentPurgePlan
ContextDocumentPurgeReconciler.record_ack(ResourceId, replica_ref, RuntimeIncarnationId, receipt_digest) -> PurgeStatus
ContextDocumentPurgeReconciler.complete(ResourceId, expected_version) -> Resource
```

Revision uploads enforce Workspace/Resource ownership, current version, exact parent
revision ancestry, byte/digest integrity, and atomic head update. Content resolution rejects
revoked or deletion-pending ContextDocuments. The target session contract invalidates active
context attachments; the Session owner stops or replaces a session at a safe boundary and
cannot reuse its old native history. That session behavior is not implemented by the current
local V1 status route. Purge receipts are durable, non-secret recovery records.
The purge plan is immutable, pins a canonical digest and target count, and lists exact
revision IDs for each target. Receipt admission must match one plan target exactly, and a
duplicate or extra acknowledgement is rejected. Only the reconciler can transition
`DELETION_PENDING` to `DELETED`, after every required Core-managed replica has acknowledged
the tombstone for the current Runtime incarnation.

### WarmHoldService

```text
prewarm(target, reason, priority, ttl_ms) -> WarmHold
release(warm_hold_id) -> Ack
reconcile_runtime_incarnation(RuntimeIncarnationId) -> ReleaseResult
```

This is Runtime-local operational state. It asks the owning host/Environment supervisor to
retain a readiness optimization until expiry; it does not launch model work or create
Task/Attempt/session/Trust authority. Hold creation and release are not replicated domain
events. The owner may refuse or evict a hold under resource pressure; callers then receive
ordinary cold-start latency.

### CapabilityBroker

Defined in `CAPABILITY-FABRIC.md`. Owns scope-aware capability resolution, grants,
activation, and invocation coordination for Conversations, Task planning, and Attempts.

### CapabilityHostSupervisor

```text
ensure_ready(ActivationHostRequest) -> CapabilityHostInstance
retain_activation(CapabilityHostInstanceId, CapabilityActivationId) -> HostUseRef
release_activation(CapabilityHostInstanceId, CapabilityActivationId) -> HostUseRef
list_runtime_hosts(RuntimeId, cursor?, limit?) -> CapabilityHostInstancePage
reconcile_incarnation(RuntimeIncarnationId) -> ReconciliationResult
```

Owns LiteCowork's normalized Runtime-local view and ActivationHostBindings for shared
capability provider instances. It derives active use count from local bindings joined to
authoritative nonterminal Activations. A nonterminal CapabilityInvocation keeps its
Activation nonterminal and retained after its AgentSession closes; release requires both
scope settlement and terminal/reconciled Invocations. It serializes concurrent readiness
requests and permits sharing only
when concurrency, pinned capability/configuration identity, and Trust isolation rules
agree. Each Invocation still authorizes against its own Grant and fence. It does not
install packages, spawn/kill provider processes, or invent LiteSPM package behavior. It
uses the internal LiteSPM adapter boundary for opaque readiness/use-reference operations;
exact LiteSPM wire methods remain deferred. `provider_instance_ref` is Runtime-private and
never exposed as a credential or public API locator. If an observation expires or the
Runtime incarnation changes, the view is stale and the host must be revalidated before
reuse.

### ProviderCircuitService

Owns per-Runtime provider failure windows and `CLOSED/OPEN/HALF_OPEN` call-admission
circuit transitions. It does not start, restart, stop, share, or remove provider processes;
LiteSPM owns package/provider process lifecycle, and Environment/Channel owners manage
their own adapters. Provider health observations feed the host view and may open this
circuit, but a circuit state is not itself proof that a process stopped. The service does
not choose a semantic fallback; CapabilityBroker or the owning provider service must
re-check compatibility and authorization before selecting another provider.

### RuntimeLifecycleService

```text
get_local_status() -> RuntimeLifecycleView
register_local_startup(DeviceIdentity, RuntimeId, RuntimeIncarnation, LocalObservation) -> RuntimeIncarnation
set_startup_policy(UpdateRuntimeStartupPolicyRequest) -> RuntimeLifecycleView
preview_stop() -> RuntimeStopPreview
request_stop(StopRuntimeRequest) -> RuntimeDrainReceipt
recover(RecoveryTrigger) -> RuntimeIncarnation
on_suspend(SuspendObservation) -> RuntimeLifecycleView
on_resume(ResumeObservation) -> RuntimeLifecycleView
```

Owns the local daemon's startup policy, incarnation creation, boot recovery gates, safe
drain, and OS resume coordination. An OS service-manager adapter starts/stops the daemon;
the Operator window is never the service. Startup recovery does not launch worker
processes. After StateStore opens, local startup atomically stores the public Runtime
descriptor, fresh incarnation, and non-secret observation. Ed25519 private material stays
in the OS credential store. The local catalog write does not create a Workspace binding,
pair with Mesh, or publish presence. Startup remains RECOVERING then DEGRADED until Task
recovery and admission gates exist. `preview_stop` reports dependencies without changing admission. `request_stop` validates
the expected incarnation and rechecks dependencies before accepting drain;
only a local authenticated Operator may request it.

### AgentHostSupervisor

```text
ensure_ready(AgentHostRequest) -> AgentHostInstance
retain_session(AgentHostInstanceId, AgentSessionId) -> HostUseRef
release_session(AgentHostInstanceId, AgentSessionId) -> HostUseRef
set_warm_policy(UpdateWarmPolicyRequest) -> WarmPolicy
reconcile_incarnation(RuntimeIncarnationId) -> ReconciliationResult
```

Starts/attaches an AgentEndpoint only for an admitted Conversation turn, planning session,
or Attempt. It checks process identity across RuntimeIncarnations, tracks session-use
references through local immutable `AgentSessionHostBinding` rows, and derives its live
session count from those rows joined to nonterminal durable AgentSessions. The binding
stores opaque adapter-native session/resume handles and is excluded from replication and
backup. It idles only LiteCowork-owned local processes after their bounded TTL.
Remote API/A2A endpoints need no local process; external/shared processes are never killed
by idle cleanup. `AgentSessionSupervisor` delegates host readiness and use-reference
management here before creating an ACTIVE session.

### ExecutionDependencyPlanner

```text
preview_step(TaskId, StepId) -> ExecutionDependencyPlan
evaluate(ExecutionDependencyRequest) -> ExecutionDependencyPlan
prepare(ExecutionDependencyPlanId, expected_digest) -> DependencyPreparationResult
release(AttemptId) -> DependencyReleaseResult
```

Builds a short-lived, read-only prerequisite DAG for placement: Runtime role/capacity,
Resource location/exposure, AgentHost, Environment, CapabilityActivation, SecretLease,
and required application attachment. It may start independent prerequisites after Task
admission and authorization, but it does not create semantic Steps or choose strategy.
It recomputes after a material dependency or RuntimeIncarnation change. AttemptRunner may
start an Attempt only after mandatory prerequisites are ready and current fences pass.
The Operator-facing preview is read-only and lists existing eligible Environment
candidates with blocker/readiness reasons. A recovery override is accepted only for a
candidate in the exact current unexpired preview digest; the planner and TaskService
recheck all hard constraints before the new Attempt commits.

### RuntimeMesh

Defined in `RUNTIME-MESH.md`. Owns runtime identity/presence/replication, execution and
channel-host leases, channel-host assignment, and handoff.

```text
assign_channel_host(AssignChannelHostRequest) -> ChannelHostLeaseGrant
renew_channel_host_lease(RenewChannelHostLeaseRequest) -> ChannelHostLeaseGrant
record_channel_host_drain_proof(RecordChannelHostDrainProofRequest) -> ChannelHostDrainProof
release_channel_host_lease(ReleaseChannelHostLeaseRequest) -> ChannelHostLeaseReleaseRecord
clear_channel_host_for_binding_revocation(ChannelBindingId, source_runtime_id,
  expected_assignment_version, RequestId) -> ChannelHostAssignmentCleared
```

`assign_channel_host` and `clear_channel_host_for_binding_revocation` are transaction-level
operations, not sequences the caller may partially commit. Every committed ACTIVE
assignment has exactly one matching current lease. A DRAINING assignment retains its
matching lease until the release transaction also commits either the next ACTIVE assignment
and its fresh lease or the cleared/unassigned result. A source receipt-insert transaction
and the ACTIVE→DRAINING transition serialize on the ChannelBinding authority row: insertion
requires the current ACTIVE assignment, unexpired lease, and active CHANNEL_HOST binding;
the ingress cursor/sequence and durable receipt are committed together. Once draining wins,
the source does not insert or acknowledge/defer-ack new events. Drain-proof admission uses
the same serialization point and requires no source-epoch PROCESSING claims, every accepted
pre-drain RECEIVED row Hub-durable and included in the successor replay frontier, outbound
Effects reconciled, and committed ingress Hub-durable. The no-target clear path first
releases the exact old lease and removes the assignment while preserving receipts, reply
targets, and release history; ChannelService marks the ChannelBinding DEGRADED until a new
host assignment is explicitly created. RuntimeWorkspaceBindingService calls this operation
before revoking a binding and cannot revoke if release/clear remains unresolved. The clear
operation is idempotent by RequestId and expected assignment version. The public
host-assignment route remains the owner-requested move path; clear is internal to Runtime
revocation, not an unauthenticated deletion API.

These are required transaction invariants, not guarantees supplied by the current v4 SQL
triggers: v4 does not guard receipt INSERT against DRAINING and has no commit-time
assignment-to-lease cardinality check. RuntimeMesh/ChannelService must enforce them in one
Hub transaction; production admission is blocked until storage-level guards and migration
preflight cover the same conditions. The v1→v4 migration must reject ACTIVE assignments
without a matching unexpired lease and DRAINING assignments without a matching persisted
lease row rather than inventing release provenance. The DRAINING lease row may be expired;
it is retained only as provenance while the v4 skew boundary is backfilled and grants no
authority.

Drain-proof and continuity-proof creation is a RuntimeMesh-only service command. It reads
and validates the authoritative Hub receipt/Effect/replication frontier, source lease or
release record, target binding, and provider cursor boundary in the same serialization
boundary as ingress and release. Caller-supplied counters, verifier JSON, or digests are
not accepted as proof. SQL provides immutable scoped storage, not proof authentication.

Every host epoch receives a globally unused opaque lease ID. `CredentialIssuer` derives a
fresh domain-separated HMAC credential from Workspace, ChannelBinding, Runtime, host epoch,
and that lease ID. Lease IDs are checked against current leases and immutable release
records before issuance; a duplicate ID fails closed. Distinct lease identities yield
collision-resistant credentials; any digest collision observed by the issuer aborts the
assignment transaction. The release record retains the lease ID, not a historical digest,
so digest uniqueness is an issuer cryptographic guarantee rather than a SQLite unique
constraint. Retries
with the same RequestId and request digest return the original grant, while conflicting
reuse is rejected.

`ChannelHostAssignmentCleared` is an internal command result containing the ChannelBindingId,
released source host epoch, immutable release-record identity, `assignment_state=UNASSIGNED`,
and `channel_binding_status=DEGRADED`; it is not a persisted aggregate or public API schema.

### EnvironmentProvider

Defined in `ENVIRONMENTS.md`. Provider boundary; EnvironmentManager owns canonical environment records.

### WorldIndexer / ResourceService

WorldIndexer observes only explicit WorkspaceRoots and authorized provider locations.
ResourceService owns stable Resource identity, revisions, locations, root grants,
resolution, and deterministic search projections. ArtifactStore owns creation of the
`ARTIFACT` Resource paired with each ArtifactVersion through the Resource-domain-owned
`ResourceAggregatePort`; ResourceService indexes these records and resolves bytes through
registered `ResourceLocationProvider` adapters. ArtifactStore supplies one such adapter.
WorldIndexer does not watch ArtifactStore paths. Neither service performs whole-machine
scans or semantic retrieval.

```text
add_root(AddWorkspaceRootRequest) -> WorkspaceRoot
list_roots(WorkspaceId, Status?, Cursor?, Limit) -> Page<WorkspaceRoot>
update_root(UpdateWorkspaceRootRequest) -> WorkspaceRoot
pause_root(WorkspaceRootId, expected_version, idempotency_key) -> WorkspaceRoot
resume_root(WorkspaceRootId, expected_version, idempotency_key) -> WorkspaceRoot
revoke_root(WorkspaceRootId, expected_version) -> WorkspaceRoot
search_resources(ResourceSearchRequest) -> Page<ResourceSearchResult>
rebuild_local_text_index(WorkspaceId, ResourceId, ResourceRevisionId, Sha256Digest, RequestId) -> ResourceTextIndexRebuildResult
list_resource_revisions(ResourceId, Cursor?, Limit) -> Page<ResourceRevisionView>
resolve_resource(ResourceRef, ResolutionPolicy) -> ResolvedResource
```

The owner-triggered rebuild is limited to the current managed Resource head and the
existing deterministic lexical parser. It uses a bounded verified source read, exact
revision/digest compare-and-swap, and atomic projection plus request-receipt commit. Its
typed result distinguishes `INDEXED` from `NOT_INDEXABLE`; it is not a semantic RAG or
provider embedding operation and does not change Resource history.

The current desktop implementation work places root registration and lifecycle operations in
`domain-workspace::WorkspaceRootService` behind `WorkspaceRootStore`; this is an internal
composition of ResourceService ownership, not a second architecture-level resource owner.
The SQLite operation atomically creates the folder Resource, observed location, private
current-incarnation locator binding, root projection, events, and idempotency receipt. It
is exposed only through the Tauri native folder chooser and private authenticated local
IPC operation; the path does not enter WebView or public Operator payloads. The local
Operator also has an owner-scoped paged root-list projection. Current source implements
expected-version/idempotent pause, resume, and revoke transitions. Pause preserves local
identity bindings and the selected-root replication preference. Resume requires an
AVAILABLE location and matching local identity bindings for the current Runtime
incarnation; startup revalidation is the operation that establishes those bindings.
Revocation appends the status event and atomically removes local locator and raw
file-identity bindings plus the selected-root replication relation. It retains
already-transferred remote copies. These source paths are unbuilt, untested, and
OS-unqualified. Watching, indexing, and freshness/invalidation projections remain
unimplemented. It does not make one-time folder-upload provenance a filesystem grant.

The current desktop content-search increment is an explicit `ON_DEMAND_CONTENT` mode on
the authenticated Operator search route. It scans only current managed encrypted-blob
Resources, at most 20 candidates, 1 MiB per Resource and 8 MiB total per request. Its
allowlist is plain UTF-8 text; ZIP members, rich-document parsers, WorkspaceRoots and
semantic retrieval are out of scope. The scan decrypts and matches in request memory and
returns only a bounded snippet with an exact revision-pinned ResourceRef. It does not
persist extracted text, terms, or snippets. Metadata-only mode remains the default. This
path is deliberately not described as an index and provides no durable indexing status,
background indexing job, or embedding/RAG service.

Resource revisions form a per-Resource DAG. The service validates parent ownership and
acyclicity, derives graph heads, and updates `current_revision_id` only when there is one
head. An unpinned reference to a multi-head Resource returns `RESOURCE_CONFLICT`; callers
can inspect revisions and explicitly pin a branch. No timestamp-based conflict winner is
chosen.

The local desktop quick-import adapter currently has a narrow `create_uploaded_resource`
path for a single verified byte object: it records weak upload identity, an initial
ResourceRevision and encrypted-blob ResourceLocation. It is not a replacement for
ResourceUploadService's resumable protocol and does not extract archives, create roots,
index content, or expose bytes to a renderer.

The local Operator router mounts a read-only ZIP-intake readiness observation at
`GET /v1/capabilities/zip-intake`, and the Tauri command forwards it to the Library. It is
owner/Workspace scoped and reports `UNAVAILABLE / ISOLATED_WORKER_NOT_QUALIFIED`; its
purpose is to prevent the UI from implying ZIP extraction is available. It does not invoke
`capabilities/zip_intake`, read a Resource, create an Invocation, or change persisted state.
The Python provider source is not a production provider: it has no supervised IPC/process
boundary that enforces hard CPU, memory, wall-clock, and output limits. Until that boundary
is qualified, ZIP upload stores an opaque Resource, and no archive member is available for
indexing, Task context, or Artifact publication. The desktop notice must state this
limitation plainly.

### ResourceUploadService

Owns bounded resumable upload sessions and immutable chunk receipts. Chunk acceptance
validates expected offset, exact range, idempotency, and SHA-256; it persists transfer
metadata without replicating chunk events. Session creation and lifecycle transitions are
domain events. Commit verifies contiguous coverage, total size, media policy, and the
whole-object digest before creating a Resource/ResourceRevision.

The authenticated local Operator's `POST /v1/resources/uploads` route delegates initial
session creation to `ResourceUploadService::create_session`; the service rechecks owner and
ACTIVE Workspace state, validates upload metadata and folder-path provenance, and accepts
only Context Document ownership metadata matching the authenticated Principal or selected
Workspace. The folder path is never resolved as a filesystem grant. Coworker/Goal-owned
Context Documents remain rejected until their aggregate admission services exist.

```text
create(CreateResourceUploadRequest) -> ResourceUploadSession
put_chunk(UploadId, ChunkIndex, ContentRange, ChunkDigest, Bytes) -> ResourceUploadSession
commit(UploadId, RequestId) -> ResourceRef
get(UploadId) -> ResourceUploadSession
list_expired(now, limit<=100) -> ResourceUploadSession[]
expire(expected_progress_version, EventDraft) -> ResourceUploadSession
```

Initial desktop uploads are capped at 100 MiB, use fixed 4 MiB chunks, and expire after
24 hours. The local daemon sweeps at most 100 expired sessions every 30 seconds.
`version` advances for lifecycle transitions; `progress_version` advances for newly
accepted chunks and fences concurrent transfer writes. The final chunk commits
OPEN -> CONTENT_RECEIVED, its lifecycle event and aggregate-state blob in the same
transaction. `expire` uses an expected `progress_version` guard and commits the `EXPIRED`
projection, its complete aggregate-state blob, and `resource.upload.status.changed.v1`
atomically. `put_chunk` verifies the inclusive HTTP range, index-derived offsets, session
state, expiry, and per-chunk digest before persisting the encrypted bytes under
`BlobPurpose::ResourceUploadChunk`. Replays for the same index are accepted only when
range and digest match. Empty content is represented by a `CONTENT_RECEIVED` session with
no chunk records. Commit re-verifies each decrypted chunk and the optional whole-file
digest; the Resource, initial revision/location, `resource.created.v1` event (or
`resource.created.v2` when the pinned session has folder-import provenance), replay receipt,
`COMMITTED` session projection and CONTENT_RECEIVED -> COMMITTED event share one SQLite
transaction. A definite stored-content integrity failure commits CONTENT_RECEIVED -> FAILED
and its aggregate snapshot/event before returning the integrity error; transient storage or
database failures leave the session retryable. Initial upload creation emits
`resource.upload.created.v1`; zero-byte uploads are created directly in CONTENT_RECEIVED at
lifecycle revision 1.

Before writing each encrypted chunk blob, the store commits an operational reservation
row. The final receipt transaction consumes that row atomically with the accepted chunk.
Expired reservations without a durable chunk reference are eligible for a bounded collector.
The collector claims a digest under a durable GC fence, checks for any accepted chunk
reference or unexpired reservation in the same Workspace, removes the exact
`RESOURCE_UPLOAD_CHUNK` object through `BlobStore.remove`, then clears the fence and
reservations. Blob deletion is idempotent; a failed removal leaves the durable fence for a
later retry. The daemon invokes this sweep after expiry processing, at most 100 objects per
30-second tick. It is not a general blob garbage collector and does not delete referenced
chunk objects. Reservations and fences are operational recovery state, not domain events.

`ContentRange` is parsed as inclusive HTTP byte offsets, normalized to a half-open
`[start_offset, end_offset_exclusive)` storage range, and checked against the negotiated
chunk index and size. The returned session derives inclusive `received_ranges` and
`next_missing_offset` from accepted chunk receipts.

### ArtifactStore

Defined in `ARTIFACTS-EVIDENCE.md`. It commits a digest-verified initial version and corresponding Artifact Resource/revision on create; every later version appends both under the expected Artifact aggregate version. Before publication it checks that `input_refs` exactly matches the distinct pinned refs in `ProvenanceRecord.source_inputs` and all transformation inputs; an external ArtifactContent source ref is included. Any paired observed digest must match its pinned ResourceRevision digest when both are present. A mismatch returns `INTEGRITY_FAILURE` before rows or events are committed. Library promotion and archive use the same optimistic concurrency boundary and emit their events atomically.

### TrustService

Defined in `TRUST.md`.

### Verifier

Defined in `ARTIFACTS-EVIDENCE.md`.

### AutomationService

```text
interface AutomationService {
  create(CreateAutomationRequest) -> Automation
  update(UpdateAutomationRequest) -> Automation
  pause(AutomationId, expected_version) -> Automation
  resume(AutomationId, expected_version) -> Automation
  disable(AutomationId, expected_version) -> Automation
  get(AutomationId) -> AutomationView
  list(WorkspaceId, Cursor?, Limit) -> Page<AutomationSummary>
}
```

An AutomationRevision pins one RoutineRevision and one or more independent TriggerSpecs.
Material edits append a revision. AutomationService validates TriggerHost eligibility and
policy but does not observe events or execute Tasks. Enabling an unsupported trigger
returns `TRIGGER_UNSUPPORTED`.

### RoutineService

```text
create(CreateRoutineRequest) -> Routine
revise(ReviseRoutineRequest) -> RoutineRevision
get(RoutineId) -> RoutineView
list(WorkspaceId, Cursor?, Limit) -> Page<RoutineSummary>
run_now(RunRoutineRequest) -> TaskView
archive(ArchiveRoutineRequest) -> Routine
```

Owns Routine lifecycle and append-only RoutineRevision records. Run-now validates the
exact active/current Routine revision and typed bounded inputs, then commits one ordinary
standalone `READY` Task through TaskStore's atomic admission boundary. It never starts
planning or execution. A non-null Conversation origin is rejected until message and Task
admission can be atomic. The Task keeps the Routine ID/revision; later planning/Trust/
verification re-resolves requirements not represented in TaskSpec. A Routine cannot carry
grants, credentials, approvals, or process/session handles from earlier runs.
Archiving is rejected while enabled Automations still point to the RoutineRevision unless
the caller pauses/disables or explicitly rebinds them.

### ChannelAdapter

Defined in `CHANNELS.md`.

### ConnectionService

Owns connection references and lifecycle metadata. It never stores provider credential
bytes. Account-specific authorization and package behavior remain with the external
provider/LiteSPM contract. Provider-owned setup creates or updates the Connection after
its authentication flow; Operator API exposes only normalized metadata, status, and
disconnect. Provider-specific setup UI, callbacks, and credential exchange are outside
the Operator API and remain deferred with the integration contract.

```text
list(WorkspaceId, ConnectionQuery, Cursor?, Limit) -> Page<ConnectionSummary>
get(ConnectionId) -> ConnectionView
disconnect(ConnectionId, expected_version) -> ConnectionView
```

### ChannelService

Owns ChannelBinding identity, assurance, action permissions, and revocation. It does not
own Conversation or Task truth; it calls ConversationService/TaskService after identity
and authorization checks.

```text
list_bindings(WorkspaceId, ChannelBindingQuery, Cursor?, Limit) -> Page<ChannelBindingSummary>
get_binding(ChannelBindingId) -> ChannelBindingView
update_allowed_actions(ChannelBindingId, expected_version, ChannelAction[]) -> ChannelBindingView
revoke_binding(ChannelBindingId, expected_version) -> ChannelBindingView
reassign_host(ChannelBindingId, ReassignChannelHostRequest, expected_assignment_version, RequestId) -> ChannelHostAssignment
process_inbound(ChannelBindingId, InboundChannelEvent, AuthenticatedChannelHostContext) -> ChannelEventOutcome
resolve_reply_target(ChannelBindingId, provider_message_ref) -> UserRequestId?
```

Channel-provider setup creates the binding from authenticated provider identity; the
owner then reviews its allowed actions. The provider, not the caller, supplies the
authenticated identity and assurance level. RuntimeMesh assigns one current host lease
and epoch per binding. `process_inbound` checks authenticated Runtime identity, current
host epoch and lease expiry, exact sender Principal, binding version/status/actions,
request expiry/status, and response schema. A successful targeted response is committed
with the ChannelEventReceipt by the assigned Runtime's Store transaction; invalid replies
leave the target active and do not fall through to generic Conversation steering. Host
reassignment checks provider compatibility, SecretRef placement, cursor/replay continuity,
source drain proof or the immutable expiry-plus-skew release time, and any explicit gap
confirmation/audit reference; it
fences the source epoch and never copies opaque reply-to references or cursor bytes. Receipt
claims can be reclaimed only after expiry or authoritative fencing, under the new lease.
If the source is unreachable, its target rows become non-authoritative through the epoch
check and are cleaned on reconnect. ChannelService advances provider cursors only after
receipt durability and required Hub replication acknowledgement.

Receipt insert, claim, settlement, and provider acknowledgement share a per-binding
serialization boundary with RuntimeMesh drain. `process_inbound` inserts only while the
source assignment and host lease are current and ACTIVE. If draining commits first, it
returns a retry/backpressure outcome without acknowledging or storing a new source receipt;
the provider event remains replayable, or the move must record an explicit ingress gap.
This applies to new receipt insertion; it does not reject terminal settlement of an existing
PROCESSING receipt when the original claim and source lease remain valid during DRAINING.
Drain proof waits for all PROCESSING claims from the source epoch and accounts for every
pre-drain RECEIVED row in the Hub-durable successor replay frontier; the new host may claim
those rows under its own lease. The service transaction makes proof and release decisions against the same
durable receipt/cursor frontier; a heartbeat or local view is not enough.

## Internal application services

### PlanningCoordinator

Consumes a TaskService-authorized transient PlanningAssignment envelope with AgentSessionSupervisor. It may start only one TASK_PLANNING session for the current Task version, lead binding, and TaskSpecRevision; the envelope is not a persisted entity. It first claims a STARTING row, then invokes the native adapter outside the storage transaction. Only after adapter readiness does it request an atomic session-ACTIVE/first-Task-RUNNING transition. It closes/reconciles the session when the Task pauses, cancels, changes lead/spec, or waits for user input. It does not author or promote PlanRevision records.

### AgentHostStore

Persists and reads Runtime-local `AgentHostInstance` observations. Creation requires a
current READY Runtime incarnation and an endpoint/profile match. State transitions use
an expected-state compare-and-set and do not emit replicated domain events. Host records
are operational inventory; local process identity is never projected through the Operator
API. The adapter does not spawn or stop processes; those decisions belong to
AgentHostSupervisor.

### AgentSessionStore

The storage port commits a version-1 STARTING AgentSession plus its aggregate snapshot,
`agent.session.starting.v1`, and idempotency receipt atomically. Admission rechecks the
Task's expected version, status, current TaskSpec revision and lead; Workspace owner and
status; enabled/lead-eligible AgentBinding; selected endpoint and current Runtime
incarnation readiness. It exposes a bounded recovery query for STARTING planning sessions
and a STARTING-to-LOST settlement operation. After observed adapter readiness, the source
also exposes an atomic activation transaction: it rechecks host/Runtime/binding/endpoint
readiness, inserts the Runtime-local host binding, and commits AgentSession ACTIVE plus
first Task RUNNING and their events/snapshots. Adapter startup remains outside storage.
PlanningCoordinator and AgentSessionSupervisor orchestration are not yet wired.

### AgentSessionSupervisor

Owns AgentSession lifecycle for CONVERSATION, TASK_PLANNING, and ATTEMPT_EXECUTION
sessions. It invokes the negotiated AgentAdapter, validates the discriminated scope before
dispatch, asks AgentHostSupervisor to ensure/retain the selected endpoint, and emits
normalized lifecycle outcomes; it cannot mutate Conversation or Task state directly.
Durable session identity, scope, selected endpoint, Runtime, and RuntimeIncarnation are
committed separately from the local `AgentSessionHostBinding`. A binding must match that
exact Runtime/incarnation and is removed only after the session reaches CLOSED or LOST. A
new Runtime creates a new session rather than attaching a source Runtime's opaque handle.
The supervisor closes sessions when their owning turn/assignment settles or durably yields;
it releases the local binding only after adapter quiescence is observed. A pending
CapabilityInvocation does not hold an AgentSession open: its Invocation runner owns the
provider lifecycle, and the Attempt can receive its result through a replacement session
only after authority and checkpoint state are revalidated.

### AttemptRunner

Responsibilities:
- prepare Attempt
- start AgentSession
- feed TaskPacket/context
- consume normalized agent events
- route host gateway calls
- checkpoint
- settle Attempt

Must not decide intellectual plan quality.

### PlacementService

```text
select(PlacementRequest) -> PlacementDecision
```

Uses hard constraints first: runtime availability, agent compatibility, required resources/secrets, environment offer, policy, failover class. Preferences/cost may break ties but never violate hard constraints.

### LeaseCoordinator

Only component allowed to acquire/renew/release authoritative ExecutionLease records.

```text
acquire(AcquireLeaseRequest, AuthenticatedRuntime) -> ExecutionLeaseGrant
renew(RenewLeaseRequest, AuthenticatedRuntime) -> LeaseRenewalResult
release(ReleaseLeaseRequest, AuthenticatedRuntime) -> ExecutionLease
validate(FencingCredential, MutationScope, AuthenticatedRuntime) -> FenceDecision
```

Acquire/renew bind the lease to the Attempt's immutable Runtime incarnation and use
RequestId deduplication. `CredentialIssuer` derives the HMAC credential from the protected
issuer key version plus immutable lease ID/epoch claims; the raw value is returned only on
authenticated Mesh control and passed to the enforcing provider. The Agent, Operator,
durable record, event, aggregate-state blob, logs, and deduplication response never receive
or store it. `ExecutionLease` stores the credential digest and non-secret issuer key version.
Renewal preserves the credential within the epoch and updates the authoritative expiry;
the provider receives the renewed lease view before extending its local execution deadline.
If delivery of that view is lost, the provider enforces its last confirmed earlier expiry
and the Runtime retries the same RequestId.

### CompletionEvaluator

Collects criteria, outputs, child status, approvals and open Effects; invokes Verifier registry; asks TaskService for final transition.

While Task pause/cancellation is pending, it starts no new VerificationRuns and cannot
finalize the Task. A result from a run pinned to the prior TaskSpec/input set may be
recorded, but TaskService rejects completion after the lifecycle transition wins its
aggregate-version race.

### VerifierRunner and VerifierRegistry

VerifierRegistry selects a compatible deterministic verifier first. VerifierRunner owns
VerificationRun start/result records, bounded deadlines, immutable Evidence append, and
typed `INCONCLUSIVE` settlement on timeout. Verifiers receive bounded read-only subject
access; they cannot mutate TaskSpecs, Artifacts, Resources, or external Effects. A retry
creates a new VerificationRun. It resolves each `ResourceInput.resource_ref` against the
pinned ResourceRevision and records the digest of bytes actually supplied when available.
If both that digest and the revision's provider-observed `content_digest` exist, they must
match; a mismatch returns `INTEGRITY_FAILURE` and cannot produce passing Evidence.

### EffectService / EffectReconciler

Own effect ledger transitions and post-failure reconciliation.

### EvidenceService

Append-only Evidence writer. No update/delete except administrative retention process that preserves audit semantics.

### DependencyService

```text
register_inputs(DependentRef, PinnedResourceRef[], CommandTransaction) -> DependencyEdge[]
process_revision_change(ResourceRevisionId, CommandTransaction) -> InvalidationRecord[]
list_dependents(ResourceId, Cursor?, Limit) -> Page<ResourceDependencyView>
list_invalidations(DependencyEdgeId, Cursor?, Limit) -> Page<InvalidationRecord>
rebuild_edge_projection(Cursor?) -> RebuildCursor
```

`register_inputs` is called by ArtifactStore or VerifierRunner inside the same
CommandTransaction that creates its dependent aggregate and event/state blob. It persists
the exact input refs and writes one immutable `DependencyEdge` per pinned source revision.
ArtifactStore passes `ArtifactVersion.input_refs`; VerifierRunner passes each
`VerificationRun.inputs[].resource_ref`. The observed digest remains paired with the
VerificationRun input and is not part of the reverse-index key.
ResourceService calls `process_revision_change` in the same CommandTransaction that
commits a newly observed ResourceRevision, so the new revision and its InvalidationRecords
become visible atomically. DependencyService also consumes Resource revision,
ArtifactVersion, and VerificationRun events to repair/rebuild its reverse index from
aggregate state/event refs; it does not treat generic ResourceEdges as
dependencies. On a new source revision, it finds edges for that logical Resource whose
consumed revision is no longer current, then appends an `InvalidationRecord` referencing
the exact edge and newly observed revision. `(dependency_edge_id, observed_revision_id)`
is the idempotency key. It changes only freshness projections; immutable artifacts,
Evidence, and verification results are retained. It depends on ResourceService read port,
ArtifactStore/VerifierRunner event projections, StateStore, and EventStore, and never
performs content retrieval itself. `DependencyIndexPort` is transaction-scoped: it joins
the caller's CommandTransaction and cannot open a nested or independent write transaction.

```text
ResourceDependencyView {
  dependency_edge: DependencyEdge
  dependent_freshness: DependencyFreshness
  latest_invalidation: InvalidationRecord?
}
```

### EnvironmentManager

Chooses provider adapter, persists canonical Environment state, enforces cleanup/checkpoint
rules, and owns EnvironmentControlLease. Human takeover increments the input-control
epoch, invalidates queued agent actions, and requires a fresh observation/reconciliation
before control returns to an AgentSession.

```text
preview_workspace_persistent(WorkspaceId, EnvironmentProvisionPreviewRequest, CommandContext) -> EnvironmentProvisionPreviewRecord
create_workspace_persistent(WorkspaceId, CreatePersistentEnvironmentRequest, CommandContext) -> EnvironmentView
list_workspace(WorkspaceId, EnvironmentQuery, Cursor?, Limit) -> Page<EnvironmentView>
get_workspace(WorkspaceId, EnvironmentId) -> EnvironmentView
suspend(WorkspaceId, EnvironmentId, expected_version, RequestId) -> EnvironmentView
resume(WorkspaceId, EnvironmentId, expected_version, RequestId) -> EnvironmentView
change_sharing_scope(EnvironmentId, target_scope, target_coworker_id?, confirm, expected_version, RequestId) -> EnvironmentView
request_destroy(WorkspaceId, EnvironmentId, DestroyEnvironmentRequest, expected_version, RequestId) -> EnvironmentView
take_control(TakeEnvironmentControlRequest) -> EnvironmentControlLeaseView
return_control(ReturnEnvironmentControlRequest) -> EnvironmentControlLeaseView
observe(EnvironmentId, expected_control_epoch) -> EnvironmentObservation
```

Workspace-persistent creation is an explicit Workspace action; the request has no Task
owner and cannot include credential bytes or grant/lease authority. EnvironmentManager
checks provider eligibility, policy, resource/network bounds, cost and retention before
provisioning. `REQUIRE_PROVIDER_ENFORCED` rejects a request unless the provider confirms
that the requested cost ceiling is enforced. `ALLOW_HOST_MONITORED` is explicitly
best-effort and cannot claim a hard cap while the Runtime/provider is offline. Suspend
waits for active Attempt use, provider Invocations and Effects to
settle; destroy also requires checkpoint/artifact holds to clear. Resume reattaches and
probes the provider, then returns only health/readiness—never prior grants, Secrets,
control epochs, or ExecutionLeases. Operator projections omit provider-private locator
fields. Archived Workspace state may be retained suspended according to its backup
policy; archive cannot stop an unknown live workload.

Preview digest binds canonical normalized request bytes, authenticated principal, the
selected Runtime incarnation, pinned Resource revisions, provider offer identity/revision,
provider policy and quote basis, and expiry. Create validates both the digest and exact
request under the same Workspace admission lock before reserving budget or provisioning.
Request mismatch returns `CONFLICT`; expired preview or changed eligibility basis returns
`STALE_VERSION`. Idempotency lookup precedes preview validation so a replay of an already
accepted command returns its original result without creating another Environment.
The short-lived preview record is single-use and is consumed in the same transaction that
creates the PROVISIONING Environment row and reserves its budget; the external provider
call follows with a stable request ID so crash recovery reconciles rather than double
allocates. Environment and `environment.created.v1` retain the preview digest as consent
provenance; the ephemeral record itself is not replicated as domain history.
Preview issuance is authenticated and rate-limited per Workspace/principal. The default
TTL is five minutes, shortened to any provider offer/quote expiry; its digest is a lookup
key only and conveys no authority.

### InvocationRunner

Owns durable CapabilityInvocation dispatch, asynchronous polling/resume, Runtime-local encrypted provider cursor binding,
cancellation, bounded partial results, usage observations, and settlement. It calls a
provider only after CapabilityBroker authorization and does not define LiteSPM's wire
contract. Provider task handles remain opaque values attached to the Invocation.

```text
deliver_user_input(InvocationContinuationAuthorization) -> ProviderInputDeliveryReceipt
```

The permit comes from ConversationService for the exact waiting ConversationTurn or
TaskService for a current Task scope. A Task-planning permit requires a fresh active
planning session for the same current lead and TaskSpec; an Attempt permit requires the
same source Attempt to be current again with a fresh higher-epoch lease on the original
Runtime incarnation/Environment. Its checkpoint, plan, Grant, provider binding, and Effects
must be reconciled. InvocationRunner verifies/consumes the exact response outbox transition
in the same transaction, then calls `tasks/update` outside the transaction. A replacement
Attempt or Runtime cannot inherit the old provider task handle; cancel/reconcile the old
Invocation before the lead agent creates new authorized work.

### ProviderInputCoordinator

```text
dispatch(UserRequestId, expected_response_version) -> ProviderInputDeliveryReceipt
```

Consumes a committed UserRequestResponse/outbox notification. It asks ConversationService
for the exact-turn permit or TaskService for the current planner/Attempt permit, then
passes that opaque permit to InvocationRunner. It does not decide whether an old provider
task is still applicable, and it does not retry an ambiguous delivery itself. The outbox
transition and provider-specific idempotency proof determine whether dispatch or retry is
allowed.

### BudgetService

Reserves and accounts for observable usage at Task/Attempt/Invocation and persistent
Environment scope. An observation attributed to a Task using a persistent Environment is
checked against two independently owned ceilings and creates separate reservations.
Unknown external-agent usage remains unknown; no budget ceiling is promised if the provider
cannot enforce or report it. For persistent Environments the service projects cumulative
usage since provisioning, never a fabricated zero during provider/Runtime gaps. A reached
ceiling stops new admissions and requests EnvironmentManager to quiesce/suspend the
Environment after in-flight Effects and Invocations are reconciled.

### ProjectionService

Consumes domain events and builds Task/Conversation/LiveDesk/Notification, GoalProgress,
RoutineHealth, WorkerPerformance, Coworker-presence, and typed Operator presentation
projections. RoutineHealth is derived from the current immutable RoutineRevision's terminal
Tasks, UsageObservations, dependency health observations, and drift Evidence; it does not
write Routine state.
Projection failure never mutates domain truth.

Presentation projections expose stable item/source identity, source revision, safe typed
payload, ordering, freshness, and an opaque stream cursor. They are authorized read models,
not domain events or durable item records. `AgentTurnCoordinator` may forward bounded,
coalesced active-turn text deltas through the authenticated Operator stream; these transient
frames are not persisted, replicated, or emitted as EventStore events. A settled
ConversationMessage replaces them. Renderer selection and local panel/scroll state belong
to the Operator; they cannot mutate a domain aggregate.

`RichResponsePolicyEvaluator`, `HostSkillRegistry`, and `RichPresentationCompiler` are
small components at existing Conversation/Operator boundaries, not a new orchestration
service stack. The evaluator is deterministic and adds no model call. HostSkillRegistry
loads only digest-verified bundled zero-authority assets. The compiler validates typed
model intent and binds it to semantic-message slices, real Artifact/Resource refs,
registered tool-result renderers, and authorized Core projections. ProjectionService
continues to own factual system projections; the rich compiler cannot synthesize them.

### TriggerCoordinator

```text
ingest(TriggerDelivery) -> AutomationOccurrence
claim(ClaimOccurrenceRequest) -> OccurrenceClaim
request_manual_occurrence(ManualOccurrenceRequest) -> AutomationOccurrence
advance_cursor(AdvanceAutomationCursorRequest, trigger_host_epoch) -> AutomationCursor
materialize_task(OccurrenceId, claim_epoch) -> TaskId
settle_occurrence(SettleOccurrenceRequest, claim_epoch) -> AutomationOccurrence
```

Consumes each AutomationRevision's independent TriggerSpecs, verifies the one authorized
TriggerHost for each trigger, advances its cursor with delivery receipt/occurrence
deduplication, pins both AutomationRevision and RoutineRevision, and fences late claimants
by `claim_epoch`. It distinguishes trigger observation placement from Task execution
placement. A due occurrence can create an ordinary Task that remains `WAITING_DEPENDENCY`
until its required Runtime/resources become available. Cursor advancement, occurrence
creation, and dedupe receipt commit atomically. It invokes TaskService; it does not own
Task execution. Run-now requires a ManualTrigger; direct Routine runs use RoutineService.

### Routine / Automation trigger processing

TriggerCoordinator is defined above; AutomationService owns the immutable definition and
RoutineService owns the reusable work template. Trigger processing does not create a
second Task executor.

The current local owner ManualTrigger slice is narrower than the full TriggerCoordinator:
the authenticated Operator resolves exact revisions and the one-shot request identity,
then TaskService/TaskStore atomically admits the occurrence plus READY Task. It does not
claim scheduled/provider trigger hosting, cursor advancement/recovery, or Task-outcome
settlement support.

### NotificationService

Owns NotificationPreference, deduplicated NotificationDelivery, channel fallback, quiet
hours, and bounded retry/backoff. It does not retry an ambiguous send until reconciliation
proves whether the provider accepted it. For a reply-capable adapter, a prompt for one pending
FORM UserRequest may be marked reply-targetable. Each send attempt pins the assigned
Runtime/host epoch before dispatch. After acknowledgement, ChannelService persists the
provider message reference in its Runtime-local `ChannelReplyTarget` map only if that exact
host still owns the current assignment and lease; failed/ambiguous delivery or an
acknowledged send from a superseded host never creates an actionable target.
`SENT` is transport acknowledgement only and never changes Task/Approval/Automation state.

### NeedsYouQueryService

```text
list(WorkspaceId, NeedsYouFilter, Cursor?, Limit) -> Page<NeedsYouItem>
```

Builds a read-only, rebuildable Workspace projection over open and retained resolved
Approvals, UserRequests, and actionable Task blockers. The default view/count includes
only OPEN rows; RESOLVED rows follow source-record history retention. A linked blocker
with `approval_id`/`user_request_id` is represented by that underlying item once; otherwise its stable identity is
`(task_id, blocker_id)`. `item_id` uses the versioned, length-prefixed SHA-256 encoding in
`SCHEMAS.md`; labels and status never affect identity. Notifications are not inbox items
and delivery retries do not inflate the count. Item actions call the owning TrustService,
ConversationService, Runtime or TaskService route; the aggregate endpoint cannot approve
or answer anything itself.

### SkillProposalService

Creates a proposed Skill draft from an explicit successful Task, scans/redacts
Task-specific data and secrets, validates it, requests user approval, and hands the
approved Artifact to LiteSPM publication when its external contract is available. It does
not independently publish or version packages.

### AuditService

Appends authorization, approval, Runtime pairing/revocation, secret-lease, and sensitive
Effect records. It has no update API. Retention must preserve evidence needed for active
Effect reconciliation and recovery.

### LiteSPM adapter boundary

CapabilityBroker and CapabilityHostSupervisor depend on one outbound adapter implementing
the future LiteSPM contract. The base URL is selected in `CAPABILITY-FABRIC.md`. Method names, payloads,
authentication, retries, version negotiation, package manifests, and activation details
are deliberately unspecified until LiteSPM's authoritative interface is supplied. No
other LiteCowork service may call the LiteSPM adapter directly; domain services use the
owning Broker or Supervisor port.

## Infrastructure ports

### StateStore

Transactional storage for current aggregate state and rebuildable projections. Aggregate
mutation and its DomainEvent append commit atomically. It exposes unit-of-work/transaction
scope to owning services, not raw SQL to domain code. The SQLite adapter may expose a
process-local writer-pressure snapshot to observability; it includes outstanding command
submissions and bounded-channel send wait, is best-effort under concurrency, and is never
durable domain state or an authorization input.

### WorkspaceSnapshotPort

Captures a Workspace-consistent database checkpoint, a per-origin EventStore cursor
barrier, schema version, included event history, and the complete referenced-BlobRef set.
Restore loads that exact point-in-time checkpoint/history into an empty installation; it
does not apply post-barrier events or restore Runtime lease authority. Future changes
arrive through ordinary Mesh replication after the new Runtime is paired.

### EventStore

Append/read the immutable per-origin domain event stream. Replication acceptance validates
origin authority and aggregate transition before a remote event updates canonical state.

### EventBus

In-process/local pub-sub for committed event delivery to projections; not source of truth.

### BlobStore

Content-addressed immutable blob storage. `put(workspace_id, purpose, bytes)` computes
the plaintext SHA-256 `BlobRef`, encrypts using a versioned Workspace-scoped key from
`WorkspaceBlobKeyProvider`, commits the immutable object, and returns only after the
committed bytes can be verified. `get` authenticates/decrypts, checks the plaintext
digest/size, and returns bytes only to an authorized caller. A missing key or failed
authentication is an error; plaintext fallback is forbidden.

### WorkspaceBlobKeyProvider

Supplies versioned encryption keys for an exact `(WorkspaceId, BlobPurpose)` scope. Key
material stays in the OS keystore/HSM or deployment key service, is never stored in
SQLite/events/backups/logs, and is not exposed through the ordinary BlobStore API. Key
creation, rotation, recovery and deletion are deployment-provider responsibilities and
must be qualified before production release.

### BackupKeyProvider

Encrypts/decrypts backup streams with authenticated encryption and creates/verifies a
provider-generated authentication value over the canonical manifest. It uses an opaque
provider-owned `BackupKeyRef`; key material never enters domain state, ordinary BlobStore
APIs, events, or logs. Algorithm and key rotation policy are supplied by the deployment's
key provider and must meet the deployment security baseline.

### SecretStorePort

Stores actual secret bytes; domain sees SecretRef/leases only.

### Clock

Injectable time source for deterministic tests.

### IdGenerator

Injectable opaque ID generator.

## Transaction rule

An application command performs validation before the transaction where possible, then
rechecks mutable versions, authorization, and lease/fence inside the transaction. The
transaction commits aggregate state, its DomainEvent(s), and request-dedup result
together. External network/provider calls never run while holding a StateStore
transaction. If a multi-record invariant spans Task/Step/Attempt/Lease, one owning
application command coordinates service owners inside a single transaction scope.

## Dependency constraints

```text
ConversationService -> StateStore, EventStore, AgentTurnCoordinator
AgentTurnCoordinator -> AgentSessionSupervisor normalized outcome stream
WorkspaceService -> StateStore, EventStore, TrustService
BackupService -> WorkspaceSnapshotPort, EventStore, BlobStore, BackupKeyProvider, RuntimeMesh, AuditService
TaskService -> StateStore, EventStore, TrustService, PlacementService, ExecutionDependencyPlanner, CompletionEvaluator, PlanningCoordinator
AgentBindingService -> RuntimeMesh read models, AgentAdapter discovery, TrustService, StateStore, EventStore
DelegationProfileService -> AgentBindingService read port, AgentAdapter option negotiation, TrustService, StateStore, EventStore
WorkerSelectionService -> AgentHarnessDescriptor observations, BudgetService, RuntimeMesh, EnvironmentManager offer read ports, Trust policy
DelegationCoordinator -> TaskService, WorkerSelectionService, DelegationProfileService, AgentSessionSupervisor, AttemptRunner, BudgetService, TrustService
CoworkerService -> WorkspaceService read port, AgentBindingService read port, DelegationProfileService read port, TaskService admission port, StateStore, EventStore
GoalService -> TaskService read port, RoutineService read port, ArtifactStore read port, EvidenceService read port, StateStore, EventStore
SuggestionService -> TaskService, RoutineService/AutomationService editor/read ports, Clock, StateStore, EventStore
DemonstrationSessionService -> EnvironmentManager, TrustService, ResourceService, SkillProposalService, StateStore, EventStore
PersonalContextService -> ResourceService, TrustService, PersonalContextProvider adapters
PlanningCoordinator -> AgentSessionSupervisor, AgentAdapter, TaskService-issued PlanningAssignment envelope
AgentSessionSupervisor -> AgentHostSupervisor, AgentAdapter, StateStore, EventStore
AgentHostSupervisor -> AgentHostStore, AgentEndpoint registry/binding, RuntimeMesh offer view, RuntimeLifecycleService, local process identity adapter
AttemptRunner -> AgentSessionSupervisor, ExecutionDependencyPlanner, CapabilityBroker, EnvironmentManager, LeaseCoordinator, Artifact/Effect services
PlacementService -> RuntimeMesh read models, Agent registry, Trust policy, Environment offers, WorldIndex
ExecutionDependencyPlanner -> PlacementService, AgentHostSupervisor, CapabilityBroker, EnvironmentManager, WorldIndex, TrustService
CapabilityBroker -> CapabilityHostSupervisor, TrustService, Runtime inventory
CapabilityHostSupervisor -> LiteSPM adapter (contract deferred), RuntimeLifecycleService, StateStore, Clock
LiteSPM adapter -> selected LiteSPM service URL (wire contract deferred)
InvocationRunner -> CapabilityBroker, provider adapters, ArtifactStore, EffectService, BudgetService
ProviderInputCoordinator -> EventBus, ConversationService continuation port, TaskService continuation port, InvocationRunner
WorldIndexer/ResourceService -> root authorization, ResourceLocationProvider registry, DependencyIndexPort, EventStore, StateStore, local search index
RuntimeMesh -> EventStore, BlobStore, StateStore, transport adapter
EnvironmentManager -> EnvironmentProvider adapters, StateStore, EventStore, TrustService, ResourceService, BudgetService, ArtifactStore/EffectService read ports
ArtifactStore -> BlobStore, ResourceAggregatePort, DependencyIndexPort, StateStore, EventStore
VerifierRunner -> VerifierRegistry, DependencyIndexPort, EvidenceService, StateStore, EventStore, Clock
DependencyService -> StateStore, EventStore
TrustService -> StateStore, EventStore, SecretStorePort
CompletionEvaluator -> VerifierRegistry/Runner, StateStore, DependencyService
RuntimeLifecycleService -> RuntimeMesh, StateStore, EventStore, OS service-manager adapter, Clock
RoutineService -> TaskService, AutomationService read port, TrustService, StateStore, EventStore
AutomationService -> RoutineService read port, RuntimeMesh/TriggerProvider offers, StateStore, EventStore
TriggerCoordinator -> AutomationService, RoutineService read port, TaskService, RuntimeMesh, StateStore, EventStore
ProjectionService -> EventStore/read stream + projection stores
NeedsYouQueryService -> Approval/UserRequest/Task projections
ChannelService -> TrustService, ConversationService, TaskService, NotificationService
ConnectionService -> TrustService, SecretStorePort, external provider adapter
AuditService -> StateStore, EventStore
ProjectionService -> event stream + Coworker/Goal/Suggestion/WorkerPerformance/TaskProgress/Presentation projection reducers
RichPresentation compiler -> ConversationService read/publish ports, BlobStore,
  authorized Resource/Artifact/Capability-result resolvers, RichPresentation schema validator
ConversationService -> RichPresentation persistence through StateStore/EventStore in the
  existing aggregate transaction boundary
```

`ResourceAggregatePort` and `ResourceLocationProvider` are Resource-domain ports. The
composition root supplies ArtifactStore as one location provider; ResourceService imports
only the port, while ArtifactStore writes its paired Resource records through the aggregate
port in the same unit of work. This keeps the module dependency graph acyclic.

Forbidden:
- AgentAdapter mutating Task/Artifact/Effect DB directly
- UI mutating StateStore directly
- provider adapters emitting user-visible completion without domain service transition
- LiteSPM package metadata bypassing TrustService for activation
- CapabilityHostSupervisor treating a shared process as shared authorization or exposing an opaque LiteSPM handle
- a second LiteCowork package/process supervisor competing with LiteSPM's actual lifecycle
- AgentAdapter or provider writing domain tables without the owning service
- a replicated event bypassing origin authorization, revision, or fencing checks
- a notification changing Task/Approval/Automation state
- Coworker, Goal, Suggestion, WorkerPerformance, or warm-state projections becoming alternate Task/Effect/authority truth
- DelegationCoordinator creating Steps outside the accepted PlanRevision or issuing child authority by copying a parent's grants/Approvals/SecretLeases
- WorkerSelectionService substituting a profile after explicit user selection or treating unknown cost/quota as zero/available
- any warm manager starting a model request or bypassing admission checks during prewarm
- any service holding a storage transaction open during a network/provider call

## Error and idempotency contract

Mutating commands accept a `RequestId` scoped to the authenticated principal. The same
ID and request digest returns the prior outcome; reusing it with a different digest is
`CONFLICT`. Commands subject to races require an expected aggregate version. Services
return the common typed error from `SCHEMAS.md`; adapters map provider-specific failures
without leaking secrets.
