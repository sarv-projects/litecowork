# Errors and recovery - Conversation co-edit and take-control

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

## Target behavior

Take-control settles the turn `CANCELLED` only after the Feature 001 supervisor observes native stop/quiescence. If a completion or failure wins the race, that terminal outcome remains authoritative and take-control reports it instead of a fabricated cancellation. The stopped turn's committed content is preserved as owner-editable context, and the Conversation enters an explicit owner-in-control state recorded by an immutable `OWNER_TOOK_CONTROL` control event.

A side-chat `OWNER_SIDE_NOTE` is valid only while its correlated turn is non-terminal. It is committed as ordinary bounded history, displayed with its correlated turn id, and excluded from that turn's native prompt. It never admits a second concurrent active turn.

Stale Conversation/turn versions, idempotency-key reuse with a different payload, quiescence timeout, and Runtime restart use the canonical Feature 001 error and recovery contracts. This package introduces no duplicate error taxonomy.

## Typed errors (accepted target interface, not yet in source)

- `TakeControlError::TurnAlreadyTerminal` - the target turn has already settled; no control event or new turn is created.
- `TakeControlError::StaleTurnVersion` - the supplied turn version does not match; the owner must refresh.
- `TakeControlError::QuiescenceTimeout` - native stop was not observed within the finite deadline; the turn is not falsely settled `CANCELLED`.
- `TakeControlError::RacedCompletionWins` - a completion/failure won the race; the terminal outcome is authoritative.
- `SideNoteError::TurnTerminal` - the correlated turn is terminal; the note is rejected and the draft preserved.
- `SideNoteError::OverBudget` - the note exceeds the bounded size; it is refused, never silently truncated.
- `SideNoteError::CrossWorkspace` - the note or correlation crosses the Workspace boundary and is rejected.

## Crash and restart

Reload the last committed Conversation turn, control events and side-notes. Do not infer a stop from an intent: if quiescence was not observed and no terminal settlement committed, the turn is reconciled through the Feature 001 restart rules, not shown as taken over. After an `OWNER_TOOK_CONTROL` event, restart restores the owner-in-control state and the preserved stopped-turn content from committed records only.

## Current gaps

- Feature 001 has Stop/cancellation/quiescence CODE foundations; integrated provider quiescence and Feature 012 atomic owner-control settlement/recovery still need qualification.
- ConversationCoeditStore and migration v32 persist control events and side notes. The user-facing authoring/takeover service and route are not served.
- The ordinary context builder reads Conversation messages, not the separate side-note table. AC-007 still requires an end-to-end exclusion regression when the authoring service is wired.

## Qualification gate

Take-control quiescence, racing completion, terminal-turn rejection, idempotent replay, side-note exclusion from the prompt, single-flight, crash and restart must be exercised on each declared OS/auth route. A source-level unit test or mounted route does not qualify the co-edit journey.
