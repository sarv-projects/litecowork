use super::*;
use storage_core::{
    DependencyEdgeRecord, DependencyFreshness, InvalidationRecord, ResourceDependencyPageRequest,
    ResourceDependencyReadStore, ResourceDependencyViewRecord,
};

impl ResourceDependencyReadStore for SqliteWorkspaceStore {
    fn list_resource_dependents_page(
        &self,
        request: ResourceDependencyPageRequest,
    ) -> Result<Option<Vec<ResourceDependencyViewRecord>>, StoreError> {
        validate_nonempty(&[
            &request.owner_principal_id,
            &request.workspace_id,
            &request.resource_id,
        ])?;
        if !(1..=201).contains(&request.limit)
            || request.after_created_at.is_some() != request.after_dependency_edge_id.is_some()
            || request.after_created_at.as_deref().is_some_and(|value| {
                value.is_empty() || value.len() > 64 || value.chars().any(char::is_control)
            })
            || request
                .after_dependency_edge_id
                .as_deref()
                .is_some_and(|value| {
                    value.trim().is_empty()
                        || value.len() > 256
                        || value.chars().any(char::is_control)
                })
        {
            return Err(StoreError::Invalid(
                "Resource dependency page query is invalid".to_owned(),
            ));
        }
        let (reply, receive) = mpsc::channel();
        self.execute_command(Command::ListResourceDependents { request, reply }, receive)
    }
}

pub(super) fn list_resource_dependents(
    connection: &Connection,
    request: &ResourceDependencyPageRequest,
) -> Result<Option<Vec<ResourceDependencyViewRecord>>, StoreError> {
    let transaction = connection
        .unchecked_transaction()
        .map_err(map_database_error)?;
    let resource_exists: bool = transaction
        .query_row(
            "SELECT EXISTS(
                 SELECT 1
                 FROM resources resource
                 JOIN workspaces workspace ON workspace.workspace_id = resource.workspace_id
                 WHERE resource.workspace_id = ?1
                   AND resource.resource_id = ?2
                   AND workspace.owner_principal_id = ?3
             )",
            params![
                request.workspace_id,
                request.resource_id,
                request.owner_principal_id
            ],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !resource_exists {
        transaction.commit().map_err(map_database_error)?;
        return Ok(None);
    }
    let limit = i64::try_from(request.limit)
        .map_err(|_| StoreError::Invalid("Resource dependency page limit is invalid".to_owned()))?;
    let page = {
        let mut statement = transaction
            .prepare(
            "SELECT edge.dependency_edge_id,
                    edge.workspace_id,
                    edge.source_resource_id,
                    edge.source_revision_id,
                    edge.dependent_kind,
                    edge.dependent_ref,
                    edge.created_at,
                    CASE
                      WHEN source_revision.resource_revision_id IS NULL OR NOT (
                        CASE
                          WHEN edge.dependent_kind = 'ARTIFACT_VERSION' THEN EXISTS (
                            SELECT 1
                            FROM artifacts artifact
                            JOIN artifact_versions version
                              ON version.artifact_id = artifact.artifact_id
                            WHERE artifact.artifact_id = edge.artifact_id
                              AND artifact.workspace_id = edge.workspace_id
                              AND version.version = edge.artifact_version
                              AND EXISTS (
                                SELECT 1
                                FROM json_each(
                                  CASE WHEN json_valid(version.input_refs_json)
                                    THEN version.input_refs_json ELSE '[]' END
                                ) input_ref
                                WHERE json_extract(input_ref.value, '$.workspace_id') = edge.workspace_id
                                  AND json_extract(input_ref.value, '$.resource_id') = edge.source_resource_id
                                  AND json_extract(input_ref.value, '$.revision_id') = edge.source_revision_id
                              )
                          )
                          WHEN edge.dependent_kind = 'VERIFICATION_RUN' THEN EXISTS (
                            SELECT 1
                            FROM verification_runs run
                            JOIN tasks task ON task.task_id = run.task_id
                            WHERE run.verification_run_id = edge.verification_run_id
                              AND task.workspace_id = edge.workspace_id
                              AND EXISTS (
                                SELECT 1
                                FROM json_each(
                                  CASE WHEN json_valid(run.inputs_json)
                                    THEN run.inputs_json ELSE '[]' END
                                ) input
                                WHERE json_extract(input.value, '$.resource_ref.workspace_id') = edge.workspace_id
                                  AND json_extract(input.value, '$.resource_ref.resource_id') = edge.source_resource_id
                                  AND json_extract(input.value, '$.resource_ref.revision_id') = edge.source_revision_id
                              )
                          )
                          ELSE 0
                        END
                      ) THEN 'UNKNOWN'
                      WHEN COALESCE(heads.head_count, 0) = 0 THEN 'UNKNOWN'
                      WHEN heads.head_count > 1 THEN 'CONFLICTED'
                      WHEN source.current_revision_id IS NULL
                        OR source.current_revision_id <> heads.sole_head_revision_id THEN 'UNKNOWN'
                      WHEN heads.sole_head_revision_id = edge.source_revision_id THEN 'CURRENT'
                      ELSE 'STALE'
                    END AS dependent_freshness,
                    invalidation.invalidation_record_id,
                    invalidation.dependency_edge_id,
                    invalidation.observed_revision_id,
                    invalidation.reason_code,
                    invalidation.created_at
             FROM dependency_edges edge
             JOIN workspaces workspace
               ON workspace.workspace_id = edge.workspace_id
             LEFT JOIN resources source
               ON source.workspace_id = edge.workspace_id
              AND source.resource_id = edge.source_resource_id
             LEFT JOIN resource_revisions source_revision
               ON source_revision.resource_id = edge.source_resource_id
              AND source_revision.resource_revision_id = edge.source_revision_id
             LEFT JOIN (
               SELECT revision.resource_id,
                      COUNT(*) AS head_count,
                      MIN(revision.resource_revision_id) AS sole_head_revision_id
               FROM resource_revisions revision
               WHERE revision.resource_id = ?2
                 AND NOT EXISTS (
                   SELECT 1
                   FROM resource_revision_parents child
                   WHERE child.resource_id = revision.resource_id
                     AND child.parent_revision_id = revision.resource_revision_id
                 )
               GROUP BY revision.resource_id
             ) heads ON heads.resource_id = source.resource_id
             LEFT JOIN invalidation_records invalidation
               ON invalidation.invalidation_record_id = (
                 SELECT latest.invalidation_record_id
                 FROM invalidation_records latest
                 WHERE latest.dependency_edge_id = edge.dependency_edge_id
                 ORDER BY latest.created_at COLLATE BINARY DESC,
                          latest.invalidation_record_id COLLATE BINARY DESC
                 LIMIT 1
               )
             WHERE edge.workspace_id = ?1
               AND edge.source_resource_id = ?2
               AND workspace.owner_principal_id = ?3
               AND (?4 IS NULL
                    OR edge.created_at COLLATE BINARY > ?4
                    OR (edge.created_at COLLATE BINARY = ?4
                        AND edge.dependency_edge_id COLLATE BINARY > ?5))
             ORDER BY edge.created_at COLLATE BINARY ASC,
                      edge.dependency_edge_id COLLATE BINARY ASC
             LIMIT ?6",
            )
            .map_err(map_database_error)?;
        let rows = statement
            .query_map(
                params![
                    request.workspace_id,
                    request.resource_id,
                    request.owner_principal_id,
                    request.after_created_at,
                    request.after_dependency_edge_id,
                    limit,
                ],
                |row| {
                    let freshness: String = row.get(7)?;
                    let dependent_freshness = match freshness.as_str() {
                        "CURRENT" => DependencyFreshness::Current,
                        "STALE" => DependencyFreshness::Stale,
                        "CONFLICTED" => DependencyFreshness::Conflicted,
                        "UNKNOWN" => DependencyFreshness::Unknown,
                        _ => {
                            return Err(rusqlite::Error::InvalidColumnType(
                                7,
                                "dependent_freshness".to_owned(),
                                rusqlite::types::Type::Text,
                            ));
                        }
                    };
                    let invalidation_id: Option<String> = row.get(8)?;
                    let latest_invalidation = invalidation_id
                        .map(
                            |invalidation_record_id| -> rusqlite::Result<InvalidationRecord> {
                                Ok(InvalidationRecord {
                                    invalidation_record_id,
                                    dependency_edge_id: row.get(9)?,
                                    observed_revision_id: row.get(10)?,
                                    reason_code: row.get(11)?,
                                    created_at: row.get(12)?,
                                })
                            },
                        )
                        .transpose()?;
                    Ok(ResourceDependencyViewRecord {
                        dependency_edge: DependencyEdgeRecord {
                            dependency_edge_id: row.get(0)?,
                            workspace_id: row.get(1)?,
                            source_resource_id: row.get(2)?,
                            source_revision_id: row.get(3)?,
                            dependent_kind: row.get(4)?,
                            dependent_ref: row.get(5)?,
                            created_at: row.get(6)?,
                        },
                        dependent_freshness,
                        latest_invalidation,
                    })
                },
            )
            .map_err(map_database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_database_error)?
    };
    transaction.commit().map_err(map_database_error)?;
    Ok(Some(page))
}
