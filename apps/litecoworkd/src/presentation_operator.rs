//! Read-only, bounded Task presentation snapshots for the Operator.
//!
//! This module deliberately projects only committed Task, Step, and Artifact records.
//! It does not infer worker activity, progress percentages, verification outcomes, or
//! stream state. Mount [`routes`] from `operator::build_operator_router` after adding
//! `#[path = "presentation_operator.rs"] mod presentation_operator;` alongside the
//! other Operator submodules.

use super::*;
use serde::Serialize;
use serde_json::json;
use storage_core::TaskPresentationReadStore;

pub(super) fn routes() -> Router<ApiState> {
    Router::new().route("/v1/tasks/{task_id}/presentation", get(get_task_presentation))
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
            .map_err(|_| presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Task presentation is unavailable",
            ))? else {
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
            label: Some(presentation_label(&task.current_spec_revision.objective, 512)),
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
            .map_err(|_| presentation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Task presentation is unavailable",
            ))?;
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
    .map_err(|_| presentation_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL",
        "Task presentation is unavailable",
    ))??;

    let mut response = Json(projection).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
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
    use super::{step_presentation_status, task_presentation_status};

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
}
