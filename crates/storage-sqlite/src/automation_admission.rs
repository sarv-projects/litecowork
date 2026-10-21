//! Atomic admission checks for an Automation occurrence.
//!
//! Two trigger kinds reach the ordinary Task admission path:
//!
//! - `MANUAL`: the owner-issued one-shot `ManualTrigger`. It is valid while the
//!   definition is PAUSED or ENABLED and never activates recurring hosting.
//! - `SCHEDULE`: a recurring slot claimed by the local trigger host. It is
//!   valid only for an ENABLED definition whose cursor this Runtime owns, and
//!   the claimed slot must be a real slot of the pinned schedule.
//!
//! Both kinds commit the occurrence, the ordinary READY Task, the events, and
//! the idempotency receipt in one storage transaction.
use super::*;
use domain_responsibility::{
    OccurrenceIdentity, ScheduleTrigger, TriggerDefinition, TriggerSpec,
};
use storage_core::AutomationTaskAdmission;

type PinnedAutomationAdmissionRow = (
    String,
    i64,
    i64,
    String,
    i64,
    Option<String>,
    Option<i64>,
    String,
);

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

/// The canonical trigger-kind tokens of an occurrence admission.
pub(super) const MANUAL_TRIGGER_KIND: &str = "MANUAL";
pub(super) const SCHEDULE_TRIGGER_KIND: &str = "SCHEDULE";

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
    let pinned: Option<PinnedAutomationAdmissionRow> = tx.query_row(
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
    if routine_id != admission.routine_id
        || from_sql_i64(routine_revision, "Routine revision")? != admission.routine_revision
    {
        return Err(StoreError::Conflict {
            expected: Some(admission.automation_revision),
            actual: None,
        });
    }
    let triggers: Vec<TriggerSpec> = serde_json::from_str(&triggers_json).map_err(|error| {
        StoreError::Integrity(format!("stored Automation triggers are invalid: {error}"))
    })?;
    let trigger = triggers
        .iter()
        .find(|trigger| trigger.trigger_id == admission.trigger_id)
        .ok_or_else(|| {
            StoreError::Invalid(
                "The claimed trigger is not part of the pinned Automation revision".to_owned(),
            )
        })?;
    validate_trigger_placement(trigger, &admission.trigger_host_runtime_id)?;
    validate_local_trigger_host(tx, &commit.task, admission)?;
    validate_coworker_provenance(tx, &commit.task, coworker_id, coworker_revision)?;
    validate_admission_bounds(admission, &commit.event.recorded_at)?;
    match admission.trigger_kind.as_str() {
        MANUAL_TRIGGER_KIND => validate_manual_admission(
            tx,
            commit,
            admission,
            &status,
            automation_revision,
            automation_version,
            &triggers,
            trigger,
        ),
        SCHEDULE_TRIGGER_KIND => validate_schedule_admission(
            tx,
            commit,
            admission,
            &status,
            automation_revision,
            automation_version,
            trigger,
        ),
        _ => Err(StoreError::Invalid(
            "unsupported Automation trigger kind".to_owned(),
        )),
    }
}

/// A trigger must be hosted by this Runtime. A Hub placement is never hosted
/// by a local Runtime, and a pinned Runtime placement never silently
/// substitutes another Runtime.
fn validate_trigger_placement(
    trigger: &TriggerSpec,
    trigger_host_runtime_id: &str,
) -> Result<(), StoreError> {
    if matches!(
        trigger.placement,
        domain_responsibility::TriggerPlacement::Hub
    ) || matches!(
        trigger.placement,
        domain_responsibility::TriggerPlacement::SpecificRuntime
    ) && trigger.runtime_id.as_deref() != Some(trigger_host_runtime_id)
    {
        return Err(StoreError::Invalid(
            "The trigger placement does not select this local Runtime".to_owned(),
        ));
    }
    Ok(())
}

/// The local Runtime must still hold an active `TRIGGER_HOST` Workspace binding
/// for the exact incarnation that claimed the occurrence.
fn validate_local_trigger_host(
    tx: &Transaction<'_>,
    task: &TaskRecord,
    admission: &AutomationTaskAdmission,
) -> Result<(), StoreError> {
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
    Ok(())
}

/// The pinned Coworker revision must still be the Task's origin and must still
/// be active. A paused or archived Coworker blocks proactive materialization.
fn validate_coworker_provenance(
    tx: &Transaction<'_>,
    task: &TaskRecord,
    coworker_id: Option<String>,
    coworker_revision: Option<i64>,
) -> Result<(), StoreError> {
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
    Ok(())
}

fn validate_admission_bounds(
    admission: &AutomationTaskAdmission,
    recorded_at: &str,
) -> Result<(), StoreError> {
    if admission.trigger_host_runtime_id.trim().is_empty()
        || admission.occurrence_id.trim().is_empty()
        || admission.expected_automation_version == 0
        || admission.trigger_host_binding_version == 0
        || admission.claim_expires_at.as_str() <= recorded_at
    {
        return Err(StoreError::Invalid(
            "Automation occurrence admission bounds are invalid".to_owned(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_manual_admission(
    tx: &Transaction<'_>,
    commit: &TaskCreateCommit,
    admission: &AutomationTaskAdmission,
    status: &str,
    automation_revision: i64,
    automation_version: i64,
    triggers: &[TriggerSpec],
    trigger: &TriggerSpec,
) -> Result<(), StoreError> {
    // An owner-issued ManualTrigger is a one-shot operation and is valid while the
    // definition is PAUSED. It never activates recurring triggers. DISABLED is
    // terminal and cannot accept new work.
    if status == "DISABLED"
        || from_sql_i64(automation_revision, "Automation revision")?
            != admission.automation_revision
        || from_sql_i64(automation_version, "Automation version")?
            != admission.expected_automation_version
    {
        return Err(StoreError::Conflict {
            expected: Some(admission.expected_automation_version),
            actual: Some(from_sql_i64(automation_version, "Automation version")?),
        });
    }
    let manual_triggers: Vec<_> = triggers
        .iter()
        .filter(|trigger| matches!(trigger.trigger, TriggerDefinition::Manual))
        .collect();
    if manual_triggers.len() != 1 {
        return Err(StoreError::Invalid("local ManualTrigger admission requires exactly one ManualTrigger in the pinned Automation revision".to_owned()));
    }
    if !matches!(trigger.trigger, TriggerDefinition::Manual) {
        return Err(StoreError::Invalid(
            "The claimed trigger is not the ManualTrigger of the pinned Automation revision".to_owned(),
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
        params![commit.task.workspace_id, admission.automation_id, admission.trigger_id],
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
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_schedule_admission(
    tx: &Transaction<'_>,
    commit: &TaskCreateCommit,
    admission: &AutomationTaskAdmission,
    status: &str,
    automation_revision: i64,
    automation_version: i64,
    trigger: &TriggerSpec,
) -> Result<(), StoreError> {
    // A recurring slot only fires for an enabled definition whose cursor this
    // Runtime owns. A paused or disabled definition never materializes a slot.
    if status != "ENABLED"
        || from_sql_i64(automation_revision, "Automation revision")?
            != admission.automation_revision
        || from_sql_i64(automation_version, "Automation version")?
            != admission.expected_automation_version
    {
        return Err(StoreError::Conflict {
            expected: Some(admission.expected_automation_version),
            actual: Some(from_sql_i64(automation_version, "Automation version")?),
        });
    }
    let TriggerDefinition::Schedule { .. } = &trigger.trigger else {
        return Err(StoreError::Invalid(
            "The claimed trigger is not a schedule trigger".to_owned(),
        ));
    };
    let Some(scheduled_epoch_ms) = admission.scheduled_epoch_ms else {
        return Err(StoreError::Invalid(
            "A scheduled occurrence must carry the resolved slot instant".to_owned(),
        ));
    };
    let expected_key = OccurrenceIdentity::Schedule {
        trigger_id: admission.trigger_id.clone(),
        scheduled_epoch_ms,
    }
    .key()
    .map_err(|_| StoreError::Invalid("Schedule occurrence identity is invalid".to_owned()))?;
    if expected_key != admission.occurrence_key {
        return Err(StoreError::Invalid(
            "Schedule occurrence key does not match the claimed slot".to_owned(),
        ));
    }
    let cursor: Option<(i64, String, i64, Option<String>)> = tx.query_row(
        "SELECT active_automation_revision, trigger_host_runtime_id, host_epoch, next_scheduled_at
           FROM automation_cursors WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3",
        params![commit.task.workspace_id, admission.automation_id, admission.trigger_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).optional().map_err(map_database_error)?;
    let Some((cursor_revision, cursor_runtime_id, _host_epoch, cursor_next)) = cursor else {
        return Err(StoreError::Conflict {
            expected: Some(admission.automation_revision),
            actual: None,
        });
    };
    if from_sql_i64(cursor_revision, "Automation cursor revision")?
        != admission.automation_revision
        || cursor_runtime_id != admission.trigger_host_runtime_id
    {
        return Err(StoreError::Conflict {
            expected: Some(admission.automation_revision),
            actual: Some(from_sql_i64(cursor_revision, "Automation cursor revision")?),
        });
    }
    let now_epoch_ms = domain_responsibility::parse_instant(&commit.event.recorded_at)
        .ok_or_else(|| StoreError::Invalid("The occurrence timestamp is invalid".to_owned()))?;
    if scheduled_epoch_ms > now_epoch_ms {
        return Err(StoreError::Invalid(
            "A schedule slot cannot be materialized before it is due".to_owned(),
        ));
    }
    if let Some(next) = cursor_next.as_deref() {
        let next_epoch_ms = domain_responsibility::parse_instant(next).ok_or_else(|| {
            StoreError::Integrity("The persisted cursor deadline is invalid".to_owned())
        })?;
        if scheduled_epoch_ms < next_epoch_ms {
            return Err(StoreError::Conflict {
                expected: Some(u64::try_from(next_epoch_ms).unwrap_or_default()),
                actual: Some(u64::try_from(scheduled_epoch_ms).unwrap_or_default()),
            });
        }
    }
    // The claimed slot must be a real slot of the pinned schedule. A host
    // cannot invent a slot that the recurrence does not produce.
    let schedule = super::automation_host::schedule_trigger(trigger).ok_or_else(|| {
        StoreError::Invalid("The pinned schedule trigger is not executable".to_owned())
    })?;
    validate_schedule_slot(&schedule, scheduled_epoch_ms)
}

/// Confirms that the claimed instant is one of the schedule's own slots.
fn validate_schedule_slot(
    schedule: &ScheduleTrigger,
    scheduled_epoch_ms: i64,
) -> Result<(), StoreError> {
    let Some(before) = scheduled_epoch_ms.checked_sub(1) else {
        return Err(StoreError::Invalid(
            "The claimed schedule slot is out of range".to_owned(),
        ));
    };
    let slot = schedule
        .next_slot_after(before)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    if slot != Some(scheduled_epoch_ms) {
        return Err(StoreError::Invalid(
            "The claimed schedule slot is not a slot of the pinned recurrence".to_owned(),
        ));
    }
    Ok(())
}
