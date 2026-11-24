//! Bounded, same-incarnation recovery from OpenCode v2.0.32's durable session log.
//!
//! This consumes provider observations only after StopUnconfirmed. It never
//! persists provider events, authorizes tools, or resends the prompt.

use std::collections::HashSet;

use serde_json::Value;

use super::adapter::{TurnFailure, TurnObservation};

const MAX_LOG_EVENTS: usize = 4096;
const MAX_LOG_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ASSISTANT_IDS: usize = 256;
const MAX_TEXT_PARTS: usize = 1024;
const MAX_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryError {
    InvalidEvidence,
    LimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Terminal {
    Succeeded,
    Failed,
    Stopped,
    Shutdown,
}

pub(crate) struct SessionLogRecovery {
    session_id: String,
    message_id: String,
    event_count: usize,
    log_bytes: usize,
    last_sequence: Option<u64>,
    synced: bool,
    watermark: Option<u64>,
    enqueued: bool,
    delivered: bool,
    execution_started: bool,
    terminal: Option<Terminal>,
    assistant_ids: HashSet<String>,
    text_part_ids: HashSet<(String, u64)>,
    text_parts: Vec<String>,
    output_bytes: usize,
    invalid_identity: bool,
}

impl SessionLogRecovery {
    pub(crate) fn new(session_id: impl Into<String>, message_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        let message_id = message_id.into();
        let invalid_identity = !valid_id(&session_id) || !valid_id(&message_id);
        Self {
            session_id,
            message_id,
            event_count: 0,
            log_bytes: 0,
            last_sequence: None,
            synced: false,
            watermark: None,
            enqueued: false,
            delivered: false,
            execution_started: false,
            terminal: None,
            assistant_ids: HashSet::new(),
            text_part_ids: HashSet::new(),
            text_parts: Vec::new(),
            output_bytes: 0,
            invalid_identity,
        }
    }

    /// Accept one decoded event from `/api/experimental/session/{id}/log`.
    /// Returns a result only after both a valid terminal observation and the
    /// replay watermark are present. The caller must bound the live request.
    pub(crate) fn observe(
        &mut self,
        event: &Value,
    ) -> Result<Option<TurnObservation>, RecoveryError> {
        if self.invalid_identity {
            return Err(RecoveryError::InvalidEvidence);
        }
        self.event_count = self
            .event_count
            .checked_add(1)
            .ok_or(RecoveryError::LimitExceeded)?;
        if self.event_count > MAX_LOG_EVENTS {
            return Err(RecoveryError::LimitExceeded);
        }
        let encoded_len = serde_json::to_vec(event)
            .map_err(|_| RecoveryError::InvalidEvidence)?
            .len();
        self.log_bytes = self
            .log_bytes
            .checked_add(encoded_len)
            .ok_or(RecoveryError::LimitExceeded)?;
        if self.log_bytes > MAX_LOG_BYTES {
            return Err(RecoveryError::LimitExceeded);
        }

        match event.get("type").and_then(Value::as_str) {
            Some("log.synced") => self.observe_sync(event)?,
            Some(_) => self.observe_durable(event)?,
            None => return Err(RecoveryError::InvalidEvidence),
        }

        Ok(self.result())
    }

    fn observe_sync(&mut self, event: &Value) -> Result<(), RecoveryError> {
        if self.synced
            || event.get("aggregateID").and_then(Value::as_str) != Some(self.session_id.as_str())
        {
            return Err(RecoveryError::InvalidEvidence);
        }
        let watermark = match event.get("seq") {
            Some(Value::Number(number)) => {
                Some(number.as_u64().ok_or(RecoveryError::InvalidEvidence)?)
            }
            Some(_) => return Err(RecoveryError::InvalidEvidence),
            None => None,
        };
        if let (Some(last_sequence), Some(watermark)) = (self.last_sequence, watermark) {
            if watermark < last_sequence {
                return Err(RecoveryError::InvalidEvidence);
            }
        }
        if self.last_sequence.is_some() && watermark.is_none() {
            return Err(RecoveryError::InvalidEvidence);
        }
        self.synced = true;
        self.watermark = watermark;
        Ok(())
    }

    fn observe_durable(&mut self, event: &Value) -> Result<(), RecoveryError> {
        let durable = event
            .get("durable")
            .and_then(Value::as_object)
            .ok_or(RecoveryError::InvalidEvidence)?;
        if durable.get("aggregateID").and_then(Value::as_str) != Some(self.session_id.as_str()) {
            return Err(RecoveryError::InvalidEvidence);
        }
        let sequence = durable
            .get("seq")
            .and_then(Value::as_u64)
            .ok_or(RecoveryError::InvalidEvidence)?;
        let version = durable
            .get("version")
            .and_then(Value::as_u64)
            .filter(|version| *version > 0)
            .ok_or(RecoveryError::InvalidEvidence)?;
        if !event
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(valid_event_id)
            || !event
                .get("created")
                .and_then(Value::as_f64)
                .is_some_and(f64::is_finite)
        {
            return Err(RecoveryError::InvalidEvidence);
        }
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or(RecoveryError::InvalidEvidence)?;
        if is_recovery_event(event_type) && version != 1 {
            return Err(RecoveryError::InvalidEvidence);
        }
        if self
            .last_sequence
            .is_some_and(|last_sequence| sequence <= last_sequence)
        {
            return Err(RecoveryError::InvalidEvidence);
        }
        if self.synced {
            if self
                .watermark
                .is_some_and(|watermark| sequence <= watermark)
            {
                return Err(RecoveryError::InvalidEvidence);
            }
        } else if self.watermark.is_some() {
            return Err(RecoveryError::InvalidEvidence);
        }
        self.last_sequence = Some(sequence);

        let data = event
            .get("data")
            .and_then(Value::as_object)
            .ok_or(RecoveryError::InvalidEvidence)?;
        if data.get("sessionID").and_then(Value::as_str) != Some(self.session_id.as_str()) {
            return Err(RecoveryError::InvalidEvidence);
        }

        match event_type {
            "session.inbox.enqueued" => self.observe_enqueued(data)?,
            "session.execution.started" => self.observe_execution_started()?,
            "session.inbox.delivered" => self.observe_delivered(data)?,
            "session.step.started" => self.observe_step_started(data)?,
            "session.text.ended" => self.observe_text_ended(data)?,
            "session.execution.succeeded" => self.observe_terminal(Terminal::Succeeded)?,
            "session.execution.failed" => self.observe_terminal(Terminal::Failed)?,
            "session.execution.interrupted" => self.observe_interrupted(data)?,
            // Other durable session events are not presentation or lifecycle
            // evidence, but their envelope and session identity were checked.
            _ => {}
        }
        Ok(())
    }

    fn observe_enqueued(
        &mut self,
        data: &serde_json::Map<String, Value>,
    ) -> Result<(), RecoveryError> {
        let item = data
            .get("item")
            .and_then(Value::as_object)
            .ok_or(RecoveryError::InvalidEvidence)?;
        if self.enqueued
            || data.get("inboxID").and_then(Value::as_str) != Some(self.message_id.as_str())
            || item.get("type").and_then(Value::as_str) != Some("user")
        {
            return Err(RecoveryError::InvalidEvidence);
        }
        self.enqueued = true;
        Ok(())
    }

    fn observe_execution_started(&mut self) -> Result<(), RecoveryError> {
        if !self.enqueued || self.execution_started || self.terminal.is_some() {
            return Err(RecoveryError::InvalidEvidence);
        }
        self.execution_started = true;
        Ok(())
    }

    fn observe_delivered(
        &mut self,
        data: &serde_json::Map<String, Value>,
    ) -> Result<(), RecoveryError> {
        if !self.enqueued
            || !self.execution_started
            || self.delivered
            || self.terminal.is_some()
            || data.get("inboxID").and_then(Value::as_str) != Some(self.message_id.as_str())
        {
            return Err(RecoveryError::InvalidEvidence);
        }
        self.delivered = true;
        Ok(())
    }

    fn observe_step_started(
        &mut self,
        data: &serde_json::Map<String, Value>,
    ) -> Result<(), RecoveryError> {
        if !self.delivered || self.terminal.is_some() {
            return Err(RecoveryError::InvalidEvidence);
        }
        let assistant_id = data
            .get("assistantMessageID")
            .and_then(Value::as_str)
            .filter(|value| valid_id(value))
            .ok_or(RecoveryError::InvalidEvidence)?;
        if self.assistant_ids.insert(assistant_id.to_owned())
            && self.assistant_ids.len() > MAX_ASSISTANT_IDS
        {
            return Err(RecoveryError::LimitExceeded);
        }
        Ok(())
    }

    fn observe_text_ended(
        &mut self,
        data: &serde_json::Map<String, Value>,
    ) -> Result<(), RecoveryError> {
        if !self.delivered || self.terminal.is_some() {
            return Err(RecoveryError::InvalidEvidence);
        }
        let assistant_id = data
            .get("assistantMessageID")
            .and_then(Value::as_str)
            .filter(|value| self.assistant_ids.contains(*value))
            .ok_or(RecoveryError::InvalidEvidence)?;
        let ordinal = data
            .get("ordinal")
            .and_then(Value::as_u64)
            .ok_or(RecoveryError::InvalidEvidence)?;
        let text = data
            .get("text")
            .and_then(Value::as_str)
            .ok_or(RecoveryError::InvalidEvidence)?;
        if self.text_parts.len() >= MAX_TEXT_PARTS
            || !self
                .text_part_ids
                .insert((assistant_id.to_owned(), ordinal))
        {
            return Err(if self.text_parts.len() >= MAX_TEXT_PARTS {
                RecoveryError::LimitExceeded
            } else {
                RecoveryError::InvalidEvidence
            });
        }
        self.output_bytes = self
            .output_bytes
            .checked_add(text.len())
            .ok_or(RecoveryError::LimitExceeded)?;
        if self.output_bytes > MAX_OUTPUT_BYTES {
            return Err(RecoveryError::LimitExceeded);
        }
        self.text_parts.push(text.to_owned());
        Ok(())
    }

    fn observe_terminal(&mut self, terminal: Terminal) -> Result<(), RecoveryError> {
        if !self.enqueued || !self.delivered || !self.execution_started || self.terminal.is_some() {
            return Err(RecoveryError::InvalidEvidence);
        }
        self.terminal = Some(terminal);
        Ok(())
    }

    fn observe_interrupted(
        &mut self,
        data: &serde_json::Map<String, Value>,
    ) -> Result<(), RecoveryError> {
        let reason = data
            .get("reason")
            .and_then(Value::as_str)
            .ok_or(RecoveryError::InvalidEvidence)?;
        let terminal = match reason {
            "user" | "superseded" | "inactivity" => Terminal::Stopped,
            "shutdown" => Terminal::Shutdown,
            _ => return Err(RecoveryError::InvalidEvidence),
        };
        self.observe_terminal(terminal)
    }

    fn result(&self) -> Option<TurnObservation> {
        if !self.synced {
            return None;
        }
        match self.terminal? {
            Terminal::Succeeded => {
                let answer = self.text_parts.concat();
                if answer.trim().is_empty() {
                    Some(TurnObservation::Failed {
                        reason_code: TurnFailure::EmptyResponse,
                    })
                } else {
                    Some(TurnObservation::Completed {
                        answer_text: answer,
                    })
                }
            }
            Terminal::Failed => Some(TurnObservation::Failed {
                reason_code: TurnFailure::ProviderError,
            }),
            Terminal::Stopped => Some(TurnObservation::Stopped),
            Terminal::Shutdown => None,
        }
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_event_id(value: &str) -> bool {
    value.starts_with("evt_") && valid_id(value)
}

fn is_recovery_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "session.inbox.enqueued"
            | "session.inbox.delivered"
            | "session.execution.started"
            | "session.execution.succeeded"
            | "session.execution.failed"
            | "session.execution.interrupted"
            | "session.step.started"
            | "session.text.ended"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const SESSION: &str = "ses_recovery_1";
    const MESSAGE: &str = "msg_recovery_1";
    const ASSISTANT: &str = "msg_assistant_1";

    fn durable(seq: u64, event_type: &str, data: Value) -> Value {
        json!({
            "id": format!("evt_recovery_{seq}"),
            "created": seq,
            "type": event_type,
            "durable": {"aggregateID": SESSION, "seq": seq, "version": 1},
            "data": data,
        })
    }

    fn user_lineage() -> Vec<Value> {
        vec![
            durable(
                1,
                "session.inbox.enqueued",
                json!({
                    "sessionID": SESSION,
                    "inboxID": MESSAGE,
                    "item": {"type":"user", "payload":{"prompt":{"text":"private"}}, "delivery":"steer"}
                }),
            ),
            // The pinned runner publishes Execution.Started before draining
            // and promoting the inbox row to InboxDelivered.
            durable(
                2,
                "session.execution.started",
                json!({"sessionID": SESSION}),
            ),
            durable(
                3,
                "session.inbox.delivered",
                json!({"sessionID": SESSION, "inboxID": MESSAGE}),
            ),
        ]
    }

    fn sync(seq: Option<u64>) -> Value {
        match seq {
            Some(seq) => json!({"type":"log.synced", "aggregateID":SESSION, "seq":seq}),
            None => json!({"type":"log.synced", "aggregateID":SESSION}),
        }
    }

    fn feed_to_result(events: impl IntoIterator<Item = Value>) -> Option<TurnObservation> {
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        let mut result = None;
        for event in events {
            result = recovery.observe(&event).expect("valid fixture event");
        }
        result
    }

    fn successful_feed() -> Vec<Value> {
        let mut events = user_lineage();
        events.push(durable(
            4,
            "session.step.started",
            json!({"sessionID": SESSION, "assistantMessageID": ASSISTANT}),
        ));
        events.push(durable(
            5,
            "session.text.ended",
            json!({"sessionID": SESSION, "assistantMessageID": ASSISTANT, "ordinal":0, "text":"Recovered answer"}),
        ));
        events.push(durable(
            6,
            "session.execution.succeeded",
            json!({"sessionID": SESSION}),
        ));
        events.push(sync(Some(6)));
        events
    }

    #[test]
    fn complete_durable_lineage_recovers_only_complete_text() {
        assert_eq!(
            feed_to_result(successful_feed()),
            Some(TurnObservation::Completed {
                answer_text: "Recovered answer".to_owned()
            })
        );
    }

    #[test]
    fn recovered_answer_preserves_exact_provider_text() {
        let mut events = successful_feed();
        events[4]["data"]["text"] = json!("  Recovered answer\n");
        assert_eq!(
            feed_to_result(events),
            Some(TurnObservation::Completed {
                answer_text: "  Recovered answer\n".to_owned()
            })
        );
    }

    #[test]
    fn recovered_provider_failure_is_closed_and_does_not_expose_raw_error() {
        let mut events = user_lineage();
        events.push(durable(
            4,
            "session.execution.failed",
            json!({"sessionID": SESSION, "error":{"name":"SecretError", "message":"private provider detail"}}),
        ));
        events.push(sync(Some(4)));
        assert_eq!(
            feed_to_result(events),
            Some(TurnObservation::Failed {
                reason_code: TurnFailure::ProviderError
            })
        );
    }

    #[test]
    fn non_shutdown_interruption_recovers_stop_but_shutdown_does_not() {
        let mut user_stop = user_lineage();
        user_stop.push(durable(
            4,
            "session.execution.interrupted",
            json!({"sessionID": SESSION, "reason":"user"}),
        ));
        user_stop.push(sync(Some(4)));
        assert_eq!(feed_to_result(user_stop), Some(TurnObservation::Stopped));

        let mut shutdown = user_lineage();
        shutdown.push(durable(
            4,
            "session.execution.interrupted",
            json!({"sessionID": SESSION, "reason":"shutdown"}),
        ));
        shutdown.push(sync(Some(4)));
        assert_eq!(feed_to_result(shutdown), None);
    }

    #[test]
    fn prompt_lineage_and_session_aggregate_must_match_exactly() {
        let mut wrong_message = user_lineage();
        wrong_message[0]["data"]["inboxID"] = json!("msg_someone_else");
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        assert_eq!(
            recovery.observe(&wrong_message[0]),
            Err(RecoveryError::InvalidEvidence)
        );

        let mut foreign = successful_feed();
        foreign[0]["durable"]["aggregateID"] = json!("ses_foreign");
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        assert_eq!(
            recovery.observe(&foreign[0]),
            Err(RecoveryError::InvalidEvidence)
        );
    }

    #[test]
    fn validates_pinned_event_identity_timestamp_and_recovery_schema_version() {
        let mut bad_id = user_lineage();
        bad_id[0]["id"] = json!("event_1");
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        assert_eq!(
            recovery.observe(&bad_id[0]),
            Err(RecoveryError::InvalidEvidence)
        );

        let mut bad_time = user_lineage();
        bad_time[0].as_object_mut().unwrap().remove("created");
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        assert_eq!(
            recovery.observe(&bad_time[0]),
            Err(RecoveryError::InvalidEvidence)
        );

        let mut wrong_version = user_lineage();
        wrong_version[0]["durable"]["version"] = json!(2);
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        assert_eq!(
            recovery.observe(&wrong_version[0]),
            Err(RecoveryError::InvalidEvidence)
        );
    }

    #[test]
    fn replay_watermark_accepts_gaps_but_rejects_regression_or_premature_sync() {
        let mut events = successful_feed();
        events[1]["durable"]["seq"] = json!(7);
        events[2]["durable"]["seq"] = json!(11);
        events[3]["durable"]["seq"] = json!(12);
        events[4]["durable"]["seq"] = json!(13);
        events[5]["durable"]["seq"] = json!(15);
        events[6]["seq"] = json!(15);
        assert!(feed_to_result(events).is_some());

        let mut regressed = successful_feed();
        regressed[4]["durable"]["seq"] = json!(4);
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        let mut failed = false;
        for event in regressed {
            if recovery.observe(&event).is_err() {
                failed = true;
                break;
            }
        }
        assert!(failed);

        let mut bad_watermark = user_lineage();
        bad_watermark.push(sync(Some(1)));
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        let mut failed = false;
        for event in bad_watermark {
            if recovery.observe(&event).is_err() {
                failed = true;
                break;
            }
        }
        assert!(failed);
    }

    #[test]
    fn live_terminal_after_sync_is_reconciled_without_contiguous_sequence_assumptions() {
        let mut events = user_lineage();
        events.push(durable(
            9,
            "session.step.started",
            json!({"sessionID": SESSION, "assistantMessageID": ASSISTANT}),
        ));
        events.push(sync(Some(9)));
        events.push(durable(
            12,
            "session.text.ended",
            json!({"sessionID": SESSION, "assistantMessageID": ASSISTANT, "ordinal":0, "text":"Live tail"}),
        ));
        events.push(durable(
            14,
            "session.execution.succeeded",
            json!({"sessionID": SESSION}),
        ));
        assert_eq!(
            feed_to_result(events),
            Some(TurnObservation::Completed {
                answer_text: "Live tail".to_owned()
            })
        );
    }

    #[test]
    fn requires_one_matching_delivery_and_terminal_after_checkpointed_execution() {
        let mut undelivered = user_lineage();
        undelivered.remove(2);
        undelivered.push(durable(
            4,
            "session.execution.succeeded",
            json!({"sessionID": SESSION}),
        ));
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        for event in undelivered {
            if recovery.observe(&event).is_err() {
                break;
            }
        }
        assert!(recovery.result().is_none());

        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        for event in user_lineage() {
            recovery.observe(&event).unwrap();
        }
        assert_eq!(
            recovery.observe(&durable(
                4,
                "session.inbox.delivered",
                json!({"sessionID": SESSION, "inboxID": MESSAGE}),
            )),
            Err(RecoveryError::InvalidEvidence)
        );
    }

    #[test]
    fn repeated_terminal_and_unrecognized_interruption_reason_fail_closed() {
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        for event in user_lineage() {
            recovery.observe(&event).unwrap();
        }
        recovery
            .observe(&durable(
                4,
                "session.execution.succeeded",
                json!({"sessionID": SESSION}),
            ))
            .unwrap();
        assert_eq!(
            recovery.observe(&durable(
                5,
                "session.execution.failed",
                json!({"sessionID": SESSION}),
            )),
            Err(RecoveryError::InvalidEvidence)
        );

        let mut invalid_reason = user_lineage();
        invalid_reason.push(durable(
            4,
            "session.execution.interrupted",
            json!({"sessionID": SESSION, "reason":"unknown"}),
        ));
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        assert_eq!(
            invalid_reason
                .iter()
                .map(|event| recovery.observe(event))
                .find(Result::is_err)
                .unwrap(),
            Err(RecoveryError::InvalidEvidence)
        );
    }

    #[test]
    fn bounds_events_and_decoded_bytes() {
        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        for sequence in 1..=MAX_LOG_EVENTS as u64 {
            recovery
                .observe(&durable(
                    sequence,
                    "session.unknown",
                    json!({"sessionID": SESSION}),
                ))
                .unwrap();
        }
        assert_eq!(
            recovery.observe(&durable(
                MAX_LOG_EVENTS as u64 + 1,
                "session.unknown",
                json!({"sessionID": SESSION}),
            )),
            Err(RecoveryError::LimitExceeded)
        );

        let mut recovery = SessionLogRecovery::new(SESSION, MESSAGE);
        for sequence in 1..=17 {
            let result = recovery.observe(&durable(
                sequence,
                "session.unknown",
                json!({"sessionID": SESSION, "padding":"x".repeat(250_000)}),
            ));
            if sequence < 17 {
                assert!(result.is_ok());
            } else {
                assert_eq!(result, Err(RecoveryError::LimitExceeded));
            }
        }
    }
}
