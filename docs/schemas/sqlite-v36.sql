-- Version thirty-six stores Runtime-local credential submission recovery state.
-- No credential bytes, raw Idempotency-Key, or unkeyed credential verifier belong here.
CREATE TABLE agent_credential_submission_receipts (
  workspace_id TEXT NOT NULL CHECK(length(trim(workspace_id)) BETWEEN 1 AND 512),
  principal_id TEXT NOT NULL CHECK(length(trim(principal_id)) BETWEEN 1 AND 512),
  agent_profile_id TEXT NOT NULL CHECK(length(trim(agent_profile_id)) BETWEEN 1 AND 512),
  credential_slot_id TEXT NOT NULL CHECK(length(trim(credential_slot_id)) BETWEEN 1 AND 128),
  idempotency_key_digest TEXT NOT NULL CHECK(
    length(idempotency_key_digest) = 71
    AND substr(idempotency_key_digest, 1, 7) = 'sha256:'
    AND substr(idempotency_key_digest, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  request_fingerprint TEXT NOT NULL CHECK(
    length(request_fingerprint) = 76
    AND substr(request_fingerprint, 1, 12) = 'hmac-sha256:'
    AND substr(request_fingerprint, 13) NOT GLOB '*[^0-9a-f]*'
  ),
  secret_kind TEXT NOT NULL CHECK(secret_kind IN ('API_KEY', 'TOKEN', 'PASSWORD', 'OTHER')),
  secret_ref_id TEXT NOT NULL UNIQUE CHECK(
    length(secret_ref_id) = 71
    AND substr(secret_ref_id, 1, 7) = 'secret_'
    AND substr(secret_ref_id, 8) NOT GLOB '*[^0-9a-f]*'
  ),
  state TEXT NOT NULL CHECK(state IN ('PENDING', 'COMMITTED')),
  provider_ref TEXT,
  created_at TEXT NOT NULL CHECK(length(trim(created_at)) BETWEEN 1 AND 64),
  committed_at TEXT CHECK(committed_at IS NULL OR length(trim(committed_at)) BETWEEN 1 AND 64),
  PRIMARY KEY (
    workspace_id, principal_id, agent_profile_id, credential_slot_id,
    idempotency_key_digest
  ),
  FOREIGN KEY(workspace_id) REFERENCES workspaces(workspace_id) ON DELETE RESTRICT,
  CHECK(
    (state = 'PENDING' AND provider_ref IS NULL AND committed_at IS NULL)
    OR
    (state = 'COMMITTED' AND provider_ref IS NOT NULL AND length(trim(provider_ref)) BETWEEN 1 AND 512
      AND committed_at IS NOT NULL)
  )
) WITHOUT ROWID;
