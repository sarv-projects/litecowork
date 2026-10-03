# Agent Fabric LLD

## Purpose

Agent Fabric normalizes external agents without taking ownership of their internal reasoning, model choice, prompts or native subagent implementation.

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
  session: { resume, steer, interrupt, cancel, fork }
  input: { text, image, file, resources }
  extension: { mcp_stdio, mcp_http, skills, plugins, dynamic_attach }
  reporting: { tool_calls, plan, usage, native_subagents, approvals }
  environment: { cwd, extra_directories }
  limits: { max_context_hint?, max_concurrent_sessions? }
}
```

## Profiles and Workspace bindings

AgentAdapter discovery yields an `AgentProfile` for an available Runtime offer. Profiles
describe protocol/features; they do not carry credentials or imply authorization. The
owner creates a Workspace `AgentBinding` that refers to a discovered profile, optional
Runtime placement, an opaque SecretRef, and non-secret adapter configuration. If
`runtime_id` is null, placement may choose any eligible Runtime currently advertising
that profile; otherwise only the named Runtime is eligible. New
bindings are disabled until explicitly enabled. Disabling prevents new planning/Attempt
admission; sessions already admitted stay pinned and settle under their current Task,
lease, and Effect rules.

The profile list is a Runtime inventory projection that includes currently observed
Runtime IDs and offer expiry; it may go stale when a Runtime goes offline. Binding
history remains durable. Agent-specific installation and login
flows remain adapter-owned; secret bytes never enter the AgentBinding API.

## SessionSpec

```text
SessionSpec {
  task_id
  workspace_id
  session_kind: LEAD_PLANNING | STEP_EXECUTION
  attempt_id?
  task_packet_ref
  environment?
  capability_attachments[]
  context_attachments[]
  execution_policy
  budget?
  deadline?
}
```

Session admission rules:
- LEAD_PLANNING is authorized by a PlanningAssignment, bound to the current lead AgentBinding and TaskSpecRevision, and has no Attempt or Environment write access.
- STEP_EXECUTION requires one admitted Attempt, its Runtime/Environment, active lease, and scoped grants.
- A planning session can search/describe capabilities and load skills for planning, but cannot invoke consequential capabilities or publish Artifacts. Execution authority is never implied by an AgentSession alone.

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

Host delegation is explicit and separate from native subagents.

```text
DelegateRequest {
  parent_attempt_id
  objective
  input_refs[]
  required_capabilities[]
  acceptance_criteria[]
  preferred_agent?
  placement_preference?
  isolation
  budget
  deadline?
  return_schema
}
```

Child receives:

```text
TaskPacket {
  parent_task_id
  parent_attempt_id
  objective
  constraints[]
  input_refs[]
  artifact_refs[]
  relevant_decisions[]
  required_output
  acceptance_criteria[]
  capability_grant_refs[]
  task_spec_revision
}
```

Child returns:

```text
ResultEnvelope {
  child_attempt_id
  status
  summary
  output_refs[]
  artifact_refs[]
  evidence_refs[]
  unresolved[]
  blockers[]
  usage?
  confidence?
}
```

## Delegation limits

Core enforces, without deciding strategy:
- max depth
- max active children per Attempt
- max Task-wide active Attempts
- budget inheritance ceiling
- deadline inheritance
- environment write isolation
- required capability availability
- runtime eligibility

Default behavior when TaskSpec changes during child execution:
1. mark child result `revision_at_start`.
2. if new revision invalidates its objective, cancel at safe boundary.
3. otherwise allow completion but require parent to validate applicability before integration.

## Protocol preference

```text
ACP
A2A
native SDK/API
structured CLI
terminal compatibility adapter
```

Preference is not a correctness guarantee. Adapter capability negotiation is authoritative.

## Native subagents

Native subagents remain agent-owned. LiteCowork may record reported lifecycle if surfaced, but must not assign host grants/leases/environments to invisible native children or represent them as host-controlled Attempts.
