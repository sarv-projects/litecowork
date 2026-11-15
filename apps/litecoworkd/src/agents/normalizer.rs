//! Normalization of untrusted OpenCode v2.0.32 Server events into the closed
//! internal event enum (001-T24).
//!
//! Provider events are untrusted observations until they are normalized and
//! correlated. This module is the single boundary that turns a raw
//! [`OpenCodeEvent`] (an SSE frame whose `data` is attacker-influenced JSON)
//! into a [`NormalizedAgentEvent`]. It never persists, logs, or projects any
//! provider text itself; it only classifies.
//!
//! Correlation rule (AGENTS.md, AC-008): every event is correlated on the
//! session id carried inside the envelope (`properties.sessionID`, or the
//! `sessionID`/`session_id`/`session` keys a part carries), never on a
//! top-level guess. A frame that does not carry the expected native session id
//! is dropped as foreign; a frame whose `type` is unknown is counted and
//! dropped. Neither can mutate turn state or the durable projection.
//!
//! Finding R-01 (RESEARCH.md): the hand-parse this normalizer replaces waited
//! for a top-level `delta`/`content`/`done`/`message_completed`, which the real
//! v2.0.32 envelope never emits. The real turn-text names are
//! `message.part.updated` / `message.part.delta` (with a `part.type` of
//! `text`), and the real turn-completion name is `session.idle`.

use serde_json::Value;

fn bounded_string<const MAX_BYTES: usize>(text: &str) -> Option<String> {
    (!text.trim().is_empty() && text.len() <= MAX_BYTES && !text.chars().any(char::is_control))
        .then(|| text.to_owned())
}

/// Reasoning content is never stored or shown to the owner unless the harness
/// itself marks it user-visible. OpenCode v2.0.32 does not mark reasoning parts
/// user-visible, so [`ReasoningVisibility::Hidden`] is the only value this
/// normalizer currently produces; the variant exists so a future harness that
/// does mark reasoning cannot be silently treated as hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReasoningVisibility {
    Hidden,
    // Produced only by a future harness that marks reasoning user-visible; the
    // variant exists so such a harness cannot be silently treated as hidden.
    #[allow(
        dead_code,
        reason = "normative visibility state; OpenCode v2.0.32 never marks reasoning user-visible"
    )]
    UserVisible,
}

/// Terminal classification an observed error or execution result carries into
/// the internal enum. The provider's own string is never trusted as authority;
/// only this closed classification crosses the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolOutcome {
    Succeeded,
    Failed,
}

/// Closed internal event enum (audit section 7). Every raw OpenCode event is
/// mapped to exactly one of these or is counted and dropped. No variant carries
/// a native handle, a credential, or raw prompt text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedAgentEvent {
    /// An incremental chunk of the assistant's user-visible answer text.
    TextDelta { text: String },
    /// The assistant text part reached its terminal (`time.end`) state; the
    /// accumulated text through this point is the committed answer so far.
    TextEnded { text: String },
    /// Reasoning began. Kept separate from answer text and never stored or
    /// shown unless `visibility` is `UserVisible`.
    ReasoningStarted { visibility: ReasoningVisibility },
    /// Reasoning ended. Carries the same visibility discipline.
    ReasoningEnded { visibility: ReasoningVisibility },
    /// A native tool invocation was observed (REPORTED, never an Effect).
    ToolCalled { tool: String, call_id: String },
    /// A native tool reported incremental progress.
    ToolProgress { call_id: String },
    /// A native tool reported a terminal outcome.
    ToolSettled {
        call_id: String,
        outcome: ToolOutcome,
    },
    /// The harness is asking the owner a native permission question. This is a
    /// native prompt answered through the harness, never a LiteCowork grant.
    PermissionAsked {
        /// Private native reply key. Never persist or project this identifier.
        permission_id: String,
        /// Session identity from the correlated event envelope; Runtime-local only.
        native_session_id: String,
        /// Provider-reported operation name, still untrusted display text.
        action: String,
        /// Provider resource identifiers/paths, Runtime-local and untrusted.
        resources: Vec<String>,
        /// Optional provider explanation; Runtime-local and untrusted.
        message: Option<String>,
    },
    /// A file was changed by the agent (REPORTED observation only).
    FileChanged { path_present: bool },
    /// The native plan/todo list changed.
    TodoUpdated { count: usize },
    /// A native step started.
    StepStarted,
    /// A native step ended.
    StepEnded,
    /// Native usage was reported. Values are not billing authority. Token counts are
    /// present only when the native frame carried them as non-negative integers.
    UsageRecorded {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    /// The whole native execution reached a terminal outcome.
    ExecutionSettled { outcome: ToolOutcome },
    /// The native session went idle: the turn's native work is complete.
    Idle,
}

/// Running counters for the drop classes AGENTS.md requires to be observable.
/// These are safe correlation facts only (counts), never provider content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NormalizerCounters {
    /// Frames whose envelope `type` is not in the known set.
    pub unknown: u64,
    /// Frames whose correlated session id is not this turn's native session.
    pub foreign_session: u64,
    /// Frames that carried no correlatable session id at all.
    pub uncorrelated: u64,
}

/// Stateless-per-call normalizer bound to one native session id. It owns only
/// the native session identity it correlates against and the safe drop
/// counters; it accumulates no provider text.
pub(crate) struct OpenCodeEventNormalizer {
    native_session_id: String,
    counters: NormalizerCounters,
}

impl OpenCodeEventNormalizer {
    pub(crate) fn new(native_session_id: impl Into<String>) -> Self {
        Self {
            native_session_id: native_session_id.into(),
            counters: NormalizerCounters::default(),
        }
    }

    pub(crate) fn counters(&self) -> NormalizerCounters {
        self.counters
    }

    /// Normalize one raw envelope. Returns:
    /// - `Ok(Some(event))` for a recognized, correlated event;
    /// - `Ok(None)` for a recognized event that is intentionally not projected
    ///   (for example a heartbeat-like `session.updated`), or for a dropped
    ///   frame (unknown type, foreign session, uncorrelated). Drops are counted.
    ///
    /// The input `event_type` is the SSE `event:` field (usually absent for
    /// OpenCode, which carries the discriminator inside `data.type`); the
    /// `data` is the parsed JSON body. The discriminator is read from
    /// `data.type` first and falls back to the SSE `event:` field.
    pub(crate) fn normalize(
        &mut self,
        event_type: Option<&str>,
        data: &Value,
    ) -> Option<NormalizedAgentEvent> {
        let discriminator = data
            .get("type")
            .and_then(Value::as_str)
            .or(event_type)?
            .to_owned();

        // The properties object carries the correlatable session id for
        // session-scoped events; a part carries its own session id.
        let properties = data.get("properties");

        match discriminator.as_str() {
            "message.part.updated" => self.on_part(properties, true),
            "message.part.delta" => self.on_part(properties, false),
            "message.updated" => {
                // Role/metadata refresh only. Correlate and drop from projection
                // without counting as unknown: it is a recognized no-projection
                // event.
                let _ = self.correlate_session(properties);
                None
            }
            "session.idle" => self
                .correlate_session(properties)
                .then_some(NormalizedAgentEvent::Idle),
            "session.error" => self.correlate_session(properties).then_some(
                NormalizedAgentEvent::ExecutionSettled {
                    outcome: ToolOutcome::Failed,
                },
            ),
            "permission.asked" => self.on_permission(properties),
            "permission.updated" => {
                let _ = self.correlate_session(properties);
                None
            }
            "todo.updated" => self.on_todo(properties),
            "file.edited" | "file.watcher.updated" => self.on_file(properties),
            "session.created" | "session.updated" | "session.compacted" | "session.status"
            | "session.deleted" => {
                // Recognized session-scoped lifecycle frames with no internal
                // projection in this normalizer. Correlate (to drop foreign
                // noise) and emit nothing.
                let _ = self.correlate_session(properties);
                None
            }
            _ => {
                self.counters.unknown = self.counters.unknown.saturating_add(1);
                None
            }
        }
    }

    /// Map a `message.part.updated` / `message.part.delta` frame. `updated` is
    /// true for the full-part form (which carries `time.end` for completion),
    /// false for the incremental delta form.
    fn on_part(
        &mut self,
        properties: Option<&Value>,
        updated: bool,
    ) -> Option<NormalizedAgentEvent> {
        let part = properties.and_then(|p| p.get("part"))?;
        // A part carries its own session id; correlate on it directly.
        if !self.correlate_part(part) {
            return None;
        }
        match part.get("type").and_then(Value::as_str)? {
            "text" => {
                let text = part.get("text").and_then(Value::as_str).unwrap_or("");
                // Synthetic text OpenCode injects into a user message is hidden
                // from its own UI; never project it as agent answer text.
                if part.get("synthetic").and_then(Value::as_bool) == Some(true) {
                    return None;
                }
                let ended = part
                    .get("time")
                    .and_then(|time| time.get("end"))
                    .is_some_and(|end| !end.is_null());
                if updated && ended {
                    Some(NormalizedAgentEvent::TextEnded {
                        text: text.to_owned(),
                    })
                } else {
                    Some(NormalizedAgentEvent::TextDelta {
                        text: text.to_owned(),
                    })
                }
            }
            "reasoning" => {
                // Reasoning is never user-visible for OpenCode v2.0.32.
                let ended = part
                    .get("time")
                    .and_then(|time| time.get("end"))
                    .is_some_and(|end| !end.is_null());
                let visibility = ReasoningVisibility::Hidden;
                Some(if ended {
                    NormalizedAgentEvent::ReasoningEnded { visibility }
                } else {
                    NormalizedAgentEvent::ReasoningStarted { visibility }
                })
            }
            "tool" => {
                let call_id = part
                    .get("callID")
                    .or_else(|| part.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let tool = part
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let status = part
                    .get("state")
                    .and_then(|state| state.get("status"))
                    .and_then(Value::as_str)
                    .or_else(|| part.get("status").and_then(Value::as_str));
                match status {
                    Some("completed") => Some(NormalizedAgentEvent::ToolSettled {
                        call_id,
                        outcome: ToolOutcome::Succeeded,
                    }),
                    Some("error") | Some("failed") => Some(NormalizedAgentEvent::ToolSettled {
                        call_id,
                        outcome: ToolOutcome::Failed,
                    }),
                    Some("running") => Some(NormalizedAgentEvent::ToolProgress { call_id }),
                    _ => Some(NormalizedAgentEvent::ToolCalled { tool, call_id }),
                }
            }
            "step-start" => Some(NormalizedAgentEvent::StepStarted),
            "step-finish" => {
                // A step-finish that carries native usage (tokens or cost) is a
                // usage observation; otherwise it is just the step boundary.
                // Usage values are never billing authority.
                let has_usage = part.get("tokens").is_some() || part.get("cost").is_some();
                if has_usage {
                    let tokens = part.get("tokens");
                    Some(NormalizedAgentEvent::UsageRecorded {
                        input_tokens: tokens.and_then(|t| t.get("input")).and_then(Value::as_u64),
                        output_tokens: tokens.and_then(|t| t.get("output")).and_then(Value::as_u64),
                    })
                } else {
                    Some(NormalizedAgentEvent::StepEnded)
                }
            }
            _ => {
                // A recognized part frame with an unrecognized part type is a
                // real unknown: count and drop.
                self.counters.unknown = self.counters.unknown.saturating_add(1);
                None
            }
        }
    }

    fn on_permission(&mut self, properties: Option<&Value>) -> Option<NormalizedAgentEvent> {
        let Some(properties) = properties else {
            return None;
        };
        if !self.correlate_session(Some(properties)) {
            return None;
        }
        let session_id = session_id_of(properties)?;
        if session_id.len() > 128 {
            return None;
        }
        let session_id = session_id.to_owned();
        let permission_id = properties
            .get("id")
            .and_then(Value::as_str)
            .and_then(bounded_string::<128>)?;
        let action = properties
            .get("action")
            .and_then(Value::as_str)
            .and_then(bounded_string::<128>)?;
        let resources_value = properties.get("resources")?.as_array()?;
        if resources_value.len() > 64 {
            return None;
        }
        let resources = resources_value
            .iter()
            .map(|value| value.as_str().and_then(bounded_string::<512>))
            .collect::<Option<Vec<_>>>()?;
        let message = match properties.get("message") {
            None | Some(Value::Null) => None,
            Some(value) => Some(value.as_str().and_then(bounded_string::<512>)?),
        };
        Some(NormalizedAgentEvent::PermissionAsked {
            permission_id,
            native_session_id: session_id,
            action,
            resources,
            message,
        })
    }

    fn on_todo(&mut self, properties: Option<&Value>) -> Option<NormalizedAgentEvent> {
        if !self.correlate_session(properties) {
            return None;
        }
        let count = properties
            .and_then(|p| p.get("todos"))
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        Some(NormalizedAgentEvent::TodoUpdated { count })
    }

    fn on_file(&mut self, properties: Option<&Value>) -> Option<NormalizedAgentEvent> {
        // File frames carry a session id on some builds and not others. When a
        // session id is present it must match; when it is absent the frame is
        // still a real file observation for this server instance's turn.
        if let Some(session) = properties.and_then(session_id_of)
            && session != self.native_session_id
        {
            self.counters.foreign_session = self.counters.foreign_session.saturating_add(1);
            return None;
        }
        let path_present = properties
            .and_then(|p| p.get("file").or_else(|| p.get("path")))
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty());
        Some(NormalizedAgentEvent::FileChanged { path_present })
    }

    /// Correlate a session-scoped event on `properties.sessionID` (or its
    /// documented aliases). Returns true only when the id matches this turn's
    /// native session. A missing id is counted as uncorrelated; a mismatching
    /// id is counted as foreign. Both are dropped.
    fn correlate_session(&mut self, properties: Option<&Value>) -> bool {
        match properties.and_then(session_id_of) {
            Some(session) if session == self.native_session_id => true,
            Some(_) => {
                self.counters.foreign_session = self.counters.foreign_session.saturating_add(1);
                false
            }
            None => {
                self.counters.uncorrelated = self.counters.uncorrelated.saturating_add(1);
                false
            }
        }
    }

    /// Correlate a part-scoped frame on the part's own `sessionID`.
    fn correlate_part(&mut self, part: &Value) -> bool {
        match session_id_of(part) {
            Some(session) if session == self.native_session_id => true,
            Some(_) => {
                self.counters.foreign_session = self.counters.foreign_session.saturating_add(1);
                false
            }
            None => {
                self.counters.uncorrelated = self.counters.uncorrelated.saturating_add(1);
                false
            }
        }
    }
}

/// Read the session id an envelope `properties`/`part`/`location` object
/// carries. OpenCode uses `sessionID`; other builds have used `session_id` or
/// `session`. A `location.sessionID` is also accepted. Never guess from an
/// unrelated top-level field.
fn session_id_of(value: &Value) -> Option<&str> {
    value
        .get("sessionID")
        .or_else(|| value.get("session_id"))
        .or_else(|| value.get("session"))
        .or_else(|| value.get("location").and_then(|loc| loc.get("sessionID")))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
}

#[cfg(test)]
mod opencode_event_normalizer_tests {
    use super::*;
    use serde_json::json;

    const SESSION: &str = "ses_fixture_0001";

    fn normalizer() -> OpenCodeEventNormalizer {
        OpenCodeEventNormalizer::new(SESSION)
    }

    #[test]
    fn text_delta_and_ended_map_from_the_real_part_envelope() {
        let mut norm = normalizer();
        let delta = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "text", "sessionID": SESSION, "messageID": "msg_1",
                "id": "prt_1", "text": "Hello"
            }}
        });
        assert_eq!(
            norm.normalize(None, &delta),
            Some(NormalizedAgentEvent::TextDelta {
                text: "Hello".to_owned()
            })
        );

        let ended = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "text", "sessionID": SESSION, "messageID": "msg_1",
                "id": "prt_1", "text": "Hello, world.",
                "time": {"start": 1, "end": 2}
            }}
        });
        assert_eq!(
            norm.normalize(None, &ended),
            Some(NormalizedAgentEvent::TextEnded {
                text: "Hello, world.".to_owned()
            })
        );
    }

    #[test]
    fn reasoning_is_hidden_and_kept_separate_from_answer_text() {
        let mut norm = normalizer();
        let reasoning = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "reasoning", "sessionID": SESSION, "id": "prt_r",
                "messageID": "msg_1", "text": "secret chain of thought"
            }}
        });
        assert_eq!(
            norm.normalize(None, &reasoning),
            Some(NormalizedAgentEvent::ReasoningStarted {
                visibility: ReasoningVisibility::Hidden
            })
        );
    }

    #[test]
    fn tool_call_progress_and_settlement_map_by_status() {
        let mut norm = normalizer();
        let called = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "tool", "sessionID": SESSION, "id": "prt_t",
                "messageID": "msg_1", "tool": "bash", "callID": "call_1",
                "state": {"status": "pending"}
            }}
        });
        assert_eq!(
            norm.normalize(None, &called),
            Some(NormalizedAgentEvent::ToolCalled {
                tool: "bash".to_owned(),
                call_id: "call_1".to_owned()
            })
        );

        let done = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "tool", "sessionID": SESSION, "id": "prt_t",
                "messageID": "msg_1", "tool": "bash", "callID": "call_1",
                "state": {"status": "completed"}
            }}
        });
        assert_eq!(
            norm.normalize(None, &done),
            Some(NormalizedAgentEvent::ToolSettled {
                call_id: "call_1".to_owned(),
                outcome: ToolOutcome::Succeeded
            })
        );
    }

    #[test]
    fn permission_file_todo_and_idle_map() {
        let mut norm = normalizer();
        assert_eq!(
            norm.normalize(
                None,
                &json!({"type": "permission.asked",
                "properties": {
                    "sessionID": SESSION,
                    "id": "perm_1",
                    "action": "read",
                    "resources": ["/workspace/report.csv"],
                    "message": "May I inspect this file?"
                }})
            ),
            Some(NormalizedAgentEvent::PermissionAsked {
                permission_id: "perm_1".to_owned(),
                native_session_id: SESSION.to_owned(),
                action: "read".to_owned(),
                resources: vec!["/workspace/report.csv".to_owned()],
                message: Some("May I inspect this file?".to_owned()),
            })
        );
        assert_eq!(
            norm.normalize(
                None,
                &json!({"type": "file.edited",
                    "properties": {"sessionID": SESSION, "file": "/ws/a.rs"}})
            ),
            Some(NormalizedAgentEvent::FileChanged { path_present: true })
        );
        assert_eq!(
            norm.normalize(
                None,
                &json!({"type": "todo.updated",
                    "properties": {"sessionID": SESSION, "todos": [{}, {}]}})
            ),
            Some(NormalizedAgentEvent::TodoUpdated { count: 2 })
        );
        assert_eq!(
            norm.normalize(
                None,
                &json!({"type": "session.idle", "properties": {"sessionID": SESSION}})
            ),
            Some(NormalizedAgentEvent::Idle)
        );
    }

    #[test]
    fn permission_asked_requires_bounded_pinned_payload() {
        let mut norm = normalizer();
        for properties in [
            json!({"sessionID": SESSION, "id": "perm", "resources": []}),
            json!({"sessionID": SESSION, "id": "perm", "action": "read"}),
            json!({"sessionID": SESSION, "id": "perm", "action": "read", "resources": "file"}),
            json!({"sessionID": SESSION, "id": "perm", "action": "read", "resources": vec!["x"; 65]}),
            json!({"sessionID": "foreign", "id": "perm", "action": "read", "resources": []}),
        ] {
            let event = json!({"type": "permission.asked", "properties": properties});
            assert_eq!(norm.normalize(None, &event), None);
        }
        assert_eq!(norm.counters().foreign_session, 1);
    }

    #[test]
    fn step_boundaries_and_usage_map_from_part_frames() {
        let mut norm = normalizer();
        let start = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "step-start", "sessionID": SESSION, "id": "prt_s", "messageID": "m"
            }}
        });
        assert_eq!(
            norm.normalize(None, &start),
            Some(NormalizedAgentEvent::StepStarted)
        );

        let finish_usage = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "step-finish", "sessionID": SESSION, "id": "prt_s", "messageID": "m",
                "cost": 0, "tokens": {"input": 10, "output": 20}
            }}
        });
        assert_eq!(
            norm.normalize(None, &finish_usage),
            Some(NormalizedAgentEvent::UsageRecorded {
                input_tokens: Some(10),
                output_tokens: Some(20)
            })
        );

        let finish_bare = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "step-finish", "sessionID": SESSION, "id": "prt_s2", "messageID": "m"
            }}
        });
        assert_eq!(
            norm.normalize(None, &finish_bare),
            Some(NormalizedAgentEvent::StepEnded)
        );
    }

    #[test]
    fn session_error_settles_execution_failed() {
        let mut norm = normalizer();
        assert_eq!(
            norm.normalize(
                None,
                &json!({"type": "session.error",
                    "properties": {"sessionID": SESSION, "error": {"name": "ProviderError"}}})
            ),
            Some(NormalizedAgentEvent::ExecutionSettled {
                outcome: ToolOutcome::Failed
            })
        );
    }

    #[test]
    fn unknown_events_are_counted_and_dropped() {
        let mut norm = normalizer();
        assert_eq!(
            norm.normalize(
                None,
                &json!({"type": "server.invented.event", "properties": {}})
            ),
            None
        );
        assert_eq!(norm.counters().unknown, 1);
    }

    #[test]
    fn foreign_session_frames_cannot_mutate_state() {
        let mut norm = normalizer();
        let foreign = json!({
            "type": "message.part.updated",
            "properties": {"part": {
                "type": "text", "sessionID": "ses_other", "id": "prt_x",
                "messageID": "msg_x", "text": "not for us"
            }}
        });
        assert_eq!(norm.normalize(None, &foreign), None);
        assert_eq!(norm.counters().foreign_session, 1);

        let foreign_idle =
            json!({"type": "session.idle", "properties": {"sessionID": "ses_other"}});
        assert_eq!(norm.normalize(None, &foreign_idle), None);
        assert_eq!(norm.counters().foreign_session, 2);
    }

    #[test]
    fn uncorrelated_frames_are_counted_and_dropped() {
        let mut norm = normalizer();
        let no_session = json!({"type": "session.idle", "properties": {}});
        assert_eq!(norm.normalize(None, &no_session), None);
        assert_eq!(norm.counters().uncorrelated, 1);
    }

    #[test]
    fn r01_legacy_hand_parse_names_are_never_matched() {
        // The names the replaced hand-parse waited for do not exist in the v2
        // envelope and must now be classified as unknown, never as completion.
        let mut norm = normalizer();
        for legacy in ["message_completed", "delta", "content", "done"] {
            assert_eq!(
                norm.normalize(Some(legacy), &json!({"properties": {"sessionID": SESSION}})),
                None,
                "{legacy}"
            );
        }
        assert_eq!(norm.counters().unknown, 4);
    }
}
