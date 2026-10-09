//! Atomic admission checks for a local ManualTrigger occurrence.
use super::*;
use domain_responsibility::{OccurrenceIdentity, TriggerDefinition, TriggerSpec};
use storage_core::AutomationTaskAdmission;

pub(super) fn validate_task_admission(
    tx: &Transaction<'_>,
    commit: &TaskCreateCommit,
) -> Result<(), StoreError> {
    let Some(admission) = commit.automation_admission.as_ref() else {
        if commit.task.automation_id.is_some() || commit.task.automation_occurrence_id.is_some() {
            return Err(StoreError::Invalid(
                "Automation provenance requires occurrence admission".to_owned(),
            ));
        }
        return Ok(());
    };
    validate_admission(tx, commit, admission)
}

fn validate_admission(
    tx: &Transaction<'_>,
    commit: &TaskCreateCommit,
    admission: &AutomationTaskAdmission,
) -> Result<(), StoreError> {
    let task = &commit.task;
    if commit.event.workspace_id != task.workspace_id
        || commit.event.origin_runtime_id != admission.trigger_host_runtime_id
    {
        return Err(StoreError::Invalid(
            "Manual occurrence actor must be the pinned local TriggerHost in the Task Workspace"
                .to_owned(),
        ));
    }
    let automation_revision_sql = to_sql_i64(admission.automation_revision, "Automation revision")?;
    let pinned: Option<(String, i64, i64, String, i64, Option<String>, Option<i64>, String)> = tx.query_row(
        "SELECT a.status, a.current_revision, a.version, ar.routine_id, ar.routine_revision, ar.coworker_id, ar.coworker_revision, ar.triggers_json
         FROM automations a JOIN automation_revisions ar
           ON ar.workspace_id = a.workspace_id AND ar.automation_id = a.automation_id AND ar.revision = a.current_revision
         WHERE a.workspace_id = ?1 AND a.automation_id = ?2 AND ar.revision = ?3",
        params![task.workspace_id, admission.automation_id, automation_revision_sql],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
    ).optional().map_err(map_database_error)?;
    let Some((
        status,
        automation_revision,
        automation_version,
        routine_id,
        routine_revision,
        coworker_id,
        coworker_revision,
        triggers_json,
    )) = pinned
    else {
        return Err(StoreError::Conflict {
            expected: Some(admission.automation_revision),
            actual: None,
        });
    };
    // An owner-issued ManualTrigger is a one-shot operation and is valid while the
    // definition is PAUSED. It never activates recurring triggers. DISABLED is
    // terminal and cannot accept new work.
    if status == "DISABLED"
        || from_sql_i64(automation_revision, "Automation revision")?
            != admission.automation_revision
        || from_sql_i64(automation_version, "Automation version")?
            != admission.expected_automation_version
        || routine_id != admission.routine_id
        || from_sql_i64(routine_revision, "Routine revision")? != admission.routine_revision
    {
        return Err(StoreError::Conflict {
            expected: Some(admission.expected_automation_version),
            actual: Some(from_sql_i64(automation_version, "Automation version")?),
        });
    }
    let triggers: Vec<TriggerSpec> = serde_json::from_str(&triggers_json).map_err(|error| {
        StoreError::Integrity(format!("stored Automation triggers are invalid: {error}"))
    })?;
    let manual_triggers: Vec<_> = triggers
        .iter()
        .filter(|trigger| matches!(trigger.trigger, TriggerDefinition::Manual))
        .collect();
    if manual_triggers.len() != 1 {
        return Err(StoreError::Invalid("local ManualTrigger admission requires exactly one ManualTrigger in the pinned Automation revision".to_owned()));
    }
    let manual = manual_triggers[0];
    if manual.trigger_id != admission.trigger_id {
        return Err(StoreError::Invalid(
            "ManualTrigger identity differs from the pinned Automation revision".to_owned(),
        ));
    }
    if matches!(
        manual.placement,
        domain_responsibility::TriggerPlacement::Hub
    ) || matches!(
        manual.placement,
        domain_responsibility::TriggerPlacement::SpecificRuntime
    ) && manual.runtime_id.as_deref() != Some(admission.trigger_host_runtime_id.as_str())
    {
        return Err(StoreError::Invalid(
            "ManualTrigger placement does not select this local Runtime".to_owned(),
        ));
    }
    let expected_key = OccurrenceIdentity::Manual {
        trigger_id: admission.trigger_id.clone(),
        principal_id: commit.request.principal_id.clone(),
        request_id: commit.request.request_id.clone(),
        automation_id: admission.automation_id.clone(),
    }
    .key()
    .map_err(|_| StoreError::Invalid("Manual occurrence identity is invalid".to_owned()))?;
    if expected_key != admission.occurrence_key {
        return Err(StoreError::Invalid(
            "Manual occurrence key does not match the authenticated request identity".to_owned(),
        ));
    }
    let cursor: Option<(i64, String, i64)> = tx.query_row(
        "SELECT active_automation_revision, trigger_host_runtime_id, host_epoch FROM automation_cursors WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3",
        params![task.workspace_id, admission.automation_id, admission.trigger_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    // If recurring hosting has already assigned this trigger, an owner run must use
    // that same local host. A never-enabled PAUSED definition has no cursor; the
    // local binding/incarnation below is its one-shot host identity.
    if let Some((revision, runtime_id, _)) = cursor.as_ref() {
        let revision = from_sql_i64(*revision, "Automation cursor revision")?;
        if revision != admission.automation_revision
            || runtime_id != &admission.trigger_host_runtime_id
        {
            return Err(StoreError::Conflict {
                expected: Some(admission.automation_revision),
                actual: Some(revision),
            });
        }
    } else if status == "ENABLED" {
        return Err(StoreError::Conflict {
            expected: Some(admission.automation_revision),
            actual: None,
        });
    }
    let host_version: Option<i64> = tx.query_row(
        "SELECT b.version FROM runtime_workspace_bindings b
           JOIN runtimes r ON r.runtime_id = b.runtime_id
           WHERE b.workspace_id = ?1 AND b.runtime_id = ?2 AND b.status = 'ACTIVE'
             AND r.current_incarnation_id = ?3
             AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'TRIGGER_HOST')",
        params![task.workspace_id, admission.trigger_host_runtime_id, admission.trigger_host_runtime_incarnation_id],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?;
    if host_version
        .map(|value| from_sql_i64(value, "Runtime Workspace binding version"))
        .transpose()?
        != Some(admission.trigger_host_binding_version)
    {
        return Err(StoreError::Conflict {
            expected: Some(admission.trigger_host_binding_version),
            actual: host_version
                .map(|value| from_sql_i64(value, "Runtime Workspace binding version"))
                .transpose()?,
        });
    }
    match (
        coworker_id,
        coworker_revision,
        task.origin_coworker_id.as_deref(),
        task.origin_coworker_revision,
    ) {
        (None, None, None, None) => {}
        (Some(expected_id), Some(expected_revision), Some(actual_id), Some(actual_revision))
            if expected_id == actual_id
                && from_sql_i64(expected_revision, "Coworker revision")? == actual_revision =>
        {
            let active: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM coworkers WHERE workspace_id = ?1 AND coworker_id = ?2 AND status = 'ACTIVE')",
                    params![task.workspace_id, actual_id], |row| row.get(0),
                ).map_err(map_database_error)?;
            if !active {
                return Err(StoreError::Invalid(
                    "Automation Coworker is not active".to_owned(),
                ));
            }
        }
        _ => {
            return Err(StoreError::Invalid(
                "Task Coworker provenance differs from its pinned AutomationRevision".to_owned(),
            ));
        }
    }
    if admission.trigger_host_runtime_id.trim().is_empty()
        || admission.occurrence_id.trim().is_empty()
        || admission.expected_automation_version == 0
        || admission.trigger_host_binding_version == 0
        || admission.claim_expires_at <= commit.event.recorded_at
    {
        return Err(StoreError::Invalid(
            "Manual occurrence admission bounds are invalid".to_owned(),
        ));
    }
    Ok(())
}
