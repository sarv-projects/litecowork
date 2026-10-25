# Conversation Session STARTING Reservation Implementation Plan

> **For agentic workers:** execute the steps in order and keep the route fail-closed until a qualified supervisor consumes these storage operations.

**Goal:** Persist a Conversation-scoped AgentSession reservation for the exact immutable turn target before native startup, and make interrupted STARTING reservations recoverable.

**Architecture:** Extend the existing AgentSessionStore boundary with Conversation-specific start, list-STARTING, and mark-LOST operations. The start transaction authorizes the principal and Workspace, verifies the Conversation/turn relationship and unique conversation.turn.created.v2 authority, resolves only that event's AgentBinding against currently eligible Runtime/endpoint facts, then persists the STARTING aggregate, event, and exact request receipt atomically. Recovery reads and settles only Conversation-scoped STARTING records with expected-version fencing. This slice does not activate a session, move a turn to RUNNING, start a process, dispatch a prompt, or enable the route.

**Tech Stack:** Rust, storage-core AgentSessionStore, rusqlite, existing SQLite v1 AgentSession scope/index/trigger contract, implementation validators.

**Normative sources:** docs/NATIVE-COMPOSER-LLD.md; docs/AGENT-FABRIC.md; docs/STATE-MACHINES.md; docs/EVENTS.md; docs/SCHEMAS.md; docs/API.md.

## Constraints

- The immutable turn-created event determines AgentBinding; the current Conversation selector and Workspace default are never consulted.
- Reservation authorization and endpoint/Runtime eligibility checks share one immediate SQLite transaction with session/event/receipt persistence.
- A Conversation session must have scope_kind=CONVERSATION, the exact same Workspace/Conversation/turn, and no Task, TaskSpec, or Attempt fields.
- STARTING is not readiness, dispatch authority, or permission to append an agent response.
- No native session ID, credential, command line, or process locator enters events, aggregate state, or receipts.
- Recovery enumeration is bounded and scoped; LOST settlement is an expected-version state transition and never settles the ConversationTurn itself.
- Keep Conversation Send, retry, and cancel behavior fail-closed until a qualified supervisor owns startup, readiness activation, dispatch, stream persistence, stop, and reconciliation.

## Task 1: Record the storage contract before implementation

**Files:** docs/NATIVE-COMPOSER-LLD.md, docs/EVENTS.md, docs/API.md, docs/TESTING.md, this plan, implementation/coverage.csv.

- [x] Specify exact event-pinned binding resolution for Conversation STARTING reservations.
- [x] Clarify that STARTING reservation and recovery are not readiness activation or route enablement.
- [x] Generalize the agent.session.starting.v1 ownership statement to supported AgentSession scopes.
- [x] Regenerate implementation coverage and run docs validators.

## Task 2: Add failing storage contract tests

**Files:** crates/storage-sqlite/src/agent_binding_tests.rs or a dedicated Conversation-session test module; crates/storage-core/src/lib.rs only for the new public input type.

- [ ] Assert valid owner/Workspace/turn reserves the binding from the turn-created event even after selector change.
- [ ] Assert foreign owner, mismatched turn, malformed/missing authority, disabled binding, expired endpoint, offline/stale Runtime, and duplicate active session fail closed.
- [ ] Assert exact reservation request replay returns the original committed session/event.
- [ ] Assert scoped STARTING enumeration returns Conversation sessions only, is bounded, and excludes closed records.
- [ ] Assert LOST settlement requires matching Workspace/session/scope/version and commits one event.
- [ ] Observe expected compile/test failure before implementation.

## Task 3: Implement durable reservation and recovery operations

**Files:** crates/storage-core/src/lib.rs; crates/storage-sqlite/src/lib.rs; focused storage tests.

- [ ] Add typed Conversation start and lost-transition inputs to AgentSessionStore.
- [ ] Validate exact scope identity and event payload before writes.
- [ ] Resolve the immutable turn event and current exact-host eligibility in one immediate transaction; do not fall back.
- [ ] Atomically insert AgentSession STARTING, aggregate snapshot, starting event, and owner-scoped request receipt.
- [ ] Add bounded Conversation STARTING listing and version-fenced LOST settlement without changing ConversationTurn state.
- [ ] Preserve Task-planning admission guards and the existing route fail-closed behavior.

## Task 4: Verify and checkpoint

- [ ] Run the focused Conversation-session reservation/recovery tests, storage-core tests, and relevant storage-sqlite tests.
- [ ] Run implementation coverage, plan validation, V1 contract audit, rustfmt check on changed test code, and git diff --check.
- [ ] Review that reservation cannot start/activate a process and all unrelated working-tree changes remain unstaged.
- [ ] Commit and push the slice.

## Boundary

A qualified supervisor still needs to own AgentHost lifecycle, native adapter startup/readiness, atomic readiness activation with the ConversationTurn, context delivery, output projection/stream persistence, usage aggregation, stop/cancel, retry, and crash reconciliation. No part of this plan enables user-facing Send.
