-- Resource content blob registry covers the gap between encrypted object publication
-- and the transaction that commits an immutable ResourceRevision.
CREATE TABLE resource_content_blob_registry (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  digest TEXT NOT NULL CHECK (
    length(digest) = 71
    AND substr(digest, 1, 7) = 'sha256:'
    AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
  media_type TEXT NOT NULL CHECK (length(media_type) > 0),
  created_at TEXT NOT NULL,
  expires_at TEXT,
  state TEXT NOT NULL CHECK (state IN ('RESERVED', 'TRACKED', 'DELETING')),
  claimed_at TEXT,
  PRIMARY KEY(workspace_id, digest),
  CHECK (
    (state = 'RESERVED' AND expires_at IS NOT NULL AND claimed_at IS NULL)
    OR (state = 'TRACKED' AND expires_at IS NULL AND claimed_at IS NULL)
    OR (state = 'DELETING' AND expires_at IS NULL AND claimed_at IS NOT NULL)
  )
) STRICT;

CREATE INDEX resource_content_blob_registry_gc
  ON resource_content_blob_registry(state, expires_at, created_at, workspace_id, digest);

-- Only immutable revisions reachable through a local managed blob location are roots.
-- External-only Resources with matching digests do not retain a local object.
INSERT INTO resource_content_blob_registry(
  workspace_id, digest, size_bytes, media_type, created_at, expires_at, state, claimed_at
)
SELECT resource.workspace_id, revision.content_digest, revision.size_bytes,
       COALESCE(revision.media_type, 'application/octet-stream'),
       MIN(revision.observed_at), NULL, 'TRACKED', NULL
FROM resources resource
JOIN resource_revisions revision ON revision.resource_id = resource.resource_id
WHERE revision.content_digest IS NOT NULL
  AND revision.size_bytes IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM resource_locations location
    WHERE location.resource_id = resource.resource_id
      AND location.provider_ref = 'litecowork.encrypted_blob'
      AND location.runtime_id IS NULL
      AND location.environment_id IS NULL
      AND location.connection_id IS NULL
  )
GROUP BY resource.workspace_id, revision.content_digest;

-- A ResourceRevision added to an existing locally-managed Resource must have an exact
-- reservation. Initial Resource creation is covered when its managed location is added.
CREATE TRIGGER resource_content_blob_registry_revision_insert_guard
BEFORE INSERT ON resource_revisions
WHEN EXISTS (
  SELECT 1 FROM resources resource
  JOIN resource_locations location ON location.resource_id = resource.resource_id
  WHERE resource.resource_id = NEW.resource_id
    AND location.provider_ref = 'litecowork.encrypted_blob'
    AND location.runtime_id IS NULL
    AND location.environment_id IS NULL
    AND location.connection_id IS NULL
)
AND NOT EXISTS (
  SELECT 1 FROM resources resource
  JOIN resource_content_blob_registry registry
    ON registry.workspace_id = resource.workspace_id
   AND registry.digest = NEW.content_digest
   AND registry.size_bytes = NEW.size_bytes
   AND registry.media_type = COALESCE(NEW.media_type, 'application/octet-stream')
  WHERE resource.resource_id = NEW.resource_id
    AND registry.state IN ('RESERVED', 'TRACKED')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONTENT_BLOB_NOT_RESERVED');
END;

-- Immutable revisions cannot be retargeted around the registry after publication.
CREATE TRIGGER resource_content_blob_registry_revision_update_guard
BEFORE UPDATE ON resource_revisions
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_REVISION_IMMUTABLE');
END;

CREATE TRIGGER resource_content_blob_registry_head_update_guard
BEFORE UPDATE OF current_revision_id ON resources
WHEN EXISTS (
  SELECT 1 FROM resource_locations location
  WHERE location.resource_id = NEW.resource_id
    AND location.provider_ref = 'litecowork.encrypted_blob'
    AND location.runtime_id IS NULL
    AND location.environment_id IS NULL
    AND location.connection_id IS NULL
)
AND NOT EXISTS (
  SELECT 1 FROM resource_revisions revision
  JOIN resource_content_blob_registry registry
    ON registry.workspace_id = NEW.workspace_id
   AND registry.digest = revision.content_digest
   AND registry.size_bytes = revision.size_bytes
   AND registry.media_type = COALESCE(revision.media_type, 'application/octet-stream')
  WHERE revision.resource_id = NEW.resource_id
    AND revision.resource_revision_id = NEW.current_revision_id
    AND registry.state IN ('RESERVED', 'TRACKED')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONTENT_BLOB_NOT_RESERVED');
END;

-- Registry object identity is immutable; only recovery state and lease timestamps move.
CREATE TRIGGER resource_content_blob_registry_identity_update_guard
BEFORE UPDATE OF workspace_id, digest, size_bytes, media_type
ON resource_content_blob_registry
WHEN NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.digest IS NOT OLD.digest
  OR NEW.size_bytes IS NOT OLD.size_bytes
  OR NEW.media_type IS NOT OLD.media_type
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONTENT_BLOB_IDENTITY_IMMUTABLE');
END;

-- A managed local location must point at a registered exact current revision.
CREATE TRIGGER resource_content_blob_registry_location_insert_guard
BEFORE INSERT ON resource_locations
WHEN NEW.provider_ref = 'litecowork.encrypted_blob'
 AND NEW.runtime_id IS NULL
 AND NEW.environment_id IS NULL
 AND NEW.connection_id IS NULL
AND NOT EXISTS (
  SELECT 1 FROM resources resource
  JOIN resource_revisions revision
    ON revision.resource_id = resource.resource_id
   AND revision.resource_revision_id = resource.current_revision_id
  JOIN resource_content_blob_registry registry
    ON registry.workspace_id = resource.workspace_id
   AND registry.digest = revision.content_digest
   AND registry.size_bytes = revision.size_bytes
   AND registry.media_type = COALESCE(revision.media_type, 'application/octet-stream')
  WHERE resource.resource_id = NEW.resource_id
    AND registry.state IN ('RESERVED', 'TRACKED')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONTENT_BLOB_NOT_RESERVED');
END;

CREATE TRIGGER resource_content_blob_registry_location_update_guard
BEFORE UPDATE OF resource_id, runtime_id, environment_id, connection_id, provider_ref,
                 locator_ref_id, observed_revision_id, observed_digest
ON resource_locations
WHEN NEW.provider_ref = 'litecowork.encrypted_blob'
 AND NEW.runtime_id IS NULL
 AND NEW.environment_id IS NULL
 AND NEW.connection_id IS NULL
AND NOT EXISTS (
  SELECT 1 FROM resources resource
  JOIN resource_revisions revision
    ON revision.resource_id = resource.resource_id
   AND revision.resource_revision_id = resource.current_revision_id
  JOIN resource_content_blob_registry registry
    ON registry.workspace_id = resource.workspace_id
   AND registry.digest = revision.content_digest
   AND registry.size_bytes = revision.size_bytes
   AND registry.media_type = COALESCE(revision.media_type, 'application/octet-stream')
  WHERE resource.resource_id = NEW.resource_id
    AND registry.state IN ('RESERVED', 'TRACKED')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONTENT_BLOB_NOT_RESERVED');
END;

-- Registry rows are local recovery metadata, but remain until every immutable local
-- revision reference to the exact Workspace/digest has been removed.
CREATE TRIGGER resource_content_blob_registry_reference_delete_guard
BEFORE DELETE ON resource_content_blob_registry
WHEN EXISTS (
  SELECT 1 FROM resources resource
  JOIN resource_revisions revision ON revision.resource_id = resource.resource_id
  WHERE resource.workspace_id = OLD.workspace_id
    AND revision.content_digest = OLD.digest
    AND EXISTS (
      SELECT 1 FROM resource_locations location
      WHERE location.resource_id = resource.resource_id
        AND location.provider_ref = 'litecowork.encrypted_blob'
        AND location.runtime_id IS NULL
        AND location.environment_id IS NULL
        AND location.connection_id IS NULL
    )
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONTENT_BLOB_STILL_REFERENCED');
END;
