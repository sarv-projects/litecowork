-- Version twenty gives VerificationRuns an aggregate CAS version and typed,
-- non-sensitive terminal failure classification. Verifiers still cannot mark a run
-- PASSED through the SQLite adapter until trusted Evidence binding is integrated.
ALTER TABLE verification_runs
  ADD COLUMN version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1);

ALTER TABLE verification_runs
  ADD COLUMN failure_code TEXT CHECK (
    failure_code IS NULL
    OR failure_code IN (
      'TIMEOUT',
      'INTEGRITY_FAILURE',
      'VERIFIER_UNAVAILABLE',
      'UNSUPPORTED_CRITERION',
      'INPUT_UNAVAILABLE',
      'TASK_CANCELLED'
    )
  );

-- A new run is inserted as PENDING and transitions to RUNNING inside the same
-- immediate transaction that appends its started event and replay receipt.
CREATE TRIGGER verification_run_insert_guard
BEFORE INSERT ON verification_runs
WHEN NEW.status <> 'PENDING'
  OR NEW.version <> 1
  OR NEW.started_at IS NOT NULL
  OR NEW.completed_at IS NOT NULL
  OR NEW.evidence_refs_json <> '[]'
  OR NEW.failure_code IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'VERIFICATION_RUN_MUST_START_PENDING');
END;

-- Bindings are immutable. Only PENDING→RUNNING and RUNNING→failure/inconclusive
-- are legal until the trusted same-run Evidence producer is integrated. Every
-- transition advances one aggregate revision.
CREATE TRIGGER verification_run_transition_guard
BEFORE UPDATE ON verification_runs
WHEN NEW.verification_run_id IS NOT OLD.verification_run_id
  OR NEW.task_id IS NOT OLD.task_id
  OR NEW.criterion_id IS NOT OLD.criterion_id
  OR NEW.task_spec_revision IS NOT OLD.task_spec_revision
  OR NEW.criterion_digest IS NOT OLD.criterion_digest
  OR NEW.verifier_kind IS NOT OLD.verifier_kind
  OR NEW.verifier_version IS NOT OLD.verifier_version
  OR NEW.subject_refs_json IS NOT OLD.subject_refs_json
  OR NEW.inputs_json IS NOT OLD.inputs_json
  OR NOT (
    (
      OLD.status = 'PENDING'
      AND NEW.status = 'RUNNING'
      AND NEW.version = OLD.version + 1
      AND OLD.started_at IS NULL
      AND NEW.started_at IS NOT NULL
      AND NEW.completed_at IS NULL
      AND NEW.evidence_refs_json = '[]'
      AND NEW.failure_code IS NULL
    )
    OR
    (
      OLD.status = 'RUNNING'
      AND NEW.status IN ('FAILED', 'INCONCLUSIVE')
      AND NEW.version = OLD.version + 1
      AND NEW.started_at IS OLD.started_at
      AND NEW.completed_at IS NOT NULL
      AND NEW.failure_code IS NOT NULL
      AND NEW.failure_code IN (
        'TIMEOUT',
        'INTEGRITY_FAILURE',
        'VERIFIER_UNAVAILABLE',
        'UNSUPPORTED_CRITERION',
        'INPUT_UNAVAILABLE',
        'TASK_CANCELLED'
      )
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'INVALID_VERIFICATION_RUN_TRANSITION');
END;

CREATE TRIGGER verification_run_terminal_immutable
BEFORE UPDATE ON verification_runs
WHEN OLD.status IN ('PASSED', 'FAILED', 'INCONCLUSIVE')
BEGIN
  SELECT RAISE(ABORT, 'VERIFICATION_RUN_TERMINAL_IMMUTABLE');
END;

CREATE TRIGGER verification_run_delete_guard
BEFORE DELETE ON verification_runs
BEGIN
  SELECT RAISE(ABORT, 'VERIFICATION_RUN_IMMUTABLE');
END;

CREATE INDEX idx_verification_runs_task_status_time
  ON verification_runs(task_id, status, started_at DESC, verification_run_id DESC);
