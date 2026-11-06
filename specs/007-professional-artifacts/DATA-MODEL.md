# Data model - Professional Artifacts One Format

**Feature:** 007 | **Status:** DRAFT

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Artifact identity.
- Immutable version and content digest.
- Provenance links.
- Export receipt.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

None in this revision. Blockers in SPEC section 5 must be closed before a migration is designed.

## Current implementation boundary (2026-11-06)

- `ArtifactRecord`, `ArtifactVersionRecord`, backing ARTIFACT Resource and ResourceRevision are canonical durable owner records. The active code reads these records and appends a new version to an already-existing Artifact; it has no production create port/Operator command.
- Initial create is a distinct target transaction: first committed managed content plus Artifact identity, backing Resource, ResourceRevision 1, ArtifactVersion 1, provenance/dependency edges, snapshots, creation events and principal-scoped receipt must agree atomically. `artifact.created.v1` is registered but not emitted by the current daemon.
- `MANAGED_BLOB` version append currently means manual UTF-8 `text/plain` edit only, capped at 1 MiB. `EXTERNAL_RESOURCE` is metadata/link-only and cannot be edited or downloaded by the current desktop path.
- Current preview media support is broader than edit: Markdown, CSV/TSV, JSON, XML and YAML can be safely previewed; only `text/plain` is accepted by the user text-version append.
- Native `Save As` reads one exact MANAGED_BLOB version up to 10 MiB and writes locally. It has no persisted ExportReceipt and does not alter Artifact/Resource version/event history.
- Durable `ExportReceipt` remains a target record, not present in the current data/store/API implementation. If introduced, it must bind immutable Artifact/version digest and export engine/version; local absolute destination paths do not belong in replicated owner records.
- Office readiness probes and UI accepted upload extensions do not imply Artifact parser/editor/export support.