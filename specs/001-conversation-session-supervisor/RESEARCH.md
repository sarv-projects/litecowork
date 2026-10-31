# Research and Decision Log - Conversation Session Supervisor

**Feature:** 001 | **Status:** DRAFT | **Checked:** 2026-10-31

## 1. Repository sources inspected

| Source | Revision/path | Finding | Limit |
|---|---|---|---|
| Architecture | main `ARCHITECTURE.md` at c7e9c05 | Owner boundaries; Runtime incarnation; Trust/Effect separation | Normative design, not proof of live integration |
| Product | `docs/PRODUCT.md`, `docs/V1-PRODUCT-RELEASE-CONTRACT.md` | Chat-first BYOA; plain Conversation; V1 local-first | Target scope |
| State machines | `docs/STATE-MACHINES.md` | Turn/session/host states; stop/retry/waiting and recovery semantics | Some supervisor components are target-only |
| Service contracts | `docs/SERVICES.md` | ConversationService, AgentTurnCoordinator and AgentSessionSupervisor responsibilities/dependencies | Not all wiring exists in source |
| API | `docs/API.md` | Turn route currently stores USER+OPEN, settles unavailable and responds 503; Send disabled | Source status dated 2026-10-25 in doc; cross-checked source plan on Oct 31 |
| Storage | `docs/STORAGE.md` | Context reader, event-prefix semantics, immediate transaction rule and host binding | Final result writer gap remains |
| Events/schemas | `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/schemas/*` | Typed event families, event payload validation, no native handles | Must promote any new exact payload |
| Agent boundary | `docs/AGENT-FABRIC.md`, `docs/AGENT-CONTROL.md` | Installation/auth/descriptor/host/session are independent; native secrets stay native | Adapter readiness handshake unresolved |
| Composer | `docs/NATIVE-COMPOSER-LLD.md` | UI/native parity and explicit disabled Send boundary | Must update Oct 31 activation fact |
| Testing | `docs/TESTING.md`, `docs/ACCEPTANCE-AND-E2E-PLAN.md` | CODE/SYSTEM/USER and OS-specific qualification | No exact first target route approved |

## 2. Preserved feature worktree evidence

**Branch:** `feature/conversation-session-supervisor`  
**HEAD:** `3052341473e964bececf7f34c9d45923848e5218`  
**Path:** `archives/legacy-codebase-c7e9c05/.worktrees/conversation-session-supervisor` (local ignored archive in docs-first main checkout).

Observed types/ports:
- `ConversationAgentSessionStart`, `ActivateConversationAgentSession`, `CommittedConversationActivation`.
- `AgentSessionStore::start_conversation_agent_session`, `mark_starting_conversation_agent_session_lost`, `list_starting_conversation_agent_sessions`, `activate_conversation_agent_session`.
- `ConversationTurnStore::load_conversation_context_for_turn`, `resolve_conversation_session_host_for_turn`, `create_conversation_turn`, `replay_conversation_turn`.
- Activation checks owner/session/turn versions, immutable turn-pinned binding, endpoint offer, Runtime/incarnation, Workspace EXECUTOR enrollment and READY/BUSY host, then stores opaque native handle only in Runtime-local host binding. Handle is skipped by Serde and redacted from Debug.
- Focused test `conversation_activation_commits_session_turn_and_local_host_binding_atomically` passed after final changes on 2026-10-31.
- `cargo test -p storage-sqlite --locked`: 593 unit tests + 2 AgentRegistryCache integration tests passed before final handle-redaction-only additions. Full suite not rerun after those final changes.
- Linux Secret Service integration intentionally ignored due active-user side effects; Windows Credential Manager tests unavailable on Linux.

This is storage evidence in a dirty worktree, not supervisor integration or end-to-end qualification. That worktree is preserved and must not be reset/stashed/edited as part of this documentation run.

## 3. Native transports

Preserved source contains OpenCode Server and Codex App Server transport helpers. OpenCode provides bounded SSE reading, session create, context-aware message send, abort and stop. Codex provides thread/turn start, interruption and event/poll handling. Both are explicitly transport-only/staged. They do not implement AgentSessionSupervisor, host ownership, trust admission, final Conversation persistence or end-to-end turn settlement.

No caller for `activate_conversation_agent_session` was found outside its storage trait/implementation/tests. No qualified supervisor exists. `apps/litecoworkd/src/agents/mod.rs` discovers local installations; install presence is not auth/session readiness.

## 4. Existing UI stream

Desktop Tauri bridge and resumable Conversation UI reducer already consume bounded durable projection snapshots/upserts with backpressure/cursors. API contract defines transient `turn.delta`, `turn.settled` and `rich.draft`, but local Operator does not emit them. New feature must reuse durable projection and not build an independent provider-to-WebView stream.

## 5. Important contract mismatches/gaps

1. Main docs-first snapshot predates the storage-only Conversation activation addition present in the dirty feature worktree. Promote only the accurately evidenced contract updates to NATIVE-COMPOSER, STORAGE, AGENT-CONTROL, EVENTS and STATE-MACHINES/SERVICES.
2. API, SERVICES and STORAGE describe context load; no supervisor dispatches it.
3. Existing storage operations create USER+OPEN, read context, resolve host and activate session/turn. No observed port appends final AGENT response and settles terminal turn in one transaction.
4. OpenCode/Codex transport input is 256 KiB while product ConversationContextPackage allows 4 MiB. Need explicit provider/context reduction contract.
5. No approved first OS/harness/auth/version tuple or exact readiness handshake.
6. Native output is not authority/effect proof. Durable projection should remain the user-visible source of truth.
7. The historical reset audit is stale on focused activation test status; Oct 31 plan records its pass and package suite limitation.

## 6. Alternatives and rationale

| Alternative | Decision | Rationale |
|---|---|---|
| Direct UI -> native provider stream | Reject | Bypasses daemon ownership, correlation, normalized events and durable reconciliation |
| Let Runtime/AgentHost READY activate turn | Reject | Process liveness is not exact native session readiness |
| Reuse provider-private session across agent bindings | Reject | Cross-harness identity/auth/private transcript cannot be migrated safely |
| Commit final message and turn separately | Reject | Crash can expose an answer without terminal state or mark completion without saved response |
| Treat transport helpers as supervisor | Reject | Transport has no durable lifecycle/host/grant/recovery owner |
| Use existing projection with durable transcript | Select | One canonical transcript and existing reconnect/backpressure path |
| Transient deltas before durable commit | Conditional | Allowed only as bounded, correlated presentation; cannot create saved message |

## 7. Open research/decision register

| ID | Topic | Next evidence needed | Owner |
|---|---|---|---|
| R-01 | First adapter exact readiness/turn protocol | Version-pinned official API/CLI contract plus captured safe fixtures | Agent Runtime |
| R-02 | Process/host lifecycle | Ownership behavior per OS, spawn/attach/stop and concurrency tests | Agent Runtime/OS |
| R-03 | Prompt input bound and compaction | Exact request serialization size, provider limit and UX decision | Product/Agent Runtime |
| R-04 | Resource materialization | Supported content types, path safety, MIME/size, provenance | Resource/Agent Runtime |
| R-05 | Final message transaction | Storage/event payload schema and crash injection | Storage |
| R-06 | Cancellation and ambiguous final event | Provider protocol race/replay behavior | Agent Runtime/Conversation |
| R-07 | Usage | Native usage signals and trust label | Usage owner |

No web research has been used to claim current upstream API behavior. Version-pinned native protocol research is a prerequisite task, not filled by assumptions.

