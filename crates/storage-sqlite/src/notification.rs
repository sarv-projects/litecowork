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
use crate::workspace_access::require_read_access;
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
                let preferred_channels: Vec<String> =
                    serde_json::from_str(&channels_json).map_err(|_| {
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
}
