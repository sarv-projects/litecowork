-- Agent Registry/control lifecycle storage for the modular AgentModule target.
-- All rows in the first four tables are installation-local operational state.
-- Secret bytes are forbidden: only bounded sanitized metadata and SecretRefs may appear.
-- Durable Workspace authority remains in agent_bindings and the versioned
-- agent_binding_configurations projection below.

CREATE TABLE agent_registry_cache (
  registry_agent_id TEXT PRIMARY KEY
    CHECK (length(registry_agent_id) BETWEEN 1 AND 128),
  source_registry TEXT NOT NULL
    CHECK (length(source_registry) BETWEEN 1 AND 256),
  source_revision TEXT
    CHECK (source_revision IS NULL OR length(source_revision) <= 256),
  metadata_json TEXT NOT NULL
    CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
  observed_at TEXT NOT NULL,
  expires_at TEXT
);

CREATE INDEX idx_agent_registry_cache_expiry
  ON agent_registry_cache(expires_at);

CREATE TABLE agent_installation_observations (
  registry_agent_id TEXT NOT NULL
    CHECK (length(registry_agent_id) BETWEEN 1 AND 128),
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  module_id TEXT NOT NULL
    CHECK (length(module_id) BETWEEN 1 AND 128),
  state TEXT NOT NULL CHECK (
    state IN (
      'MISSING',
      'INSTALLING',
      'INSTALLED',
      'UPDATE_AVAILABLE',
      'UPDATING',
      'VERSION_UNAVAILABLE',
      'BROKEN'
    )
  ),
  installed_version TEXT
    CHECK (installed_version IS NULL OR length(installed_version) <= 128),
  available_version TEXT
    CHECK (available_version IS NULL OR length(available_version) <= 128),
  installation_source TEXT
    CHECK (installation_source IS NULL OR length(installation_source) <= 256),
  observed_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  PRIMARY KEY (registry_agent_id, runtime_incarnation_id),
  FOREIGN KEY (runtime_id, runtime_incarnation_id)
    REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
    ON DELETE CASCADE
);

CREATE INDEX idx_agent_installation_observations_runtime
  ON agent_installation_observations(runtime_id, runtime_incarnation_id);

CREATE INDEX idx_agent_installation_observations_expiry
  ON agent_installation_observations(expires_at);

CREATE TABLE agent_control_descriptors (
  descriptor_digest TEXT PRIMARY KEY
    CHECK (
      length(descriptor_digest) = 71
      AND substr(descriptor_digest, 1, 7) = 'sha256:'
      AND substr(descriptor_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
  registry_agent_id TEXT NOT NULL
    CHECK (length(registry_agent_id) BETWEEN 1 AND 128),
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  module_id TEXT NOT NULL
    CHECK (length(module_id) BETWEEN 1 AND 128),
  module_version TEXT NOT NULL
    CHECK (length(module_version) BETWEEN 1 AND 128),
  descriptor_api_version INTEGER NOT NULL
    CHECK (descriptor_api_version >= 1),
  descriptor_json TEXT NOT NULL
    CHECK (json_valid(descriptor_json) AND json_type(descriptor_json) = 'object'),
  observed_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  FOREIGN KEY (runtime_id, runtime_incarnation_id)
    REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
    ON DELETE CASCADE
);

CREATE INDEX idx_agent_control_descriptors_agent_runtime
  ON agent_control_descriptors(
    registry_agent_id,
    runtime_id,
    runtime_incarnation_id,
    expires_at
  );

CREATE TABLE agent_lifecycle_operations (
  lifecycle_operation_id TEXT PRIMARY KEY,
  registry_agent_id TEXT NOT NULL
    CHECK (length(registry_agent_id) BETWEEN 1 AND 128),
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  operation TEXT NOT NULL
    CHECK (operation IN ('INSTALL', 'UPDATE', 'REPAIR')),
  request_id TEXT NOT NULL
    CHECK (length(request_id) BETWEEN 1 AND 128),
  request_digest TEXT NOT NULL
    CHECK (
      length(request_digest) = 71
      AND substr(request_digest, 1, 7) = 'sha256:'
      AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
  state TEXT NOT NULL
    CHECK (state IN ('QUEUED', 'RUNNING', 'SUCCEEDED', 'FAILED', 'UNKNOWN')),
  observed_version TEXT
    CHECK (observed_version IS NULL OR length(observed_version) <= 128),
  message TEXT
    CHECK (message IS NULL OR length(message) <= 512),
  result_json TEXT
    CHECK (
      result_json IS NULL
      OR (json_valid(result_json) AND json_type(result_json) = 'object')
    ),
  started_at TEXT NOT NULL,
  settled_at TEXT,
  FOREIGN KEY (runtime_id, runtime_incarnation_id)
    REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
    ON DELETE CASCADE,
  UNIQUE (runtime_incarnation_id, request_id)
);

CREATE INDEX idx_agent_lifecycle_operations_agent
  ON agent_lifecycle_operations(registry_agent_id, started_at);

CREATE TABLE agent_binding_configurations (
  agent_binding_id TEXT PRIMARY KEY
    REFERENCES agent_bindings(agent_binding_id)
    ON DELETE CASCADE,

  configuration_descriptor_digest TEXT
    CHECK (
      configuration_descriptor_digest IS NULL
      OR (
        length(configuration_descriptor_digest) = 71
        AND substr(configuration_descriptor_digest, 1, 7) = 'sha256:'
        AND substr(configuration_descriptor_digest, 8)
          NOT GLOB '*[^0-9a-f]*'
      )
    ),

  configuration_json TEXT NOT NULL DEFAULT '{}'
    CHECK (
      json_valid(configuration_json)
      AND json_type(configuration_json) = 'object'
    ),

  credential_bindings_json TEXT NOT NULL DEFAULT '[]'
    CHECK (
      json_valid(credential_bindings_json)
      AND json_type(credential_bindings_json) = 'array'
      AND json_array_length(credential_bindings_json) <= 32
    ),

  session_options_descriptor_digest TEXT
    CHECK (
      session_options_descriptor_digest IS NULL
      OR (
        length(session_options_descriptor_digest) = 71
        AND substr(session_options_descriptor_digest, 1, 7) = 'sha256:'
        AND substr(session_options_descriptor_digest, 8)
          NOT GLOB '*[^0-9a-f]*'
      )
    ),

  default_session_options_json TEXT NOT NULL DEFAULT '{}'
    CHECK (
      json_valid(default_session_options_json)
      AND json_type(default_session_options_json) = 'object'
    ),

  configuration_digest TEXT NOT NULL
    CHECK (
      length(configuration_digest) = 71
      AND substr(configuration_digest, 1, 7) = 'sha256:'
      AND substr(configuration_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),

  default_session_options_digest TEXT NOT NULL
    CHECK (
      length(default_session_options_digest) = 71
      AND substr(default_session_options_digest, 1, 7) = 'sha256:'
      AND substr(default_session_options_digest, 8)
        NOT GLOB '*[^0-9a-f]*'
    ),

  version INTEGER NOT NULL DEFAULT 1
    CHECK (version >= 1),
  updated_at TEXT NOT NULL
);

CREATE TRIGGER agent_binding_configurations_version_guard
BEFORE UPDATE ON agent_binding_configurations
WHEN NEW.version <> OLD.version + 1
BEGIN
  SELECT RAISE(ABORT, 'AGENT_BINDING_CONFIGURATION_VERSION_INVALID');
END;

CREATE TRIGGER agent_binding_configuration_binding_version_guard
BEFORE INSERT ON agent_binding_configurations
WHEN NOT EXISTS (
  SELECT 1
  FROM agent_bindings b
  WHERE b.agent_binding_id = NEW.agent_binding_id
)
BEGIN
  SELECT RAISE(ABORT, 'AGENT_BINDING_CONFIGURATION_BINDING_MISSING');
END;

-- The database can enforce shape and digest bounds but cannot decide whether a JSON key
-- is secret or allowed by one AgentModule. AgentBindingService MUST validate the current
-- closed AgentConfigurationDescriptor and credential-slot declarations before writing.
