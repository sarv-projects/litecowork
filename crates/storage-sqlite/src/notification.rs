//! Owner-scoped reads of the canonical notification tables.
//!
//! Authority: [`docs/DATA-MODEL.md`](../../../docs/DATA-MODEL.md) "Notifications",
//! [`docs/API.md`](../../../docs/API.md) "User requests and notifications", and
//! the `Notification` / `NotificationPreference` OpenAPI components.
//!
//! The tables already exist in [`docs/schemas/sqlite-v1.sql`](../../../docs/schemas/sqlite-v1.sql);
//! this module adds no schema. Reads use [`require_read_access`] so an ARCHIVED
//! Workspace stays readable, while a caller who does not own it receives
//! `NOT_FOUND` rather than a distinction between "missing" and "archived".
//!
//! Keyset pagination is ascending on `(created_at, delivery_id)`. `created_at`
//! alone is not unique, so a page boundary that used only the timestamp could
//! repeat or skip a delivery whose timestamp ties with the last row.

use super::*;
use crate::workspace_access::{require_read_access, require_write_access};
use storage_core::{NotificationPreferenceRecord, NotificationRecord, QuietHours};

/// The OpenAPI `Limit` maximum is 200. The route asks for one extra row to
/// decide whether a `next_cursor` exists, so the storage port accepts 201.
const MAX_PAGE: usize = 201;

const DELIVERY_STATUSES: [&str; 6] = [
    "PENDING",
    "SENDING",
    "SENT",
    "FAILED",
    "AMBIGUOUS",
    "SUPPRESSED",
];

fn run<T: Send + 'static>(
    store: &SqliteWorkspaceStore,
    operation: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, StoreError> {
    let (send, recv) = mpsc::channel();
    store.execute_command(
        Command::Operation {
            operation: Box::new(move |connection| {
                let _ = send.send(operation(connection));
            }),
        },
        recv,
    )
}

fn invalid_identifier(value: &str, max_len: usize) -> bool {
    value.trim().is_empty() || value.len() > max_len || value.chars().any(char::is_control)
}

impl SqliteWorkspaceStore {
    /// Reads one owner-scoped page of delivery history.
    ///
    /// `status` is the exact persisted token. The cursor pair is all-or-nothing:
    /// a caller cannot resume from a timestamp alone, because that would be
    /// ambiguous when two deliveries share the same second.
    pub fn list_notification_deliveries(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        status: Option<&str>,
        after_created_at: Option<&str>,
        after_delivery_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<NotificationRecord>, StoreError> {
        if !(1..=MAX_PAGE).contains(&limit)
            || invalid_identifier(owner_principal_id, 256)
            || invalid_identifier(workspace_id, 256)
            || status.is_some_and(|value| !DELIVERY_STATUSES.contains(&value))
            || after_created_at.is_some() != after_delivery_id.is_some()
            || after_created_at.is_some_and(|value| invalid_identifier(value, 64))
            || after_delivery_id.is_some_and(|value| invalid_identifier(value, 256))
        {
            return Err(StoreError::Invalid(
                "Notification page query is invalid".to_owned(),
            ));
        }
        let principal = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let status = status.map(str::to_owned);
        let after = after_created_at
            .zip(after_delivery_id)
            .map(|(created_at, delivery_id)| (created_at.to_owned(), delivery_id.to_owned()));
        run(self, move |connection| {
            require_read_access(connection, &workspace, &principal)?;
            let after_created_at = after.as_ref().map(|(created_at, _)| created_at);
            let after_delivery_id = after.as_ref().map(|(_, delivery_id)| delivery_id);
            let mut statement = connection
                .prepare(
                    "SELECT delivery_id, workspace_id, dedupe_key, source_event_id, \
                            channel_binding_id, attempt_runtime_id, attempt_host_epoch, status, \
                            attempt_count, next_attempt_at, last_error_code, created_at, settled_at \
                     FROM notification_deliveries \
                     WHERE workspace_id = ?1 \
                       AND (?2 IS NULL OR status = ?2) \
                       AND (?3 IS NULL OR created_at > ?3 \
                            OR (created_at = ?3 AND delivery_id > ?4)) \
                     ORDER BY created_at, delivery_id \
                     LIMIT ?5",
                )
                .map_err(map_database_error)?;
            let mut rows = statement
                .query(params![
                    workspace,
                    status,
                    after_created_at,
                    after_delivery_id,
                    limit as i64
                ])
                .map_err(map_database_error)?;
            let mut deliveries = Vec::new();
            while let Some(row) = rows.next().map_err(map_database_error)? {
                let attempt_host_epoch = row
                    .get::<_, Option<i64>>(6)
                    .map_err(map_database_error)?
                    .map(|value| {
                        u64::try_from(value).map_err(|_| {
                            StoreError::Integrity(
                                "notification delivery host epoch is not positive".to_owned(),
                            )
                        })
                    })
                    .transpose()?;
                let attempt_count = row
                    .get::<_, i64>(8)
                    .map_err(map_database_error)?
                    .try_into()
                    .map_err(|_| {
                        StoreError::Integrity(
                            "notification delivery attempt count is not positive".to_owned(),
                        )
                    })?;
                deliveries.push(NotificationRecord {
                    delivery_id: row.get(0).map_err(map_database_error)?,
                    workspace_id: row.get(1).map_err(map_database_error)?,
                    dedupe_key: row.get(2).map_err(map_database_error)?,
                    source_event_id: row.get(3).map_err(map_database_error)?,
                    channel_binding_id: row.get(4).map_err(map_database_error)?,
                    attempt_runtime_id: row.get(5).map_err(map_database_error)?,
                    attempt_host_epoch,
                    status: row.get(7).map_err(map_database_error)?,
                    attempt_count,
                    next_attempt_at: row.get(9).map_err(map_database_error)?,
                    last_error_code: row.get(10).map_err(map_database_error)?,
                    created_at: row.get(11).map_err(map_database_error)?,
                    settled_at: row.get(12).map_err(map_database_error)?,
                });
            }
            Ok(deliveries)
        })
    }

    /// Reads every persisted policy for one owner-scoped Workspace.
    ///
    /// No default row is invented: an event class with no persisted preference
    /// means the owner never configured one, and the notification service keeps
    /// its own documented default. Returning a synthetic row here would make the
    /// route disagree with the table.
    pub fn list_notification_preferences(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<NotificationPreferenceRecord>, StoreError> {
        if invalid_identifier(owner_principal_id, 256) || invalid_identifier(workspace_id, 256) {
            return Err(StoreError::Invalid(
                "Notification preference query is invalid".to_owned(),
            ));
        }
        let principal = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        run(self, move |connection| {
            require_read_access(connection, &workspace, &principal)?;
            let mut statement = connection
                .prepare(
                    "SELECT workspace_id, event_class, policy, preferred_channels_json, \
                            quiet_hours_json, version \
                     FROM notification_preferences \
                     WHERE workspace_id = ?1 \
                     ORDER BY event_class",
                )
                .map_err(map_database_error)?;
            let mut rows = statement
                .query(params![workspace])
                .map_err(map_database_error)?;
            let mut preferences = Vec::new();
            while let Some(row) = rows.next().map_err(map_database_error)? {
                let channels_json: String = row.get(3).map_err(map_database_error)?;
                let preferred_channels: Vec<String> = serde_json::from_str(&channels_json)
                    .map_err(|_| {
                        StoreError::Integrity(
                            "notification preference channels are unreadable".to_owned(),
                        )
                    })?;
                let quiet_hours = row
                    .get::<_, Option<String>>(4)
                    .map_err(map_database_error)?
                    .map(|value| {
                        serde_json::from_str::<QuietHours>(&value).map_err(|_| {
                            StoreError::Integrity(
                                "notification quiet hours are unreadable".to_owned(),
                            )
                        })
                    })
                    .transpose()?;
                let version = row
                    .get::<_, i64>(5)
                    .map_err(map_database_error)?
                    .try_into()
                    .map_err(|_| {
                        StoreError::Integrity(
                            "notification preference version is not positive".to_owned(),
                        )
                    })?;
                preferences.push(NotificationPreferenceRecord {
                    workspace_id: row.get(0).map_err(map_database_error)?,
                    event_class: row.get(1).map_err(map_database_error)?,
                    policy: row.get(2).map_err(map_database_error)?,
                    preferred_channels,
                    quiet_hours,
                    version,
                });
            }
            Ok(preferences)
        })
    }

    /// Writes one versioned event-class policy and its
    /// `notification.preference.changed.v1` event atomically.
    ///
    /// `expected_version` is `0` for the first write. A replay with the same
    /// `Idempotency-Key` and the same canonical request returns the committed
    /// record; reusing the key for a different request is
    /// [`StoreError::IdempotencyConflict`]. A different version is
    /// [`StoreError::Conflict`]. An ARCHIVED Workspace is refused with
    /// `WORKSPACE_ARCHIVED`, because this is a domain mutation.
    pub fn set_notification_preference(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        event_class: &str,
        policy: &str,
        preferred_channels: &[String],
        quiet_hours: Option<&QuietHours>,
        expected_version: u64,
        request_id: &str,
        context: &NotificationEventContext,
    ) -> Result<NotificationPreferenceRecord, StoreError> {
        if invalid_identifier(owner_principal_id, 256)
            || invalid_identifier(workspace_id, 256)
            || invalid_identifier(event_class, 128)
            || !NOTIFICATION_POLICIES.contains(&policy)
            || preferred_channels.len() > 64
            || preferred_channels
                .iter()
                .any(|channel| invalid_identifier(channel, 256))
            || quiet_hours.is_some_and(|quiet| !valid_quiet_hours(quiet))
            || request_id.is_empty()
            || request_id.len() > 200
            || context.event_id.is_empty()
            || context.origin_runtime_id.is_empty()
            || context.correlation_id.is_empty()
            || context.recorded_at.is_empty()
        {
            return Err(StoreError::Invalid(
                "Notification preference request is invalid".to_owned(),
            ));
        }
        let version = expected_version.checked_add(1).ok_or_else(|| {
            StoreError::Invalid("Notification preference version overflowed".to_owned())
        })?;
        let record = NotificationPreferenceRecord {
            workspace_id: workspace_id.to_owned(),
            event_class: event_class.to_owned(),
            policy: policy.to_owned(),
            preferred_channels: preferred_channels.to_vec(),
            quiet_hours: quiet_hours.cloned(),
            version,
        };
        let state_bytes = canonical_json(&record)?;
        let blob = self.inner.blobs.put(
            workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            NOTIFICATION_PREFERENCE_MEDIA_TYPE,
        )?;
        if blob.size_bytes != state_bytes.len() as u64
            || self
                .inner
                .blobs
                .get(workspace_id, BlobPurpose::AggregateState, &blob)?
                != state_bytes
        {
            return Err(StoreError::Integrity(
                "notification preference aggregate state failed BlobStore verification".to_owned(),
            ));
        }
        let fingerprint = digest(&canonical_json(&json!({
            "workspace_id": workspace_id,
            "event_class": event_class,
            "policy": policy,
            "preferred_channels": preferred_channels,
            "quiet_hours": quiet_hours,
            "expected_version": expected_version,
        }))?);
        let principal = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let event_class = event_class.to_owned();
        let request_id = request_id.to_owned();
        let context = context.clone();
        run(self, move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            require_write_access(&tx, &workspace, &principal)?;
            if let Some(prior) = preference_receipt(
                &tx,
                &principal,
                &request_id,
                &fingerprint,
                &workspace,
                &event_class,
            )? {
                return Ok(prior);
            }
            let current: Option<i64> = tx
                .query_row(
                    "SELECT version FROM notification_preferences \
                     WHERE workspace_id = ?1 AND event_class = ?2",
                    params![workspace, event_class],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            let current = current.unwrap_or(0);
            if u64::try_from(current).ok() != Some(expected_version) {
                return Err(StoreError::Conflict {
                    expected: Some(expected_version),
                    actual: u64::try_from(current).ok(),
                });
            }
            let channels_json = encode(&record.preferred_channels)?;
            let quiet_hours_json = record.quiet_hours.as_ref().map(encode).transpose()?;
            tx.execute(
                "INSERT INTO notification_preferences(workspace_id, event_class, policy, \
                     preferred_channels_json, quiet_hours_json, version) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(workspace_id, event_class) DO UPDATE SET \
                     policy = excluded.policy, \
                     preferred_channels_json = excluded.preferred_channels_json, \
                     quiet_hours_json = excluded.quiet_hours_json, \
                     version = excluded.version",
                params![
                    workspace,
                    event_class,
                    record.policy,
                    channels_json,
                    quiet_hours_json,
                    i64::try_from(version).map_err(|_| StoreError::Integrity(
                        "notification preference version does not fit SQLite".to_owned()
                    ))?
                ],
            )
            .map_err(map_database_error)?;
            write_preference_event(&tx, &workspace, &context, &record, blob)?;
            let response = encode(&record)?;
            tx.execute(
                "INSERT INTO request_dedup(principal_id, request_id, request_digest, \
                     response_json, response_digest, created_at, expires_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                params![
                    principal,
                    request_id,
                    fingerprint,
                    response,
                    digest(response.as_bytes()),
                    context.recorded_at
                ],
            )
            .map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            Ok(record)
        })
    }
}

/// Event context supplied by the Operator caller.
///
/// Kept separate from the domain service context so the storage port does not
/// depend on a feature crate's private event shape.
#[derive(Clone, Debug)]
pub struct NotificationEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

fn preference_receipt(
    tx: &Transaction<'_>,
    owner: &str,
    request_id: &str,
    fingerprint: &str,
    workspace: &str,
    event_class: &str,
) -> Result<Option<NotificationPreferenceRecord>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest, response_json, response_digest FROM request_dedup \
             WHERE principal_id = ?1 AND request_id = ?2",
            params![owner, request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((actual, response, response_digest)) = prior else {
        return Ok(None);
    };
    if actual != fingerprint {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response.ok_or_else(|| {
        StoreError::Integrity("notification preference receipt has no response".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "notification preference receipt digest does not match its response".to_owned(),
        ));
    }
    let preference: NotificationPreferenceRecord =
        serde_json::from_str(&response).map_err(|_| {
            StoreError::Integrity(
                "notification preference receipt response is unreadable".to_owned(),
            )
        })?;
    if preference.workspace_id != workspace || preference.event_class != event_class {
        return Err(StoreError::Integrity(
            "notification preference receipt belongs to another aggregate".to_owned(),
        ));
    }
    Ok(Some(preference))
}

fn write_preference_event(
    tx: &Transaction<'_>,
    workspace: &str,
    context: &NotificationEventContext,
    record: &NotificationPreferenceRecord,
    blob: BlobRef,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) \
         VALUES (?1, ?2, 1) \
         ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
        params![workspace, context.origin_runtime_id],
    )
    .map_err(map_database_error)?;
    let sequence: i64 = tx
        .query_row(
            "SELECT last_sequence FROM workspace_origin_sequences \
             WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
            params![workspace, context.origin_runtime_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let payload = json!({
        "workspace_id": record.workspace_id,
        "event_class": record.event_class,
        "policy": record.policy,
        "preferred_channels": record.preferred_channels,
        "aggregate_version": record.version,
    });
    let payload_json = encode(&payload)?;
    let state_ref = AggregateStateRef {
        blob,
        entity_revision: record.version,
        record_schema_version: 1,
    };
    tx.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, \
             origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, \
             correlation_id, causation_id, schema_version, type, payload_json, \
             aggregate_state_ref_json, recorded_at, payload_digest) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12, ?13, ?14, ?15)",
        params![
            context.event_id,
            workspace,
            "NotificationPreference",
            record.event_class,
            context.origin_runtime_id,
            sequence,
            i64::try_from(record.version).map_err(|_| StoreError::Integrity(
                "notification preference event revision does not fit SQLite".to_owned()
            ))?,
            context.hlc_timestamp,
            context.correlation_id,
            context.causation_id,
            NOTIFICATION_PREFERENCE_EVENT_TYPE,
            payload_json,
            encode(&state_ref)?,
            context.recorded_at,
            digest(payload_json.as_bytes()),
        ],
    )
    .map_err(map_database_error)?;
    Ok(())
}

const NOTIFICATION_POLICIES: [&str; 5] = [
    "ALWAYS",
    "ON_SUCCESS",
    "ON_FAILURE",
    "ON_CONDITION",
    "SILENT",
];

const NOTIFICATION_PREFERENCE_MEDIA_TYPE: &str =
    "application/vnd.litecow.notification-preference+json";

const NOTIFICATION_PREFERENCE_EVENT_TYPE: &str = "notification.preference.changed.v1";

const WEEKDAYS: [&str; 7] = [
    "MONDAY",
    "TUESDAY",
    "WEDNESDAY",
    "THURSDAY",
    "FRIDAY",
    "SATURDAY",
    "SUNDAY",
];

fn valid_quiet_hours(quiet: &QuietHours) -> bool {
    let mut seen = std::collections::HashSet::new();
    !quiet.timezone.trim().is_empty()
        && quiet.timezone.len() <= 64
        && !quiet.timezone.chars().any(char::is_control)
        && !quiet.weekdays.is_empty()
        && quiet.weekdays.len() <= WEEKDAYS.len()
        && quiet
            .weekdays
            .iter()
            .all(|day| WEEKDAYS.contains(&day.as_str()) && seen.insert(day))
        && valid_local_time(&quiet.start_local_time)
        && valid_local_time(&quiet.end_local_time)
}

fn valid_local_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.is_ascii()
        && bytes.len() == 5
        && bytes[2] == b':'
        && bytes[1].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
        && value[0..2].parse::<u8>().is_ok_and(|hour| hour < 24)
        && value[3..5].parse::<u8>().is_ok_and(|minute| minute < 60)
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, StoreError> {
    let bytes = canonical_json(value)?;
    String::from_utf8(bytes)
        .map_err(|_| StoreError::Integrity("notification preference is not UTF-8".to_owned()))
}
