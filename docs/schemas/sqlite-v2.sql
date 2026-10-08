-- Additive migration from the immutable SQLite schema v1.
-- Resource upload path metadata is descriptive provenance, never a filesystem locator,
-- authorization grant, or ResourceLocationBinding.
ALTER TABLE resource_upload_sessions
  ADD COLUMN committed_resource_id TEXT REFERENCES resources(resource_id);

ALTER TABLE resource_upload_sessions
  ADD COLUMN progress_version INTEGER NOT NULL DEFAULT 1 CHECK (progress_version >= 1);

ALTER TABLE resource_upload_sessions
  ADD COLUMN folder_import_json TEXT CHECK (
    folder_import_json IS NULL OR (
      json_valid(folder_import_json)
      AND json_type(folder_import_json) = 'object'
      AND json_type(folder_import_json, '$.relative_path') = 'text'
      AND length(json_extract(folder_import_json, '$.relative_path')) BETWEEN 1 AND 240
    )
  );

-- Recover committed Resource identity from its durable transition event where possible.
-- Historical committed rows with no matching event remain explicitly unmapped; they are
-- retained for audit and must not be treated as resumable uploads.
UPDATE resource_upload_sessions
SET committed_resource_id = (
  SELECT json_extract(event.payload_json, '$.resource_id')
  FROM domain_events AS event
  WHERE event.entity_type = 'ResourceUpload'
    AND event.entity_id = resource_upload_sessions.upload_id
    AND event.type = 'resource.upload.status.changed.v1'
    AND json_extract(event.payload_json, '$.to') = 'COMMITTED'
    AND json_type(event.payload_json, '$.resource_id') = 'text'
  ORDER BY event.entity_revision DESC
  LIMIT 1
)
WHERE state = 'COMMITTED';

CREATE TRIGGER resource_upload_initial_version_guard
BEFORE INSERT ON resource_upload_sessions
WHEN NEW.version <> 1 OR NEW.progress_version <> 1
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_INITIAL_VERSION_INVALID');
END;

CREATE TRIGGER resource_upload_new_contract_guard
BEFORE INSERT ON resource_upload_sessions
WHEN NEW.expected_size_bytes NOT BETWEEN 0 AND 104857600
  OR NEW.expected_digest IS NULL
  OR length(NEW.expected_digest) <> 71
  OR substr(NEW.expected_digest, 1, 7) <> 'sha256:'
  OR substr(NEW.expected_digest, 8) GLOB '*[^0-9a-f]*'
  OR NEW.chunk_size_bytes NOT BETWEEN 1 AND 4194304
  OR (NEW.state = 'COMMITTED' AND NEW.committed_resource_id IS NULL)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_INITIAL_CONTRACT_INVALID');
END;

CREATE TRIGGER resource_upload_immutable_metadata_guard
BEFORE UPDATE OF workspace_id, display_name, media_type, expected_size_bytes,
  expected_digest, context_document_json, folder_import_json, chunk_size_bytes,
  resource_id, expected_resource_version, parent_revision_ids_json, created_at
ON resource_upload_sessions
WHEN NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.display_name IS NOT OLD.display_name
  OR NEW.media_type IS NOT OLD.media_type
  OR NEW.expected_size_bytes IS NOT OLD.expected_size_bytes
  OR NEW.expected_digest IS NOT OLD.expected_digest
  OR NEW.context_document_json IS NOT OLD.context_document_json
  OR NEW.folder_import_json IS NOT OLD.folder_import_json
  OR NEW.chunk_size_bytes IS NOT OLD.chunk_size_bytes
  OR NEW.resource_id IS NOT OLD.resource_id
  OR NEW.expected_resource_version IS NOT OLD.expected_resource_version
  OR NEW.parent_revision_ids_json IS NOT OLD.parent_revision_ids_json
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_METADATA_IS_IMMUTABLE');
END;

CREATE TRIGGER resource_upload_folder_import_initial_only
BEFORE INSERT ON resource_upload_sessions
WHEN NEW.folder_import_json IS NOT NULL AND NEW.resource_id IS NOT NULL
  OR (NEW.folder_import_json IS NOT NULL
    AND json_extract(NEW.folder_import_json, '$.relative_path') IS NOT NEW.display_name)
BEGIN
  SELECT RAISE(ABORT, 'FOLDER_IMPORT_ONLY_ALLOWED_FOR_INITIAL_RESOURCE_UPLOAD');
END;

CREATE TRIGGER resource_upload_state_transition_guard
BEFORE UPDATE OF state, version, progress_version, committed_resource_id ON resource_upload_sessions
WHEN NOT (
  (OLD.state = 'OPEN' AND NEW.state = 'OPEN'
    AND OLD.version = NEW.version
    AND NEW.progress_version = OLD.progress_version + 1
    AND NEW.committed_resource_id IS OLD.committed_resource_id)
  OR (OLD.state = 'OPEN' AND NEW.state = 'CONTENT_RECEIVED'
    AND NEW.version = OLD.version + 1
    AND NEW.progress_version = OLD.progress_version + 1
    AND NEW.committed_resource_id IS OLD.committed_resource_id)
  OR (OLD.state IN ('OPEN', 'CONTENT_RECEIVED') AND NEW.state = 'EXPIRED'
    AND NEW.version = OLD.version + 1
    AND NEW.progress_version = OLD.progress_version
    AND NEW.committed_resource_id IS OLD.committed_resource_id)
  OR (OLD.state = 'CONTENT_RECEIVED' AND NEW.state = 'FAILED'
    AND NEW.version = OLD.version + 1
    AND NEW.progress_version = OLD.progress_version
    AND NEW.committed_resource_id IS OLD.committed_resource_id)
  OR (OLD.state = 'CONTENT_RECEIVED' AND NEW.state = 'COMMITTED'
    AND NEW.version = OLD.version + 1
    AND NEW.progress_version = OLD.progress_version
    AND NEW.committed_resource_id IS NOT NULL
    AND NEW.committed_resource_id IS NOT OLD.committed_resource_id)
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_VERSION_TRANSITION_INVALID');
END;

CREATE TABLE resource_upload_chunk_requests (
  upload_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  chunk_index INTEGER NOT NULL,
  request_payload_digest TEXT NOT NULL CHECK (length(request_payload_digest) = 71 AND substr(request_payload_digest, 1, 7) = 'sha256:' AND substr(request_payload_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  created_at TEXT NOT NULL,
  PRIMARY KEY(upload_id, request_id),
  FOREIGN KEY(upload_id, chunk_index) REFERENCES resource_upload_chunks(upload_id, chunk_index)
);

-- Operational write intents make encrypted chunk objects reclaimable if the process
-- stops after BlobStore.put but before the chunk receipt transaction commits.
CREATE TABLE resource_upload_blob_reservations (
  upload_id TEXT NOT NULL REFERENCES resource_upload_sessions(upload_id),
  request_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  chunk_index INTEGER NOT NULL,
  digest TEXT NOT NULL CHECK (length(digest) = 71 AND substr(digest, 1, 7) = 'sha256:' AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'),
  size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 4194304),
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('RESERVED', 'DELETING')),
  PRIMARY KEY(upload_id, request_id)
);

CREATE INDEX resource_upload_blob_reservations_gc
  ON resource_upload_blob_reservations(workspace_id, digest, state, expires_at);

-- DELETING is a durable fence: new chunk writes for this object are rejected until
-- deletion finishes. A restart can safely retry deletion before clearing this row.
CREATE TABLE resource_upload_blob_gc_fences (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  digest TEXT NOT NULL CHECK (length(digest) = 71 AND substr(digest, 1, 7) = 'sha256:' AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'),
  size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 4194304),
  claimed_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, digest)
);
