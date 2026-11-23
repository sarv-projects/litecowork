# Conversation Runtime Readiness Implementation Plan

> **For agentic workers:** Execute this plan task-by-task in the current workspace. Keep changes inside T32's allowed paths and preserve the exact replay-before-readiness order.

**Goal:** Refuse fresh Conversation dispatch admission unless the exact current local Runtime is ONLINE and its incarnation is READY, while preserving idempotent receipt recovery.

**Architecture:** The UI uses an identity-matched lifecycle projection for useful disabled states. The authenticated Operator supplies its own Runtime/incarnation identity to SQLite, where the final readiness fence is checked inside each IMMEDIATE transaction after exact request replay and before writes. Runtime lifecycle remains the only readiness authority.

**Tech Stack:** Rust, SQLite, Tauri v2, React/TypeScript, existing OpenAPI/JSON Schema validators.

**Spec:** [Conversation Runtime Readiness design](../specs/2026-11-23-conversation-runtime-readiness-design.md), Feature 001 `SPEC.md` and delegated A-119.

## Global constraints

- New turns and Retry require exact current `ONLINE` + `READY`.
- Exact idempotency replay precedes readiness and mutable admission checks.
- `RUNTIME_UNAVAILABLE` is the public typed refusal; no new public request identity is added.
- Stop, settlement, reads and exact pending-send recovery remain available.
- Native auth remains harness-owned; readiness is not qualification.
- Keep Feature 001 revision 1.2 IN_REVIEW and T09 open for packaged/provider evidence.

## Review focus

- A stale/superseded Runtime identity must not admit a turn.
- A Runtime that becomes degraded between UI rendering and SQLite commit must not admit a turn.
- Exact first-send/existing-send/Retry replay after degradation must still return the original receipt.
- A failed fresh request must not leave a message, turn, event, intent, snapshot or request receipt.
- Stop and pending exact-send reconciliation must not be hidden by the new readiness gate.

## File map

- Feature 001 owner package and A-119 register row: normative direction, task and lifecycle truth.
- `storage-core` and `storage-sqlite`: ephemeral Runtime fence type and same-transaction checks.
- `conversation_operator.rs` and daemon tests: authenticated Runtime identity wiring and 503 mapping.
- UI runtime helper, App and Conversation state/page tests: identity-matched operational readiness feedback.
- API/OpenAPI/UI ledgers/WIRING/status/current run: same-change traceability and evidence.

## Tasks

### Task 1: Pin storage admission behavior

**Files:** `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, related storage tests, Feature 001 `TASKS.md`.

- [ ] Add failing tests for unready/stale fence denial without writes.
- [ ] Add failing tests proving exact receipt replay still succeeds after the Runtime becomes unready.
- [ ] Run the narrow Rust tests and confirm the new cases fail for the intended missing gate.
- [ ] Add a required internal Runtime fence to production first-send, existing-send and Retry writes; check it after replay inside the same IMMEDIATE transaction.
- [ ] Run focused storage tests, including single-flight, replay, retry-intent and rollback regressions.

### Task 2: Wire the authenticated Operator

**Files:** `apps/litecoworkd/src/conversation_operator.rs`, daemon tests, `docs/API.md`, OpenAPI, Feature 001 API/error contract.

- [ ] Pass the Operator's own Runtime/incarnation identity for each fresh admission/Retry.
- [ ] Map the storage readiness refusal to existing `503 RUNTIME_UNAVAILABLE`.
- [ ] Prove existing exact-replay paths return before new-work checks; prove fresh refusal creates no records.
- [ ] Keep request bodies, public IDs, events and idempotency digests unchanged.

### Task 3: Make UI readiness truthful

**Files:** UI `App.tsx`, `runtime-health.ts`, Conversation state/page, focused UI tests, UI screen/action ledgers.

- [ ] Add failing tests for missing/mismatched lifecycle, OFFLINE/DEGRADED/unknown state and ready state.
- [ ] Gate fresh Send and Retry on matching ONLINE/READY lifecycle state.
- [ ] Keep Stop and the exact saved-send recovery action available under their existing rules.
- [ ] Show a concise Runtime refresh reason when Send/Retry is closed.
- [ ] Run focused UI tests and TypeScript validation.

### Task 4: Reconcile owner docs and evidence

**Files:** Feature 001 `SPEC.md`, `TASKS.md`, `DATA-MODEL.md`, `WIRING.yaml`, API/UI contracts, global API/OpenAPI/UI ledgers, master plan, implementation status, external audit disposition, `CURRENT-RUN.md`.

- [ ] Keep one readiness definition in the owner package and link it from traceability.
- [ ] Record exact commands/results and remaining T09 qualification gates.
- [ ] Run API route, feature-package, owner-consistency and markdown validators.
- [ ] Inspect the full diff and ensure no status says provider or release qualified.

## Sequencing note

T32 is a bounded CODE/SERVED implementation slice. It must not be marked as satisfying T09's packaged native-provider qualification acceptance. Cross-owner review and platform USER evidence remain separate gates.
