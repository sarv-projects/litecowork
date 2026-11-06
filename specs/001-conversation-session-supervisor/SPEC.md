# Feature Specification - Conversation Session Supervisor and Native Turn Execution

- **Feature ID:** 001 | **Status:** IN_REVIEW | **Revision:** 1.0
- **Target:** LiteCowork V1 native Conversation
- **Owners:** Conversation, Agent Runtime, Storage, Operator/API, Desktop UI, Security/Trust, Product
- **Dependencies:** Conversation/turn contract; AgentSession reservation and activation; AgentHostSupervisor; qualified native adapter; durable result settlement.

## 1. Problem and user outcome

LiteCowork is local-first, chat-first and BYOA. A normal Conversation must invoke the user's chosen native harness without creating a Task, Attempt or Coworker. LiteCowork owns canonical message/turn state, admission, correlation, display, cancellation, final persistence and recovery; the harness continues to own its reasoning, authentication, private transcript and native tools.

The active daemon now contains an OpenCode-only Conversation dispatch path. `submit_conversation_turn` atomically commits the USER message, OPEN turn, events and receipt, then starts `AgentSessionSupervisor::dispatch_turn` in a detached Tokio task and returns 202. The supervisor resolves the turn-pinned host, reserves STARTING, starts OpenCode Server v2.0.32, creates a native session, atomically activates the session and turn, loads the bounded Conversation context, sends it, consumes the response stream, and calls the SQLite final-response transaction. This is source-level integration evidence, not a qualified user journey. The implementation still has material recovery and concurrency defects: the commit-to-spawn interval is not durably represented; pre-dispatch supervisor errors can be logged while leaving OPEN turns stranded; retry changes the turn to RUNNING before calling a reservation path that requires OPEN; RUNNING cancellation is settled CANCELLED without observing native interrupt/quiescence; the accepted single-flight policy is not enforced in the submit transaction; and there is no startup sweep for orphaned Conversation turns/sessions. Keep Send and release claims gated on those fixes plus exact Windows/provider/UI qualification.

**Outcome:** the user selects a supported native agent, sends a message, sees truthful progress, receives a durable answer, can stop/retry safely, and can reload/reconnect after daemon failure. A turn never silently changes its pinned agent.

## 2. Goals

- G-01 Plain Conversation works without Task/Attempt/Coworker.
- G-02 Immutable turn-created authority pins the selected AgentBinding; no fallback to changed selector/default.
- G-03 Turn becomes RUNNING only after an exact native session readiness handshake and atomic activation.
- G-04 Dispatch uses bounded, same-Workspace committed context; ResourceRefs are reauthorized.
- G-05 Final AGENT response and terminal turn outcome are durable exactly once with session/binding provenance.
- G-06 Transient output is bounded and never confused with a saved answer.
- G-07 Stop, retry, provider input, host degradation, crash, timeout and restart have deterministic recovery.
- G-08 Native handles/secrets/private provider state never enter public projections, replicated events or ordinary logs.
- G-09 Send remains unavailable until one exact OS/build/harness/version/auth route passes CODE, SYSTEM and owner USER gates.

## 3. Non-goals

No Task/Attempt/Plan/Coworker; no universal model router; no automatic provider fallback; no transfer of provider-private transcript/session between harnesses; no general native command execution from Composer; no direct provider stream to WebView; no new Trust/Effect authority from provider output; no generated apps/rich media; no LiteSPM, connector, OAuth, Environment or sandbox redesign; no cloud Runtime, remote continuation or mobile; no arbitrary native module management.

## 4. Current implementation/evidence boundary

| Capability | Current evidence | Limitation |
|---|---|---|
| USER message, OPEN turn and idempotency receipt | CODE/SERVED in the active Operator | Durable admission does not prove provider dispatch |
| Send qualification gate | TARGET says disabled until exact OS/provider/auth/USER qualification; current UI violates the gate | Send is enabled for a non-empty draft when local isTurnRunning is false; no qualified binding/provider or Operator readiness guard; backend error is relied on for unsupported/no binding |
| Conversation snapshots, pagination and create | CODE/SERVED with desktop UI calls | Exact packaged desktop/restart qualification is still required |
| Desktop stop/retry controls | PARTIAL / inconsistent | Stop is displayed for active RUNNING status but its handler searches only the presentation-stream transient-turn collection; the OpenCode Operator path does not emit those transient turns. If a transient turn exists, the call hardcodes If-Match version 1. The Operator cancellation route writes CANCEL_REQUESTED then CANCELLED without native interrupt/quiescence. Retry route exists but has no UI call site or failed-turn affordance; active-turn projection omits aggregate version. |
| Composer affordances | PARTIAL / placeholder | The agent-binding selector, textarea, Enter-to-send and Send button are wired. Add context/file and Voice Input buttons have no handlers; More options has no handler; Turbo is a static non-control; “@ to mention, / for actions” is placeholder copy with no mention/action implementation. Every submit sends resource_refs: [], so no attachment can be selected from this composer. |
| Active-turn composer gate | PARTIAL / inconsistent | isTurnRunning checks only RUNNING plus presentation-stream transient turns. OPEN (shown as “Preparing”), WAITING_DEPENDENCY and CANCEL_REQUESTED are not consistently fenced; after a 202 response the local sending flag clears before the OPEN turn necessarily becomes RUNNING. The backend submit transaction has no single-flight check, so duplicate active turns remain possible. WAITING_USER continuation semantics remain a separate owner decision. |
| Empty Conversation state | PLACEHOLDER / misleading | The no-selection canvas contains hard-coded “I am Antigravity” copy and capability claims (including verifiable command execution and multi-step orchestration) that are not evidence of LiteCowork capabilities. Replace with truthful product-owned empty-state content when the UI is next changed. |
| Operator source comment | STALE SOURCE COMMENT | conversation_operator.rs module header says turn dispatch is intentionally absent, but production ConversationApiState::from_ref constructs AgentSessionSupervisor and submit/retry handlers call dispatch_turn. The comment is stale; the actual source path is OpenCode-only and remains unqualified. |
| Turn-pinned AgentBinding and host resolution | CODE in the active source | Discovery and host resolution are not session readiness |
| STARTING reservation and atomic readiness activation | CODE; invoked by the OpenCode supervisor path | Storage primitives exist; no daemon-start recovery sweep settles/reconciles orphaned STARTING Conversation sessions |
| Context package loading and OpenCode input | CODE in the active supervisor path | The OpenCode branch loads the bounded package and sends it; complete resource-format coverage and exact prompt-bound behavior remain unqualified |
| OpenCode server/session/turn dispatch | CODE path in `AgentSessionSupervisor` | Only provider key `opencode` is dispatched, using OpenCode Server v2.0.32; no exact Windows/USER qualification is recorded |
| Final AGENT response and terminal turn commit | CODE in the OpenCode supervisor path and SQLite store | Atomic transaction and exact idempotent replay exist; storage test coverage does not yet prove every rollback, ownership, retry-provenance and crash window |
| Retry lifecycle | PARTIAL / inconsistent | Operator transitions FAILED to RUNNING before calling a dispatcher whose STARTING reservation requires OPEN; retry dispatch is not correctly integrated |
| Single-flight admission | NOT ENFORCED in storage | Submit transaction inserts OPEN without checking another nonterminal turn in the Conversation; no matching active-turn uniqueness constraint was found |
| Startup recovery of OPEN / STARTING / ACTIVE Conversation work | NOT WIRED | Storage can list STARTING sessions and mark them LOST, but no Runtime startup caller was found; committed OPEN turns have no durable dispatch outbox/claim |
| Claude Code, Codex, Gemini CLI and Cline turn execution | TARGET/adapter work varies by provider | Profile probes or binding records do not make them Send-capable |
| Conversation transient token stream | TARGET/partial presentation projection | POST /v1/stream is a rich-presentation stream, not proof of a complete native token-stream path; current Stop UI depends on transient turns that the OpenCode path does not emit |
| Durable Conversation recovery across commit-before-dispatch crash | NOT IMPLEMENTED / NOT QUALIFIED | Submit commits OPEN before detached spawn; exact replay returns its 202 receipt without rechecking or re-admitting dispatch |
| End-to-end native Send | CODE PATH ONLY for OpenCode; QUALIFIED is not established | Exact provider/version/auth/OS/UI and all negative/recovery cases remain release gates |

Do not duplicate transient test totals in this feature package. The completed Windows serial full-workspace run and current frontend/structural validation evidence are recorded in [docs/IMPLEMENTATION-STATUS.md](../../docs/IMPLEMENTATION-STATUS.md) and [CURRENT-RUN.md](../../CURRENT-RUN.md). The latest focused Conversation storage and domain-state-machine rerun also passed; it confirms the current stale-snapshot test still admits two OPEN turns and the current domain retry test encodes the pre-dispatch RUNNING transition that conflicts with STARTING reservation.

## 5. Decision register

### Inherited LOCKED invariants (canonical owners; not feature approval)

| ID | Invariant | Authority |
|---|---|---|
| L-01 | Plain Conversation is not a Task and creates no Task/Attempt/Coworker. | PRODUCT, DATA-MODEL |
| L-02 | Turn-created event pins AgentBinding; later selection/default changes affect future turns only. | STATE-MACHINES, AGENT-CONTROL |
| L-03 | One Conversation AgentSession is scoped to exactly one turn. Retry/continuation uses a fresh session. | STATE-MACHINES |
| L-04 | STARTING is reservation only. Adapter/process/network calls never occur inside SQLite. | STATE-MACHINES, STORAGE |
| L-05 | Session ACTIVE, turn RUNNING, snapshots/events and Runtime-local host binding commit atomically after readiness. | STORAGE, EVENTS |
| L-06 | Native session handle exists only in exact Runtime/RuntimeIncarnation host binding and is excluded from Serde, Debug (except presence boolean), events, snapshots, API responses and logs. | AGENT-CONTROL, STORAGE |
| L-07 | AgentSessionSupervisor owns session/adapter lifecycle; AgentHostSupervisor owns host/process lifecycle; AgentTurnCoordinator owns turn orchestration; ConversationService owns Conversation/message/turn mutations. | SERVICES |
| L-08 | Native events are untrusted observations, not authorization, verified Effects or durable completion. | ARCHITECTURE, TRUST, AGENT-FABRIC |
| L-09 | ResourceRefs do not grant access; authorize exact same-Workspace revision at dispatch. | STORAGE, TRUST |
| L-10 | Persisted semantic messages and terminal outcomes are authoritative; deltas/rich drafts are transient. | API, EVENTS, PRESENTATION-RUNTIME |
| L-11 | Cancellation settles only after stop/quiescence is observed; completion/failure that wins the race remains authoritative. | STATE-MACHINES |
| L-12 | Retry is explicit, version/idempotency protected, increments retry_ordinal, uses a fresh session and retains prior provenance. | STATE-MACHINES |
| L-13 | No automatic fallback. Unsupported or expired target fails visibly and preserves user content. | PRODUCT, AGENT-CONTROL |
| L-14 | Final AGENT message and terminal turn state must commit atomically with events/snapshots/receipt. | STATE-MACHINES, STORAGE; SQLite implementation exists, qualification and contract completeness remain open |

### FLEXIBLE

| ID | Choice | Allowed options | Constraint |
|---|---|---|---|
| F-01 | Coordinator internal shape | Keyed actor or bounded per-Conversation serial lane | One active dispatch per chosen policy; finite queue |
| F-02 | Host reuse | Per-session host or eligible shared host | Host ownership/concurrency and exact local binding rules |
| F-03 | Event transport | Bounded SSE, App Server events or polling | Correlation, ordering, bounds and backpressure |
| F-04 | First UI progress | Durable projections only, then bounded transient frames | Saved transcript remains source of truth |
| F-05 | Internal module names | Existing crate boundaries or focused module | No duplicated provider framework |

### DECLARED DECISIONS AND ARCHITECTURAL CONTRACTS (OWNER ACCEPTANCE NOT YET EVIDENCED)

| ID | Topic | Resolution & Architectural Contract | Canonical Authority & Consequence |
|---|---|---|---|
| B-01 | Concurrent send policy per Conversation | **Strict Serialized Turn Lane.** A Conversation enforces a single-flight active turn (`OPEN` -> `RUNNING` -> `SETTLED`). Concurrent submit requests are rejected with `CONVERSATION_TURN_IN_PROGRESS` (HTTP 409 Conflict). The client UI disables the composer Send button and shows an in-flight status indicator while a turn is active. Local typing buffer is preserved in desktop UI state. | [STATE-MACHINES.md](../../docs/STATE-MACHINES.md), [API.md](../../docs/API.md). Eliminates race conditions in HLC ordering, message correlation, and context packaging. |
| B-02 | First GA OS/build, harness, adapter version and auth mode | **Windows 11 / Windows 10 (`x86_64-pc-windows-msvc`) + Claude Code CLI & OpenCode / Codex CLI.** Adapter Version: `1.0.0` subprocess framed transport. Auth Mode: **Harness-Managed Native Pass-Through**. The CLI harness authenticates directly with the model provider via its native credentials (e.g. `claude login` or provider API keys in user environment/keyring). LiteCowork passes no raw bearer keys and stores no user provider credentials, preserving BYOA zero-knowledge security. | [AGENT-CONTROL.md](../../docs/AGENT-CONTROL.md), [LOCAL-OPERATOR-IPC.md](../../docs/LOCAL-OPERATOR-IPC.md). Windows MSVC toolchain is fully native with VS 2022 Build Tools. |
| B-03 | Exact readiness handshake that proves session identity | **Three-Phase Session Handshake Protocol**: 1. `AdapterVersionProbe`: Host probes adapter executable via `--version`/ping, receiving `AdapterVersionInfo { adapter_version: "1.0.0", harness_version, capabilities: [STREAMING, INTERRUPT] }`. 2. `CreateSessionRequest`: Supervisor sends `{ workspace_id, conversation_id, turn_id, options }` and adapter establishes native session, returning `SessionReadyResponse { native_session_id, protocol_version }`. 3. `Atomic Activation`: Supervisor executes SQLite CAS `activate_conversation_agent_session` binding the native handle to local SQLite only and advancing turn `OPEN` -> `RUNNING`. Prompt dispatch occurs only after activation commits. | [AGENT-CONTROL.md](../../docs/AGENT-CONTROL.md), [STORAGE.md](../../docs/STORAGE.md). Distinguishes host process presence from active turn readiness. |
| B-04 | Exact atomic storage operation for final response commit | **`commit_conversation_agent_response`** SQLite Immediate Transaction. Atomically executes: 1. Verifies `expected_turn_version` and `expected_session_version`. 2. Inserts final `conversation_messages` row (`message_kind = 'AGENT'`, content, tokens, source refs). 3. Updates `conversation_turns` to `SETTLED` (`outcome = COMPLETED | FAILED | CANCELLED`, `settled_at = datetime('now')`). 4. Updates `agent_sessions` to `CLOSED`. 5. Appends `conversation.message.added.v1` and `conversation.turn.settled.v1` to `event_journal`. 6. Commits idempotency receipt to `request_dedup`. Failure leaves state unchanged with zero partial writes. | [STORAGE.md](../../docs/STORAGE.md), [EVENTS.md](../../docs/EVENTS.md), [STATE-MACHINES.md](../../docs/STATE-MACHINES.md). Prevents partial final-response commits and phantom messages; it does not replace durable dispatch admission or daemon-restart recovery. |
| B-05 | Resumable streaming sequence vs stream gaps | **Best-Effort Transient Streaming with Authoritative REST Resync.** `TURN_DELTA` frames are transient, sequence-tagged, and emitted over the Operator stream bridge. If a stream disconnection or sequence gap occurs, the UI discards transient token buffers and issues an authenticated REST query `GET /v1/conversations/{id}/messages` to restore the committed semantic transcript. Transient frames are never replayed from historical storage. | [PRESENTATION-RUNTIME.md](../../docs/PRESENTATION-RUNTIME.md), [API.md](../../docs/API.md), Architecture Invariant 17. |
| B-06 | Conversation-scoped UserRequest continuation | **State-Machine Driven Continuation.** Commits immutable `UserRequestResponse` and transitions exact `WAITING_USER` turn to `WAITING_DEPENDENCY`. Reconciles provider input acceptance against the Runtime-local input key; only then allocates a fresh `AgentSession` reserved `STARTING` and returns to `RUNNING` on readiness. Rejection or ambiguous acceptance follows canonical state machines. Never creates Task or Attempt authority. | [STATE-MACHINES.md](../../docs/STATE-MACHINES.md), [API.md](../../docs/API.md), [STORAGE.md](../../docs/STORAGE.md), [TRUST.md](../../docs/TRUST.md). |
| B-07 | Context package reduction for tight harness budgets | **Bounded Deterministic Sliding Window with Pinned Preamble**: 1. Pinned Workspace instructions / system prompt (always retained). 2. Selected `ResourceRefs` / `ContextAttachments` with cryptographic digests. 3. Most recent contiguous messages backwards from latest user turn until 80% of provider context budget is reached. 4. Explicit truncation receipt header appended to prompt: `[Notice: Messages 1..N omitted due to model context window budget]`. Never silently drops content without audit provenance. | [CONTEXT.md](../../docs/CONTEXT.md), [SCHEMAS.md](../../docs/SCHEMAS.md). Transparent and reviewable context limits. |
| B-08 | Attachment/resource materialization | **Dual-Mode Materialization**: 1. Text / Markdown / Code: materialized inline into the prompt envelope with delimited `ResourceRef` headers and checksums. 2. Binary / Workspace local files: materialized as verified, read-only canonical paths within the admitted WorkspaceRoot and passed via native adapter `--file <path>` CLI arguments. Arbitrary external paths are rejected. | [STORAGE.md](../../docs/STORAGE.md), [TRUST.md](../../docs/TRUST.md). Prevents path traversal and enforces Workspace boundaries. |
| B-09 | Process ownership, lifecycle and idle shutdown | **Supervised Subprocess with Job Object / Cgroup and 10-Minute Idle TTL.** `AgentHostSupervisor` launches adapter under an OS Job Object (Windows) or cgroup (Linux). Sessions register live use references. When all active turns finish and reference count reaches 0, an idle timer starts. After 10 minutes of inactivity, supervisor sends graceful termination signal, waits 5 seconds for clean exit, and forcibly kills lingering processes. Shared external processes are never killed. | [RUNTIME-LIFECYCLE.md](../../docs/RUNTIME-LIFECYCLE.md), [LOCAL-OPERATOR-IPC.md](../../docs/LOCAL-OPERATOR-IPC.md). Clean process isolation and zero zombie processes. |
| B-10 | Usage and cost attribution | **Tiered Telemetry Attribution**: 1. If provider/harness reports token counts in final response, record in `usage_records` with `attribution_source = PROVIDER_REPORTED`. 2. If provider omits token usage, calculate byte-based heuristic (`tokens = max(1, utf8_byte_count / 4)`) and record with `attribution_source = ESTIMATED_LOCAL`. Turn settlement and response display are never blocked on missing provider billing telemetry. | [OBSERVABILITY.md](../../docs/OBSERVABILITY.md), [SCHEMAS.md](../../docs/SCHEMAS.md). Truthful, non-fabricated usage accounting. |

B-01 through B-10 are recorded as declared architectural decisions, but this feature package remains IN_REVIEW: source conformance, exact first-route alignment and owner acceptance are not complete. Do not infer owner sign-off or release qualification from the presence of a decision row.

## 6. High-level design

```mermaid
flowchart TD
  UI[Desktop Conversation UI] --> OP[Authenticated Operator]
  OP --> CS[ConversationService]
  CS --> DB[(SQLite State and Event Store)]
  CS --> CO[AgentTurnCoordinator]
  CO --> SS[AgentSessionSupervisor]
  SS --> HS[AgentHostSupervisor]
  HS --> HOST[Native host/process]
  SS <--> AD[Qualified AgentAdapter]
  AD <--> HOST
  SS --> DB
  AD -->|untrusted bounded events| SS
  SS -->|normalized outcomes| CO
  CO --> CS
  DB --> PO[Presentation Operator]
  PO --> BR[Tauri stream bridge]
  BR --> UI
```

### Target request sequence (not the current source wiring)

1. UI creates a stable send idempotency key and sends authenticated Workspace/Conversation request.
2. Operator authenticates, applies expected version/idempotency and owner scope. ConversationService commits USER message + OPEN turn + receipt atomically.
3. Turn target comes from unique immutable turn-created event, not the latest selector or Workspace default.
4. AgentTurnCoordinator obtains the bounded ConversationContextPackage and exact pinned host resolution.
5. AgentSessionSupervisor persists STARTING; AgentHostSupervisor ensures/retains exact eligible host. Native process/adapter startup happens outside SQLite.
6. Supervisor validates the exact adapter SESSION_READY handshake and rechecks host/endpoint/Runtime incarnation/Workspace enrollment/session+turn versions.
7. Storage atomically activates AgentSession, changes turn OPEN->RUNNING, writes snapshots/events and stores opaque handle only in Runtime-local host binding.
8. Only after activation does adapter receive the bounded context and current USER message. Every ResourceRef is reauthorized/materialized.
9. Provider output is treated as untrusted; bounded normalized observations correlate to Workspace/Conversation/turn/retry/session and sequence.
10. The OpenCode supervisor calls the implemented SQLite final-response transaction to commit the AGENT message and terminal turn/session outcome atomically; its current contract and failure/replay coverage remain incomplete and unqualified.
11. Supervisor waits for quiescence, closes session and releases host reference. Durable projection supports reload/reconnect.

**Current source divergence:** the Operator currently commits USER+OPEN directly through SQLite and starts `AgentSessionSupervisor` with detached `tokio::spawn`; the `AgentTurnCoordinator` and `ConversationService` ownership boundaries in this target sequence are not wired. The OpenCode supervisor also starts/stops its server in the per-turn path rather than delegating to a separately wired `AgentHostSupervisor`. These are architectural integration gaps, not merely naming differences.

### Ownership

| Owner | Responsibility | Excluded responsibility |
|---|---|---|
| UI | Draft, local selection, accessibility, transient display | Provider process, secrets or durable authority |
| Operator | Auth, Workspace scoping, API validation and projection access | Native process lifecycle |
| ConversationService | Conversation/message/turn writes and receipts | Parsing raw provider events |
| AgentTurnCoordinator | Turn orchestration, ordered outcomes, retry/continuation | Direct aggregate writes |
| AgentSessionSupervisor | Session lifecycle, adapter invocation, readiness, normalization, quiescence | Host process ownership or turn aggregate writes |
| AgentHostSupervisor | Host instance, process, health, references, stop | Turn or Conversation outcome |
| AgentAdapter | Protocol-specific startup/input/events/interrupt | Core authorization or durable state |
| Storage | Transactions, invariants, receipts, snapshots/events, local binding | External process/network calls |
| Presentation Operator | Authorized snapshots/upserts and bounded frames | Converting provider text directly to durable state |

## 7. Low-level design and interfaces

### 7.1 Existing storage signatures

Observed in the active main-worktree source; the OpenCode supervisor invokes the activation path. Existence and invocation are CODE evidence, not SYSTEM/USER qualification.

```rust
pub trait AgentSessionStore: Send + Sync {
    fn start_conversation_agent_session(
        &self, start: ConversationAgentSessionStart,
    ) -> Result<CommittedAgentSession, StoreError>;

    fn mark_starting_conversation_agent_session_lost(
        &self, transition: MarkStartingConversationAgentSessionLost,
    ) -> Result<CommittedAgentSession, StoreError>;

    fn list_starting_conversation_agent_sessions(
        &self, workspace_id: &str, after_agent_session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AgentSessionRecord>, StoreError>;

    fn activate_conversation_agent_session(
        &self, activation: ActivateConversationAgentSession,
    ) -> Result<CommittedConversationActivation, StoreError>;
}
```

Activation request fields: principal_id, workspace_id, agent_session_id, expected_session_version, expected_turn_version, occurred_at, host_instance_id, native_session_ref, session_event and turn_event. The handle is skipped by Serde and Debug prints only a boolean presence marker. The committed result contains session, session event, turn and turn event only.

Existing `ConversationTurnStore` operations:
- `replay_conversation_turn(principal_id, request_id, request_payload)`
- `create_conversation_turn(commit)`
- `load_conversation_context_for_turn(principal_id, workspace_id, conversation_id, turn_id)`
- `resolve_conversation_session_host_for_turn(principal_id, workspace_id, conversation_id, turn_id)`
- `transition_conversation_turn(principal_id, workspace_id, conversation_id, turn_id, expected_version, command, event)`

Generic OPEN->RUNNING through `transition_conversation_turn` remains forbidden; only readiness activation may do it.

### 7.2 Service contracts

**ConversationService** owns Conversation/message/turn aggregates. Target operations include:
- `submit_turn(principal, workspace, conversation, expected_version, idempotency_key, request) -> ConversationTurnReceipt`
- `cancel_turn(turn_id, expected_version, request_id) -> CancelReceipt`
- `retry_turn(turn_id, expected_version, request_id) -> ConversationTurnReceipt`
- `commit_conversation_agent_response(commit) -> CommittedConversationAgentResponse` - implemented in `ConversationTurnStore` and called by the OpenCode supervisor; the Operator does not yet expose a separate ConversationService/AgentTurnCoordinator owner boundary.

**AgentTurnCoordinator** consumes committed dispatch, resolves exact turn binding/context, requests supervisor startup/dispatch, consumes normalized results, asks ConversationService for durable settlement and continues a WAITING_DEPENDENCY turn only after provider acceptance. It never writes Conversation/Turn directly.

**AgentSessionSupervisor** validates scope and exact host, reserves STARTING, calls host/adapter outside SQLite, verifies readiness, activates atomically, normalizes events, requests stop, awaits quiescence, closes session and releases local host binding.

**AgentHostSupervisor** ensures/attaches only the exact endpoint and Runtime incarnation, supervises process/health/concurrency and stops only LiteCowork-owned processes with no live references.

### 7.3 Implemented final-result transaction and remaining proof

The active source defines `CommitConversationAgentResponse` and `CommittedConversationAgentResponse` in `crates/storage-core/src/conversation.rs`. `ConversationTurnStore::commit_conversation_agent_response` is implemented in `crates/storage-sqlite/src/conversations.rs` and is called by the OpenCode branch in `apps/litecoworkd/src/agents/supervisor.rs`.

The SQLite IMMEDIATE transaction currently:
- checks the owner/workspace boundary and active Workspace state;
- verifies the Conversation exists and compares the current turn version/status/session ID;
- verifies the AgentSession version, ACTIVE state and exact Conversation/turn scope;
- inserts the AGENT message, settles the turn, closes the session, returns the host instance to READY and deletes the ephemeral host binding;
- writes aggregate snapshots, domain events and an idempotency receipt before commit; and
- resolves an exact request replay through the stored receipt/digest.

The current storage contract is narrower than the target contract described in B-04: `CommitConversationAgentResponse` has no explicit retry ordinal or stable provider-response identity field. The transaction overwrites the message role and binding/session/turn pointers, but the message author is passed through from the caller; inspect/strengthen author/provenance validation before treating this as a complete trust boundary. The existing adjacent test covers stale turn version, stale session version, foreign principal, successful settlement and exact replay; it does not by itself prove every wrong-Workspace/session, retry ordinal, crash/rollback and concurrent final-response case.

Keep this as CODE, not QUALIFIED. Retain the owner requirement: one atomic transaction must protect message + terminal turn + session close + events + snapshots + receipt. If commit outcome is uncertain, query the stable storage receipt/authoritative state before retrying the local commit. Never resend the provider prompt to repair a DB write.

### 7.4 Native adapter boundary

A qualified adapter needs a negotiated/versioned contract for process/session start, exact readiness handshake, prompt/context input, bounded event observation, interrupt and shutdown/quiescence. No first harness contract is locked until B-02/B-03.

The active supervisor calls the OpenCode Server helper: it creates a session, activates the durable AgentSession/ConversationTurn, loads the bounded Conversation context, sends that context, consumes SSE observations and commits a final response. Only the `opencode` provider key has a dispatch branch, pinned to OpenCode Server protocol v2.0.32. Codex transport helpers exist but are not called by this supervisor; Claude Code, Codex, Gemini CLI and Cline are not Send-capable through this path. The current implementation directly starts/stops an OpenCode server inside the per-turn blocking dispatch flow; it does not yet demonstrate the separately owned AgentHostSupervisor/AgentTurnCoordinator topology in the target diagram. Provider events remain untrusted observations, not authorization or Effect proof. Exact Windows packaging, credentials, stop/quiescence and end-to-end USER behavior are unqualified.

### 7.5 Context projection

Use the existing ConversationContextPackage builder:
- binding resolved from the unique immutable turn-created event;
- same-owner/same-Workspace committed USER, CHANNEL and prior AGENT messages through the submitted USER message;
- Conversation event revision orders the prefix, not timestamps or opaque IDs;
- earlier AGENT messages retain original binding provenance;
- excludes product notices, drafts, native handles, credentials/cookies and private harness history;
- max 1,024 messages / 4 MiB before materialization;
- ResourceRefs only; every reference reauthorized at dispatch.

SCHEMAS.md caps ConversationContextPackage at 1,024 messages and 4 MiB. API.md caps a UserRequest detail prompt at 256 KiB. That 256 KiB figure is not the adapter request budget. CONVERSATION_CONTEXT_LIMIT_EXCEEDED is named in SCHEMAS.md and is absent from the 131-code error registry, so it is not a promotable error until the schema and SCHEMAS.md change together. Silent truncation is forbidden. Any smaller harness budget is a pinned provider observation, not this package inventing 256 KiB as the product limit.

### 7.6 Presentation

Use the existing durable Conversation presentation snapshot/upsert cursor, Tauri bridge, bounded backpressure and resumable UI reducer. If provider deltas are added, they are transient frames correlated to Workspace/Conversation/turn/retry/session and monotonic sequence, bounded/coalesced/redacted, never DomainEvents/messages/replicated content. A sequence gap requires replay or discard/resync. A committed AGENT message supersedes deltas. Local Operator does not currently emit `turn.delta`, `turn.settled` or `rich.draft`.

## 8. Security, trust and privacy

- Authenticate every write and scope it to the owner and Workspace.
- Never reroute an admitted turn using selector/default reads.
- Installation, Runtime health or AgentHost READY is not adapter session readiness.
- Never call external process/network/provider inside a SQLite transaction.
- Provider output/events/tool names/arguments are untrusted; they cannot grant authority, approve requests, create Effects or prove completion.
- Native handle stays only in the exact Runtime-local AgentSessionHostBinding; never serialised, logged, emitted, snapshotted or displayed.
- Provider credentials/cookies remain with the native harness or authorized secret provider.
- Conversation text is untrusted input, not system/developer policy.
- ResourceRefs are not permissions: recheck Workspace, owner, exact revision, root/location, media and adapter support.
- Native direct action and Core-mediated Effect are different assurance classes.
- Diagnostics contain correlation and safe classifications only, never prompt text, hidden reasoning, secret or handle values.

## 9. Errors, edge cases and recovery

See [errors-recovery.md](contracts/errors-recovery.md). Global rules: external uncertainty is reconciled before retry; no ambiguous prompt is blindly replayed; no response becomes visible as completed before commit.

## 10. Configuration and compatibility

- No hidden global router or silent fallback.
- Qualification is pinned to OS/build/harness/version/adapter/auth mode and exact capability descriptor.
- Readiness, process shutdown, event frame, buffer, queue, concurrency and retry deadlines must be finite; exact initial values are blocking.
- Product context limit is 1,024 messages / 4 MiB. The 256 KiB figure is the UserRequest detail prompt cap in API.md, not the adapter budget. Change limits only through owner contracts and tests.
- Model, reasoning effort and native command catalog remain harness-owned; Core keeps only approved non-secret selections/digests.
- UI/API eligibility stays false until complete route qualification.
- Existing activation feature does not automatically require a schema migration; final-result persistence may. Add a forward-only migration only when approved.

## 11. Observability

Record safe correlation IDs for request, Workspace, Conversation, turn, retry, session and host; stage duration; readiness result; normalized terminal classification; queue pressure; event gaps; resync; restart recovery. Exclude provider text, handles, auth and secret bytes. Usage distinguishes measured, native-reported, estimate and UNKNOWN; do not infer billing.

## 12. EARS acceptance criteria

- AC-001 WHEN a valid owner submits a turn, THE SYSTEM SHALL atomically commit one USER message, OPEN turn and idempotency receipt before native startup. Verify: Operator/SQLite integration.
- AC-002 WHEN a plain Conversation turn is admitted, THE SYSTEM SHALL NOT create Task, Attempt, PlanRevision or Coworker. Verify: storage assertions.
- AC-003 WHEN selector/default changes after turn commit, THE SYSTEM SHALL retain the immutable turn-pinned AgentBinding. Verify: race test.
- AC-004 WHEN exact native readiness succeeds and all fences pass, THE SYSTEM SHALL atomically commit ACTIVE session, RUNNING turn, snapshots/events and local handle binding. Verify: storage atomicity test.
- AC-005 IF only Runtime/AgentHost READY is known but no exact session handshake exists, THEN THE SYSTEM SHALL NOT activate or report RUNNING. Verify: negative test.
- AC-006 WHEN dispatch begins, THE SYSTEM SHALL send only the bounded committed prefix through the submitted USER message and reauthorize ResourceRefs. Verify: context/adapter test.
- AC-007 IF context exceeds the selected adapter bound, THEN THE SYSTEM SHALL refuse or require approved reduction without silently truncating. Verify: boundary test.
- AC-008 WHEN a provider event is received, THE SYSTEM SHALL enforce size, session/turn/retry correlation and sequence before projection and SHALL NOT treat it as authority. Verify: malformed/replay tests.
- AC-009 WHEN one final response is accepted, THE SYSTEM SHALL atomically commit one provenance-linked AGENT message and terminal turn receipt. Verify: result write crash/replay test.
- AC-010 WHEN stop is requested, THE SYSTEM SHALL settle CANCELLED only after observed stop/quiescence; a winning completion/failure remains authoritative. Verify: race test.
- AC-011 WHEN a FAILED turn is explicitly retried with expected version/idempotency, THE SYSTEM SHALL use a fresh session, increment retry ordinal and preserve earlier provenance. Verify: retry replay.
- AC-012 WHEN daemon restarts with prior-incarnation STARTING session, THE SYSTEM SHALL reconcile it LOST unless same live incarnation and adapter prove safe resume. Verify: restart test.
- AC-013 WHEN request/event/snapshot/Debug/API/log values are serialized, THE SYSTEM SHALL exclude the native handle. Verify: redaction tests.
- AC-014 WHEN a stream reconnects, THE SYSTEM SHALL reconcile an authorized durable snapshot/cursor without duplicating messages; a transient gap requires replay or discard/resync. Verify: projection stream tests.
- AC-015 IF pinned adapter is missing, expired, unauthenticated or fails startup, THEN THE SYSTEM SHALL preserve the USER message, report typed failure and SHALL NOT fall back. Verify: failure matrix.
- AC-016 WHILE supervisor/result writer/route qualification is incomplete, THE SYSTEM SHALL keep Send fail-closed and SHALL NOT claim end-to-end support. Verify: route/UI negative.
- AC-017 WHEN an authenticated owner answers a valid Conversation-scoped UserRequest for the exact turn in WAITING_USER, THE SYSTEM SHALL atomically persist the immutable UserRequestResponse and move that turn to WAITING_DEPENDENCY before enqueueing continuation, without creating Task, Attempt, PlanRevision or Coworker authority. Verify: response transaction and stale/replay tests.
- AC-018 WHEN a Conversation-scoped UserRequest represents provider INPUT_REQUIRED, THE SYSTEM SHALL correlate the response to the exact Runtime-local provider input key; an update acknowledgement alone SHALL NOT count as acceptance, and for MCP the Runtime SHALL poll the same provider task until that exact key resolves or the provider reaches a terminal state. Acceptance of that response SHALL allow a new Conversation-scoped AgentSession reserved STARTING. THE SYSTEM SHALL commit RUNNING only after readiness, in the same agent.session.started.v1 transition that moves the turn from OPEN to RUNNING. THE SYSTEM SHALL NOT treat readiness as a precondition for creating the STARTING reservation that the handshake proves. Verify: exact-key provider fixture, replay and timeout integration tests.
- AC-019 IF the provider rejects the input but remains INPUT_REQUIRED, THE SYSTEM SHALL close the old UserRequest, create a new request for the current provider key and return the exact turn to WAITING_USER; IF acceptance is ambiguous, THE SYSTEM SHALL keep the turn WAITING_DEPENDENCY and prohibit a new AgentSession until reconciliation. Verify: reject, ambiguity and recovery tests.

## 13. Executable acceptance command

**Canonical feature acceptance command (PROPOSED; target test does not exist yet):**

```bash
cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture
```

**Narrow foundation evidence only:** the focused storage activation test passed in an earlier run; the current Windows full-workspace serial run is separately recorded in `CURRENT-RUN.md` and `docs/IMPLEMENTATION-STATUS.md`. Storage activation/result tests do not by themselves prove packaged desktop, provider auth, cancellation/quiescence, crash recovery or the end-to-end USER journey.

Do not mark the feature VERIFIED until the canonical target exists, covers AC-001 through AC-019 and passes on the pinned OS/provider.

## 14. Out-of-bounds paths and side effects

This package is IN_REVIEW and is an audit/specification record, not authorization to change runtime implementation. Any future source change requires an explicit scoped engineering task, fresh worktree review and owner approval. Candidate paths are focused daemon Conversation/agent supervisor modules, AgentSession/Conversation storage ports and SQLite transaction code, Conversation UI/Tauri bridge, tests and canonical owner docs/schemas.

Out of scope: Task planning/Attempt execution; Coworker/delegation; generic routing; LiteSPM; connectors/OAuth/SecretStore redesign; Environment/sandbox; browser/computer/terminal; arbitrary native module system; automated memory; interactive artifact runtime; rich media; cloud Runtime; mobile; replication; unrelated schema migrations.

Do not execute native prompts, install/update/remove software, access/change user-native profiles or secrets, cause network/filesystem effects or stop an externally owned process during design validation.

## 15. Assumptions and open questions

| ID | Question | Evidence/limitation | Owner | Gate |
|---|---|---|---|---|
| O-01 | First harness and exact route match. | Active Conversation dispatch contains only the OpenCode Server v2.0.32 branch; Codex transport helper is not called by this supervisor. Declared B-02 includes other harnesses and a generic framed Adapter 1.0 contract, so the source and declared route do not align. | Product/Agent Runtime | B-02 / T04 |
| O-02 | Handshake conformance and host/session identity. | OpenCode path performs session creation and readiness activation, but conformance to the declared generic Adapter 1.0 handshake and independently owned host lifecycle is not proven. | Agent Runtime | B-03 / T03-T04 |
| O-03 | Enforce the declared strict single-flight policy. | Storage admission currently inserts OPEN without checking for another nonterminal turn; no active-turn uniqueness constraint was found. | Conversation/Storage | B-01 / T01 |
| O-04 | Complete and qualify final-result transaction contract. | SQLite operation exists and is invoked by OpenCode supervisor; retry ordinal/provider-response identity and full provenance/rollback/concurrency tests remain gaps. | Storage/Conversation | B-04 / T02 / T06 |
| O-05 | A pinned harness may accept less than the 4 MiB / 1,024-message package. | 256 KiB in API.md is the UserRequest detail prompt cap, not the adapter budget. | Product/Agent Runtime | B-07 |
| O-06 | First adapter can report ordered bounded output/final/stop events. | Need pinned provider fixture. | Agent Runtime | B-05 |
| O-07 | Attachment/resource transfer is provider-specific. | Package exposes ResourceRefs only. | Resource/Agent Runtime | B-08 |
| O-08 | Native usage may be opaque/unknown. | No fabricated cost. | Usage | B-10 |

## 16. Requirement traceability

| Requirement | Canonical owner | Task | Acceptance |
|---|---|---|---|
| Turn admission and idempotency | API, STORAGE, EVENTS | 001-T09 | AC-001 |
| Plain chat, no Task | PRODUCT, DATA-MODEL | 001-T09 | AC-002 |
| Pin selected binding | STATE-MACHINES, AGENT-CONTROL | 001-T03 | AC-003 |
| Atomic readiness activation | STORAGE, EVENTS | 001-T03 | AC-004 / AC-005 |
| Supervisor + host lifecycle | SERVICES, STATE-MACHINES | 001-T03 / 001-T08 | AC-012 |
| Context/resources | STORAGE, SCHEMAS, TRUST | 001-T05 | AC-006 / AC-007 |
| Durable final message/turn commit | STORAGE, EVENTS | 001-T02 / 001-T06 | AC-009 |
| Provider event normalization | API, EVENTS, AGENT-CONTROL | 001-T04 / 001-T07 | AC-008 |
| Projection/stream/reconnect | API, PRESENTATION-RUNTIME | 001-T07 | AC-014 |
| Stop/retry | STATE-MACHINES, API | 001-T07 | AC-010 / AC-011 |
| Native handle privacy | STORAGE, AGENT-CONTROL | 001-T03 / 001-T09 | AC-013 |
| Conversation-scoped UserRequest continuation | API, STATE-MACHINES, STORAGE, TRUST | 001-T07 | AC-017 / AC-018 / AC-019 |
| Fail closed/route qualification | TESTING, ACCEPTANCE-AND-E2E-PLAN | 001-T09 | AC-015 / AC-016 |

