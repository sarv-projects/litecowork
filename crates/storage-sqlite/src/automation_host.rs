//! Durable Automation trigger-host state: cursors, occurrences, and settlement.
//!
//! This module owns the storage side of the local trigger host. It reads the
//! Automation definition, resolves the pinned schedule trigger, and persists the
//! cursor that decides which slot is due next. It never creates a Task: the
//! ordinary Task admission path materializes an occurrence into a READY Task and
//! links them atomically.
//!
//! Guarantees enforced here:
//!
//! - A misfire is recorded as a bounded `SKIPPED` occurrence; it is never
//!   silently dropped.
//! - The `(automation_id, trigger_id, occurrence_key)` uniqueness constraint
//!   makes a repeated settlement a no-op instead of a second occurrence.
//! - A cursor only moves forward. A clock that moved backwards never rewinds a
//!   persisted deadline.

use super::routines::RoutineTaskOutcome;
use super::*;
use domain_responsibility::{
    AutomationCursor, AutomationExecutionPolicy, AutomationStatus, CoworkerRevisionRef,
    CursorFacts, MisfirePolicy, OccurrenceIdentity, OccurrenceStatus, ScheduleTrigger,
    SettlementWindow, TriggerDefinition, TriggerSpec, decide_cursor, format_instant,
    parse_instant,
};
use std::sync::mpsc;

/// The writer operation type used by this module.

/// One Automation trigger that the local host currently owns.
#[derive(Clone, Debug, PartialEq)]
pub struct DueSchedule {
    pub workspace_id: String,
    pub automation_id: String,
    pub automation_revision: u64,
    pub automation_version: u64,
    pub automation_name: String,
    pub routine_id: String,
    pub routine_revision: u64,
    pub coworker_ref: Option<CoworkerRevisionRef>,
    pub trigger: TriggerSpec,
    pub execution_policy: AutomationExecutionPolicy,
    /// The persisted cursor, when the trigger already has one.
    pub cursor: Option<AutomationCursor>,
    pub schedule: ScheduleTrigger,
}

/// A recorded occurrence that was not materialized into a Task.
#[derive(Clone, Debug, PartialEq)]
pub struct SkippedOccurrence {
    pub occurrence_id: String,
    pub automation_id: String,
    pub trigger_id: String,
    pub occurrence_key: String,
    pub scheduled_for: String,
    pub reason: String,
    pub version: u64,
}

/// One already persisted occurrence for a logical trigger delivery.
#[derive(Clone, Debug, PartialEq)]
pub struct ExistingOccurrence {
    pub occurrence_id: String,
    pub status: OccurrenceStatus,
    pub task_id: Option<String>,
    pub version: u64,
    pub scheduled_for: Option<String>,
}

/// The persisted state of one local trigger host.
#[derive(Clone, Debug)]
pub struct AutomationHostContext {
    pub origin_runtime_id: String,
    pub recorded_at: String,
}

/// The storage adapter for the local Automation trigger host.
#[derive(Clone)]
pub struct SqliteAutomationHostStore {
    store: SqliteWorkspaceStore,
    context: AutomationHostContext,
}

impl SqliteAutomationHostStore {
    pub fn new(
        store: SqliteWorkspaceStore,
        context: AutomationHostContext,
    ) -> Result<Self, StoreError> {
        if context.origin_runtime_id.trim().is_empty()
            || context.recorded_at.trim().is_empty()
        {
            return Err(StoreError::Invalid(
                "Automation host context must name its Runtime and timestamp".to_owned(),
            ));
        }
        Ok(Self { store, context })
    }

    fn run<T, F>(&self, operation: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }

    /// Lists every enabled Automation schedule trigger that this Runtime owns.
    ///
    /// A trigger without a cursor is not returned: a cursor is created when the
    /// owner enables the Automation, so a missing cursor means the Automation is
    /// not hosted here and must not be fired.
    pub fn list_due_schedules(
        &self,
        runtime_id: &str,
        now: i64,
    ) -> Result<Vec<DueSchedule>, StoreError> {
        let runtime_id = runtime_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let mut statement = tx
                .prepare(
                    "SELECT a.workspace_id, a.automation_id, a.current_revision, a.version, a.name,
                            ar.routine_id, ar.routine_revision, ar.coworker_id, ar.coworker_revision,
                            ar.triggers_json, ar.execution_policy_json,
                            c.trigger_id, c.active_automation_revision, c.trigger_host_runtime_id, c.host_epoch,
                            c.cursor_digest, c.last_seen_digest, c.last_observation_ref_json,
                            c.next_scheduled_at, c.last_checked_at, c.observation_gap_since, c.version
                       FROM automations a
                       JOIN automation_revisions ar
                         ON ar.workspace_id = a.workspace_id
                        AND ar.automation_id = a.automation_id
                        AND ar.revision = a.current_revision
                       JOIN automation_cursors c
                         ON c.workspace_id = a.workspace_id
                        AND c.automation_id = a.automation_id
                        AND c.trigger_host_runtime_id = ?1
                       WHERE a.status = 'ENABLED'",
                )
                .map_err(map_database_error)?;
            let rows = statement
                .query_map(params![runtime_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        from_row_u64(row, 2)?,
                        from_row_u64(row, 3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        from_row_u64(row, 6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        from_row_u64(row, 12)?,
                        row.get::<_, String>(13)?,
                        from_row_u64(row, 14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, Option<String>>(16)?,
                        row.get::<_, Option<String>>(17)?,
                        row.get::<_, Option<String>>(18)?,
                        row.get::<_, String>(19)?,
                        row.get::<_, Option<String>>(20)?,
                        from_row_u64(row, 21)?,
                    ))
                })
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            let mut schedules = Vec::new();
            for row in rows {
                let (
                    workspace_id,
                    automation_id,
                    automation_revision,
                    automation_version,
                    automation_name,
                    routine_id,
                    routine_revision,
                    coworker_id,
                    coworker_revision,
                    triggers_json,
                    execution_policy_json,
                    cursor_trigger_id,
                    cursor_revision,
                    cursor_runtime_id,
                    host_epoch,
                    cursor_digest,
                    last_seen_digest,
                    last_observation_ref_json,
                    next_scheduled_at,
                    last_checked_at,
                    observation_gap_since,
                    cursor_version,
                ) = row;
                if cursor_runtime_id != runtime_id {
                    continue;
                }
                let triggers: Vec<TriggerSpec> = serde_json::from_str(&triggers_json).map_err(
                    |error| {
                        StoreError::Integrity(format!(
                            "stored Automation triggers are invalid: {error}"
                        ))
                    },
                )?;
                let execution_policy: AutomationExecutionPolicy =
                    serde_json::from_str(&execution_policy_json).map_err(|error| {
                        StoreError::Integrity(format!(
                            "stored Automation execution policy is invalid: {error}"
                        ))
                    })?;
                let cursor = AutomationCursor {
                    automation_id: automation_id.clone(),
                    trigger_id: cursor_trigger_id.clone(),
                    active_automation_revision: cursor_revision,
                    trigger_host_runtime_id: cursor_runtime_id,
                    host_epoch,
                    cursor_digest,
                    last_seen_digest,
                    last_observation_ref: last_observation_ref_json
                        .and_then(|value| serde_json::from_str(&value).ok()),
                    next_scheduled_at: next_scheduled_at
                        .as_deref()
                        .and_then(parse_instant),
                    last_checked_at: parse_instant(&last_checked_at).unwrap_or_default(),
                    observation_gap_since: observation_gap_since
                        .as_deref()
                        .and_then(parse_instant),
                    version: cursor_version,
                };
                let coworker_ref = match (coworker_id, coworker_revision) {
                    (Some(coworker_id), Some(coworker_revision)) => Some(CoworkerRevisionRef {
                        coworker_id,
                        revision: u64::try_from(coworker_revision).map_err(|_| {
                            StoreError::Integrity("Coworker revision is out of range".to_owned())
                        })?,
                    }),
                    _ => None,
                };
                for trigger in triggers {
                    let schedule = match schedule_trigger(&trigger) {
                        Some(schedule) => schedule,
                        None => continue,
                    };
                    // The cursor identity is `(automation_id, trigger_id)`. A trigger
                    // without its own cursor is not hosted here and must never be
                    // settled against another trigger's cursor.
                    if cursor.trigger_id != trigger.trigger_id {
                        continue;
                    }
                    // The cursor only advances against the revision it owns. A cursor
                    // that points at another revision is not hosted here and must not
                    // be fired until the owner re-establishes the binding.
                    if cursor.active_automation_revision != automation_revision {
                        continue;
                    }
                    // A cursor without a deadline has never observed the schedule; the
                    // host initializes it with the next future slot. A cursor whose
                    // deadline is still in the future is not due.
                    let is_due = match cursor.next_scheduled_at {
                        None => true,
                        Some(next) => next <= now,
                    };
                    if !is_due {
                        continue;
                    }
                    schedules.push(DueSchedule {
                        workspace_id: workspace_id.clone(),
                        automation_id: automation_id.clone(),
                        automation_revision,
                        automation_version,
                        automation_name: automation_name.clone(),
                        routine_id: routine_id.clone(),
                        routine_revision,
                        coworker_ref: coworker_ref.clone(),
                        trigger,
                        execution_policy: execution_policy.clone(),
                        cursor: Some(cursor.clone()),
                        schedule,
                    });
                }
            }
            Ok(schedules)
        })
    }

    /// Returns the Task linked to an occurrence, when the occurrence was already
    /// materialized. A repeated settlement must not create a second Task.
    pub fn occurrence_task(
        &self,
        workspace_id: &str,
        automation_id: &str,
        trigger_id: &str,
        occurrence_key: &str,
    ) -> Result<Option<String>, StoreError> {
        let (workspace_id, automation_id, trigger_id, occurrence_key) = (
            workspace_id.to_owned(),
            automation_id.to_owned(),
            trigger_id.to_owned(),
            occurrence_key.to_owned(),
        );
        self.run(move |connection| {
            connection
                .query_row(
                    "SELECT task_id FROM automation_occurrences
                      WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3 AND occurrence_key = ?4",
                    params![workspace_id, automation_id, trigger_id, occurrence_key],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(map_database_error)
                .map(|value| value.flatten())
        })
    }

    /// Returns the persisted occurrence of one logical trigger delivery.
    ///
    /// A repeated settlement of the same slot finds the occurrence that already
    /// exists and must not create a second Task.
    pub fn occurrence_for_slot(
        &self,
        workspace_id: &str,
        automation_id: &str,
        trigger_id: &str,
        occurrence_key: &str,
    ) -> Result<Option<ExistingOccurrence>, StoreError> {
        let (workspace_id, automation_id, trigger_id, occurrence_key) = (
            workspace_id.to_owned(),
            automation_id.to_owned(),
            trigger_id.to_owned(),
            occurrence_key.to_owned(),
        );
        self.run(move |connection| {
            let row: Option<(String, String, Option<String>, i64, Option<String>)> = connection
                .query_row(
                    "SELECT occurrence_id, status, task_id, version, scheduled_for
                       FROM automation_occurrences
                      WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3 AND occurrence_key = ?4",
                    params![workspace_id, automation_id, trigger_id, occurrence_key],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()
                .map_err(map_database_error)?;
            row.map(
                |(occurrence_id, status, task_id, version, scheduled_for)| {
                    let status = OccurrenceStatus::parse(&status).ok_or_else(|| {
                        StoreError::Integrity(format!(
                            "stored occurrence status {status} is not a known status"
                        ))
                    })?;
                    Ok(ExistingOccurrence {
                        occurrence_id,
                        status,
                        task_id,
                        version: u64::try_from(version).unwrap_or_default(),
                        scheduled_for,
                    })
                },
            )
            .transpose()
        })
    }

    /// Counts the occurrence Tasks of one Automation that are not yet terminal.
    ///
    /// The count enforces `max_concurrent_occurrences` and the overlap policy.
    pub fn nonterminal_occurrence_count(
        &self,
        workspace_id: &str,
        automation_id: &str,
    ) -> Result<u32, StoreError> {
        let (workspace_id, automation_id) = (workspace_id.to_owned(), automation_id.to_owned());
        self.run(move |connection| {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM automation_occurrences
                      WHERE workspace_id = ?1 AND automation_id = ?2
                        AND status NOT IN ('COMPLETED', 'SKIPPED', 'FAILED')",
                    params![workspace_id, automation_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            u32::try_from(count).map_err(|_| {
                StoreError::Integrity("The occurrence count is out of range".to_owned())
            })
        })
    }

    /// Records one bounded `SKIPPED` occurrence and advances nothing else.
    ///
    /// The insert is idempotent: a repeated settlement of the same slot records
    /// nothing new and reports that the occurrence already existed.
    ///
    /// An occurrence always starts `PENDING` at version 1 and then settles to
    /// `SKIPPED` at version 2, so the persisted history matches every other
    /// occurrence lifecycle. The skip reason is stored as a typed diagnostic.
    pub fn record_skipped_occurrence(
        &self,
        workspace_id: &str,
        automation_id: &str,
        automation_revision: u64,
        routine_id: &str,
        routine_revision: u64,
        trigger_id: &str,
        trigger_host_runtime_id: &str,
        occurrence_id: &str,
        occurrence_key: &str,
        scheduled_for: i64,
        reason: &str,
    ) -> Result<bool, StoreError> {
        let recorded_at = self.context.recorded_at.clone();
        let origin_runtime_id = self.context.origin_runtime_id.clone();
        let (
            workspace_id,
            automation_id,
            routine_id,
            trigger_id,
            trigger_host_runtime_id,
            occurrence_id,
            occurrence_key,
        ) = (
            workspace_id.to_owned(),
            automation_id.to_owned(),
            routine_id.to_owned(),
            trigger_id.to_owned(),
            trigger_host_runtime_id.to_owned(),
            occurrence_id.to_owned(),
            occurrence_key.to_owned(),
        );
        let automation_revision = to_sql_i64(automation_revision, "Automation revision")?;
        let routine_revision = to_sql_i64(routine_revision, "Routine revision")?;
        let scheduled_for = format_instant(scheduled_for)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        let reason = reason.to_owned();
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            let snapshot = |status: &str, version: u64| {
                json!({
                    "workspace_id": workspace_id,
                    "occurrence_id": occurrence_id,
                    "automation_id": automation_id,
                    "automation_revision": automation_revision,
                    "routine_id": routine_id,
                    "routine_revision": routine_revision,
                    "trigger_id": trigger_id,
                    "trigger_host_runtime_id": trigger_host_runtime_id,
                    "occurrence_key": occurrence_key,
                    "status": status,
                    "version": version,
                    "claim_epoch": 0,
                    "claim_expires_at": Value::Null,
                    "task_id": Value::Null,
                    "scheduled_for": scheduled_for,
                    "covered_misfire_range": Value::Null,
                    "trigger_input_ref": Value::Null,
                    "trigger_payload_digest": Value::Null,
                    "blockers": [{"code": reason}],
                    "created_at": recorded_at,
                    "updated_at": recorded_at,
                })
            };
            let created_bytes = canonical_json(&snapshot("PENDING", 1))?;
            let skipped_bytes = canonical_json(&snapshot("SKIPPED", 2))?;
            let created_blob = blobs.put(
                &workspace_id,
                BlobPurpose::AggregateState,
                &created_bytes,
                "application/vnd.litecow.automation-occurrence+json",
            )?;
            let skipped_blob = blobs.put(
                &workspace_id,
                BlobPurpose::AggregateState,
                &skipped_bytes,
                "application/vnd.litecow.automation-occurrence+json",
            )?;
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT version FROM automation_occurrences WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3 AND occurrence_key = ?4",
                    params![workspace_id, automation_id, trigger_id, occurrence_key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            if existing.is_some() {
                tx.commit().map_err(map_database_error)?;
                return Ok(false);
            }
            let changed = tx
                .execute(
                    "INSERT INTO automation_occurrences(workspace_id, occurrence_id, automation_id, automation_revision, routine_id, routine_revision, trigger_id, trigger_host_runtime_id, occurrence_key, scheduled_for, covered_misfire_range_json, trigger_input_ref_json, trigger_payload_digest, blockers_json, claim_epoch, claim_expires_at, task_id, status, created_at, updated_at, version)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL, NULL, ?11, 0, NULL, NULL, 'PENDING', ?12, ?12, 1)",
                    params![
                        workspace_id,
                        occurrence_id,
                        automation_id,
                        automation_revision,
                        routine_id,
                        routine_revision,
                        trigger_id,
                        trigger_host_runtime_id,
                        occurrence_key,
                        scheduled_for,
                        canonical_json(&json!([{"code": reason}]))?,
                        recorded_at,
                    ],
                )
                .map_err(map_database_error)?;
            if changed != 1 {
                tx.commit().map_err(map_database_error)?;
                return Ok(false);
            }
            write_occurrence_event(
                &tx,
                &workspace_id,
                &origin_runtime_id,
                &occurrence_id,
                1,
                &created_blob,
                "automation.occurrence.created.v1",
                &json!({
                    "occurrence_id": occurrence_id,
                    "automation_id": automation_id,
                    "automation_revision": automation_revision,
                    "routine_id": routine_id,
                    "routine_revision": routine_revision,
                    "trigger_id": trigger_id,
                    "trigger_host_runtime_id": trigger_host_runtime_id,
                    "occurrence_key": occurrence_key,
                    "version": 1,
                    "claim_epoch": 0,
                    "claim_expires_at": Value::Null,
                    "task_id": Value::Null,
                    "from": Value::Null,
                    "to": "PENDING",
                }),
                &recorded_at,
            )?;
            let settled = tx
                .execute(
                    "UPDATE automation_occurrences SET status = 'SKIPPED', blockers_json = ?1, updated_at = ?2, version = 2 WHERE workspace_id = ?3 AND occurrence_id = ?4 AND status = 'PENDING' AND version = 1",
                    params![
                        canonical_json(&json!([{"code": reason}]))?,
                        recorded_at,
                        workspace_id,
                        occurrence_id,
                    ],
                )
                .map_err(map_database_error)?;
            if settled != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(1),
                    actual: None,
                });
            }
            write_occurrence_event(
                &tx,
                &workspace_id,
                &origin_runtime_id,
                &occurrence_id,
                2,
                &skipped_blob,
                "automation.occurrence.status.changed.v1",
                &json!({
                    "occurrence_id": occurrence_id,
                    "automation_id": automation_id,
                    "automation_revision": automation_revision,
                    "routine_id": routine_id,
                    "routine_revision": routine_revision,
                    "trigger_id": trigger_id,
                    "trigger_host_runtime_id": trigger_host_runtime_id,
                    "occurrence_key": occurrence_key,
                    "version": 2,
                    "claim_epoch": 0,
                    "claim_expires_at": Value::Null,
                    "task_id": Value::Null,
                    "from": "PENDING",
                    "to": "SKIPPED",
                }),
                &recorded_at,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(true)
        })
    }

    /// Advances the persisted cursor after the trigger's slots are settled.
    ///
    /// The advance is monotonic: a stale or repeated advance never rewinds the
    /// persisted deadline and never reopens a settled slot.
    pub fn advance_cursor(
        &self,
        workspace_id: &str,
        automation_id: &str,
        trigger_id: &str,
        next_scheduled_at: Option<i64>,
        now: i64,
        misfired: bool,
    ) -> Result<AutomationCursor, StoreError> {
        let (workspace_id, automation_id, trigger_id) = (
            workspace_id.to_owned(),
            automation_id.to_owned(),
            trigger_id.to_owned(),
        );
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let row: Option<(u64, u64, String, String, Option<String>, Option<String>, Option<String>, String, Option<String>, u64)> = tx
                .query_row(
                    "SELECT active_automation_revision, host_epoch, trigger_host_runtime_id, cursor_digest, last_seen_digest, last_observation_ref_json, next_scheduled_at, last_checked_at, observation_gap_since, version
                       FROM automation_cursors WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3",
                    params![workspace_id, automation_id, trigger_id],
                    |row| {
                        Ok((
                            from_row_u64(row, 0)?,
                            from_row_u64(row, 1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                            row.get(8)?,
                            from_row_u64(row, 9)?,
                        ))
                    },
                )
                .optional()
                .map_err(map_database_error)?;
            let Some((
                active_automation_revision,
                host_epoch,
                trigger_host_runtime_id,
                cursor_digest,
                last_seen_digest,
                last_observation_ref_json,
                stored_next,
                stored_checked,
                observation_gap_since,
                version,
            )) = row
            else {
                return Err(StoreError::NotFound);
            };
            let stored_next_instant = stored_next.as_deref().and_then(|value| parse_instant(value));
            let stored = AutomationCursor {
                automation_id: automation_id.clone(),
                trigger_id: trigger_id.clone(),
                active_automation_revision,
                trigger_host_runtime_id,
                host_epoch,
                cursor_digest,
                last_seen_digest,
                last_observation_ref: last_observation_ref_json
                    .and_then(|value| serde_json::from_str(&value).ok()),
                next_scheduled_at: stored_next_instant,
                last_checked_at: parse_instant(&stored_checked).unwrap_or_default(),
                observation_gap_since: observation_gap_since
                    .as_deref()
                    .and_then(parse_instant),
                version,
            };
            let decision = decide_cursor(
                &stored,
                CursorFacts {
                    now,
                    next_scheduled_at,
                    misfired,
                },
                &stored.trigger_host_runtime_id,
            )
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
            let next_text = match decision {
                domain_responsibility::CursorDecision::Unchanged => stored_next.clone(),
                domain_responsibility::CursorDecision::Advance {
                    next_scheduled_at, ..
                } => next_scheduled_at
                    .map(|value| format_instant(value))
                    .transpose()
                    .map_err(|error| StoreError::Invalid(error.to_string()))?,
                domain_responsibility::CursorDecision::TransferHost { .. } => {
                    return Err(StoreError::Invalid(
                        "a cursor host transfer requires an explicit Automation revision".to_owned(),
                    ));
                }
            };
            // The advance is monotonic: it never rewinds a persisted deadline.
            let changed = tx
                .execute(
                    "UPDATE automation_cursors
                        SET next_scheduled_at = ?1, last_checked_at = ?2, observation_gap_since = ?3, version = version + 1
                      WHERE workspace_id = ?4 AND automation_id = ?5 AND trigger_id = ?6
                        AND (next_scheduled_at IS NULL OR ?1 IS NULL OR next_scheduled_at < ?1)",
                    params![
                        next_text,
                        format_instant(now)
                            .map_err(|error| StoreError::Invalid(error.to_string()))?,
                        if misfired {
                            stored
                                .next_scheduled_at
                                .map(|value| format_instant(value))
                                .transpose()
                                .map_err(|error| StoreError::Invalid(error.to_string()))?
                        } else {
                            observation_gap_since.clone()
                        },
                        workspace_id,
                        automation_id,
                        trigger_id,
                    ],
                )
                .map_err(map_database_error)?;
            let row: (Option<String>, String, Option<String>, u64) = tx
                .query_row(
                    "SELECT next_scheduled_at, last_checked_at, observation_gap_since, version
                       FROM automation_cursors WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3",
                    params![workspace_id, automation_id, trigger_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, from_row_u64(row, 3)?)),
                )
                .map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            if changed == 0 {
                return Ok(stored);
            }
            Ok(AutomationCursor {
                automation_id,
                trigger_id,
                active_automation_revision: stored.active_automation_revision,
                trigger_host_runtime_id: stored.trigger_host_runtime_id,
                host_epoch: stored.host_epoch,
                cursor_digest: stored.cursor_digest,
                last_seen_digest: stored.last_seen_digest,
                last_observation_ref: stored.last_observation_ref,
                next_scheduled_at: row.0.as_deref().and_then(|value| parse_instant(value)),
                last_checked_at: parse_instant(&row.1).unwrap_or_default(),
                observation_gap_since: row.2.as_deref().and_then(|value| parse_instant(value)),
                version: row.3,
            })
        })
    }

    /// Creates the durable cursor for one trigger when the owner enables the
    /// Automation. The cursor pins the Runtime that owns the trigger.
    pub fn create_cursor(
        &self,
        workspace_id: &str,
        automation_id: &str,
        automation_revision: u64,
        trigger_id: &str,
        trigger_host_runtime_id: &str,
        host_epoch: u64,
        next_scheduled_at: Option<i64>,
    ) -> Result<AutomationCursor, StoreError> {
        let recorded_at = self.context.recorded_at.clone();
        let (workspace_id, automation_id, trigger_id, trigger_host_runtime_id) = (
            workspace_id.to_owned(),
            automation_id.to_owned(),
            trigger_id.to_owned(),
            trigger_host_runtime_id.to_owned(),
        );
        let automation_revision = to_sql_i64(automation_revision, "Automation revision")?;
        let host_epoch = to_sql_i64(host_epoch, "Automation host epoch")?;
        let next_scheduled_at = next_scheduled_at
            .map(|value| format_instant(value))
            .transpose()
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT host_epoch FROM automation_cursors WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3",
                    params![workspace_id, automation_id, trigger_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            if let Some(existing) = existing {
                if existing != host_epoch {
                    return Err(StoreError::Conflict {
                        expected: Some(host_epoch as u64),
                        actual: Some(existing as u64),
                    });
                }
                tx.commit().map_err(map_database_error)?;
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: Some(existing as u64),
                });
            }
            // The digest of an empty provider cursor. It never carries plaintext
            // provider bytes and cannot recover another host's cursor.
            let cursor_digest = format!("sha256:{}", "0".repeat(64));
            tx.execute(
                "INSERT INTO automation_cursors(workspace_id, automation_id, active_automation_revision, trigger_id, trigger_host_runtime_id, host_epoch, cursor_digest, last_seen_digest, last_observation_ref_json, next_scheduled_at, last_checked_at, observation_gap_since, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8, ?9, NULL, 1)",
                params![
                    workspace_id,
                    automation_id,
                    automation_revision,
                    trigger_id,
                    trigger_host_runtime_id,
                    host_epoch,
                    cursor_digest,
                    next_scheduled_at,
                    recorded_at,
                ],
            )
            .map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            Ok(AutomationCursor {
                automation_id,
                trigger_id,
                active_automation_revision: u64::try_from(automation_revision).unwrap_or_default(),
                trigger_host_runtime_id,
                host_epoch: u64::try_from(host_epoch).unwrap_or_default(),
                cursor_digest,
                last_seen_digest: None,
                last_observation_ref: None,
                next_scheduled_at: next_scheduled_at
                    .as_deref()
                    .and_then(parse_instant),
                last_checked_at: parse_instant(&recorded_at).unwrap_or_default(),
                observation_gap_since: None,
                version: 1,
            })
        })
    }

    /// Lists the persisted occurrences of one Automation, newest first.
    pub fn list_occurrences(
        &self,
        workspace_id: &str,
        automation_id: &str,
        limit: usize,
    ) -> Result<Vec<domain_responsibility::OccurrenceStatus>, StoreError> {
        let (workspace_id, automation_id) = (workspace_id.to_owned(), automation_id.to_owned());
        if !(1..=200).contains(&limit) {
            return Err(StoreError::Invalid(
                "occurrence page size is out of range".to_owned(),
            ));
        }
        self.run(move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT status FROM automation_occurrences WHERE workspace_id = ?1 AND automation_id = ?2 ORDER BY created_at DESC, occurrence_id DESC LIMIT ?3",
                )
                .map_err(map_database_error)?;
            let rows = statement
                .query_map(params![workspace_id, automation_id, limit as i64], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            rows.into_iter()
                .map(|value| {
                    OccurrenceStatus::parse(&value).ok_or_else(|| {
                        StoreError::Integrity(format!(
                            "stored occurrence status {value} is not a known status"
                        ))
                    })
                })
                .collect()
        })
    }

    /// Lists the Workspaces this Runtime may host triggers for.
    ///
    /// A Workspace is listed only when this Runtime still holds an `ACTIVE`
    /// binding that carries the `TRIGGER_HOST` role and the Workspace itself is
    /// `ACTIVE`. A revoked binding or an archived Workspace removes the
    /// Workspace from the host list immediately; it never keeps a stale
    /// recurring owner alive.
    pub fn list_trigger_host_workspaces(
        &self,
        runtime_id: &str,
    ) -> Result<Vec<TriggerHostWorkspace>, StoreError> {
        let runtime_id = runtime_id.to_owned();
        self.run(move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT w.workspace_id, w.current_instruction_revision, w.default_agent_binding_id
                       FROM runtime_workspace_bindings b
                       JOIN workspaces w ON w.workspace_id = b.workspace_id
                      WHERE b.runtime_id = ?1
                        AND b.status = 'ACTIVE'
                        AND w.status = 'ACTIVE'
                        AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'TRIGGER_HOST')
                      ORDER BY w.workspace_id",
                )
                .map_err(map_database_error)?;
            let rows = statement
                .query_map(params![runtime_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            rows.into_iter()
                .map(|(workspace_id, instruction_revision, default_agent_binding_id)| {
                    Ok(TriggerHostWorkspace {
                        workspace_id,
                        current_instruction_revision: instruction_revision
                            .map(|value| u64::try_from(value).unwrap_or_default()),
                        default_agent_binding_id,
                    })
                })
                .collect()
        })
    }

    /// Lists the occurrence history of one Automation, newest first.
    ///
    /// The page reports the persisted occurrence state and the ordinary Task it
    /// links. It never reports a Task as settled because a schedule completed:
    /// the Task outcome and the occurrence outcome are separate facts.
    ///
    /// The optional keyset cursor names the last occurrence of the previous
    /// page. It is bound to the ordered `(created_at, occurrence_id)` position.
    pub fn list_occurrence_page(
        &self,
        workspace_id: &str,
        automation_id: &str,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<Vec<OccurrencePageItem>, StoreError> {
        let (workspace_id, automation_id) = (workspace_id.to_owned(), automation_id.to_owned());
        if !(1..=200).contains(&limit) {
            return Err(StoreError::Invalid(
                "occurrence page size is out of range".to_owned(),
            ));
        }
        self.run(move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT occurrence_id, automation_id, automation_revision, routine_id, routine_revision,
                            trigger_id, trigger_host_runtime_id, occurrence_key, claim_epoch, claim_expires_at,
                            status, task_id, scheduled_for, covered_misfire_range_json, trigger_input_ref_json,
                            trigger_payload_digest, blockers_json, created_at, updated_at, version
                       FROM automation_occurrences
                      WHERE workspace_id = ?1 AND automation_id = ?2
                        AND (?3 IS NULL OR created_at < ?3 OR (created_at = ?3 AND occurrence_id < ?4))
                      ORDER BY created_at DESC, occurrence_id DESC
                      LIMIT ?5",
                )
                .map_err(map_database_error)?;
            let after_created = after.as_ref().map(|value| value.0.clone());
            let after_occurrence = after.as_ref().map(|value| value.1.clone());
            let rows = statement
                .query_map(
                    params![
                        workspace_id,
                        automation_id,
                        after_created,
                        after_occurrence,
                        limit as i64
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            from_row_u64(row, 2)?,
                            row.get::<_, String>(3)?,
                            from_row_u64(row, 4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                            from_row_u64(row, 8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, String>(10)?,
                            row.get::<_, Option<String>>(11)?,
                            row.get::<_, Option<String>>(12)?,
                            row.get::<_, Option<String>>(13)?,
                            row.get::<_, Option<String>>(14)?,
                            row.get::<_, Option<String>>(15)?,
                            row.get::<_, String>(16)?,
                            row.get::<_, String>(17)?,
                            row.get::<_, String>(18)?,
                            from_row_u64(row, 19)?,
                        ))
                    },
                )
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            rows.into_iter()
                .map(
                    |(
                        occurrence_id,
                        automation_id,
                        automation_revision,
                        routine_id,
                        routine_revision,
                        trigger_id,
                        trigger_host_runtime_id,
                        occurrence_key,
                        claim_epoch,
                        claim_expires_at,
                        status,
                        task_id,
                        scheduled_for,
                        covered_misfire_range_json,
                        trigger_input_ref_json,
                        trigger_payload_digest,
                        blockers_json,
                        created_at,
                        updated_at,
                        version,
                    )| {
                        let parsed = OccurrenceStatus::parse(&status).ok_or_else(|| {
                            StoreError::Integrity(format!(
                                "stored occurrence status {status} is not a known status"
                            ))
                        })?;
                        Ok(OccurrencePageItem {
                            occurrence_id,
                            automation_id,
                            automation_revision,
                            routine_id,
                            routine_revision,
                            trigger_id,
                            trigger_host_runtime_id,
                            occurrence_key,
                            version,
                            claim_epoch,
                            claim_expires_at,
                            status: parsed.as_str().to_owned(),
                            task_id,
                            scheduled_for,
                            covered_misfire_range: parse_json_column(covered_misfire_range_json)?,
                            trigger_input_ref: parse_json_column(trigger_input_ref_json)?,
                            trigger_payload_digest,
                            blockers: parse_json_column(Some(blockers_json))?
                                .and_then(|value| value.as_array().cloned())
                                .unwrap_or_default(),
                            created_at,
                            updated_at,
                        })
                    },
                )
                .collect()
        })
    }

    /// Reads the persisted facts a health projection needs.
    ///
    /// The projection is read-only. It never repairs, never re-enables, and
    /// never advances a cursor.
    pub fn health_facts(
        &self,
        workspace_id: &str,
        automation_id: &str,
    ) -> Result<Option<AutomationHealthFacts>, StoreError> {
        let (workspace_id, automation_id) = (workspace_id.to_owned(), automation_id.to_owned());
        self.run(move |connection| {
            let definition: Option<(u64, String, u64, u64, String)> = connection
                .query_row(
                    "SELECT a.current_revision, a.status, a.version, ar.routine_revision, ar.routine_id
                       FROM automations a
                       JOIN automation_revisions ar
                         ON ar.workspace_id = a.workspace_id
                        AND ar.automation_id = a.automation_id
                        AND ar.revision = a.current_revision
                      WHERE a.workspace_id = ?1 AND a.automation_id = ?2",
                    params![workspace_id, automation_id],
                    |row| {
                        Ok((
                            from_row_u64(row, 0)?,
                            row.get::<_, String>(1)?,
                            from_row_u64(row, 2)?,
                            from_row_u64(row, 3)?,
                            row.get::<_, String>(4)?,
                        ))
                    },
                )
                .optional()
                .map_err(map_database_error)?;
            let Some((current_revision, status, version, routine_revision, routine_id)) = definition
            else {
                return Ok(None);
            };
            let routine: Option<(u64, String)> = connection
                .query_row(
                    "SELECT current_revision, status FROM routines WHERE workspace_id = ?1 AND routine_id = ?2",
                    params![workspace_id, routine_id],
                    |row| Ok((from_row_u64(row, 0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            let mut counts = domain_responsibility::OccurrenceCounts::default();
            let mut statement = connection
                .prepare(
                    "SELECT status, COUNT(*) FROM automation_occurrences
                      WHERE workspace_id = ?1 AND automation_id = ?2 GROUP BY status",
                )
                .map_err(map_database_error)?;
            let grouped = statement
                .query_map(params![workspace_id, automation_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            for (value, count) in grouped {
                let count = u32::try_from(count).unwrap_or(u32::MAX);
                match OccurrenceStatus::parse(&value) {
                    Some(OccurrenceStatus::Pending) => counts.pending = count,
                    Some(OccurrenceStatus::Claimed) => counts.claimed = count,
                    Some(OccurrenceStatus::WaitingDependency) => counts.waiting_dependency = count,
                    Some(OccurrenceStatus::Started) => counts.started = count,
                    Some(OccurrenceStatus::Completed) => counts.completed = count,
                    Some(OccurrenceStatus::Skipped) => counts.skipped = count,
                    Some(OccurrenceStatus::Failed) => counts.failed = count,
                    None => {
                        return Err(StoreError::Integrity(format!(
                            "stored occurrence status {value} is not a known status"
                        )));
                    }
                }
            }
            let host_ready: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM runtime_workspace_bindings b
                                    WHERE b.workspace_id = ?1 AND b.status = 'ACTIVE'
                                      AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'TRIGGER_HOST'))",
                    params![workspace_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            let cursor_deadlines: Vec<(Option<String>, Option<String>)> = {
                let mut cursor_statement = connection
                    .prepare(
                        "SELECT next_scheduled_at, last_checked_at FROM automation_cursors
                          WHERE workspace_id = ?1 AND automation_id = ?2",
                    )
                    .map_err(map_database_error)?;
                cursor_statement
                    .query_map(params![workspace_id, automation_id], |row| {
                        Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?))
                    })
                    .map_err(map_database_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(map_database_error)?
            };
            let next_occurrence_at = cursor_deadlines
                .iter()
                .filter_map(|(next, _)| next.as_deref().and_then(parse_instant))
                .min();
            let last_occurrence_at = cursor_deadlines
                .iter()
                .filter_map(|(_, checked)| checked.as_deref().and_then(parse_instant))
                .max();
            let mut task_statement = connection
                .prepare(
                    "SELECT status, created_at, completed_at FROM tasks
                      WHERE workspace_id = ?1 AND routine_id = ?2 AND routine_revision = ?3
                        AND status IN ('COMPLETED', 'FAILED', 'INCOMPLETE', 'CANCELLED')
                      ORDER BY created_at DESC, task_id DESC LIMIT 10",
                )
                .map_err(map_database_error)?;
            let routine_tasks = task_statement
                .query_map(
                    params![workspace_id, routine_id, to_sql_i64(routine_revision, "Routine revision")?],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            let nonterminal = counts.pending
                + counts.claimed
                + counts.waiting_dependency
                + counts.started;
            Ok(Some(AutomationHealthFacts {
                automation_id,
                workspace_id,
                current_revision,
                status,
                version,
                routine_id,
                routine_revision,
                routine_current_revision: routine.as_ref().map(|value| value.0).unwrap_or(0),
                routine_status: routine
                    .as_ref()
                    .map(|value| value.1.clone())
                    .unwrap_or_else(|| "UNKNOWN".to_owned()),
                host_ready,
                counts,
                nonterminal_occurrences: nonterminal,
                next_occurrence_at,
                last_occurrence_at,
                routine_tasks: routine_tasks
                    .into_iter()
                    .map(|(status, created_at, completed_at)| RoutineTaskOutcome {
                        status,
                        created_at,
                        completed_at,
                    })
                    .collect(),
            }))
        })
    }
}

/// One Workspace this Runtime may host triggers for.
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerHostWorkspace {
    pub workspace_id: String,
    pub current_instruction_revision: Option<u64>,
    pub default_agent_binding_id: Option<String>,
}

/// One persisted occurrence in an Automation's run history.
#[derive(Clone, Debug, PartialEq)]
pub struct OccurrencePageItem {
    pub occurrence_id: String,
    pub automation_id: String,
    pub automation_revision: u64,
    pub routine_id: String,
    pub routine_revision: u64,
    pub trigger_id: String,
    pub trigger_host_runtime_id: String,
    pub occurrence_key: String,
    pub version: u64,
    pub claim_epoch: u64,
    pub claim_expires_at: Option<String>,
    pub status: String,
    pub task_id: Option<String>,
    pub scheduled_for: Option<String>,
    pub covered_misfire_range: Option<serde_json::Value>,
    pub trigger_input_ref: Option<serde_json::Value>,
    pub trigger_payload_digest: Option<String>,
    pub blockers: Vec<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// Parses one optional JSON column of a persisted row.
fn parse_json_column(value: Option<String>) -> Result<Option<serde_json::Value>, StoreError> {
    value
        .map(|text| {
            serde_json::from_str(&text).map_err(|error| {
                StoreError::Integrity(format!("stored JSON column is invalid: {error}"))
            })
        })
        .transpose()
}

/// The persisted facts a read-only health projection reads.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomationHealthFacts {
    pub automation_id: String,
    pub workspace_id: String,
    pub current_revision: u64,
    pub status: String,
    pub version: u64,
    pub routine_id: String,
    pub routine_revision: u64,
    pub routine_current_revision: u64,
    pub routine_status: String,
    pub host_ready: bool,
    pub counts: domain_responsibility::OccurrenceCounts,
    pub nonterminal_occurrences: u32,
    /// The earliest persisted slot deadline, in Unix epoch milliseconds.
    pub next_occurrence_at: Option<i64>,
    /// The latest persisted cursor observation, in Unix epoch milliseconds.
    pub last_occurrence_at: Option<i64>,
    pub routine_tasks: Vec<RoutineTaskOutcome>,
}

/// Writes one AutomationOccurrence domain event inside the caller's transaction.
#[allow(clippy::too_many_arguments)]
fn write_occurrence_event(
    tx: &Transaction<'_>,
    workspace_id: &str,
    origin_runtime_id: &str,
    occurrence_id: &str,
    entity_revision: u64,
    state_blob: &BlobRef,
    event_type: &str,
    payload: &Value,
    recorded_at: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
        params![workspace_id, origin_runtime_id],
    )
    .map_err(map_database_error)?;
    let sequence: i64 = tx
        .query_row(
            "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
            params![workspace_id, origin_runtime_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let payload_bytes = canonical_json(payload)?;
    let payload_text = String::from_utf8(payload_bytes.clone())
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let state_ref = AggregateStateRef {
        blob: state_blob.clone(),
        entity_revision,
        record_schema_version: 1,
    };
    let state_text = String::from_utf8(canonical_json(&state_ref)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event_id = format!("{origin_runtime_id}:occ:{occurrence_id}:{entity_revision}");
    tx.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest)
         VALUES (?1, ?2, 'AutomationOccurrence', ?3, ?4, ?5, ?6, ?7, ?8, NULL, 1, ?9, ?10, ?11, ?12, ?13)",
        params![
            event_id,
            workspace_id,
            occurrence_id,
            origin_runtime_id,
            sequence,
            to_sql_i64(entity_revision, "AutomationOccurrence revision")?,
            recorded_at,
            recorded_at,
            event_type,
            payload_text,
            state_text,
            recorded_at,
            digest(&payload_bytes),
        ],
    )
    .map_err(map_database_error)?;
    Ok(())
}

/// Builds the executable schedule for a schedule trigger, or `None` when the
/// trigger is not a schedule.
pub fn schedule_trigger(trigger: &TriggerSpec) -> Option<ScheduleTrigger> {
    let TriggerDefinition::Schedule {
        recurrence_format,
        timezone,
        rrule_or_cron,
        misfire_policy,
        ambiguous_local_time,
        nonexistent_local_time,
        start_at,
        end_at,
        ..
    } = &trigger.trigger
    else {
        return None;
    };
    let start_at = start_at.as_deref().and_then(|value| parse_instant(value));
    let end_at = end_at.as_deref().and_then(|value| parse_instant(value));
    ScheduleTrigger::parse(
        *recurrence_format,
        timezone,
        rrule_or_cron,
        misfire_policy.clone(),
        *ambiguous_local_time,
        *nonexistent_local_time,
        start_at,
        end_at,
    )
    .ok()
}

/// The stable occurrence identity for a scheduled slot.
pub fn schedule_occurrence_key(
    trigger_id: &str,
    scheduled_epoch_ms: i64,
) -> Result<String, StoreError> {
    OccurrenceIdentity::Schedule {
        trigger_id: trigger_id.to_owned(),
        scheduled_epoch_ms,
    }
    .key()
    .map_err(|error| StoreError::Invalid(error.to_string()))
}

/// The settlement window a host uses for one poll.
pub fn settlement_window(now: i64, grace_ms: i64) -> SettlementWindow {
    SettlementWindow { now, grace_ms }
}

/// The misfire policy a stored trigger declares.
pub fn trigger_misfire_policy(trigger: &TriggerSpec) -> Option<MisfirePolicy> {
    match &trigger.trigger {
        TriggerDefinition::Schedule { misfire_policy, .. } => Some(misfire_policy.clone()),
        TriggerDefinition::OneShot { misfire_policy, .. } => Some(misfire_policy.clone()),
        _ => None,
    }
}

/// A status token that an occurrence can hold.
pub fn occurrence_status_token(status: OccurrenceStatus) -> &'static str {
    status.as_str()
}

/// The status of a stored Automation definition.
pub fn automation_status_token(status: AutomationStatus) -> &'static str {
    match status {
        AutomationStatus::Enabled => "ENABLED",
        AutomationStatus::Paused => "PAUSED",
        AutomationStatus::Disabled => "DISABLED",
    }
}
