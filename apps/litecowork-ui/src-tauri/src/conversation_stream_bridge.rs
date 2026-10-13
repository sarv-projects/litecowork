use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, OnceLock,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};

const STREAM_EVENT: &str = "conversation-presentation-stream";
const TASK_STREAM_EVENT: &str = "task-presentation-stream";
const RUNTIME_INVENTORY_STREAM_EVENT: &str = "runtime-inventory-stream";
const SAFE_STREAM_ERROR: &str =
    "Live saved-conversation updates are unavailable. Reconnect to try again.";
const MAX_SESSION_ID_BYTES: usize = 128;
const MAX_ID_BYTES: usize = 256;
const MAX_CURSOR_BYTES: usize = 2048;

static NEXT_SESSION_TOKEN: AtomicU64 = AtomicU64::new(1);
static CANCELLATIONS: OnceLock<Mutex<HashMap<String, CancellationEntry>>> = OnceLock::new();
static FRAME_ACKNOWLEDGEMENTS: OnceLock<FrameAcknowledgements> = OnceLock::new();

struct CancellationEntry {
    token: u64,
    sender: oneshot::Sender<()>,
}

#[derive(Default, Clone)]
struct FrameAcknowledgements {
    pending: Arc<Mutex<HashMap<String, PendingFrameAcknowledgement>>>,
}

struct PendingFrameAcknowledgement {
    frame_id: String,
    sender: oneshot::Sender<()>,
}

struct FrameAcknowledgementGuard {
    pending: Arc<Mutex<HashMap<String, PendingFrameAcknowledgement>>>,
    session_id: String,
    frame_id: String,
}

impl Drop for FrameAcknowledgementGuard {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.pending.lock() {
            if pending
                .get(&self.session_id)
                .is_some_and(|entry| entry.frame_id == self.frame_id)
            {
                pending.remove(&self.session_id);
            }
        }
    }
}

impl FrameAcknowledgements {
    fn register(
        &self,
        session_id: &str,
        frame_id: &str,
    ) -> Result<(oneshot::Receiver<()>, FrameAcknowledgementGuard), String> {
        if !valid_session_id(session_id) || !valid_session_id(frame_id) {
            return Err("Conversation stream frame acknowledgement is invalid".to_owned());
        }
        let (sender, receiver) = oneshot::channel();
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "Conversation stream is unavailable".to_owned())?;
        if pending.contains_key(session_id) {
            return Err("Conversation stream already has an unacknowledged frame".to_owned());
        }
        pending.insert(
            session_id.to_owned(),
            PendingFrameAcknowledgement {
                frame_id: frame_id.to_owned(),
                sender,
            },
        );
        Ok((
            receiver,
            FrameAcknowledgementGuard {
                pending: Arc::clone(&self.pending),
                session_id: session_id.to_owned(),
                frame_id: frame_id.to_owned(),
            },
        ))
    }

    fn acknowledge(&self, session_id: &str, frame_id: &str) -> Result<(), String> {
        if !valid_session_id(session_id) || !valid_session_id(frame_id) {
            return Err("Conversation stream frame acknowledgement is invalid".to_owned());
        }
        let entry = {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| "Conversation stream is unavailable".to_owned())?;
            if !pending
                .get(session_id)
                .is_some_and(|entry| entry.frame_id == frame_id)
            {
                return Err("Conversation stream frame acknowledgement is stale".to_owned());
            }
            pending.remove(session_id)
        };
        entry
            .expect("matching pending acknowledgement was removed")
            .sender
            .send(())
            .map_err(|_| "Conversation stream frame acknowledgement expired".to_owned())
    }
}

struct SessionRegistryGuard {
    session_id: String,
    token: u64,
}

impl Drop for SessionRegistryGuard {
    fn drop(&mut self) {
        remove_if_current(&self.session_id, self.token);
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum StreamEventKind {
    Frame,
    Closed,
    Error,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamEventEnvelope {
    session_id: String,
    workspace_id: String,
    conversation_id: String,
    kind: StreamEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskStreamEventEnvelope {
    session_id: String,
    workspace_id: String,
    task_id: String,
    kind: StreamEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInventoryStreamEventEnvelope {
    session_id: String,
    workspace_id: String,
    runtime_id: String,
    kind: StreamEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Clone)]
enum StreamTarget {
    Conversation(String),
    Task(String),
    RuntimeInventory(String),
}

impl StreamTarget {
    fn id(&self) -> &str {
        match self {
            Self::Conversation(id) | Self::Task(id) | Self::RuntimeInventory(id) => id,
        }
    }

    fn into_local_target(self) -> super::LocalPresentationStreamTarget {
        match self {
            Self::Conversation(id) => super::LocalPresentationStreamTarget::Conversation(id),
            Self::Task(id) => super::LocalPresentationStreamTarget::Task(id),
            Self::RuntimeInventory(id) => {
                super::LocalPresentationStreamTarget::RuntimeInventory(id)
            }
        }
    }

    fn emit(
        &self,
        app: &AppHandle,
        session_id: &str,
        workspace_id: &str,
        kind: StreamEventKind,
        frame_id: Option<&str>,
        frame: Option<Value>,
        message: Option<&str>,
    ) -> Result<(), String> {
        match self {
            Self::Conversation(conversation_id) => app
                .emit(
                    STREAM_EVENT,
                    envelope(
                        session_id,
                        workspace_id,
                        conversation_id,
                        kind,
                        frame_id,
                        frame,
                        message,
                    ),
                )
                .map_err(|_| "Conversation stream listener is unavailable".to_owned()),
            Self::Task(task_id) => app
                .emit(
                    TASK_STREAM_EVENT,
                    TaskStreamEventEnvelope {
                        session_id: session_id.to_owned(),
                        workspace_id: workspace_id.to_owned(),
                        task_id: task_id.clone(),
                        kind,
                        frame_id: frame_id.map(str::to_owned),
                        frame,
                        message: message.map(str::to_owned),
                    },
                )
                .map_err(|_| "Task stream listener is unavailable".to_owned()),
            Self::RuntimeInventory(runtime_id) => app
                .emit(
                    RUNTIME_INVENTORY_STREAM_EVENT,
                    RuntimeInventoryStreamEventEnvelope {
                        session_id: session_id.to_owned(),
                        workspace_id: workspace_id.to_owned(),
                        runtime_id: runtime_id.clone(),
                        kind,
                        frame_id: frame_id.map(str::to_owned),
                        frame,
                        message: message.map(str::to_owned),
                    },
                )
                .map_err(|_| "Runtime inventory stream listener is unavailable".to_owned()),
        }
    }
}

fn valid_id(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_session_id(value: &str) -> bool {
    valid_id(value, MAX_SESSION_ID_BYTES)
}

fn valid_resume_cursor(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_CURSOR_BYTES && !value.chars().any(char::is_control)
}

fn envelope(
    session_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    kind: StreamEventKind,
    frame_id: Option<&str>,
    frame: Option<Value>,
    message: Option<&str>,
) -> StreamEventEnvelope {
    StreamEventEnvelope {
        session_id: session_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        kind,
        frame_id: frame_id.map(str::to_owned),
        frame,
        message: message.map(str::to_owned),
    }
}

fn frame_acknowledgements() -> &'static FrameAcknowledgements {
    FRAME_ACKNOWLEDGEMENTS.get_or_init(FrameAcknowledgements::default)
}

async fn wait_for_frame_acknowledgement(
    acknowledged: oneshot::Receiver<()>,
    maximum_wait: Duration,
) -> Result<(), String> {
    timeout(maximum_wait, acknowledged)
        .await
        .map_err(|_| "Conversation stream frame acknowledgement timed out".to_owned())?
        .map_err(|_| "Conversation stream frame acknowledgement expired".to_owned())
}

fn cancellation_registry() -> &'static Mutex<HashMap<String, CancellationEntry>> {
    CANCELLATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remove_if_current(session_id: &str, token: u64) {
    if let Ok(mut registry) = cancellation_registry().lock() {
        if registry
            .get(session_id)
            .is_some_and(|entry| entry.token == token)
        {
            registry.remove(session_id);
        }
    }
}

#[tauri::command]
pub(crate) async fn subscribe_conversation_stream(
    app: AppHandle,
    session_id: String,
    workspace_id: String,
    conversation_id: String,
    resume_cursor: Option<String>,
) -> Result<(), String> {
    subscribe_stream(
        app,
        session_id,
        workspace_id,
        StreamTarget::Conversation(conversation_id),
        resume_cursor,
    )
    .await
}

#[tauri::command]
pub(crate) async fn subscribe_task_stream(
    app: AppHandle,
    session_id: String,
    workspace_id: String,
    task_id: String,
    resume_cursor: Option<String>,
) -> Result<(), String> {
    subscribe_stream(
        app,
        session_id,
        workspace_id,
        StreamTarget::Task(task_id),
        resume_cursor,
    )
    .await
}

#[tauri::command]
pub(crate) async fn subscribe_runtime_inventory_stream(
    app: AppHandle,
    session_id: String,
    workspace_id: String,
    runtime_id: String,
    resume_cursor: Option<String>,
) -> Result<(), String> {
    subscribe_stream(
        app,
        session_id,
        workspace_id,
        StreamTarget::RuntimeInventory(runtime_id),
        resume_cursor,
    )
    .await
}

async fn subscribe_stream(
    app: AppHandle,
    session_id: String,
    workspace_id: String,
    target: StreamTarget,
    resume_cursor: Option<String>,
) -> Result<(), String> {
    if !valid_session_id(&session_id)
        || !valid_id(&workspace_id, MAX_ID_BYTES)
        || !valid_id(target.id(), MAX_ID_BYTES)
        || resume_cursor
            .as_deref()
            .is_some_and(|cursor| !valid_resume_cursor(cursor))
    {
        return Err("Presentation stream selection is invalid".to_owned());
    }

    let token = NEXT_SESSION_TOKEN.fetch_add(1, Ordering::Relaxed);
    let (cancel_tx, cancel_rx) = oneshot::channel();
    {
        let mut registry = cancellation_registry()
            .lock()
            .map_err(|_| "Conversation stream is unavailable".to_owned())?;
        if registry.contains_key(&session_id) {
            return Err("Conversation stream session is already active".to_owned());
        }
        registry.insert(
            session_id.clone(),
            CancellationEntry {
                token,
                sender: cancel_tx,
            },
        );
    }

    let task_session_id = session_id.clone();
    let task_workspace_id = workspace_id.clone();
    let frame_sequence = Arc::new(AtomicU64::new(1));
    let registry_guard = SessionRegistryGuard {
        session_id: task_session_id.clone(),
        token,
    };
    tauri::async_runtime::spawn(async move {
        let _registry_guard = registry_guard;
        let frame_app = app.clone();
        let frame_session_id = task_session_id.clone();
        let frame_workspace_id = task_workspace_id.clone();
        let frame_target = target.clone();
        let frame_sequence = Arc::clone(&frame_sequence);
        let acknowledgements = frame_acknowledgements().clone();
        let result = super::read_local_presentation_stream(
            app.clone(),
            task_workspace_id.clone(),
            target.clone().into_local_target(),
            resume_cursor,
            cancel_rx,
            move |frame| {
                let frame_app = frame_app.clone();
                let frame_session_id = frame_session_id.clone();
                let frame_workspace_id = frame_workspace_id.clone();
                let frame_target = frame_target.clone();
                let acknowledgements = acknowledgements.clone();
                let frame_sequence = Arc::clone(&frame_sequence);
                async move {
                    let sequence = frame_sequence
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                            current.checked_add(1)
                        })
                        .map_err(|_| {
                            "Conversation stream frame sequence is exhausted".to_owned()
                        })?;
                    let frame_id = format!("{token}-{sequence}");
                    let (acknowledged, _guard) =
                        acknowledgements.register(&frame_session_id, &frame_id)?;
                    frame_target.emit(
                        &frame_app,
                        &frame_session_id,
                        &frame_workspace_id,
                        StreamEventKind::Frame,
                        Some(&frame_id),
                        Some(frame),
                        None,
                    )?;
                    wait_for_frame_acknowledgement(acknowledged, Duration::from_secs(5)).await
                }
            },
        )
        .await;

        let (kind, message) = match result {
            Ok(()) => (StreamEventKind::Closed, None),
            Err(_) => (StreamEventKind::Error, Some(SAFE_STREAM_ERROR)),
        };
        let _ = target.emit(
            &app,
            &task_session_id,
            &task_workspace_id,
            kind,
            None,
            None,
            message,
        );
    });

    Ok(())
}

#[tauri::command]
pub(crate) async fn unsubscribe_conversation_stream(session_id: String) -> Result<(), String> {
    if !valid_session_id(&session_id) {
        return Err("Conversation stream session is invalid".to_owned());
    }
    let entry = cancellation_registry()
        .lock()
        .map_err(|_| "Conversation stream is unavailable".to_owned())?
        .remove(&session_id);
    if let Some(entry) = entry {
        let _ = entry.sender.send(());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn acknowledge_conversation_stream_frame(
    session_id: String,
    frame_id: String,
) -> Result<(), String> {
    frame_acknowledgements().acknowledge(&session_id, &frame_id)
}

#[cfg(test)]
mod tests {
    use super::{
        envelope, valid_id, valid_resume_cursor, valid_session_id, FrameAcknowledgements,
        StreamEventKind, TaskStreamEventEnvelope,
    };
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[test]
    fn validates_session_ids_and_selected_object_ids() {
        assert!(valid_session_id("session-01_ab"));
        assert!(!valid_session_id(""));
        assert!(!valid_session_id("session/01"));
        assert!(!valid_session_id(&"s".repeat(129)));
        assert!(valid_id("workspace-1", 256));
        assert!(!valid_id("workspace 1", 256));
    }

    #[test]
    fn accepts_only_bounded_nonempty_control_free_resume_cursors() {
        assert!(valid_resume_cursor("opaque.cursor_1"));
        assert!(!valid_resume_cursor(""));
        assert!(!valid_resume_cursor("cursor\nforged"));
        assert!(!valid_resume_cursor(&"c".repeat(2049)));
    }

    #[test]
    fn serializes_the_frame_envelope_with_camel_case_identity_and_optional_fields() {
        let frame = envelope(
            "session-1",
            "workspace-1",
            "conversation-1",
            StreamEventKind::Frame,
            Some("9-1"),
            Some(json!({"type": "stream.ready"})),
            None,
        );
        assert_eq!(
            serde_json::to_value(frame).expect("event serializes"),
            json!({
                "sessionId": "session-1",
                "workspaceId": "workspace-1",
                "conversationId": "conversation-1",
                "kind": "frame",
                "frameId": "9-1",
                "frame": {"type": "stream.ready"}
            })
        );
    }

    #[test]
    fn serializes_task_stream_identity_without_conversation_fields() {
        let event = TaskStreamEventEnvelope {
            session_id: "session-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            task_id: "task-1".to_owned(),
            kind: StreamEventKind::Frame,
            frame_id: Some("7-1".to_owned()),
            frame: Some(json!({"type": "stream.ready"})),
            message: None,
        };
        assert_eq!(
            serde_json::to_value(event).expect("Task stream event serializes"),
            json!({
                "sessionId": "session-1",
                "workspaceId": "workspace-1",
                "taskId": "task-1",
                "kind": "frame",
                "frameId": "7-1",
                "frame": {"type": "stream.ready"}
            })
        );
    }

    #[test]
    fn serializes_safe_terminal_event_shapes_without_irrelevant_fields() {
        let closed = envelope(
            "session-1",
            "workspace-1",
            "conversation-1",
            StreamEventKind::Closed,
            None,
            None,
            None,
        );
        assert_eq!(
            serde_json::to_value(closed).expect("event serializes"),
            json!({
                "sessionId": "session-1",
                "workspaceId": "workspace-1",
                "conversationId": "conversation-1",
                "kind": "closed"
            })
        );
    }

    #[tokio::test]
    async fn frame_acknowledgements_are_single_slot_and_fenced_to_session_and_frame() {
        let acknowledgements = FrameAcknowledgements::default();
        let (receiver, _guard) = acknowledgements
            .register("session-1", "frame-1")
            .expect("first frame registers");

        assert!(acknowledgements.register("session-1", "frame-2").is_err());
        assert!(acknowledgements
            .acknowledge("session-2", "frame-1")
            .is_err());
        assert!(acknowledgements
            .acknowledge("session-1", "stale-frame")
            .is_err());
        assert!(acknowledgements.acknowledge("session-1", "frame-1").is_ok());
        assert!(receiver.await.is_ok());
        assert!(acknowledgements
            .acknowledge("session-1", "frame-1")
            .is_err());
    }

    #[tokio::test]
    async fn an_unacknowledged_frame_times_out() {
        let (_sender, receiver) = oneshot::channel();
        let result =
            super::wait_for_frame_acknowledgement(receiver, Duration::from_millis(1)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[test]
    fn dropping_a_frame_ack_guard_releases_the_pending_slot() {
        let acknowledgements = FrameAcknowledgements::default();
        let (_receiver, guard) = acknowledgements
            .register("session-1", "frame-1")
            .expect("frame registers");
        drop(guard);

        let (_next_receiver, _next_guard) = acknowledgements
            .register("session-1", "frame-2")
            .expect("cancelled frame releases its slot");
    }
}
