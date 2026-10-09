-- The original trigger omitted Environment sharing and principal/Coworker ownership
-- fields. Replace it forward-only; historical migrations remain immutable.
DROP TRIGGER environment_identity_immutable;

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
