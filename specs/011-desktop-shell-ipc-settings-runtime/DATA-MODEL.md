# Data model - Desktop Shell IPC Settings and Runtime Health

**Feature:** 011 | **Status:** DRAFT | **Updated:** 2026-11-06

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

Target local display preference changes may commit to a desktop preference store without Operator mutation. No such durable preference store is used by the mounted SettingsView today: theme, density, text-scale and reduced-motion values exist only in component-local React state, are not applied globally and reset with component state. Domain settings use the owning service's existing versioned/idempotent transaction and receipt. OS operations happen outside SQLite and require a verified provider response plus release/cleanup behavior. Usage filters are local read selections; the Usage ledger/projections are never modified by dashboard navigation, filtering or refresh.

## Events, privacy and retention

Use existing event names in docs/EVENTS.md. New durable domain events require same-change EVENTS.md, JSON Schema, OpenAPI/Operator and migration. Never emit events for hover, filter changes or local preview. Do not log notification payloads, secret bytes, prompts, native transcripts, raw CLI arguments or private paths. OS assertion observations contain only the minimum process/mode/status metadata. Usage local readers extract counters/model/response IDs/timestamps only and never ingest prompt/completion/reasoning content.

## Migration

No migration is authorized in this revision. Feature 011 must not create shadow copies of Workspace, AgentBinding, Coworker, Trust, Runtime or Usage owner records.