//! Read-only, bounded Task presentation snapshots for the Operator.
//!
//! This module deliberately projects committed Task, Step, current Attempt, Evidence,
//! and Artifact records. It does not infer worker liveness, provider progress, progress
//! percentages, verification outcomes, or stream state. Mount [`routes`] from
//! `operator::build_operator_router` after adding
//! `#[path = "presentation_operator.rs"] mod presentation_operator;` alongside the
//! other Operator submodules.

#![allow(clippy::result_large_err)] // Handlers return full Axum Responses to preserve status, body, and headers.

use super::*;
use axum::body::Bytes;
use futures_util::stream;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use storage_core::{
    TaskPresentationActivityEvent, TaskPresentationReadModel, TaskPresentationReadStore,
    rich_presentation::{
        ConversationPresentationRead, MAX_CONVERSATION_PRESENTATION_BATCH,
        MAX_CONVERSATION_PRESENTATION_MESSAGES, MAX_RICH_PRESENTATION_BYTES,
        RichPresentationRecord, RichPresentationStore,
    },
};
use storage_sqlite::SqliteRichPresentationStore;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/stream",
            post(subscribe_stream).layer(DefaultBodyLimit::max(
                operator_ipc::MAX_STREAM_SUBSCRIBE_BYTES,
            )),
        )
        .route(
            "/v1/conversations/{conversation_id}/presentation",
            get(get_conversation_presentation),
        )
        .route(
            "/v1/tasks/{task_id}/presentation",
            get(get_task_presentation),
        )
        .route("/v1/tasks/{task_id}/progress", get(get_task_progress))
        .route(
            "/v1/rich-presentations/{presentation_id}",
            get(get_rich_presentation),
        )
}

#[derive(Clone)]
pub(super) struct StreamApiState {
    pub(super) store: SqliteWorkspaceStore,
    pub(super) principal_id: String,
    pub(super) runtime_id: String,
    pub(super) local_incarnation_id: String,
    pub(super) runtime_inventory_revision: Arc<AtomicU64>,
    pub(super) stream_admission: Arc<tokio::sync::Semaphore>,
    pub(super) stream_shutdown: tokio::sync::watch::Receiver<bool>,
    #[cfg(test)]
    pub(super) task_stream_test_gate: Option<Arc<TaskStreamTestGate>>,
    #[cfg(test)]
    pub(super) task_stream_snapshot_test_gate: Option<Arc<TaskStreamTestGate>>,
}

#[cfg(test)]
pub(super) struct TaskStreamTestGate {
    pub(super) reached: tokio::sync::mpsc::UnboundedSender<()>,
    pub(super) release: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

#[cfg(test)]
impl TaskStreamTestGate {
    async fn pause(&self) {
        let _ = self.reached.send(());
        if let Some(release) = self.release.lock().await.take() {
            let _ = release.await;
        }
    }
}

impl axum::extract::FromRef<ApiState> for StreamApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            runtime_id: state.runtime_id.clone(),
            local_incarnation_id: state.local_incarnation_id.clone(),
            runtime_inventory_revision: Arc::clone(&state.runtime_inventory_revision),
            stream_admission: Arc::clone(&state.stream_admission),
            stream_shutdown: state.stream_shutdown.clone(),
            #[cfg(test)]
            task_stream_test_gate: None,
            #[cfg(test)]
            task_stream_snapshot_test_gate: None,
        }
    }
}

const STREAM_PROJECTION_VERSION: u32 = 1;
const STREAM_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StreamCursor {
    version: u32,
    workspace_id: String,
    origin_runtime_id: String,
    conversation_id: String,
    projection_version: u32,
    origin_sequence: u64,
    #[serde(default)]
    message_frontier: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConversationBatchStreamCursor {
    version: u32,
    workspace_id: String,
    origin_runtime_id: String,
    filter_digest: String,
    projection_version: u32,
    origin_sequence: u64,
    message_frontiers: Vec<Option<String>>,
    projection_revisions: Vec<u64>,
    snapshot_complete: bool,
}

const MAX_BATCH_STREAM_CURSOR_BYTES: usize = 14 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskStreamCursor {
    version: u32,
    workspace_id: String,
    origin_runtime_id: String,
    task_id: String,
    projection_version: u32,
    origin_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeInventoryStreamCursor {
    version: u32,
    workspace_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    projection_version: u32,
    revision: u64,
}

#[derive(Serialize)]
struct RuntimeInventoryNotification<'a> {
    runtime_id: &'a str,
    change_kind: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StreamSubscribe {
    workspace_id: String,
    #[serde(default)]
    conversation_ids: Vec<String>,
    #[serde(default)]
    task_ids: Vec<String>,
    #[serde(default)]
    runtime_ids: Vec<String>,
    projection_types: Vec<String>,
    resume_cursor: Option<String>,
}

#[derive(Serialize)]
struct StreamReady<'a> {
    #[serde(rename = "type")]
    frame_type: &'static str,
    workspace_id: &'a str,
    projection_version: u32,
    cursor: String,
    max_frame_bytes: usize,
}

#[derive(Serialize)]
struct StreamResyncRequired<'a> {
    #[serde(rename = "type")]
    frame_type: &'static str,
    workspace_id: &'a str,
    reason: &'static str,
}

#[derive(Serialize)]
struct StreamProjection<'a, T: Serialize> {
    #[serde(rename = "type")]
    frame_type: &'static str,
    workspace_id: &'a str,
    projection_type: &'static str,
    projection_revision: u64,
    cursor: String,
    payload: &'a T,
}

#[derive(Serialize)]
struct TaskStreamPayload {
    #[serde(flatten)]
    snapshot: TaskPresentationSnapshot,
    origin_sequence: String,
}

#[derive(Serialize)]
struct ConversationProjectionUpsert<'a> {
    #[serde(rename = "type")]
    frame_type: &'static str,
    workspace_id: &'a str,
    projection_type: &'static str,
    projection_revision: u64,
    cursor: String,
    payload: ConversationProjectionUpsertPayload<'a>,
}

#[derive(Serialize)]
struct ConversationProjectionUpsertPayload<'a> {
    conversation_id: &'a str,
    stream_cursor: &'a str,
    origin_sequence: &'a str,
    item: &'a serde_json::Value,
}

pub(super) async fn subscribe_stream(
    State(state): State<StreamApiState>,
    headers: HeaderMap,
    Json(subscribe): Json<StreamSubscribe>,
) -> Result<Response, Response> {
    let selected = selected_workspace(&headers)?;
    if selected != subscribe.workspace_id {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Stream Workspace does not match the selected Workspace",
        ));
    }
    if valid_task_query_id(&selected)
        && subscribe.conversation_ids.is_empty()
        && subscribe.task_ids.is_empty()
        && subscribe.runtime_ids.len() == 1
        && subscribe.projection_types == ["runtime_inventory"]
        && valid_task_query_id(&subscribe.runtime_ids[0])
    {
        let runtime_id = subscribe.runtime_ids[0].clone();
        let resume_cursor = subscribe.resume_cursor.clone();
        return subscribe_runtime_inventory_stream(state, selected, runtime_id, resume_cursor)
            .await;
    }
    if valid_task_query_id(&selected)
        && subscribe.conversation_ids.is_empty()
        && subscribe.runtime_ids.is_empty()
        && subscribe.task_ids.len() == 1
        && subscribe.projection_types == ["task_presentation"]
        && valid_task_query_id(&subscribe.task_ids[0])
    {
        let task_id = subscribe.task_ids[0].clone();
        let resume_cursor = subscribe.resume_cursor.clone();
        return subscribe_task_stream(state, selected, task_id, resume_cursor).await;
    }
    if !valid_task_query_id(&selected)
        || subscribe.conversation_ids.is_empty()
        || subscribe.conversation_ids.len() > MAX_CONVERSATION_PRESENTATION_BATCH
        || !subscribe.task_ids.is_empty()
        || !subscribe.runtime_ids.is_empty()
        || subscribe.projection_types != ["conversation_presentation"]
    {
        return Err(presentation_error(
            StatusCode::NOT_IMPLEMENTED,
            "UNSUPPORTED_PROJECTION",
            "This local stream supports only Conversation, Task, or Runtime inventory projections",
        ));
    }
    let mut conversation_ids = subscribe.conversation_ids;
    if conversation_ids.iter().any(|id| !valid_task_query_id(id)) {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation stream filters are invalid",
        ));
    }
    conversation_ids.sort();
    if conversation_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation stream filters are invalid",
        ));
    }
    if conversation_ids.len() > 1 {
        return subscribe_conversation_batch_stream(
            state,
            selected,
            conversation_ids,
            subscribe.resume_cursor,
        )
        .await;
    }
    let conversation_id = conversation_ids[0].clone();
    let stream_permit = Arc::clone(&state.stream_admission)
        .try_acquire_owned()
        .map_err(|_| {
            presentation_error(
                StatusCode::TOO_MANY_REQUESTS,
                "CAPACITY_EXCEEDED",
                "The local stream session limit has been reached",
            )
        })?;

    let snapshot = tokio::task::spawn_blocking({
        let store = state.store.clone();
        let principal_id = state.principal_id.clone();
        let runtime_id = state.runtime_id.clone();
        let workspace_id = selected.clone();
        let conversation_id = conversation_id.clone();
        move || {
            read_stream_conversation_snapshot(
                &store,
                &principal_id,
                &runtime_id,
                &workspace_id,
                &conversation_id,
            )
        }
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Conversation stream snapshot is unavailable",
        )
    })??;
    let snapshot_sequence = snapshot.origin_sequence.parse::<u64>().map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Conversation stream snapshot is unavailable",
        )
    })?;
    let cursor = StreamCursor {
        version: 1,
        workspace_id: selected.clone(),
        origin_runtime_id: state.runtime_id.clone(),
        conversation_id: conversation_id.clone(),
        projection_version: STREAM_PROJECTION_VERSION,
        origin_sequence: snapshot_sequence,
        message_frontier: snapshot.stream_message_frontier.clone(),
    };
    let (resume_cursor, should_resync) = validate_resume_cursor(
        subscribe.resume_cursor.as_deref(),
        &selected,
        &state.runtime_id,
        &conversation_id,
        &state.store,
    )
    .await;
    let first_message_cursor = resume_cursor
        .as_ref()
        .and_then(|cursor| cursor.message_frontier.clone())
        .or_else(|| {
            should_resync
                .then(|| {
                    snapshot.items.last().and_then(|item| {
                        item.get("message")?
                            .get("message_id")?
                            .as_str()
                            .map(str::to_owned)
                    })
                })
                .flatten()
        })
        .or_else(|| snapshot.stream_message_frontier.clone());
    let mut emitted_cursor = if !should_resync {
        resume_cursor.clone().unwrap_or(cursor)
    } else {
        cursor
    };
    if should_resync {
        emitted_cursor.message_frontier = first_message_cursor.clone();
    }
    let current_cursor = encode_stream_cursor(&emitted_cursor)?;

    let mut initial = Vec::with_capacity(3);
    initial.push(serialize_stream_frame(&StreamReady {
        frame_type: "stream.ready",
        workspace_id: &selected,
        projection_version: STREAM_PROJECTION_VERSION,
        cursor: current_cursor.clone(),
        max_frame_bytes: operator_ipc::MAX_STREAM_FRAME_BYTES,
    })?);
    if should_resync {
        initial.push(serialize_stream_frame(&StreamResyncRequired {
            frame_type: "stream.resync_required",
            workspace_id: &selected,
            reason: "resume_cursor_invalid_or_expired",
        })?);
    }
    initial.push(serialize_stream_frame(&StreamProjection {
        frame_type: "projection",
        workspace_id: &selected,
        projection_type: "conversation_presentation",
        projection_revision: snapshot.projection_revision,
        cursor: current_cursor,
        payload: &snapshot,
    })?);

    let shutdown = state.stream_shutdown.clone();
    let store = state.store;
    let principal_id = state.principal_id;
    let runtime_id = state.runtime_id;
    let workspace_id = selected;
    // Start polling from the frontier captured in the exact snapshot transaction.
    // In particular, never advance to a resume cursor newer than that snapshot: a
    // concurrent writer could otherwise be skipped between the two reads.
    let first_sequence = if !should_resync {
        resume_cursor
            .as_ref()
            .map_or(snapshot_sequence, |cursor| cursor.origin_sequence)
    } else {
        snapshot_sequence
    };
    let first_catch_up = resume_cursor.is_some() || should_resync;
    let initial = initial.into_iter();
    let output = stream::unfold(
        (
            stream_permit,
            shutdown,
            store,
            principal_id,
            runtime_id,
            workspace_id,
            conversation_id,
            first_sequence,
            first_message_cursor,
            first_catch_up,
            initial,
        ),
        move |(
            permit,
            mut shutdown,
            store,
            principal_id,
            runtime_id,
            workspace_id,
            conversation_id,
            mut sequence,
            mut message_cursor,
            mut catch_up,
            mut initial,
        )| async move {
            if let Some(frame) = initial.next() {
                return Some((
                    Ok::<Bytes, std::convert::Infallible>(Bytes::from(frame)),
                    (
                        permit,
                        shutdown,
                        store,
                        principal_id,
                        runtime_id,
                        workspace_id,
                        conversation_id,
                        sequence,
                        message_cursor,
                        catch_up,
                        initial,
                    ),
                ));
            }
            if catch_up {
                // Cursor validation precedes the lazy body stream. Recheck its
                // retained event range here so history archived in that window
                // produces an explicit resync instead of a silent cursor advance.
                let store_for_page = store.clone();
                let workspace_for_page = workspace_id.clone();
                let runtime_for_page = runtime_id.clone();
                let page = tokio::task::spawn_blocking(move || {
                    use storage_core::EventStore;
                    store_for_page.read_workspace_event_page(
                        &workspace_for_page,
                        &runtime_for_page,
                        sequence,
                        500,
                    )
                })
                .await;
                let page = match page {
                    Ok(Ok(page)) => page,
                    _ => return None,
                };
                if page.cursor_is_ahead || page.has_gap {
                    let snapshot_for_read = store.clone();
                    let workspace_for_read = workspace_id.clone();
                    let conversation_for_read = conversation_id.clone();
                    let principal_for_read = principal_id.clone();
                    let runtime_for_read = runtime_id.clone();
                    let latest = tokio::task::spawn_blocking(move || {
                        read_stream_conversation_snapshot(
                            &snapshot_for_read,
                            &principal_for_read,
                            &runtime_for_read,
                            &workspace_for_read,
                            &conversation_for_read,
                        )
                    })
                    .await;
                    let Ok(Ok(latest)) = latest else {
                        return None;
                    };
                    let Ok(next_sequence) = latest.origin_sequence.parse::<u64>() else {
                        return None;
                    };
                    let cursor = StreamCursor {
                        version: 1,
                        workspace_id: workspace_id.clone(),
                        origin_runtime_id: runtime_id.clone(),
                        conversation_id: conversation_id.clone(),
                        projection_version: STREAM_PROJECTION_VERSION,
                        origin_sequence: next_sequence,
                        message_frontier: message_cursor.clone(),
                    };
                    let Ok(cursor) = encode_stream_cursor(&cursor) else {
                        return None;
                    };
                    let Ok(resync) = serialize_stream_frame(&StreamResyncRequired {
                        frame_type: "stream.resync_required",
                        workspace_id: &workspace_id,
                        reason: "replay_history_gap",
                    }) else {
                        return None;
                    };
                    let Ok(projection) = serialize_stream_frame(&StreamProjection {
                        frame_type: "projection",
                        workspace_id: &workspace_id,
                        projection_type: "conversation_presentation",
                        projection_revision: latest.projection_revision,
                        cursor,
                        payload: &latest,
                    }) else {
                        return None;
                    };
                    sequence = next_sequence;
                    catch_up = latest.stream_message_frontier != message_cursor;
                    initial = vec![resync, projection].into_iter();
                    let frame = initial.next()?;
                    return Some((
                        Ok(Bytes::from(frame)),
                        (
                            permit,
                            shutdown,
                            store,
                            principal_id,
                            runtime_id,
                            workspace_id,
                            conversation_id,
                            sequence,
                            message_cursor,
                            catch_up,
                            initial,
                        ),
                    ));
                }
                let store_for_read = store.clone();
                let principal_for_read = principal_id.clone();
                let runtime_for_read = runtime_id.clone();
                let workspace_for_read = workspace_id.clone();
                let conversation_for_read = conversation_id.clone();
                let cursor_for_read = message_cursor.clone();
                let page = tokio::task::spawn_blocking(move || {
                    read_stream_conversation_page(
                        &store_for_read,
                        &principal_for_read,
                        &runtime_for_read,
                        &workspace_for_read,
                        &conversation_for_read,
                        cursor_for_read.as_deref(),
                    )
                })
                .await;
                let Ok(Ok(page)) = page else {
                    return None;
                };
                let page_frames = match serialize_conversation_stream_page(
                    &page,
                    &workspace_id,
                    &runtime_id,
                    &conversation_id,
                ) {
                    Ok(result) => result,
                    Err(_) => return None,
                };
                let Ok(next_sequence) = page.origin_sequence.parse::<u64>() else {
                    return None;
                };
                sequence = next_sequence;
                message_cursor = page_frames.message_frontier;
                catch_up = page_frames.has_more;
                initial = page_frames.frames.into_iter();
                let frame = initial.next()?;
                return Some((
                    Ok(Bytes::from(frame)),
                    (
                        permit,
                        shutdown,
                        store,
                        principal_id,
                        runtime_id,
                        workspace_id,
                        conversation_id,
                        sequence,
                        message_cursor,
                        catch_up,
                        initial,
                    ),
                ));
            }
            loop {
                tokio::select! {
                    _ = shutdown.changed() => return None,
                    _ = tokio::time::sleep(STREAM_POLL_INTERVAL) => {}
                }
                let store_for_page = store.clone();
                let workspace_for_page = workspace_id.clone();
                let runtime_for_page = runtime_id.clone();
                let page = tokio::task::spawn_blocking(move || {
                    use storage_core::EventStore;
                    store_for_page.read_workspace_event_page(
                        &workspace_for_page,
                        &runtime_for_page,
                        sequence,
                        500,
                    )
                })
                .await;
                let page = match page {
                    Ok(Ok(page)) => page,
                    _ => return None,
                };
                if page.cursor_is_ahead || page.has_gap {
                    let snapshot_for_read = store.clone();
                    let workspace_for_read = workspace_id.clone();
                    let conversation_for_read = conversation_id.clone();
                    let principal_for_read = principal_id.clone();
                    let runtime_for_read = runtime_id.clone();
                    let latest = tokio::task::spawn_blocking(move || {
                        read_stream_conversation_snapshot(
                            &snapshot_for_read,
                            &principal_for_read,
                            &runtime_for_read,
                            &workspace_for_read,
                            &conversation_for_read,
                        )
                    })
                    .await;
                    let Ok(Ok(latest)) = latest else {
                        return None;
                    };
                    let Ok(next_sequence) = latest.origin_sequence.parse::<u64>() else {
                        return None;
                    };
                    let cursor = StreamCursor {
                        version: 1,
                        workspace_id: workspace_id.clone(),
                        origin_runtime_id: runtime_id.clone(),
                        conversation_id: conversation_id.clone(),
                        projection_version: STREAM_PROJECTION_VERSION,
                        origin_sequence: next_sequence,
                        message_frontier: message_cursor.clone(),
                    };
                    let Ok(cursor) = encode_stream_cursor(&cursor) else {
                        return None;
                    };
                    let resync = StreamResyncRequired {
                        frame_type: "stream.resync_required",
                        workspace_id: &workspace_id,
                        reason: "replay_history_gap",
                    };
                    let projection = StreamProjection {
                        frame_type: "projection",
                        workspace_id: &workspace_id,
                        projection_type: "conversation_presentation",
                        projection_revision: latest.projection_revision,
                        cursor: cursor.clone(),
                        payload: &latest,
                    };
                    let Ok(resync) = serialize_stream_frame(&resync) else {
                        return None;
                    };
                    let Ok(projection) = serialize_stream_frame(&projection) else {
                        return None;
                    };
                    sequence = next_sequence;
                    catch_up = latest.stream_message_frontier != message_cursor;
                    initial = vec![resync, projection].into_iter();
                    if let Some(frame) = initial.next() {
                        return Some((
                            Ok(Bytes::from(frame)),
                            (
                                permit,
                                shutdown,
                                store,
                                principal_id,
                                runtime_id,
                                workspace_id,
                                conversation_id,
                                sequence,
                                message_cursor,
                                catch_up,
                                initial,
                            ),
                        ));
                    }
                    continue;
                }
                if page.head_sequence > sequence {
                    let snapshot_for_read = store.clone();
                    let workspace_for_read = workspace_id.clone();
                    let conversation_for_read = conversation_id.clone();
                    let principal_for_read = principal_id.clone();
                    let runtime_for_read = runtime_id.clone();
                    let cursor_for_read = message_cursor.clone();
                    let latest = tokio::task::spawn_blocking(move || {
                        read_stream_conversation_page(
                            &snapshot_for_read,
                            &principal_for_read,
                            &runtime_for_read,
                            &workspace_for_read,
                            &conversation_for_read,
                            cursor_for_read.as_deref(),
                        )
                    })
                    .await;
                    let Ok(Ok(latest)) = latest else {
                        return None;
                    };
                    let Ok(next_sequence) = latest.origin_sequence.parse::<u64>() else {
                        return None;
                    };
                    let page_frames = match serialize_conversation_stream_page(
                        &latest,
                        &workspace_id,
                        &runtime_id,
                        &conversation_id,
                    ) {
                        Ok(result) => result,
                        Err(_) => return None,
                    };
                    sequence = next_sequence;
                    message_cursor = page_frames.message_frontier;
                    catch_up = page_frames.has_more;
                    initial = page_frames.frames.into_iter();
                    let frame = initial.next()?;
                    return Some((
                        Ok(Bytes::from(frame)),
                        (
                            permit,
                            shutdown,
                            store,
                            principal_id,
                            runtime_id,
                            workspace_id,
                            conversation_id,
                            sequence,
                            message_cursor,
                            catch_up,
                            initial,
                        ),
                    ));
                }
            }
        },
    );
    let mut response = Response::new(Body::from_stream(output));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.litecowork.operator-stream+json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn subscribe_conversation_batch_stream(
    state: StreamApiState,
    workspace_id: String,
    conversation_ids: Vec<String>,
    resume_cursor: Option<String>,
) -> Result<Response, Response> {
    let stream_permit = Arc::clone(&state.stream_admission)
        .try_acquire_owned()
        .map_err(|_| {
            presentation_error(
                StatusCode::TOO_MANY_REQUESTS,
                "CAPACITY_EXCEEDED",
                "The local stream session limit has been reached",
            )
        })?;
    let snapshots = tokio::task::spawn_blocking({
        let store = state.store.clone();
        let principal = state.principal_id.clone();
        let runtime = state.runtime_id.clone();
        let workspace = workspace_id.clone();
        let ids = conversation_ids.clone();
        move || {
            let cursors = vec![None; ids.len()];
            read_stream_conversation_batch(
                &store, &principal, &runtime, &workspace, &ids, &cursors, 1,
            )
        }
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Conversation stream snapshot is unavailable",
        )
    })??;
    let snapshot_sequence = snapshots
        .first()
        .and_then(|snapshot| snapshot.origin_sequence.parse::<u64>().ok())
        .ok_or_else(|| {
            presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTEGRITY_FAILURE",
                "Conversation stream snapshot is unavailable",
            )
        })?;
    if snapshots
        .iter()
        .any(|snapshot| snapshot.origin_sequence != snapshot_sequence.to_string())
    {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Conversation stream snapshots do not share a source frontier",
        ));
    }
    let filter_digest = conversation_filter_digest(&conversation_ids)?;
    let (resume, should_resync) = validate_batch_resume_cursor(
        resume_cursor.as_deref(),
        &workspace_id,
        &state.principal_id,
        &state.runtime_id,
        &conversation_ids,
        &state.store,
    )
    .await;
    let fresh_frontiers = snapshots
        .iter()
        .map(|snapshot| snapshot.stream_message_frontier.clone())
        .collect::<Vec<_>>();
    let fresh_projection_revisions = snapshots
        .iter()
        .map(|snapshot| snapshot.projection_revision)
        .collect::<Vec<_>>();
    let mut initial_cursor = if let Some(cursor) = resume.as_ref().filter(|_| !should_resync) {
        cursor.clone()
    } else {
        ConversationBatchStreamCursor {
            version: 1,
            workspace_id: workspace_id.clone(),
            origin_runtime_id: state.runtime_id.clone(),
            filter_digest: filter_digest.clone(),
            projection_version: STREAM_PROJECTION_VERSION,
            origin_sequence: 0,
            message_frontiers: vec![None; conversation_ids.len()],
            projection_revisions: vec![0; conversation_ids.len()],
            snapshot_complete: false,
        }
    };
    let mut initial = Vec::with_capacity(conversation_ids.len() + 2);
    initial.push(serialize_stream_frame(&StreamReady {
        frame_type: "stream.ready",
        workspace_id: &workspace_id,
        projection_version: STREAM_PROJECTION_VERSION,
        cursor: encode_batch_stream_cursor(&initial_cursor)?,
        max_frame_bytes: operator_ipc::MAX_STREAM_FRAME_BYTES,
    })?);
    if should_resync {
        initial.push(serialize_stream_frame(&StreamResyncRequired {
            frame_type: "stream.resync_required",
            workspace_id: &workspace_id,
            reason: "resume_cursor_invalid_or_expired",
        })?);
    }
    for (index, snapshot) in snapshots.iter().enumerate() {
        let cursor = if let Some(cursor) = resume.as_ref().filter(|_| !should_resync) {
            encode_batch_stream_cursor(cursor)?
        } else {
            initial_cursor.message_frontiers[index] = snapshot.stream_message_frontier.clone();
            initial_cursor.projection_revisions[index] = snapshot.projection_revision;
            if index + 1 == snapshots.len() {
                initial_cursor.origin_sequence = snapshot_sequence;
                initial_cursor
                    .message_frontiers
                    .clone_from(&fresh_frontiers);
                initial_cursor
                    .projection_revisions
                    .clone_from(&fresh_projection_revisions);
                initial_cursor.snapshot_complete = true;
            }
            encode_batch_stream_cursor(&initial_cursor)?
        };
        initial.push(serialize_stream_frame(&StreamProjection {
            frame_type: "projection",
            workspace_id: &workspace_id,
            projection_type: "conversation_presentation",
            projection_revision: snapshot.projection_revision,
            cursor,
            payload: snapshot,
        })?);
    }
    let (sequence, message_frontiers, projection_revisions, catch_up) =
        if let Some(cursor) = resume.filter(|_| !should_resync) {
            (
                cursor.origin_sequence,
                cursor.message_frontiers,
                cursor.projection_revisions,
                true,
            )
        } else {
            (
                snapshot_sequence,
                fresh_frontiers,
                fresh_projection_revisions,
                false,
            )
        };
    let initial = initial.into_iter();
    let mut response = Response::new(Body::from_stream(stream::unfold(
        (
            stream_permit,
            state.stream_shutdown,
            state.store,
            state.principal_id,
            state.runtime_id,
            workspace_id,
            conversation_ids,
            filter_digest,
            sequence,
            message_frontiers,
            projection_revisions,
            catch_up,
            initial,
        ),
        move |(
            permit,
            mut shutdown,
            store,
            principal,
            runtime,
            workspace,
            ids,
            filter_digest,
            mut sequence,
            mut frontiers,
            mut projection_revisions,
            mut catch_up,
            mut initial,
        )| async move {
            if let Some(frame) = initial.next() {
                return Some((
                    Ok::<Bytes, std::convert::Infallible>(Bytes::from(frame)),
                    (
                        permit,
                        shutdown,
                        store,
                        principal,
                        runtime,
                        workspace,
                        ids,
                        filter_digest,
                        sequence,
                        frontiers,
                        projection_revisions,
                        catch_up,
                        initial,
                    ),
                ));
            }
            if catch_up {
                // Cursor validation happens before the lazy stream body is polled.
                // Recheck retained history at the catch-up boundary so archival
                // between admission and replay cannot silently advance the cursor.
                let store_for_page = store.clone();
                let workspace_for_page = workspace.clone();
                let runtime_for_page = runtime.clone();
                let page = tokio::task::spawn_blocking(move || {
                    use storage_core::EventStore;
                    store_for_page.read_workspace_event_page(
                        &workspace_for_page,
                        &runtime_for_page,
                        sequence,
                        500,
                    )
                })
                .await;
                let page = match page {
                    Ok(Ok(page)) => page,
                    _ => return None,
                };
                if page.cursor_is_ahead || page.has_gap {
                    let store_read = store.clone();
                    let principal_read = principal.clone();
                    let runtime_read = runtime.clone();
                    let workspace_read = workspace.clone();
                    let ids_read = ids.clone();
                    let cursors_read = vec![None; ids.len()];
                    let snapshots = tokio::task::spawn_blocking(move || {
                        read_stream_conversation_batch(
                            &store_read,
                            &principal_read,
                            &runtime_read,
                            &workspace_read,
                            &ids_read,
                            &cursors_read,
                            1,
                        )
                    })
                    .await;
                    let Ok(Ok(snapshots)) = snapshots else {
                        return None;
                    };
                    let serialized = match serialize_conversation_resync_frames(
                        &snapshots,
                        &workspace,
                        &runtime,
                        &filter_digest,
                    ) {
                        Ok(serialized) => serialized,
                        Err(_) => return None,
                    };
                    sequence = serialized.origin_sequence;
                    frontiers = serialized.message_frontiers;
                    projection_revisions = serialized.projection_revisions;
                    catch_up = serialized.has_more;
                    initial = serialized.frames.into_iter();
                    let frame = initial.next()?;
                    return Some((
                        Ok(Bytes::from(frame)),
                        (
                            permit,
                            shutdown,
                            store,
                            principal,
                            runtime,
                            workspace,
                            ids,
                            filter_digest,
                            sequence,
                            frontiers,
                            projection_revisions,
                            catch_up,
                            initial,
                        ),
                    ));
                }
                let store_read = store.clone();
                let principal_read = principal.clone();
                let runtime_read = runtime.clone();
                let workspace_read = workspace.clone();
                let ids_read = ids.clone();
                let cursors_read = frontiers.clone();
                let revisions_read = projection_revisions.clone();
                let snapshots = tokio::task::spawn_blocking(move || {
                    read_stream_conversation_batch(
                        &store_read,
                        &principal_read,
                        &runtime_read,
                        &workspace_read,
                        &ids_read,
                        &cursors_read,
                        1,
                    )
                })
                .await;
                let Ok(Ok(snapshots)) = snapshots else {
                    return None;
                };
                let serialized = match serialize_conversation_batch_pages(
                    &snapshots,
                    &workspace,
                    &runtime,
                    &filter_digest,
                    sequence,
                    &frontiers,
                    &revisions_read,
                ) {
                    Ok(serialized) => serialized,
                    Err(_) => return None,
                };
                sequence = serialized.origin_sequence;
                frontiers = serialized.message_frontiers;
                projection_revisions = serialized.projection_revisions;
                catch_up = serialized.has_more;
                initial = serialized.frames.into_iter();
                let frame = initial.next()?;
                return Some((
                    Ok(Bytes::from(frame)),
                    (
                        permit,
                        shutdown,
                        store,
                        principal,
                        runtime,
                        workspace,
                        ids,
                        filter_digest,
                        sequence,
                        frontiers,
                        projection_revisions,
                        catch_up,
                        initial,
                    ),
                ));
            }
            loop {
                tokio::select! {
                    _ = shutdown.changed() => return None,
                    _ = tokio::time::sleep(STREAM_POLL_INTERVAL) => {}
                }
                let store_for_auth = store.clone();
                let principal_for_auth = principal.clone();
                let workspace_for_auth = workspace.clone();
                let authorized = tokio::task::spawn_blocking(move || {
                    ensure_workspace_owner_for(
                        &store_for_auth,
                        &principal_for_auth,
                        &workspace_for_auth,
                    )
                })
                .await;
                if !matches!(authorized, Ok(Ok(_))) {
                    return None;
                }
                let store_for_page = store.clone();
                let workspace_for_page = workspace.clone();
                let runtime_for_page = runtime.clone();
                let page = tokio::task::spawn_blocking(move || {
                    use storage_core::EventStore;
                    store_for_page.read_workspace_event_page(
                        &workspace_for_page,
                        &runtime_for_page,
                        sequence,
                        500,
                    )
                })
                .await;
                let page = match page {
                    Ok(Ok(page)) => page,
                    _ => return None,
                };
                if page.cursor_is_ahead || page.has_gap {
                    let store_read = store.clone();
                    let principal_read = principal.clone();
                    let runtime_read = runtime.clone();
                    let workspace_read = workspace.clone();
                    let ids_read = ids.clone();
                    let cursors_read = vec![None; ids.len()];
                    let snapshots = tokio::task::spawn_blocking(move || {
                        read_stream_conversation_batch(
                            &store_read,
                            &principal_read,
                            &runtime_read,
                            &workspace_read,
                            &ids_read,
                            &cursors_read,
                            1,
                        )
                    })
                    .await;
                    let Ok(Ok(snapshots)) = snapshots else {
                        return None;
                    };
                    let serialized = match serialize_conversation_resync_frames(
                        &snapshots,
                        &workspace,
                        &runtime,
                        &filter_digest,
                    ) {
                        Ok(serialized) => serialized,
                        Err(_) => return None,
                    };
                    sequence = serialized.origin_sequence;
                    frontiers = serialized.message_frontiers;
                    projection_revisions = serialized.projection_revisions;
                    catch_up = serialized.has_more;
                    initial = serialized.frames.into_iter();
                    let frame = initial.next()?;
                    return Some((
                        Ok(Bytes::from(frame)),
                        (
                            permit,
                            shutdown,
                            store,
                            principal,
                            runtime,
                            workspace,
                            ids,
                            filter_digest,
                            sequence,
                            frontiers,
                            projection_revisions,
                            catch_up,
                            initial,
                        ),
                    ));
                }
                if page.head_sequence > sequence {
                    let store_read = store.clone();
                    let principal_read = principal.clone();
                    let runtime_read = runtime.clone();
                    let workspace_read = workspace.clone();
                    let ids_read = ids.clone();
                    let cursors_read = frontiers.clone();
                    let revisions_read = projection_revisions.clone();
                    let snapshots = tokio::task::spawn_blocking(move || {
                        read_stream_conversation_batch(
                            &store_read,
                            &principal_read,
                            &runtime_read,
                            &workspace_read,
                            &ids_read,
                            &cursors_read,
                            1,
                        )
                    })
                    .await;
                    let Ok(Ok(snapshots)) = snapshots else {
                        return None;
                    };
                    let serialized = match serialize_conversation_batch_pages(
                        &snapshots,
                        &workspace,
                        &runtime,
                        &filter_digest,
                        sequence,
                        &frontiers,
                        &revisions_read,
                    ) {
                        Ok(serialized) => serialized,
                        Err(_) => return None,
                    };
                    sequence = serialized.origin_sequence;
                    frontiers = serialized.message_frontiers;
                    projection_revisions = serialized.projection_revisions;
                    catch_up = serialized.has_more;
                    initial = serialized.frames.into_iter();
                    let frame = initial.next()?;
                    return Some((
                        Ok(Bytes::from(frame)),
                        (
                            permit,
                            shutdown,
                            store,
                            principal,
                            runtime,
                            workspace,
                            ids,
                            filter_digest,
                            sequence,
                            frontiers,
                            projection_revisions,
                            catch_up,
                            initial,
                        ),
                    ));
                }
            }
        },
    )));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.litecowork.operator-stream+json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

fn read_stream_conversation_batch(
    store: &SqliteWorkspaceStore,
    principal_id: &str,
    runtime_id: &str,
    workspace_id: &str,
    conversation_ids: &[String],
    after_message_ids: &[Option<String>],
    limit: usize,
) -> Result<Vec<storage_core::rich_presentation::ConversationPresentationSnapshot>, Response> {
    if conversation_ids.is_empty()
        || conversation_ids.len() != after_message_ids.len()
        || conversation_ids.len() > MAX_CONVERSATION_PRESENTATION_BATCH
    {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation stream filters are invalid",
        ));
    }
    let reads = conversation_ids
        .iter()
        .zip(after_message_ids)
        .map(
            |(conversation_id, after_message_id)| ConversationPresentationRead {
                conversation_id: conversation_id.clone(),
                after_message_id: after_message_id.clone(),
            },
        )
        .collect::<Vec<_>>();
    SqliteRichPresentationStore::new(store.clone())
        .read_conversation_presentation_batch(principal_id, workspace_id, &reads, runtime_id, limit)
        .map_err(|error| match error {
            StoreError::NotFound => presentation_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Conversation is unavailable",
            ),
            StoreError::Invalid(_) => presentation_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Conversation stream subscription is invalid",
            ),
            _ => presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTEGRITY_FAILURE",
                "Conversation stream snapshot is unavailable",
            ),
        })
}

fn conversation_filter_digest(conversation_ids: &[String]) -> Result<String, Response> {
    let canonical = serde_json::to_vec(conversation_ids).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Conversation stream cursor is unavailable",
        )
    })?;
    Ok(hex::encode(Sha256::digest(canonical)))
}

fn encode_batch_stream_cursor(cursor: &ConversationBatchStreamCursor) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Conversation stream cursor is unavailable",
        )
    })?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    if encoded.len() > MAX_BATCH_STREAM_CURSOR_BYTES {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Conversation stream cursor exceeds its limit",
        ));
    }
    Ok(encoded)
}

fn decode_batch_stream_cursor(encoded: &str) -> Result<ConversationBatchStreamCursor, ()> {
    if encoded.is_empty() || encoded.len() > MAX_BATCH_STREAM_CURSOR_BYTES {
        return Err(());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

async fn validate_batch_resume_cursor(
    encoded: Option<&str>,
    workspace_id: &str,
    principal_id: &str,
    runtime_id: &str,
    conversation_ids: &[String],
    store: &SqliteWorkspaceStore,
) -> (Option<ConversationBatchStreamCursor>, bool) {
    let Some(encoded) = encoded else {
        return (None, false);
    };
    let Ok(cursor) = decode_batch_stream_cursor(encoded) else {
        return (None, true);
    };
    let Ok(filter_digest) = conversation_filter_digest(conversation_ids) else {
        return (None, true);
    };
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.origin_runtime_id != runtime_id
        || cursor.filter_digest != filter_digest
        || cursor.projection_version != STREAM_PROJECTION_VERSION
        || !cursor.snapshot_complete
        || cursor.message_frontiers.len() != conversation_ids.len()
        || cursor.projection_revisions.len() != conversation_ids.len()
        || cursor.projection_revisions.contains(&0)
        || cursor
            .message_frontiers
            .iter()
            .flatten()
            .any(|id| !valid_task_query_id(id))
    {
        return (None, true);
    }
    let validation_store = store.clone();
    let principal = principal_id.to_owned();
    let workspace = workspace_id.to_owned();
    let runtime = runtime_id.to_owned();
    let ids = conversation_ids.to_vec();
    let frontiers = cursor.message_frontiers.clone();
    let snapshot = tokio::task::spawn_blocking(move || {
        read_stream_conversation_batch(
            &validation_store,
            &principal,
            &runtime,
            &workspace,
            &ids,
            &frontiers,
            1,
        )
    })
    .await;
    if !matches!(snapshot, Ok(Ok(_))) {
        return (None, true);
    }
    let store = store.clone();
    let workspace = workspace_id.to_owned();
    let runtime = runtime_id.to_owned();
    let sequence = cursor.origin_sequence;
    match tokio::task::spawn_blocking(move || {
        use storage_core::EventStore;
        store.read_workspace_event_page(&workspace, &runtime, sequence, 1)
    })
    .await
    {
        Ok(Ok(page)) if !page.cursor_is_ahead && !page.has_gap => (Some(cursor), false),
        _ => (Some(cursor), true),
    }
}

struct SerializedConversationResync {
    frames: Vec<Vec<u8>>,
    message_frontiers: Vec<Option<String>>,
    projection_revisions: Vec<u64>,
    origin_sequence: u64,
    has_more: bool,
}

fn serialize_conversation_resync_frames(
    snapshots: &[storage_core::rich_presentation::ConversationPresentationSnapshot],
    workspace_id: &str,
    runtime_id: &str,
    filter_digest: &str,
) -> Result<SerializedConversationResync, Response> {
    let sequence = snapshots
        .first()
        .and_then(|snapshot| snapshot.origin_sequence.parse::<u64>().ok())
        .ok_or_else(|| {
            presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTEGRITY_FAILURE",
                "Conversation resync snapshot is unavailable",
            )
        })?;
    if snapshots
        .iter()
        .any(|snapshot| snapshot.origin_sequence != sequence.to_string())
    {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Conversation resync snapshots do not share a source frontier",
        ));
    }
    let has_more = snapshots
        .iter()
        .any(|snapshot| snapshot.next_cursor.is_some());
    let projection_revisions = snapshots
        .iter()
        .map(|snapshot| snapshot.projection_revision)
        .collect::<Vec<_>>();
    let mut message_frontiers = vec![None; snapshots.len()];
    let mut frames = vec![serialize_stream_frame(&StreamResyncRequired {
        frame_type: "stream.resync_required",
        workspace_id,
        reason: "replay_history_gap",
    })?];
    let mut replacement = ConversationBatchStreamCursor {
        version: 1,
        workspace_id: workspace_id.to_owned(),
        origin_runtime_id: runtime_id.to_owned(),
        filter_digest: filter_digest.to_owned(),
        projection_version: STREAM_PROJECTION_VERSION,
        origin_sequence: 0,
        message_frontiers: vec![None; snapshots.len()],
        projection_revisions: vec![0; snapshots.len()],
        snapshot_complete: false,
    };
    for (index, snapshot) in snapshots.iter().enumerate() {
        let message_frontier = if snapshot.next_cursor.is_some() {
            Some(
                snapshot
                    .items
                    .last()
                    .and_then(|item| item.get("message"))
                    .and_then(|message| message.get("message_id"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|id| valid_task_query_id(id))
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        presentation_error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "INTEGRITY_FAILURE",
                            "Conversation resync snapshot is unavailable",
                        )
                    })?,
            )
        } else {
            snapshot.stream_message_frontier.clone()
        };
        message_frontiers[index] = message_frontier;
        replacement.message_frontiers[index] = message_frontiers[index].clone();
        replacement.projection_revisions[index] = snapshot.projection_revision;
        if index + 1 == snapshots.len() {
            replacement.origin_sequence = sequence;
            replacement
                .projection_revisions
                .clone_from(&projection_revisions);
            replacement.snapshot_complete = true;
        }
        frames.push(serialize_stream_frame(&StreamProjection {
            frame_type: "projection",
            workspace_id,
            projection_type: "conversation_presentation",
            projection_revision: snapshot.projection_revision,
            cursor: encode_batch_stream_cursor(&replacement)?,
            payload: snapshot,
        })?);
    }
    Ok(SerializedConversationResync {
        frames,
        message_frontiers,
        projection_revisions,
        origin_sequence: sequence,
        has_more,
    })
}

struct SerializedConversationBatchPage {
    frames: Vec<Vec<u8>>,
    message_frontiers: Vec<Option<String>>,
    projection_revisions: Vec<u64>,
    origin_sequence: u64,
    has_more: bool,
}

fn serialize_conversation_batch_pages(
    snapshots: &[storage_core::rich_presentation::ConversationPresentationSnapshot],
    workspace_id: &str,
    runtime_id: &str,
    filter_digest: &str,
    previous_sequence: u64,
    previous_frontiers: &[Option<String>],
    previous_projection_revisions: &[u64],
) -> Result<SerializedConversationBatchPage, Response> {
    if snapshots.len() != previous_frontiers.len()
        || snapshots.len() != previous_projection_revisions.len()
    {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Conversation stream batch is unavailable",
        ));
    }
    let sequence = snapshots
        .first()
        .and_then(|snapshot| snapshot.origin_sequence.parse::<u64>().ok())
        .ok_or_else(|| {
            presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTEGRITY_FAILURE",
                "Conversation stream batch is unavailable",
            )
        })?;
    if snapshots
        .iter()
        .any(|snapshot| snapshot.origin_sequence != sequence.to_string())
    {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Conversation stream batch has inconsistent frontiers",
        ));
    }
    let has_more = snapshots
        .iter()
        .any(|snapshot| snapshot.next_cursor.is_some());
    let mut message_frontiers = previous_frontiers.to_vec();
    let mut projection_revisions = previous_projection_revisions.to_vec();
    let mut frames = Vec::new();
    for (index, snapshot) in snapshots.iter().enumerate() {
        let conversation_id = &snapshot.conversation_id;
        for item in &snapshot.items {
            let message_id = item
                .get("message")
                .and_then(|message| message.get("message_id"))
                .and_then(serde_json::Value::as_str)
                .filter(|id| valid_task_query_id(id))
                .ok_or_else(|| {
                    presentation_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "INTEGRITY_FAILURE",
                        "Conversation stream batch is unavailable",
                    )
                })?;
            let mut cursor = ConversationBatchStreamCursor {
                version: 1,
                workspace_id: workspace_id.to_owned(),
                origin_runtime_id: runtime_id.to_owned(),
                filter_digest: filter_digest.to_owned(),
                projection_version: STREAM_PROJECTION_VERSION,
                origin_sequence: previous_sequence,
                message_frontiers: message_frontiers.clone(),
                projection_revisions: projection_revisions.clone(),
                // This serializer runs only after a complete baseline was delivered.
                // Per-message cursors are resumable; stale projection revisions cause
                // an authorized snapshot refresh if disconnect interrupts catch-up.
                snapshot_complete: true,
            };
            cursor.message_frontiers[index] = Some(message_id.to_owned());
            cursor.origin_sequence = sequence;
            let encoded_cursor = encode_batch_stream_cursor(&cursor)?;
            frames.push(serialize_stream_frame(&ConversationProjectionUpsert {
                frame_type: "projection",
                workspace_id,
                projection_type: "conversation_presentation",
                projection_revision: snapshot.projection_revision,
                cursor: encoded_cursor,
                payload: ConversationProjectionUpsertPayload {
                    conversation_id,
                    stream_cursor: &snapshot.stream_cursor,
                    origin_sequence: &snapshot.origin_sequence,
                    item,
                },
            })?);
            message_frontiers[index] = Some(message_id.to_owned());
        }
        let projection_changed = snapshot.projection_revision != projection_revisions[index];
        if snapshot.next_cursor.is_none() && (projection_changed || index + 1 == snapshots.len()) {
            message_frontiers[index] = snapshot.stream_message_frontier.clone();
            projection_revisions[index] = snapshot.projection_revision;
            let cursor = ConversationBatchStreamCursor {
                version: 1,
                workspace_id: workspace_id.to_owned(),
                origin_runtime_id: runtime_id.to_owned(),
                filter_digest: filter_digest.to_owned(),
                projection_version: STREAM_PROJECTION_VERSION,
                origin_sequence: sequence,
                message_frontiers: message_frontiers.clone(),
                projection_revisions: projection_revisions.clone(),
                // Every cursor in a live catch-up stream can resume because the initial
                // all-filter baseline is complete and the cursor retains each filter's
                // own message frontier and projection revision.
                snapshot_complete: true,
            };
            frames.push(serialize_stream_frame(&StreamProjection {
                frame_type: "projection",
                workspace_id,
                projection_type: "conversation_presentation",
                projection_revision: snapshot.projection_revision,
                cursor: encode_batch_stream_cursor(&cursor)?,
                payload: snapshot,
            })?);
        }
    }
    Ok(SerializedConversationBatchPage {
        frames,
        message_frontiers,
        projection_revisions,
        origin_sequence: sequence,
        has_more,
    })
}

fn read_stream_conversation_snapshot(
    store: &SqliteWorkspaceStore,
    principal_id: &str,
    runtime_id: &str,
    workspace_id: &str,
    conversation_id: &str,
) -> Result<storage_core::rich_presentation::ConversationPresentationSnapshot, Response> {
    read_stream_conversation_page(
        store,
        principal_id,
        runtime_id,
        workspace_id,
        conversation_id,
        None,
    )
}

fn read_stream_conversation_page(
    store: &SqliteWorkspaceStore,
    principal_id: &str,
    runtime_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    after_message_id: Option<&str>,
) -> Result<storage_core::rich_presentation::ConversationPresentationSnapshot, Response> {
    SqliteRichPresentationStore::new(store.clone())
        .read_conversation_presentation(
            principal_id,
            workspace_id,
            conversation_id,
            runtime_id,
            after_message_id,
            MAX_CONVERSATION_PRESENTATION_MESSAGES,
        )
        .map_err(|error| match error {
            StoreError::NotFound => presentation_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Conversation is unavailable",
            ),
            StoreError::Invalid(_) => presentation_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Conversation stream subscription is invalid",
            ),
            _ => presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTEGRITY_FAILURE",
                "Conversation stream snapshot is unavailable",
            ),
        })
}

struct SerializedConversationPage {
    frames: Vec<Vec<u8>>,
    message_frontier: Option<String>,
    has_more: bool,
}

fn serialize_conversation_stream_page(
    snapshot: &storage_core::rich_presentation::ConversationPresentationSnapshot,
    workspace_id: &str,
    runtime_id: &str,
    conversation_id: &str,
) -> Result<SerializedConversationPage, Response> {
    let sequence = snapshot.origin_sequence.parse::<u64>().map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Conversation stream snapshot is unavailable",
        )
    })?;
    let mut frames =
        Vec::with_capacity(snapshot.items.len() + usize::from(snapshot.next_cursor.is_none()));
    let mut message_frontier = None;
    for item in &snapshot.items {
        let message_id = item
            .get("message")
            .and_then(|message| message.get("message_id"))
            .and_then(serde_json::Value::as_str)
            .filter(|id| valid_task_query_id(id))
            .ok_or_else(|| {
                presentation_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTEGRITY_FAILURE",
                    "Conversation stream snapshot is unavailable",
                )
            })?;
        let cursor = encode_stream_cursor(&StreamCursor {
            version: 1,
            workspace_id: workspace_id.to_owned(),
            origin_runtime_id: runtime_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            projection_version: STREAM_PROJECTION_VERSION,
            origin_sequence: sequence,
            message_frontier: Some(message_id.to_owned()),
        })?;
        frames.push(serialize_stream_frame(&ConversationProjectionUpsert {
            frame_type: "projection",
            workspace_id,
            projection_type: "conversation_presentation",
            projection_revision: snapshot.projection_revision,
            cursor,
            payload: ConversationProjectionUpsertPayload {
                conversation_id: &snapshot.conversation_id,
                stream_cursor: &snapshot.stream_cursor,
                origin_sequence: &snapshot.origin_sequence,
                item,
            },
        })?);
        message_frontier = Some(message_id.to_owned());
    }
    if snapshot.next_cursor.is_none() {
        let cursor = encode_stream_cursor(&StreamCursor {
            version: 1,
            workspace_id: workspace_id.to_owned(),
            origin_runtime_id: runtime_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            projection_version: STREAM_PROJECTION_VERSION,
            origin_sequence: sequence,
            message_frontier: snapshot.stream_message_frontier.clone(),
        })?;
        frames.push(serialize_stream_frame(&StreamProjection {
            frame_type: "projection",
            workspace_id,
            projection_type: "conversation_presentation",
            projection_revision: snapshot.projection_revision,
            cursor,
            payload: snapshot,
        })?);
        message_frontier = snapshot.stream_message_frontier.clone();
    }
    Ok(SerializedConversationPage {
        frames,
        message_frontier,
        has_more: snapshot.next_cursor.is_some(),
    })
}

async fn validate_resume_cursor(
    encoded: Option<&str>,
    workspace_id: &str,
    runtime_id: &str,
    conversation_id: &str,
    store: &SqliteWorkspaceStore,
) -> (Option<StreamCursor>, bool) {
    let Some(encoded) = encoded else {
        return (None, false);
    };
    let Ok(cursor) = decode_stream_cursor(encoded) else {
        return (None, true);
    };
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.origin_runtime_id != runtime_id
        || cursor.conversation_id != conversation_id
        || cursor.projection_version != STREAM_PROJECTION_VERSION
        || cursor
            .message_frontier
            .as_deref()
            .is_some_and(|id| !valid_task_query_id(id))
    {
        return (None, true);
    }
    let store = store.clone();
    let workspace_id = workspace_id.to_owned();
    let runtime_id = runtime_id.to_owned();
    let sequence = cursor.origin_sequence;
    match tokio::task::spawn_blocking(move || {
        use storage_core::EventStore;
        store.read_workspace_event_page(&workspace_id, &runtime_id, sequence, 1)
    })
    .await
    {
        Ok(Ok(page)) if !page.cursor_is_ahead && !page.has_gap => (Some(cursor), false),
        _ => (Some(cursor), true),
    }
}

fn encode_task_stream_cursor(cursor: &TaskStreamCursor) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Task stream cursor is unavailable",
        )
    })?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    if encoded.len() > 2048 {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Task stream cursor is unavailable",
        ));
    }
    Ok(encoded)
}

fn decode_task_stream_cursor(encoded: &str) -> Result<TaskStreamCursor, ()> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

async fn task_resume_requires_resync(
    encoded: Option<&str>,
    workspace_id: &str,
    runtime_id: &str,
    task_id: &str,
    store: &SqliteWorkspaceStore,
) -> bool {
    let Some(encoded) = encoded else {
        return false;
    };
    let Ok(cursor) = decode_task_stream_cursor(encoded) else {
        return true;
    };
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.origin_runtime_id != runtime_id
        || cursor.task_id != task_id
        || cursor.projection_version != STREAM_PROJECTION_VERSION
    {
        return true;
    }
    let store = store.clone();
    let workspace_id = workspace_id.to_owned();
    let runtime_id = runtime_id.to_owned();
    let sequence = cursor.origin_sequence;
    !matches!(
        tokio::task::spawn_blocking(move || {
            use storage_core::EventStore;
            store.read_workspace_event_page(&workspace_id, &runtime_id, sequence, 1)
        })
        .await,
        Ok(Ok(page)) if !page.cursor_is_ahead && !page.has_gap
    )
}

fn read_task_stream_snapshot(
    store: &SqliteWorkspaceStore,
    principal_id: &str,
    runtime_id: &str,
    workspace_id: &str,
    task_id: &str,
) -> Result<(TaskPresentationSnapshot, u64), Response> {
    ensure_workspace_owner_for(store, principal_id, workspace_id)?;
    let snapshot = store
        .get_task_presentation(workspace_id, task_id, runtime_id)
        .map_err(|_| {
            presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Task presentation is unavailable",
            )
        })?
        .ok_or_else(|| {
            presentation_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Task presentation is unavailable",
            )
        })?;
    task_presentation_view(snapshot, workspace_id.to_owned())
}

fn task_projection_revision(origin_sequence: u64) -> Result<u64, Response> {
    let revision = origin_sequence.max(1);
    if revision > 9_007_199_254_740_991 {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Task presentation revision is unavailable",
        ));
    }
    Ok(revision)
}

async fn subscribe_runtime_inventory_stream(
    state: StreamApiState,
    workspace_id: String,
    runtime_id: String,
    resume_cursor: Option<String>,
) -> Result<Response, Response> {
    if runtime_id != state.runtime_id {
        return Err(presentation_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Runtime inventory is unavailable",
        ));
    }
    let stream_permit = Arc::clone(&state.stream_admission)
        .try_acquire_owned()
        .map_err(|_| {
            presentation_error(
                StatusCode::TOO_MANY_REQUESTS,
                "CAPACITY_EXCEEDED",
                "The local stream session limit has been reached",
            )
        })?;

    let now = operator_now()?;
    let authorization_store = state.store.clone();
    let owner = state.principal_id.clone();
    let authorized_workspace = workspace_id.clone();
    let authorized_runtime = runtime_id.clone();
    let expected_incarnation = state.local_incarnation_id.clone();
    tokio::task::spawn_blocking(move || {
        use storage_core::AgentCatalogStore;
        authorization_store.list_runtime_offers(storage_core::RuntimeOfferPageRequest {
            owner_principal_id: owner,
            workspace_id: authorized_workspace,
            runtime_id: authorized_runtime,
            expected_incarnation_id: Some(expected_incarnation),
            now,
            after: None,
            limit: 1,
        })
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Runtime inventory authorization is unavailable",
        )
    })?
    .map_err(|error| map_agent_store_error(error, "Runtime inventory is unavailable"))?;

    let current_revision = state.runtime_inventory_revision.load(Ordering::Acquire);
    let cursor = resume_cursor
        .as_deref()
        .and_then(|value| decode_runtime_inventory_cursor(value).ok());
    let should_resync = resume_cursor.is_some()
        && !cursor.as_ref().is_some_and(|cursor| {
            cursor.version == 1
                && cursor.workspace_id == workspace_id
                && cursor.runtime_id == runtime_id
                && cursor.runtime_incarnation_id == state.local_incarnation_id
                && cursor.projection_version == STREAM_PROJECTION_VERSION
                && cursor.revision <= current_revision
        });
    let ready_cursor = encode_runtime_inventory_cursor(&RuntimeInventoryStreamCursor {
        version: 1,
        workspace_id: workspace_id.clone(),
        runtime_id: runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
        projection_version: STREAM_PROJECTION_VERSION,
        revision: current_revision,
    })?;
    let mut initial = vec![serialize_stream_frame(&StreamReady {
        frame_type: "stream.ready",
        workspace_id: &workspace_id,
        projection_version: STREAM_PROJECTION_VERSION,
        cursor: ready_cursor,
        max_frame_bytes: operator_ipc::MAX_STREAM_FRAME_BYTES,
    })?];
    if should_resync {
        initial.push(serialize_stream_frame(&StreamResyncRequired {
            frame_type: "stream.resync_required",
            workspace_id: &workspace_id,
            reason: "resume_cursor_invalid_or_expired",
        })?);
    }
    // Always send one initial invalidation. The consumer obtains current data only
    // through the separately authorized offers route.
    initial.push(serialize_runtime_inventory_notification(
        &workspace_id,
        &runtime_id,
        &state.local_incarnation_id,
        current_revision,
    )?);

    let shutdown = state.stream_shutdown.clone();
    let store = state.store;
    let owner = state.principal_id;
    let incarnation_for_poll = state.local_incarnation_id.clone();
    let revision = Arc::clone(&state.runtime_inventory_revision);
    let initial = initial.into_iter();
    let output = stream::unfold(
        (
            stream_permit,
            shutdown,
            store,
            owner,
            runtime_id,
            incarnation_for_poll,
            workspace_id,
            revision,
            current_revision,
            initial,
        ),
        |(
            permit,
            mut shutdown,
            store,
            owner,
            runtime_id,
            incarnation_id,
            workspace_id,
            revision,
            mut seen_revision,
            mut initial,
        )| async move {
            if let Some(frame) = initial.next() {
                return Some((
                    Ok::<Bytes, std::convert::Infallible>(Bytes::from(frame)),
                    (
                        permit,
                        shutdown,
                        store,
                        owner,
                        runtime_id,
                        incarnation_id,
                        workspace_id,
                        revision,
                        seen_revision,
                        initial,
                    ),
                ));
            }
            loop {
                tokio::select! {
                    _ = shutdown.changed() => return None,
                    _ = tokio::time::sleep(STREAM_POLL_INTERVAL) => {}
                }
                let page_store = store.clone();
                let page_owner = owner.clone();
                let page_workspace = workspace_id.clone();
                let page_runtime = runtime_id.clone();
                let page_incarnation = incarnation_id.clone();
                let now = match operator_now_value() {
                    Ok(now) => now,
                    Err(_) => return None,
                };
                let authorized = tokio::task::spawn_blocking(move || {
                    use storage_core::AgentCatalogStore;
                    page_store.list_runtime_offers(storage_core::RuntimeOfferPageRequest {
                        owner_principal_id: page_owner,
                        workspace_id: page_workspace,
                        runtime_id: page_runtime,
                        expected_incarnation_id: Some(page_incarnation),
                        now,
                        after: None,
                        limit: 1,
                    })
                })
                .await;
                if !matches!(authorized, Ok(Ok(_))) {
                    return None;
                }
                let latest_revision = revision.load(Ordering::Acquire);
                if latest_revision <= seen_revision {
                    continue;
                }
                let frame = match serialize_runtime_inventory_notification(
                    &workspace_id,
                    &runtime_id,
                    &incarnation_id,
                    latest_revision,
                ) {
                    Ok(frame) => frame,
                    Err(_) => return None,
                };
                seen_revision = latest_revision;
                return Some((
                    Ok(Bytes::from(frame)),
                    (
                        permit,
                        shutdown,
                        store,
                        owner,
                        runtime_id,
                        incarnation_id,
                        workspace_id,
                        revision,
                        seen_revision,
                        initial,
                    ),
                ));
            }
        },
    );
    let mut response = Response::new(Body::from_stream(output));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.litecowork.operator-stream+json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

fn serialize_runtime_inventory_notification(
    workspace_id: &str,
    runtime_id: &str,
    incarnation_id: &str,
    revision: u64,
) -> Result<Vec<u8>, Response> {
    let cursor = encode_runtime_inventory_cursor(&RuntimeInventoryStreamCursor {
        version: 1,
        workspace_id: workspace_id.to_owned(),
        runtime_id: runtime_id.to_owned(),
        runtime_incarnation_id: incarnation_id.to_owned(),
        projection_version: STREAM_PROJECTION_VERSION,
        revision,
    })?;
    let payload = RuntimeInventoryNotification {
        runtime_id,
        change_kind: "OFFERS_CHANGED",
    };
    serialize_stream_frame(&StreamProjection {
        frame_type: "projection",
        workspace_id,
        projection_type: "runtime_inventory",
        projection_revision: revision,
        cursor,
        payload: &payload,
    })
}

fn encode_runtime_inventory_cursor(
    cursor: &RuntimeInventoryStreamCursor,
) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Runtime inventory cursor is unavailable",
        )
    })?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    if encoded.len() > 2048 {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Runtime inventory cursor is unavailable",
        ));
    }
    Ok(encoded)
}

fn decode_runtime_inventory_cursor(encoded: &str) -> Result<RuntimeInventoryStreamCursor, ()> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

fn serialize_task_projection(
    snapshot: TaskPresentationSnapshot,
    workspace_id: &str,
    runtime_id: &str,
    task_id: &str,
    origin_sequence: u64,
    reason: Option<&'static str>,
) -> Result<Vec<Vec<u8>>, Response> {
    let cursor = encode_task_stream_cursor(&TaskStreamCursor {
        version: 1,
        workspace_id: workspace_id.to_owned(),
        origin_runtime_id: runtime_id.to_owned(),
        task_id: task_id.to_owned(),
        projection_version: STREAM_PROJECTION_VERSION,
        origin_sequence,
    })?;
    let mut frames = Vec::with_capacity(if reason.is_some() { 2 } else { 1 });
    if let Some(reason) = reason {
        frames.push(serialize_stream_frame(&StreamResyncRequired {
            frame_type: "stream.resync_required",
            workspace_id,
            reason,
        })?);
    }
    let payload = TaskStreamPayload {
        snapshot,
        origin_sequence: origin_sequence.to_string(),
    };
    frames.push(serialize_stream_frame(&StreamProjection {
        frame_type: "projection",
        workspace_id,
        projection_type: "task_presentation",
        projection_revision: task_projection_revision(origin_sequence)?,
        cursor,
        payload: &payload,
    })?);
    Ok(frames)
}

async fn subscribe_task_stream(
    state: StreamApiState,
    workspace_id: String,
    task_id: String,
    resume_cursor: Option<String>,
) -> Result<Response, Response> {
    #[cfg(test)]
    let task_stream_test_gate = Arc::new(tokio::sync::Mutex::new(state.task_stream_test_gate));
    #[cfg(test)]
    let task_stream_snapshot_test_gate = Arc::new(tokio::sync::Mutex::new(
        state.task_stream_snapshot_test_gate,
    ));
    let stream_permit = Arc::clone(&state.stream_admission)
        .try_acquire_owned()
        .map_err(|_| {
            presentation_error(
                StatusCode::TOO_MANY_REQUESTS,
                "CAPACITY_EXCEEDED",
                "The local stream session limit has been reached",
            )
        })?;
    let snapshot_store = state.store.clone();
    let snapshot_principal = state.principal_id.clone();
    let snapshot_runtime = state.runtime_id.clone();
    let snapshot_workspace = workspace_id.clone();
    let snapshot_task = task_id.clone();
    let (snapshot, origin_sequence) = tokio::task::spawn_blocking(move || {
        read_task_stream_snapshot(
            &snapshot_store,
            &snapshot_principal,
            &snapshot_runtime,
            &snapshot_workspace,
            &snapshot_task,
        )
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Task stream snapshot is unavailable",
        )
    })??;
    let should_resync = task_resume_requires_resync(
        resume_cursor.as_deref(),
        &workspace_id,
        &state.runtime_id,
        &task_id,
        &state.store,
    )
    .await;
    let initial_cursor = encode_task_stream_cursor(&TaskStreamCursor {
        version: 1,
        workspace_id: workspace_id.clone(),
        origin_runtime_id: state.runtime_id.clone(),
        task_id: task_id.clone(),
        projection_version: STREAM_PROJECTION_VERSION,
        origin_sequence,
    })?;
    let mut initial = vec![serialize_stream_frame(&StreamReady {
        frame_type: "stream.ready",
        workspace_id: &workspace_id,
        projection_version: STREAM_PROJECTION_VERSION,
        cursor: initial_cursor,
        max_frame_bytes: operator_ipc::MAX_STREAM_FRAME_BYTES,
    })?];
    initial.extend(serialize_task_projection(
        snapshot,
        &workspace_id,
        &state.runtime_id,
        &task_id,
        origin_sequence,
        should_resync.then_some("resume_cursor_invalid_or_expired"),
    )?);

    let shutdown = state.stream_shutdown;
    let store = state.store;
    let principal_id = state.principal_id;
    let runtime_id = state.runtime_id;
    let initial = initial.into_iter();
    let output = stream::unfold(
        (
            stream_permit,
            shutdown,
            store,
            principal_id,
            runtime_id,
            workspace_id,
            task_id,
            origin_sequence,
            initial,
        ),
        move |(
            permit,
            mut shutdown,
            store,
            principal_id,
            runtime_id,
            workspace_id,
            task_id,
            mut sequence,
            mut initial,
        )| {
            #[cfg(test)]
            let task_stream_test_gate = Arc::clone(&task_stream_test_gate);
            #[cfg(test)]
            let task_stream_snapshot_test_gate = Arc::clone(&task_stream_snapshot_test_gate);
            async move {
                if let Some(frame) = initial.next() {
                    return Some((
                        Ok::<Bytes, std::convert::Infallible>(Bytes::from(frame)),
                        (
                            permit,
                            shutdown,
                            store,
                            principal_id,
                            runtime_id,
                            workspace_id,
                            task_id,
                            sequence,
                            initial,
                        ),
                    ));
                }
                loop {
                    tokio::select! {
                        _ = shutdown.changed() => return None,
                        _ = tokio::time::sleep(STREAM_POLL_INTERVAL) => {}
                    }
                    let page_store = store.clone();
                    let page_workspace = workspace_id.clone();
                    let page_runtime = runtime_id.clone();
                    let page = tokio::task::spawn_blocking(move || {
                        use storage_core::EventStore;
                        page_store.read_workspace_event_page(
                            &page_workspace,
                            &page_runtime,
                            sequence,
                            500,
                        )
                    })
                    .await;
                    let Ok(Ok(page)) = page else {
                        return None;
                    };
                    if page.head_sequence <= sequence && !page.cursor_is_ahead && !page.has_gap {
                        continue;
                    }

                    #[cfg(test)]
                    {
                        let gate = task_stream_test_gate.lock().await.take();
                        if let Some(gate) = gate {
                            gate.pause().await;
                        }
                    }

                    let snapshot_store = store.clone();
                    let snapshot_principal = principal_id.clone();
                    let snapshot_runtime = runtime_id.clone();
                    let snapshot_workspace = workspace_id.clone();
                    let snapshot_task = task_id.clone();
                    let refreshed = tokio::task::spawn_blocking(move || {
                        read_task_stream_snapshot(
                            &snapshot_store,
                            &snapshot_principal,
                            &snapshot_runtime,
                            &snapshot_workspace,
                            &snapshot_task,
                        )
                    })
                    .await;
                    let Ok(Ok((snapshot, next_sequence))) = refreshed else {
                        return None;
                    };
                    #[cfg(test)]
                    {
                        let gate = task_stream_snapshot_test_gate.lock().await.take();
                        if let Some(gate) = gate {
                            gate.pause().await;
                        }
                    }
                    // Retention may change after the first replay read and before the
                    // authorized projection snapshot is read. Confirm the range again
                    // before advancing the response cursor so a newly missing event
                    // forces a replacement snapshot instead of a silent skip.
                    let confirmation_store = store.clone();
                    let confirmation_workspace = workspace_id.clone();
                    let confirmation_runtime = runtime_id.clone();
                    let confirmed = tokio::task::spawn_blocking(move || {
                        use storage_core::EventStore;
                        confirmation_store.read_workspace_event_page(
                            &confirmation_workspace,
                            &confirmation_runtime,
                            sequence,
                            500,
                        )
                    })
                    .await;
                    let Ok(Ok(confirmed_page)) = confirmed else {
                        return None;
                    };
                    if next_sequence < page.head_sequence
                        || next_sequence < confirmed_page.head_sequence
                    {
                        // Never advance the cursor past the source frontier represented by
                        // this snapshot. Leave the old cursor in place and retry after the
                        // next poll interval.
                        continue;
                    }
                    let resync_reason = (page.cursor_is_ahead
                        || page.has_gap
                        || confirmed_page.cursor_is_ahead
                        || confirmed_page.has_gap)
                        .then_some("replay_history_gap");
                    let frames = match serialize_task_projection(
                        snapshot,
                        &workspace_id,
                        &runtime_id,
                        &task_id,
                        next_sequence,
                        resync_reason,
                    ) {
                        Ok(frames) => frames,
                        Err(_) => return None,
                    };
                    sequence = next_sequence;
                    initial = frames.into_iter();
                    let frame = initial.next()?;
                    return Some((
                        Ok(Bytes::from(frame)),
                        (
                            permit,
                            shutdown,
                            store,
                            principal_id,
                            runtime_id,
                            workspace_id,
                            task_id,
                            sequence,
                            initial,
                        ),
                    ));
                }
            }
        },
    );
    let mut response = Response::new(Body::from_stream(output));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.litecowork.operator-stream+json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

fn encode_stream_cursor(cursor: &StreamCursor) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Stream cursor is unavailable",
        )
    })?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    if encoded.len() > 2048 {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Stream cursor is unavailable",
        ));
    }
    Ok(encoded)
}

fn decode_stream_cursor(encoded: &str) -> Result<StreamCursor, ()> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

fn serialize_stream_frame(frame: &impl Serialize) -> Result<Vec<u8>, Response> {
    let bytes = serde_json::to_vec(frame).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Stream frame could not be created",
        )
    })?;
    if bytes.is_empty() || bytes.len() > operator_ipc::MAX_STREAM_FRAME_BYTES {
        return Err(presentation_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "LIMIT_EXCEEDED",
            "Stream projection exceeds the frame limit",
        ));
    }
    Ok(bytes)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationPresentationQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

async fn get_conversation_presentation(
    State(state): State<ApiState>,
    Path(conversation_id): Path<String>,
    Query(query): Query<ConversationPresentationQuery>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let limit = query.limit.unwrap_or(50);
    if !valid_task_query_id(&workspace_id)
        || !valid_task_query_id(&conversation_id)
        || query
            .cursor
            .as_deref()
            .is_some_and(|id| !valid_task_query_id(id))
        || !(1..=MAX_CONVERSATION_PRESENTATION_MESSAGES).contains(&limit)
    {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation presentation lookup is invalid",
        ));
    }

    tokio::task::spawn_blocking(move || {
        conversation_presentation_response(
            &state.store,
            &state.principal_id,
            &state.runtime_id,
            &workspace_id,
            &conversation_id,
            query.cursor.as_deref(),
            limit,
        )
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Conversation presentation is unavailable",
        )
    })?
}

fn conversation_presentation_response(
    store: &storage_sqlite::SqliteWorkspaceStore,
    principal_id: &str,
    origin_runtime_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Response, Response> {
    let snapshot = SqliteRichPresentationStore::new(store.clone())
        .read_conversation_presentation(
            principal_id,
            workspace_id,
            conversation_id,
            origin_runtime_id,
            cursor,
            limit,
        )
        .map_err(|error| match error {
            StoreError::NotFound => presentation_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Conversation is unavailable",
            ),
            StoreError::Invalid(_) => presentation_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Conversation presentation lookup is invalid",
            ),
            _ => presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTEGRITY_FAILURE",
                "Conversation presentation is unavailable",
            ),
        })?;
    let mut response = Json(snapshot).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

#[derive(Serialize)]
struct RichPresentationDocumentResponse {
    workspace_id: String,
    conversation_id: String,
    message_id: String,
    presentation_id: String,
    schema_version: u32,
    renderer_contract_version: u32,
    semantic_content_digest: String,
    document_digest: String,
    document_size_bytes: u64,
    document: serde_json::Value,
}

async fn get_rich_presentation(
    State(state): State<ApiState>,
    Path(presentation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&presentation_id) {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Rich presentation lookup is invalid",
        ));
    }

    tokio::task::spawn_blocking(move || {
        rich_presentation_response(
            &state.store,
            &state.principal_id,
            &workspace_id,
            &presentation_id,
        )
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Rich presentation is unavailable",
        )
    })?
}

/// Reads only the explicitly contracted document fields. The storage adapter checks
/// owner authority again and verifies the immutable document digest, semantic message
/// digest, Conversation/Message binding, and producer-session binding in its read
/// transaction. This Operator boundary repeats identity/size checks before JSON
/// serialization and never returns private session or host-guidance metadata.
fn rich_presentation_response(
    store: &storage_sqlite::SqliteWorkspaceStore,
    principal_id: &str,
    workspace_id: &str,
    presentation_id: &str,
) -> Result<Response, Response> {
    let workspace = store.get_workspace(workspace_id).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Rich presentation is unavailable",
        )
    })?;
    if workspace
        .as_ref()
        .is_none_or(|workspace| workspace.owner_principal_id != principal_id)
    {
        return Err(presentation_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }

    let stored = SqliteRichPresentationStore::new(store.clone())
        .read_rich_presentation(principal_id, workspace_id, presentation_id)
        .map_err(|_| {
            presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTEGRITY_FAILURE",
                "Rich presentation is unavailable",
            )
        })?
        .ok_or_else(|| {
            presentation_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Rich presentation is unavailable",
            )
        })?;

    let record = stored.presentation;
    let bytes = stored.canonical_document;
    let document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Rich presentation is unavailable",
        )
    })?;
    if !document.is_object()
        || !rich_presentation_response_binding_is_valid(
            &record,
            workspace_id,
            presentation_id,
            &bytes,
            &document,
        )
    {
        return Err(presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Rich presentation is unavailable",
        ));
    }

    let response = RichPresentationDocumentResponse {
        workspace_id: record.workspace_id,
        conversation_id: record.conversation_id,
        message_id: record.message_id,
        presentation_id: record.presentation_id,
        schema_version: record.schema_version,
        renderer_contract_version: record.renderer_contract_version,
        semantic_content_digest: record.semantic_content_digest,
        document_digest: record.document_digest,
        document_size_bytes: record.document_size_bytes,
        document,
    };
    let mut response = Json(response).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

fn rich_presentation_response_binding_is_valid(
    record: &RichPresentationRecord,
    workspace_id: &str,
    presentation_id: &str,
    bytes: &[u8],
    document: &serde_json::Value,
) -> bool {
    record.workspace_id == workspace_id
        && record.presentation_id == presentation_id
        && record.schema_version == 1
        && record.renderer_contract_version >= 1
        && record.version == 1
        && record.document_size_bytes > 0
        && record.document_size_bytes <= MAX_RICH_PRESENTATION_BYTES
        && record.document_size_bytes == bytes.len() as u64
        && record.document_ref.size_bytes == record.document_size_bytes
        && record.document_ref.digest == record.document_digest
        && record.document_digest == sha256_digest(bytes)
        && document["presentation_id"] == record.presentation_id
        && document["message_id"] == record.message_id
        && document["semantic_content_digest"] == record.semantic_content_digest
        && document["schema_version"] == record.schema_version
        && document["renderer_contract_version"] == record.renderer_contract_version
}

#[derive(Serialize)]
struct TaskProgressProjection {
    task_id: String,
    computed_at: String,
    last_activity_at: Option<String>,
    last_activity_source: Option<&'static str>,
    last_evidence_at: Option<String>,
    activity_summary: Option<String>,
    active_workstreams: Vec<TaskWorkstreamProjection>,
    blockers: Vec<serde_json::Value>,
    newest_artifact: Option<ArtifactVersionRef>,
}

#[derive(Serialize)]
struct TaskWorkstreamProjection {
    step_id: String,
    title: String,
    step_status: String,
    active_attempt_ids: Vec<String>,
    worker_labels: Vec<String>,
    last_activity_at: Option<String>,
}

#[derive(Serialize)]
struct ArtifactVersionRef {
    workspace_id: String,
    artifact_id: String,
    version: u64,
}

#[derive(Serialize)]
struct TaskPresentationSnapshot {
    workspace_id: String,
    task_id: String,
    computed_at: String,
    freshness: &'static str,
    items: Vec<PresentationItem>,
}

#[derive(Serialize)]
struct PresentationItem {
    item_key: String,
    kind: &'static str,
    payload_version: u32,
    source_refs: Vec<PresentationSourceRef>,
    occurred_at: Option<String>,
    order_key: String,
    status: &'static str,
    label: Option<String>,
    payload: serde_json::Value,
    freshness: &'static str,
}

#[derive(Serialize)]
struct PresentationSourceRef {
    kind: &'static str,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<String>,
}

fn task_presentation_view(
    snapshot: TaskPresentationReadModel,
    workspace_id: String,
) -> Result<(TaskPresentationSnapshot, u64), Response> {
    if snapshot.steps_overflow {
        return Err(presentation_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Task has too many current Steps for one presentation snapshot",
        ));
    }
    if snapshot.artifacts_overflow {
        return Err(presentation_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Task has too many Artifacts for one presentation snapshot",
        ));
    }
    let origin_sequence = snapshot.origin_sequence;
    let task = snapshot.task;
    let task_id = task.task.task_id.clone();
    let mut items = Vec::with_capacity(1 + snapshot.steps.len() + snapshot.artifacts.len());
    items.push(PresentationItem {
        item_key: format!("task:{task_id}"),
        kind: "TASK_CARD",
        payload_version: 1,
        source_refs: vec![PresentationSourceRef {
            kind: "TASK",
            id: task_id.clone(),
            revision: Some(task.task.version.to_string()),
        }],
        occurred_at: Some(task.task.updated_at.clone()),
        order_key: format!("0:{task_id}"),
        status: task_presentation_status(&task.task.status),
        label: Some(presentation_label(
            &task.current_spec_revision.objective,
            512,
        )),
        payload: json!({
            "task_id": task_id,
            "objective": task.current_spec_revision.objective,
            "status": task.task.status,
        }),
        freshness: "CURRENT",
    });

    for step in snapshot.steps {
        items.push(PresentationItem {
            item_key: format!("step:{}", step.step_id),
            kind: "ACTIVITY",
            payload_version: 1,
            source_refs: vec![PresentationSourceRef {
                kind: "STEP",
                id: step.step_id.clone(),
                revision: Some(step.version.to_string()),
            }],
            occurred_at: Some(step.updated_at.clone()),
            order_key: format!("1:{}:{}", step.plan_revision, step.step_id),
            status: step_presentation_status(&step.status),
            label: Some(presentation_label(&step.title, 512)),
            payload: json!({
                "summary": step.title,
                "detail": format!("Step status: {}", step.status),
            }),
            freshness: "CURRENT",
        });
    }

    for source in snapshot.artifacts {
        let artifact = source.artifact;
        let version = source.version;
        if version.version > 9_007_199_254_740_991 {
            return Err(presentation_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INTEGRITY_FAILURE",
                "Task presentation contains an unsupported Artifact version",
            ));
        }
        let source_refs = vec![
            PresentationSourceRef {
                kind: "ARTIFACT_VERSION",
                id: artifact.artifact_id.clone(),
                revision: Some(version.version.to_string()),
            },
            PresentationSourceRef {
                kind: "RESOURCE_REVISION",
                id: version.resource_revision_id.clone(),
                revision: None,
            },
        ];
        items.push(PresentationItem {
            item_key: format!("artifact:{}:{}", artifact.artifact_id, version.version),
            kind: "ARTIFACT",
            payload_version: 1,
            source_refs,
            occurred_at: Some(version.created_at.clone()),
            order_key: format!("2:{}:{}", version.created_at, artifact.artifact_id),
            status: "UNKNOWN",
            label: Some(presentation_label(&artifact.display_name, 512)),
            payload: json!({
                "artifact_id": artifact.artifact_id,
                "version": version.version,
                "display_name": artifact.display_name,
                "artifact_kind": artifact.kind,
            }),
            freshness: "CURRENT",
        });
    }

    let computed_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| {
            presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Task presentation is unavailable",
            )
        })?;
    Ok((
        TaskPresentationSnapshot {
            workspace_id,
            task_id,
            computed_at,
            freshness: "CURRENT",
            items,
        },
        origin_sequence,
    ))
}

async fn get_task_presentation(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Task presentation lookup is invalid",
        ));
    }

    let projection = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &workspace_id)?;
        let Some(snapshot) = state
            .store
            .get_task_presentation(&workspace_id, &task_id, &state.runtime_id)
            .map_err(|_| {
                presentation_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL",
                    "Task presentation is unavailable",
                )
            })?
        else {
            return Err(presentation_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Task presentation is unavailable",
            ));
        };
        task_presentation_view(snapshot, workspace_id).map(|(projection, _)| projection)
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Task presentation is unavailable",
        )
    })??;

    let mut response = Json(projection).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

/// Returns a bounded progress projection from one persisted Task presentation read.
/// This endpoint does not sample process liveness, provider progress, or Environment
/// observations, so those source classes remain absent rather than inferred.
async fn get_task_progress(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(presentation_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Task progress lookup is invalid",
        ));
    }

    let projection = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &workspace_id)?;
        let Some(snapshot) = state
            .store
            .get_task_presentation(&workspace_id, &task_id, &state.runtime_id)
            .map_err(|_| {
                presentation_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL",
                    "Task progress is unavailable",
                )
            })?
        else {
            return Err(presentation_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Task progress is unavailable",
            ));
        };
        if snapshot.steps_overflow || snapshot.artifacts_overflow {
            return Err(presentation_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "INVALID_ARGUMENT",
                "Task progress source set exceeds the bounded projection limit",
            ));
        }
        project_task_progress(snapshot)
    })
    .await
    .map_err(|_| {
        presentation_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Task progress request did not complete",
        )
    })??;

    let mut response = Json(projection).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

fn project_task_progress(
    snapshot: TaskPresentationReadModel,
) -> Result<TaskProgressProjection, Response> {
    if snapshot.task.task.blocking_conditions.len() > 100 {
        return Err(presentation_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Task has too many blockers for one progress projection",
        ));
    }
    let task_id = snapshot.task.task.task_id.clone();
    let blockers = snapshot.task.task.blocking_conditions.clone();
    let last_evidence_at = snapshot.last_evidence_at.clone();
    let current_step_ids: std::collections::HashSet<&str> = snapshot
        .steps
        .iter()
        .map(|step| step.step_id.as_str())
        .collect();
    let latest_event = snapshot.activity_events.iter()
        .filter(|event| event.entity_type == "Task"
            || (event.entity_type == "Step" && current_step_ids.contains(event.entity_id.as_str()))
            // Current terminal Attempts remain valid historical activity sources.
            // Workstream admission below independently filters to nonterminal Attempts.
            || (event.entity_type == "Attempt" && snapshot.current_attempts.iter().any(|attempt| attempt.attempt_id == event.entity_id)))
        .max_by(|left, right| left.recorded_at.cmp(&right.recorded_at)
            .then_with(|| left.entity_type.cmp(&right.entity_type))
            .then_with(|| left.entity_id.cmp(&right.entity_id)));

    let last_activity_at = latest_event.map(|event| event.recorded_at.clone());
    let last_activity_source = latest_event.and_then(|event| match event.entity_type.as_str() {
        "Task" => Some("TASK_EVENT"),
        "Step" => Some("STEP_EVENT"),
        "Attempt" => Some("ATTEMPT_EVENT"),
        _ => None,
    });
    let activity_summary = latest_event.and_then(safe_activity_summary);

    let mut active_workstreams = Vec::new();
    for step in &snapshot.steps {
        let active: Vec<_> = snapshot
            .current_attempts
            .iter()
            .filter(|attempt| {
                attempt.step_id == step.step_id && is_nonterminal_attempt_status(&attempt.status)
            })
            .collect();
        if active.is_empty() {
            continue;
        }
        let mut attempt_ids = Vec::with_capacity(active.len());
        let mut worker_labels = Vec::with_capacity(active.len());
        let mut last_activity_at: Option<&str> = None;
        for attempt in active {
            attempt_ids.push(attempt.attempt_id.clone());
            if let Some(label) = attempt.worker_label.as_deref()
                && !worker_labels.contains(&label.to_owned())
            {
                worker_labels.push(label.to_owned());
            }
            if let Some(event_at) = attempt
                .last_event
                .as_ref()
                .map(|event| event.recorded_at.as_str())
                && last_activity_at.is_none_or(|current| event_at > current)
            {
                last_activity_at = Some(event_at);
            }
        }
        active_workstreams.push(TaskWorkstreamProjection {
            step_id: step.step_id.clone(),
            title: step.title.clone(),
            step_status: step.status.clone(),
            active_attempt_ids: attempt_ids,
            worker_labels,
            last_activity_at: last_activity_at.map(str::to_owned),
        });
    }

    let newest_artifact = snapshot
        .artifacts
        .iter()
        .max_by(|left, right| {
            left.version
                .created_at
                .cmp(&right.version.created_at)
                .then_with(|| left.artifact.artifact_id.cmp(&right.artifact.artifact_id))
        })
        .filter(|artifact| artifact.version.version <= 9_007_199_254_740_991)
        .map(|artifact| ArtifactVersionRef {
            workspace_id: artifact.artifact.workspace_id.clone(),
            artifact_id: artifact.artifact.artifact_id.clone(),
            version: artifact.version.version,
        });

    let computed_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| {
            presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Task progress is unavailable",
            )
        })?;
    Ok(TaskProgressProjection {
        task_id,
        computed_at,
        last_activity_at,
        last_activity_source,
        last_evidence_at,
        activity_summary,
        active_workstreams,
        blockers,
        newest_artifact,
    })
}

fn is_nonterminal_attempt_status(status: &str) -> bool {
    matches!(
        status,
        "CREATED"
            | "PREPARING"
            | "RUNNING"
            | "WAITING_APPROVAL"
            | "WAITING_RESOURCE"
            | "CHECKPOINTING"
            | "CANCEL_REQUESTED"
    )
}

fn safe_activity_summary(event: &TaskPresentationActivityEvent) -> Option<String> {
    match (event.entity_type.as_str(), event.event_type.as_str()) {
        ("Task", "task.created.v1") => Some("Task saved".to_owned()),
        ("Task", "task.status.changed.v1") => Some("Task status changed".to_owned()),
        ("Task", "task.spec.revised.v1") => Some("Task details updated".to_owned()),
        ("Task", "task.plan.revised.v1") => Some("Plan updated".to_owned()),
        ("Step", "step.status.changed.v1") => Some("Step status changed".to_owned()),
        ("Attempt", "attempt.created.v1") => Some("Work attempt started".to_owned()),
        ("Attempt", "attempt.status.changed.v1") => Some("Work attempt status changed".to_owned()),
        _ => None,
    }
}

fn task_presentation_status(task_status: &str) -> &'static str {
    match task_status {
        "RUNNING" | "VERIFYING" | "PAUSE_REQUESTED" | "CANCEL_REQUESTED" => "IN_PROGRESS",
        "WAITING_USER" | "NEEDS_USER" => "NEEDS_USER",
        "BLOCKED" | "PAUSED" | "READY" => "WAITING",
        "COMPLETED" => "COMPLETE",
        "FAILED" => "FAILED",
        "INCOMPLETE" | "CANCELLED" => "INCOMPLETE",
        _ => "UNKNOWN",
    }
}

fn step_presentation_status(step_status: &str) -> &'static str {
    match step_status {
        "RUNNING" | "VERIFYING" | "CANCEL_REQUESTED" => "IN_PROGRESS",
        "WAITING_USER" => "NEEDS_USER",
        "PENDING" | "READY" | "BLOCKED" => "WAITING",
        "COMPLETED" => "COMPLETE",
        "FAILED" => "FAILED",
        "CANCELLED" | "SUPERSEDED" => "INCOMPLETE",
        _ => "UNKNOWN",
    }
}

fn presentation_label(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn presentation_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    operator_error(status, code, message)
}

#[cfg(test)]
mod tests {
    use super::{
        ConversationBatchStreamCursor, decode_batch_stream_cursor, encode_batch_stream_cursor,
        is_nonterminal_attempt_status, serialize_conversation_batch_pages,
        serialize_conversation_resync_frames, step_presentation_status, task_presentation_status,
    };
    use serde_json::json;
    use storage_core::rich_presentation::ConversationPresentationSnapshot;

    #[test]
    fn task_status_projection_is_total_and_truthful() {
        assert_eq!(task_presentation_status("RUNNING"), "IN_PROGRESS");
        assert_eq!(task_presentation_status("NEEDS_USER"), "NEEDS_USER");
        assert_eq!(task_presentation_status("READY"), "WAITING");
        assert_eq!(task_presentation_status("COMPLETED"), "COMPLETE");
        assert_eq!(task_presentation_status("FAILED"), "FAILED");
        assert_eq!(task_presentation_status("FUTURE_STATUS"), "UNKNOWN");
    }

    #[test]
    fn step_status_projection_does_not_claim_unobserved_activity() {
        assert_eq!(step_presentation_status("RUNNING"), "IN_PROGRESS");
        assert_eq!(step_presentation_status("PENDING"), "WAITING");
        assert_eq!(step_presentation_status("COMPLETED"), "COMPLETE");
        assert_eq!(step_presentation_status("FUTURE_STATUS"), "UNKNOWN");
    }

    #[test]
    fn terminal_current_attempts_are_activity_sources_not_active_workstreams() {
        assert!(is_nonterminal_attempt_status("RUNNING"));
        assert!(is_nonterminal_attempt_status("WAITING_RESOURCE"));
        assert!(!is_nonterminal_attempt_status("COMPLETED"));
        assert!(!is_nonterminal_attempt_status("FAILED"));
        assert!(!is_nonterminal_attempt_status("ABANDONED"));
        assert!(!is_nonterminal_attempt_status("CANCELLED"));
        assert!(!is_nonterminal_attempt_status("FUTURE_STATUS"));
    }

    #[test]
    fn twenty_maximum_escaped_message_frontiers_fit_the_stream_cursor_bound() {
        let cursor = ConversationBatchStreamCursor {
            version: 1,
            workspace_id: "\"".repeat(200),
            origin_runtime_id: "\"".repeat(200),
            filter_digest: "a".repeat(64),
            projection_version: 1,
            origin_sequence: u64::MAX,
            message_frontiers: vec![Some("\"".repeat(200)); 20],
            projection_revisions: vec![u64::MAX; 20],
            snapshot_complete: true,
        };
        let encoded = encode_batch_stream_cursor(&cursor)
            .expect("maximum valid multi-Conversation cursor remains bounded");
        assert!(encoded.len() <= super::MAX_BATCH_STREAM_CURSOR_BYTES);
        assert_eq!(decode_batch_stream_cursor(&encoded), Ok(cursor));
    }

    #[test]
    fn batch_replay_sends_changed_turn_state_and_skips_unchanged_conversations() {
        let snapshots = vec![
            ConversationPresentationSnapshot {
                workspace_id: "workspace-1".to_owned(),
                conversation_id: "conversation-1".to_owned(),
                projection_revision: 2,
                stream_cursor: "conversation:1:2".to_owned(),
                origin_sequence: "7".to_owned(),
                stream_message_frontier: None,
                items: Vec::new(),
                next_cursor: None,
                active_turn: Some(json!({"turn_id": "turn-1", "status": "RUNNING"})),
            },
            ConversationPresentationSnapshot {
                workspace_id: "workspace-1".to_owned(),
                conversation_id: "conversation-2".to_owned(),
                projection_revision: 1,
                stream_cursor: "conversation:2:1".to_owned(),
                origin_sequence: "7".to_owned(),
                stream_message_frontier: None,
                items: Vec::new(),
                next_cursor: None,
                active_turn: None,
            },
        ];
        let filter_digest = "a".repeat(64);
        let result = serialize_conversation_batch_pages(
            &snapshots,
            "workspace-1",
            "runtime-1",
            &filter_digest,
            6,
            &[None, None],
            &[1, 1],
        )
        .expect("batch replay serializes bounded snapshots");
        assert_eq!(result.frames.len(), 2);
        let first: serde_json::Value =
            serde_json::from_slice(&result.frames[0]).expect("changed turn snapshot JSON");
        assert_eq!(first["payload"]["conversation_id"], "conversation-1");
        assert_eq!(first["payload"]["active_turn"]["turn_id"], "turn-1");
        let first_cursor =
            decode_batch_stream_cursor(first["cursor"].as_str().expect("snapshot frame cursor"))
                .expect("live batch snapshot cursor is resumable");
        assert!(first_cursor.snapshot_complete);
        let checkpoint: serde_json::Value =
            serde_json::from_slice(&result.frames[1]).expect("final checkpoint JSON");
        assert_eq!(checkpoint["payload"]["conversation_id"], "conversation-2");

        let unchanged = serialize_conversation_batch_pages(
            &snapshots,
            "workspace-1",
            "runtime-1",
            &filter_digest,
            6,
            &[None, None],
            &[2, 1],
        )
        .expect("unchanged replay serializes its final checkpoint");
        assert_eq!(unchanged.frames.len(), 1);
        let checkpoint: serde_json::Value =
            serde_json::from_slice(&unchanged.frames[0]).expect("checkpoint JSON");
        assert_eq!(checkpoint["payload"]["conversation_id"], "conversation-2");
    }

    #[test]
    fn batch_resync_cursor_stays_at_delivered_message_pages_and_requests_catch_up() {
        let snapshots = vec![
            ConversationPresentationSnapshot {
                workspace_id: "workspace-1".to_owned(),
                conversation_id: "conversation-1".to_owned(),
                projection_revision: 2,
                stream_cursor: "conversation:1:2".to_owned(),
                origin_sequence: "7".to_owned(),
                stream_message_frontier: Some("message-2".to_owned()),
                items: vec![json!({"message":{"message_id":"message-1"}})],
                next_cursor: Some("message-1".to_owned()),
                active_turn: None,
            },
            ConversationPresentationSnapshot {
                workspace_id: "workspace-1".to_owned(),
                conversation_id: "conversation-2".to_owned(),
                projection_revision: 1,
                stream_cursor: "conversation:2:1".to_owned(),
                origin_sequence: "7".to_owned(),
                stream_message_frontier: Some("message-b".to_owned()),
                items: vec![json!({"message":{"message_id":"message-b"}})],
                next_cursor: None,
                active_turn: None,
            },
        ];

        let result = serialize_conversation_resync_frames(
            &snapshots,
            "workspace-1",
            "runtime-1",
            &"a".repeat(64),
        )
        .expect("bounded replacement snapshots serialize");

        assert!(result.has_more);
        assert_eq!(
            result.message_frontiers,
            vec![Some("message-1".to_owned()), Some("message-b".to_owned())]
        );
        let first: serde_json::Value =
            serde_json::from_slice(&result.frames[1]).expect("first snapshot frame is JSON");
        let first_cursor =
            decode_batch_stream_cursor(first["cursor"].as_str().expect("first cursor is present"))
                .expect("first replacement cursor decodes");
        assert!(!first_cursor.snapshot_complete);
        assert_eq!(
            first_cursor.message_frontiers[0].as_deref(),
            Some("message-1")
        );

        let last: serde_json::Value =
            serde_json::from_slice(&result.frames[2]).expect("last snapshot frame is JSON");
        let last_cursor =
            decode_batch_stream_cursor(last["cursor"].as_str().expect("last cursor is present"))
                .expect("last replacement cursor decodes");
        assert!(last_cursor.snapshot_complete);
        assert_eq!(last_cursor.message_frontiers, result.message_frontiers);
    }
}

#[cfg(test)]
mod rich_presentation_read_tests {
    use super::{conversation_presentation_response, rich_presentation_response};
    use axum::{
        body::to_bytes,
        http::{StatusCode, header},
    };
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use serde_json::{Value, json};
    use std::{fs, sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, EventDraft, EventStore, StoreError, rich_presentation::*};
    use storage_sqlite::{
        FileBlobStore, SqliteConfig, SqliteRichPresentationStore, SqliteWorkspaceStore,
        WorkspaceBlobKey, WorkspaceBlobKeyProvider,
    };
    use zeroize::Zeroizing;

    const OWNER: &str = "rich-read-owner";
    const WORKSPACE: &str = "rich-read-workspace";
    const OTHER_WORKSPACE: &str = "rich-read-other-workspace";
    const RUNTIME: &str = "runtime-rich-read";
    const PRESENTATION: &str = "rich-read-presentation";
    const NOW: &str = "2026-10-09T12:00:00.000000000Z";

    #[derive(Clone)]
    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([31_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown test key version".to_owned()));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    struct Fixture {
        _directory: tempfile::TempDir,
        database: std::path::PathBuf,
        store: SqliteWorkspaceStore,
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("temporary fixture directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
                .expect("private fixture directory");
        }
        let database = directory.path().join("state.sqlite3");
        let blobs = Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys));
        let store = SqliteWorkspaceStore::open(
            &database,
            blobs,
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("SQLite store opens");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Rich read fixture".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "rich-read-workspace-event".to_owned(),
                    origin_runtime_id: "runtime-rich-read".to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "rich-read-workspace-correlation".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: OTHER_WORKSPACE.to_owned(),
                name: "Other owned Workspace".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "rich-read-other-workspace-event".to_owned(),
                    origin_runtime_id: "runtime-rich-read".to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "rich-read-other-workspace-correlation".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("second owned Workspace commits");

        let connection = Connection::open(&database).expect("fixture database opens");
        connection
            .execute(
                "INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES('rich-read-conversation',?1,?2)",
                rusqlite::params![WORKSPACE, NOW],
            )
            .expect("Conversation fixture persists");
        connection
            .execute(
                "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('rich-read-message','rich-read-conversation','{}','AGENT','rich-read-turn','[{\"kind\":\"TEXT\",\"text\":\"Hello from LiteCowork\"}]',?1)",
                [NOW],
            )
            .expect("semantic message fixture persists");
        drop(connection);

        let semantic = json!({
            "content": [{"kind": "TEXT", "text": "Hello from LiteCowork"}],
            "resource_refs": []
        });
        let semantic_content_digest = digest(&canonical_json(&semantic));
        let document = json!({
            "schema_version": 1,
            "renderer_contract_version": 1,
            "presentation_id": PRESENTATION,
            "message_id": "rich-read-message",
            "semantic_content_digest": semantic_content_digest,
            "root_blocks": [{
                "kind": "TEXT_SLICE",
                "source": {
                    "start_utf8_byte": 0,
                    "end_utf8_byte_exclusive": "Hello from LiteCowork".len(),
                    "slice_digest": digest(b"Hello from LiteCowork")
                }
            }],
            "block_provenance": [{
                "block_path": "/0",
                "origin": "SEMANTIC_MESSAGE",
                "resource_refs": [],
                "artifact_refs": [],
                "evidence_refs": [],
                "verification_refs": []
            }]
        });
        let canonical_document = canonical_json(&document);
        let document_digest = digest(&canonical_document);
        let record = RichPresentationRecord {
            presentation_id: PRESENTATION.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            conversation_id: "rich-read-conversation".to_owned(),
            message_id: "rich-read-message".to_owned(),
            schema_version: 1,
            renderer_contract_version: 1,
            semantic_content_digest: semantic_content_digest.clone(),
            document_ref: storage_core::BlobRef {
                digest: document_digest.clone(),
                size_bytes: canonical_document.len() as u64,
                media_type: storage_core::rich_presentation::RICH_PRESENTATION_MEDIA_TYPE
                    .to_owned(),
            },
            document_digest: document_digest.clone(),
            document_size_bytes: canonical_document.len() as u64,
            producer_agent_session_id: None,
            host_instruction_digest: None,
            host_skill_refs: vec![],
            created_at: NOW.to_owned(),
            version: 1,
        };
        let event = EventDraft {
            event_id: "rich-read-publication-event".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "RichPresentation".to_owned(),
            entity_id: PRESENTATION.to_owned(),
            origin_runtime_id: "runtime-rich-read".to_owned(),
            entity_revision: 1,
            hlc_timestamp: NOW.to_owned(),
            correlation_id: "rich-read-publication-correlation".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "rich.presentation.published.v1".to_owned(),
            payload: json!({
                "presentation_id": PRESENTATION,
                "conversation_id": "rich-read-conversation",
                "message_id": "rich-read-message",
                "schema_version": 1,
                "renderer_contract_version": 1,
                "semantic_content_digest": semantic_content_digest,
                "document_digest": document_digest,
                "document_size_bytes": canonical_document.len(),
                "host_skill_refs": [],
                "aggregate_version": 1
            }),
            recorded_at: NOW.to_owned(),
        };
        SqliteRichPresentationStore::new(store.clone())
            .publish_rich_presentation(PublishRichPresentation {
                principal_id: OWNER.to_owned(),
                presentation: record,
                canonical_document,
                event,
            })
            .expect("RichPresentation publishes");

        Fixture {
            _directory: directory,
            database,
            store,
        }
    }

    async fn body_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("bounded response body");
        serde_json::from_slice(&bytes).expect("JSON response")
    }

    fn canonical_json(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).expect("JSON serializes")
    }

    fn digest(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        format!("sha256:{:x}", Sha256::digest(bytes))
    }

    #[tokio::test]
    async fn owner_reads_verified_document_without_cache_or_private_session_metadata() {
        let fixture = fixture();
        let response = rich_presentation_response(&fixture.store, OWNER, WORKSPACE, PRESENTATION)
            .expect("Workspace owner can read published document");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body = body_json(response).await;
        assert_eq!(body["workspace_id"], WORKSPACE);
        assert_eq!(body["conversation_id"], "rich-read-conversation");
        assert_eq!(body["message_id"], "rich-read-message");
        assert_eq!(body["presentation_id"], PRESENTATION);
        assert_eq!(body["schema_version"], 1);
        assert_eq!(
            body["document_size_bytes"],
            body["document"].to_string().len()
        );
        assert_eq!(body["document"]["root_blocks"][0]["kind"], "TEXT_SLICE");
        assert!(body.get("producer_agent_session_id").is_none());
        assert!(body.get("host_instruction_digest").is_none());
        assert!(body.get("host_skill_refs").is_none());
    }

    #[tokio::test]
    async fn owner_reads_semantic_conversation_snapshot_with_optional_presentation_ref() {
        let fixture = fixture();
        let response = conversation_presentation_response(
            &fixture.store,
            OWNER,
            RUNTIME,
            WORKSPACE,
            "rich-read-conversation",
            None,
            50,
        )
        .expect("Conversation owner can read its bounded snapshot");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body = body_json(response).await;
        assert_eq!(body["workspace_id"], WORKSPACE);
        assert_eq!(body["conversation_id"], "rich-read-conversation");
        assert_eq!(
            body["origin_sequence"],
            fixture
                .store
                .read_workspace_events(WORKSPACE)
                .unwrap()
                .last()
                .unwrap()
                .origin_sequence
                .to_string()
        );
        assert_eq!(
            body["items"][0]["message"]["message_id"],
            "rich-read-message"
        );
        assert_eq!(
            body["items"][0]["message"]["content"][0]["text"],
            "Hello from LiteCowork"
        );
        assert_eq!(
            body["items"][0]["rich_presentation"]["presentation_id"],
            PRESENTATION
        );
        assert!(body["items"][0].get("document").is_none());
        assert_eq!(body["items"][0]["linked_items"], json!([]));
    }

    #[test]
    fn conversation_snapshot_uses_the_same_nondisclosing_owner_boundary() {
        let fixture = fixture();
        let denied = conversation_presentation_response(
            &fixture.store,
            "foreign-principal",
            RUNTIME,
            WORKSPACE,
            "rich-read-conversation",
            None,
            50,
        )
        .expect_err("foreign owner is denied");
        assert_eq!(denied.status(), StatusCode::NOT_FOUND);

        let invalid_cursor = conversation_presentation_response(
            &fixture.store,
            OWNER,
            RUNTIME,
            WORKSPACE,
            "rich-read-conversation",
            Some("unknown-message"),
            50,
        )
        .expect_err("cursor cannot cross or invent message scope");
        assert_eq!(invalid_cursor.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn foreign_principal_cannot_read_or_learn_whether_presentation_exists() {
        let fixture = fixture();
        let denied = rich_presentation_response(
            &fixture.store,
            "foreign-principal",
            WORKSPACE,
            PRESENTATION,
        )
        .expect_err("foreign principal must be denied");
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn selecting_another_owned_workspace_does_not_expose_the_presentation() {
        let fixture = fixture();
        let response =
            rich_presentation_response(&fixture.store, OWNER, OTHER_WORKSPACE, PRESENTATION)
                .expect_err(
                    "a presentation bound to a different selected Workspace is unavailable",
                );

        // The owner is allowed to access the selected Workspace, but the presentation
        // is not in it. Keep the same non-disclosing response as an unknown id.
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn selected_workspace_header_is_required_and_rejects_blank_values() {
        let mut headers = axum::http::HeaderMap::new();
        let missing = super::super::selected_workspace(&headers)
            .expect_err("missing selected Workspace must be rejected");
        assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

        headers.insert("x-workspace-id", "  ".parse().expect("header value"));
        let blank = super::super::selected_workspace(&headers)
            .expect_err("blank selected Workspace must be rejected");
        assert_eq!(blank.status(), StatusCode::BAD_REQUEST);

        headers.insert("x-workspace-id", WORKSPACE.parse().expect("header value"));
        assert_eq!(
            super::super::selected_workspace(&headers)
                .expect("valid selected Workspace is retained"),
            WORKSPACE
        );
    }

    #[test]
    fn malformed_rich_presentation_path_ids_are_rejected_by_the_operator_id_policy() {
        for malformed in ["", " \t", "x\nheader", &"x".repeat(201)] {
            assert!(
                !super::super::valid_task_query_id(malformed),
                "malformed id should be rejected: {malformed:?}"
            );
        }
        assert!(super::super::valid_task_query_id(WORKSPACE));
        assert!(super::super::valid_task_query_id(PRESENTATION));
    }

    #[test]
    fn corrupted_stored_digest_fails_closed_without_returning_document() {
        let fixture = fixture();
        let connection = Connection::open(&fixture.database).expect("database opens");
        // Simulate on-disk tampering below the immutable aggregate boundary.
        connection
            .execute_batch("DROP TRIGGER rich_presentation_no_update")
            .expect("test disables immutability guard for corruption injection");
        connection
            .execute(
                "UPDATE rich_presentations SET document_digest=?1 WHERE presentation_id=?2",
                rusqlite::params![format!("sha256:{}", "0".repeat(64)), PRESENTATION],
            )
            .expect("fixture corruption succeeds");
        drop(connection);

        let response = rich_presentation_response(&fixture.store, OWNER, WORKSPACE, PRESENTATION)
            .expect_err("corrupt document must not be served");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn document_bound_to_another_message_fails_closed() {
        let fixture = fixture();
        let connection = Connection::open(&fixture.database).expect("database opens");
        // Simulate corruption below the immutable aggregate boundary.
        connection
            .execute_batch("DROP TRIGGER rich_presentation_no_update")
            .expect("test disables immutability guard for corruption injection");
        connection
            .execute(
                "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('foreign-message','rich-read-conversation','{}','AGENT','foreign-turn','[{\"kind\":\"TEXT\",\"text\":\"Hello from LiteCowork\"}]',?1)",
                [NOW],
            )
            .expect("second committed-message fixture persists");
        connection
            .execute(
                "UPDATE rich_presentations SET message_id='foreign-message' WHERE presentation_id=?1",
                [PRESENTATION],
            )
            .expect("fixture message binding corruption succeeds");
        drop(connection);

        let response = rich_presentation_response(&fixture.store, OWNER, WORKSPACE, PRESENTATION)
            .expect_err("document without its committed message must not be served");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
