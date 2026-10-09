# Agent Fabric LLD

## Purpose

Agent Fabric normalizes external agents without taking ownership of their internal
reasoning, model choice, prompts, or native subagent implementation. Host delegation,
worker profiles, selection, cost policy, and warmth are specified in
[`DELEGATION.md`](DELEGATION.md).

## AgentAdapter

```text
interface AgentAdapter {
  discover() -> AgentProfile
  probe() -> AgentCapabilities
  authenticate(AuthContext) -> AuthResult

  start_session(SessionSpec) -> SessionHandle
  resume_session(ResumeSessionSpec) -> SessionHandle

  send(SessionHandle, AgentInput) -> SendAck
  steer(SessionHandle, AgentInput) -> SendAck
  interrupt(SessionHandle) -> Ack
  cancel(SessionHandle) -> Ack

  attach_capabilities(SessionHandle, CapabilityAttachment[]) -> AttachmentResult
  attach_context(SessionHandle, ContextAttachment[]) -> AttachmentResult

  stream_events(SessionHandle, Cursor?) -> Stream<NormalizedAgentEvent>
  snapshot_session(SessionHandle) -> AgentSessionSnapshot? 
  close(SessionHandle) -> Ack
}
```

Optional features are capability-negotiated. Never branch on agent brand name.

## AgentCapabilities

```text
AgentCapabilities {
  protocol
  protocol_version?
  session: { resume, steer, interrupt, cancel, fork, model_switch: LIVE | NEW_SESSION | UNSUPPORTED }
  input: { text, image, file, resources }
  extension: { mcp_stdio, mcp_http, skills, plugins, dynamic_attach }
  host_instruction_delivery: NATIVE_SYSTEM | NATIVE_DEVELOPER | SESSION_GUIDANCE | CONTEXT_ATTACHMENT | UNSUPPORTED
  reporting: { tool_calls, plan, usage, native_subagents, approvals }
  environment: { cwd, extra_directories }
  limits: { max_context_hint?, max_concurrent_sessions? }
}
```

## Profiles and Workspace bindings

### Local installation inventory

The authenticated local Operator may expose a runtime-local installation inventory
before protocol negotiation. An inventory row proves only that a known executable
returned a bounded, recognized version string to its `--version` invocation. It is
not an `AgentProfile`, `AgentEndpoint`, or `AgentBinding` and does not prove auth,
provider/model access, session support, or lead/delegation eligibility. The inventory
does not persist executable paths or raw command output. Its readiness fields remain
`UNKNOWN` / `NOT_PROBED` permanently; later negotiated auth/session readiness belongs
to separate AgentProfile/RuntimeOffer observations and does not mutate the inventory
row. Creating an enabled binding, launching a turn, or mutating native config is not part
of inventory discovery. The local snapshot may be cached for at most five seconds to
coalesce concurrent UI refreshes.

For the initial desktop Codex and OpenCode adapters, creating or refreshing an
AgentProfile is an explicit owner-triggered operation, separate from installation
inventory. The selected Workspace must already authorize the exact current local Runtime
incarnation through an ACTIVE `LOCAL_ENROLLMENT` RuntimeWorkspaceBinding carrying
`EXECUTOR` and `OPERATOR_ENDPOINT`; serving the authenticated Operator does not imply this
enrollment. The Codex probe performs bounded App Server initialization, account-state
read without refresh, and one bounded model-catalog page. The OpenCode probe starts the
owned local Server and reads only the documented `/provider` connection summary and
`/config/providers` catalog routes. Its projection contains bounded provider/model IDs and
display names plus explicitly labeled reported connected-provider IDs; it never returns
native provider/model objects, options, headers, keys, URLs, or raw responses. A reported
connected provider is not proof of authentication, entitlement, or inference success.
OpenCode's catalog is display-only: session model selection is `NOT_QUALIFIED`, and its
RuntimeOffer is always incompatible until session-option semantics, Task admission,
Environment isolation, native capability mediation, process-tree containment, Effect
reconciliation, and lease fencing are integrated and qualified. Both probes stop their
owned direct process and store sanitized observations plus an expiring RuntimeOffer. They
do not start a Task, AgentSession, Environment, or inference turn, and do not prove model
entitlement or descendant-writer quiescence. Catalog metadata is not an entitlement
claim. Runtime-local executable locators remain private to their Runtime incarnation.
See [F95](FLOWS.md#f95-explicit-local-coding-agent-profile-probe-and-binding-setup) for
the end-to-end setup flow.

AgentAdapter discovery yields an `AgentProfile` for an available Runtime offer. Profiles
describe protocol/features; they do not carry credentials or imply authorization. The
owner creates a Workspace `AgentBinding` that refers to a discovered profile, optional
Runtime placement, an explicit endpoint selection policy, an opaque SecretRef, and
non-secret adapter configuration. The policy either pins one endpoint or selects an
eligible endpoint by required feature set and that binding's preferred topologies. A
profile may expose multiple stable `AgentEndpoint` identities; a Runtime-local
`AgentEndpointBinding` supplies the private command/socket/URL locator for one exact
Runtime incarnation, while expiring RuntimeOffers report current readiness. The
Operator sees the stable identity and availability, never the locator. Binding/placement
selects a compatible endpoint using topology and negotiated features, not one global protocol
ranking. If
`runtime_id` is null, placement may choose any eligible Runtime currently advertising
that profile; otherwise only the named Runtime is eligible. New
bindings are disabled until explicitly enabled. Disabling prevents new planning/Attempt
admission; sessions already admitted stay pinned and settle under their current Task,
lease, and Effect rules.

Until an adapter-specific non-secret configuration allowlist exists, V1 AgentBinding
`configuration` must be an empty object. Reject unknown configuration rather than
persisting arbitrary values that may contain credentials. Credentials belong in the
SecretStore and are referenced only through a validated `SecretRef`.

The profile list includes stable software/protocol identity plus a Runtime inventory
projection with currently observed Runtime IDs and offer expiry; it may go stale when a
Runtime goes offline. Endpoint command paths, sockets, and URLs are local bindings and are
excluded from events and Workspace backups. Binding history remains durable. Agent-specific installation and login
flows remain adapter-owned; secret bytes never enter the AgentBinding API.
Discovery/probe must not keep an agent process alive. `AgentHostSupervisor` lazily starts
or attaches the selected endpoint only when a Conversation turn, planning assignment, or
admitted Attempt needs it; process ownership, incarnation tagging, derived use counts, and
idle stop rules are in `RUNTIME-LIFECYCLE.md`. AgentSession stores the selected Runtime
and incarnation as durable provenance. The host binding and opaque native session/resume
handle live in a Runtime-local `AgentSessionHostBinding`, which is not event state and is
excluded from Workspace backup. Remote API/A2A endpoints use a local binding for their
connection/resume handle but do not imply a local process.

Local native CLI hosts start with a cleared process environment and receive only an
explicit Runtime-selected allowlist needed for the agent's native home/config/data paths,
helper executable search path, operating-system support, and temporary directory. The
daemon's arbitrary environment is not inherited. Provider credentials remain in the
agent's native credential store or an explicitly authorized SecretLease flow; they must
not be copied into this environment, logs, or durable session state. Adapter-specific
allowlists must preserve the native harness without widening process authority.

### OpenCode Server transport identity gate

The current local OpenCode Server transport retains OpenCode's native configuration and
credentials; it does not rewrite the user's config or copy provider secrets. Since the
documented `opencode serve` interface accepts a port rather than an inherited listener,
the adapter may reserve an OS-selected loopback port and then release it before spawn. It
must not send its generated HTTP Basic credential based only on a health response: a local
process could otherwise win that bind race and impersonate the server. Before the first
authenticated request, the adapter requires a bounded `server listening on
http://127.0.0.1:<selected-port>` startup line from the owned child's stdout, emitted after
the native server reports a successful bind. Recognized legacy/current prefixes are
strictly parsed; missing, oversized, unexpected, or mismatched output fails closed and
the direct child is stopped. The reader continues draining bounded stdout after readiness
so the child cannot block on a full pipe. The server password is never sent before this
gate.

This is a process-origin bind-success signal, not general OS peer authentication, child
process containment, or proof that OpenCode's descendants stopped writing. It depends on
the installed CLI's startup output contract and must be covered by the supported-version
qualification matrix ([OpenCode Server documentation](https://opencode.ai/docs/server/)).
This transport remains unusable for Task/Attempt execution until
Task admission, isolated Environment, native capability mediation, process-tree
containment, Effect reconciliation, and lease fencing are integrated and qualified.

## SessionSpec

```text
SessionSpec {
  workspace_id
  scope: AgentSessionScope
  task_spec_revision? # required for Task scopes; absent for Conversation
  context_packet_ref?
  environment?
  capability_attachments[]
  context_attachments[]
  host_instruction_bundle_ref? # optional, bounded, digest-pinned presentation guidance
  preloaded_host_skill_refs[]  # zero-authority bundled HostSkills only
  execution_policy
  budget?
  deadline?
}
```

Host guidance is optional and never a required AgentFeature. `UNSUPPORTED` delivery does
not exclude an otherwise eligible lead/worker; host-side deterministic compilation remains
available. By default, response-design guidance is sent only to human-facing
`CONVERSATION` sessions, not Task planners or delegated Attempts. Adapters do not rewrite
native instruction/config files. See [`HOST-GUIDANCE.md`](HOST-GUIDANCE.md).

Session admission rules:
- `CONVERSATION {conversation_id, conversation_turn_id}` is authorized by ConversationService and the active
  AgentBinding override when set, otherwise the Workspace default AgentBinding. If
  neither is enabled, no session starts and the Operator returns `AGENT_UNAVAILABLE`
  without losing the message draft. A disabled explicit override does not silently fall
  back. An enabled binding with `lead_eligible=false` returns
  `AGENT_NOT_LEAD_ELIGIBLE` for a lead selection. Only enabled, lead-eligible bindings
  may be Workspace defaults or explicit Conversation/Task leads. Changing the Workspace
  default affects new turns only; it has no Task mutation, plan submission, Artifact
  publication, or consequential capability authority. It may issue read-only
  CapabilityInvocations scoped to that Conversation. `context_packet_ref` is absent; the
  adapter receives a bounded Conversation projection and selected ResourceRefs from
  ContextService.
- `TASK_PLANNING {task_id}` is authorized by a transient PlanningAssignment envelope,
  bound to the current lead AgentBinding and TaskSpecRevision, and has no Attempt or
  Environment write access. The envelope is not durable; the AgentSession and Task state
  are. Storage permits at most one active planning session per Task.
  A task-planning context packet is required and is pinned to that TaskSpec revision.
  The durable AgentSession records that revision. If the TaskSpec head changes, the old
  planner is fenced from new Invocations and plan acceptance, its in-flight Invocations
  are settled, and its session is closed before a fresh envelope/session is admitted.
  The current Codex App Server transport now has typed read-only thread/turn constructors
  and a strict initial-plan output schema. Its turn policy sets `type: readOnly` and
  `networkAccess: false`; this disables writes and shell network for the Codex sandbox but
  does not restrict reads to a set of Resource roots. Codex's default read-only access may
  include the host filesystem, so this path is unusable for Task planning until the
  Runtime provides an externally isolated, Task-specific Environment. It also does not
  disable or mediate user-configured MCP servers, app tools, or other non-filesystem native
  capabilities; establish Environment identity, Task admission, process-tree containment,
  or writer quiescence. A coordinator must fail closed until the native capability set is
  disabled or individually authorized/mediated and the sandbox payload is qualified for
  the installed Codex protocol version.
  In the current SQLite implementation, `AgentSessionStore` independently rejects both
  planning-session reservation and activation with
  `TASK_PLANNING_ISOLATION_UNAVAILABLE`, before durable session/Task state is written.
  This protects the storage boundary if an internal caller skips PlanningCoordinator's
  preflight. It is an implementation gate, not a new authorization proof and not a
  replacement for the Runtime-owned attestation required by the admission contract.
- `ATTEMPT_EXECUTION {task_id, attempt_id}` requires one admitted Attempt, its
  Runtime/Environment, active lease, and scoped grants. Its TaskPacket is required and
  names the TaskSpecRevision referenced by the Step's accepted PlanRevision, that
  PlanRevision, the relevant Step, and revision-pinned inputs. The durable AgentSession
  records that revision; a later TaskSpec change does not silently rewrite an existing
  Attempt's context.
- The AgentSession's Runtime/incarnation is chosen at admission and cannot be rewritten.
  An existing local binding can resume only after its adapter validates the native handle
  against the current host incarnation. Cross-Runtime continuation creates a new
  AgentSession with a new bounded context packet; the prior session remains history.
- A planning session can search/describe capabilities and load skills for planning, but
  may invoke explicitly granted read-only capabilities, but cannot invoke consequential
  operations or publish Artifacts. Execution authority is
  never implied by an AgentSession alone.

## Agent options and switching

AgentProfile identifies agent software and AgentEndpoint identifies one concrete protocol
route. Model/reasoning options are opaque agent-owned session configuration, not a
LiteCowork model catalog or router. Each turn/Attempt pins the binding, selected endpoint,
and a digest of the normalized agent options used. Changing model/options does not change
the AgentProfile or start another AgentHost; the adapter reports whether the option can
change in the live session, requires a fresh AgentSession, or is unsupported. New sessions
receive a new configuration digest; prior messages/Attempts retain provenance.

Changing a Task's lead AgentBinding affects future planning assignments and plan proposals.
Already-admitted Attempts remain pinned and drain or stop cooperatively at a safe boundary.
The prior planning session closes after the assignment changes; the newly selected host is
started only when a new planning/execution session is actually admitted. Editing a default
binding in Settings does not prewarm or launch an Agent.

## Session lifetime and conversation continuity

An AgentSession is an active adapter interaction, not the durable conversation transcript.
Open one for an admitted Conversation turn, PlanningAssignment, or Attempt execution
period; close it when that operation settles or deliberately yields control. Durable
Conversation messages, Task revisions, decisions, checkpoints, Invocation results, and
Artifacts provide bounded context for a later session. Native agent transcripts and
session handles are optional same-incarnation optimizations and never the continuity source
of truth.

- A Conversation turn gets a fresh Conversation-scoped AgentSession. On final response,
  structured `UserRequest`, cancellation, or failure settlement, close the session and
  release its host-use reference after adapter stop/closure is observed. A turn waiting on
  the user has no live AgentSession. When the user responds, continue that same
  ConversationTurn with a fresh session and a bounded projection containing the request,
  response, and relevant Conversation history. An explicit retry also uses a fresh session.
- A transient PlanningAssignment uses a Task-planning session only while producing a plan proposal
  or clarification. Close it when the assignment settles or waits for user input; a later
  planning assignment opens a fresh session against the then-current TaskSpec revision.
- An Attempt may have sequential AgentSessions under its unchanged AgentBinding, Runtime,
  RuntimeIncarnation, Environment, and lease. Close/release the current session when it
  settles, enters a durable wait for user/resource/provider completion, or is lost. A new
  session may continue that Attempt only in the same Runtime incarnation with the same
  valid lease and reconciled checkpoint/Invocation state. Otherwise abandon the Attempt and
  create a replacement Attempt under a higher lease epoch.
- Provider turn state is not process-quiescence evidence. In the Codex App Server probe,
  `turn/completed` with status `interrupted` arrived while the sandboxed shell command kept
  writing. The adapter must stop or fence the owned host/Environment and observe that its
  writable child processes are quiescent before closing the session as safely stopped or
  admitting a replacement Attempt. If the adapter cannot prove this, retain cancellation as
  pending/ambiguous and block replacement. A provider acknowledgement or UI state alone never
  establishes stale-writer exclusion.
- A long-running CapabilityInvocation is owned by the Invocation runner, not by the
  AgentSession process. It may continue after the agent yields and its session closes. On
  completion, the AttemptRunner can start a replacement session and deliver the durable
  result if the Attempt still has authority. An agent's live tool call that has not become
  a durable Invocation must be observed as stopped or ambiguous before session closure.

For a ConversationTurn, `agent_session_id` identifies the current/most recent session and
is updated on a retry or post-user-response continuation. Each produced message retains
its immutable session/binding provenance. For an Attempt, the pointer similarly names the
current/most recent execution session; replacing it is an audited transition and does not
rewrite the Attempt's pinned execution identity.

## Normalized events

```text
SESSION_STARTED
SESSION_READY
TEXT_DELTA             # ephemeral
MESSAGE_COMPLETED
TOOL_CALL_REPORTED
TOOL_RESULT_REPORTED
PLAN_REPORTED
USAGE_REPORTED
NATIVE_SUBAGENT_STARTED
NATIVE_SUBAGENT_COMPLETED
APPROVAL_REQUEST_REPORTED
CHECKPOINT_AVAILABLE
ERROR
SESSION_LOST
SESSION_CLOSED
```

Only selected events become durable domain events. Token deltas remain ephemeral.

## Delegation

Host delegation is admitted only for a READY Step in the current accepted PlanRevision.
The canonical `DelegateRequest`, TaskPacket, ResultEnvelope, profile pinning, child
Attempt admission, and result applicability rules are defined in
[`DELEGATION.md`](DELEGATION.md). No arbitrary child Step is appended during execution.

## Delegation limits

Core enforces the profile and Task limits in `DELEGATION.md`, without deciding the lead's
reasoning strategy. Every child receives new Attempt-scoped grants; parent authority is
never inherited. Bounded retries create new Attempts and retain the failed try's history.

When the TaskSpec head changes during child execution, mark the result with its
`revision_at_start`; cancel only if the change invalidates the Step at a safe boundary.
Otherwise let it settle and require current-lead applicability review before integration.

## Protocol preference

Select by topology and required feature support:

- ACP for an interactive local/client-to-agent session.
- A2A for an independent remote agent service that remains opaque to the host.
- Vendor SDK/API when it exposes richer supported controls or state.
- Structured CLI/terminal adapter only as a qualified fallback.

This is not a universal preference ranking. Protocol capability/version negotiation is
authoritative; ACP v2 draft behavior must be negotiated and version-gated.

## Native subagents

Native subagents remain agent-owned. LiteCowork may record reported lifecycle if surfaced, but must not assign host grants/leases/environments to invisible native children or represent them as host-controlled Attempts.
