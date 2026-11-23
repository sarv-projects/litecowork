//! Runtime-local receipts for replay-safe credential writes across SQLite and OS keyrings.

use super::{Command, SqliteWorkspaceStore, canonical_json, digest, map_database_error};
use crate::{StoreError, canonicalize_utc_timestamp, workspace_owner_active};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::json;
use std::sync::mpsc;
use storage_core::{
    AgentCredentialSubmissionKey, AgentCredentialSubmissionReceipt,
    AgentCredentialSubmissionReservation, AgentCredentialSubmissionState,
    AgentCredentialSubmissionStore, SecretKind, validate_agent_credential_submission_key,
    validate_agent_credential_submission_reservation,
    validate_agent_credential_submission_settlement,
};

impl AgentCredentialSubmissionStore for SqliteWorkspaceStore {
    fn get_agent_credential_submission(
        &self,
        key: AgentCredentialSubmissionKey,
    ) -> Result<Option<AgentCredentialSubmissionReceipt>, StoreError> {
        validate_agent_credential_submission_key(&key)?;
        self.run_credential_submission_operation(move |connection| {
            workspace_owner_matches(connection, &key.workspace_id, &key.principal_id)?;
            read_receipt(connection, &key)
        })
    }

    fn reserve_agent_credential_submission(
        &self,
        mut reservation: AgentCredentialSubmissionReservation,
    ) -> Result<AgentCredentialSubmissionReceipt, StoreError> {
        validate_agent_credential_submission_reservation(&reservation)?;
        reservation.created_at = canonicalize_utc_timestamp(&reservation.created_at)?;
        self.run_credential_submission_operation(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            workspace_owner_active(
                &transaction,
                &reservation.key.principal_id,
                &reservation.key.workspace_id,
            )?;
            if let Some(existing) = read_receipt(&transaction, &reservation.key)? {
                if !fingerprints_equal(
                    &existing.request_fingerprint,
                    &reservation.request_fingerprint,
                ) {
                    return Err(StoreError::IdempotencyConflict);
                }
                if existing.secret_kind != reservation.secret_kind {
                    return Err(StoreError::Integrity(
                        "credential retry descriptor changed SecretKind under the same request fingerprint".to_owned(),
                    ));
                }
                transaction.commit().map_err(map_database_error)?;
                return Ok(existing);
            }

            transaction
                .execute(
                    "INSERT INTO agent_credential_submission_receipts(
                       workspace_id, principal_id, agent_profile_id, credential_slot_id,
                       idempotency_key_digest, request_fingerprint, secret_kind, secret_ref_id,
                       state, provider_ref, created_at, committed_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'PENDING', NULL, ?9, NULL)",
                    params![
                        reservation.key.workspace_id,
                        reservation.key.principal_id,
                        reservation.key.agent_profile_id,
                        reservation.key.credential_slot_id,
                        reservation.key.idempotency_key_digest,
                        reservation.request_fingerprint,
                        secret_kind_as_str(reservation.secret_kind),
                        reservation.secret_ref_id,
                        reservation.created_at,
                    ],
                )
                .map_err(map_database_error)?;
            let receipt = read_receipt(&transaction, &reservation.key)?
                .ok_or_else(|| StoreError::CorruptSchema("credential receipt insert disappeared".to_owned()))?;
            transaction.commit().map_err(map_database_error)?;
            Ok(receipt)
        })
    }

    fn commit_agent_credential_submission(
        &self,
        key: AgentCredentialSubmissionKey,
        request_fingerprint: String,
        provider_ref: String,
        committed_at: String,
    ) -> Result<AgentCredentialSubmissionReceipt, StoreError> {
        validate_agent_credential_submission_settlement(
            &key,
            &request_fingerprint,
            &provider_ref,
            &committed_at,
        )?;
        let committed_at = canonicalize_utc_timestamp(&committed_at)?;
        self.run_credential_submission_operation(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let current = read_receipt(&transaction, &key)?.ok_or(StoreError::NotFound)?;
            if !fingerprints_equal(&current.request_fingerprint, &request_fingerprint) {
                return Err(StoreError::IdempotencyConflict);
            }
            if current.state == AgentCredentialSubmissionState::Committed {
                if current.provider_ref.as_deref() != Some(provider_ref.as_str()) {
                    return Err(StoreError::Integrity(
                        "credential receipt was settled with a different provider reference"
                            .to_owned(),
                    ));
                }
                transaction.commit().map_err(map_database_error)?;
                return Ok(current);
            }

            let changed = transaction
                .execute(
                    "UPDATE agent_credential_submission_receipts
                     SET state = 'COMMITTED', provider_ref = ?1, committed_at = ?2
                     WHERE workspace_id = ?3 AND principal_id = ?4 AND agent_profile_id = ?5
                       AND credential_slot_id = ?6 AND idempotency_key_digest = ?7
                       AND request_fingerprint = ?8 AND state = 'PENDING'
                       AND secret_ref_id = ?9",
                    params![
                        provider_ref,
                        committed_at,
                        key.workspace_id,
                        key.principal_id,
                        key.agent_profile_id,
                        key.credential_slot_id,
                        key.idempotency_key_digest,
                        request_fingerprint,
                        current.secret_ref_id,
                    ],
                )
                .map_err(map_database_error)?;
            if changed != 1 {
                return Err(StoreError::Integrity(
                    "credential receipt changed during settlement".to_owned(),
                ));
            }
            insert_credential_submission_audit(
                &transaction,
                &current,
                &provider_ref,
                &committed_at,
            )?;
            let committed = read_receipt(&transaction, &key)?.ok_or_else(|| {
                StoreError::CorruptSchema("committed credential receipt disappeared".to_owned())
            })?;
            transaction.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }
}

impl SqliteWorkspaceStore {
    fn run_credential_submission_operation<T: Send + 'static>(
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

fn workspace_owner_matches(
    connection: &Connection,
    workspace_id: &str,
    principal_id: &str,
) -> Result<(), StoreError> {
    let owned: bool = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM workspaces
               WHERE workspace_id = ?1 AND owner_principal_id = ?2
             )",
            params![workspace_id, principal_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if owned {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn read_receipt(
    connection: &Connection,
    key: &AgentCredentialSubmissionKey,
) -> Result<Option<AgentCredentialSubmissionReceipt>, StoreError> {
    connection
        .query_row(
            "SELECT request_fingerprint, secret_kind, secret_ref_id, state, provider_ref,
                    created_at, committed_at
             FROM agent_credential_submission_receipts
             WHERE workspace_id = ?1 AND principal_id = ?2 AND agent_profile_id = ?3
               AND credential_slot_id = ?4 AND idempotency_key_digest = ?5",
            params![
                key.workspace_id,
                key.principal_id,
                key.agent_profile_id,
                key.credential_slot_id,
                key.idempotency_key_digest,
            ],
            |row| {
                let state_name: String = row.get(3)?;
                let secret_kind_name: String = row.get(1)?;
                Ok((
                    row.get::<_, String>(0)?,
                    secret_kind_name,
                    row.get::<_, String>(2)?,
                    state_name,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?
        .map(
            |(
                request_fingerprint,
                secret_kind_name,
                secret_ref_id,
                state_name,
                provider_ref,
                created_at,
                committed_at,
            )| {
                let secret_kind = secret_kind_from_str(&secret_kind_name).ok_or_else(|| {
                    StoreError::CorruptSchema(
                        "credential receipt has an unknown SecretKind".to_owned(),
                    )
                })?;
                let state = AgentCredentialSubmissionState::from_str(&state_name)?;
                if (state == AgentCredentialSubmissionState::Pending)
                    != (provider_ref.is_none() && committed_at.is_none())
                {
                    return Err(StoreError::CorruptSchema(
                        "credential receipt state is inconsistent".to_owned(),
                    ));
                }
                Ok(AgentCredentialSubmissionReceipt {
                    key: key.clone(),
                    request_fingerprint,
                    secret_kind,
                    secret_ref_id,
                    state,
                    provider_ref,
                    created_at,
                    committed_at,
                })
            },
        )
        .transpose()
}

fn secret_kind_as_str(kind: SecretKind) -> &'static str {
    match kind {
        SecretKind::ApiKey => "API_KEY",
        SecretKind::Token => "TOKEN",
        SecretKind::Password => "PASSWORD",
        SecretKind::Other => "OTHER",
    }
}

fn secret_kind_from_str(value: &str) -> Option<SecretKind> {
    match value {
        "API_KEY" => Some(SecretKind::ApiKey),
        "TOKEN" => Some(SecretKind::Token),
        "PASSWORD" => Some(SecretKind::Password),
        "OTHER" => Some(SecretKind::Other),
        _ => None,
    }
}

fn fingerprints_equal(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

fn insert_credential_submission_audit(
    transaction: &Transaction<'_>,
    receipt: &AgentCredentialSubmissionReceipt,
    provider_ref: &str,
    occurred_at: &str,
) -> Result<(), StoreError> {
    let key = &receipt.key;
    let principal_json = String::from_utf8(canonical_json(&json!({
        "principal_id": key.principal_id,
        "kind": "USER",
    }))?)
    .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let resource_ref_json = String::from_utf8(canonical_json(&json!({
        "kind": "AGENT_CREDENTIAL_SLOT",
        "workspace_id": key.workspace_id,
        "agent_profile_id": key.agent_profile_id,
        "credential_slot_id": key.credential_slot_id,
        "secret_ref_id": receipt.secret_ref_id,
        "provider_ref": provider_ref,
    }))?)
    .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let audit_identity = json!({
        "operation": "agent.credential.submit",
        "workspace_id": key.workspace_id,
        "agent_profile_id": key.agent_profile_id,
        "credential_slot_id": key.credential_slot_id,
        "secret_ref_id": receipt.secret_ref_id,
        "provider_ref": provider_ref,
        "principal_id": key.principal_id,
    });
    let audit_digest = digest(&canonical_json(&audit_identity)?);
    let audit_record_id = format!("audit_agent_credential_{}", &audit_digest[7..]);
    let correlation_id = format!("credential-operation:{}", &key.idempotency_key_digest[7..]);
    transaction
        .execute(
            "INSERT INTO audit_records(
               audit_record_id, workspace_id, principal_json, action, resource_ref_json,
               decision, reason_code, correlation_id, occurred_at, payload_digest
             ) VALUES (?1, ?2, ?3, 'agent.credential.submit', ?4, 'ALLOW',
                       'CREDENTIAL_STORED', ?5, ?6, ?7)",
            params![
                audit_record_id,
                key.workspace_id,
                principal_json,
                resource_ref_json,
                correlation_id,
                occurred_at,
                audit_digest,
            ],
        )
        .map_err(map_database_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BlobPurpose, FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use std::sync::Arc;
    use zeroize::Zeroizing;

    const WORKSPACE: &str = "credential-receipt-workspace";
    const OWNER: &str = "credential-receipt-owner";
    const NOW: &str = "2026-11-23T12:00:00Z";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([73; 32]),
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

    fn fixture() -> (tempfile::TempDir, SqliteWorkspaceStore) {
        let directory = tempfile::tempdir().expect("temporary credential receipt directory");
        let blobs = Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys));
        let store = SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            blobs,
            SqliteConfig::default(),
        )
        .expect("SQLite store opens with v36 migration");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Credential receipt tests".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "credential-receipt-workspace-created".to_owned(),
                    origin_runtime_id: "credential-receipt-runtime".to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "credential-receipt-workspace-created".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");
        (directory, store)
    }

    fn key() -> AgentCredentialSubmissionKey {
        AgentCredentialSubmissionKey {
            workspace_id: WORKSPACE.to_owned(),
            principal_id: OWNER.to_owned(),
            agent_profile_id: "credential-profile".to_owned(),
            credential_slot_id: "api-key".to_owned(),
            idempotency_key_digest: format!("sha256:{}", "a".repeat(64)),
        }
    }

    fn reservation(
        fingerprint_byte: char,
        secret_ref_suffix: char,
    ) -> AgentCredentialSubmissionReservation {
        AgentCredentialSubmissionReservation {
            key: key(),
            request_fingerprint: format!("hmac-sha256:{}", fingerprint_byte.to_string().repeat(64)),
            secret_kind: SecretKind::ApiKey,
            secret_ref_id: format!("secret_{}", secret_ref_suffix.to_string().repeat(64)),
            created_at: NOW.to_owned(),
        }
    }

    #[test]
    fn exact_retry_and_commit_replay_reuse_one_ref_and_one_audit_row() {
        let (directory, store) = fixture();
        let reservation = reservation('b', 'c');
        let pending = store
            .reserve_agent_credential_submission(reservation.clone())
            .expect("reserve before keyring mutation");
        assert_eq!(pending.state, AgentCredentialSubmissionState::Pending);

        let committed = store
            .commit_agent_credential_submission(
                reservation.key.clone(),
                reservation.request_fingerprint.clone(),
                "os-keyring:v1:opaque-account-hash".to_owned(),
                "2026-11-23T12:00:01Z".to_owned(),
            )
            .expect("settle the receipt and audit atomically");
        assert_eq!(committed.state, AgentCredentialSubmissionState::Committed);
        assert_eq!(committed.secret_ref_id, pending.secret_ref_id);

        let duplicate_reservation = AgentCredentialSubmissionReservation {
            secret_ref_id: format!("secret_{}", "d".repeat(64)),
            created_at: "2026-11-23T12:00:02Z".to_owned(),
            ..reservation.clone()
        };
        let exact_retry = store
            .reserve_agent_credential_submission(duplicate_reservation)
            .expect("same request returns its existing committed receipt");
        assert_eq!(exact_retry, committed);
        let committed_retry = store
            .commit_agent_credential_submission(
                reservation.key.clone(),
                reservation.request_fingerprint,
                "os-keyring:v1:opaque-account-hash".to_owned(),
                "2026-11-23T12:00:03Z".to_owned(),
            )
            .expect("lost commit response can be replayed");
        assert_eq!(committed_retry, committed);

        let audit_connection = Connection::open(directory.path().join("state.sqlite3"))
            .expect("open read-only audit inspection connection");
        let audit_count: i64 = audit_connection
            .query_row(
                "SELECT COUNT(*) FROM audit_records WHERE workspace_id = ?1 AND action = 'agent.credential.submit'",
                [WORKSPACE],
                |row| row.get(0),
            )
            .expect("count credential audits");
        assert_eq!(audit_count, 1, "replay never creates a second audit row");
    }

    #[test]
    fn changed_body_under_same_operation_key_conflicts_without_replacing_reservation() {
        let (_directory, store) = fixture();
        let original = reservation('b', 'c');
        let first = store
            .reserve_agent_credential_submission(original.clone())
            .expect("initial request reserves");
        let changed = reservation('e', 'f');
        assert_eq!(
            store.reserve_agent_credential_submission(changed),
            Err(StoreError::IdempotencyConflict)
        );
        assert_eq!(
            store
                .get_agent_credential_submission(original.key)
                .expect("read original receipt")
                .expect("original receipt remains")
                .secret_ref_id,
            first.secret_ref_id
        );
    }
}
