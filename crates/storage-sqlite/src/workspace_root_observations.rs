//! Fenced, rebuildable observations collected beneath explicitly authorized WorkspaceRoots.

use rusqlite::{OptionalExtension, Transaction, params};
use std::sync::mpsc;
#[cfg(test)]
use storage_core::WorkspaceRootFileObservation;
use storage_core::{
    MAX_WORKSPACE_ROOT_SCAN_ENTRIES, RootObservationCommitSummary, RootObservationFreshness,
    RootObservationScanStatus, RootObservationSearchRecord, RootObservationSearchRequest,
    StoreError, WorkspaceRootObservationBatch, WorkspaceRootObservationStore,
};

use super::super::{Command, SqliteWorkspaceStore};
use crate::resource_index::WorkspaceRootObservationWriterOperation;

#[cfg(test)]
const V21_DDL: &str = include_str!("../../../docs/schemas/sqlite-v21.sql");
const MAX_AGGREGATE_LOCATOR_BYTES: usize = 1024 * 1024;
pub(crate) type WriterOperation = Box<dyn FnOnce(&mut rusqlite::Connection) + Send>;

impl WorkspaceRootObservationStore for SqliteWorkspaceStore {
    fn search_workspace_root_observations(
        &self,
        request: RootObservationSearchRequest,
    ) -> Result<Vec<RootObservationSearchRecord>, StoreError> {
        let (reply, receiver) = mpsc::channel();
        let operation: WriterOperation = Box::new(move |connection| {
            let result = (|| {
                let transaction = connection
                    .transaction()
                    .map_err(map_observation_database_error)?;
                let page = search_workspace_root_observations(&transaction, &request)?;
                transaction
                    .commit()
                    .map_err(map_observation_database_error)?;
                Ok(page)
            })();
            let _ = reply.send(result);
        });
        self.execute_command(
            Command::WorkspaceRootObservationOperation {
                operation: operation as WorkspaceRootObservationWriterOperation,
            },
            receiver,
        )
    }
}

/// Deterministic owner-scoped root name search. It returns no paths, raw OS identities,
/// content, or implicit ResourceRef. The caller must authenticate the Workspace owner.
pub(crate) fn search_workspace_root_observations(
    connection: &rusqlite::Connection,
    request: &RootObservationSearchRequest,
) -> Result<Vec<RootObservationSearchRecord>, StoreError> {
    if request.workspace_id.is_empty()
        || request.workspace_root_id.is_empty()
        || request.owner_principal_id.is_empty()
        || request.runtime_id.is_empty()
        || request.runtime_incarnation_id.is_empty()
        || request.query.len() > 256
        || request.query.contains('\0')
        || request.expected_root_version == 0
        || !(1..=100).contains(&request.limit)
        || request.after_display_name.is_some() != request.after_observation_id.is_some()
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot observation search is invalid".to_owned(),
        ));
    }
    let owner: bool = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM workspaces
               WHERE workspace_id = ?1 AND owner_principal_id = ?2
             )",
            params![request.workspace_id, request.owner_principal_id],
            |row| row.get(0),
        )
        .map_err(map_observation_database_error)?;
    if !owner {
        return Err(StoreError::NotFound);
    }
    let current_root: Option<(i64, String)> = connection
        .query_row(
            "SELECT version, status FROM workspace_roots
             WHERE workspace_id = ?1 AND workspace_root_id = ?2",
            params![request.workspace_id, request.workspace_root_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_observation_database_error)?;
    let Some((current_root_version, root_status)) = current_root else {
        return Err(StoreError::NotFound);
    };
    if root_status != "ACTIVE" {
        return Err(StoreError::NotFound);
    }
    let current_root_version = u64::try_from(current_root_version)
        .map_err(|_| StoreError::Integrity("WorkspaceRoot version is invalid".to_owned()))?;
    if current_root_version != request.expected_root_version {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_root_version),
            actual: Some(current_root_version),
        });
    }
    // Read one extra row so the Operator can issue a stable keyset cursor only when
    // another matching row exists.
    let limit = i64::try_from(request.limit + 1)
        .map_err(|_| StoreError::Invalid("WorkspaceRoot search limit is invalid".to_owned()))?;
    let mut statement = connection
        .prepare(
            "SELECT observation.observation_id, observation.file_identity_digest,
                    observation.resource_id, observation.display_name,
                    observation.size_bytes, observation.modified_at,
                    observation.observed_at, observation.freshness
             FROM workspace_root_observations observation
             JOIN workspace_roots root
               ON root.workspace_id = observation.workspace_id
              AND root.workspace_root_id = observation.workspace_root_id
              AND root.status = 'ACTIVE'
             JOIN runtimes runtime
               ON runtime.runtime_id = observation.runtime_id
              AND runtime.current_incarnation_id = observation.runtime_incarnation_id
              AND runtime.runtime_id = ?3
              AND runtime.current_incarnation_id = ?4
             JOIN resource_locations location
               ON location.location_id = root.location_id
              AND location.resource_id = root.resource_id
              AND location.availability = 'AVAILABLE'
              AND (location.runtime_id IS NULL OR location.runtime_id = runtime.runtime_id)
             JOIN runtime_workspace_bindings workspace_binding
               ON workspace_binding.workspace_id = root.workspace_id
              AND workspace_binding.runtime_id = runtime.runtime_id
              AND workspace_binding.status = 'ACTIVE'
             JOIN resource_location_bindings locator_binding
               ON locator_binding.location_id = root.location_id
              AND locator_binding.runtime_id = runtime.runtime_id
              AND locator_binding.runtime_incarnation_id = runtime.current_incarnation_id
             JOIN file_identity_bindings identity_binding
               ON identity_binding.location_id = root.location_id
              AND identity_binding.runtime_id = runtime.runtime_id
              AND identity_binding.runtime_incarnation_id = runtime.current_incarnation_id
             WHERE observation.workspace_id = ?1
               AND observation.workspace_root_id = ?2
               AND observation.runtime_id = ?3
               AND observation.runtime_incarnation_id = ?4
               AND root.version = ?5
               AND (?6 = '' OR instr(observation.display_name_search_key, ?6) > 0)
               AND (?7 IS NULL OR observation.freshness = ?7)
               AND (?8 IS NULL OR observation.display_name > ?8
                    OR (observation.display_name = ?8 AND observation.observation_id > ?9))
             ORDER BY observation.display_name COLLATE BINARY, observation.observation_id
             LIMIT ?10",
        )
        .map_err(map_observation_database_error)?;
    let rows = statement
        .query_map(
            params![
                request.workspace_id,
                request.workspace_root_id,
                request.runtime_id,
                request.runtime_incarnation_id,
                to_sql_i64(request.expected_root_version)?,
                request.query.to_lowercase(),
                request.freshness.map(RootObservationFreshness::as_str),
                request.after_display_name,
                request.after_observation_id,
                limit,
            ],
            |row| {
                let size: i64 = row.get(4)?;
                Ok(RootObservationSearchRecord {
                    observation_id: row.get(0)?,
                    file_identity_digest: row.get(1)?,
                    resource_id: row.get(2)?,
                    display_name: row.get(3)?,
                    size_bytes: u64::try_from(size)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, size))?,
                    modified_at: row.get(5)?,
                    observed_at: row.get(6)?,
                    freshness: parse_freshness(&row.get::<_, String>(7)?).map_err(|_| {
                        rusqlite::Error::InvalidColumnType(
                            7,
                            "freshness".to_owned(),
                            rusqlite::types::Type::Text,
                        )
                    })?,
                })
            },
        )
        .map_err(map_observation_database_error)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(map_observation_database_error)
}

/// Apply the local membership projection inside the exact WorkspaceRoot scan commit.
/// The caller must keep this and the public count/status receipt in one SQLite
/// IMMEDIATE transaction. The fence is rechecked here as defense in depth.
pub(crate) fn reconcile_workspace_root_observations_in_transaction(
    transaction: &Transaction<'_>,
    batch: &WorkspaceRootObservationBatch,
) -> Result<RootObservationCommitSummary, StoreError> {
    validate_batch(batch)?;
    let prior: Option<(String, i64, i64, i64)> = transaction
        .query_row(
            "SELECT request_digest, complete_generation, observations_upserted,
                    observations_marked_stale
             FROM workspace_root_observation_batch_receipts
             WHERE workspace_id = ?1 AND workspace_root_id = ?2 AND request_id = ?3",
            params![
                batch.workspace_id,
                batch.workspace_root_id,
                batch.request_id
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(map_observation_database_error)?;
    if let Some((digest, generation, upserted, stale)) = prior {
        if digest != batch.request_digest {
            return Err(StoreError::IdempotencyConflict);
        }
        return Ok(RootObservationCommitSummary {
            complete_generation: u64::try_from(generation).map_err(|_| {
                StoreError::Integrity("WorkspaceRoot observation generation is invalid".to_owned())
            })?,
            observations_upserted: u32::try_from(upserted).map_err(|_| {
                StoreError::Integrity(
                    "WorkspaceRoot observation receipt count is invalid".to_owned(),
                )
            })?,
            observations_marked_stale: u32::try_from(stale).map_err(|_| {
                StoreError::Integrity(
                    "WorkspaceRoot stale observation receipt count is invalid".to_owned(),
                )
            })?,
        });
    }
    let current: bool = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM workspace_roots root
                JOIN runtimes runtime
                  ON runtime.runtime_id = ?4
                 AND runtime.current_incarnation_id = ?5
                JOIN resource_locations location
                  ON location.location_id = root.location_id
                 AND location.resource_id = root.resource_id
                 AND location.availability = 'AVAILABLE'
                 AND (location.runtime_id IS NULL OR location.runtime_id = runtime.runtime_id)
                JOIN runtime_workspace_bindings workspace_binding
                  ON workspace_binding.workspace_id = root.workspace_id
                 AND workspace_binding.runtime_id = runtime.runtime_id
                 AND workspace_binding.status = 'ACTIVE'
                JOIN resource_location_bindings locator_binding
                  ON locator_binding.location_id = root.location_id
                 AND locator_binding.runtime_id = runtime.runtime_id
                 AND locator_binding.runtime_incarnation_id = runtime.current_incarnation_id
                JOIN file_identity_bindings identity_binding
                  ON identity_binding.location_id = root.location_id
                 AND identity_binding.runtime_id = runtime.runtime_id
                 AND identity_binding.runtime_incarnation_id = runtime.current_incarnation_id
                WHERE root.workspace_id = ?1
                  AND root.workspace_root_id = ?2
                  AND root.version = ?3
                  AND root.status = 'ACTIVE'
            )",
            params![
                batch.workspace_id,
                batch.workspace_root_id,
                to_sql_i64(batch.expected_root_version)?,
                batch.runtime_id,
                batch.runtime_incarnation_id,
            ],
            |row| row.get(0),
        )
        .map_err(map_observation_database_error)?;
    if !current {
        return Err(StoreError::Conflict {
            expected: Some(batch.expected_root_version),
            actual: None,
        });
    }

    transaction
        .execute(
            "INSERT INTO workspace_root_observation_generations(
                workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id,
                complete_generation, last_complete_scan_request_id, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 0, NULL, ?5)
             ON CONFLICT(workspace_id, workspace_root_id, runtime_incarnation_id)
             DO NOTHING",
            params![
                batch.workspace_id,
                batch.workspace_root_id,
                batch.runtime_id,
                batch.runtime_incarnation_id,
                batch.completed_at,
            ],
        )
        .map_err(map_observation_database_error)?;
    let old_generation: i64 = transaction
        .query_row(
            "SELECT complete_generation FROM workspace_root_observation_generations
             WHERE workspace_id = ?1 AND workspace_root_id = ?2
               AND runtime_id = ?3 AND runtime_incarnation_id = ?4",
            params![
                batch.workspace_id,
                batch.workspace_root_id,
                batch.runtime_id,
                batch.runtime_incarnation_id,
            ],
            |row| row.get(0),
        )
        .map_err(map_observation_database_error)?;
    let new_generation = match batch.status {
        RootObservationScanStatus::Complete => old_generation.checked_add(1).ok_or_else(|| {
            StoreError::Integrity("WorkspaceRoot observation generation overflow".to_owned())
        })?,
        RootObservationScanStatus::Partial => old_generation,
    };
    if batch.status == RootObservationScanStatus::Complete {
        transaction
            .execute(
                "UPDATE workspace_root_observation_generations
                 SET complete_generation = ?5, last_complete_scan_request_id = ?6, updated_at = ?7
                 WHERE workspace_id = ?1 AND workspace_root_id = ?2
                   AND runtime_id = ?3 AND runtime_incarnation_id = ?4",
                params![
                    batch.workspace_id,
                    batch.workspace_root_id,
                    batch.runtime_id,
                    batch.runtime_incarnation_id,
                    new_generation,
                    batch.request_id,
                    batch.completed_at,
                ],
            )
            .map_err(map_observation_database_error)?;
    }

    for observation in &batch.observations {
        let observation_generation = match batch.status {
            RootObservationScanStatus::Complete => new_generation,
            RootObservationScanStatus::Partial => 0,
        };
        transaction
            .execute(
                "INSERT INTO workspace_root_observations(
                    workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id,
                    observation_id, file_identity_digest, resource_id, display_name,
                    display_name_search_key,
                    size_bytes, modified_at, observed_at, completed_generation,
                    last_scan_request_id, freshness
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(workspace_id, workspace_root_id, runtime_incarnation_id, observation_id)
                 DO UPDATE SET
                    file_identity_digest = excluded.file_identity_digest,
                    resource_id = excluded.resource_id,
                    display_name = excluded.display_name,
                    display_name_search_key = excluded.display_name_search_key,
                    size_bytes = excluded.size_bytes,
                    modified_at = excluded.modified_at,
                    observed_at = excluded.observed_at,
                    completed_generation = MAX(
                      workspace_root_observations.completed_generation,
                      excluded.completed_generation
                    ),
                    last_scan_request_id = excluded.last_scan_request_id,
                    freshness = CASE
                      WHEN excluded.completed_generation > workspace_root_observations.completed_generation
                        THEN 'CURRENT'
                      ELSE workspace_root_observations.freshness
                    END",
                params![
                    batch.workspace_id,
                    batch.workspace_root_id,
                    batch.runtime_id,
                    batch.runtime_incarnation_id,
                    observation.observation_id,
                    observation.file_identity_digest,
                    observation.resource_id,
                    observation.display_name,
                    observation.display_name.to_lowercase(),
                    to_sql_i64(observation.size_bytes)?,
                    observation.modified_at,
                    observation.observed_at,
                    observation_generation,
                    batch.request_id,
                    match batch.status {
                        RootObservationScanStatus::Complete => "CURRENT",
                        RootObservationScanStatus::Partial => "UNKNOWN",
                    },
                ],
            )
            .map_err(map_observation_database_error)?;
        transaction
            .execute(
                "INSERT INTO workspace_root_observation_bindings(
                    workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id,
                    observation_id, private_relative_locator, observed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(workspace_id, workspace_root_id, runtime_incarnation_id, observation_id)
                 DO UPDATE SET private_relative_locator = excluded.private_relative_locator,
                               observed_at = excluded.observed_at",
                params![
                    batch.workspace_id,
                    batch.workspace_root_id,
                    batch.runtime_id,
                    batch.runtime_incarnation_id,
                    observation.observation_id,
                    observation.private_relative_locator,
                    observation.observed_at,
                ],
            )
            .map_err(map_observation_database_error)?;
    }

    let marked_stale = if batch.status == RootObservationScanStatus::Complete {
        transaction
            .execute(
                "UPDATE workspace_root_observations
                 SET freshness = 'STALE'
                 WHERE workspace_id = ?1 AND workspace_root_id = ?2
                   AND runtime_id = ?3 AND runtime_incarnation_id = ?4
                   AND last_scan_request_id <> ?5 AND freshness <> 'STALE'",
                params![
                    batch.workspace_id,
                    batch.workspace_root_id,
                    batch.runtime_id,
                    batch.runtime_incarnation_id,
                    batch.request_id,
                ],
            )
            .map_err(map_observation_database_error)?
    } else {
        0
    };
    if batch.status == RootObservationScanStatus::Complete {
        transaction
            .execute(
                "DELETE FROM workspace_root_observation_bindings
                 WHERE workspace_id = ?1 AND workspace_root_id = ?2
                   AND runtime_id = ?3 AND runtime_incarnation_id = ?4
                   AND EXISTS (
                     SELECT 1 FROM workspace_root_observations observation
                     WHERE observation.workspace_id = workspace_root_observation_bindings.workspace_id
                       AND observation.workspace_root_id = workspace_root_observation_bindings.workspace_root_id
                       AND observation.runtime_incarnation_id = workspace_root_observation_bindings.runtime_incarnation_id
                       AND observation.observation_id = workspace_root_observation_bindings.observation_id
                       AND observation.freshness = 'STALE'
                   )",
                params![
                    batch.workspace_id,
                    batch.workspace_root_id,
                    batch.runtime_id,
                    batch.runtime_incarnation_id,
                ],
            )
            .map_err(map_observation_database_error)?;
    }
    let complete_generation = u64::try_from(new_generation).map_err(|_| {
        StoreError::Integrity("WorkspaceRoot observation generation is invalid".to_owned())
    })?;
    let observations_upserted = u32::try_from(batch.observations.len()).map_err(|_| {
        StoreError::Invalid("WorkspaceRoot scan contains too many observations".to_owned())
    })?;
    let observations_marked_stale = u32::try_from(marked_stale).map_err(|_| {
        StoreError::Integrity("WorkspaceRoot stale observation count is invalid".to_owned())
    })?;
    let result = RootObservationCommitSummary {
        complete_generation,
        observations_upserted,
        observations_marked_stale,
    };
    let complete_generation_sql = to_sql_i64(result.complete_generation)?;
    let observations_upserted_sql = i64::from(result.observations_upserted);
    let observations_marked_stale_sql = i64::from(result.observations_marked_stale);
    transaction
        .execute(
            "INSERT INTO workspace_root_observation_batch_receipts(
                workspace_id, workspace_root_id, runtime_id, runtime_incarnation_id,
                request_id, request_digest, scan_status, complete_generation,
                observations_upserted, observations_marked_stale, committed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                batch.workspace_id,
                batch.workspace_root_id,
                batch.runtime_id,
                batch.runtime_incarnation_id,
                batch.request_id,
                batch.request_digest,
                match batch.status {
                    RootObservationScanStatus::Complete => "COMPLETE",
                    RootObservationScanStatus::Partial => "PARTIAL",
                },
                complete_generation_sql,
                observations_upserted_sql,
                observations_marked_stale_sql,
                batch.completed_at,
            ],
        )
        .map_err(map_observation_database_error)?;
    Ok(result)
}

fn validate_batch(batch: &WorkspaceRootObservationBatch) -> Result<(), StoreError> {
    if batch.workspace_id.is_empty()
        || batch.workspace_root_id.is_empty()
        || batch.runtime_id.is_empty()
        || batch.runtime_incarnation_id.is_empty()
        || batch.request_id.is_empty()
        || !is_sha256_digest(&batch.request_digest)
        || batch.expected_root_version == 0
        || batch.observations.len() > MAX_WORKSPACE_ROOT_SCAN_ENTRIES as usize
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot observation batch identity or bounds are invalid".to_owned(),
        ));
    }
    let mut locator_bytes = 0usize;
    let mut observation_ids = std::collections::BTreeSet::new();
    for observation in &batch.observations {
        if !is_sha256_digest(&observation.observation_id)
            || !is_sha256_digest(&observation.file_identity_digest)
            || observation.display_name.is_empty()
            || observation.display_name.len() > 1024
            || observation.display_name.to_lowercase().len() > 2048
            || observation.display_name.contains('\0')
            || observation.display_name.contains('/')
            || observation.display_name.chars().any(char::is_control)
            || !observation_ids.insert(&observation.observation_id)
            || observation.private_relative_locator.is_empty()
            || !is_safe_relative_locator(&observation.private_relative_locator)
        {
            return Err(StoreError::Invalid(
                "WorkspaceRoot observation is malformed".to_owned(),
            ));
        }
        locator_bytes = locator_bytes
            .checked_add(observation.private_relative_locator.len())
            .ok_or_else(|| {
                StoreError::Invalid("WorkspaceRoot observation locator budget overflow".to_owned())
            })?;
        if locator_bytes > MAX_AGGREGATE_LOCATOR_BYTES {
            return Err(StoreError::Invalid(
                "WorkspaceRoot observation locator budget exceeded".to_owned(),
            ));
        }
        to_sql_i64(observation.size_bytes)?;
    }
    Ok(())
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_safe_relative_locator(locator: &[u8]) -> bool {
    if locator.is_empty()
        || locator.len() > 32768
        || locator[0] == b'/'
        || locator.contains(&0)
        || locator.contains(&b'\\')
    {
        return false;
    }
    locator
        .split(|byte| *byte == b'/')
        .all(|part| !part.is_empty() && part != b"." && part != b"..")
}

fn to_sql_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| {
        StoreError::Invalid("WorkspaceRoot observation integer exceeds SQLite range".to_owned())
    })
}

fn parse_freshness(value: &str) -> Result<RootObservationFreshness, StoreError> {
    match value {
        "CURRENT" => Ok(RootObservationFreshness::Current),
        "STALE" => Ok(RootObservationFreshness::Stale),
        "UNKNOWN" => Ok(RootObservationFreshness::Unknown),
        _ => Err(StoreError::Integrity(
            "WorkspaceRoot observation freshness is invalid".to_owned(),
        )),
    }
}

fn map_observation_database_error(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::DatabaseBusy
                || code.code == rusqlite::ErrorCode::DatabaseLocked =>
        {
            StoreError::Busy
        }
        rusqlite::Error::SqliteFailure(code, Some(message))
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Integrity(message)
        }
        other => StoreError::Database(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, TransactionBehavior};

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE workspaces(workspace_id TEXT PRIMARY KEY, owner_principal_id TEXT NOT NULL);
                 CREATE TABLE runtimes(runtime_id TEXT PRIMARY KEY, current_incarnation_id TEXT);
                 CREATE TABLE runtime_incarnations(runtime_id TEXT NOT NULL, runtime_incarnation_id TEXT NOT NULL, PRIMARY KEY(runtime_id, runtime_incarnation_id));
                 CREATE TABLE runtime_workspace_bindings(workspace_id TEXT, runtime_id TEXT, status TEXT);
                 CREATE TABLE resources(workspace_id TEXT NOT NULL, resource_id TEXT NOT NULL, PRIMARY KEY(workspace_id, resource_id));
                 CREATE TABLE resource_locations(location_id TEXT PRIMARY KEY, resource_id TEXT, runtime_id TEXT, availability TEXT);
                 CREATE TABLE resource_location_bindings(location_id TEXT, runtime_id TEXT, runtime_incarnation_id TEXT);
                 CREATE TABLE file_identity_bindings(location_id TEXT, runtime_id TEXT, runtime_incarnation_id TEXT);
                 CREATE TABLE workspace_roots(workspace_id TEXT, workspace_root_id TEXT, resource_id TEXT, location_id TEXT, status TEXT, version INTEGER, PRIMARY KEY(workspace_id, workspace_root_id));",
            )
            .expect("baseline tables");
        connection
            .execute_batch(V21_DDL)
            .expect("v21 observation schema");
        connection
            .execute_batch(
                "INSERT INTO workspaces VALUES ('ws-1', 'owner-1');
                 INSERT INTO runtime_incarnations VALUES ('rt-1', 'inc-1');
                 INSERT INTO runtimes VALUES ('rt-1', 'inc-1');
                 INSERT INTO runtime_workspace_bindings VALUES ('ws-1', 'rt-1', 'ACTIVE');
                 INSERT INTO resources VALUES ('ws-1', 'root-resource');
                 INSERT INTO resource_locations VALUES ('root-location', 'root-resource', 'rt-1', 'AVAILABLE');
                 INSERT INTO resource_location_bindings VALUES ('root-location', 'rt-1', 'inc-1');
                 INSERT INTO file_identity_bindings VALUES ('root-location', 'rt-1', 'inc-1');
                 INSERT INTO workspace_roots VALUES ('ws-1', 'root-1', 'root-resource', 'root-location', 'ACTIVE', 7);",
            )
            .expect("current root/runtime binding");
        connection
    }

    fn observation(id: &str, basename: &str, locator: &[u8]) -> WorkspaceRootFileObservation {
        WorkspaceRootFileObservation {
            observation_id: format!("sha256:{}", id.repeat(64 / id.len())),
            file_identity_digest: format!(
                "sha256:{}",
                (id.chars().next().unwrap_or('a').to_string()).repeat(64)
            ),
            resource_id: None,
            display_name: basename.to_owned(),
            size_bytes: 12,
            modified_at: Some("2026-10-17T12:00:00Z".to_owned()),
            observed_at: "2026-10-17T12:01:00Z".to_owned(),
            private_relative_locator: locator.to_vec(),
        }
    }

    fn batch(
        scan_id: &str,
        status: RootObservationScanStatus,
        observations: Vec<WorkspaceRootFileObservation>,
    ) -> WorkspaceRootObservationBatch {
        WorkspaceRootObservationBatch {
            workspace_id: "ws-1".to_owned(),
            workspace_root_id: "root-1".to_owned(),
            expected_root_version: 7,
            runtime_id: "rt-1".to_owned(),
            runtime_incarnation_id: "inc-1".to_owned(),
            request_id: scan_id.to_owned(),
            request_digest: format!("sha256:{}", "a".repeat(64)),
            status,
            observations,
            completed_at: "2026-10-17T12:02:00Z".to_owned(),
        }
    }

    #[test]
    fn complete_reconciliation_marks_unseen_observations_stale_without_deleting_them() {
        let mut connection = database();
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-1",
                RootObservationScanStatus::Complete,
                vec![
                    observation("b", "one.txt", b"one.txt"),
                    observation("c", "two.txt", b"two.txt"),
                ],
            ),
        )
        .unwrap();
        tx.commit().unwrap();

        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-2",
                RootObservationScanStatus::Complete,
                vec![observation("b", "one.txt", b"one.txt")],
            ),
        )
        .unwrap();
        tx.commit().unwrap();

        let rows: Vec<(String, String)> = connection.prepare("SELECT display_name, freshness FROM workspace_root_observations ORDER BY display_name").unwrap().query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(
            rows,
            [
                ("one.txt".to_owned(), "CURRENT".to_owned()),
                ("two.txt".to_owned(), "STALE".to_owned())
            ]
        );
        let private_binding_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_root_observation_bindings",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(private_binding_count, 1);
    }

    #[test]
    fn partial_reconciliation_updates_seen_files_but_never_changes_unseen_freshness() {
        let mut connection = database();
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-1",
                RootObservationScanStatus::Complete,
                vec![
                    observation("b", "one.txt", b"one.txt"),
                    observation("c", "two.txt", b"two.txt"),
                    observation("e", "three.txt", b"three.txt"),
                ],
            ),
        )
        .unwrap();
        tx.commit().unwrap();

        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-2",
                RootObservationScanStatus::Complete,
                vec![observation("b", "one.txt", b"one.txt")],
            ),
        )
        .unwrap();
        tx.commit().unwrap();

        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-3",
                RootObservationScanStatus::Partial,
                vec![
                    observation("b", "one.txt", b"one.txt"),
                    observation("c", "two.txt", b"two.txt"),
                    observation("d", "partial-only.txt", b"partial-only.txt"),
                ],
            ),
        )
        .unwrap();
        tx.commit().unwrap();

        let seen_in_partial: String = connection
            .query_row(
                "SELECT freshness FROM workspace_root_observations WHERE display_name = 'one.txt'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(seen_in_partial, "CURRENT");
        let stale_seen_in_partial: (String, i64) = connection
            .query_row(
                "SELECT freshness, completed_generation FROM workspace_root_observations WHERE display_name = 'two.txt'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stale_seen_in_partial, ("STALE".to_owned(), 1));
        let unseen: String = connection
            .query_row(
                "SELECT freshness FROM workspace_root_observations WHERE display_name = 'three.txt'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unseen, "STALE");
        let partial_only: (String, i64) = connection
            .query_row(
                "SELECT freshness, completed_generation FROM workspace_root_observations WHERE display_name = 'partial-only.txt'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(partial_only, ("UNKNOWN".to_owned(), 0));

        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-4",
                RootObservationScanStatus::Complete,
                vec![observation("d", "partial-only.txt", b"partial-only.txt")],
            ),
        )
        .unwrap();
        tx.commit().unwrap();
        let promoted: (String, i64) = connection
            .query_row(
                "SELECT freshness, completed_generation FROM workspace_root_observations WHERE display_name = 'partial-only.txt'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(promoted, ("CURRENT".to_owned(), 3));
        let absent_after_complete: String = connection
            .query_row(
                "SELECT freshness FROM workspace_root_observations WHERE display_name = 'one.txt'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(absent_after_complete, "STALE");
    }

    #[test]
    fn stale_root_or_runtime_binding_rejects_batch_without_writes() {
        let mut connection = database();
        connection
            .execute(
                "UPDATE runtimes SET current_incarnation_id = 'inc-new' WHERE runtime_id = 'rt-1'",
                [],
            )
            .unwrap();
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        assert!(
            reconcile_workspace_root_observations_in_transaction(
                &tx,
                &batch(
                    "scan-1",
                    RootObservationScanStatus::Complete,
                    vec![observation("b", "one.txt", b"one.txt")]
                )
            )
            .is_err()
        );
        drop(tx);
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_root_observations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn exact_batch_replay_precedes_current_runtime_fence_and_changed_replay_conflicts() {
        let mut connection = database();
        let original = batch(
            "scan-1",
            RootObservationScanStatus::Complete,
            vec![observation("b", "one.txt", b"one.txt")],
        );
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let first = reconcile_workspace_root_observations_in_transaction(&tx, &original).unwrap();
        tx.commit().unwrap();
        connection
            .execute(
                "UPDATE runtimes SET current_incarnation_id = 'inc-new' WHERE runtime_id = 'rt-1'",
                [],
            )
            .unwrap();

        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let replay = reconcile_workspace_root_observations_in_transaction(&tx, &original).unwrap();
        assert_eq!(replay, first);
        tx.commit().unwrap();

        let mut changed = batch(
            "scan-1",
            RootObservationScanStatus::Complete,
            vec![observation("c", "different.txt", b"different.txt")],
        );
        changed.request_digest = format!("sha256:{}", "f".repeat(64));
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        assert!(matches!(
            reconcile_workspace_root_observations_in_transaction(&tx, &changed),
            Err(StoreError::IdempotencyConflict)
        ));
        drop(tx);
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_root_observations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn malformed_and_escaping_private_locators_are_rejected() {
        let mut escaped = observation("b", "one.txt", b"../outside.txt");
        let mut batch = batch("scan-1", RootObservationScanStatus::Complete, vec![escaped]);
        assert!(matches!(
            validate_batch(&batch),
            Err(StoreError::Invalid(_))
        ));
        batch.observations[0].private_relative_locator = b"inside/../outside.txt".to_vec();
        assert!(matches!(
            validate_batch(&batch),
            Err(StoreError::Invalid(_))
        ));
        batch.observations[0].private_relative_locator = b"inside/one.txt".to_vec();
        assert!(validate_batch(&batch).is_ok());
        escaped = observation("b", "one.txt", b"/outside.txt");
        batch.observations = vec![escaped];
        assert!(matches!(
            validate_batch(&batch),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn root_search_uses_literal_case_insensitive_names_and_keyset_pages() {
        let mut connection = database();
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-1",
                RootObservationScanStatus::Complete,
                vec![
                    observation("b", "Report%_One.txt", b"private/Report%_One.txt"),
                    observation("c", "ReportXX_One.txt", b"private/ReportXX_One.txt"),
                    observation("d", "report-z.txt", b"private/report-z.txt"),
                    observation("f", "RÉSUMÉ.txt", "private/RÉSUMÉ.txt".as_bytes()),
                ],
            ),
        )
        .unwrap();
        tx.commit().unwrap();

        let matching = search_workspace_root_observations(
            &connection,
            &RootObservationSearchRequest {
                workspace_id: "ws-1".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                workspace_root_id: "root-1".to_owned(),
                expected_root_version: 7,
                runtime_id: "rt-1".to_owned(),
                runtime_incarnation_id: "inc-1".to_owned(),
                query: "%_".to_owned(),
                freshness: Some(RootObservationFreshness::Current),
                after_display_name: None,
                after_observation_id: None,
                limit: 10,
            },
        )
        .unwrap();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].display_name, "Report%_One.txt");

        let unicode_matching = search_workspace_root_observations(
            &connection,
            &RootObservationSearchRequest {
                workspace_id: "ws-1".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                workspace_root_id: "root-1".to_owned(),
                expected_root_version: 7,
                runtime_id: "rt-1".to_owned(),
                runtime_incarnation_id: "inc-1".to_owned(),
                query: "résumé".to_owned(),
                freshness: None,
                after_display_name: None,
                after_observation_id: None,
                limit: 10,
            },
        )
        .unwrap();
        assert_eq!(unicode_matching.len(), 1);
        assert_eq!(unicode_matching[0].display_name, "RÉSUMÉ.txt");

        let first_page = search_workspace_root_observations(
            &connection,
            &RootObservationSearchRequest {
                workspace_id: "ws-1".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                workspace_root_id: "root-1".to_owned(),
                expected_root_version: 7,
                runtime_id: "rt-1".to_owned(),
                runtime_incarnation_id: "inc-1".to_owned(),
                query: "REPORT".to_owned(),
                freshness: None,
                after_display_name: None,
                after_observation_id: None,
                limit: 1,
            },
        )
        .unwrap();
        assert_eq!(
            first_page.len(),
            2,
            "search reads one extra row for has-more"
        );
        let next_page = search_workspace_root_observations(
            &connection,
            &RootObservationSearchRequest {
                workspace_id: "ws-1".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                workspace_root_id: "root-1".to_owned(),
                expected_root_version: 7,
                runtime_id: "rt-1".to_owned(),
                runtime_incarnation_id: "inc-1".to_owned(),
                query: "REPORT".to_owned(),
                freshness: None,
                after_display_name: Some(first_page[0].display_name.clone()),
                after_observation_id: Some(first_page[0].observation_id.clone()),
                limit: 1,
            },
        )
        .unwrap();
        assert_eq!(
            next_page.len(),
            2,
            "the next page also includes a has-more probe"
        );
        assert_ne!(first_page[0].observation_id, next_page[0].observation_id);
        assert!(!format!("{:?}", next_page[0]).contains("private/"));
    }

    #[test]
    fn root_search_fails_closed_for_paused_roots_and_non_current_incarnations() {
        let mut connection = database();
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-1",
                RootObservationScanStatus::Complete,
                vec![observation("b", "visible.txt", b"visible.txt")],
            ),
        )
        .unwrap();
        tx.commit().unwrap();
        let request = RootObservationSearchRequest {
            workspace_id: "ws-1".to_owned(),
            owner_principal_id: "owner-1".to_owned(),
            workspace_root_id: "root-1".to_owned(),
            expected_root_version: 7,
            runtime_id: "rt-1".to_owned(),
            runtime_incarnation_id: "inc-1".to_owned(),
            query: String::new(),
            freshness: None,
            after_display_name: None,
            after_observation_id: None,
            limit: 10,
        };
        connection
            .execute(
                "UPDATE workspace_roots SET status = 'PAUSED' WHERE workspace_root_id = 'root-1'",
                [],
            )
            .unwrap();
        assert_eq!(
            search_workspace_root_observations(&connection, &request).unwrap_err(),
            StoreError::NotFound,
            "a paused root is not discoverable through observed-file search",
        );
        connection
            .execute(
                "UPDATE workspace_roots SET status = 'ACTIVE' WHERE workspace_root_id = 'root-1'",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE resource_locations SET availability = 'UNAVAILABLE' WHERE location_id = 'root-location'",
                [],
            )
            .unwrap();
        assert!(
            search_workspace_root_observations(&connection, &request)
                .unwrap()
                .is_empty()
        );
        connection
            .execute(
                "UPDATE resource_locations SET availability = 'AVAILABLE' WHERE location_id = 'root-location'",
                [],
            )
            .unwrap();
        let mut old_incarnation = request;
        old_incarnation.runtime_incarnation_id = "inc-old".to_owned();
        assert!(
            search_workspace_root_observations(&connection, &old_incarnation)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn root_search_requires_workspace_owner_and_current_root_version() {
        let connection = database();
        let mut request = RootObservationSearchRequest {
            workspace_id: "ws-1".to_owned(),
            owner_principal_id: "somebody-else".to_owned(),
            workspace_root_id: "root-1".to_owned(),
            expected_root_version: 7,
            runtime_id: "rt-1".to_owned(),
            runtime_incarnation_id: "inc-1".to_owned(),
            query: String::new(),
            freshness: None,
            after_display_name: None,
            after_observation_id: None,
            limit: 10,
        };
        assert_eq!(
            search_workspace_root_observations(&connection, &request).unwrap_err(),
            StoreError::NotFound,
        );
        request.owner_principal_id = "owner-1".to_owned();
        request.expected_root_version = 6;
        assert_eq!(
            search_workspace_root_observations(&connection, &request).unwrap_err(),
            StoreError::Conflict {
                expected: Some(6),
                actual: Some(7),
            },
        );
    }

    #[test]
    fn root_revocation_removes_private_locators_but_retains_observation_history() {
        let mut connection = database();
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        reconcile_workspace_root_observations_in_transaction(
            &tx,
            &batch(
                "scan-1",
                RootObservationScanStatus::Complete,
                vec![observation("b", "private.txt", b"private/private.txt")],
            ),
        )
        .unwrap();
        tx.commit().unwrap();
        connection
            .execute(
                "UPDATE workspace_roots SET status = 'REVOKED' WHERE workspace_root_id = 'root-1'",
                [],
            )
            .unwrap();
        let locator_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_root_observation_bindings",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let observation_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_root_observations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(locator_count, 0);
        assert_eq!(observation_count, 1);
    }

    #[test]
    fn private_relative_locator_is_not_printed_in_debug_output() {
        let value = observation("b", "one.txt", b"secret/owner/path/one.txt");
        assert!(!format!("{value:?}").contains("secret/owner/path"));
    }
}
