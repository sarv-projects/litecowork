-- Invocation lifecycle transitions are validated at the database boundary. Dispatch
-- remains closed until Trust, Effect, ApprovalUse and lease admission commit atomically.
-- No direct or provider-originated INSERT is accepted before the combined writer exists.
CREATE TRIGGER capability_invocation_creation_admission_closed
BEFORE INSERT ON capability_invocations
BEGIN
  SELECT RAISE(ABORT, 'INVOCATION_CREATION_ADMISSION_UNAVAILABLE');
END;

CREATE TRIGGER capability_invocation_status_transition_guard
BEFORE UPDATE OF status, version ON capability_invocations
WHEN NEW.version <> OLD.version + 1
  OR NEW.status = 'DISPATCHED'
  OR NOT (
    (OLD.status = 'CREATED' AND NEW.status = 'CANCELLED')
    OR (OLD.status = 'DISPATCHED' AND NEW.status IN ('SUCCEEDED', 'FAILED', 'WAITING', 'INPUT_REQUIRED', 'CANCEL_REQUESTED', 'AMBIGUOUS'))
    OR (OLD.status = 'WAITING' AND NEW.status IN ('INPUT_REQUIRED', 'CANCEL_REQUESTED', 'AMBIGUOUS'))
    OR (OLD.status = 'INPUT_REQUIRED' AND NEW.status IN ('WAITING', 'CANCEL_REQUESTED', 'AMBIGUOUS'))
    OR (OLD.status = 'CANCEL_REQUESTED' AND NEW.status IN ('CANCELLED', 'SUCCEEDED', 'FAILED', 'AMBIGUOUS'))
    OR (OLD.status = 'AMBIGUOUS' AND NEW.status IN ('WAITING', 'INPUT_REQUIRED', 'CANCEL_REQUESTED', 'SUCCEEDED', 'FAILED', 'CANCELLED'))
  )
BEGIN
  SELECT RAISE(ABORT, 'CAPABILITY_INVOCATION_TRANSITION_INVALID_OR_DISPATCH_CLOSED');
END;
