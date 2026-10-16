use super::*;
use std::collections::BTreeMap;
use storage_core::{
    AcknowledgeContextDocumentPurgeTargetCommand, CommittedContextDocumentPurge,
    ContextDocumentDeletionStatus, ContextDocumentPurgeReplicaKind,
    ContextDocumentPurgeStartCommand, ContextDocumentPurgeStatus, ContextDocumentPurgeStore,
    ContextDocumentPurgeTarget, ContextDocumentPurgeUploadChunkRef,
    ContextDocumentStatusEventContext, PendingContextDocumentPurgeTarget,
    PendingEmptyContextDocumentPurge,
};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

type PurgeTargetGroupKey = (String, &'static str);
type PurgeTargetRevisionAndChunkRefs = (Vec<String>, Vec<ContextDocumentPurgeUploadChunkRef>);
type PurgeReceiptIdentityRow = (String, Option<String>, Option<String>, String, String);

const PURGE_RECONCILER_SERVICE_ID: &str = "context_document_purge_reconciler";
const RESOURCE_AGGREGATE_MEDIA_TYPE: &str = "application/vnd.litecowork.resource+json";

impl ContextDocumentPurgeStore for SqliteWorkspaceStore {
    fn begin_context_document_purge(
        &self,
        mut command: ContextDocumentPurgeStartCommand,
    ) -> Result<CommittedContextDocumentPurge, StoreError> {
        validate_nonempty(&[
            &command.workspace_id,
            &command.resource_id,
            &command.principal_id,
            &command.request_id,
        ])?;
        if command.workspace_id.contains('\0')
            || command.resource_id.contains('\0')
            || command.principal_id.contains('\0')
            || !valid_shared_idempotency_key(&command.request_id)
            || command.expected_version == 0
        {
            return Err(StoreError::Invalid(
                "ContextDocument purge command identity is invalid".to_owned(),
            ));
        }
        command.event = normalize_event_context(command.event)?;
        let expected_payload = json!({
            "workspace_id": &command.workspace_id,
            "resource_id": &command.resource_id,
            "status": "DELETION_PENDING",
            "expected_version": command.expected_version,
        });
        if command.request_payload != expected_payload {
            return Err(StoreError::Invalid(
                "ContextDocument purge request payload is inconsistent".to_owned(),
            ));
        }
        let request_digest = digest(&canonical_json(&command.request_payload)?);

        // ResourceIndex writers and its collector use this workspace lock. Exact
        // Resource content references are locked separately before entering SQLite so
        // the collector can finish an already-claimed deletion without a lock cycle.
        let index_lock = self.resource_index_write_lock(&command.workspace_id);
        let _index_guard = self.lock_resource_index_write(&command.workspace_id, &index_lock);
        let content_digests = self.resource_content_digests(&command.resource_id)?;
        let content_locks = content_digests
            .iter()
            .map(|digest| self.resource_content_blob_operation_lock(&command.workspace_id, digest))
            .collect::<Vec<_>>();
        let _content_guards = content_locks
            .iter()
            .map(|lock| {
                lock.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            })
            .collect::<Vec<_>>();
        let upload_digests = self.resource_upload_chunk_digests(&command.resource_id)?;
        let upload_locks = upload_digests
            .iter()
            .map(|digest| self.blob_operation_lock(&command.workspace_id, digest))
            .collect::<Vec<_>>();
        let _upload_guards = upload_locks
            .iter()
            .map(|lock| {
                lock.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            })
            .collect::<Vec<_>>();

        let blobs = Arc::clone(&self.inner.blobs);
        let preflight_command = command.clone();
        let preflight_digest = request_digest.clone();
        let inspection = run_operation(self, move |connection| {
            inspect_context_document_purge_start(connection, &preflight_command, &preflight_digest)
        })?;
        let candidate = match inspection {
            PurgeStartInspection::Replay(replayed) => return Ok(replayed),
            PurgeStartInspection::Candidate(candidate) => candidate,
        };
        if unregistered_resource_revision_blob_present(
            blobs.as_ref(),
            &command.workspace_id,
            &candidate.unregistered_revision_blobs,
        )? {
            return Err(StoreError::Integrity(
                "unregistered Resource revision content remains in the local BlobStore".to_owned(),
            ));
        }
        let resource_state_ref =
            write_resource_snapshot(blobs.as_ref(), &command.workspace_id, &candidate.resource)?;
        let mut upload_state_refs = Vec::with_capacity(candidate.uploads.len());
        for upload in &candidate.uploads {
            upload_state_refs.push(write_resource_upload_snapshot(
                blobs.as_ref(),
                &command.workspace_id,
                &upload.expired,
            )?);
        }
        let prepared = PreparedContextDocumentPurgeStart {
            candidate,
            resource_state_ref,
            upload_state_refs,
        };
        run_operation(self, move |connection| {
            commit_context_document_purge_start(connection, command, &request_digest, prepared)
        })
    }

    fn get_context_document_deletion_status(
        &self,
        workspace_id: &str,
        resource_id: &str,
        principal_id: &str,
    ) -> Result<Option<ContextDocumentDeletionStatus>, StoreError> {
        validate_nonempty(&[workspace_id, resource_id, principal_id])?;
        if workspace_id.contains('\0') || resource_id.contains('\0') || principal_id.contains('\0')
        {
            return Err(StoreError::Invalid(
                "ContextDocument deletion status identity is invalid".to_owned(),
            ));
        }
        let workspace_id = workspace_id.to_owned();
        let resource_id = resource_id.to_owned();
        let principal_id = principal_id.to_owned();
        run_operation(self, move |connection| {
            context_document_deletion_status(connection, &workspace_id, &resource_id, &principal_id)
        })
    }

    fn list_pending_context_document_purge_targets(
        &self,
        limit: usize,
    ) -> Result<Vec<PendingContextDocumentPurgeTarget>, StoreError> {
        validate_limit(limit)?;
        run_operation(self, move |connection| {
            list_pending_targets(connection, limit)
        })
    }

    fn list_pending_empty_context_document_purge_plans(
        &self,
        limit: usize,
    ) -> Result<Vec<PendingEmptyContextDocumentPurge>, StoreError> {
        validate_limit(limit)?;
        run_operation(self, move |connection| {
            list_pending_empty_plans(connection, limit)
        })
    }

    fn acknowledge_context_document_purge_target_if_object_physically_removed(
        &self,
        mut command: AcknowledgeContextDocumentPurgeTargetCommand,
    ) -> Result<bool, StoreError> {
        validate_nonempty(&[
            &command.workspace_id,
            &command.resource_id,
            &command.manifest_digest,
        ])?;
        if command.workspace_id.contains('\0')
            || command.resource_id.contains('\0')
            || !storage_core::is_sha256_digest(&command.manifest_digest)
        {
            return Err(StoreError::Invalid(
                "ContextDocument purge acknowledgement identity is invalid".to_owned(),
            ));
        }
        validate_local_target(&command.target)?;
        command.acknowledged_at = canonicalize_utc_timestamp(&command.acknowledged_at)?;
        command.event = normalize_event_context(command.event)?;
        command.completion_event = normalize_event_context(command.completion_event)?;

        let index_lock = self.resource_index_write_lock(&command.workspace_id);
        let _index_guard = self.lock_resource_index_write(&command.workspace_id, &index_lock);
        let content_lock = (command.target.replica_kind == ContextDocumentPurgeReplicaKind::Blob)
            .then(|| {
                self.resource_content_blob_operation_lock(
                    &command.workspace_id,
                    &command.target.replica_ref,
                )
            });
        let upload_lock = (command.target.replica_kind
            == ContextDocumentPurgeReplicaKind::UploadChunk)
            .then(|| self.blob_operation_lock(&command.workspace_id, &command.target.replica_ref));
        let _content_guard = content_lock.as_ref().map(|lock| {
            lock.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        let _upload_guard = upload_lock.as_ref().map(|lock| {
            lock.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        let blobs = Arc::clone(&self.inner.blobs);
        let preflight_command = command.clone();
        let inspection = run_operation(self, move |connection| {
            inspect_purge_target_acknowledgement(connection, &preflight_command)
        })?;
        let candidate = match inspection {
            PurgeAcknowledgementInspection::Result(result) => return Ok(result),
            PurgeAcknowledgementInspection::Candidate(candidate) => *candidate,
        };
        let state_ref =
            write_resource_snapshot(blobs.as_ref(), &command.workspace_id, &candidate.resource)?;
        let prepared = PreparedPurgeAcknowledgement {
            candidate,
            state_ref,
        };
        run_operation(self, move |connection| {
            commit_purge_target_acknowledgement(connection, command, prepared)
        })
    }

    fn complete_empty_context_document_purge_plan_if_verified_empty(
        &self,
        workspace_id: &str,
        resource_id: &str,
        manifest_digest: &str,
        event: ContextDocumentStatusEventContext,
    ) -> Result<bool, StoreError> {
        validate_nonempty(&[workspace_id, resource_id, manifest_digest])?;
        if workspace_id.contains('\0')
            || resource_id.contains('\0')
            || !storage_core::is_sha256_digest(manifest_digest)
        {
            return Err(StoreError::Invalid(
                "empty ContextDocument purge identity is invalid".to_owned(),
            ));
        }
        let event = normalize_event_context(event)?;
        let workspace_id = workspace_id.to_owned();
        let resource_id = resource_id.to_owned();
        let manifest_digest = manifest_digest.to_owned();

        let index_lock = self.resource_index_write_lock(&workspace_id);
        let _index_guard = self.lock_resource_index_write(&workspace_id, &index_lock);
        let content_digests = self.resource_content_digests(&resource_id)?;
        let content_locks = content_digests
            .iter()
            .map(|digest| self.resource_content_blob_operation_lock(&workspace_id, digest))
            .collect::<Vec<_>>();
        let _content_guards = content_locks
            .iter()
            .map(|lock| {
                lock.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            })
            .collect::<Vec<_>>();
        let upload_digests = self.resource_upload_chunk_digests(&resource_id)?;
        let upload_locks = upload_digests
            .iter()
            .map(|digest| self.blob_operation_lock(&workspace_id, digest))
            .collect::<Vec<_>>();
        let _upload_guards = upload_locks
            .iter()
            .map(|lock| {
                lock.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            })
            .collect::<Vec<_>>();

        let blobs = Arc::clone(&self.inner.blobs);
        let preflight_workspace_id = workspace_id.clone();
        let preflight_resource_id = resource_id.clone();
        let preflight_manifest_digest = manifest_digest.clone();
        let preflight_event = event.clone();
        let inspection = run_operation(self, move |connection| {
            inspect_empty_purge_completion(
                connection,
                &preflight_workspace_id,
                &preflight_resource_id,
                &preflight_manifest_digest,
                &preflight_event,
            )
        })?;
        let candidate = match inspection {
            EmptyPurgeCompletionInspection::Result(result) => return Ok(result),
            EmptyPurgeCompletionInspection::Candidate(candidate) => *candidate,
        };
        if unregistered_resource_revision_blob_present(
            blobs.as_ref(),
            &workspace_id,
            &candidate.unregistered_revision_blobs,
        )? {
            return Ok(false);
        }
        let state_ref =
            write_resource_snapshot(blobs.as_ref(), &workspace_id, &candidate.resource)?;
        let prepared = PreparedEmptyPurgeCompletion {
            candidate,
            state_ref,
        };
        run_operation(self, move |connection| {
            commit_empty_purge_completion(
                connection,
                &workspace_id,
                &resource_id,
                &manifest_digest,
                event,
                prepared,
            )
        })
    }
}

impl SqliteWorkspaceStore {
    fn resource_content_digests(&self, resource_id: &str) -> Result<Vec<String>, StoreError> {
        let resource_id = resource_id.to_owned();
        run_operation(self, move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT DISTINCT content_digest FROM resource_revisions
                     WHERE resource_id = ?1 AND content_digest IS NOT NULL
                     ORDER BY content_digest",
                )
                .map_err(map_database_error)?;
            statement
                .query_map([resource_id], |row| row.get::<_, String>(0))
                .map_err(map_database_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_database_error)
        })
    }

    fn resource_upload_chunk_digests(&self, resource_id: &str) -> Result<Vec<String>, StoreError> {
        let resource_id = resource_id.to_owned();
        run_operation(self, move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT DISTINCT refs.digest
                     FROM (
                       SELECT chunk.temporary_blob_ref AS digest, chunk.upload_id
                       FROM resource_upload_chunks chunk
                       UNION
                       SELECT reservation.digest, reservation.upload_id
                       FROM resource_upload_blob_reservations reservation
                     ) refs
                     JOIN resource_upload_sessions session ON session.upload_id = refs.upload_id
                     WHERE session.resource_id = ?1 OR session.committed_resource_id = ?1
                     ORDER BY refs.digest",
                )
                .map_err(map_database_error)?;
            statement
                .query_map([resource_id], |row| row.get::<_, String>(0))
                .map_err(map_database_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_database_error)
        })
    }
}

fn run_operation<T, F>(store: &SqliteWorkspaceStore, operation: F) -> Result<T, StoreError>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
{
    let (reply, receive) = mpsc::channel();
    store.execute_command(
        Command::ContextDocumentPurgeOperation {
            operation: Box::new(move |connection| {
                let _ = reply.send(operation(connection));
            }),
        },
        receive,
    )
}

fn validate_limit(limit: usize) -> Result<(), StoreError> {
    if !(1..=100).contains(&limit) {
        return Err(StoreError::Invalid(
            "ContextDocument purge page limit must be between 1 and 100".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_event_context(
    mut event: ContextDocumentStatusEventContext,
) -> Result<ContextDocumentStatusEventContext, StoreError> {
    validate_nonempty(&[
        &event.event_id,
        &event.origin_runtime_id,
        &event.hlc_timestamp,
        &event.correlation_id,
        &event.recorded_at,
    ])?;
    if [
        event.event_id.as_str(),
        event.origin_runtime_id.as_str(),
        event.hlc_timestamp.as_str(),
        event.correlation_id.as_str(),
    ]
    .iter()
    .any(|value| value.contains('\0'))
        || event
            .causation_id
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.contains('\0'))
    {
        return Err(StoreError::Invalid(
            "ContextDocument purge event identity is invalid".to_owned(),
        ));
    }
    event.recorded_at = canonicalize_utc_timestamp(&event.recorded_at)?;
    Ok(event)
}

#[derive(Clone, Debug, PartialEq)]
struct PreparedUploadExpiration {
    current: storage_core::ResourceUploadSessionRecord,
    expired: storage_core::ResourceUploadSessionRecord,
}

#[derive(Clone, Debug, PartialEq)]
struct ContextDocumentPurgeStartCandidate {
    current_resource_version: u64,
    current_status: String,
    resource: storage_core::ResourceDetailRecord,
    targets: Vec<ContextDocumentPurgeTarget>,
    manifest_digest: String,
    target_count: u32,
    uploads: Vec<PreparedUploadExpiration>,
    unregistered_revision_blobs: Vec<BlobRef>,
}

enum PurgeStartInspection {
    Replay(CommittedContextDocumentPurge),
    Candidate(ContextDocumentPurgeStartCandidate),
}

struct PreparedContextDocumentPurgeStart {
    candidate: ContextDocumentPurgeStartCandidate,
    resource_state_ref: AggregateStateRef,
    upload_state_refs: Vec<AggregateStateRef>,
}

fn inspect_context_document_purge_start(
    connection: &Connection,
    command: &ContextDocumentPurgeStartCommand,
    request_digest: &str,
) -> Result<PurgeStartInspection, StoreError> {
    if let Some(replayed) = verify_request_receipt::<CommittedContextDocumentPurge>(
        connection,
        &command.principal_id,
        &command.request_id,
        request_digest,
    )? {
        if replayed.status.resource_id != command.resource_id
            || replayed.event.workspace_id != command.workspace_id
            || replayed.event.entity_id != command.resource_id
            || replayed.event.entity_type != "Resource"
        {
            return Err(StoreError::Integrity(
                "ContextDocument purge replay identity does not match its request".to_owned(),
            ));
        }
        return Ok(PurgeStartInspection::Replay(replayed));
    }

    workspace_owner_active(connection, &command.principal_id, &command.workspace_id)?;
    let mut resource =
        load_resource_detail_record(connection, &command.workspace_id, &command.resource_id)?
            .ok_or(StoreError::NotFound)?;
    let current_resource_version = resource.resource.version;
    let mut metadata = resource
        .context_document
        .take()
        .ok_or(StoreError::NotFound)?;
    if current_resource_version != command.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(current_resource_version),
        });
    }
    let metadata = metadata.as_object_mut().ok_or_else(|| {
        StoreError::Integrity("ContextDocument metadata is not an object".to_owned())
    })?;
    let current_status = metadata
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::Integrity("ContextDocument status is missing".to_owned()))?
        .to_owned();
    if !matches!(current_status.as_str(), "ACTIVE" | "REVOKED") {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(current_resource_version),
        });
    }
    let resource_updated_at = canonicalize_utc_timestamp(&resource.resource.updated_at)?;
    if command.event.recorded_at <= resource_updated_at {
        return Err(StoreError::Invalid(
            "ContextDocument purge timestamp must advance the Resource".to_owned(),
        ));
    }
    if metadata
        .get("purge_manifest_digest")
        .is_some_and(|value| !value.is_null())
        || metadata
            .get("purge_target_count")
            .is_some_and(|value| !value.is_null())
    {
        return Err(StoreError::Integrity(
            "ContextDocument already contains purge metadata without a plan".to_owned(),
        ));
    }
    reject_external_locations(connection, &command.resource_id)?;
    let targets = discover_local_targets(connection, &command.workspace_id, &command.resource_id)?;
    if targets.is_empty()
        && empty_plan_inventory_present(connection, &command.workspace_id, &command.resource_id)?
    {
        return Err(StoreError::Invalid(
            "ContextDocument purge cannot seal an empty plan while local inventory remains"
                .to_owned(),
        ));
    }
    let unregistered_revision_blobs = unregistered_resource_revision_blobs(
        connection,
        &command.workspace_id,
        &command.resource_id,
    )?;
    let target_count = u32::try_from(targets.len())
        .map_err(|_| StoreError::Invalid("ContextDocument purge plan is too large".to_owned()))?;
    let targets_json_bytes = canonical_json(&targets)?;
    let manifest_digest = digest(&targets_json_bytes);
    let uploads = inspect_linked_revision_upload_expirations(connection, command)?;

    metadata.insert(
        "status".to_owned(),
        Value::String(
            ContextDocumentPurgeStatus::DeletionPending
                .as_str()
                .to_owned(),
        ),
    );
    metadata.insert(
        "purge_manifest_digest".to_owned(),
        Value::String(manifest_digest.clone()),
    );
    metadata.insert(
        "purge_target_count".to_owned(),
        Value::Number(target_count.into()),
    );
    resource.resource.version = current_resource_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("Resource version overflow".to_owned()))?;
    resource.resource.updated_at = command.event.recorded_at.clone();
    resource.context_document = Some(Value::Object(metadata.clone()));

    Ok(PurgeStartInspection::Candidate(
        ContextDocumentPurgeStartCandidate {
            current_resource_version,
            current_status,
            resource,
            targets,
            manifest_digest,
            target_count,
            uploads,
            unregistered_revision_blobs,
        },
    ))
}

fn inspect_linked_revision_upload_expirations(
    connection: &Connection,
    command: &ContextDocumentPurgeStartCommand,
) -> Result<Vec<PreparedUploadExpiration>, StoreError> {
    let upload_ids: Vec<String> = {
        let mut statement = connection
            .prepare(
                "SELECT upload_id FROM resource_upload_sessions
                 WHERE workspace_id = ?1
                   AND (resource_id = ?2 OR committed_resource_id = ?2)
                   AND state IN ('OPEN', 'CONTENT_RECEIVED')
                 ORDER BY upload_id",
            )
            .map_err(map_database_error)?;
        statement
            .query_map(params![command.workspace_id, command.resource_id], |row| {
                row.get(0)
            })
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?
    };
    upload_ids
        .into_iter()
        .map(|upload_id| {
            let current = load_resource_upload(connection, &command.workspace_id, &upload_id)?
                .ok_or_else(|| {
                    StoreError::Integrity("linked ResourceUpload disappeared".to_owned())
                })?;
            if !matches!(
                current.state,
                storage_core::ResourceUploadState::Open
                    | storage_core::ResourceUploadState::ContentReceived
            ) {
                return Err(StoreError::Integrity(
                    "linked ResourceUpload is not active".to_owned(),
                ));
            }
            let mut expired = current.clone();
            expired.state = storage_core::ResourceUploadState::Expired;
            expired.version = current
                .version
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("ResourceUpload version overflow".to_owned()))?;
            Ok(PreparedUploadExpiration { current, expired })
        })
        .collect()
}

fn commit_context_document_purge_start(
    connection: &mut Connection,
    command: ContextDocumentPurgeStartCommand,
    request_digest: &str,
    prepared: PreparedContextDocumentPurgeStart,
) -> Result<CommittedContextDocumentPurge, StoreError> {
    if prepared.upload_state_refs.len() != prepared.candidate.uploads.len() {
        return Err(StoreError::Integrity(
            "prepared ContextDocument upload snapshot count is inconsistent".to_owned(),
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let candidate =
        match inspect_context_document_purge_start(&transaction, &command, request_digest)? {
            PurgeStartInspection::Replay(replayed) => return Ok(replayed),
            PurgeStartInspection::Candidate(candidate) => candidate,
        };
    if candidate != prepared.candidate {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: (candidate.current_resource_version
                != prepared.candidate.current_resource_version)
                .then_some(candidate.current_resource_version),
        });
    }
    let ContextDocumentPurgeStartCandidate {
        current_resource_version,
        current_status,
        resource,
        targets,
        manifest_digest,
        target_count,
        uploads,
        unregistered_revision_blobs: _,
    } = prepared.candidate;
    let targets_json = String::from_utf8(canonical_json(&targets)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;

    transaction
        .execute(
            "INSERT INTO context_document_purge_plans(
                 workspace_id, resource_id, manifest_digest, target_count, targets_json, sealed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                command.workspace_id,
                command.resource_id,
                manifest_digest,
                i64::from(target_count),
                targets_json,
                command.event.recorded_at,
            ],
        )
        .map_err(map_database_error)?;

    let metadata_json = String::from_utf8(canonical_json(&resource.context_document)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let changed = transaction
        .execute(
            "UPDATE resources SET context_document_json = ?1, updated_at = ?2, version = ?3
             WHERE workspace_id = ?4 AND resource_id = ?5 AND version = ?6",
            params![
                metadata_json,
                resource.resource.updated_at,
                to_sql_i64(resource.resource.version, "Resource version")?,
                command.workspace_id,
                command.resource_id,
                to_sql_i64(current_resource_version, "expected Resource version")?,
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(current_resource_version),
            actual: None,
        });
    }

    for target in &targets {
        let revisions_json = String::from_utf8(canonical_json(&target.target_revision_ids)?)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        let upload_chunks_json =
            String::from_utf8(canonical_json(&target.target_upload_chunk_refs)?)
                .map_err(|error| StoreError::Invalid(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO context_document_purge_receipts(
                     workspace_id, resource_id, replica_ref, replica_kind, runtime_id,
                     runtime_incarnation_id, target_revision_ids_json,
                     target_upload_chunk_refs_json, status, requested_at,
                     acknowledged_at, receipt_digest
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'PENDING', ?9, NULL, NULL)",
                params![
                    command.workspace_id,
                    command.resource_id,
                    target.replica_ref,
                    target.replica_kind.as_str(),
                    target.runtime_id,
                    target.runtime_incarnation_id,
                    revisions_json,
                    upload_chunks_json,
                    command.event.recorded_at,
                ],
            )
            .map_err(map_database_error)?;
    }

    expire_linked_revision_uploads(
        &transaction,
        &command,
        &uploads,
        &prepared.upload_state_refs,
    )?;
    transaction
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![command.workspace_id, command.resource_id],
        )
        .map_err(map_database_error)?;
    transaction
        .execute(
            "DELETE FROM resource_locations WHERE resource_id = ?1",
            [&command.resource_id],
        )
        .map_err(map_database_error)?;

    let status_event = insert_domain_event(
        &transaction,
        EventDraft {
            event_id: command.event.event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "Resource".to_owned(),
            entity_id: command.resource_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: resource.resource.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "resource.context_document.status.changed.v1".to_owned(),
            payload: status_changed_payload(
                &command.resource_id,
                &current_status,
                ContextDocumentPurgeStatus::DeletionPending,
                json!({"kind": "USER", "principal_id": command.principal_id.clone()}),
                &manifest_digest,
                target_count,
                resource.resource.version,
            ),
            recorded_at: command.event.recorded_at.clone(),
        },
        &prepared.resource_state_ref,
    )?;

    let status = ContextDocumentDeletionStatus {
        resource_id: command.resource_id.clone(),
        status: ContextDocumentPurgeStatus::DeletionPending,
        manifest_digest: Some(manifest_digest),
        required_replica_count: target_count,
        acknowledged_replica_count: 0,
        updated_at: resource.resource.updated_at.clone(),
    };
    let committed = CommittedContextDocumentPurge {
        status,
        event: status_event,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction
        .execute(
            "INSERT INTO request_dedup(
                 principal_id, request_id, request_digest, response_json, response_digest,
                 created_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                command.principal_id,
                command.request_id,
                request_digest,
                response_json,
                digest(response_json.as_bytes()),
                command.event.recorded_at,
            ],
        )
        .map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn expire_linked_revision_uploads(
    transaction: &Transaction<'_>,
    command: &ContextDocumentPurgeStartCommand,
    uploads: &[PreparedUploadExpiration],
    state_refs: &[AggregateStateRef],
) -> Result<(), StoreError> {
    if uploads.len() != state_refs.len() {
        return Err(StoreError::Integrity(
            "prepared ContextDocument upload snapshots are inconsistent".to_owned(),
        ));
    }
    for (upload, state_ref) in uploads.iter().zip(state_refs) {
        let current = &upload.current;
        let expired = &upload.expired;
        let upload_id = &current.upload_id;
        let event_id_seed = format!("{}:{}", command.event.event_id, upload_id);
        let event_id = format!(
            "event-purge-upload-{}",
            &digest(event_id_seed.as_bytes())[7..]
        );
        let draft = EventDraft {
            event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: upload_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id.clone(),
            entity_revision: expired.version,
            hlc_timestamp: command.event.hlc_timestamp.clone(),
            correlation_id: command.event.correlation_id.clone(),
            causation_id: Some(command.event.event_id.clone()),
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({
                "upload_id": upload_id,
                "from": match current.state {
                    storage_core::ResourceUploadState::Open => "OPEN",
                    storage_core::ResourceUploadState::ContentReceived => "CONTENT_RECEIVED",
                    _ => return Err(StoreError::Integrity(
                        "linked ResourceUpload is not active".to_owned(),
                    )),
                },
                "to": "EXPIRED",
                "aggregate_version": expired.version,
                "reason_code": "CONTEXT_DOCUMENT_DELETION"
            }),
            recorded_at: command.event.recorded_at.clone(),
        };
        let changed = transaction
            .execute(
                "UPDATE resource_upload_sessions
                 SET state = 'EXPIRED', version = version + 1
                 WHERE workspace_id = ?1 AND upload_id = ?2
                   AND state IN ('OPEN', 'CONTENT_RECEIVED') AND version = ?3
                   AND progress_version = ?4",
                params![
                    command.workspace_id,
                    upload_id,
                    to_sql_i64(current.version, "ResourceUpload version")?,
                    to_sql_i64(current.progress_version, "ResourceUpload progress version")?,
                ],
            )
            .map_err(map_database_error)?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                expected: Some(current.progress_version),
                actual: None,
            });
        }
        transaction
            .execute(
                "UPDATE resource_upload_blob_reservations
                 SET state = 'DELETING'
                 WHERE workspace_id = ?1 AND upload_id = ?2 AND state = 'RESERVED'",
                params![command.workspace_id, upload_id],
            )
            .map_err(map_database_error)?;
        insert_upload_domain_event(transaction, draft, state_ref.clone())?;
    }
    Ok(())
}

fn write_resource_upload_snapshot(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    upload: &storage_core::ResourceUploadSessionRecord,
) -> Result<AggregateStateRef, StoreError> {
    let state_bytes = canonical_json(upload)?;
    let blob = blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &state_bytes,
        "application/vnd.litecowork.resource-upload+json",
    )?;
    if blob.size_bytes != state_bytes.len() as u64
        || blobs.get(workspace_id, BlobPurpose::AggregateState, &blob)? != state_bytes
    {
        return Err(StoreError::Integrity(
            "ContextDocument deletion upload snapshot failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: upload.version,
        record_schema_version: 1,
    })
}

fn reject_external_locations(connection: &Connection, resource_id: &str) -> Result<(), StoreError> {
    let external: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM resource_locations
                 WHERE resource_id = ?1
                   AND NOT (
                       provider_ref = 'litecowork.encrypted_blob'
                       AND runtime_id IS NULL
                       AND environment_id IS NULL
                       AND connection_id IS NULL
                   )
             )",
            [resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if external {
        return Err(StoreError::Invalid(
            "local ContextDocument purge cannot remove provider-backed locations".to_owned(),
        ));
    }
    Ok(())
}

fn discover_local_targets(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
) -> Result<Vec<ContextDocumentPurgeTarget>, StoreError> {
    let has_local_location: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM resource_locations
                 WHERE resource_id = ?1
                   AND provider_ref = 'litecowork.encrypted_blob'
                   AND runtime_id IS NULL AND environment_id IS NULL AND connection_id IS NULL
             )",
            [resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let missing_local_registry: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM resource_revisions revision
                 LEFT JOIN resource_content_blob_registry registry
                   ON registry.workspace_id = ?1 AND registry.digest = revision.content_digest
                 WHERE revision.resource_id = ?2
                   AND revision.content_digest IS NOT NULL
                   AND registry.digest IS NULL
             )",
            params![workspace_id, resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if has_local_location && missing_local_registry {
        return Err(StoreError::Integrity(
            "managed Resource revision is missing its content registry row".to_owned(),
        ));
    }

    let mut grouped: BTreeMap<PurgeTargetGroupKey, PurgeTargetRevisionAndChunkRefs> =
        BTreeMap::new();
    {
        let mut statement = connection
            .prepare(
                "SELECT revision.content_digest, revision.resource_revision_id,
                        revision.size_bytes, COALESCE(revision.media_type, 'application/octet-stream'),
                        registry.size_bytes, registry.media_type, registry.state
                 FROM resource_revisions revision
                 JOIN resource_content_blob_registry registry
                   ON registry.workspace_id = ?1 AND registry.digest = revision.content_digest
                 WHERE revision.resource_id = ?2
                 ORDER BY revision.content_digest, revision.resource_revision_id",
            )
            .map_err(map_database_error)?;
        let rows = statement
            .query_map(params![workspace_id, resource_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(map_database_error)?;
        for row in rows {
            let (
                replica_ref,
                revision_id,
                size,
                media_type,
                registry_size,
                registry_media_type,
                state,
            ) = row.map_err(map_database_error)?;
            if !storage_core::is_sha256_digest(&replica_ref)
                || size.is_none_or(|size| size < 0)
                || from_sql_i64(registry_size, "Resource content registry size")?
                    != u64::try_from(size.unwrap_or_default()).unwrap_or_default()
                || media_type != registry_media_type
                || !matches!(state.as_str(), "RESERVED" | "TRACKED" | "DELETING")
            {
                return Err(StoreError::Integrity(
                    "Resource revision and content registry disagree".to_owned(),
                ));
            }
            grouped
                .entry((replica_ref, ContextDocumentPurgeReplicaKind::Blob.as_str()))
                .or_default()
                .0
                .push(revision_id);
        }
    }

    {
        let mut statement = connection
            .prepare(
                "SELECT index_row.extracted_blob_digest, index_row.resource_revision_id,
                        index_row.extracted_size_bytes, registry.size_bytes, registry.state
                 FROM resource_text_indexes index_row
                 LEFT JOIN resource_index_blob_registry registry
                   ON registry.workspace_id = index_row.workspace_id
                  AND registry.digest = index_row.extracted_blob_digest
                 WHERE index_row.workspace_id = ?1 AND index_row.resource_id = ?2
                 ORDER BY index_row.extracted_blob_digest, index_row.resource_revision_id",
            )
            .map_err(map_database_error)?;
        let rows = statement
            .query_map(params![workspace_id, resource_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(map_database_error)?;
        for row in rows {
            let (replica_ref, revision_id, extracted_size, registry_size, state) =
                row.map_err(map_database_error)?;
            if !storage_core::is_sha256_digest(&replica_ref)
                || registry_size != Some(extracted_size)
                || !matches!(state.as_deref(), Some("RESERVED" | "TRACKED" | "DELETING"))
            {
                return Err(StoreError::Integrity(
                    "ResourceIndex projection and registry disagree".to_owned(),
                ));
            }
            grouped
                .entry((
                    replica_ref,
                    ContextDocumentPurgeReplicaKind::DerivedIndex.as_str(),
                ))
                .or_default()
                .0
                .push(revision_id);
        }
    }

    {
        let mut statement = connection
            .prepare(
                "SELECT refs.digest, refs.upload_id, refs.chunk_index, refs.size_bytes,
                        session.state, registry.size_bytes, registry.state
                 FROM (
                   SELECT chunk.upload_id, chunk.chunk_index,
                          chunk.temporary_blob_ref AS digest,
                          chunk.end_offset_exclusive - chunk.start_offset AS size_bytes
                   FROM resource_upload_chunks chunk
                   UNION
                   SELECT reservation.upload_id, reservation.chunk_index,
                          reservation.digest, reservation.size_bytes
                   FROM resource_upload_blob_reservations reservation
                 ) refs
                 JOIN resource_upload_sessions session ON session.upload_id = refs.upload_id
                 LEFT JOIN resource_upload_chunk_blob_registry registry
                   ON registry.workspace_id = session.workspace_id
                  AND registry.digest = refs.digest
                 WHERE session.workspace_id = ?1
                   AND (session.resource_id = ?2 OR session.committed_resource_id = ?2)
                 ORDER BY refs.digest, refs.upload_id, refs.chunk_index",
            )
            .map_err(map_database_error)?;
        let rows = statement
            .query_map(params![workspace_id, resource_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(map_database_error)?;
        for row in rows {
            let (
                replica_ref,
                upload_id,
                chunk_index,
                size_bytes,
                session_state,
                registry_size,
                registry_state,
            ) = row.map_err(map_database_error)?;
            if !storage_core::is_sha256_digest(&replica_ref)
                || upload_id.trim().is_empty()
                || upload_id.contains('\0')
                || size_bytes <= 0
                || from_sql_i64(size_bytes, "ResourceUpload chunk size")? > 4_194_304
            {
                return Err(StoreError::Integrity(
                    "ResourceUpload chunk ledger identity is invalid".to_owned(),
                ));
            }
            let Some(registry_size) = registry_size else {
                if matches!(session_state.as_str(), "OPEN" | "CONTENT_RECEIVED") {
                    return Err(StoreError::Integrity(
                        "active ResourceUpload chunk is missing its blob registry row".to_owned(),
                    ));
                }
                // Terminal uploads may already have had their transfer object collected.
                continue;
            };
            if registry_size != size_bytes
                || !matches!(
                    registry_state.as_deref(),
                    Some("RESERVED" | "TRACKED" | "DELETING")
                )
            {
                return Err(StoreError::Integrity(
                    "ResourceUpload chunk and blob registry disagree".to_owned(),
                ));
            }
            let chunk_index = from_sql_i64(chunk_index, "ResourceUpload chunk index")?;
            grouped
                .entry((
                    replica_ref,
                    ContextDocumentPurgeReplicaKind::UploadChunk.as_str(),
                ))
                .or_default()
                .1
                .push(ContextDocumentPurgeUploadChunkRef {
                    upload_id,
                    chunk_index,
                });
        }
    }

    let mut targets = grouped
        .into_iter()
        .map(
            |((replica_ref, kind), (mut target_revision_ids, mut target_upload_chunk_refs))| {
                target_revision_ids.sort();
                target_revision_ids.dedup();
                target_upload_chunk_refs.sort();
                target_upload_chunk_refs.dedup();
                let replica_kind = match kind {
                    "BLOB" => ContextDocumentPurgeReplicaKind::Blob,
                    "DERIVED_INDEX" => ContextDocumentPurgeReplicaKind::DerivedIndex,
                    "UPLOAD_CHUNK" => ContextDocumentPurgeReplicaKind::UploadChunk,
                    _ => unreachable!("the grouped replica kind is locally constructed"),
                };
                if replica_kind == ContextDocumentPurgeReplicaKind::UploadChunk {
                    target_revision_ids.clear();
                } else {
                    target_upload_chunk_refs.clear();
                }
                ContextDocumentPurgeTarget {
                    replica_ref,
                    replica_kind,
                    runtime_id: None,
                    runtime_incarnation_id: None,
                    target_revision_ids,
                    target_upload_chunk_refs,
                }
            },
        )
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        left.replica_ref
            .as_bytes()
            .cmp(right.replica_ref.as_bytes())
            .then_with(|| left.replica_kind.as_str().cmp(right.replica_kind.as_str()))
    });
    Ok(targets)
}

fn unregistered_resource_revision_blobs(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
) -> Result<Vec<BlobRef>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT revision.content_digest, revision.size_bytes, revision.media_type
             FROM resource_revisions revision
             JOIN resources resource
               ON resource.workspace_id = ?1 AND resource.resource_id = revision.resource_id
             LEFT JOIN resource_content_blob_registry registry
               ON registry.workspace_id = resource.workspace_id
              AND registry.digest = revision.content_digest
             WHERE revision.resource_id = ?2
               AND revision.content_digest IS NOT NULL
               AND registry.digest IS NULL
             ORDER BY revision.content_digest, revision.resource_revision_id",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(params![workspace_id, resource_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(map_database_error)?;
    let mut by_digest = BTreeMap::<String, BlobRef>::new();
    for row in rows {
        let (digest, size_bytes, media_type) = row.map_err(map_database_error)?;
        if !storage_core::is_sha256_digest(&digest) {
            return Err(StoreError::Integrity(
                "unregistered Resource revision has an invalid content digest".to_owned(),
            ));
        }
        let size_bytes = size_bytes
            .and_then(|size| u64::try_from(size).ok())
            .ok_or_else(|| {
                StoreError::Integrity(
                    "unregistered Resource revision has no valid content size".to_owned(),
                )
            })?;
        let media_type = media_type.unwrap_or_else(|| "application/octet-stream".to_owned());
        if media_type.trim().is_empty() || media_type.contains('\0') {
            return Err(StoreError::Integrity(
                "unregistered Resource revision has invalid content media type".to_owned(),
            ));
        }
        let blob = BlobRef {
            digest: digest.clone(),
            size_bytes,
            media_type,
        };
        if let Some(existing) = by_digest.get(&digest) {
            if existing != &blob {
                return Err(StoreError::Integrity(
                    "unregistered Resource revisions disagree on blob metadata".to_owned(),
                ));
            }
        } else {
            by_digest.insert(digest, blob);
        }
    }
    Ok(by_digest.into_values().collect())
}

// BlobStore deliberately exposes exact reads rather than an inventory API. Probe only
// retained revision digests that have lost their recovery-registry row. A successful
// read means the exact local object still exists; NotFound proves only that this exact
// object is absent. Callers
// perform this outside SQLite transactions while holding the digest locks.
fn unregistered_resource_revision_blob_present(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    unregistered_blobs: &[BlobRef],
) -> Result<bool, StoreError> {
    for blob in unregistered_blobs {
        match blobs.get(workspace_id, BlobPurpose::Resource, blob) {
            Ok(_) => return Ok(true),
            Err(StoreError::NotFound) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

fn context_document_deletion_status(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
    principal_id: &str,
) -> Result<Option<ContextDocumentDeletionStatus>, StoreError> {
    let owner: Option<String> = connection
        .query_row(
            "SELECT owner_principal_id FROM workspaces WHERE workspace_id = ?1",
            [workspace_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    if owner.as_deref() != Some(principal_id) {
        return Err(StoreError::NotFound);
    }
    let Some(mut resource) = load_resource_detail_record(connection, workspace_id, resource_id)?
    else {
        return Ok(None);
    };
    let Some(metadata) = resource.context_document.take() else {
        return Ok(None);
    };
    let metadata = metadata.as_object().ok_or_else(|| {
        StoreError::Integrity("ContextDocument metadata is not an object".to_owned())
    })?;
    let status = parse_purge_status(
        metadata
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| StoreError::Integrity("ContextDocument status is missing".to_owned()))?,
    )?;
    let plan: Option<(String, i64, String)> = connection
        .query_row(
            "SELECT manifest_digest, target_count, targets_json
             FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            params![workspace_id, resource_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match status {
        ContextDocumentPurgeStatus::Active | ContextDocumentPurgeStatus::Revoked => {
            if plan.is_some()
                || metadata
                    .get("purge_manifest_digest")
                    .is_some_and(|value| !value.is_null())
                || metadata
                    .get("purge_target_count")
                    .is_some_and(|value| !value.is_null())
            {
                return Err(StoreError::Integrity(
                    "active ContextDocument has purge state".to_owned(),
                ));
            }
            Ok(Some(ContextDocumentDeletionStatus {
                resource_id: resource_id.to_owned(),
                status,
                manifest_digest: None,
                required_replica_count: 0,
                acknowledged_replica_count: 0,
                updated_at: resource.resource.updated_at,
            }))
        }
        ContextDocumentPurgeStatus::DeletionPending | ContextDocumentPurgeStatus::Deleted => {
            let digest_in_metadata = metadata
                .get("purge_manifest_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StoreError::Integrity("purge manifest digest is missing".to_owned())
                })?;
            let target_count_in_metadata = metadata
                .get("purge_target_count")
                .and_then(Value::as_u64)
                .and_then(|count| u32::try_from(count).ok())
                .ok_or_else(|| StoreError::Integrity("purge target count is missing".to_owned()))?;
            let Some((plan_digest, plan_count, targets_json)) = plan else {
                return Err(StoreError::Integrity(
                    "ContextDocument purge tombstone has no sealed plan".to_owned(),
                ));
            };
            let targets: Vec<ContextDocumentPurgeTarget> = serde_json::from_str(&targets_json)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            if !storage_core::is_sha256_digest(&plan_digest)
                || plan_digest != digest_in_metadata
                || from_sql_i64(plan_count, "ContextDocument purge target count")?
                    != u64::from(target_count_in_metadata)
                || targets.len()
                    != usize::try_from(target_count_in_metadata).map_err(|_| {
                        StoreError::Integrity("purge target count overflows usize".to_owned())
                    })?
                || digest(&canonical_json(&targets)?) != plan_digest
            {
                return Err(StoreError::Integrity(
                    "ContextDocument purge metadata does not match its sealed plan".to_owned(),
                ));
            }
            let (receipt_count, acknowledged_count): (i64, i64) = connection
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(status = 'ACKNOWLEDGED'), 0)
                     FROM context_document_purge_receipts
                     WHERE workspace_id = ?1 AND resource_id = ?2",
                    params![workspace_id, resource_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(map_database_error)?;
            let receipt_count = from_sql_i64(receipt_count, "ContextDocument purge receipt count")?;
            let acknowledged_count = from_sql_i64(
                acknowledged_count,
                "ContextDocument purge acknowledged count",
            )?;
            if receipt_count != u64::from(target_count_in_metadata)
                || acknowledged_count > receipt_count
                || (status == ContextDocumentPurgeStatus::Deleted
                    && acknowledged_count != receipt_count)
            {
                return Err(StoreError::Integrity(
                    "ContextDocument purge receipt totals are inconsistent".to_owned(),
                ));
            }
            Ok(Some(ContextDocumentDeletionStatus {
                resource_id: resource_id.to_owned(),
                status,
                manifest_digest: Some(plan_digest),
                required_replica_count: target_count_in_metadata,
                acknowledged_replica_count: u32::try_from(acknowledged_count).map_err(|_| {
                    StoreError::Integrity("purge receipt count overflows".to_owned())
                })?,
                updated_at: resource.resource.updated_at,
            }))
        }
    }
}

fn list_pending_targets(
    connection: &Connection,
    limit: usize,
) -> Result<Vec<PendingContextDocumentPurgeTarget>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT receipt.workspace_id, receipt.resource_id, plan.manifest_digest,
                    receipt.replica_ref, receipt.replica_kind, receipt.runtime_id,
                    receipt.runtime_incarnation_id, receipt.target_revision_ids_json,
                    receipt.target_upload_chunk_refs_json,
                    plan.targets_json
             FROM context_document_purge_receipts receipt
             JOIN context_document_purge_plans plan
               ON plan.workspace_id = receipt.workspace_id AND plan.resource_id = receipt.resource_id
             JOIN resources resource
               ON resource.workspace_id = receipt.workspace_id AND resource.resource_id = receipt.resource_id
             WHERE receipt.status = 'PENDING'
               AND json_extract(resource.context_document_json, '$.status') = 'DELETION_PENDING'
             ORDER BY receipt.workspace_id, receipt.resource_id, receipt.replica_ref, receipt.replica_kind
             LIMIT ?1",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([to_sql_i64(limit as u64, "purge page limit")?], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(map_database_error)?;
    rows.map(|row| {
        let (
            workspace_id,
            resource_id,
            manifest_digest,
            replica_ref,
            replica_kind,
            runtime_id,
            runtime_incarnation_id,
            target_revision_ids_json,
            target_upload_chunk_refs_json,
            targets_json,
        ) = row.map_err(map_database_error)?;
        let target = ContextDocumentPurgeTarget {
            replica_ref,
            replica_kind: parse_replica_kind(&replica_kind)?,
            runtime_id,
            runtime_incarnation_id,
            target_revision_ids: serde_json::from_str(&target_revision_ids_json)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            target_upload_chunk_refs: serde_json::from_str(&target_upload_chunk_refs_json)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
        };
        validate_local_target(&target)?;
        let manifest: Vec<ContextDocumentPurgeTarget> = serde_json::from_str(&targets_json)
            .map_err(|error| StoreError::Integrity(error.to_string()))?;
        if !manifest.contains(&target) || digest(&canonical_json(&manifest)?) != manifest_digest {
            return Err(StoreError::Integrity(
                "pending purge receipt is outside its sealed manifest".to_owned(),
            ));
        }
        Ok(PendingContextDocumentPurgeTarget {
            workspace_id,
            resource_id,
            manifest_digest,
            target,
        })
    })
    .collect()
}

fn list_pending_empty_plans(
    connection: &Connection,
    limit: usize,
) -> Result<Vec<PendingEmptyContextDocumentPurge>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT plan.workspace_id, plan.resource_id, plan.manifest_digest, plan.targets_json
             FROM context_document_purge_plans plan
             JOIN resources resource
               ON resource.workspace_id = plan.workspace_id AND resource.resource_id = plan.resource_id
             WHERE plan.target_count = 0
               AND json_extract(resource.context_document_json, '$.status') = 'DELETION_PENDING'
             ORDER BY plan.workspace_id, plan.resource_id
             LIMIT ?1",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([to_sql_i64(limit as u64, "purge page limit")?], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(map_database_error)?;
    rows.map(|row| {
        let (workspace_id, resource_id, manifest_digest, targets_json) =
            row.map_err(map_database_error)?;
        let targets: Vec<ContextDocumentPurgeTarget> = serde_json::from_str(&targets_json)
            .map_err(|error| StoreError::Integrity(error.to_string()))?;
        if !targets.is_empty() || digest(&canonical_json(&targets)?) != manifest_digest {
            return Err(StoreError::Integrity(
                "empty ContextDocument purge plan is inconsistent".to_owned(),
            ));
        }
        Ok(PendingEmptyContextDocumentPurge {
            workspace_id,
            resource_id,
            manifest_digest,
        })
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq)]
struct PurgeAcknowledgementCandidate {
    original_resource_version: u64,
    target_count: u32,
    receipt_digest: String,
    acknowledgement_payload: Value,
    final_acknowledgement: bool,
    resource: storage_core::ResourceDetailRecord,
}

enum PurgeAcknowledgementInspection {
    Result(bool),
    Candidate(Box<PurgeAcknowledgementCandidate>),
}

struct PreparedPurgeAcknowledgement {
    candidate: PurgeAcknowledgementCandidate,
    state_ref: AggregateStateRef,
}

fn inspect_purge_target_acknowledgement(
    connection: &Connection,
    command: &AcknowledgeContextDocumentPurgeTargetCommand,
) -> Result<PurgeAcknowledgementInspection, StoreError> {
    let plan: Option<(String, i64, String)> = connection
        .query_row(
            "SELECT manifest_digest, target_count, targets_json
             FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            params![command.workspace_id, command.resource_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((plan_digest, plan_count, targets_json)) = plan else {
        return Ok(PurgeAcknowledgementInspection::Result(false));
    };
    if plan_digest != command.manifest_digest {
        return Ok(PurgeAcknowledgementInspection::Result(false));
    }
    let targets: Vec<ContextDocumentPurgeTarget> = serde_json::from_str(&targets_json)
        .map_err(|error| StoreError::Integrity(error.to_string()))?;
    if digest(&canonical_json(&targets)?) != plan_digest || !targets.contains(&command.target) {
        return Ok(PurgeAcknowledgementInspection::Result(false));
    }
    let target_count = u32::try_from(from_sql_i64(
        plan_count,
        "ContextDocument purge target count",
    )?)
    .map_err(|_| StoreError::Integrity("purge target count overflows".to_owned()))?;

    let receipt: Option<PurgeReceiptIdentityRow> = connection
        .query_row(
            "SELECT status, runtime_id, runtime_incarnation_id, target_revision_ids_json,
                    target_upload_chunk_refs_json
             FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2
               AND replica_ref = ?3 AND replica_kind = ?4",
            params![
                command.workspace_id,
                command.resource_id,
                command.target.replica_ref,
                command.target.replica_kind.as_str(),
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        receipt_status,
        runtime_id,
        runtime_incarnation_id,
        revision_ids_json,
        upload_refs_json,
    )) = receipt
    else {
        return Ok(PurgeAcknowledgementInspection::Result(false));
    };
    let receipt_revision_ids: Vec<String> = serde_json::from_str(&revision_ids_json)
        .map_err(|error| StoreError::Integrity(error.to_string()))?;
    let receipt_upload_refs: Vec<ContextDocumentPurgeUploadChunkRef> =
        serde_json::from_str(&upload_refs_json)
            .map_err(|error| StoreError::Integrity(error.to_string()))?;
    if runtime_id != command.target.runtime_id
        || runtime_incarnation_id != command.target.runtime_incarnation_id
        || receipt_revision_ids != command.target.target_revision_ids
        || receipt_upload_refs != command.target.target_upload_chunk_refs
    {
        return Ok(PurgeAcknowledgementInspection::Result(false));
    }
    if receipt_status == "ACKNOWLEDGED" {
        return Ok(PurgeAcknowledgementInspection::Result(true));
    }
    if receipt_status != "PENDING" {
        return Err(StoreError::Integrity(
            "ContextDocument purge receipt has an invalid state".to_owned(),
        ));
    }
    let mut resource =
        load_resource_detail_record(connection, &command.workspace_id, &command.resource_id)?
            .ok_or(StoreError::NotFound)?;
    let original_resource_version = resource.resource.version;
    let mut metadata = resource
        .context_document
        .take()
        .ok_or(StoreError::NotFound)?;
    let metadata_object = metadata.as_object_mut().ok_or_else(|| {
        StoreError::Integrity("ContextDocument metadata is not an object".to_owned())
    })?;
    if metadata_object.get("status").and_then(Value::as_str) != Some("DELETION_PENDING")
        || metadata_object
            .get("purge_manifest_digest")
            .and_then(Value::as_str)
            != Some(command.manifest_digest.as_str())
        || metadata_object
            .get("purge_target_count")
            .and_then(Value::as_u64)
            != Some(u64::from(target_count))
    {
        return Ok(PurgeAcknowledgementInspection::Result(false));
    }
    if replica_registry_exists(connection, &command.workspace_id, &command.target)? {
        return Ok(PurgeAcknowledgementInspection::Result(false));
    }
    let (total_receipts, acknowledged_receipts): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(status = 'ACKNOWLEDGED'), 0)
             FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![command.workspace_id, command.resource_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_database_error)?;
    let total_receipts = from_sql_i64(total_receipts, "ContextDocument purge receipt count")?;
    let acknowledged_receipts = from_sql_i64(
        acknowledged_receipts,
        "ContextDocument purge acknowledgement count",
    )?;
    if total_receipts != u64::from(target_count) || acknowledged_receipts >= total_receipts {
        return Err(StoreError::Integrity(
            "ContextDocument purge receipt totals are inconsistent".to_owned(),
        ));
    }
    let final_acknowledgement = acknowledged_receipts + 1 == total_receipts;
    let previous_updated_at = canonicalize_utc_timestamp(&resource.resource.updated_at)?;
    if command.event.recorded_at <= previous_updated_at
        || (final_acknowledgement
            && (command.completion_event.recorded_at <= previous_updated_at
                || command.completion_event.recorded_at < command.event.recorded_at))
    {
        return Err(StoreError::Invalid(
            "purge acknowledgement event timestamps must advance".to_owned(),
        ));
    }

    let target_digest = digest(&canonical_json(&command.target)?);
    let acknowledgement_payload = json!({
        "resource_id": command.resource_id,
        "purge_manifest_digest": command.manifest_digest,
        "replica_ref": command.target.replica_ref,
        "replica_kind": command.target.replica_kind.as_str(),
        "runtime_id": command.target.runtime_id,
        "runtime_incarnation_id": command.target.runtime_incarnation_id,
        "target_digest": target_digest,
        "acknowledged_at": command.acknowledged_at,
    });
    let receipt_digest = digest(&canonical_json(&acknowledgement_payload)?);
    if final_acknowledgement {
        metadata_object.insert(
            "status".to_owned(),
            Value::String(ContextDocumentPurgeStatus::Deleted.as_str().to_owned()),
        );
        resource.resource.updated_at = command.completion_event.recorded_at.clone();
    } else {
        resource.resource.updated_at = command.event.recorded_at.clone();
    }
    resource.resource.version = resource
        .resource
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("Resource version overflow".to_owned()))?;
    resource.context_document = Some(Value::Object(metadata_object.clone()));

    Ok(PurgeAcknowledgementInspection::Candidate(Box::new(
        PurgeAcknowledgementCandidate {
            original_resource_version,
            target_count,
            receipt_digest,
            acknowledgement_payload,
            final_acknowledgement,
            resource,
        },
    )))
}

fn commit_purge_target_acknowledgement(
    connection: &mut Connection,
    command: AcknowledgeContextDocumentPurgeTargetCommand,
    prepared: PreparedPurgeAcknowledgement,
) -> Result<bool, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let candidate = match inspect_purge_target_acknowledgement(&transaction, &command)? {
        PurgeAcknowledgementInspection::Result(result) => return Ok(result),
        PurgeAcknowledgementInspection::Candidate(candidate) => *candidate,
    };
    if candidate != prepared.candidate {
        return Ok(false);
    }
    let PurgeAcknowledgementCandidate {
        original_resource_version,
        target_count,
        receipt_digest,
        mut acknowledgement_payload,
        final_acknowledgement,
        resource,
    } = prepared.candidate;
    let changed_receipt = transaction
        .execute(
            "UPDATE context_document_purge_receipts
             SET status = 'ACKNOWLEDGED', acknowledged_at = ?1, receipt_digest = ?2
             WHERE workspace_id = ?3 AND resource_id = ?4
               AND replica_ref = ?5 AND replica_kind = ?6 AND status = 'PENDING'",
            params![
                command.acknowledged_at,
                receipt_digest,
                command.workspace_id,
                command.resource_id,
                command.target.replica_ref,
                command.target.replica_kind.as_str(),
            ],
        )
        .map_err(map_database_error)?;
    if changed_receipt != 1 {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let metadata_json = String::from_utf8(canonical_json(&resource.context_document)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let changed = transaction
        .execute(
            "UPDATE resources SET context_document_json = ?1, updated_at = ?2, version = ?3
             WHERE workspace_id = ?4 AND resource_id = ?5 AND version = ?6",
            params![
                metadata_json,
                resource.resource.updated_at,
                to_sql_i64(resource.resource.version, "Resource version")?,
                command.workspace_id,
                command.resource_id,
                to_sql_i64(original_resource_version, "previous Resource version")?,
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(original_resource_version),
            actual: None,
        });
    }
    acknowledgement_payload["receipt_digest"] = Value::String(receipt_digest);
    acknowledgement_payload["aggregate_version"] = json!(resource.resource.version);
    insert_domain_event(
        &transaction,
        EventDraft {
            event_id: command.event.event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "Resource".to_owned(),
            entity_id: command.resource_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: resource.resource.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "resource.context_document.purge.acknowledged.v2".to_owned(),
            payload: acknowledgement_payload,
            recorded_at: command.event.recorded_at.clone(),
        },
        &prepared.state_ref,
    )?;
    if final_acknowledgement {
        insert_domain_event(
            &transaction,
            EventDraft {
                event_id: command.completion_event.event_id,
                workspace_id: command.workspace_id.clone(),
                entity_type: "Resource".to_owned(),
                entity_id: command.resource_id.clone(),
                origin_runtime_id: command.completion_event.origin_runtime_id,
                entity_revision: resource.resource.version,
                hlc_timestamp: command.completion_event.hlc_timestamp,
                correlation_id: command.completion_event.correlation_id,
                causation_id: command.completion_event.causation_id,
                schema_version: 1,
                event_type: "resource.context_document.status.changed.v1".to_owned(),
                payload: status_changed_payload(
                    &command.resource_id,
                    ContextDocumentPurgeStatus::DeletionPending.as_str(),
                    ContextDocumentPurgeStatus::Deleted,
                    json!({"kind": "SERVICE", "service_id": PURGE_RECONCILER_SERVICE_ID}),
                    &command.manifest_digest,
                    target_count,
                    resource.resource.version,
                ),
                recorded_at: command.completion_event.recorded_at,
            },
            &prepared.state_ref,
        )?;
    }
    transaction.commit().map_err(map_database_error)?;
    Ok(true)
}

fn replica_registry_exists(
    connection: &Connection,
    workspace_id: &str,
    target: &ContextDocumentPurgeTarget,
) -> Result<bool, StoreError> {
    let table = match target.replica_kind {
        ContextDocumentPurgeReplicaKind::Blob => "resource_content_blob_registry",
        ContextDocumentPurgeReplicaKind::DerivedIndex => "resource_index_blob_registry",
        ContextDocumentPurgeReplicaKind::UploadChunk => "resource_upload_chunk_blob_registry",
    };
    let sql =
        format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE workspace_id = ?1 AND digest = ?2)");
    connection
        .query_row(&sql, params![workspace_id, target.replica_ref], |row| {
            row.get(0)
        })
        .map_err(map_database_error)
}

#[derive(Clone, Debug, PartialEq)]
struct EmptyPurgeCompletionCandidate {
    original_resource_version: u64,
    resource: storage_core::ResourceDetailRecord,
    unregistered_revision_blobs: Vec<BlobRef>,
}

enum EmptyPurgeCompletionInspection {
    Result(bool),
    Candidate(Box<EmptyPurgeCompletionCandidate>),
}

struct PreparedEmptyPurgeCompletion {
    candidate: EmptyPurgeCompletionCandidate,
    state_ref: AggregateStateRef,
}

fn inspect_empty_purge_completion(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
    manifest_digest: &str,
    event: &ContextDocumentStatusEventContext,
) -> Result<EmptyPurgeCompletionInspection, StoreError> {
    let plan: Option<(String, i64, String)> = connection
        .query_row(
            "SELECT manifest_digest, target_count, targets_json
             FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            params![workspace_id, resource_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((plan_digest, target_count, targets_json)) = plan else {
        return Ok(EmptyPurgeCompletionInspection::Result(false));
    };
    if plan_digest != manifest_digest || target_count != 0 || targets_json != "[]" {
        return Ok(EmptyPurgeCompletionInspection::Result(false));
    }
    let Some(mut resource) = load_resource_detail_record(connection, workspace_id, resource_id)?
    else {
        return Ok(EmptyPurgeCompletionInspection::Result(false));
    };
    let original_resource_version = resource.resource.version;
    let mut metadata = resource
        .context_document
        .take()
        .ok_or(StoreError::NotFound)?;
    let metadata_object = metadata.as_object_mut().ok_or_else(|| {
        StoreError::Integrity("ContextDocument metadata is not an object".to_owned())
    })?;
    let current_status = metadata_object.get("status").and_then(Value::as_str);
    if metadata_object
        .get("purge_manifest_digest")
        .and_then(Value::as_str)
        != Some(manifest_digest)
        || metadata_object
            .get("purge_target_count")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Ok(EmptyPurgeCompletionInspection::Result(false));
    }
    if current_status == Some(ContextDocumentPurgeStatus::Deleted.as_str()) {
        return Ok(EmptyPurgeCompletionInspection::Result(true));
    }
    if current_status != Some(ContextDocumentPurgeStatus::DeletionPending.as_str()) {
        return Ok(EmptyPurgeCompletionInspection::Result(false));
    }
    if empty_plan_inventory_present(connection, workspace_id, resource_id)? {
        return Ok(EmptyPurgeCompletionInspection::Result(false));
    }
    let unregistered_revision_blobs =
        unregistered_resource_revision_blobs(connection, workspace_id, resource_id)?;
    let previous_updated_at = canonicalize_utc_timestamp(&resource.resource.updated_at)?;
    if event.recorded_at <= previous_updated_at {
        return Err(StoreError::Invalid(
            "ContextDocument purge completion timestamp must advance the Resource".to_owned(),
        ));
    }
    metadata_object.insert(
        "status".to_owned(),
        Value::String(ContextDocumentPurgeStatus::Deleted.as_str().to_owned()),
    );
    resource.resource.version = resource
        .resource
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("Resource version overflow".to_owned()))?;
    resource.resource.updated_at = event.recorded_at.clone();
    resource.context_document = Some(Value::Object(metadata_object.clone()));
    Ok(EmptyPurgeCompletionInspection::Candidate(Box::new(
        EmptyPurgeCompletionCandidate {
            original_resource_version,
            resource,
            unregistered_revision_blobs,
        },
    )))
}

fn commit_empty_purge_completion(
    connection: &mut Connection,
    workspace_id: &str,
    resource_id: &str,
    manifest_digest: &str,
    event: ContextDocumentStatusEventContext,
    prepared: PreparedEmptyPurgeCompletion,
) -> Result<bool, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let candidate = match inspect_empty_purge_completion(
        &transaction,
        workspace_id,
        resource_id,
        manifest_digest,
        &event,
    )? {
        EmptyPurgeCompletionInspection::Result(result) => return Ok(result),
        EmptyPurgeCompletionInspection::Candidate(candidate) => *candidate,
    };
    if candidate != prepared.candidate {
        return Ok(false);
    }
    let EmptyPurgeCompletionCandidate {
        original_resource_version,
        resource,
        unregistered_revision_blobs: _,
    } = prepared.candidate;
    let metadata_json = String::from_utf8(canonical_json(&resource.context_document)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let changed = transaction
        .execute(
            "UPDATE resources SET context_document_json = ?1, updated_at = ?2, version = ?3
             WHERE workspace_id = ?4 AND resource_id = ?5 AND version = ?6",
            params![
                metadata_json,
                resource.resource.updated_at,
                to_sql_i64(resource.resource.version, "Resource version")?,
                workspace_id,
                resource_id,
                to_sql_i64(original_resource_version, "previous Resource version")?,
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(original_resource_version),
            actual: None,
        });
    }
    insert_domain_event(
        &transaction,
        EventDraft {
            event_id: event.event_id,
            workspace_id: workspace_id.to_owned(),
            entity_type: "Resource".to_owned(),
            entity_id: resource_id.to_owned(),
            origin_runtime_id: event.origin_runtime_id,
            entity_revision: resource.resource.version,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            schema_version: 1,
            event_type: "resource.context_document.status.changed.v1".to_owned(),
            payload: status_changed_payload(
                resource_id,
                ContextDocumentPurgeStatus::DeletionPending.as_str(),
                ContextDocumentPurgeStatus::Deleted,
                json!({"kind": "SERVICE", "service_id": PURGE_RECONCILER_SERVICE_ID}),
                manifest_digest,
                0,
                resource.resource.version,
            ),
            recorded_at: event.recorded_at,
        },
        &prepared.state_ref,
    )?;
    transaction.commit().map_err(map_database_error)?;
    Ok(true)
}

fn empty_plan_inventory_present(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
) -> Result<bool, StoreError> {
    let any_locations: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM resource_locations WHERE resource_id = ?1)",
            [resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if any_locations {
        return Ok(true);
    }
    let any_indexes: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM resource_text_indexes
                 WHERE workspace_id = ?1 AND resource_id = ?2
             )",
            params![workspace_id, resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if any_indexes {
        return Ok(true);
    }
    let content_object_registered: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM resource_revisions revision
                 JOIN resource_content_blob_registry registry
                   ON registry.workspace_id = ?1 AND registry.digest = revision.content_digest
                 WHERE revision.resource_id = ?2 AND revision.content_digest IS NOT NULL
             )",
            params![workspace_id, resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if content_object_registered {
        return Ok(true);
    }
    let upload_object_registered: bool = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM resource_upload_chunk_blob_registry registry
               WHERE registry.workspace_id = ?1
                 AND (
                   EXISTS (
                     SELECT 1 FROM resource_upload_chunks chunk
                     JOIN resource_upload_sessions session ON session.upload_id = chunk.upload_id
                     WHERE session.workspace_id = registry.workspace_id
                       AND chunk.temporary_blob_ref = registry.digest
                       AND (session.resource_id = ?2 OR session.committed_resource_id = ?2)
                   ) OR EXISTS (
                   SELECT 1 FROM resource_upload_blob_reservations reservation
                   JOIN resource_upload_sessions session ON session.upload_id = reservation.upload_id
                   WHERE session.workspace_id = registry.workspace_id
                     AND reservation.digest = registry.digest
                     AND (session.resource_id = ?2 OR session.committed_resource_id = ?2)
                 )
                 )
             )",
            params![workspace_id, resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if upload_object_registered {
        return Ok(true);
    }
    // Once a ResourceIndex row is removed there is no longer a durable owner pointer
    // from that digest to a particular Resource. An unrooted Workspace registry row is
    // therefore conservatively treated as unresolved local inventory until the fenced
    // collector removes it. Rooted objects belonging to other Resources do not block.
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM resource_index_blob_registry registry
                 WHERE registry.workspace_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM resource_text_indexes index_row
                       WHERE index_row.workspace_id = registry.workspace_id
                         AND index_row.extracted_blob_digest = registry.digest
                   )
             )",
            [workspace_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)
}

fn write_resource_snapshot(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    resource: &storage_core::ResourceDetailRecord,
) -> Result<AggregateStateRef, StoreError> {
    let state_bytes = canonical_json(resource)?;
    let blob = blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &state_bytes,
        RESOURCE_AGGREGATE_MEDIA_TYPE,
    )?;
    if blobs.get(workspace_id, BlobPurpose::AggregateState, &blob)? != state_bytes {
        return Err(StoreError::Integrity(
            "ContextDocument purge aggregate snapshot failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: resource.resource.version,
        record_schema_version: 1,
    })
}

fn status_changed_payload(
    resource_id: &str,
    from: &str,
    to: ContextDocumentPurgeStatus,
    changed_by: Value,
    manifest_digest: &str,
    target_count: u32,
    aggregate_version: u64,
) -> Value {
    json!({
        "resource_id": resource_id,
        "from": from,
        "to": to.as_str(),
        "changed_by": changed_by,
        "aggregate_version": aggregate_version,
        "purge_manifest_digest": manifest_digest,
        "purge_target_count": target_count,
    })
}

fn parse_purge_status(value: &str) -> Result<ContextDocumentPurgeStatus, StoreError> {
    match value {
        "ACTIVE" => Ok(ContextDocumentPurgeStatus::Active),
        "REVOKED" => Ok(ContextDocumentPurgeStatus::Revoked),
        "DELETION_PENDING" => Ok(ContextDocumentPurgeStatus::DeletionPending),
        "DELETED" => Ok(ContextDocumentPurgeStatus::Deleted),
        _ => Err(StoreError::Integrity(
            "stored ContextDocument status is invalid".to_owned(),
        )),
    }
}

fn parse_replica_kind(value: &str) -> Result<ContextDocumentPurgeReplicaKind, StoreError> {
    match value {
        "BLOB" => Ok(ContextDocumentPurgeReplicaKind::Blob),
        "DERIVED_INDEX" => Ok(ContextDocumentPurgeReplicaKind::DerivedIndex),
        "UPLOAD_CHUNK" => Ok(ContextDocumentPurgeReplicaKind::UploadChunk),
        _ => Err(StoreError::Integrity(
            "stored ContextDocument purge replica kind is invalid".to_owned(),
        )),
    }
}

fn validate_local_target(target: &ContextDocumentPurgeTarget) -> Result<(), StoreError> {
    if !storage_core::is_sha256_digest(&target.replica_ref)
        || target.runtime_id.is_some()
        || target.runtime_incarnation_id.is_some()
        || target
            .target_revision_ids
            .iter()
            .any(|revision_id| revision_id.trim().is_empty() || revision_id.contains('\0'))
    {
        return Err(StoreError::Invalid(
            "ContextDocument purge target is not a valid local replica".to_owned(),
        ));
    }
    match target.replica_kind {
        ContextDocumentPurgeReplicaKind::Blob | ContextDocumentPurgeReplicaKind::DerivedIndex => {
            if target.target_revision_ids.is_empty() || !target.target_upload_chunk_refs.is_empty()
            {
                return Err(StoreError::Invalid(
                    "ContextDocument purge Resource target has the wrong reference set".to_owned(),
                ));
            }
        }
        ContextDocumentPurgeReplicaKind::UploadChunk => {
            if !target.target_revision_ids.is_empty() || target.target_upload_chunk_refs.is_empty()
            {
                return Err(StoreError::Invalid(
                    "ContextDocument purge upload target has the wrong reference set".to_owned(),
                ));
            }
            if target.target_upload_chunk_refs.iter().any(|reference| {
                reference.upload_id.trim().is_empty()
                    || reference.upload_id.contains('\0')
                    || reference.chunk_index > i64::MAX as u64
            }) {
                return Err(StoreError::Invalid(
                    "ContextDocument purge upload chunk reference is invalid".to_owned(),
                ));
            }
            let mut sorted_refs = target.target_upload_chunk_refs.clone();
            sorted_refs.sort();
            sorted_refs.dedup();
            if sorted_refs != target.target_upload_chunk_refs {
                return Err(StoreError::Invalid(
                    "ContextDocument purge upload chunk refs are not uniquely sorted".to_owned(),
                ));
            }
        }
    }
    let mut sorted_revision_ids = target.target_revision_ids.clone();
    sorted_revision_ids.sort();
    sorted_revision_ids.dedup();
    if sorted_revision_ids != target.target_revision_ids {
        return Err(StoreError::Invalid(
            "ContextDocument purge target revision IDs are not uniquely sorted".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        time::Duration,
    };
    use storage_core::{
        BlobPurpose, BlobRef, BlobStore, ContextDocumentPurgeStore,
        ContextDocumentStatusEventContext, EventDraft, ResourceRecord, ResourceRevisionRecord,
        ResourceStore, ResourceUploadSessionRecord, ResourceUploadState, StoreError,
        WorkspaceCreateRequest,
    };
    use zeroize::Zeroizing;

    const WORKSPACE: &str = "workspace-purge-blob-boundary";
    const OWNER: &str = "owner-purge-blob-boundary";
    const CREATED_AT: &str = "2026-10-15T10:00:00Z";
    const CONTEXT_ADDED_AT: &str = "2026-10-15T10:00:02Z";
    const GC_AT: &str = "2099-01-01T00:00:00Z";

    struct BoundaryTestKeys;

    impl WorkspaceBlobKeyProvider for BoundaryTestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([41_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob(
                    "unknown transaction-boundary test key version".to_owned(),
                ));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    struct BoundaryBlobStore {
        inner: Arc<FileBlobStore<BoundaryTestKeys>>,
        database_path: PathBuf,
        calls: AtomicUsize,
        resource_gets: AtomicUsize,
        violations: AtomicUsize,
        reject_next_put_by_version_update: AtomicBool,
        raced_resource: Mutex<Option<(String, u64)>>,
        last_aggregate_state_digest: Mutex<Option<String>>,
    }

    impl BoundaryBlobStore {
        fn new(root: &Path, database_path: PathBuf) -> Self {
            Self {
                inner: Arc::new(FileBlobStore::new(root, BoundaryTestKeys)),
                database_path,
                calls: AtomicUsize::new(0),
                resource_gets: AtomicUsize::new(0),
                violations: AtomicUsize::new(0),
                reject_next_put_by_version_update: AtomicBool::new(false),
                raced_resource: Mutex::new(None),
                last_aggregate_state_digest: Mutex::new(None),
            }
        }

        fn arm_resource_version_race(&self, resource_id: &str, expected_version: u64) {
            *self
                .raced_resource
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((resource_id.to_owned(), expected_version));
            self.reject_next_put_by_version_update
                .store(true, Ordering::SeqCst);
        }

        fn verify_no_sql_writer_transaction(&self) -> Result<(), StoreError> {
            let connection = Connection::open(&self.database_path).map_err(|error| {
                StoreError::Blob(format!("open BlobStore boundary probe: {error}"))
            })?;
            connection.busy_timeout(Duration::ZERO).map_err(|error| {
                StoreError::Blob(format!("configure BlobStore boundary probe: {error}"))
            })?;
            match connection.execute_batch("BEGIN IMMEDIATE") {
                Ok(()) => {
                    connection.execute_batch("ROLLBACK").map_err(|error| {
                        StoreError::Blob(format!("rollback BlobStore boundary probe: {error}"))
                    })?;
                    self.calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
                Err(error) => {
                    self.violations.fetch_add(1, Ordering::SeqCst);
                    Err(StoreError::Blob(format!(
                        "BlobStore I/O ran while the SQLite writer transaction was open: {error}"
                    )))
                }
            }
        }

        fn race_resource_version_if_armed(&self) -> Result<(), StoreError> {
            if !self
                .reject_next_put_by_version_update
                .swap(false, Ordering::SeqCst)
            {
                return Ok(());
            }
            let (resource_id, expected_version) = self
                .raced_resource
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .expect("armed version race has a Resource identity and version");
            let connection = Connection::open(&self.database_path).map_err(map_database_error)?;
            let changed = connection
                .execute(
                    "UPDATE resources SET version = version + 1, updated_at = '2026-10-15T10:00:03Z'
                     WHERE workspace_id = ?1 AND resource_id = ?2 AND version = ?3",
                    params![
                        WORKSPACE,
                        resource_id,
                        to_sql_i64(expected_version, "raced Resource version")?,
                    ],
                )
                .map_err(map_database_error)?;
            if changed != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(expected_version),
                    actual: None,
                });
            }
            Ok(())
        }
    }

    impl BlobStore for BoundaryBlobStore {
        fn put(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            bytes: &[u8],
            media_type: &str,
        ) -> Result<BlobRef, StoreError> {
            self.verify_no_sql_writer_transaction()?;
            let blob = self.inner.put(workspace_id, purpose, bytes, media_type)?;
            if purpose == BlobPurpose::AggregateState {
                *self
                    .last_aggregate_state_digest
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(blob.digest.clone());
                self.race_resource_version_if_armed()?;
            }
            Ok(blob)
        }

        fn get(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            blob: &BlobRef,
        ) -> Result<Vec<u8>, StoreError> {
            if purpose == BlobPurpose::Resource {
                self.resource_gets.fetch_add(1, Ordering::SeqCst);
            }
            self.verify_no_sql_writer_transaction()?;
            self.inner.get(workspace_id, purpose, blob)
        }

        fn remove(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            blob: &BlobRef,
        ) -> Result<(), StoreError> {
            self.inner.remove(workspace_id, purpose, blob)
        }

        fn resource_index_token(
            &self,
            workspace_id: &str,
            key_version: Option<u32>,
            normalized_term: &str,
        ) -> Result<(u32, String), StoreError> {
            self.inner
                .resource_index_token(workspace_id, key_version, normalized_term)
        }

        fn resource_index_tokens(
            &self,
            workspace_id: &str,
            key_version: Option<u32>,
            normalized_terms: &[String],
        ) -> Result<(u32, Vec<String>), StoreError> {
            self.inner
                .resource_index_tokens(workspace_id, key_version, normalized_terms)
        }
    }

    struct PurgeFixture {
        directory: tempfile::TempDir,
        blobs: Arc<BoundaryBlobStore>,
        store: SqliteWorkspaceStore,
    }

    fn fixture() -> PurgeFixture {
        let directory = tempfile::tempdir().expect("create temporary purge fixture");
        let database_path = directory.path().join("state").join("state.sqlite3");
        let blobs = Arc::new(BoundaryBlobStore::new(
            &directory.path().join("blobs"),
            database_path,
        ));
        let store = SqliteWorkspaceStore::open(
            directory.path().join("state").join("state.sqlite3"),
            blobs.clone(),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("open temporary purge fixture");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Blob boundary test Workspace".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "event-create-purge-boundary-workspace".to_owned(),
                    origin_runtime_id: "runtime-local".to_owned(),
                    hlc_timestamp: CREATED_AT.to_owned(),
                    correlation_id: "correlation-create-purge-boundary-workspace".to_owned(),
                    causation_id: None,
                    recorded_at: CREATED_AT.to_owned(),
                },
            })
            .expect("create fixture Workspace");
        PurgeFixture {
            directory,
            blobs,
            store,
        }
    }

    fn event(event_id: &str, timestamp: &str) -> ContextDocumentStatusEventContext {
        ContextDocumentStatusEventContext {
            event_id: event_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            hlc_timestamp: timestamp.to_owned(),
            correlation_id: format!("correlation-{event_id}"),
            causation_id: None,
            recorded_at: timestamp.to_owned(),
        }
    }

    fn create_context_resource(
        fixture: &PurgeFixture,
        resource_id: &str,
        revision_id: &str,
        content: &[u8],
    ) -> (String, String) {
        let content_digest = digest(content);
        let resource = ResourceRecord {
            resource_id: resource_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            kind: "FILE".to_owned(),
            provider_identity: json!({}),
            identity_digest: None,
            display_name: format!("{resource_id}.txt"),
            current_revision_id: Some(revision_id.to_owned()),
            sensitivity: "PERSONAL".to_owned(),
            provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
            created_at: CREATED_AT.to_owned(),
            updated_at: CREATED_AT.to_owned(),
            version: 1,
        };
        let revision = ResourceRevisionRecord {
            resource_revision_id: revision_id.to_owned(),
            resource_id: resource_id.to_owned(),
            parent_revision_ids: Vec::new(),
            provider_revision: None,
            content_digest: Some(content_digest.clone()),
            size_bytes: Some(content.len() as u64),
            media_type: Some("text/plain".to_owned()),
            observed_at: CREATED_AT.to_owned(),
            created_by: json!({"kind": "USER", "principal_id": OWNER}),
        };
        let event_id = format!("event-create-{resource_id}");
        fixture
            .store
            .create_resource(
                WorkspaceCreateRequest {
                    principal_id: OWNER.to_owned(),
                    request_id: format!("request-create-{resource_id}"),
                    request_payload: json!({
                        "operation": "resource.create.v1",
                        "workspace_id": WORKSPACE,
                        "resource_id": resource_id,
                        "upload_id": format!("fixture-upload-{resource_id}"),
                    }),
                },
                resource.clone(),
                revision,
                EventDraft {
                    event_id: event_id.clone(),
                    workspace_id: WORKSPACE.to_owned(),
                    entity_type: "Resource".to_owned(),
                    entity_id: resource_id.to_owned(),
                    origin_runtime_id: "runtime-local".to_owned(),
                    entity_revision: 1,
                    hlc_timestamp: CREATED_AT.to_owned(),
                    correlation_id: format!("correlation-{event_id}"),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "resource.created.v1".to_owned(),
                    payload: json!({
                        "resource_id": resource_id,
                        "workspace_id": WORKSPACE,
                        "kind": "FILE",
                        "provenance": resource.provenance,
                        "aggregate_version": 1,
                    }),
                    recorded_at: CREATED_AT.to_owned(),
                },
                content.to_vec(),
            )
            .expect("create managed Resource content");
        let database_path = fixture.directory.path().join("state").join("state.sqlite3");
        Connection::open(database_path)
            .expect("open fixture Resource database")
            .execute(
                "UPDATE resources SET context_document_json = ?1, updated_at = ?2, version = 2
                 WHERE workspace_id = ?3 AND resource_id = ?4 AND version = 1",
                params![
                    String::from_utf8(
                        canonical_json(&json!({
                            "kind": "WORKSPACE_NOTES",
                            "owner_ref": {"kind": "WORKSPACE", "workspace_id": WORKSPACE},
                            "status": "ACTIVE",
                        }))
                        .expect("serialize ContextDocument metadata")
                    )
                    .expect("metadata is UTF-8"),
                    CONTEXT_ADDED_AT,
                    WORKSPACE,
                    resource_id,
                ],
            )
            .expect("attach active ContextDocument metadata");
        let index_digest =
            Connection::open(fixture.directory.path().join("state").join("state.sqlite3"))
                .expect("open fixture index database")
                .query_row(
                    "SELECT extracted_blob_digest FROM resource_text_indexes
                 WHERE workspace_id = ?1 AND resource_id = ?2 AND resource_revision_id = ?3",
                    params![WORKSPACE, resource_id, revision_id],
                    |row| row.get(0),
                )
                .expect("read ResourceIndex object identity");
        (content_digest, index_digest)
    }

    fn begin_command(
        resource_id: &str,
        request_id: &str,
        event_id: &str,
        time: &str,
    ) -> ContextDocumentPurgeStartCommand {
        ContextDocumentPurgeStartCommand {
            workspace_id: WORKSPACE.to_owned(),
            resource_id: resource_id.to_owned(),
            principal_id: OWNER.to_owned(),
            request_id: request_id.to_owned(),
            expected_version: 2,
            request_payload: json!({
                "workspace_id": WORKSPACE,
                "resource_id": resource_id,
                "status": "DELETION_PENDING",
                "expected_version": 2,
            }),
            event: event(event_id, time),
        }
    }

    fn create_open_pinned_upload(fixture: &PurgeFixture, resource_id: &str, revision_id: &str) {
        let upload_id = "upload-purge-blob-boundary";
        let expected_digest = digest(b"revision bytes awaiting upload");
        fixture
            .store
            .create_revision(
                WorkspaceCreateRequest {
                    principal_id: OWNER.to_owned(),
                    request_id: format!("request-create-{upload_id}"),
                    request_payload: json!({
                        "operation": "resource.revision.upload.create.v1",
                        "workspace_id": WORKSPACE,
                        "resource_id": resource_id,
                        "expected_resource_version": 2,
                        "parent_revision_ids": [revision_id],
                        "media_type": "text/plain",
                        "size_bytes": b"revision bytes awaiting upload".len(),
                        "expected_digest": expected_digest,
                    }),
                },
                ResourceUploadSessionRecord {
                    upload_id: upload_id.to_owned(),
                    workspace_id: WORKSPACE.to_owned(),
                    display_name: String::new(),
                    media_type: "text/plain".to_owned(),
                    expected_size_bytes: b"revision bytes awaiting upload".len() as u64,
                    expected_digest: Some(expected_digest.clone()),
                    context_document: None,
                    folder_import: None,
                    resource_id: Some(resource_id.to_owned()),
                    committed_resource_id: None,
                    expected_resource_version: Some(2),
                    parent_revision_ids: vec![revision_id.to_owned()],
                    chunk_size_bytes: 4_194_304,
                    received_ranges: Vec::new(),
                    next_missing_offset: 0,
                    state: ResourceUploadState::Open,
                    expires_at: "2099-01-01T00:00:00Z".to_owned(),
                    created_at: "2026-10-15T10:00:03Z".to_owned(),
                    version: 1,
                    progress_version: 1,
                },
                EventDraft {
                    event_id: format!("event-create-{upload_id}"),
                    workspace_id: WORKSPACE.to_owned(),
                    entity_type: "ResourceUpload".to_owned(),
                    entity_id: upload_id.to_owned(),
                    origin_runtime_id: "runtime-local".to_owned(),
                    entity_revision: 1,
                    hlc_timestamp: "2026-10-15T10:00:03Z".to_owned(),
                    correlation_id: format!("correlation-create-{upload_id}"),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "resource.upload.created.v1".to_owned(),
                    payload: json!({
                        "upload_id": upload_id,
                        "workspace_id": WORKSPACE,
                        "expected_size_bytes": b"revision bytes awaiting upload".len(),
                        "expected_digest": expected_digest,
                        "chunk_size_bytes": 4_194_304,
                        "expires_at": "2099-01-01T00:00:00Z",
                        "resource_id": resource_id,
                        "expected_resource_version": 2,
                        "parent_revision_ids": [revision_id],
                        "aggregate_version": 1,
                    }),
                    recorded_at: "2026-10-15T10:00:03Z".to_owned(),
                },
            )
            .expect("create a pinned open upload for the purge expiry path");
    }

    #[test]
    fn purge_start_acknowledgement_and_empty_completion_do_blob_io_outside_sql_transactions() {
        let fixture = fixture();
        let (content_digest, index_digest) = create_context_resource(
            &fixture,
            "resource-purge-blob-boundary-nonempty",
            "revision-purge-blob-boundary-nonempty",
            b"purge aggregate snapshot boundary",
        );
        create_open_pinned_upload(
            &fixture,
            "resource-purge-blob-boundary-nonempty",
            "revision-purge-blob-boundary-nonempty",
        );
        let calls_before_start = fixture.blobs.calls.load(Ordering::SeqCst);
        let start = begin_command(
            "resource-purge-blob-boundary-nonempty",
            "request-purge-blob-boundary-nonempty",
            "event-purge-blob-boundary-nonempty",
            "2026-10-15T10:00:03Z",
        );
        let first = fixture
            .store
            .begin_context_document_purge(start.clone())
            .expect("begin purge prepares its snapshots outside SQL transactions");
        let calls_after_first_start = fixture.blobs.calls.load(Ordering::SeqCst);
        assert!(
            calls_after_first_start >= calls_before_start + 4,
            "start prepares and verifies both the Resource and linked upload snapshots"
        );
        fixture
            .store
            .begin_context_document_purge(start)
            .expect("exact replay returns the committed purge result");
        assert_eq!(
            fixture.blobs.calls.load(Ordering::SeqCst),
            calls_after_first_start,
            "an exact start replay does not prepare duplicate snapshots"
        );
        assert_eq!(first.status.required_replica_count, 2);
        let upload_state: String =
            Connection::open(fixture.directory.path().join("state").join("state.sqlite3"))
                .expect("open the expired-upload fixture state")
                .query_row(
                    "SELECT state FROM resource_upload_sessions WHERE upload_id = ?1",
                    ["upload-purge-blob-boundary"],
                    |row| row.get(0),
                )
                .expect("read the upload lifecycle after purge admission");
        assert_eq!(upload_state, "EXPIRED");

        assert_eq!(
            fixture
                .store
                .collect_orphan_resource_content_blobs(GC_AT, 100)
                .expect("physically remove the exact unshared Resource object"),
            1
        );
        assert_eq!(
            fixture
                .store
                .collect_orphan_resource_index_blobs(GC_AT, 100)
                .expect("physically remove the exact unshared ResourceIndex object"),
            1
        );
        let targets = fixture
            .store
            .list_pending_context_document_purge_targets(100)
            .expect("list exact pending purge targets");
        assert_eq!(targets.len(), 2);
        for (index, target) in targets.iter().enumerate() {
            let calls_before_ack = fixture.blobs.calls.load(Ordering::SeqCst);
            assert!(
                fixture
                    .store
                    .acknowledge_context_document_purge_target_if_object_physically_removed(
                        AcknowledgeContextDocumentPurgeTargetCommand {
                            workspace_id: target.workspace_id.clone(),
                            resource_id: target.resource_id.clone(),
                            manifest_digest: target.manifest_digest.clone(),
                            target: target.target.clone(),
                            acknowledged_at: format!("2026-10-15T10:00:{:02}Z", index + 10),
                            event: event(
                                &format!("event-purge-boundary-ack-{index}"),
                                &format!("2026-10-15T10:00:{:02}Z", index + 10),
                            ),
                            completion_event: event(
                                &format!("event-purge-boundary-complete-{index}"),
                                &format!("2026-10-15T10:00:{:02}Z", index + 11),
                            ),
                        }
                    )
                    .expect("acknowledge the exact physically removed object")
            );
            assert!(
                fixture.blobs.calls.load(Ordering::SeqCst) >= calls_before_ack + 2,
                "each acknowledgement prepares and verifies its resulting Resource snapshot"
            );
        }
        let ack_call_count = fixture.blobs.calls.load(Ordering::SeqCst);
        assert!(
            fixture
                .store
                .acknowledge_context_document_purge_target_if_object_physically_removed(
                    AcknowledgeContextDocumentPurgeTargetCommand {
                        workspace_id: targets[0].workspace_id.clone(),
                        resource_id: targets[0].resource_id.clone(),
                        manifest_digest: targets[0].manifest_digest.clone(),
                        target: targets[0].target.clone(),
                        acknowledged_at: "2026-10-15T10:00:20Z".to_owned(),
                        event: event("event-purge-boundary-ack-replay", "2026-10-15T10:00:20Z"),
                        completion_event: event(
                            "event-purge-boundary-complete-replay",
                            "2026-10-15T10:00:21Z",
                        ),
                    }
                )
                .expect("exact acknowledgement replay is idempotent")
        );
        assert_eq!(
            fixture.blobs.calls.load(Ordering::SeqCst),
            ack_call_count,
            "an acknowledgement replay does not prepare another Resource snapshot"
        );

        let (_empty_content_digest, _empty_index_digest) = create_context_resource(
            &fixture,
            "resource-purge-blob-boundary-empty",
            "revision-purge-blob-boundary-empty",
            b"removed before the empty plan is sealed",
        );
        let connection =
            Connection::open(fixture.directory.path().join("state").join("state.sqlite3"))
                .expect("open empty-plan fixture database");
        connection
            .execute(
                "DELETE FROM resource_locations WHERE resource_id = ?1",
                ["resource-purge-blob-boundary-empty"],
            )
            .expect("remove the Resource location before sealing an empty plan");
        connection
            .execute(
                "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
                params![WORKSPACE, "resource-purge-blob-boundary-empty"],
            )
            .expect("remove the derived projection before sealing an empty plan");
        drop(connection);
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("remove the empty-plan Resource object");
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("remove the empty-plan ResourceIndex object");
        let resource_gets_before_empty_start = fixture.blobs.resource_gets.load(Ordering::SeqCst);
        let empty_start = fixture
            .store
            .begin_context_document_purge(begin_command(
                "resource-purge-blob-boundary-empty",
                "request-purge-blob-boundary-empty",
                "event-purge-blob-boundary-empty",
                "2026-10-15T10:00:03Z",
            ))
            .expect("seal the verified empty purge plan");
        assert_eq!(empty_start.status.required_replica_count, 0);
        assert_eq!(
            fixture.blobs.resource_gets.load(Ordering::SeqCst),
            resource_gets_before_empty_start + 1,
            "empty-plan admission probes the retained digest whose registry row was collected"
        );
        let resource_gets_before_empty_completion =
            fixture.blobs.resource_gets.load(Ordering::SeqCst);
        let calls_before_empty_completion = fixture.blobs.calls.load(Ordering::SeqCst);
        assert!(
            fixture
                .store
                .complete_empty_context_document_purge_plan_if_verified_empty(
                    WORKSPACE,
                    "resource-purge-blob-boundary-empty",
                    empty_start
                        .status
                        .manifest_digest
                        .as_deref()
                        .expect("empty plan digest"),
                    event(
                        "event-purge-blob-boundary-empty-complete",
                        "2026-10-15T10:00:04Z"
                    ),
                )
                .expect("complete the exact verified empty plan")
        );
        assert_eq!(
            fixture.blobs.resource_gets.load(Ordering::SeqCst),
            resource_gets_before_empty_completion + 1,
            "empty-plan completion repeats the exact retained-digest absence probe"
        );
        let empty_completion_calls = fixture.blobs.calls.load(Ordering::SeqCst);
        assert!(
            empty_completion_calls >= calls_before_empty_completion + 2,
            "empty-plan completion prepares and verifies its resulting Resource snapshot"
        );
        assert!(
            fixture
                .store
                .complete_empty_context_document_purge_plan_if_verified_empty(
                    WORKSPACE,
                    "resource-purge-blob-boundary-empty",
                    empty_start
                        .status
                        .manifest_digest
                        .as_deref()
                        .expect("empty plan digest"),
                    event(
                        "event-purge-blob-boundary-empty-replay",
                        "2026-10-15T10:00:05Z"
                    ),
                )
                .expect("empty completion replay is idempotent")
        );
        assert_eq!(
            fixture.blobs.calls.load(Ordering::SeqCst),
            empty_completion_calls,
            "an already-completed empty plan does not prepare another snapshot"
        );

        assert_eq!(
            fixture.blobs.violations.load(Ordering::SeqCst),
            0,
            "every BlobStore put/get must run while a competing BEGIN IMMEDIATE can succeed"
        );
        assert!(fixture.blobs.calls.load(Ordering::SeqCst) > 0);
        assert!(!content_digest.is_empty());
        assert!(!index_digest.is_empty());
    }

    #[test]
    fn purge_start_rejects_stale_preflight_and_leaves_only_an_unreferenced_snapshot() {
        let fixture = fixture();
        let (content_digest, _index_digest) = create_context_resource(
            &fixture,
            "resource-purge-blob-boundary-race",
            "revision-purge-blob-boundary-race",
            b"stale purge preparation must not commit",
        );
        fixture
            .blobs
            .arm_resource_version_race("resource-purge-blob-boundary-race", 2);
        let result = fixture.store.begin_context_document_purge(begin_command(
            "resource-purge-blob-boundary-race",
            "request-purge-blob-boundary-race",
            "event-purge-blob-boundary-race",
            "2026-10-15T10:00:04Z",
        ));
        assert!(matches!(result, Err(StoreError::Conflict { .. })));

        let connection =
            Connection::open(fixture.directory.path().join("state").join("state.sqlite3"))
                .expect("inspect the rejected stale purge");
        let (version, status): (i64, String) = connection
            .query_row(
                "SELECT version, json_extract(context_document_json, '$.status')
                 FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
                params![WORKSPACE, "resource-purge-blob-boundary-race"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read the concurrently changed Resource");
        assert_eq!(version, 3);
        assert_eq!(status, "ACTIVE");
        let plans: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
                params![WORKSPACE, "resource-purge-blob-boundary-race"],
                |row| row.get(0),
            )
            .expect("count purge plans");
        let purge_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2
                   AND type LIKE 'resource.context_document.purge.%'",
                params![WORKSPACE, "resource-purge-blob-boundary-race"],
                |row| row.get(0),
            )
            .expect("count purge events");
        let purge_receipts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
                params![OWNER, "request-purge-blob-boundary-race"],
                |row| row.get(0),
            )
            .expect("count purge idempotency receipts");
        let snapshot_digest = fixture
            .blobs
            .last_aggregate_state_digest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("stale preflight reached aggregate snapshot preparation");
        let referenced_snapshots: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE aggregate_state_ref_json LIKE ?1",
                [format!("%{snapshot_digest}%")],
                |row| row.get(0),
            )
            .expect("check whether the prepared snapshot was referenced");

        assert_eq!(plans, 0);
        assert_eq!(purge_events, 0);
        assert_eq!(purge_receipts, 0);
        assert_eq!(referenced_snapshots, 0);
        assert_ne!(snapshot_digest, content_digest);
        assert_eq!(fixture.blobs.violations.load(Ordering::SeqCst), 0);
        assert!(fixture.blobs.calls.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn purge_acknowledgement_rejects_stale_preflight_without_receipt_or_event() {
        let fixture = fixture();
        let (content_digest, _) = create_context_resource(
            &fixture,
            "resource-purge-ack-boundary-race",
            "revision-purge-ack-boundary-race",
            b"stale acknowledgement preparation must not commit",
        );
        let purge = fixture
            .store
            .begin_context_document_purge(begin_command(
                "resource-purge-ack-boundary-race",
                "request-purge-ack-boundary-race",
                "event-purge-ack-boundary-race",
                "2026-10-15T10:00:03Z",
            ))
            .expect("admit the purge before acknowledgement");
        assert_eq!(purge.status.required_replica_count, 2);
        assert_eq!(
            fixture
                .store
                .collect_orphan_resource_content_blobs(GC_AT, 100)
                .expect("remove the exact Resource content object"),
            1
        );
        assert_eq!(
            fixture
                .store
                .collect_orphan_resource_index_blobs(GC_AT, 100)
                .expect("remove the exact ResourceIndex object"),
            1
        );
        let target = fixture
            .store
            .list_pending_context_document_purge_targets(100)
            .expect("list the durable purge targets")
            .into_iter()
            .next()
            .expect("the purge has a pending target");
        fixture
            .blobs
            .arm_resource_version_race(&target.resource_id, 3);

        let result = fixture
            .store
            .acknowledge_context_document_purge_target_if_object_physically_removed(
                AcknowledgeContextDocumentPurgeTargetCommand {
                    workspace_id: target.workspace_id.clone(),
                    resource_id: target.resource_id.clone(),
                    manifest_digest: target.manifest_digest.clone(),
                    target: target.target.clone(),
                    acknowledged_at: "2026-10-15T10:00:10Z".to_owned(),
                    event: event("event-purge-ack-boundary-race", "2026-10-15T10:00:10Z"),
                    completion_event: event(
                        "event-purge-ack-boundary-race-complete",
                        "2026-10-15T10:00:11Z",
                    ),
                },
            );
        assert!(matches!(result, Ok(false)));

        let connection =
            Connection::open(fixture.directory.path().join("state").join("state.sqlite3"))
                .expect("inspect rejected stale acknowledgement");
        let (version, status): (i64, String) = connection
            .query_row(
                "SELECT version, json_extract(context_document_json, '$.status')
                 FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
                params![WORKSPACE, target.resource_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read the concurrently changed Resource");
        assert_eq!((version, status.as_str()), (4, "DELETION_PENDING"));
        let receipt_status: String = connection
            .query_row(
                "SELECT status FROM context_document_purge_receipts
                 WHERE workspace_id = ?1 AND resource_id = ?2 AND replica_ref = ?3 AND replica_kind = ?4",
                params![
                    WORKSPACE,
                    target.resource_id,
                    target.target.replica_ref,
                    target.target.replica_kind.as_str(),
                ],
                |row| row.get(0),
            )
            .expect("read the pending purge receipt");
        assert_eq!(receipt_status, "PENDING");
        let acknowledgement_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2
                   AND type = 'resource.context_document.purge.acknowledged.v2'",
                params![WORKSPACE, target.resource_id],
                |row| row.get(0),
            )
            .expect("count purge acknowledgement events");
        assert_eq!(acknowledgement_events, 0);
        let snapshot_digest = fixture
            .blobs
            .last_aggregate_state_digest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("stale acknowledgement reached snapshot preparation");
        let referenced_snapshots: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE aggregate_state_ref_json LIKE ?1",
                [format!("%{snapshot_digest}%")],
                |row| row.get(0),
            )
            .expect("check whether the prepared snapshot was referenced");
        assert_eq!(referenced_snapshots, 0);
        assert_ne!(snapshot_digest, content_digest);
        assert_eq!(fixture.blobs.violations.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn empty_purge_completion_rejects_stale_preflight_without_deletion_event() {
        let fixture = fixture();
        create_context_resource(
            &fixture,
            "resource-purge-empty-boundary-race",
            "revision-purge-empty-boundary-race",
            b"stale empty completion must not commit",
        );
        let connection =
            Connection::open(fixture.directory.path().join("state").join("state.sqlite3"))
                .expect("open empty-plan fixture database");
        connection
            .execute(
                "DELETE FROM resource_locations WHERE resource_id = ?1",
                ["resource-purge-empty-boundary-race"],
            )
            .expect("remove the Resource location before sealing an empty plan");
        connection
            .execute(
                "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
                params![WORKSPACE, "resource-purge-empty-boundary-race"],
            )
            .expect("remove the derived projection before sealing an empty plan");
        drop(connection);
        assert_eq!(
            fixture
                .store
                .collect_orphan_resource_content_blobs(GC_AT, 100)
                .expect("remove the empty-plan Resource object"),
            1
        );
        assert_eq!(
            fixture
                .store
                .collect_orphan_resource_index_blobs(GC_AT, 100)
                .expect("remove the empty-plan ResourceIndex object"),
            1
        );
        let purge = fixture
            .store
            .begin_context_document_purge(begin_command(
                "resource-purge-empty-boundary-race",
                "request-purge-empty-boundary-race",
                "event-purge-empty-boundary-race",
                "2026-10-15T10:00:03Z",
            ))
            .expect("seal the verified empty plan");
        assert_eq!(purge.status.required_replica_count, 0);
        let manifest_digest = purge
            .status
            .manifest_digest
            .expect("empty plan has a stable manifest digest");
        fixture
            .blobs
            .arm_resource_version_race("resource-purge-empty-boundary-race", 3);

        let result = fixture
            .store
            .complete_empty_context_document_purge_plan_if_verified_empty(
                WORKSPACE,
                "resource-purge-empty-boundary-race",
                &manifest_digest,
                event(
                    "event-purge-empty-boundary-race-complete",
                    "2026-10-15T10:00:10Z",
                ),
            );
        assert!(matches!(result, Ok(false)));

        let connection =
            Connection::open(fixture.directory.path().join("state").join("state.sqlite3"))
                .expect("inspect rejected stale empty completion");
        let (version, status): (i64, String) = connection
            .query_row(
                "SELECT version, json_extract(context_document_json, '$.status')
                 FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
                params![WORKSPACE, "resource-purge-empty-boundary-race"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read the concurrently changed Resource");
        assert_eq!((version, status.as_str()), (4, "DELETION_PENDING"));
        let deletion_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2
                   AND type = 'resource.context_document.status.changed.v1'
                   AND json_extract(payload_json, '$.to') = 'DELETED'",
                params![WORKSPACE, "resource-purge-empty-boundary-race"],
                |row| row.get(0),
            )
            .expect("count empty-plan deletion events");
        assert_eq!(deletion_events, 0);
        let snapshot_digest = fixture
            .blobs
            .last_aggregate_state_digest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("stale empty completion reached snapshot preparation");
        let referenced_snapshots: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE aggregate_state_ref_json LIKE ?1",
                [format!("%{snapshot_digest}%")],
                |row| row.get(0),
            )
            .expect("check whether the prepared snapshot was referenced");
        assert_eq!(referenced_snapshots, 0);
        assert_eq!(fixture.blobs.violations.load(Ordering::SeqCst), 0);
    }
}
