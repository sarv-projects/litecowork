-- Workspace-scoped Task delegation policy revisions and immutable TaskSpec pins.
-- Native conversation delegation remains agent-owned and is not controlled by this policy.

ALTER TABLE workspaces ADD COLUMN current_delegation_settings_revision INTEGER NOT NULL
  DEFAULT 1 CHECK (current_delegation_settings_revision >= 1);

ALTER TABLE coworker_revisions ADD COLUMN delegation_policy_source TEXT NOT NULL
  DEFAULT 'COWORKER_OVERRIDE'
  CHECK (delegation_policy_source IN ('WORKSPACE_DEFAULT', 'COWORKER_OVERRIDE'));

ALTER TABLE task_spec_revisions ADD COLUMN delegation_policy_snapshot_json TEXT
  CHECK (delegation_policy_snapshot_json IS NULL OR json_valid(delegation_policy_snapshot_json));

ALTER TABLE delegation_profile_revisions ADD COLUMN worker_model_selection_policy TEXT NOT NULL
  DEFAULT 'PROFILE_CONFIGURED'
  CHECK (worker_model_selection_policy IN ('PROFILE_CONFIGURED', 'MATCH_PARENT_IF_EXACT_ROUTE_AVAILABLE'));

CREATE TABLE workspace_delegation_settings_revisions (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  default_strategy TEXT NOT NULL
    CHECK (default_strategy IN ('NATIVE_DEFAULT', 'BALANCED', 'COST_SAVER', 'HOST_DELEGATION_ONLY')),
  enabled_delegation_profile_revisions_json TEXT NOT NULL
    CHECK (json_valid(enabled_delegation_profile_revisions_json)),
  native_child_model_policy TEXT NOT NULL
    CHECK (native_child_model_policy IN ('AGENT_DEFAULT', 'INHERIT_PARENT_IF_SUPPORTED')),
  authored_by_json TEXT NOT NULL CHECK (json_valid(authored_by_json)),
  created_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, revision),
  CHECK (length(CAST(enabled_delegation_profile_revisions_json AS BLOB)) <= 65536)
);

CREATE INDEX idx_workspace_delegation_settings_revision
  ON workspace_delegation_settings_revisions(workspace_id, revision DESC);

CREATE TRIGGER workspace_delegation_settings_revision_insert_guard
BEFORE INSERT ON workspace_delegation_settings_revisions
WHEN NEW.revision <> COALESCE((
       SELECT MAX(revision) + 1
       FROM workspace_delegation_settings_revisions
       WHERE workspace_id = NEW.workspace_id
     ), 1)
  OR CASE
       WHEN json_valid(NEW.enabled_delegation_profile_revisions_json)
       THEN json_type(NEW.enabled_delegation_profile_revisions_json)
       ELSE ''
     END <> 'array'
  OR CASE
       WHEN json_valid(NEW.enabled_delegation_profile_revisions_json)
       THEN json_array_length(NEW.enabled_delegation_profile_revisions_json)
       ELSE 65
     END > 64
  OR EXISTS (
       SELECT 1
       FROM json_each(CASE
         WHEN json_valid(NEW.enabled_delegation_profile_revisions_json)
           AND json_type(NEW.enabled_delegation_profile_revisions_json) = 'array'
         THEN NEW.enabled_delegation_profile_revisions_json ELSE '[]' END) item
       WHERE item.type <> 'object'
          OR CASE WHEN item.type = 'object'
             THEN COALESCE(json_type(item.value, '$.profile_id'), '') ELSE '' END <> 'text'
          OR CASE WHEN item.type = 'object'
             THEN COALESCE(json_type(item.value, '$.revision'), '') ELSE '' END <> 'integer'
          OR NOT EXISTS (
               SELECT 1
               FROM delegation_profiles p
               WHERE p.delegation_profile_id =
                 CASE WHEN item.type = 'object'
                   THEN json_extract(item.value, '$.profile_id') ELSE NULL END
                 AND p.workspace_id = NEW.workspace_id
                 AND p.status = 'ENABLED'
                 AND EXISTS (
                   SELECT 1
                   FROM delegation_profile_revisions profile_revision
                   WHERE profile_revision.delegation_profile_id = p.delegation_profile_id
                     AND profile_revision.workspace_id = NEW.workspace_id
                     AND profile_revision.revision =
                       CASE WHEN item.type = 'object'
                         THEN json_extract(item.value, '$.revision') ELSE NULL END
                 )
             )
          OR EXISTS (
               SELECT 1
               FROM json_each(CASE WHEN item.type = 'object' THEN item.value ELSE '{}' END) property
               WHERE property.key NOT IN ('profile_id', 'revision')
             )
     )
  OR EXISTS (
       SELECT json_extract(item.value, '$.profile_id')
       FROM json_each(CASE
         WHEN json_valid(NEW.enabled_delegation_profile_revisions_json)
           AND json_type(NEW.enabled_delegation_profile_revisions_json) = 'array'
         THEN NEW.enabled_delegation_profile_revisions_json ELSE '[]' END) item
       WHERE item.type = 'object'
       GROUP BY json_extract(item.value, '$.profile_id')
       HAVING COUNT(*) > 1
     )
BEGIN
  SELECT RAISE(ABORT, 'WORKSPACE_DELEGATION_SETTINGS_INVALID');
END;

CREATE TRIGGER workspace_delegation_settings_revision_immutable_update
BEFORE UPDATE ON workspace_delegation_settings_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_WORKSPACE_DELEGATION_SETTINGS_REVISION');
END;

CREATE TRIGGER workspace_delegation_settings_revision_immutable_delete
BEFORE DELETE ON workspace_delegation_settings_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_WORKSPACE_DELEGATION_SETTINGS_REVISION');
END;

-- Backfill a deterministic native-only policy for existing Workspaces. The ceiling
-- includes each currently enabled profile at its exact head so existing Coworker
-- overrides remain within the ceiling; NATIVE_DEFAULT does not invoke those profiles.
INSERT INTO workspace_delegation_settings_revisions (
  workspace_id, revision, default_strategy,
  enabled_delegation_profile_revisions_json, native_child_model_policy,
  authored_by_json, created_at
)
SELECT
  w.workspace_id,
  1,
  'NATIVE_DEFAULT',
  COALESCE((
    SELECT json_group_array(json_object('profile_id', ordered.delegation_profile_id,
                                        'revision', ordered.current_revision))
    FROM (
      SELECT p.delegation_profile_id, p.current_revision
      FROM delegation_profiles p
      WHERE p.workspace_id = w.workspace_id AND p.status = 'ENABLED'
      ORDER BY p.delegation_profile_id
    ) ordered
  ), '[]'),
  'AGENT_DEFAULT',
  json_object('kind', 'USER', 'principal_id', w.owner_principal_id),
  w.created_at
FROM workspaces w;

CREATE TRIGGER workspace_delegation_settings_head_insert_guard
BEFORE INSERT ON workspaces
WHEN NEW.current_delegation_settings_revision <> 1
BEGIN
  SELECT RAISE(ABORT, 'WORKSPACE_DELEGATION_SETTINGS_HEAD_INVALID');
END;

CREATE TRIGGER workspace_delegation_settings_default_on_insert
AFTER INSERT ON workspaces
BEGIN
  INSERT INTO workspace_delegation_settings_revisions (
    workspace_id, revision, default_strategy,
    enabled_delegation_profile_revisions_json, native_child_model_policy,
    authored_by_json, created_at
  )
  VALUES (
    NEW.workspace_id, 1, 'NATIVE_DEFAULT', '[]', 'AGENT_DEFAULT',
    json_object('kind', 'USER', 'principal_id', NEW.owner_principal_id),
    NEW.created_at
  );
END;

CREATE TRIGGER workspace_delegation_settings_head_guard
BEFORE UPDATE OF current_delegation_settings_revision ON workspaces
WHEN NEW.current_delegation_settings_revision <> OLD.current_delegation_settings_revision
  AND (
    NEW.current_delegation_settings_revision <> OLD.current_delegation_settings_revision + 1
    OR NEW.version <> OLD.version + 1
    OR NOT EXISTS (
      SELECT 1
      FROM workspace_delegation_settings_revisions r
      WHERE r.workspace_id = NEW.workspace_id
        AND r.revision = NEW.current_delegation_settings_revision
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'WORKSPACE_DELEGATION_SETTINGS_HEAD_INVALID');
END;

CREATE TRIGGER coworker_revision_delegation_source_guard
BEFORE INSERT ON coworker_revisions
WHEN (
    NEW.delegation_policy_source = 'WORKSPACE_DEFAULT'
    AND (
      NEW.delegation_strategy <> 'NATIVE_DEFAULT'
      OR NEW.enabled_delegation_profile_ids_json <> '[]'
    )
  )
  OR (
    NEW.delegation_policy_source = 'COWORKER_OVERRIDE'
    AND EXISTS (
      SELECT 1
      FROM json_each(NEW.enabled_delegation_profile_ids_json) selected
      LEFT JOIN workspaces w ON w.workspace_id = NEW.workspace_id
      LEFT JOIN workspace_delegation_settings_revisions s
        ON s.workspace_id = w.workspace_id
       AND s.revision = w.current_delegation_settings_revision
      LEFT JOIN json_each(CASE
        WHEN s.enabled_delegation_profile_revisions_json IS NOT NULL
        THEN s.enabled_delegation_profile_revisions_json ELSE '[]' END) allowed
        ON json_extract(allowed.value, '$.profile_id') = selected.value
      WHERE selected.type <> 'text' OR allowed.value IS NULL
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'COWORKER_DELEGATION_POLICY_OUTSIDE_WORKSPACE_CEILING');
END;

CREATE TRIGGER task_spec_delegation_policy_snapshot_guard
BEFORE INSERT ON task_spec_revisions
WHEN NEW.delegation_policy_snapshot_json IS NULL
  OR CASE
       WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN json_type(NEW.delegation_policy_snapshot_json)
       ELSE ''
     END <> 'object'
  OR COALESCE(json_extract(NEW.delegation_policy_snapshot_json, '$.source'), '')
       NOT IN ('WORKSPACE_DEFAULT', 'COWORKER_OVERRIDE')
  OR COALESCE(json_extract(NEW.delegation_policy_snapshot_json, '$.strategy'), '')
       NOT IN ('NATIVE_DEFAULT', 'BALANCED', 'COST_SAVER', 'HOST_DELEGATION_ONLY')
  OR COALESCE(json_extract(NEW.delegation_policy_snapshot_json, '$.native_child_model_policy'), '')
       NOT IN ('AGENT_DEFAULT', 'INHERIT_PARENT_IF_SUPPORTED')
  OR COALESCE(json_type(NEW.delegation_policy_snapshot_json, '$.worker_profiles'), '') <> 'array'
  OR json_array_length(NEW.delegation_policy_snapshot_json, '$.worker_profiles') > 64
  OR NOT EXISTS (
       SELECT 1
       FROM workspace_delegation_settings_revisions settings
       WHERE settings.workspace_id = NEW.workspace_id
         AND settings.revision =
           json_extract(NEW.delegation_policy_snapshot_json, '$.workspace_settings_revision')
     )
BEGIN
  SELECT RAISE(ABORT, 'TASK_DELEGATION_POLICY_SNAPSHOT_INVALID');
END;
