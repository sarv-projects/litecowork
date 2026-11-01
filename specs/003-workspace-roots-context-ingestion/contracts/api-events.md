# API and events - Workspace Roots and Context Ingestion

Workspace root operations already described in docs/API.md and WORLD-RESOURCES. POST/PATCH workspace-roots that OpenAPI marks without a mounted handler stay target-only until this package is locked and the handler is served.

Event names must already exist in [docs/EVENTS.md](../../../docs/EVENTS.md) or be added there with the schema in the same reviewed change. This DRAFT does not declare new event types.

Error codes must already exist in [docs/SCHEMAS.md](../../../docs/SCHEMAS.md) or be added with `docs/schemas/error-codes.schema.json`.
