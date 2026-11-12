# Data model - Desktop Shell IPC Settings and Runtime Health

**Feature:** 011 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification. | **Updated:** 2026-11-09

Identity and durable state remain in the canonical domain owners. This package coordinates the shell and does not create a shadow settings database for domain policy.

## Records and authority

- Runtime and RuntimeIncarnation: Runtime lifecycle owner; process incarnation, serving/readiness/drain and recovery are distinct.
- Authenticated IPC session: OS peer identity from LOCAL-OPERATOR-IPC; WebView never supplies Principal.
- Local UI preference: appearance, density, zoom, reduced motion, shortcuts, and selected view layout only; no grants, credentials or domain settings.
- Settings category/search selection: presentation state only.
- Per-control map entry: semantic map class and link to exact owner operation/requirement; it is not a persistent settings value.
- NotificationPreference: existing notification owner; channels/classes/quiet hours must be separate from Needs You requests and delivery receipts.
- OS power assertion observation: native OS provider result with process/runtime owner, mode, active state, observation timestamp and release result; not a React boolean.
- Usage projection: defined by V1-TELEMETRY-USAGE-RESILIENCE-LLD; immutable inference leaves and separate quota snapshots remain Observability-owned, not Settings-owned.

## Relations and scope

Workspace is the security boundary for Workspace-owned settings and Usage. Runtime installation/auth observations are Runtime-local and may not be replicated as Workspace settings. Global display preferences are device/window local. Agent defaults are Workspace/AgentBinding scoped and affect future admissions. Coworker defaults are Coworker revision scoped. OS power assertions are host-process scoped. Settings navigation may carry a selected Workspace/Runtime hint but never an authenticated actor identity.

## Persistence and transactions

Local display preferences are loaded/persisted by App.tsx through settings/local-preferences.ts and applied to document.documentElement. Mounted SettingsView edits the App-owned preference state; theme, density, text scale and reduced motion survive reload when localStorage is available. Storage failures use safe defaults and do not imply domain persistence. Domain settings use the owning service's existing versioned/idempotent transaction and receipt. OS operations happen outside SQLite and require a verified provider response plus release/cleanup behavior. Usage filters are local read selections; the Usage ledger/projections are never modified by dashboard navigation, filtering or refresh.

## Events, privacy and retention

Use existing event names in docs/EVENTS.md. New durable domain events require same-change EVENTS.md, JSON Schema, OpenAPI/Operator and migration. Never emit events for hover, filter changes or local preview. Do not log notification payloads, secret bytes, prompts, native transcripts, raw CLI arguments or private paths. OS assertion observations contain only the minimum process/mode/status metadata. Usage local readers extract counters/model/response IDs/timestamps only and never ingest prompt/completion/reasoning content.

## Revision 1.0 gap records

- StopPreview: a computed read over owner records (turns, Attempts, Effects, leases, provider references, cleanup); not persisted.
- Runtime stop request: idempotency receipt and incarnation transition rows already used by `persist_incarnation_transition` in `apps/litecoworkd/src/runtime.rs`.
- PowerAssertion observation: process-scoped, not replicated, cleared at restart.
- Workspace name (existing `workspaces.name`): owned by the Workspace aggregate; rename is a versioned mutation with event and receipt.
- Local preferences: validated device-local localStorage snapshot, never synced, never a grant.

## Migration

No migration is needed for revision 1.0: rename reuses `workspaces.name` and `version`; stop, keep-awake and preferences add no SQLite table. Feature 011 must not create shadow copies of Workspace, AgentBinding, Coworker, Trust, Runtime or Usage owner records.
