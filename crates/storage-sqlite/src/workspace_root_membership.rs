//! Canonical FILE Resource and ResourceLocation materialization for METADATA scans.
//!
//! This module only stages canonical metadata and Runtime-local bindings. It never reads
//! file bytes, creates a ResourceRevision, infers deletion, or publishes private identity
//! or locator data. The caller must invoke it inside the existing fenced scan transaction.

use rusqlite::{OptionalExtension, Transaction, params};
use storage_core::{
    LocalFileIdentityBindingRecord, LocalResourceLocationBindingRecord, ResourceLocationRecord,
    ResourceRecord, StoreError, WorkspaceRootFileObservation,
};

pub(crate) const MAX_ROOT_FILE_CLEANUP_BATCH: usize = 250;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceRootCleanupLocation {
    pub location_id: String,
    pub resource_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceRootCleanupBatch {
    pub workspace_id: String,
    pub workspace_root_id: String,
    pub after_location_id: Option<String>,
    pub expected_version: u64,
    pub locations: Vec<WorkspaceRootCleanupLocation>,
    pub has_more: bool,
}

pub(crate) struct MembershipMutation {
    pub resource_created: bool,
    pub location_created_or_rebound: bool,
    pub observation: WorkspaceRootFileObservation,
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub locator_binding: LocalResourceLocationBindingRecord,
    pub file_identity_binding: LocalFileIdentityBindingRecord,
}

/// Store-local view of the runtime-only Core candidate. It remains private to the
/// SQLite adapter so raw identity and locator values cannot acquire a Debug/Serde path.
pub(crate) struct WorkspaceRootFileMembershipCandidate {
    pub observation: WorkspaceRootFileObservation,
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub locator_binding: LocalResourceLocationBindingRecord,
    pub file_identity_binding: LocalFileIdentityBindingRecord,
}

/// Reuses a FILE Resource by Workspace-scoped keyed FileIdentity and materializes one
/// path-specific location. Call only after the enclosing transaction has validated the
/// Workspace owner, active root/version, Runtime incarnation, and scan receipt.
pub(crate) fn reconcile_file_membership(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    workspace_root_id: &str,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    candidate: WorkspaceRootFileMembershipCandidate,
) -> Result<MembershipMutation, StoreError> {
    let root_is_active: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM workspace_roots
               WHERE workspace_id = ?1 AND workspace_root_id = ?2 AND status = 'ACTIVE'
             )",
            params![workspace_id, workspace_root_id],
            |row| row.get(0),
        )
        .map_err(map_membership_database_error)?;
    if !root_is_active {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    validate_candidate(
        workspace_id,
        workspace_root_id,
        runtime_id,
        runtime_incarnation_id,
        &candidate,
    )?;

    let observation = &candidate.observation;
    let resource = &candidate.resource;
    let location = &candidate.location;
    let existing_resource: Option<(String, String)> = transaction
        .query_row(
            "SELECT resource_id, kind FROM resources
             WHERE workspace_id = ?1 AND identity_digest = ?2",
            params![workspace_id, observation.file_identity_digest],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_membership_database_error)?;

    let (resource_id, resource_created) = match existing_resource {
        Some((resource_id, kind)) if kind == "FILE" => (resource_id, false),
        Some(_) => {
            return Err(StoreError::Integrity(
                "FileIdentity is bound to a non-FILE Resource".to_owned(),
            ));
        }
        None => {
            transaction
                .execute(
                    "INSERT INTO resources(
                       resource_id, workspace_id, kind, provider_identity_json,
                       identity_digest, display_name, current_revision_id, sensitivity,
                       context_document_json, provenance_json, created_at, updated_at, version
                     ) VALUES (?1, ?2, 'FILE', ?3, ?4, ?5, NULL, ?6, NULL, ?7, ?8, ?8, 1)",
                    params![
                        resource.resource_id,
                        workspace_id,
                        String::from_utf8(super::super::canonical_json(
                            &resource.provider_identity,
                        )?)
                        .map_err(|error| StoreError::Invalid(error.to_string()))?,
                        observation.file_identity_digest,
                        resource.display_name,
                        resource.sensitivity,
                        String::from_utf8(super::super::canonical_json(&resource.provenance)?)
                            .map_err(|error| StoreError::Invalid(error.to_string()))?,
                        observation.observed_at,
                    ],
                )
                .map_err(map_membership_database_error)?;
            (resource.resource_id.clone(), true)
        }
    };

    let old_location: Option<(String, Option<String>, Option<String>, String, String)> =
        transaction
            .query_row(
                "SELECT resource_id, runtime_id, provider_ref, locator_ref_id, availability
             FROM resource_locations WHERE location_id = ?1",
                [&location.location_id],
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
            .map_err(map_membership_database_error)?;
    if old_location
        .as_ref()
        .is_some_and(|(old_resource_id, _, provider, locator, _)| {
            old_resource_id != &resource_id
                || provider.as_deref() != Some("litecowork.local_filesystem")
                || locator != &location.locator_ref_id
        })
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    // A root membership row is a revocation/cleanup index, not a cache that may be
    // repaired by silently rebinding it. Detect an inconsistent existing relation
    // before replacing either private identity binding below.
    let old_root_relation: Option<(String, String)> = transaction
        .query_row(
            "SELECT workspace_id, resource_id FROM workspace_root_file_locations
             WHERE workspace_root_id = ?1 AND location_id = ?2",
            params![workspace_root_id, location.location_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_membership_database_error)?;
    if old_root_relation
        .as_ref()
        .is_some_and(|(old_workspace_id, old_resource_id)| {
            old_workspace_id != workspace_id || old_resource_id != &resource_id
        })
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let location_created_or_rebound =
        old_location
            .as_ref()
            .is_none_or(|(old_resource_id, old_runtime, _, _, availability)| {
                old_resource_id != &resource_id
                    || old_runtime.as_deref() != Some(runtime_id)
                    || availability != "AVAILABLE"
            });
    let mut resolved_location = location.clone();
    resolved_location.resource_id = resource_id.clone();
    transaction
        .execute(
            "INSERT INTO resource_locations(
               location_id, resource_id, runtime_id, environment_id, connection_id,
               provider_ref, locator_ref_id, availability, writable,
               observed_revision_id, observed_digest, observed_at, last_checked_at
             ) VALUES (?1, ?2, ?3, NULL, NULL, 'litecowork.local_filesystem', ?4,
                       'AVAILABLE', 0, NULL, NULL, ?5, ?5)
             ON CONFLICT(location_id) DO UPDATE SET
               resource_id = excluded.resource_id,
               runtime_id = excluded.runtime_id,
               availability = 'AVAILABLE', writable = 0,
               observed_revision_id = NULL, observed_digest = NULL,
               observed_at = excluded.observed_at, last_checked_at = excluded.last_checked_at
             WHERE resource_locations.provider_ref = excluded.provider_ref
               AND resource_locations.locator_ref_id = excluded.locator_ref_id",
            params![
                location.location_id,
                resource_id,
                runtime_id,
                location.locator_ref_id,
                observation.observed_at,
            ],
        )
        .map_err(map_membership_database_error)?;
    let canonical_location: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT resource_id, runtime_id, locator_ref_id FROM resource_locations
             WHERE location_id = ?1",
            [&location.location_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_membership_database_error)?;
    if canonical_location.as_ref()
        != Some(&(
            resource_id.clone(),
            runtime_id.to_owned(),
            location.locator_ref_id.clone(),
        ))
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }

    // Bindings are replaced only after the new canonical location has passed the current
    // incarnation trigger. Raw values and locator bytes are confined to these tables.
    transaction
        .execute(
            "DELETE FROM resource_location_bindings WHERE location_id = ?1",
            [&location.location_id],
        )
        .map_err(map_membership_database_error)?;
    transaction
        .execute(
            "DELETE FROM file_identity_bindings WHERE location_id = ?1",
            [&location.location_id],
        )
        .map_err(map_membership_database_error)?;
    transaction
        .execute(
            "INSERT INTO resource_location_bindings(
               location_id, locator_ref_id, runtime_id, runtime_incarnation_id,
               private_locator, observed_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                location.location_id,
                location.locator_ref_id,
                runtime_id,
                runtime_incarnation_id,
                candidate.locator_binding.private_locator,
                observation.observed_at,
            ],
        )
        .map_err(map_membership_database_error)?;
    transaction
        .execute(
            "INSERT INTO file_identity_bindings(
               location_id, runtime_id, runtime_incarnation_id,
               raw_filesystem_instance_id, raw_volume_id, raw_file_id,
               raw_generation, platform_kind, observed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                location.location_id,
                runtime_id,
                runtime_incarnation_id,
                candidate.file_identity_binding.raw_filesystem_instance_id,
                candidate.file_identity_binding.raw_volume_id,
                candidate.file_identity_binding.raw_file_id,
                candidate.file_identity_binding.raw_generation,
                candidate.file_identity_binding.platform_kind,
                observation.observed_at,
            ],
        )
        .map_err(map_membership_database_error)?;

    // This explicit relation lets revocation synchronously remove all private bindings
    // without parsing or scanning private locator strings. The root status transaction
    // will use it to enqueue bounded public-projection cleanup.
    transaction
        .execute(
            "INSERT INTO workspace_root_file_locations(
               workspace_id, workspace_root_id, location_id, resource_id, observed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workspace_root_id, location_id) DO UPDATE SET
               resource_id = excluded.resource_id,
               observed_at = excluded.observed_at
             WHERE workspace_root_file_locations.workspace_id = excluded.workspace_id",
            params![
                workspace_id,
                workspace_root_id,
                location.location_id,
                resource_id,
                observation.observed_at,
            ],
        )
        .map_err(map_membership_database_error)?;
    let relation: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT workspace_id, resource_id, observed_at
             FROM workspace_root_file_locations
             WHERE workspace_root_id = ?1 AND location_id = ?2",
            params![workspace_root_id, location.location_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_membership_database_error)?;
    if relation.as_ref()
        != Some(&(
            workspace_id.to_owned(),
            resource_id.clone(),
            observation.observed_at.clone(),
        ))
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }

    let mut resolved_observation = candidate.observation;
    resolved_observation.resource_id = Some(resource_id);
    let resolved_resource = super::super::load_resource_record(
        transaction,
        workspace_id,
        &resolved_location.resource_id,
    )?
    .ok_or_else(|| StoreError::Integrity("FILE Resource disappeared during scan".to_owned()))?;
    Ok(MembershipMutation {
        resource_created,
        location_created_or_rebound,
        observation: resolved_observation,
        resource: resolved_resource,
        location: resolved_location,
        locator_binding: candidate.locator_binding,
        file_identity_binding: candidate.file_identity_binding,
    })
}

fn validate_candidate(
    workspace_id: &str,
    workspace_root_id: &str,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    candidate: &WorkspaceRootFileMembershipCandidate,
) -> Result<(), StoreError> {
    let observation = &candidate.observation;
    let resource = &candidate.resource;
    let location = &candidate.location;
    let locator = &candidate.locator_binding;
    let identity = &candidate.file_identity_binding;
    if workspace_id.is_empty()
        || workspace_root_id.is_empty()
        || runtime_id.is_empty()
        || runtime_incarnation_id.is_empty()
        || observation.observation_id.is_empty()
        || !storage_core::is_sha256_digest(&observation.file_identity_digest)
        || observation.display_name.is_empty()
        || observation.display_name.contains('\0')
        || observation.private_relative_locator.is_empty()
        || observation.private_relative_locator.len() > 32 * 1024
        || i64::try_from(observation.size_bytes).is_err()
        || resource.workspace_id != workspace_id
        || resource.kind != "FILE"
        || resource.identity_digest.as_deref() != Some(observation.file_identity_digest.as_str())
        || observation.resource_id.as_deref() != Some(resource.resource_id.as_str())
        || resource.current_revision_id.is_some()
        || resource
            .provider_identity
            .get("provider_instance_id")
            .and_then(serde_json::Value::as_str)
            != Some("litecowork.local_filesystem")
        || resource
            .provider_identity
            .get("stable_object_id")
            .and_then(serde_json::Value::as_str)
            != Some(observation.file_identity_digest.as_str())
        || resource
            .provider_identity
            .get("confidence")
            .and_then(serde_json::Value::as_str)
            != Some("FILESYSTEM_ID")
        || location.resource_id != resource.resource_id
        || location.runtime_id != runtime_id
        || location.provider_ref != "litecowork.local_filesystem"
        || location.availability != "AVAILABLE"
        || location.writable
        || location.observed_revision_id.is_some()
        || location.observed_digest.is_some()
        || location.observed_at != observation.observed_at
        || location.location_id.is_empty()
        || location.locator_ref_id.is_empty()
        || locator.location_id != location.location_id
        || locator.locator_ref_id != location.locator_ref_id
        || locator.runtime_id != runtime_id
        || locator.runtime_incarnation_id != runtime_incarnation_id
        || locator.private_locator.is_empty()
        || locator.observed_at != observation.observed_at
        || identity.location_id != location.location_id
        || identity.runtime_id != runtime_id
        || identity.runtime_incarnation_id != runtime_incarnation_id
        || identity.raw_filesystem_instance_id.trim().is_empty()
        || identity.raw_file_id.trim().is_empty()
        || identity.platform_kind.trim().is_empty()
        || identity.observed_at != observation.observed_at
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot FILE membership proposal is invalid".to_owned(),
        ));
    }
    Ok(())
}

/// Enqueues idempotent cleanup after the caller has committed the immediate REVOKED policy
/// fence in the same transaction. Root-linked rows remain available to deny reads until
/// each bounded cleanup batch terminally revokes its ResourceLocation.
pub(crate) fn enqueue_workspace_root_file_cleanup(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    workspace_root_id: &str,
    now: &str,
) -> Result<(), StoreError> {
    if workspace_id.trim().is_empty()
        || workspace_root_id.trim().is_empty()
        || now.trim().is_empty()
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot cleanup identity is invalid".to_owned(),
        ));
    }
    let status: Option<String> = transaction
        .query_row(
            "SELECT status FROM workspace_roots
             WHERE workspace_id = ?1 AND workspace_root_id = ?2",
            params![workspace_id, workspace_root_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_membership_database_error)?;
    if status.as_deref() != Some("REVOKED") {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    transaction
        .execute(
            "INSERT INTO workspace_root_cleanup_outbox(
               workspace_id, workspace_root_id, after_location_id, status,
               created_at, updated_at, version
             ) VALUES (?1, ?2, NULL, 'PENDING', ?3, ?3, 1)
             ON CONFLICT(workspace_root_id) DO NOTHING",
            params![workspace_id, workspace_root_id, now],
        )
        .map_err(map_membership_database_error)?;
    let existing: Option<(String, String)> = transaction
        .query_row(
            "SELECT workspace_id, status FROM workspace_root_cleanup_outbox
             WHERE workspace_root_id = ?1",
            [workspace_root_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_membership_database_error)?;
    if existing.as_ref().is_none_or(|(stored_workspace, status)| {
        stored_workspace != workspace_id || !matches!(status.as_str(), "PENDING" | "COMPLETE")
    }) {
        return Err(StoreError::Integrity(
            "WorkspaceRoot cleanup outbox identity is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

/// Removes every Runtime-incarnation private locator and raw identity binding for a
/// revoked root's indexed FILE locations, then ensures restart-safe public projection
/// cleanup is queued. The caller performs this in the same transaction as the root's
/// ACTIVE/PAUSED/UNAVAILABLE -> REVOKED policy transition.
pub(crate) fn fence_revoked_workspace_root_file_locations(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    workspace_root_id: &str,
    now: &str,
) -> Result<(), StoreError> {
    enqueue_workspace_root_file_cleanup(transaction, workspace_id, workspace_root_id, now)?;
    transaction
        .execute(
            "DELETE FROM resource_location_bindings
             WHERE location_id IN (
               SELECT location_id FROM workspace_root_file_locations
               WHERE workspace_id = ?1 AND workspace_root_id = ?2
             )",
            params![workspace_id, workspace_root_id],
        )
        .map_err(map_membership_database_error)?;
    transaction
        .execute(
            "DELETE FROM file_identity_bindings
             WHERE location_id IN (
               SELECT location_id FROM workspace_root_file_locations
               WHERE workspace_id = ?1 AND workspace_root_id = ?2
             )",
            params![workspace_id, workspace_root_id],
        )
        .map_err(map_membership_database_error)?;
    Ok(())
}

/// Resolves whether a FILE location remains within an ACTIVE root. A linked location is
/// denied if its root is missing, PAUSED, UNAVAILABLE, or REVOKED; unrelated locations do
/// not inherit this root's policy.
pub(crate) fn root_linked_file_location_is_readable(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    resource_id: &str,
    location_id: &str,
) -> Result<bool, StoreError> {
    transaction
        .query_row(
            "SELECT NOT EXISTS(
               SELECT 1
               FROM workspace_root_file_locations member
               LEFT JOIN workspace_roots root
                 ON root.workspace_id = member.workspace_id
                AND root.workspace_root_id = member.workspace_root_id
               WHERE member.workspace_id = ?1
                 AND member.resource_id = ?2
                 AND member.location_id = ?3
                 AND (root.workspace_root_id IS NULL OR root.status <> 'ACTIVE')
             )",
            params![workspace_id, resource_id, location_id],
            |row| row.get(0),
        )
        .map_err(map_membership_database_error)
}

/// Reads one stable, bounded cleanup batch inside the caller's IMMEDIATE transaction.
/// The batch includes one lookahead member so `has_more` is exact without unbounded scans.
pub(crate) fn next_workspace_root_file_cleanup_batch(
    transaction: &Transaction<'_>,
    workspace_root_id: &str,
    limit: usize,
) -> Result<Option<WorkspaceRootCleanupBatch>, StoreError> {
    if limit == 0 || limit > MAX_ROOT_FILE_CLEANUP_BATCH {
        return Err(StoreError::Invalid(
            "WorkspaceRoot cleanup batch size is outside its bound".to_owned(),
        ));
    }
    let outbox: Option<(String, Option<String>, String, i64)> = transaction
        .query_row(
            "SELECT workspace_id, after_location_id, status, version
             FROM workspace_root_cleanup_outbox WHERE workspace_root_id = ?1",
            [workspace_root_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(map_membership_database_error)?;
    let Some((workspace_id, after_location_id, status, version)) = outbox else {
        return Ok(None);
    };
    if status == "COMPLETE" {
        return Ok(None);
    }
    if status != "PENDING" || version <= 0 {
        return Err(StoreError::Integrity(
            "WorkspaceRoot cleanup cursor is malformed".to_owned(),
        ));
    }
    let sql_limit = i64::try_from(limit + 1)
        .map_err(|_| StoreError::Invalid("cleanup batch size is invalid".to_owned()))?;
    let mut statement = transaction
        .prepare(
            "SELECT location_id, resource_id
             FROM workspace_root_file_locations
             WHERE workspace_id = ?1 AND workspace_root_id = ?2
               AND (?3 IS NULL OR location_id > ?3)
             ORDER BY location_id LIMIT ?4",
        )
        .map_err(map_membership_database_error)?;
    let mut locations = statement
        .query_map(
            params![
                workspace_id,
                workspace_root_id,
                after_location_id,
                sql_limit
            ],
            |row| {
                Ok(WorkspaceRootCleanupLocation {
                    location_id: row.get(0)?,
                    resource_id: row.get(1)?,
                })
            },
        )
        .map_err(map_membership_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_membership_database_error)?;
    let has_more = locations.len() > limit;
    if has_more {
        locations.pop();
    }
    Ok(Some(WorkspaceRootCleanupBatch {
        workspace_id,
        workspace_root_id: workspace_root_id.to_owned(),
        after_location_id,
        expected_version: u64::try_from(version).map_err(|_| {
            StoreError::Integrity("WorkspaceRoot cleanup version is invalid".to_owned())
        })?,
        locations,
        has_more,
    }))
}

/// Finishes a batch only after each location has its terminal availability, its matching
/// Resource location event, and no private locator or raw identity binding. Cursor advance,
/// relation removal and completion are committed with those transitions by the caller.
pub(crate) fn commit_workspace_root_file_cleanup_batch(
    transaction: &Transaction<'_>,
    batch: &WorkspaceRootCleanupBatch,
    now: &str,
) -> Result<(), StoreError> {
    if batch.workspace_id.trim().is_empty()
        || batch.workspace_root_id.trim().is_empty()
        || now.trim().is_empty()
        || batch.locations.len() > MAX_ROOT_FILE_CLEANUP_BATCH
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot cleanup batch is invalid".to_owned(),
        ));
    }
    let version = i64::try_from(batch.expected_version)
        .map_err(|_| StoreError::Invalid("cleanup version is invalid".to_owned()))?;
    if version <= 0 {
        return Err(StoreError::Invalid(
            "cleanup version must be positive".to_owned(),
        ));
    }
    let mut prior_cursor = batch.after_location_id.as_deref();
    for location in &batch.locations {
        if location.location_id.trim().is_empty()
            || location.resource_id.trim().is_empty()
            || prior_cursor.is_some_and(|cursor| location.location_id.as_str() <= cursor)
        {
            return Err(StoreError::Integrity(
                "WorkspaceRoot cleanup batch order is invalid".to_owned(),
            ));
        }
        prior_cursor = Some(&location.location_id);
        let state: Option<(String, i64, i64, i64)> = transaction
            .query_row(
                "SELECT availability,
                        (SELECT COUNT(*) FROM resource_location_bindings b WHERE b.location_id = l.location_id),
                        (SELECT COUNT(*) FROM file_identity_bindings f WHERE f.location_id = l.location_id),
                        (SELECT COUNT(*) FROM domain_events e
                         WHERE e.entity_type = 'Resource' AND e.entity_id = l.resource_id
                           AND e.type = 'resource.location.changed.v1'
                           AND json_extract(e.payload_json, '$.location_id') = l.location_id
                           AND json_extract(e.payload_json, '$.availability') = 'REVOKED')
                 FROM resource_locations l
                 WHERE l.location_id = ?1 AND l.resource_id = ?2",
                params![location.location_id, location.resource_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(map_membership_database_error)?;
        if state
            .as_ref()
            .is_none_or(|(availability, locators, identities, events)| {
                availability != "REVOKED" || *locators != 0 || *identities != 0 || *events <= 0
            })
        {
            return Err(StoreError::Integrity(
                "WorkspaceRoot cleanup location is not durably revoked".to_owned(),
            ));
        }
        let removed = transaction
            .execute(
                "DELETE FROM workspace_root_file_locations
                 WHERE workspace_id = ?1 AND workspace_root_id = ?2
                   AND location_id = ?3 AND resource_id = ?4",
                params![
                    batch.workspace_id,
                    batch.workspace_root_id,
                    location.location_id,
                    location.resource_id
                ],
            )
            .map_err(map_membership_database_error)?;
        if removed != 1 {
            return Err(StoreError::Conflict {
                expected: Some(batch.expected_version),
                actual: None,
            });
        }
    }
    let after_location_id = batch
        .locations
        .last()
        .map(|location| location.location_id.as_str())
        .or(batch.after_location_id.as_deref());
    let remaining: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM workspace_root_file_locations
               WHERE workspace_id = ?1 AND workspace_root_id = ?2
                 AND (?3 IS NULL OR location_id > ?3)
             )",
            params![
                batch.workspace_id,
                batch.workspace_root_id,
                after_location_id
            ],
            |row| row.get(0),
        )
        .map_err(map_membership_database_error)?;
    if remaining != batch.has_more {
        return Err(StoreError::Conflict {
            expected: Some(batch.expected_version),
            actual: None,
        });
    }
    let status = if remaining { "PENDING" } else { "COMPLETE" };
    let changed = transaction
        .execute(
            "UPDATE workspace_root_cleanup_outbox
             SET after_location_id = ?1, status = ?2, updated_at = ?3, version = version + 1
             WHERE workspace_root_id = ?4 AND workspace_id = ?5
               AND after_location_id IS ?6 AND version = ?7 AND version < 9223372036854775807
               AND status = 'PENDING'",
            params![
                after_location_id,
                status,
                now,
                batch.workspace_root_id,
                batch.workspace_id,
                batch.after_location_id,
                version,
            ],
        )
        .map_err(map_membership_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(batch.expected_version),
            actual: None,
        });
    }
    Ok(())
}

fn map_membership_database_error(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::SqliteFailure(ref failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Conflict {
                expected: None,
                actual: None,
            }
        }
        other => StoreError::Invalid(format!("WorkspaceRoot membership database error: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    const WORKSPACE: &str = "workspace-membership";
    const ROOT: &str = "root-membership";
    const RUNTIME: &str = "runtime-membership";
    const INCARNATION: &str = "incarnation-membership";
    const NOW: &str = "2026-10-17T12:00:00Z";

    fn schema(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE resources (
                   resource_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL,
                   kind TEXT NOT NULL, provider_identity_json TEXT NOT NULL,
                   identity_digest TEXT, display_name TEXT NOT NULL,
                   current_revision_id TEXT, sensitivity TEXT NOT NULL,
                   context_document_json TEXT, provenance_json TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   version INTEGER NOT NULL,
                   UNIQUE(workspace_id, identity_digest)
                 );
                 CREATE TABLE resource_locations (
                   location_id TEXT PRIMARY KEY, resource_id TEXT NOT NULL,
                   runtime_id TEXT, environment_id TEXT, connection_id TEXT,
                   provider_ref TEXT, locator_ref_id TEXT NOT NULL,
                   availability TEXT NOT NULL, writable INTEGER NOT NULL,
                   observed_revision_id TEXT, observed_digest TEXT,
                   observed_at TEXT NOT NULL, last_checked_at TEXT
                 );
                 CREATE TABLE resource_location_bindings (
                   location_id TEXT NOT NULL, locator_ref_id TEXT NOT NULL,
                   runtime_id TEXT NOT NULL, runtime_incarnation_id TEXT NOT NULL,
                   private_locator TEXT NOT NULL, observed_at TEXT NOT NULL,
                   expires_at TEXT, PRIMARY KEY(location_id, runtime_incarnation_id)
                 );
                 CREATE TABLE file_identity_bindings (
                   location_id TEXT NOT NULL, runtime_id TEXT NOT NULL,
                   runtime_incarnation_id TEXT NOT NULL,
                   raw_filesystem_instance_id TEXT NOT NULL, raw_volume_id TEXT,
                   raw_file_id TEXT NOT NULL, raw_generation TEXT,
                   platform_kind TEXT NOT NULL, observed_at TEXT NOT NULL,
                   PRIMARY KEY(location_id, runtime_incarnation_id)
                 );
                 CREATE TABLE workspace_root_file_locations (
                   workspace_id TEXT NOT NULL, workspace_root_id TEXT NOT NULL,
                   location_id TEXT NOT NULL UNIQUE, resource_id TEXT NOT NULL,
                   observed_at TEXT NOT NULL,
                   PRIMARY KEY(workspace_root_id, location_id)
                 );
                 CREATE INDEX idx_workspace_root_file_locations_cleanup
                   ON workspace_root_file_locations(workspace_root_id, location_id);
                 CREATE TABLE workspace_root_cleanup_outbox (
                   workspace_id TEXT NOT NULL, workspace_root_id TEXT PRIMARY KEY,
                   after_location_id TEXT, status TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   version INTEGER NOT NULL
                 );
                 CREATE TABLE workspace_roots (
                   workspace_id TEXT NOT NULL, workspace_root_id TEXT PRIMARY KEY,
                   status TEXT NOT NULL
                 );
                 CREATE TABLE domain_events (
                   entity_type TEXT NOT NULL, entity_id TEXT NOT NULL,
                   type TEXT NOT NULL, payload_json TEXT NOT NULL
                 );
                 CREATE TABLE resource_revisions (
                   resource_revision_id TEXT PRIMARY KEY, resource_id TEXT NOT NULL
                 );",
            )
            .expect("membership fixture schema is valid");
        connection
            .execute(
                "INSERT INTO workspace_roots(workspace_id, workspace_root_id, status)
                 VALUES (?1, ?2, 'ACTIVE')",
                params![WORKSPACE, ROOT],
            )
            .expect("seed active WorkspaceRoot");
    }

    fn candidate(
        resource_id: &str,
        identity_digest: &str,
        observation_id: &str,
        location_id: &str,
        name: &str,
    ) -> WorkspaceRootFileMembershipCandidate {
        let locator_ref_id = format!("locator-{location_id}");
        WorkspaceRootFileMembershipCandidate {
            observation: WorkspaceRootFileObservation {
                observation_id: observation_id.to_owned(),
                file_identity_digest: identity_digest.to_owned(),
                resource_id: Some(resource_id.to_owned()),
                display_name: name.to_owned(),
                size_bytes: 41,
                modified_at: None,
                observed_at: NOW.to_owned(),
                private_relative_locator: format!("private/{name}").into_bytes(),
            },
            resource: ResourceRecord {
                resource_id: resource_id.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                kind: "FILE".to_owned(),
                provider_identity: json!({
                    "provider_instance_id": "litecowork.local_filesystem",
                    "stable_object_id": identity_digest,
                    "confidence": "FILESYSTEM_ID",
                }),
                identity_digest: Some(identity_digest.to_owned()),
                display_name: name.to_owned(),
                current_revision_id: None,
                sensitivity: "PRIVATE".to_owned(),
                provenance: json!({"kind": "WORKSPACE_ROOT_SCAN", "workspace_root_id": ROOT}),
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
                version: 1,
            },
            location: ResourceLocationRecord {
                location_id: location_id.to_owned(),
                resource_id: resource_id.to_owned(),
                runtime_id: RUNTIME.to_owned(),
                locator_ref_id: locator_ref_id.clone(),
                provider_ref: "litecowork.local_filesystem".to_owned(),
                availability: "AVAILABLE".to_owned(),
                writable: false,
                observed_revision_id: None,
                observed_digest: None,
                observed_at: NOW.to_owned(),
            },
            locator_binding: LocalResourceLocationBindingRecord {
                location_id: location_id.to_owned(),
                locator_ref_id,
                runtime_id: RUNTIME.to_owned(),
                runtime_incarnation_id: INCARNATION.to_owned(),
                private_locator: format!("root-relative-v1:{ROOT}:private/{name}"),
                observed_at: NOW.to_owned(),
            },
            file_identity_binding: LocalFileIdentityBindingRecord {
                location_id: location_id.to_owned(),
                runtime_id: RUNTIME.to_owned(),
                runtime_incarnation_id: INCARNATION.to_owned(),
                raw_filesystem_instance_id: "private-filesystem-id".to_owned(),
                raw_volume_id: None,
                raw_file_id: format!("private-file-id-{identity_digest}"),
                raw_generation: None,
                platform_kind: "LINUX_DEVICE_INODE".to_owned(),
                observed_at: NOW.to_owned(),
            },
        }
    }

    #[test]
    fn file_identity_reuses_resource_across_paths_and_replacement_changes_identity() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        let transaction = connection
            .transaction()
            .expect("start membership transaction");

        let original = candidate(
            "resource-file-one",
            &format!("sha256:{}", "1".repeat(64)),
            &format!("sha256:{}", "a".repeat(64)),
            "location-path-a",
            "report.txt",
        );
        let first = reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            original,
        )
        .expect("first scan creates one canonical FILE Resource");
        assert!(first.resource_created);
        assert!(first.location_created_or_rebound);
        assert_eq!(first.resource.resource_id, "resource-file-one");
        assert_eq!(
            first.observation.resource_id.as_deref(),
            Some("resource-file-one")
        );
        assert_eq!(first.location.resource_id, "resource-file-one");
        assert_eq!(first.locator_binding.runtime_incarnation_id, INCARNATION);
        assert_eq!(
            first.file_identity_binding.runtime_incarnation_id,
            INCARNATION
        );

        let renamed = candidate(
            "proposed-resource-after-rename",
            &format!("sha256:{}", "1".repeat(64)),
            &format!("sha256:{}", "b".repeat(64)),
            "location-path-b",
            "renamed.txt",
        );
        let second =
            reconcile_file_membership(&transaction, WORKSPACE, ROOT, RUNTIME, INCARNATION, renamed)
                .expect("rename reuses Resource identity and adds a path location");
        assert!(!second.resource_created);
        assert_eq!(second.resource.resource_id, "resource-file-one");
        assert_eq!(second.location.resource_id, "resource-file-one");

        let replacement = candidate(
            "resource-file-replacement",
            &format!("sha256:{}", "2".repeat(64)),
            &format!("sha256:{}", "a".repeat(64)),
            "location-path-a-replacement",
            "report.txt",
        );
        let third = reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            replacement,
        )
        .expect("replacement identity creates a different Resource at the same path");
        assert!(third.resource_created);
        assert_eq!(third.resource.resource_id, "resource-file-replacement");
        assert_eq!(
            third.observation.resource_id.as_deref(),
            Some("resource-file-replacement")
        );

        let resource_count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))
            .expect("count canonical resources");
        let location_a: (String, Option<String>, Option<String>) = transaction
            .query_row(
                "SELECT resource_id, observed_revision_id, observed_digest
                 FROM resource_locations WHERE location_id = 'location-path-a'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load replaced path location");
        assert_eq!(resource_count, 2);
        assert_eq!(location_a, ("resource-file-one".to_owned(), None, None));
        let location_replacement: String = transaction
            .query_row(
                "SELECT resource_id FROM resource_locations WHERE location_id = 'location-path-a-replacement'",
                [],
                |row| row.get(0),
            )
            .expect("load replacement's identity-qualified path location");
        assert_eq!(location_replacement, "resource-file-replacement");
        let mapped_resource: String = transaction
            .query_row(
                "SELECT resource_id FROM workspace_root_file_locations
                 WHERE workspace_root_id = ?1 AND location_id = 'location-path-a-replacement'",
                [ROOT],
                |row| row.get(0),
            )
            .expect("load replacement path's root membership");
        assert_eq!(mapped_resource, "resource-file-replacement");
        let revision_count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM resource_revisions", [], |row| {
                row.get(0)
            })
            .expect("count Resource revisions");
        assert_eq!(revision_count, 0, "metadata-only scans create no revisions");
        transaction.commit().expect("commit membership fixture");
    }

    #[test]
    fn location_id_collision_cannot_rebind_an_existing_resource_location() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        let transaction = connection
            .transaction()
            .expect("start membership transaction");
        reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            candidate(
                "resource-file-original",
                &format!("sha256:{}", "1".repeat(64)),
                &format!("sha256:{}", "a".repeat(64)),
                "location-same-path",
                "report.txt",
            ),
        )
        .expect("create original path location");

        let replacement = reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            candidate(
                "resource-file-replacement",
                &format!("sha256:{}", "2".repeat(64)),
                &format!("sha256:{}", "b".repeat(64)),
                "location-same-path",
                "report.txt",
            ),
        );
        assert!(matches!(replacement, Err(StoreError::Conflict { .. })));
        let still_original: String = transaction
            .query_row(
                "SELECT resource_id FROM resource_locations WHERE location_id = 'location-same-path'",
                [],
                |row| row.get(0),
            )
            .expect("load unchanged original path location");
        assert_eq!(still_original, "resource-file-original");
        transaction.rollback().expect("roll back collision fixture");
    }

    #[test]
    fn inconsistent_root_membership_is_not_silently_rebound() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        let transaction = connection
            .transaction()
            .expect("start membership transaction");
        let original = candidate(
            "resource-file-original",
            &format!("sha256:{}", "1".repeat(64)),
            &format!("sha256:{}", "a".repeat(64)),
            "location-root-map",
            "report.txt",
        );
        reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            original,
        )
        .expect("create original membership");
        transaction
            .execute(
                "UPDATE workspace_root_file_locations SET resource_id = 'corrupt-resource'
                 WHERE workspace_root_id = ?1 AND location_id = 'location-root-map'",
                [ROOT],
            )
            .expect("inject inconsistent root projection");

        let retry = reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            candidate(
                "resource-file-original",
                &format!("sha256:{}", "1".repeat(64)),
                &format!("sha256:{}", "b".repeat(64)),
                "location-root-map",
                "report.txt",
            ),
        );
        assert!(matches!(retry, Err(StoreError::Conflict { .. })));
        let still_inconsistent: String = transaction
            .query_row(
                "SELECT resource_id FROM workspace_root_file_locations
                 WHERE workspace_root_id = ?1 AND location_id = 'location-root-map'",
                [ROOT],
                |row| row.get(0),
            )
            .expect("load unchanged root membership projection");
        assert_eq!(still_inconsistent, "corrupt-resource");
        let still_original_location: String = transaction
            .query_row(
                "SELECT resource_id FROM resource_locations WHERE location_id = 'location-root-map'",
                [],
                |row| row.get(0),
            )
            .expect("load unchanged canonical location");
        assert_eq!(still_original_location, "resource-file-original");
        transaction
            .rollback()
            .expect("roll back corruption fixture");
    }

    #[test]
    fn malformed_file_identity_digest_is_rejected_before_membership_is_written() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        let transaction = connection
            .transaction()
            .expect("start membership transaction");
        let malformed = candidate(
            "resource-file-invalid-digest",
            &format!("sha256:{}", "z".repeat(64)),
            &format!("sha256:{}", "a".repeat(64)),
            "location-invalid-digest",
            "report.txt",
        );

        let result = reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            malformed,
        );

        assert!(matches!(result, Err(StoreError::Invalid(_))));
        let resources: i64 = transaction
            .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))
            .expect("count Resources after rejected candidate");
        assert_eq!(resources, 0);
    }

    #[test]
    fn identity_digest_cannot_alias_a_non_file_resource() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        connection
            .execute(
                "INSERT INTO resources(
                   resource_id, workspace_id, kind, provider_identity_json,
                   identity_digest, display_name, current_revision_id, sensitivity,
                   context_document_json, provenance_json, created_at, updated_at, version
                 ) VALUES ('resource-folder', ?1, 'FOLDER', '{}', ?2, 'folder', NULL,
                           'PRIVATE', NULL, '{}', ?3, ?3, 1)",
                params![WORKSPACE, format!("sha256:{}", "4".repeat(64)), NOW],
            )
            .expect("insert conflicting aggregate");
        let transaction = connection
            .transaction()
            .expect("start membership transaction");
        let file = candidate(
            "resource-file",
            &format!("sha256:{}", "4".repeat(64)),
            &format!("sha256:{}", "d".repeat(64)),
            "location-conflict",
            "data.bin",
        );
        let result =
            reconcile_file_membership(&transaction, WORKSPACE, ROOT, RUNTIME, INCARNATION, file);
        assert!(matches!(result, Err(StoreError::Integrity(_))));
    }

    #[test]
    fn root_file_cleanup_is_stable_bounded_retryable_and_retains_resources() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        let transaction = connection
            .transaction()
            .expect("start membership transaction");
        for (location_id, name, suffix) in [
            ("location-one", "one.txt", "6"),
            ("location-two", "two.txt", "7"),
        ] {
            let proposal = candidate(
                "resource-shared-file",
                &format!("sha256:{}", "5".repeat(64)),
                &format!("sha256:{}", suffix.repeat(64)),
                location_id,
                name,
            );
            reconcile_file_membership(
                &transaction,
                WORKSPACE,
                ROOT,
                RUNTIME,
                INCARNATION,
                proposal,
            )
            .expect("commit root-linked FILE location");
        }
        transaction
            .execute(
                "UPDATE workspace_roots SET status = 'REVOKED' WHERE workspace_root_id = ?1",
                [ROOT],
            )
            .expect("revoke owning root");
        enqueue_workspace_root_file_cleanup(&transaction, WORKSPACE, ROOT, NOW)
            .expect("idempotently enqueue revoked root cleanup");
        enqueue_workspace_root_file_cleanup(&transaction, WORKSPACE, ROOT, NOW)
            .expect("exact outbox replay is a no-op");

        let first_batch = next_workspace_root_file_cleanup_batch(&transaction, ROOT, 1)
            .expect("read first bounded batch")
            .expect("pending cleanup exists");
        assert_eq!(first_batch.locations.len(), 1);
        assert!(first_batch.has_more);
        assert_eq!(first_batch.locations[0].location_id, "location-one");
        terminally_revoke_test_location(&transaction, &first_batch.locations[0]);
        commit_workspace_root_file_cleanup_batch(&transaction, &first_batch, NOW)
            .expect("advance first location and cursor atomically");

        let stale_replay =
            commit_workspace_root_file_cleanup_batch(&transaction, &first_batch, NOW);
        assert!(matches!(stale_replay, Err(StoreError::Conflict { .. })));
        let second_batch = next_workspace_root_file_cleanup_batch(&transaction, ROOT, 1)
            .expect("read the next bounded batch")
            .expect("cleanup remains pending");
        assert_eq!(
            second_batch.after_location_id.as_deref(),
            Some("location-one")
        );
        assert_eq!(second_batch.locations[0].location_id, "location-two");
        assert!(!second_batch.has_more);
        terminally_revoke_test_location(&transaction, &second_batch.locations[0]);
        commit_workspace_root_file_cleanup_batch(&transaction, &second_batch, NOW)
            .expect("complete the final location and cursor atomically");

        let (remaining_members, outbox_status, resource_count): (i64, String, i64) = transaction
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM workspace_root_file_locations),
                   (SELECT status FROM workspace_root_cleanup_outbox WHERE workspace_root_id = ?1),
                   (SELECT COUNT(*) FROM resources)",
                [ROOT],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read cleanup outcome");
        assert_eq!(remaining_members, 0);
        assert_eq!(outbox_status, "COMPLETE");
        assert_eq!(resource_count, 1, "canonical FILE Resource is retained");
        let (availability, revisions): (String, i64) = transaction
            .query_row(
                "SELECT l.availability,
                        (SELECT COUNT(*) FROM resource_revisions r WHERE r.resource_id = l.resource_id)
                 FROM resource_locations l WHERE l.location_id = 'location-two'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read terminal location");
        assert_eq!(availability, "REVOKED");
        assert_eq!(revisions, 0, "metadata-only membership creates no revision");
    }

    #[test]
    fn root_cleanup_rejects_unrevoked_roots_and_batches_over_the_hard_limit() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        let transaction = connection
            .transaction()
            .expect("start membership transaction");
        assert!(matches!(
            enqueue_workspace_root_file_cleanup(&transaction, WORKSPACE, ROOT, NOW),
            Err(StoreError::Conflict { .. })
        ));
        transaction
            .execute(
                "UPDATE workspace_roots SET status = 'REVOKED' WHERE workspace_root_id = ?1",
                [ROOT],
            )
            .expect("revoke root");
        enqueue_workspace_root_file_cleanup(&transaction, WORKSPACE, ROOT, NOW)
            .expect("enqueue after terminal root transition");
        assert!(matches!(
            next_workspace_root_file_cleanup_batch(
                &transaction,
                ROOT,
                MAX_ROOT_FILE_CLEANUP_BATCH + 1
            ),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn revoke_fence_removes_all_private_bindings_and_denies_pending_projection_cleanup() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        schema(&connection);
        let transaction = connection
            .transaction()
            .expect("start membership transaction");
        let proposal = candidate(
            "resource-file",
            &format!("sha256:{}", "8".repeat(64)),
            &format!("sha256:{}", "9".repeat(64)),
            "location-rooted",
            "private.txt",
        );
        reconcile_file_membership(
            &transaction,
            WORKSPACE,
            ROOT,
            RUNTIME,
            INCARNATION,
            proposal,
        )
        .expect("commit root-linked file membership");
        transaction
            .execute(
                "UPDATE workspace_roots SET status = 'REVOKED' WHERE workspace_root_id = ?1",
                [ROOT],
            )
            .expect("apply immediate root policy fence");
        let late_scan = candidate(
            "resource-late-file",
            &format!("sha256:{}", "a".repeat(64)),
            &format!("sha256:{}", "b".repeat(64)),
            "location-late-scan",
            "late.txt",
        );
        assert!(matches!(
            reconcile_file_membership(
                &transaction,
                WORKSPACE,
                ROOT,
                RUNTIME,
                INCARNATION,
                late_scan
            ),
            Err(StoreError::Conflict { .. })
        ));
        transaction
            .execute(
                "INSERT INTO resource_location_bindings(
                   location_id, locator_ref_id, runtime_id, runtime_incarnation_id,
                   private_locator, observed_at
                 ) VALUES ('location-rooted', 'old-locator', ?1, 'old-incarnation',
                           'old-private-locator', ?2)",
                params![RUNTIME, NOW],
            )
            .expect("insert retained prior-incarnation binding");
        transaction
            .execute(
                "INSERT INTO file_identity_bindings(
                   location_id, runtime_id, runtime_incarnation_id,
                   raw_filesystem_instance_id, raw_file_id, platform_kind, observed_at
                 ) VALUES ('location-rooted', ?1, 'old-incarnation', 'raw-volume',
                           'raw-file', 'LINUX_DEVICE_INODE', ?2)",
                params![RUNTIME, NOW],
            )
            .expect("insert prior-incarnation raw identity");

        assert!(
            !root_linked_file_location_is_readable(
                &transaction,
                WORKSPACE,
                "resource-file",
                "location-rooted"
            )
            .expect("revoked root denies member read")
        );
        assert!(
            root_linked_file_location_is_readable(
                &transaction,
                WORKSPACE,
                "resource-unrelated",
                "location-unrelated"
            )
            .expect("unrelated location has no root grant")
        );
        fence_revoked_workspace_root_file_locations(&transaction, WORKSPACE, ROOT, NOW)
            .expect("remove old and current bindings and queue projection cleanup");

        let (locator_count, identity_count, location_availability, outbox_status):
            (i64, i64, String, String) = transaction
                .query_row(
                    "SELECT
                       (SELECT COUNT(*) FROM resource_location_bindings WHERE location_id = 'location-rooted'),
                       (SELECT COUNT(*) FROM file_identity_bindings WHERE location_id = 'location-rooted'),
                       (SELECT availability FROM resource_locations WHERE location_id = 'location-rooted'),
                       (SELECT status FROM workspace_root_cleanup_outbox WHERE workspace_root_id = ?1)",
                    [ROOT],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("read revocation fence outcome");
        assert_eq!((locator_count, identity_count), (0, 0));
        assert_eq!(
            location_availability, "AVAILABLE",
            "batch projection cleanup is pending"
        );
        assert_eq!(outbox_status, "PENDING");
        assert!(
            !root_linked_file_location_is_readable(
                &transaction,
                WORKSPACE,
                "resource-file",
                "location-rooted"
            )
            .expect("root fence denies reads before outbox completion")
        );

        transaction
            .execute(
                "UPDATE workspace_roots SET status = 'PAUSED' WHERE workspace_root_id = ?1",
                [ROOT],
            )
            .expect("simulate an independently paused root");
        assert!(
            !root_linked_file_location_is_readable(
                &transaction,
                WORKSPACE,
                "resource-file",
                "location-rooted"
            )
            .expect("paused roots also deny location use")
        );
        transaction.commit().expect("commit root cleanup test");
    }

    fn terminally_revoke_test_location(
        transaction: &Transaction<'_>,
        location: &WorkspaceRootCleanupLocation,
    ) {
        transaction
            .execute(
                "UPDATE resource_locations SET availability = 'REVOKED', writable = 0
                 WHERE location_id = ?1 AND resource_id = ?2",
                params![location.location_id, location.resource_id],
            )
            .expect("terminally revoke location");
        transaction
            .execute(
                "DELETE FROM resource_location_bindings WHERE location_id = ?1",
                [&location.location_id],
            )
            .expect("remove private locator");
        transaction
            .execute(
                "DELETE FROM file_identity_bindings WHERE location_id = ?1",
                [&location.location_id],
            )
            .expect("remove raw identity");
        transaction
            .execute(
                "INSERT INTO domain_events(entity_type, entity_id, type, payload_json)
                 VALUES ('Resource', ?1, 'resource.location.changed.v1', ?2)",
                params![
                    location.resource_id,
                    serde_json::json!({
                        "location_id": location.location_id,
                        "resource_id": location.resource_id,
                        "availability": "REVOKED",
                        "observed_at": NOW,
                    })
                    .to_string(),
                ],
            )
            .expect("append ResourceLocation revocation event");
    }
}
