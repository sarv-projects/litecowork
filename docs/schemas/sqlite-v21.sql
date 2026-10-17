-- Version twenty-one stores a rebuildable, Runtime-local projection of file observations
-- made beneath explicitly selected WorkspaceRoots. These rows do not create Resource,
-- ResourceRevision, or ResourceLocation aggregates. Stable identity values are keyed
-- pseudonyms; raw relative locators live only in the private binding table below.

CREATE TABLE workspace_root_observation_generations (
  workspace_id TEXT NOT NULL,
  workspace_root_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  complete_generation INTEGER NOT NULL DEFAULT 0 CHECK (complete_generation >= 0),
  last_complete_scan_request_id TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, workspace_root_id, runtime_incarnation_id),
  UNIQUE (workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id),
  FOREIGN KEY (workspace_id, workspace_root_id)
    REFERENCES workspace_roots(workspace_id, workspace_root_id),
  FOREIGN KEY (runtime_id, runtime_incarnation_id)
    REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
) STRICT;

CREATE TABLE workspace_root_observations (
  workspace_id TEXT NOT NULL,
  workspace_root_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  observation_id TEXT NOT NULL CHECK (
    length(observation_id) = 71
    AND substr(observation_id, 1, 7) = 'sha256:'
    AND substr(observation_id, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  file_identity_digest TEXT NOT NULL CHECK (
    length(file_identity_digest) = 71
    AND substr(file_identity_digest, 1, 7) = 'sha256:'
    AND substr(file_identity_digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  resource_id TEXT,
  display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 1024),
  display_name_search_key TEXT NOT NULL CHECK (length(display_name_search_key) BETWEEN 1 AND 2048),
  size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
  modified_at TEXT,
  observed_at TEXT NOT NULL,
  completed_generation INTEGER NOT NULL CHECK (completed_generation >= 0),
  last_scan_request_id TEXT NOT NULL,
  freshness TEXT NOT NULL CHECK (freshness IN ('CURRENT', 'STALE', 'UNKNOWN')),
  PRIMARY KEY (workspace_id, workspace_root_id, runtime_incarnation_id, observation_id),
  FOREIGN KEY (workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id)
    REFERENCES workspace_root_observation_generations(
      workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id
    ),
  FOREIGN KEY (workspace_id, resource_id)
    REFERENCES resources(workspace_id, resource_id)
) STRICT;

CREATE INDEX workspace_root_observations_search
  ON workspace_root_observations(
    workspace_id, workspace_root_id, runtime_incarnation_id, freshness,
    display_name_search_key, display_name
  );

CREATE INDEX workspace_root_observations_identity
  ON workspace_root_observations(workspace_id, file_identity_digest);

-- Relative locators are private Runtime-local resolution data. They are not part of
-- the rebuildable search projection and are never serialized into events or API output.
CREATE TABLE workspace_root_observation_bindings (
  workspace_id TEXT NOT NULL,
  workspace_root_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  observation_id TEXT NOT NULL,
  private_relative_locator BLOB NOT NULL CHECK (length(private_relative_locator) BETWEEN 1 AND 32768),
  observed_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, workspace_root_id, runtime_incarnation_id, observation_id),
  FOREIGN KEY (workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id)
    REFERENCES workspace_root_observation_generations(
      workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id
    ),
  FOREIGN KEY (
    workspace_id, workspace_root_id, runtime_incarnation_id, observation_id
  ) REFERENCES workspace_root_observations(
    workspace_id, workspace_root_id, runtime_incarnation_id, observation_id
  ) ON DELETE CASCADE
) STRICT;

CREATE INDEX workspace_root_observation_bindings_runtime
  ON workspace_root_observation_bindings(runtime_id, runtime_incarnation_id, workspace_root_id);

-- This receipt is committed atomically with the public count/status receipt by the
-- owning scan transaction. It makes projection replay independent of current filesystem
-- availability after an acknowledgement is lost.
CREATE TABLE workspace_root_observation_batch_receipts (
  workspace_id TEXT NOT NULL,
  workspace_root_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  request_digest TEXT NOT NULL CHECK (
    length(request_digest) = 71
    AND substr(request_digest, 1, 7) = 'sha256:'
    AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  scan_status TEXT NOT NULL CHECK (scan_status IN ('COMPLETE', 'PARTIAL')),
  complete_generation INTEGER NOT NULL CHECK (complete_generation >= 0),
  observations_upserted INTEGER NOT NULL CHECK (observations_upserted BETWEEN 0 AND 1000),
  observations_marked_stale INTEGER NOT NULL CHECK (observations_marked_stale BETWEEN 0 AND 1000),
  committed_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, workspace_root_id, request_id),
  FOREIGN KEY (workspace_id, workspace_root_id)
    REFERENCES workspace_roots(workspace_id, workspace_root_id),
  FOREIGN KEY (runtime_id, runtime_incarnation_id)
    REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
) STRICT;

CREATE TRIGGER workspace_root_observations_delete_private_binding
AFTER UPDATE OF status ON workspace_roots
WHEN NEW.status = 'REVOKED' AND OLD.status <> 'REVOKED'
BEGIN
  DELETE FROM workspace_root_observation_bindings
  WHERE workspace_id = NEW.workspace_id AND workspace_root_id = NEW.workspace_root_id;
END;
