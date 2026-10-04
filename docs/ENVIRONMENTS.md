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
Direct local workspace. Best for user files/device resources. May be LOCAL_BOUND.

### GitWorktree
Creates isolated git worktree pinned to base commit/branch. Preferred for parallel coding workers.

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

## Workspace archive

A persistent workload must be stopped/suspended and observed quiescent before its Workspace
can become read-only. Admission is fenced during archive; unknown provider state blocks
archive rather than pretending a background process stopped. Retained filesystem and
checkpoint state is preserved under the Workspace's storage/backup policy. Archive does
not itself authorize destructive Environment cleanup.
