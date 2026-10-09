# Environment Fabric LLD

## Definition

Runtime is the LiteCowork daemon. Environment is the substrate in which an Attempt acts.

A Runtime may host many Environments.

## Provider contract

```text
interface EnvironmentProvider {
  probe(ProbeRequest) -> EnvironmentProviderCapabilities
  create(EnvironmentSpec) -> EnvironmentHandle
  attach(EnvironmentHandle) -> EnvironmentDescriptor
  execute(EnvironmentHandle, ExecutionRequest) -> ExecutionHandle
  expose_file(EnvironmentHandle, ResourceRef, FileExposureSpec) -> ExposedPath
  expose_port(EnvironmentHandle, PortExposureSpec) -> ExposedPort
  checkpoint(EnvironmentHandle, CheckpointSpec) -> EnvironmentCheckpoint?
  restore(EnvironmentCheckpoint, RestoreSpec) -> EnvironmentHandle?
  destroy(EnvironmentHandle, DestroySpec) -> Ack
}
```

## EnvironmentSpec

```text
EnvironmentSpec {
  class
  runtime_id
  owner_workspace_id
  lifetime: ATTEMPT | TASK_RETAINED | WORKSPACE_PERSISTENT
  owner_task_id?       # required for ATTEMPT / TASK_RETAINED
  attempt_id?         # required for ATTEMPT once bound
  owner_coworker_id?  # required for COWORKER_PRIVATE
  owner_principal_id? # required for USER_SHARED
  sharing_scope: ATTEMPT_PRIVATE | TASK_SHARED | COWORKER_PRIVATE | WORKSPACE_SHARED | USER_SHARED
  isolation
  resource_limits
  budget_ceiling
  budget_enforcement_policy: REQUIRE_PROVIDER_ENFORCED | ALLOW_HOST_MONITORED
  filesystem
  network_policy
  secret_leases[]
  source_resources[]
  ttl?
  cleanup_policy
}
```

`lifetime` controls how long the substrate may exist; `sharing_scope` controls who may
reuse it. Admission enforces these owner bindings:

| Sharing scope | Required owner | Reuse boundary |
|---|---|---|
| `ATTEMPT_PRIVATE` | Task + exact Attempt | That Attempt only; never reused by replacement Attempt |
| `TASK_SHARED` | Task | Attempts within that Task, subject to write/control fencing |
| `COWORKER_PRIVATE` | Same-Workspace Coworker | Tasks pinned to that Coworker; workspace-persistent retention only |
| `WORKSPACE_SHARED` | Workspace | Eligible Tasks in that Workspace |
| `USER_SHARED` | Principal | Reserved for a user-level owner/attachment contract; provision and cross-Workspace mounting are unavailable in v1 |

Share scope is not an authorization grant. Every new Attempt receives current capability
grants, Effect policy, budget admission, and an Environment attachment. A browser's
persisted authentication does not transfer input control: one `EnvironmentControlLease`
owner acts at a time. Parallel code writers default to private worktrees/overlays. A
shared writable base requires an explicit provider write lease or deterministic merge
workflow. A stale/missing provider handle, Runtime incarnation, auth state, or freshness
observation prevents reuse until revalidated.

Creation requires a provider request ID and explicit Workspace authorization. ATTEMPT
and TASK_RETAINED require an owner Task in the same Workspace; WORKSPACE_PERSISTENT has
no owner Task and is admitted through an explicit Workspace resource action. Execution
still names the using Task/Attempt and receives fresh scoped authority. A persistent
Environment's provision-time `secret_leases[]` are short-lived allocation credentials,
not retained connector tokens or grants inherited by later work. `REQUIRE_PROVIDER_ENFORCED`
blocks provisioning unless the provider enforces the requested cost cap. The explicit
`ALLOW_HOST_MONITORED` policy is monitor-only and can overshoot when the host is offline,
suspended, or the provider reports charges late; the UI states that limitation before
confirmation. An `UNAVAILABLE` estimate/enforcement state never appears as a guaranteed
ceiling. Persistent Environment cost and wall-time ceilings are cumulative from creation
and do not reset on a schedule. Budget usage is attributed to the Environment and, when
applicable, separately to each using Task; unknown or stale observations remain unknown.
Cost comparisons require the pinned budget currency; v1 performs no implicit FX
conversion. At the limit, admission of new uses stops. EnvironmentManager requests a
safe checkpoint/suspend and reconciles in-flight Invocations and Effects; unresolved
Effects block destructive cleanup. A provider-enforced cap remains active while the
Runtime is offline. A host-monitored threshold may be exceeded during a disconnection and
must not be described as a hard cap.

For v1, `budget_ceiling` and `budget_enforcement_policy` are immutable after successful
provisioning; there is no in-place top-up, reset, or budget-change API. At `LIMIT_REACHED`,
the Environment stops admitting new uses and follows the safe checkpoint/suspend path.
Current consumers are told which Task/Step is blocked. They may select another already
eligible Environment or explicitly provision a replacement through the normal preview
and approval flow. A replacement is a different Environment: LiteCowork does not assume
that provider process state or private filesystem contents can be cloned. Reuse requires
explicitly resolving the needed files/state as Resources or Artifacts and admitting a new
Attempt with fresh grants, leases, and budget checks. Existing Attempt bindings never
silently change.

## Isolation

```text
IsolationSpec {
  filesystem: SHARED_READONLY | PRIVATE_COPY | WORKTREE | CONTAINER_FS | VM_FS
  process: HOST | NAMESPACE | CONTAINER | VM | REMOTE
  network: NONE | RESTRICTED | DEFAULT | CUSTOM
  write_scope: ResourceScope
}
```

Host delegation should default to isolated write scope when two workers can modify related resources.

## Built-in provider classes

### LocalWorkspace
Direct local workspace for owner-controlled files and device resources. It may be
`LOCAL_BOUND`, but a host directory or process working directory is not a security
boundary. A `LocalWorkspace` with `process: HOST` must not be admitted for an external
Agent that can read or write files unless a separately qualified OS containment provider
enforces the requested filesystem, process, and network restrictions.

### Local Task execution isolation

The desktop V1 native-agent path uses a Task-scoped Environment prepared from exact
`PinnedResourceRef`s. The host working directory, Git root, or Git worktree alone is not
containment: a child process may still read the user's home directory, follow a symlink,
or start a descendant that continues writing after the parent exits. `GitWorktree` provides
change isolation and mergeability; it does not by itself provide read isolation or
process-tree fencing.

Before an AgentSession or Attempt can start, the selected provider must return a
Runtime-local, non-secret `IsolationAttestation` bound to the Environment ID, Task ID,
Attempt/session admission, current Runtime incarnation, provider implementation/version,
OS build, and normalized isolation policy digest. It records observed enforcement for:

```text
readable roots       exact staged input roots plus explicitly allowed runtime files
writable roots       separate bounded Attempt output/work roots
network              NONE or the exact policy-enforced allowlist
process containment  provider-owned process group/job/container boundary
resource limits      enforced limits or explicit unsupported/unknown results
```

The provider resolves each source only by its exact Workspace/Resource/revision/digest
pin. It verifies bytes against the pinned digest before exposure, rejects missing, stale,
foreign, duplicate, traversal, and symlinked inputs, and presents source inputs read-only
under the enforcing OS boundary. Outputs and generated files go to a distinct bounded
writable root. The provider must not mount a home directory or ambient Workspace path.
Import limits apply before extraction or copying; an unsupported format is rejected or
exposed as opaque bytes, never silently treated as parsed content.

Runtime-local evidence is represented as bounded typed values, stored only in the private
incarnation-scoped binding store and never in domain events, aggregate snapshots, backups,
Operator projections, Agent prompts, or Task packets:

```text
IsolationAttestation {
  environment_id: EnvironmentId
  workspace_id: WorkspaceId
  task_id: TaskId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId

  provider_kind: string
  provider_version: string
  operating_system: string
  operating_system_build: string

  normalized_policy_digest: Sha256Digest
  pinned_input_set_digest: Sha256Digest
  readable_root_set_digest: Sha256Digest
  writable_root_set_digest: Sha256Digest

  filesystem_enforcement: ENFORCED | NOT_ENFORCED | UNKNOWN
  process_tree_enforcement: ENFORCED | NOT_ENFORCED | UNKNOWN
  network_enforcement: ENFORCED | NOT_ENFORCED | UNKNOWN
  resource_limit_enforcement: ENFORCED | NOT_ENFORCED | UNKNOWN

  observed_at: Timestamp
  expires_at: Timestamp
}

QuiescenceObservation {
  environment_id: EnvironmentId
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId
  execution_identity_digest: Sha256Digest
  process_scope_digest: Sha256Digest

  result: QUIESCENT | UNKNOWN
  observed_at: Timestamp
}
```

Provider adapters do not return raw filesystem paths in these attestations. The Runtime
checks that every required enforcement field is `ENFORCED`, the attestation is current,
and its policy/input digests match the admission request. A quiescence observation is
accepted only for the current Environment/Runtime incarnation and exact private execution
identity; a different or expired observation cannot release a writer fence. `NOT_ENFORCED`
is an observed rejection, while `UNKNOWN` preserves uncertainty. Neither is coerced to
success.

`IsolationAttestation` is operational evidence, not a Grant, CapabilityActivation,
ExecutionLease, or authorization. It is held in Runtime-local private state and cannot be
supplied by an Agent or Operator request. `UNKNOWN`, `UNAVAILABLE`, a version mismatch,
or a provider that cannot prove the requested restriction fails admission closed. Native
agent permissions, hooks, plugins, MCP, and subagents remain intact, but their effective
access is still constrained by the OS boundary and LiteCowork Gateway. A native direct
capability that bypasses required Trust/Effect controls remains unavailable for that
Attempt; prompt instructions and adapter configuration are not substitutes for operating
system enforcement.

Provider stop/cancel acknowledgement is not proof that descendants have stopped. Before
releasing a writer lease, settling an Attempt as safely stopped, reusing a writable root,
or destroying an Environment, the provider must return a current-incarnation
`QuiescenceObservation` tied to the provider execution identity. Only a positive
`QUIESCENT` observation permits those transitions. `UNKNOWN`, timeout, lost provider
identity, or a Runtime restart leaves the Attempt/environment fenced and blocks replacement
writers until recovery revalidates process identity or the owner resolves the blocker.

Local containment is qualified separately by supported OS and implementation mechanism.
An OS/provider combination that has not passed the adversarial system cases is reported as
unavailable and cannot run native Task work. It may still support owner-only file
management and read-only preview. Linux qualification does not imply macOS or Windows
support.

### GitWorktree
Creates an isolated Git worktree pinned to a base commit/branch. It isolates Git changes
and supports deterministic review/merge, but it is not a filesystem or process security
boundary. Native-agent execution still requires an independently qualified OS containment
provider and exact read/write root enforcement.

### Container
Disposable or persistent container with bounded resource/network policy.

### VM
Stronger isolation and checkpoint options; provider-specific.

### CloudSandbox
External sandbox capability exposed through EnvironmentProvider adapter.

### RemoteMachine
SSH/agent-based remote workstation environment.

### Browser
Represents a browser execution context/session. DOM/accessibility/vision operations themselves are capabilities/provider tools.

### Desktop
Represents a human-visible OS session. Computer-use intelligence remains a capability/provider.

## Lifecycle

Provider owns provisioning mechanics. EnvironmentManager owns canonical state and cleanup policy.

Environment is destroyed only when:
- no authoritative Attempt uses it,
- retention policy allows destruction,
- required checkpoints/artifacts are committed,
- unresolved effects do not require the environment for reconciliation.

## Lifetime and reuse

Every Environment declares a lifetime:

```text
ATTEMPT
TASK_RETAINED
WORKSPACE_PERSISTENT
```

Lifetime and sharing scope are independent. Lifetime states how long a provider may
retain an Environment; sharing scope states which principals/tasks may request reuse.
Both are checked during placement. Missing sharing scope defaults to `ATTEMPT_PRIVATE`.
`USER_SHARED` and `WORKSPACE_SHARED` never imply shared write access or authority.
Reuse requires a fresh authorization and current Environment health check.

## Changing persistent Environment sharing

V1 permits an owner to change a `WORKSPACE_PERSISTENT` Environment between
`COWORKER_PRIVATE` and `WORKSPACE_SHARED` through an explicit versioned command. The
Environment must be `SUSPENDED`; there may be no active Attempt, CapabilityInvocation,
EnvironmentControlLease, unresolved Effect, or checkpoint hold. The command does not
resume, copy, clone, or attach the substrate. Changing to `COWORKER_PRIVATE` requires an
active or paused same-Workspace Coworker; changing to `WORKSPACE_SHARED` clears the
Coworker owner. `USER_SHARED` remains unavailable. The mutation and
`environment.sharing_scope.changed.v1` event commit together. Every later attachment
still checks current Environment health, resource freshness, Workspace policy, Trust,
budget, and fresh Attempt authority.

`ATTEMPT` is the default and is eligible for cleanup after that Attempt settles and all
checkpoint/effect-reconciliation holds clear. `TASK_RETAINED` may be reused only within
the owning Task and its explicit recovery policy. `WORKSPACE_PERSISTENT` is an explicit,
user-visible Environment that may retain files, installed dependencies, or processes
between Tasks. It is a provider-owned resource with Workspace owner, retention/expiry,
cost, storage, network, backup, and cleanup policy; it is not implicit in a Task checkpoint
or ordinary cloud sandbox. Reuse never carries grants, approvals, SecretLeases,
EnvironmentControlLeases, or ExecutionLeases into a new Task/Attempt. Each use rechecks
identity, Environment health, resource revisions, trust policy, and current authorization.
Persistent Environments are not destroyed by ordinary Attempt cleanup and require an
explicit owner action or expiry policy.

Providers must report whether an Environment survives a Runtime incarnation. A process
bound to a local daemon is reattached only after process identity verification; a cloud VM
may persist, but its provider must independently verify that the recorded handle points to
the expected Environment before use.

## File exposure

`expose_file` resolves a ResourceRef into an environment-accessible path or mount.

Rules:
- content identity must be checked when reference is revision/digest pinned.
- writable exposure requires explicit grant/scope.
- mutable external files should include version/etag information where available.
- no silent full-home-directory mounts.

## Network exposure

Ports are denied by default for isolated environments unless the provider or Task explicitly requires them.

## Checkpoint semantics

EnvironmentCheckpoint is an optimization, never Task truth. Its opaque provider handle is
stored only in an incarnation-scoped Runtime-local binding and is never accepted as
authorization. The checkpoint digest identifies provider-reported snapshot content; it
does not make that content available to another Runtime. Cross-Runtime Task recovery uses
the ResumePacket and pinned Resource/Artifact inputs to create a new Environment.

A provider that exports actual snapshot bytes may publish them as a content-addressed
`portable_snapshot_ref`; its BlobRef digest must equal the checkpoint digest. Only that
form is eligible for cross-Runtime restore, and only when the receiving provider supports
the exact checkpoint format/version. Blob replication and backup must satisfy both the
Environment's `INCLUDE_CHECKPOINTS` policy and the Workspace replication policy. A
provider-only checkpoint has no portable ref and is never presented as portable merely
because a digest exists. Restore always rechecks provider compatibility, Environment
identity/health, current TaskSpec, grants, current lease ID/epoch and Runtime incarnation,
the private fencing credential at the enforcing provider, and Effect state. The credential
travels only over authenticated private Runtime/provider control; it is not included in the
restore request visible to an Agent or Operator.

A restored Environment receives current TaskSpec, grants, and Effect state. Its enforcing
provider receives the current lease/fence over private authenticated control, never through
the Agent-facing restore request.

## Provider result and isolation rules

- `probe` reports explicit feature flags and limits. Unknown support is unavailable, not
  assumed.
- `create` and `destroy` are idempotent for the same provider request ID. A timeout is
  reconciled through provider state before retrying a non-idempotent allocation.
- Provider handles are opaque, scoped to the Environment, and never treated as user
  authorization.
- `execute` receives a bounded request, Environment, Task/Attempt, lease identity, and
  execution policy. The provider must reject a stale fence for any mutation it mediates.
- `expose_file` resolves a ResourceRef only after TrustService validates resource scope.
  It rejects path traversal, checks pinned digest/revision, and exposes only the selected
  file or folder. No implicit home-directory mount exists.
- Secret bytes are provided only through a current SecretLease, never by serializing a
  SecretRef into an agent prompt or ordinary Environment manifest.
- Network policy is deny-by-default for isolated containers/VMs. A requested exception
  includes destination/protocol scope and passes TrustService policy.
- Output crosses the Environment boundary as a digest-verified ResourceRef/Artifact;
  the provider's private filesystem is not itself durable Task state.
- Environment state changes are written by EnvironmentManager and emitted as
  `environment.*` events. Provider logs do not directly change Task or Attempt state.
- An application attachment records whether the provider attached to a pre-existing user
  process or launched a process it owns. Cleanup may stop only a LiteCowork-launched
  instance under the explicit Environment cleanup policy; it never closes a pre-existing
  application. Browser/desktop UI permission and login/session requirements are rechecked
  at each Attempt.

## Attempt binding

An Attempt binds one Environment ID and provider version/identity at creation. If that
Environment is lost or cannot be restored, create a new Attempt and Environment unless
the same live lease and provider contract permit safe same-Attempt recovery. A replacement
Environment receives the current TaskSpec revision, ResumePacket, active grants, current
lease/fence, input references, and reconciled Effect state.

Providers are qualified with conformance tests for isolation, stale-fence rejection,
resource exposure, cleanup, crash recovery, and output integrity before their offers are
advertised as eligible.

Parallel coding workers use distinct Git worktrees or private overlays by default. Shared
writes require an explicit provider lock/lease or an ordered merge protocol. Browser and
desktop Environments may persist under an explicit scope, but exactly one current
`EnvironmentControlLease` owns user input at a time. A provider unable to prove that
fence is not eligible for concurrent agent/human control. Environment reuse never carries
ExecutionLeases, CapabilityGrants, Approvals, SecretLeases, or control ownership between
Attempts.

## Workspace archive

A persistent workload must be stopped/suspended and observed quiescent before its Workspace
can become read-only. Admission is fenced during archive; unknown provider state blocks
archive rather than pretending a background process stopped. Retained filesystem and
checkpoint state is preserved under the Workspace's storage/backup policy. Archive does
not itself authorize destructive Environment cleanup.
