# Runtime and Worker Lifecycle

This document defines process ownership and lifecycle for the LiteCowork Operator,
`litecoworkd`, agent hosts, capability activations, Environments, and OS applications.
Runtime identity and cross-device coordination remain in `RUNTIME-MESH.md`; this document
defines one Runtime's local start, recovery, drain, sleep, and lazy dependency behavior.

## Three independent lifecycles

```text
Operator surfaces (desktop / web / mobile)
    may connect and disconnect at any time
                    │ Operator API
                    ▼
litecoworkd (durable local Runtime)
    scheduler, journal, local resource observation, supervision
                    │ scoped lifecycle leases
                    ▼
workers and services (agents, providers, apps, Environments)
    started only when admitted work or an authorized trigger needs them
```

The desktop Operator is a separate executable from the Runtime service. Closing a window
or quitting the UI does not stop `litecoworkd` when background operation is enabled.
Workers are never launched merely because they are installed, configured, or visible in
Discover. Runtime recovery may inspect prior owned processes and refresh offers, but it
does not start every Agent, MCP server, browser, office application, or Environment.

## Runtime startup policy

```text
RuntimeStartupPolicy = MANUAL | LOGIN_BACKGROUND | ALWAYS_ON_SERVICE
```

- `MANUAL`: the Operator starts the per-user Runtime on demand; the OS does not guarantee
  startup after logout/reboot. Closing the last Operator offers a safe drain and stops the
  daemon only when there are no active local Attempts, required local TriggerHosts,
  enabled local observation duties, or explicitly shared services. Otherwise it remains
  alive or reports the unresolved duty. WorkspaceRoots may become stale while stopped
  and are reconciled on next start.
- `LOGIN_BACKGROUND`: the OS starts the unprivileged per-user Runtime at login. The GUI may
  exit while the Runtime and its authorized background duties continue. This is the
  recommended personal-desktop mode.
- `ALWAYS_ON_SERVICE`: a server/VPS service manager starts the daemon under a dedicated
  unprivileged account. A cloud Runtime is normally always on.

Platform integration uses the native service manager: macOS LaunchAgent; Windows per-user
Scheduled Task or equivalent supported user service; Linux `systemd --user`; server Linux
system service under a dedicated account. Installation, update, status, and uninstall are
owned by RuntimeLifecycleService and require the OS permissions appropriate to that scope.
The Operator is not installed as a privileged service. The scheduler never depends on an
Automation whose purpose is to start the scheduler; OS/service-manager startup owns daemon
startup, while Automation triggers create user work.

Desktop builds therefore install/register two programs with distinct lifetimes: the
Operator shell (window/tray/optional global Quick Entry) and `litecoworkd` (headless
Runtime). On-demand startup uses authenticated local IPC. The Operator may request a
Runtime start or stop but cannot report the daemon ready until the Runtime lifecycle
projection confirms it.

## Runtime incarnation

`RuntimeId` identifies a paired installation/device across daemon restarts. Every daemon
process that acquires the single-instance lock creates a new `RuntimeIncarnationId` before
recovery begins:

```text
RuntimeIncarnation {
  runtime_id
  runtime_incarnation_id
  process_started_at
  litecowork_version
  recovered_from_unclean_shutdown
  recovery_state
  ready_at?
  stopped_at?
  version
}
```

`RuntimeIncarnation` is registered as durable Mesh metadata. The OS boot ID and local
diagnostic references live in `RuntimeIncarnationLocalObservation` and never leave the
Runtime or enter Workspace backups. An incarnation is not a second device identity. Agent host process
references, CapabilityHostInstance observations/provider handles, process/application
ownership tokens, watcher cursors, AgentEndpoint command/socket/URL locators, Environment
provider locators, checkpoint handles, ResourceLocation private locators, raw file identity
tuples, offers, and local resource observations are tagged with the incarnation that
observed or created them. `AgentEndpointBinding`, `EnvironmentProviderBinding`,
`EnvironmentCheckpointProviderBinding`, `ResourceLocationBinding`, and
`FileIdentityBinding` rows are local and omitted
from Workspace backups. A new incarnation may receive a binding only after its provider
verifies and reattaches the underlying resource or WorldIndexer revalidates its root/file
identity; it never treats an old binding as current merely
because its RuntimeId matches. A PID alone is never a process identity.
Use the platform process start identity plus executable/owner evidence where available.
After a new incarnation starts, old handles are stale until the owning adapter verifies
them; unverifiable handles are treated as lost and reconciled. Persistent cloud Environments
may survive a daemon incarnation, but must be reattached and checked by their provider.

Each admitted Attempt, AgentSession, ExecutionLease, EnvironmentControlLease, and
provider binding pins the exact Runtime incarnation that owns its execution. A daemon
restart never updates an old Attempt to the new incarnation. After lease expiry or
authoritative release, reconciliation marks the old Attempt abandoned as appropriate;
eligible continuation starts a new Attempt with a fresh AgentSession and higher lease
epoch. A new AgentSession may replace a lost session under the same Attempt only while
the original Runtime incarnation and lease remain valid.

## Boot and recovery sequence

```text
STOPPED → STARTING → RECOVERING → READY | DEGRADED
```

1. Acquire the per-installation single-instance lock; reject a duplicate daemon.
2. Open StateStore and BlobStore, apply forward migrations, and validate journal/checkpoint
   integrity. Unsupported schema or corruption leaves the Runtime unavailable for work.
3. Persist the new RuntimeIncarnation in local StateStore as `RECOVERING`; keep any OS boot
   identifier in local-only observation state. Do not advertise execution readiness.
4. Reconcile previous-incarnation AgentHost/CapabilityHost/Environment/application handles
   by verified process identity or provider observation. Do not trust a PID or replay a
   command just because its process is gone.
5. Reconnect to the Workspace Hub and register the authenticated incarnation before
   publishing presence, offers, or domain events that reference it. Exchange
   Workspace-scoped replication receipts, fetch authorized state blobs/snapshots, and
   reject stale leases/fences.
6. Recover claims idempotently, re-evaluate expired leases through their authority, and
   reconcile open Effects/CapabilityInvocations before admitting replacement work.
7. Restore Automation cursors/deadlines and apply the pinned MisfirePolicy. Startup does
   not immediately run all overdue work or launch the workers that might perform it.
8. Resume authorized WorldIndex watchers. Mark observation gaps UNKNOWN/STALE and schedule
   bounded rescans before claiming current freshness.
9. Revalidate each ChannelIngressCursorBinding against the new incarnation and current
   host epoch. Mark stale opaque cursors RECONCILIATION_REQUIRED; use provider replay from
   the last committed event ID when supported. Do not start polling/acknowledging channel
   ingress while continuity or receipt durability is unresolved.
10. Refresh RuntimeOffers and probe non-running adapters without starting worker processes.
11. Publish presence as ONLINE only after incarnation registration and when storage and
    required coordination are ready; use DEGRADED with explicit blockers when safe
    read-only operation is possible but a required subsystem is not ready.

The order may overlap independent recovery tasks, but readiness gates and safety rules are
fixed. A late Hub, corrupt checkpoint, unresolved external Effect, or failed required
storage adapter cannot be hidden by an ONLINE projection.

## Sleep, lock, shutdown, and restart semantics

| Host state | Local Runtime | Cloud Attempt | Local-bound Attempt | Trigger behavior |
|---|---|---|---|---|
| Operator window closed | Follows startup policy; usually stays alive | Continues | Continues if Runtime remains awake | Continues on its assigned TriggerHost |
| User session locked | Usually alive; OS-specific permissions may be reduced | Continues | Continues only if its required desktop/session access remains valid | Assigned host continues |
| Suspend/lid sleep | OS suspends it; presence expires later | Continues | Stops making progress and is reconciled after wake | Hub triggers continue; local triggers record an observation gap/misfire |
| Powered off | Offline | Continues | Waits for the local Runtime | Hub triggers continue; local trigger policy applies on return |
| Daemon crash | Service manager may restart it | Unaffected | New incarnation recovers before resuming | Durable trigger definitions/cursors reload; no blind replay |
| Graceful Runtime stop | Drains and records clean stop | Unaffected | Pauses/blocks after safe settlement | Local trigger host stops; Hub schedules remain independent |

Locking a laptop does not guarantee GUI automation remains possible: desktop/browser
requirements are rechecked. Sleep/offline does not mean a Task vanished. A local-bound
Task retains its blocker and can continue only after fresh resource, application, lease,
and Effect checks. Remote Tasks are not silently migrated as running processes; eligible
continuation creates a new Attempt under a new lease epoch.

`preview_stop` reports active Attempts, local trigger duties, roots, managed processes
and eligible handoffs without changing admission. After user confirmation, `request_stop`
checks the expected incarnation and current dependencies before setting `DRAINING` and
rejecting new local admission. `CANCEL` changes nothing. The user can move eligible work
through ordinary handoff or stop with explicit consequences; a stale preview does not
reserve dependencies or authorize shutdown of a restarted daemon. A graceful
stop checkpoints safe work, reconciles Effects, releases leases, releases scoped provider
activation references, stops only LiteCowork-owned worker processes, marks local
observations stale as needed, and records a clean incarnation stop. Forced termination may
leave Ambiguous Effects and Abandoned Attempts; next startup reconciles them before retry.
Stopping the Runtime is distinct from cancelling Tasks and never implies that a remote
Runtime stopped.

Optional future `WakeProvider` support is best-effort only. `WakePolicy` is `NEVER`,
`TRY_WAKE`, or `REQUIRE_RUNTIME_AWAKE`; hardware, power state, lid policy, and OS support
are rechecked at run time. A powered-off device is never assumed wakeable. Failed wake
leaves the occurrence waiting or applies its configured misfire policy; it never authorizes
cloud access to a local resource.

## Lazy AgentHost lifecycle

Discovery and installation do not imply a process. `AgentHostSupervisor.ensure_ready`
runs only when a Conversation turn, Task-planning session, or admitted Attempt needs the
selected AgentEndpoint and a current, unexpired RuntimeOffer plus matching
`AgentEndpointBinding` are available. It may spawn a local adapter, attach to a shared local service,
attach to a user-owned external process, or connect to a remote API/A2A endpoint. Remote
endpoints need no local OS process. Embedded SDKs run inside a supervised host process.

```text
AgentHostInstance {
  instance_id
  runtime_id
  runtime_incarnation_id
  agent_profile_id
  agent_endpoint_id
  hosting_mode
  state: STARTING | READY | BUSY | DEGRADED | STOPPING | STOPPED | FAILED
  process_identity_ref?       # opaque; never PID alone
  ownership: LITECOWORK | EXTERNAL | REMOTE
  active_session_count: derived from local AgentSessionHostBinding rows joined to
    nonterminal AgentSessions; never maintained as an independent counter
  started_at
  last_used_at
  idle_since?
}

HostingMode = REMOTE_API | REMOTE_A2A | LOCAL_SHARED_DAEMON |
  LOCAL_PER_SESSION | EMBEDDED_SDK | EXTERNAL_PROCESS
```

`AgentHostInstance` is Runtime-operational state, not a Workspace domain object or
replicated Task truth. A durable `AgentSession` records its Runtime/incarnation, endpoint
identity, scope, and lifecycle. The local `AgentSessionHostBinding` stores the host link
and opaque adapter-native session/resume handle; it is excluded from event state and
Workspace backup. A host's displayed session count is derived from live bindings, so
crashes cannot leave a stale reference count. It is not necessarily an OS process:
remote/API endpoints use a connection handle and report no local process identity. Host
process startup success precedes
`AgentSession.ACTIVE`; failures return a typed blocker without creating fake work.

`ensure_ready` is idempotent and single-flight for the same endpoint, Runtime incarnation,
and hosting mode. Concurrent sessions share a host only when the endpoint advertises
compatible concurrency and isolation; otherwise the supervisor creates isolated instances
or queues admission. A new session must acquire its use reference before a host can enter
`STOPPING`. If a session arrives during idle shutdown, the supervisor either cancels the
shutdown before the process stop commits or creates a fresh host instance; it never attaches
to a half-stopped process. Session-count displays are derived from authoritative use refs,
not trusted from a stale counter.

Every live adapter session retains a host-use reference. The owning coordinator closes a
session and releases that reference when its Conversation turn or PlanningAssignment
settles, when it waits for user input, or when an Attempt durably yields while a separately
owned CapabilityInvocation/provider operation continues. The durable ConversationTurn or
Attempt may remain open; its next agent interaction uses a fresh AgentSession populated
from the bounded durable projection. A transient disconnect may resume the same session
only after adapter validation in the same Runtime incarnation. When the reference count
reaches zero, a locally spawned host is stopped after its host-specific idle TTL (initial
default 10 minutes for interactive CLI adapters; providers may set another bounded policy).
Shared/external processes are never killed unless LiteCowork started and owns them and the
configured cleanup policy permits it. `AgentWarmPolicy` is an optional latency
optimization (`COLD`, `KEEP_DEFAULT_WARM`, `KEEP_RECENT_WARM`); default is `COLD`, and
prewarming never starts every installed agent.

Changing a Task lead binding affects future planning assignments. Existing Attempts stay
pinned to their original binding, endpoint, Runtime, and Environment until they settle or
are safely cancelled. The prior planning session drains/closes; the replacement host starts
only when new planning or execution is actually admitted. Changing an agent's model/options
does not by itself change the AgentProfile or start another host. Adapter negotiation says
whether an option is mutable in-session, requires a fresh AgentSession, or is unsupported.

## Lazy capabilities, Environments, and applications

Installed, offered, startable, process-ready, authorized, activated, and attached are
distinct states. RuntimeOffer carries compatibility evidence and an `OfferReadiness`:
`AVAILABLE`, `STARTABLE`, `STARTING`, `READY`, `BUSY`, `DEGRADED`, `OFFLINE`, `NEEDS_AUTH`,
or `UNAVAILABLE`. The observation is scoped to Runtime/incarnation and expires. Placement
rechecks it before admission.

LiteCowork requests a scope-appropriate CapabilityActivation only after selection and
grant. LiteCowork's CapabilityHostSupervisor keeps a Runtime-local view of each normalized
provider instance, current health/readiness observation, and the number of LiteCowork
Activations that reference it. The count is derived from Activation records and is not
LitePSM's global process reference count. LitePSM remains authoritative for package
installation, provider process startup/stop, isolation implementation, provider-level
health/restart, and process idle timeout. The Supervisor requests/relinquishes use
references through the future LitePSM adapter contract; it never starts or kills the
process itself. Sharing requires declared safe concurrency and a matching configuration
and Trust isolation partition. Every invocation retains its own Grant and Effect checks.
Remote MCP/connectors have a logical provider host view but need no local process. Stale or
failed health opens LiteCowork's provider circuit and blocks or triggers an explicitly
eligible fallback; it does not widen grants.

Environment providers create/attach Environments when an admitted Attempt needs them.
Applications such as Excel, Chrome, VS Code, or Figma start only when a selected Environment
requires them. The provider records whether it attached to a pre-existing application or
launched an application it owns. It may close only its own launched instance under an
explicit cleanup policy; it never closes a user's pre-existing app.

The Runtime keeps a small local application inventory/offer projection warm with the
daemon: stable app identity, installed/launchable status, supported Environment/actions,
and fresh readiness. It does not start those applications or collect window titles and
general activity. Process/window identity is resolved only for a selected app attachment
or a user-authorized Quick Entry capture. Raw process/window handles are local to the
current RuntimeIncarnation and are invalid after restart until re-probed.

An application attachment moves `STARTING -> READY | FAILED`, `READY <-> BUSY`, then
`READY | BUSY -> STOPPING -> STOPPED`. Stop is permitted only for a process whose verified
identity is marked `LITECOWORK`-owned and whose dependency references have drained; a
pre-existing USER/EXTERNAL process is detached. A Runtime restart invalidates the binding
and requires process-start identity revalidation before another action.

```text
ExecutionDependencyPlan
  = read-only, digestible Attempt-preparation result
  = candidate Runtime + prerequisite DAG + availability evidence + blockers

Prerequisite kinds:
  resource location/exposure, agent endpoint/host, Environment, capability activation,
  SecretLease, application attachment, Runtime role/capacity
```

The dependency planner is host infrastructure, not an intellectual workflow engine. It
does not create semantic Steps or decide strategy. It can prepare independent prerequisites
in parallel, but the Attempt becomes RUNNING only after all mandatory dependencies are
ready and its lease/fences are current. It de-duplicates shared prerequisites by stable
identity, validates the DAG for cycles, and reports optional versus mandatory dependencies.
Preparation is idempotent for `(attempt_id, plan_digest)`; a stale digest is rejected.
Partial failure releases only leases and resources it acquired, in reverse dependency
order, and never closes a pre-existing user application. Secrets are leased only after
their consumer is ready and are not included in the plan. Plans contain references and
safe status only, never secret bytes. Recompute after sleep, Runtime-incarnation change,
resource revision change, grant revocation, or provider/auth change; do not trust stale
readiness. Unsupported or conflicting dependencies are blockers rather than implicit
fallbacks.

## Persistent Environment lifetime

Environment lifetime is independent of Task lifetime. Default `ATTEMPT` Environments are
disposable/cleanup-eligible when no Attempt, checkpoint, or Effect reconciliation needs
them. Providers may offer explicit `TASK_RETAINED` or `WORKSPACE_PERSISTENT` Environments
for jobs/services that need files, installed dependencies, or processes to survive between
Tasks. Persistent state is user-visible, Workspace-scoped, explicitly authorized, subject
to cost/retention/backup/network policies, and never inferred from a resumable Task. Each
new Task/Attempt still receives fresh grants, SecretLeases, EnvironmentControlLease, and
ExecutionLease; reuse of filesystem/process state does not reuse authority. The provider
must reattach and revalidate a persistent Environment after Runtime restart before use.

## Operational guarantees

- Runtime discovery and resource indexing stay lightweight; workers are demand-started.
- A daemon crash/reboot never implies that an external operation did not happen.
- A worker process exists only while a live session/use reference or explicit warm policy
  justifies it.
- Runtime readiness, AgentHost readiness, CapabilityHost health, Activation readiness, Environment
  readiness, and Task state are separate projections.
- See `STATE-MACHINES.md`, `SERVICES.md`, `FLOWS.md`, and `FAILURE-RECOVERY.md` for the
  canonical transition owners and recovery sequences.
