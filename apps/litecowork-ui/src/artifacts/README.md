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

Native Save As re-reads the exact ArtifactVersion and its ResourceRevision before opening
the OS destination picker. After a confirmed destination, it fetches that exact authorized
version, independently checks the returned media type, length, and SHA-256 digest, then
uses a same-directory staged write and rename. A cancelled picker fetches no bytes or
touches the destination. Only `SAVED` or `CANCELLED` crosses the Tauri WebView boundary,
and the Workbench reports either outcome through a status message.

The view supports direct selection of an exact version number, history navigation, and
an on-demand recent-history panel that reads at most ten exact immutable version records
from the loaded Artifact head. Older versions remain addressable by number; the panel is
not a paginated version-list API or live stream. It provides escaped UTF-8 text preview up to 1 MiB, copying that selected preview, side-by-side
comparison with another explicitly selected committed text version, authorized download, and provenance
inspection including the stored media type, size, and content digest where available. It
never embeds HTML, SVG, provider UI, or external URLs. Evidence references are disclosed
without claiming their assurance level has been checked. Refresh re-reads authority and
committed metadata; it does not claim a live event projection.

The **Compare with version** field accepts any other version from 1 through the loaded
Artifact head, including versions outside the recent-history panel. Both sides use the
existing exact metadata/content routes and each retains its own ResourceRevision and
renderer/media type. Inputs outside that committed snapshot and same-version comparison
are rejected locally. Unsupported, oversized, unavailable, denied, corrupt or invalid-UTF-8
comparison content yields an explicit error while the selected authorized preview remains
readable. The comparison can be retried or stopped; changing the selected version clears
the comparison. This action publishes nothing and changes no edit/restore/idempotency state.

For text content, comparison can display a bounded literal line diff with line numbers and
added/removed counts, or retain the side-by-side renderer. The diff is limited to 400 lines
per side, 160,000 line-pairs, and 16,384 characters per line; inputs beyond those bounds
remain available in side-by-side view. This is not a semantic diff and treats every line as
untrusted literal text.

Deferred owner acceptance for this improvement: compare nonadjacent text versions in
both directions, compare an old version beyond the recent ten records, reject same/out-of-
range/fractional versions, and attempt a binary, oversized or missing target. Confirm that
each pane labels its exact version/revision, the selected preview survives target failure,
retry works after a transient failure, and stopping comparison returns to one pane.
Exercise keyboard entry, editing-disabled controls and narrow-window wrapping. Commands,
system/provider versions and observed results remain to be recorded; no acceptance pass
is claimed by these instructions.

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
Library promotion/archive is mounted through `ArtifactLibraryWriteStore`, the atomic
SQLite state/event/receipt transaction, Operator IPC, and the native bridge. Owner-confirmed
Workbench actions pin the current Artifact aggregate version and retain an unconfirmed
request ID for retry. The response is checked against the exact original identity,
metadata, status and aggregate increment; a follow-up read handles replay receipts older
than the current head. The standalone Library updates its filter after that committed
response. Managed and linked content versions/Resource heads remain unchanged; the linked
provider is neither fetched nor mutated. Archive is terminal; stale expected version still
conflicts on an already-archived Artifact. Library changes are disabled during text edits.
Pending request IDs currently live only in the mounted Workbench; durable cross-restart
desktop retry recovery and event-driven Library synchronization are not implemented.

Deferred owner acceptance: promote a real TRANSIENT output, restart and find it in Saved;
archive it and find it in Archived, then read/save its historical bytes. Use concurrent
clients to force stale promotion/archive, drop a response and retry the original key, reuse
the key with a different command, and archive an already-archived Artifact with current
versus stale If-Match. Confirm linked promotion/archive never accesses or deletes provider
content. Check a denied owner and archived Workspace, and verify the status/version,
transition event, state snapshot and receipt agree in storage. No system/user pass is claimed.

External content remains metadata-only: download returns `PROVIDER_UNAVAILABLE` until
qualified provider resolution checks current source permission and exact revision.
Office/PDF/image/table renderers and provider editing are not qualified by this slice.
Text comparison does not claim a structured document or spreadsheet diff.

## Focused client receipt verification — 2026-10-09

Text publication now requires the receipt's managed content digest and exact UTF-8 byte
count to match the submitted draft, for both fresh and replayed receipts. Head and content
inputs are captured before awaiting the transport. A mismatched receipt remains unconfirmed:
the Workbench's existing error path preserves the draft and original unchanged retry ID.
This changes no route, store transaction, immutable version, or request fingerprint.

Run from `apps/litecowork-ui`:

```sh
node --experimental-transform-types --test tests/artifact-publication.test.ts
```

On 2026-10-09 this focused command passed six tests: exact Unicode content for fresh and
replayed receipts, different same-size content rejection for both receipt kinds, UTF-16
character count rejection, and unchanged request/version/body retry after an unconfirmed
receipt. The Node type-transform feature emitted its experimental warning. These tests use
controlled transport responses to exercise the actual client adapter; they do not prove
SQLite, Operator IPC, native Save As, rendered UI, provider integration, or OS behavior.

## Remaining verification deferred

New SQLite tests in `crates/storage-sqlite/src/artifact_tests.rs` seed committed records
under the real schema, then exercise exact historical reads after a new head/restart,
Workspace isolation, Library filtering, transfer limits, missing versions and missing blobs.
They do not qualify publication. No tests, builds, formatters, or validators were run
for this delegated change. The owner must run compilation and the new read tests,
then the real IPC/desktop acceptance flow, before claiming runtime correctness.
