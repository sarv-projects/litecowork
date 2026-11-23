# Conversation Dispatch Claim Recovery Implementation Plan

> **For agentic workers:** execute this plan inline, one task at a time. Use the repository's Feature 001 T20 owner contract as the implementation authority; this plan is sequencing only.

**Goal:** recover a durably admitted Conversation turn after a transient/ambiguous dispatch-intent claim failure while the daemon remains open, without starting provider work until exact durable ownership is proven.

**Architecture:** add a bounded exact `(turn_id, retry_ordinal)` read to the existing `ConversationTurnStore`. The supervisor reconciles a failed/false compare-and-set from that exact durable row. The coordinator retries only unresolved claim/storage outcomes with capped exponential delay. A same-incarnation CLAIMED row proves the current serialized worker owns the attempt; a different owner or SETTLED row is a no-op; an UNCLAIMED row is retryable; missing or malformed ownership is a closed diagnostic. Provider/session work remains strictly after ownership proof.

**Scope:** `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, `crates/storage-sqlite/src/conversation_turn_tests.rs`, `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecoworkd/src/agents/turn_coordinator.rs`, adjacent tests, Feature 001 owner docs, storage/service/state-machine documentation, traceability and current-run status. No public route/event/schema, migration, provider adapter behavior, or UI state is added.

## Task 1 — Pin the decision and contracts

- Record A-116 as USER-DELEGATED DIRECTION, not owner acceptance, in the alignment register.
- Expand Feature 001-T20's allowed paths and define exact-claim outcomes, recovery timing, and failure semantics in SPEC §7.9, DATA-MODEL, WIRING, and errors/recovery.
- Update global Storage, Services, State Machines, master-plan, traceability and status wording to match the owner proposal.
- Run structural/design consistency checks before source edits.

## Task 2 — Add the exact durable read (RED → GREEN)

- Add a storage test for missing intent, exact UNCLAIMED record, exact CLAIMED owner, SETTLED state, and mismatched retry ordinal.
- Run `cargo test -j 1 -p storage-sqlite --locked conversation_dispatch_intent_exact_read -- --nocapture`; it must fail to compile because the exact getter is absent.
- Add `get_conversation_dispatch_intent(turn_id, retry_ordinal)` to `ConversationTurnStore` and SQLite, using an exact primary-key query and strict state decoding.
- Rerun the focused test and storage crate tests affected by the contract.

## Task 3 — Reconcile claims before preflight (RED → GREEN)

- Add a pure supervisor regression for each exact claim reconciliation result.
- Run the new filtered daemon test before implementation and confirm the desired helper/behavior is absent.
- For a confirmed CAS success, continue. After CAS false/error, inspect only the exact intent: same-incarnation CLAIMED continues; another incarnation or SETTLED returns without preflight/settlement; UNCLAIMED returns retryable claim failure; missing/malformed ownership returns a closed non-retryable diagnostic.
- Run the supervisor's focused dispatch filter and the new reconciliation test.

## Task 4 — Retry only claim reconciliation in-process

- Add unit tests for the delay schedule and cap.
- Retry `DispatchIntentClaim` only, using 100 ms exponential backoff capped at five seconds. Keep the same turn/retry identity. Do not retry setup/provider failures or an ambiguous provider send.
- Verify a transient SQLite claim failure while the daemon remains open is retried and progresses once the store recovers; verify the provider/session path cannot run before claim resolution.
- Run focused Conversation coordinator/supervisor tests.

## Task 5 — Evidence and handoff

- Update T20 acceptance only for the implemented exact-read/retry behavior. Keep T20 PARTIAL because service-boundary extraction, exhaustive fault coverage, provider, packaged, and qualification gates remain.
- Record exact command/output, red baseline, final green evidence, doc-validator counts and unresolved gates in `CURRENT-RUN.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md`, WIRING and traceability.
- Run focused Rust tests, format checks, feature-package validation, owner-consistency and Markdown audits. Inspect the entire intended diff before commit.
