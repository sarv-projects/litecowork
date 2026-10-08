-- Version seven adds a rebuildable revision-scoped local text index. Extracted text is
-- encrypted in the Workspace BlobStore under RESOURCE_INDEX. SQLite retains only the
-- content digest/blob pointer, parser/key version, and HMAC term tokens.
CREATE TABLE resource_text_indexes (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  resource_id TEXT NOT NULL,
  resource_revision_id TEXT NOT NULL,
  source_content_digest TEXT NOT NULL CHECK (
    length(source_content_digest) = 71
    AND substr(source_content_digest, 1, 7) = 'sha256:'
    AND substr(source_content_digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  extracted_blob_digest TEXT NOT NULL CHECK (
    length(extracted_blob_digest) = 71
    AND substr(extracted_blob_digest, 1, 7) = 'sha256:'
    AND substr(extracted_blob_digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  extracted_size_bytes INTEGER NOT NULL CHECK (extracted_size_bytes BETWEEN 0 AND 1048576),
  parser_id TEXT NOT NULL CHECK (parser_id = 'litecowork.plain-text.v1'),
  token_key_version INTEGER NOT NULL CHECK (token_key_version > 0),
  indexed_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, resource_id, resource_revision_id),
  FOREIGN KEY(workspace_id, resource_id) REFERENCES resources(workspace_id, resource_id),
  FOREIGN KEY(resource_id, resource_revision_id)
    REFERENCES resource_revisions(resource_id, resource_revision_id) ON DELETE CASCADE
);

CREATE TABLE resource_text_index_terms (
  workspace_id TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  resource_revision_id TEXT NOT NULL,
  token_key_version INTEGER NOT NULL CHECK (token_key_version > 0),
  term_token TEXT NOT NULL CHECK (
    length(term_token) = 64 AND term_token NOT GLOB '*[^0-9a-f]*'
  ),
  PRIMARY KEY(workspace_id, resource_id, resource_revision_id, token_key_version, term_token),
  FOREIGN KEY(workspace_id, resource_id, resource_revision_id)
    REFERENCES resource_text_indexes(workspace_id, resource_id, resource_revision_id)
    ON DELETE CASCADE
);

CREATE INDEX idx_resource_text_index_terms_lookup
  ON resource_text_index_terms(workspace_id, token_key_version, term_token, resource_id, resource_revision_id);

CREATE INDEX idx_resource_text_indexes_resource_revision
  ON resource_text_indexes(workspace_id, resource_id, resource_revision_id);

CREATE TRIGGER resource_text_index_scope_guard
BEFORE INSERT ON resource_text_indexes
WHEN NOT EXISTS (
  SELECT 1 FROM resources r
  JOIN resource_revisions revision ON revision.resource_id = r.resource_id
  WHERE r.workspace_id = NEW.workspace_id
    AND r.resource_id = NEW.resource_id
    AND revision.resource_revision_id = NEW.resource_revision_id
    AND revision.content_digest = NEW.source_content_digest
    AND revision.size_bytes = NEW.extracted_size_bytes
    AND r.current_revision_id = NEW.resource_revision_id
    AND (
      r.context_document_json IS NULL
      OR json_extract(r.context_document_json, '$.status') = 'ACTIVE'
    )
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_TEXT_INDEX_SCOPE_MISMATCH');
END;

CREATE TRIGGER resource_text_index_terms_scope_guard
BEFORE INSERT ON resource_text_index_terms
WHEN NOT EXISTS (
  SELECT 1 FROM resource_text_indexes i
  WHERE i.workspace_id = NEW.workspace_id
    AND i.resource_id = NEW.resource_id
    AND i.resource_revision_id = NEW.resource_revision_id
    AND i.token_key_version = NEW.token_key_version
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_TEXT_INDEX_TERM_SCOPE_MISMATCH');
END;

CREATE TRIGGER resource_text_index_update_guard
BEFORE UPDATE ON resource_text_indexes
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_TEXT_INDEX_REPLACE_REQUIRED');
END;
