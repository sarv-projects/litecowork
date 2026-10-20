-- Version twenty-four persists immutable child-Attempt ResultEnvelopes as their own
-- aggregate. The result event and envelope row are committed in one transaction.

CREATE TABLE delegation_results (
  child_attempt_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  published_by_agent_session_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('COMPLETED', 'FAILED', 'BLOCKED', 'CANCELLED')),
  revision_at_start INTEGER NOT NULL CHECK (revision_at_start > 0),
  envelope_json TEXT NOT NULL CHECK (json_valid(envelope_json) AND length(CAST(envelope_json AS BLOB)) BETWEEN 2 AND 1048576),
  envelope_digest TEXT NOT NULL CHECK (
    length(envelope_digest) = 71
    AND substr(envelope_digest, 1, 7) = 'sha256:'
    AND substr(envelope_digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  event_id TEXT NOT NULL UNIQUE REFERENCES domain_events(event_id),
  published_at TEXT NOT NULL,
  FOREIGN KEY (task_id, child_attempt_id)
    REFERENCES attempts(task_id, attempt_id),
  FOREIGN KEY (task_id, workspace_id)
    REFERENCES tasks(task_id, workspace_id),
  FOREIGN KEY (
    task_id, child_attempt_id, published_by_agent_session_id, runtime_id, runtime_incarnation_id
  ) REFERENCES agent_sessions(
    task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id
  )
) STRICT;

CREATE TRIGGER delegation_results_immutable_update
BEFORE UPDATE ON delegation_results
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_DELEGATION_RESULT');
END;

CREATE TRIGGER delegation_results_immutable_delete
BEFORE DELETE ON delegation_results
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_DELEGATION_RESULT');
END;
