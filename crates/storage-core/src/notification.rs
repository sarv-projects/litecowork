//! Notification delivery history and preference projections.
//!
//! Authority: [`docs/DATA-MODEL.md`](../../../docs/DATA-MODEL.md) "Notifications",
//! [`docs/API.md`](../../../docs/API.md) "User requests and notifications", and
//! the `Notification` / `NotificationPreference` OpenAPI components.
//!
//! These are read projections of the canonical `notification_deliveries` and
//! `notification_preferences` tables. Whether a notification is due is decided
//! by the domain `NotificationService`; this module only carries what an
//! authenticated owner read may return.
//!
//! `SENT` is transport acknowledgement, never Task completion. No field here
//! claims that a Task succeeded.

use serde::{Deserialize, Serialize};

/// One durable delivery attempt record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationRecord {
    pub delivery_id: String,
    pub workspace_id: String,
    pub dedupe_key: String,
    pub source_event_id: String,
    pub channel_binding_id: Option<String>,
    pub attempt_runtime_id: Option<String>,
    pub attempt_host_epoch: Option<u64>,
    pub status: String,
    pub attempt_count: u32,
    pub next_attempt_at: Option<String>,
    pub last_error_code: Option<String>,
    pub created_at: String,
    pub settled_at: Option<String>,
}

/// Owner-configured quiet window for one event class.
///
/// The persisted `quiet_hours_json` column is validated against this shape when
/// read. An unreadable value is a storage integrity failure, not a silent
/// `None`, because dropping quiet hours would change delivery behavior.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuietHours {
    pub timezone: String,
    pub weekdays: Vec<String>,
    pub start_local_time: String,
    pub end_local_time: String,
}

/// One persisted event-class notification policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationPreferenceRecord {
    pub workspace_id: String,
    pub event_class: String,
    pub policy: String,
    pub preferred_channels: Vec<String>,
    pub quiet_hours: Option<QuietHours>,
    pub version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delivery_record_round_trips_through_its_contract_shape() {
        let record = NotificationRecord {
            delivery_id: "delivery-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            dedupe_key: "occurrence:1:TASK_COMPLETED".to_owned(),
            source_event_id: "event-1".to_owned(),
            channel_binding_id: None,
            attempt_runtime_id: None,
            attempt_host_epoch: None,
            status: "PENDING".to_owned(),
            attempt_count: 0,
            next_attempt_at: None,
            last_error_code: None,
            created_at: "2026-10-21T09:00:00Z".to_owned(),
            settled_at: None,
        };
        let encoded = serde_json::to_value(&record).expect("serializes");
        assert_eq!(encoded["status"], "PENDING");
        assert_eq!(encoded["attempt_count"], 0);
        let decoded: NotificationRecord = serde_json::from_value(encoded).expect("deserializes");
        assert_eq!(decoded, record);
    }

    #[test]
    fn an_unknown_delivery_field_is_rejected() {
        let encoded = serde_json::json!({
            "delivery_id": "delivery-1",
            "workspace_id": "workspace-1",
            "dedupe_key": "dedupe",
            "source_event_id": "event-1",
            "channel_binding_id": null,
            "attempt_runtime_id": null,
            "attempt_host_epoch": null,
            "status": "SENT",
            "attempt_count": 1,
            "next_attempt_at": null,
            "last_error_code": null,
            "created_at": "2026-10-21T09:00:00Z",
            "settled_at": null,
            "task_completed": true
        });
        assert!(serde_json::from_value::<NotificationRecord>(encoded).is_err());
    }

    #[test]
    fn quiet_hours_reject_an_unknown_field() {
        let encoded = serde_json::json!({
            "timezone": "Europe/Berlin",
            "weekdays": ["MONDAY"],
            "start_local_time": "22:00",
            "end_local_time": "07:00",
            "enabled": true
        });
        assert!(serde_json::from_value::<QuietHours>(encoded).is_err());
    }

    #[test]
    fn a_preference_keeps_an_absent_quiet_window_absent() {
        let preference = NotificationPreferenceRecord {
            workspace_id: "workspace-1".to_owned(),
            event_class: "TASK_FAILED".to_owned(),
            policy: "ON_FAILURE".to_owned(),
            preferred_channels: vec!["channel-1".to_owned()],
            quiet_hours: None,
            version: 3,
        };
        let encoded = serde_json::to_value(&preference).expect("serializes");
        assert_eq!(encoded["quiet_hours"], serde_json::Value::Null);
        assert_eq!(encoded["preferred_channels"][0], "channel-1");
    }
}
