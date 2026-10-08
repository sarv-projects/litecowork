-- Version eight makes saved Routine revisions append-only and protects the Routine
-- aggregate's one-way ACTIVE -> ARCHIVED lifecycle from direct SQL mutations.
CREATE TRIGGER routine_revision_append_guard
BEFORE INSERT ON routine_revisions
WHEN NOT EXISTS (
  SELECT 1 FROM routines r
  WHERE r.routine_id = NEW.routine_id
    AND r.workspace_id = NEW.workspace_id
    AND r.current_revision = NEW.revision
    AND r.status = 'ACTIVE'
    AND (
      NEW.revision = 1
      OR EXISTS (
        SELECT 1 FROM routine_revisions previous
        WHERE previous.routine_id = NEW.routine_id
          AND previous.workspace_id = NEW.workspace_id
          AND previous.revision = NEW.revision - 1
      )
    )
)
BEGIN
  SELECT RAISE(ABORT, 'ROUTINE_REVISION_APPEND_INVALID');
END;

CREATE TRIGGER routine_revision_immutable_update
BEFORE UPDATE ON routine_revisions
BEGIN
  SELECT RAISE(ABORT, 'ROUTINE_REVISION_IMMUTABLE');
END;

CREATE TRIGGER routine_revision_immutable_delete
BEFORE DELETE ON routine_revisions
BEGIN
  SELECT RAISE(ABORT, 'ROUTINE_REVISION_IMMUTABLE');
END;

CREATE TRIGGER routine_head_update_guard
BEFORE UPDATE ON routines
WHEN OLD.status <> 'ACTIVE'
  OR NEW.routine_id <> OLD.routine_id
  OR NEW.workspace_id <> OLD.workspace_id
  OR NEW.name <> OLD.name
  OR NEW.created_at <> OLD.created_at
  OR NEW.version <> OLD.version + 1
  OR NOT (
    (NEW.status = 'ACTIVE' AND NEW.current_revision = OLD.current_revision + 1)
    OR (NEW.status = 'ARCHIVED' AND NEW.current_revision = OLD.current_revision)
  )
BEGIN
  SELECT RAISE(ABORT, 'ROUTINE_HEAD_TRANSITION_INVALID');
END;

CREATE TRIGGER routine_head_delete_guard
BEFORE DELETE ON routines
BEGIN
  SELECT RAISE(ABORT, 'ROUTINE_DELETE_UNSUPPORTED');
END;
