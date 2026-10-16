-- Version nineteen registers encrypted ResourceIndex snapshots for bounded orphan recovery.
-- ResourceIndex blob registry makes snapshots discoverable without provider inventory.
-- RESERVED records cover the BlobStore/SQLite commit gap; TRACKED records remain after
-- projection replacement or purge so unreferenced derived snapshots can be collected.
CREATE TABLE resource_index_blob_registry (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  digest TEXT NOT NULL CHECK (
    length(digest) = 71
    AND substr(digest, 1, 7) = 'sha256:'
    AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 0 AND 1048576),
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

CREATE INDEX resource_index_blob_registry_gc
  ON resource_index_blob_registry(state, expires_at, created_at, workspace_id, digest);

-- Existing index references become tracked GC roots. Any pre-migration orphan with no
-- ResourceIndex row remains undiscoverable because BlobStore deliberately has no listing.
INSERT INTO resource_index_blob_registry(
  workspace_id, digest, size_bytes, created_at, expires_at, state, claimed_at
)
SELECT workspace_id, extracted_blob_digest, MAX(extracted_size_bytes), MIN(indexed_at),
       NULL, 'TRACKED', NULL
FROM resource_text_indexes
GROUP BY workspace_id, extracted_blob_digest;

-- A projection row may only point at a registered, same-sized encrypted snapshot. A
-- DELETING registry row fences new references until exact-object removal completes.
CREATE TRIGGER resource_text_index_blob_registry_insert_guard
BEFORE INSERT ON resource_text_indexes
WHEN NOT EXISTS (
  SELECT 1 FROM resource_index_blob_registry registry
  WHERE registry.workspace_id = NEW.workspace_id
    AND registry.digest = NEW.extracted_blob_digest
    AND registry.size_bytes = NEW.extracted_size_bytes
    AND registry.state IN ('RESERVED', 'TRACKED')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_INDEX_BLOB_NOT_RESERVED');
END;

CREATE TRIGGER resource_text_index_blob_registry_update_guard
BEFORE UPDATE OF workspace_id, extracted_blob_digest, extracted_size_bytes
ON resource_text_indexes
WHEN NOT EXISTS (
  SELECT 1 FROM resource_index_blob_registry registry
  WHERE registry.workspace_id = NEW.workspace_id
    AND registry.digest = NEW.extracted_blob_digest
    AND registry.size_bytes = NEW.extracted_size_bytes
    AND registry.state IN ('RESERVED', 'TRACKED')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_INDEX_BLOB_NOT_RESERVED');
END;

-- The registry row is operational recovery state, but it cannot be dropped while any
-- immutable revision projection still relies on its exact Workspace/digest object.
CREATE TRIGGER resource_index_blob_registry_reference_delete_guard
BEFORE DELETE ON resource_index_blob_registry
WHEN EXISTS (
  SELECT 1 FROM resource_text_indexes idx
  WHERE idx.workspace_id = OLD.workspace_id
    AND idx.extracted_blob_digest = OLD.digest
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_INDEX_BLOB_STILL_REFERENCED');
END;
