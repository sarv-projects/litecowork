# Durable Capability Invocations

## Purpose

`CapabilityInvocation` records one request to a capability, independently of whether the
request causes a real-world mutation. It is the durable lifecycle for read-only work,
long-running analysis, streams, asynchronous provider jobs, and MCP Tasks. `Effect`
remains the separate ledger for consequential external state changes. One Invocation may
reference an Effect; many read-only Invocations have none.

`execution_method` is execution provenance, not agent-reported UI text. The adapter selects
the route before admitting the Invocation and records it on creation. `UNKNOWN` is valid
only when the adapter/provider cannot prove a more specific route. The selected method is
immutable, and the dispatch event repeats it. A fallback to a materially different route
creates a new Invocation with its own request digest and idempotency identity. Effects
retain the method of their exact source Invocation.

`NATIVE_AGENT_TOOL` is permitted only when the call crosses the LiteCowork Gateway or a
provider adapter enforces the equivalent authorization, idempotency, fencing, Effect, and
Evidence contract. An arbitrary native tool call that bypasses that boundary is not
represented as a Core-mediated Invocation or Effect.

## Canonical record

```text
CapabilityInvocation {
  invocation_id
  workspace_id
  scope: CapabilityInvocationScope
  agent_session_id
  capability_grant_id
  activation_id
  capability_ref
  operation
  request_digest
  execution_method: STRUCTURED_API | STRUCTURED_BROWSER | ACCESSIBILITY_BROWSER |
                    SCREEN_COMPUTER_USE | DETERMINISTIC_LOCAL |
                    NATIVE_AGENT_TOOL | UNKNOWN
  action_batch?: ActionBatchMemberRef
  idempotency_key?
  status: CREATED | DISPATCHED | WAITING | INPUT_REQUIRED |
          CANCEL_REQUESTED | SUCCEEDED | FAILED | CANCELLED | AMBIGUOUS
  provider_task_status: ProviderTaskStatus?
  provider_task_created_at?
  provider_task_expires_at?
  provider_task_ttl_ms?
  provider_poll_after_ms?
  provider_updated_at?
  partial_result_refs[]
  result_refs[]
  effect_id?
  failure?
  created_at
  updated_at
  completed_at?
  version
}
```

`CapabilityInvocationScope` is a tagged union and the invocation stores exactly one
variant:

```text
CONVERSATION { conversation_id }
TASK_PLANNING { task_id }
ATTEMPT_EXECUTION { task_id, attempt_id }
```

Every variant is bound to the AgentSession on the Invocation. Its Conversation/Task/Attempt
parent identifiers must agree with that AgentSession; a Conversation AgentSession also
pins one `conversation_turn_id`, enforced by the live-turn admission check. Task sessions
also record their pinned `task_spec_revision`. Nullable combinations cannot express two
scopes or no scope.

Conversation- and Task-planning-scoped Invocations are read-only and cannot publish
Artifacts, mutate Tasks, or create Effects. Consequential operations require an
Attempt-scoped Invocation and its normal grant, Approval, Effect, idempotency, and fencing
checks. An Agent that asks for a write during simple conversation or planning must first
materialize/execute a Task Step through ordinary product rules.

## Bounded ActionBatch

An ActionBatch groups 1–64 ordered operations for transport efficiency; it is a value
carried by the invoking adapter, not a domain aggregate, transaction, or authority grant.
Before provider dispatch, Core durably admits one ordinary CapabilityInvocation per
member, each with a zero-based ordinal, shared batch ID/count/digest, independent member
request digest, independent idempotency key, and the same selected execution method.
Every member repeats the ordered-batch digest. A provider may receive one transport call
only when it returns a separately correlatable outcome for each member; otherwise the
adapter dispatches members separately.

Each member rechecks its preconditions immediately before dispatch and evaluates its
postconditions from returned or observed evidence. A consequential member has its own
Attempt-scoped Invocation and at most one ordinary Effect. The batch does not make its
Effects atomic: on failure, timeout, failed precondition, or abort condition, undispatched
members are cancelled and every dispatched member is settled/reconciled independently
before fallback or continuation. No later member runs after a failed abort condition.
An ActionBatch cannot elevate Coworker interaction defaults, Trust decisions, Grants,
Approvals, or lease scope.

The batch is not a provider transaction. If a member fails after earlier members were
dispatched, Core first settles or reconciles every earlier member and linked Effect. It
then records the batch outcome as a projection of its member Invocations; no separate
ActionBatch aggregate or synthetic all-or-nothing Effect exists.

## Lifecycle and ownership

- `CapabilityBroker` verifies the scoped grant, creates the record with that exact grant
  reference, verifies that the requested operation is covered and that the grant has not
  expired, then stores its immutable request digest before dispatch. The first
  `CREATED -> DISPATCHED` transition repeats parent, lease/incarnation, grant, and
  activation checks atomically; if the owner stopped or paused after creation, the
  Invocation is cancelled before any provider call.
- Admission also checks the parent lifecycle atomically: the ConversationTurn must be
  RUNNING and point to this active session; planning requires a RUNNING Task, its current
  lead binding, and current TaskSpecRevision; execution requires the exact RUNNING Attempt,
  its PlanRevision-pinned TaskSpecRevision, and the same Runtime/incarnation's unexpired
  ACTIVE ExecutionLease. Pause/cancel or a newer planning spec therefore fences new calls.
- `InvocationRunner` records dispatch, polls/resumes provider state, stores bounded
  partial/final results, and settles provider failures.
- `CapabilityBroker` authorizes cancellation; `InvocationRunner` asks the provider to
  cancel and records the actual result.
- `EffectService` owns any linked Effect; it does not own Invocation status.
- `TaskService` and `ConversationService` consume Invocation projections but cannot
  rewrite provider history.

### Dispatch readiness gate

The schema and state machine define `CREATED -> DISPATCHED`, but the current local
storage implementation does not expose an Invocation transition writer or Trust-issued
exact-action dispatch decision. For Attempt-scoped consequential operations it also has
no atomic `ApprovalUse` consumer. Until one authoritative transaction rechecks the live
owner/lease, grant, activation, Trust decision and any exact Approval; transitions the
Invocation; links and starts its Effect; and writes the related state snapshots, events,
audit and idempotency receipt, external provider dispatch is unavailable. The current
Effect store returns typed missing-prerequisite blockers before any write when asked to
enter `STARTED`. A `PROPOSED` Effect is not a dispatch permit, and retrying an Effect does
not bypass the gate. No provider call may be made based only on these existing SQLite
tables or the state-machine definition.

```text
CREATED -> DISPATCHED | CANCELLED
DISPATCHED -> SUCCEEDED | FAILED | WAITING | INPUT_REQUIRED | AMBIGUOUS
WAITING <-> INPUT_REQUIRED
WAITING | INPUT_REQUIRED -> DISPATCHED | CANCEL_REQUESTED | AMBIGUOUS
DISPATCHED -> CANCEL_REQUESTED | SUCCEEDED | FAILED | AMBIGUOUS
CANCEL_REQUESTED -> CANCELLED | SUCCEEDED | FAILED | AMBIGUOUS
AMBIGUOUS -> DISPATCHED | WAITING | INPUT_REQUIRED | CANCEL_REQUESTED |
             SUCCEEDED | FAILED | CANCELLED
```

`AMBIGUOUS` is an unresolved state, not terminal. It may result from losing authoritative
provider state while an Invocation is `DISPATCHED`, `WAITING`, or `INPUT_REQUIRED`.
InvocationRunner may leave it only after querying the provider or receiving authenticated
provider state; it may not infer
completion/cancellation from a timeout. `SUCCEEDED`, `FAILED`, and `CANCELLED` are terminal
and immutable. A retried request is a new Invocation unless the provider
supports the same idempotency identity and the operation contract proves it is the same
logical invocation. A transport timeout after dispatch becomes `AMBIGUOUS` until provider
state is reconciled. For MCP asynchronous calls, this includes losing the initial
`CreateTaskResult` before its opaque task handle is durably stored in the encrypted
Runtime-local binding: if no provider lookup by a precommitted idempotency key can recover
the same handle, do not issue a second
`tools/call`. Keep the Invocation `AMBIGUOUS`, record the orphan-risk blocker, and request
provider-specific or user resolution. A retry with a new provider task could duplicate
work or effects. Cancellation is cooperative; a cancellation request does not mean the
provider stopped. Late success after cancellation is recorded as success and its Effects
are reconciled; it is never discarded.

Only the normalized `ProviderTaskStatus` enum is replicated. If an adapter observes a
provider status it does not recognize, it records `UNKNOWN`, moves the Invocation to
`AMBIGUOUS`, and preserves the raw status only inside the encrypted Runtime-local binding
when needed for adapter reconciliation. It must not treat an unknown status as working,
terminal, safe to retry, or safe to deliver user input against.

## Task pause and cancellation

TaskService blocks new Task/Attempt Invocation creation when pause or cancellation is
requested. An undispatched `CREATED` Invocation can be settled locally as `CANCELLED`;
the first-dispatch recheck also closes the race where an Invocation was created just
before pause/cancel committed.
For a dispatched Invocation, InvocationRunner requests provider cancellation and waits for
authoritative terminal state; an acknowledgement alone is not settlement. During pause, a
provider-confirmed `INPUT_REQUIRED` status is quiescent and may remain pending with its
UserRequest, while `DISPATCHED`, `WAITING`, or `AMBIGUOUS` work must settle or keep the
Task in `PAUSE_REQUESTED` with a blocker. A Task becomes `PAUSED` only when no
provider operation can still act and linked Effects are reconciled. Task cancellation is
stricter: every Task/Attempt Invocation must be terminal before the Task becomes
`CANCELLED`. Conversation-scoped Invocations keep their own Conversation lifecycle and
are unaffected by Task control.

## MCP Tasks mapping

LiteCowork targets the stateless MCP base protocol dated `2026-07-28` and negotiates
extensions per operation. The MCP Tasks extension is an optional provider-operation
extension (`io.modelcontextprotocol/tasks`), not LiteCowork's user-facing Task model.

When an MCP `tools/call` returns an asynchronous task handle, persist it in the encrypted
Runtime-local `CapabilityInvocationProviderBinding` for the already-created Invocation.
Replicate only normalized status, timestamps, TTL/poll observations, digests, and result
references. Map provider status notifications or `tasks/get` results to Invocation
transitions. An MCP task ID is never used as a LiteCowork TaskId. A Task can own multiple
Attempts and Invocations; a Conversation may own read-only Invocations without any Task.

A worker-session loss does not orphan an Invocation. A replacement Session receives the
current status/result reference and may continue polling through the Gateway only while
its ConversationTurn remains current or its Task Attempt has a valid current lease and
reconciled checkpoint. Provider tasks that can expire are surfaced with their TTL and
retention limitations; LiteCowork must not promise recovery after a provider discards its
state.

Mapping rules for the versioned Tasks extension:

| MCP result/state | LiteCowork representation |
|---|---|
| `tools/call` returns `resultType: task` | Keep the already-created Invocation; store the opaque provider task ID in the encrypted local binding and commit normalized initial provider state before returning the Gateway response. |
| `working` | Invocation remains `DISPATCHED` or `WAITING`, with provider status, update time, and suggested next poll delay recorded. |
| `input_required` | Invocation becomes `INPUT_REQUIRED`; normalize each supported outstanding request by method and elicitation mode. Non-sensitive `elicitation/create` form input becomes a scoped `FORM` UserRequest. `elicitation/create` URL input becomes a scoped `EXTERNAL_URL` UserRequest whose response contains only `accept`, `decline`, or `cancel`; its raw URL and opaque provider key stay encrypted in the Runtime-local `ProviderInputBinding`. Unsupported embedded methods fail closed with `PROVIDER_INPUT_UNSUPPORTED`. Only mode-valid responses are sent through `tasks/update`. |
| `completed` | Invocation becomes `SUCCEEDED` with immutable result refs. A tool result's own `isError` value is preserved as result content; it is distinct from Tasks-extension `failed`. |
| `failed` | MCP Tasks uses this only for an underlying JSON-RPC execution error; Invocation becomes `FAILED` with bounded error metadata and no secret-bearing raw payload. A completed tool result with `isError: true` remains `SUCCEEDED` at the transport lifecycle and carries the tool error for Task verification/recovery. |
| `tasks/cancel` acknowledgement | Record cancellation request only; remain `CANCEL_REQUESTED` until provider status confirms cancellation or the result is reconciled. |
| `cancelled` | Invocation becomes `CANCELLED`; any linked Effect is reconciled independently. |

Persist the provider's opaque task reference and resume cursor only in the encrypted
Runtime-local binding. Shared Invocation state may retain normalized provider status, task
expiry, latest reported TTL in milliseconds, last update time, poll hint, partial-result
references, the immutable request digest, and digests of ordinary result content where
available. It does not replicate a digest of an opaque task ID, cursor, or input key. The
binding's ciphertext digest is local integrity metadata. Because a provider may change
TTL, the expiry and TTL fields are the latest observation, not a fixed promise or
authority.

After a user answers an MCP `input_required` request, commit the immutable response.
Conversation scope moves its exact ConversationTurn from `WAITING_USER` to
`WAITING_DEPENDENCY`; Task scope keeps the response in the Runtime-local outbox until the
Task has resumed. Before dispatch, InvocationRunner obtains current owner authorization:
the Invocation's originating AgentSession must already be `CLOSED` or `LOST`; the old
session is never revived. Conversation delivery is reconciled while the exact turn still
points to that historical session, before a replacement Conversation session is admitted.
For a Conversation, the exact turn must still be current and waiting on this Invocation;
for Task planning, the TaskSpec and lead binding must remain current and a fresh active
planning session with a different AgentSessionId for that revision must be ready on a
Runtime's current incarnation; it may be hosted elsewhere because the provider task and
input key remain on their original Runtime. The operation remains read-only. For Attempt
scope, the source Step/Attempt, pinned
PlanRevision/TaskSpec, capability lock, active Grant, provider binding, and Environment
must still be eligible, and the Attempt's current pointer must name a distinct active
replacement AgentSession on that same Runtime incarnation. A Task paused with
a quiescent `INPUT_REQUIRED` Attempt Invocation may retain its source Attempt as
`WAITING_RESOURCE` without a live lease. It may resume that same Attempt only on the same
Runtime incarnation and Environment, after checkpoint/effect reconciliation, by acquiring
a strictly higher lease epoch and starting a fresh AgentSession. Task-planning input does
not revive its old session; it waits for a fresh session under the same current lead and
TaskSpec. `tasks/update` is then authorized only for the exact stored request key and
response digest. It cannot create another capability operation or revive the old
lease/session. An acknowledgement is only `ACKNOWLEDGED`, not proof the provider accepted
it. Continue `tasks/get` polling until that exact request key is no longer outstanding or
the provider reports a terminal state. If the provider confirms acceptance, mark the local
input binding `ACCEPTED`; only then may the turn coordinator or AttemptRunner deliver
continuation to the ready AgentSession. If the
provider rejects the response but remains `INPUT_REQUIRED`, mark this binding `REJECTED`,
create a new scoped UserRequest for the provider's current request key, and move the exact
ConversationTurn back to `WAITING_USER` or keep the Task Step `WAITING_RESOURCE`. If the
provider rejects it and becomes terminal, settle the Invocation and follow normal recovery.
If acceptance cannot be determined, mark the binding `AMBIGUOUS` and keep its parent
`WAITING_DEPENDENCY`; do not start another AgentSession or resend until reconciliation
proves the exact response duplicate-safe. `AMBIGUOUS -> DISPATCHED` requires an
authenticated `tasks/get` observation that proves the exact same provider key remains
outstanding and a negotiated adapter contract that guarantees the exact response digest is
duplicate-safe; increment the dispatch count and use the unchanged key/digest. Otherwise
remain `AMBIGUOUS`; never replay a response with a new provider key or resume a different
ConversationTurn/Attempt.

An Attempt-bound provider task/input key cannot migrate to a replacement Attempt,
Runtime, or Runtime incarnation. If the source Attempt cannot be safely re-admitted in
place, do not dispatch the queued answer to that provider task. Reconcile or cancel the
old Invocation and its Effects first; a replacement lead/Attempt may receive the immutable
UserRequest/response as bounded context and issue new work under a new Grant, Effect, and
lease. The answer is never silently dropped or replayed against a different provider key.

The owner service issues an internal, short-lived `InvocationContinuationAuthorization`
for that one outbox dispatch. It binds the Invocation, UserRequest, immutable response
digest, owner aggregate version, current Runtime incarnation, and exactly one owner scope:

```text
Conversation: conversation_id, conversation_turn_id, turn_version
Task planning: task_id, task_spec_revision, lead_agent_binding_id, active_planning_session_id
Attempt execution: task_id, step_id, attempt_id, task_spec_revision, plan_revision, lease_id, lease_epoch
```

InvocationRunner consumes it atomically with
`PENDING -> DISPATCHED`; after that transition, provider I/O occurs outside the database
transaction and all uncertain outcomes go through reconciliation. The authorization is
not an AgentSession token, Gateway credential, general CapabilityGrant, or permission to
call another operation. It is not exposed through Operator API or replicated as a secret.
The extension currently uses `tasks/get`, `tasks/update`, and `tasks/cancel`; do not
implement the removed experimental `tasks/result` API or infer an MCP task from an
arbitrary provider job ID. The 2026-07-28 extension page is marked Draft at review time,
so implementations must negotiate and version the adapter.

The input mapping follows the published MCP contract: form elicitation is described as
non-sensitive, URL elicitation is out-of-band, and a URL-mode response omits submitted
content. MCP Tasks says embedded requests retain the same trust and user-facing semantics
as their standalone equivalents. LiteCowork therefore does not treat all
`input_required` payloads as interchangeable JSON questions. See the [2026-07-28 MCP
schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2026-07-28/schema.json)
and the [MCP Tasks extension specification](https://tasks.extensions.modelcontextprotocol.io/specification/draft/tasks).

### User input and credential boundary

MCP form elicitation is for non-sensitive information. It may become a replicated,
immutable UserRequest/response and may be delivered to the provider in `tasks/update`.
LiteCowork never uses this path for passwords, API keys, recovery codes, access tokens,
or other credentials. If the provider asks for credentials, the capability adapter must
use its supported authentication/connection flow or MCP URL-mode elicitation; it must
not relabel a secret field as an ordinary form question. A URL-mode request keeps the
provider URL, query, fragment, and raw request in the encrypted Runtime-local binding.
The normal UserRequest projection contains no URL and uses a host-authored summary.
After the user explicitly opens the handoff and returns, only an action enum is submitted
to `tasks/update`; credentials are entered directly at the provider's HTTPS origin and
are never copied into the LiteCowork response record.

LiteCowork supports `elicitation/create` form and URL modes for this path. Embedded
`sampling/createMessage`, roots requests, unknown methods, and unknown modes are not
converted into generic user questions or automatically answered by an Agent/model.
They fail closed as `PROVIDER_INPUT_UNSUPPORTED` until a separate, reviewed policy and
adapter contract exists. Form schemas are bounded and checked for credential-like field
names, descriptions, and password/secret formats; suspicious or undeclared sensitive
input fails with `SENSITIVE_INPUT_UNSUPPORTED`. This check is defense in depth: arbitrary
free text cannot be proven secret-free by a schema, so the UI explicitly says not to enter
credentials in ordinary answers. No detector is treated as the security boundary.

URL-mode handoffs are revealed only after an authenticated, explicit Operator action.
The raw URL is returned with `Cache-Control: no-store`, excluded from event payloads,
shared projections, logs, analytics, crash reports, and backups, and is not embedded in an
MCP App or rendered as active content. LiteCowork accepts HTTPS URLs only, rejects
userinfo and private/loopback/link-local/metadata destinations, displays the verified
origin and capability publisher before opening the system browser, and never follows
redirects itself. The provider's out-of-band completion remains unverified until the
provider task reports the input key resolved and the Invocation is reconciled.

For Streamable HTTP, each `tasks/get`, `tasks/update`, and `tasks/cancel` request carries
`Mcp-Method` with that method name and `Mcp-Name` with the opaque provider `taskId`. The
adapter preserves this routing identity across reconnects; it must not substitute the
capability/tool name for `Mcp-Name` on provider-task operations.

## Direct calls and durability

An asynchronously running operation that must survive an AgentSession loss uses the
LiteCowork Gateway proxy, or a qualified host-managed native-tool attachment that exposes
a durable provider handle and lets LiteCowork record/update the Invocation. An opaque
unmediated agent-owned tool call with no lifecycle callback is not resumable evidence: it
is shown as agent-reported, and automatic failover cannot assume it completed or stopped.

Synchronous calls may use the host-managed native-tool surface when its relay records the
Invocation and the declared trust/provenance policy permits it. A consequential call still
must pass through an enforcing boundary equivalent to LiteCowork's Effect/fence contract.

## Streaming

Transient chunks and progress deltas use an ephemeral stream. Durable checkpoints retain
the last provider cursor, bounded partial result references, usage observations, and
status. A final result is an immutable ResourceRef/Artifact and is digest-verified before
the Invocation becomes `SUCCEEDED`. Reconnect resumes from a provider cursor when
supported; otherwise the adapter re-reads provider state and deduplicates by provider
sequence/id.

## MCP Skills and Apps

The broker may normalize Skills from LiteSPM-managed packages or from an MCP Skills
extension server. The client uses `skills/list`/`skills/get` only after the server
advertises the Skills extension and its required Resources capability; individual files
are read with `resources/read`. A skill can also be named by an explicit URI in server
instructions, another Skill, or a user request without appearing in a partial
`skills/list` result; LiteCowork confirms that exact URI with `skills/get` before treating
it as a Skill. It never infers Skill identity from a `skill://` scheme alone. Identity is
`(host-assigned authenticated server identity, exact SKILL.md URI)`, not the display name
or URI scheme; same-named Skills from different origins remain distinct and cannot shadow
one another. A valid entry must include a complete resource manifest array or the literal
`dynamic`. The optional `resources/directory/read` method is used only when the server
advertises `directoryRead: true`. It lists live direct children but does not extend the
pinned manifest; a child absent from that manifest requires a refreshed `skills/get`,
manifest comparison, and new approval before use. `ttlMs` is a cache-freshness hint, not
an integrity claim. LiteCowork v1 may display a dynamic entry but does not activate it:
it has no content integrity or content-bound approval. A manifest array must list every
file exactly once. Compute `skill_manifest_digest` as SHA-256 over RFC 8785 canonical JSON
for `{origin_id, skill_uri, resources}`, where `origin_id` is the host-assigned server
identity and `resources` is sorted by exact URI and contains each `{uri, digest, size}`.
Persist user approval in a scoped CapabilityGrant bound to that exact Skill CapabilityRef
and manifest digest. A refreshed manifest that changes any entry revokes the prior
approval/grant and Task lock and requires a new grant. If the skill is already active in a
session, stop that acting window at a safe boundary and require approval before attaching
the refreshed skill. Entry digests prove consistency with that server's manifest, not
publisher trust.

Fetch files lazily only when needed, verify both SHA-256 and byte size before use, and
compare parsed `SKILL.md` frontmatter field-by-field with the advertised frontmatter. While
the Skill is active, only read supporting files listed in the pinned manifest; an unlisted
file requires refreshing `skills/get` and reapproval. Resolve relative references beneath
that Skill's root and keep every read bound to the originating server. A nested `SKILL.md`
listed as supporting content is ordinary text until independently discovered/referenced,
verified, and explicitly approved as its own Skill. Apply the standard per-Skill limits of
512 files and 16 MiB total. Treat Skill text as untrusted input, ignore
`allowed-tools` unless a separate explicit TrustService grant authorizes those tools, and
require fresh consent for a nested Skill. A Skill cannot trigger host code execution
without the ordinary per-skill/user approval for that execution authority. Reading a Skill
file as an ordinary Resource does not activate it. This does not define LiteSPM's package
or API schema.

MCP Apps are optional Workbench views. The client advertises App support during protocol
capability negotiation; the server may then expose a `ui://` resource in tool metadata.
LiteCowork fetches that resource through the authenticated provider, pins the server
identity and URI, computes a digest over the fetched bytes for change/provenance tracking,
and validates the declared CSP. That computed digest is not a publisher signature or
provider-supplied integrity proof. LiteCowork renders it in an isolated sandboxed iframe
with scripts enabled for the AppBridge, but without `allow-same-origin`, forms, popups,
downloads, or top navigation by default. It has no host DOM, cookies, storage, local files,
SecretStore, or native IPC access. The AppBridge exposes only structured tool results and
explicitly granted host actions; external links use a host confirmation path. All
tool/resource requests return through the host and CapabilityBroker, create durable
CapabilityInvocations, and use Effect/fence/approval handling when consequential. The
host enforces declared `connectDomains` and `resourceDomains` against egress policy,
limits navigation and payload sizes, and tears down the frame when its scope ends. It
shows server/package identity, version/digest, grants, and Workspace/Task scope in
Inspector. If App support is unavailable or rejected by policy, the ordinary tool result
remains usable without embedding the UI.

MCP Apps degrade to their ordinary tool/resource result when the Operator host does not
support embedded views.

## References

- [MCP 2026-07-28 base specification announcement](https://blog.modelcontextprotocol.io/posts/2026-07-28/)
- [Versioned MCP Tasks extension](https://tasks.extensions.modelcontextprotocol.io/specification/draft/tasks)
- [MCP Skills extension](https://skills.extensions.modelcontextprotocol.io/specification/stable/skills)
- [MCP Apps overview](https://apps.extensions.modelcontextprotocol.io/api/documents/overview.html)
- [MCP Apps CSP and CORS](https://apps.extensions.modelcontextprotocol.io/api/documents/csp-and-cors.html)
