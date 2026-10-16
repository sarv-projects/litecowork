-- ContextDocument purge now includes exact ResourceUpload chunk objects. The registry
-- also closes the process-crash gap between chunk publication and receipt acceptance.
CREATE TABLE resource_upload_chunk_blob_registry (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  digest TEXT NOT NULL CHECK (
    length(digest) = 71
    AND substr(digest, 1, 7) = 'sha256:'
    AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 4194304),
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

CREATE INDEX resource_upload_chunk_blob_registry_gc
  ON resource_upload_chunk_blob_registry(state, expires_at, created_at, workspace_id, digest);

-- Preserve every accepted range and every interrupted reservation. The physical
-- object is tracked once per Workspace/digest, independently of its exact ledger refs.
WITH registry_sources AS (
  SELECT session.workspace_id, chunk.temporary_blob_ref AS digest,
         chunk.end_offset_exclusive - chunk.start_offset AS size_bytes,
         chunk.received_at AS created_at, NULL AS expires_at,
         'TRACKED' AS state, NULL AS claimed_at
  FROM resource_upload_chunks chunk
  JOIN resource_upload_sessions session ON session.upload_id = chunk.upload_id
  UNION ALL
  SELECT reservation.workspace_id, reservation.digest, reservation.size_bytes,
         reservation.created_at, reservation.expires_at, reservation.state,
         NULL AS claimed_at
  FROM resource_upload_blob_reservations reservation
  UNION ALL
  SELECT fence.workspace_id, fence.digest, fence.size_bytes,
         fence.claimed_at AS created_at, NULL AS expires_at,
         'DELETING' AS state, fence.claimed_at
  FROM resource_upload_blob_gc_fences fence
)
INSERT INTO resource_upload_chunk_blob_registry(
  workspace_id, digest, size_bytes, created_at, expires_at, state, claimed_at
)
SELECT workspace_id, digest, MIN(size_bytes), MIN(created_at),
       CASE WHEN MAX(state = 'DELETING') = 1 OR MAX(state = 'TRACKED') = 1
            THEN NULL ELSE MAX(expires_at) END,
       CASE WHEN MAX(state = 'DELETING') = 1 THEN 'DELETING'
            WHEN MAX(state = 'TRACKED') = 1 THEN 'TRACKED'
            ELSE 'RESERVED' END,
       CASE WHEN MAX(state = 'DELETING') = 1
            THEN MAX(claimed_at) ELSE NULL END
FROM registry_sources
GROUP BY workspace_id, digest;

CREATE TRIGGER resource_upload_chunk_blob_registry_identity_update_guard
BEFORE UPDATE OF workspace_id, digest, size_bytes ON resource_upload_chunk_blob_registry
WHEN NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.digest IS NOT OLD.digest
  OR NEW.size_bytes IS NOT OLD.size_bytes
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_CHUNK_BLOB_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER resource_upload_chunk_blob_registry_state_update_guard
BEFORE UPDATE OF state ON resource_upload_chunk_blob_registry
WHEN NOT (
  OLD.state = NEW.state
  OR (OLD.state = 'RESERVED' AND NEW.state IN ('TRACKED', 'DELETING'))
  OR (OLD.state = 'TRACKED' AND NEW.state = 'DELETING')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_CHUNK_BLOB_STATE_TRANSITION_INVALID');
END;

CREATE TRIGGER resource_upload_chunk_reservation_resource_guard
BEFORE INSERT ON resource_upload_blob_reservations
WHEN NOT EXISTS (
  SELECT 1 FROM resource_upload_sessions session
  LEFT JOIN resources resource
    ON resource.workspace_id = session.workspace_id
   AND resource.resource_id = session.resource_id
  WHERE session.upload_id = NEW.upload_id
    AND session.workspace_id = NEW.workspace_id
    AND session.state = 'OPEN'
    AND (session.resource_id IS NULL OR (
      resource.resource_id IS NOT NULL
      AND resource.version = session.expected_resource_version
      AND (resource.context_document_json IS NULL
           OR json_extract(resource.context_document_json, '$.status') = 'ACTIVE')
    ))
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONFLICT');
END;

CREATE TRIGGER resource_upload_chunk_reservation_registry_guard
BEFORE INSERT ON resource_upload_blob_reservations
WHEN NOT EXISTS (
  SELECT 1 FROM resource_upload_chunk_blob_registry registry
  WHERE registry.workspace_id = NEW.workspace_id
    AND registry.digest = NEW.digest
    AND registry.size_bytes = NEW.size_bytes
    AND registry.state IN ('RESERVED', 'TRACKED')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_CHUNK_BLOB_NOT_RESERVED');
END;

CREATE TRIGGER resource_upload_chunk_acceptance_resource_guard
BEFORE INSERT ON resource_upload_chunks
WHEN NOT EXISTS (
  SELECT 1 FROM resource_upload_sessions session
  LEFT JOIN resources resource
    ON resource.workspace_id = session.workspace_id
   AND resource.resource_id = session.resource_id
  WHERE session.upload_id = NEW.upload_id
    AND session.state = 'OPEN'
    AND (session.resource_id IS NULL OR (
      resource.resource_id IS NOT NULL
      AND resource.version = session.expected_resource_version
      AND (resource.context_document_json IS NULL
           OR json_extract(resource.context_document_json, '$.status') = 'ACTIVE')
    ))
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_CONFLICT');
END;

CREATE TRIGGER resource_upload_chunk_registry_insert_guard
BEFORE INSERT ON resource_upload_chunks
WHEN NEW.sha256 <> NEW.temporary_blob_ref
  OR NOT EXISTS (
  SELECT 1 FROM resource_upload_sessions session
  JOIN resource_upload_chunk_blob_registry registry
    ON registry.workspace_id = session.workspace_id
   AND registry.digest = NEW.temporary_blob_ref
   AND registry.size_bytes = NEW.end_offset_exclusive - NEW.start_offset
   AND registry.state = 'TRACKED'
  WHERE session.upload_id = NEW.upload_id
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_CHUNK_BLOB_NOT_TRACKED');
END;

CREATE TRIGGER resource_upload_chunk_ledger_no_update
BEFORE UPDATE ON resource_upload_chunks
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_CHUNK_LEDGER_IMMUTABLE');
END;

CREATE TRIGGER resource_upload_chunk_ledger_no_delete
BEFORE DELETE ON resource_upload_chunks
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_CHUNK_LEDGER_IMMUTABLE');
END;

CREATE TRIGGER resource_upload_chunk_blob_registry_delete_guard
BEFORE DELETE ON resource_upload_chunk_blob_registry
WHEN OLD.state <> 'DELETING'
  OR NOT EXISTS (
    SELECT 1 FROM resource_upload_blob_gc_fences fence
    WHERE fence.workspace_id = OLD.workspace_id
      AND fence.digest = OLD.digest
      AND fence.size_bytes = OLD.size_bytes
  )
  OR EXISTS (
  SELECT 1 FROM resource_upload_chunks chunk
  JOIN resource_upload_sessions session ON session.upload_id = chunk.upload_id
  WHERE session.workspace_id = OLD.workspace_id
    AND chunk.temporary_blob_ref = OLD.digest
    AND session.state IN ('OPEN', 'CONTENT_RECEIVED')
)
  OR EXISTS (
    SELECT 1 FROM resource_upload_blob_reservations reservation
    JOIN resource_upload_sessions session ON session.upload_id = reservation.upload_id
    WHERE reservation.workspace_id = OLD.workspace_id
      AND reservation.digest = OLD.digest
      AND reservation.state IN ('RESERVED', 'DELETING')
      AND session.state IN ('OPEN', 'CONTENT_RECEIVED')
  )
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_CHUNK_BLOB_STILL_REFERENCED');
END;

CREATE TRIGGER resource_upload_blob_reservation_identity_update_guard
BEFORE UPDATE OF upload_id, request_id, workspace_id, chunk_index, digest, size_bytes
ON resource_upload_blob_reservations
WHEN NEW.upload_id IS NOT OLD.upload_id
  OR NEW.request_id IS NOT OLD.request_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.chunk_index IS NOT OLD.chunk_index
  OR NEW.digest IS NOT OLD.digest
  OR NEW.size_bytes IS NOT OLD.size_bytes
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_BLOB_RESERVATION_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER resource_upload_blob_reservation_state_update_guard
BEFORE UPDATE OF state ON resource_upload_blob_reservations
WHEN NOT (
  OLD.state = NEW.state
  OR (OLD.state = 'RESERVED' AND NEW.state = 'DELETING')
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_BLOB_RESERVATION_STATE_TRANSITION_INVALID');
END;

CREATE TRIGGER resource_upload_blob_reservation_delete_guard
BEFORE DELETE ON resource_upload_blob_reservations
WHEN NOT (
  (OLD.state = 'RESERVED' AND EXISTS (
    SELECT 1 FROM resource_upload_chunks chunk
    WHERE chunk.upload_id = OLD.upload_id
      AND chunk.chunk_index = OLD.chunk_index
      AND chunk.temporary_blob_ref = OLD.digest
      AND chunk.end_offset_exclusive - chunk.start_offset = OLD.size_bytes
  ))
  OR (OLD.state = 'DELETING' AND EXISTS (
    SELECT 1 FROM resource_upload_chunk_blob_registry registry
    JOIN resource_upload_blob_gc_fences fence
      ON fence.workspace_id = registry.workspace_id
     AND fence.digest = registry.digest
     AND fence.size_bytes = registry.size_bytes
    WHERE registry.workspace_id = OLD.workspace_id
      AND registry.digest = OLD.digest
      AND registry.state = 'DELETING'
  ))
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_UPLOAD_BLOB_RESERVATION_STILL_OWNED');
END;

-- The original v1 purge tables are retained byte-for-byte. Rebuild only the receipt
-- table so UPLOAD_CHUNK is an explicit kind and its exact refs survive restart.
DROP TRIGGER context_document_purge_receipt_admission_guard;
DROP TRIGGER context_document_purge_receipt_transition_guard;
DROP TRIGGER context_document_deleted_requires_purge_ack;
DROP TRIGGER context_document_purge_plan_shape_guard;
DROP INDEX idx_context_purge_pending;
ALTER TABLE context_document_purge_receipts RENAME TO context_document_purge_receipts_v17;

CREATE TABLE context_document_purge_receipts (
  workspace_id TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  replica_ref TEXT NOT NULL CHECK (
    length(replica_ref) = 71
    AND substr(replica_ref, 1, 7) = 'sha256:'
    AND substr(replica_ref, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  replica_kind TEXT NOT NULL CHECK (replica_kind IN ('BLOB', 'DERIVED_INDEX', 'UPLOAD_CHUNK')),
  runtime_id TEXT REFERENCES runtimes(runtime_id),
  runtime_incarnation_id TEXT,
  target_revision_ids_json TEXT NOT NULL CHECK (
    json_valid(target_revision_ids_json) AND json_type(target_revision_ids_json) = 'array'
  ),
  target_upload_chunk_refs_json TEXT NOT NULL DEFAULT '[]' CHECK (
    json_valid(target_upload_chunk_refs_json)
    AND json_type(target_upload_chunk_refs_json) = 'array'
  ),
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'ACKNOWLEDGED')),
  requested_at TEXT NOT NULL,
  acknowledged_at TEXT,
  receipt_digest TEXT CHECK (receipt_digest IS NULL OR (
    length(receipt_digest) = 71
    AND substr(receipt_digest, 1, 7) = 'sha256:'
    AND substr(receipt_digest, 8) NOT GLOB '*[^0-9a-f]*'
  )),
  PRIMARY KEY(workspace_id, resource_id, replica_ref, replica_kind),
  FOREIGN KEY(workspace_id, resource_id) REFERENCES resources(workspace_id, resource_id),
  CHECK ((runtime_id IS NULL AND runtime_incarnation_id IS NULL) OR
         (runtime_id IS NOT NULL AND runtime_incarnation_id IS NOT NULL)),
  CHECK ((status = 'PENDING' AND acknowledged_at IS NULL AND receipt_digest IS NULL) OR
         (status = 'ACKNOWLEDGED' AND acknowledged_at IS NOT NULL AND receipt_digest IS NOT NULL))
) STRICT;

INSERT INTO context_document_purge_receipts(
  workspace_id, resource_id, replica_ref, replica_kind, runtime_id,
  runtime_incarnation_id, target_revision_ids_json, target_upload_chunk_refs_json,
  status, requested_at, acknowledged_at, receipt_digest
)
SELECT workspace_id, resource_id, replica_ref, replica_kind, runtime_id,
       runtime_incarnation_id, target_revision_ids_json, '[]', status, requested_at,
       acknowledged_at, receipt_digest
FROM context_document_purge_receipts_v17;
DROP TABLE context_document_purge_receipts_v17;
CREATE INDEX idx_context_purge_pending
  ON context_document_purge_receipts(workspace_id, resource_id, status);

CREATE TRIGGER context_document_purge_plan_shape_guard
BEFORE INSERT ON context_document_purge_plans
WHEN EXISTS (
  SELECT 1 FROM json_each(NEW.targets_json) target
  WHERE json_type(target.value) <> 'object'
    OR COALESCE(json_type(target.value, '$.replica_ref'), '') <> 'text'
    OR length(json_extract(target.value, '$.replica_ref')) <> 71
    OR substr(json_extract(target.value, '$.replica_ref'), 1, 7) <> 'sha256:'
    OR substr(json_extract(target.value, '$.replica_ref'), 8) GLOB '*[^0-9a-f]*'
    OR COALESCE(json_type(target.value, '$.replica_kind'), '') <> 'text'
    OR json_extract(target.value, '$.replica_kind') NOT IN ('BLOB', 'DERIVED_INDEX', 'UPLOAD_CHUNK')
    OR COALESCE(json_type(target.value, '$.runtime_id'), '') NOT IN ('text', 'null')
    OR COALESCE(json_type(target.value, '$.runtime_incarnation_id'), '') NOT IN ('text', 'null')
    OR ((json_type(target.value, '$.runtime_id') = 'text') <>
        (json_type(target.value, '$.runtime_incarnation_id') = 'text'))
    OR COALESCE(json_type(target.value, '$.target_revision_ids'), '') <> 'array'
    OR COALESCE(json_type(target.value, '$.target_upload_chunk_refs'), 'array') <> 'array'
    OR (json_extract(target.value, '$.replica_kind') = 'UPLOAD_CHUNK'
        AND (json_array_length(target.value, '$.target_revision_ids') <> 0
             OR json_array_length(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) = 0))
    OR (json_extract(target.value, '$.replica_kind') <> 'UPLOAD_CHUNK'
        AND (json_array_length(target.value, '$.target_revision_ids') = 0
             OR json_array_length(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) <> 0))
    OR (json_extract(target.value, '$.replica_kind') = 'UPLOAD_CHUNK'
        AND NOT EXISTS (
          SELECT 1 FROM resource_upload_chunk_blob_registry registry
          WHERE registry.workspace_id = NEW.workspace_id
            AND registry.digest = json_extract(target.value, '$.replica_ref')
            AND registry.state IN ('RESERVED', 'TRACKED', 'DELETING')
        ))
    OR EXISTS (
      SELECT 1 FROM json_each(target.value, '$.target_revision_ids') revision
      WHERE revision.type <> 'text' OR length(revision.value) = 0
    )
    OR EXISTS (
      SELECT revision.value FROM json_each(target.value, '$.target_revision_ids') revision
      GROUP BY revision.value HAVING COUNT(*) > 1
    )
    OR EXISTS (
      SELECT 1 FROM json_each(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) chunk_ref
      WHERE json_type(chunk_ref.value) <> 'object'
        OR COALESCE(json_type(chunk_ref.value, '$.upload_id'), '') <> 'text'
        OR length(json_extract(chunk_ref.value, '$.upload_id')) = 0
        OR COALESCE(json_type(chunk_ref.value, '$.chunk_index'), '') <> 'integer'
        OR json_extract(chunk_ref.value, '$.chunk_index') < 0
        OR EXISTS (
          SELECT 1 FROM json_each(chunk_ref.value) property
          WHERE property.key NOT IN ('upload_id', 'chunk_index')
        )
    )
    OR EXISTS (
      SELECT json_extract(chunk_ref.value, '$.upload_id'),
             json_extract(chunk_ref.value, '$.chunk_index')
      FROM json_each(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) chunk_ref
      GROUP BY json_extract(chunk_ref.value, '$.upload_id'),
               json_extract(chunk_ref.value, '$.chunk_index')
      HAVING COUNT(*) > 1
    )
    OR EXISTS (
      SELECT 1
      FROM json_each(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) current_ref
      JOIN json_each(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) previous_ref
        ON CAST(previous_ref.key AS INTEGER) = CAST(current_ref.key AS INTEGER) - 1
      WHERE json_extract(previous_ref.value, '$.upload_id') > json_extract(current_ref.value, '$.upload_id')
         OR (json_extract(previous_ref.value, '$.upload_id') = json_extract(current_ref.value, '$.upload_id')
             AND json_extract(previous_ref.value, '$.chunk_index') >= json_extract(current_ref.value, '$.chunk_index'))
    )
    OR EXISTS (
      SELECT 1 FROM json_each(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) chunk_ref
      WHERE NOT EXISTS (
        SELECT 1 FROM resource_upload_sessions session
        WHERE session.workspace_id = NEW.workspace_id
          AND (session.resource_id = NEW.resource_id OR session.committed_resource_id = NEW.resource_id)
          AND session.upload_id = json_extract(chunk_ref.value, '$.upload_id')
          AND (
            EXISTS (
              SELECT 1 FROM resource_upload_chunks chunk
              WHERE chunk.upload_id = session.upload_id
                AND chunk.chunk_index = json_extract(chunk_ref.value, '$.chunk_index')
                AND chunk.temporary_blob_ref = json_extract(target.value, '$.replica_ref')
            )
            OR EXISTS (
              SELECT 1 FROM resource_upload_blob_reservations reservation
              WHERE reservation.upload_id = session.upload_id
                AND reservation.chunk_index = json_extract(chunk_ref.value, '$.chunk_index')
                AND reservation.digest = json_extract(target.value, '$.replica_ref')
            )
          )
      )
    )
    OR EXISTS (
      SELECT 1 FROM resource_upload_chunks chunk
      JOIN resource_upload_sessions session ON session.upload_id = chunk.upload_id
      WHERE session.workspace_id = NEW.workspace_id
        AND json_extract(target.value, '$.replica_kind') = 'UPLOAD_CHUNK'
        AND (session.resource_id = NEW.resource_id OR session.committed_resource_id = NEW.resource_id)
        AND chunk.temporary_blob_ref = json_extract(target.value, '$.replica_ref')
        AND NOT EXISTS (
          SELECT 1 FROM json_each(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) chunk_ref
          WHERE json_extract(chunk_ref.value, '$.upload_id') = chunk.upload_id
            AND json_extract(chunk_ref.value, '$.chunk_index') = chunk.chunk_index
        )
      )
    OR EXISTS (
      SELECT 1 FROM resource_upload_blob_reservations reservation
      JOIN resource_upload_sessions session ON session.upload_id = reservation.upload_id
      WHERE session.workspace_id = NEW.workspace_id
        AND json_extract(target.value, '$.replica_kind') = 'UPLOAD_CHUNK'
        AND (session.resource_id = NEW.resource_id OR session.committed_resource_id = NEW.resource_id)
        AND reservation.digest = json_extract(target.value, '$.replica_ref')
        AND NOT EXISTS (
          SELECT 1 FROM json_each(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) chunk_ref
          WHERE json_extract(chunk_ref.value, '$.upload_id') = reservation.upload_id
            AND json_extract(chunk_ref.value, '$.chunk_index') = reservation.chunk_index
        )
    )
    OR EXISTS (
      SELECT 1 FROM json_each(target.value) property
      WHERE property.key NOT IN (
        'replica_ref', 'replica_kind', 'runtime_id', 'runtime_incarnation_id',
        'target_revision_ids', 'target_upload_chunk_refs'
      )
    )
)
  OR EXISTS (
    SELECT json_extract(target.value, '$.replica_ref'), json_extract(target.value, '$.replica_kind')
    FROM json_each(NEW.targets_json) target
    GROUP BY json_extract(target.value, '$.replica_ref'), json_extract(target.value, '$.replica_kind')
    HAVING COUNT(*) > 1
  )
BEGIN
  SELECT RAISE(ABORT, 'INVALID_ARGUMENT');
END;

CREATE TRIGGER context_document_purge_receipt_admission_guard
BEFORE INSERT ON context_document_purge_receipts
WHEN NOT EXISTS (
  SELECT 1 FROM resources resource
  JOIN context_document_purge_plans plan
    ON plan.workspace_id = resource.workspace_id AND plan.resource_id = resource.resource_id
  WHERE resource.workspace_id = NEW.workspace_id
    AND resource.resource_id = NEW.resource_id
    AND json_extract(resource.context_document_json, '$.status') = 'DELETION_PENDING'
    AND json_extract(resource.context_document_json, '$.purge_manifest_digest') = plan.manifest_digest
    AND json_extract(resource.context_document_json, '$.purge_target_count') = plan.target_count
    AND EXISTS (
      SELECT 1 FROM json_each(plan.targets_json) target
      WHERE json_extract(target.value, '$.replica_ref') = NEW.replica_ref
        AND json_extract(target.value, '$.replica_kind') = NEW.replica_kind
        AND json_extract(target.value, '$.runtime_id') IS NEW.runtime_id
        AND json_extract(target.value, '$.runtime_incarnation_id') IS NEW.runtime_incarnation_id
        AND json(json_extract(target.value, '$.target_revision_ids')) = json(NEW.target_revision_ids_json)
        AND json(COALESCE(json_extract(target.value, '$.target_upload_chunk_refs'), '[]')) =
            json(NEW.target_upload_chunk_refs_json)
    )
)
BEGIN
  SELECT RAISE(ABORT, 'CONTEXT_DOCUMENT_NOT_PENDING_DELETION');
END;

CREATE TRIGGER context_document_purge_receipt_transition_guard
BEFORE UPDATE ON context_document_purge_receipts
WHEN NEW.workspace_id <> OLD.workspace_id
  OR NEW.resource_id <> OLD.resource_id
  OR NEW.replica_ref <> OLD.replica_ref
  OR NEW.replica_kind <> OLD.replica_kind
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.runtime_incarnation_id IS NOT OLD.runtime_incarnation_id
  OR NEW.target_revision_ids_json <> OLD.target_revision_ids_json
  OR NEW.target_upload_chunk_refs_json <> OLD.target_upload_chunk_refs_json
  OR NEW.requested_at <> OLD.requested_at
  OR OLD.status <> 'PENDING'
  OR NEW.status <> 'ACKNOWLEDGED'
BEGIN
  SELECT RAISE(ABORT, 'CONTEXT_PURGE_RECEIPT_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER context_document_deleted_requires_purge_ack
BEFORE UPDATE OF context_document_json ON resources
WHEN json_extract(NEW.context_document_json, '$.status') = 'DELETED'
  AND (
    NOT EXISTS (
      SELECT 1 FROM context_document_purge_plans plan
      WHERE plan.workspace_id = NEW.workspace_id AND plan.resource_id = NEW.resource_id
        AND plan.manifest_digest = json_extract(NEW.context_document_json, '$.purge_manifest_digest')
        AND plan.target_count = json_extract(NEW.context_document_json, '$.purge_target_count')
    )
    OR (SELECT COUNT(*) FROM context_document_purge_receipts receipt
        WHERE receipt.workspace_id = NEW.workspace_id AND receipt.resource_id = NEW.resource_id)
       <> json_extract(NEW.context_document_json, '$.purge_target_count')
    OR EXISTS (
      SELECT 1 FROM context_document_purge_receipts receipt
      WHERE receipt.workspace_id = NEW.workspace_id
        AND receipt.resource_id = NEW.resource_id
        AND receipt.status <> 'ACKNOWLEDGED'
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'CONTEXT_DOCUMENT_PURGE_INCOMPLETE');
END;
