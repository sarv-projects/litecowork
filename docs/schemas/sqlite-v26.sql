-- The combined Trust + ApprovalUse + Invocation + Effect admission writer now exists and
-- commits that decision, its ApprovalUse, the Invocation, the Effect, the events and the
-- receipt atomically. The blanket creation block installed in version fifteen is removed
-- so that writer can create an Invocation.
--
-- Admission is not thereby opened. Version one still installs the authoritative boundary
-- triggers, which continue to reject an unauthorized Invocation at the database:
--   * capability_invocation_admission_matches_scope requires a live, ACTIVE agent session
--     in the live_agent_session_invocation_authority view, an ACTIVE and unexpired
--     capability grant with a matching scope, capability identity and an operation in the
--     grant's allowed operation list, and an ACTIVE capability activation bound to the
--     same workspace, scope and runtime incarnation;
--   * capability_invocation_identity_immutable keeps the admitted identity columns
--     immutable across updates.
--
-- The status guard is re-created with the same version rule and the same transition
-- table as version fifteen, with one addition: CREATED may now advance to DISPATCHED.
-- That transition is additionally revalidated by the version one trigger
-- capability_invocation_first_dispatch_revalidates_owner, which refuses a dispatch whose
-- session, grant or activation is no longer live, matching or unexpired.
DROP TRIGGER capability_invocation_creation_admission_closed;
DROP TRIGGER capability_invocation_status_transition_guard;

CREATE TRIGGER capability_invocation_status_transition_guard
BEFORE UPDATE OF status, version ON capability_invocations
WHEN NEW.version <> OLD.version + 1
  OR NOT (
    (OLD.status = 'CREATED' AND NEW.status IN ('DISPATCHED', 'CANCELLED'))
    OR (OLD.status = 'DISPATCHED' AND NEW.status IN ('SUCCEEDED', 'FAILED', 'WAITING', 'INPUT_REQUIRED', 'CANCEL_REQUESTED', 'AMBIGUOUS'))
    OR (OLD.status = 'WAITING' AND NEW.status IN ('INPUT_REQUIRED', 'CANCEL_REQUESTED', 'AMBIGUOUS'))
    OR (OLD.status = 'INPUT_REQUIRED' AND NEW.status IN ('WAITING', 'CANCEL_REQUESTED', 'AMBIGUOUS'))
    OR (OLD.status = 'CANCEL_REQUESTED' AND NEW.status IN ('CANCELLED', 'SUCCEEDED', 'FAILED', 'AMBIGUOUS'))
    OR (OLD.status = 'AMBIGUOUS' AND NEW.status IN ('WAITING', 'INPUT_REQUIRED', 'CANCEL_REQUESTED', 'SUCCEEDED', 'FAILED', 'CANCELLED'))
  )
BEGIN
  SELECT RAISE(ABORT, 'CAPABILITY_INVOCATION_TRANSITION_INVALID');
END;

-- A dispatch is the point where a consequential operation becomes real, so the database
-- refuses it unless the exact Effect being dispatched is already bound to a consumed
-- Approval whose approval is APPROVED for the same Task. The combined admission writer
-- records that ApprovalUse in the same transaction, before it advances the Invocation.
--
-- This closes the path that removing the version fifteen creation block would otherwise
-- open: a direct status transition that skipped the Trust decision and the ApprovalUse
-- can no longer reach DISPATCHED, even when the version one live-authority triggers are
-- satisfied.
CREATE TRIGGER capability_invocation_dispatch_requires_consumed_approval
BEFORE UPDATE OF status, effect_id ON capability_invocations
WHEN OLD.status = 'CREATED' AND NEW.status = 'DISPATCHED'
  AND NOT EXISTS (
    SELECT 1
    FROM approval_uses u
    JOIN approvals a ON a.approval_id = u.approval_id
    WHERE NEW.effect_id IS NOT NULL
      AND u.effect_id = NEW.effect_id
      AND a.status = 'APPROVED'
      AND a.task_id = NEW.task_id
  )
BEGIN
  SELECT RAISE(ABORT, 'INVOCATION_DISPATCH_REQUIRES_CONSUMED_APPROVAL');
END;
