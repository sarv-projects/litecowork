-- Automation test runs are ordinary Task admissions pinned to an exact
-- AutomationRevision. A test run is not an AutomationOccurrence and it never
-- advances a trigger cursor. The row is inserted in the same transaction as
-- the Task, links to that Task once, and then becomes immutable.
CREATE TABLE automation_test_runs (
  test_run_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  command_key TEXT NOT NULL CHECK (length(command_key) = 64),
  automation_id TEXT NOT NULL,
  automation_revision INTEGER NOT NULL,
  routine_id TEXT NOT NULL,
  routine_revision INTEGER NOT NULL,
  task_id TEXT REFERENCES tasks(task_id) DEFERRABLE INITIALLY DEFERRED,
  requested_by_json TEXT NOT NULL CHECK (json_valid(requested_by_json)),
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version IN (1, 2)),
  UNIQUE(workspace_id, command_key),
  UNIQUE(task_id),
  FOREIGN KEY(automation_id, automation_revision, workspace_id)
    REFERENCES automation_revisions(automation_id, revision, workspace_id),
  FOREIGN KEY(routine_id, routine_revision, workspace_id)
    REFERENCES routine_revisions(routine_id, revision, workspace_id)
);

-- The new Task provenance column is nullable. Existing occurrence Tasks keep
-- the column NULL; only an atomic automation_test_admission may set it.
ALTER TABLE tasks ADD COLUMN automation_test_run_id TEXT
  REFERENCES automation_test_runs(test_run_id);

CREATE TRIGGER automation_test_run_insert_guard
BEFORE INSERT ON automation_test_runs
WHEN NEW.task_id IS NOT NULL
  OR NEW.version != 1
  OR NEW.test_run_id = ''
  OR NEW.command_key = ''
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_TEST_RUN_INVALID');
END;

CREATE TRIGGER automation_test_run_link_guard
BEFORE UPDATE ON automation_test_runs
WHEN OLD.task_id IS NOT NULL
  OR NEW.task_id IS NULL
  OR NEW.version != OLD.version + 1
  OR NEW.test_run_id != OLD.test_run_id
  OR NEW.workspace_id != OLD.workspace_id
  OR NEW.command_key != OLD.command_key
  OR NEW.automation_id != OLD.automation_id
  OR NEW.automation_revision != OLD.automation_revision
  OR NEW.routine_id != OLD.routine_id
  OR NEW.routine_revision != OLD.routine_revision
  OR NEW.requested_by_json != OLD.requested_by_json
  OR NEW.created_at != OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_TEST_RUN_TRANSITION_INVALID');
END;

-- A test Task must reference a reserved, unlinked test run with the same
-- Workspace and Routine pins, and it must not also carry occurrence
-- provenance. The Task row is inserted after the reservation row and the
-- reservation is linked immediately after the Task insert.
CREATE TRIGGER task_automation_test_run_guard
BEFORE INSERT ON tasks
WHEN NEW.automation_test_run_id IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'TASK_AUTOMATION_TEST_RUN_INVALID')
  WHERE NEW.automation_id IS NOT NULL
     OR NEW.automation_occurrence_id IS NOT NULL
     OR NOT EXISTS (
       SELECT 1 FROM automation_test_runs t
       WHERE t.test_run_id = NEW.automation_test_run_id
         AND t.workspace_id = NEW.workspace_id
         AND t.task_id IS NULL
         AND t.routine_id = NEW.routine_id
         AND t.routine_revision = NEW.routine_revision
     );
END;

CREATE TRIGGER task_automation_test_run_immutable
BEFORE UPDATE OF automation_test_run_id ON tasks
WHEN NEW.automation_test_run_id IS NOT OLD.automation_test_run_id
BEGIN
  SELECT RAISE(ABORT, 'TASK_AUTOMATION_TEST_RUN_IMMUTABLE');
END;

CREATE TRIGGER task_automation_test_occurrence_exclusive
BEFORE UPDATE OF automation_id, automation_occurrence_id ON tasks
WHEN NEW.automation_test_run_id IS NOT NULL
  AND (NEW.automation_id IS NOT NULL OR NEW.automation_occurrence_id IS NOT NULL)
BEGIN
  SELECT RAISE(ABORT, 'TASK_AUTOMATION_PIN_INVALID');
END;
