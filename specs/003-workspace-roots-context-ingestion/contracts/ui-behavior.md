# UI behavior - Workspace Roots and Context Ingestion

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Flows and states

- Add root uses the OS picker then an explicit confirm.
- Paused, unavailable and revoked are different labels with different repairs.
- Context Used lists citations; search hits require an explicit attach.
- Add persistent folder and Scan folder are disabled before selection, with a stated reason, on a platform where they are not qualified (AC-015).
- The Feature 001 composer @-reference picker lists only Resources of the current Workspace under ACTIVE roots or otherwise admitted, pins exact revisions, and is a labelled keyboard-operable listbox (AC-017). The "Add context or file" button opens the same picker.

## Commands

The UI renders owner state and sends the Operator command named by the canonical API. It does not encode a second state machine. TARGET controls stay disabled until a served handler exists.

## Current implementation crosswalk (not the target)

- `LibraryPage.tsx` owns the Persistent folders area. It lists roots and exposes Add, pause/resume/revoke, manual Scan folder and paginated Browse observed files; `App.tsx` mounts the Library route but does not own these controls.
- Add opens the native folder picker and calls the private local-only root-selection route. On supported targets the folder selection immediately commits; there is no separate post-picker preview/confirm, and the UI hardcodes `METADATA` watch policy plus `NONE` replication policy. On Windows the daemon route returns 503; the current button is not proactively disabled and displays the failure only after selection.
- Scan is metadata-only and Linux-only. On Windows/macOS the first attempt returns `SCAN_UNSUPPORTED_PLATFORM`; the UI then displays the unsupported state and disables further scans. No file contents are read by root scanning, and the UI explicitly says watching/indexing are not active.
- The current Conversation attachment control is an **Add context or file** button, available only after a Conversation exists. It loads one `list_resources` page, filters to same-Workspace attachable text Resources, and sends pinned `resourceRefs`; it does not open when the owner types `@`, does not expose the accepted labelled listbox/arrow-key/count behavior, and cannot attach on first-message creation. These controls attach already-admitted Resources; they do not expose files found by metadata-only WorkspaceRoot scans.
- Browse observed files shows names, size and freshness only. An observed file may have no content revision. Resource upload/indexing is a separate flow.
- The current Resource index is limited to allowlisted UTF-8 text formats up to 1 MiB. PDF/Office/media/archive extraction is not present. Library mounts `ZipIntakeNotice`, which explicitly says ZIP extraction is unavailable and that archives remain opaque, unindexed and unattached. The source ZIP metadata-preview route is not called by the UI and its worker is development-feature-only, absent from release sidecars.
- There is no Context Used panel or production call to the grounded retrieval endpoint. The retrieval decoder helper is currently unused by the app; citations are not attached to Conversation turns.

## Validation

Empty, denied, stale version, offline and unqualified provider states are visible. Success requires a receipt or a verified observation.
