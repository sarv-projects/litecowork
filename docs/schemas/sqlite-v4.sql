-- Rebuild Runtime-owned tables for installation-scoped Runtime identity.
-- Applied by the migration runner with foreign_keys disabled before BEGIN IMMEDIATE.
-- The runner validates PRAGMA foreign_key_check inside the transaction and restores
-- foreign_keys before returning. v1-v3 source files remain immutable.

DROP TRIGGER attempt_environment_owner_guard;

CREATE TABLE runtimes_v4 (
  runtime_id TEXT PRIMARY KEY,
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
  FOREIGN KEY(runtime_id, current_incarnation_id) REFERENCES runtime_incarnations(runtime_id, runtime_incarnation_id) DEFERRABLE INITIALLY DEFERRED
);
INSERT INTO runtimes_v4 (
  runtime_id, device_identity_json, runtime_version, platform, architecture, roles_json,
  trust_zone, availability, startup_policy, current_incarnation_id,
  resource_capacity_json, last_seen, version
)
SELECT
  runtime_id, device_identity_json, runtime_version, platform, architecture, roles_json,
  trust_zone, availability, startup_policy, current_incarnation_id,
  resource_capacity_json, last_seen, version
FROM runtimes;
DROP TABLE runtimes;
ALTER TABLE runtimes_v4 RENAME TO runtimes;
CREATE INDEX idx_runtimes_availability ON runtimes(availability, runtime_id);

CREATE TABLE environments_v4 (
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
INSERT INTO environments_v4 SELECT * FROM environments;
DROP TABLE environments;
ALTER TABLE environments_v4 RENAME TO environments;

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

CREATE TRIGGER environment_requires_active_runtime_workspace_binding
BEFORE INSERT ON environments
WHEN NOT EXISTS (
  SELECT 1
  FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.runtime_id
    AND rwb.workspace_id = NEW.owner_workspace_id
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'EXECUTOR')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_EXECUTOR');
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

CREATE TABLE channel_host_assignments_v4 (
  channel_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL REFERENCES runtimes(runtime_id),
  host_epoch INTEGER NOT NULL CHECK (host_epoch > 0),
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'DRAINING')),
  ingress_continuity TEXT NOT NULL DEFAULT 'CONTINUOUS' CHECK (ingress_continuity IN ('CONTINUOUS', 'GAP_ACCEPTED')),
  ingress_gap_since TEXT,
  ingress_gap_decision_audit_id TEXT REFERENCES audit_records(audit_record_id),
  continuity_proof_ref TEXT REFERENCES channel_host_continuity_proofs(proof_id)
    DEFERRABLE INITIALLY DEFERRED,
  assigned_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  CHECK ((ingress_continuity = 'GAP_ACCEPTED') = (ingress_gap_since IS NOT NULL)),
  CHECK ((ingress_continuity = 'GAP_ACCEPTED') = (ingress_gap_decision_audit_id IS NOT NULL)),
  CHECK (ingress_continuity = 'GAP_ACCEPTED' OR ingress_gap_decision_audit_id IS NULL),
  CHECK (ingress_continuity = 'CONTINUOUS' OR continuity_proof_ref IS NULL),
  CHECK (ingress_gap_since IS NULL OR julianday(ingress_gap_since) IS NOT NULL),
  UNIQUE(channel_binding_id, host_epoch),
  UNIQUE(channel_binding_id, workspace_id, runtime_id, host_epoch),
  FOREIGN KEY(channel_binding_id, workspace_id) REFERENCES channel_bindings(channel_binding_id, workspace_id),
  FOREIGN KEY(channel_binding_id, workspace_id, runtime_id, host_epoch)
    REFERENCES channel_host_lease_records(channel_binding_id, workspace_id, runtime_id, host_epoch)
    DEFERRABLE INITIALLY DEFERRED
);
INSERT INTO channel_host_assignments_v4 (
  channel_binding_id, workspace_id, runtime_id, host_epoch, status, ingress_continuity,
  ingress_gap_since, ingress_gap_decision_audit_id, continuity_proof_ref, assigned_at, version
)
SELECT channel_binding_id, workspace_id, runtime_id, host_epoch, status, ingress_continuity,
  ingress_gap_since, NULL, NULL, assigned_at, version
FROM channel_host_assignments;
DROP TABLE channel_host_assignments;
ALTER TABLE channel_host_assignments_v4 RENAME TO channel_host_assignments;
CREATE INDEX idx_channel_host_runtime ON channel_host_assignments(runtime_id, status);

-- Pin the expiry-plus-clock-skew boundary at lease issuance/renewal. v1-v3 did not persist
-- a configurable margin, so the migration uses the v4 contract minimum for those leases;
-- new leases pin their configured margin. A release tombstone preserves the boundary.
CREATE TABLE channel_host_lease_records_v4 (
  channel_binding_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  host_epoch INTEGER NOT NULL CHECK (host_epoch > 0),
  lease_id TEXT NOT NULL,
  fencing_token_digest TEXT NOT NULL CHECK (length(fencing_token_digest) = 71 AND substr(fencing_token_digest, 1, 7) = 'sha256:' AND substr(fencing_token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  lease_expires_at TEXT NOT NULL,
  control_version INTEGER NOT NULL DEFAULT 1 CHECK (control_version >= 1),
  clock_skew_margin_ms INTEGER NOT NULL CHECK (clock_skew_margin_ms >= 30000),
  safe_reassign_after TEXT NOT NULL,
  UNIQUE(lease_id),
  UNIQUE(fencing_token_digest),
  UNIQUE(channel_binding_id, workspace_id, runtime_id, host_epoch),
  CHECK (julianday(lease_expires_at) IS NOT NULL),
  CHECK (julianday(safe_reassign_after) IS NOT NULL),
  CHECK (julianday(safe_reassign_after) >= julianday(lease_expires_at) + clock_skew_margin_ms / 86400000.0),
  FOREIGN KEY(channel_binding_id, workspace_id, runtime_id, host_epoch)
    REFERENCES channel_host_assignments(channel_binding_id, workspace_id, runtime_id, host_epoch)
    DEFERRABLE INITIALLY DEFERRED
);
INSERT INTO channel_host_lease_records_v4 (
  channel_binding_id, workspace_id, runtime_id, host_epoch, lease_id,
  fencing_token_digest, lease_expires_at, control_version, clock_skew_margin_ms,
  safe_reassign_after
)
SELECT channel_binding_id, workspace_id, runtime_id, host_epoch, lease_id,
  fencing_token_digest, lease_expires_at, control_version, 30000,
  strftime('%Y-%m-%dT%H:%M:%fZ', julianday(lease_expires_at) + 30.0 / 86400.0)
FROM channel_host_lease_records;
DROP TABLE channel_host_lease_records;
ALTER TABLE channel_host_lease_records_v4 RENAME TO channel_host_lease_records;
CREATE INDEX idx_channel_host_lease_expiry ON channel_host_lease_records(lease_expires_at);

CREATE TABLE channel_host_drain_proofs (
  proof_id TEXT PRIMARY KEY,
  channel_binding_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  host_epoch INTEGER NOT NULL CHECK (host_epoch > 0),
  lease_id TEXT NOT NULL,
  lease_control_version INTEGER NOT NULL CHECK (lease_control_version >= 1),
  proved_at TEXT NOT NULL CHECK (julianday(proved_at) IS NOT NULL),
  proof_digest TEXT NOT NULL CHECK (length(proof_digest) = 71 AND substr(proof_digest, 1, 7) = 'sha256:' AND substr(proof_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  verified_by_json TEXT NOT NULL CHECK (json_valid(verified_by_json)),
  unsettled_receipt_count INTEGER NOT NULL CHECK (unsettled_receipt_count = 0),
  unresolved_effect_count INTEGER NOT NULL CHECK (unresolved_effect_count = 0),
  unreplicated_receipt_count INTEGER NOT NULL CHECK (unreplicated_receipt_count = 0),
  UNIQUE(channel_binding_id, host_epoch),
  UNIQUE(proof_id, channel_binding_id, host_epoch)
);

CREATE TABLE channel_host_lease_releases (
  channel_binding_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  runtime_id TEXT NOT NULL,
  host_epoch INTEGER NOT NULL CHECK (host_epoch > 0),
  lease_id TEXT NOT NULL,
  control_version INTEGER NOT NULL CHECK (control_version >= 1),
  fencing_token_digest TEXT NOT NULL CHECK (length(fencing_token_digest) = 71 AND substr(fencing_token_digest, 1, 7) = 'sha256:' AND substr(fencing_token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  safe_reassign_after TEXT NOT NULL,
  released_at TEXT NOT NULL,
  release_kind TEXT NOT NULL CHECK (release_kind IN ('QUIESCENT', 'EXPIRY_PLUS_SKEW')),
  drain_proof_id TEXT,
  PRIMARY KEY(channel_binding_id, host_epoch),
  UNIQUE(lease_id),
  UNIQUE(fencing_token_digest),
  CHECK ((release_kind = 'QUIESCENT') = (drain_proof_id IS NOT NULL)),
  FOREIGN KEY(drain_proof_id, channel_binding_id, host_epoch)
    REFERENCES channel_host_drain_proofs(proof_id, channel_binding_id, host_epoch)
);

CREATE TABLE channel_host_continuity_proofs (
  proof_id TEXT PRIMARY KEY,
  channel_binding_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  source_runtime_id TEXT NOT NULL,
  source_host_epoch INTEGER NOT NULL CHECK (source_host_epoch > 0),
  source_lease_id TEXT NOT NULL,
  source_lease_control_version INTEGER NOT NULL CHECK (source_lease_control_version >= 1),
  target_runtime_id TEXT NOT NULL,
  target_host_epoch INTEGER NOT NULL CHECK (target_host_epoch = source_host_epoch + 1),
  last_replicated_receipt_ref_json TEXT NOT NULL CHECK (json_valid(last_replicated_receipt_ref_json)),
  source_cursor_digest TEXT NOT NULL CHECK (length(source_cursor_digest) = 71 AND substr(source_cursor_digest, 1, 7) = 'sha256:' AND substr(source_cursor_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  target_cursor_digest TEXT NOT NULL CHECK (length(target_cursor_digest) = 71 AND substr(target_cursor_digest, 1, 7) = 'sha256:' AND substr(target_cursor_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  verified_at TEXT NOT NULL CHECK (julianday(verified_at) IS NOT NULL),
  proof_digest TEXT NOT NULL CHECK (length(proof_digest) = 71 AND substr(proof_digest, 1, 7) = 'sha256:' AND substr(proof_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  verified_by_json TEXT NOT NULL CHECK (json_valid(verified_by_json)),
  UNIQUE(channel_binding_id, target_host_epoch),
  UNIQUE(proof_id, channel_binding_id, workspace_id, target_runtime_id, target_host_epoch)
);

CREATE TRIGGER channel_host_continuity_proof_admission
BEFORE INSERT ON channel_host_continuity_proofs
WHEN NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings target_binding
  WHERE target_binding.runtime_id = NEW.target_runtime_id
    AND target_binding.workspace_id = NEW.workspace_id
    AND target_binding.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(target_binding.roles_json) role WHERE role.value = 'CHANNEL_HOST')
    AND NEW.target_host_epoch = NEW.source_host_epoch + 1
    AND NEW.target_runtime_id <> NEW.source_runtime_id
    AND julianday(NEW.verified_at) <= julianday('now')
    AND (
      EXISTS (
        SELECT 1 FROM channel_host_assignments a
        JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
          AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
        WHERE a.channel_binding_id = NEW.channel_binding_id AND a.workspace_id = NEW.workspace_id
          AND a.runtime_id = NEW.source_runtime_id AND a.host_epoch = NEW.source_host_epoch
          AND a.status = 'DRAINING' AND l.lease_id = NEW.source_lease_id
          AND l.control_version = NEW.source_lease_control_version
          AND julianday(l.lease_expires_at) > julianday('now')
      )
      OR EXISTS (
        SELECT 1 FROM channel_host_lease_releases r
        WHERE r.channel_binding_id = NEW.channel_binding_id AND r.workspace_id = NEW.workspace_id
          AND r.runtime_id = NEW.source_runtime_id AND r.host_epoch = NEW.source_host_epoch
          AND r.lease_id = NEW.source_lease_id
          AND r.control_version = NEW.source_lease_control_version
          AND (r.release_kind = 'QUIESCENT'
            OR julianday(r.released_at) >= julianday(r.safe_reassign_after))
          AND (
            EXISTS (
              SELECT 1 FROM channel_host_assignments a
              WHERE a.channel_binding_id = NEW.channel_binding_id
                AND a.workspace_id = NEW.workspace_id AND a.runtime_id = NEW.source_runtime_id
                AND a.host_epoch = NEW.source_host_epoch AND a.status = 'DRAINING'
            )
            OR NOT EXISTS (
              SELECT 1 FROM channel_host_assignments a
              WHERE a.channel_binding_id = NEW.channel_binding_id
            )
          )
      )
    )
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_CONTINUITY_PROOF_PRECONDITION_FAILED');
END;

CREATE TRIGGER channel_host_continuity_proof_immutable
BEFORE UPDATE ON channel_host_continuity_proofs
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_CONTINUITY_PROOF_IMMUTABLE');
END;

CREATE TRIGGER channel_host_continuity_proof_no_delete
BEFORE DELETE ON channel_host_continuity_proofs
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_CONTINUITY_PROOF_IMMUTABLE');
END;

CREATE TRIGGER channel_host_drain_proof_admission
BEFORE INSERT ON channel_host_drain_proofs
WHEN NOT EXISTS (
  SELECT 1 FROM channel_host_assignments a
  JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
    AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
  JOIN runtime_workspace_bindings source_binding
    ON source_binding.runtime_id = a.runtime_id AND source_binding.workspace_id = a.workspace_id
  WHERE a.channel_binding_id = NEW.channel_binding_id AND a.workspace_id = NEW.workspace_id
    AND a.runtime_id = NEW.runtime_id AND a.host_epoch = NEW.host_epoch AND a.status = 'DRAINING'
    AND source_binding.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(source_binding.roles_json) role WHERE role.value = 'CHANNEL_HOST')
    AND l.lease_id = NEW.lease_id AND l.control_version = NEW.lease_control_version
    AND julianday(l.lease_expires_at) IS NOT NULL
    AND julianday(l.lease_expires_at) > julianday('now')
    AND julianday(NEW.proved_at) <= julianday('now')
    AND NOT EXISTS (
      SELECT 1 FROM channel_event_receipts r
      WHERE r.channel_binding_id = a.channel_binding_id AND r.state = 'PROCESSING'
        AND r.claim_runtime_id = a.runtime_id AND r.claim_host_epoch = a.host_epoch
    )
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_DRAIN_PROOF_PRECONDITION_FAILED');
END;

CREATE TRIGGER channel_host_drain_proof_immutable
BEFORE UPDATE ON channel_host_drain_proofs
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_DRAIN_PROOF_IMMUTABLE');
END;

CREATE TRIGGER channel_host_drain_proof_no_delete
BEFORE DELETE ON channel_host_drain_proofs
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_DRAIN_PROOF_IMMUTABLE');
END;

CREATE TRIGGER channel_host_lease_release_immutable
BEFORE UPDATE ON channel_host_lease_releases
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_LEASE_RELEASE_IMMUTABLE');
END;

CREATE TRIGGER channel_host_lease_release_no_delete
BEFORE DELETE ON channel_host_lease_releases
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_LEASE_RELEASE_IMMUTABLE');
END;

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

CREATE TRIGGER channel_host_assignment_requires_active_binding_insert
BEFORE INSERT ON channel_host_assignments
WHEN NEW.status <> 'ACTIVE' OR NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'CHANNEL_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_CHANNEL_HOST');
END;

CREATE TRIGGER channel_host_assignment_epoch_insert_monotonic
BEFORE INSERT ON channel_host_assignments
WHEN NEW.host_epoch <> COALESCE((
  SELECT MAX(r.host_epoch) FROM channel_host_lease_releases r
  WHERE r.channel_binding_id = NEW.channel_binding_id
), 0) + 1
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_ASSIGNMENT_EPOCH_STALE');
END;

CREATE TRIGGER channel_host_assignment_transition_guard
BEFORE UPDATE ON channel_host_assignments
WHEN NOT (
  (OLD.status = 'ACTIVE' AND NEW.status = 'DRAINING'
    AND NEW.runtime_id = OLD.runtime_id AND NEW.host_epoch = OLD.host_epoch)
  OR ((OLD.status = 'DRAINING' AND NEW.status = 'ACTIVE'
    AND NEW.runtime_id = OLD.runtime_id AND NEW.host_epoch = OLD.host_epoch
    AND EXISTS (
      SELECT 1 FROM channel_host_lease_records l
      WHERE l.channel_binding_id = OLD.channel_binding_id
        AND l.workspace_id = OLD.workspace_id AND l.runtime_id = OLD.runtime_id
        AND l.host_epoch = OLD.host_epoch
        AND julianday(l.lease_expires_at) > julianday('now')
    ))
    AND NOT EXISTS (
      SELECT 1 FROM channel_host_drain_proofs p
      WHERE p.channel_binding_id = OLD.channel_binding_id
        AND p.workspace_id = OLD.workspace_id AND p.runtime_id = OLD.runtime_id
        AND p.host_epoch = OLD.host_epoch
    ))
  OR ((OLD.status = 'DRAINING' AND NEW.status = 'ACTIVE'
    AND NEW.runtime_id <> OLD.runtime_id AND NEW.host_epoch = OLD.host_epoch + 1
    AND NOT EXISTS (
      SELECT 1 FROM channel_host_lease_records l
      WHERE l.channel_binding_id = OLD.channel_binding_id
    ))
    AND EXISTS (
      SELECT 1 FROM channel_host_lease_releases r
      WHERE r.channel_binding_id = OLD.channel_binding_id
        AND r.workspace_id = OLD.workspace_id AND r.runtime_id = OLD.runtime_id
        AND r.host_epoch = OLD.host_epoch
        AND julianday(r.released_at) IS NOT NULL
        AND julianday(r.released_at) <= julianday('now')
        AND (r.release_kind = 'QUIESCENT' OR (
          r.release_kind = 'EXPIRY_PLUS_SKEW'
          AND julianday(r.released_at) >= julianday(r.safe_reassign_after)
        ))
    ))
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_ASSIGNMENT_TRANSITION_INVALID');
END;

CREATE TRIGGER channel_host_assignment_continuity_provenance_guard
BEFORE UPDATE ON channel_host_assignments
WHEN (
  NEW.host_epoch = OLD.host_epoch AND (
    NEW.ingress_continuity IS NOT OLD.ingress_continuity
    OR NEW.ingress_gap_since IS NOT OLD.ingress_gap_since
    OR NEW.ingress_gap_decision_audit_id IS NOT OLD.ingress_gap_decision_audit_id
    OR NEW.continuity_proof_ref IS NOT OLD.continuity_proof_ref
  )
) OR (
  NEW.host_epoch = OLD.host_epoch + 1 AND (
    (NEW.ingress_continuity = 'GAP_ACCEPTED' AND NOT EXISTS (
      SELECT 1 FROM audit_records ar
      WHERE ar.audit_record_id = NEW.ingress_gap_decision_audit_id
        AND ar.workspace_id = NEW.workspace_id AND ar.decision = 'ALLOW'
        AND ar.action = 'channel.host.ingress_gap.accept'
        AND json_extract(ar.principal_json, '$.principal_id') = (
          SELECT w.owner_principal_id FROM workspaces w WHERE w.workspace_id = NEW.workspace_id
        )
        AND json_extract(ar.resource_ref_json, '$.channel_binding_id') = NEW.channel_binding_id
        AND json_extract(ar.resource_ref_json, '$.target_runtime_id') = NEW.runtime_id
        AND json_extract(ar.resource_ref_json, '$.target_host_epoch') = NEW.host_epoch
    ))
    OR (NEW.ingress_continuity = 'CONTINUOUS' AND NOT EXISTS (
      SELECT 1 FROM channel_host_continuity_proofs p
      WHERE p.proof_id = NEW.continuity_proof_ref
        AND p.channel_binding_id = NEW.channel_binding_id AND p.workspace_id = NEW.workspace_id
        AND p.source_runtime_id = OLD.runtime_id AND p.source_host_epoch = OLD.host_epoch
        AND p.target_runtime_id = NEW.runtime_id AND p.target_host_epoch = NEW.host_epoch
    ))
  )
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_CONTINUITY_PROVENANCE_INVALID');
END;

CREATE TRIGGER channel_host_assignment_insert_continuity_provenance
BEFORE INSERT ON channel_host_assignments
WHEN (NEW.ingress_continuity = 'GAP_ACCEPTED' AND NOT EXISTS (
    SELECT 1 FROM audit_records ar
    WHERE ar.audit_record_id = NEW.ingress_gap_decision_audit_id
      AND ar.workspace_id = NEW.workspace_id AND ar.decision = 'ALLOW'
      AND ar.action = 'channel.host.ingress_gap.accept'
      AND json_extract(ar.principal_json, '$.principal_id') = (
        SELECT w.owner_principal_id FROM workspaces w WHERE w.workspace_id = NEW.workspace_id
      )
      AND json_extract(ar.resource_ref_json, '$.channel_binding_id') = NEW.channel_binding_id
      AND json_extract(ar.resource_ref_json, '$.target_runtime_id') = NEW.runtime_id
      AND json_extract(ar.resource_ref_json, '$.target_host_epoch') = NEW.host_epoch
  ))
  OR (NEW.host_epoch > 1 AND NEW.ingress_continuity = 'CONTINUOUS' AND NOT EXISTS (
    SELECT 1 FROM channel_host_continuity_proofs p
    WHERE p.proof_id = NEW.continuity_proof_ref
      AND p.channel_binding_id = NEW.channel_binding_id AND p.workspace_id = NEW.workspace_id
      AND p.target_runtime_id = NEW.runtime_id AND p.target_host_epoch = NEW.host_epoch
  ))
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_CONTINUITY_PROVENANCE_INVALID');
END;

CREATE TRIGGER channel_host_assignment_assigned_at_guard
BEFORE UPDATE ON channel_host_assignments
WHEN NEW.assigned_at IS NOT OLD.assigned_at
  AND NOT (OLD.status = 'DRAINING' AND NEW.status = 'ACTIVE'
    AND NEW.runtime_id <> OLD.runtime_id AND NEW.host_epoch = OLD.host_epoch + 1)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_ASSIGNMENT_TIMESTAMP_IMMUTABLE');
END;

CREATE TRIGGER channel_host_assignment_requires_active_binding_update
BEFORE UPDATE ON channel_host_assignments
WHEN (NEW.runtime_id IS NOT OLD.runtime_id
   OR NEW.workspace_id IS NOT OLD.workspace_id
   OR NEW.status = 'ACTIVE')
  AND NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'CHANNEL_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_CHANNEL_HOST');
END;

CREATE TRIGGER channel_host_assignment_delete_after_release
BEFORE DELETE ON channel_host_assignments
WHEN OLD.status <> 'DRAINING'
  OR EXISTS (
    SELECT 1 FROM channel_host_lease_records l
    WHERE l.channel_binding_id = OLD.channel_binding_id
  )
  OR NOT EXISTS (
    SELECT 1 FROM channel_host_lease_releases r
    WHERE r.channel_binding_id = OLD.channel_binding_id
      AND r.workspace_id = OLD.workspace_id AND r.runtime_id = OLD.runtime_id
      AND r.host_epoch = OLD.host_epoch
      AND julianday(r.released_at) IS NOT NULL
      AND julianday(r.released_at) <= julianday('now')
      AND (r.release_kind = 'QUIESCENT' OR
        julianday(r.released_at) >= julianday(r.safe_reassign_after))
  )
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_ASSIGNMENT_DELETE_REQUIRES_SAFE_RELEASE');
END;

CREATE TRIGGER channel_host_lease_requires_active_binding_insert
BEFORE INSERT ON channel_host_lease_records
WHEN NEW.control_version <> 1
  OR NEW.clock_skew_margin_ms < 30000
  OR julianday(NEW.safe_reassign_after) IS NULL
  OR julianday(NEW.safe_reassign_after) < julianday(NEW.lease_expires_at) + NEW.clock_skew_margin_ms / 86400000.0
  OR julianday(NEW.lease_expires_at) IS NULL
  OR julianday(NEW.lease_expires_at) <= julianday('now')
  OR EXISTS (
    SELECT 1 FROM channel_host_lease_releases r
    WHERE r.lease_id = NEW.lease_id OR r.fencing_token_digest = NEW.fencing_token_digest
  )
  OR NOT EXISTS (
  SELECT 1 FROM channel_host_assignments a
  JOIN runtime_workspace_bindings rwb
    ON rwb.runtime_id = a.runtime_id AND rwb.workspace_id = a.workspace_id
  WHERE a.channel_binding_id = NEW.channel_binding_id
    AND a.workspace_id = NEW.workspace_id AND a.runtime_id = NEW.runtime_id
    AND a.host_epoch = NEW.host_epoch AND a.status = 'ACTIVE'
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'CHANNEL_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_CHANNEL_HOST');
END;

CREATE TRIGGER channel_host_lease_requires_active_binding_update
BEFORE UPDATE ON channel_host_lease_records
WHEN NEW.channel_binding_id IS NOT OLD.channel_binding_id
  OR NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.runtime_id IS NOT OLD.runtime_id
  OR NEW.host_epoch IS NOT OLD.host_epoch
  OR NEW.lease_id IS NOT OLD.lease_id
  OR NEW.fencing_token_digest IS NOT OLD.fencing_token_digest
  OR NEW.clock_skew_margin_ms <> OLD.clock_skew_margin_ms
  OR NEW.control_version <> OLD.control_version + 1
  OR julianday(NEW.lease_expires_at) IS NULL
  OR julianday(OLD.lease_expires_at) IS NULL
  OR julianday(NEW.safe_reassign_after) IS NULL
  OR julianday(NEW.safe_reassign_after) < julianday(NEW.lease_expires_at) + NEW.clock_skew_margin_ms / 86400000.0
  OR julianday(NEW.lease_expires_at) <= julianday(OLD.lease_expires_at)
  OR julianday(OLD.lease_expires_at) <= julianday('now')
  OR NOT EXISTS (
    SELECT 1 FROM channel_host_assignments a
    JOIN runtime_workspace_bindings rwb
      ON rwb.runtime_id = a.runtime_id AND rwb.workspace_id = a.workspace_id
    WHERE a.channel_binding_id = NEW.channel_binding_id
      AND a.workspace_id = NEW.workspace_id AND a.runtime_id = NEW.runtime_id
      AND a.host_epoch = NEW.host_epoch AND a.status = 'ACTIVE'
      AND rwb.status = 'ACTIVE'
      AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'CHANNEL_HOST')
  )
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_LEASE_RENEWAL_INVALID');
END;

CREATE TRIGGER channel_host_lease_delete_after_drain_or_expiry
BEFORE DELETE ON channel_host_lease_records
WHEN NOT EXISTS (
    SELECT 1 FROM channel_host_assignments a
    WHERE a.channel_binding_id = OLD.channel_binding_id
      AND a.workspace_id = OLD.workspace_id AND a.runtime_id = OLD.runtime_id
      AND a.host_epoch = OLD.host_epoch AND a.status = 'DRAINING'
  )
  OR julianday(OLD.lease_expires_at) IS NULL
  OR julianday(OLD.safe_reassign_after) IS NULL
  OR NOT (
    julianday('now') >= julianday(OLD.safe_reassign_after)
    OR EXISTS (
      SELECT 1 FROM channel_host_assignments a
      JOIN channel_host_drain_proofs p ON p.channel_binding_id = a.channel_binding_id
        AND p.workspace_id = a.workspace_id AND p.runtime_id = a.runtime_id
        AND p.host_epoch = a.host_epoch
      WHERE a.channel_binding_id = OLD.channel_binding_id
        AND a.workspace_id = OLD.workspace_id AND a.runtime_id = OLD.runtime_id
        AND a.host_epoch = OLD.host_epoch AND a.status = 'DRAINING'
        AND p.lease_id = OLD.lease_id AND p.lease_control_version = OLD.control_version
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_LEASE_RELEASE_REQUIRES_DRAIN_PROOF_OR_EXPIRY_PLUS_SKEW');
END;

CREATE TRIGGER channel_host_lease_release_record_on_delete
BEFORE DELETE ON channel_host_lease_records
BEGIN
  INSERT INTO channel_host_lease_releases (
    channel_binding_id, workspace_id, runtime_id, host_epoch, lease_id,
    control_version, fencing_token_digest, safe_reassign_after, released_at, release_kind, drain_proof_id
  )
  SELECT OLD.channel_binding_id, OLD.workspace_id, OLD.runtime_id, OLD.host_epoch, OLD.lease_id,
    OLD.control_version, OLD.fencing_token_digest, OLD.safe_reassign_after, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    CASE WHEN EXISTS (
      SELECT 1 FROM channel_host_drain_proofs p
      JOIN channel_host_assignments a ON a.channel_binding_id = p.channel_binding_id
        AND a.workspace_id = p.workspace_id AND a.runtime_id = p.runtime_id
        AND a.host_epoch = p.host_epoch
      WHERE p.channel_binding_id = OLD.channel_binding_id
        AND p.workspace_id = OLD.workspace_id AND p.runtime_id = OLD.runtime_id
        AND p.host_epoch = OLD.host_epoch AND p.lease_id = OLD.lease_id
        AND p.lease_control_version = OLD.control_version AND a.status = 'DRAINING'
    ) THEN 'QUIESCENT' ELSE 'EXPIRY_PLUS_SKEW' END,
    (SELECT p.proof_id FROM channel_host_drain_proofs p
      WHERE p.channel_binding_id = OLD.channel_binding_id AND p.host_epoch = OLD.host_epoch
        AND p.lease_id = OLD.lease_id AND p.lease_control_version = OLD.control_version)
  WHERE NOT EXISTS (
    SELECT 1 FROM channel_host_lease_releases r
    WHERE r.channel_binding_id = OLD.channel_binding_id AND r.host_epoch = OLD.host_epoch
  );
END;

CREATE TRIGGER channel_host_lease_release_record_admission
BEFORE INSERT ON channel_host_lease_releases
WHEN NOT EXISTS (
  SELECT 1 FROM channel_host_assignments a
  JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
    AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
  WHERE a.channel_binding_id = NEW.channel_binding_id AND a.workspace_id = NEW.workspace_id
    AND a.runtime_id = NEW.runtime_id AND a.host_epoch = NEW.host_epoch AND a.status = 'DRAINING'
    AND l.lease_id = NEW.lease_id AND l.control_version = NEW.control_version
    AND l.fencing_token_digest = NEW.fencing_token_digest
    AND l.safe_reassign_after = NEW.safe_reassign_after
    AND (
      (NEW.release_kind = 'QUIESCENT' AND EXISTS (
        SELECT 1 FROM channel_host_drain_proofs p
        WHERE p.proof_id = NEW.drain_proof_id
          AND p.channel_binding_id = NEW.channel_binding_id AND p.workspace_id = NEW.workspace_id
          AND p.runtime_id = NEW.runtime_id AND p.host_epoch = NEW.host_epoch
          AND p.lease_id = NEW.lease_id AND p.lease_control_version = NEW.control_version
      ))
      OR (NEW.release_kind = 'EXPIRY_PLUS_SKEW'
        AND julianday(NEW.released_at) >= julianday(NEW.safe_reassign_after))
    )
)
BEGIN
  SELECT RAISE(ABORT, 'CHANNEL_HOST_LEASE_RELEASE_RECORD_INVALID');
END;

DROP TRIGGER channel_receipt_transition_guard;
CREATE TRIGGER channel_receipt_transition_guard
BEFORE UPDATE ON channel_event_receipts
WHEN (OLD.claim_expires_at IS NOT NULL AND julianday(OLD.claim_expires_at) IS NULL)
  OR (NEW.claim_expires_at IS NOT NULL AND julianday(NEW.claim_expires_at) IS NULL)
  OR NOT (
  (OLD.state = 'RECEIVED' AND NEW.state = 'PROCESSING'
    AND NEW.claim_epoch = 1
    AND julianday(NEW.claim_expires_at) IS NOT NULL
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
    AND julianday(NEW.claim_expires_at) IS NOT NULL
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
    AND julianday(OLD.claim_expires_at) IS NOT NULL
    AND julianday(OLD.claim_expires_at) > julianday('now')
    AND NEW.claim_runtime_id = OLD.claim_runtime_id
    AND NEW.claim_host_epoch = OLD.claim_host_epoch
    AND NEW.claim_expires_at IS NULL
    AND EXISTS (
      SELECT 1 FROM channel_host_assignments a
      JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
        AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id AND l.host_epoch = a.host_epoch
      WHERE a.channel_binding_id = NEW.channel_binding_id AND a.runtime_id = NEW.claim_runtime_id
        AND a.host_epoch = NEW.claim_host_epoch AND a.status IN ('ACTIVE', 'DRAINING')
        AND julianday(l.lease_expires_at) > julianday('now')
    ))
)
BEGIN
  SELECT RAISE(ABORT, 'INVALID_CHANNEL_RECEIPT_TRANSITION');
END;

CREATE TRIGGER channel_receipt_requires_active_binding
BEFORE UPDATE ON channel_event_receipts
WHEN NEW.state = 'PROCESSING' AND NOT EXISTS (
  SELECT 1 FROM channel_host_assignments a
  JOIN runtime_workspace_bindings rwb
    ON rwb.runtime_id = a.runtime_id AND rwb.workspace_id = a.workspace_id
  WHERE a.channel_binding_id = NEW.channel_binding_id
    AND a.runtime_id = NEW.claim_runtime_id AND a.host_epoch = NEW.claim_host_epoch
    AND a.status = 'ACTIVE' AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'CHANNEL_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_CHANNEL_HOST');
END;

CREATE TABLE automation_occurrences_v4 (
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
  claim_epoch INTEGER NOT NULL DEFAULT 0 CHECK (claim_epoch >= 0),
  claim_expires_at TEXT,
  task_id TEXT REFERENCES tasks(task_id),
  status TEXT NOT NULL CHECK (status IN ('PENDING', 'CLAIMED', 'WAITING_DEPENDENCY', 'STARTED', 'COMPLETED', 'SKIPPED', 'FAILED')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(automation_id, occurrence_id),
  UNIQUE(automation_id, occurrence_id, workspace_id),
  FOREIGN KEY(task_id, workspace_id) REFERENCES tasks(task_id, workspace_id),
  UNIQUE(automation_id, trigger_id, occurrence_key),
  FOREIGN KEY(automation_id, automation_revision, workspace_id) REFERENCES automation_revisions(automation_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(routine_id, routine_revision, workspace_id) REFERENCES routine_revisions(routine_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED
);
INSERT INTO automation_occurrences_v4 SELECT * FROM automation_occurrences;
DROP TABLE automation_occurrences;
ALTER TABLE automation_occurrences_v4 RENAME TO automation_occurrences;
CREATE INDEX idx_automation_occurrences_status ON automation_occurrences(automation_id, status, created_at);
CREATE INDEX idx_automation_occurrences_claim ON automation_occurrences(status, claim_expires_at);
CREATE UNIQUE INDEX uq_task_automation_occurrence
  ON tasks(automation_id, automation_occurrence_id)
  WHERE automation_occurrence_id IS NOT NULL;
CREATE UNIQUE INDEX uq_automation_occurrence_task ON automation_occurrences(task_id)
  WHERE task_id IS NOT NULL;

CREATE TRIGGER automation_occurrence_insert_starts_unmaterialized
BEFORE INSERT ON automation_occurrences
WHEN NEW.status <> 'PENDING' OR NEW.task_id IS NOT NULL
  OR NEW.claim_epoch <> 0 OR NEW.claim_expires_at IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_MUST_START_PENDING');
END;

CREATE TRIGGER automation_occurrence_identity_immutable
BEFORE UPDATE ON automation_occurrences
WHEN NEW.workspace_id IS NOT OLD.workspace_id
  OR NEW.occurrence_id IS NOT OLD.occurrence_id
  OR NEW.automation_id IS NOT OLD.automation_id
  OR NEW.automation_revision IS NOT OLD.automation_revision
  OR NEW.routine_id IS NOT OLD.routine_id
  OR NEW.routine_revision IS NOT OLD.routine_revision
  OR NEW.trigger_id IS NOT OLD.trigger_id
  OR NEW.trigger_host_runtime_id IS NOT OLD.trigger_host_runtime_id
  OR NEW.occurrence_key IS NOT OLD.occurrence_key
  OR NEW.scheduled_for IS NOT OLD.scheduled_for
  OR NEW.covered_misfire_range_json IS NOT OLD.covered_misfire_range_json
  OR NEW.trigger_input_ref_json IS NOT OLD.trigger_input_ref_json
  OR NEW.trigger_payload_digest IS NOT OLD.trigger_payload_digest
  OR NEW.created_at IS NOT OLD.created_at
  OR (OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id)
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_IDENTITY_IMMUTABLE');
END;

CREATE TRIGGER automation_occurrence_task_link_matches_origin
BEFORE UPDATE OF task_id ON automation_occurrences
WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks t
  WHERE t.task_id = NEW.task_id AND t.workspace_id = NEW.workspace_id
    AND t.automation_id = NEW.automation_id
    AND t.automation_occurrence_id = NEW.occurrence_id
)
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_TASK_LINK_MISMATCH');
END;

CREATE TRIGGER automation_occurrence_status_transition_guard
BEFORE UPDATE OF status ON automation_occurrences
WHEN NEW.status IS NOT OLD.status
  AND NOT (
    (OLD.status = 'PENDING' AND NEW.status IN ('CLAIMED', 'SKIPPED'))
    OR (OLD.status = 'CLAIMED' AND NEW.status IN ('PENDING', 'STARTED', 'WAITING_DEPENDENCY'))
    OR (OLD.status = 'WAITING_DEPENDENCY' AND NEW.status IN ('STARTED', 'SKIPPED', 'FAILED'))
    OR (OLD.status = 'STARTED' AND NEW.status IN ('COMPLETED', 'SKIPPED', 'FAILED'))
  )
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_STATUS_TRANSITION_INVALID');
END;

CREATE TRIGGER automation_occurrence_claim_fencing_guard
BEFORE UPDATE ON automation_occurrences
WHEN (OLD.claim_expires_at IS NOT NULL AND julianday(OLD.claim_expires_at) IS NULL)
  OR (NEW.claim_expires_at IS NOT NULL AND julianday(NEW.claim_expires_at) IS NULL)
  OR NEW.claim_epoch < OLD.claim_epoch
  OR NEW.claim_epoch > OLD.claim_epoch + 1
  OR (NEW.claim_epoch <> OLD.claim_epoch AND NOT (OLD.status = 'PENDING' AND NEW.status = 'CLAIMED'))
  OR (NEW.status = 'CLAIMED' AND OLD.status <> 'CLAIMED' AND (
    NEW.claim_epoch <> OLD.claim_epoch + 1
    OR NEW.claim_expires_at IS NULL
    OR julianday(NEW.claim_expires_at) IS NULL
    OR julianday(NEW.claim_expires_at) <= julianday('now')
  ))
  OR (OLD.status = 'CLAIMED' AND NEW.status = 'PENDING' AND (
    OLD.claim_expires_at IS NULL
    OR julianday(OLD.claim_expires_at) IS NULL
    OR julianday(OLD.claim_expires_at) > julianday('now')
    OR NEW.claim_epoch <> OLD.claim_epoch
    OR NEW.claim_expires_at IS NOT OLD.claim_expires_at
  ))
  OR (OLD.status = 'CLAIMED' AND NEW.status IN ('STARTED', 'WAITING_DEPENDENCY') AND (
    OLD.claim_expires_at IS NULL
    OR julianday(OLD.claim_expires_at) IS NULL
    OR julianday(OLD.claim_expires_at) <= julianday('now')
    OR NEW.claim_epoch <> OLD.claim_epoch
    OR NEW.claim_expires_at IS NOT OLD.claim_expires_at
  ))
  OR (NEW.status = OLD.status AND (
    NEW.claim_epoch <> OLD.claim_epoch
    OR NEW.claim_expires_at IS NOT OLD.claim_expires_at
  ))
  OR (NEW.status <> OLD.status
      AND NOT (OLD.status = 'PENDING' AND NEW.status = 'CLAIMED')
      AND NOT (OLD.status = 'CLAIMED' AND NEW.status IN ('PENDING', 'STARTED', 'WAITING_DEPENDENCY'))
      AND (NEW.claim_epoch <> OLD.claim_epoch OR NEW.claim_expires_at IS NOT OLD.claim_expires_at))
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_CLAIM_FENCE_INVALID');
END;

CREATE TRIGGER automation_occurrence_materialized_state_requires_task
BEFORE UPDATE OF status ON automation_occurrences
WHEN NEW.status IN ('STARTED', 'WAITING_DEPENDENCY', 'COMPLETED', 'FAILED')
  AND NEW.task_id IS NULL
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_MATERIALIZED_TASK_REQUIRED');
END;

CREATE TRIGGER automation_occurrence_skipped_task_consistency
BEFORE UPDATE OF status ON automation_occurrences
WHEN NEW.status = 'SKIPPED' AND OLD.status <> 'PENDING' AND NEW.task_id IS NULL
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_MATERIALIZED_TASK_REQUIRED');
END;

CREATE TRIGGER automation_occurrence_terminal_immutable
BEFORE UPDATE ON automation_occurrences
WHEN OLD.status IN ('COMPLETED', 'SKIPPED', 'FAILED')
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_TERMINAL_IMMUTABLE');
END;

CREATE TRIGGER automation_occurrence_task_link_admission_guard
BEFORE UPDATE OF task_id ON automation_occurrences
WHEN OLD.task_id IS NULL AND NEW.task_id IS NOT NULL
  AND NOT (OLD.status = 'CLAIMED' AND NEW.status IN ('STARTED', 'WAITING_DEPENDENCY'))
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_TASK_LINK_ADMISSION_INVALID');
END;

CREATE TRIGGER automation_occurrence_requires_active_trigger_host_insert
BEFORE INSERT ON automation_occurrences
WHEN NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.trigger_host_runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'TRIGGER_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_TRIGGER_HOST');
END;

CREATE TRIGGER automation_occurrence_requires_active_trigger_host_update
BEFORE UPDATE ON automation_occurrences
WHEN NEW.status NOT IN ('COMPLETED', 'SKIPPED', 'FAILED')
  AND NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.trigger_host_runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'TRIGGER_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_TRIGGER_HOST');
END;

CREATE TABLE automation_cursors_v4 (
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
  FOREIGN KEY(automation_id, active_automation_revision, workspace_id) REFERENCES automation_revisions(automation_id, revision, workspace_id) DEFERRABLE INITIALLY DEFERRED
);
INSERT INTO automation_cursors_v4 SELECT * FROM automation_cursors;
DROP TABLE automation_cursors;
ALTER TABLE automation_cursors_v4 RENAME TO automation_cursors;

CREATE TRIGGER automation_cursor_requires_active_trigger_host_insert
BEFORE INSERT ON automation_cursors
WHEN NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.trigger_host_runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'TRIGGER_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_TRIGGER_HOST');
END;

CREATE TRIGGER automation_cursor_requires_active_trigger_host_update
BEFORE UPDATE OF trigger_host_runtime_id, workspace_id ON automation_cursors
WHEN NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.trigger_host_runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'TRIGGER_HOST')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_TRIGGER_HOST');
END;

CREATE TRIGGER workspace_hub_runtime_requires_active_binding_insert
BEFORE INSERT ON workspaces
WHEN NEW.hub_runtime_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.hub_runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND rwb.enrollment_mode = 'MESH_PAIRING'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'WORKSPACE_HUB')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_WORKSPACE_HUB');
END;

CREATE TRIGGER workspace_hub_runtime_requires_active_binding_update
BEFORE UPDATE OF hub_runtime_id ON workspaces
WHEN NEW.hub_runtime_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM runtime_workspace_bindings rwb
  WHERE rwb.runtime_id = NEW.hub_runtime_id AND rwb.workspace_id = NEW.workspace_id
    AND rwb.status = 'ACTIVE'
    AND rwb.enrollment_mode = 'MESH_PAIRING'
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'WORKSPACE_HUB')
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_NOT_ACTIVE_WORKSPACE_HUB');
END;

CREATE TRIGGER runtime_workspace_binding_revoke_not_workspace_hub
BEFORE UPDATE OF status ON runtime_workspace_bindings
WHEN NEW.status = 'REVOKED' AND EXISTS (
  SELECT 1 FROM workspaces w
  WHERE w.workspace_id = OLD.workspace_id AND w.hub_runtime_id = OLD.runtime_id
)
BEGIN
  SELECT RAISE(ABORT, 'WORKSPACE_HUB_BINDING_MUST_BE_CLEARED');
END;

CREATE TRIGGER runtime_workspace_binding_revoke_requires_host_drain
BEFORE UPDATE OF status ON runtime_workspace_bindings
WHEN OLD.status = 'ACTIVE' AND NEW.status = 'REVOKED' AND (
  EXISTS (
    SELECT 1 FROM channel_host_assignments a
    WHERE a.runtime_id = OLD.runtime_id AND a.workspace_id = OLD.workspace_id
      AND a.status = 'ACTIVE'
  )
  OR EXISTS (
    SELECT 1 FROM channel_host_lease_records l
    WHERE l.runtime_id = OLD.runtime_id AND l.workspace_id = OLD.workspace_id
      AND julianday(l.lease_expires_at) > julianday('now')
  )
  OR EXISTS (
    SELECT 1 FROM automation_cursors c
    JOIN automations a ON a.automation_id = c.automation_id AND a.workspace_id = c.workspace_id
    WHERE c.trigger_host_runtime_id = OLD.runtime_id AND c.workspace_id = OLD.workspace_id
      AND a.status = 'ENABLED'
  )
  OR EXISTS (
    SELECT 1 FROM automation_occurrences o
    WHERE o.trigger_host_runtime_id = OLD.runtime_id AND o.workspace_id = OLD.workspace_id
      AND o.status NOT IN ('COMPLETED', 'SKIPPED', 'FAILED')
  )
)
BEGIN
  SELECT RAISE(ABORT, 'RUNTIME_WORKSPACE_BINDING_HOSTS_NOT_DRAINED');
END;

CREATE TRIGGER attempt_environment_owner_guard
BEFORE INSERT ON attempts
WHEN NOT EXISTS (
  SELECT 1
  FROM tasks t
  JOIN environments e ON e.environment_id = NEW.environment_id
  JOIN agent_bindings b ON b.agent_binding_id = NEW.agent_binding_id
  JOIN runtimes r ON r.runtime_id = NEW.runtime_id
  JOIN runtime_workspace_bindings rwb
    ON rwb.runtime_id = NEW.runtime_id
   AND rwb.workspace_id = t.workspace_id
   AND rwb.status = 'ACTIVE'
  WHERE t.task_id = NEW.task_id
    AND e.runtime_id = NEW.runtime_id
    AND e.owner_workspace_id = t.workspace_id
    AND (e.owner_task_id IS NULL OR e.owner_task_id = t.task_id)
    AND (e.sharing_scope <> 'ATTEMPT_PRIVATE' OR e.owner_attempt_id = NEW.attempt_id)
    AND (e.sharing_scope <> 'TASK_SHARED' OR e.owner_task_id = t.task_id)
    AND (e.sharing_scope <> 'COWORKER_PRIVATE' OR e.owner_coworker_id = t.origin_coworker_id)
    AND b.workspace_id = t.workspace_id
    AND r.current_incarnation_id = NEW.runtime_incarnation_id
    AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'EXECUTOR')
)
BEGIN
  SELECT RAISE(ABORT, 'ATTEMPT_ENVIRONMENT_SCOPE_MISMATCH');
END;
