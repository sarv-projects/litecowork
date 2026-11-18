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
| `TASK_SHARED` | Task | Read-only Task-planning Environment pinned to one TaskSpec revision and Resource set; execution sharing is not permitted in V1 |
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

Task planning and execution use distinct Environment identities. A planning session uses
a Task-owned `TASK_SHARED` / `TASK_RETAINED` Environment with read-only filesystem access,
bound to the current TaskSpec revision, Workspace, pinned Resources and Runtime
incarnation. Its public Environment ID may be stored on the AgentSession; attestation
details and provider handles remain in Runtime-private incarnation-scoped storage. An
execution, delegated-child or verifier Attempt is admitted only with a newly prepared
`ATTEMPT_PRIVATE` Environment owned by that exact Attempt. Planning and prior Attempt
Environments are never reused.

The provider/Attempt boundary has two distinct launches. Before the short admission
transaction, Runtime first persists a private, owner-only preparation-journal record, then
starts a trusted fixed-identity idle supervisor inside the requested OS scope. The
supervisor applies the exact mounts, namespaces, limits and network policy and runs only
bounded built-in enforcement probes. It does not start the native harness, agent tools,
credentials, session or user code. Runtime derives the attestation from the live held scope,
rechecks its process-start identity and policy/input digests immediately before admission,
then commits Environment metadata and the appropriate Session/Attempt/lease reservation.
Only a resolved durable receipt permits a one-time private handoff that starts the native
provider/harness inside that same held scope. If the provider cannot preserve that exact
scope across the commit boundary, admission fails closed; a detached capability probe or
a short-lived test scope cannot attest the later worker's Environment.

The preparation journal is a Runtime recovery aid, not an Environment aggregate, public
event, Operator projection, backup item, credential store or authority source. It records
the proposed owner identities, current/predecessor Runtime incarnation, normalized config
and input digests, and an exact provider-scope reconciliation identity; it contains no
native credentials. The journal is durably written before provider creation and is removed
only after a confirmed handoff or observed cleanup. On startup, Runtime reconciles each
entry against the idempotency receipt and exact provider/process identity. A committed
entry follows ordinary prior-incarnation Attempt recovery; an uncommitted entry is cleaned
only after exact provider identity and quiescence are observed. Unknown identity,
quiescence or journal integrity retains a typed blocker and prevents replacement admission.
Never clean up by PID or process name alone. If admission storage may have committed, query
the original receipt before launching or destroying the scope.

Storage's nullable AgentSession `environment_id` supports historical and Conversation rows;
the Task admission owner must require a valid non-null binding for every new Task-scoped
session before native provider/harness start.

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
Attempt/session admission, current Runtime incarnation, provider archive/legacy-implementation-c7e9c05/version,
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
For the Linux Bubblewrap provider, these writes live on one size-capped sandbox-private
`/workspace` tmpfs that also backs the worker's `/tmp`; the exact staged input tree is
mounted read-only below it. The cap comes from the admitted `EnvironmentConfig` storage
limit and is enforced by tmpfs, not inferred from a post-run scan. The host staging/output
directory is used only as a trusted launcher working directory and is never bind-mounted
into the worker. A positive cap that cannot be applied or observed fails closed. Any
candidate intended to become a durable proposal must enter through the managed Resource
revision lifecycle; the transient scratch tree is not itself a committed Artifact or
Effect payload. For Feature 005, the current revision 1.1 implementation proposal starts a
trusted inner runner as the Bubblewrap command. The native harness writes one candidate to
the fixed `/workspace/outputs/proposal` path on the same bounded tmpfs. The runner becomes
a child subreaper, prevents the untrusted child from reading the runner's own `/proc`
descriptors, waits for the harness and reparented descendants, opens only that regular file
without following symlinks, enforces the accepted Step byte limit, and emits one
versioned, length-framed payload over the existing scope stderr pipe. The harness retains
its native stdin/stdout protocol; harness stderr is separately framed as logs. The Runtime
may parse the candidate frame into a bounded transient buffer, but it returns bytes to
Task Runtime only after the exact process scope reports recursive cgroup-v2
`populated 0` and the direct launcher is reaped. No host output/staging directory or
candidate socket is exposed. The candidate frame contains no host path, target path,
ResourceRef, digest or authority. Runtime computes the content digest and persists the
bytes as an immutable Resource revision with `created_by_attempt` provenance. This capture
is data only and does not issue Trust authority or create an Effect. As of 2026-11-18,
lower-level CODE exists for the Bubblewrap `--cap-drop ALL` launch plan, the trusted
scope-gate runner, fixed-path bounded file reading and the versioned stderr frame format.
The daemon also has CODE for mapping a ready current-incarnation Attempt-private
Environment and exact staged Resource pins into the supported Bubblewrap launch spec,
including fail-closed resource-limit conversion. Five mapper and three capture-session
tests pass on WSL Ubuntu 24.04, and the staging crate passes 38 tests. The private capture
session starts a mapped Bubblewrap scope, retains native harness stdin/stdout, drains the
separate bounded candidate channel concurrently, and releases bytes only after recursive
cgroup quiescence and direct-launcher reap. Timeout before a known exit result retains the same session for retry; a nonzero exit drains/discards its frame and remains terminal on retry. Cancellation discards output after bounded kill-and-wait. These are CODE tests using a
fake process scope, not a real provider run. A separate opt-in WSL Bubblewrap integration
test launches the locally built scope-gate runner as PID 1, blocks the child from forging
a candidate frame, and waits for a delayed reparented descendant before collection. It
bypasses `ManagedScope` and systemd/cgroup supervision, so it does not exercise the daemon
capture helper or establish provider quiescence. A second opt-in WSL Bubblewrap test runs
the production `prepare_bubblewrap_command` output directly, reads back its fixed 1 MiB
`/workspace` tmpfs cap, writes through both `/workspace/outputs` and the in-sandbox `/tmp`
symlink, and observes `ENOSPC` when both paths consume the shared quota. This exposed and
fixed a prior invalid host-side `--bind /workspace/tmp /tmp`; it still does not test mapping
an admitted Environment's configured storage limit, `ManagedScope`, systemd/cgroup, or the
managed provider. The helper has no production Task Runtime caller and creates no attestation
or Resource. Attestation and candidate-to-Resource flow remain TARGET; capture is not
integrated and file dispatch is unavailable until provider integration and native Linux
evidence are complete. The 2026-11-19 T01 cross-check found that this capture helper cannot
attest before Attempt admission because it launches the candidate-producing harness. Source
now has a CODE-level `--held-attempt-runner` protocol that announces readiness inside the
requested Bubblewrap namespace and blocks before native harness creation until a one-shot
signal; its builder, protocol and no-retry helper tests pass. It does not yet run same-scope
enforcement probes, resolve a SQLite admission receipt or have a Task Runtime caller. A
separate `RuntimeLocalPreparationJournal` now has CODE for private durable intent/phase/receipt
records and fail-closed recovery classification; the focused daemon test filter passed 22/22
on native Windows. This primitive is not wired before provider creation or into startup
recovery. Before a caller, pending-entry enumeration still needs no-follow/regular-file/
owner/mode/canonical-key validation, and phase updates need serialization across shared or
cross-process users. The revised target still requires the trusted idle supervisor, probes,
exact receipt lookup, attestation, admission and restart reconciliation; the contract remains
IN_REVIEW, and no current attestation or qualification claim is made.
Import limits apply before extraction or copying; an unsupported format is rejected or
exposed as opaque bytes, never silently treated as parsed content.

For the first mediated file replacement (Feature 005 B-18), a host path string is not an
Environment capability. Runtime resolves the WorkspaceRoot's private local identity
binding and opens the root before constructing an opaque, process-local handle plus a
validated relative path. The code uses Linux descriptor-relative `openat2` resolution and
Windows handle-relative no-reparse operations; the Windows replacement path requires an
optional narrowly scoped root handle and NTFS, and fails closed if either is unavailable.
Neither implementation turns a Task-supplied path into a new host root. The file primitive
has focused CODE tests, but it is not wired into a Task Environment/provider or mediated
dispatch. It does not attest Environment isolation and does not grant write authority.
Trust and the committed dispatch transaction remain required, and Windows Task execution
remains unavailable until B-35 qualifies.

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

## Desktop Coworker environment availability target

A Coworker has no implicit computer access. Browser, filesystem, terminal and installed local applications are capabilities only after a qualified Environment provider enforces the requested roots, process containment and egress. A merely installed local app is not a qualified automation adapter. Locked display sessions, unavailable browser profiles and sleeping devices are environment blockers; restart may require a new Attempt with fresh lease and authorization. Do not migrate local GUI sessions to cloud by relabeling placement. On-demand activation must obey process/CPU/memory limits despite an unlimited configured connection catalog.


## Consumer file-root mount modes — competitor-informed target

For general desktop users, authorized local roots should support explicit host-enforced modes: **read only**, **read/write**, and **read/write/no delete** where the platform provider can enforce them. This follows the useful security shape documented by Claude Cowork: the user chooses a comprehensible boundary once instead of reviewing shell commands they may not understand.

Mount mode is an Environment/resource boundary, not a model instruction. Symlink/path resolution must occur before containment checks; a child path may not escape an authorized root. Credentials remain outside the execution guest/process where supported. A provider that cannot reliably enforce a requested no-delete/read-only boundary must mark that mode unsupported rather than simulate it in prompt text.


## Coworker-private computer semantics

A Coworker may prefer or own a COWORKER_PRIVATE persistent Environment where explicitly
created, but the product phrase "its own computer" is not a requirement to provision a
permanent VM/container. Default work can use Task/Attempt-private Environments and scoped
browser/terminal/computer leases.

Connector credentials remain separate SecretStore/Connection state and are injected only
for authorized operations; cloning/checkpointing an Environment never clones provider
refresh tokens or makes another Coworker eligible for them.

## V1 detailed design integration — 2026-10-12

The qualified Chrome/browser provider HLD/LLD is CHROME-BROWSER-LLD.md. Tauri WebView is NOT Chrome. Managed browser uses isolated persistent directory; existing Chrome needs user-visible per-session permission and high-trust scope; default Chrome user profile cannot be automated by pointing Playwright at it. Human takeover requires epoch-fenced single input actor, fresh DOM/a11y observation after return, and re-evaluated Trust/Effect/leases. Browser surface and provider are distinct.

## Shell/PTY Environment specialization — V1 target

[Integrated Terminal LLD](V1-INTEGRATED-TERMINAL-LLD.md) refines EnvironmentProvider behavior for local PTY and Windows ConPTY, Linux/macOS PTY, WSL, controlled cwd, session incarnation, viewport stream, manual-vs-Agent Attempt ownership and control epoch. A WebView terminal cannot spawn a process directly. A stopped Runtime/PTY is LOST, not magically resumed; the existing Environment/Task/Effect Trust authority stays intact. New command routes/services are unpromoted proposals until schema and native OS qualification.


## Non-normative desktop sandbox comparison - 2026-10-31

See [DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md) for the ZCode plugin guest boundary comparison. Its Chromium partition/CSP/owner checks are useful defense-in-depth for rendered interactive content, but do not prove LiteCowork's process/filesystem/network isolation. Preserve the Environment contract: exact readable/writable roots, process-tree containment, egress policy, stale-fence rejection and provider attestation must be enforced by the selected provider; a WebView, origin allowlist or Git worktree is not sufficient.

## Requested live-computer scope revision - 2026-11-12

The selected screen/window Computer surface is specified in [Computer observation and control LLD](COMPUTER-OBSERVATION-AND-CONTROL-LLD.md) and Feature 008. Observation, disclosure to a native agent/model and injected input require separate explicit permissions and incarnation-scoped provider support. A manual-host desktop is not an attested Task Environment. Screen capture/input capability must not bypass native Task containment, network, filesystem or credential boundaries. No provider is qualified by this scope revision.
