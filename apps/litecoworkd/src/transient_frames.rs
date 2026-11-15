//! Bounded in-memory transient Conversation frames (`turn.delta`, `turn.settled`).
//!
//! Frames published here are presentation control frames, never DomainEvents, never
//! persisted and never replicated. The channel is a fixed-capacity broadcast ring:
//! publishing never blocks the native read loop, a slow subscriber loses the oldest
//! frames, and the UI treats the resulting sequence gap as a reason to drop its
//! transient buffer and re-read the committed projection (001-T13, B-05, AC-014).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::sync::broadcast;

use crate::agents::adapter::TurnObserver;
use crate::agents::normalizer::NormalizedAgentEvent;

/// Ring capacity: the most frames a lagging subscriber can be behind before it loses
/// the oldest ones.
const BUS_CAPACITY: usize = 256;
/// Largest `text_delta` carried by one frame (the schema allows 32 KiB; the stream
/// frame limit is lower, so stay well under both).
const MAX_DELTA_BYTES: usize = 8 * 1024;
/// A pending delta is flushed once it reaches this size ...
const FLUSH_BYTES: usize = 1024;
/// ... or once this long has passed since the previous flush.
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
/// Total transient text published for one attempt. The UI retains at most 256 KiB per
/// turn; past this the publisher stops and the committed message supersedes the text.
const MAX_ATTEMPT_BYTES: usize = 256 * 1024;

/// One already-serialized transient frame with its routing scope.
#[derive(Clone, Debug)]
pub(crate) struct TransientFrame {
    pub(crate) workspace_id: String,
    pub(crate) conversation_id: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct TransientFrameBus {
    sender: broadcast::Sender<TransientFrame>,
}

impl TransientFrameBus {
    fn new() -> Self {
        Self {
            sender: broadcast::channel(BUS_CAPACITY).0,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self::new()
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<TransientFrame> {
        self.sender.subscribe()
    }

    /// Never blocks. With no subscriber the frame is simply dropped.
    pub(crate) fn publish(&self, frame: TransientFrame) {
        let _ = self.sender.send(frame);
    }
}

/// The process-wide transient frame bus. One daemon serves one local Operator.
pub(crate) fn transient_bus() -> &'static TransientFrameBus {
    static BUS: OnceLock<TransientFrameBus> = OnceLock::new();
    BUS.get_or_init(TransientFrameBus::new)
}

/// Publishes the coalesced answer text of one `(turn, retry ordinal)` attempt.
pub(crate) struct TurnFramePublisher {
    bus: TransientFrameBus,
    workspace_id: String,
    conversation_id: String,
    turn_id: String,
    retry_ordinal: u32,
    sequence: u64,
    /// Everything already published for this attempt (bounded by `MAX_ATTEMPT_BYTES`).
    published: String,
    pending: String,
    last_flush: Instant,
    /// Set when the text can no longer be streamed truthfully (divergent terminal
    /// text or the attempt cap). Nothing further is published; the committed message
    /// replaces whatever the UI already shows.
    stopped: bool,
}

impl TurnFramePublisher {
    pub(crate) fn new(
        bus: TransientFrameBus,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
    ) -> Self {
        Self {
            bus,
            workspace_id: workspace_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            turn_id: turn_id.to_owned(),
            retry_ordinal,
            sequence: 0,
            published: String::new(),
            pending: String::new(),
            last_flush: Instant::now(),
            stopped: false,
        }
    }

    fn append(&mut self, text: &str) {
        if self.stopped || text.is_empty() {
            return;
        }
        if self.published.len() + self.pending.len() + text.len() > MAX_ATTEMPT_BYTES {
            self.stopped = true;
            self.pending.clear();
            return;
        }
        self.pending.push_str(text);
        if self.pending.len() >= FLUSH_BYTES || self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush();
        }
    }

    /// Publishes the pending text as one or more bounded frames on char boundaries.
    fn flush(&mut self) {
        self.last_flush = Instant::now();
        while !self.pending.is_empty() {
            let mut end = self.pending.len().min(MAX_DELTA_BYTES);
            while !self.pending.is_char_boundary(end) {
                end -= 1;
            }
            let chunk: String = self.pending.drain(..end).collect();
            self.sequence += 1;
            self.published.push_str(&chunk);
            let frame = json!({
                "type": "turn.delta",
                "workspace_id": self.workspace_id,
                "conversation_id": self.conversation_id,
                "turn_id": self.turn_id,
                "retry_ordinal": self.retry_ordinal,
                "sequence": self.sequence,
                "text_delta": chunk,
            });
            self.send(&frame);
        }
    }

    fn send(&self, frame: &serde_json::Value) {
        if let Ok(bytes) = serde_json::to_vec(frame) {
            self.bus.publish(TransientFrame {
                workspace_id: self.workspace_id.clone(),
                conversation_id: self.conversation_id.clone(),
                bytes,
            });
        }
    }

    /// The terminal text part re-sends the full answer. Stream only the unseen suffix
    /// when it extends what was published; any divergence stops streaming rather than
    /// concatenating across different content.
    fn terminal_text(&mut self, full: &str) {
        if self.stopped {
            return;
        }
        let already = format!("{}{}", self.published, self.pending);
        match full.strip_prefix(already.as_str()) {
            Some(rest) => self.append(rest),
            None => {
                self.stopped = true;
                self.pending.clear();
            }
        }
    }

    /// Publishes `turn.settled`. A committed message id is attached only when the
    /// durable commit succeeded; otherwise the UI marks its transient text incomplete.
    pub(crate) fn settle(&mut self, committed_message_id: Option<&str>) {
        self.flush();
        let mut frame = json!({
            "type": "turn.settled",
            "workspace_id": self.workspace_id,
            "conversation_id": self.conversation_id,
            "turn_id": self.turn_id,
            "retry_ordinal": self.retry_ordinal,
        });
        if let Some(id) = committed_message_id {
            frame["committed_message_id"] = json!(id);
        }
        self.send(&frame);
    }
}

impl TurnObserver for TurnFramePublisher {
    fn on_event(&mut self, event: &NormalizedAgentEvent) {
        match event {
            NormalizedAgentEvent::TextDelta { text } => self.append(text),
            NormalizedAgentEvent::TextEnded { text } => self.terminal_text(text),
            _ => {}
        }
    }

    fn finish(&mut self) {
        if !self.stopped {
            self.flush();
        }
    }
}

/// Receives each REPORTED activity observation for persistence. Failures are swallowed
/// by the recorder: activity is an observation aid and must never fail a turn.
pub(crate) type ActivitySink =
    Box<dyn Fn(storage_core::conversation::ConversationActivityRecord) -> bool + Send>;

/// Turns tool, file, permission, todo and step events into transient `turn.activity`
/// frames and REPORTED activity records (001-T25, AC-029). Records carry only a bounded
/// display word, never a path, argument, output or reasoning text, and are never Effects.
pub(crate) struct TurnActivityRecorder {
    bus: TransientFrameBus,
    workspace_id: String,
    conversation_id: String,
    turn_id: String,
    retry_ordinal: u32,
    sequence: u32,
    sink: ActivitySink,
}

/// A bounded display word: ASCII letters, digits and `_ - . :` only, at most 64 bytes.
fn activity_label(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
        .take(64)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn activity_call_ref(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
        .take(128)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

impl TurnActivityRecorder {
    pub(crate) fn new(
        bus: TransientFrameBus,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        sink: ActivitySink,
    ) -> Self {
        Self {
            bus,
            workspace_id: workspace_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            turn_id: turn_id.to_owned(),
            retry_ordinal,
            sequence: 0,
            sink,
        }
    }

    fn record(
        &mut self,
        kind: &str,
        status: &str,
        call_ref: Option<String>,
        label: Option<String>,
    ) {
        if self.sequence as usize
            >= storage_core::conversation::CONVERSATION_ACTIVITY_MAX_PER_ATTEMPT
        {
            return;
        }
        self.sequence += 1;
        let record = storage_core::conversation::ConversationActivityRecord {
            turn_id: self.turn_id.clone(),
            retry_ordinal: self.retry_ordinal,
            conversation_id: self.conversation_id.clone(),
            sequence: self.sequence,
            kind: kind.to_owned(),
            status: status.to_owned(),
            call_ref,
            label,
            created_at: crate::agents::supervisor::canonical_now(),
        };
        let frame = json!({
            "type": "turn.activity",
            "workspace_id": self.workspace_id,
            "conversation_id": self.conversation_id,
            "turn_id": self.turn_id,
            "retry_ordinal": self.retry_ordinal,
            "sequence": record.sequence,
            "kind": record.kind,
            "status": record.status,
            "call_ref": record.call_ref,
            "label": record.label,
        });
        // Persist first so a frame never advertises activity that cannot be reconstructed.
        if !(self.sink)(record) {
            return;
        }
        if let Ok(bytes) = serde_json::to_vec(&frame) {
            self.bus.publish(TransientFrame {
                workspace_id: self.workspace_id.clone(),
                conversation_id: self.conversation_id.clone(),
                bytes,
            });
        }
    }
}

impl TurnObserver for TurnActivityRecorder {
    fn on_event(&mut self, event: &NormalizedAgentEvent) {
        use crate::agents::normalizer::ToolOutcome;
        match event {
            NormalizedAgentEvent::ToolCalled { tool, call_id } => {
                self.record(
                    "TOOL",
                    "STARTED",
                    activity_call_ref(call_id),
                    activity_label(tool),
                );
            }
            NormalizedAgentEvent::ToolSettled { call_id, outcome } => {
                let status = if *outcome == ToolOutcome::Succeeded {
                    "SUCCEEDED"
                } else {
                    "FAILED"
                };
                self.record("TOOL", status, activity_call_ref(call_id), None);
            }
            NormalizedAgentEvent::PermissionAsked { .. } => {
                // The native permission ID is a Runtime-local reply key, not a
                // public activity correlation reference or durable record.
                self.record("PERMISSION", "ASKED", None, None);
            }
            NormalizedAgentEvent::FileChanged { .. } => self.record("FILE", "UPDATED", None, None),
            NormalizedAgentEvent::TodoUpdated { count } => {
                self.record("TODO", "UPDATED", None, Some(count.min(&1000).to_string()));
            }
            NormalizedAgentEvent::StepStarted => self.record("STEP", "STARTED", None, None),
            NormalizedAgentEvent::StepEnded => self.record("STEP", "ENDED", None, None),
            // ToolProgress, text, reasoning and usage are not activity records: progress
            // is noise against the per-attempt cap and the others have their own paths.
            _ => {}
        }
    }

    fn finish(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn publisher(bus: &TransientFrameBus) -> TurnFramePublisher {
        TurnFramePublisher::new(bus.clone(), "ws", "conv", "turn", 2)
    }

    fn drain(rx: &mut broadcast::Receiver<TransientFrame>) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            out.push(serde_json::from_slice(&frame.bytes).expect("frame json"));
        }
        out
    }

    #[test]
    fn conversation_turn_delta_frames_are_correlated_sequenced_and_coalesced() {
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        let mut p = publisher(&bus);
        p.on_event(&NormalizedAgentEvent::TextDelta { text: "Hel".into() });
        p.on_event(&NormalizedAgentEvent::TextDelta { text: "lo".into() });
        // Small deltas inside the flush window coalesce into the finish flush.
        assert!(drain(&mut rx).len() <= 1);
        p.finish();
        let frames = drain(&mut rx);
        let all: String = frames
            .iter()
            .filter_map(|f| f["text_delta"].as_str())
            .collect();
        assert_eq!(all, "Hello");
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame["type"], "turn.delta");
            assert_eq!(frame["retry_ordinal"], 2);
            assert_eq!(frame["conversation_id"], "conv");
            assert_eq!(frame["sequence"], index as u64 + 1);
        }
    }

    #[test]
    fn conversation_turn_delta_terminal_text_streams_only_the_unseen_suffix() {
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        let mut p = publisher(&bus);
        p.on_event(&NormalizedAgentEvent::TextDelta { text: "abc".into() });
        p.finish();
        p.on_event(&NormalizedAgentEvent::TextEnded {
            text: "abcdef".into(),
        });
        p.finish();
        let all: String = drain(&mut rx)
            .iter()
            .filter_map(|f| f["text_delta"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(all, "abcdef", "no duplicated or concatenated text");
    }

    #[test]
    fn conversation_turn_delta_divergent_terminal_text_stops_streaming() {
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        let mut p = publisher(&bus);
        p.on_event(&NormalizedAgentEvent::TextDelta { text: "abc".into() });
        p.finish();
        p.on_event(&NormalizedAgentEvent::TextEnded { text: "xyz".into() });
        p.finish();
        let frames = drain(&mut rx);
        assert_eq!(frames.len(), 1, "nothing is published after a divergence");
    }

    #[test]
    fn conversation_turn_delta_large_text_is_chunked_on_char_boundaries_and_capped() {
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        let mut p = publisher(&bus);
        p.on_event(&NormalizedAgentEvent::TextDelta {
            text: "é".repeat(MAX_DELTA_BYTES),
        });
        p.finish();
        let frames = drain(&mut rx);
        assert!(frames.len() >= 2);
        for frame in &frames {
            assert!(frame["text_delta"].as_str().unwrap().len() <= MAX_DELTA_BYTES);
        }
        // Beyond the per-attempt cap, publishing stops instead of growing.
        let mut capped = publisher(&bus);
        capped.on_event(&NormalizedAgentEvent::TextDelta {
            text: "x".repeat(MAX_ATTEMPT_BYTES + 1),
        });
        capped.finish();
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn conversation_turn_delta_slow_subscriber_never_blocks_the_publisher() {
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        // Far more frames than the ring holds, with nobody reading. Several attempts
        // share the bus because one attempt is capped below the ring size times 1 KiB.
        for _ in 0..4 {
            let mut p = publisher(&bus);
            for _ in 0..(BUS_CAPACITY - 1) {
                p.on_event(&NormalizedAgentEvent::TextDelta {
                    text: "y".repeat(FLUSH_BYTES),
                });
            }
        }
        assert!(matches!(
            rx.try_recv(),
            Err(broadcast::error::TryRecvError::Lagged(_))
        ));
        // The survivors carry a sequence beyond 1, which the UI reads as a gap.
        let first = drain(&mut rx).into_iter().next().expect("surviving frame");
        assert!(first["sequence"].as_u64().unwrap() > 1);
    }

    #[test]
    fn conversation_turn_delta_settled_frame_carries_the_committed_message_id() {
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        let mut p = publisher(&bus);
        p.settle(Some("msg-1"));
        let frames = drain(&mut rx);
        assert_eq!(frames[0]["type"], "turn.settled");
        assert_eq!(frames[0]["committed_message_id"], "msg-1");
    }

    #[test]
    fn turn_activity_records_publish_frames_and_persist_reported_rows_without_content() {
        use crate::agents::normalizer::ToolOutcome;
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        let stored = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink_store = stored.clone();
        let mut recorder = TurnActivityRecorder::new(
            bus.clone(),
            "ws",
            "conv",
            "turn",
            1,
            Box::new(move |record| {
                sink_store.lock().unwrap().push(record);
                true
            }),
        );
        recorder.on_event(&NormalizedAgentEvent::ToolCalled {
            tool: "ba sh/../x".into(),
            call_id: "call-1".into(),
        });
        recorder.on_event(&NormalizedAgentEvent::ToolProgress {
            call_id: "call-1".into(),
        });
        recorder.on_event(&NormalizedAgentEvent::ToolSettled {
            call_id: "call-1".into(),
            outcome: ToolOutcome::Failed,
        });
        recorder.on_event(&NormalizedAgentEvent::FileChanged { path_present: true });
        recorder.on_event(&NormalizedAgentEvent::TodoUpdated { count: 3 });
        recorder.on_event(&NormalizedAgentEvent::PermissionAsked {
            permission_id: "perm-1".into(),
        });
        recorder.on_event(&NormalizedAgentEvent::TextDelta {
            text: "ignored".into(),
        });

        let frames = drain(&mut rx);
        assert_eq!(frames.len(), 5, "progress and text are not activity");
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame["type"], "turn.activity");
            assert_eq!(frame["retry_ordinal"], 1);
            assert_eq!(frame["sequence"], index as u64 + 1);
        }
        assert_eq!(
            frames[0]["label"], "bash..x",
            "label keeps only safe characters"
        );
        assert_eq!(frames[1]["status"], "FAILED");
        assert!(
            frames[4]["call_ref"].is_null(),
            "native permission IDs must not enter public frames"
        );
        let rows = stored.lock().unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[3].kind, "TODO");
        assert_eq!(rows[3].label.as_deref(), Some("3"));
        assert_eq!(rows[4].kind, "PERMISSION");
        assert_eq!(
            rows[4].call_ref, None,
            "native permission IDs must not be persisted"
        );
        assert!(rows.iter().all(|r| r.kind != "EFFECT"));
    }

    #[test]
    fn turn_activity_records_are_bounded_and_a_failed_persist_publishes_nothing() {
        let bus = TransientFrameBus::new();
        let mut rx = bus.subscribe();
        let mut refusing =
            TurnActivityRecorder::new(bus.clone(), "ws", "conv", "turn", 0, Box::new(|_| false));
        refusing.on_event(&NormalizedAgentEvent::StepStarted);
        assert!(drain(&mut rx).is_empty());

        let mut capped =
            TurnActivityRecorder::new(bus.clone(), "ws", "conv", "turn", 0, Box::new(|_| true));
        for _ in 0..(storage_core::conversation::CONVERSATION_ACTIVITY_MAX_PER_ATTEMPT + 20) {
            capped.on_event(&NormalizedAgentEvent::StepStarted);
        }
        assert_eq!(
            drain(&mut rx).len(),
            storage_core::conversation::CONVERSATION_ACTIVITY_MAX_PER_ATTEMPT
        );
    }
}
