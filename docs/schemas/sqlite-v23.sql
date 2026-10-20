-- Version twenty-three promotes scan-backed FILE ResourceLocations into an explicit
-- WorkspaceRoot membership index and adds a restart-safe revoke cleanup cursor.

CREATE TABLE workspace_root_file_locations (
  workspace_id TEXT NOT NULL,
  workspace_root_id TEXT NOT NULL,
  location_id TEXT NOT NULL UNIQUE,
  resource_id TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  PRIMARY KEY (workspace_root_id, location_id),
  FOREIGN KEY (workspace_id, workspace_root_id)
    REFERENCES workspace_roots(workspace_id, workspace_root_id),
  FOREIGN KEY (workspace_id, resource_id)
    REFERENCES resources(workspace_id, resource_id),
  FOREIGN KEY (resource_id, location_id)
    REFERENCES resource_locations(resource_id, location_id)
) STRICT;

CREATE INDEX idx_workspace_root_file_locations_cleanup
  ON workspace_root_file_locations(workspace_id, workspace_root_id, location_id);

CREATE TABLE workspace_root_cleanup_outbox (
  workspace_id TEXT NOT NULL,
  workspace_root_id TEXT PRIMARY KEY,
  after_location_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'COMPLETE')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version > 0),
  FOREIGN KEY (workspace_id, workspace_root_id)
    REFERENCES workspace_roots(workspace_id, workspace_root_id)
) STRICT;
