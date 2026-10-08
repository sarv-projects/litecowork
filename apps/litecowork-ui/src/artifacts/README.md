# Artifact read foundation

This slice reads committed Artifact records through SQLite, the authenticated local
Operator IPC server, and the Tauri native bridge. It contains no sample Artifacts and
does not turn imported Resources into Artifacts. Until E04-S05 implements publication,
an installation without committed Artifacts returns an empty catalog.

## Desktop integration

`App.tsx` memoizes the API for the selected Workspace. `ArtifactLibrary` is mounted
in the Library route, and Task details lazily load committed outputs when their
collapsible Outputs section is opened:

```tsx
const artifactApi = useMemo(() => desktopArtifactApi(workspaceId), [workspaceId]);
<ArtifactLibrary api={artifactApi} workspaceId={workspaceId} />
<TaskArtifactOutputs api={artifactApi} workspaceId={workspaceId} taskId={taskId} operatorReady={operatorReady} />
// For one Artifact ID obtained from committed Task or Library state:
<ArtifactWorkbench api={artifactApi} workspaceId={workspaceId} artifactId={artifactId} />
```

Imports come from `./artifacts/ArtifactLibrary`, `./artifacts/TaskArtifactOutputs`,
`./artifacts/ArtifactWorkbench`, and `./artifacts/desktop-artifact-api`. The optional
`onOpenSource` callback receives the
exact historical pinned source revision; its host must recheck current authorization.
Without that callback, references are rendered as escaped historical text.

## Supported paths

- `GET /v1/artifacts`: Workspace-scoped keyset pagination and Library status filter.
- `GET /v1/library`: only SAVED Artifacts.
- `GET /v1/tasks/{taskId}/artifacts`: committed Task outputs, capped at 200 records;
  oversized collections fail explicitly because this existing route has no page contract.
- `GET /v1/artifacts/{artifactId}` and `/versions/{version}`: committed metadata,
  including archived Artifacts and exact ResourceRevision identity.
- `GET /v1/artifacts/{artifactId}/versions/{version}/content`: exact managed bytes,
  authenticated Workspace owner access, no-store, size and digest verification, and
  attachment delivery using the stored media type. Local IPC transfers cap at 10 MiB.

The view supports direct selection of an exact version number, history navigation, and
an on-demand recent-history panel that reads at most ten exact immutable version records
from the loaded Artifact head. Older versions remain addressable by number; the panel is
not a paginated version-list API or live stream. It provides escaped UTF-8 text preview up to 1 MiB, copying that selected preview, side-by-side
comparison with the preceding supported text version, authorized download, and provenance
inspection including the stored media type, size, and content digest where available. It
never embeds HTML, SVG, provider UI, or external URLs. Evidence references are disclosed
without claiming their assurance level has been checked. Refresh re-reads authority and
committed metadata; it does not claim a live event projection.

For a historical managed `text/plain` version up to 1 MiB, the owner can confirm a
restore action that copies the selected verified text into a draft based on the latest
editable head. The owner must separately publish; that appends an immutable version and
uses the normal stale-head conflict path. The current API records the result as a user text
edit but does not persist a separate restored-from link. Linked content and other media
types remain unavailable to this action.

## Remaining boundaries

The current Operator supports only the narrow managed `text/plain` append used by the
desktop edit and restore-draft paths. General Artifact creation/publication, richer editor
types, a dedicated restored-from provenance relation, and other edits are not available.
`ArtifactReadStore` deliberately has no writes; the existing text append uses the separate
version-write port with atomic Artifact/Resource heads, provenance dependency edges,
domain events, aggregate state, authorization, expected versions, and idempotency.
Library promote/archive endpoints are contracted but remain unmounted until their atomic
state/event/idempotency implementation exists. The generic wire client describes those
commands; the desktop bridge rejects those mutations explicitly.

External content remains metadata-only: download returns `PROVIDER_UNAVAILABLE` until
qualified provider resolution checks current source permission and exact revision.
Office/PDF/image/table renderers and provider editing are not qualified by this slice.
Text comparison does not claim a structured document or spreadsheet diff.

## Verification deferred

New SQLite tests in `crates/storage-sqlite/src/artifact_tests.rs` seed committed records
under the real schema, then exercise exact historical reads after a new head/restart,
Workspace isolation, Library filtering, transfer limits, missing versions and missing blobs.
They do not qualify publication. No tests, builds, formatters, or validators were run
for this delegated change. The owner must run compilation and the new read tests,
then the real IPC/desktop acceptance flow, before claiming runtime correctness.
