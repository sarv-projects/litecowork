# Capability Fabric

## Purpose and ownership

LiteCowork gives an Attempt progressive access to task-relevant capabilities. It owns
the task-scoped reference, compatibility decision, authorization grant, activation
record, invocation policy, provenance, and revocation edge. LitePSM owns the package
ecosystem and package lifecycle. LiteCowork does not implement a registry, marketplace,
installer, package verifier, or plugin manager of its own.

The selected LitePSM service base URL is:

```text
https://litepsm.sarveshbh-2022.workers.dev/
```

This architecture records the service selection and system boundary only. The LitePSM
API, authentication, manifests, package taxonomy, plugin composition, MCP discovery,
installation, update, supervision, and health contracts are intentionally deferred to
LitePSM's own authority. Do not infer or duplicate those contracts here. LiteCowork's
broker will adapt to the contract LitePSM exposes.

## Capability records

```text
CapabilityRef {
  capability_id
  source_ref
  package_version
  digest
  component_ref?
}

CapabilityLock {
  task_id
  capability_ref
  resolved_at
}

CapabilityGrant {
  grant_id
  task_id
  attempt_id?
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
  runtime_id
  mode
  provider_handle_ref?
  health
  created_at
  updated_at
}
```

Definitions and constraints for these records live in `DATA-MODEL.md`. An offer means a
Runtime reports possible compatibility; it is not proof of installation, health, an
account connection, or permission. A grant authorizes a bounded operation; activation
only makes an implementation available. These states are not interchangeable.

## LiteCowork broker contract

The stable internal port is owned by LiteCowork. Wire methods toward LitePSM are not
specified until its API contract is available.

```text
CapabilityBroker
  search(requirement, task, attempt, placement) -> candidates
  describe(capability_ref) -> task-relevant descriptor
  lock(selection, task) -> pinned CapabilityLock
  request_grant(scope, task, attempt) -> grant | approval | denial
  activate(lock, grant, runtime, agent_features) -> activation
  invoke(activation, grant, operation, args, effect_context) -> result
  list_active(task, attempt) -> activations
  deactivate(activation) -> result
  health(activation) -> health result
```

The exact request/response schema for LiteCowork's internal methods is specified in
`SERVICES.md` and shared value types in `SCHEMAS.md`. The LitePSM-side protocol remains
out of scope here.

## Progressive discovery

Workers initially receive only the stable LiteCowork Gateway tools. Capability search
returns a bounded number of relevant candidates and summaries. Full operation schemas
are loaded only after the worker selects or describes a candidate. Every active Task
pins the exact resolved digest/version it uses; an update never silently changes an
in-flight Attempt.

Permanent Gateway tool names:

```text
litecowork.capabilities.search
litecowork.capabilities.describe
litecow.capabilities.activate
litecow.capabilities.invoke
litecow.capabilities.list_active
litecow.skills.search
litecow.skills.load
litecow.agents.search
litecow.agents.delegate
litecow.agents.status
litecow.agents.message
litecow.agents.cancel
litecow.task.read
litecow.task.update_plan
litecow.task.finish
litecow.artifacts.read
litecow.artifacts.publish
litecow.user.ask
```

The Gateway never exposes package-manager internals or every installed tool at once.

## Direct attachment and proxy execution

The adapter negotiates support from the actual agent binding, never from a product name.

- **Direct attachment** is permitted when the attached capability receives a Task-scoped
  grant and can enforce the required authorization and lifecycle guarantees. It is the
  preferred path for read-only operations and for providers that enforce equivalent
  effect, idempotency, and fencing semantics themselves.
- **Gateway proxy** is required for consequential mutations when LiteCowork must persist
  an Effect before the call, check the active lease/fence, enforce resource scope, or
  reconcile a lost response and the direct provider path cannot guarantee those steps.
- **Native agent capability** means the action is outside LiteCowork mediation. Record
  only what the agent reports or what can be independently observed; classify its
  side-effects honestly for recovery.

If the agent cannot attach a capability during a live turn, proxy through the Gateway
for that turn. Attach directly at a later safe session boundary only if it passes the
same policy checks. Capability discovery alone must not restart a session.

Every mediated mutation has an Effect record before dispatch. A provider response does
not itself prove the desired postcondition. See `ARTIFACTS-EVIDENCE.md`, `TRUST.md`, and
`FAILURE-RECOVERY.md`.

## Skills and plugins

Skill metadata is discovered progressively. Loading resolves a pinned content digest and
adds bounded procedural context using the agent's supported mechanism; otherwise the
content is a bounded Task attachment. Skill text cannot grant capabilities or secrets.
LiteCowork never edits a worker's global configuration or copies a catalog into it.

Plugins and other package forms are whatever LitePSM defines. LiteCowork does not infer
their component structure. Each capability actually invoked still receives its own
compatibility and authorization decision. Package installation or enabling is never a
blanket Task permission.

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
