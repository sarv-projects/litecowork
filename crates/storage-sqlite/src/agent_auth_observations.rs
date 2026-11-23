//! Runtime-local native authentication observations (Feature 002 T07 / v35).

use super::{Command, SqliteWorkspaceStore, map_database_error};
use crate::{StoreError, canonicalize_utc_timestamp};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::sync::mpsc;
use storage_core::{
    AgentAuthObservationKey, AgentAuthObservationRecord, AgentAuthObservationReservation,
    AgentAuthObservationScope, AgentAuthObservationState, AgentAuthObservationStore,
    validate_agent_auth_observation_key, validate_agent_auth_observation_scope,
};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

const AUTHENTICATING_MAX_SECONDS: i64 = 300;
const AUTH_OUTCOME_MAX_SECONDS: i64 = 900;
const MAX_DESCRIPTOR_AUTH_METHODS: usize = 256;

impl AgentAuthObservationStore for SqliteWorkspaceStore {
    fn reserve_agent_auth_observation(
        &self,
        key: AgentAuthObservationKey,
        observed_at: &str,
        expires_at: &str,
    ) -> Result<AgentAuthObservationReservation, StoreError> {
        validate_agent_auth_observation_key(&key)?;
        let (observed_at, expires_at) =
            normalize_window(observed_at, expires_at, AUTHENTICATING_MAX_SECONDS)?;
        let key_for_operation = key.clone();
        self.run_auth_observation_operation(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            // Expired local observations are no longer evidence. Opportunistic
            // bounded cleanup keeps old Runtime incarnations from accumulating.
            transaction
                .execute(
                    "DELETE FROM agent_auth_observations
                     WHERE expires_at <= ?1
                       AND NOT (agent_profile_id = ?2 AND runtime_id = ?3
                         AND runtime_incarnation_id = ?4 AND module_id = ?5
                         AND module_version = ?6 AND auth_method_id = ?7)",
                    params![
                        observed_at,
                        key_for_operation.agent_profile_id,
                        key_for_operation.runtime_id,
                        key_for_operation.runtime_incarnation_id,
                        key_for_operation.module_id,
                        key_for_operation.module_version,
                        key_for_operation.auth_method_id,
                    ],
                )
                .map_err(map_database_error)?;
            if let Some(existing) = read_exact(&transaction, &key_for_operation)?
                && existing.state == AgentAuthObservationState::Authenticating
                && existing.expires_at > observed_at
            {
                transaction.commit().map_err(map_database_error)?;
                return Ok(AgentAuthObservationReservation::InFlight(existing));
            }
            let prior_generation = read_exact(&transaction, &key_for_operation)?
                .map(|record| record.generation)
                .unwrap_or(0);
            let generation = prior_generation.checked_add(1).ok_or_else(|| {
                StoreError::Invalid("agent auth observation generation exhausted".to_owned())
            })?;
            let generation_sql = i64::try_from(generation).map_err(|_| {
                StoreError::Invalid("agent auth observation generation exhausted".to_owned())
            })?;
            transaction
                .execute(
                    "INSERT INTO agent_auth_observations(
                       agent_profile_id, runtime_id, runtime_incarnation_id, module_id,
                       module_version, auth_method_id, state, generation, observed_at, expires_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'AUTHENTICATING', ?7, ?8, ?9)
                     ON CONFLICT(
                       agent_profile_id, runtime_id, runtime_incarnation_id,
                       module_id, module_version, auth_method_id
                     ) DO UPDATE SET
                       state = 'AUTHENTICATING', generation = excluded.generation,
                       observed_at = excluded.observed_at, expires_at = excluded.expires_at",
                    params![
                        key_for_operation.agent_profile_id,
                        key_for_operation.runtime_id,
                        key_for_operation.runtime_incarnation_id,
                        key_for_operation.module_id,
                        key_for_operation.module_version,
                        key_for_operation.auth_method_id,
                        generation_sql,
                        observed_at,
                        expires_at,
                    ],
                )
                .map_err(map_database_error)?;
            transaction.commit().map_err(map_database_error)?;
            Ok(AgentAuthObservationReservation::Reserved {
                generation,
                expires_at,
            })
        })
    }

    fn settle_agent_auth_observation(
        &self,
        key: AgentAuthObservationKey,
        generation: u64,
        state: AgentAuthObservationState,
        observed_at: &str,
        expires_at: &str,
    ) -> Result<bool, StoreError> {
        validate_agent_auth_observation_key(&key)?;
        if state == AgentAuthObservationState::Authenticating {
            return Err(StoreError::Invalid(
                "agent auth settlement must be a terminal observation".to_owned(),
            ));
        }
        let generation_sql = i64::try_from(generation).map_err(|_| {
            StoreError::Invalid("agent auth observation generation is invalid".to_owned())
        })?;
        if generation_sql < 1 {
            return Err(StoreError::Invalid(
                "agent auth observation generation is invalid".to_owned(),
            ));
        }
        let (observed_at, expires_at) =
            normalize_window(observed_at, expires_at, AUTH_OUTCOME_MAX_SECONDS)?;
        self.run_auth_observation_operation(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let changed = transaction
                .execute(
                    "UPDATE agent_auth_observations
                     SET state = ?1, observed_at = ?2, expires_at = ?3
                     WHERE agent_profile_id = ?4 AND runtime_id = ?5
                       AND runtime_incarnation_id = ?6 AND module_id = ?7
                       AND module_version = ?8 AND auth_method_id = ?9
                       AND generation = ?10 AND state = 'AUTHENTICATING'
                       AND expires_at > ?2",
                    params![
                        state.as_str(),
                        observed_at,
                        expires_at,
                        key.agent_profile_id,
                        key.runtime_id,
                        key.runtime_incarnation_id,
                        key.module_id,
                        key.module_version,
                        key.auth_method_id,
                        generation_sql,
                    ],
                )
                .map_err(map_database_error)?;
            transaction.commit().map_err(map_database_error)?;
            Ok(changed == 1)
        })
    }

    fn current_agent_auth_observation(
        &self,
        key: AgentAuthObservationKey,
        now: &str,
    ) -> Result<Option<AgentAuthObservationRecord>, StoreError> {
        validate_agent_auth_observation_key(&key)?;
        let now = canonicalize_utc_timestamp(now)?;
        self.run_auth_observation_operation(move |connection| {
            Ok(read_exact(connection, &key)?.filter(|record| record.expires_at > now))
        })
    }

    fn list_agent_auth_observations(
        &self,
        scope: AgentAuthObservationScope,
        now: &str,
    ) -> Result<Vec<AgentAuthObservationRecord>, StoreError> {
        validate_agent_auth_observation_scope(&scope)?;
        let now = canonicalize_utc_timestamp(now)?;
        self.run_auth_observation_operation(move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT agent_profile_id, runtime_id, runtime_incarnation_id, module_id,
                            module_version, auth_method_id, state, generation, observed_at, expires_at
                     FROM agent_auth_observations
                     WHERE agent_profile_id = ?1 AND runtime_id = ?2
                       AND runtime_incarnation_id = ?3 AND module_id = ?4
                       AND module_version = ?5 AND expires_at > ?6
                     ORDER BY observed_at DESC, auth_method_id
                     LIMIT ?7",
                )
                .map_err(map_database_error)?;
            let records = statement
                .query_map(
                    params![
                        scope.agent_profile_id,
                        scope.runtime_id,
                        scope.runtime_incarnation_id,
                        scope.module_id,
                        scope.module_version,
                        now,
                        i64::try_from(MAX_DESCRIPTOR_AUTH_METHODS + 1).expect("small limit"),
                    ],
                    observation_from_row,
                )
                .map_err(map_database_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_database_error)?;
            if records.len() > MAX_DESCRIPTOR_AUTH_METHODS {
                return Err(StoreError::CorruptSchema(
                    "agent auth observation set exceeds the descriptor method limit".to_owned(),
                ));
            }
            Ok(records)
        })
    }
}

impl SqliteWorkspaceStore {
    fn run_auth_observation_operation<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, receive) = mpsc::channel();
        self.execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

fn normalize_window(
    observed_at: &str,
    expires_at: &str,
    max_seconds: i64,
) -> Result<(String, String), StoreError> {
    let observed_at = canonicalize_utc_timestamp(observed_at)?;
    let expires_at = canonicalize_utc_timestamp(expires_at)?;
    let observed = OffsetDateTime::parse(&observed_at, &Rfc3339)
        .map_err(|_| StoreError::Invalid("auth observation time is invalid".to_owned()))?;
    let expires = OffsetDateTime::parse(&expires_at, &Rfc3339)
        .map_err(|_| StoreError::Invalid("auth observation time is invalid".to_owned()))?;
    if expires <= observed || expires - observed > Duration::seconds(max_seconds) {
        return Err(StoreError::Invalid(
            "auth observation expiry is invalid or exceeds its bounded window".to_owned(),
        ));
    }
    Ok((observed_at, expires_at))
}

fn read_exact(
    connection: &Connection,
    key: &AgentAuthObservationKey,
) -> Result<Option<AgentAuthObservationRecord>, StoreError> {
    connection
        .query_row(
            "SELECT agent_profile_id, runtime_id, runtime_incarnation_id, module_id,
                    module_version, auth_method_id, state, generation, observed_at, expires_at
             FROM agent_auth_observations
             WHERE agent_profile_id = ?1 AND runtime_id = ?2
               AND runtime_incarnation_id = ?3 AND module_id = ?4
               AND module_version = ?5 AND auth_method_id = ?6",
            params![
                key.agent_profile_id,
                key.runtime_id,
                key.runtime_incarnation_id,
                key.module_id,
                key.module_version,
                key.auth_method_id,
            ],
            observation_from_row,
        )
        .optional()
        .map_err(map_database_error)
}

fn observation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentAuthObservationRecord> {
    let state: String = row.get(6)?;
    let state = AgentAuthObservationState::from_str(&state).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unknown agent auth observation state",
            )),
        )
    })?;
    let generation: i64 = row.get(7)?;
    let generation = u64::try_from(generation).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    Ok(AgentAuthObservationRecord {
        key: AgentAuthObservationKey {
            agent_profile_id: row.get(0)?,
            runtime_id: row.get(1)?,
            runtime_incarnation_id: row.get(2)?,
            module_id: row.get(3)?,
            module_version: row.get(4)?,
            auth_method_id: row.get(5)?,
        },
        state,
        generation,
        observed_at: row.get(8)?,
        expires_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use std::sync::{Arc, Barrier};
    use storage_core::BlobPurpose;
    use zeroize::Zeroizing;

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
                bytes: Zeroizing::new([41; 32]),
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

    fn open_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
        let blobs = Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys));
        SqliteWorkspaceStore::open(
            directory.path().join("litecowork.sqlite3"),
            blobs,
            SqliteConfig::default(),
        )
        .expect("storage opens with v35 migration")
    }

    fn key(
        profile: &str,
        runtime: &str,
        incarnation: &str,
        method: &str,
    ) -> AgentAuthObservationKey {
        AgentAuthObservationKey {
            agent_profile_id: profile.to_owned(),
            runtime_id: runtime.to_owned(),
            runtime_incarnation_id: incarnation.to_owned(),
            module_id: "opencode".to_owned(),
            module_version: "1.0.0".to_owned(),
            auth_method_id: method.to_owned(),
        }
    }

    fn timestamp(epoch: i64) -> String {
        let formatted = OffsetDateTime::from_unix_timestamp(epoch)
            .expect("timestamp")
            .format(&Rfc3339)
            .expect("RFC3339 timestamp");
        canonicalize_utc_timestamp(&formatted).expect("canonical RFC3339 timestamp")
    }

    #[test]
    fn exact_profile_runtime_incarnation_and_method_scope_is_persistent() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = open_store(&directory);
        let exact = key("profile-a", "runtime-a", "incarnation-a", "browser-login");
        let reservation = store
            .reserve_agent_auth_observation(exact.clone(), &timestamp(100), &timestamp(400))
            .expect("reserve exact profile observation");
        assert_eq!(
            reservation,
            AgentAuthObservationReservation::Reserved {
                generation: 1,
                expires_at: timestamp(400),
            }
        );
        store
            .settle_agent_auth_observation(
                exact.clone(),
                1,
                AgentAuthObservationState::Authenticated,
                &timestamp(120),
                &timestamp(900),
            )
            .expect("store module-reported outcome");

        drop(store);
        let reopened = open_store(&directory);
        assert_eq!(
            reopened
                .current_agent_auth_observation(exact.clone(), &timestamp(121))
                .expect("read persisted exact scope")
                .expect("current exact row")
                .state,
            AgentAuthObservationState::Authenticated
        );
        for wrong_scope in [
            key("profile-b", "runtime-a", "incarnation-a", "browser-login"),
            key("profile-a", "runtime-b", "incarnation-a", "browser-login"),
            key("profile-a", "runtime-a", "incarnation-b", "browser-login"),
            key("profile-a", "runtime-a", "incarnation-a", "other-method"),
        ] {
            assert!(
                reopened
                    .current_agent_auth_observation(wrong_scope, &timestamp(121))
                    .expect("read different scope")
                    .is_none()
            );
        }
        let listed = reopened
            .list_agent_auth_observations(exact.scope(), &timestamp(121))
            .expect("list exact descriptor scope");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].key, exact);
    }

    #[test]
    fn immediate_reservation_is_single_flight_across_concurrent_callers() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = open_store(&directory);
        let barrier = Arc::new(Barrier::new(8));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let store = store.clone();
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                store
                    .reserve_agent_auth_observation(
                        key("profile-a", "runtime-a", "incarnation-a", "browser-login"),
                        &timestamp(100),
                        &timestamp(400),
                    )
                    .expect("reserve or observe the existing flight")
            }));
        }
        let outcomes: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("reservation worker completes"))
            .collect();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    AgentAuthObservationReservation::Reserved { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, AgentAuthObservationReservation::InFlight(_)))
                .count(),
            7
        );
    }

    #[test]
    fn expired_attempt_advances_generation_and_rejects_a_late_outcome() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = open_store(&directory);
        let key = key("profile-a", "runtime-a", "incarnation-a", "browser-login");
        assert_eq!(
            store
                .reserve_agent_auth_observation(key.clone(), &timestamp(100), &timestamp(400))
                .expect("first generation"),
            AgentAuthObservationReservation::Reserved {
                generation: 1,
                expires_at: timestamp(400),
            }
        );
        assert_eq!(
            store
                .reserve_agent_auth_observation(key.clone(), &timestamp(401), &timestamp(701))
                .expect("expired flight is replaced"),
            AgentAuthObservationReservation::Reserved {
                generation: 2,
                expires_at: timestamp(701),
            }
        );
        assert!(
            !store
                .settle_agent_auth_observation(
                    key.clone(),
                    1,
                    AgentAuthObservationState::Authenticated,
                    &timestamp(402),
                    &timestamp(800),
                )
                .expect("late generation is ignored")
        );
        assert!(
            store
                .settle_agent_auth_observation(
                    key.clone(),
                    2,
                    AgentAuthObservationState::NeedsAuth,
                    &timestamp(402),
                    &timestamp(800),
                )
                .expect("current generation settles")
        );
        assert_eq!(
            store
                .current_agent_auth_observation(key, &timestamp(403))
                .expect("read settled observation")
                .expect("settled row")
                .state,
            AgentAuthObservationState::NeedsAuth
        );
    }

    #[test]
    fn invalid_identity_state_and_unbounded_expiry_fail_closed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = open_store(&directory);
        let mut invalid = key("profile-a", "runtime-a", "incarnation-a", "browser-login");
        invalid.agent_profile_id = "profile\nforged".to_owned();
        assert!(
            store
                .reserve_agent_auth_observation(invalid, &timestamp(100), &timestamp(400))
                .is_err()
        );
        assert!(
            store
                .reserve_agent_auth_observation(
                    key("profile-a", "runtime-a", "incarnation-a", "browser-login"),
                    &timestamp(100),
                    &timestamp(401),
                )
                .is_err()
        );
        assert!(
            store
                .settle_agent_auth_observation(
                    key("profile-a", "runtime-a", "incarnation-a", "browser-login"),
                    1,
                    AgentAuthObservationState::Authenticating,
                    &timestamp(100),
                    &timestamp(200),
                )
                .is_err()
        );
    }
}
