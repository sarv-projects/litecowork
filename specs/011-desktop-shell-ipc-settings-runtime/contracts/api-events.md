# API and events - Desktop Shell IPC Settings and Runtime Health

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

IPC is a transport for docs/API.md operations. Readiness and drain follow RUNTIME-LIFECYCLE. Settings classes follow the ledger.

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. Current source serves Workspace rename through its versioned owning route and read-only previewLocalRuntimeStop. The stop request remains unmounted, and update/power observation integration remains incomplete. Existing Workspace rename event/schema ownership must be reused; any new event requires same-change owner/schema promotion. A stop preview is not proof of quiescence.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.
