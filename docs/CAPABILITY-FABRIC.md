# Capability Fabric

## Purpose and ownership

LiteCowork gives a Conversation, planning session, or Attempt progressive access to
scope-appropriate capabilities. It owns the scoped reference, compatibility decision,
authorization grant, activation record, invocation policy, provenance, and revocation
edge. LitePSM owns the package ecosystem and package lifecycle. LiteCowork does not implement a registry, marketplace,
installer, package verifier, or plugin manager of its own.

The selected LitePSM service base URL is:

```text
https://litepsm.sarveshbh-2022.workers.dev/
```

This architecture records the service selection and system boundary only. The LitePSM
API, authentication, manifests, package taxonomy, plugin composition, MCP discovery,
installation, update, and upstream process/health operations are intentionally deferred
to LitePSM's own authority. Do not infer or duplicate those wire contracts here. This
document does define LiteCowork's normalized per-Runtime provider-host view and scoped
Activation-use accounting; the Broker/Supervisor will adapt these requirements to the
contract LitePSM exposes.

## Capability records

```text
CapabilityRef # normalized value defined in SCHEMAS.md

CapabilityLock {
  task_id
  capability_ref
  locked_at
}

CapabilityGrant {
  capability_grant_id
  scope: CONVERSATION | TASK_PLANNING | ATTEMPT_EXECUTION
  conversation_id?     # only for CONVERSATION
  task_id?              # TASK_PLANNING / ATTEMPT_EXECUTION
  attempt_id?           # only for ATTEMPT_EXECUTION
  capability_ref
  operations[]
  resource_scope
  secret_refs[]
  issued_by
  expires_at?
  status
}

CapabilityActivation {
  activation_id
  capability_ref
  scope: CONVERSATION | TASK_PLANNING | ATTEMPT_EXECUTION
  # tagged-union fields identical to the CapabilityGrant/Invocation scope; Conversation
  # scope is the parent Conversation of the AgentSession's one bound turn
  conversation_id?      # CONVERSATION only
  task_id?               # TASK_PLANNING / ATTEMPT_EXECUTION
  attempt_id?            # ATTEMPT_EXECUTION only
  runtime_id
  runtime_incarnation_id
  mode
  status: STARTING | ACTIVE | FAILED | STOPPING | STOPPED
  health: HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
  created_at
  updated_at
}

CapabilityActivationHostBinding { # origin-Runtime-local operational relation
  activation_id
  host_instance_id
  provider_handle_ref? # opaque, Runtime-private
  bound_at
}
```

CapabilityLock pins a Task's selected capability version/digest for its lifetime. A
Conversation with no Task does not create a Task lock; its exact `CapabilityRef` is pinned
on the scoped Grant and CapabilityInvocation instead.

Definitions and constraints for these records live in `DATA-MODEL.md`. An offer means a
Runtime reports possible compatibility; it is not proof of installation, health, an
account connection, or permission. A grant authorizes a bounded operation; activation
only makes an implementation available. These states are not interchangeable.

Installed, offered, startable, running, granted, activated, and attached remain distinct.
LiteCowork creates an Activation only for a session/Attempt scope after authorization. A
RuntimeOffer records expiring readiness such as `STARTABLE` or `READY`, but does not itself
start a process. LitePSM owns package launch/stop, process isolation, provider health
probes/restarts, and its global idle shutdown. LiteCowork still needs a normalized,
Runtime-local view of provider instances so placement and the Operator can see where a
provider is running, whether its observation is fresh/healthy, and how many LiteCowork
Activations currently reference it.

`CapabilityHostSupervisor` coordinates that view and the scoped use references; it is not
a second process/package supervisor. Its `active_activation_count` is derived from
nonterminal LiteCowork Activations that point to the host instance, not a claim about
LitePSM's global client count. A host can be shared only when the provider declares
compatible concurrency and isolation, the pinned capability/configuration digests match,
and the activations occupy the same authorized isolation partition. Otherwise use an
exclusive or Task-isolated instance. Unknown or stateful providers default to Task-isolated;
sharing requires an explicit provider compatibility declaration and Trust approval.
`EXCLUSIVE` uses a unique Activation partition; `TASK_ISOLATED` uses a Task partition;
`TRUST_PARTITION_SHARED` accepts only a TrustService-issued partition. Each invocation
still checks its own Grant, resource scope, lease, and Effect policy; sharing a process
never merges authority. LitePSM's
provider-instance reference remains opaque, and all wire method names, payloads, and
upstream reference-count semantics remain deferred until LitePSM defines them.

The durable `CapabilityActivation` never embeds a Runtime-local host ID or opaque provider
handle. It persists its exact scope and origin Runtime/incarnation so Invocation admission
can prove scope equality and reject stale runtime generations. The origin Runtime records
the provider binding separately in a local
`CapabilityActivationHostBinding` while an Activation is using a provider. That binding
does not replicate with the Activation/Task event stream and is not part of Workspace
backup; a new Runtime must resolve a fresh host and create a new Activation when it takes
over work. This prevents a remote Runtime from receiving a dangling local process handle.

## LiteCowork broker contract

The stable internal port is owned by LiteCowork. Wire methods toward LitePSM are not
specified until its API contract is available.

```text
CapabilityBroker
  search(requirement, session_scope, placement) -> candidates
  describe(capability_ref) -> task-relevant descriptor
  lock(selection, task_id) -> pinned CapabilityLock
  request_grant(scope, capability_ref, operations, resource_scope, expiry) -> grant | approval | denial
  activate(capability_ref, grant, runtime, agent_features) -> activation
  invoke(activation, grant, operation, args, effect_context) -> result
  list_active(session_scope) -> activations
  deactivate(activation) -> result
  health(activation) -> health result
```

The Broker asks `CapabilityHostSupervisor` to obtain/observe a usable host after grant
admission. A nonterminal CapabilityInvocation retains its Activation and provider-host
use reference even after the originating AgentSession closes; release is allowed only
after the owning scope has settled and every linked Invocation is terminal or
reconciled/cancelled. This is required for asynchronous `tasks/get`/`tasks/update` recovery.
The host view reports normalized state/health and the derived LiteCowork Activation count; only the
LitePSM adapter communicates with LitePSM. No client or domain service calls its service
URL directly.

The exact request/response schema for LiteCowork's internal methods is specified in
`SERVICES.md` and shared value types in `SCHEMAS.md`. The LitePSM-side protocol remains
out of scope here.

`CapabilityRef` is LiteCowork's internal normalization boundary: package references carry
the LitePSM-normalized source, package version, and package digest; an MCP Skill carries
the host-authenticated server identity in `source`, the exact advertised `SKILL.md` URI
in `component`, and its manifest digest in `digest`, with no synthetic package version.
The identity tuple is defined in `SCHEMAS.md`; `capability_id` and display-name behavior
follow that canonical rule. This shape does not prescribe LitePSM's API or package model.

Each mediated call creates a durable `CapabilityInvocation` before dispatch. Invocation
tracks read-only work, streams, asynchronous provider tasks, cancellation, results, and
Runtime-local encrypted provider continuation bindings. Shared Invocation records carry
only normalized status and safe cursor digests. A linked Effect is present only when the operation can change external
state. See [`CAPABILITY-INVOCATIONS.md`](CAPABILITY-INVOCATIONS.md); CapabilityInvocation
is not replaced by Effect.

## Progressive discovery

Workers initially receive only the stable LiteCowork Gateway tools. Capability search
returns a bounded number of relevant candidates and summaries. Full operation schemas
are loaded only after the worker selects or describes a candidate. Every active Task
pins the exact resolved digest/version it uses; an update never silently changes an
in-flight Attempt. A Task-free Conversation pins its exact capability reference on the
read-only Grant and Invocation.

Permanent Gateway tool names:

```text
litecowork.capabilities.search
litecowork.capabilities.describe
litecowork.capabilities.activate
litecowork.capabilities.invoke
litecowork.capabilities.list_active
litecowork.skills.search
litecowork.skills.load
litecowork.agents.search
litecowork.agents.delegate
litecowork.agents.status
litecowork.agents.message
litecowork.agents.cancel
litecowork.task.read
litecowork.task.update_plan
litecowork.task.finish
litecowork.artifacts.read
litecowork.artifacts.publish
litecowork.user.ask
```

The Gateway never exposes package-manager internals or every installed tool at once.
`litecowork.user.ask` accepts only non-sensitive `QUESTION`, `DECISION`, or
`RESOURCE_SELECTION` forms. It cannot create an Approval or `EXTERNAL_AUTHORIZATION`
request, and its response must not contain credentials. Sensitive-looking schema fields
are rejected with `SENSITIVE_INPUT_UNSUPPORTED`; credentials are handled by the owning
Connection/SecretStore flow. A compromised agent may lie in natural language, so this
policy is backed by a user-facing “do not enter credentials here” warning and bounded
credential-pattern checks, not a claim that arbitrary prose can be classified perfectly.

## Direct attachment and proxy execution

The adapter negotiates support from the actual agent binding, never from a product name.

- **Direct attachment** means the agent receives the provider's native typed tool
  surface. For LiteCowork-controlled execution, the attached endpoint is a host-managed
  broker relay: every call is authorized and durably recorded as a CapabilityInvocation
  before dispatch, while the worker keeps the provider's native tool names and schemas.
  An independently connected provider is eligible only when its adapter can import an
  equivalent authenticated invocation record and enforce the matching grant/lifecycle
  guarantees. Direct attachment is not permission for an unobserved network bypass.
- **Gateway proxy** is required for consequential mutations when LiteCowork must persist
  an Effect before the call, check the active lease/fence, enforce resource scope, or
  reconcile a lost response and the direct native-tool endpoint cannot guarantee those
  steps. The worker then calls the stable `litecowork.capabilities.invoke` tool.
- **Native agent capability** means the action is outside LiteCowork mediation. Record
  only what the agent reports or what can be independently observed; classify its
  side-effects honestly for recovery. It cannot satisfy LiteCowork's durable Invocation
  or mediated Effect/fencing guarantees.

If the agent cannot attach a capability during a live turn, proxy through the Gateway
for that turn. Attach directly at a later safe session boundary only if it passes the
same policy checks. Capability discovery alone must not restart a session.

Conversation- and planning-scoped grants may contain only read-only operations, must
expire, and cannot reference SecretRefs. Attempt-scoped grants may authorize mutations
only under that Attempt's current policy and lease. Every CapabilityInvocation records
the grant that authorized it. Every mediated mutation has an Effect record before
dispatch. A provider response does not itself prove the desired postcondition. See
`ARTIFACTS-EVIDENCE.md`, `TRUST.md`, and `FAILURE-RECOVERY.md`.

## Skills and plugins

Skill metadata is discovered progressively. Loading resolves a pinned content digest and
adds bounded procedural context using the agent's supported mechanism; otherwise the
content is a bounded Task attachment. Skill text cannot grant capabilities or secrets.
LiteCowork never edits a worker's global configuration or copies a catalog into it.

Plugins and other package forms are whatever LitePSM defines. LiteCowork does not infer
their component structure. Each capability actually invoked still receives its own
compatibility and authorization decision. Package installation or enabling is never a
blanket Task permission.

LiteCowork's current MCP interoperability target is base protocol `2026-07-28`. Its core
is stateless: do not assume an `initialize` handshake or transport session, and do not
make durable invocation state depend on a live connection. A legacy provider may be
supported only through an explicitly negotiated compatibility adapter; new product
contracts cannot depend on legacy sessionful behavior.

The MCP Tasks extension (`io.modelcontextprotocol/tasks`) represents one provider-side
operation; its opaque task handle is stored in the encrypted Runtime-local
`CapabilityInvocationProviderBinding`, not in LiteCowork Task or shared Invocation state.
The current versioned extension page is marked Draft, so support remains
feature-negotiated and adapter-versioned; LiteCowork does not treat it as a mandatory MCP
base feature. MCP Skills server-published `SKILL.md` resources are another discovery
source, identified by the pair of server identity and exact skill URI. A skill is activated
only through LiteCowork's skill-loading path after digest verification and authorization;
merely reading an MCP resource does not activate it. This transport source can be
normalized alongside LitePSM-managed packages without changing LitePSM package ownership.
MCP Apps are optional progressive Workbench views. They run in a sandboxed iframe, and
all tool/resource calls return through the host for authorization, invocation/effect
recording, and auditing. Apps degrade to ordinary tool results where the host does not
support them. See `CAPABILITY-INVOCATIONS.md` and `NETWORK-SECURITY.md`.

Primary references checked 2026-10-04:
- [MCP 2026-07-28 base protocol release](https://blog.modelcontextprotocol.io/posts/2026-07-28/)
- [Versioned MCP Tasks extension (currently Draft)](https://tasks.extensions.modelcontextprotocol.io/specification/draft/tasks)
- [Stable MCP Skills extension](https://skills.extensions.modelcontextprotocol.io/specification/stable/skills)
- [MCP Apps overview](https://apps.extensions.modelcontextprotocol.io/api/documents/overview.html)

## Trust and lifecycle

1. Resolve and pin the selected candidate.
2. Check runtime, environment, and agent compatibility.
3. Request the narrow operation and resource scope.
4. Ask TrustService for allow, approval, or denial.
5. Activate only on the authorized Runtime and with eligible secret leases.
6. Invoke through direct attachment or proxy according to the guarantees above.
7. Record provenance and Effect/Evidence state.
8. Revoke or expire grants at their boundary; stop activation when no authorized use
   remains or the provider becomes unhealthy.

Unknown permission, provenance, health, or compatibility facts fail closed for protected
operations. A catalog listing is not trusted merely because it exists.
