# Runtime Mesh

The Runtime Mesh lets one logical Workspace coordinate local and remote
`agentcoworkd` instances. It is the system-wide home for cloud continuation, not a
special case of Environment Fabric.

## Runtime roles

A RuntimeDescriptor identifies a Runtime and device, version, platform/architecture,
roles, available agents, capability/environment offers, resource capacity, trust zone,
availability, and last-seen time. Roles may include workspace hub, executor,
resource node, channel host, trigger host, and operator endpoint.

One standalone Runtime may combine all required roles. Connected Runtimes use outbound
authenticated transport by default; private network, SSH, or WireGuard/Tailscale-like
transport can be optional self-hosted choices, not protocol dependencies.

## Responsibilities

- Runtime identity and pairing.
- Presence and health, with bounded polling and typed unavailable states.
- Domain-event and immutable-artifact replication.
- Agent/capability/environment inventory and placement eligibility.
- Execution leases, epoch fencing, handoff, and recovery coordination.
- Remote invocation and channel/trigger availability.

The initial Workspace has one authoritative Hub. Other Runtimes may execute work and
replicate state, but do not become independent authorities for the same lease epoch.
Cross-runtime coordination pauses if the Hub is unavailable unless a later qualified
design explicitly changes that rule.

## Replication

Replicate versioned domain events, Task revisions, artifact manifests and blobs, selected
Task inputs/resources, capability locks, approvals, and execution ownership. Do not
replicate database files or native agent private state. A Workspace policy selects
whether to sync metadata only, active Task inputs, selected folders, or an explicitly
chosen full workspace. The default cloud-enabled policy is active Task inputs.

ResourceRefs carry identity across Runtimes rather than assuming a local path has the
same meaning everywhere. Examples include content-addressed artifacts, workspace file
identity/version, repository commit/path, connector resource/revision, browser
session/tab, and vault secret reference.

## Lease and handoff

An ExecutionLease binds one Task/Step/Attempt to one Runtime and epoch. Renewals extend
its expiry; takeover creates the next epoch. Every Core-mediated mutation checks the
current fence. Graceful handoff checkpoints the Task, flushes events, replicates required
artifacts, reconciles open Effects, releases the lease, then allows the destination
Runtime to create a new Attempt. Do not claim that a running process moved.

Unexpected Runtime loss waits for lease expiry, reconciles Effects, evaluates
continuation eligibility, then either starts a fresh Attempt, waits for a local resource,
requires explicit handoff, or asks the user.
