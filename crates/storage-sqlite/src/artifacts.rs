use super::*;
use serde::Serialize;
use storage_core::{
    ArtifactContentRecord, ArtifactReadStore, ArtifactRecord, ArtifactVersionAppendCommit,
    ArtifactVersionWriteStore, ArtifactVersionRecord, CommittedArtifactVersionAppend,
    DomainEvent, ResourceRecord, ResourceRevisionRecord, TaskPresentationArtifactVersion,
    TaskPresentationReadModel, TaskPresentationReadStore, MAX_TASK_PRESENTATION_ARTIFACTS,
    MAX_TASK_PRESENTATION_STEPS,
};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

impl ArtifactReadStore for SqliteWorkspaceStore {
    fn get_artifact(&self, workspace_id: &str, artifact_id: &str) -> Result<Option<ArtifactRecord>, StoreError> {
        validate_nonempty(&[workspace_id, artifact_id])?;
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::GetArtifact { workspace_id: workspace_id.to_owned(), artifact_id: artifact_id.to_owned(), reply }, receive)
    }

    fn get_artifact_append_heads(&self, workspace_id: &str, artifact_id: &str) -> Result<Option<storage_core::ArtifactAppendHeads>, StoreError> {
        validate_nonempty(&[workspace_id, artifact_id])?;
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::GetArtifactAppendHeads {
            workspace_id: workspace_id.to_owned(), artifact_id: artifact_id.to_owned(), reply,
        }, receive)
    }

    fn list_artifacts_page(&self, workspace_id: &str, library_status: Option<&str>, task_id: Option<&str>, after_created_at: Option<&str>, after_artifact_id: Option<&str>, limit: usize) -> Result<Vec<ArtifactRecord>, StoreError> {
        validate_nonempty(&[workspace_id])?;
        if !(1..=201).contains(&limit)
            || library_status.is_some_and(|status| !["TRANSIENT", "SAVED", "ARCHIVED"].contains(&status))
            || after_created_at.is_some() != after_artifact_id.is_some()
            || task_id.is_some_and(|id| id.trim().is_empty())
        { return Err(StoreError::Invalid("Artifact page query is invalid".to_owned())); }
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::ListArtifacts {
            workspace_id: workspace_id.to_owned(), library_status: library_status.map(str::to_owned), task_id: task_id.map(str::to_owned),
            after_created_at: after_created_at.map(str::to_owned), after_artifact_id: after_artifact_id.map(str::to_owned), limit, reply,
        }, receive)
    }

    fn get_artifact_version(&self, workspace_id: &str, artifact_id: &str, version: u64) -> Result<Option<ArtifactVersionRecord>, StoreError> {
        validate_nonempty(&[workspace_id, artifact_id])?;
        if version == 0 { return Err(StoreError::Invalid("Artifact version is invalid".to_owned())); }
        to_sql_i64(version, "Artifact version")?;
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::GetArtifactVersion { workspace_id: workspace_id.to_owned(), artifact_id: artifact_id.to_owned(), version, reply }, receive)
    }

    fn read_artifact_content_bounded(&self, workspace_id: &str, artifact_id: &str, version: u64, maximum_bytes: u64) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(version) = self.get_artifact_version(workspace_id, artifact_id, version)? else { return Ok(None) };
        let ArtifactContentRecord::ManagedBlob { storage_ref, content_digest, media_type, size_bytes } = version.content else {
            // External provider resolution must recheck current source authorization;
            // a pinned locator is not permission to perform a direct fetch.
            return Err(StoreError::Invalid("ARTIFACT_EXTERNAL_CONTENT_UNAVAILABLE".to_owned()));
        };
        if size_bytes > maximum_bytes { return Err(StoreError::Invalid("ARTIFACT_READ_LIMIT_EXCEEDED".to_owned())); }
        if !storage_core::is_sha256_digest(&content_digest) || storage_ref.digest != content_digest || storage_ref.size_bytes != size_bytes || storage_ref.media_type != media_type {
            return Err(StoreError::Integrity("Artifact content metadata does not match".to_owned()));
        }
        let bytes = self.inner.blobs.get(workspace_id, BlobPurpose::Artifact, &storage_ref)?;
        if bytes.len() as u64 != size_bytes || digest(&bytes) != content_digest {
            return Err(StoreError::Integrity("Artifact content failed verification".to_owned()));
        }
        Ok(Some(bytes))
    }
}

impl TaskPresentationReadStore for SqliteWorkspaceStore {
    fn get_task_presentation(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Option<TaskPresentationReadModel>, StoreError> {
        validate_nonempty(&[workspace_id, task_id])?;
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::GetTaskPresentation {
            workspace_id: workspace_id.to_owned(),
            task_id: task_id.to_owned(),
            reply,
        }, receive)
    }
}

/// Collects every source through one deferred read transaction. The SQLite writer
/// queue also prevents another LiteCowork write command from interleaving this read;
/// the transaction pins the database view for all SELECTs explicitly.
pub(super) fn read_task_presentation(
    connection: &mut Connection,
    workspace_id: &str,
    task_id: &str,
) -> Result<Option<TaskPresentationReadModel>, StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(map_database_error)?;
    let Some(task) = super::load_task_view(&transaction, workspace_id, task_id)? else {
        return Ok(None);
    };

    let (steps, steps_overflow) = if let Some(plan_revision) = task.task.current_plan_revision {
        let mut steps = list_presentation_steps(
            &transaction,
            workspace_id,
            task_id,
            plan_revision,
        )?;
        let overflow = steps.len() > MAX_TASK_PRESENTATION_STEPS;
        if overflow { steps.truncate(MAX_TASK_PRESENTATION_STEPS); }
        (steps, overflow)
    } else {
        (Vec::new(), false)
    };

    let mut artifact_rows = list_artifacts(
        &transaction,
        workspace_id,
        None,
        Some(task_id),
        None,
        None,
        MAX_TASK_PRESENTATION_ARTIFACTS + 1,
    )?;
    let artifacts_overflow = artifact_rows.len() > MAX_TASK_PRESENTATION_ARTIFACTS;
    artifact_rows.truncate(MAX_TASK_PRESENTATION_ARTIFACTS);

    let mut artifacts = Vec::with_capacity(artifact_rows.len());
    for artifact in artifact_rows {
        if let Some(version) = get_artifact_version(
            &transaction,
            workspace_id,
            &artifact.artifact_id,
            artifact.current_version,
        )? {
            artifacts.push(TaskPresentationArtifactVersion { artifact, version });
        }
    }

    transaction.commit().map_err(map_database_error)?;
    Ok(Some(TaskPresentationReadModel {
        task,
        steps,
        steps_overflow,
        artifacts,
        artifacts_overflow,
    }))
}

fn list_presentation_steps(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
    plan_revision: u64,
) -> Result<Vec<storage_core::StepRecord>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT s.step_id, s.task_id, s.plan_revision, s.logical_key, s.title, s.objective,
                s.dependencies_json, s.required_capabilities_json, s.acceptance_criteria_json,
                s.status, s.current_attempt_id, s.created_at, s.updated_at, s.version
         FROM steps s JOIN tasks t ON t.task_id = s.task_id
         WHERE t.workspace_id = ?1 AND s.task_id = ?2 AND s.plan_revision = ?3
         ORDER BY s.created_at ASC, s.step_id ASC LIMIT ?4",
    ).map_err(map_database_error)?;
    let rows = statement.query_map(
        params![
            workspace_id,
            task_id,
            to_sql_i64(plan_revision, "PlanRevision")?,
            to_sql_i64((MAX_TASK_PRESENTATION_STEPS + 1) as u64, "Task presentation Step limit")?,
        ],
        |row| {
            let parse_json = |index: usize| -> rusqlite::Result<serde_json::Value> {
                let text: String = row.get(index)?;
                serde_json::from_str(&text).map_err(|error| rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                ))
            };
            let dependencies: Vec<String> = serde_json::from_value(parse_json(6)?)
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(error)))?;
            let required_capabilities: Vec<serde_json::Value> = serde_json::from_value(parse_json(7)?)
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(error)))?;
            let acceptance_criteria: Vec<serde_json::Value> = serde_json::from_value(parse_json(8)?)
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(error)))?;
            Ok(storage_core::StepRecord {
                step_id: row.get(0)?,
                task_id: row.get(1)?,
                plan_revision: from_row_u64(row, 2)?,
                logical_key: row.get(3)?,
                title: row.get(4)?,
                objective: row.get(5)?,
                dependencies,
                required_capabilities,
                acceptance_criteria,
                status: row.get(9)?,
                current_attempt_id: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
                version: from_row_u64(row, 13)?,
            })
        },
    ).map_err(map_database_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_database_error)
}

impl ArtifactVersionWriteStore for SqliteWorkspaceStore {
    fn resolve_artifact_version_append_replay(
        &self,
        principal_id: &str,
        workspace_id: &str,
        request_id: &str,
        request_payload: &serde_json::Value,
    ) -> Result<Option<CommittedArtifactVersionAppend>, StoreError> {
        validate_nonempty(&[principal_id, workspace_id, request_id])?;
        let request_digest = digest(&canonical_json(request_payload)?);
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::ResolveArtifactAppendReplay {
            workspace_id: workspace_id.to_owned(), principal_id: principal_id.to_owned(),
            request_id: request_id.to_owned(), request_digest, reply,
        }, receive)
    }

    fn stage_text_content(
        &self,
        principal_id: &str,
        workspace_id: &str,
        artifact_id: &str,
        bytes: &[u8],
    ) -> Result<storage_core::BlobRef, StoreError> {
        validate_nonempty(&[principal_id, workspace_id, artifact_id])?;
        if bytes.len() as u64 > MAX_USER_TEXT_ARTIFACT_BYTES || std::str::from_utf8(bytes).is_err() {
            return Err(StoreError::Invalid("USER_TEXT_ARTIFACT_LIMIT_OR_ENCODING_INVALID".to_owned()));
        }
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::AuthorizeArtifactAppend {
            workspace_id: workspace_id.to_owned(), artifact_id: artifact_id.to_owned(),
            principal_id: principal_id.to_owned(), expected_artifact_version: None, expected_content_version: None,
            reply,
        }, receive)?;
        let blob = self.inner.blobs.put(workspace_id, BlobPurpose::Artifact, bytes, "text/plain")?;
        if blob.size_bytes != bytes.len() as u64 || blob.digest != digest(bytes)
            || blob.media_type != "text/plain"
            || self.inner.blobs.get(workspace_id, BlobPurpose::Artifact, &blob)? != bytes
        {
            return Err(StoreError::Integrity("staged Artifact text failed BlobStore verification".to_owned()));
        }
        Ok(blob)
    }

    fn append_artifact_version(&self, commit: ArtifactVersionAppendCommit) -> Result<CommittedArtifactVersionAppend, StoreError> {
        validate_append_commit_shape(&commit)?;
        let request_fingerprint = append_request_fingerprint(&commit)?;
        let expected_payload = append_request_payload(&commit)?;
        if canonical_json(&commit.request.request_payload)? != canonical_json(&expected_payload)? {
            return Err(StoreError::Invalid("Artifact append request payload does not match its content and pinned heads".to_owned()));
        }
        let (replay_reply, replay_receive) = mpsc::channel();
        if let Some(replayed) = self.execute_command(Command::ResolveArtifactAppendReplay {
            workspace_id: commit.workspace_id.clone(),
            principal_id: commit.request.principal_id.clone(),
            request_id: commit.request.request_id.clone(),
            request_digest: request_fingerprint.clone(),
            reply: replay_reply,
        }, replay_receive)? {
            return Ok(replayed);
        }
        let (authorization_reply, authorization_receive) = mpsc::channel();
        self.execute_command(Command::AuthorizeArtifactAppend {
            workspace_id: commit.workspace_id.clone(),
            artifact_id: commit.artifact_id.clone(),
            principal_id: commit.request.principal_id.clone(),
            expected_artifact_version: Some(commit.expected_artifact_version),
            expected_content_version: Some(commit.expected_content_version),
            reply: authorization_reply,
        }, authorization_receive)?;
        let ArtifactContentRecord::ManagedBlob { storage_ref, content_digest, media_type, size_bytes } = &commit.version.content else {
            return Err(StoreError::Invalid("EXTERNAL_ARTIFACT_PUBLICATION_UNSUPPORTED".to_owned()));
        };
        if media_type != "text/plain" || *size_bytes > MAX_USER_TEXT_ARTIFACT_BYTES {
            return Err(StoreError::Invalid("USER_TEXT_ARTIFACT_LIMIT_EXCEEDED".to_owned()));
        }

        // The local text editor is deliberately bounded so verifying the referenced
        // object cannot allocate an unbounded payload. BlobStore I/O occurs before the
        // SQLite writer transaction, as required by the storage contract.
        let bytes = self.inner.blobs.get(&commit.workspace_id, BlobPurpose::Artifact, storage_ref)?;
        if bytes.len() as u64 != *size_bytes || digest(&bytes) != *content_digest
            || storage_ref.digest != *content_digest || storage_ref.size_bytes != *size_bytes
            || storage_ref.media_type != *media_type || std::str::from_utf8(&bytes).is_err()
        {
            return Err(StoreError::Integrity("text Artifact blob does not match its committed metadata".to_owned()));
        }

        validate_append_provenance(&commit)?;
        validate_append_events(&commit)?;
        let resource_state = put_artifact_state(
            &self.inner.blobs,
            &commit.workspace_id,
            &commit.resource,
            "application/vnd.litecowork.resource+json",
            commit.resource.version,
        )?;
        let artifact_state = put_artifact_state(
            &self.inner.blobs,
            &commit.workspace_id,
            &ArtifactAggregateSnapshot { artifact: commit.artifact.clone(), current_version: commit.version.clone() },
            "application/vnd.litecowork.artifact+json",
            commit.artifact.version,
        )?;

        let workspace_id = commit.workspace_id.clone();
        let principal_id = commit.request.principal_id.clone();
        let request_id = commit.request.request_id.clone();
        let (reply, receive) = mpsc::channel();
        self.execute_command(
            Command::ArtifactOperation { operation: Box::new(move |connection| {
                let result = append_artifact_version_transaction(
                    connection,
                    commit,
                    &request_fingerprint,
                    resource_state,
                    artifact_state,
                );
                let _ = reply.send(result);
            }) },
            receive,
        ).map_err(|error| match error {
            StoreError::NotFound => StoreError::NotFound,
            other => other,
        })
    }
}

const MAX_USER_TEXT_ARTIFACT_BYTES: u64 = 1_048_576;

pub(super) fn authorize_artifact_append(
    connection: &Connection,
    workspace_id: &str,
    artifact_id: &str,
    principal_id: &str,
    expected_artifact_version: Option<u64>,
    expected_content_version: Option<u64>,
) -> Result<(), StoreError> {
    let workspace: Option<(String, String)> = connection.query_row(
        "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
        [workspace_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(map_database_error)?;
    let Some((owner, workspace_status)) = workspace else { return Err(StoreError::NotFound); };
    if owner != principal_id { return Err(StoreError::NotFound); }
    if workspace_status != "ACTIVE" { return Err(StoreError::NotFound); }
    let artifact = get_artifact(connection, workspace_id, artifact_id)?.ok_or(StoreError::NotFound)?;
    if artifact.library_status == "ARCHIVED" { return Err(StoreError::Invalid("ARTIFACT_ARCHIVED".to_owned())); }
    if expected_artifact_version.is_some_and(|expected| artifact.version != expected)
        || expected_content_version.is_some_and(|expected| artifact.current_version != expected)
    {
        return Err(StoreError::Conflict { expected: expected_artifact_version, actual: Some(artifact.version) });
    }
    Ok(())
}

pub(super) fn get_artifact_append_heads(
    connection: &Connection,
    workspace_id: &str,
    artifact_id: &str,
) -> Result<Option<storage_core::ArtifactAppendHeads>, StoreError> {
    let Some(artifact) = get_artifact(connection, workspace_id, artifact_id)? else { return Ok(None); };
    let resource = load_resource_record(connection, workspace_id, &artifact.resource_id)?.ok_or_else(|| StoreError::Integrity("Artifact backing Resource is missing".to_owned()))?;
    Ok(Some(storage_core::ArtifactAppendHeads { artifact, resource }))
}

pub(super) fn resolve_artifact_append_replay(
    connection: &Connection,
    workspace_id: &str,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
) -> Result<Option<CommittedArtifactVersionAppend>, StoreError> {
    let owner: Option<String> = connection.query_row(
        "SELECT owner_principal_id FROM workspaces WHERE workspace_id = ?1",
        [workspace_id],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?;
    if owner.as_deref() != Some(principal_id) { return Err(StoreError::NotFound); }
    let prior: Option<(String, Option<String>, Option<String>)> = connection.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![principal_id, request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((prior_digest, response_json, response_digest)) = prior else { return Ok(None); };
    if prior_digest != request_digest { return Err(StoreError::Conflict { expected: None, actual: None }); }
    let response_json = response_json.ok_or_else(|| StoreError::Integrity("Artifact append idempotency receipt is incomplete".to_owned()))?;
    if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
        return Err(StoreError::Integrity("Artifact append idempotency receipt digest is invalid".to_owned()));
    }
    let mut response: CommittedArtifactVersionAppend = serde_json::from_str(&response_json)
        .map_err(|error| StoreError::Integrity(format!("Artifact append receipt is invalid: {error}")))?;
    response.replayed = true;
    Ok(Some(response))
}

#[derive(Serialize)]
struct ArtifactAggregateSnapshot {
    artifact: ArtifactRecord,
    current_version: ArtifactVersionRecord,
}

fn put_artifact_state<T: Serialize>(
    blobs: &Arc<dyn BlobStore>,
    workspace_id: &str,
    state: &T,
    media_type: &str,
    entity_revision: u64,
) -> Result<storage_core::AggregateStateRef, StoreError> {
    let bytes = canonical_json(state)?;
    let blob = blobs.put(workspace_id, BlobPurpose::AggregateState, &bytes, media_type)?;
    if blob.digest != digest(&bytes) || blob.size_bytes != bytes.len() as u64
        || blobs.get(workspace_id, BlobPurpose::AggregateState, &blob)? != bytes
    {
        return Err(StoreError::Integrity("Artifact aggregate-state blob failed verification".to_owned()));
    }
    Ok(storage_core::AggregateStateRef { blob, entity_revision, record_schema_version: 1 })
}

fn validate_append_commit_shape(commit: &ArtifactVersionAppendCommit) -> Result<(), StoreError> {
    for value in [
        commit.workspace_id.as_str(), commit.artifact_id.as_str(),
        commit.expected_parent_resource_revision_id.as_str(),
        commit.request.principal_id.as_str(), commit.request.request_id.as_str(),
    ] {
        if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(StoreError::Invalid("Artifact append identity is invalid".to_owned()));
        }
    }
    if commit.expected_artifact_version == 0 || commit.expected_content_version == 0 || commit.expected_resource_version == 0
        || commit.artifact.artifact_id != commit.artifact_id || commit.artifact.workspace_id != commit.workspace_id
        || commit.version.artifact_id != commit.artifact_id
        || commit.resource.workspace_id != commit.workspace_id
        || commit.resource.resource_id != commit.artifact.resource_id
        || commit.resource_revision.resource_id != commit.resource.resource_id
        || commit.version.resource_revision_id != commit.resource_revision.resource_revision_id
        || commit.version.version != commit.expected_content_version.checked_add(1).unwrap_or(0)
        || commit.artifact.current_version != commit.version.version
        || commit.artifact.version != commit.expected_artifact_version.checked_add(1).unwrap_or(0)
        || commit.resource.version != commit.expected_resource_version.checked_add(1).unwrap_or(0)
        || commit.resource.current_revision_id.as_deref() != Some(commit.resource_revision.resource_revision_id.as_str())
        || commit.resource_revision.parent_revision_ids != [commit.expected_parent_resource_revision_id.clone()]
        || commit.resource.updated_at != commit.resource_revision.observed_at
        || commit.version.created_at != commit.resource_revision.observed_at
        || commit.resource_revision.created_by.get("principal_id").and_then(Value::as_str) != Some(commit.request.principal_id.as_str())
        || commit.resource_revision.created_by.get("kind").and_then(Value::as_str) != Some("USER")
    {
        return Err(StoreError::Invalid("Artifact append aggregate identities or expected heads are inconsistent".to_owned()));
    }
    let ArtifactContentRecord::ManagedBlob { content_digest, media_type, size_bytes, .. } = &commit.version.content else {
        return Err(StoreError::Invalid("external Artifact append is unsupported".to_owned()));
    };
    if commit.resource_revision.content_digest.as_deref() != Some(content_digest.as_str())
        || commit.resource_revision.size_bytes != Some(*size_bytes)
        || commit.resource_revision.media_type.as_deref() != Some(media_type.as_str())
        || commit.resource_revision.provider_revision.is_some()
    {
        return Err(StoreError::Invalid("Artifact ResourceRevision metadata does not match the published blob".to_owned()));
    }
    for timestamp in [&commit.resource_revision.observed_at, &commit.version.created_at, &commit.resource.updated_at,
        &commit.resource_event.recorded_at, &commit.artifact_event.recorded_at]
    {
        if canonicalize_utc_timestamp(timestamp)? != timestamp.as_str() {
            return Err(StoreError::Invalid("Artifact append timestamps must be canonical UTC values".to_owned()));
        }
    }
    to_sql_i64(commit.expected_artifact_version, "Artifact version")?;
    to_sql_i64(commit.expected_content_version, "Artifact content version")?;
    to_sql_i64(commit.expected_resource_version, "Resource version")?;
    Ok(())
}

fn append_request_payload(commit: &ArtifactVersionAppendCommit) -> Result<serde_json::Value, StoreError> {
    let ArtifactContentRecord::ManagedBlob { content_digest, media_type, size_bytes, .. } = &commit.version.content else {
        return Err(StoreError::Invalid("external Artifact append is unsupported".to_owned()));
    };
    Ok(json!({
        "workspace_id": commit.workspace_id,
        "artifact_id": commit.artifact_id,
        "expected_artifact_version": commit.expected_artifact_version,
        "expected_content_version": commit.expected_content_version,
        "expected_resource_version": commit.expected_resource_version,
        "expected_parent_resource_revision_id": commit.expected_parent_resource_revision_id,
        "resource_revision_id": commit.resource_revision.resource_revision_id,
        "content_digest": content_digest,
        "content_media_type": media_type,
        "content_size_bytes": size_bytes,
        "input_refs": commit.version.input_refs,
        "provenance": commit.version.provenance,
        "verification_refs": commit.version.verification_refs,
        "created_by_attempt": commit.version.created_by_attempt,
        "authored_by": commit.resource_revision.created_by,
    }))
}

fn append_request_fingerprint(commit: &ArtifactVersionAppendCommit) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&append_request_payload(commit)?)?))
}

fn validate_append_provenance(commit: &ArtifactVersionAppendCommit) -> Result<(), StoreError> {
    let provenance = commit.version.provenance.as_object()
        .ok_or_else(|| StoreError::Invalid("Artifact provenance must be an object".to_owned()))?;
    let mut inputs = Vec::<serde_json::Value>::new();
    let mut collect = |value: &serde_json::Value| -> Result<(), StoreError> {
        let list = value.as_array().ok_or_else(|| StoreError::Invalid("provenance inputs must be arrays".to_owned()))?;
        for input in list {
            let reference = input.get("resource_ref").ok_or_else(|| StoreError::Invalid("Artifact ResourceInput is missing resource_ref".to_owned()))?;
            let workspace = reference.get("workspace_id").and_then(serde_json::Value::as_str);
            let resource = reference.get("resource_id").and_then(serde_json::Value::as_str);
            let revision = reference.get("revision_id").and_then(serde_json::Value::as_str);
            if workspace != Some(commit.workspace_id.as_str()) || resource.map_or(true, str::is_empty) || revision.map_or(true, str::is_empty)
                || resource == Some(commit.resource.resource_id.as_str())
            {
                return Err(StoreError::Invalid("Artifact input reference is invalid or self-referential".to_owned()));
            }
            inputs.push(reference.clone());
        }
        Ok(())
    };
    collect(provenance.get("source_inputs").ok_or_else(|| StoreError::Invalid("Artifact provenance source_inputs are required".to_owned()))?)?;
    let transformations = provenance.get("transformations").and_then(serde_json::Value::as_array)
        .ok_or_else(|| StoreError::Invalid("Artifact provenance transformations are required".to_owned()))?;
    for transformation in transformations {
        collect(transformation.get("inputs").ok_or_else(|| StoreError::Invalid("transformation inputs are required".to_owned()))?)?;
    }
    let mut input_keys = inputs.into_iter().map(|value| Ok((canonical_json(&value)?, value)))
        .collect::<Result<Vec<_>, StoreError>>()?;
    input_keys.sort_by(|left, right| left.0.cmp(&right.0));
    input_keys.dedup_by(|left, right| left.0 == right.0);
    let mut declared_keys = commit.version.input_refs.iter().map(canonical_json)
        .collect::<Result<Vec<_>, _>>()?;
    declared_keys.sort();
    if declared_keys != input_keys.into_iter().map(|(key, _)| key).collect::<Vec<_>>() {
        return Err(StoreError::Invalid("Artifact input_refs do not match provenance inputs".to_owned()));
    }
    Ok(())
}

fn validate_append_events(commit: &ArtifactVersionAppendCommit) -> Result<(), StoreError> {
    let resource = &commit.resource_event;
    let artifact = &commit.artifact_event;
    let mut expected_resource_payload = json!({
        "resource_id": commit.resource.resource_id,
        "resource_revision_id": commit.resource_revision.resource_revision_id,
        "parent_revision_ids": commit.resource_revision.parent_revision_ids,
        "observed_at": commit.resource_revision.observed_at,
    });
    if let Some(value) = &commit.resource_revision.provider_revision { expected_resource_payload["provider_revision"] = json!(value); }
    if let Some(value) = &commit.resource_revision.content_digest { expected_resource_payload["content_digest"] = json!(value); }
    let ArtifactContentRecord::ManagedBlob { storage_ref, content_digest, .. } = &commit.version.content else {
        return Err(StoreError::Invalid("external Artifact append is unsupported".to_owned()));
    };
    let mut expected_artifact_payload = json!({
        "artifact_id": commit.artifact.artifact_id,
        "version": commit.version.version,
        "resource_id": commit.artifact.resource_id,
        "resource_revision_id": commit.version.resource_revision_id,
        "input_refs": commit.version.input_refs,
        "content_kind": "MANAGED_BLOB",
        "content_digest": content_digest,
        "storage_ref": storage_ref,
        "aggregate_version": commit.artifact.version,
    });
    if let Some(value) = &commit.version.created_by_attempt { expected_artifact_payload["created_by_attempt"] = json!(value); }
    if resource.workspace_id != commit.workspace_id || resource.entity_type != "Resource"
        || resource.entity_id != commit.resource.resource_id || resource.event_type != "resource.revision.observed.v1"
        || resource.entity_revision != commit.resource.version
        || artifact.workspace_id != commit.workspace_id || artifact.entity_type != "Artifact"
        || artifact.entity_id != commit.artifact.artifact_id || artifact.event_type != "artifact.version.created.v1"
        || artifact.entity_revision != commit.artifact.version || resource.schema_version != 1 || artifact.schema_version != 1
        || resource.event_id == artifact.event_id || resource.origin_runtime_id != artifact.origin_runtime_id
        || resource.correlation_id != artifact.correlation_id || resource.recorded_at != artifact.recorded_at
        || resource.recorded_at != commit.version.created_at
        || canonical_json(&resource.payload)? != canonical_json(&expected_resource_payload)?
        || canonical_json(&artifact.payload)? != canonical_json(&expected_artifact_payload)?
    {
        return Err(StoreError::Invalid("Artifact append events do not match their immutable records".to_owned()));
    }
    Ok(())
}

fn append_artifact_version_transaction(
    connection: &mut Connection,
    commit: ArtifactVersionAppendCommit,
    request_digest: &str,
    resource_state: storage_core::AggregateStateRef,
    artifact_state: storage_core::AggregateStateRef,
) -> Result<CommittedArtifactVersionAppend, StoreError> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
    let workspace: Option<(String, String)> = tx.query_row(
        "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
        [&commit.workspace_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(map_database_error)?;
    let Some((owner, workspace_status)) = workspace else { return Err(StoreError::NotFound); };
    if owner != commit.request.principal_id { return Err(StoreError::NotFound); }

    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![commit.request.principal_id, commit.request.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest { return Err(StoreError::Conflict { expected: None, actual: None }); }
        let response_json = response_json.ok_or_else(|| StoreError::Integrity("Artifact append idempotency receipt is incomplete".to_owned()))?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity("Artifact append idempotency receipt digest is invalid".to_owned()));
        }
        let mut response: CommittedArtifactVersionAppend = serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(format!("Artifact append receipt is invalid: {error}")))?;
        response.replayed = true;
        return Ok(response);
    }
    if workspace_status != "ACTIVE" { return Err(StoreError::NotFound); }

    let current_artifact = get_artifact(&tx, &commit.workspace_id, &commit.artifact_id)?.ok_or(StoreError::NotFound)?;
    if current_artifact.library_status == "ARCHIVED" {
        return Err(StoreError::Invalid("ARCHIVED_ARTIFACT_IMMUTABLE".to_owned()));
    }
    if current_artifact.version != commit.expected_artifact_version
        || current_artifact.current_version != commit.expected_content_version
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_artifact_version),
            actual: Some(current_artifact.version),
        });
    }
    if current_artifact.resource_id != commit.resource.resource_id
        || current_artifact.workspace_id != commit.workspace_id
        || current_artifact.kind != commit.artifact.kind
        || current_artifact.display_name != commit.artifact.display_name
        || current_artifact.task_id != commit.artifact.task_id
        || current_artifact.created_at != commit.artifact.created_at
        || current_artifact.library_status != commit.artifact.library_status
    {
        return Err(StoreError::Conflict { expected: Some(commit.expected_artifact_version), actual: Some(current_artifact.version) });
    }
    let current_resource = load_resource_record(&tx, &commit.workspace_id, &commit.resource.resource_id)?.ok_or(StoreError::NotFound)?;
    if current_resource.version != commit.expected_resource_version
        || current_resource.current_revision_id.as_deref() != Some(commit.expected_parent_resource_revision_id.as_str())
    {
        return Err(StoreError::Conflict { expected: Some(commit.expected_resource_version), actual: Some(current_resource.version) });
    }
    if current_resource.kind != "ARTIFACT"
        || current_resource.provider_identity != commit.resource.provider_identity
        || current_resource.identity_digest != commit.resource.identity_digest
        || current_resource.display_name != commit.resource.display_name
        || current_resource.sensitivity != commit.resource.sensitivity
        || current_resource.provenance != commit.resource.provenance
        || current_resource.created_at != commit.resource.created_at
    {
        return Err(StoreError::Integrity("Artifact Resource immutable metadata changed".to_owned()));
    }
    let current_parent_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_revisions WHERE resource_id = ?1 AND resource_revision_id = ?2)",
        params![commit.resource.resource_id, commit.expected_parent_resource_revision_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !current_parent_exists { return Err(StoreError::Integrity("Artifact Resource head revision is missing".to_owned())); }

    validate_source_inputs(&tx, &commit)?;
    if let Some(attempt_id) = &commit.version.created_by_attempt {
        let attempt_task: Option<(String, String)> = tx.query_row(
            "SELECT a.task_id, t.workspace_id FROM attempts a JOIN tasks t ON t.task_id = a.task_id WHERE a.attempt_id = ?1",
            [attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional().map_err(map_database_error)?;
        let Some((attempt_task_id, attempt_workspace_id)) = attempt_task else { return Err(StoreError::NotFound); };
        if attempt_workspace_id != commit.workspace_id
            || commit.artifact.task_id.as_deref().is_some_and(|artifact_task_id| artifact_task_id != attempt_task_id)
        {
            return Err(StoreError::Invalid("Artifact creating Attempt is outside its Workspace or Task".to_owned()));
        }
    }
    let new_revision_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_revisions WHERE resource_revision_id = ?1)",
        [&commit.resource_revision.resource_revision_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if new_revision_exists { return Err(StoreError::Conflict { expected: None, actual: None }); }

    let ArtifactContentRecord::ManagedBlob { storage_ref, content_digest, media_type, size_bytes } = &commit.version.content else {
        return Err(StoreError::Invalid("EXTERNAL_ARTIFACT_PUBLICATION_UNSUPPORTED".to_owned()));
    };
    let created_by_json = String::from_utf8(canonical_json(&commit.resource_revision.created_by)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, content_digest, size_bytes, media_type, observed_at, created_by_json) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)",
        params![commit.resource_revision.resource_revision_id, commit.resource.resource_id, content_digest,
            to_sql_i64(*size_bytes, "Artifact size")?, media_type, commit.resource_revision.observed_at, created_by_json],
    ).map_err(map_database_error)?;
    for parent_revision_id in &commit.resource_revision.parent_revision_ids {
        tx.execute(
            "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id) VALUES (?1, ?2, ?3)",
            params![commit.resource.resource_id, commit.resource_revision.resource_revision_id, parent_revision_id],
        ).map_err(map_database_error)?;
    }

    let updated_resource = tx.execute(
        "UPDATE resources SET current_revision_id = ?1, updated_at = ?2, version = ?3 WHERE workspace_id = ?4 AND resource_id = ?5 AND version = ?6 AND current_revision_id = ?7",
        params![commit.resource_revision.resource_revision_id, commit.resource.updated_at,
            to_sql_i64(commit.resource.version, "Resource version")?, commit.workspace_id,
            commit.resource.resource_id, to_sql_i64(commit.expected_resource_version, "Resource version")?,
            commit.expected_parent_resource_revision_id],
    ).map_err(map_database_error)?;
    if updated_resource != 1 { return Err(StoreError::Conflict { expected: Some(commit.expected_resource_version), actual: None }); }

    let (content_kind, storage_ref_json, resource_ref_json) = match &commit.version.content {
        ArtifactContentRecord::ManagedBlob { storage_ref, .. } => (
            "MANAGED_BLOB",
            Some(String::from_utf8(canonical_json(storage_ref)?).map_err(|error| StoreError::Invalid(error.to_string()))?),
            None,
        ),
        ArtifactContentRecord::ExternalResource { .. } => unreachable!("external publication rejected before transaction"),
    };
    tx.execute(
        "INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, created_by_attempt, input_refs_json, content_kind, content_digest, storage_ref_json, resource_ref_json, provider_revision, observed_digest, content_media_type, content_size_bytes, content_observed_at, provenance_json, verification_refs_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL, ?11, ?12, NULL, ?13, ?14, ?15)",
        params![commit.artifact_id, commit.resource.resource_id, to_sql_i64(commit.version.version, "Artifact content version")?,
            commit.version.resource_revision_id, commit.version.created_by_attempt,
            String::from_utf8(canonical_json(&commit.version.input_refs)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
            content_kind, content_digest, storage_ref_json, resource_ref_json, media_type,
            to_sql_i64(*size_bytes, "Artifact size")?,
            String::from_utf8(canonical_json(&commit.version.provenance)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
            String::from_utf8(canonical_json(&commit.version.verification_refs)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
            commit.version.created_at],
    ).map_err(map_database_error)?;

    let dependent_ref = format!("artifact://{}/{}@v{}", commit.workspace_id, commit.artifact_id, commit.version.version);
    for input in &commit.version.input_refs {
        let source_workspace = input.get("workspace_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact input reference has no workspace_id".to_owned()))?;
        let source_resource = input.get("resource_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact input reference has no resource_id".to_owned()))?;
        let source_revision = input.get("revision_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact input reference has no revision_id".to_owned()))?;
        let identity = json!([source_resource, source_revision, "ARTIFACT_VERSION", dependent_ref]);
        let edge_id = format!("dependency-{}", digest(&canonical_json(&identity)?).trim_start_matches("sha256:"));
        tx.execute(
            "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, artifact_id, artifact_version, verification_run_id, created_at) VALUES (?1, ?2, ?3, ?4, 'ARTIFACT_VERSION', ?5, ?6, ?7, NULL, ?8)",
            params![edge_id, source_workspace, source_resource, source_revision, dependent_ref,
                commit.artifact_id, to_sql_i64(commit.version.version, "Artifact content version")?, commit.version.created_at],
        ).map_err(map_database_error)?;
    }

    let invalidation_edges: Vec<(String, String)> = {
        let mut statement = tx.prepare("SELECT dependency_edge_id, source_revision_id FROM dependency_edges WHERE source_resource_id = ?1 AND source_revision_id <> ?2 ORDER BY dependency_edge_id")
            .map_err(map_database_error)?;
        let rows = statement.query_map(params![commit.resource.resource_id, commit.resource_revision.resource_revision_id],
            |row| Ok((row.get(0)?, row.get(1)?))).map_err(map_database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_database_error)?
    };
    for (dependency_edge_id, _source_revision_id) in invalidation_edges {
        let invalidation_id = format!("invalidation-{}", digest(&canonical_json(&json!([dependency_edge_id, commit.resource_revision.resource_revision_id]))?).trim_start_matches("sha256:"));
        tx.execute(
            "INSERT OR IGNORE INTO invalidation_records(invalidation_record_id, dependency_edge_id, observed_revision_id, reason_code, created_at) VALUES (?1, ?2, ?3, 'SOURCE_RESOURCE_REVISION_ADVANCED', ?4)",
            params![invalidation_id, dependency_edge_id, commit.resource_revision.resource_revision_id, commit.version.created_at],
        ).map_err(map_database_error)?;
    }

    let updated_artifact = tx.execute(
        "UPDATE artifacts SET current_version = ?1, version = ?2 WHERE workspace_id = ?3 AND artifact_id = ?4 AND current_version = ?5 AND version = ?6 AND library_status <> 'ARCHIVED'",
        params![to_sql_i64(commit.artifact.current_version, "Artifact content version")?,
            to_sql_i64(commit.artifact.version, "Artifact aggregate version")?, commit.workspace_id,
            commit.artifact_id, to_sql_i64(commit.expected_content_version, "Artifact content version")?,
            to_sql_i64(commit.expected_artifact_version, "Artifact aggregate version")?],
    ).map_err(map_database_error)?;
    if updated_artifact != 1 { return Err(StoreError::Conflict { expected: Some(commit.expected_artifact_version), actual: None }); }

    let resource_event = insert_domain_event(&tx, &commit.resource_event, &resource_state)?;
    let artifact_event = insert_domain_event(&tx, &commit.artifact_event, &artifact_state)?;
    let mut committed = CommittedArtifactVersionAppend {
        artifact: commit.artifact,
        version: commit.version,
        resource: commit.resource,
        resource_revision: commit.resource_revision,
        events: [resource_event, artifact_event],
        replayed: false,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request.principal_id, commit.request.request_id, request_digest,
            response_json, digest(response_json.as_bytes()), committed.version.created_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    // Keep the response's replay flag false on the first commit; retries decode the
    // exact receipt and set it true without re-running any append side effects.
    committed.replayed = false;
    Ok(committed)
}

fn validate_source_inputs(
    tx: &Transaction<'_>,
    commit: &ArtifactVersionAppendCommit,
) -> Result<(), StoreError> {
    for input in &commit.version.input_refs {
        let source_workspace = input.get("workspace_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact input reference has no workspace_id".to_owned()))?;
        let source_resource = input.get("resource_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact input reference has no resource_id".to_owned()))?;
        let source_revision = input.get("revision_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact input reference has no revision_id".to_owned()))?;
        if source_workspace != commit.workspace_id || source_resource == commit.resource.resource_id {
            return Err(StoreError::Invalid("Artifact input reference is outside the Workspace or self-referential".to_owned()));
        }
        let source_revision_row: Option<(Option<String>,)> = tx.query_row(
            "SELECT rr.content_digest FROM resources r JOIN resource_revisions rr ON rr.resource_id = r.resource_id WHERE r.workspace_id = ?1 AND r.resource_id = ?2 AND rr.resource_revision_id = ?3",
            params![source_workspace, source_resource, source_revision],
            |row| Ok((row.get(0)?,)),
        ).optional().map_err(map_database_error)?;
        let Some((source_digest,)) = source_revision_row else { return Err(StoreError::NotFound); };
    }
    let provenance = &commit.version.provenance;
    let source_inputs = provenance.get("source_inputs").and_then(Value::as_array)
        .ok_or_else(|| StoreError::Invalid("Artifact provenance source_inputs are required".to_owned()))?;
    let transformations = provenance.get("transformations").and_then(Value::as_array)
        .ok_or_else(|| StoreError::Invalid("Artifact provenance transformations are required".to_owned()))?;
    let mut provenance_inputs: Vec<&Value> = source_inputs.iter().collect();
    for transformation in transformations {
        let inputs = transformation.get("inputs").and_then(Value::as_array)
            .ok_or_else(|| StoreError::Invalid("Artifact transformation inputs are required".to_owned()))?;
        provenance_inputs.extend(inputs.iter());
    }
    for input in provenance_inputs {
        let resource_ref = input.get("resource_ref").ok_or_else(|| StoreError::Invalid("Artifact provenance input has no resource_ref".to_owned()))?;
        let source_workspace = resource_ref.get("workspace_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact provenance input has no workspace_id".to_owned()))?;
        let source_resource = resource_ref.get("resource_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact provenance input has no resource_id".to_owned()))?;
        let source_revision = resource_ref.get("revision_id").and_then(Value::as_str).ok_or_else(|| StoreError::Invalid("Artifact provenance input has no revision_id".to_owned()))?;
        if let Some(observed_digest) = input.get("observed_digest").and_then(Value::as_str) {
            let source_digest: Option<String> = tx.query_row(
                "SELECT rr.content_digest FROM resources r JOIN resource_revisions rr ON rr.resource_id = r.resource_id WHERE r.workspace_id = ?1 AND r.resource_id = ?2 AND rr.resource_revision_id = ?3",
                params![source_workspace, source_resource, source_revision],
                |row| row.get(0),
            ).optional().map_err(map_database_error)?;
            if source_digest.as_deref().is_some_and(|digest| digest != observed_digest) {
                return Err(StoreError::Integrity("Artifact provenance input digest does not match its pinned ResourceRevision".to_owned()));
            }
        }
    }
    Ok(())
}

const ARTIFACT_COLUMNS: &str = "artifact_id, workspace_id, resource_id, task_id, kind, display_name, current_version, library_status, created_at, version";

fn artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRecord> {
    Ok(ArtifactRecord {
        artifact_id: row.get(0)?, workspace_id: row.get(1)?, resource_id: row.get(2)?, task_id: row.get(3)?,
        kind: row.get(4)?, display_name: row.get(5)?, current_version: row.get(6)?, library_status: row.get(7)?, created_at: row.get(8)?, version: row.get(9)?,
    })
}

pub(super) fn get_artifact(connection: &Connection, workspace_id: &str, artifact_id: &str) -> Result<Option<ArtifactRecord>, StoreError> {
    connection.query_row(&format!("SELECT {ARTIFACT_COLUMNS} FROM artifacts WHERE workspace_id = ?1 AND artifact_id = ?2"), params![workspace_id, artifact_id], artifact_row)
        .optional().map_err(map_database_error)
}

pub(super) fn list_artifacts(connection: &Connection, workspace_id: &str, status: Option<&str>, task_id: Option<&str>, after_created_at: Option<&str>, after_id: Option<&str>, limit: usize) -> Result<Vec<ArtifactRecord>, StoreError> {
    let mut statement = connection.prepare(&format!("SELECT {ARTIFACT_COLUMNS} FROM artifacts WHERE workspace_id = ?1 AND (?2 IS NULL OR library_status = ?2) AND (?3 IS NULL OR task_id = ?3) AND (?4 IS NULL OR created_at < ?4 OR (created_at = ?4 AND artifact_id < ?5)) ORDER BY created_at DESC, artifact_id DESC LIMIT ?6")).map_err(map_database_error)?;
    let rows = statement.query_map(params![workspace_id, status, task_id, after_created_at, after_id, limit as i64], artifact_row).map_err(map_database_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_database_error)
}

fn json_column<T: serde::de::DeserializeOwned>(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<T> {
    let text: String = row.get(index)?;
    serde_json::from_str(&text).map_err(|error| rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error)))
}

pub(super) fn get_artifact_version(connection: &Connection, workspace_id: &str, artifact_id: &str, version: u64) -> Result<Option<ArtifactVersionRecord>, StoreError> {
    connection.query_row(
        "SELECT av.artifact_id, av.version, av.resource_revision_id, av.input_refs_json, av.content_kind, av.storage_ref_json, av.content_digest, av.content_media_type, av.content_size_bytes, av.resource_ref_json, av.provider_revision, av.observed_digest, av.content_observed_at, av.provenance_json, av.created_by_attempt, av.verification_refs_json, av.created_at FROM artifact_versions av JOIN artifacts a ON a.artifact_id = av.artifact_id AND a.resource_id = av.resource_id JOIN resource_revisions rr ON rr.resource_id = av.resource_id AND rr.resource_revision_id = av.resource_revision_id WHERE a.workspace_id = ?1 AND av.artifact_id = ?2 AND av.version = ?3",
        params![workspace_id, artifact_id, to_sql_i64(version, "Artifact version")?],
        |row| {
            let kind: String = row.get(4)?;
            let content = match kind.as_str() {
                "MANAGED_BLOB" => ArtifactContentRecord::ManagedBlob { storage_ref: json_column(row, 5)?, content_digest: row.get(6)?, media_type: row.get(7)?, size_bytes: row.get(8)? },
                "EXTERNAL_RESOURCE" => ArtifactContentRecord::ExternalResource { resource_ref: json_column(row, 9)?, provider_revision: row.get(10)?, observed_digest: row.get(11)?, observed_at: row.get(12)? },
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(ArtifactVersionRecord { artifact_id: row.get(0)?, version: row.get(1)?, resource_revision_id: row.get(2)?, input_refs: json_column(row, 3)?, content, provenance: json_column(row, 13)?, created_by_attempt: row.get(14)?, verification_refs: json_column(row, 15)?, created_at: row.get(16)? })
        },
    ).optional().map_err(map_database_error)
}
