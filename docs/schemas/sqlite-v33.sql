-- Feature 005 T01 (owner-directed revision 1.1, cross-owner review pending).
-- Bind every newly reserved Task AgentSession to its already-prepared Environment.
-- Conversation sessions remain outside Task Environments. This migration does not
-- create an Environment, Attempt, lease, Effect, grant, or ApprovalUse record.

ALTER TABLE agent_sessions
  ADD COLUMN environment_id TEXT REFERENCES environments(environment_id);

CREATE INDEX idx_agent_sessions_environment_id
  ON agent_sessions(environment_id);

DROP TRIGGER agent_session_identity_immutable;
CREATE TRIGGER agent_session_identity_immutable
BEFORE UPDATE OF workspace_id, scope_kind, conversation_id, conversation_turn_id,
  task_id, task_spec_revision, attempt_id, endpoint_id, runtime_id,
  runtime_incarnation_id, agent_binding_id, configuration_digest, environment_id
ON agent_sessions
WHEN (
    OLD.workspace_id <> NEW.workspace_id
    OR OLD.scope_kind <> NEW.scope_kind
    OR OLD.conversation_id IS NOT NEW.conversation_id
    OR OLD.conversation_turn_id IS NOT NEW.conversation_turn_id
    OR OLD.task_id IS NOT NEW.task_id
    OR OLD.task_spec_revision IS NOT NEW.task_spec_revision
    OR OLD.attempt_id IS NOT NEW.attempt_id
    OR OLD.endpoint_id <> NEW.endpoint_id
    OR OLD.runtime_id <> NEW.runtime_id
    OR OLD.runtime_incarnation_id <> NEW.runtime_incarnation_id
    OR OLD.agent_binding_id <> NEW.agent_binding_id
    OR OLD.configuration_digest IS NOT NEW.configuration_digest
    OR OLD.environment_id IS NOT NEW.environment_id
  )
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_AGENT_SESSION_IDENTITY');
END;

CREATE TRIGGER agent_session_environment_scope_guard_insert
BEFORE INSERT ON agent_sessions
WHEN (NEW.scope_kind = 'CONVERSATION' AND NEW.environment_id IS NOT NULL)
  OR (NEW.scope_kind IN ('TASK_PLANNING', 'ATTEMPT_EXECUTION')
      AND NEW.status = 'STARTING' AND NEW.environment_id IS NULL)
  OR (NEW.environment_id IS NOT NULL AND NOT EXISTS (
    SELECT 1
    FROM environments e
    WHERE e.environment_id = NEW.environment_id
      AND e.owner_workspace_id = NEW.workspace_id
      AND e.owner_task_id = NEW.task_id
      AND e.runtime_id = NEW.runtime_id
      AND e.created_by_incarnation_id = NEW.runtime_incarnation_id
      AND e.status IN ('READY', 'BUSY')
      AND (
        (NEW.scope_kind = 'TASK_PLANNING'
          AND e.lifetime = 'TASK_RETAINED'
          AND e.sharing_scope = 'TASK_SHARED'
          AND e.owner_attempt_id IS NULL)
        OR
        (NEW.scope_kind = 'ATTEMPT_EXECUTION'
          AND e.lifetime = 'ATTEMPT'
          AND e.sharing_scope = 'ATTEMPT_PRIVATE'
          AND e.owner_attempt_id = NEW.attempt_id
          AND EXISTS (
            SELECT 1
            FROM attempts a
            WHERE a.task_id = NEW.task_id
              AND a.attempt_id = NEW.attempt_id
              AND a.agent_session_id = NEW.agent_session_id
              AND a.environment_id = NEW.environment_id
              AND a.runtime_id = NEW.runtime_id
              AND a.runtime_incarnation_id = NEW.runtime_incarnation_id
          ))
      )
  ))
BEGIN
  SELECT RAISE(ABORT, 'AGENT_SESSION_ENVIRONMENT_SCOPE_MISMATCH');
END;

CREATE TRIGGER agent_session_environment_scope_guard_start
BEFORE UPDATE OF status ON agent_sessions
WHEN NEW.status = 'STARTING'
  AND NEW.scope_kind IN ('TASK_PLANNING', 'ATTEMPT_EXECUTION')
  AND (NEW.environment_id IS NULL OR NOT EXISTS (
    SELECT 1
    FROM environments e
    WHERE e.environment_id = NEW.environment_id
      AND e.owner_workspace_id = NEW.workspace_id
      AND e.owner_task_id = NEW.task_id
      AND e.runtime_id = NEW.runtime_id
      AND e.created_by_incarnation_id = NEW.runtime_incarnation_id
      AND e.status IN ('READY', 'BUSY')
      AND (
        (NEW.scope_kind = 'TASK_PLANNING'
          AND e.lifetime = 'TASK_RETAINED'
          AND e.sharing_scope = 'TASK_SHARED'
          AND e.owner_attempt_id IS NULL)
        OR
        (NEW.scope_kind = 'ATTEMPT_EXECUTION'
          AND e.lifetime = 'ATTEMPT'
          AND e.sharing_scope = 'ATTEMPT_PRIVATE'
          AND e.owner_attempt_id = NEW.attempt_id
          AND EXISTS (
            SELECT 1
            FROM attempts a
            WHERE a.task_id = NEW.task_id
              AND a.attempt_id = NEW.attempt_id
              AND a.agent_session_id = NEW.agent_session_id
              AND a.environment_id = NEW.environment_id
              AND a.runtime_id = NEW.runtime_id
              AND a.runtime_incarnation_id = NEW.runtime_incarnation_id
          ))
      )
  ))
BEGIN
  SELECT RAISE(ABORT, 'AGENT_SESSION_ENVIRONMENT_SCOPE_MISMATCH');
END;
