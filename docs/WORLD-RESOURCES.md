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

The authenticated local content route accepts an exact `revision_id` and resolves bytes
from that immutable revision's digest-addressed managed local blob. It never treats the
pin as a current-head precondition and never substitutes a newer head. Storage verifies
Workspace/Resource/revision membership, ContextDocument ACTIVE status, local managed
provider availability, the caller's size bound, byte length, and digest before returning
content. Historical external-provider observations without a locally managed blob remain
metadata-only and return a typed unsupported-content error. A missing local blob is
unavailable, not permission to fetch from an external provider. ContextDocument
revocation/deletion continues to deny historical reads as well as current reads.

An authenticated owner may append a new revision to an existing managed Resource through
the revision-upload protocol in `API.md`. Admission requires `If-Match` for the exact
Resource version and a non-empty set of unique parent IDs equal to every current head. The
upload pins that version and parent set; commit rechecks both under the storage transaction,
verifies the full-content SHA-256, and preserves Resource identity and metadata. A concurrent
edit therefore returns `RESOURCE_CONFLICT`; clients must reload and deliberately create a
new upload with the current complete head set. This local upload operation does not merge
conflicted heads implicitly and is unavailable when the Resource lacks an available managed
local content location.

Revision-history pages return a bounded keyset page over immutable SQLite append order,
with the last revision ID as the cursor. Since a parent must already exist before a child
ancestry edge can be committed, append order is a stable parent-before-child traversal
without loading and sorting the full history in the daemon. The cursor is scoped to the
Workspace and Resource; appends after a page naturally appear after its cursor. The current
schema has no indexed append ordinal, so SQLite may scan matching revisions internally;
bounded database work needs an additive append-order column/index migration.

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
`UNAVAILABLE` when its observing Runtime cannot prove the saved root identity. Before
Operator IPC starts, Runtime startup reopens the saved private locator using no-follow
directory-handle traversal, compares the prior raw operating-system identity, and checks the
Resource's stable keyed identity digest and identity projection. Only an exact match binds
fresh private locator and raw-identity rows to the current Runtime incarnation; the root
never falls back to a new path or broader scope. A missing/mismatched binding, unsupported
platform, invalid locator, or changed/unopenable directory commits the root and location as
`UNAVAILABLE` and removes partial current-incarnation bindings. Root status, location
availability, safe events/snapshots, and private binding writes are one SQLite transaction.
Root-specific failures do not prevent unrelated Runtime service startup. Revoked roots are
excluded and cannot be reactivated. `PAUSED` is an explicit user/service lifecycle state,
not a synonym for offline availability. This startup implementation currently has Linux/macOS
source support only and is not OS-qualified; Windows and unsupported platforms fail closed.
Loss of the OS-principal/keyring identity currently blocks startup before this root
transaction, so that key-loss path does not yet durably mark roots unavailable. Root
The owner may pause an ACTIVE root without deleting its identity bindings or selected-folder
replication preference; every observer, search, exposure, and replication transfer must
still filter for ACTIVE status. Resuming a PAUSED root is a separate versioned user action
and requires the current Runtime incarnation to have the matching AVAILABLE location and
both validated private identity bindings. A stale/missing current-incarnation binding
returns a conflict and leaves the root PAUSED. UNAVAILABLE is recovered only by Runtime
identity revalidation, not by the resume button. Revocation remains terminal and deletes the
private bindings and selected-root replication relation. Root observation does not grant a
capability, secret, write permission, or cloud replication by
itself; these are independent decisions. `SELECTED_FOLDERS` stores
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

**Desktop local text index:** Current managed Resource revisions with an allowlisted plain
text media type/extension, at most 1 MiB, and at most 20,000 distinct normalized terms
of at most 128 Unicode characters each are indexed when imported or uploaded. Inputs
outside those parser bounds are left unindexed rather than represented by a partial
index. The
index is a rebuildable projection scoped by Workspace, Resource, and exact
ResourceRevision. Extracted UTF-8 text is stored only as an encrypted `RESOURCE_INDEX`
BlobStore object under a Workspace/purpose key. SQLite stores the encrypted blob digest,
source revision/digest, parser version, key version, and distinct HMAC-SHA256 term tokens;
it stores neither plaintext extracted text nor raw terms. Matching is deterministic,
case-normalized Unicode token equality with AND across query terms, not semantic
similarity. Results include the exact revision-pinned ResourceRef and a bounded excerpt
read back from the encrypted index object. `ResourceSearchResult` also carries that
revision's `source_content_digest` and `source_matches`: one first-occurrence record per
distinct query term, with zero-based half-open UTF-8 byte offsets into the original
ResourceRevision bytes. These spans are created only after digest verification and remain
bound to the result's pinned ResourceRef/digest; they must never be applied to a newer
Resource head or to the shortened display snippet. Non-indexed results carry an empty
`source_matches` array, and on-demand excerpts do not currently have exact span metadata.
The search boundary must recheck Workspace,
current revision, source digest, and ContextDocument active status after reading the
snapshot. An unavailable/lost index key disables indexed search and never falls back to
plaintext indexing.

The storage result computes these spans, but the daemon's ResourceSearchResult serializer
has not yet mapped them to the Operator API. Until that implementation step is complete,
the mounted route does not satisfy the documented `source_content_digest` and
`source_matches` response fields; no client may derive citation offsets from a snippet.

Index preparation happens before the Resource SQLite writer transaction; the index row
and keyed-term rows are committed with initial Resource creation/upload. An owner can
explicitly rebuild the local projection for the exact current managed Resource revision
from Library. The request pins both revision ID and content digest, verifies the current
head before and after bounded byte access, and atomically commits the encrypted projection
and a `request_dedup` receipt containing only the typed outcome. A retry with the same
principal/request ID and normalized pin replays the original result; a changed head returns
a conflict and is never indexed implicitly. Unsupported, oversized, invalid UTF-8,
control-character, or over-term-limit inputs return `NOT_INDEXABLE` with a reason and no
source content or terms. The no-index result removes any obsolete projection for that
exact revision. This owner action changes no Resource/Task history and emits no domain
event. An encryption or key-provider error is surfaced and cannot create a plaintext
index. New Resource revisions do not reuse older terms; they require an index built from
the verified bytes for that revision. The shared Resource content-read admission boundary
rejects non-`ACTIVE` ContextDocuments before BlobStore access. A read admitted while
`ACTIVE` may finish after a concurrent status change, but the rebuild's final transaction
rechecks status and the exact source pin and will not publish that prepared projection.
Old revision projections are excluded from current-resource search and are
removed with their ResourceRevision or through the registered derived-index purge target.
Root revocation removes the root from future observation/search; it does not silently
purge separately uploaded Resource content. ContextDocument revocation immediately
excludes its rows from indexed search. Physical deletion of the encrypted index snapshot
is not yet wired to the Resource/ContextDocument purge worker; until that integration
exists, the purge plan must account for `RESOURCE_INDEX` blobs and term rows before
claiming `DELETED`.

The earlier `ON_DEMAND_CONTENT` mode remains a bounded compatibility path, not the
persistent index. It scans at most 20 candidates, 1 MiB per Resource and 8 MiB total per
request. It requires a non-empty query and examines only allowlisted valid UTF-8 text.
Search cursors bind Workspace, query, mode, and filters. ZIP files remain opaque Resources
and are never indexed or extracted here. PDF/Office parsing, OCR, embeddings, semantic
retrieval, local-model RAG, persistent WorkspaceRoot crawling, and folder recursive
indexing are not implemented by this slice. They require isolated, qualified providers
and Core-mediated Resource revision, authorization, provenance, deletion, and freshness.
Search results are not implicit Task/Agent attachments; a user or authorized workflow
explicitly selects a pinned ResourceRef.

The local Operator's read-only `GET /v1/capabilities/zip-intake` readiness route is
mounted and Workspace-owner scoped; a Tauri command exposes that observation to the Library.
Its current status is `UNAVAILABLE` because the parser has not been integrated behind a
supervised isolated worker with enforced hard resource budgets. The desktop may save a ZIP
as an opaque Resource but must tell the user its members are not unpacked or indexed. Do not
pass archive bytes to the in-process Python provider. Extraction remains deferred until
worker containment, cancellation and crash handling, per-entry provenance, transactional
child-Resource publication, deletion, and platform/system qualification are implemented.

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
