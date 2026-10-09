-- Persist exact-action Trust decision provenance on immutable audit records.
-- No authorization permit or credential is stored here.
ALTER TABLE audit_records
  ADD COLUMN policy_decision_json TEXT
  CHECK (policy_decision_json IS NULL OR json_valid(policy_decision_json));

CREATE TRIGGER audit_records_immutable_update
BEFORE UPDATE ON audit_records
BEGIN
  SELECT RAISE(ABORT, 'AUDIT_RECORD_APPEND_ONLY');
END;

CREATE TRIGGER audit_records_immutable_delete
BEFORE DELETE ON audit_records
BEGIN
  SELECT RAISE(ABORT, 'AUDIT_RECORD_APPEND_ONLY');
END;
