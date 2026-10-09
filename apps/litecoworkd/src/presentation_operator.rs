//! Read-only, bounded Task presentation snapshots for the Operator.
//!
//! This module deliberately projects committed Task, Step, current Attempt, Evidence,
//! and Artifact records. It does not infer worker liveness, provider progress, progress
//! percentages, verification outcomes, or stream state. Mount [`routes`] from
//! `operator::build_operator_router` after adding
//! `#[path = "presentation_operator.rs"] mod presentation_operator;` alongside the
//! other Operator submodules.

use super::*;
use serde::Serialize;
use serde_json::json;
use storage_core::{
    TaskPresentationActivityEvent, TaskPresentationReadModel, TaskPresentationReadStore,
    rich_presentation::{
        MAX_CONVERSATION_PRESENTATION_MESSAGES, MAX_RICH_PRESENTATION_BYTES,
        RichPresentationRecord, RichPresentationStore,
    },
};
use storage_sqlite::SqliteRichPresentationStore;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
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
    workspace_id: &str,
    conversation_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Response, Response> {
    let snapshot = SqliteRichPresentationStore::new(store.clone())
        .read_conversation_presentation(principal_id, workspace_id, conversation_id, cursor, limit)
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
            .get_task_presentation(&workspace_id, &task_id)
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

        let task = snapshot.task;
        let mut items = Vec::with_capacity(1 + snapshot.steps.len() + snapshot.artifacts.len());
        let task_status = task_presentation_status(&task.task.status);
        items.push(PresentationItem {
            item_key: format!("task:{}", task.task.task_id),
            kind: "TASK_CARD",
            payload_version: 1,
            source_refs: vec![PresentationSourceRef {
                kind: "TASK",
                id: task.task.task_id.clone(),
                revision: Some(task.task.version.to_string()),
            }],
            occurred_at: Some(task.task.updated_at.clone()),
            order_key: format!("0:{}", task.task.task_id),
            status: task_status,
            label: Some(presentation_label(
                &task.current_spec_revision.objective,
                512,
            )),
            payload: json!({
                "task_id": task.task.task_id,
                "objective": task.current_spec_revision.objective,
                "status": task.task.status,
            }),
            freshness: "CURRENT",
        });

        for step in snapshot.steps {
            let status = step_presentation_status(&step.status);
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
                status,
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
            let mut source_refs = vec![PresentationSourceRef {
                kind: "ARTIFACT_VERSION",
                id: artifact.artifact_id.clone(),
                revision: Some(version.version.to_string()),
            }];
            source_refs.push(PresentationSourceRef {
                kind: "RESOURCE_REVISION",
                id: version.resource_revision_id.clone(),
                revision: None,
            });
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
        Ok::<_, Response>(TaskPresentationSnapshot {
            workspace_id,
            task_id,
            computed_at,
            // CURRENT means these persisted source heads were read from one SQLite
            // snapshot. It says nothing about worker liveness, execution or verification.
            freshness: "CURRENT",
            items,
        })
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
            .get_task_presentation(&workspace_id, &task_id)
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
        Ok::<_, Response>(project_task_progress(snapshot)?)
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
            if let Some(label) = attempt.worker_label.as_deref() {
                if !worker_labels.contains(&label.to_owned()) {
                    worker_labels.push(label.to_owned());
                }
            }
            if let Some(event_at) = attempt
                .last_event
                .as_ref()
                .map(|event| event.recorded_at.as_str())
            {
                if last_activity_at.map_or(true, |current| event_at > current) {
                    last_activity_at = Some(event_at);
                }
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
        is_nonterminal_attempt_status, step_presentation_status, task_presentation_status,
    };

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
    use storage_core::{BlobPurpose, EventDraft, StoreError, rich_presentation::*};
    use storage_sqlite::{
        FileBlobStore, SqliteConfig, SqliteRichPresentationStore, SqliteWorkspaceStore,
        WorkspaceBlobKey, WorkspaceBlobKeyProvider,
    };
    use zeroize::Zeroizing;

    const OWNER: &str = "rich-read-owner";
    const WORKSPACE: &str = "rich-read-workspace";
    const OTHER_WORKSPACE: &str = "rich-read-other-workspace";
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
