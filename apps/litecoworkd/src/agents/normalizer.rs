//! Normalization of untrusted OpenCode v2.0.32 Server events into the closed
//! internal event enum (001-T24).
//!
//! Provider events are untrusted observations until they are normalized and
//! correlated. This module is the single boundary that turns a raw
//! [`OpenCodeEvent`] (an SSE frame whose `data` is attacker-influenced JSON)
//! into a [`NormalizedAgentEvent`]. It never persists, logs, or projects any
//! provider text itself; it only classifies.
//!
//! Correlation rule (AGENTS.md, AC-008): session-scoped events are correlated
//! only on `data.sessionID` from the pinned v2.0.32 `{ type, data }` envelope.
//! Global event location, the SSE `event:` field, and legacy aliases are never
//! treated as session identity. Frames without the exact session identity are
//! dropped and counted; unknown event names are counted and dropped.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExecutionOutcome {
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
    /// A native step ended. Provider-reported usage remains explicitly
    /// reported and is never billing authority.
    StepEnded {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    /// A native step failed. Raw provider error details are discarded.
    StepFailed {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    /// Native session-wide usage snapshot. OpenCode reports cumulative totals
    /// across the whole native session, so this must not be treated as a
    /// per-turn delta or added to step usage.
    UsageSnapshot {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    /// The native execution entered its active state.
    ExecutionStarted,
    /// The whole native execution reached a terminal outcome.
    ExecutionSettled { outcome: ExecutionOutcome },
    /// The native execution was interrupted by the provider.
    ExecutionInterrupted,
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

    /// Normalize one raw pinned-v2 envelope. Returns `Some(event)` for a
    /// recognized, correlated event, and `None` for an intentionally
    /// unprojected or dropped frame. Unknown, foreign-session and uncorrelated
    /// drops are counted.
    ///
    /// The SSE `event:` field is intentionally ignored: the pinned protocol's
    /// discriminator is the envelope's `type`, and session correlation comes
    /// only from the envelope's `data.sessionID`.
    pub(crate) fn normalize(
        &mut self,
        _event_type: Option<&str>,
        data: &Value,
    ) -> Option<NormalizedAgentEvent> {
        let Some(discriminator) = data.get("type").and_then(Value::as_str) else {
            self.counters.unknown = self.counters.unknown.saturating_add(1);
            return None;
        };
        let Some(payload) = data.get("data") else {
            self.counters.uncorrelated = self.counters.uncorrelated.saturating_add(1);
            return None;
        };

        match discriminator {
            "server.connected" => None,
            "session.text.delta" => self.on_text(payload, "delta", false),
            "session.text.ended" => self.on_text(payload, "text", true),
            "session.reasoning.started" => self.correlate_session(Some(payload)).then_some(
                NormalizedAgentEvent::ReasoningStarted {
                    visibility: ReasoningVisibility::Hidden,
                },
            ),
            "session.reasoning.ended" => self.correlate_session(Some(payload)).then_some(
                NormalizedAgentEvent::ReasoningEnded {
                    visibility: ReasoningVisibility::Hidden,
                },
            ),
            "session.reasoning.delta" => {
                let _ = self.correlate_session(Some(payload));
                None
            }
            "session.tool.input.started" => self.on_tool_started(payload),
            "session.tool.input.delta" | "session.tool.input.ended" | "session.tool.progress" => {
                self.on_tool_progress(payload)
            }
            "session.tool.success" => self.on_tool_settled(payload, ToolOutcome::Succeeded),
            "session.tool.failed" => self.on_tool_settled(payload, ToolOutcome::Failed),
            "session.step.started" => self
                .correlate_session(Some(payload))
                .then_some(NormalizedAgentEvent::StepStarted),
            "session.step.ended" => self.on_step_ended(payload, false),
            "session.step.failed" => self.on_step_ended(payload, true),
            "session.usage.updated" => self.on_usage(payload),
            "session.execution.started" => self
                .correlate_session(Some(payload))
                .then_some(NormalizedAgentEvent::ExecutionStarted),
            "session.execution.succeeded" => self.correlate_session(Some(payload)).then_some(
                NormalizedAgentEvent::ExecutionSettled {
                    outcome: ExecutionOutcome::Succeeded,
                },
            ),
            "session.execution.failed" => self.correlate_session(Some(payload)).then_some(
                NormalizedAgentEvent::ExecutionSettled {
                    outcome: ExecutionOutcome::Failed,
                },
            ),
            "session.execution.interrupted" => self
                .correlate_session(Some(payload))
                .then_some(NormalizedAgentEvent::ExecutionInterrupted),
            "session.text.started"
            | "session.tool.called"
            | "permission.replied"
            | "permission.updated"
            | "session.created"
            | "session.updated"
            | "session.deleted"
            | "session.step.streamed" => {
                let _ = self.correlate_session(Some(payload));
                None
            }
            "permission.asked" => self.on_permission(Some(payload)),
            _ => {
                self.counters.unknown = self.counters.unknown.saturating_add(1);
                None
            }
        }
    }

    fn on_text(
        &mut self,
        payload: &Value,
        field: &str,
        ended: bool,
    ) -> Option<NormalizedAgentEvent> {
        if !self.correlate_session(Some(payload)) {
            return None;
        }
        let Some(text) = payload.get(field).and_then(Value::as_str) else {
            return None;
        };
        if ended {
            Some(NormalizedAgentEvent::TextEnded {
                text: text.to_owned(),
            })
        } else {
            Some(NormalizedAgentEvent::TextDelta {
                text: text.to_owned(),
            })
        }
    }

    fn on_tool_started(&mut self, payload: &Value) -> Option<NormalizedAgentEvent> {
        if !self.correlate_session(Some(payload)) {
            return None;
        }
        Some(NormalizedAgentEvent::ToolCalled {
            tool: payload
                .get("name")
                .and_then(Value::as_str)
                .and_then(bounded_string::<128>)?,
            call_id: payload
                .get("id")
                .and_then(Value::as_str)
                .and_then(bounded_string::<128>)?,
        })
    }

    fn on_tool_progress(&mut self, payload: &Value) -> Option<NormalizedAgentEvent> {
        if !self.correlate_session(Some(payload)) {
            return None;
        }
        let call_id = payload
            .get("id")
            .and_then(Value::as_str)
            .and_then(bounded_string::<128>)?;
        Some(NormalizedAgentEvent::ToolProgress { call_id })
    }

    fn on_tool_settled(
        &mut self,
        payload: &Value,
        outcome: ToolOutcome,
    ) -> Option<NormalizedAgentEvent> {
        if !self.correlate_session(Some(payload)) {
            return None;
        }
        let call_id = payload
            .get("id")
            .and_then(Value::as_str)
            .and_then(bounded_string::<128>)?;
        Some(NormalizedAgentEvent::ToolSettled { call_id, outcome })
    }

    fn on_step_ended(&mut self, payload: &Value, failed: bool) -> Option<NormalizedAgentEvent> {
        if !self.correlate_session(Some(payload)) {
            return None;
        }
        let tokens = payload.get("tokens");
        let input_tokens = tokens
            .and_then(|value| value.get("input"))
            .and_then(Value::as_u64);
        let output_tokens = tokens
            .and_then(|value| value.get("output"))
            .and_then(Value::as_u64);
        if failed {
            Some(NormalizedAgentEvent::StepFailed {
                input_tokens,
                output_tokens,
            })
        } else {
            Some(NormalizedAgentEvent::StepEnded {
                input_tokens,
                output_tokens,
            })
        }
    }

    fn on_usage(&mut self, payload: &Value) -> Option<NormalizedAgentEvent> {
        if !self.correlate_session(Some(payload)) {
            return None;
        }
        let tokens = payload.get("tokens")?;
        Some(NormalizedAgentEvent::UsageSnapshot {
            input_tokens: tokens.get("input").and_then(Value::as_u64),
            output_tokens: tokens.get("output").and_then(Value::as_u64),
        })
    }

    fn on_permission(&mut self, properties: Option<&Value>) -> Option<NormalizedAgentEvent> {
        let Some(properties) = properties else {
            return None;
        };
        if !self.correlate_session(Some(properties)) {
            return None;
        }
        let session_id = properties.get("sessionID").and_then(Value::as_str)?;
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

    /// Correlate only the pinned schema's `data.sessionID` field. A missing id
    /// is counted as uncorrelated; a mismatching id is counted as foreign.
    fn correlate_session(&mut self, properties: Option<&Value>) -> bool {
        match properties
            .and_then(|value| value.get("sessionID"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        {
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

#[cfg(test)]
mod opencode_event_normalizer_tests {
    use super::*;
    use serde_json::json;

    const SESSION: &str = "ses_fixture_0001";

    fn normalizer() -> OpenCodeEventNormalizer {
        OpenCodeEventNormalizer::new(SESSION)
    }
    fn event(event_type: &str, data: Value) -> Value {
        json!({"id":"evt_fixture_1", "created":1, "type":event_type, "data":data})
    }

    #[test]
    fn text_and_execution_map_from_the_pinned_v2_envelope() {
        let mut norm = normalizer();
        let delta = event(
            "session.text.delta",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "ordinal":0, "delta":"Hello"}),
        );
        assert_eq!(
            norm.normalize(None, &delta),
            Some(NormalizedAgentEvent::TextDelta {
                text: "Hello".to_owned()
            })
        );
        let ended = event(
            "session.text.ended",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "ordinal":0, "text":"Hello, world."}),
        );
        assert_eq!(
            norm.normalize(None, &ended),
            Some(NormalizedAgentEvent::TextEnded {
                text: "Hello, world.".to_owned()
            })
        );
        let succeeded = event("session.execution.succeeded", json!({"sessionID":SESSION}));
        assert_eq!(
            norm.normalize(None, &succeeded),
            Some(NormalizedAgentEvent::ExecutionSettled {
                outcome: ExecutionOutcome::Succeeded
            })
        );
    }

    #[test]
    fn reasoning_content_is_never_returned_by_the_normalizer() {
        let mut norm = normalizer();
        let started = event(
            "session.reasoning.started",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "ordinal":0}),
        );
        assert_eq!(
            norm.normalize(None, &started),
            Some(NormalizedAgentEvent::ReasoningStarted {
                visibility: ReasoningVisibility::Hidden
            })
        );
        let delta = event(
            "session.reasoning.delta",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "ordinal":0, "delta":"secret chain of thought"}),
        );
        assert_eq!(norm.normalize(None, &delta), None);
        let ended = event(
            "session.reasoning.ended",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "ordinal":0, "text":"secret chain of thought"}),
        );
        assert_eq!(
            norm.normalize(None, &ended),
            Some(NormalizedAgentEvent::ReasoningEnded {
                visibility: ReasoningVisibility::Hidden
            })
        );
    }

    #[test]
    fn tool_events_use_pinned_names_and_ids_without_projecting_input() {
        let mut norm = normalizer();
        let started = event(
            "session.tool.input.started",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "id":"call_1", "name":"bash"}),
        );
        assert_eq!(
            norm.normalize(None, &started),
            Some(NormalizedAgentEvent::ToolCalled {
                tool: "bash".to_owned(),
                call_id: "call_1".to_owned()
            })
        );
        let progress = event(
            "session.tool.progress",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "id":"call_1", "metadata":{"message":"running"}}),
        );
        assert_eq!(
            norm.normalize(None, &progress),
            Some(NormalizedAgentEvent::ToolProgress {
                call_id: "call_1".to_owned()
            })
        );
        let done = event(
            "session.tool.success",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "id":"call_1", "content":[{"type":"text", "text":"done"}], "executed":true}),
        );
        assert_eq!(
            norm.normalize(None, &done),
            Some(NormalizedAgentEvent::ToolSettled {
                call_id: "call_1".to_owned(),
                outcome: ToolOutcome::Succeeded
            })
        );
    }

    #[test]
    fn permission_asked_maps_only_the_correlated_pinned_payload() {
        let mut norm = normalizer();
        let asked = event(
            "permission.asked",
            json!({"sessionID":SESSION, "id":"perm_1", "action":"read", "resources":["/workspace/report.csv"], "message":"May I inspect this file?", "save":[], "metadata":{}, "source":{"type":"tool", "messageID":"msg_1", "id":"call_1"}}),
        );
        assert_eq!(
            norm.normalize(None, &asked),
            Some(NormalizedAgentEvent::PermissionAsked {
                permission_id: "perm_1".to_owned(),
                native_session_id: SESSION.to_owned(),
                action: "read".to_owned(),
                resources: vec!["/workspace/report.csv".to_owned()],
                message: Some("May I inspect this file?".to_owned())
            })
        );
    }

    #[test]
    fn permission_asked_requires_bounded_pinned_payload() {
        let mut norm = normalizer();
        for payload in [
            json!({"sessionID":SESSION, "id":"perm", "resources":[]}),
            json!({"sessionID":SESSION, "id":"perm", "action":"read"}),
            json!({"sessionID":SESSION, "id":"perm", "action":"read", "resources":"file"}),
            json!({"sessionID":SESSION, "id":"perm", "action":"read", "resources":vec!["x";65]}),
            json!({"sessionID":"foreign", "id":"perm", "action":"read", "resources":[]}),
        ] {
            assert_eq!(
                norm.normalize(None, &event("permission.asked", payload)),
                None
            );
        }
        assert_eq!(norm.counters().foreign_session, 1);
    }

    #[test]
    fn step_boundaries_and_reported_usage_map_from_pinned_events() {
        let mut norm = normalizer();
        let start = event(
            "session.step.started",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "started":1}),
        );
        assert_eq!(
            norm.normalize(None, &start),
            Some(NormalizedAgentEvent::StepStarted)
        );
        let ended_with_usage = event(
            "session.step.ended",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_1", "finish":"stop", "cost":0, "tokens":{"input":10, "output":20}}),
        );
        assert_eq!(
            norm.normalize(None, &ended_with_usage),
            Some(NormalizedAgentEvent::StepEnded {
                input_tokens: Some(10),
                output_tokens: Some(20)
            })
        );
        let ended = event(
            "session.step.ended",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_2", "finish":"stop"}),
        );
        assert_eq!(
            norm.normalize(None, &ended),
            Some(NormalizedAgentEvent::StepEnded {
                input_tokens: None,
                output_tokens: None
            })
        );
        let failed = event(
            "session.step.failed",
            json!({"sessionID":SESSION, "assistantMessageID":"msg_3", "error":{"type":"ProviderError", "message":"[REDACTED]"}, "tokens":{"input":5,"output":0,"reasoning":0,"cache":{"read":0,"write":0}}}),
        );
        assert_eq!(
            norm.normalize(None, &failed),
            Some(NormalizedAgentEvent::StepFailed {
                input_tokens: Some(5),
                output_tokens: Some(0)
            })
        );
        let usage = event(
            "session.usage.updated",
            json!({"sessionID":SESSION, "cost":0, "tokens":{"input":3, "output":4}}),
        );
        assert_eq!(
            norm.normalize(None, &usage),
            Some(NormalizedAgentEvent::UsageSnapshot {
                input_tokens: Some(3),
                output_tokens: Some(4)
            })
        );
    }

    #[test]
    fn execution_failure_and_interruption_are_terminal_provider_events() {
        let mut norm = normalizer();
        let started = event("session.execution.started", json!({"sessionID":SESSION}));
        assert_eq!(
            norm.normalize(None, &started),
            Some(NormalizedAgentEvent::ExecutionStarted)
        );
        let failed = event(
            "session.execution.failed",
            json!({"sessionID":SESSION, "error":{"name":"ProviderError"}}),
        );
        assert_eq!(
            norm.normalize(None, &failed),
            Some(NormalizedAgentEvent::ExecutionSettled {
                outcome: ExecutionOutcome::Failed
            })
        );
        let interrupted = event(
            "session.execution.interrupted",
            json!({"sessionID":SESSION, "reason":"user"}),
        );
        assert_eq!(
            norm.normalize(None, &interrupted),
            Some(NormalizedAgentEvent::ExecutionInterrupted)
        );
    }

    #[test]
    fn unknown_events_are_counted_and_dropped() {
        let mut norm = normalizer();
        assert_eq!(
            norm.normalize(None, &event("server.invented.event", json!({}))),
            None
        );
        assert_eq!(norm.counters().unknown, 1);
    }

    #[test]
    fn foreign_session_frames_cannot_mutate_state() {
        let mut norm = normalizer();
        let foreign_text = event(
            "session.text.delta",
            json!({"sessionID":"ses_other", "assistantMessageID":"msg_x", "ordinal":0, "delta":"not for us"}),
        );
        assert_eq!(norm.normalize(None, &foreign_text), None);
        assert_eq!(norm.counters().foreign_session, 1);
        let foreign_execution = event(
            "session.execution.succeeded",
            json!({"sessionID":"ses_other"}),
        );
        assert_eq!(norm.normalize(None, &foreign_execution), None);
        assert_eq!(norm.counters().foreign_session, 2);
    }

    #[test]
    fn uncorrelated_frames_are_counted_and_dropped_without_aliases() {
        let mut norm = normalizer();
        let no_session = event(
            "session.execution.succeeded",
            json!({"session_id":SESSION, "location":{"sessionID":SESSION}}),
        );
        assert_eq!(norm.normalize(None, &no_session), None);
        assert_eq!(norm.counters().uncorrelated, 1);
    }

    #[test]
    fn legacy_and_unqualified_v1_names_are_never_matched() {
        let mut norm = normalizer();
        for legacy in [
            "message_completed",
            "delta",
            "content",
            "done",
            "message.part.updated",
            "session.idle",
            "todo.updated",
            "file.edited",
        ] {
            assert_eq!(
                norm.normalize(None, &event(legacy, json!({"sessionID":SESSION}))),
                None,
                "{legacy}"
            );
        }
        assert_eq!(norm.counters().unknown, 8);
        assert_eq!(
            norm.normalize(
                Some("session.text.delta"),
                &json!({"data":{"sessionID":SESSION}})
            ),
            None
        );
        assert_eq!(norm.counters().unknown, 9);
    }
}
