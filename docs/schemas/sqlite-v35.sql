-- Version thirty-five stores only redacted, Runtime-local native-auth observations.
-- Secret bytes, provider transcripts, native handles and Workspace event state are absent.
CREATE TABLE agent_auth_observations (
  agent_profile_id TEXT NOT NULL CHECK(length(trim(agent_profile_id)) BETWEEN 1 AND 256),
  runtime_id TEXT NOT NULL CHECK(length(trim(runtime_id)) BETWEEN 1 AND 256),
  runtime_incarnation_id TEXT NOT NULL CHECK(length(trim(runtime_incarnation_id)) BETWEEN 1 AND 256),
  module_id TEXT NOT NULL CHECK(length(trim(module_id)) BETWEEN 1 AND 128),
  module_version TEXT NOT NULL CHECK(length(trim(module_version)) BETWEEN 1 AND 128),
  auth_method_id TEXT NOT NULL CHECK(length(trim(auth_method_id)) BETWEEN 1 AND 128),
  state TEXT NOT NULL CHECK(state IN (
    'NEEDS_AUTH', 'AUTHENTICATING', 'AUTHENTICATED', 'EXPIRED', 'DEGRADED', 'ERROR'
  )),
  generation INTEGER NOT NULL CHECK(generation >= 1),
  observed_at TEXT NOT NULL CHECK(length(trim(observed_at)) > 0),
  expires_at TEXT NOT NULL CHECK(length(trim(expires_at)) > 0 AND observed_at < expires_at),
  PRIMARY KEY (
    agent_profile_id, runtime_id, runtime_incarnation_id,
    module_id, module_version, auth_method_id
  )
) WITHOUT ROWID;

CREATE INDEX agent_auth_observations_current_scope
  ON agent_auth_observations (
    agent_profile_id, runtime_id, runtime_incarnation_id,
    module_id, module_version, expires_at, observed_at
  );
