-- Version nine enforces the documented Effect lifecycle and append-only Evidence
-- invariant for direct SQLite writers as well as the application adapter.
CREATE TRIGGER effect_initial_proposal_guard
BEFORE INSERT ON effects
WHEN NEW.state <> 'PROPOSED' OR NEW.dispatch_ordinal <> 0 OR NEW.version <> 1
BEGIN
  SELECT RAISE(ABORT, 'EFFECT_MUST_BEGIN_AS_PROPOSED');
END;

CREATE TRIGGER effect_transition_guard
BEFORE UPDATE ON effects
WHEN NEW.effect_id <> OLD.effect_id
  OR NEW.task_id <> OLD.task_id
  OR NEW.attempt_id <> OLD.attempt_id
  OR NEW.capability_ref_json IS NOT OLD.capability_ref_json
  OR NEW.operation <> OLD.operation
  OR NEW.target_json <> OLD.target_json
  OR NEW.idempotency_key IS NOT OLD.idempotency_key
  OR NEW.request_digest <> OLD.request_digest
  OR NEW.capability_invocation_id <> OLD.capability_invocation_id
  OR NEW.execution_method <> OLD.execution_method
  OR NEW.created_at <> OLD.created_at
  OR NEW.version <> OLD.version + 1
  OR NOT (
    (OLD.state = 'PROPOSED' AND NEW.state IN ('STARTED', 'FAILED'))
    OR (OLD.state = 'STARTED' AND NEW.state IN ('ACKNOWLEDGED', 'FAILED', 'AMBIGUOUS'))
    OR (OLD.state = 'ACKNOWLEDGED' AND NEW.state IN ('OBSERVED', 'FAILED', 'AMBIGUOUS'))
    OR (OLD.state = 'OBSERVED' AND NEW.state IN ('VERIFIED', 'AMBIGUOUS'))
    OR (OLD.state = 'AMBIGUOUS' AND NEW.state = 'RECONCILING')
    OR (OLD.state = 'RECONCILING' AND NEW.state IN ('OBSERVED', 'FAILED', 'AMBIGUOUS', 'STARTED'))
  )
  OR (NEW.state = 'STARTED' AND NEW.dispatch_ordinal <> OLD.dispatch_ordinal + 1)
  OR (NEW.state <> 'STARTED' AND NEW.dispatch_ordinal <> OLD.dispatch_ordinal)
  OR (NEW.result_ref_json IS NOT OLD.result_ref_json AND NEW.state NOT IN ('ACKNOWLEDGED', 'OBSERVED', 'VERIFIED'))
  OR (NEW.observed_state_json IS NOT OLD.observed_state_json AND NEW.state NOT IN ('OBSERVED', 'VERIFIED', 'RECONCILING'))
  OR (NEW.verification_ref IS NOT OLD.verification_ref AND NEW.state <> 'VERIFIED')
  OR (NEW.state = 'OBSERVED' AND NOT EXISTS (
    SELECT 1 FROM evidence e
    WHERE e.evidence_id = json_extract(NEW.observed_state_json, '$._evidence_id')
      AND e.task_id = OLD.task_id
      AND e.level IN ('OBSERVED', 'VERIFIED')
      AND e.subject_ref = 'effect:' || OLD.effect_id
  ))
  OR (NEW.state = 'VERIFIED' AND NOT EXISTS (
    SELECT 1 FROM evidence e
    WHERE e.evidence_id = NEW.verification_ref
      AND e.task_id = OLD.task_id
      AND e.level = 'VERIFIED'
      AND e.subject_ref = 'effect:' || OLD.effect_id
      AND EXISTS (
        SELECT 1 FROM verification_runs v
        WHERE v.task_id = e.task_id
          AND v.status = 'PASSED'
          AND json_extract(e.producer_json, '$.service_id') = v.verifier_kind
          AND EXISTS (SELECT 1 FROM json_each(v.evidence_refs_json) ref WHERE ref.value = e.evidence_id)
      )
  ))
BEGIN
  SELECT RAISE(ABORT, 'EFFECT_TRANSITION_INVALID');
END;

-- The current Runtime-authenticated Evidence writer has no separate observer/verifier
-- assurance credential. Fail closed until that contract is implemented; otherwise a
-- Runtime could label its own report VERIFIED by naming an arbitrary ServiceRef.
CREATE TRIGGER evidence_assurance_admission_unavailable
BEFORE INSERT ON evidence
WHEN NEW.level IN ('OBSERVED', 'VERIFIED')
BEGIN
  SELECT RAISE(ABORT, 'EVIDENCE_ASSURANCE_ADMISSION_UNAVAILABLE');
END;

CREATE TRIGGER effect_delete_unsupported
BEFORE DELETE ON effects
BEGIN
  SELECT RAISE(ABORT, 'EFFECT_DELETE_UNSUPPORTED');
END;

CREATE TRIGGER evidence_immutable_update
BEFORE UPDATE ON evidence
BEGIN
  SELECT RAISE(ABORT, 'EVIDENCE_APPEND_ONLY');
END;

CREATE TRIGGER evidence_immutable_delete
BEFORE DELETE ON evidence
BEGIN
  SELECT RAISE(ABORT, 'EVIDENCE_APPEND_ONLY');
END;
