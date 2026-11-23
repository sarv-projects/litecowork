-- Version thirty-four validates the one canonical OpenAPI snapshot shape. Migration
-- code enriches non-null v28 snapshots from their immutable source rows before this
-- guard replaces the weaker v28 insert check.
DROP TRIGGER IF EXISTS task_spec_delegation_policy_snapshot_guard;

CREATE TRIGGER task_spec_delegation_policy_snapshot_guard
BEFORE INSERT ON task_spec_revisions
WHEN NEW.delegation_policy_snapshot_json IS NULL
  OR CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN json_type(NEW.delegation_policy_snapshot_json) ELSE '' END <> 'object'
  OR (SELECT COUNT(*) FROM json_each(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END)) <> 8
  OR EXISTS (
       SELECT 1 FROM json_each(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
         THEN NEW.delegation_policy_snapshot_json ELSE '{}' END) property
       WHERE property.key NOT IN ('source', 'workspace_settings_revision', 'coworker_id',
         'coworker_revision', 'strategy', 'native_child_model_policy', 'worker_profiles', 'resolved_at')
     )
  OR COALESCE(json_extract(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.source'), '')
       NOT IN ('WORKSPACE_DEFAULT', 'COWORKER_OVERRIDE')
  OR COALESCE(json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.workspace_settings_revision'), '') <> 'integer'
  OR COALESCE(json_extract(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.workspace_settings_revision'), 0) < 1
  OR COALESCE(json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.coworker_id'), '') NOT IN ('null', 'text')
  OR COALESCE(json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.coworker_revision'), '') NOT IN ('null', 'integer')
  OR COALESCE(json_extract(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.strategy'), '')
       NOT IN ('NATIVE_DEFAULT', 'BALANCED', 'COST_SAVER', 'HOST_DELEGATION_ONLY')
  OR COALESCE(json_extract(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.native_child_model_policy'), '')
       NOT IN ('AGENT_DEFAULT', 'INHERIT_PARENT_IF_SUPPORTED')
  OR COALESCE(json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.worker_profiles'), '') <> 'array'
  OR json_array_length(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{"worker_profiles":[]}' END, '$.worker_profiles') > 64
  OR COALESCE(json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.resolved_at'), '') <> 'text'
  OR length(trim(COALESCE(json_extract(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.resolved_at'), ''))) = 0
  OR (json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.coworker_id') = 'null'
      AND json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.coworker_revision') <> 'null')
  OR (json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.coworker_id') = 'text'
      AND json_type(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
       THEN NEW.delegation_policy_snapshot_json ELSE '{}' END, '$.coworker_revision') <> 'integer')
  OR EXISTS (
       SELECT 1 FROM json_each(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
         AND json_type(NEW.delegation_policy_snapshot_json, '$.worker_profiles') = 'array'
         THEN json_extract(NEW.delegation_policy_snapshot_json, '$.worker_profiles') ELSE '[]' END) profile
       WHERE profile.type <> 'object'
          OR (SELECT COUNT(*) FROM json_each(CASE WHEN profile.type = 'object' THEN profile.value ELSE '{}' END)) <> 3
          OR EXISTS (SELECT 1 FROM json_each(CASE WHEN profile.type = 'object' THEN profile.value ELSE '{}' END) field
             WHERE field.key NOT IN ('profile_id', 'revision', 'worker_model_selection_policy'))
          OR COALESCE(json_type(profile.value, '$.profile_id'), '') <> 'text'
          OR length(trim(COALESCE(json_extract(profile.value, '$.profile_id'), ''))) = 0
          OR COALESCE(json_type(profile.value, '$.revision'), '') <> 'integer'
          OR COALESCE(json_extract(profile.value, '$.revision'), 0) < 1
          OR COALESCE(json_extract(profile.value, '$.worker_model_selection_policy'), '')
             NOT IN ('PROFILE_CONFIGURED', 'MATCH_PARENT_IF_EXACT_ROUTE_AVAILABLE')
     )
  OR EXISTS (
       SELECT json_extract(profile.value, '$.profile_id')
       FROM json_each(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
         AND json_type(NEW.delegation_policy_snapshot_json, '$.worker_profiles') = 'array'
         THEN json_extract(NEW.delegation_policy_snapshot_json, '$.worker_profiles') ELSE '[]' END) profile
       GROUP BY json_extract(profile.value, '$.profile_id') HAVING COUNT(*) > 1
     )
  OR (json_extract(NEW.delegation_policy_snapshot_json, '$.strategy') = 'NATIVE_DEFAULT'
      AND json_array_length(NEW.delegation_policy_snapshot_json, '$.worker_profiles') <> 0)
  OR NOT EXISTS (
       SELECT 1 FROM workspace_delegation_settings_revisions settings
       WHERE settings.workspace_id = NEW.workspace_id
         AND settings.revision = json_extract(NEW.delegation_policy_snapshot_json, '$.workspace_settings_revision')
         AND settings.native_child_model_policy = json_extract(NEW.delegation_policy_snapshot_json, '$.native_child_model_policy')
         AND (
           (json_extract(NEW.delegation_policy_snapshot_json, '$.source') = 'WORKSPACE_DEFAULT'
             AND settings.default_strategy = json_extract(NEW.delegation_policy_snapshot_json, '$.strategy'))
           OR EXISTS (
             SELECT 1 FROM tasks t JOIN coworker_revisions c
               ON c.workspace_id = t.workspace_id AND c.coworker_id = t.origin_coworker_id
              AND c.revision = t.origin_coworker_revision
             WHERE t.task_id = NEW.task_id AND t.workspace_id = NEW.workspace_id
               AND t.origin_coworker_id IS NOT NULL
               AND json_extract(NEW.delegation_policy_snapshot_json, '$.coworker_id') = t.origin_coworker_id
               AND json_extract(NEW.delegation_policy_snapshot_json, '$.coworker_revision') = t.origin_coworker_revision
               AND c.delegation_policy_source = json_extract(NEW.delegation_policy_snapshot_json, '$.source')
               AND (c.delegation_policy_source = 'WORKSPACE_DEFAULT'
                 OR c.delegation_strategy = json_extract(NEW.delegation_policy_snapshot_json, '$.strategy'))
           )
         )
     )
  OR NOT EXISTS (
       SELECT 1 FROM tasks t WHERE t.task_id = NEW.task_id AND t.workspace_id = NEW.workspace_id
         AND t.origin_coworker_id IS json_extract(NEW.delegation_policy_snapshot_json, '$.coworker_id')
         AND t.origin_coworker_revision IS json_extract(NEW.delegation_policy_snapshot_json, '$.coworker_revision')
         AND (t.origin_coworker_id IS NOT NULL
              OR json_extract(NEW.delegation_policy_snapshot_json, '$.source') = 'WORKSPACE_DEFAULT')
     )
  OR EXISTS (
       SELECT 1 FROM tasks t JOIN coworker_revisions c
         ON c.workspace_id = t.workspace_id AND c.coworker_id = t.origin_coworker_id
        AND c.revision = t.origin_coworker_revision
       WHERE t.task_id = NEW.task_id AND t.workspace_id = NEW.workspace_id
         AND t.origin_coworker_id IS NOT NULL
         AND c.delegation_policy_source <> json_extract(NEW.delegation_policy_snapshot_json, '$.source')
     )
  OR EXISTS (
       SELECT 1 FROM json_each(CASE WHEN json_valid(NEW.delegation_policy_snapshot_json)
         AND json_type(NEW.delegation_policy_snapshot_json, '$.worker_profiles') = 'array'
         THEN json_extract(NEW.delegation_policy_snapshot_json, '$.worker_profiles') ELSE '[]' END) selected
       WHERE NOT EXISTS (
         SELECT 1 FROM workspace_delegation_settings_revisions settings,
           json_each(settings.enabled_delegation_profile_revisions_json) allowed
         WHERE settings.workspace_id = NEW.workspace_id
           AND settings.revision = json_extract(NEW.delegation_policy_snapshot_json, '$.workspace_settings_revision')
           AND json_extract(allowed.value, '$.profile_id') = json_extract(selected.value, '$.profile_id')
           AND json_extract(allowed.value, '$.revision') = json_extract(selected.value, '$.revision')
       ) OR NOT EXISTS (
         SELECT 1 FROM delegation_profile_revisions profile
         WHERE profile.workspace_id = NEW.workspace_id
           AND profile.delegation_profile_id = json_extract(selected.value, '$.profile_id')
           AND profile.revision = json_extract(selected.value, '$.revision')
           AND profile.worker_model_selection_policy = json_extract(selected.value, '$.worker_model_selection_policy')
       )
       OR (json_extract(NEW.delegation_policy_snapshot_json, '$.source') = 'COWORKER_OVERRIDE'
           AND NOT EXISTS (
             SELECT 1 FROM tasks t JOIN coworker_revisions c
               ON c.workspace_id = t.workspace_id AND c.coworker_id = t.origin_coworker_id
              AND c.revision = t.origin_coworker_revision,
               json_each(c.enabled_delegation_profile_ids_json) coworker_profile
             WHERE t.task_id = NEW.task_id AND t.workspace_id = NEW.workspace_id
               AND coworker_profile.value = json_extract(selected.value, '$.profile_id')
           ))
     )
  OR (json_extract(NEW.delegation_policy_snapshot_json, '$.source') = 'WORKSPACE_DEFAULT'
      AND json_extract(NEW.delegation_policy_snapshot_json, '$.strategy') <> 'NATIVE_DEFAULT'
      AND json_array_length(NEW.delegation_policy_snapshot_json, '$.worker_profiles') <> (
        SELECT json_array_length(settings.enabled_delegation_profile_revisions_json)
        FROM workspace_delegation_settings_revisions settings
        WHERE settings.workspace_id = NEW.workspace_id
          AND settings.revision = json_extract(NEW.delegation_policy_snapshot_json, '$.workspace_settings_revision')
      ))
  OR (json_extract(NEW.delegation_policy_snapshot_json, '$.source') = 'COWORKER_OVERRIDE'
      AND json_extract(NEW.delegation_policy_snapshot_json, '$.strategy') <> 'NATIVE_DEFAULT'
      AND json_array_length(NEW.delegation_policy_snapshot_json, '$.worker_profiles') <> (
        SELECT json_array_length(c.enabled_delegation_profile_ids_json)
        FROM tasks t JOIN coworker_revisions c
          ON c.workspace_id = t.workspace_id AND c.coworker_id = t.origin_coworker_id
         AND c.revision = t.origin_coworker_revision
        WHERE t.task_id = NEW.task_id AND t.workspace_id = NEW.workspace_id
      ))
BEGIN
  SELECT RAISE(ABORT, 'TASK_DELEGATION_POLICY_SNAPSHOT_INVALID');
END;

CREATE TRIGGER task_spec_delegation_policy_snapshot_immutable
BEFORE UPDATE OF delegation_policy_snapshot_json ON task_spec_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_TASK_DELEGATION_POLICY_SNAPSHOT');
END;
