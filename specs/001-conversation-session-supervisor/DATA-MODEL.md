# Data Model - Conversation Session Supervisor

**Feature:** 001 | **Status:** DRAFT | **Authority:** DATA-MODEL.md, STATE-MACHINES.md, STORAGE.md, EVENTS.md, SCHEMAS.md, AGENT-CONTROL.md.

This feature reuses canonical records. It must not create a parallel Conversation transcript, provider-session aggregate or model-routing database.

## 1. Existing durable records

| Record | Identity/scope | Owner | Feature use |
|---|---|---|---|
| Workspace | WorkspaceId; owner/security boundary | WorkspaceService | Every read/write binds same owner and Workspace |
| Conversation | ConversationId within Workspace | ConversationService | Stable user-visible thread; selection override applies to future turns |
| ConversationMessage | MessageId, ConversationId, immutable content and provenance | ConversationService | USER, prior AGENT and CHANNEL prefix; final AGENT response |
| ConversationTurn | TurnId; ConversationId; version/status/retry ordinal; pinned AgentBinding; user message; current session pointer | ConversationService | Durable dispatch/lifecycle authority |
| AgentBinding | AgentBindingId within Workspace | AgentBindingService | Exact selected configured native agent |
| AgentSession | AgentSessionId; scope CONVERSATION with exact Conversation + one Turn; agent binding, endpoint, Runtime/incarnation; version/status | AgentSessionStore / Supervisor lifecycle | One native session per admitted turn attempt |
| AgentHostInstance | HostInstanceId local to Runtime/incarnation | AgentHostSupervisor | Process/endpoint state, concurrency and lifecycle |
| AgentSessionHostBinding | Local binding from AgentSession to AgentHostInstance and opaque native handle | AgentSessionStore / host lifecycle | Holds process-private handle; never replicated or projected |
| DomainEvent | EventId, aggregate, version, workspace origin sequence | EventStore | Immutable turn/agent lifecycle facts |
| Request receipt | Principal-scoped idempotency identity and payload digest | owning command/storage | Prevent duplicate turn/result commit |
| ConversationContextPackage | Internal bounded projection, not persisted aggregate | ConversationService/reader | Exact committed transcript through submitted USER message |
| RichPresentation | Independent immutable presentation bound to committed AGENT message | Presentation owner | Optional later extension; outside this slice |

The canonical Conversation-scoped UserRequest and immutable UserRequestResponse are existing ConversationService-owned records, not new feature aggregates. Provider input keys and task handles remain in Runtime-local ProviderInputBinding state; this feature introduces no parallel provider-input store or continuation state machine. See [STATE-MACHINES.md](../../docs/STATE-MACHINES.md), [API.md](../../docs/API.md), [STORAGE.md](../../docs/STORAGE.md) and [TRUST.md](../../docs/TRUST.md).

## 2. Session/turn state ownership

ConversationTurn lifecycle is defined only in STATE-MACHINES.md. The feature uses OPEN -> RUNNING after adapter readiness, then COMPLETED/FAILED/CANCEL_REQUESTED/CANCELLED or WAITING_USER/WAITING_DEPENDENCY according to that owner state machine. A lost AgentSession does not itself fail the ConversationTurn.

AgentSession lifecycle is STARTING -> ACTIVE -> INTERRUPTING/ CLOSING -> CLOSED or LOST according to the agent-session owner contract. The Conversation session is scoped to exactly one turn. New retry/continuation is a new AgentSession. Opaque handle is tied to same Runtime incarnation and cannot move to another.

AgentHostInstance has its own STARTING/READY/BUSY/DEGRADED/FAILED/STOPPING/STOPPED state. READY or BUSY is not adapter session readiness. Active session count is derived from live local host bindings and nonterminal sessions.

## 3. Turn/context identity invariants

- Turn target AgentBinding is derived from unique immutable `conversation.turn.created.v2`, never current selector/default.
- Context boundary is the event revision for the current submitted USER message; timestamps and opaque IDs do not define prefix order.
- Every prior AGENT message retains its immutable source AgentBinding/AgentSession provenance.
- Context includes committed USER/AGENT/CHANNEL entries only; excludes SYSTEM_NOTICE, draft, native private history, secrets and handles.
- Bounds: 1,024 messages / 4 MiB projected JSON. ResourceRefs are references, not grants.
- Adapter request bound is separately negotiated; currently observed OpenCode/Codex transport is 256 KiB per request.

## 4. Existing storage operations

Observed in feature worktree: `start_conversation_agent_session`, `mark_starting_conversation_agent_session_lost`, `list_starting_conversation_agent_sessions`, `activate_conversation_agent_session`, `load_conversation_context_for_turn`, `resolve_conversation_session_host_for_turn`, `create_conversation_turn`, `replay_conversation_turn`.

Activation is one SQLite immediate transaction. It rechecks principal/Workspace, exact session and turn versions, pinned binding, endpoint offer, Runtime incarnation, Workspace EXECUTOR enrollment and eligible host, then commits AgentSession ACTIVE, turn RUNNING, events/snapshots and a local binding. Native handle is excluded from Serde/Debug payload, event, snapshot and committed result.

## 5. Proposed result write boundary (not current)

No inspected ConversationTurnStore function commits the final AGENT message and terminal turn outcome together. B-04 must define:

`CommitConversationAgentResponse` (PROPOSED): principal, Workspace, Conversation, turn, exact active AgentSession, retry ordinal, expected session/turn versions, stable response identity, immutable AGENT message/provenance and event drafts.

`CommittedConversationAgentResponse` (PROPOSED): committed message, terminal turn and committed event(s)/receipt; never a native handle.

The single transaction must validate owner, Workspace, turn RUNNING, exact active session and pinned binding; deduplicate response identity; append message/event/snapshot; settle terminal turn/event/snapshot; store receipt. All-or-nothing. The exact field list and event payloads are blocking owner work, not asserted schema today.

## 6. Event and projection ownership

Existing global event contracts are in EVENTS.md and their JSON schemas. Current relevant families: `conversation.turn.created.v2`, `conversation.message.added.v1`, `agent.session.starting.v1`, `agent.session.started.v1`, `agent.session.lost.v1`, `agent.session.closed.v1`, `conversation.turn.status.changed.v1`, `conversation.turn.settled.v1`, `conversation.turn.retried.v1`, `conversation.turn.resumed.v1`. No new event is approved by this DRAFT.

Durable presentation is an authorized message snapshot/upsert with opaque cursor. Transient deltas are not evented aggregate state and are discarded on gap unless exact replay is supported.

## 7. Privacy, retention and deletion

- Native handle: Runtime-local host binding only, ephemeral to exact Runtime incarnation; remove only after session CLOSED/LOST and quiescence/recovery rule.
- Provider credential/cookie: native harness or SecretStore; not LiteCowork Conversation state.
- Transcript: canonical Conversation records, user-visible and owner-scoped; not hidden provider private transcript.
- Context package: request-local transient projection, bounded; do not store a second copy.
- Raw provider frames: transient diagnostic input; do not persist unredacted payload or surface arbitrary frame content.
- Event/snapshot: no handle, secret, private prompt or provider session token.
- Backup/replication: local host binding and native handles are excluded.

## 8. Migration/compatibility

Existing migration chain is sqlite-v1 through sqlite-v28. Do not edit released migrations. Activation code observed in preserved source currently uses existing tables. A final-result transaction may reuse current entities/events/receipts; any additional durable column/index/table requires approved forward-only migration, source checksum/schema fingerprint updates, migration test and old-binary compatibility assessment.

