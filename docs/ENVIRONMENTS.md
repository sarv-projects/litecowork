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
  task_id
  attempt_id?
  isolation
  resource_limits
  filesystem
  network_policy
  secret_leases[]
  source_resources[]
  ttl?
  cleanup_policy
}
```

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

EnvironmentCheckpoint is an optimization, never Task truth.

A restored environment must still receive current TaskSpec, grants, lease/fencing token and effect state.

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

## Attempt binding

An Attempt binds one Environment ID and provider version/identity at creation. If that
Environment is lost or cannot be restored, create a new Attempt and Environment unless
the same live lease and provider contract permit safe same-Attempt recovery. A replacement
Environment receives the current TaskSpec revision, ResumePacket, active grants, current
lease/fence, input references, and reconciled Effect state.

Providers are qualified with conformance tests for isolation, stale-fence rejection,
resource exposure, cleanup, crash recovery, and output integrity before their offers are
advertised as eligible.
