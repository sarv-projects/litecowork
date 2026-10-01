PRAGMA foreign_keys = ON;

CREATE TABLE workspaces (
  workspace_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  owner_principal_id TEXT NOT NULL,
  replication_policy TEXT NOT NULL,
  hub_runtime_id TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE conversations (
  conversation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  title TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE conversation_messages (
  message_id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id),
  author_json TEXT NOT NULL,
  role TEXT NOT NULL,
  content_json TEXT NOT NULL,
  resource_refs_json TEXT NOT NULL DEFAULT '[]',
  source_channel_ref_json TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_messages_conversation_time ON conversation_messages(conversation_id, created_at);

CREATE TABLE tasks (
  task_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  conversation_id TEXT REFERENCES conversations(conversation_id),
  current_spec_revision INTEGER NOT NULL,
  current_plan_revision INTEGER,
  status TEXT NOT NULL,
  lead_attempt_id TEXT,
  priority TEXT NOT NULL,
  created_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_tasks_workspace_status ON tasks(workspace_id, status, updated_at DESC);

CREATE TABLE task_spec_revisions (
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  revision INTEGER NOT NULL,
  parent_revisions_json TEXT NOT NULL DEFAULT '[]',
  objective TEXT NOT NULL,
  constraints_json TEXT NOT NULL DEFAULT '[]',
  non_goals_json TEXT NOT NULL DEFAULT '[]',
  input_refs_json TEXT NOT NULL DEFAULT '[]',
  required_outputs_json TEXT NOT NULL DEFAULT '[]',
  acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
  approvals_required_json TEXT NOT NULL DEFAULT '[]',
  budget_json TEXT,
  deadline TEXT,
  source_message_refs_json TEXT NOT NULL DEFAULT '[]',
  authored_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (task_id, revision)
);

CREATE TABLE plan_revisions (
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  revision INTEGER NOT NULL,
  task_spec_revision INTEGER NOT NULL,
  produced_by_attempt TEXT NOT NULL,
  steps_json TEXT NOT NULL,
  reason_for_revision TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (task_id, revision)
);

CREATE TABLE steps (
  step_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  plan_revision INTEGER NOT NULL,
  logical_key TEXT,
  title TEXT NOT NULL,
  objective TEXT NOT NULL,
  dependencies_json TEXT NOT NULL DEFAULT '[]',
  required_capabilities_json TEXT NOT NULL DEFAULT '[]',
  acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL,
  current_attempt_id TEXT REFERENCES attempts(attempt_id) DEFERRABLE INITIALLY DEFERRED,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_steps_task_status ON steps(task_id, status);

CREATE TABLE agent_profiles (
  agent_profile_id TEXT PRIMARY KEY,
  provider_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  adapter_kind TEXT NOT NULL,
  capabilities_json TEXT NOT NULL,
  discovered_at TEXT NOT NULL
);

CREATE TABLE agent_bindings (
  agent_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  agent_profile_id TEXT NOT NULL REFERENCES agent_profiles(agent_profile_id),
  runtime_id TEXT,
  auth_ref TEXT,
  configuration_json TEXT NOT NULL DEFAULT '{}',
  enabled INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE runtimes (
  runtime_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  device_identity_json TEXT NOT NULL,
  runtime_version TEXT NOT NULL,
  platform TEXT NOT NULL,
  architecture TEXT NOT NULL,
  roles_json TEXT NOT NULL,
  trust_zone TEXT NOT NULL,
  availability TEXT NOT NULL,
  resource_capacity_json TEXT NOT NULL DEFAULT '{}',
  last_seen TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_runtimes_workspace_availability ON runtimes(workspace_id, availability);

CREATE TABLE runtime_offers (
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  offer_kind TEXT NOT NULL,
  offer_ref TEXT NOT NULL,
  compatible INTEGER NOT NULL,
  constraints_json TEXT NOT NULL DEFAULT '{}',
  observed_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  PRIMARY KEY(runtime_id, offer_kind, offer_ref)
);
CREATE INDEX idx_runtime_offers_expiry ON runtime_offers(expires_at);

CREATE TABLE pairing_tokens (
  token_digest TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  issued_by_json TEXT NOT NULL,
  allowed_roles_json TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_pairing_tokens_expiry ON pairing_tokens(expires_at);

CREATE TABLE environments (
  environment_id TEXT PRIMARY KEY,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  provider_kind TEXT NOT NULL,
  class TEXT NOT NULL,
  status TEXT NOT NULL,
  locator_json TEXT NOT NULL DEFAULT '{}',
  isolation_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE environment_checkpoints (
  checkpoint_id TEXT PRIMARY KEY,
  environment_id TEXT NOT NULL REFERENCES environments(environment_id),
  provider_ref TEXT NOT NULL,
  digest TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE attempts (
  attempt_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  step_id TEXT NOT NULL REFERENCES steps(step_id),
  parent_attempt_id TEXT REFERENCES attempts(attempt_id),
  agent_binding_id TEXT NOT NULL REFERENCES agent_bindings(agent_binding_id),
  agent_session_id TEXT REFERENCES agent_sessions(agent_session_id) DEFERRABLE INITIALLY DEFERRED,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  environment_id TEXT NOT NULL REFERENCES environments(environment_id),
  capability_grant_ids_json TEXT NOT NULL DEFAULT '[]',
  execution_lease_id TEXT,
  failover_class TEXT NOT NULL,
  checkpoint_ref_json TEXT,
  status TEXT NOT NULL,
  failure_json TEXT,
  started_at TEXT,
  settled_at TEXT,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_attempts_task_step_status ON attempts(task_id, step_id, status);

CREATE TABLE agent_sessions (
  agent_session_id TEXT PRIMARY KEY,
  agent_binding_id TEXT NOT NULL REFERENCES agent_bindings(agent_binding_id),
  attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  native_session_ref TEXT,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  last_event_at TEXT,
  closed_at TEXT,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE execution_leases (
  lease_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  step_id TEXT NOT NULL REFERENCES steps(step_id),
  attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  epoch INTEGER NOT NULL,
  fencing_token TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  checkpoint_ref_json TEXT,
  acquired_at TEXT NOT NULL,
  renew_by TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(step_id, epoch)
);
CREATE INDEX idx_leases_step_state ON execution_leases(step_id, state);
CREATE UNIQUE INDEX uq_active_lease_per_step ON execution_leases(step_id) WHERE state = 'ACTIVE';

CREATE TABLE capability_grants (
  capability_grant_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  attempt_id TEXT REFERENCES attempts(attempt_id),
  capability_ref_json TEXT NOT NULL,
  allowed_operations_json TEXT NOT NULL,
  resource_scope_json TEXT NOT NULL,
  secret_refs_json TEXT NOT NULL DEFAULT '[]',
  granted_by_json TEXT NOT NULL,
  expires_at TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE secret_leases (
  secret_lease_id TEXT PRIMARY KEY,
  secret_ref_json TEXT NOT NULL,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  attempt_id TEXT REFERENCES attempts(attempt_id),
  capability_ref_json TEXT,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  allowed_usage_json TEXT NOT NULL,
  status TEXT NOT NULL,
  issued_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_secret_leases_task_status ON secret_leases(task_id, status);

CREATE TABLE capability_activations (
  activation_id TEXT PRIMARY KEY,
  capability_ref_json TEXT NOT NULL,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  mode TEXT NOT NULL,
  provider_handle_ref TEXT,
  health TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE capability_locks (
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  capability_id TEXT NOT NULL,
  package_version TEXT NOT NULL,
  digest TEXT NOT NULL,
  source TEXT NOT NULL,
  resolved_components_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  PRIMARY KEY(task_id, capability_id, digest)
);

CREATE TABLE connections (
  connection_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  external_provider_ref TEXT NOT NULL,
  account_ref TEXT,
  secret_refs_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE channel_bindings (
  channel_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  provider_ref TEXT NOT NULL,
  external_account_ref TEXT NOT NULL,
  identity_ref_json TEXT NOT NULL,
  assurance_level TEXT NOT NULL,
  allowed_actions_json TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE channel_thread_mappings (
  channel_binding_id TEXT NOT NULL REFERENCES channel_bindings(channel_binding_id),
  provider_thread_id TEXT NOT NULL,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id),
  created_at TEXT NOT NULL,
  PRIMARY KEY(channel_binding_id, provider_thread_id)
);

CREATE TABLE channel_event_receipts (
  channel_binding_id TEXT NOT NULL REFERENCES channel_bindings(channel_binding_id),
  provider_event_id TEXT NOT NULL,
  event_kind TEXT NOT NULL,
  payload_digest TEXT NOT NULL,
  conversation_id TEXT REFERENCES conversations(conversation_id),
  message_id TEXT REFERENCES conversation_messages(message_id),
  received_at TEXT NOT NULL,
  state TEXT NOT NULL,
  PRIMARY KEY(channel_binding_id, provider_event_id)
);

CREATE TABLE artifacts (
  artifact_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  task_id TEXT REFERENCES tasks(task_id),
  kind TEXT NOT NULL,
  display_name TEXT NOT NULL,
  current_version INTEGER NOT NULL,
  library_status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_artifacts_task ON artifacts(task_id, current_version);

CREATE TABLE artifact_versions (
  artifact_id TEXT NOT NULL REFERENCES artifacts(artifact_id),
  version INTEGER NOT NULL,
  created_by_attempt TEXT REFERENCES attempts(attempt_id),
  input_refs_json TEXT NOT NULL DEFAULT '[]',
  content_digest TEXT NOT NULL,
  storage_ref TEXT NOT NULL,
  media_type TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  provenance_json TEXT NOT NULL,
  verification_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  PRIMARY KEY(artifact_id, version)
);
CREATE INDEX idx_artifact_digest ON artifact_versions(content_digest);

CREATE TABLE effects (
  effect_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  capability_ref_json TEXT,
  operation TEXT NOT NULL,
  target_json TEXT NOT NULL,
  idempotency_key TEXT,
  state TEXT NOT NULL,
  request_digest TEXT NOT NULL,
  result_ref_json TEXT,
  observed_state_json TEXT,
  verification_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_effects_task_state ON effects(task_id, state);
CREATE INDEX idx_effects_idempotency ON effects(idempotency_key);

CREATE TABLE evidence (
  evidence_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  subject_ref TEXT NOT NULL,
  level TEXT NOT NULL,
  kind TEXT NOT NULL,
  producer_json TEXT NOT NULL,
  payload_ref_json TEXT,
  payload_digest TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE audit_records (
  audit_record_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  principal_json TEXT NOT NULL,
  action TEXT NOT NULL,
  resource_ref_json TEXT,
  decision TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  correlation_id TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  payload_digest TEXT
);
CREATE INDEX idx_audit_workspace_time ON audit_records(workspace_id, occurred_at);

CREATE TABLE verification_runs (
  verification_run_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  criterion_id TEXT NOT NULL,
  verifier_kind TEXT NOT NULL,
  subject_refs_json TEXT NOT NULL,
  status TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  started_at TEXT,
  completed_at TEXT
);

CREATE TABLE approvals (
  approval_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  requested_by_attempt TEXT REFERENCES attempts(attempt_id),
  kind TEXT NOT NULL,
  action_summary TEXT NOT NULL,
  action_digest TEXT NOT NULL,
  risk TEXT NOT NULL,
  required_assurance TEXT NOT NULL,
  status TEXT NOT NULL,
  requested_at TEXT NOT NULL,
  expires_at TEXT,
  resolved_by_json TEXT,
  resolved_at TEXT,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_approvals_workspace_status ON approvals(task_id, status);

CREATE TABLE automations (
  automation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  name TEXT NOT NULL,
  trigger_json TEXT NOT NULL,
  task_template_json TEXT NOT NULL,
  execution_policy_json TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE automation_occurrences (
  occurrence_id TEXT PRIMARY KEY,
  automation_id TEXT NOT NULL REFERENCES automations(automation_id),
  automation_version INTEGER NOT NULL,
  scheduled_key TEXT NOT NULL,
  scheduled_for TEXT NOT NULL,
  task_id TEXT REFERENCES tasks(task_id),
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(automation_id, scheduled_key)
);

CREATE TABLE domain_events (
  event_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  origin_runtime_id TEXT NOT NULL,
  origin_sequence INTEGER NOT NULL,
  entity_revision INTEGER,
  hlc_timestamp TEXT NOT NULL,
  correlation_id TEXT NOT NULL,
  causation_id TEXT,
  schema_version INTEGER NOT NULL,
  type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  payload_digest TEXT NOT NULL,
  UNIQUE(origin_runtime_id, origin_sequence)
);
CREATE INDEX idx_events_workspace_hlc ON domain_events(workspace_id, hlc_timestamp);
CREATE INDEX idx_events_entity ON domain_events(entity_type, entity_id, entity_revision);

CREATE TABLE request_dedup (
  principal_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  request_digest TEXT NOT NULL,
  response_json TEXT,
  response_digest TEXT,
  created_at TEXT NOT NULL,
  expires_at TEXT,
  PRIMARY KEY(principal_id, request_id)
);

CREATE TABLE replication_cursors (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  peer_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  origin_runtime_id TEXT NOT NULL,
  highest_contiguous_sequence INTEGER NOT NULL DEFAULT 0,
  missing_sequences_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, peer_runtime_id, origin_runtime_id)
);

CREATE TABLE handoffs (
  handoff_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  step_id TEXT NOT NULL REFERENCES steps(step_id),
  source_attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  source_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  target_runtime_id TEXT REFERENCES runtimes(runtime_id),
  phase TEXT NOT NULL,
  resume_packet_ref_json TEXT,
  blockers_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
