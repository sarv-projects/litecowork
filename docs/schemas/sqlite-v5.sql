-- Add integrity guards for the durable PlanRevision and Step model.
-- v1-v4 are immutable migration sources; fresh and existing stores both apply v5.

CREATE UNIQUE INDEX uq_steps_task_plan_logical_key
  ON steps(task_id, plan_revision, logical_key)
  WHERE logical_key IS NOT NULL;

CREATE TRIGGER plan_revision_no_update
BEFORE UPDATE ON plan_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_PLAN_REVISION');
END;

CREATE TRIGGER plan_revision_no_delete
BEFORE DELETE ON plan_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_PLAN_REVISION');
END;

CREATE TRIGGER step_identity_no_update
BEFORE UPDATE ON steps
WHEN OLD.step_id <> NEW.step_id
  OR OLD.task_id <> NEW.task_id
  OR OLD.plan_revision <> NEW.plan_revision
  OR OLD.logical_key IS NOT NEW.logical_key
  OR OLD.title <> NEW.title
  OR OLD.objective <> NEW.objective
  OR OLD.dependencies_json <> NEW.dependencies_json
  OR OLD.required_capabilities_json <> NEW.required_capabilities_json
  OR OLD.acceptance_criteria_json <> NEW.acceptance_criteria_json
  OR OLD.created_at <> NEW.created_at
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_STEP_IDENTITY');
END;

CREATE TRIGGER step_no_delete
BEFORE DELETE ON steps
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_STEP');
END;
