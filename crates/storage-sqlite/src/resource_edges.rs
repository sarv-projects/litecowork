use super::*;
use storage_core::{CommittedDerivedResource, DerivedResourceCreateCommit, ResourceEdgeRecord};

pub(crate) fn create_derived_resource(
    store: &SqliteWorkspaceStore,
    mut commit: DerivedResourceCreateCommit,
) -> Result<CommittedDerivedResource, StoreError> {
    let edge = &commit.edge;
    if commit.request.principal_id.trim().is_empty()
        || commit.request.request_id.trim().is_empty()
        || commit.resource.version != 1
        || commit.revision.resource_id != commit.resource.resource_id
        || commit.edge.workspace_id != commit.resource.workspace_id
        || commit.edge.to_resource_id != commit.resource.resource_id
        || commit.edge.to_revision_id.as_deref()
            != Some(commit.revision.resource_revision_id.as_str())
        || commit.edge.relation != "DERIVED_FROM"
        || commit.edge.from_revision_id.is_none()
        || commit.resource_event.workspace_id != commit.resource.workspace_id
        || commit.resource_event.entity_type != "Resource"
        || commit.resource_event.entity_id != commit.resource.resource_id
        || commit.resource_event.entity_revision != commit.resource.version
    {
        return Err(StoreError::Invalid(
            "derived Resource publication identity is inconsistent".to_owned(),
        ));
    }
    let commit_fingerprint = digest(&canonical_json(&json!({
        "resource": &commit.resource,
        "revision": &commit.revision,
        "resource_event": &commit.resource_event,
        "edge": &commit.edge,
        "edge_event": &commit.edge_event,
    }))?);
    let request_object = commit
        .request
        .request_payload
        .as_object_mut()
        .ok_or_else(|| {
            StoreError::Invalid("Resource create request payload must be an object".to_owned())
        })?;
    if request_object.contains_key("derived_resource_edge") {
        return Err(StoreError::Invalid(
            "Resource create request already contains derived edge identity".to_owned(),
        ));
    }
    request_object.insert(
        "derived_resource_edge".to_owned(),
        json!({
            "edge_id": &edge.edge_id,
            "from_resource_id": &edge.from_resource_id,
            "from_revision_id": &edge.from_revision_id,
            "relation": &edge.relation,
            "to_resource_id": &edge.to_resource_id,
            "to_revision_id": &edge.to_revision_id,
            "edge_event_id": &commit.edge_event.event_id,
            "child_resource_id": &commit.resource.resource_id,
            "child_revision_id": &commit.revision.resource_revision_id,
            "content_digest": &commit.revision.content_digest,
            "commit_fingerprint": commit_fingerprint,
        }),
    );
    let content_digest = digest(&commit.content);
    if commit.revision.size_bytes != Some(commit.content.len() as u64)
        || commit.revision.content_digest.as_deref() != Some(content_digest.as_str())
    {
        return Err(StoreError::Invalid(
            "derived Resource content does not match its immutable revision".to_owned(),
        ));
    }
    let request_lock =
        store.create_request_lock(&commit.request.principal_id, &commit.request.request_id);
    let _request_guard = request_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(resource) = store.check_resource_create_receipt(&commit.request)? {
        let (reply, receiver) = mpsc::channel();
        let edge_event = store
            .execute_command(
                Command::GetWorkspaceEvent {
                    workspace_id: commit.resource.workspace_id.clone(),
                    event_id: commit.edge_event.event_id.clone(),
                    reply,
                },
                receiver,
            )?
            .ok_or_else(|| {
                StoreError::Integrity(
                    "derived Resource replay is missing its edge event".to_owned(),
                )
            })?;
        return Ok(CommittedDerivedResource {
            resource,
            edge: commit.edge,
            edge_event,
        });
    }
    match store.get_workspace(&commit.resource.workspace_id)? {
        Some(workspace) if workspace.status == "ACTIVE" => {}
        Some(_) => {
            return Err(StoreError::Invalid(
                "archived Workspace is read-only".to_owned(),
            ));
        }
        None => return Err(StoreError::NotFound),
    }
    let media_type = commit
        .revision
        .media_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    commit.revision.media_type = Some(media_type.clone());
    let should_index = resource_index::not_indexable_reason(
        &commit.resource.display_name,
        &media_type,
        &commit.content,
    )
    .is_none();
    let index_write_lock =
        should_index.then(|| store.resource_index_write_lock(&commit.resource.workspace_id));
    let _index_write_guard = index_write_lock
        .as_ref()
        .map(|lock| store.lock_resource_index_write(&commit.resource.workspace_id, lock));
    let text_index = if should_index {
        let key_version = store
            .ensure_resource_index_key_history_available(&commit.resource.workspace_id, None)?;
        resource_index::prepare(
            store.inner.blobs.as_ref(),
            &commit.resource.workspace_id,
            &commit.resource.resource_id,
            &commit.revision.resource_revision_id,
            &commit.resource.display_name,
            &media_type,
            commit
                .revision
                .content_digest
                .as_deref()
                .unwrap_or_default(),
            &commit.revision.observed_at,
            &commit.content,
            key_version,
            |blob| store.reserve_resource_index_snapshot(&commit.resource.workspace_id, blob),
        )?
    } else {
        None
    };
    let state_bytes = canonical_json(&commit.resource)?;
    let state_blob = store.inner.blobs.put(
        &commit.resource.workspace_id,
        BlobPurpose::AggregateState,
        &state_bytes,
        "application/vnd.litecowork.resource+json",
    )?;
    if store.inner.blobs.get(
        &commit.resource.workspace_id,
        BlobPurpose::AggregateState,
        &state_blob,
    )? != state_bytes
    {
        return Err(StoreError::Integrity(
            "Resource aggregate state failed blob verification".to_owned(),
        ));
    }
    let state_ref = AggregateStateRef {
        blob: state_blob,
        entity_revision: commit.resource.version,
        record_schema_version: 1,
    };
    let content_ref = BlobRef {
        digest: content_digest.clone(),
        size_bytes: commit.content.len() as u64,
        media_type: media_type.clone(),
    };
    let content_lock =
        store.resource_content_blob_operation_lock(&commit.resource.workspace_id, &content_digest);
    let _content_guard = content_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    store.reserve_resource_content_blob(&commit.resource.workspace_id, &content_ref)?;
    let content_blob = store.inner.blobs.put(
        &commit.resource.workspace_id,
        BlobPurpose::Resource,
        &commit.content,
        &media_type,
    )?;
    if content_blob != content_ref
        || store.inner.blobs.get(
            &commit.resource.workspace_id,
            BlobPurpose::Resource,
            &content_blob,
        )? != commit.content
    {
        return Err(StoreError::Integrity(
            "Resource content failed blob verification".to_owned(),
        ));
    }
    let edge_state_bytes = canonical_json(&commit.edge)?;
    let edge_state_blob = store.inner.blobs.put(
        &commit.edge.workspace_id,
        BlobPurpose::AggregateState,
        &edge_state_bytes,
        "application/vnd.litecowork.resource-edge+json",
    )?;
    if store.inner.blobs.get(
        &commit.edge.workspace_id,
        BlobPurpose::AggregateState,
        &edge_state_blob,
    )? != edge_state_bytes
    {
        return Err(StoreError::Integrity(
            "ResourceEdge aggregate state failed blob verification".to_owned(),
        ));
    }
    let edge_state_ref = AggregateStateRef {
        blob: edge_state_blob,
        entity_revision: 1,
        record_schema_version: 1,
    };
    let (reply, receiver) = mpsc::channel();
    let resource = store.execute_command(
        Command::CreateResource {
            request: commit.request,
            resource: commit.resource.clone(),
            revision: commit.revision,
            draft: commit.resource_event,
            state_ref,
            content_blob,
            text_index,
            location_id: format!("loc_{}", commit.resource.resource_id),
            edge: Some(Write {
                edge: commit.edge.clone(),
                event: commit.edge_event.clone(),
                state_ref: edge_state_ref,
            }),
            reply,
        },
        receiver,
    )?;
    let (reply, receiver) = mpsc::channel();
    let edge_event = store
        .execute_command(
            Command::GetWorkspaceEvent {
                workspace_id: resource.resource.workspace_id.clone(),
                event_id: commit.edge_event.event_id,
                reply,
            },
            receiver,
        )?
        .ok_or_else(|| {
            StoreError::Integrity("committed ResourceEdge event is missing".to_owned())
        })?;
    Ok(CommittedDerivedResource {
        resource,
        edge: commit.edge,
        edge_event,
    })
}

pub(crate) struct Write {
    pub edge: ResourceEdgeRecord,
    pub event: EventDraft,
    pub state_ref: AggregateStateRef,
}

pub(crate) fn insert(
    tx: &Transaction<'_>,
    edge: &ResourceEdgeRecord,
    draft: EventDraft,
    state_ref: AggregateStateRef,
) -> Result<DomainEvent, StoreError> {
    if edge.edge_id.trim().is_empty()
        || edge.workspace_id.trim().is_empty()
        || edge.from_resource_id.trim().is_empty()
        || edge.to_resource_id.trim().is_empty()
        || edge.from_resource_id == edge.to_resource_id
        || edge.observed_at.trim().is_empty()
        || !matches!(
            edge.relation.as_str(),
            "CONTAINS" | "DERIVED_FROM" | "REFERENCES" | "SAME_PROVIDER_OBJECT"
        )
        || draft.workspace_id != edge.workspace_id
        || draft.entity_type != "ResourceEdge"
        || draft.entity_id != edge.edge_id
        || draft.entity_revision != 1
        || draft.schema_version != 1
        || draft.event_type != "resource.edge.created.v1"
        || draft.payload
            != json!({
                "edge_id": edge.edge_id,
                "from_resource_id": edge.from_resource_id,
                "to_resource_id": edge.to_resource_id,
                "relation": edge.relation,
                "observed_at": edge.observed_at,
            })
    {
        return Err(StoreError::Invalid(
            "ResourceEdge and publication event are inconsistent".to_owned(),
        ));
    }
    let provenance_json = String::from_utf8(canonical_json(&edge.provenance)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let source_context_status: Option<String> = tx
        .query_row(
            "SELECT json_extract(context_document_json, '$.status') FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![edge.workspace_id, edge.from_resource_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .flatten();
    if source_context_status
        .as_deref()
        .is_some_and(|status| status != "ACTIVE")
    {
        return Err(StoreError::Invalid(
            "derived Resource source is not active".to_owned(),
        ));
    }
    tx.execute(
        "INSERT INTO resource_edges(edge_id, workspace_id, from_resource_id, from_revision_id, relation, to_resource_id, to_revision_id, observed_at, provenance_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![edge.edge_id, edge.workspace_id, edge.from_resource_id, edge.from_revision_id, edge.relation, edge.to_resource_id, edge.to_revision_id, edge.observed_at, provenance_json],
    )
    .map_err(map_database_error)?;

    tx.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
        params![draft.workspace_id, draft.origin_runtime_id],
    )
    .map_err(map_database_error)?;
    let sequence: i64 = tx
        .query_row(
            "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
            params![draft.workspace_id, draft.origin_runtime_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event = DomainEvent {
        event_id: draft.event_id,
        workspace_id: draft.workspace_id,
        entity_type: draft.entity_type,
        entity_id: draft.entity_id,
        origin_runtime_id: draft.origin_runtime_id,
        origin_sequence: from_sql_i64(sequence, "origin sequence")?,
        entity_revision: draft.entity_revision,
        hlc_timestamp: draft.hlc_timestamp,
        correlation_id: draft.correlation_id,
        causation_id: draft.causation_id,
        schema_version: draft.schema_version,
        event_type: draft.event_type,
        payload: draft.payload,
        aggregate_state_ref: state_ref,
        recorded_at: draft.recorded_at,
        payload_digest: digest(payload_json.as_bytes()),
    };
    tx.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![event.event_id, event.workspace_id, event.entity_type, event.entity_id, event.origin_runtime_id, to_sql_i64(event.origin_sequence, "origin sequence")?, to_sql_i64(event.entity_revision, "event revision")?, event.hlc_timestamp, event.correlation_id, event.causation_id, i64::from(event.schema_version), event.event_type, payload_json, String::from_utf8(canonical_json(&event.aggregate_state_ref)?).map_err(|error| StoreError::Invalid(error.to_string()))?, event.recorded_at, event.payload_digest],
    )
    .map_err(map_database_error)?;
    Ok(event)
}
