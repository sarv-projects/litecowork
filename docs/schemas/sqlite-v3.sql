-- Additive migration from immutable SQLite schema v2.
-- RuntimeWorkspaceBinding makes Workspace authorization explicit. Legacy v1 composite
-- foreign keys still limit each Runtime row to its existing Workspace; removing those
-- constraints requires a later dependency-aware table rebuild before multi-Workspace
-- Runtime use is advertised.

CREATE TABLE runtime_workspace_bindings (
  runtime_workspace_binding_id TEXT PRIMARY KEY,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  enrollment_mode TEXT NOT NULL CHECK (enrollment_mode IN ('LOCAL_ENROLLMENT', 'MESH_PAIRING')),
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'ACTIVE', 'REVOKED')),
  roles_json TEXT NOT NULL CHECK (json_valid(roles_json) AND json_type(roles_json) = 'array'),
  created_at TEXT NOT NULL,
  activated_at TEXT,
  revoked_at TEXT,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
  CHECK ((status = 'PENDING' AND activated_at IS NULL AND revoked_at IS NULL)
      OR (status = 'ACTIVE' AND activated_at IS NOT NULL AND revoked_at IS NULL)
      OR (status = 'REVOKED' AND revoked_at IS NOT NULL)),
  UNIQUE(runtime_workspace_binding_id, runtime_id, workspace_id)
);

CREATE UNIQUE INDEX idx_runtime_workspace_one_open_binding
  ON runtime_workspace_bindings(runtime_id, workspace_id)
  WHERE status IN ('PENDING', 'ACTIVE');
CREATE INDEX idx_runtime_workspace_bindings_workspace
  ON runtime_workspace_bindings(workspace_id, status, runtime_id);

-- Existing records were created by the authenticated Workspace-scoped pairing flow.
-- Preserve global device revocation while migrating active/recoverable device records.
INSERT INTO runtime_workspace_bindings (
  runtime_workspace_binding_id, runtime_id, workspace_id, enrollment_mode, status,
  roles_json, created_at, activated_at, revoked_at, version
)
SELECT
  'rwb_migrated_' || runtime_id,
  runtime_id,
  workspace_id,
  'MESH_PAIRING',
  CASE WHEN availability = 'REVOKED' THEN 'REVOKED' ELSE 'ACTIVE' END,
  roles_json,
  last_seen,
  CASE WHEN availability = 'REVOKED' THEN NULL ELSE last_seen END,
  CASE WHEN availability = 'REVOKED' THEN last_seen ELSE NULL END,
  1
FROM runtimes;

CREATE TRIGGER runtime_workspace_binding_initial_version_guard
BEFORE INSERT ON runtime_workspace_bindings
WHEN NEW.version <> 1
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_INITIAL_VERSION_INVALID');
END;

CREATE TRIGGER runtime_workspace_binding_immutable_identity_guard
BEFORE UPDATE OF runtime_workspace_binding_id, runtime_id, workspace_id,
  enrollment_mode, roles_json, created_at ON runtime_workspace_bindings
WHEN NEW.runtime_workspace_binding_id IS NOT OLD.runtime_workspace_binding_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.enrollment_mode IS NOT OLD.enrollment_mode
  OR NEW.roles_json IS NOT OLD.roles_json
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER runtime_workspace_binding_transition_guard
BEFORE UPDATE OF status, activated_at, revoked_at, version ON runtime_workspace_bindings
WHEN NOT (
  (OLD.status = 'PENDING' AND NEW.status = 'ACTIVE'
    AND OLD.activated_at IS NULL AND NEW.activated_at IS NOT NULL
    AND OLD.revoked_at IS NULL AND NEW.revoked_at IS NULL
    AND NEW.version = OLD.version + 1)
  OR (OLD.status = 'PENDING' AND NEW.status = 'REVOKED'
    AND OLD.activated_at IS NULL AND NEW.activated_at IS NULL
    AND OLD.revoked_at IS NULL AND NEW.revoked_at IS NOT NULL
    AND NEW.version = OLD.version + 1)
  OR (OLD.status = 'ACTIVE' AND NEW.status = 'REVOKED'
    AND OLD.activated_at IS NEW.activated_at
    AND OLD.revoked_at IS NULL AND NEW.revoked_at IS NOT NULL
    AND NEW.version = OLD.version + 1)
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_TRANSITION_INVALID');
END;

DROP TRIGGER attempt_environment_owner_guard;
CREATE TRIGGER attempt_environment_owner_guard
BEFORE INSERT ON attempts
WHEN NOT EXISTS (
  SELECT 1
  FROM tasks t
  JOIN environments e ON e.environment_id = NEW.environment_id
  JOIN agent_bindings b ON b.agent_binding_id = NEW.agent_binding_id
  JOIN runtimes r ON r.runtime_id = NEW.runtime_id
  JOIN runtime_workspace_bindings rwb
    ON rwb.runtime_id = NEW.runtime_id
   AND rwb.workspace_id = t.workspace_id
   AND rwb.status = 'ACTIVE'
  WHERE t.task_id = NEW.task_id
    AND e.runtime_id = NEW.runtime_id
    AND e.owner_workspace_id = t.workspace_id
    AND (e.owner_task_id IS NULL OR e.owner_task_id = t.task_id)
    AND (e.sharing_scope <> 'ATTEMPT_PRIVATE' OR e.owner_attempt_id = NEW.attempt_id)
    AND (e.sharing_scope <> 'TASK_SHARED' OR e.owner_task_id = t.task_id)
    AND (e.sharing_scope <> 'COWORKER_PRIVATE' OR e.owner_coworker_id = t.origin_coworker_id)
    AND b.workspace_id = t.workspace_id
    AND r.workspace_id = t.workspace_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'ATTEMPT_ENVIRONMENT_SCOPE_MISMATCH');
END;
