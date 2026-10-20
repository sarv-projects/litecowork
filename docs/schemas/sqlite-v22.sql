-- Version twenty-two pins every Evidence record emitted for a verification run to
-- that exact immutable run. PASSED remains unavailable until an authenticated
-- verifier producer and settlement path are wired through Core.
ALTER TABLE evidence
  ADD COLUMN verification_run_id TEXT REFERENCES verification_runs(verification_run_id);

CREATE INDEX evidence_verification_run
  ON evidence(task_id, verification_run_id)
  WHERE verification_run_id IS NOT NULL;

-- A run-scoped Evidence record can only be appended while its exact run is active
-- and only to the same Task. The run id is immutable with the Evidence row.
CREATE TRIGGER evidence_verification_run_binding_insert_guard
BEFORE INSERT ON evidence
WHEN NEW.verification_run_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM verification_runs run
    WHERE run.verification_run_id = NEW.verification_run_id
      AND run.task_id = NEW.task_id
      AND run.status = 'RUNNING'
  )
BEGIN
  SELECT RAISE(ABORT, 'EVIDENCE_VERIFICATION_RUN_BINDING_INVALID');
END;

-- Defense in depth for any future producer that enables PASSED: every listed ref
-- must be VERIFIED Evidence bound to this exact Task and run, no bound Evidence
-- may be omitted, and duplicate refs are rejected.
CREATE TRIGGER verification_run_pass_requires_bound_evidence
BEFORE UPDATE OF status, evidence_refs_json ON verification_runs
WHEN NEW.status = 'PASSED'
  AND CASE WHEN json_valid(NEW.evidence_refs_json) THEN (
      json_type(NEW.evidence_refs_json) <> 'array'
      OR json_array_length(NEW.evidence_refs_json) = 0
      OR json_array_length(NEW.evidence_refs_json) <> (
        SELECT COUNT(DISTINCT ref.value)
        FROM json_each(NEW.evidence_refs_json) ref
      )
      OR EXISTS (
        SELECT 1
        FROM json_each(NEW.evidence_refs_json) ref
        LEFT JOIN evidence e ON e.evidence_id = ref.value
        WHERE e.evidence_id IS NULL
          OR e.task_id <> NEW.task_id
          OR e.level <> 'VERIFIED'
          OR e.verification_run_id IS NOT NEW.verification_run_id
          OR json_extract(e.producer_json, '$.service_id') <> (
            SELECT verifier_kind FROM verification_runs
            WHERE verification_run_id = NEW.verification_run_id
          )
      )
      OR EXISTS (
        SELECT 1 FROM evidence e
        WHERE e.task_id = NEW.task_id
          AND e.verification_run_id = NEW.verification_run_id
          AND NOT EXISTS (
            SELECT 1 FROM json_each(NEW.evidence_refs_json) ref
            WHERE ref.value = e.evidence_id
          )
      )
    ) ELSE 1 END
BEGIN
  SELECT RAISE(ABORT, 'VERIFICATION_RUN_EVIDENCE_BINDING_INVALID');
END;

-- An Effect can only use the exact run that owns its selected Evidence. The older
-- v9 trigger already checks the passing run and verifier identity; this additive
-- guard closes the same-run relation now available in schema v22.
CREATE TRIGGER effect_verified_requires_bound_evidence_v22
BEFORE UPDATE ON effects
WHEN NEW.state = 'VERIFIED'
  AND NOT EXISTS (
    SELECT 1
    FROM evidence e
    JOIN verification_runs run ON run.verification_run_id = e.verification_run_id
    WHERE e.evidence_id = NEW.verification_ref
      AND e.task_id = NEW.task_id
      AND e.level = 'VERIFIED'
      AND e.subject_ref = 'effect:' || NEW.effect_id
      AND run.task_id = e.task_id
      AND run.status = 'PASSED'
      AND run.verifier_kind = json_extract(e.producer_json, '$.service_id')
      AND EXISTS (
        SELECT 1 FROM json_each(run.evidence_refs_json) ref
        WHERE ref.value = e.evidence_id
      )
  )
BEGIN
  SELECT RAISE(ABORT, 'EFFECT_VERIFICATION_RUN_EVIDENCE_BINDING_INVALID');
END;
