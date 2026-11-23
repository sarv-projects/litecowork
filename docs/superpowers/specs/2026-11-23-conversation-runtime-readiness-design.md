# Conversation Runtime Readiness Gate

**Status:** USER-DELEGATED implementation direction, recorded as A-119. Feature 001 revision 1.2 remains IN_REVIEW. This design does not qualify a provider or packaged release.

## Intent

Prevent new Conversation work from being admitted while the selected local Runtime is recovering, degraded, draining, stopped, or no longer the current incarnation. Keep exact request replay available so a lost acknowledgement can still recover an already committed receipt. This is an operational readiness gate and remains separate from Feature 001 G-09/AC-016's exact OS/build/harness/version/auth qualification gate.

## Goals and constraints

- Keep ordinary Conversation independent from Task and Coworker flows.
- Require both `Runtime.availability=ONLINE` and the exact current `RuntimeIncarnation.recovery_state=READY` for a **new** first send, existing-Conversation send, or Retry.
- Check those facts in the same SQLite IMMEDIATE transaction that would create the admission or retry intent, after checking an exact idempotency replay and before any write.
- Bind the check to the daemon's own `runtime_id` and `runtime_incarnation_id`; never accept either identity from a desktop request body.
- Return the existing typed `503 RUNTIME_UNAVAILABLE` response with no partial records when the fence is absent, stale, superseded, or not ready.
- Make the composer and Retry control reflect the identity-matched lifecycle snapshot. Keep Stop available for an admitted running turn and keep exact saved-send replay available while the Runtime recovers.
- Preserve all current Conversation, attachment, native-agent and V1 capability scope.

## Approaches considered

1. **UI-only lifecycle check.** Simple, but a stale UI can admit a turn and direct authenticated requests bypass it. Rejected as the authority boundary.
2. **Read Runtime status before the transaction.** Better feedback, but lifecycle could change between the read and turn write. Rejected as the correctness boundary.
3. **UI preflight plus transaction-bound Runtime-incarnation fence.** The UI communicates current readiness; SQLite serializes the final check with the admission/retry write, and exact replay is handled first. Selected because it closes both the user-facing and race/bypass gaps while reusing the existing Runtime lifecycle records and error code.

## HLD and ownership

- **Desktop UI:** receives only the current identity-matched lifecycle projection; uses it to disable fresh Send/Retry and explain the repair path. It does not grant authority.
- **Authenticated Conversation Operator:** supplies its process-owned Runtime/incarnation IDs to Storage; it does not accept caller-selected Runtime identity.
- **SQLite Conversation storage:** owns the final check inside the IMMEDIATE transaction for first-send, existing-turn admission and Retry. It never calls external processes in the transaction.
- **Runtime lifecycle owner:** continues to own readiness state and recovery blockers. Conversation does not create a second readiness state machine.
- **Native harness:** continues to own its authentication, models, private transcript and tools. Runtime readiness does not imply harness authentication or route qualification.

## LLD, transaction and replay order

For each new dispatching operation:

1. Authenticate the local Operator principal and resolve the Workspace/Conversation scope as today.
2. Verify the exact operation/request digest and return an exact committed receipt if present.
3. In the same IMMEDIATE SQLite transaction, require the supplied Runtime identity to equal `runtimes.current_incarnation_id`, `runtimes.availability='ONLINE'`, and that row's `runtime_incarnations.recovery_state='READY'`.
4. Only then run existing Workspace, binding, single-flight, version, resource and event validation and write the USER message/turn or Retry transition/dispatch intent/receipt.
5. On a failed fence, return `RUNTIME_UNAVAILABLE`; rollback leaves message, turn, event, intent, aggregate snapshot and idempotency receipt unchanged.

Exact New Chat and existing-Conversation receipt replays bypass the new-work readiness check and do not dispatch. Retry checks the existing command replay first, then performs its readiness check in the write transaction before creating the next ordinal and intent. Stop, cancellation, settlement, read-only projection and recovery operations are not blocked by this gate.

The internal storage fence carries only `{ runtime_id, runtime_incarnation_id }`; it is command metadata and is not persisted, projected, logged, replicated, or included in caller idempotency digests. No migration, event, public request field, or competing Runtime state machine is introduced.

## UI states and errors

- `CHECKING`: the current lifecycle projection is loading; new Send and Retry are disabled with a checking message.
- `READY`: exact current identity, Operator ready, `ONLINE`, and `READY`; ordinary Send and Retry follow their existing eligibility checks.
- `UNAVAILABLE`: no/mismatched lifecycle identity, Operator unavailable, `DEGRADED`, or any other availability/recovery state; fresh Send and Retry are disabled with an actionable Runtime refresh message.
- An exact pending saved-send retry remains actionable whenever the authenticated local Operator responds. The daemon returns its prior receipt before checking current readiness; if it was not committed, the new admission is refused and no turn is written.
- A 503 racing with a previously enabled control refreshes the durable conversation projection and keeps the exact request identity for safe reconciliation.

## Trust and failure boundaries

The UI projection can be old or manipulated, so it is never authorization. The transaction is the final guard. Runtime identity mismatch, missing Runtime rows, unknown state, SQL read failure, stale incarnation or state other than the exact allowed pair all fail closed. Database busy/error behavior remains the existing typed storage/Operator failure behavior; no readiness success is inferred. The gate does not prove a native process, valid provider credentials, provider input support, screen capture, tool mediation, or OS/build qualification.

## Acceptance

- First-send, existing send and Retry admit only when the exact current Runtime is ONLINE/READY.
- Missing, stale, superseded, OFFLINE, DEGRADED, RECOVERING, DRAINING and unknown lifecycle states produce no write and a typed refusal.
- Exact first-send, existing-send and Retry replays return their committed receipts even after the Runtime becomes unavailable, without duplicate messages, turns, events or intents.
- Stop and read paths remain available when readiness is closed.
- The UI disables fresh Send/Retry for missing/mismatched/non-ready lifecycle state and presents a clear reason; pending exact send recovery remains separate.
- Tests cover request replay, failed transactions, concurrency/version behavior, UI state and mounted route mapping.
- T09's real provider, packaged desktop, exact OS/build/auth tuple and USER qualification remain open until their evidence exists.

## Implementation sequence

1. Add this delegated direction and T32 to Feature 001; update owner prose and the cross-owner alignment register.
2. Add failing SQLite tests for readiness denial and exact replay ordering; then implement a required Runtime fence on production admission/retry paths.
3. Add failing UI state tests; then wire identity-matched Runtime lifecycle readiness into Send/Retry, preserving Stop and exact pending-send recovery.
4. Update API/error prose, UI ledgers, WIRING, master plan, implementation status and CURRENT-RUN.
5. Run focused Rust/UI tests, API/package validators and review the full diff. Report CODE/SERVED only; do not close T09 or claim qualification.
