PRAGMA foreign_keys = ON;

CREATE TABLE workspaces (
  workspace_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  owner_principal_id TEXT NOT NULL,
  replication_policy TEXT NOT NULL CHECK (replication_policy IN ('LOCAL_ONLY', 'METADATA_ONLY', 'ACTIVE_TASK_INPUTS', 'SELECTED_FOLDERS', 'FULL_WORKSPACE')),
  current_instruction_revision INTEGER,
  default_agent_binding_id TEXT,
  primary_coworker_id TEXT,
  hub_runtime_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ARCHIVED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(workspace_id, current_instruction_revision) REFERENCES workspace_instruction_revisions(workspace_id, revision) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(workspace_id, default_agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(workspace_id, primary_coworker_id) REFERENCES coworkers(workspace_id, coworker_id) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE workspace_instruction_revisions (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  revision INTEGER NOT NULL,
  parent_revisions_json TEXT NOT NULL DEFAULT '[]',
  content_ref_json TEXT NOT NULL,
  content_digest TEXT NOT NULL CHECK (length(content_digest) = 71 AND substr(content_digest, 1, 7) = 'sha256:' AND substr(content_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  authored_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, revision)
);

CREATE TABLE conversations (
  conversation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  title TEXT,
  active_agent_binding_id TEXT REFERENCES agent_bindings(agent_binding_id),
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, conversation_id),
  FOREIGN KEY(workspace_id, active_agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE conversation_messages (
  message_id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id),
  author_json TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('USER', 'AGENT', 'SYSTEM_NOTICE', 'CHANNEL')),
  agent_session_id TEXT,
  agent_binding_id TEXT,
  turn_id TEXT,
  content_json TEXT NOT NULL,
  resource_refs_json TEXT NOT NULL DEFAULT '[]',
  source_channel_ref_json TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_messages_conversation_time ON conversation_messages(conversation_id, created_at);

CREATE TABLE conversation_turns (
  turn_id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id),
  user_message_id TEXT NOT NULL REFERENCES conversation_messages(message_id),
  agent_session_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('OPEN', 'RUNNING', 'WAITING_USER', 'WAITING_DEPENDENCY', 'COMPLETED', 'FAILED', 'CANCEL_REQUESTED', 'CANCELLED')),
  retry_ordinal INTEGER NOT NULL DEFAULT 0 CHECK (retry_ordinal >= 0),
  created_at TEXT NOT NULL,
  settled_at TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(conversation_id, turn_id),
  FOREIGN KEY(conversation_id, turn_id, agent_session_id)
    REFERENCES agent_sessions(conversation_id, conversation_turn_id, agent_session_id)
    DEFERRABLE INITIALLY DEFERRED
);
CREATE UNIQUE INDEX uq_conversation_turn_current_agent_session
  ON conversation_turns(agent_session_id) WHERE agent_session_id IS NOT NULL;

CREATE TABLE provider_circuit_states (
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('CAPABILITY', 'ENVIRONMENT', 'CHANNEL')),
  provider_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('CLOSED', 'OPEN', 'HALF_OPEN')),
  failure_window_started_at TEXT,
  consecutive_failures INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0 AND consecutive_failures <= 4294967295),
  open_until TEXT,
  half_open_probe_id TEXT,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY(runtime_id, provider_kind, provider_ref),
  CHECK ((status = 'OPEN' AND open_until IS NOT NULL) OR status <> 'OPEN'),
  CHECK ((status = 'HALF_OPEN' AND half_open_probe_id IS NOT NULL) OR status <> 'HALF_OPEN')
);
CREATE INDEX idx_provider_circuit_open_until ON provider_circuit_states(status, open_until);

CREATE TABLE tasks (
  task_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  conversation_id TEXT REFERENCES conversations(conversation_id),
  routine_id TEXT,
  routine_revision INTEGER,
  automation_id TEXT,
  automation_occurrence_id TEXT,
  origin_coworker_id TEXT,
  origin_coworker_revision INTEGER,
  current_spec_revision INTEGER NOT NULL,
  current_plan_revision INTEGER,
  resume_status TEXT CHECK (resume_status IS NULL OR resume_status IN ('READY', 'RUNNING', 'WAITING_USER', 'BLOCKED', 'VERIFYING')),
  status TEXT NOT NULL CHECK (status IN ('READY', 'RUNNING', 'WAITING_USER', 'BLOCKED', 'VERIFYING', 'NEEDS_USER', 'INCOMPLETE', 'PAUSE_REQUESTED', 'PAUSED', 'COMPLETED', 'FAILED', 'CANCEL_REQUESTED', 'CANCELLED')),
  lead_agent_binding_id TEXT NOT NULL REFERENCES agent_bindings(agent_binding_id) DEFERRABLE INITIALLY DEFERRED,
  blocking_conditions_json TEXT NOT NULL DEFAULT '[]',
  priority TEXT NOT NULL CHECK (priority IN ('LOW', 'NORMAL', 'HIGH')),
  created_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(task_id, workspace_id),
  FOREIGN KEY(workspace_id, lead_agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(workspace_id, origin_coworker_id) REFERENCES coworkers(workspace_id, coworker_id),
  FOREIGN KEY(origin_coworker_id, origin_coworker_revision, workspace_id) REFERENCES coworker_revisions(coworker_id, revision, workspace_id),
  FOREIGN KEY(task_id, current_spec_revision) REFERENCES task_spec_revisions(task_id, revision) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(task_id, current_plan_revision) REFERENCES plan_revisions(task_id, revision) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(routine_id, routine_revision, workspace_id) REFERENCES routine_revisions(routine_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(automation_id, automation_occurrence_id, workspace_id) REFERENCES automation_occurrences(automation_id, occurrence_id, workspace_id) DEFERRABLE INITIALLY DEFERRED,
  CHECK ((routine_id IS NULL AND routine_revision IS NULL) OR (routine_id IS NOT NULL AND routine_revision IS NOT NULL)),
  CHECK ((automation_id IS NULL AND automation_occurrence_id IS NULL) OR (automation_id IS NOT NULL AND automation_occurrence_id IS NOT NULL)),
  CHECK ((origin_coworker_id IS NULL AND origin_coworker_revision IS NULL) OR (origin_coworker_id IS NOT NULL AND origin_coworker_revision IS NOT NULL))
);
CREATE INDEX idx_tasks_workspace_status ON tasks(workspace_id, status, updated_at DESC);

CREATE TABLE goals (
  goal_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  coworker_id TEXT,
  current_revision INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'PAUSED', 'COMPLETED', 'ARCHIVED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, goal_id),
  FOREIGN KEY(workspace_id, coworker_id) REFERENCES coworkers(workspace_id, coworker_id),
  FOREIGN KEY(goal_id, current_revision) REFERENCES goal_revisions(goal_id, revision) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_goals_workspace_status ON goals(workspace_id, status, updated_at DESC);

CREATE TRIGGER goal_identity_immutable
BEFORE UPDATE ON goals
WHEN NEW.goal_id IS NOT OLD.goal_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.coworker_id IS NOT OLD.coworker_id
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'GOAL_IDENTITY_IMMUTABLE');
END;

CREATE TABLE goal_revisions (
  goal_id TEXT NOT NULL REFERENCES goals(goal_id),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  objective TEXT NOT NULL,
  success_criteria_json TEXT NOT NULL CHECK (json_valid(success_criteria_json)),
  constraints_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(constraints_json)),
  horizon TEXT,
  authored_by_json TEXT NOT NULL CHECK (json_valid(authored_by_json)),
  created_at TEXT NOT NULL,
  PRIMARY KEY(goal_id, revision),
  UNIQUE(goal_id, revision, workspace_id),
  FOREIGN KEY(workspace_id, goal_id) REFERENCES goals(workspace_id, goal_id)
);

CREATE TABLE goal_task_links (
  goal_id TEXT NOT NULL,
  revision INTEGER NOT NULL,
  workspace_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  PRIMARY KEY(goal_id, revision, task_id),
  FOREIGN KEY(goal_id, revision, workspace_id) REFERENCES goal_revisions(goal_id, revision, workspace_id),
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id)
);
CREATE INDEX idx_goal_task_links_task ON goal_task_links(task_id, goal_id, revision);

CREATE TRIGGER goal_revision_immutable_update
BEFORE UPDATE ON goal_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_GOAL_REVISION');
END;

CREATE TRIGGER goal_revision_immutable_delete
BEFORE DELETE ON goal_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_GOAL_REVISION');
END;

CREATE TRIGGER goal_task_link_current_revision_guard
BEFORE INSERT ON goal_task_links
WHEN NOT EXISTS (
  SELECT 1 FROM goals g
  WHERE g.goal_id = NEW.goal_id
    AND g.workspace_id = NEW.workspace_id
    AND g.current_revision = NEW.revision
    AND g.status <> 'ARCHIVED'
)
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;

CREATE TRIGGER goal_task_link_immutable_update
BEFORE UPDATE ON goal_task_links
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;

CREATE TRIGGER goal_task_link_immutable_delete
BEFORE DELETE ON goal_task_links
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;

CREATE TABLE goal_routine_links (
  goal_id TEXT NOT NULL,
  revision INTEGER NOT NULL,
  workspace_id TEXT NOT NULL,
  routine_id TEXT NOT NULL,
  routine_revision INTEGER NOT NULL,
  PRIMARY KEY(goal_id, revision, routine_id, routine_revision),
  FOREIGN KEY(goal_id, revision, workspace_id) REFERENCES goal_revisions(goal_id, revision, workspace_id),
  FOREIGN KEY(routine_id, routine_revision, workspace_id) REFERENCES routine_revisions(routine_id, revision, workspace_id)
);
CREATE INDEX idx_goal_routine_links_routine ON goal_routine_links(routine_id, routine_revision, goal_id, revision);

CREATE TRIGGER goal_routine_link_current_revision_guard
BEFORE INSERT ON goal_routine_links
WHEN NOT EXISTS (
  SELECT 1 FROM goals g
  WHERE g.goal_id = NEW.goal_id
    AND g.workspace_id = NEW.workspace_id
    AND g.current_revision = NEW.revision
    AND g.status <> 'ARCHIVED'
)
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;

CREATE TRIGGER goal_routine_link_immutable_update
BEFORE UPDATE ON goal_routine_links
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;

CREATE TRIGGER goal_routine_link_immutable_delete
BEFORE DELETE ON goal_routine_links
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;

CREATE TABLE suggestions (
  suggestion_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  coworker_id TEXT,
  dedupe_key TEXT NOT NULL CHECK (length(dedupe_key) = 71 AND substr(dedupe_key, 1, 7) = 'sha256:' AND substr(dedupe_key, 8) NOT GLOB '*[^0-9a-f]*'),
  kind TEXT NOT NULL CHECK (kind IN ('TASK_OPPORTUNITY', 'ROUTINE_OPPORTUNITY', 'AUTOMATION_OPPORTUNITY')),
  reason TEXT NOT NULL,
  source_refs_json TEXT NOT NULL CHECK (json_valid(source_refs_json)),
  goal_refs_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(goal_refs_json)),
  proposed_action TEXT NOT NULL CHECK (proposed_action IN ('TASK', 'OPEN_ROUTINE_EDITOR', 'OPEN_AUTOMATION_EDITOR')),
  proposed_task_spec_json TEXT CHECK (proposed_task_spec_json IS NULL OR json_valid(proposed_task_spec_json)),
  estimated_cost_json TEXT CHECK (estimated_cost_json IS NULL OR json_valid(estimated_cost_json)),
  estimated_duration_class TEXT CHECK (estimated_duration_class IS NULL OR estimated_duration_class IN ('STANDARD', 'INTERACTIVE', 'DEADLINE_SENSITIVE')),
  status TEXT NOT NULL CHECK (status IN ('PROPOSED', 'ACCEPTED', 'DISMISSED', 'EXPIRED')),
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  snoozed_until TEXT,
  resolved_at TEXT,
  resolved_by_json TEXT CHECK (resolved_by_json IS NULL OR json_valid(resolved_by_json)),
  resolution_reason TEXT CHECK (resolution_reason IS NULL OR resolution_reason IN ('ACCEPTED_BY_OWNER', 'DISMISSED_BY_OWNER', 'MUTED_KIND', 'SYSTEM_EXPIRY')),
  result_task_id TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(workspace_id, coworker_id) REFERENCES coworkers(workspace_id, coworker_id),
  FOREIGN KEY(result_task_id, workspace_id) REFERENCES tasks(task_id, workspace_id),
  CHECK ((proposed_action = 'TASK' AND kind = 'TASK_OPPORTUNITY') OR
         (proposed_action = 'OPEN_ROUTINE_EDITOR' AND kind = 'ROUTINE_OPPORTUNITY') OR
         (proposed_action = 'OPEN_AUTOMATION_EDITOR' AND kind = 'AUTOMATION_OPPORTUNITY')),
  CHECK (created_at < expires_at),
  CHECK ((status = 'PROPOSED' AND resolved_at IS NULL AND resolved_by_json IS NULL AND resolution_reason IS NULL) OR
         (status <> 'PROPOSED' AND resolved_at IS NOT NULL AND resolved_by_json IS NOT NULL AND resolution_reason IS NOT NULL)),
  CHECK ((status = 'PROPOSED') OR
         (status = 'ACCEPTED' AND resolution_reason = 'ACCEPTED_BY_OWNER') OR
         (status = 'DISMISSED' AND resolution_reason IN ('DISMISSED_BY_OWNER', 'MUTED_KIND')) OR
         (status = 'EXPIRED' AND resolution_reason = 'SYSTEM_EXPIRY')),
  CHECK (snoozed_until IS NULL OR (status = 'PROPOSED' AND snoozed_until <= expires_at)),
  CHECK ((proposed_action = 'TASK' AND proposed_task_spec_json IS NOT NULL) OR (proposed_action <> 'TASK' AND proposed_task_spec_json IS NULL)),
  CHECK ((result_task_id IS NULL OR (proposed_action = 'TASK' AND status = 'ACCEPTED')) AND
         (status <> 'ACCEPTED' OR proposed_action <> 'TASK' OR result_task_id IS NOT NULL))
);

CREATE TRIGGER suggestion_proposal_immutable
BEFORE UPDATE ON suggestions
WHEN NEW.suggestion_id IS NOT OLD.suggestion_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.coworker_id IS NOT OLD.coworker_id
  OR NEW.dedupe_key IS NOT OLD.dedupe_key
  OR NEW.kind IS NOT OLD.kind
  OR NEW.reason IS NOT OLD.reason
  OR NEW.source_refs_json IS NOT OLD.source_refs_json
  OR NEW.goal_refs_json IS NOT OLD.goal_refs_json
  OR NEW.proposed_action IS NOT OLD.proposed_action
  OR NEW.proposed_task_spec_json IS NOT OLD.proposed_task_spec_json
  OR NEW.estimated_cost_json IS NOT OLD.estimated_cost_json
  OR NEW.estimated_duration_class IS NOT OLD.estimated_duration_class
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.expires_at IS NOT OLD.expires_at
BEGIN
  SELECT RAISE(ABORT, 'SUGGESTION_PROPOSAL_IMMUTABLE');
END;
CREATE INDEX idx_suggestions_workspace_status_created ON suggestions(workspace_id, status, created_at DESC);
CREATE UNIQUE INDEX uq_suggestions_open_dedupe ON suggestions(workspace_id, dedupe_key) WHERE status = 'PROPOSED';
CREATE INDEX idx_suggestions_dismissal_cooldown ON suggestions(workspace_id, dedupe_key, resolved_at DESC) WHERE status = 'DISMISSED';

CREATE TABLE suggestion_preferences (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  kind TEXT NOT NULL CHECK (kind IN ('TASK_OPPORTUNITY', 'ROUTINE_OPPORTUNITY', 'AUTOMATION_OPPORTUNITY')),
  muted INTEGER NOT NULL CHECK (muted IN (0, 1)),
  updated_by_json TEXT NOT NULL CHECK (json_valid(updated_by_json)),
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
  PRIMARY KEY(workspace_id, kind)
);

CREATE TABLE demonstration_sessions (
  demonstration_session_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  environment_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('CREATED', 'CAPTURING', 'REVIEW', 'CONVERTED', 'ABORTED')),
  started_at TEXT NOT NULL,
  completed_at TEXT,
  trace_resource_id TEXT,
  skill_proposal_id TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(environment_id, workspace_id) REFERENCES environments(environment_id, owner_workspace_id),
  FOREIGN KEY(workspace_id, trace_resource_id) REFERENCES resources(workspace_id, resource_id),
  FOREIGN KEY(workspace_id, skill_proposal_id) REFERENCES skill_proposals(workspace_id, skill_proposal_id),
  CHECK ((status = 'CONVERTED' AND skill_proposal_id IS NOT NULL) OR status <> 'CONVERTED')
);
CREATE INDEX idx_demonstration_sessions_workspace_status ON demonstration_sessions(workspace_id, status, started_at DESC);

CREATE TABLE task_spec_revisions (
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  revision INTEGER NOT NULL,
  parent_revisions_json TEXT NOT NULL DEFAULT '[]',
  objective TEXT NOT NULL,
  task_category TEXT CHECK (task_category IS NULL OR task_category IN ('SOFTWARE_ENGINEERING', 'RESEARCH', 'WRITING', 'DATA_ANALYSIS', 'OFFICE', 'BROWSER', 'PERSONAL_ADMIN', 'OTHER')),
  constraints_json TEXT NOT NULL DEFAULT '[]',
  non_goals_json TEXT NOT NULL DEFAULT '[]',
  input_refs_json TEXT NOT NULL DEFAULT '[]',
  workspace_instruction_revision INTEGER,
  required_outputs_json TEXT NOT NULL DEFAULT '[]',
  acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
  approvals_required_json TEXT NOT NULL DEFAULT '[]',
  budget_json TEXT,
  delegation_budget_policy_json TEXT CHECK (delegation_budget_policy_json IS NULL OR json_valid(delegation_budget_policy_json)),
  deadline TEXT,
  source_message_refs_json TEXT NOT NULL DEFAULT '[]',
  placement_preference TEXT NOT NULL,
  preferred_lead_agent_binding_id TEXT REFERENCES agent_bindings(agent_binding_id),
  authored_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (task_id, revision),
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(workspace_id, preferred_lead_agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(workspace_id, workspace_instruction_revision) REFERENCES workspace_instruction_revisions(workspace_id, revision) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE plan_revisions (
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  revision INTEGER NOT NULL,
  task_spec_revision INTEGER NOT NULL,
  produced_by_agent_session_id TEXT NOT NULL,
  produced_by_attempt_id TEXT,
  steps_json TEXT NOT NULL,
  reason_for_revision TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (task_id, revision),
  FOREIGN KEY(task_id, task_spec_revision) REFERENCES task_spec_revisions(task_id, revision) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(task_id, produced_by_agent_session_id) REFERENCES agent_sessions(task_id, agent_session_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(task_id, produced_by_attempt_id, produced_by_agent_session_id) REFERENCES agent_sessions(task_id, attempt_id, agent_session_id) DEFERRABLE INITIALLY DEFERRED
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
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'READY', 'RUNNING', 'WAITING_USER', 'BLOCKED', 'VERIFYING', 'COMPLETED', 'FAILED', 'CANCEL_REQUESTED', 'CANCELLED', 'SUPERSEDED')),
  current_attempt_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(task_id, step_id),
  FOREIGN KEY(task_id, plan_revision) REFERENCES plan_revisions(task_id, revision) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(task_id, current_attempt_id) REFERENCES attempts(task_id, attempt_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_steps_task_status ON steps(task_id, status);

CREATE TABLE agent_profiles (
  agent_profile_id TEXT PRIMARY KEY,
  provider_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  discovered_at TEXT NOT NULL
);

CREATE TABLE agent_endpoints (
  endpoint_id TEXT PRIMARY KEY,
  agent_profile_id TEXT NOT NULL REFERENCES agent_profiles(agent_profile_id),
  protocol TEXT NOT NULL CHECK (protocol IN ('ACP', 'A2A', 'SDK', 'API', 'CLI', 'TERMINAL')),
  topology TEXT NOT NULL CHECK (topology IN ('LOCAL_INTERACTIVE', 'REMOTE_AGENT_SERVICE', 'VENDOR_SERVICE', 'PROCESS_ADAPTER')),
  protocol_version TEXT,
  capabilities_json TEXT NOT NULL,
  UNIQUE(agent_profile_id, endpoint_id)
);

CREATE TABLE agent_bindings (
  agent_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  agent_profile_id TEXT NOT NULL REFERENCES agent_profiles(agent_profile_id),
  runtime_id TEXT REFERENCES runtimes(runtime_id),
  endpoint_selection_policy_json TEXT NOT NULL,
  auth_ref TEXT,
  configuration_json TEXT NOT NULL DEFAULT '{}',
  enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
  lead_eligible INTEGER NOT NULL DEFAULT 1 CHECK (lead_eligible IN (0, 1)),
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, agent_binding_id)
);

CREATE TRIGGER workspace_default_agent_binding_insert_guard
BEFORE INSERT ON workspaces
WHEN NEW.default_agent_binding_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM agent_bindings b
  WHERE b.workspace_id = NEW.workspace_id
    AND b.agent_binding_id = NEW.default_agent_binding_id
    AND b.enabled = 1
    AND b.lead_eligible = 1
)
BEGIN
  SELECT RAISE(ABORT, 'DEFAULT_AGENT_BINDING_NOT_LEAD_ELIGIBLE');
END;

CREATE TRIGGER workspace_default_agent_binding_update_guard
BEFORE UPDATE OF default_agent_binding_id, workspace_id ON workspaces
WHEN NEW.default_agent_binding_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM agent_bindings b
  WHERE b.workspace_id = NEW.workspace_id
    AND b.agent_binding_id = NEW.default_agent_binding_id
    AND b.enabled = 1
    AND b.lead_eligible = 1
)
BEGIN
  SELECT RAISE(ABORT, 'DEFAULT_AGENT_BINDING_NOT_LEAD_ELIGIBLE');
END;

CREATE TRIGGER agent_binding_default_lead_guard
BEFORE UPDATE OF enabled, lead_eligible ON agent_bindings
WHEN (NEW.enabled = 0 OR NEW.lead_eligible = 0) AND EXISTS (
  SELECT 1 FROM workspaces w
  WHERE w.workspace_id = OLD.workspace_id
    AND w.default_agent_binding_id = OLD.agent_binding_id
)
BEGIN
  SELECT RAISE(ABORT, 'DEFAULT_AGENT_BINDING_MUST_BE_CHANGED');
END;

CREATE TABLE delegation_profiles (
  delegation_profile_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  agent_binding_id TEXT NOT NULL,
  name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 120),
  name_key TEXT NOT NULL CHECK (length(name_key) > 0),
  current_revision INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ENABLED', 'DISABLED', 'ARCHIVED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, delegation_profile_id),
  FOREIGN KEY(workspace_id, agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id),
  FOREIGN KEY(delegation_profile_id, current_revision) REFERENCES delegation_profile_revisions(delegation_profile_id, revision) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_delegation_profiles_workspace_status ON delegation_profiles(workspace_id, status);
CREATE INDEX idx_delegation_profiles_binding_status ON delegation_profiles(agent_binding_id, status);
CREATE UNIQUE INDEX uq_delegation_profiles_binding_name ON delegation_profiles(workspace_id, agent_binding_id, name_key) WHERE status <> 'ARCHIVED';

CREATE TRIGGER delegation_profile_identity_immutable
BEFORE UPDATE ON delegation_profiles
WHEN NEW.delegation_profile_id IS NOT OLD.delegation_profile_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.agent_binding_id IS NOT OLD.agent_binding_id
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'DELEGATION_PROFILE_IDENTITY_IMMUTABLE');
END;

CREATE TABLE delegation_profile_revisions (
  delegation_profile_id TEXT NOT NULL REFERENCES delegation_profiles(delegation_profile_id),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 120),
  name_key TEXT NOT NULL CHECK (length(name_key) > 0),
  routing_description TEXT NOT NULL CHECK (length(routing_description) BETWEEN 1 AND 240),
  instructions TEXT,
  session_options_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(session_options_json)),
  session_options_descriptor_digest TEXT CHECK (session_options_descriptor_digest IS NULL OR (length(session_options_descriptor_digest) = 71 AND substr(session_options_descriptor_digest, 1, 7) = 'sha256:' AND substr(session_options_descriptor_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  required_features_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(required_features_json)),
  preferred_features_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(preferred_features_json)),
  enforced_policy_json TEXT NOT NULL CHECK (json_valid(enforced_policy_json)),
  optimization_preference TEXT NOT NULL CHECK (optimization_preference IN ('QUALITY_FIRST', 'BALANCED', 'COST_FIRST', 'LATENCY_FIRST')),
  quality_floor_json TEXT CHECK (quality_floor_json IS NULL OR json_valid(quality_floor_json)),
  max_concurrency INTEGER NOT NULL CHECK (max_concurrency BETWEEN 1 AND 8),
  max_host_delegation_depth INTEGER NOT NULL CHECK (max_host_delegation_depth BETWEEN 0 AND 2),
  budget_ceiling_json TEXT CHECK (budget_ceiling_json IS NULL OR json_valid(budget_ceiling_json)),
  latency_class TEXT NOT NULL CHECK (latency_class IN ('STANDARD', 'INTERACTIVE', 'DEADLINE_SENSITIVE')),
  environment_policy_json TEXT NOT NULL CHECK (json_valid(environment_policy_json)),
  native_delegation_policy TEXT NOT NULL CHECK (native_delegation_policy IN ('INHERIT', 'ALLOW', 'DENY_IF_SUPPORTED')),
  warm_policy_json TEXT NOT NULL CHECK (json_valid(warm_policy_json)),
  authored_by_json TEXT NOT NULL CHECK (json_valid(authored_by_json)),
  created_at TEXT NOT NULL,
  PRIMARY KEY(delegation_profile_id, revision),
  UNIQUE(delegation_profile_id, revision, workspace_id),
  FOREIGN KEY(workspace_id, delegation_profile_id) REFERENCES delegation_profiles(workspace_id, delegation_profile_id)
);

CREATE TRIGGER delegation_profile_revision_immutable_update
BEFORE UPDATE ON delegation_profile_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_DELEGATION_PROFILE_REVISION');
END;

CREATE TRIGGER delegation_profile_revision_immutable_delete
BEFORE DELETE ON delegation_profile_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_DELEGATION_PROFILE_REVISION');
END;

CREATE TRIGGER delegation_profile_revision_head_name_guard
AFTER INSERT ON delegation_profile_revisions
WHEN EXISTS (
  SELECT 1 FROM delegation_profiles p
  WHERE p.delegation_profile_id = NEW.delegation_profile_id
    AND p.current_revision = NEW.revision
    AND (p.name <> NEW.name OR p.name_key <> NEW.name_key)
)
BEGIN
  SELECT RAISE(ABORT, 'INTEGRITY_FAILURE');
END;

CREATE TRIGGER delegation_profile_head_name_guard
BEFORE UPDATE OF current_revision, name, name_key ON delegation_profiles
WHEN NOT EXISTS (
  SELECT 1 FROM delegation_profile_revisions r
  WHERE r.delegation_profile_id = NEW.delegation_profile_id
    AND r.revision = NEW.current_revision
    AND r.name = NEW.name
    AND r.name_key = NEW.name_key
)
BEGIN
  SELECT RAISE(ABORT, 'INTEGRITY_FAILURE');
END;

CREATE TRIGGER delegation_profile_options_guard
BEFORE INSERT ON delegation_profile_revisions
WHEN json_type(NEW.session_options_json) <> 'object'
  OR length(CAST(NEW.session_options_json AS BLOB)) > 65536
  OR (SELECT COUNT(*) FROM json_each(NEW.session_options_json)) > 64
  OR ((SELECT COUNT(*) FROM json_each(NEW.session_options_json)) = 0
      AND NEW.session_options_descriptor_digest IS NOT NULL)
  OR ((SELECT COUNT(*) FROM json_each(NEW.session_options_json)) > 0
      AND NEW.session_options_descriptor_digest IS NULL)
BEGIN
  SELECT RAISE(ABORT, 'DELEGATION_PROFILE_OPTIONS_INVALID');
END;

CREATE TABLE coworkers (
  coworker_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  current_revision INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'PAUSED', 'ARCHIVED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, coworker_id),
  FOREIGN KEY(coworker_id, current_revision) REFERENCES coworker_revisions(coworker_id, revision) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_coworkers_workspace_status ON coworkers(workspace_id, status, updated_at DESC);

CREATE TABLE coworker_revisions (
  coworker_id TEXT NOT NULL REFERENCES coworkers(coworker_id),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
  avatar_ref_json TEXT CHECK (avatar_ref_json IS NULL OR json_valid(avatar_ref_json)),
  role_description TEXT NOT NULL,
  default_lead_agent_binding_id TEXT,
  delegation_strategy TEXT NOT NULL CHECK (delegation_strategy IN ('NATIVE_DEFAULT', 'BALANCED', 'COST_SAVER', 'HOST_DELEGATION_ONLY')),
  enabled_delegation_profile_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(enabled_delegation_profile_ids_json)),
  delegation_budget_policy_json TEXT CHECK (delegation_budget_policy_json IS NULL OR json_valid(delegation_budget_policy_json)),
  context_policy_json TEXT NOT NULL CHECK (json_valid(context_policy_json)),
  notification_policy_json TEXT NOT NULL CHECK (json_valid(notification_policy_json)),
  authored_by_json TEXT NOT NULL CHECK (json_valid(authored_by_json)),
  created_at TEXT NOT NULL,
  PRIMARY KEY(coworker_id, revision),
  UNIQUE(coworker_id, revision, workspace_id),
  FOREIGN KEY(workspace_id, coworker_id) REFERENCES coworkers(workspace_id, coworker_id),
  FOREIGN KEY(workspace_id, default_lead_agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id)
);

CREATE TRIGGER coworker_revision_worker_scope_guard
BEFORE INSERT ON coworker_revisions
WHEN
  (NEW.default_lead_agent_binding_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM agent_bindings b
    WHERE b.workspace_id = NEW.workspace_id
      AND b.agent_binding_id = NEW.default_lead_agent_binding_id
      AND b.enabled = 1
      AND b.lead_eligible = 1
  ))
  OR EXISTS (
    SELECT 1
    FROM json_each(NEW.enabled_delegation_profile_ids_json) selected
    LEFT JOIN delegation_profiles p
      ON p.delegation_profile_id = selected.value
     AND p.workspace_id = NEW.workspace_id
     AND p.status = 'ENABLED'
    WHERE p.delegation_profile_id IS NULL
  )
BEGIN
  SELECT RAISE(ABORT, 'COWORKER_REVISION_WORKER_SCOPE_MISMATCH');
END;

CREATE TRIGGER coworker_revision_immutable_update
BEFORE UPDATE ON coworker_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_COWORKER_REVISION');
END;

CREATE TRIGGER coworker_revision_immutable_delete
BEFORE DELETE ON coworker_revisions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_COWORKER_REVISION');
END;

CREATE TRIGGER coworker_identity_immutable
BEFORE UPDATE ON coworkers
WHEN NEW.coworker_id IS NOT OLD.coworker_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'COWORKER_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER workspace_primary_coworker_state_insert_guard
BEFORE INSERT ON workspaces
WHEN NEW.primary_coworker_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM coworkers c
  WHERE c.workspace_id = NEW.workspace_id
    AND c.coworker_id = NEW.primary_coworker_id
    AND c.status IN ('ACTIVE', 'PAUSED')
)
BEGIN
  SELECT RAISE(ABORT, 'PRIMARY_COWORKER_NOT_SELECTABLE');
END;

CREATE TRIGGER workspace_primary_coworker_state_update_guard
BEFORE UPDATE OF primary_coworker_id, workspace_id ON workspaces
WHEN NEW.primary_coworker_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM coworkers c
  WHERE c.workspace_id = NEW.workspace_id
    AND c.coworker_id = NEW.primary_coworker_id
    AND c.status IN ('ACTIVE', 'PAUSED')
)
BEGIN
  SELECT RAISE(ABORT, 'PRIMARY_COWORKER_NOT_SELECTABLE');
END;

CREATE TRIGGER primary_coworker_archive_guard
BEFORE UPDATE OF status ON coworkers
WHEN NEW.status = 'ARCHIVED' AND EXISTS (
  SELECT 1 FROM workspaces w
  WHERE w.workspace_id = OLD.workspace_id
    AND w.primary_coworker_id = OLD.coworker_id
)
BEGIN
  SELECT RAISE(ABORT, 'PRIMARY_COWORKER_MUST_BE_CLEARED');
END;

CREATE TRIGGER task_coworker_origin_immutable
BEFORE UPDATE OF origin_coworker_id, origin_coworker_revision ON tasks
WHEN OLD.origin_coworker_id IS NOT NEW.origin_coworker_id
  OR OLD.origin_coworker_revision IS NOT NEW.origin_coworker_revision
BEGIN
  SELECT RAISE(ABORT, 'TASK_COWORKER_ORIGIN_IMMUTABLE');
END;

CREATE TRIGGER task_coworker_origin_not_archived
BEFORE INSERT ON tasks
WHEN NEW.origin_coworker_id IS NOT NULL AND EXISTS (
  SELECT 1 FROM coworkers c
  WHERE c.workspace_id = NEW.workspace_id
    AND c.coworker_id = NEW.origin_coworker_id
    AND c.status = 'ARCHIVED'
)
BEGIN
  SELECT RAISE(ABORT, 'COWORKER_ARCHIVED');
END;

CREATE TABLE agent_host_instances (
  host_instance_id TEXT PRIMARY KEY,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  agent_profile_id TEXT NOT NULL REFERENCES agent_profiles(agent_profile_id),
  endpoint_id TEXT NOT NULL REFERENCES agent_endpoints(endpoint_id),
  hosting_mode TEXT NOT NULL CHECK (hosting_mode IN ('REMOTE_API', 'REMOTE_A2A', 'LOCAL_SHARED_DAEMON', 'LOCAL_PER_SESSION', 'EMBEDDED_SDK', 'EXTERNAL_PROCESS')),
  state TEXT NOT NULL CHECK (state IN ('STARTING', 'READY', 'BUSY', 'DEGRADED', 'STOPPING', 'STOPPED', 'FAILED')),
  process_identity_ref TEXT,
  ownership TEXT NOT NULL CHECK (ownership IN ('LITECOWORK', 'EXTERNAL', 'REMOTE')),
  started_at TEXT NOT NULL,
  last_used_at TEXT NOT NULL,
  idle_since TEXT,
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_agent_host_instances_runtime_state ON agent_host_instances(runtime_id, state);

CREATE TABLE runtimes (
  runtime_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  device_identity_json TEXT NOT NULL,
  runtime_version TEXT NOT NULL,
  platform TEXT NOT NULL,
  architecture TEXT NOT NULL,
  roles_json TEXT NOT NULL,
  trust_zone TEXT NOT NULL,
  availability TEXT NOT NULL CHECK (availability IN ('PAIRING', 'STARTING', 'RECOVERING', 'ONLINE', 'DEGRADED', 'DRAINING', 'OFFLINE', 'REVOKED')),
  startup_policy TEXT NOT NULL CHECK (startup_policy IN ('MANUAL', 'LOGIN_BACKGROUND', 'ALWAYS_ON_SERVICE')),
  current_incarnation_id TEXT,
  resource_capacity_json TEXT NOT NULL DEFAULT '{}',
  last_seen TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(runtime_id, workspace_id),
  FOREIGN KEY(runtime_id, current_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_runtimes_workspace_availability ON runtimes(workspace_id, availability);

CREATE TABLE runtime_incarnations (
  runtime_incarnation_id TEXT PRIMARY KEY,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  process_started_at TEXT NOT NULL,
  litecowork_version TEXT NOT NULL,
  recovered_from_unclean_shutdown INTEGER NOT NULL CHECK (recovered_from_unclean_shutdown IN (0, 1)),
  recovery_state TEXT NOT NULL CHECK (recovery_state IN ('STARTING', 'RECOVERING', 'READY', 'DEGRADED', 'DRAINING', 'STOPPING', 'STOPPED')),
  ready_at TEXT,
  stopped_at TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(runtime_id, runtime_incarnation_id)
);
CREATE INDEX idx_runtime_incarnations_runtime_started ON runtime_incarnations(runtime_id, process_started_at DESC);

CREATE TABLE runtime_incarnation_local_observations (
  runtime_incarnation_id TEXT PRIMARY KEY REFERENCES runtime_incarnations(runtime_incarnation_id),
  os_boot_id TEXT,
  observed_at TEXT NOT NULL,
  diagnostic_ref_json TEXT
);

-- Endpoint command/socket/URL locators are local configuration and are refreshed per
-- daemon incarnation; stable AgentEndpoint identity contains no executable locator.
CREATE TABLE agent_endpoint_bindings (
  endpoint_id TEXT NOT NULL REFERENCES agent_endpoints(endpoint_id),
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  endpoint_ref TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  expires_at TEXT,
  PRIMARY KEY(endpoint_id, runtime_incarnation_id),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
);
CREATE INDEX idx_agent_endpoint_bindings_runtime ON agent_endpoint_bindings(runtime_id, runtime_incarnation_id, endpoint_id);

CREATE TRIGGER agent_endpoint_binding_matches_current_incarnation
BEFORE INSERT ON agent_endpoint_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM runtimes r
  WHERE r.runtime_id = NEW.runtime_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'AGENT_ENDPOINT_BINDING_MISMATCH');
END;

CREATE TRIGGER agent_endpoint_binding_identity_immutable
BEFORE UPDATE ON agent_endpoint_bindings
WHEN NEW.endpoint_id IS NOT OLD.endpoint_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.runtime_incarnation_id IS NOT OLD.runtime_incarnation_id
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_AGENT_ENDPOINT_BINDING_IDENTITY');
END;

CREATE TRIGGER agent_endpoint_binding_delete_requires_drained_host
BEFORE DELETE ON agent_endpoint_bindings
WHEN EXISTS (
  SELECT 1 FROM agent_host_instances h
  LEFT JOIN agent_session_host_bindings b ON b.host_instance_id = h.host_instance_id
  LEFT JOIN agent_sessions s ON s.agent_session_id = b.agent_session_id
  WHERE h.endpoint_id = OLD.endpoint_id
    AND h.runtime_id = OLD.runtime_id
    AND h.runtime_incarnation_id = OLD.runtime_incarnation_id
    AND (h.state NOT IN ('STOPPED', 'FAILED') OR s.status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING'))
)
BEGIN
  SELECT RAISE(ABORT, 'AGENT_ENDPOINT_HOST_NOT_DRAINED');
END;
CREATE TABLE runtime_offers (
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  offer_kind TEXT NOT NULL CHECK (offer_kind IN ('AGENT_ENDPOINT', 'CAPABILITY_PROVIDER', 'ENVIRONMENT_PROVIDER', 'CHANNEL_ADAPTER', 'TRIGGER_PROVIDER', 'APPLICATION')),
  offer_ref TEXT NOT NULL,
  compatible INTEGER NOT NULL CHECK (compatible IN (0, 1)),
  readiness TEXT NOT NULL CHECK (readiness IN ('AVAILABLE', 'STARTABLE', 'STARTING', 'READY', 'BUSY', 'DEGRADED', 'OFFLINE', 'NEEDS_AUTH', 'UNAVAILABLE')),
  constraints_json TEXT NOT NULL DEFAULT '{}',
  observed_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  PRIMARY KEY(runtime_id, runtime_incarnation_id, offer_kind, offer_ref),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_runtime_offers_expiry ON runtime_offers(expires_at);

CREATE TABLE pairing_tokens (
  token_digest TEXT PRIMARY KEY CHECK (length(token_digest) = 71 AND substr(token_digest, 1, 7) = 'sha256:' AND substr(token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
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
  lifetime TEXT NOT NULL CHECK (lifetime IN ('ATTEMPT', 'TASK_RETAINED', 'WORKSPACE_PERSISTENT')),
  owner_workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  owner_task_id TEXT REFERENCES tasks(task_id),
  owner_attempt_id TEXT,
  owner_coworker_id TEXT,
  owner_principal_id TEXT,
  sharing_scope TEXT NOT NULL DEFAULT 'ATTEMPT_PRIVATE' CHECK (sharing_scope IN ('ATTEMPT_PRIVATE', 'TASK_SHARED', 'COWORKER_PRIVATE', 'WORKSPACE_SHARED', 'USER_SHARED')),
  name TEXT NOT NULL,
  created_by_incarnation_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('NEW', 'PROVISIONING', 'READY', 'BUSY', 'CHECKPOINTING', 'SUSPENDED', 'FAILED', 'DESTROYING', 'DESTROYED')),
  health TEXT NOT NULL CHECK (health IN ('HEALTHY', 'DEGRADED', 'UNHEALTHY', 'UNKNOWN')),
  budget_enforcement_policy TEXT NOT NULL CHECK (budget_enforcement_policy IN ('REQUIRE_PROVIDER_ENFORCED', 'ALLOW_HOST_MONITORED')),
  budget_enforcement TEXT NOT NULL CHECK (budget_enforcement IN ('PROVIDER_ENFORCED', 'HOST_MONITORED', 'UNAVAILABLE')),
  source_resources_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(source_resources_json)),
  resource_limits_json TEXT NOT NULL CHECK (json_valid(resource_limits_json)),
  network_policy_json TEXT NOT NULL CHECK (json_valid(network_policy_json)),
  budget_ceiling_json TEXT NOT NULL CHECK (json_valid(budget_ceiling_json)),
  provision_preview_digest TEXT CHECK (provision_preview_digest IS NULL OR (length(provision_preview_digest) = 71 AND substr(provision_preview_digest, 1, 7) = 'sha256:' AND substr(provision_preview_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  retention_expires_at TEXT,
  backup_policy TEXT NOT NULL CHECK (backup_policy IN ('EXCLUDED', 'INCLUDE_CHECKPOINTS')),
  isolation_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(runtime_id, created_by_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(runtime_id, owner_workspace_id) REFERENCES runtimes(runtime_id, workspace_id),
  FOREIGN KEY(owner_task_id, owner_workspace_id) REFERENCES tasks(task_id, workspace_id),
  FOREIGN KEY(owner_task_id, owner_attempt_id) REFERENCES attempts(task_id, attempt_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(owner_workspace_id, owner_coworker_id) REFERENCES coworkers(workspace_id, coworker_id),
  UNIQUE(provision_preview_digest),
  UNIQUE(environment_id, owner_workspace_id),
  UNIQUE(environment_id, runtime_id),
  UNIQUE(environment_id, runtime_id, provider_kind),
  CHECK ((lifetime IN ('ATTEMPT', 'TASK_RETAINED') AND owner_task_id IS NOT NULL) OR
         (lifetime = 'WORKSPACE_PERSISTENT' AND owner_task_id IS NULL)),
  CHECK (
    (sharing_scope = 'ATTEMPT_PRIVATE' AND owner_task_id IS NOT NULL AND owner_attempt_id IS NOT NULL AND owner_coworker_id IS NULL AND owner_principal_id IS NULL) OR
    (sharing_scope = 'TASK_SHARED' AND owner_task_id IS NOT NULL AND owner_attempt_id IS NULL AND owner_coworker_id IS NULL AND owner_principal_id IS NULL) OR
    (sharing_scope = 'COWORKER_PRIVATE' AND owner_task_id IS NULL AND owner_attempt_id IS NULL AND owner_coworker_id IS NOT NULL AND owner_principal_id IS NULL) OR
    (sharing_scope = 'WORKSPACE_SHARED' AND owner_task_id IS NULL AND owner_attempt_id IS NULL AND owner_coworker_id IS NULL AND owner_principal_id IS NULL) OR
    (sharing_scope = 'USER_SHARED' AND owner_attempt_id IS NULL AND owner_task_id IS NULL AND owner_coworker_id IS NULL AND owner_principal_id IS NOT NULL)
  ),
  CHECK ((lifetime = 'WORKSPACE_PERSISTENT' AND provision_preview_digest IS NOT NULL) OR
         (lifetime <> 'WORKSPACE_PERSISTENT' AND provision_preview_digest IS NULL))
);

CREATE TRIGGER environment_identity_immutable
BEFORE UPDATE ON environments
WHEN NEW.environment_id IS NOT OLD.environment_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.provider_kind IS NOT OLD.provider_kind
  OR NEW.class IS NOT OLD.class
  OR NEW.lifetime IS NOT OLD.lifetime
  OR NEW.owner_workspace_id IS NOT OLD.owner_workspace_id
  OR NEW.owner_task_id IS NOT OLD.owner_task_id
  OR NEW.owner_attempt_id IS NOT OLD.owner_attempt_id
  OR NEW.name IS NOT OLD.name
  OR NEW.created_by_incarnation_id IS NOT OLD.created_by_incarnation_id
  OR NEW.budget_enforcement_policy IS NOT OLD.budget_enforcement_policy
  OR NEW.source_resources_json IS NOT OLD.source_resources_json
  OR NEW.resource_limits_json IS NOT OLD.resource_limits_json
  OR NEW.network_policy_json IS NOT OLD.network_policy_json
  OR NEW.budget_ceiling_json IS NOT OLD.budget_ceiling_json
  OR NEW.provision_preview_digest IS NOT OLD.provision_preview_digest
  OR NEW.retention_expires_at IS NOT OLD.retention_expires_at
  OR NEW.backup_policy IS NOT OLD.backup_policy
  OR NEW.isolation_json IS NOT OLD.isolation_json
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'ENVIRONMENT_IDENTITY_IMMUTABLE');
END;

-- Provider locators are Runtime-local and incarnation-scoped. The Environment's
-- durable row contains identity/configuration only, never a provider-native handle.
CREATE TABLE environment_provider_bindings (
  environment_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  provider_kind TEXT NOT NULL,
  opaque_locator_ref TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  expires_at TEXT,
  PRIMARY KEY(environment_id, runtime_incarnation_id),
  FOREIGN KEY(environment_id, runtime_id, provider_kind) REFERENCES environments(environment_id, runtime_id, provider_kind),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
);
CREATE INDEX idx_environment_provider_bindings_incarnation ON environment_provider_bindings(runtime_id, runtime_incarnation_id, environment_id);

CREATE TRIGGER environment_provider_binding_matches_current_incarnation
BEFORE INSERT ON environment_provider_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM environments e
  JOIN runtimes r ON r.runtime_id = e.runtime_id
  WHERE e.environment_id = NEW.environment_id
    AND e.runtime_id = NEW.runtime_id
    AND e.provider_kind = NEW.provider_kind
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'ENVIRONMENT_PROVIDER_BINDING_MISMATCH');
END;

CREATE TRIGGER environment_provider_binding_identity_immutable
BEFORE UPDATE ON environment_provider_bindings
WHEN NEW.environment_id IS NOT OLD.environment_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.runtime_incarnation_id IS NOT OLD.runtime_incarnation_id
  OR NEW.provider_kind IS NOT OLD.provider_kind
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_ENVIRONMENT_PROVIDER_BINDING');
END;

CREATE TABLE environment_provision_previews (
  preview_digest TEXT PRIMARY KEY CHECK (length(preview_digest) = 71 AND substr(preview_digest, 1, 7) = 'sha256:' AND substr(preview_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  authenticated_principal_json TEXT NOT NULL CHECK (json_valid(authenticated_principal_json)),
  normalized_request_digest TEXT NOT NULL CHECK (length(normalized_request_digest) = 71 AND substr(normalized_request_digest, 1, 7) = 'sha256:' AND substr(normalized_request_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  eligibility_basis_json TEXT NOT NULL CHECK (json_valid(eligibility_basis_json)),
  expires_at TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ISSUED', 'CONSUMED', 'EXPIRED')),
  consumed_by_request_id TEXT,
  environment_id TEXT,
  created_at TEXT NOT NULL,
  UNIQUE(preview_digest, workspace_id),
  UNIQUE(workspace_id, consumed_by_request_id),
  FOREIGN KEY(environment_id, workspace_id) REFERENCES environments(environment_id, owner_workspace_id) DEFERRABLE INITIALLY DEFERRED,
  CHECK ((status = 'ISSUED' AND consumed_by_request_id IS NULL AND environment_id IS NULL) OR
         (status = 'CONSUMED' AND consumed_by_request_id IS NOT NULL AND environment_id IS NOT NULL) OR
         (status = 'EXPIRED' AND consumed_by_request_id IS NULL AND environment_id IS NULL))
);
CREATE INDEX idx_environment_provision_previews_expiry ON environment_provision_previews(status, expires_at);

CREATE TRIGGER environment_requires_issued_preview
BEFORE INSERT ON environments
WHEN NEW.lifetime = 'WORKSPACE_PERSISTENT' AND NOT EXISTS (
  SELECT 1 FROM environment_provision_previews p
  WHERE p.preview_digest = NEW.provision_preview_digest
    AND p.workspace_id = NEW.owner_workspace_id
    AND p.status = 'ISSUED'
    AND julianday(p.expires_at) > julianday('now')
)
BEGIN
  SELECT RAISE(ABORT, 'ENVIRONMENT_PREVIEW_INVALID');
END;

CREATE TRIGGER environment_preview_consumption_requires_environment
BEFORE UPDATE OF status, consumed_by_request_id, environment_id ON environment_provision_previews
WHEN NEW.status = 'CONSUMED' AND NOT EXISTS (
  SELECT 1 FROM environments e
  WHERE e.environment_id = NEW.environment_id
    AND e.owner_workspace_id = NEW.workspace_id
    AND e.provision_preview_digest = NEW.preview_digest
)
BEGIN
  SELECT RAISE(ABORT, 'ENVIRONMENT_PREVIEW_NOT_BOUND');
END;

CREATE TRIGGER environment_preview_transition_is_single_use
BEFORE UPDATE ON environment_provision_previews
WHEN OLD.status <> 'ISSUED'
  OR NEW.status NOT IN ('CONSUMED', 'EXPIRED')
  OR NEW.preview_digest IS NOT OLD.preview_digest
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.authenticated_principal_json IS NOT OLD.authenticated_principal_json
  OR NEW.normalized_request_digest IS NOT OLD.normalized_request_digest
  OR NEW.eligibility_basis_json IS NOT OLD.eligibility_basis_json
  OR NEW.expires_at IS NOT OLD.expires_at
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'ENVIRONMENT_PREVIEW_IMMUTABLE');
END;

CREATE TABLE environment_checkpoints (
  checkpoint_id TEXT PRIMARY KEY,
  environment_id TEXT NOT NULL REFERENCES environments(environment_id),
  digest TEXT NOT NULL CHECK (length(digest) = 71 AND substr(digest, 1, 7) = 'sha256:' AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'),
  portable_snapshot_ref_json TEXT CHECK (portable_snapshot_ref_json IS NULL OR (json_valid(portable_snapshot_ref_json) AND json_extract(portable_snapshot_ref_json, '$.digest') = digest AND json_type(portable_snapshot_ref_json, '$.size_bytes') = 'integer' AND json_type(portable_snapshot_ref_json, '$.media_type') = 'text')),
  created_at TEXT NOT NULL
);

CREATE TRIGGER environment_checkpoint_immutable
BEFORE UPDATE ON environment_checkpoints
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_ENVIRONMENT_CHECKPOINT');
END;

-- The provider-specific checkpoint handle is also local and incarnation-scoped.
CREATE TABLE environment_checkpoint_provider_bindings (
  checkpoint_id TEXT NOT NULL REFERENCES environment_checkpoints(checkpoint_id),
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  provider_ref TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  PRIMARY KEY(checkpoint_id, runtime_incarnation_id),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
);
CREATE INDEX idx_environment_checkpoint_bindings_incarnation ON environment_checkpoint_provider_bindings(runtime_id, runtime_incarnation_id, checkpoint_id);

CREATE TRIGGER environment_checkpoint_binding_matches_current_incarnation
BEFORE INSERT ON environment_checkpoint_provider_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM environment_checkpoints c
  JOIN environments e ON e.environment_id = c.environment_id
  JOIN runtimes r ON r.runtime_id = e.runtime_id
  WHERE c.checkpoint_id = NEW.checkpoint_id
    AND e.runtime_id = NEW.runtime_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'ENVIRONMENT_CHECKPOINT_BINDING_MISMATCH');
END;

CREATE TRIGGER environment_checkpoint_binding_immutable
BEFORE UPDATE ON environment_checkpoint_provider_bindings
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_ENVIRONMENT_CHECKPOINT_BINDING');
END;

CREATE TABLE attempts (
  attempt_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  step_id TEXT NOT NULL,
  parent_attempt_id TEXT,
  agent_binding_id TEXT NOT NULL REFERENCES agent_bindings(agent_binding_id),
  delegation_profile_id TEXT,
  delegation_profile_revision INTEGER,
  agent_session_id TEXT,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  runtime_incarnation_id TEXT NOT NULL,
  environment_id TEXT NOT NULL REFERENCES environments(environment_id),
  capability_grant_ids_json TEXT NOT NULL DEFAULT '[]',
  execution_lease_id TEXT,
  failover_class TEXT NOT NULL CHECK (failover_class IN ('SAFE_PORTABLE', 'REPLAYABLE', 'HANDOFF_REQUIRED', 'LOCAL_BOUND')),
  checkpoint_ref_json TEXT,
  status TEXT NOT NULL CHECK (status IN ('CREATED', 'PREPARING', 'RUNNING', 'WAITING_APPROVAL', 'WAITING_RESOURCE', 'CHECKPOINTING', 'COMPLETED', 'FAILED', 'ABANDONED', 'CANCEL_REQUESTED', 'CANCELLED')),
  failure_json TEXT,
  started_at TEXT,
  settled_at TEXT,
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(task_id, attempt_id),
  FOREIGN KEY(task_id, parent_attempt_id) REFERENCES attempts(task_id, attempt_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(delegation_profile_id, delegation_profile_revision) REFERENCES delegation_profile_revisions(delegation_profile_id, revision) DEFERRABLE INITIALLY DEFERRED,
  UNIQUE(attempt_id, runtime_id, runtime_incarnation_id),
  UNIQUE(task_id, attempt_id, runtime_id, runtime_incarnation_id),
  UNIQUE(task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id),
  UNIQUE(task_id, attempt_id, environment_id, runtime_id, runtime_incarnation_id),
  FOREIGN KEY(task_id, step_id) REFERENCES steps(task_id, step_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id, agent_binding_id) REFERENCES agent_sessions(task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id, agent_binding_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(environment_id, runtime_id) REFERENCES environments(environment_id, runtime_id) DEFERRABLE INITIALLY DEFERRED,
  CHECK ((parent_attempt_id IS NULL AND delegation_profile_id IS NULL AND delegation_profile_revision IS NULL) OR
         (parent_attempt_id IS NOT NULL AND delegation_profile_id IS NOT NULL AND delegation_profile_revision IS NOT NULL))
);
CREATE INDEX idx_attempts_task_step_status ON attempts(task_id, step_id, status);

CREATE TRIGGER attempt_delegation_profile_admission_guard
BEFORE INSERT ON attempts
WHEN NEW.parent_attempt_id IS NOT NULL AND NOT EXISTS (
  SELECT 1
  FROM tasks t
  JOIN delegation_profiles p
    ON p.delegation_profile_id = NEW.delegation_profile_id
   AND p.workspace_id = t.workspace_id
   AND p.agent_binding_id = NEW.agent_binding_id
   AND p.status = 'ENABLED'
  JOIN delegation_profile_revisions pr
    ON pr.delegation_profile_id = p.delegation_profile_id
   AND pr.revision = NEW.delegation_profile_revision
   AND pr.workspace_id = t.workspace_id
  WHERE t.task_id = NEW.task_id
    AND (
      t.origin_coworker_id IS NULL OR EXISTS (
        SELECT 1
        FROM coworker_revisions cr, json_each(cr.enabled_delegation_profile_ids_json) selected
        WHERE cr.coworker_id = t.origin_coworker_id
          AND cr.revision = t.origin_coworker_revision
          AND cr.workspace_id = t.workspace_id
          AND selected.value = NEW.delegation_profile_id
      )
    )
)
BEGIN
  SELECT RAISE(ABORT, 'ATTEMPT_DELEGATION_PROFILE_INELIGIBLE');
END;

CREATE TRIGGER attempt_environment_owner_guard
BEFORE INSERT ON attempts
WHEN NOT EXISTS (
  SELECT 1
  FROM tasks t
  JOIN environments e ON e.environment_id = NEW.environment_id
  JOIN agent_bindings b ON b.agent_binding_id = NEW.agent_binding_id
  JOIN runtimes r ON r.runtime_id = NEW.runtime_id
  WHERE t.task_id = NEW.task_id
    AND e.runtime_id = NEW.runtime_id
    AND e.owner_workspace_id = t.workspace_id
    AND (e.owner_task_id IS NULL OR e.owner_task_id = t.task_id)
    AND (e.sharing_scope <> 'ATTEMPT_PRIVATE' OR e.owner_attempt_id = NEW.attempt_id)
    AND (e.sharing_scope <> 'TASK_SHARED' OR e.owner_task_id = t.task_id)
    AND (e.sharing_scope <> 'COWORKER_PRIVATE' OR e.owner_coworker_id = t.origin_coworker_id)
    AND b.workspace_id = t.workspace_id
    AND r.workspace_id = t.workspace_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'ATTEMPT_ENVIRONMENT_SCOPE_MISMATCH');
END;

CREATE TRIGGER attempt_execution_identity_immutable
BEFORE UPDATE OF task_id, step_id, parent_attempt_id, agent_binding_id, delegation_profile_id, delegation_profile_revision, runtime_id, runtime_incarnation_id, environment_id ON attempts
WHEN OLD.task_id <> NEW.task_id
  OR OLD.step_id <> NEW.step_id
  OR OLD.parent_attempt_id IS NOT NEW.parent_attempt_id
  OR OLD.agent_binding_id <> NEW.agent_binding_id
  OR OLD.delegation_profile_id IS NOT NEW.delegation_profile_id
  OR OLD.delegation_profile_revision IS NOT NEW.delegation_profile_revision
  OR OLD.runtime_id <> NEW.runtime_id
  OR OLD.runtime_incarnation_id <> NEW.runtime_incarnation_id
  OR OLD.environment_id <> NEW.environment_id
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_ATTEMPT_EXECUTION_IDENTITY');
END;

CREATE TABLE agent_sessions (
  agent_session_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  scope_kind TEXT NOT NULL CHECK (scope_kind IN ('CONVERSATION', 'TASK_PLANNING', 'ATTEMPT_EXECUTION')),
  conversation_id TEXT REFERENCES conversations(conversation_id),
  conversation_turn_id TEXT,
  task_id TEXT REFERENCES tasks(task_id),
  task_spec_revision INTEGER,
  attempt_id TEXT,
  agent_binding_id TEXT NOT NULL,
  endpoint_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  configuration_digest TEXT CHECK (configuration_digest IS NULL OR (length(configuration_digest) = 71 AND substr(configuration_digest, 1, 7) = 'sha256:' AND substr(configuration_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  harness_descriptor_digest TEXT CHECK (harness_descriptor_digest IS NULL OR (length(harness_descriptor_digest) = 71 AND substr(harness_descriptor_digest, 1, 7) = 'sha256:' AND substr(harness_descriptor_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  status TEXT NOT NULL CHECK (status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING', 'CLOSED', 'LOST')),
  started_at TEXT NOT NULL,
  last_event_at TEXT,
  closed_at TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, agent_session_id),
  UNIQUE(task_id, agent_session_id),
  UNIQUE(task_id, attempt_id, agent_session_id),
  UNIQUE(conversation_id, conversation_turn_id, agent_session_id),
  UNIQUE(task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id),
  UNIQUE(task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id, agent_binding_id),
  FOREIGN KEY(workspace_id, conversation_id) REFERENCES conversations(workspace_id, conversation_id),
  FOREIGN KEY(conversation_id, conversation_turn_id) REFERENCES conversation_turns(conversation_id, turn_id),
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id),
  FOREIGN KEY(task_id, task_spec_revision) REFERENCES task_spec_revisions(task_id, revision),
  FOREIGN KEY(task_id, attempt_id) REFERENCES attempts(task_id, attempt_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(workspace_id, agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id),
  CHECK (
    (scope_kind = 'CONVERSATION' AND conversation_id IS NOT NULL AND conversation_turn_id IS NOT NULL AND task_id IS NULL AND task_spec_revision IS NULL AND attempt_id IS NULL) OR
    (scope_kind = 'TASK_PLANNING' AND conversation_id IS NULL AND conversation_turn_id IS NULL AND task_id IS NOT NULL AND task_spec_revision IS NOT NULL AND attempt_id IS NULL) OR
    (scope_kind = 'ATTEMPT_EXECUTION' AND conversation_id IS NULL AND conversation_turn_id IS NULL AND task_id IS NOT NULL AND task_spec_revision IS NOT NULL AND attempt_id IS NOT NULL)
  )
);
CREATE INDEX idx_agent_sessions_runtime_status ON agent_sessions(runtime_id, runtime_incarnation_id, status);
CREATE UNIQUE INDEX uq_active_task_planning_session ON agent_sessions(task_id)
  WHERE scope_kind = 'TASK_PLANNING' AND status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING');
CREATE UNIQUE INDEX uq_active_conversation_turn_session ON agent_sessions(conversation_turn_id)
  WHERE scope_kind = 'CONVERSATION' AND status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING');
CREATE INDEX idx_conversation_sessions ON agent_sessions(conversation_id, conversation_turn_id, status)
  WHERE scope_kind = 'CONVERSATION';

CREATE TRIGGER conversation_turn_agent_session_scope_guard_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.agent_session_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM agent_sessions s
  WHERE s.agent_session_id = NEW.agent_session_id
    AND s.scope_kind = 'CONVERSATION'
    AND s.conversation_id = NEW.conversation_id
    AND s.conversation_turn_id = NEW.turn_id
)
BEGIN
  SELECT RAISE(ABORT, 'CONVERSATION_TURN_AGENT_SESSION_SCOPE_MISMATCH');
END;

CREATE TRIGGER conversation_turn_agent_session_scope_guard
BEFORE UPDATE OF agent_session_id ON conversation_turns
WHEN NEW.agent_session_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM agent_sessions s
  WHERE s.agent_session_id = NEW.agent_session_id
    AND s.scope_kind = 'CONVERSATION'
    AND s.conversation_id = NEW.conversation_id
    AND s.conversation_turn_id = NEW.turn_id
)
BEGIN
  SELECT RAISE(ABORT, 'CONVERSATION_TURN_AGENT_SESSION_SCOPE_MISMATCH');
END;

CREATE TABLE agent_session_host_bindings (
  agent_session_id TEXT PRIMARY KEY REFERENCES agent_sessions(agent_session_id),
  host_instance_id TEXT NOT NULL REFERENCES agent_host_instances(host_instance_id),
  native_session_ref TEXT,
  bound_at TEXT NOT NULL
);
CREATE INDEX idx_agent_session_host_bindings_host ON agent_session_host_bindings(host_instance_id, agent_session_id);

CREATE TRIGGER agent_session_host_binding_matches_origin
BEFORE INSERT ON agent_session_host_bindings
WHEN NOT EXISTS (
  SELECT 1
  FROM agent_sessions s
  JOIN agent_host_instances h ON h.host_instance_id = NEW.host_instance_id
  JOIN agent_bindings b ON b.agent_binding_id = s.agent_binding_id
  WHERE s.agent_session_id = NEW.agent_session_id
    AND s.runtime_id = h.runtime_id
    AND s.runtime_incarnation_id = h.runtime_incarnation_id
    AND s.endpoint_id = h.endpoint_id
    AND b.agent_profile_id = h.agent_profile_id
    AND s.status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING')
)
BEGIN
  SELECT RAISE(ABORT, 'AGENT_SESSION_HOST_MISMATCH');
END;

CREATE TRIGGER agent_session_host_binding_immutable
BEFORE UPDATE ON agent_session_host_bindings
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_AGENT_SESSION_HOST_BINDING');
END;

CREATE TRIGGER agent_session_host_binding_delete_requires_settlement
BEFORE DELETE ON agent_session_host_bindings
WHEN (SELECT status FROM agent_sessions WHERE agent_session_id = OLD.agent_session_id)
  NOT IN ('CLOSED', 'LOST')
BEGIN
  SELECT RAISE(ABORT, 'AGENT_SESSION_HOST_STILL_ACTIVE');
END;

CREATE TRIGGER agent_session_identity_immutable
BEFORE UPDATE OF workspace_id, scope_kind, conversation_id, conversation_turn_id, task_id, task_spec_revision, attempt_id, endpoint_id, runtime_id, runtime_incarnation_id, agent_binding_id, configuration_digest ON agent_sessions
WHEN (
    OLD.workspace_id <> NEW.workspace_id
    OR OLD.scope_kind <> NEW.scope_kind
    OR OLD.conversation_id IS NOT NEW.conversation_id
    OR OLD.conversation_turn_id IS NOT NEW.conversation_turn_id
    OR OLD.task_id IS NOT NEW.task_id
    OR OLD.task_spec_revision IS NOT NEW.task_spec_revision
    OR OLD.attempt_id IS NOT NEW.attempt_id
    OR OLD.endpoint_id <> NEW.endpoint_id
    OR OLD.runtime_id <> NEW.runtime_id
    OR OLD.runtime_incarnation_id <> NEW.runtime_incarnation_id
    OR OLD.agent_binding_id <> NEW.agent_binding_id
    OR OLD.configuration_digest IS NOT NEW.configuration_digest
  )
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_AGENT_SESSION_IDENTITY');
END;

CREATE TRIGGER plan_revision_agent_session_context_guard
BEFORE INSERT ON plan_revisions
WHEN NOT EXISTS (
  SELECT 1 FROM agent_sessions s
  WHERE s.agent_session_id = NEW.produced_by_agent_session_id
    AND s.task_id = NEW.task_id
    AND s.task_spec_revision = NEW.task_spec_revision
    AND (
      (s.scope_kind = 'TASK_PLANNING' AND NEW.produced_by_attempt_id IS NULL)
      OR
      (s.scope_kind = 'ATTEMPT_EXECUTION' AND s.attempt_id = NEW.produced_by_attempt_id)
    )
)
BEGIN
  SELECT RAISE(ABORT, 'PLAN_REVISION_SESSION_CONTEXT_MISMATCH');
END;

CREATE TABLE execution_leases (
  lease_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  step_id TEXT NOT NULL REFERENCES steps(step_id),
  attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  runtime_incarnation_id TEXT NOT NULL,
  epoch INTEGER NOT NULL,
  issuer_key_version INTEGER NOT NULL CHECK (issuer_key_version > 0),
  fencing_token_digest TEXT NOT NULL UNIQUE CHECK (length(fencing_token_digest) = 71 AND substr(fencing_token_digest, 1, 7) = 'sha256:' AND substr(fencing_token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  state TEXT NOT NULL CHECK (state IN ('ACTIVE', 'RELEASING', 'RELEASED', 'EXPIRED', 'REVOKED')),
  checkpoint_ref_json TEXT,
  acquired_at TEXT NOT NULL,
  renew_by TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(step_id, epoch),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id) REFERENCES attempts(task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_leases_step_state ON execution_leases(step_id, state);
CREATE UNIQUE INDEX uq_active_lease_per_step ON execution_leases(step_id) WHERE state = 'ACTIVE';

CREATE TRIGGER execution_lease_current_incarnation_guard
BEFORE INSERT ON execution_leases
WHEN NEW.state = 'ACTIVE' AND NOT EXISTS (
  SELECT 1 FROM runtimes r
  WHERE r.runtime_id = NEW.runtime_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'EXECUTION_LEASE_STALE_RUNTIME_INCARNATION');
END;

CREATE TRIGGER execution_lease_current_incarnation_update_guard
BEFORE UPDATE OF state, expires_at ON execution_leases
WHEN NEW.state = 'ACTIVE' AND NOT EXISTS (
  SELECT 1 FROM runtimes r
  WHERE r.runtime_id = NEW.runtime_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'EXECUTION_LEASE_STALE_RUNTIME_INCARNATION');
END;

CREATE TRIGGER execution_lease_identity_immutable
BEFORE UPDATE OF task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch, issuer_key_version, fencing_token_digest ON execution_leases
WHEN OLD.task_id <> NEW.task_id
  OR OLD.step_id <> NEW.step_id
  OR OLD.attempt_id <> NEW.attempt_id
  OR OLD.runtime_id <> NEW.runtime_id
  OR OLD.runtime_incarnation_id <> NEW.runtime_incarnation_id
  OR OLD.epoch <> NEW.epoch
  OR OLD.issuer_key_version <> NEW.issuer_key_version
  OR OLD.fencing_token_digest <> NEW.fencing_token_digest
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_EXECUTION_LEASE_IDENTITY');
END;

CREATE TRIGGER execution_lease_no_reactivation
BEFORE UPDATE OF state ON execution_leases
WHEN OLD.state <> 'ACTIVE' AND NEW.state = 'ACTIVE'
BEGIN
  SELECT RAISE(ABORT, 'EXECUTION_LEASE_CANNOT_REACTIVATE');
END;

CREATE TABLE capability_grants (
  capability_grant_id TEXT PRIMARY KEY,
  scope_kind TEXT NOT NULL CHECK (scope_kind IN ('CONVERSATION', 'TASK_PLANNING', 'ATTEMPT_EXECUTION')),
  conversation_id TEXT REFERENCES conversations(conversation_id),
  task_id TEXT REFERENCES tasks(task_id),
  attempt_id TEXT REFERENCES attempts(attempt_id),
  capability_ref_json TEXT NOT NULL,
  allowed_operations_json TEXT NOT NULL CHECK (json_valid(allowed_operations_json)),
  resource_scope_json TEXT NOT NULL CHECK (json_valid(resource_scope_json)),
  secret_refs_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(secret_refs_json)),
  granted_by_json TEXT NOT NULL,
  expires_at TEXT,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'REVOKED', 'EXPIRED')),
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  CHECK ((scope_kind = 'CONVERSATION' AND conversation_id IS NOT NULL AND task_id IS NULL AND attempt_id IS NULL) OR (scope_kind = 'TASK_PLANNING' AND conversation_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NULL) OR (scope_kind = 'ATTEMPT_EXECUTION' AND conversation_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NOT NULL)),
  CHECK (scope_kind = 'ATTEMPT_EXECUTION' OR secret_refs_json = '[]')
);

CREATE TABLE secret_leases (
  secret_lease_id TEXT PRIMARY KEY,
  secret_ref_json TEXT NOT NULL,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  attempt_id TEXT REFERENCES attempts(attempt_id),
  capability_ref_json TEXT,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  allowed_usage_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'REVOKED', 'EXPIRED')),
  issued_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_secret_leases_task_status ON secret_leases(task_id, status);

CREATE TABLE capability_host_instances (
  host_instance_id TEXT PRIMARY KEY,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  capability_ref_json TEXT NOT NULL,
  capability_identity_digest TEXT NOT NULL CHECK (length(capability_identity_digest) = 71 AND substr(capability_identity_digest, 1, 7) = 'sha256:' AND substr(capability_identity_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  configuration_digest TEXT NOT NULL CHECK (length(configuration_digest) = 71 AND substr(configuration_digest, 1, 7) = 'sha256:' AND substr(configuration_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  isolation_partition_digest TEXT NOT NULL CHECK (length(isolation_partition_digest) = 71 AND substr(isolation_partition_digest, 1, 7) = 'sha256:' AND substr(isolation_partition_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  sharing_policy TEXT NOT NULL CHECK (sharing_policy IN ('EXCLUSIVE', 'TASK_ISOLATED', 'TRUST_PARTITION_SHARED')),
  hosting_mode TEXT NOT NULL CHECK (hosting_mode IN ('LOCAL_MANAGED', 'REMOTE_PROVIDER')),
  provider_instance_ref TEXT,
  state TEXT NOT NULL CHECK (state IN ('STARTING', 'READY', 'BUSY', 'DEGRADED', 'STOPPING', 'STOPPED', 'FAILED')),
  health TEXT NOT NULL CHECK (health IN ('HEALTHY', 'DEGRADED', 'UNHEALTHY', 'UNKNOWN')),
  observed_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(runtime_id, runtime_incarnation_id, capability_identity_digest, configuration_digest, isolation_partition_digest),
  UNIQUE(runtime_id, runtime_incarnation_id, provider_instance_ref),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_capability_host_instances_runtime_state ON capability_host_instances(runtime_id, state, expires_at);
CREATE TRIGGER capability_host_instance_identity_immutable
BEFORE UPDATE ON capability_host_instances
WHEN OLD.runtime_id <> NEW.runtime_id
  OR OLD.runtime_incarnation_id <> NEW.runtime_incarnation_id
  OR OLD.capability_ref_json <> NEW.capability_ref_json
  OR OLD.capability_identity_digest <> NEW.capability_identity_digest
  OR OLD.configuration_digest <> NEW.configuration_digest
  OR OLD.isolation_partition_digest <> NEW.isolation_partition_digest
  OR OLD.sharing_policy <> NEW.sharing_policy
  OR OLD.hosting_mode <> NEW.hosting_mode
  OR OLD.provider_instance_ref IS NOT NEW.provider_instance_ref
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_CAPABILITY_HOST_IDENTITY');
END;

CREATE TABLE capability_activations (
  activation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  scope_kind TEXT NOT NULL CHECK (scope_kind IN ('CONVERSATION', 'TASK_PLANNING', 'ATTEMPT_EXECUTION')),
  conversation_id TEXT,
  task_id TEXT,
  attempt_id TEXT,
  capability_ref_json TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('STARTING', 'ACTIVE', 'FAILED', 'STOPPING', 'STOPPED')),
  health TEXT NOT NULL CHECK (health IN ('HEALTHY', 'DEGRADED', 'UNHEALTHY', 'UNKNOWN')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(workspace_id, conversation_id) REFERENCES conversations(workspace_id, conversation_id),
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id),
  FOREIGN KEY(task_id, attempt_id) REFERENCES attempts(task_id, attempt_id),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id),
  CHECK ((scope_kind = 'CONVERSATION' AND conversation_id IS NOT NULL AND task_id IS NULL AND attempt_id IS NULL) OR
         (scope_kind = 'TASK_PLANNING' AND conversation_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NULL) OR
         (scope_kind = 'ATTEMPT_EXECUTION' AND conversation_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NOT NULL))
);

CREATE TABLE capability_activation_host_bindings (
  activation_id TEXT PRIMARY KEY REFERENCES capability_activations(activation_id),
  host_instance_id TEXT NOT NULL REFERENCES capability_host_instances(host_instance_id),
  provider_handle_ref TEXT,
  bound_at TEXT NOT NULL
);
CREATE INDEX idx_capability_activation_host_bindings_host ON capability_activation_host_bindings(host_instance_id, activation_id);

CREATE TRIGGER capability_activation_host_binding_matches_origin
BEFORE INSERT ON capability_activation_host_bindings
WHEN NOT EXISTS (
  SELECT 1
  FROM capability_activations a
  JOIN capability_host_instances h ON h.host_instance_id = NEW.host_instance_id
  WHERE a.activation_id = NEW.activation_id
    AND a.mode <> 'NATIVE_AGENT'
    AND a.runtime_id = h.runtime_id
    AND a.runtime_incarnation_id = h.runtime_incarnation_id
    AND a.capability_ref_json = h.capability_ref_json
    AND a.status IN ('STARTING', 'ACTIVE')
)
BEGIN
  SELECT RAISE(ABORT, 'CAPABILITY_ACTIVATION_HOST_MISMATCH');
END;
CREATE TRIGGER capability_activation_host_binding_immutable
BEFORE UPDATE ON capability_activation_host_bindings
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_CAPABILITY_ACTIVATION_HOST_BINDING');
END;

CREATE TRIGGER capability_activation_host_binding_delete_requires_settlement
BEFORE DELETE ON capability_activation_host_bindings
WHEN (SELECT status FROM capability_activations WHERE activation_id = OLD.activation_id)
  NOT IN ('FAILED', 'STOPPED')
BEGIN
  SELECT RAISE(ABORT, 'CAPABILITY_ACTIVATION_HOST_STILL_ACTIVE');
END;

CREATE TRIGGER capability_activation_identity_immutable
BEFORE UPDATE OF workspace_id, scope_kind, conversation_id, task_id, attempt_id, runtime_id, runtime_incarnation_id, capability_ref_json, mode ON capability_activations
WHEN (
    OLD.workspace_id <> NEW.workspace_id
    OR OLD.scope_kind <> NEW.scope_kind
    OR OLD.conversation_id IS NOT NEW.conversation_id
    OR OLD.task_id IS NOT NEW.task_id
    OR OLD.attempt_id IS NOT NEW.attempt_id
    OR OLD.runtime_id <> NEW.runtime_id
    OR OLD.runtime_incarnation_id IS NOT NEW.runtime_incarnation_id
    OR OLD.capability_ref_json <> NEW.capability_ref_json
    OR OLD.mode <> NEW.mode
  )
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_CAPABILITY_ACTIVATION_IDENTITY');
END;

CREATE TABLE capability_locks (
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  identity_kind TEXT NOT NULL CHECK (identity_kind IN ('PACKAGE_COMPONENT', 'MCP_SKILL')),
  source TEXT NOT NULL,
  capability_id TEXT NOT NULL,
  package_version TEXT,
  digest TEXT NOT NULL CHECK (digest GLOB 'sha256:*' AND length(digest) = 71 AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'),
  component TEXT NOT NULL DEFAULT '',
  capability_ref_key_digest TEXT NOT NULL CHECK (capability_ref_key_digest GLOB 'sha256:*' AND length(capability_ref_key_digest) = 71 AND substr(capability_ref_key_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  locked_at TEXT NOT NULL,
  PRIMARY KEY(task_id, identity_kind, source, capability_id, component),
  UNIQUE(task_id, capability_ref_key_digest),
  CHECK (
    (identity_kind = 'PACKAGE_COMPONENT' AND package_version IS NOT NULL AND package_version <> '') OR
    (identity_kind = 'MCP_SKILL' AND package_version IS NULL AND component <> '')
  )
);

CREATE TABLE connections (
  connection_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  external_provider_ref TEXT NOT NULL,
  account_ref TEXT,
  secret_refs_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL CHECK (status IN ('CONNECTING', 'CONNECTED', 'DEGRADED', 'REAUTH_REQUIRED', 'DISCONNECTED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE channel_bindings (
  channel_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  connection_id TEXT REFERENCES connections(connection_id),
  provider_ref TEXT NOT NULL,
  external_account_ref TEXT NOT NULL,
  identity_ref_json TEXT NOT NULL,
  assurance_level TEXT NOT NULL CHECK (assurance_level IN ('VIEW_ONLY', 'STEER_SAFE', 'APPROVE_SAFE', 'APPROVE_SENSITIVE', 'LOCAL_STRONG')),
  allowed_actions_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'DEGRADED', 'REVOKED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(channel_binding_id, workspace_id)
);

-- Current durable channel owner. The append-only event journal preserves each
-- reassignment; lease renewal is control-plane state and does not revise this aggregate.
CREATE TABLE channel_host_assignments (
  channel_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  host_epoch INTEGER NOT NULL CHECK (host_epoch > 0),
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'DRAINING')),
  ingress_continuity TEXT NOT NULL DEFAULT 'CONTINUOUS' CHECK (ingress_continuity IN ('CONTINUOUS', 'GAP_ACCEPTED')),
  ingress_gap_since TEXT,
  assigned_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  CHECK ((ingress_continuity = 'GAP_ACCEPTED') = (ingress_gap_since IS NOT NULL)),
  UNIQUE(channel_binding_id, host_epoch),
  UNIQUE(channel_binding_id, workspace_id, runtime_id, host_epoch),
  FOREIGN KEY(channel_binding_id, workspace_id) REFERENCES channel_bindings(channel_binding_id, workspace_id),
  FOREIGN KEY(runtime_id, workspace_id) REFERENCES runtimes(runtime_id, workspace_id)
);
CREATE INDEX idx_channel_host_runtime ON channel_host_assignments(runtime_id, status);

-- Hub/control-plane lease state. The assigned Runtime receives the raw credential over
-- authenticated control transport; only the digest is stored here. Renewals do not emit
-- DomainEvents or modify the ChannelHostAssignment aggregate.
CREATE TABLE channel_host_lease_records (
  channel_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  host_epoch INTEGER NOT NULL CHECK (host_epoch > 0),
  lease_id TEXT NOT NULL,
  fencing_token_digest TEXT NOT NULL CHECK (length(fencing_token_digest) = 71 AND substr(fencing_token_digest, 1, 7) = 'sha256:' AND substr(fencing_token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  lease_expires_at TEXT NOT NULL,
  control_version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(channel_binding_id, workspace_id, runtime_id, host_epoch)
    REFERENCES channel_host_assignments(channel_binding_id, workspace_id, runtime_id, host_epoch)
    DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_channel_host_lease_expiry ON channel_host_lease_records(lease_expires_at);

CREATE TRIGGER channel_host_assignment_epoch_monotonic
BEFORE UPDATE ON channel_host_assignments
WHEN NEW.channel_binding_id IS NOT OLD.channel_binding_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.host_epoch < OLD.host_epoch
  OR (NEW.runtime_id IS NOT OLD.runtime_id AND NEW.host_epoch <= OLD.host_epoch)
  OR NEW.version <> OLD.version + 1
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_ASSIGNMENT_EPOCH_STALE');
END;

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
  origin_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  origin_host_epoch INTEGER NOT NULL CHECK (origin_host_epoch > 0),
  claim_runtime_id TEXT REFERENCES runtimes(runtime_id),
  claim_host_epoch INTEGER CHECK (claim_host_epoch IS NULL OR claim_host_epoch > 0),
  event_kind TEXT NOT NULL CHECK (event_kind IN ('INBOUND', 'EDIT', 'DELETE')),
  ingress_sequence INTEGER NOT NULL CHECK (ingress_sequence > 0),
  payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71 AND substr(payload_digest, 1, 7) = 'sha256:' AND substr(payload_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  conversation_id TEXT REFERENCES conversations(conversation_id),
  message_id TEXT REFERENCES conversation_messages(message_id),
  received_at TEXT NOT NULL,
  claim_epoch INTEGER NOT NULL DEFAULT 0,
  claim_expires_at TEXT,
  state TEXT NOT NULL CHECK (state IN ('RECEIVED', 'PROCESSING', 'ACCEPTED', 'REJECTED', 'FAILED')),
  CHECK ((claim_runtime_id IS NULL) = (claim_host_epoch IS NULL)),
  CHECK ((state = 'PROCESSING') = (claim_expires_at IS NOT NULL)),
  CHECK ((state = 'RECEIVED' AND claim_epoch = 0 AND claim_runtime_id IS NULL AND claim_host_epoch IS NULL)
    OR (state IN ('PROCESSING', 'ACCEPTED', 'REJECTED', 'FAILED') AND claim_epoch >= 1 AND claim_runtime_id IS NOT NULL AND claim_host_epoch IS NOT NULL)),
  PRIMARY KEY(channel_binding_id, provider_event_id),
  UNIQUE(channel_binding_id, origin_host_epoch, ingress_sequence)
);

CREATE TRIGGER channel_receipt_origin_immutable
BEFORE UPDATE ON channel_event_receipts
WHEN NEW.channel_binding_id IS NOT OLD.channel_binding_id
  OR NEW.provider_event_id IS NOT OLD.provider_event_id
  OR NEW.origin_runtime_id IS NOT OLD.origin_runtime_id
  OR NEW.origin_host_epoch IS NOT OLD.origin_host_epoch
  OR NEW.ingress_sequence IS NOT OLD.ingress_sequence
  OR NEW.event_kind IS NOT OLD.event_kind
  OR NEW.payload_digest IS NOT OLD.payload_digest
  OR NEW.received_at IS NOT OLD.received_at
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_RECEIPT_ORIGIN_IMMUTABLE');
END;

CREATE TRIGGER channel_receipt_transition_guard
BEFORE UPDATE ON channel_event_receipts
WHEN NOT (
  (OLD.state = 'RECEIVED' AND NEW.state = 'PROCESSING'
    AND NEW.claim_epoch = 1
    AND julianday(NEW.claim_expires_at) > julianday('now')
    AND EXISTS (
      SELECT 1 FROM channel_host_assignments a
      JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
        AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
      WHERE a.channel_binding_id = NEW.channel_binding_id AND a.runtime_id = NEW.claim_runtime_id
        AND a.host_epoch = NEW.claim_host_epoch AND a.status = 'ACTIVE'
        AND julianday(l.lease_expires_at) > julianday('now')
    ))
  OR
  (OLD.state = 'PROCESSING' AND NEW.state = 'PROCESSING'
    AND NEW.claim_epoch = OLD.claim_epoch + 1
    AND julianday(NEW.claim_expires_at) > julianday('now')
    AND (julianday(OLD.claim_expires_at) <= julianday('now') OR NOT EXISTS (
      SELECT 1 FROM channel_host_assignments a
      JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
        AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
      WHERE a.channel_binding_id = OLD.channel_binding_id AND a.runtime_id = OLD.claim_runtime_id
        AND a.host_epoch = OLD.claim_host_epoch AND a.status = 'ACTIVE'
        AND julianday(l.lease_expires_at) > julianday('now')
    ))
    AND EXISTS (
      SELECT 1 FROM channel_host_assignments a
      JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
        AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
      WHERE a.channel_binding_id = NEW.channel_binding_id AND a.runtime_id = NEW.claim_runtime_id
        AND a.host_epoch = NEW.claim_host_epoch AND a.status = 'ACTIVE'
        AND julianday(l.lease_expires_at) > julianday('now')
    ))
  OR
  (OLD.state = 'PROCESSING' AND NEW.state IN ('ACCEPTED', 'REJECTED', 'FAILED')
    AND NEW.claim_epoch = OLD.claim_epoch
    AND julianday(OLD.claim_expires_at) > julianday('now')
    AND NEW.claim_runtime_id = OLD.claim_runtime_id
    AND NEW.claim_host_epoch = OLD.claim_host_epoch
    AND NEW.claim_expires_at IS NULL
    AND EXISTS (
      SELECT 1 FROM channel_host_assignments a
      JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
        AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
      WHERE a.channel_binding_id = NEW.channel_binding_id AND a.runtime_id = NEW.claim_runtime_id
        AND a.host_epoch = NEW.claim_host_epoch AND a.status = 'ACTIVE'
        AND julianday(l.lease_expires_at) > julianday('now')
    ))
)
BEGIN
  SELECT RAISE(ABORT, 'INVALID_CHANNEL_RECEIPT_TRANSITION');
END;

-- Opaque cursors are per-Runtime encrypted operational state and never enter Mesh state,
-- API projections, logs, or Workspace backups. The host epoch scopes each continuation.
CREATE TABLE channel_ingress_cursor_bindings (
  channel_binding_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  runtime_incarnation_id TEXT NOT NULL,
  host_epoch INTEGER NOT NULL CHECK (host_epoch >= 1),
  cursor_ciphertext BLOB NOT NULL CHECK (length(cursor_ciphertext) > 0),
  encryption_key_version INTEGER NOT NULL CHECK (encryption_key_version >= 1),
  cursor_digest TEXT NOT NULL CHECK (length(cursor_digest) = 71 AND substr(cursor_digest, 1, 7) = 'sha256:' AND substr(cursor_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  last_committed_origin_host_epoch INTEGER CHECK (last_committed_origin_host_epoch IS NULL OR last_committed_origin_host_epoch >= 1),
  last_committed_ingress_sequence INTEGER CHECK (last_committed_ingress_sequence IS NULL OR last_committed_ingress_sequence >= 1),
  state TEXT NOT NULL CHECK (state IN ('AVAILABLE', 'RECONCILIATION_REQUIRED', 'UNAVAILABLE')),
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
  PRIMARY KEY(channel_binding_id, host_epoch),
  CHECK ((last_committed_origin_host_epoch IS NULL) = (last_committed_ingress_sequence IS NULL)),
  FOREIGN KEY(channel_binding_id, workspace_id) REFERENCES channel_bindings(channel_binding_id, workspace_id),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id),
  FOREIGN KEY(channel_binding_id, last_committed_origin_host_epoch, last_committed_ingress_sequence)
    REFERENCES channel_event_receipts(channel_binding_id, origin_host_epoch, ingress_sequence)
);
CREATE INDEX idx_channel_ingress_cursor_runtime ON channel_ingress_cursor_bindings(runtime_id, state, updated_at);

CREATE TRIGGER channel_ingress_cursor_binding_insert_owner
BEFORE INSERT ON channel_ingress_cursor_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM channel_host_assignments a
  JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
    AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
  JOIN runtimes r ON r.runtime_id = a.runtime_id
  WHERE a.channel_binding_id = NEW.channel_binding_id AND a.workspace_id = NEW.workspace_id
    AND a.runtime_id = NEW.runtime_id AND a.host_epoch = NEW.host_epoch AND a.status = 'ACTIVE'
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
    AND julianday(l.lease_expires_at) > julianday('now')
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_INGRESS_CURSOR_OWNER_MISMATCH');
END;

CREATE TRIGGER channel_ingress_cursor_binding_update_guard
BEFORE UPDATE ON channel_ingress_cursor_bindings
WHEN NEW.channel_binding_id IS NOT OLD.channel_binding_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.host_epoch IS NOT OLD.host_epoch
  OR NEW.version <> OLD.version + 1
  OR (NEW.state = 'AVAILABLE' AND NOT EXISTS (
    SELECT 1 FROM channel_host_assignments a
    JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
      AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
    JOIN runtimes r ON r.runtime_id = a.runtime_id
    WHERE a.channel_binding_id = NEW.channel_binding_id AND a.workspace_id = NEW.workspace_id
      AND a.runtime_id = NEW.runtime_id AND a.host_epoch = NEW.host_epoch AND a.status = 'ACTIVE'
      AND r.current_incarnation_id = NEW.runtime_incarnation_id
      AND julianday(l.lease_expires_at) > julianday('now')
  ))
BEGIN
  SELECT RAISE(ABORT, 'INVALID_CHANNEL_INGRESS_CURSOR_BINDING_UPDATE');
END;

CREATE TRIGGER channel_ingress_cursor_binding_identity_immutable
BEFORE UPDATE ON channel_ingress_cursor_bindings
WHEN NEW.channel_binding_id IS NOT OLD.channel_binding_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.host_epoch IS NOT OLD.host_epoch
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_INGRESS_CURSOR_BINDING_IDENTITY_IMMUTABLE');
END;

CREATE TABLE artifacts (
  artifact_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  resource_id TEXT NOT NULL REFERENCES resources(resource_id),
  task_id TEXT REFERENCES tasks(task_id),
  kind TEXT NOT NULL,
  display_name TEXT NOT NULL,
  current_version INTEGER NOT NULL,
  library_status TEXT NOT NULL CHECK (library_status IN ('TRANSIENT', 'SAVED', 'ARCHIVED')),
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
  CHECK (current_version >= 1),
  UNIQUE(artifact_id, resource_id),
  FOREIGN KEY(artifact_id, current_version) REFERENCES artifact_versions(artifact_id, version) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(artifact_id, resource_id, current_version) REFERENCES artifact_versions(artifact_id, resource_id, version) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_artifacts_task ON artifacts(task_id, current_version);

CREATE TRIGGER guard_artifact_update
BEFORE UPDATE ON artifacts
WHEN NEW.version <> OLD.version + 1
  OR (OLD.library_status <> NEW.library_status AND NOT (
    (OLD.library_status = 'TRANSIENT' AND NEW.library_status = 'SAVED') OR
    (OLD.library_status = 'SAVED' AND NEW.library_status = 'ARCHIVED')
  ))
  OR (OLD.library_status = 'ARCHIVED' AND NEW.current_version <> OLD.current_version)
BEGIN
  SELECT RAISE(ABORT, 'INVALID_ARTIFACT_TRANSITION');
END;

CREATE TABLE artifact_versions (
  artifact_id TEXT NOT NULL REFERENCES artifacts(artifact_id),
  resource_id TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  resource_revision_id TEXT NOT NULL,
  created_by_attempt TEXT REFERENCES attempts(attempt_id),
  input_refs_json TEXT NOT NULL DEFAULT '[]',
  content_kind TEXT NOT NULL CHECK (content_kind IN ('MANAGED_BLOB', 'EXTERNAL_RESOURCE')),
  content_digest TEXT CHECK (content_digest IS NULL OR (length(content_digest) = 71 AND substr(content_digest, 1, 7) = 'sha256:' AND substr(content_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  storage_ref_json TEXT,
  resource_ref_json TEXT,
  provider_revision TEXT,
  observed_digest TEXT CHECK (observed_digest IS NULL OR (length(observed_digest) = 71 AND substr(observed_digest, 1, 7) = 'sha256:' AND substr(observed_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  content_media_type TEXT,
  content_size_bytes INTEGER CHECK (content_size_bytes IS NULL OR content_size_bytes >= 0),
  content_observed_at TEXT,
  provenance_json TEXT NOT NULL,
  verification_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  PRIMARY KEY(artifact_id, version),
  UNIQUE(artifact_id, resource_id, version),
  FOREIGN KEY(artifact_id, resource_id) REFERENCES artifacts(artifact_id, resource_id),
  FOREIGN KEY(resource_id, resource_revision_id) REFERENCES resource_revisions(resource_id, resource_revision_id),
  CHECK (
    (content_kind = 'MANAGED_BLOB' AND content_digest IS NOT NULL AND storage_ref_json IS NOT NULL AND content_media_type IS NOT NULL AND content_size_bytes IS NOT NULL AND resource_ref_json IS NULL) OR
    (content_kind = 'EXTERNAL_RESOURCE' AND resource_ref_json IS NOT NULL AND content_observed_at IS NOT NULL AND storage_ref_json IS NULL)
  )
);
CREATE TRIGGER guard_artifact_current_revision
BEFORE UPDATE OF current_version ON artifacts
WHEN NOT EXISTS (
  SELECT 1
  FROM artifact_versions av
  JOIN resources r ON r.resource_id = av.resource_id
  WHERE av.artifact_id = NEW.artifact_id
    AND av.resource_id = NEW.resource_id
    AND av.version = NEW.current_version
    AND r.current_revision_id = av.resource_revision_id
)
BEGIN
  SELECT RAISE(ABORT, 'ARTIFACT_RESOURCE_REVISION_MISMATCH');
END;

CREATE TRIGGER guard_artifact_initial_revision
AFTER INSERT ON artifact_versions
WHEN NEW.version = (SELECT current_version FROM artifacts WHERE artifact_id = NEW.artifact_id)
  AND NOT EXISTS (
    SELECT 1
    FROM resources r
    WHERE r.resource_id = NEW.resource_id
      AND r.current_revision_id = NEW.resource_revision_id
  )
BEGIN
  SELECT RAISE(ABORT, 'ARTIFACT_RESOURCE_REVISION_MISMATCH');
END;

CREATE TRIGGER guard_artifact_version_insert
BEFORE INSERT ON artifact_versions
WHEN (SELECT library_status FROM artifacts WHERE artifact_id = NEW.artifact_id) = 'ARCHIVED'
  OR (
    EXISTS (SELECT 1 FROM artifact_versions WHERE artifact_id = NEW.artifact_id)
    AND NEW.version <> (SELECT current_version + 1 FROM artifacts WHERE artifact_id = NEW.artifact_id)
  )
  OR (
    NOT EXISTS (SELECT 1 FROM artifact_versions WHERE artifact_id = NEW.artifact_id)
    AND NEW.version <> (SELECT current_version FROM artifacts WHERE artifact_id = NEW.artifact_id)
  )
BEGIN
  SELECT RAISE(ABORT, 'ARTIFACT_VERSION_CONFLICT');
END;

CREATE TRIGGER artifact_versions_no_update
BEFORE UPDATE ON artifact_versions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_ARTIFACT_VERSION');
END;

CREATE TRIGGER artifact_versions_no_delete
BEFORE DELETE ON artifact_versions
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_ARTIFACT_VERSION');
END;

CREATE INDEX idx_artifact_digest ON artifact_versions(content_digest) WHERE content_digest IS NOT NULL;

CREATE TABLE effects (
  effect_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  capability_ref_json TEXT,
  operation TEXT NOT NULL,
  target_json TEXT NOT NULL,
  idempotency_key TEXT,
  state TEXT NOT NULL CHECK (state IN ('PROPOSED', 'STARTED', 'ACKNOWLEDGED', 'RECONCILING', 'OBSERVED', 'VERIFIED', 'FAILED', 'AMBIGUOUS')),
  request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  result_ref_json TEXT,
  observed_state_json TEXT,
  verification_ref TEXT,
  dispatch_ordinal INTEGER NOT NULL DEFAULT 0,
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
  level TEXT NOT NULL CHECK (level IN ('REPORTED', 'OBSERVED', 'VERIFIED')),
  kind TEXT NOT NULL,
  producer_json TEXT NOT NULL,
  payload_ref_json TEXT,
  payload_digest TEXT CHECK (payload_digest IS NULL OR (length(payload_digest) = 71 AND substr(payload_digest, 1, 7) = 'sha256:' AND substr(payload_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  created_at TEXT NOT NULL
);

CREATE TABLE audit_records (
  audit_record_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  principal_json TEXT NOT NULL,
  action TEXT NOT NULL,
  resource_ref_json TEXT,
  decision TEXT NOT NULL CHECK (decision IN ('ALLOW', 'DENY', 'REQUIRE_APPROVAL')),
  reason_code TEXT NOT NULL,
  correlation_id TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  payload_digest TEXT CHECK (payload_digest IS NULL OR (length(payload_digest) = 71 AND substr(payload_digest, 1, 7) = 'sha256:' AND substr(payload_digest, 8) NOT GLOB '*[^0-9a-f]*'))
);
CREATE INDEX idx_audit_workspace_time ON audit_records(workspace_id, occurred_at);

CREATE TABLE verification_runs (
  verification_run_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  criterion_id TEXT NOT NULL,
  task_spec_revision INTEGER NOT NULL,
  criterion_digest TEXT NOT NULL CHECK (length(criterion_digest) = 71 AND substr(criterion_digest, 1, 7) = 'sha256:' AND substr(criterion_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  verifier_kind TEXT NOT NULL,
  verifier_version TEXT NOT NULL,
  subject_refs_json TEXT NOT NULL,
  inputs_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'RUNNING', 'PASSED', 'FAILED', 'INCONCLUSIVE')),
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  started_at TEXT,
  completed_at TEXT,
  FOREIGN KEY(task_id, task_spec_revision) REFERENCES task_spec_revisions(task_id, revision)
);

CREATE TABLE approvals (
  approval_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  requested_by_attempt TEXT REFERENCES attempts(attempt_id),
  kind TEXT NOT NULL,
  action_summary TEXT NOT NULL,
  target_ref_json TEXT NOT NULL,
  scope_digest TEXT NOT NULL CHECK (length(scope_digest) = 71 AND substr(scope_digest, 1, 7) = 'sha256:' AND substr(scope_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  action_digest TEXT NOT NULL CHECK (length(action_digest) = 71 AND substr(action_digest, 1, 7) = 'sha256:' AND substr(action_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  risk TEXT NOT NULL CHECK (risk IN ('SAFE', 'SENSITIVE', 'HIGH_IMPACT')),
  required_assurance TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'APPROVED', 'DENIED', 'EXPIRED', 'CANCELLED')),
  requested_at TEXT NOT NULL,
  expires_at TEXT,
  resolved_by_json TEXT,
  resolved_at TEXT,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_approvals_task_status ON approvals(task_id, status);

CREATE TABLE approval_uses (
  approval_use_id TEXT PRIMARY KEY,
  approval_id TEXT NOT NULL UNIQUE REFERENCES approvals(approval_id),
  effect_id TEXT REFERENCES effects(effect_id),
  capability_grant_id TEXT REFERENCES capability_grants(capability_grant_id),
  request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  consumed_at TEXT NOT NULL,
  CHECK ((effect_id IS NOT NULL AND capability_grant_id IS NULL) OR (effect_id IS NULL AND capability_grant_id IS NOT NULL))
);

CREATE TABLE routines (
  routine_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  name TEXT NOT NULL,
  current_revision INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ARCHIVED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(routine_id, workspace_id),
  FOREIGN KEY(routine_id, current_revision) REFERENCES routine_revisions(routine_id, revision) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_routines_workspace_status ON routines(workspace_id, status, updated_at DESC);

CREATE TABLE routine_revisions (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  routine_id TEXT NOT NULL REFERENCES routines(routine_id),
  revision INTEGER NOT NULL,
  objective_template TEXT NOT NULL,
  instructions TEXT NOT NULL,
  input_schema_json TEXT NOT NULL,
  constraints_json TEXT NOT NULL DEFAULT '[]',
  non_goals_json TEXT NOT NULL DEFAULT '[]',
  required_outputs_json TEXT NOT NULL DEFAULT '[]',
  acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
  approvals_required_json TEXT NOT NULL DEFAULT '[]',
  input_bindings_json TEXT NOT NULL DEFAULT '[]',
  required_capabilities_json TEXT NOT NULL DEFAULT '[]',
  preferred_agent_binding_id TEXT,
  placement_preference_json TEXT NOT NULL,
  budget_ceiling_json TEXT,
  verification_policy_json TEXT NOT NULL,
  authored_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(routine_id, revision),
  UNIQUE(routine_id, revision, workspace_id),
  FOREIGN KEY(routine_id, workspace_id) REFERENCES routines(routine_id, workspace_id),
  FOREIGN KEY(workspace_id, preferred_agent_binding_id) REFERENCES agent_bindings(workspace_id, agent_binding_id)
);

CREATE TABLE automations (
  automation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  name TEXT NOT NULL,
  current_revision INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ENABLED', 'PAUSED', 'DISABLED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(automation_id, workspace_id),
  FOREIGN KEY(automation_id, current_revision) REFERENCES automation_revisions(automation_id, revision) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE automation_revisions (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  automation_id TEXT NOT NULL REFERENCES automations(automation_id),
  revision INTEGER NOT NULL,
  routine_id TEXT NOT NULL,
  routine_revision INTEGER NOT NULL,
  triggers_json TEXT NOT NULL CHECK (json_valid(triggers_json)),
  execution_policy_json TEXT NOT NULL CHECK (json_valid(execution_policy_json)),
  authored_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(automation_id, revision),
  UNIQUE(automation_id, revision, workspace_id),
  FOREIGN KEY(automation_id, workspace_id) REFERENCES automations(automation_id, workspace_id),
  FOREIGN KEY(routine_id, routine_revision, workspace_id) REFERENCES routine_revisions(routine_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE automation_occurrences (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  occurrence_id TEXT PRIMARY KEY,
  automation_id TEXT NOT NULL REFERENCES automations(automation_id),
  automation_revision INTEGER NOT NULL,
  routine_id TEXT NOT NULL,
  routine_revision INTEGER NOT NULL,
  trigger_id TEXT NOT NULL,
  trigger_host_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  occurrence_key TEXT NOT NULL,
  scheduled_for TEXT,
  covered_misfire_range_json TEXT,
  trigger_input_ref_json TEXT,
  trigger_payload_digest TEXT CHECK (trigger_payload_digest IS NULL OR (length(trigger_payload_digest) = 71 AND substr(trigger_payload_digest, 1, 7) = 'sha256:' AND substr(trigger_payload_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  blockers_json TEXT NOT NULL DEFAULT '[]',
  claim_epoch INTEGER NOT NULL DEFAULT 0,
  claim_expires_at TEXT,
  task_id TEXT REFERENCES tasks(task_id),
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'CLAIMED', 'WAITING_DEPENDENCY', 'STARTED', 'COMPLETED', 'SKIPPED', 'FAILED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(automation_id, occurrence_id),
  UNIQUE(automation_id, occurrence_id, workspace_id),
  FOREIGN KEY(trigger_host_runtime_id, workspace_id) REFERENCES runtimes(runtime_id, workspace_id),
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id),
  UNIQUE(automation_id, trigger_id, occurrence_key),
  FOREIGN KEY(automation_id, automation_revision, workspace_id) REFERENCES automation_revisions(automation_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(routine_id, routine_revision, workspace_id) REFERENCES routine_revisions(routine_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_automation_occurrences_status ON automation_occurrences(automation_id, status, created_at);
CREATE INDEX idx_automation_occurrences_claim ON automation_occurrences(status, claim_expires_at);

CREATE TABLE automation_cursors (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  automation_id TEXT NOT NULL,
  active_automation_revision INTEGER NOT NULL,
  trigger_id TEXT NOT NULL,
  trigger_host_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  host_epoch INTEGER NOT NULL CHECK (host_epoch >= 1),
  cursor_digest TEXT NOT NULL CHECK (length(cursor_digest) = 71 AND substr(cursor_digest, 1, 7) = 'sha256:' AND substr(cursor_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  last_seen_digest TEXT CHECK (last_seen_digest IS NULL OR (length(last_seen_digest) = 71 AND substr(last_seen_digest, 1, 7) = 'sha256:' AND substr(last_seen_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  last_observation_ref_json TEXT,
  next_scheduled_at TEXT,
  last_checked_at TEXT NOT NULL,
  observation_gap_since TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY(automation_id, trigger_id),
  FOREIGN KEY(trigger_host_runtime_id, workspace_id) REFERENCES runtimes(runtime_id, workspace_id),
  FOREIGN KEY(automation_id, active_automation_revision, workspace_id) REFERENCES automation_revisions(automation_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED
);

-- Opaque provider cursors may be bearer-like. This Runtime-local encrypted binding is
-- excluded from Mesh replication, Operator API projections, and Workspace backups.
CREATE TABLE automation_trigger_bindings (
  automation_id TEXT NOT NULL,
  trigger_id TEXT NOT NULL,
  trigger_host_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  host_epoch INTEGER NOT NULL CHECK (host_epoch >= 1),
  cursor_ciphertext BLOB NOT NULL,
  encryption_key_version INTEGER NOT NULL CHECK (encryption_key_version >= 1),
  cursor_digest TEXT NOT NULL CHECK (length(cursor_digest) = 71 AND substr(cursor_digest, 1, 7) = 'sha256:' AND substr(cursor_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  state TEXT NOT NULL CHECK (state IN ('AVAILABLE', 'RECONCILIATION_REQUIRED', 'UNAVAILABLE')),
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY(automation_id, trigger_id, host_epoch),
  FOREIGN KEY(automation_id, trigger_id) REFERENCES automation_cursors(automation_id, trigger_id)
);
CREATE INDEX idx_automation_trigger_bindings_runtime ON automation_trigger_bindings(trigger_host_runtime_id, state, updated_at);

CREATE TRIGGER automation_trigger_binding_matches_current_owner
BEFORE INSERT ON automation_trigger_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM automation_cursors c
  WHERE c.automation_id = NEW.automation_id
    AND c.trigger_id = NEW.trigger_id
    AND c.trigger_host_runtime_id = NEW.trigger_host_runtime_id
    AND c.host_epoch = NEW.host_epoch
    AND c.cursor_digest = NEW.cursor_digest
)
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_TRIGGER_BINDING_OWNER_MISMATCH');
END;

CREATE TRIGGER automation_trigger_binding_update_matches_current_owner
BEFORE UPDATE ON automation_trigger_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM automation_cursors c
  WHERE c.automation_id = NEW.automation_id
    AND c.trigger_id = NEW.trigger_id
    AND c.trigger_host_runtime_id = NEW.trigger_host_runtime_id
    AND c.host_epoch = NEW.host_epoch
    AND c.cursor_digest = NEW.cursor_digest
)
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_TRIGGER_BINDING_OWNER_MISMATCH');
END;

CREATE TRIGGER automation_trigger_binding_identity_immutable
BEFORE UPDATE ON automation_trigger_bindings
WHEN OLD.automation_id <> NEW.automation_id
  OR OLD.trigger_id <> NEW.trigger_id
  OR OLD.trigger_host_runtime_id <> NEW.trigger_host_runtime_id
  OR OLD.host_epoch <> NEW.host_epoch
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_TRIGGER_BINDING_IDENTITY_IMMUTABLE');
END;

CREATE INDEX idx_channel_receipts_claim ON channel_event_receipts(state, claim_expires_at);

CREATE TABLE domain_events (
  event_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  origin_runtime_id TEXT NOT NULL,
  origin_sequence INTEGER NOT NULL,
  entity_revision INTEGER NOT NULL,
  hlc_timestamp TEXT NOT NULL,
  correlation_id TEXT NOT NULL,
  causation_id TEXT,
  schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
  type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  aggregate_state_ref_json TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71 AND substr(payload_digest, 1, 7) = 'sha256:' AND substr(payload_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  UNIQUE(workspace_id, origin_runtime_id, origin_sequence),
  UNIQUE(event_id, workspace_id, origin_runtime_id, origin_sequence)
);
CREATE INDEX idx_events_workspace_hlc ON domain_events(workspace_id, hlc_timestamp);
CREATE INDEX idx_events_entity ON domain_events(entity_type, entity_id, entity_revision);

CREATE TABLE request_dedup (
  principal_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  response_json TEXT,
  response_digest TEXT CHECK (response_digest IS NULL OR (length(response_digest) = 71 AND substr(response_digest, 1, 7) = 'sha256:' AND substr(response_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  created_at TEXT NOT NULL,
  expires_at TEXT,
  PRIMARY KEY(principal_id, request_id)
);

CREATE TABLE replication_cursors (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  receiver_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  origin_runtime_id TEXT NOT NULL,
  highest_received_contiguous_sequence INTEGER NOT NULL DEFAULT 0,
  missing_sequences_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, receiver_runtime_id, origin_runtime_id)
);

CREATE TABLE replication_receipts (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  receiver_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  origin_runtime_id TEXT NOT NULL,
  origin_sequence INTEGER NOT NULL CHECK (origin_sequence > 0),
  disposition TEXT NOT NULL CHECK (disposition IN ('EVENT_STORED', 'POLICY_OMITTED')),
  event_id TEXT,
  envelope_digest TEXT CHECK (envelope_digest IS NULL OR (length(envelope_digest) = 71 AND substr(envelope_digest, 1, 7) = 'sha256:' AND substr(envelope_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),
  omission_commitment TEXT,
  received_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, receiver_runtime_id, origin_runtime_id, origin_sequence),
  CHECK (
    (disposition = 'EVENT_STORED' AND event_id IS NOT NULL AND envelope_digest IS NOT NULL AND omission_commitment IS NULL) OR
    (disposition = 'POLICY_OMITTED' AND event_id IS NULL AND envelope_digest IS NULL AND omission_commitment IS NOT NULL)
  ),
  FOREIGN KEY(event_id, workspace_id, origin_runtime_id, origin_sequence)
    REFERENCES domain_events(event_id, workspace_id, origin_runtime_id, origin_sequence)
);

CREATE TABLE replication_aggregate_positions (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  receiver_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  applied_revision INTEGER NOT NULL DEFAULT 0 CHECK (applied_revision >= 0),
  state_digest TEXT CHECK (state_digest IS NULL OR (length(state_digest) = 71 AND substr(state_digest, 1, 7) = 'sha256:' AND substr(state_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  status TEXT NOT NULL CHECK (status IN ('CURRENT', 'SNAPSHOT_REQUIRED', 'POLICY_WITHHELD')),
  updated_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, receiver_runtime_id, entity_type, entity_id)
);

CREATE TABLE pending_replication_events (
  workspace_id TEXT NOT NULL,
  receiver_runtime_id TEXT NOT NULL,
  origin_runtime_id TEXT NOT NULL,
  origin_sequence INTEGER NOT NULL,
  event_id TEXT NOT NULL,
  state_blob_available INTEGER NOT NULL DEFAULT 0 CHECK (state_blob_available IN (0, 1)),
  pending_reason TEXT NOT NULL CHECK (pending_reason IN ('STATE_BLOB_MISSING', 'REVISION_GAP', 'SNAPSHOT_REQUIRED')),
  created_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, receiver_runtime_id, origin_runtime_id, origin_sequence),
  FOREIGN KEY(workspace_id, receiver_runtime_id, origin_runtime_id, origin_sequence)
    REFERENCES replication_receipts(workspace_id, receiver_runtime_id, origin_runtime_id, origin_sequence),
  FOREIGN KEY(event_id, workspace_id, origin_runtime_id, origin_sequence)
    REFERENCES domain_events(event_id, workspace_id, origin_runtime_id, origin_sequence)
);

CREATE TABLE resources (
  resource_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  kind TEXT NOT NULL CHECK (kind IN ('FILE', 'FOLDER', 'ARTIFACT', 'CONNECTOR_OBJECT', 'WEB_RESOURCE', 'OTHER')),
  provider_identity_json TEXT NOT NULL,
  identity_digest TEXT CHECK (
    identity_digest IS NULL OR (
      length(identity_digest) = 71
      AND substr(identity_digest, 1, 7) = 'sha256:'
      AND substr(identity_digest, 8) NOT GLOB '*[^0-9a-f]*'
    )
  ),
  display_name TEXT NOT NULL,
  current_revision_id TEXT,
  sensitivity TEXT NOT NULL,
  context_document_json TEXT CHECK (context_document_json IS NULL OR json_valid(context_document_json)),
  provenance_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, resource_id),
  FOREIGN KEY(resource_id, current_revision_id)
    REFERENCES resource_revisions(resource_id, resource_revision_id)
    DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_resources_workspace_name ON resources(workspace_id, display_name);
CREATE UNIQUE INDEX uq_resources_identity_digest
  ON resources(workspace_id, identity_digest) WHERE identity_digest IS NOT NULL;

CREATE TABLE resource_revisions (
  resource_revision_id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES resources(resource_id),
  -- Parents are stored in resource_revision_parents so same-Resource ownership can be FK-enforced.
  provider_revision TEXT,
  content_digest TEXT CHECK (content_digest IS NULL OR (length(content_digest) = 71 AND substr(content_digest, 1, 7) = 'sha256:' AND substr(content_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
  media_type TEXT,
  observed_at TEXT NOT NULL,
  created_by_json TEXT NOT NULL,
  UNIQUE(resource_id, resource_revision_id)
);

CREATE TABLE resource_revision_parents (
  resource_id TEXT NOT NULL,
  child_revision_id TEXT NOT NULL,
  parent_revision_id TEXT NOT NULL,
  PRIMARY KEY(resource_id, child_revision_id, parent_revision_id),
  CHECK(child_revision_id <> parent_revision_id),
  FOREIGN KEY(resource_id, child_revision_id)
    REFERENCES resource_revisions(resource_id, resource_revision_id),
  FOREIGN KEY(resource_id, parent_revision_id)
    REFERENCES resource_revisions(resource_id, resource_revision_id)
);
CREATE INDEX idx_resource_revision_parents_parent
  ON resource_revision_parents(resource_id, parent_revision_id);

CREATE TABLE resource_locations (
  location_id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES resources(resource_id),
  runtime_id TEXT REFERENCES runtimes(runtime_id),
  environment_id TEXT REFERENCES environments(environment_id),
  connection_id TEXT REFERENCES connections(connection_id),
  provider_ref TEXT,
  locator_ref_id TEXT NOT NULL,
  availability TEXT NOT NULL CHECK (availability IN ('AVAILABLE', 'OFFLINE', 'PLACEHOLDER', 'REVOKED', 'UNKNOWN')),
  writable INTEGER NOT NULL CHECK (writable IN (0, 1)),
  observed_revision_id TEXT,
  observed_digest TEXT CHECK (observed_digest IS NULL OR (length(observed_digest) = 71 AND substr(observed_digest, 1, 7) = 'sha256:' AND substr(observed_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  observed_at TEXT NOT NULL,
  last_checked_at TEXT,
  UNIQUE(resource_id, location_id),
  UNIQUE(location_id, locator_ref_id),
  FOREIGN KEY(resource_id, observed_revision_id)
    REFERENCES resource_revisions(resource_id, resource_revision_id)
);
CREATE INDEX idx_resource_locations_resource ON resource_locations(resource_id, availability);

-- Raw paths, connector object locators, and browser/session handles remain local.
CREATE TABLE resource_location_bindings (
  location_id TEXT NOT NULL,
  locator_ref_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  private_locator TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  expires_at TEXT,
  PRIMARY KEY(location_id, runtime_incarnation_id),
  FOREIGN KEY(location_id, locator_ref_id) REFERENCES resource_locations(location_id, locator_ref_id),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
);
CREATE INDEX idx_resource_location_bindings_incarnation ON resource_location_bindings(runtime_id, runtime_incarnation_id, location_id);

CREATE TRIGGER resource_location_binding_matches_current_incarnation
BEFORE INSERT ON resource_location_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM resource_locations l
  JOIN runtimes r ON r.runtime_id = NEW.runtime_id
  WHERE l.location_id = NEW.location_id
    AND l.locator_ref_id = NEW.locator_ref_id
    AND (l.runtime_id IS NULL OR l.runtime_id = NEW.runtime_id)
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'RESOURCE_LOCATION_BINDING_MISMATCH');
END;

CREATE TRIGGER resource_location_binding_identity_immutable
BEFORE UPDATE ON resource_location_bindings
WHEN NEW.location_id IS NOT OLD.location_id
  OR NEW.locator_ref_id IS NOT OLD.locator_ref_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.runtime_incarnation_id IS NOT OLD.runtime_incarnation_id
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_RESOURCE_LOCATION_BINDING_IDENTITY');
END;

-- Raw OS file identifiers stay on their source Runtime. Aggregate FileIdentity fields
-- contain keyed pseudonyms computed with the Runtime identity key in the OS keystore.
CREATE TABLE file_identity_bindings (
  location_id TEXT NOT NULL REFERENCES resource_locations(location_id),
  runtime_id TEXT NOT NULL,
  runtime_incarnation_id TEXT NOT NULL,
  raw_filesystem_instance_id TEXT NOT NULL,
  raw_volume_id TEXT,
  raw_file_id TEXT NOT NULL,
  raw_generation TEXT,
  platform_kind TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  PRIMARY KEY(location_id, runtime_incarnation_id),
  FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
);
CREATE INDEX idx_file_identity_bindings_incarnation ON file_identity_bindings(runtime_id, runtime_incarnation_id, location_id);

CREATE TRIGGER file_identity_binding_matches_current_incarnation
BEFORE INSERT ON file_identity_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM resource_locations l
  JOIN runtimes r ON r.runtime_id = NEW.runtime_id
  WHERE l.location_id = NEW.location_id
    AND (l.runtime_id IS NULL OR l.runtime_id = NEW.runtime_id)
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'FILE_IDENTITY_BINDING_MISMATCH');
END;

CREATE TABLE workspace_roots (
  workspace_root_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  resource_id TEXT NOT NULL REFERENCES resources(resource_id),
  location_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  watch_policy TEXT NOT NULL CHECK (watch_policy IN ('METADATA', 'CONTENT_DIGESTS', 'SELECTED_TEXT_EXTRACTION')),
  replication_policy TEXT NOT NULL CHECK (replication_policy IN ('NONE', 'ACTIVE_TASKS', 'SELECTED_WORKSPACE_POLICY')),
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'PAUSED', 'REVOKED', 'UNAVAILABLE')),
  added_by_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, workspace_root_id),
  FOREIGN KEY(workspace_id, resource_id) REFERENCES resources(workspace_id, resource_id),
  FOREIGN KEY(resource_id, location_id) REFERENCES resource_locations(resource_id, location_id)
);
CREATE UNIQUE INDEX uq_active_workspace_root_location ON workspace_roots(workspace_id, resource_id, location_id) WHERE status <> 'REVOKED';

CREATE TABLE workspace_replication_roots (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  workspace_root_id TEXT NOT NULL,
  PRIMARY KEY(workspace_id, workspace_root_id),
  FOREIGN KEY(workspace_id, workspace_root_id)
    REFERENCES workspace_roots(workspace_id, workspace_root_id)
);

CREATE TABLE resource_edges (
  edge_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  from_resource_id TEXT NOT NULL,
  from_revision_id TEXT,
  relation TEXT NOT NULL CHECK (relation IN ('CONTAINS', 'DERIVED_FROM', 'REFERENCES', 'SAME_PROVIDER_OBJECT')),
  to_resource_id TEXT NOT NULL,
  to_revision_id TEXT,
  observed_at TEXT NOT NULL,
  provenance_json TEXT NOT NULL,
  FOREIGN KEY(workspace_id, from_resource_id) REFERENCES resources(workspace_id, resource_id),
  FOREIGN KEY(from_resource_id, from_revision_id) REFERENCES resource_revisions(resource_id, resource_revision_id),
  FOREIGN KEY(workspace_id, to_resource_id) REFERENCES resources(workspace_id, resource_id),
  FOREIGN KEY(to_resource_id, to_revision_id) REFERENCES resource_revisions(resource_id, resource_revision_id)
);
CREATE INDEX idx_resource_edges_from ON resource_edges(from_resource_id, relation);
CREATE INDEX idx_resource_edges_to ON resource_edges(to_resource_id, relation);

-- Rebuildable reverse dependency index. Authoritative input refs live on immutable
-- ArtifactVersion/VerificationRun aggregate state and their creation events.
CREATE TABLE dependency_edges (
  dependency_edge_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  source_resource_id TEXT NOT NULL,
  source_revision_id TEXT NOT NULL,
  dependent_kind TEXT NOT NULL CHECK (dependent_kind IN ('ARTIFACT_VERSION', 'VERIFICATION_RUN')),
  dependent_ref TEXT NOT NULL,
  artifact_id TEXT,
  artifact_version INTEGER,
  verification_run_id TEXT REFERENCES verification_runs(verification_run_id),
  created_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id, source_resource_id) REFERENCES resources(workspace_id, resource_id),
  FOREIGN KEY(source_resource_id, source_revision_id) REFERENCES resource_revisions(resource_id, resource_revision_id),
  FOREIGN KEY(artifact_id, artifact_version) REFERENCES artifact_versions(artifact_id, version),
  CHECK (
    (dependent_kind = 'ARTIFACT_VERSION' AND artifact_id IS NOT NULL AND artifact_version IS NOT NULL AND verification_run_id IS NULL AND dependent_ref = 'artifact://' || workspace_id || '/' || artifact_id || '@v' || artifact_version) OR
    (dependent_kind = 'VERIFICATION_RUN' AND artifact_id IS NULL AND artifact_version IS NULL AND verification_run_id IS NOT NULL AND dependent_ref = 'verification://' || verification_run_id)
  ),
  UNIQUE(source_resource_id, source_revision_id, dependent_kind, dependent_ref)
);
CREATE INDEX idx_dependency_edges_source ON dependency_edges(source_resource_id, source_revision_id);
CREATE INDEX idx_dependency_edges_dependent ON dependency_edges(dependent_kind, dependent_ref);

CREATE TRIGGER dependency_edge_same_workspace
BEFORE INSERT ON dependency_edges
WHEN
  (NEW.dependent_kind = 'ARTIFACT_VERSION' AND (SELECT workspace_id FROM artifacts WHERE artifact_id = NEW.artifact_id) <> NEW.workspace_id) OR
  (NEW.dependent_kind = 'VERIFICATION_RUN' AND (SELECT w.workspace_id FROM verification_runs vr JOIN tasks t ON t.task_id = vr.task_id JOIN workspaces w ON w.workspace_id = t.workspace_id WHERE vr.verification_run_id = NEW.verification_run_id) <> NEW.workspace_id)
BEGIN
  SELECT RAISE(ABORT, 'DEPENDENCY_TARGET_WORKSPACE_MISMATCH');
END;

CREATE TABLE invalidation_records (
  invalidation_record_id TEXT PRIMARY KEY,
  dependency_edge_id TEXT NOT NULL REFERENCES dependency_edges(dependency_edge_id),
  observed_revision_id TEXT NOT NULL REFERENCES resource_revisions(resource_revision_id),
  reason_code TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(dependency_edge_id, observed_revision_id)
);
CREATE INDEX idx_invalidations_observed_revision ON invalidation_records(observed_revision_id);

CREATE TRIGGER invalidation_observation_same_resource
BEFORE INSERT ON invalidation_records
WHEN (SELECT source_resource_id FROM dependency_edges WHERE dependency_edge_id = NEW.dependency_edge_id)
  <> (SELECT resource_id FROM resource_revisions WHERE resource_revision_id = NEW.observed_revision_id)
  OR (SELECT source_revision_id FROM dependency_edges WHERE dependency_edge_id = NEW.dependency_edge_id)
   = NEW.observed_revision_id
BEGIN
  SELECT RAISE(ABORT, 'INVALID_INVALIDATION_SOURCE');
END;

-- A live AgentSession may admit a new provider call only while its parent turn/task/
-- attempt still owns execution authority on the current Runtime incarnation.
CREATE VIEW live_agent_session_invocation_authority AS
SELECT s.agent_session_id
FROM agent_sessions s
WHERE s.status = 'ACTIVE'
  AND EXISTS (
    SELECT 1 FROM runtimes r
    WHERE r.runtime_id = s.runtime_id
      AND r.current_incarnation_id = s.runtime_incarnation_id
  )
  AND (
    (s.scope_kind = 'CONVERSATION' AND EXISTS (
      SELECT 1 FROM conversation_turns ct
      WHERE ct.conversation_id = s.conversation_id
        AND ct.turn_id = s.conversation_turn_id
        AND ct.agent_session_id = s.agent_session_id
        AND ct.status = 'RUNNING'
    ))
    OR
    (s.scope_kind = 'TASK_PLANNING' AND EXISTS (
      SELECT 1 FROM tasks t
      WHERE t.task_id = s.task_id
        AND t.workspace_id = s.workspace_id
        AND t.status = 'RUNNING'
        AND t.current_spec_revision = s.task_spec_revision
        AND t.lead_agent_binding_id = s.agent_binding_id
    ))
    OR
    (s.scope_kind = 'ATTEMPT_EXECUTION' AND EXISTS (
      SELECT 1
      FROM attempts p
      JOIN tasks t ON t.task_id = p.task_id
      JOIN steps st ON st.step_id = p.step_id AND st.task_id = p.task_id
      JOIN plan_revisions pr ON pr.task_id = st.task_id AND pr.revision = st.plan_revision
      JOIN execution_leases l ON l.lease_id = p.execution_lease_id
      WHERE p.attempt_id = s.attempt_id
        AND p.task_id = s.task_id
        AND p.agent_session_id = s.agent_session_id
        AND p.agent_binding_id = s.agent_binding_id
        AND p.runtime_id = s.runtime_id
        AND p.runtime_incarnation_id = s.runtime_incarnation_id
        AND p.status = 'RUNNING'
        AND st.status = 'RUNNING'
        AND st.current_attempt_id = p.attempt_id
        AND l.task_id = p.task_id
        AND l.step_id = p.step_id
        AND l.attempt_id = p.attempt_id
        AND l.runtime_id = p.runtime_id
        AND l.runtime_incarnation_id = p.runtime_incarnation_id
        AND l.state = 'ACTIVE'
        AND julianday(l.expires_at) > julianday('now')
        AND t.workspace_id = s.workspace_id
        AND t.status = 'RUNNING'
        AND pr.task_spec_revision = s.task_spec_revision
    ))
  );

CREATE TABLE capability_invocations (
  invocation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  scope_kind TEXT NOT NULL CHECK (scope_kind IN ('CONVERSATION', 'TASK_PLANNING', 'ATTEMPT_EXECUTION')),
  conversation_id TEXT REFERENCES conversations(conversation_id),
  task_id TEXT REFERENCES tasks(task_id),
  attempt_id TEXT REFERENCES attempts(attempt_id),
  agent_session_id TEXT NOT NULL REFERENCES agent_sessions(agent_session_id),
  activation_id TEXT NOT NULL REFERENCES capability_activations(activation_id),
  capability_ref_json TEXT NOT NULL,
  operation TEXT NOT NULL,
  request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  capability_grant_id TEXT NOT NULL REFERENCES capability_grants(capability_grant_id),
  idempotency_key TEXT,
  status TEXT NOT NULL CHECK (status IN ('CREATED', 'DISPATCHED', 'WAITING', 'INPUT_REQUIRED', 'CANCEL_REQUESTED', 'SUCCEEDED', 'FAILED', 'CANCELLED', 'AMBIGUOUS')),
  provider_task_status TEXT CHECK (provider_task_status IS NULL OR provider_task_status IN ('WORKING', 'INPUT_REQUIRED', 'COMPLETED', 'FAILED', 'CANCELLED', 'UNKNOWN')),
  provider_task_created_at TEXT,
  provider_task_expires_at TEXT,
  provider_task_ttl_ms INTEGER CHECK (provider_task_ttl_ms IS NULL OR provider_task_ttl_ms >= 0),
  provider_poll_after_ms INTEGER CHECK (provider_poll_after_ms IS NULL OR provider_poll_after_ms >= 0),
  provider_updated_at TEXT,
  partial_result_refs_json TEXT NOT NULL DEFAULT '[]',
  result_refs_json TEXT NOT NULL DEFAULT '[]',
  effect_id TEXT REFERENCES effects(effect_id),
  failure_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  CHECK ((scope_kind = 'CONVERSATION' AND conversation_id IS NOT NULL AND task_id IS NULL AND attempt_id IS NULL AND effect_id IS NULL) OR (scope_kind = 'TASK_PLANNING' AND conversation_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NULL AND effect_id IS NULL) OR (scope_kind = 'ATTEMPT_EXECUTION' AND conversation_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NOT NULL))
);
CREATE INDEX idx_invocations_workspace_status ON capability_invocations(workspace_id, status, updated_at);

-- Runtime-local encrypted provider handles/cursors. Never include these values in
-- DomainEvents, aggregate state blobs, Mesh replication, Operator API, or backups.
CREATE TABLE capability_invocation_provider_bindings (
  invocation_id TEXT PRIMARY KEY REFERENCES capability_invocations(invocation_id),
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  last_runtime_incarnation_id TEXT NOT NULL,
  binding_ciphertext BLOB NOT NULL,
  encryption_key_version INTEGER NOT NULL CHECK (encryption_key_version >= 1),
  binding_digest TEXT NOT NULL CHECK (length(binding_digest) = 71 AND substr(binding_digest, 1, 7) = 'sha256:' AND substr(binding_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  state TEXT NOT NULL CHECK (state IN ('AVAILABLE', 'RECONCILIATION_REQUIRED', 'UNAVAILABLE')),
  observed_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(runtime_id, last_runtime_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id)
);

CREATE TRIGGER capability_invocation_provider_binding_matches_activation
BEFORE INSERT ON capability_invocation_provider_bindings
WHEN NOT EXISTS (
  SELECT 1 FROM capability_invocations i
  JOIN capability_activations a ON a.activation_id = i.activation_id
  WHERE i.invocation_id = NEW.invocation_id
    AND a.runtime_id = NEW.runtime_id
)
BEGIN
  SELECT RAISE(ABORT, 'INVOCATION_PROVIDER_BINDING_RUNTIME_MISMATCH');
END;

CREATE TRIGGER capability_invocation_provider_binding_identity_immutable
BEFORE UPDATE ON capability_invocation_provider_bindings
WHEN OLD.invocation_id <> NEW.invocation_id
  OR OLD.runtime_id <> NEW.runtime_id
BEGIN
  SELECT RAISE(ABORT, 'INVOCATION_PROVIDER_BINDING_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER capability_invocation_admission_matches_scope
BEFORE INSERT ON capability_invocations
WHEN NOT EXISTS (
  SELECT 1
  FROM agent_sessions s
  JOIN live_agent_session_invocation_authority live ON live.agent_session_id = s.agent_session_id
  JOIN capability_grants g ON g.capability_grant_id = NEW.capability_grant_id
  JOIN capability_activations a ON a.activation_id = NEW.activation_id
  WHERE s.agent_session_id = NEW.agent_session_id
    AND s.workspace_id = NEW.workspace_id
    AND s.scope_kind = NEW.scope_kind
    AND s.conversation_id IS NEW.conversation_id
    AND s.task_id IS NEW.task_id
    AND s.attempt_id IS NEW.attempt_id
    AND s.status = 'ACTIVE'
    AND g.scope_kind = NEW.scope_kind
    AND g.conversation_id IS NEW.conversation_id
    AND g.task_id IS NEW.task_id
    AND g.attempt_id IS NEW.attempt_id
    AND g.capability_ref_json = NEW.capability_ref_json
    AND g.status = 'ACTIVE'
    AND (g.expires_at IS NULL OR julianday(g.expires_at) > julianday('now'))
    AND EXISTS (
      SELECT 1 FROM json_each(g.allowed_operations_json) op
      WHERE op.value = NEW.operation
    )
    AND a.capability_ref_json = NEW.capability_ref_json
    AND a.status = 'ACTIVE'
    AND a.workspace_id = s.workspace_id
    AND a.scope_kind = s.scope_kind
    AND a.conversation_id IS s.conversation_id
    AND a.task_id IS s.task_id
    AND a.attempt_id IS s.attempt_id
    AND a.runtime_id = s.runtime_id
    AND a.runtime_incarnation_id = s.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH');
END;

CREATE TRIGGER capability_invocation_first_dispatch_revalidates_owner
BEFORE UPDATE OF status ON capability_invocations
WHEN OLD.status = 'CREATED' AND NEW.status = 'DISPATCHED' AND NOT EXISTS (
  SELECT 1
  FROM agent_sessions s
  JOIN live_agent_session_invocation_authority live ON live.agent_session_id = s.agent_session_id
  JOIN capability_grants g ON g.capability_grant_id = NEW.capability_grant_id
  JOIN capability_activations a ON a.activation_id = NEW.activation_id
  WHERE s.agent_session_id = NEW.agent_session_id
    AND s.workspace_id = NEW.workspace_id
    AND s.scope_kind = NEW.scope_kind
    AND s.conversation_id IS NEW.conversation_id
    AND s.task_id IS NEW.task_id
    AND s.attempt_id IS NEW.attempt_id
    AND g.scope_kind = NEW.scope_kind
    AND g.conversation_id IS NEW.conversation_id
    AND g.task_id IS NEW.task_id
    AND g.attempt_id IS NEW.attempt_id
    AND g.capability_ref_json = NEW.capability_ref_json
    AND g.status = 'ACTIVE'
    AND (g.expires_at IS NULL OR julianday(g.expires_at) > julianday('now'))
    AND EXISTS (
      SELECT 1 FROM json_each(g.allowed_operations_json) op
      WHERE op.value = NEW.operation
    )
    AND a.workspace_id = s.workspace_id
    AND a.scope_kind = s.scope_kind
    AND a.conversation_id IS s.conversation_id
    AND a.task_id IS s.task_id
    AND a.attempt_id IS s.attempt_id
    AND a.capability_ref_json = NEW.capability_ref_json
    AND a.runtime_id = s.runtime_id
    AND a.runtime_incarnation_id = s.runtime_incarnation_id
    AND a.status = 'ACTIVE'
)
BEGIN
  SELECT RAISE(ABORT, 'INVOCATION_OWNER_NOT_LIVE_AT_DISPATCH');
END;

CREATE TRIGGER capability_invocation_identity_immutable
BEFORE UPDATE ON capability_invocations
WHEN OLD.workspace_id <> NEW.workspace_id
  OR OLD.scope_kind <> NEW.scope_kind
  OR OLD.conversation_id IS NOT NEW.conversation_id
  OR OLD.task_id IS NOT NEW.task_id
  OR OLD.attempt_id IS NOT NEW.attempt_id
  OR OLD.agent_session_id <> NEW.agent_session_id
  OR OLD.capability_grant_id <> NEW.capability_grant_id
  OR OLD.activation_id <> NEW.activation_id
  OR OLD.capability_ref_json <> NEW.capability_ref_json
  OR OLD.operation <> NEW.operation
  OR OLD.request_digest <> NEW.request_digest
BEGIN
  SELECT RAISE(ABORT, 'INVOCATION_IDENTITY_IMMUTABLE');
END;

CREATE TABLE user_requests (
  request_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  conversation_id TEXT REFERENCES conversations(conversation_id),
  conversation_turn_id TEXT,
  task_id TEXT REFERENCES tasks(task_id),
  attempt_id TEXT REFERENCES attempts(attempt_id),
  invocation_id TEXT REFERENCES capability_invocations(invocation_id),
  agent_session_id TEXT NOT NULL REFERENCES agent_sessions(agent_session_id),
  kind TEXT NOT NULL CHECK (kind IN ('QUESTION', 'DECISION', 'RESOURCE_SELECTION', 'EXTERNAL_AUTHORIZATION')),
  interaction_mode TEXT NOT NULL DEFAULT 'FORM' CHECK (interaction_mode IN ('FORM', 'EXTERNAL_URL')),
  prompt TEXT NOT NULL,
  response_schema_json TEXT,
  choices_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'ANSWERED', 'DISMISSED', 'EXPIRED', 'CANCELLED')),
  expires_at TEXT,
  created_at TEXT NOT NULL,
  resolved_at TEXT,
  resolved_by_json TEXT,
  response_digest TEXT CHECK (response_digest IS NULL OR (length(response_digest) = 71 AND substr(response_digest, 1, 7) = 'sha256:' AND substr(response_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY(conversation_id, conversation_turn_id) REFERENCES conversation_turns(conversation_id, turn_id),
  CHECK (
    (conversation_id IS NOT NULL AND conversation_turn_id IS NOT NULL AND task_id IS NULL AND attempt_id IS NULL) OR
    (conversation_id IS NULL AND conversation_turn_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NULL) OR
    (conversation_id IS NULL AND conversation_turn_id IS NULL AND task_id IS NOT NULL AND attempt_id IS NOT NULL)
  ),
  CHECK (
    (status = 'ANSWERED' AND response_digest IS NOT NULL AND resolved_at IS NOT NULL AND resolved_by_json IS NOT NULL) OR
    (status <> 'ANSWERED' AND response_digest IS NULL)
  ),
  CHECK (
    (interaction_mode = 'FORM' AND kind <> 'EXTERNAL_AUTHORIZATION') OR
    (interaction_mode = 'EXTERNAL_URL' AND kind = 'EXTERNAL_AUTHORIZATION' AND response_schema_json IS NULL AND choices_json = '[]')
  )
);
CREATE INDEX idx_user_requests_inbox ON user_requests(workspace_id, status, created_at);

CREATE TRIGGER user_request_admission_matches_scope
BEFORE INSERT ON user_requests
WHEN NOT EXISTS (
  SELECT 1 FROM agent_sessions s
  WHERE s.agent_session_id = NEW.agent_session_id
    AND s.workspace_id = NEW.workspace_id
    AND s.conversation_id IS NEW.conversation_id
    AND s.task_id IS NEW.task_id
    AND s.attempt_id IS NEW.attempt_id
    AND (NEW.conversation_id IS NULL OR EXISTS (
      SELECT 1 FROM conversation_turns t
      WHERE t.turn_id = NEW.conversation_turn_id
        AND t.conversation_id = NEW.conversation_id
        AND t.agent_session_id = NEW.agent_session_id
        AND t.status = 'WAITING_USER'
    ))
)
OR (
  NEW.invocation_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM capability_invocations i
    WHERE i.invocation_id = NEW.invocation_id
      AND i.workspace_id = NEW.workspace_id
      AND i.agent_session_id = NEW.agent_session_id
      AND i.conversation_id IS NEW.conversation_id
      AND i.task_id IS NEW.task_id
      AND i.attempt_id IS NEW.attempt_id
  )
)
BEGIN
  SELECT RAISE(ABORT, 'USER_REQUEST_SCOPE_MISMATCH');
END;

CREATE TRIGGER user_request_provenance_immutable
BEFORE UPDATE ON user_requests
WHEN OLD.workspace_id <> NEW.workspace_id
  OR OLD.conversation_id IS NOT NEW.conversation_id
  OR OLD.conversation_turn_id IS NOT NEW.conversation_turn_id
  OR OLD.task_id IS NOT NEW.task_id
  OR OLD.attempt_id IS NOT NEW.attempt_id
  OR OLD.agent_session_id <> NEW.agent_session_id
  OR OLD.invocation_id IS NOT NEW.invocation_id
  OR OLD.kind <> NEW.kind
  OR OLD.interaction_mode <> NEW.interaction_mode
  OR OLD.prompt <> NEW.prompt
  OR OLD.response_schema_json IS NOT NEW.response_schema_json
  OR OLD.choices_json <> NEW.choices_json
BEGIN
  SELECT RAISE(ABORT, 'USER_REQUEST_PROVENANCE_IMMUTABLE');
END;

CREATE TRIGGER user_request_status_transition_guard
BEFORE UPDATE OF status ON user_requests
WHEN OLD.status <> NEW.status
  AND NOT (OLD.status = 'PENDING' AND NEW.status IN ('ANSWERED', 'DISMISSED', 'EXPIRED', 'CANCELLED'))
BEGIN
  SELECT RAISE(ABORT, 'INVALID_USER_REQUEST_TRANSITION');
END;

CREATE TRIGGER user_request_answer_matches_response
BEFORE UPDATE OF status, resolved_at, resolved_by_json, response_digest ON user_requests
WHEN NEW.status = 'ANSWERED' AND NOT EXISTS (
  SELECT 1 FROM user_request_responses r
  WHERE r.request_id = NEW.request_id
    AND r.response_digest = NEW.response_digest
    AND r.responded_at = NEW.resolved_at
    AND r.responded_by_json = NEW.resolved_by_json
)
BEGIN
  SELECT RAISE(ABORT, 'USER_REQUEST_RESPONSE_MISMATCH');
END;

CREATE TABLE user_request_responses (
  response_id TEXT PRIMARY KEY,
  request_id TEXT NOT NULL REFERENCES user_requests(request_id),
  response_json TEXT NOT NULL CHECK (json_valid(response_json)),
  response_digest TEXT NOT NULL CHECK (length(response_digest) = 71 AND substr(response_digest, 1, 7) = 'sha256:' AND substr(response_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  responded_by_json TEXT NOT NULL,
  response_channel_binding_id TEXT,
  response_provider_event_id TEXT,
  responded_at TEXT NOT NULL,
  UNIQUE(request_id),
  CHECK ((response_channel_binding_id IS NULL) = (response_provider_event_id IS NULL))
);

CREATE TRIGGER user_request_response_insert_pending
BEFORE INSERT ON user_request_responses
WHEN NOT EXISTS (
  SELECT 1 FROM user_requests u
  WHERE u.request_id = NEW.request_id
    AND u.status = 'PENDING'
    AND u.response_digest IS NULL
)
BEGIN
  SELECT RAISE(ABORT, 'USER_REQUEST_NOT_PENDING');
END;

CREATE TRIGGER user_request_external_response_shape
BEFORE INSERT ON user_request_responses
WHEN EXISTS (
  SELECT 1 FROM user_requests u
  WHERE u.request_id = NEW.request_id
    AND u.interaction_mode = 'EXTERNAL_URL'
)
AND (
  json_type(NEW.response_json) IS NOT 'object'
  OR json_type(NEW.response_json, '$.action') IS NOT 'text'
  OR json_extract(NEW.response_json, '$.action') IS NULL
  OR json_extract(NEW.response_json, '$.action') NOT IN ('accept', 'decline', 'cancel')
  OR (SELECT count(*) FROM json_each(NEW.response_json)) <> 1
)
BEGIN
  SELECT RAISE(ABORT, 'INVALID_EXTERNAL_AUTH_RESPONSE');
END;

CREATE TRIGGER user_request_response_parent_eligible
BEFORE INSERT ON user_request_responses
WHEN EXISTS (
  SELECT 1 FROM user_requests u
  WHERE u.request_id = NEW.request_id
    AND (
      (u.conversation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM conversation_turns ct
        WHERE ct.conversation_id = u.conversation_id
          AND ct.turn_id = u.conversation_turn_id
          AND ct.status = 'WAITING_USER'
      ))
      OR
      (u.task_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM tasks t
        WHERE t.task_id = u.task_id
          AND t.status IN ('READY', 'RUNNING', 'WAITING_USER', 'BLOCKED', 'NEEDS_USER', 'INCOMPLETE', 'PAUSED')
      ))
    )
)
BEGIN
  SELECT RAISE(ABORT, 'USER_REQUEST_PARENT_NOT_RESPONDABLE');
END;

CREATE TRIGGER user_request_response_immutable_update
BEFORE UPDATE ON user_request_responses
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_USER_REQUEST_RESPONSE');
END;

CREATE TRIGGER user_request_response_immutable_delete
BEFORE DELETE ON user_request_responses
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_USER_REQUEST_RESPONSE');
END;

-- Provider request keys and response-delivery state stay with the Runtime that owns
-- the provider task. Only request/response IDs and safe digests replicate.
CREATE TABLE provider_input_bindings (
  request_id TEXT PRIMARY KEY REFERENCES user_requests(request_id),
  invocation_id TEXT NOT NULL REFERENCES capability_invocations(invocation_id),
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  provider_input_key_ciphertext BLOB NOT NULL,
  provider_input_payload_ciphertext BLOB NOT NULL,
  encryption_key_version INTEGER NOT NULL CHECK (encryption_key_version >= 1),
  provider_input_key_tag TEXT NOT NULL CHECK (length(provider_input_key_tag) = 71 AND substr(provider_input_key_tag, 1, 7) = 'sha256:' AND substr(provider_input_key_tag, 8) NOT GLOB '*[^0-9a-f]*'),
  input_request_digest TEXT NOT NULL CHECK (length(input_request_digest) = 71 AND substr(input_request_digest, 1, 7) = 'sha256:' AND substr(input_request_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  response_id TEXT REFERENCES user_request_responses(response_id),
  response_digest TEXT CHECK (response_digest IS NULL OR (length(response_digest) = 71 AND substr(response_digest, 1, 7) = 'sha256:' AND substr(response_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  delivery_status TEXT NOT NULL CHECK (delivery_status IN ('AWAITING_RESPONSE', 'PENDING', 'DISPATCHED', 'ACKNOWLEDGED', 'ACCEPTED', 'AMBIGUOUS', 'REJECTED', 'EXPIRED', 'CANCELLED')),
  dispatch_count INTEGER NOT NULL DEFAULT 0 CHECK (dispatch_count >= 0),
  last_dispatch_at TEXT,
  last_provider_observation_at TEXT,
  retry_safety_proof_digest TEXT CHECK (retry_safety_proof_digest IS NULL OR (length(retry_safety_proof_digest) = 71 AND substr(retry_safety_proof_digest, 1, 7) = 'sha256:' AND substr(retry_safety_proof_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  failure_code TEXT,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(invocation_id, provider_input_key_tag),
  CHECK (
    (response_id IS NULL AND response_digest IS NULL AND delivery_status IN ('AWAITING_RESPONSE', 'EXPIRED', 'REJECTED', 'CANCELLED'))
    OR
    (response_id IS NOT NULL AND response_digest IS NOT NULL AND delivery_status IN ('PENDING', 'DISPATCHED', 'ACKNOWLEDGED', 'ACCEPTED', 'AMBIGUOUS', 'REJECTED', 'CANCELLED'))
  )
);
CREATE INDEX idx_provider_input_delivery ON provider_input_bindings(runtime_id, delivery_status, updated_at);

CREATE TRIGGER provider_input_binding_insert_is_awaiting
BEFORE INSERT ON provider_input_bindings
WHEN NEW.delivery_status <> 'AWAITING_RESPONSE'
  OR NEW.response_id IS NOT NULL
  OR NEW.response_digest IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'PROVIDER_INPUT_BINDING_MUST_START_AWAITING');
END;

CREATE TRIGGER provider_input_binding_scope_matches_request
BEFORE INSERT ON provider_input_bindings
WHEN NOT EXISTS (
  SELECT 1
  FROM user_requests u JOIN capability_invocations i ON i.invocation_id = NEW.invocation_id
  JOIN capability_activations a ON a.activation_id = i.activation_id
  WHERE u.request_id = NEW.request_id
    AND u.invocation_id = NEW.invocation_id
    AND i.agent_session_id = u.agent_session_id
    AND i.workspace_id = u.workspace_id
    AND a.runtime_id = NEW.runtime_id
)
BEGIN
  SELECT RAISE(ABORT, 'PROVIDER_INPUT_BINDING_SCOPE_MISMATCH');
END;

CREATE TRIGGER provider_input_binding_identity_immutable
BEFORE UPDATE ON provider_input_bindings
WHEN OLD.request_id <> NEW.request_id
  OR OLD.invocation_id <> NEW.invocation_id
  OR OLD.runtime_id <> NEW.runtime_id
  OR OLD.provider_input_key_ciphertext <> NEW.provider_input_key_ciphertext
  OR OLD.provider_input_payload_ciphertext <> NEW.provider_input_payload_ciphertext
  OR OLD.encryption_key_version <> NEW.encryption_key_version
  OR OLD.provider_input_key_tag <> NEW.provider_input_key_tag
  OR OLD.input_request_digest <> NEW.input_request_digest
BEGIN
  SELECT RAISE(ABORT, 'PROVIDER_INPUT_BINDING_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER provider_input_binding_response_immutable
BEFORE UPDATE ON provider_input_bindings
WHEN (OLD.response_id IS NOT NULL AND (
        OLD.response_id IS NOT NEW.response_id
        OR OLD.response_digest IS NOT NEW.response_digest
      ))
  OR (OLD.response_id IS NULL AND NEW.response_id IS NOT NULL AND NOT (
        OLD.delivery_status = 'AWAITING_RESPONSE'
        AND NEW.delivery_status = 'PENDING'
      ))
  OR (NEW.response_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM user_request_responses r
        WHERE r.response_id = NEW.response_id
          AND r.request_id = NEW.request_id
          AND r.response_digest = NEW.response_digest
      ))
BEGIN
  SELECT RAISE(ABORT, 'PROVIDER_INPUT_RESPONSE_IMMUTABLE_OR_MISMATCHED');
END;

CREATE TRIGGER provider_input_binding_transition_guard
BEFORE UPDATE OF delivery_status ON provider_input_bindings
WHEN OLD.delivery_status <> NEW.delivery_status
  AND NOT (
    (OLD.delivery_status = 'AWAITING_RESPONSE' AND NEW.delivery_status IN ('PENDING', 'REJECTED', 'EXPIRED', 'CANCELLED'))
    OR (OLD.delivery_status = 'PENDING' AND NEW.delivery_status IN ('DISPATCHED', 'CANCELLED'))
    OR (OLD.delivery_status = 'DISPATCHED' AND NEW.delivery_status IN ('ACKNOWLEDGED', 'AMBIGUOUS', 'REJECTED'))
    OR (OLD.delivery_status = 'ACKNOWLEDGED' AND NEW.delivery_status IN ('ACCEPTED', 'AMBIGUOUS', 'REJECTED'))
    OR (OLD.delivery_status = 'AMBIGUOUS' AND NEW.delivery_status IN ('DISPATCHED', 'ACCEPTED', 'REJECTED'))
  )
BEGIN
  SELECT RAISE(ABORT, 'INVALID_PROVIDER_INPUT_BINDING_TRANSITION');
END;

CREATE TRIGGER provider_input_binding_dispatch_proof_guard
BEFORE UPDATE ON provider_input_bindings
WHEN (NEW.delivery_status = 'DISPATCHED'
      AND OLD.delivery_status <> 'DISPATCHED'
      AND (NEW.dispatch_count <> OLD.dispatch_count + 1 OR NEW.last_dispatch_at IS NULL))
  OR (NOT (OLD.delivery_status <> 'DISPATCHED' AND NEW.delivery_status = 'DISPATCHED')
      AND NEW.dispatch_count <> OLD.dispatch_count)
  OR (OLD.delivery_status = 'PENDING' AND NEW.delivery_status = 'DISPATCHED'
      AND NEW.retry_safety_proof_digest IS NOT NULL)
  OR (OLD.delivery_status = 'AMBIGUOUS' AND NEW.delivery_status = 'DISPATCHED'
      AND (NEW.retry_safety_proof_digest IS NULL OR NEW.last_provider_observation_at IS NULL))
  OR (OLD.delivery_status IN ('DISPATCHED', 'ACKNOWLEDGED') AND NEW.delivery_status = 'AMBIGUOUS'
      AND NEW.retry_safety_proof_digest IS NOT NULL)
BEGIN
  SELECT RAISE(ABORT, 'PROVIDER_INPUT_DISPATCH_PROOF_OR_COUNT_INVALID');
END;

-- A response may reach the provider only while its original owner is current again.
-- This is separate from AgentSession call admission: only the exact stored response to
-- this already-running provider task is authorized, not a new capability operation.
CREATE TRIGGER provider_input_binding_dispatch_authority
BEFORE UPDATE OF delivery_status ON provider_input_bindings
WHEN OLD.delivery_status IN ('PENDING', 'AMBIGUOUS')
  AND NEW.delivery_status = 'DISPATCHED'
  AND NOT EXISTS (
    SELECT 1
    FROM user_requests u
    JOIN user_request_responses ur ON ur.response_id = NEW.response_id
    JOIN capability_invocations i ON i.invocation_id = NEW.invocation_id
    JOIN agent_sessions source_session ON source_session.agent_session_id = i.agent_session_id
    JOIN capability_grants g ON g.capability_grant_id = i.capability_grant_id
    JOIN capability_activations a ON a.activation_id = i.activation_id
    JOIN capability_invocation_provider_bindings provider_binding
      ON provider_binding.invocation_id = i.invocation_id
    JOIN runtimes r ON r.runtime_id = NEW.runtime_id
    WHERE u.request_id = NEW.request_id
      AND u.status = 'ANSWERED'
      AND ur.request_id = u.request_id
      AND ur.response_digest = NEW.response_digest
      AND u.response_digest = NEW.response_digest
      AND u.invocation_id = i.invocation_id
      AND u.agent_session_id = i.agent_session_id
      AND source_session.status IN ('CLOSED', 'LOST')
      AND i.status = 'INPUT_REQUIRED'
      AND i.provider_task_status = 'INPUT_REQUIRED'
      AND provider_binding.runtime_id = NEW.runtime_id
      AND provider_binding.state = 'AVAILABLE'
      AND provider_binding.last_runtime_incarnation_id = source_session.runtime_incarnation_id
      AND r.current_incarnation_id = source_session.runtime_incarnation_id
      AND a.workspace_id = i.workspace_id
      AND a.scope_kind = i.scope_kind
      AND a.conversation_id IS i.conversation_id
      AND a.task_id IS i.task_id
      AND a.attempt_id IS i.attempt_id
      AND a.capability_ref_json = i.capability_ref_json
      AND a.runtime_id = source_session.runtime_id
      AND a.runtime_incarnation_id = source_session.runtime_incarnation_id
      AND a.status = 'ACTIVE'
      AND g.scope_kind = i.scope_kind
      AND g.conversation_id IS i.conversation_id
      AND g.task_id IS i.task_id
      AND g.attempt_id IS i.attempt_id
      AND g.capability_ref_json = i.capability_ref_json
      AND g.status = 'ACTIVE'
      AND (g.expires_at IS NULL OR julianday(g.expires_at) > julianday('now'))
      AND EXISTS (
        SELECT 1 FROM json_each(g.allowed_operations_json) op
        WHERE op.value = i.operation
      )
      AND (
        (i.scope_kind = 'CONVERSATION' AND EXISTS (
          SELECT 1 FROM conversation_turns ct
          WHERE ct.conversation_id = i.conversation_id
            AND ct.turn_id = source_session.conversation_turn_id
            AND ct.agent_session_id = i.agent_session_id
            AND ct.status = 'WAITING_DEPENDENCY'
        ))
        OR
        (i.scope_kind = 'TASK_PLANNING' AND EXISTS (
          SELECT 1 FROM tasks t
          WHERE t.task_id = i.task_id
            AND t.workspace_id = i.workspace_id
            AND t.status = 'RUNNING'
            AND t.current_spec_revision = source_session.task_spec_revision
            AND t.lead_agent_binding_id = source_session.agent_binding_id
            AND EXISTS (
              SELECT 1 FROM agent_sessions current_planner
              WHERE current_planner.scope_kind = 'TASK_PLANNING'
                AND current_planner.workspace_id = t.workspace_id
                AND current_planner.task_id = t.task_id
                AND current_planner.task_spec_revision = t.current_spec_revision
                AND current_planner.agent_binding_id = t.lead_agent_binding_id
                AND current_planner.agent_session_id <> source_session.agent_session_id
                AND current_planner.status = 'ACTIVE'
                AND EXISTS (
                  SELECT 1 FROM runtimes planner_runtime
                  WHERE planner_runtime.runtime_id = current_planner.runtime_id
                    AND planner_runtime.current_incarnation_id = current_planner.runtime_incarnation_id
                )
            )
        ))
        OR
        (i.scope_kind = 'ATTEMPT_EXECUTION' AND EXISTS (
          SELECT 1
          FROM tasks t
          JOIN attempts p ON p.task_id = t.task_id
          JOIN steps st ON st.task_id = p.task_id AND st.step_id = p.step_id
          JOIN plan_revisions pr ON pr.task_id = st.task_id AND pr.revision = st.plan_revision
          JOIN agent_sessions active_session
            ON active_session.agent_session_id = p.agent_session_id
          JOIN execution_leases l ON l.lease_id = p.execution_lease_id
          WHERE t.task_id = i.task_id
            AND t.workspace_id = i.workspace_id
            AND t.status = 'RUNNING'
            AND t.current_plan_revision = st.plan_revision
            AND p.attempt_id = i.attempt_id
            AND p.step_id = st.step_id
            AND p.agent_binding_id = source_session.agent_binding_id
            AND p.runtime_id = source_session.runtime_id
            AND p.runtime_incarnation_id = source_session.runtime_incarnation_id
            AND p.status = 'RUNNING'
            AND active_session.agent_session_id <> source_session.agent_session_id
            AND active_session.scope_kind = 'ATTEMPT_EXECUTION'
            AND active_session.task_id = p.task_id
            AND active_session.attempt_id = p.attempt_id
            AND active_session.agent_binding_id = p.agent_binding_id
            AND active_session.runtime_id = p.runtime_id
            AND active_session.runtime_incarnation_id = p.runtime_incarnation_id
            AND active_session.task_spec_revision = pr.task_spec_revision
            AND active_session.status = 'ACTIVE'
            AND st.current_attempt_id = p.attempt_id
            AND st.status = 'RUNNING'
            AND pr.task_spec_revision = source_session.task_spec_revision
            AND l.task_id = p.task_id
            AND l.step_id = p.step_id
            AND l.attempt_id = p.attempt_id
            AND l.runtime_id = p.runtime_id
            AND l.runtime_incarnation_id = p.runtime_incarnation_id
            AND l.state = 'ACTIVE'
            AND julianday(l.expires_at) > julianday('now')
        ))
      )
  )
BEGIN
  SELECT RAISE(ABORT, 'PROVIDER_INPUT_OWNER_NOT_LIVE');
END;

CREATE TABLE usage_observations (
  usage_observation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  task_id TEXT,
  environment_id TEXT,
  attempt_id TEXT,
  invocation_id TEXT REFERENCES capability_invocations(invocation_id),
  agent_session_id TEXT REFERENCES agent_sessions(agent_session_id),
  source TEXT NOT NULL CHECK (source IN ('AGENT_REPORTED', 'PROVIDER_REPORTED', 'HOST_MEASURED')),
  metric TEXT NOT NULL,
  quantity TEXT,
  unit TEXT NOT NULL,
  currency TEXT,
  confidence TEXT NOT NULL CHECK (confidence IN ('EXACT', 'ESTIMATED', 'UNKNOWN')),
  observed_at TEXT NOT NULL,
  source_ref TEXT,
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id),
  FOREIGN KEY(environment_id, workspace_id) REFERENCES environments(environment_id, owner_workspace_id),
  FOREIGN KEY(task_id, attempt_id) REFERENCES attempts(task_id, attempt_id),
  CHECK (attempt_id IS NULL OR task_id IS NOT NULL),
  CHECK ((confidence = 'UNKNOWN' AND quantity IS NULL) OR (confidence <> 'UNKNOWN' AND quantity IS NOT NULL)),
  CHECK ((metric = 'COST' AND (currency IS NOT NULL OR confidence = 'UNKNOWN')) OR (metric <> 'COST' AND currency IS NULL))
);
CREATE INDEX idx_usage_task_time ON usage_observations(task_id, observed_at);
CREATE INDEX idx_usage_environment_time ON usage_observations(environment_id, observed_at);

CREATE TABLE budget_reservations (
  reservation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  budget_scope TEXT NOT NULL CHECK (budget_scope IN ('TASK', 'ENVIRONMENT')),
  task_id TEXT,
  environment_id TEXT,
  attempt_id TEXT,
  metric TEXT NOT NULL,
  quantity TEXT NOT NULL,
  unit TEXT NOT NULL,
  currency TEXT,
  state TEXT NOT NULL CHECK (state IN ('RESERVED', 'COMMITTED', 'RELEASED', 'EXPIRED')),
  created_at TEXT NOT NULL,
  expires_at TEXT,
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id),
  FOREIGN KEY(environment_id, workspace_id) REFERENCES environments(environment_id, owner_workspace_id),
  FOREIGN KEY(task_id, attempt_id) REFERENCES attempts(task_id, attempt_id),
  CHECK ((budget_scope = 'TASK' AND task_id IS NOT NULL AND environment_id IS NULL) OR
         (budget_scope = 'ENVIRONMENT' AND task_id IS NULL AND environment_id IS NOT NULL)),
  CHECK (attempt_id IS NULL OR budget_scope = 'TASK'),
  CHECK ((metric = 'COST' AND currency IS NOT NULL) OR (metric <> 'COST' AND currency IS NULL))
);
CREATE INDEX idx_budget_reservations_workspace_scope ON budget_reservations(workspace_id, budget_scope, state);

CREATE TABLE notification_preferences (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  event_class TEXT NOT NULL,
  policy TEXT NOT NULL CHECK (policy IN ('ALWAYS', 'ON_SUCCESS', 'ON_FAILURE', 'ON_CONDITION', 'SILENT')),
  preferred_channels_json TEXT NOT NULL DEFAULT '[]',
  quiet_hours_json TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY(workspace_id, event_class)
);

CREATE TABLE notification_deliveries (
  delivery_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  dedupe_key TEXT NOT NULL,
  source_event_id TEXT NOT NULL REFERENCES domain_events(event_id),
  channel_binding_id TEXT REFERENCES channel_bindings(channel_binding_id),
  attempt_runtime_id TEXT REFERENCES runtimes(runtime_id),
  attempt_host_epoch INTEGER CHECK (attempt_host_epoch IS NULL OR attempt_host_epoch > 0),
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'SENDING', 'SENT', 'FAILED', 'AMBIGUOUS', 'SUPPRESSED')),
  attempt_count INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT,
  last_error_code TEXT,
  created_at TEXT NOT NULL,
  settled_at TEXT,
  UNIQUE(workspace_id, dedupe_key),
  CHECK ((attempt_runtime_id IS NULL) = (attempt_host_epoch IS NULL))
);
CREATE INDEX idx_notification_due ON notification_deliveries(status, next_attempt_at);

-- Exact reply correlation is Runtime-local operational state. Provider message refs are
-- never included in DomainEvents or replicated to another Runtime.
CREATE TABLE channel_reply_targets (
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  channel_binding_id TEXT NOT NULL REFERENCES channel_bindings(channel_binding_id),
  host_epoch INTEGER NOT NULL CHECK (host_epoch > 0),
  provider_message_ref TEXT NOT NULL CHECK (length(provider_message_ref) BETWEEN 1 AND 512),
  delivery_id TEXT NOT NULL REFERENCES notification_deliveries(delivery_id),
  user_request_id TEXT NOT NULL REFERENCES user_requests(request_id),
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'CONSUMED', 'CLOSED', 'EXPIRED')),
  consumed_by_provider_event_id TEXT,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  closed_at TEXT,
  PRIMARY KEY(runtime_id, channel_binding_id, provider_message_ref),
  CHECK ((status = 'ACTIVE') = (closed_at IS NULL)),
  CHECK ((status = 'CONSUMED') = (consumed_by_provider_event_id IS NOT NULL)),
  CHECK (expires_at > created_at),
  FOREIGN KEY(channel_binding_id, consumed_by_provider_event_id)
    REFERENCES channel_event_receipts(channel_binding_id, provider_event_id)
);
CREATE INDEX idx_channel_reply_targets_request ON channel_reply_targets(user_request_id, status);

CREATE TRIGGER user_request_channel_response_authorized
BEFORE INSERT ON user_request_responses
WHEN NEW.response_channel_binding_id IS NOT NULL AND NOT EXISTS (
  SELECT 1
  FROM channel_event_receipts r
  JOIN channel_bindings b ON b.channel_binding_id = r.channel_binding_id
  JOIN user_requests u ON u.request_id = NEW.request_id
  JOIN channel_reply_targets t
    ON t.channel_binding_id = r.channel_binding_id
   AND t.user_request_id = u.request_id
   AND t.status = 'ACTIVE'
  WHERE r.channel_binding_id = NEW.response_channel_binding_id
    AND r.provider_event_id = NEW.response_provider_event_id
    AND r.claim_runtime_id = t.runtime_id
    AND r.claim_host_epoch = t.host_epoch
    AND r.state = 'PROCESSING'
    AND EXISTS (
      SELECT 1 FROM channel_host_assignments a
      JOIN channel_host_lease_records l
        ON l.channel_binding_id = a.channel_binding_id
       AND l.workspace_id = a.workspace_id
       AND l.runtime_id = a.runtime_id
       AND l.host_epoch = a.host_epoch
      WHERE a.channel_binding_id = t.channel_binding_id
        AND a.runtime_id = t.runtime_id
        AND a.host_epoch = t.host_epoch
        AND a.status = 'ACTIVE'
        AND julianday(l.lease_expires_at) > julianday('now')
    )
    AND b.status = 'ACTIVE'
    AND b.workspace_id = u.workspace_id
    AND EXISTS (SELECT 1 FROM json_each(b.allowed_actions_json) a WHERE a.value = 'RESPOND')
    AND b.identity_ref_json = NEW.responded_by_json
    AND u.status = 'PENDING'
    AND u.interaction_mode = 'FORM'
    AND u.kind <> 'EXTERNAL_AUTHORIZATION'
    AND (u.expires_at IS NULL OR julianday(u.expires_at) > julianday('now'))
    AND julianday(t.expires_at) > julianday('now')
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_USER_REQUEST_RESPONSE_NOT_AUTHORIZED');
END;

CREATE TRIGGER channel_reply_target_matches_delivery
BEFORE INSERT ON channel_reply_targets
WHEN NOT EXISTS (
  SELECT 1
  FROM notification_deliveries d
  JOIN domain_events e ON e.event_id = d.source_event_id
  JOIN channel_bindings b ON b.channel_binding_id = d.channel_binding_id
  JOIN channel_host_assignments a ON a.channel_binding_id = b.channel_binding_id
  JOIN channel_host_lease_records l
    ON l.channel_binding_id = a.channel_binding_id
   AND l.workspace_id = a.workspace_id
   AND l.runtime_id = a.runtime_id
   AND l.host_epoch = a.host_epoch
  JOIN user_requests u ON u.request_id = NEW.user_request_id
  WHERE d.delivery_id = NEW.delivery_id
    AND d.channel_binding_id = NEW.channel_binding_id
    AND d.attempt_runtime_id = NEW.runtime_id
    AND d.attempt_host_epoch = NEW.host_epoch
    AND a.runtime_id = NEW.runtime_id
    AND a.host_epoch = NEW.host_epoch
    AND a.status = 'ACTIVE'
    AND julianday(l.lease_expires_at) > julianday('now')
    AND d.status = 'SENT'
    AND e.type = 'user.request.created.v1'
    AND json_extract(e.payload_json, '$.request_id') = NEW.user_request_id
    AND b.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(b.allowed_actions_json) a WHERE a.value = 'RESPOND')
    AND u.workspace_id = d.workspace_id
    AND u.status = 'PENDING'
    AND u.interaction_mode = 'FORM'
    AND u.kind <> 'EXTERNAL_AUTHORIZATION'
    AND (u.expires_at IS NULL OR julianday(u.expires_at) > julianday('now'))
    AND (u.expires_at IS NULL OR NEW.expires_at <= u.expires_at)
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_REPLY_TARGET_DELIVERY_MISMATCH');
END;

CREATE TRIGGER channel_reply_target_terminal
BEFORE UPDATE ON channel_reply_targets
WHEN OLD.status <> 'ACTIVE'
  OR NEW.status = 'ACTIVE'
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.channel_binding_id IS NOT OLD.channel_binding_id
  OR NEW.host_epoch IS NOT OLD.host_epoch
  OR NEW.provider_message_ref IS NOT OLD.provider_message_ref
  OR NEW.delivery_id IS NOT OLD.delivery_id
  OR NEW.user_request_id IS NOT OLD.user_request_id
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.expires_at IS NOT OLD.expires_at
  OR (NEW.status = 'CONSUMED' AND NOT EXISTS (
    SELECT 1 FROM channel_event_receipts r
    JOIN user_request_responses ur
      ON ur.request_id = NEW.user_request_id
     AND ur.response_channel_binding_id = NEW.channel_binding_id
     AND ur.response_provider_event_id = r.provider_event_id
    WHERE r.channel_binding_id = NEW.channel_binding_id
      AND r.provider_event_id = NEW.consumed_by_provider_event_id
      AND r.state = 'PROCESSING'
  ))
BEGIN
  SELECT RAISE(ABORT, 'INVALID_CHANNEL_REPLY_TARGET_TRANSITION');
END;

CREATE TABLE skill_proposals (
  skill_proposal_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  source_task_id TEXT NOT NULL REFERENCES tasks(task_id),
  source_artifact_id TEXT NOT NULL REFERENCES artifacts(artifact_id),
  draft_resource_ref_json TEXT NOT NULL,
  draft_digest TEXT NOT NULL CHECK (length(draft_digest) = 71 AND substr(draft_digest, 1, 7) = 'sha256:' AND substr(draft_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  redaction_status TEXT NOT NULL CHECK (redaction_status IN ('PENDING', 'PASSED', 'FAILED')),
  status TEXT NOT NULL CHECK (status IN ('DRAFT', 'REVIEW', 'APPROVED', 'REJECTED', 'PUBLISHED')),
  approved_by_json TEXT,
  published_capability_ref_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(workspace_id, skill_proposal_id)
);

CREATE TABLE environment_control_leases (
  control_lease_id TEXT PRIMARY KEY,
  environment_id TEXT NOT NULL REFERENCES environments(environment_id),
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  runtime_incarnation_id TEXT NOT NULL,
  owner_kind TEXT NOT NULL CHECK (owner_kind IN ('AGENT', 'HUMAN')),
  owner_ref_json TEXT NOT NULL,
  epoch INTEGER NOT NULL,
  issuer_key_version INTEGER NOT NULL CHECK (issuer_key_version > 0),
  fencing_token_digest TEXT NOT NULL UNIQUE CHECK (length(fencing_token_digest) = 71 AND substr(fencing_token_digest, 1, 7) = 'sha256:' AND substr(fencing_token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  state TEXT NOT NULL CHECK (state IN ('ACTIVE', 'RELEASING', 'RELEASED', 'EXPIRED', 'REVOKED')),
  acquired_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE(environment_id, epoch),
  FOREIGN KEY(environment_id, runtime_id) REFERENCES environments(environment_id, runtime_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(task_id, attempt_id, environment_id, runtime_id, runtime_incarnation_id) REFERENCES attempts(task_id, attempt_id, environment_id, runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE UNIQUE INDEX uq_active_environment_control ON environment_control_leases(environment_id) WHERE state = 'ACTIVE';

CREATE TRIGGER environment_control_lease_current_incarnation_guard
BEFORE INSERT ON environment_control_leases
WHEN NEW.state = 'ACTIVE' AND NOT EXISTS (
  SELECT 1 FROM runtimes r
  WHERE r.runtime_id = NEW.runtime_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'CONTROL_LEASE_STALE_RUNTIME_INCARNATION');
END;

CREATE TRIGGER environment_control_lease_current_incarnation_update_guard
BEFORE UPDATE OF state, expires_at ON environment_control_leases
WHEN NEW.state = 'ACTIVE' AND NOT EXISTS (
  SELECT 1 FROM runtimes r
  WHERE r.runtime_id = NEW.runtime_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
)
BEGIN
  SELECT RAISE(ABORT, 'CONTROL_LEASE_STALE_RUNTIME_INCARNATION');
END;

CREATE TRIGGER environment_control_lease_identity_immutable
BEFORE UPDATE OF environment_id, task_id, attempt_id, runtime_id, runtime_incarnation_id ON environment_control_leases
WHEN OLD.environment_id <> NEW.environment_id
  OR OLD.task_id <> NEW.task_id
  OR OLD.attempt_id <> NEW.attempt_id
  OR OLD.runtime_id <> NEW.runtime_id
  OR OLD.runtime_incarnation_id <> NEW.runtime_incarnation_id
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_CONTROL_LEASE_IDENTITY');
END;

CREATE TRIGGER environment_control_lease_no_reactivation
BEFORE UPDATE OF state ON environment_control_leases
WHEN OLD.state <> 'ACTIVE' AND NEW.state = 'ACTIVE'
BEGIN
  SELECT RAISE(ABORT, 'CONTROL_LEASE_CANNOT_REACTIVATE');
END;

CREATE TRIGGER environment_control_lease_owner_epoch_guard
BEFORE UPDATE OF owner_kind, owner_ref_json, epoch, issuer_key_version, fencing_token_digest ON environment_control_leases
WHEN (
  OLD.owner_kind IS NOT NEW.owner_kind OR OLD.owner_ref_json IS NOT NEW.owner_ref_json
) AND (
  NEW.epoch <> OLD.epoch + 1 OR NEW.fencing_token_digest = OLD.fencing_token_digest
)
OR (
  OLD.owner_kind IS NEW.owner_kind AND OLD.owner_ref_json IS NEW.owner_ref_json
  AND (NEW.epoch <> OLD.epoch OR NEW.issuer_key_version <> OLD.issuer_key_version
       OR NEW.fencing_token_digest <> OLD.fencing_token_digest)
)
BEGIN
  SELECT RAISE(ABORT, 'CONTROL_LEASE_OWNER_CHANGE_REQUIRES_NEW_EPOCH');
END;

CREATE TABLE resource_upload_sessions (
  upload_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  display_name TEXT NOT NULL,
  media_type TEXT NOT NULL,
  expected_size_bytes INTEGER NOT NULL CHECK (expected_size_bytes >= 0),
  expected_digest TEXT CHECK (expected_digest IS NULL OR (length(expected_digest) = 71 AND substr(expected_digest, 1, 7) = 'sha256:' AND substr(expected_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  context_document_json TEXT CHECK (context_document_json IS NULL OR json_valid(context_document_json)),
  chunk_size_bytes INTEGER NOT NULL CHECK (chunk_size_bytes > 0),
  state TEXT NOT NULL CHECK (state IN ('OPEN', 'CONTENT_RECEIVED', 'COMMITTED', 'FAILED', 'EXPIRED')),
  expires_at TEXT NOT NULL,
  resource_id TEXT REFERENCES resources(resource_id),
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE resource_upload_chunks (
  upload_id TEXT NOT NULL REFERENCES resource_upload_sessions(upload_id),
  chunk_index INTEGER NOT NULL,
  start_offset INTEGER NOT NULL,
  end_offset_exclusive INTEGER NOT NULL CHECK (end_offset_exclusive > start_offset),
  sha256 TEXT NOT NULL CHECK (length(sha256) = 71 AND substr(sha256, 1, 7) = 'sha256:' AND substr(sha256, 8) NOT GLOB '*[^0-9a-f]*'),
  temporary_blob_ref TEXT NOT NULL,
  received_at TEXT NOT NULL,
  PRIMARY KEY(upload_id, chunk_index),
  UNIQUE(upload_id, start_offset, end_offset_exclusive)
);

CREATE TABLE aggregate_snapshots (
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  through_revision INTEGER NOT NULL,
  projection_schema_version INTEGER NOT NULL,
  state_digest TEXT NOT NULL CHECK (length(state_digest) = 71 AND substr(state_digest, 1, 7) = 'sha256:' AND substr(state_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  blob_ref_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id, entity_type, entity_id, through_revision)
);

CREATE TABLE event_archive_segments (
  segment_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  origin_runtime_id TEXT NOT NULL,
  first_sequence INTEGER NOT NULL,
  last_sequence INTEGER NOT NULL,
  content_digest TEXT NOT NULL CHECK (length(content_digest) = 71 AND substr(content_digest, 1, 7) = 'sha256:' AND substr(content_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  blob_ref_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(workspace_id, origin_runtime_id, first_sequence, last_sequence)
);

CREATE TABLE workspace_backup_manifests (
  backup_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  schema_version INTEGER NOT NULL,
  event_cursors_json TEXT NOT NULL,
  blob_manifest_ref_json TEXT NOT NULL,
  blob_manifest_digest TEXT NOT NULL CHECK (length(blob_manifest_digest) = 71 AND substr(blob_manifest_digest, 1, 7) = 'sha256:' AND substr(blob_manifest_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  database_snapshot_ref_json TEXT NOT NULL,
  encryption_key_ref TEXT NOT NULL,
  integrity_digest TEXT NOT NULL CHECK (length(integrity_digest) = 71 AND substr(integrity_digest, 1, 7) = 'sha256:' AND substr(integrity_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  manifest_authentication TEXT NOT NULL,
  created_at TEXT NOT NULL,
  verified_at TEXT NOT NULL
);

CREATE TRIGGER immutable_workspace_backup_manifest_update
BEFORE UPDATE ON workspace_backup_manifests
BEGIN
  SELECT RAISE(ABORT, 'IMMUTABLE_WORKSPACE_BACKUP_MANIFEST');
END;

CREATE TRIGGER environment_sharing_scope_change_guard
BEFORE UPDATE OF sharing_scope, owner_coworker_id, owner_principal_id ON environments
WHEN (NEW.sharing_scope IS NOT OLD.sharing_scope
   OR NEW.owner_coworker_id IS NOT OLD.owner_coworker_id
   OR NEW.owner_principal_id IS NOT OLD.owner_principal_id)
  AND NOT (
    OLD.lifetime = 'WORKSPACE_PERSISTENT'
    AND NEW.lifetime = 'WORKSPACE_PERSISTENT'
    AND OLD.status = 'SUSPENDED'
    AND NEW.status = 'SUSPENDED'
    AND OLD.owner_workspace_id = NEW.owner_workspace_id
    AND NEW.owner_task_id IS NULL
    AND NEW.owner_attempt_id IS NULL
    AND NEW.owner_principal_id IS NULL
    AND OLD.owner_principal_id IS NULL
    AND OLD.sharing_scope IN ('COWORKER_PRIVATE', 'WORKSPACE_SHARED')
    AND NEW.sharing_scope IN ('COWORKER_PRIVATE', 'WORKSPACE_SHARED')
    AND (
      (NEW.sharing_scope = 'WORKSPACE_SHARED' AND NEW.owner_coworker_id IS NULL)
      OR
      (NEW.sharing_scope = 'COWORKER_PRIVATE' AND NEW.owner_coworker_id IS NOT NULL
        AND EXISTS (
          SELECT 1 FROM coworkers c
          WHERE c.workspace_id = NEW.owner_workspace_id
            AND c.coworker_id = NEW.owner_coworker_id
            AND c.status IN ('ACTIVE', 'PAUSED')
        ))
    )
    AND NOT EXISTS (
      SELECT 1 FROM attempts a
      WHERE a.environment_id = OLD.environment_id
        AND a.status NOT IN ('COMPLETED', 'FAILED', 'ABANDONED', 'CANCELLED')
    )
    AND NOT EXISTS (
      SELECT 1 FROM environment_control_leases c
      WHERE c.environment_id = OLD.environment_id
        AND c.state IN ('ACTIVE', 'RELEASING')
    )
    AND NOT EXISTS (
      SELECT 1 FROM effects e
      JOIN attempts a ON a.task_id = e.task_id AND a.attempt_id = e.attempt_id
      WHERE a.environment_id = OLD.environment_id
        AND e.state IN ('PROPOSED', 'STARTED', 'ACKNOWLEDGED', 'RECONCILING', 'AMBIGUOUS')
    )
    AND NOT EXISTS (
      SELECT 1 FROM capability_invocations i
      JOIN attempts a ON a.attempt_id = i.attempt_id
      WHERE a.environment_id = OLD.environment_id
        AND i.status NOT IN ('SUCCEEDED', 'FAILED', 'CANCELLED')
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'ENVIRONMENT_SHARING_SCOPE_CHANGE_UNSAFE');
END;

CREATE TABLE handoffs (
  handoff_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(task_id),
  step_id TEXT NOT NULL REFERENCES steps(step_id),
  source_attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id),
  source_runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  target_runtime_id TEXT REFERENCES runtimes(runtime_id),
  phase TEXT NOT NULL CHECK (phase IN ('REQUESTED', 'DRAINING_SOURCE', 'CHECKPOINTING', 'REPLICATING', 'RECONCILING', 'LEASE_RELEASE', 'TARGET_PREPARE', 'TARGET_LEASE', 'TARGET_ATTEMPT', 'COMPLETED', 'FAILED')),
  resume_packet_ref_json TEXT,
  blockers_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1
);
