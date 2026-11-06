# Research and Decision Log - Conversation Session Supervisor

**Feature:** 001 | **Status:** IN_REVIEW | **Checked:** 2026-11-06 — active-source audit; upstream protocol claims are not web-verified.

## 1. Repository sources inspected

| Source | Revision/path | Finding | Limit |
|---|---|---|---|
| Architecture | main `ARCHITECTURE.md` at c7e9c05 | Owner boundaries; Runtime incarnation; Trust/Effect separation | Normative design, not proof of live integration |
| Product | `docs/PRODUCT.md`, `docs/V1-PRODUCT-RELEASE-CONTRACT.md` | Chat-first BYOA; plain Conversation; V1 local-first | Target scope |
| State machines | `docs/STATE-MACHINES.md` | Turn/session/host states; stop/retry/waiting and recovery semantics | Some supervisor components are target-only |
| Service contracts | `docs/SERVICES.md` | ConversationService, AgentTurnCoordinator and AgentSessionSupervisor responsibilities/dependencies | Not all wiring exists in source |
| API | `docs/API.md`, `apps/litecoworkd/src/conversation_operator.rs` | Active submit path stores USER+OPEN, then detached-dispatches when a supervisor is present and returns 202; retry/cancel routes exist but have state-machine mismatches | Source integration is not end-to-end OS/USER qualification |
| Storage | `docs/STORAGE.md`, `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs` | Context reader, event-prefix semantics, activation and atomic final-result write are implemented | Recovery/outbox/single-flight and full fault-injection coverage remain open |
| Events/schemas | `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/schemas/*` | Typed event families, event payload validation, no native handles | Must promote any new exact payload |
| Agent boundary | `docs/AGENT-FABRIC.md`, `docs/AGENT-CONTROL.md` | Installation/auth/descriptor/host/session are independent; native secrets stay native | Adapter readiness handshake unresolved |
| Composer | `docs/NATIVE-COMPOSER-LLD.md` | UI/native parity and explicit Send eligibility boundary | Native dispatch code now exists for OpenCode, but exact desktop Send qualification remains absent |
| Testing | `docs/TESTING.md`, `docs/ACCEPTANCE-AND-E2E-PLAN.md` | CODE/SYSTEM/USER and OS-specific qualification | Current OpenCode implementation route is not yet reconciled to an approved/qualified OS-auth tuple |

## 2. Active main-worktree source evidence

The audit was performed against the live working tree at `C:\Users\Sonali\Desktop\projectwork\business_Dev\litecowork`; all pre-existing user changes were preserved. No reset, clean, commit or push was performed.

Observed types and call paths:
- `AgentSessionSupervisor::dispatch_turn` is called by the Operator after `create_conversation_turn` commits. The call is launched with detached `tokio::spawn`; dispatch errors are emitted to stderr.
- The supervisor reserves STARTING, uses `activate_conversation_agent_session`, loads `ConversationContextPackage`, and has one `opencode` provider branch pinned to OpenCode Server protocol v2.0.32.
- `ConversationTurnStore::commit_conversation_agent_response` exists in storage-core/SQLite and the OpenCode path invokes it.
- Storage also exposes `list_starting_conversation_agent_sessions` and `mark_starting_conversation_agent_session_lost`, but no active Runtime-startup caller for Conversation recovery was found.
- Exact-replay submit returns the stored 202 receipt; it does not recheck whether the detached dispatch worker is still active or re-admit it.
- The active supervisor creates/updates a host-instance record but directly owns the per-turn OpenCode server start/stop; a separate AgentHostSupervisor and AgentTurnCoordinator call path was not found.

Storage-level activation and result commits are code evidence. Neither proves a qualified Windows desktop conversation lifecycle.

## 3. Native transport and supervisor boundary

The OpenCode helper is called by the active supervisor and provides session create, context-aware message send and bounded SSE/event reading for the v2.0.32 protocol. The Codex helper exists but was not found in the Conversation supervisor call path. Only the `opencode` provider key is dispatched. The code starts/stops an OpenCode server within the per-turn blocking flow; it does not show the independently owned AgentHostSupervisor, nor does it route through an AgentTurnCoordinator.

The current stream consumer accumulates output for final persistence; the Operator does not forward native token deltas as a complete correlated transient stream. Events are untrusted observations. Exact adapter protocol conformance, stop/quiescence, Windows packaging, credentials and end-to-end owner USER proof remain open.

## 4. Existing UI stream

Desktop Tauri bridge and resumable Conversation UI reducer already consume bounded durable projection snapshots/upserts with backpressure/cursors. API contract defines transient `turn.delta`, `turn.settled` and `rich.draft`, but local Operator does not emit them. New feature must reuse durable projection and not build an independent provider-to-WebView stream.

## 5. Desktop Conversation UI audit

The desktop source was traced from the active-turn projection through the composer, stop action and generated API contract. The current UI does not complete the target stop/retry contract:

1. `ConversationTurnPresentation` in `apps/litecowork-ui/src/api/operator-api.ts` exposes only `turn_id`, `status`, `retry_ordinal` and `presentation_preference`. It does not expose the aggregate `version` required by `If-Match` on retry/cancel. The UI therefore cannot construct a valid optimistic-concurrency precondition from its active-turn snapshot.
2. `ConversationsPage.tsx` computes `isTurnRunning` from transient STREAMING/RECONNECTING frames or snapshot status RUNNING. However, `cancelTurn` searches only `transientTurns` and returns immediately when none exists; it does not fall back to `currentSnapshot.active_turn`. The current Operator does not emit the documented `turn.delta` / `turn.settled` native token stream, so the normal OpenCode execution path can show the Stop button from the durable RUNNING snapshot while providing no transient turn for the handler to cancel.
3. When a transient turn does exist, the UI sends `expectedVersion: 1` unconditionally. The storage activation transaction advances a newly admitted turn from version 1 (OPEN) to version 2 (RUNNING), so the normal running turn is already stale against this precondition. A valid cancel must use the current authoritative turn version and then wait for supervisor-observed native stop/quiescence; the current backend instead immediately settles CANCELLED.
4. The generated OpenAPI TypeScript surface includes `retryConversationTurn`, but repository-wide source search found no desktop source or test call site for it. The Conversation page has no failed-turn retry action or UI flow, despite the API route being registered.
5. The composer treats only RUNNING or a transient stream as active. OPEN is labelled “Preparing” but is not included in `isTurnRunning`, so the Send control remains available during durable OPEN admission. WAITING_USER, WAITING_DEPENDENCY and CANCEL_REQUESTED are also not included in the send guard. This contradicts the strict single-flight B-01 interaction contract and becomes observable because storage currently admits multiple nonterminal turns.
6. The optimistic USER message is appended before the POST and removed/restored only on request failure; the UI does not visibly reconcile the accepted `ConversationTurnReceipt` into a local active-turn projection. It relies on subsequent projection delivery. Since the native transient stream is not emitted, progress/stop behavior must be qualified against the durable projection path, not inferred from the optimistic bubble.

These are source-level UI findings, not a browser USER test. They require UI/API contract changes and rendered regression tests after the backend lifecycle is corrected. Do not paper over them by inventing a version or treating a transient stream sequence as the aggregate version.

## 6. Important contract mismatches/gaps

1. The actual Operator path bypasses the target AgentTurnCoordinator and starts AgentSessionSupervisor directly with detached `tokio::spawn`; no durable dispatch intent/outbox/claim is committed with USER+OPEN.
2. Exact idempotent replay returns the 202 receipt without proving a dispatch worker exists. No Runtime-startup sweep for orphaned OPEN turns or STARTING Conversation sessions was found.
3. `create_conversation_turn` inserts a new OPEN turn but has no same-Conversation nonterminal-turn guard; no matching active-turn unique constraint was found. The accepted B-01 strict single-flight rule is therefore not enforced in the storage admission boundary.
4. Retry is inconsistent: Operator's `TurnCommand::Retry` sets the turn RUNNING and assigns a new session ID before dispatch, while `start_conversation_agent_session` requires the turn to remain OPEN with no session pointer. The supervisor start call can therefore reject the retry before provider dispatch.
5. Running cancellation is inconsistent with the state-machine owner contract: the Operator transitions RUNNING -> CANCEL_REQUESTED and immediately settles CANCELLED in the same request path; no native interrupt/quiescence observation is made before terminal settlement.
6. Several errors before the supervisor's `execute_agent_turn` error-handling block (host resolution, profile lookup, host-instance setup, STARTING reservation) can return to the detached caller, where the Operator only logs them. Such failures can leave the durable turn OPEN; the error path inside the supervisor is not a substitute for startup recovery.
7. The OpenCode code path is real but only CODE-level. It is not the declared generic framed Adapter 1.0 route, and the source has no Claude Code/Codex/Gemini/Cline Conversation dispatch qualification.
8. The final-result SQLite transaction exists, but its current typed request lacks explicit retry ordinal/provider-response identity; author JSON is passed through from the caller. Existing test coverage is focused rather than complete transaction fault injection.
9. OpenCode/Codex transport input is 256 KiB while product ConversationContextPackage allows 4 MiB. Need an explicit provider/context reduction contract.
10. Native output is not authority/effect proof. Durable projection should remain the user-visible source of truth.
11. The active-turn projection omits the aggregate version needed by the If-Match contract; the UI cannot safely cancel/retry from the projection it currently receives.
12. The Stop control is not connected to the durable active-turn identity: it searches only transient stream turns, while the current Operator does not emit the transient native token frames that would populate that collection.
13. The Stop request hardcodes expected version 1, but activation advances the turn to version 2 before RUNNING; even a populated transient turn would submit a stale precondition.
14. The retry route is present in generated API types but has no desktop call site or failed-turn retry UI. Composer activity guards also omit OPEN, WAITING_USER, WAITING_DEPENDENCY and CANCEL_REQUESTED.
15. UI source assertions and reducer tests do not substitute for rendered browser tests of stop/retry, stale version, repeated send, and accepted-turn reconciliation.

## 7. Alternatives and rationale

| Alternative | Decision | Rationale |
|---|---|---|
| Direct UI -> native provider stream | Reject | Bypasses daemon ownership, correlation, normalized events and durable reconciliation |
| Let Runtime/AgentHost READY activate turn | Reject | Process liveness is not exact native session readiness |
| Reuse provider-private session across agent bindings | Reject | Cross-harness identity/auth/private transcript cannot be migrated safely |
| Commit final message and turn separately | Reject | Crash can expose an answer without terminal state or mark completion without saved response |
| Treat transport helpers as supervisor | Reject | Transport has no durable lifecycle/host/grant/recovery owner |
| Use existing projection with durable transcript | Select | One canonical transcript and existing reconnect/backpressure path |
| Transient deltas before durable commit | Conditional | Allowed only as bounded, correlated presentation; cannot create saved message |

## 8. Open research/decision register

| ID | Topic | Next evidence needed | Owner |
|---|---|---|---|
| R-01 | OpenCode first-route protocol contract | Reconcile exact v2.0.32 Server request/event semantics, readiness, timeout and termination against version-pinned upstream contract and safe captured fixtures; active code does not implement generic Adapter 1.0 | Agent Runtime |
| R-02 | Process/host lifecycle | Prove Windows spawn/attach/stop ownership, Job Object containment, cancellation, concurrency and host reuse; current Conversation path starts/stops OpenCode per turn | Agent Runtime/OS |
| R-03 | Prompt input bound and compaction | Reconcile 4 MiB provider-neutral context with 256 KiB OpenCode input; define reviewable compaction/resource materialization and exact serialization tests | Product/Agent Runtime |
| R-04 | Resource materialization | Supported ResourceRef content types, authorization, path safety, MIME/size, digest and provenance | Resource/Agent Runtime |
| R-05 | Final message transaction | Strengthen author/provenance validation; add explicit retry/provider-response identity if required; wrong-scope, conflicting replay, rollback/fault-injection, crash-window and concurrent settlement tests | Storage |
| R-06 | Cancellation and ambiguous final event | Implement native interrupt, observe quiescence, reconcile final-event-vs-stop races, and prove retry never duplicates a prompt | Agent Runtime/Conversation |
| R-07 | Usage | Native usage signals and trust label; no Conversation cross-agent usage writer is currently wired | Usage owner |
| R-08 | Desktop stop/retry interaction | Add authoritative turn version to the safe active-turn projection; bind Stop to that turn and a real supervisor stop request; add failed-turn retry action with idempotency/version handling; enforce all nonterminal statuses in composer admission; add rendered tests for no-stream, stale-version, repeated-send and racing settlement cases | Desktop UI / Operator / Conversation |

No web research has been used to claim current upstream API behavior. Version-pinned native protocol research is a prerequisite task, not filled by assumptions.

