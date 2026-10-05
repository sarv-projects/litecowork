# Workspace Resources and Deterministic Search

## Purpose and boundary

LiteCowork maintains a factual, permission-scoped index of resources that a user has
explicitly selected or connected. This is the **World Index**: a catalog of observed
resources, locations, revisions, relationships, and freshness. It is not a cognitive
World Model, personal-memory engine, semantic reasoning system, or whole-machine monitor.

The index lets placement and agents answer deterministic questions without asking a model
to rediscover local facts or spending tokens. Web research, embeddings, semantic RAG,
OCR-heavy interpretation, and domain-specific search remain capabilities. `DATA-MODEL.md`
owns the canonical record fields; `API.md`, `STATE-MACHINES.md`, `EVENTS.md`, and
`schemas/sqlite-v1.sql` carry their transport and persistence forms.

## Identity and location

Resource identity is independent of where bytes can be reached. A resource may have
several locations, including a local file, a replicated cloud object, and a connected
provider object. Moving or replicating it does not create a new Resource.

Canonical fields for Resource, ResourceRevision, ResourceLocation, FileIdentity,
WorkspaceRoot, ResourceEdge, and InvalidationRecord are in
[`DATA-MODEL.md`](DATA-MODEL.md). Freshness is a derived projection for a requested
revision and candidate location, not one global Resource value: one location may be
current while another is stale or offline. Location freshness is `CURRENT`, `STALE`,
`UNKNOWN`, or `UNAVAILABLE`. Logical Resource freshness is `CONFLICTED` when the revision
graph has multiple heads; that state is distinct from any one location's freshness.
Timestamped location observations are the facts. `UNAVAILABLE` is a location condition,
not proof the logical Resource does not exist.

`ResourceRef` identifies a Resource and may pin an immutable revision, as specified in
`DATA-MODEL.md` and `SCHEMAS.md`. Digests belong to ResourceRevision observations or to a
`ResourceInput` that records bytes actually consumed. Its display URI is
`resource://<workspace-id>/<resource-id>@<revision-id?>`. Runtime IDs,
paths, connector handles, browser tabs, and secret-store handles belong to a Runtime-local
`ResourceLocationBinding` or the owning provider; they are never part of logical Resource
identity or replicated location metadata. Durable `ResourceLocation` contains a stable,
non-secret `locator_ref_id` used only to join to that local binding. The binding holds raw
locators, matches the current Runtime incarnation, and is omitted from events, replication,
Operator responses, and Workspace backups. Equal content digests do not by themselves
prove two resources are the same logical resource.

### Revision ancestry and concurrent edits

`ResourceRevision` is immutable and records `parent_revision_ids`. A revision parent must
belong to the same Resource, and the revision graph is acyclic. A normal local observation
extends the last revision observed at that same location; providers may report a different
parent set only when they supply verified revision ancestry. This avoids treating wall
clock order or Runtime identity as proof that one concurrent edit supersedes another.

The Resource has a `current_revision_id` only while the graph has exactly one head. No
observed revision means no head; two or more incomparable heads mean a revision conflict
and `current_revision_id` remains null. `CONFLICTED` is exposed in search/freshness
projections to distinguish this state from `UNKNOWN`. An unpinned ResourceRef cannot be
resolved while conflicted and returns `RESOURCE_CONFLICT`. A pinned ResourceRef can select
one branch. Resolution never drops sibling heads. A merge must create a new revision whose
parent set includes every head it actually merges; merely selecting a branch is not a
merge.

Only verified `STRONG` or `PROVIDER_SCOPED` provider identity tuples receive an
`identity_digest` eligible for Workspace-scoped deduplication. `WEAK` observations have no
cross-location deduplication digest and remain separate Resources until stronger evidence
establishes their identity. Content equality, path, size, and modification time alone do
not establish identity.

Artifacts are Resources of kind `ARTIFACT`, created and versioned by ArtifactStore rather
than watched by WorldIndexer. Each ArtifactVersion maps to one ResourceRevision; its stable
ResourceRef can be selected as an input by a later Task, and pinning the revision selects
that exact immutable ArtifactVersion. ArtifactStore is a registered ResourceLocationProvider
for resolving managed blobs or following a pinned external ArtifactContent reference.

Location availability and revision observations are time-scoped facts. An offline or
unobserved location never proves that the Resource is absent. A reference pinned to a
revision remains a reference to that revision even when a newer revision becomes current.

## File identity

For local filesystems, the indexer uses the strongest stable identity exposed by the
platform, such as filesystem/volume identity plus file ID/inode and generation where
available, in `FileIdentityBinding`. The durable `FileIdentity` fields are keyed
pseudonyms computed from the raw tuple using a stable Runtime identity key held by the OS
keystore; raw IDs stay local. Normalized relative path is a locator/display aid, not
identity, and is held in `ResourceLocationBinding`. If the platform cannot supply stable
identity, the indexer uses a provider-scoped fallback and marks identity confidence and
observation freshness accordingly; path plus size/mtime alone is not treated as proof that
content is unchanged. If the local identity key is lost, re-indexing lowers confidence and
does not merge Resources based only on equal content.

All indexing and access is rooted in explicit `WorkspaceRoot` grants. No home-directory,
drive-wide, or whole-machine scan is implied. Revoking a root stops future observation,
search, exposure, and replication under that root. It does not erase Task history or
already-created Artifacts; retained derived content follows Workspace retention policy.

## Workspace roots

Adding a folder as a message attachment is a one-time input reference. Adding it as a
WorkspaceRoot is a persistent user-authorized relationship that enables ongoing
observation and deterministic search under the selected policy. A WorkspaceRoot becomes
`UNAVAILABLE` when its only observing Runtime is offline and returns to `ACTIVE` only
after the root identity is revalidated. It never falls back to another path or broader
root. `PAUSED` is an explicit user/service lifecycle state, not a synonym for offline
availability. Root observation does not grant a capability, secret, write permission, or
cloud replication by itself; these are independent decisions. `SELECTED_FOLDERS` stores
stable WorkspaceRoot IDs, not revision-pinned ResourceRefs, so newly observed content
under an authorized selected root remains eligible. Workspace and root replication
policies intersect; a root-level setting cannot broaden the Workspace's scope.

## Relationships and dependency invalidation

`ResourceEdge` records observed structural or provenance relationships using the
canonical fields in `DATA-MODEL.md`; it is not used as an implicit dependency index.

ArtifactVersion and VerificationRun input references create immutable `DependencyEdge`
rows to the exact consumed Resource revisions. These rows are a normalized, rebuildable
reverse index over authoritative input refs in their aggregate records and events. When a
new ResourceRevision is observed, `DependencyService` appends an `InvalidationRecord` for
each dependent edge that no longer points at the observed revision. The record links the
edge to the newly observed revision and changes the dependent's freshness projection to
`STALE`; it never edits immutable ArtifactVersion, VerificationRun, or Evidence records.
Re-verification creates a new VerificationRun bound to new `ResourceInput` values.

## Indexing and freshness

`WorldIndexer` watches only active roots and provider locations authorized for the
Workspace. Watcher notifications are hints, not a complete change log. On startup,
reconnect, overflow, permission error, or detected watcher gap, it marks affected
locations `UNKNOWN` or `STALE` and schedules a bounded reconciliation scan. It records
observed revision/digest and timestamp after a successful read. A freshness claim is
never stronger than the most recent successful observation.

Index projections are rebuildable and are not replicated as authoritative event history.
The durable Resource, ResourceRevision, ResourceLocation, root, and edge changes are
evented. Local indexes may use SQLite FTS or an equivalent deterministic inverted index.
Extracted text is bounded by media type, size, and Workspace policy; index data is
workspace-scoped and deleted or rebuilt according to root revocation and retention rules.

## Deterministic resource search

LiteCowork provides first-party `LocalResourceSearch` for selected local roots and
available connected/replicated Resources. It consumes no model tokens. Search supports
name/path tokens, type/media type, timestamps, revision/freshness, and bounded extracted
text matches. Results include a stable ResourceRef, matching location(s), freshness,
availability, and why each result matched. It does not claim semantic relevance.

```text
interface LocalResourceSearch {
  search(ResourceSearchRequest) -> Page<ResourceSearchResult>
  suggest(PartialResourceQuery) -> ResourceSuggestion[]
}
```

Search always applies Workspace ownership, active-root scope, Resource sensitivity, and
caller grants before returning names or snippets. Snippets are bounded and are not
automatically attached to an AgentSession. The user/agent explicitly selects which
ResourceRefs enter a TaskPacket or ContextAttachment. Semantic RAG, web search, and
cross-document reasoning are external capabilities.

## Local application availability

Installed applications are not Workspace Resources. Their normalized availability is
advertised as incarnation-scoped `APPLICATION` RuntimeOffers, with stable application
identity, supported Environment classes/actions, and `OfferReadiness`. Inventory contains
only the minimum app identity/version/launchability facts needed for placement; it does not
index user documents, window titles, or general activity. Application process/window
identity is observed only to attach to an explicitly selected application or with the
user-authorized Quick Entry context. Raw process handles and window IDs remain in a
Runtime-incarnation-local `ApplicationInstanceBinding`. That binding records whether
LiteCowork launched the instance. Only an owned launched instance can be closed by policy;
a pre-existing user instance is detached, never terminated. Applications do not become
warm merely because they are inventoried.

## Resource resolution and placement

```text
interface ResourceResolver {
  resolve(ResourceRef, ResolutionPolicy) -> ResolvedResource
  locations(ResourceId) -> ResourceLocation[]
  list_revisions(ResourceId) -> Page<ResourceRevisionView>
  expose(ResourceRef, EnvironmentId, AccessMode) -> ExposedResource
}
```

```text
ResolvedResource {
  resource_ref: PinnedResourceRef
  location_id: ResourceLocationId
  runtime_id: RuntimeId?
  environment_id: EnvironmentId?
  provider_revision: string?
  location_freshness: ResourceLocationFreshness
  resolved_at: Timestamp
}
```

Resolution verifies Workspace scope, revision, freshness, availability, write authority,
Runtime/Environment compatibility, and replication policy. An unpinned input resolves
only when the Resource has one unconflicted current head; the result pins that revision.
The resolver may choose any eligible location that represents the requested logical
Resource revision. It must not silently substitute a newer revision when the reference is
pinned. Placement uses this inventory to identify remote-eligible work and blockers such
as “file is only available on the laptop” or “selected spreadsheet application is local.”
When the consumer records a `ResourceInput.observed_digest`, it is the hash of the bytes
actually delivered. If it and the pinned ResourceRevision's `content_digest` are both
known, they must match; a mismatch returns `INTEGRITY_FAILURE` and cannot support
Artifact publication or passing verification.

## Filesystem race defense

Path validation alone is insufficient. Providers resolve selected roots using
platform-native directory handles and no-follow/beneath-root traversal where available.
Before a mutation they revalidate the stable file identity and expected revision/digest
through the held handle, then write to a temporary sibling and atomically replace/rename
only if the target identity still matches. Symlink targets are rejected unless explicitly
authorized as a separate Resource. Where a platform cannot provide these guarantees, the
provider must use a private copy/snapshot or downgrade the operation to read-only and
report the limitation.

## Events and API

The event registry includes Resource creation/revision/location/freshness changes,
WorkspaceRoot lifecycle, ResourceEdge creation, ArtifactVersion/VerificationRun input
refs, and InvalidationRecord creation. DependencyEdge rows are rebuilt from those
authoritative aggregate events. The Operator API exposes root management, bounded
deterministic search, Resource metadata/location/revision projections, dependency views,
and paginated invalidation history. Raw absolute paths, provider object locators, browser
handles, and credential data are never included in replicated events or public responses.
`locator_ref_id` is a stable non-secret resolver key; its private locator is resolved only
inside the Runtime/provider boundary that owns a current `ResourceLocationBinding`.

## Deferred boundaries

This document does not define a semantic memory engine, embeddings service, general
machine activity monitor, or LiteSPM package contract. A separately isolated opt-in
Machine Observer may publish specific observations only after explicit user authorization;
it is not required for World Index or Task placement.
