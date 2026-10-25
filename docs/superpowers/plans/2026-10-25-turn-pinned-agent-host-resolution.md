# Turn-Pinned Agent Host Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Resolve a Conversation turn's native host from the AgentBinding recorded in that turn's immutable creation event, even if the Conversation selector changes afterward.

**Architecture:** Replace current-selector host lookup with a turn-scoped storage read that verifies owner, Workspace, Conversation, turn, and unique creation event in one SQLite read transaction. Resolve only the exact pinned binding against currently eligible endpoint and Runtime-incarnation facts; never substitute a newer selection or Workspace default. This is observation only and does not start a process or establish session readiness.

**Tech Stack:** Rust, storage-core traits, rusqlite, SQLite, existing contract validators.

**Spec:** docs/NATIVE-COMPOSER-LLD.md; docs/AGENT-FABRIC.md; docs/SCHEMAS.md; docs/EVENTS.md.

## Global Constraints

- The immutable ConversationTurn creation event is authoritative for its AgentBinding.
- A later agent selection affects future turns and never reroutes an already committed turn.
- Host resolution proves current inventory eligibility only; it does not prove native startup, authentication, readiness, or dispatch.
- Keep same-owner, same-Workspace authorization and return no host for an ineligible or disabled pinned binding; never fall back to another binding.
- Keep Conversation Send fail-closed until the native session supervisor and safety gates are qualified.
- Do not copy credentials, native session IDs, provider history, or tool state across bindings.

## Review Focus

- Agent selector changes after turn commit: test that the host remains pinned to the turn's original binding.
- A turn ID from another Conversation or Workspace: test that resolution refuses it.
- Missing, duplicate, or malformed turn-created event: test integrity failure with no host.
- Pinned binding becomes disabled or its endpoint offer expires: test no-host result without selector fallback.
- Runtime goes offline or its current incarnation changes: test no-host result for the stale host.

---

### Task 1: Update the normative resolution contract

**Files:**
- Modify: docs/NATIVE-COMPOSER-LLD.md
- Modify: docs/AGENT-FABRIC.md
- Modify: docs/TESTING.md
- Regenerate: implementation/coverage.csv

**Interfaces:**
- Produces: the turn-scoped resolver contract used by the storage and supervisor layers.

- [x] Clarify that session host resolution receives principal, Workspace, Conversation, and turn identity and derives the binding from the unique conversation.turn.created.v2 event.
- [x] State that selection changes after turn creation cannot reroute that turn and that disabled/stale pinned hosts fail closed without fallback.
- [x] Keep route-level native startup, readiness, dispatch, response streaming, and usage ingestion explicitly outstanding.
- [x] Run python3 scripts/validate_implementation_coverage.py --write and python3 scripts/validate_implementation_plan.py.

### Task 2: Add failing storage tests

**Files:**
- Modify: crates/storage-sqlite/src/agent_binding_tests.rs
- Modify: apps/litecoworkd/src/conversation_operator_tests.rs to migrate the existing route test from unscoped to committed-turn host resolution.
- Modify: crates/storage-core/src/conversation.rs only if test compilation requires the new typed contract.

**Interfaces:**
- Consumes: the current enabled binding and eligible Runtime fixture from setup().
- Produces: SqliteWorkspaceStore::resolve_conversation_session_host_for_turn(principal_id, workspace_id, conversation_id, turn_id) -> Result<Option<ResolvedConversationHost>, StoreError>.

- [x] Add conversation_host_resolution_uses_turn_pinned_binding_after_selection_changes. Create a turn pinned to BINDING_ID, move the Conversation selector to another enabled binding, and assert that the returned host still names BINDING_ID, ENDPOINT_ID, RUNTIME_ID, and INCARNATION_ID.
- [x] Add a mismatched turn/Conversation or non-owner assertion and require NotFound.
- [x] Add disabled-binding, expired-endpoint, and offline-Runtime cases; each requires None with no fallback to the newly selected binding.
- [x] Corrupt and remove the turn-created event in the fixture; require Integrity for malformed identity and missing authority.
- [x] Run cargo test --locked -p storage-sqlite conversation_host_resolution_uses_turn_pinned_binding_after_selection_changes; expected: compile/test failure before implementation.

### Task 3: Implement exact turn-scoped host resolution

**Files:**
- Modify: crates/storage-core/src/conversation.rs
- Modify: crates/storage-sqlite/src/conversations.rs

**Interfaces:**
- Implement: resolve_conversation_session_host_for_turn(&self, principal_id: &str, workspace_id: &str, conversation_id: &str, turn_id: &str) -> Result<Option<ResolvedConversationHost>, StoreError>.
- Preserve: ResolvedConversationHost fields agent_binding_id, endpoint_id, runtime_id, runtime_incarnation_id.

- [x] Read the active Workspace owner, turn-to-Conversation relation, and unique turn-created event in one deferred read transaction.
- [x] Validate the event payload identity and non-empty pinned AgentBinding ID; reject missing or ambiguous authority as Integrity.
- [x] Resolve only that exact same-Workspace binding against enabled/lead-eligible status, endpoint policy, fresh endpoint binding, active EXECUTOR Workspace enrollment, online Runtime, and exact READY current incarnation.
- [x] Return None for an ineligible pinned host. Do not read Conversation.active_agent_binding_id or Workspace.default_agent_binding_id as a substitute.
- [x] Include the incarnation from the exact eligible endpoint binding row rather than doing a second current-incarnation lookup.
- [x] Run the storage host-resolution tests to verify the implementation.

### Task 4: Revalidate contracts and checkpoint

**Files:**
- Include only the plan, normative docs, storage trait/implementation, focused storage/route tests, and generated coverage inventory.

- [x] Run rustfmt --check on the added storage test and git diff --check; inspect edited hunks in pre-existing Rust files whose whole-file formatting differs from the repository baseline.
- [x] Run the focused host-resolution test and full storage-sqlite package suite.
- [x] Run the migrated litecoworkd fail-closed route test and all storage-core tests.
- [x] Run python3 scripts/validate_implementation_coverage.py --write, python3 scripts/validate_implementation_plan.py, and python3 scripts/audit_v1_contracts.py.
- [x] Review the final diff; leave all unrelated existing working-tree changes unstaged.
- [x] Commit and push the verified slice.

---

## Scope Boundary

This plan fixes the target-binding/host-resolution boundary only. The Conversation route still has no qualified native session supervisor; this change does not activate the session, dispatch prompts, stream responses, stop/retry providers, recover native processes, or enable Send.
