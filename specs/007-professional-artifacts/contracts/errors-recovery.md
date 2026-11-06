# Errors and recovery - Professional Artifacts One Format

## Target recovery rules

- Artifact create first verifies the exact source bytes and digest outside SQLite. Initial Artifact, backing ARTIFACT Resource, ResourceRevision, version 1, provenance/dependency edges, aggregate snapshots, events and receipt are committed together. If any transaction step fails, no partial owner record is visible; unreferenced staged blobs are eligible for cleanup.
- New version publication is compare-and-swap against both Artifact and backing Resource heads plus exact parent ResourceRevision. A stale head does not overwrite or silently branch; it preserves the draft and requires an explicit rebase. Earlier versions are immutable.
- Idempotent replay of the same request identity/payload returns its original receipt, even after later version/archive operations where the owner contract permits replay. Same key with changed payload conflicts.
- Format adapter runs outside SQLite under bounded input/output/process limits. A failed parse/validation or unsupported format does not update the current Artifact pointer.
- Export pins exact ArtifactVersion/digest and destination class. Export failure leaves the version pointer unchanged. Uncertain external/provider export remains pending/ambiguous until the selected reconciliation contract determines whether the operation occurred; it is not automatically repeated.
- Native desktop Save As is a local OS file operation and remains separate from the durable export receipt. A canceled picker fetches no bytes or touches a destination. Failed staged write cleans its temporary file and does not claim SAVED.
- Restart reloads last committed Artifact/Resource state and immutable versions. An uncommitted edit remains a local draft only if the UI has retained it; the server does not infer a save from editor state.
- Archived Artifacts and Workspaces are read-only for new content publication. Historical authorized versions remain readable according to retention rules.

## Current implementation gaps

- The active store has read, Library status transition and bounded managed-text append ports; it has no production Artifact create port/Operator route and no export receipt store.
- The current append is only owner-authored UTF-8 `text/plain` ≤1 MiB to an existing managed Artifact; no general format editor, exporter, document/office round-trip or external-resource edit is connected.
- Native Save As is exact managed Blob Save As ≤10 MiB with digest/type/size checks and atomic local write, but produces no durable receipt or event. This is not a provider export.
- Task output Artifact creation remains blocked by Feature 005's missing Task planning/Attempt/Environment execution path.
- Office qualification probes do not install or execute an Office document editing engine. ZIP intake does not publish extracted members or transform Office files.

## Canonical error contract

Use canonical errors in `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`: `STALE_VERSION`, `IDEMPOTENCY_CONFLICT`, `ARTIFACT_ARCHIVED`, `WORKSPACE_ARCHIVED`, `PROVIDER_UNAVAILABLE`, `INVALID_ARGUMENT`, `INTEGRITY_FAILURE`, `INVALID_ARGUMENT`, `UNSUPPORTED_MEDIA_TYPE` where applicable. No feature-local errors are added.

## Qualification gate

Exercise initial create failure before/after blob stage, receipt replay/conflict, corrupt digest, stale Artifact and Resource heads, archive race, format parser/validator refusal, process timeout/cancellation, oversized output, export target failure/ambiguity/reconciliation, native picker cancel, atomic file save failure, crash/restart, exact version reopen, compare older than the recent ten and semantic fallback. No current professional format end-to-end USER flow was executed.
