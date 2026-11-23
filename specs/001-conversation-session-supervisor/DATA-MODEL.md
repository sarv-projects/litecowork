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

AgentHostInstance has its own STARTING/READY/BUSY/DEGRADED/FAILED/STOPPING/STOPPED state. READY or BUSY is not adapter session readiness. Active session count is derived from live local host bindings and nonterminal sessions. Per the canonical state machine, orderly shutdown after the AgentSession is durably CLOSED/LOST first persists STOPPING, then persists STOPPED only after observed process exit; uncertain exit remains STOPPING. SQLite now rejects direct transitions to STOPPED from STARTING, READY, BUSY or DEGRADED. StopUnconfirmed still lacks retained process-handle ownership and later reconciliation.

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

The final-result operation accepts terminal outcomes only: `COMPLETED`, `FAILED` or `CANCELLED`. `WAITING_USER` and `WAITING_DEPENDENCY` are nonterminal and must use the separate UserRequest and continuation operations; this response transaction rejects them before opening a write transaction, so it cannot insert a final AGENT message, write `settled_at`, close the AgentSession or settle the dispatch intent for a waiting state. This is the 2026-11-19 user-delegated implementation alignment for A-93 and follows locked B-04/B-06 without changing their target semantics.

This is CODE, not QUALIFIED. Retain the all-or-nothing requirement across AGENT message + terminal turn + session close + events + snapshots + receipt.

## 6. Dispatch durability and event/projection ownership

Migration v29 persists Conversation dispatch intent in the admission transaction; the supervisor claims it by turn/retry ordinal using the dispatching Runtime incarnation before host/binding/profile preflight; a losing claim cannot settle the turn. Store operations include claim, release, bounded intent listing for startup sweep and, under USER-DELEGATED A-116, an exact `(turn_id, retry_ordinal)` point read. After CAS false/error, only a point read proving `CLAIMED` by this exact Runtime incarnation permits dispatch to continue; another owner or `SETTLED` is a no-op, while `UNCLAIMED`/unreadable state is retried by the coordinator with capped backoff. A missing or malformed row fails closed. Same-Conversation nonterminal admission is guarded by storage. Startup recovery settles unsettled prior-incarnation attempts FAILED with RUNTIME_RESTARTED and releases orphaned STARTING sessions as LOST; it does not silently resend a prompt. An exact admission replay returns its original receipt and is not a new dispatch authorization. This direction does not add a durable record, migration, event or public API and is not Storage/Conversation owner ratification. These are CODE-level recovery boundaries, not evidence of live provider quiescence or packaged desktop qualification.

Existing global event contracts are in EVENTS.md and their JSON schemas. Current relevant families: `conversation.turn.created.v2`, `conversation.message.added.v1`, `agent.session.starting.v1`, `agent.session.started.v1`, `agent.session.lost.v1`, `agent.session.closed.v1`, `conversation.turn.status.changed.v1`, `conversation.turn.settled.v1`, `conversation.turn.retried.v1`, `conversation.turn.resumed.v1`. No new event is proposed by this feature review; use the existing global event names and schemas unless the canonical event owner approves a versioned change.

Durable presentation is an authorized message snapshot/upsert with opaque cursor. Transient deltas are not evented aggregate state and are discarded on gap unless exact replay is supported. The existing `ConversationPresentationSnapshot` also carries nullable `activity_turn` (`turn_id`, `retry_ordinal`, closed lifecycle `status`) for the latest Conversation turn. This is a derived read projection from the same SQLite snapshot, not a persisted record or authority. It lets the client reopen the existing bounded REPORTED activity query after a successful turn or process restart; active and retryable projections remain separate control identities. A-108 is user-delegated implementation direction, not a domain-owner ratification. No migration or new event is introduced.

Each `ConversationPresentationMessage` wrapper also carries required nullable `turn_status` from the exact message's immutable `turn_id` joined to `conversation_turns` by both `turn_id` and `conversation_id` in the same owner-authorized SQLite read transaction (A-114 / 001-T31). Null means the read cannot establish an exact owning turn; clients treat it as unknown. This field is only lineage/display metadata for downstream explicit actions such as Feature 005 Task promotion. It does not make a turn transition, infer completion from message order, or authorize any operation. The same field is included in snapshot pages, stream snapshots and message upserts so reconnect and live state do not disagree.

## 7. Privacy, retention and deletion

### 7.1 Private desktop pending-send recovery (A-113 / T30)

`PendingConversationSend` is client recovery material only; it is not a Conversation message, event, receipt, provider transcript, or authority record. It contains the exact Workspace ID, Conversation ID, principal-scoped request key, bounded USER content, up to 16 pinned ResourceRefs, selected AgentBinding (when supplied), presentation preference and ordered attachment labels needed to reconstruct the same request and visible draft chips. It is stored in version 3 of the existing private atomic desktop journal, which preserves New Chat entries and remains able to read v1/v2 files, under its file lock and combined size/entry bounds. The exact record is written before HTTP dispatch. A matching receipt is returned by the bridge but the journal row remains until the UI receives that result and explicitly acknowledges the exact row; a local Tauri response loss therefore remains replayable. The bridge returns a closed typed refusal only for known pre-commit HTTP-status/error-code pairs. The row remains until that typed result reaches the UI and is explicitly acknowledged, after which the UI reconstructs text and chips before allowing correction. Generic status inference, malformed/out-of-scope responses, server errors, transport failures and acknowledgement-write failures do not clear it. A restored chip retains the originally pinned revision rather than substituting a newer picker revision. The daemon and SQLite request receipt remain the source of truth. One unresolved pending send per Conversation is permitted; a different body/key cannot replace it while its commit status is unknown. No credential, cookie, provider handle, native transcript, or capability is stored.

- Native handle: Runtime-local host binding only, ephemeral to exact Runtime incarnation; remove only after session CLOSED/LOST and quiescence/recovery rule.
- Provider credential/cookie: native harness or SecretStore; not LiteCowork Conversation state.
- Transcript: canonical Conversation records, user-visible and owner-scoped; not hidden provider private transcript.
- Context package: request-local transient projection, bounded; do not store a second copy.
- Raw provider frames: transient diagnostic input; do not persist unredacted payload or surface arbitrary frame content.
- Event/snapshot: no handle, secret, private prompt or provider session token.
- Backup/replication: local host binding and native handles are excluded.

## 8. Migration/compatibility

The active SQLite migration chain is wired through sqlite-v34 (`SCHEMA_VERSION = 34` in `crates/storage-sqlite/src/lib.rs`; v34 is the Feature 015 Task delegation-policy snapshot migration). The Conversation-owned v29 dispatch-intent/receipt structures and v30 activity records remain earlier immutable migrations; later versions are owned by their respective features. Do not edit released migrations. T30 reuses the existing idempotency receipt and does not need a new migration. Any future durable Conversation column/index/table requires a separate owner-reviewed forward-only migration, source checksum/schema fingerprint updates, migration tests and old-binary compatibility assessment.


## 9. Atomic first-send transaction (user-delegated direction, 2026-11-19)

`CreateConversationWithFirstTurnCommit` carries the authenticated principal/request key and canonical request digest, initial `ConversationRecord` (version 1 input), USER `ConversationMessageRecord`, OPEN `ConversationTurn`, and the three event drafts. `CommittedConversationFirstTurn` returns the final Conversation projection (revision 2), committed USER message, OPEN turn, create/message/turn events and exact dedup receipt. It reuses the v29 dispatch-intent table; no schema migration is required.

The desktop resolves the selector's visible default to its exact AgentBinding ID and journals that ID in the request. The route requires an exact ID; it does not resolve a mutable Workspace default. One `BEGIN IMMEDIATE` transaction performs, in order: exact receipt replay check; active Workspace ownership check; exact initial AgentBinding eligibility recheck; Conversation row insert with the selected binding as its initial override; revision-1 Conversation snapshot and `conversation.created.v1`; USER message, OPEN turn and UNCLAIMED dispatch-intent inserts; Conversation row/snapshot advance to version 2; `conversation.message.added.v1` at revision 2; `conversation.turn.created.v2` at turn version 1; composite response receipt insert; commit. Any refusal or storage failure rolls back every row and event. Exact replay returns the stored composite receipt before rechecking mutable Workspace/binding admission state; same key with a changed payload conflicts. Blob snapshots are verified before SQLite commit and remain content-addressed under the existing BlobStore contract.

The desktop's existing private, atomic Conversation-create journal gains a versioned optional first-turn payload. Legacy v1 entries remain readable as create-only requests; v2 writes preserve the exact bounded first-send payload across restart. The total journal remains capped; user content is never copied into ordinary logs/events for recovery. A successful entry is removed only after the composite response is validated. The Tauri bridge may also remove an entry on an authoritative typed pre-commit 4xx; the UI retains the rejected draft, and any corrected submission receives a new request identity. Timeout, transport failure, 5xx and malformed success keep the journal row. There is no new message/transcript table or migration.
