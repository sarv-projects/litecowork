# Data Model - Conversation Session Supervisor

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Feature:** 001 | **Authority:** DATA-MODEL.md, STATE-MACHINES.md, STORAGE.md, EVENTS.md, SCHEMAS.md, AGENT-CONTROL.md.

This feature reuses canonical records. It must not create a parallel Conversation transcript, provider-session aggregate or model-routing database.

## 1. Existing durable records

| Record | Identity/scope | Owner | Feature use |
|---|---|---|---|
| Workspace | WorkspaceId; owner/security boundary | WorkspaceService | Every read/write binds same owner and Workspace |
| Conversation | ConversationId within Workspace | ConversationService | Stable user-visible thread; selection override applies to future turns |
| ConversationMessage | MessageId, ConversationId, immutable content and provenance | ConversationService | USER, prior AGENT and CHANNEL prefix; final AGENT response |
| ConversationTurn | TurnId; ConversationId; version/status/retry ordinal; pinned AgentBinding; user message; current session pointer | ConversationService | Durable lifecycle state; v29 dispatch intent keyed by turn/retry ordinal and fenced claim records dispatch ownership |
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
- Adapter request bound is separately negotiated and not established by the 256 KiB UserRequest-detail prompt cap in the API contract. Measure the exact serialized OpenCode request and enforce a provider-specific bound before qualification.

## 4. Existing storage operations

Observed in active main-worktree source: `start_conversation_agent_session`, `mark_starting_conversation_agent_session_lost`, `list_starting_conversation_agent_sessions`, `activate_conversation_agent_session`, `load_conversation_context_for_turn`, `resolve_conversation_session_host_for_turn`, `create_conversation_turn`, `replay_conversation_turn`.

Activation is one SQLite immediate transaction. It rechecks principal/Workspace, exact session and turn versions, pinned binding, endpoint offer, Runtime incarnation, Workspace EXECUTOR enrollment and eligible host, then commits AgentSession ACTIVE, turn RUNNING, events/snapshots and a local binding. Native handle is excluded from Serde/Debug payload, event, snapshot and committed result. The active OpenCode supervisor calls this operation, and daemon startup calls `agents/conversation_recovery.rs::recover_conversation_sessions` to settle prior-incarnation unsettled intents and orphaned STARTING reservations without replaying uncertain prompts.

## 5. Implemented result-write boundary and remaining gaps

`CommitConversationAgentResponse` and `CommittedConversationAgentResponse` are current storage-core types. `ConversationTurnStore::commit_conversation_agent_response` is implemented in SQLite and called by the OpenCode supervisor path. The transaction checks owner/Workspace, turn/session versions and states, exact session scope, then commits the AGENT message, terminal turn, closed session, host release, snapshots, events and request receipt atomically. Exact replay returns the stored response receipt.

The current typed request carries `retry_ordinal` and `provider_response_id`; the SQLite boundary checks attempt and response identity alongside principal/key and payload digest. It also compares the caller's AGENT author object to the exact turn-pinned `agent_binding_id` and verifies the message Conversation identity before writing, then sets the storage role and provenance pointers from the validated session. Public response/schema promotion and exact-provider qualification remain separately tracked in T01/T09. Broader wrong-Workspace/session, conflicting replay, rollback/fault injection and concurrent settlement evidence still needs to be established.

This is CODE, not QUALIFIED. Retain the all-or-nothing requirement across AGENT message + terminal turn + session close + events + snapshots + receipt.

## 6. Dispatch durability and event/projection ownership

Migration v29 persists Conversation dispatch intent in the admission transaction; the supervisor claims it by turn/retry ordinal before dispatch. Store operations include claim, release and bounded intent listing. Same-Conversation nonterminal admission is guarded by storage. Startup recovery settles unsettled prior-incarnation attempts FAILED with RUNTIME_RESTARTED and releases orphaned STARTING sessions as LOST; it does not silently resend a prompt. An exact admission replay returns its original receipt and is not a new dispatch authorization. These are CODE-level recovery boundaries, not evidence of live provider quiescence or packaged desktop qualification.

Existing global event contracts are in EVENTS.md and their JSON schemas. Current relevant families: `conversation.turn.created.v2`, `conversation.message.added.v1`, `agent.session.starting.v1`, `agent.session.started.v1`, `agent.session.lost.v1`, `agent.session.closed.v1`, `conversation.turn.status.changed.v1`, `conversation.turn.settled.v1`, `conversation.turn.retried.v1`, `conversation.turn.resumed.v1`. No new event is proposed by this feature review; use the existing global event names and schemas unless the canonical event owner approves a versioned change.

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

Existing migration chain is sqlite-v1 through sqlite-v32 (v28 delegation settings revisions, v29 durable turn, v30 REPORTED turn activity records and v31/v32 from Features 014/012 are all wired; `SCHEMA_VERSION` is 32). Do not edit released migrations. Activation and final-result transaction code in the active main worktree currently uses existing tables. A final-result transaction may reuse current entities/events/receipts; any additional durable column/index/table requires approved forward-only migration, source checksum/schema fingerprint updates, migration test and old-binary compatibility assessment.

