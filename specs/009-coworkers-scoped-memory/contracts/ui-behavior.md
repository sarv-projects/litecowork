# UI behavior - Coworkers and Scoped Memory

**Feature:** 009 | **Status:** DRAFT | **Source cross-check:** 2026-11-06

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Target user journey

1. Ordinary New Chat remains available without a Coworker and creates a normal Conversation.
2. Quick Create creates a Coworker and opens its first Coworker-owned Conversation. The Coworker ID/revision is immutable Conversation origin; it is not merely a Task default.
3. A Coworker may have multiple Conversations. Opening the Coworker restores its most-recent Conversation; New chat starts a clean transcript while preserving only permitted Coworker context.
4. The Coworker Memory & Knowledge view lists exact Coworker-owned ContextDocuments/MemoryRecords with source, revision, scope, sensitivity, status, expiry and deletion/revocation outcome. Add Memory is owner-authored context and must not forge a LearningCandidate.
5. Coworker-specific retrieval shows exactly what was actually attached/resolved, with source/revision/freshness; it must not silently inherit Workspace-only content beyond explicit policy.
6. Revocation blocks future context reads and assembly; purge reports pending/complete from owner receipts. It does not claim to recall content already sent to a native AgentSession.
7. No Memory or Learning control creates a Grant, Connection, package assignment, Automation or schedule.

## Current UI evidence

- Home `HomeCoworkerOnboarding.tsx` offers optional creation of a general Assistant and Workspace primary selection. It does not open a Coworker chat. Copy states primary only prefills future Task settings and does not start an agent.
- CoworkerSettings.tsx renders a cursor-paginated roster, detail view, create/edit revision form, Workspace primary set/clear, pause/resume/archive actions, presence projection, focus restoration, Escape/dirty-draft handling and version/idempotency-aware writes. It is a roster/settings page, not the target chat-first Coworker experience.
- Its editor persists name/role, default lead binding, delegation strategy, enabled worker-profile IDs, five interaction defaults, lead-failover mode/triggers/fallback order/change limit, context kinds/item limit/Task-summary retention and notification preferences. Avatar selection and delegation-budget editing are not exposed; delegation_policy_source is not in the current mutation schema.
- These fields are not all enforcement: the default lead is consumed by Task creation; lead-failover is copied into TaskSpec but has no production fallback dispatcher; interaction policy has no Trust consumer; notification policy has no Coworker delivery consumer; delegation strategy has no worker selector; context policy has no retrieval consumer. The enabled worker-profile allowlist and budget are read by a SQLite child-Attempt admission primitive, but no production daemon/Operator caller invokes child-Attempt admission.
- The Coworker page's PAUSED copy says new proactive/scheduled work stops, but a PAUSED transition can occur while enabled Automations remain and the TriggerHost does not re-check Coworker status at occurrence settlement. Automation enable checks Coworker active only when enabling; Task creation storage rejects ARCHIVED, not PAUSED. The Task composer nevertheless rejects a PAUSED Coworker for owner Task creation, while the canonical target allows an explicit owner Task to name a PAUSED Coworker. Treat pause as partially enforced and the UI copy/Task composer as misaligned.
- The context editor currently lists PERSONAL_PROFILE, COWORKER_NOTES, WORKSPACE_NOTES and GOAL_NOTES. The two Coworker/Goal values are not accepted by current ContextDocument owner validation. The editor also persists policy fields that retrieval does not currently enforce.
- Its helper copy says memory proposals require review and automatic quiet learning is inactive. No Coworker-private memory proposal UI, Memory browser or Learning Inbox is present; that sentence must not be interpreted as a working review flow.
- Presence errors are rendered as unavailable; do not replace the 503 with fabricated AVAILABLE/WORKING/NEEDS_YOU data.
- The Task composer can select a Coworker and pin Task origin Coworker ID/revision. It does not create a Coworker-owned Conversation.

## Commands and state requirements

The UI renders owner state and sends only commands supported by the canonical API. A target control must remain disabled or explicitly unavailable until a served handler exists. Do not submit COWORKER_NOTES/GOAL_NOTES as though they are currently valid ContextDocument owners.

For every future create/revise/status/revoke/purge action, show:
- Workspace/Coworker identity and exact owner scope;
- current version and stale-version conflict;
- pending request identity and replay/reconciliation state;
- success only after committed receipt or verified owner observation;
- unavailable, forbidden, conflict, storage failure and ambiguous result states;
- focus restoration and keyboard semantics for dialogs/drawers.

## Acceptance states

Empty, loading, denied, stale version, offline, presence unavailable, invalid owner kind, not implemented, deletion pending, deleted, revoked, restore pending, and unknown recovery outcomes must be visibly distinct. Cross-Coworker denial requires a real Coworker-owned data path and cannot be asserted from current Workspace-scoped retrieval.
