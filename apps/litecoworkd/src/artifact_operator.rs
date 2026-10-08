use super::*;
use storage_core::{
    ArtifactContentRecord, ArtifactReadStore, ArtifactRecord, ArtifactVersionAppendCommit,
    ArtifactVersionRecord, ArtifactVersionWriteStore, ResourceRecord, ResourceRevisionRecord,
    WorkspaceCreateRequest,
};

const MAX_USER_TEXT_BYTES: usize = 1_048_576;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/artifacts", get(list_artifacts))
        .route("/v1/library", get(list_library))
        .route("/v1/artifacts/{artifact_id}", get(get_artifact))
        .route("/v1/artifacts/{artifact_id}/versions/{version}", get(get_artifact_version))
        .route("/v1/artifacts/{artifact_id}/versions/{version}/content", get(download_artifact_version))
        .route("/v1/artifacts/{artifact_id}/edit-head", get(get_text_edit_head))
        .route("/v1/artifacts/{artifact_id}/text-version", post(append_text_version).layer(DefaultBodyLimit::max(7 * 1024 * 1024)))
        .route("/v1/tasks/{task_id}/artifacts", get(list_task_artifacts))
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactQuery { library_status: Option<String>, cursor: Option<String>, limit: Option<usize> }

#[derive(Deserialize, Serialize)]
struct ArtifactCursor { workspace_id: String, library_status: Option<String>, created_at: String, artifact_id: String }

fn artifact_error(error: StoreError) -> Response {
    match error {
        StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "STALE_VERSION", "Artifact changed; reload the current version before saving"),
        StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Artifact is unavailable"),
        StoreError::Invalid(ref message) if message == "ARTIFACT_ARCHIVED" => operator_error(StatusCode::CONFLICT, "ARTIFACT_ARCHIVED", "Archived Artifacts cannot be edited"),
        StoreError::Invalid(ref message) if message == "ARTIFACT_READ_LIMIT_EXCEEDED" => operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Artifact exceeds the local Operator transfer limit"),
        StoreError::Invalid(ref message) if message == "ARTIFACT_EXTERNAL_CONTENT_UNAVAILABLE" => operator_error(StatusCode::SERVICE_UNAVAILABLE, "PROVIDER_UNAVAILABLE", "The linked Artifact provider is unavailable"),
        StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Artifact selection is invalid"),
        StoreError::Blob(_) | StoreError::Integrity(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INTEGRITY_FAILURE", "Artifact content could not be verified"),
        _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Artifact storage is unavailable"),
    }
}

#[derive(Serialize)]
struct TextEditHead {
    artifact_id: String,
    aggregate_version: u64,
    content_version: u64,
    resource_version: u64,
    parent_resource_revision_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppendTextVersionRequest {
    expected_content_version: u64,
    expected_resource_version: u64,
    expected_parent_resource_revision_id: String,
    content: String,
}

async fn get_text_edit_head(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(artifact_id): Path<String>,
) -> Result<Response, Response> {
    let workspace_id = authorized_workspace(&state, &headers)?;
    let heads = state.store.get_artifact_append_heads(&workspace_id, &artifact_id)
        .map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
    let version = state.store.get_artifact_version(&workspace_id, &artifact_id, heads.artifact.current_version)
        .map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
    let parent = heads.resource.current_revision_id.clone().ok_or_else(|| artifact_error(StoreError::Invalid("Artifact resource has no current revision".to_owned())))?;
    if version.resource_revision_id != parent {
        return Err(artifact_error(StoreError::Integrity("Artifact and backing Resource heads do not match".to_owned())));
    }
    if heads.artifact.library_status == "ARCHIVED"
        || !matches!(version.content, ArtifactContentRecord::ManagedBlob { ref media_type, size_bytes, .. } if media_type == "text/plain" && size_bytes <= MAX_USER_TEXT_BYTES as u64)
    {
        return Err(operator_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_ARGUMENT", "Only managed text/plain Artifacts up to 1 MiB can be edited"));
    }
    Ok(no_store(Json(TextEditHead {
        artifact_id,
        aggregate_version: heads.artifact.version,
        content_version: heads.artifact.current_version,
        resource_version: heads.resource.version,
        parent_resource_revision_id: parent,
    }).into_response()))
}

async fn append_text_version(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(artifact_id): Path<String>,
    Json(body): Json<AppendTextVersionRequest>,
) -> Result<Response, Response> {
    let workspace_id = authorized_workspace(&state, &headers)?;
    let expected_artifact_version = parse_if_match(&headers)?;
    let expected_content_version = body.expected_content_version;
    let expected_resource_version = body.expected_resource_version;
    let parent_revision_id = body.expected_parent_resource_revision_id;
    if expected_content_version == 0 || expected_resource_version == 0 || parent_revision_id.is_empty()
        || parent_revision_id.len() > 160 || !parent_revision_id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Artifact revision preconditions are invalid"));
    }
    let request_id = idempotency_key(&headers)?;
    if body.content.len() > MAX_USER_TEXT_BYTES {
        return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Text Artifact versions must not exceed 1 MiB"));
    }
    if body.content.chars().any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t')) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Artifact text cannot contain control characters other than line breaks and tabs"));
    }
    let expected_parent = parent_revision_id;
    let revision_id = idempotent_id("rrev", &state.principal_id, &request_id, "artifact-text-edit");
    let correlation_id = idempotent_id("cor", &state.principal_id, &request_id, "artifact-text-edit");
    let provenance = json!({"source_inputs":[], "transformations":[{"operation":"user.text_edit", "inputs":[]}], "tool_reports":[]});
    let authored_by = json!({"principal_id":state.principal_id.clone(), "kind":"USER"});
    let request_payload = json!({
        "workspace_id":workspace_id.clone(), "artifact_id":artifact_id.clone(),
        "expected_artifact_version":expected_artifact_version, "expected_content_version":expected_content_version,
        "expected_resource_version":expected_resource_version, "expected_parent_resource_revision_id":expected_parent.clone(),
        "resource_revision_id":revision_id.clone(), "content_digest":sha256_digest(body.content.as_bytes()),
        "content_media_type":"text/plain", "content_size_bytes":body.content.len(),
        "input_refs":[], "provenance":provenance.clone(), "verification_refs":[], "created_by_attempt":null,
        "authored_by":authored_by.clone(),
    });
    if let Some(replayed) = state.store.resolve_artifact_version_append_replay(
        &state.principal_id, &workspace_id, &request_id, &request_payload,
    ).map_err(artifact_error)? {
        return Ok(append_response(replayed, StatusCode::OK));
    }
    let heads = state.store.get_artifact_append_heads(&workspace_id, &artifact_id)
        .map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
    let current = state.store.get_artifact_version(&workspace_id, &artifact_id, heads.artifact.current_version)
        .map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
    if !matches!(current.content, ArtifactContentRecord::ManagedBlob { ref media_type, size_bytes, .. } if media_type == "text/plain" && size_bytes <= MAX_USER_TEXT_BYTES as u64) {
        return Err(operator_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_ARGUMENT", "Only managed text/plain Artifacts up to 1 MiB can be edited"));
    }
    // Storage staging rechecks owner/workspace/artifact eligibility. Exact head freshness
    // is deliberately checked by append inside its publication transaction, after replay
    // lookup, so an identical retry can return its original committed receipt.
    let content = state.store.stage_text_content(&state.principal_id, &workspace_id, &artifact_id, body.content.as_bytes()).map_err(artifact_error)?;
    let now = operator_now().map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Artifact version could not be initialized"))?;
    let resource = ResourceRecord {
        current_revision_id: Some(revision_id.clone()), updated_at: now.clone(),
        version: heads.resource.version.checked_add(1).ok_or_else(|| operator_error(StatusCode::CONFLICT, "CONFLICT", "Resource version is exhausted"))?,
        ..heads.resource.clone()
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.clone(), resource_id: heads.resource.resource_id.clone(),
        parent_revision_ids: vec![expected_parent.clone()], provider_revision: None,
        content_digest: Some(content.digest.clone()), size_bytes: Some(content.size_bytes),
        media_type: Some("text/plain".to_owned()), observed_at: now.clone(),
        created_by: authored_by.clone(),
    };
    let next_content_version = expected_content_version.checked_add(1).ok_or_else(|| operator_error(StatusCode::CONFLICT, "CONFLICT", "Artifact version is exhausted"))?;
    let next_artifact_version = expected_artifact_version.checked_add(1).ok_or_else(|| operator_error(StatusCode::CONFLICT, "CONFLICT", "Artifact version is exhausted"))?;
    let artifact = ArtifactRecord {
        current_version: next_content_version, version: next_artifact_version,
        ..heads.artifact.clone()
    };
    let version = ArtifactVersionRecord {
        artifact_id: artifact_id.clone(), version: next_content_version,
        resource_revision_id: revision_id.clone(), input_refs: Vec::new(),
        content: ArtifactContentRecord::ManagedBlob {
            storage_ref: content.clone(), content_digest: content.digest.clone(),
            media_type: "text/plain".to_owned(), size_bytes: content.size_bytes,
        },
        provenance: provenance.clone(), created_by_attempt: None, verification_refs: Vec::new(), created_at: now.clone(),
    };
    let event_ctx = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Artifact events could not be initialized"))?;
    let resource_event = storage_core::EventDraft {
        event_id: new_id("ev").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Artifact events could not be initialized"))?,
        workspace_id: workspace_id.clone(), entity_type: "Resource".to_owned(), entity_id: resource.resource_id.clone(),
        origin_runtime_id: state.runtime_id.clone(), entity_revision: resource.version,
        hlc_timestamp: event_ctx.hlc_timestamp.clone(), correlation_id: correlation_id.clone(), causation_id: None,
        schema_version: 1, event_type: "resource.revision.observed.v1".to_owned(),
        payload: json!({"resource_id":resource.resource_id, "resource_revision_id":revision_id, "parent_revision_ids":[expected_parent], "content_digest":content.digest, "observed_at":now}),
        recorded_at: now.clone(),
    };
    let artifact_event = storage_core::EventDraft {
        event_id: new_id("ev").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Artifact events could not be initialized"))?,
        workspace_id: workspace_id.clone(), entity_type: "Artifact".to_owned(), entity_id: artifact_id.clone(),
        origin_runtime_id: state.runtime_id.clone(), entity_revision: artifact.version,
        hlc_timestamp: event_ctx.hlc_timestamp, correlation_id, causation_id: None,
        schema_version: 1, event_type: "artifact.version.created.v1".to_owned(),
        payload: json!({"artifact_id":artifact_id, "version":version.version, "resource_id":artifact.resource_id, "resource_revision_id":version.resource_revision_id, "input_refs":[], "content_kind":"MANAGED_BLOB", "content_digest":content.digest, "storage_ref":content, "aggregate_version":artifact.version}),
        recorded_at: now.clone(),
    };
    let commit = ArtifactVersionAppendCommit {
        request: WorkspaceCreateRequest { principal_id: state.principal_id, request_id, request_payload },
        workspace_id, artifact_id, expected_artifact_version, expected_content_version,
        expected_resource_version, expected_parent_resource_revision_id: expected_parent,
        artifact, version, resource, resource_revision: revision, resource_event, artifact_event,
    };
    let committed = state.store.append_artifact_version(commit).map_err(artifact_error)?;
    let status = if committed.replayed { StatusCode::OK } else { StatusCode::CREATED };
    Ok(append_response(committed, status))
}

fn append_response(committed: storage_core::CommittedArtifactVersionAppend, status: StatusCode) -> Response {
    let correlation_id = committed.events[0].correlation_id.clone();
    let mut response = (status, Json(json!({
        "artifact": committed.artifact,
        "version": committed.version,
        "replayed": committed.replayed,
    }))).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    no_store(response)
}

fn authorized_workspace(state: &ApiState, headers: &HeaderMap) -> Result<String, Response> {
    let workspace_id = selected_workspace(headers)?;
    ensure_workspace_owner(state, &workspace_id)?;
    Ok(workspace_id)
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

fn artifact_page(state: &ApiState, workspace_id: &str, query: ArtifactQuery) -> Result<Json<serde_json::Value>, Response> {
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) { return Err(artifact_error(StoreError::Invalid("limit".to_owned()))); }
    let cursor = query.cursor.as_deref().map(|encoded| {
        if encoded.len() > 2048 { return Err(artifact_error(StoreError::Invalid("cursor".to_owned()))); }
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| artifact_error(StoreError::Invalid("cursor".to_owned())))?;
        let cursor: ArtifactCursor = serde_json::from_slice(&bytes).map_err(|_| artifact_error(StoreError::Invalid("cursor".to_owned())))?;
        if cursor.workspace_id != workspace_id || cursor.library_status != query.library_status || cursor.created_at.is_empty() || cursor.created_at.len() > 64 || cursor.artifact_id.is_empty() || cursor.artifact_id.len() > 160 {
            return Err(artifact_error(StoreError::Invalid("cursor scope".to_owned())));
        }
        Ok(cursor)
    }).transpose()?;
    let mut items = state.store.list_artifacts_page(workspace_id, query.library_status.as_deref(), None, cursor.as_ref().map(|c| c.created_at.as_str()), cursor.as_ref().map(|c| c.artifact_id.as_str()), limit + 1).map_err(artifact_error)?;
    let has_more = items.len() > limit; items.truncate(limit);
    let next_cursor = if has_more {
        items.last().map(|item| serde_json::to_vec(&ArtifactCursor { workspace_id: workspace_id.to_owned(), library_status: query.library_status, created_at: item.created_at.clone(), artifact_id: item.artifact_id.clone() }).map(|bytes| URL_SAFE_NO_PAD.encode(bytes))).transpose().map_err(|_| artifact_error(StoreError::Integrity("cursor".to_owned())))?
    } else { None };
    Ok(Json(json!({ "items": items, "next_cursor": next_cursor })))
}

async fn list_artifacts(State(state): State<ApiState>, headers: HeaderMap, Query(query): Query<ArtifactQuery>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    Ok(no_store(artifact_page(&state, &workspace, query)?.into_response()))
}

async fn list_library(State(state): State<ApiState>, headers: HeaderMap, Query(mut query): Query<ArtifactQuery>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    query.library_status = Some("SAVED".to_owned());
    Ok(no_store(artifact_page(&state, &workspace, query)?.into_response()))
}

async fn get_artifact(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let artifact = state.store.get_artifact(&workspace, &id).map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
    Ok(no_store(Json(artifact).into_response()))
}

async fn get_artifact_version(State(state): State<ApiState>, headers: HeaderMap, Path((id, version)): Path<(String, u64)>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let artifact_version = state.store.get_artifact_version(&workspace, &id, version).map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
    Ok(no_store(Json(artifact_version).into_response()))
}

async fn download_artifact_version(State(state): State<ApiState>, headers: HeaderMap, Path((id, version)): Path<(String, u64)>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let (bytes, media_type) = tokio::task::spawn_blocking(move || {
        let metadata = state.store.get_artifact_version(&workspace, &id, version).map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
        let bytes = state.store.read_artifact_content_bounded(&workspace, &id, version, MAX_OPERATOR_RESPONSE_BYTES as u64).map_err(artifact_error)?.ok_or_else(|| artifact_error(StoreError::NotFound))?;
        // Recheck authority after resolving bytes. Archived Workspaces retain reads.
        ensure_workspace_owner(&state, &workspace)?;
        let media_type = match metadata.content {
            storage_core::ArtifactContentRecord::ManagedBlob { media_type, .. } => media_type,
            _ => return Err(artifact_error(StoreError::Invalid("ARTIFACT_EXTERNAL_CONTENT_UNAVAILABLE".to_owned()))),
        };
        Ok::<_, Response>((bytes, media_type))
    }).await.map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Artifact transfer did not complete"))??;
    Response::builder().status(StatusCode::OK)
        .header(header::CONTENT_TYPE, media_type)
        .header(header::CONTENT_LENGTH, bytes.len().to_string())
        .header(header::CONTENT_DISPOSITION, "attachment")
        .header("x-content-type-options", "nosniff")
        .header("content-security-policy", "sandbox; default-src 'none'")
        .header("cache-control", "no-store")
        .body(Body::from(bytes)).map_err(|_| artifact_error(StoreError::Integrity("response metadata".to_owned())))
}

async fn list_task_artifacts(State(state): State<ApiState>, headers: HeaderMap, Path(task_id): Path<String>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if state.store.get_task(&workspace, &task_id).map_err(artifact_error)?.is_none() { return Err(artifact_error(StoreError::NotFound)); }
    let items = state.store.list_artifacts_page(&workspace, None, Some(&task_id), None, None, 201).map_err(artifact_error)?;
    if items.len() > 200 { return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Task Artifact list exceeds the local read limit")); }
    Ok(no_store(Json(items).into_response()))
}
