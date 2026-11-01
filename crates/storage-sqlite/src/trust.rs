//! Durable Trust approval records and the local write-only SecretStore adapter.
//!
//! Approval resolution records an owner's one-time decision. It does not consume
//! that decision, issue a CapabilityGrant or SecretLease, or admit an Effect.
//! SecretStore writes return only opaque references and have no credential read path.

use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use keyring::Entry;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::mpsc;
use storage_core::{
    ApprovalCursor, ApprovalDecision, ApprovalPage, ApprovalPageRequest, ApprovalRecord,
    ApprovalStore, CapabilityGrantRecord, CapabilityGrantRevocationCommit, CapabilityGrantScope,
    CapabilityGrantStatus, CapabilityGrantStore, CommittedApproval, CommittedCapabilityGrant,
    CommittedCapabilityGrantRevocation, CommittedSecretLease, IssueCapabilityGrantCommit,
    IssueSecretLeaseCommit, RequestApprovalCommit, ResolveApprovalCommit, RevokeSecretLeaseCommit,
    SecretKind, SecretLeaseRecord, SecretLeaseStore, SecretPlacement, SecretRef, SecretStore,
    StoreError,
};
use zeroize::{Zeroize, Zeroizing};

const MAX_APPROVAL_ID_BYTES: usize = 512;
const MAX_PRINCIPAL_ID_BYTES: usize = 512;
const MAX_REQUEST_ID_BYTES: usize = 200;
const MAX_APPROVAL_TEXT_BYTES: usize = 16_384;
const MAX_APPROVAL_TARGET_DEPTH: usize = 8;
const MAX_APPROVAL_TARGET_COLLECTION_ITEMS: usize = 64;
const SECRET_KEYRING_SERVICE: &str = "com.litecow.secret-store";
const MAX_SECRET_BYTES: usize = 48 * 1024;

trait SecretCredentialBackend: Send + Sync {
    fn set_secret(&self, account: &str, secret: &str) -> Result<(), StoreError>;
    fn remove_secret(&self, account: &str) -> Result<(), StoreError>;
}

#[derive(Clone, Copy, Debug, Default)]
struct OsSecretCredentialBackend;

impl SecretCredentialBackend for OsSecretCredentialBackend {
    fn set_secret(&self, account: &str, secret: &str) -> Result<(), StoreError> {
        let entry = secret_keyring_entry(account).map_err(|_| secret_store_error())?;
        entry.set_password(secret).map_err(|_| secret_store_error())
    }

    fn remove_secret(&self, account: &str) -> Result<(), StoreError> {
        let entry = secret_keyring_entry(account).map_err(|_| secret_store_error())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(secret_store_error()),
        }
    }
}

/// Local OS-keychain SecretStore. This implementation intentionally supports
/// write and opaque-ref creation only; it cannot return credential bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsSecretStore;

impl SecretStore for OsSecretStore {
    fn write(
        &self,
        workspace_id: &str,
        secret_kind: SecretKind,
        secret_bytes: &mut [u8],
    ) -> Result<SecretRef, StoreError> {
        write_secret_with(
            &OsSecretCredentialBackend,
            workspace_id,
            secret_kind,
            secret_bytes,
        )
    }
}

fn write_secret_with(
    backend: &impl SecretCredentialBackend,
    workspace_id: &str,
    secret_kind: SecretKind,
    secret_bytes: &mut [u8],
) -> Result<SecretRef, StoreError> {
    // Own the caller's mutable buffer immediately so every return path zeroizes
    // it, including invalid scope/kind/size and keychain errors.
    let secret = Zeroizing::new(secret_bytes.to_vec());
    secret_bytes.zeroize();
    validate_secret_scope(workspace_id, secret.len())?;
    let mut secret_ref_random = Zeroizing::new([0_u8; 32]);
    getrandom::fill(secret_ref_random.as_mut()).map_err(|_| {
        StoreError::Blob("OS SecretStore could not allocate a reference".to_owned())
    })?;
    let secret_ref_id = format!("secret_{}", hex::encode(secret_ref_random.as_ref()));
    let account_hash = secret_account_hash(workspace_id, secret_kind, &secret_ref_id);
    let account = format!("v1:{account_hash}");
    let provider_ref = format!("os-keyring:v1:{account_hash}");
    let encoded = Zeroizing::new(URL_SAFE_NO_PAD.encode(secret.as_slice()));
    if let Err(error) = backend.set_secret(&account, encoded.as_str()) {
        let _ = backend.remove_secret(&account);
        return Err(error);
    }
    Ok(SecretRef {
        secret_ref_id,
        provider_ref,
        placement: SecretPlacement::LocalOnly,
    })
}

fn validate_secret_scope(workspace_id: &str, secret_bytes: usize) -> Result<(), StoreError> {
    if workspace_id.trim().is_empty()
        || workspace_id.len() > MAX_PRINCIPAL_ID_BYTES
        || workspace_id.chars().any(char::is_control)
        || secret_bytes == 0
        || secret_bytes > MAX_SECRET_BYTES
    {
        return Err(StoreError::Invalid(
            "SecretStore input scope, kind, or size is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn secret_account_hash(workspace_id: &str, secret_kind: SecretKind, secret_ref_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"litecowork-secret-store-v1\0");
    let kind = match secret_kind {
        SecretKind::ApiKey => "API_KEY",
        SecretKind::Token => "TOKEN",
        SecretKind::Password => "PASSWORD",
        SecretKind::Other => "OTHER",
    };
    for part in [
        workspace_id.as_bytes(),
        kind.as_bytes(),
        secret_ref_id.as_bytes(),
    ] {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part);
    }
    hex::encode(digest.finalize())
}

fn secret_keyring_entry(account: &str) -> Result<Entry, keyring::Error> {
    #[cfg(target_os = "linux")]
    {
        let credential = keyring::secret_service::SsCredential::new_with_target(
            None,
            SECRET_KEYRING_SERVICE,
            account,
        )?;
        Ok(Entry::new_with_credential(Box::new(credential)))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Entry::new(SECRET_KEYRING_SERVICE, account)
    }
}

fn secret_store_error() -> StoreError {
    StoreError::Blob("OS SecretStore is unavailable or rejected the write".to_owned())
}


#[derive(Clone)]
pub struct SqliteApprovalStore {
    store: SqliteWorkspaceStore,
}

impl SqliteApprovalStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    fn run_trust<T, F>(&self, operation: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

impl ApprovalStore for SqliteApprovalStore {
    fn request_approval(
        &self,
        mut command: RequestApprovalCommit,
    ) -> Result<CommittedApproval, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        command.approval.requested_at = canonicalize_utc_timestamp(&command.approval.requested_at)?;
        if let Some(expires_at) = command.approval.expires_at.as_mut() {
            *expires_at = canonicalize_utc_timestamp(expires_at)?;
        }
        validate_request_command(&command)?;

        let replay = self.run_trust({
            let command = command.clone();
            move |connection| {
                let tx = connection.transaction().map_err(map_database_error)?;
                authorize_owner(
                    &tx,
                    &command.workspace_id,
                    &command.actor.owner_principal_id,
                )?;
                let replay = find_request_replay(&tx, &command)?;
                tx.commit().map_err(map_database_error)?;
                Ok(replay)
            }
        })?;
        if let Some(replay) = replay {
            return Ok(replay);
        }

        let state_ref = put_approval_state(&self.store, &command.workspace_id, &command.approval)?;
        self.run_trust(move |connection| request_transaction(connection, command, state_ref))
    }

    fn get_approval(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        approval_id: &str,
    ) -> Result<Option<ApprovalRecord>, StoreError> {
        for (name, value) in [
            ("owner principal ID", owner_principal_id),
            ("Workspace ID", workspace_id),
            ("Approval ID", approval_id),
        ] {
            if value.trim().is_empty()
                || value.len() > MAX_APPROVAL_ID_BYTES
                || value.chars().any(char::is_control)
            {
                return Err(StoreError::Invalid(format!("{name} is malformed")));
            }
        }
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let approval = approval_id.to_owned();
        self.run_trust(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &owner)?;
            let record = load_approval(&tx, &workspace, &approval)?;
            tx.commit().map_err(map_database_error)?;
            Ok(record)
        })
    }

    fn list_approvals_page(
        &self,
        owner_principal_id: &str,
        request: ApprovalPageRequest,
    ) -> Result<ApprovalPage, StoreError> {
        validate_approval_page_request(owner_principal_id, &request)?;
        let owner = owner_principal_id.to_owned();
        self.run_trust(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_owner(&tx, &request.workspace_id, &owner)?;

            let (after_requested_at, after_approval_id) = request
                .cursor
                .as_ref()
                .map(|cursor| {
                    (
                        Some(cursor.after_requested_at.as_str()),
                        Some(cursor.after_approval_id.as_str()),
                    )
                })
                .unwrap_or((None, None));
            let query_limit = request
                .limit
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("Approval page limit is invalid".into()))?;
            let query_limit = i64::try_from(query_limit)
                .map_err(|_| StoreError::Invalid("Approval page limit is invalid".into()))?;
            let keys = {
                let mut statement = tx
                    .prepare(
                        "SELECT a.approval_id,a.requested_at \
                         FROM approvals a JOIN tasks t ON t.task_id=a.task_id \
                         WHERE t.workspace_id=?1 AND (?2 IS NULL OR a.status=?2) \
                           AND (?3 IS NULL OR a.requested_at < ?3 \
                             OR (a.requested_at = ?3 AND a.approval_id < ?4)) \
                         ORDER BY a.requested_at DESC,a.approval_id DESC LIMIT ?5",
                    )
                    .map_err(map_database_error)?;
                statement
                    .query_map(
                        params![
                            request.workspace_id,
                            request.status,
                            after_requested_at,
                            after_approval_id,
                            query_limit,
                        ],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .map_err(map_database_error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(map_database_error)?
            };
            let has_more = keys.len() > request.limit;
            let mut keys = keys;
            if has_more {
                keys.pop();
            }
            let mut items = Vec::with_capacity(keys.len());
            for (approval_id, _) in &keys {
                let record =
                    load_approval(&tx, &request.workspace_id, approval_id)?.ok_or_else(|| {
                        StoreError::Integrity(
                            "Approval disappeared during its page read".to_owned(),
                        )
                    })?;
                items.push(record);
            }
            let next_cursor = if has_more {
                keys.last()
                    .map(|(approval_id, requested_at)| {
                        Ok(ApprovalCursor {
                            workspace_id: request.workspace_id.clone(),
                            status: request.status.clone(),
                            limit: request.limit,
                            // Preserve the exact stored key used by SQLite's lexical
                            // page ordering; normalizing equivalent RFC3339 spellings
                            // could skip rows that share this timestamp.
                            after_requested_at: requested_at.clone(),
                            after_approval_id: approval_id.clone(),
                        })
                    })
                    .transpose()?
            } else {
                None
            };
            tx.commit().map_err(map_database_error)?;
            Ok(ApprovalPage { items, next_cursor })
        })
    }

    fn resolve_approval(
        &self,
        mut command: ResolveApprovalCommit,
    ) -> Result<CommittedApproval, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_command(&command)?;

        let approval = self.run_trust({
            let command = command.clone();
            move |connection| {
                let tx = connection.transaction().map_err(map_database_error)?;
                authorize_owner(
                    &tx,
                    &command.workspace_id,
                    &command.actor.owner_principal_id,
                )?;
                let prior = find_replay(&tx, &command)?;
                let current = if prior.is_none() {
                    Some(
                        load_approval(&tx, &command.workspace_id, &command.approval_id)?
                            .ok_or(StoreError::NotFound)?,
                    )
                } else {
                    None
                };
                tx.commit().map_err(map_database_error)?;
                if let Some(prior) = prior {
                    return Ok(ReadApproval::Replay(prior));
                }
                Ok(ReadApproval::Current(current.ok_or(StoreError::NotFound)?))
            }
        })?;
        let current = match approval {
            ReadApproval::Replay(prior) => return Ok(prior),
            ReadApproval::Current(current) => current,
        };
        if current.version != command.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(current.version),
            });
        }
        if current.status != "PENDING" {
            return Err(StoreError::Invalid(
                "only a pending Approval can be resolved".to_owned(),
            ));
        }
        if !assurance_satisfies(&command.actor.assurance_level, &current.required_assurance) {
            return Err(StoreError::Invalid(
                "authenticated actor assurance is insufficient for this Approval".to_owned(),
            ));
        }
        if current
            .expires_at
            .as_deref()
            .is_some_and(|expires_at| expires_at <= command.event.recorded_at.as_str())
        {
            return Err(StoreError::Invalid(
                "expired Approval cannot be resolved".to_owned(),
            ));
        }
        if command
            .confirmation_digest
            .as_deref()
            .is_some_and(|digest| digest != current.action_digest)
        {
            return Err(StoreError::Invalid(
                "Approval confirmation does not match its exact action".to_owned(),
            ));
        }
        let mut next = current;
        next.status = command.decision.status().to_owned();
        next.resolved_by = Some(json!({
            "principal_id": command.actor.principal_id,
            "kind": command.actor.principal_kind,
        }));
        next.resolved_at = Some(command.event.recorded_at.clone());
        next.version = next
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Approval version overflow".to_owned()))?;

        let state_ref = put_approval_state(&self.store, &command.workspace_id, &next)?;
        self.run_trust(move |connection| resolve_transaction(connection, command, next, state_ref))
    }
}

impl CapabilityGrantStore for SqliteApprovalStore {
    fn issue_capability_grant(
        &self,
        mut command: IssueCapabilityGrantCommit,
    ) -> Result<CommittedCapabilityGrant, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        command.grant.created_at = canonicalize_utc_timestamp(&command.grant.created_at)?;
        if let Some(expires_at) = command.grant.expires_at.as_mut() {
            *expires_at = canonicalize_utc_timestamp(expires_at)?;
        }
        validate_capability_grant_issue(&command)?;
        // The receipt digest binds the caller's unresolved intent. Resolve mutable
        // Resource heads only after exact replay has had a chance to return the
        // original receipt, even if those Resources have since changed or vanished.
        let request_digest = capability_grant_issue_digest(&command)?;
        let workspace_id = command.workspace_id.clone();
        let owner_id = command.actor.owner_principal_id.clone();
        let principal_id = command.actor.principal_id.clone();
        let request_id = command.request_id.clone();
        let replay_digest = request_digest.clone();
        let replay = self.run_trust(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_owner(&tx, &workspace_id, &owner_id)?;
            let replay = find_capability_grant_issue_replay(
                &tx,
                &principal_id,
                &request_id,
                &replay_digest,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(replay)
        })?;
        if let Some(replay) = replay {
            return Ok(replay);
        }
        command.grant.resource_scope = self.resolve_grant_resource_revisions(
            &command.actor.owner_principal_id,
            &command.workspace_id,
            &command.grant.resource_scope,
        )?;
        validate_capability_grant_issue(&command)?;
        let stored = stored_grant_from_record(&command.grant)?;
        let state_ref =
            put_capability_grant_state(&self.store, &command.workspace_id, &stored, "ACTIVE", 1)?;
        self.run_trust(move |connection| {
            issue_capability_grant_transaction(connection, command, state_ref, request_digest)
        })
    }

    fn revoke_capability_grant(
        &self,
        mut command: CapabilityGrantRevocationCommit,
    ) -> Result<CommittedCapabilityGrantRevocation, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_grant_revocation_command(&command)?;
        let request_digest = grant_revocation_request_digest(&command)?;
        let principal = command.actor.principal_id.clone();
        let workspace = command.workspace_id.clone();
        let grant_id = command.capability_grant_id.clone();
        let request_id = command.request_id.clone();
        let actor = command.actor.clone();
        let lookup_digest = request_digest.clone();
        let (prior, current) = self.run_trust(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &actor.owner_principal_id)?;
            let prior = find_grant_revocation_replay(&tx, &principal, &request_id, &lookup_digest)?;
            let current = if prior.is_none() {
                Some(
                    load_capability_grant(&tx, &workspace, &grant_id)?
                        .ok_or(StoreError::NotFound)?,
                )
            } else {
                None
            };
            tx.commit().map_err(map_database_error)?;
            Ok((prior, current))
        })?;
        if let Some(prior) = prior {
            return Ok(prior);
        }
        let current = current.ok_or(StoreError::NotFound)?;
        if current.version != command.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(current.version),
            });
        }
        if current.status != "ACTIVE" {
            return Err(StoreError::Invalid(
                "only an active CapabilityGrant can be revoked".into(),
            ));
        }
        let next_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("CapabilityGrant version overflow".into()))?;
        let state_ref = put_capability_grant_state(
            &self.store,
            &command.workspace_id,
            &current,
            "REVOKED",
            next_version,
        )?;
        self.run_trust(move |connection| {
            revoke_capability_grant_transaction(
                connection,
                command,
                current,
                state_ref,
                request_digest,
            )
        })
    }
}

impl SecretLeaseStore for SqliteApprovalStore {
    fn issue_secret_lease(
        &self,
        mut command: IssueSecretLeaseCommit,
    ) -> Result<CommittedSecretLease, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        command.lease.issued_at = canonicalize_utc_timestamp(&command.lease.issued_at)?;
        command.lease.expires_at = canonicalize_utc_timestamp(&command.lease.expires_at)?;
        validate_secret_lease_issue(&command)?;
        let replay = self.run_trust({
            let command = command.clone();
            move |connection| {
                let tx = connection.transaction().map_err(map_database_error)?;
                authorize_owner(
                    &tx,
                    &command.workspace_id,
                    &command.actor.owner_principal_id,
                )?;
                let replay = find_secret_lease_replay(
                    &tx,
                    &command.actor.principal_id,
                    &command.request_id,
                    &secret_lease_issue_digest(&command)?,
                )?;
                tx.commit().map_err(map_database_error)?;
                Ok(replay)
            }
        })?;
        if let Some(replay) = replay {
            return Ok(replay);
        }
        let state_ref = put_secret_lease_state(&self.store, &command.workspace_id, &command.lease)?;
        self.run_trust(move |connection| {
            issue_secret_lease_transaction(connection, command, state_ref)
        })
    }

    fn revoke_secret_lease(
        &self,
        mut command: RevokeSecretLeaseCommit,
    ) -> Result<CommittedSecretLease, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_secret_lease_revoke(&command)?;
        let replay = self.run_trust({
            let command = command.clone();
            move |connection| {
                let tx = connection.transaction().map_err(map_database_error)?;
                authorize_owner(
                    &tx,
                    &command.workspace_id,
                    &command.actor.owner_principal_id,
                )?;
                let replay = find_secret_lease_replay(
                    &tx,
                    &command.actor.principal_id,
                    &command.request_id,
                    &secret_lease_revoke_digest(&command)?,
                )?;
                tx.commit().map_err(map_database_error)?;
                Ok(replay)
            }
        })?;
        if let Some(replay) = replay {
            return Ok(replay);
        }
        let current = self
            .get_secret_lease_for_owner(
                &command.actor.owner_principal_id,
                &command.workspace_id,
                &command.secret_lease_id,
            )?
            .ok_or(StoreError::NotFound)?;
        let mut next = current;
        next.status = "REVOKED".into();
        next.version = command
            .expected_version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("SecretLease version exhausted".into()))?;
        let state_ref = put_secret_lease_state(&self.store, &command.workspace_id, &next)?;
        self.run_trust(move |connection| {
            revoke_secret_lease_transaction(connection, command, next, state_ref)
        })
    }
}

impl SqliteApprovalStore {
    fn resolve_grant_resource_revisions(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        resource_scope: &Value,
    ) -> Result<Value, StoreError> {
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let mut scope = resource_scope.clone();
        let refs = scope
            .get("resource_refs")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                StoreError::Invalid("CapabilityGrant resource refs are invalid".into())
            })?;
        self.run_trust(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &owner)?;
            let mut resolved = Vec::with_capacity(refs.len());
            for reference in refs {
                let object = reference.as_object().ok_or_else(|| {
                    StoreError::Invalid("CapabilityGrant ResourceRef is invalid".into())
                })?;
                let ref_workspace = object
                    .get("workspace_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| StoreError::Invalid("ResourceRef Workspace is missing".into()))?;
                let resource_id = object
                    .get("resource_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| StoreError::Invalid("ResourceRef ID is missing".into()))?;
                if ref_workspace != workspace {
                    return Err(StoreError::Invalid(
                        "CapabilityGrant ResourceRef is outside the selected Workspace".into(),
                    ));
                }
                let requested_revision = object.get("revision_id").and_then(Value::as_str);
                let revision = if let Some(revision) = requested_revision {
                    let exists: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM resource_revisions rr JOIN resources r ON r.resource_id=rr.resource_id WHERE r.workspace_id=?1 AND r.resource_id=?2 AND rr.resource_revision_id=?3)",
                        params![workspace, resource_id, revision],
                        |row| row.get(0),
                    ).map_err(map_database_error)?;
                    if !exists {
                        return Err(StoreError::NotFound);
                    }
                    revision.to_owned()
                } else {
                    tx.query_row(
                        "SELECT current_revision_id FROM resources WHERE workspace_id=?1 AND resource_id=?2",
                        params![workspace, resource_id],
                        |row| row.get::<_, Option<String>>(0),
                    )
                    .optional()
                    .map_err(map_database_error)?
                    .flatten()
                    .ok_or(StoreError::NotFound)?
                };
                let resolved_ref = json!({
                    "workspace_id": ref_workspace,
                    "resource_id": resource_id,
                    "revision_id": revision,
                });
                if object.keys().any(|key| !["workspace_id", "resource_id", "revision_id"].contains(&key.as_str())) {
                    return Err(StoreError::Invalid("ResourceRef has unknown fields".into()));
                }
                // Keep the shape canonical and exact for the durable grant.
                resolved.push(resolved_ref);
            }
            tx.commit().map_err(map_database_error)?;
            Ok(resolved)
        })
        .map(|resolved| {
            scope["resource_refs"] = Value::Array(resolved);
            scope
        })
    }

    fn get_secret_lease_for_owner(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        secret_lease_id: &str,
    ) -> Result<Option<SecretLeaseRecord>, StoreError> {
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let lease_id = secret_lease_id.to_owned();
        self.run_trust(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &owner)?;
            let record = load_secret_lease(&tx, &workspace, &lease_id)?;
            tx.commit().map_err(map_database_error)?;
            Ok(record)
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct StoredCapabilityGrant {
    capability_grant_id: String,
    scope_kind: String,
    conversation_id: Option<String>,
    task_id: Option<String>,
    attempt_id: Option<String>,
    capability_ref: Value,
    allowed_operations: Vec<String>,
    resource_scope: Value,
    secret_refs: Vec<SecretRef>,
    granted_by: Value,
    expires_at: Option<String>,
    status: String,
    created_at: String,
    version: u64,
}

fn stored_grant_from_record(
    grant: &CapabilityGrantRecord,
) -> Result<StoredCapabilityGrant, StoreError> {
    let (scope_kind, conversation_id, task_id, attempt_id) = match &grant.scope {
        CapabilityGrantScope::Conversation { conversation_id } => (
            "CONVERSATION".to_owned(),
            Some(conversation_id.clone()),
            None,
            None,
        ),
        CapabilityGrantScope::TaskPlanning { task_id } => (
            "TASK_PLANNING".to_owned(),
            None,
            Some(task_id.clone()),
            None,
        ),
        CapabilityGrantScope::AttemptExecution {
            task_id,
            attempt_id,
        } => (
            "ATTEMPT_EXECUTION".to_owned(),
            None,
            Some(task_id.clone()),
            Some(attempt_id.clone()),
        ),
    };
    let stored = StoredCapabilityGrant {
        capability_grant_id: grant.capability_grant_id.clone(),
        scope_kind,
        conversation_id,
        task_id,
        attempt_id,
        capability_ref: grant.capability_ref.clone(),
        allowed_operations: grant.allowed_operations.clone(),
        resource_scope: grant.resource_scope.clone(),
        secret_refs: grant.secret_refs.clone(),
        granted_by: grant.granted_by.clone(),
        expires_at: grant.expires_at.clone(),
        status: match grant.status {
            CapabilityGrantStatus::Active => "ACTIVE".into(),
            CapabilityGrantStatus::Revoked => "REVOKED".into(),
            CapabilityGrantStatus::Expired => "EXPIRED".into(),
        },
        created_at: grant.created_at.clone(),
        version: grant.version,
    };
    stored.scope()?;
    Ok(stored)
}

fn grant_record_from_stored(
    grant: StoredCapabilityGrant,
) -> Result<CapabilityGrantRecord, StoreError> {
    let scope = match grant.scope_kind.as_str() {
        "CONVERSATION" => CapabilityGrantScope::Conversation {
            conversation_id: grant.conversation_id.ok_or_else(|| {
                StoreError::Integrity("CapabilityGrant conversation scope is missing".into())
            })?,
        },
        "TASK_PLANNING" => CapabilityGrantScope::TaskPlanning {
            task_id: grant.task_id.ok_or_else(|| {
                StoreError::Integrity("CapabilityGrant Task scope is missing".into())
            })?,
        },
        "ATTEMPT_EXECUTION" => CapabilityGrantScope::AttemptExecution {
            task_id: grant.task_id.ok_or_else(|| {
                StoreError::Integrity("CapabilityGrant Attempt Task is missing".into())
            })?,
            attempt_id: grant.attempt_id.ok_or_else(|| {
                StoreError::Integrity("CapabilityGrant Attempt is missing".into())
            })?,
        },
        _ => {
            return Err(StoreError::Integrity(
                "CapabilityGrant scope is invalid".into(),
            ));
        }
    };
    let status = match grant.status.as_str() {
        "ACTIVE" => CapabilityGrantStatus::Active,
        "REVOKED" => CapabilityGrantStatus::Revoked,
        "EXPIRED" => CapabilityGrantStatus::Expired,
        _ => {
            return Err(StoreError::Integrity(
                "CapabilityGrant status is invalid".into(),
            ));
        }
    };
    Ok(CapabilityGrantRecord {
        capability_grant_id: grant.capability_grant_id,
        scope,
        capability_ref: grant.capability_ref,
        allowed_operations: grant.allowed_operations,
        resource_scope: grant.resource_scope,
        secret_refs: grant.secret_refs,
        granted_by: grant.granted_by,
        expires_at: grant.expires_at,
        status,
        created_at: grant.created_at,
        version: grant.version,
    })
}

impl StoredCapabilityGrant {
    fn scope(&self) -> Result<Value, StoreError> {
        match self.scope_kind.as_str() {
            "CONVERSATION"
                if self.conversation_id.is_some()
                    && self.task_id.is_none()
                    && self.attempt_id.is_none() =>
            {
                Ok(json!({"kind":"CONVERSATION","conversation_id":self.conversation_id}))
            }
            "TASK_PLANNING"
                if self.conversation_id.is_none()
                    && self.task_id.is_some()
                    && self.attempt_id.is_none() =>
            {
                Ok(json!({"kind":"TASK_PLANNING","task_id":self.task_id}))
            }
            "ATTEMPT_EXECUTION"
                if self.conversation_id.is_none()
                    && self.task_id.is_some()
                    && self.attempt_id.is_some() =>
            {
                Ok(
                    json!({"kind":"ATTEMPT_EXECUTION","task_id":self.task_id,"attempt_id":self.attempt_id}),
                )
            }
            _ => Err(StoreError::Integrity(
                "CapabilityGrant scope row is malformed".into(),
            )),
        }
    }

    fn scope_digest(&self) -> Result<String, StoreError> {
        let secret_refs = self
            .secret_refs
            .iter()
            .map(|secret_ref| {
                json!({
                    "secret_ref_id": secret_ref.secret_ref_id,
                    "placement": secret_ref.placement,
                })
            })
            .collect::<Vec<_>>();
        Ok(digest(&canonical_json(&json!({
            "scope": self.scope()?,
            "capability_ref": self.capability_ref,
            "allowed_operations": self.allowed_operations,
            "resource_scope": self.resource_scope,
            "secret_refs": secret_refs,
        }))?))
    }

    fn event_payload(&self) -> Result<Value, StoreError> {
        let mut payload = json!({
            "capability_grant_id": self.capability_grant_id,
            "scope": self.scope()?,
            "capability_ref": self.capability_ref,
            "scope_digest": self.scope_digest()?,
        });
        if let Some(expires_at) = &self.expires_at {
            payload["expires_at"] = json!(expires_at);
        }
        Ok(payload)
    }

    fn redacted_snapshot(&self, status: &str, version: u64) -> Result<Value, StoreError> {
        Ok(json!({
            "capability_grant_id": self.capability_grant_id,
            "scope": self.scope()?,
            "capability_ref": self.capability_ref,
            "allowed_operations": self.allowed_operations,
            "resource_scope": self.resource_scope,
            "secret_refs": self.secret_refs.iter().map(|secret_ref| json!({
                "secret_ref_id": secret_ref.secret_ref_id,
                "placement": secret_ref.placement,
            })).collect::<Vec<_>>(),
            "granted_by": self.granted_by,
            "expires_at": self.expires_at,
            "status": status,
            "created_at": self.created_at,
            "version": version,
        }))
    }
}

fn load_capability_grant(
    connection: &Connection,
    workspace_id: &str,
    capability_grant_id: &str,
) -> Result<Option<StoredCapabilityGrant>, StoreError> {
    let row = connection
        .query_row(
            "SELECT g.capability_grant_id,g.scope_kind,g.conversation_id,g.task_id,g.attempt_id,
                g.capability_ref_json,g.allowed_operations_json,g.resource_scope_json,
                g.secret_refs_json,g.granted_by_json,g.expires_at,g.status,g.created_at,g.version
         FROM capability_grants g
         LEFT JOIN conversations c ON c.conversation_id=g.conversation_id
         LEFT JOIN tasks t ON t.task_id=g.task_id
         WHERE g.capability_grant_id=?1
           AND ((g.scope_kind='CONVERSATION' AND c.workspace_id=?2)
             OR (g.scope_kind IN ('TASK_PLANNING','ATTEMPT_EXECUTION') AND t.workspace_id=?2))",
            params![capability_grant_id, workspace_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, i64>(13)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        id,
        scope_kind,
        conversation_id,
        task_id,
        attempt_id,
        capability_ref,
        operations,
        resource_scope,
        secret_refs,
        granted_by,
        expires_at,
        status,
        created_at,
        version,
    )) = row
    else {
        return Ok(None);
    };
    let grant = StoredCapabilityGrant {
        capability_grant_id: id,
        scope_kind,
        conversation_id,
        task_id,
        attempt_id,
        capability_ref: serde_json::from_str(&capability_ref).map_err(|_| {
            StoreError::Integrity("CapabilityGrant capability ref is invalid JSON".into())
        })?,
        allowed_operations: serde_json::from_str(&operations).map_err(|_| {
            StoreError::Integrity("CapabilityGrant operations are invalid JSON".into())
        })?,
        resource_scope: serde_json::from_str(&resource_scope).map_err(|_| {
            StoreError::Integrity("CapabilityGrant resource scope is invalid JSON".into())
        })?,
        secret_refs: serde_json::from_str(&secret_refs).map_err(|_| {
            StoreError::Integrity("CapabilityGrant SecretRefs are invalid JSON".into())
        })?,
        granted_by: serde_json::from_str(&granted_by).map_err(|_| {
            StoreError::Integrity("CapabilityGrant principal is invalid JSON".into())
        })?,
        expires_at: expires_at
            .as_deref()
            .map(|value| stored_timestamp(value, "CapabilityGrant expires_at"))
            .transpose()?,
        status,
        created_at: stored_timestamp(&created_at, "CapabilityGrant created_at")?,
        version: from_sql_i64(version, "CapabilityGrant version")?,
    };
    grant.scope()?;
    if !matches!(grant.status.as_str(), "ACTIVE" | "REVOKED" | "EXPIRED")
        || !capability_ref_matches_contract(&grant.capability_ref)
        || grant.allowed_operations.is_empty()
        || grant.allowed_operations.len() > 128
        || grant.allowed_operations.iter().any(|value| {
            value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control)
        })
        || !resource_scope_matches_contract(&grant.resource_scope, None)
        || grant
            .granted_by
            .get("principal_id")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(StoreError::Integrity(
            "CapabilityGrant row violates its record contract".into(),
        ));
    }
    Ok(Some(grant))
}

fn capability_ref_matches_contract(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let text = |key: &str, maximum: usize| {
        object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| {
                !value.trim().is_empty()
                    && value.len() <= maximum
                    && !value.chars().any(char::is_control)
            })
    };
    if !text("capability_id", 256)
        || !text("source", 1024)
        || !object
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(valid_sha256)
    {
        return false;
    }
    match object.get("identity_kind").and_then(Value::as_str) {
        Some("PACKAGE_COMPONENT") => {
            object.len() >= 5
                && object.len() <= 6
                && object.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "capability_id"
                            | "identity_kind"
                            | "source"
                            | "package_version"
                            | "digest"
                            | "component"
                    )
                })
                && text("package_version", 256)
                && object
                    .get("component")
                    .is_none_or(|_| text("component", 1024))
        }
        Some("MCP_SKILL") => {
            object.len() == 5
                && object.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "capability_id" | "identity_kind" | "source" | "digest" | "component"
                    )
                })
                && text("component", 2048)
        }
        _ => false,
    }
}

fn resource_scope_matches_contract(value: &Value, workspace_id: Option<&str>) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != 3
        || !object.contains_key("resource_refs")
        || !object.contains_key("operation_ids")
        || !object.contains_key("constraints")
    {
        return false;
    }
    let Some(refs) = object.get("resource_refs").and_then(Value::as_array) else {
        return false;
    };
    if refs.len() > 256
        || refs.iter().any(|reference| {
            let Some(reference) = reference.as_object() else {
                return true;
            };
            if !(2..=3).contains(&reference.len())
                || !reference.keys().all(|key| {
                    matches!(key.as_str(), "workspace_id" | "resource_id" | "revision_id")
                })
            {
                return true;
            }
            let Some(reference_workspace) = reference.get("workspace_id").and_then(Value::as_str)
            else {
                return true;
            };
            let valid_ref_text = |key: &str| {
                reference
                    .get(key)
                    .and_then(Value::as_str)
                    .is_some_and(|text| {
                        !text.trim().is_empty()
                            && text.len() <= 512
                            && !text.chars().any(char::is_control)
                    })
            };
            !valid_ref_text("workspace_id")
                || !valid_ref_text("resource_id")
                || (reference.contains_key("revision_id") && !valid_ref_text("revision_id"))
                || workspace_id.is_some_and(|expected| reference_workspace != expected)
        })
    {
        return false;
    }
    let Some(operations) = object.get("operation_ids").and_then(Value::as_array) else {
        return false;
    };
    if operations.is_empty()
        || operations.len() > 128
        || operations.iter().any(|operation| {
            !operation.as_str().is_some_and(|value| {
                !value.trim().is_empty()
                    && value.len() <= 256
                    && !value.chars().any(char::is_control)
            })
        })
        || !object.get("constraints").is_some_and(Value::is_object)
    {
        return false;
    }
    serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= MAX_APPROVAL_TEXT_BYTES)
}

/// Confirms that every declared ResourceRef resolves inside the authorized Workspace.
/// A revision pin must belong to that same Resource; revoked/pending/deleted
/// ContextDocuments cannot be newly granted. All misses are intentionally nondisclosing.
fn validate_grant_resource_refs(
    connection: &Connection,
    workspace_id: &str,
    resource_scope: &Value,
) -> Result<(), StoreError> {
    let references = resource_scope
        .get("resource_refs")
        .and_then(Value::as_array)
        .ok_or_else(|| StoreError::Invalid("CapabilityGrant ResourceRefs are invalid".into()))?;
    for reference in references {
        let resource_workspace = reference
            .get("workspace_id")
            .and_then(Value::as_str)
            .ok_or_else(|| StoreError::Invalid("CapabilityGrant ResourceRef is invalid".into()))?;
        let resource_id = reference
            .get("resource_id")
            .and_then(Value::as_str)
            .ok_or_else(|| StoreError::Invalid("CapabilityGrant ResourceRef is invalid".into()))?;
        let revision_id = reference.get("revision_id").and_then(Value::as_str);
        if resource_workspace != workspace_id {
            return Err(StoreError::NotFound);
        }
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM resources r
                     WHERE r.workspace_id = ?1
                       AND r.resource_id = ?2
                       AND (r.context_document_json IS NULL
                            OR json_extract(r.context_document_json, '$.status') = 'ACTIVE')
                       AND (?3 IS NULL OR EXISTS(
                            SELECT 1 FROM resource_revisions rr
                             WHERE rr.resource_id = r.resource_id
                               AND rr.resource_revision_id = ?3
                       ))
                 )",
                params![workspace_id, resource_id, revision_id],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if !exists {
            return Err(StoreError::NotFound);
        }
    }
    Ok(())
}

fn put_capability_grant_state(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    grant: &StoredCapabilityGrant,
    status: &str,
    version: u64,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(&grant.redacted_snapshot(status, version)?)?;
    let blob = store.inner.blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        "application/vnd.litecow.capability-grant+json",
    )?;
    if blob.digest != digest(&bytes)
        || blob.size_bytes != bytes.len() as u64
        || store
            .inner
            .blobs
            .get(workspace_id, BlobPurpose::AggregateState, &blob)?
            != bytes
    {
        return Err(StoreError::Integrity(
            "CapabilityGrant aggregate-state blob failed verification".into(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: version,
        record_schema_version: 1,
    })
}

fn validate_capability_grant_issue(command: &IssueCapabilityGrantCommit) -> Result<(), StoreError> {
    let event = &command.event;
    let grant = &command.grant;
    if command.workspace_id.trim().is_empty()
        || command.workspace_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.actor.principal_kind != "USER"
        || command.actor.principal_id != command.actor.owner_principal_id
        || !assurance_satisfies(&command.actor.assurance_level, "APPROVE_SAFE")
        || command.request_id.trim().is_empty()
        || command.request_id.len() > MAX_REQUEST_ID_BYTES
        || command.approval_id.is_some()
        || grant.capability_grant_id.trim().is_empty()
        || grant.capability_grant_id.len() > MAX_APPROVAL_ID_BYTES
        || grant.status != CapabilityGrantStatus::Active
        || grant.version != 1
        || grant.allowed_operations.is_empty()
        || grant.allowed_operations.len() > 128
        || grant.allowed_operations.iter().any(|operation| {
            operation.trim().is_empty()
                || operation.len() > 256
                || operation.chars().any(char::is_control)
        })
        || !capability_ref_matches_contract(&grant.capability_ref)
        || !resource_scope_matches_contract(&grant.resource_scope, Some(&command.workspace_id))
        || !grant.secret_refs.is_empty()
        || grant.expires_at.is_none()
        || grant.granted_by.get("principal_id").and_then(Value::as_str)
            != Some(command.actor.principal_id.as_str())
        || grant.granted_by.get("kind").and_then(Value::as_str) != Some("USER")
        || event.workspace_id != command.workspace_id
        || event.entity_type != "capability_grant"
        || event.entity_id != grant.capability_grant_id
        || event.event_type != "capability.grant.created.v1"
        || event.schema_version != 1
        || event.entity_revision != 1
        || event.event_id.trim().is_empty()
        || event.origin_runtime_id.trim().is_empty()
        || event.hlc_timestamp.trim().is_empty()
        || event.correlation_id.trim().is_empty()
        || grant.created_at != event.recorded_at
        || grant
            .expires_at
            .as_deref()
            .is_some_and(|expires| expires <= grant.created_at.as_str())
    {
        return Err(StoreError::Invalid(
            "CapabilityGrant issue request is malformed or exceeds owner-scoped authority".into(),
        ));
    }
    let allowed = match &grant.scope {
        CapabilityGrantScope::Conversation { .. } => {
            return Err(StoreError::Invalid(
                "Conversation grants require an exact live turn binding, which this storage contract does not yet carry".into(),
            ));
        }
        CapabilityGrantScope::TaskPlanning { task_id } => {
            if task_id.trim().is_empty() || !grant.secret_refs.is_empty() {
                return Err(StoreError::Invalid(
                    "Task-planning grants must be read-only and contain no SecretRefs".into(),
                ));
            }
            true
        }
        CapabilityGrantScope::AttemptExecution { .. } => {
            // Attempt grant IDs are pinned by the Attempt admission transaction.
            // A standalone insert would create authority that no Attempt owns.
            false
        }
    };
    if !allowed {
        return Err(StoreError::Invalid(
            "Attempt grants must be issued atomically by Attempt admission".into(),
        ));
    }
    // This adapter only enables the concrete read capability already served by
    // the local product. Unknown operations remain denied until their own policy
    // evaluator and provider admission path are wired.
    if grant
        .allowed_operations
        .iter()
        .any(|operation| operation != "workspace.resource.read")
        || grant.resource_scope.get("operation_ids") != Some(&json!(grant.allowed_operations))
    {
        return Err(StoreError::Invalid(
            "only the explicitly supported read-only operation may be granted here".into(),
        ));
    }
    if grant
        .resource_scope
        .get("resource_refs")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        return Err(StoreError::Invalid(
            "the supported read grant requires explicit same-Workspace ResourceRefs".into(),
        ));
    }
    let created = time::OffsetDateTime::parse(
        &grant.created_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| StoreError::Invalid("CapabilityGrant creation time is invalid".into()))?;
    let expires_at = grant.expires_at.as_deref().ok_or_else(|| {
        StoreError::Invalid("Conversation and planning grants must expire".into())
    })?;
    let expires =
        time::OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
            .map_err(|_| StoreError::Invalid("CapabilityGrant expiry is invalid".into()))?;
    if expires - created > time::Duration::hours(24) {
        return Err(StoreError::Invalid(
            "Conversation and planning grants are limited to 24 hours".into(),
        ));
    }
    for value in [
        command.workspace_id.as_str(),
        command.actor.principal_id.as_str(),
        command.actor.owner_principal_id.as_str(),
        command.request_id.as_str(),
        grant.capability_grant_id.as_str(),
        event.event_id.as_str(),
        event.origin_runtime_id.as_str(),
        event.hlc_timestamp.as_str(),
        event.correlation_id.as_str(),
    ] {
        if value.chars().any(char::is_control) {
            return Err(StoreError::Invalid(
                "CapabilityGrant identifiers are malformed".into(),
            ));
        }
    }
    Ok(())
}

fn capability_grant_issue_digest(
    command: &IssueCapabilityGrantCommit,
) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "workspace_id": command.workspace_id,
        // Grant ID, event ID and creation time are generated by Core. A retry
        // after a lost response binds the same requested authority, not those
        // regenerated receipt identifiers.
        "intent": {
            "scope": &command.grant.scope,
            "capability_ref": &command.grant.capability_ref,
            "allowed_operations": &command.grant.allowed_operations,
            "resource_scope": &command.grant.resource_scope,
            "secret_refs": command.grant.secret_refs.iter().map(|secret_ref| json!({
                "secret_ref_id": secret_ref.secret_ref_id,
                "placement": secret_ref.placement,
            })).collect::<Vec<_>>(),
            "granted_by": &command.grant.granted_by,
            "expires_at": &command.grant.expires_at,
        },
    }))?))
}

fn find_capability_grant_issue_replay(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
) -> Result<Option<CommittedCapabilityGrant>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest,response_json,response_digest FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
        params![principal_id, request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((stored_digest, response, response_digest)) = prior else {
        return Ok(None);
    };
    if stored_digest != request_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response.ok_or_else(|| {
        StoreError::Integrity("CapabilityGrant issue receipt has no response".into())
    })?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "CapabilityGrant issue receipt digest mismatch".into(),
        ));
    }
    let mut committed: CommittedCapabilityGrant = serde_json::from_str(&response)
        .map_err(|_| StoreError::Integrity("CapabilityGrant issue receipt is malformed".into()))?;
    committed.replayed = true;
    Ok(Some(committed))
}

fn issue_capability_grant_transaction(
    connection: &mut Connection,
    command: IssueCapabilityGrantCommit,
    state_ref: AggregateStateRef,
    request_digest: String,
) -> Result<CommittedCapabilityGrant, StoreError> {
    validate_capability_grant_issue(&command)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    if let Some(replay) = find_capability_grant_issue_replay(
        &tx,
        &command.actor.principal_id,
        &command.request_id,
        &request_digest,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replay);
    }
    match &command.grant.scope {
        CapabilityGrantScope::Conversation { .. } => unreachable!("validated above"),
        CapabilityGrantScope::TaskPlanning { task_id } => {
            let active: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1 AND workspace_id=?2 AND status IN ('OPEN','RUNNING','WAITING_USER','WAITING_DEPENDENCY'))",
                params![task_id, command.workspace_id], |row| row.get(0),
            ).map_err(map_database_error)?;
            if !active {
                return Err(StoreError::Invalid(
                    "Task-planning grant requires an active Task in this Workspace".into(),
                ));
            }
            validate_grant_resource_refs(
                &tx,
                &command.workspace_id,
                &command.grant.resource_scope,
            )?;
        }
        CapabilityGrantScope::AttemptExecution { .. } => unreachable!("validated above"),
    }
    let stored = stored_grant_from_record(&command.grant)?;
    let payload = stored.event_payload()?;
    tx.execute(
        "INSERT INTO capability_grants(capability_grant_id,scope_kind,conversation_id,task_id,attempt_id,capability_ref_json,allowed_operations_json,resource_scope_json,secret_refs_json,granted_by_json,expires_at,status,created_at,version) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'[]',?9,?10,'ACTIVE',?11,1)",
        params![stored.capability_grant_id, stored.scope_kind, stored.conversation_id, stored.task_id, stored.attempt_id,
            encode(&stored.capability_ref)?, encode(&stored.allowed_operations)?, encode(&stored.resource_scope)?, encode(&stored.granted_by)?, stored.expires_at, stored.created_at],
    ).map_err(map_database_error)?;
    let mut event = command.event.clone();
    event.payload = payload;
    let committed_event = insert_domain_event(&tx, &event, &state_ref)?;
    tx.execute(
        "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at) VALUES (?1,'capability_grant',?2,1,1,?3,?4,?5)",
        params![command.workspace_id, stored.capability_grant_id, state_ref.blob.digest, encode(&state_ref.blob)?, event.recorded_at],
    ).map_err(map_database_error)?;
    let scope_digest = stored.scope_digest()?;
    let principal =
        json!({"principal_id": command.actor.principal_id, "kind": command.actor.principal_kind});
    let audit_digest = digest(&canonical_json(
        &json!({"capability_grant_id": stored.capability_grant_id, "scope_digest": scope_digest}),
    )?);
    tx.execute(
        "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest) VALUES (?1,?2,?3,'CAPABILITY_GRANT_ISSUED',?4,'ALLOW','OWNER_AUTHORIZED_READ_GRANT',?5,?6,?7)",
        params![format!("{}:audit", event.event_id), command.workspace_id, encode(&principal)?, encode(&json!({"capability_grant_id":stored.capability_grant_id}))?, event.correlation_id, event.recorded_at, audit_digest],
    ).map_err(map_database_error)?;
    let grant = grant_record_from_stored(stored)?;
    let committed = CommittedCapabilityGrant {
        grant,
        event: committed_event,
        replayed: false,
    };
    let response = encode(&committed)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![command.actor.principal_id, command.request_id, request_digest, response, digest(response.as_bytes()), event.recorded_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn validate_grant_revocation_command(
    command: &CapabilityGrantRevocationCommit,
) -> Result<(), StoreError> {
    let event = &command.event;
    if command.workspace_id.trim().is_empty()
        || command.workspace_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.actor.principal_id != command.actor.owner_principal_id
        || command.actor.principal_kind != "USER"
        || assurance_level(&command.actor.assurance_level).is_none()
        || command.actor.principal_id.trim().is_empty()
        || command.actor.principal_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.request_id.trim().is_empty()
        || command.request_id.len() > MAX_REQUEST_ID_BYTES
        || command.capability_grant_id.trim().is_empty()
        || command.capability_grant_id.len() > MAX_APPROVAL_ID_BYTES
        || command.expected_version == 0
        || event.workspace_id != command.workspace_id
        || event.entity_type != "capability_grant"
        || event.entity_id != command.capability_grant_id
        || event.event_type != "capability.grant.revoked.v1"
        || event.schema_version != 1
        || event.entity_revision
            != command
                .expected_version
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("CapabilityGrant version overflow".into()))?
        || event.origin_runtime_id.trim().is_empty()
        || event.hlc_timestamp.trim().is_empty()
        || event.correlation_id.trim().is_empty()
        || [
            command.workspace_id.as_str(),
            command.actor.principal_id.as_str(),
            command.request_id.as_str(),
            command.capability_grant_id.as_str(),
            event.event_id.as_str(),
            event.origin_runtime_id.as_str(),
            event.hlc_timestamp.as_str(),
            event.correlation_id.as_str(),
        ]
        .iter()
        .any(|value| value.chars().any(char::is_control))
    {
        return Err(StoreError::Invalid(
            "CapabilityGrant revocation command is invalid".into(),
        ));
    }
    Ok(())
}

fn grant_revocation_request_digest(
    command: &CapabilityGrantRevocationCommit,
) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "workspace_id": command.workspace_id,
        "capability_grant_id": command.capability_grant_id,
        "expected_version": command.expected_version,
    }))?))
}

fn find_grant_revocation_replay(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
) -> Result<Option<CommittedCapabilityGrantRevocation>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest,response_json,response_digest FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
        params![principal_id, request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((stored_digest, response, response_digest)) = prior else {
        return Ok(None);
    };
    if stored_digest != request_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response
        .ok_or_else(|| StoreError::Integrity("CapabilityGrant receipt has no response".into()))?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "CapabilityGrant receipt digest mismatch".into(),
        ));
    }
    let mut committed: CommittedCapabilityGrantRevocation = serde_json::from_str(&response)
        .map_err(|_| StoreError::Integrity("CapabilityGrant receipt is malformed".into()))?;
    committed.replayed = true;
    Ok(Some(committed))
}

fn revoke_capability_grant_transaction(
    connection: &mut Connection,
    command: CapabilityGrantRevocationCommit,
    current: StoredCapabilityGrant,
    state_ref: AggregateStateRef,
    request_digest: String,
) -> Result<CommittedCapabilityGrantRevocation, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    let principal_id = command.actor.principal_id.clone();
    if let Some(prior) =
        find_grant_revocation_replay(&tx, &principal_id, &command.request_id, &request_digest)?
    {
        tx.commit().map_err(map_database_error)?;
        return Ok(prior);
    }
    let actual = load_capability_grant(&tx, &command.workspace_id, &command.capability_grant_id)?
        .ok_or(StoreError::NotFound)?;
    if actual != current || actual.version != command.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(actual.version),
        });
    }
    if actual.status != "ACTIVE" {
        return Err(StoreError::Invalid(
            "only an active CapabilityGrant can be revoked".into(),
        ));
    }
    let next_version = actual
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("CapabilityGrant version overflow".into()))?;
    let changed = tx.execute(
        "UPDATE capability_grants SET status='REVOKED',version=?1 WHERE capability_grant_id=?2 AND status='ACTIVE' AND version=?3",
        params![to_sql_i64(next_version, "CapabilityGrant version")?, command.capability_grant_id, to_sql_i64(command.expected_version, "CapabilityGrant version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: None,
        });
    }
    let mut event = command.event.clone();
    event.entity_revision = next_version;
    event.payload = actual.event_payload()?;
    let committed_event = insert_domain_event(&tx, &event, &state_ref)?;
    tx.execute(
        "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at) VALUES (?1,'capability_grant',?2,?3,1,?4,?5,?6)",
        params![command.workspace_id, command.capability_grant_id, to_sql_i64(next_version, "CapabilityGrant version")?, state_ref.blob.digest, encode(&state_ref.blob)?, event.recorded_at],
    ).map_err(map_database_error)?;
    let audit_digest = digest(&canonical_json(&json!({
        "capability_grant_id": command.capability_grant_id,
        "version": next_version,
        "scope_digest": actual.scope_digest()?,
    }))?);
    tx.execute(
        "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest) VALUES (?1,?2,?3,'CAPABILITY_GRANT_REVOKED',?4,'ALLOW','OWNER_REVOKED_GRANT',?5,?6,?7)",
        params![format!("{}:audit", event.event_id), command.workspace_id, encode(&json!({"principal_id":command.actor.principal_id,"kind":command.actor.principal_kind}))?, encode(&json!({"capability_grant_id":command.capability_grant_id}))?, event.correlation_id, event.recorded_at, audit_digest],
    ).map_err(map_database_error)?;
    let committed = CommittedCapabilityGrantRevocation {
        capability_grant_id: command.capability_grant_id.clone(),
        status: CapabilityGrantStatus::Revoked,
        version: next_version,
        event: committed_event,
        replayed: false,
    };
    let response = encode(&committed)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![command.actor.principal_id, command.request_id, request_digest, response, digest(response.as_bytes()), event.recorded_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn validate_secret_lease_issue(command: &IssueSecretLeaseCommit) -> Result<(), StoreError> {
    let lease = &command.lease;
    validate_nonempty(&[
        &command.workspace_id,
        &command.actor.principal_id,
        &command.actor.owner_principal_id,
        &command.request_id,
        &lease.secret_lease_id,
        &lease.secret_ref.secret_ref_id,
        &lease.secret_ref.provider_ref,
        &lease.task_id,
        &lease.attempt_id,
        &lease.runtime_id,
    ])?;
    if [
        command.workspace_id.as_str(),
        command.actor.principal_id.as_str(),
        command.actor.owner_principal_id.as_str(),
        command.request_id.as_str(),
        lease.secret_lease_id.as_str(),
        lease.secret_ref.secret_ref_id.as_str(),
        lease.secret_ref.provider_ref.as_str(),
        lease.task_id.as_str(),
        lease.attempt_id.as_str(),
        lease.runtime_id.as_str(),
        command.actor.assurance_level.as_str(),
    ]
    .iter()
    .any(|value| value.chars().any(char::is_control))
    {
        return Err(StoreError::Invalid(
            "SecretLease identifiers contain control characters".into(),
        ));
    }
    if command.workspace_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.actor.principal_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.actor.owner_principal_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.request_id.len() > MAX_REQUEST_ID_BYTES
        || lease.secret_lease_id.len() > MAX_APPROVAL_ID_BYTES
        || lease.secret_ref.secret_ref_id.len() > MAX_APPROVAL_ID_BYTES
        || lease.secret_ref.provider_ref.len() > MAX_APPROVAL_TEXT_BYTES
        || lease.task_id.len() > MAX_APPROVAL_ID_BYTES
        || lease.attempt_id.len() > MAX_APPROVAL_ID_BYTES
        || lease.runtime_id.len() > MAX_APPROVAL_ID_BYTES
        || lease.allowed_usage.is_empty()
        || lease.allowed_usage.len() > 32
        || lease.allowed_usage.iter().any(|usage| {
            usage.trim().is_empty() || usage.len() > 128 || usage.chars().any(char::is_control)
        })
        || canonical_json(&lease.capability_ref)?.len() > MAX_APPROVAL_TEXT_BYTES
        || lease.status != "ACTIVE"
        || lease.version != 1
        || lease.secret_ref.placement == SecretPlacement::ExternalAgentOwned
        || !lease.capability_ref.is_object()
        || command.actor.principal_kind != "USER"
        || command.actor.principal_id != command.actor.owner_principal_id
        || !assurance_satisfies(&command.actor.assurance_level, "APPROVE_SENSITIVE")
        || command.event.workspace_id != command.workspace_id
        || command.event.entity_type != "secret_lease"
        || command.event.entity_id != lease.secret_lease_id
        || command.event.event_type != "secret.lease.issued.v1"
        || command.event.schema_version != 1
        || command.event.entity_revision != 1
        || lease.issued_at != command.event.recorded_at
        || lease.expires_at <= lease.issued_at
    {
        return Err(StoreError::Invalid(
            "SecretLease issue request is malformed".into(),
        ));
    }
    Ok(())
}

fn validate_secret_lease_revoke(command: &RevokeSecretLeaseCommit) -> Result<(), StoreError> {
    validate_nonempty(&[
        &command.workspace_id,
        &command.actor.principal_id,
        &command.actor.owner_principal_id,
        &command.request_id,
        &command.secret_lease_id,
    ])?;
    if [
        command.workspace_id.as_str(),
        command.actor.principal_id.as_str(),
        command.actor.owner_principal_id.as_str(),
        command.request_id.as_str(),
        command.secret_lease_id.as_str(),
        command.actor.assurance_level.as_str(),
    ]
    .iter()
    .any(|value| value.chars().any(char::is_control))
    {
        return Err(StoreError::Invalid(
            "SecretLease identifiers contain control characters".into(),
        ));
    }
    if command.workspace_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.actor.principal_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.actor.owner_principal_id.len() > MAX_PRINCIPAL_ID_BYTES
        || command.actor.principal_kind != "USER"
        || command.actor.principal_id != command.actor.owner_principal_id
        || assurance_level(&command.actor.assurance_level).is_none()
        || command.request_id.len() > MAX_REQUEST_ID_BYTES
        || command.secret_lease_id.len() > MAX_APPROVAL_ID_BYTES
        || command.expected_version == 0
        || command.event.workspace_id != command.workspace_id
        || command.event.entity_type != "secret_lease"
        || command.event.entity_id != command.secret_lease_id
        || command.event.event_type != "secret.lease.revoked.v1"
        || command.event.schema_version != 1
    {
        return Err(StoreError::Invalid(
            "SecretLease revoke request is malformed".into(),
        ));
    }
    Ok(())
}

fn issue_secret_lease_transaction(
    connection: &mut Connection,
    command: IssueSecretLeaseCommit,
    state_ref: AggregateStateRef,
) -> Result<CommittedSecretLease, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    if let Some(replay) = find_secret_lease_replay(
        &tx,
        &command.actor.principal_id,
        &command.request_id,
        &secret_lease_issue_digest(&command)?,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replay);
    }
    let lease = &command.lease;
    let authority: bool = tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM capability_grants g
           JOIN tasks t ON t.task_id=g.task_id AND t.workspace_id=?1
           JOIN attempts a ON a.attempt_id=g.attempt_id AND a.task_id=t.task_id
           JOIN runtimes r ON r.runtime_id=a.runtime_id AND r.workspace_id=t.workspace_id
           JOIN runtime_incarnations ri ON ri.runtime_id=r.runtime_id AND ri.runtime_incarnation_id=a.runtime_incarnation_id
           WHERE g.capability_grant_id IN (SELECT value FROM json_each(a.capability_grant_ids_json))
             AND g.scope_kind='ATTEMPT_EXECUTION' AND g.status='ACTIVE'
             AND g.task_id=?2 AND g.attempt_id=?3
             AND a.runtime_id=?4 AND r.runtime_id=?4 AND r.availability='ONLINE'
             AND r.current_incarnation_id=a.runtime_incarnation_id AND ri.recovery_state='READY'
             AND EXISTS (SELECT 1 FROM execution_leases el
               WHERE el.task_id=t.task_id AND el.attempt_id=a.attempt_id
                 AND el.runtime_id=a.runtime_id
                 AND el.runtime_incarnation_id=a.runtime_incarnation_id
                 AND el.state='ACTIVE' AND el.expires_at>=?5)
             AND a.status IN ('PREPARING','RUNNING','WAITING_APPROVAL','WAITING_RESOURCE','CHECKPOINTING')
             AND t.status NOT IN ('CANCEL_REQUESTED','CANCELLED','COMPLETED','FAILED')
             AND (g.expires_at IS NULL OR g.expires_at>=?5)
             AND ?10 <= strftime('%Y-%m-%dT%H:%M:%fZ','now')
             AND ?11 > strftime('%Y-%m-%dT%H:%M:%fZ','now')
             AND EXISTS (SELECT 1 FROM json_each(g.secret_refs_json) sr
                         WHERE json_extract(sr.value,'$.secret_ref_id')=?6
                           AND json_extract(sr.value,'$.provider_ref')=?7
                           AND json_extract(sr.value,'$.placement')=?8)
             AND json(g.capability_ref_json)=json(?9)
             AND NOT EXISTS (SELECT 1 FROM json_each(?12) usage
               WHERE NOT EXISTS (SELECT 1 FROM json_each(g.allowed_operations_json) op
                                 WHERE op.value=usage.value))
         )",
        params![command.workspace_id, lease.task_id, lease.attempt_id, lease.runtime_id,
            lease.expires_at, lease.secret_ref.secret_ref_id, lease.secret_ref.provider_ref,
            secret_placement_name(lease.secret_ref.placement),
            encode(&lease.capability_ref)?, lease.issued_at, lease.expires_at, encode(&lease.allowed_usage)?],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !authority {
        return Err(StoreError::Invalid(
            "SecretLease requires an active Attempt grant containing this secret and capability"
                .into(),
        ));
    }
    let payload = secret_lease_event_payload(lease)?;
    let mut event = command.event.clone();
    event.payload = payload;
    tx.execute(
        "INSERT INTO secret_leases(secret_lease_id,secret_ref_json,task_id,attempt_id,capability_ref_json,runtime_id,allowed_usage_json,status,issued_at,expires_at,version) VALUES (?1,?2,?3,?4,?5,?6,?7,'ACTIVE',?8,?9,1)",
        params![lease.secret_lease_id, encode(&lease.secret_ref)?, lease.task_id, lease.attempt_id,
            encode(&lease.capability_ref)?, lease.runtime_id, encode(&lease.allowed_usage)?, lease.issued_at, lease.expires_at],
    ).map_err(map_database_error)?;
    let event = insert_domain_event(&tx, &event, &state_ref)?;
    tx.execute(
        "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at) VALUES (?1,'secret_lease',?2,1,1,?3,?4,?5)",
        params![command.workspace_id, lease.secret_lease_id, state_ref.blob.digest, encode(&state_ref.blob)?, event.recorded_at],
    ).map_err(map_database_error)?;
    let scope_digest = event
        .payload
        .get("scope_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::Integrity("SecretLease event omitted scope digest".into()))?;
    tx.execute(
        "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest) VALUES (?1,?2,?3,'SECRET_LEASE_ISSUED',?4,'ALLOW','ACTIVE_ATTEMPT_GRANT',?5,?6,?7)",
        params![format!("{}:audit", command.event.event_id), command.workspace_id,
            encode(&json!({"principal_id":command.actor.principal_id,"kind":command.actor.principal_kind}))?,
            encode(&json!({"secret_lease_id":lease.secret_lease_id,"task_id":lease.task_id,"attempt_id":lease.attempt_id,"runtime_id":lease.runtime_id}))?,
            command.event.correlation_id, event.recorded_at, scope_digest],
    ).map_err(map_database_error)?;
    let committed = CommittedSecretLease {
        lease: lease.clone(),
        event,
        replayed: false,
    };
    insert_secret_lease_receipt(
        &tx,
        &command.actor.principal_id,
        &command.request_id,
        &secret_lease_issue_digest(&command)?,
        &committed,
        &command.event.recorded_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn secret_placement_name(placement: SecretPlacement) -> &'static str {
    match placement {
        SecretPlacement::LocalOnly => "LOCAL_ONLY",
        SecretPlacement::CloudAvailable => "CLOUD_AVAILABLE",
        SecretPlacement::RuntimeBound => "RUNTIME_BOUND",
        SecretPlacement::ExternalAgentOwned => "EXTERNAL_AGENT_OWNED",
    }
}

fn revoke_secret_lease_transaction(
    connection: &mut Connection,
    command: RevokeSecretLeaseCommit,
    next: SecretLeaseRecord,
    state_ref: AggregateStateRef,
) -> Result<CommittedSecretLease, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    if let Some(replay) = find_secret_lease_replay(
        &tx,
        &command.actor.principal_id,
        &command.request_id,
        &secret_lease_revoke_digest(&command)?,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replay);
    }
    let current = load_secret_lease(&tx, &command.workspace_id, &command.secret_lease_id)?
        .ok_or(StoreError::NotFound)?;
    if current.version != command.expected_version || current.status != "ACTIVE" {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(current.version),
        });
    }
    let mut expected_next = current.clone();
    expected_next.status = "REVOKED".into();
    expected_next.version = current
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("SecretLease version exhausted".into()))?;
    if next != expected_next {
        return Err(StoreError::Invalid(
            "SecretLease revocation cannot change its immutable authority scope".into(),
        ));
    }
    let changed = tx.execute(
        "UPDATE secret_leases SET status='REVOKED',version=?1 WHERE secret_lease_id=?2 AND status='ACTIVE' AND version=?3",
        params![to_sql_i64(next.version, "SecretLease version")?, command.secret_lease_id, to_sql_i64(command.expected_version, "SecretLease version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: None,
        });
    }
    let mut event = command.event.clone();
    event.entity_revision = next.version;
    event.payload = secret_lease_event_payload(&next)?;
    let event = insert_domain_event(&tx, &event, &state_ref)?;
    tx.execute(
        "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at) VALUES (?1,'secret_lease',?2,?3,1,?4,?5,?6)",
        params![command.workspace_id, next.secret_lease_id, to_sql_i64(next.version, "SecretLease version")?, state_ref.blob.digest, encode(&state_ref.blob)?, event.recorded_at],
    ).map_err(map_database_error)?;
    let scope_digest = event
        .payload
        .get("scope_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::Integrity("SecretLease event omitted scope digest".into()))?;
    tx.execute(
        "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest) VALUES (?1,?2,?3,'SECRET_LEASE_REVOKED',?4,'ALLOW','OWNER_REVOKED',?5,?6,?7)",
        params![format!("{}:audit", command.event.event_id), command.workspace_id,
            encode(&json!({"principal_id":command.actor.principal_id,"kind":command.actor.principal_kind}))?,
            encode(&json!({"secret_lease_id":next.secret_lease_id,"task_id":next.task_id,"attempt_id":next.attempt_id,"runtime_id":next.runtime_id}))?,
            command.event.correlation_id, event.recorded_at, scope_digest],
    ).map_err(map_database_error)?;
    let committed = CommittedSecretLease {
        lease: next,
        event,
        replayed: false,
    };
    insert_secret_lease_receipt(
        &tx,
        &command.actor.principal_id,
        &command.request_id,
        &secret_lease_revoke_digest(&command)?,
        &committed,
        &command.event.recorded_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn find_secret_lease_replay(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
) -> Result<Option<CommittedSecretLease>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest,response_json,response_digest FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            params![principal_id, request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((stored_digest, response, response_digest)) = prior else {
        return Ok(None);
    };
    if stored_digest != request_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response
        .ok_or_else(|| StoreError::Integrity("SecretLease receipt omitted response".into()))?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "SecretLease receipt digest mismatch".into(),
        ));
    }
    let mut committed: CommittedSecretLease = serde_json::from_str(&response)
        .map_err(|_| StoreError::Integrity("SecretLease receipt is malformed".into()))?;
    committed.replayed = true;
    Ok(Some(committed))
}

fn insert_secret_lease_receipt(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
    committed: &CommittedSecretLease,
    recorded_at: &str,
) -> Result<(), StoreError> {
    let response = encode(committed)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![principal_id,request_id,request_digest,response,digest(response.as_bytes()),recorded_at],
    ).map_err(map_database_error)?;
    Ok(())
}

fn secret_lease_issue_digest(command: &IssueSecretLeaseCommit) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "workspace_id":command.workspace_id,
        "secret_ref_id":command.lease.secret_ref.secret_ref_id,
        "provider_ref_digest":digest(command.lease.secret_ref.provider_ref.as_bytes()),
        "placement":command.lease.secret_ref.placement,
        "task_id":command.lease.task_id,
        "attempt_id":command.lease.attempt_id,
        "capability_ref":command.lease.capability_ref,
        "runtime_id":command.lease.runtime_id,
        "allowed_usage":command.lease.allowed_usage,
    }))?))
}

fn secret_lease_revoke_digest(command: &RevokeSecretLeaseCommit) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "workspace_id":command.workspace_id,
        "secret_lease_id":command.secret_lease_id,
        "expected_version":command.expected_version,
    }))?))
}

fn load_secret_lease(
    tx: &Transaction<'_>,
    workspace_id: &str,
    lease_id: &str,
) -> Result<Option<SecretLeaseRecord>, StoreError> {
    let raw = tx.query_row(
        "SELECT l.secret_lease_id,l.secret_ref_json,l.task_id,l.attempt_id,l.capability_ref_json,l.runtime_id,l.allowed_usage_json,l.status,l.issued_at,l.expires_at,l.version FROM secret_leases l JOIN tasks t ON t.task_id=l.task_id WHERE t.workspace_id=?1 AND l.secret_lease_id=?2",
        params![workspace_id, lease_id],
        |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,Option<String>>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,String>(7)?,row.get::<_,String>(8)?,row.get::<_,String>(9)?,row.get::<_,i64>(10)?)),
    ).optional().map_err(map_database_error)?;
    raw.map(|r| {
        let attempt_id = r.3.ok_or_else(|| {
            StoreError::Integrity("SecretLease is missing its Attempt binding".into())
        })?;
        let capability_ref = r.4.ok_or_else(|| {
            StoreError::Integrity("SecretLease is missing its capability binding".into())
        })?;
        Ok(SecretLeaseRecord {
            secret_lease_id: r.0,
            secret_ref: serde_json::from_str(&r.1)
                .map_err(|e| StoreError::Integrity(e.to_string()))?,
            task_id: r.2,
            attempt_id,
            capability_ref: serde_json::from_str(&capability_ref)
                .map_err(|e| StoreError::Integrity(e.to_string()))?,
            runtime_id: r.5,
            allowed_usage: serde_json::from_str(&r.6)
                .map_err(|e| StoreError::Integrity(e.to_string()))?,
            status: r.7,
            issued_at: r.8,
            expires_at: r.9,
            version: from_sql_i64(r.10, "SecretLease version")?,
        })
    })
    .transpose()
}

fn secret_lease_event_payload(lease: &SecretLeaseRecord) -> Result<Value, StoreError> {
    let scope_digest = digest(&canonical_json(&json!({
        "secret_ref_id":lease.secret_ref.secret_ref_id,
        "task_id":lease.task_id,"attempt_id":lease.attempt_id,"capability_ref":lease.capability_ref,
        "runtime_id":lease.runtime_id,"allowed_usage":lease.allowed_usage,
    }))?);
    Ok(
        json!({"secret_lease_id":lease.secret_lease_id,"task_id":lease.task_id,
        "attempt_id":lease.attempt_id,"runtime_id":lease.runtime_id,
        "scope_digest":scope_digest,"expires_at":lease.expires_at}),
    )
}

fn put_secret_lease_state(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    lease: &SecretLeaseRecord,
) -> Result<AggregateStateRef, StoreError> {
    // Aggregate snapshots can be replicated with their events. Keep the local
    // OS-keychain locator out of that portable state and persist only its opaque
    // reference ID and a one-way digest of the provider locator.
    let bytes = canonical_json(&secret_lease_aggregate_state(lease))?;
    let blob = store.inner.blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        "application/vnd.litecow.secret-lease+json",
    )?;
    if blob.digest != digest(&bytes)
        || blob.size_bytes != bytes.len() as u64
        || store
            .inner
            .blobs
            .get(workspace_id, BlobPurpose::AggregateState, &blob)?
            != bytes
    {
        return Err(StoreError::Integrity(
            "SecretLease aggregate-state blob failed verification".into(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: lease.version,
        record_schema_version: 1,
    })
}

fn secret_lease_aggregate_state(lease: &SecretLeaseRecord) -> Value {
    json!({
        "secret_lease_id": lease.secret_lease_id,
        "secret_ref_id": lease.secret_ref.secret_ref_id,
        "provider_ref_digest": digest(lease.secret_ref.provider_ref.as_bytes()),
        "placement": lease.secret_ref.placement,
        "task_id": lease.task_id,
        "attempt_id": lease.attempt_id,
        "capability_ref": lease.capability_ref,
        "runtime_id": lease.runtime_id,
        "allowed_usage": lease.allowed_usage,
        "status": lease.status,
        "issued_at": lease.issued_at,
        "expires_at": lease.expires_at,
        "version": lease.version,
    })
}

fn validate_approval_page_request(
    owner_principal_id: &str,
    request: &ApprovalPageRequest,
) -> Result<(), StoreError> {
    for (name, value) in [
        ("owner principal ID", owner_principal_id),
        ("Workspace ID", request.workspace_id.as_str()),
    ] {
        if value.trim().is_empty()
            || value.len() > MAX_PRINCIPAL_ID_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(StoreError::Invalid(format!("{name} is malformed")));
        }
    }
    if !(1..=200).contains(&request.limit)
        || request.status.as_deref().is_some_and(|status| {
            !matches!(
                status,
                "PENDING" | "APPROVED" | "DENIED" | "EXPIRED" | "CANCELLED"
            )
        })
    {
        return Err(StoreError::Invalid(
            "Approval page filter or limit is invalid".to_owned(),
        ));
    }
    if let Some(cursor) = request.cursor.as_ref() {
        canonicalize_utc_timestamp(&cursor.after_requested_at)
            .map_err(|_| StoreError::Invalid("Approval cursor is invalid".to_owned()))?;
        if cursor.workspace_id != request.workspace_id
            || cursor.status != request.status
            || cursor.limit != request.limit
            || cursor.after_approval_id.trim().is_empty()
            || cursor.after_approval_id.len() > MAX_APPROVAL_ID_BYTES
            || cursor.after_approval_id.chars().any(char::is_control)
        {
            return Err(StoreError::Invalid(
                "Approval cursor does not match its page query".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_request_command(command: &RequestApprovalCommit) -> Result<(), StoreError> {
    let approval = &command.approval;
    for (name, value, max) in [
        (
            "Workspace ID",
            command.workspace_id.as_str(),
            MAX_PRINCIPAL_ID_BYTES,
        ),
        (
            "principal ID",
            command.actor.principal_id.as_str(),
            MAX_PRINCIPAL_ID_BYTES,
        ),
        (
            "owner principal ID",
            command.actor.owner_principal_id.as_str(),
            MAX_PRINCIPAL_ID_BYTES,
        ),
        (
            "request ID",
            command.request_id.as_str(),
            MAX_REQUEST_ID_BYTES,
        ),
        (
            "Approval ID",
            approval.approval_id.as_str(),
            MAX_APPROVAL_ID_BYTES,
        ),
        ("Task ID", approval.task_id.as_str(), MAX_APPROVAL_ID_BYTES),
        ("Approval kind", approval.kind.as_str(), 128),
        (
            "action summary",
            approval.action_summary.as_str(),
            MAX_APPROVAL_TEXT_BYTES,
        ),
        (
            "required assurance",
            approval.required_assurance.as_str(),
            64,
        ),
    ] {
        if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
            return Err(StoreError::Invalid(format!("{name} is malformed")));
        }
    }
    if !matches!(
        command.actor.principal_kind.as_str(),
        "USER" | "AGENT" | "SERVICE" | "RUNTIME"
    ) || command.actor.assurance_level.trim().is_empty()
        || command.actor.assurance_level.len() > 64
        || command.actor.assurance_level.chars().any(char::is_control)
    {
        return Err(StoreError::Invalid(
            "Approval requester identity is malformed".to_owned(),
        ));
    }
    if let Some(attempt_id) = approval.requested_by_attempt.as_deref() {
        if attempt_id.trim().is_empty()
            || attempt_id.len() > MAX_APPROVAL_ID_BYTES
            || attempt_id.chars().any(char::is_control)
        {
            return Err(StoreError::Invalid(
                "Approval Attempt ID is malformed".to_owned(),
            ));
        }
    }
    if approval.status != "PENDING"
        || approval.version != 1
        || approval.resolved_by.is_some()
        || approval.resolved_at.is_some()
        || !matches!(approval.risk.as_str(), "SAFE" | "SENSITIVE" | "HIGH_IMPACT")
        || assurance_level(&approval.required_assurance).is_none()
        || !valid_sha256(&approval.scope_digest)
        || !valid_sha256(&approval.action_digest)
    {
        return Err(StoreError::Invalid(
            "Approval request fields are inconsistent".to_owned(),
        ));
    }
    validate_target_ref(&approval.target_ref)?;
    let canonical_action = approval_action_digest(approval)?;
    if canonical_action != approval.action_digest {
        return Err(StoreError::Invalid(
            "Approval action digest does not bind the exact Task, Attempt, action, target, and scope".to_owned(),
        ));
    }
    if command.event.schema_version != 1
        || command.event.workspace_id != command.workspace_id
        || command.event.entity_type != "approval"
        || command.event.entity_id != approval.approval_id
        || command.event.entity_revision != 1
        || command.event.event_type != "approval.requested.v1"
        || command.event.event_id.trim().is_empty()
        || command.event.origin_runtime_id.trim().is_empty()
    {
        return Err(StoreError::Invalid(
            "Approval request event context does not match the requested record".to_owned(),
        ));
    }
    if approval
        .expires_at
        .as_deref()
        .is_some_and(|expires_at| expires_at <= command.event.recorded_at.as_str())
    {
        return Err(StoreError::Invalid(
            "Approval expiry must be after its request time".to_owned(),
        ));
    }
    if approval.requested_at != command.event.recorded_at {
        return Err(StoreError::Invalid(
            "Approval request and event timestamps must match".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Serialize)]
struct ApprovalActionBinding<'a> {
    task_id: &'a str,
    attempt_id: Option<&'a str>,
    action: &'a str,
    target: &'a Value,
    scope_digest: &'a str,
}

fn approval_action_digest(approval: &ApprovalRecord) -> Result<String, StoreError> {
    let binding = ApprovalActionBinding {
        task_id: &approval.task_id,
        attempt_id: approval.requested_by_attempt.as_deref(),
        action: &approval.kind,
        target: &approval.target_ref,
        scope_digest: &approval.scope_digest,
    };
    Ok(digest(&canonical_json(&binding)?))
}

fn validate_target_ref(target: &Value) -> Result<(), StoreError> {
    let bytes = canonical_json(target)?;
    if bytes.len() > MAX_APPROVAL_TEXT_BYTES
        || !target_ref_is_safe(target)
        || !approval_target_ref_matches_contract(target)
    {
        return Err(StoreError::Invalid(
            "Approval target reference is oversized or does not match the credential-free contract"
                .to_owned(),
        ));
    }
    Ok(())
}

fn approval_target_ref_matches_contract(target: &Value) -> bool {
    let Some(object) = target.as_object() else {
        return false;
    };
    if object.len() != 4
        || ![
            "capability_ref",
            "target_digest",
            "execution_method",
            "effect_id",
        ]
        .iter()
        .all(|key| object.contains_key(*key))
    {
        return false;
    }
    let capability_valid = match object.get("capability_ref") {
        Some(Value::Null) => true,
        Some(Value::Object(capability)) => {
            let base = ["capability_id", "identity_kind", "source", "digest"];
            if !base.iter().all(|key| capability.contains_key(*key))
                || capability.keys().any(|key| {
                    ![
                        "capability_id",
                        "identity_kind",
                        "source",
                        "digest",
                        "package_version",
                        "component",
                    ]
                    .contains(&key.as_str())
                })
                || !capability
                    .get("capability_id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty() && value.len() <= 256)
                || !capability
                    .get("source")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty() && value.len() <= 1024)
                || !capability
                    .get("digest")
                    .and_then(Value::as_str)
                    .is_some_and(valid_sha256)
            {
                false
            } else {
                match capability.get("identity_kind").and_then(Value::as_str) {
                    Some("PACKAGE_COMPONENT") => {
                        capability
                            .get("package_version")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.trim().is_empty() && value.len() <= 256)
                            && capability.get("component").is_none_or(|value| {
                                value.as_str().is_some_and(|component| {
                                    !component.trim().is_empty() && component.len() <= 1024
                                })
                            })
                    }
                    Some("MCP_SKILL") => {
                        !capability.contains_key("package_version")
                            && capability
                                .get("component")
                                .and_then(Value::as_str)
                                .is_some_and(|value| {
                                    !value.trim().is_empty() && value.len() <= 2048
                                })
                    }
                    _ => false,
                }
            }
        }
        _ => false,
    };
    let digest_valid = match object.get("target_digest") {
        Some(Value::Null) => true,
        Some(Value::String(digest)) => valid_sha256(digest),
        _ => false,
    };
    let method_valid = match object.get("execution_method") {
        Some(Value::Null) => true,
        Some(Value::String(method)) => matches!(
            method.as_str(),
            "STRUCTURED_API"
                | "STRUCTURED_BROWSER"
                | "ACCESSIBILITY_BROWSER"
                | "SCREEN_COMPUTER_USE"
                | "DETERMINISTIC_LOCAL"
                | "NATIVE_AGENT_TOOL"
                | "UNKNOWN"
        ),
        _ => false,
    };
    let effect_valid = match object.get("effect_id") {
        Some(Value::Null) => true,
        Some(Value::String(effect_id)) => !effect_id.trim().is_empty() && effect_id.len() <= 256,
        _ => false,
    };
    capability_valid && digest_valid && method_valid && effect_valid
}

fn target_ref_is_safe(target: &Value) -> bool {
    fn visit(value: &Value, depth: usize) -> bool {
        if depth > MAX_APPROVAL_TARGET_DEPTH {
            return false;
        }
        match value {
            Value::Object(object) => {
                !object.is_empty()
                    && object.len() <= MAX_APPROVAL_TARGET_COLLECTION_ITEMS
                    && object.iter().all(|(key, value)| {
                        !key.trim().is_empty()
                            && key.len() <= 128
                            && !key.chars().any(char::is_control)
                            && !is_credential_field_name(key)
                            && visit(value, depth + 1)
                    })
            }
            Value::Array(values) => {
                values.len() <= MAX_APPROVAL_TARGET_COLLECTION_ITEMS
                    && values.iter().all(|value| visit(value, depth + 1))
            }
            Value::String(value) => value.len() <= 4096 && !value.chars().any(char::is_control),
            Value::Number(_) | Value::Bool(_) | Value::Null => true,
        }
    }
    visit(target, 0)
}

fn is_credential_field_name(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    [
        "secret",
        "password",
        "passwd",
        "token",
        "credential",
        "authorization",
        "privatekey",
        "apikey",
        "cookie",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

enum ReadApproval {
    Current(ApprovalRecord),
    Replay(CommittedApproval),
}

fn validate_command(command: &ResolveApprovalCommit) -> Result<(), StoreError> {
    for (name, value, max) in [
        (
            "Workspace ID",
            command.workspace_id.as_str(),
            MAX_PRINCIPAL_ID_BYTES,
        ),
        (
            "Approval ID",
            command.approval_id.as_str(),
            MAX_APPROVAL_ID_BYTES,
        ),
        (
            "principal ID",
            command.actor.principal_id.as_str(),
            MAX_PRINCIPAL_ID_BYTES,
        ),
        (
            "owner principal ID",
            command.actor.owner_principal_id.as_str(),
            MAX_PRINCIPAL_ID_BYTES,
        ),
        (
            "request ID",
            command.request_id.as_str(),
            MAX_REQUEST_ID_BYTES,
        ),
    ] {
        if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
            return Err(StoreError::Invalid(format!("{name} is malformed")));
        }
    }
    if command.actor.principal_kind != "USER"
        || command.actor.principal_id != command.actor.owner_principal_id
        || !assurance_level(&command.actor.assurance_level).is_some()
    {
        return Err(StoreError::Invalid(
            "only the authenticated Workspace owner can resolve an Approval".to_owned(),
        ));
    }
    if command.expected_version == 0
        || command.event.schema_version != 1
        || command.event.workspace_id != command.workspace_id
        || command.event.entity_type != "approval"
        || command.event.entity_id != command.approval_id
        || command.event.entity_revision != command.expected_version.saturating_add(1)
        || command.event.event_type != "approval.resolved.v1"
        || command.event.event_id.trim().is_empty()
        || command.event.origin_runtime_id.trim().is_empty()
    {
        return Err(StoreError::Invalid(
            "Approval event context does not match the requested transition".to_owned(),
        ));
    }
    if let Some(digest) = command.confirmation_digest.as_deref() {
        if !valid_sha256(digest) {
            return Err(StoreError::Invalid(
                "Approval confirmation digest is malformed".to_owned(),
            ));
        }
    }
    Ok(())
}

fn authorize_owner(
    tx: &Transaction<'_>,
    workspace_id: &str,
    owner_principal_id: &str,
) -> Result<(), StoreError> {
    let active: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
            params![workspace_id, owner_principal_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if active {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn load_approval(
    connection: &Connection,
    workspace_id: &str,
    approval_id: &str,
) -> Result<Option<ApprovalRecord>, StoreError> {
    let row = connection
        .query_row(
            "SELECT a.approval_id,a.task_id,a.requested_by_attempt,a.kind,a.action_summary,a.target_ref_json,a.scope_digest,a.action_digest,a.risk,a.required_assurance,a.status,a.requested_at,a.expires_at,a.resolved_by_json,a.resolved_at,a.version FROM approvals a JOIN tasks t ON t.task_id=a.task_id WHERE t.workspace_id=?1 AND a.approval_id=?2",
            params![workspace_id, approval_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?, row.get::<_, String>(10)?, row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?, row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<String>>(14)?, row.get::<_, i64>(15)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        id,
        task_id,
        attempt,
        kind,
        summary,
        target,
        scope,
        action,
        risk,
        assurance,
        status,
        requested,
        expires,
        resolved_by,
        resolved_at,
        version,
    )) = row
    else {
        return Ok(None);
    };
    if summary.len() > MAX_APPROVAL_TEXT_BYTES
        || !valid_sha256(&scope)
        || !valid_sha256(&action)
        || !matches!(risk.as_str(), "SAFE" | "SENSITIVE" | "HIGH_IMPACT")
        || !matches!(
            status.as_str(),
            "PENDING" | "APPROVED" | "DENIED" | "EXPIRED" | "CANCELLED"
        )
    {
        return Err(StoreError::Integrity(
            "stored Approval fields are invalid".to_owned(),
        ));
    }
    let target_ref: Value = serde_json::from_str(&target)
        .map_err(|_| StoreError::Integrity("Approval target is invalid JSON".to_owned()))?;
    if !target_ref_is_safe(&target_ref) || !approval_target_ref_matches_contract(&target_ref) {
        return Err(StoreError::Integrity(
            "stored Approval target violates the credential-free contract".to_owned(),
        ));
    }
    let resolved_by: Option<Value> = resolved_by
        .map(|value| {
            serde_json::from_str(&value).map_err(|_| {
                StoreError::Integrity("Approval resolver identity is invalid JSON".to_owned())
            })
        })
        .transpose()?;
    Ok(Some(ApprovalRecord {
        approval_id: id,
        task_id,
        requested_by_attempt: attempt,
        kind,
        action_summary: summary,
        target_ref,
        scope_digest: scope,
        action_digest: action,
        risk,
        required_assurance: assurance,
        status,
        requested_at: stored_timestamp(&requested, "Approval requested_at")?,
        expires_at: expires
            .as_deref()
            .map(|value| stored_timestamp(value, "Approval expires_at"))
            .transpose()?,
        resolved_by,
        resolved_at: resolved_at
            .as_deref()
            .map(|value| stored_timestamp(value, "Approval resolved_at"))
            .transpose()?,
        version: from_sql_i64(version, "Approval version")?,
    }))
}

fn resolve_transaction(
    connection: &mut Connection,
    command: ResolveApprovalCommit,
    next: ApprovalRecord,
    state_ref: AggregateStateRef,
) -> Result<CommittedApproval, StoreError> {
    validate_command(&command)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    if let Some(prior) = find_replay(&tx, &command)? {
        return Ok(prior);
    }
    let current = load_approval(&tx, &command.workspace_id, &command.approval_id)?
        .ok_or(StoreError::NotFound)?;
    if current.version != command.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(current.version),
        });
    }
    if current.status != "PENDING"
        || next.version != current.version.saturating_add(1)
        || next.action_digest != current.action_digest
        || next.scope_digest != current.scope_digest
        || next.task_id != current.task_id
        || next.status != command.decision.status()
        || next.resolved_at.as_deref() != Some(command.event.recorded_at.as_str())
    {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(current.version),
        });
    }
    if current
        .expires_at
        .as_deref()
        .is_some_and(|expires_at| expires_at <= command.event.recorded_at.as_str())
    {
        return Err(StoreError::Invalid(
            "expired Approval cannot be resolved".to_owned(),
        ));
    }
    if command
        .confirmation_digest
        .as_deref()
        .is_some_and(|digest| digest != current.action_digest)
    {
        return Err(StoreError::Invalid(
            "Approval confirmation does not match its exact action".to_owned(),
        ));
    }
    if !assurance_satisfies(&command.actor.assurance_level, &current.required_assurance) {
        return Err(StoreError::Invalid(
            "authenticated actor assurance is insufficient for this Approval".to_owned(),
        ));
    }
    let task_active: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1 AND workspace_id=?2 AND status NOT IN ('CANCEL_REQUESTED','CANCELLED','COMPLETED','FAILED'))",
        params![current.task_id, command.workspace_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !task_active {
        return Err(StoreError::Invalid(
            "Approval Task is no longer active".to_owned(),
        ));
    }
    let resolved_by =
        json!({"principal_id":command.actor.principal_id,"kind":command.actor.principal_kind});
    let expected_payload = json!({
        "approval_id": current.approval_id,
        "task_id": current.task_id,
        "action_digest": current.action_digest,
        "required_assurance": current.required_assurance,
        "from": "PENDING",
        "to": next.status,
        "resolved_by": resolved_by,
    });
    let mut event = command.event.clone();
    event.payload = expected_payload;
    event.entity_revision = next.version;
    let updated = tx.execute(
        "UPDATE approvals SET status=?1,resolved_by_json=?2,resolved_at=?3,version=?4 WHERE approval_id=?5 AND status='PENDING' AND version=?6",
        params![next.status, encode(&resolved_by)?, command.event.recorded_at, to_sql_i64(next.version, "Approval version")?, command.approval_id, to_sql_i64(command.expected_version, "Approval version")?],
    ).map_err(map_database_error)?;
    if updated != 1 {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: None,
        });
    }
    let committed_event = insert_domain_event(&tx, &event, &state_ref)?;
    tx.execute(
        "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at) VALUES (?1,'approval',?2,?3,1,?4,?5,?6)",
        params![command.workspace_id, command.approval_id, to_sql_i64(next.version, "Approval version")?, state_ref.blob.digest, encode(&state_ref.blob)?, command.event.recorded_at],
    ).map_err(map_database_error)?;
    let committed = CommittedApproval {
        approval: next,
        event: committed_event,
        replayed: false,
    };
    let response = encode(&committed)?;
    let request_digest = resolve_request_digest(&command)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![command.actor.principal_id, command.request_id, request_digest, response, digest(response.as_bytes()), command.event.recorded_at],
    ).map_err(map_database_error)?;
    let decision = if command.decision == ApprovalDecision::Approve {
        "ALLOW"
    } else {
        "DENY"
    };
    let audit_digest = digest(&canonical_json(&json!({
        "approval_id":current.approval_id,
        "action_digest":current.action_digest,
        "decision":command.decision,
        "resolved_by":resolved_by,
    }))?);
    tx.execute(
        "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest) VALUES (?1,?2,?3,'APPROVAL_RESOLVED',?4,?5,?6,?7,?8,?9)",
        params![format!("{}:audit", command.event.event_id), command.workspace_id, encode(&resolved_by)?, encode(&current.target_ref)?, decision, if decision == "ALLOW" { "OWNER_APPROVED" } else { "OWNER_DENIED" }, command.event.correlation_id, command.event.recorded_at, audit_digest],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn request_transaction(
    connection: &mut Connection,
    command: RequestApprovalCommit,
    state_ref: AggregateStateRef,
) -> Result<CommittedApproval, StoreError> {
    validate_request_command(&command)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    if let Some(prior) = find_request_replay(&tx, &command)? {
        return Ok(prior);
    }
    let task_status: Option<String> = tx
        .query_row(
            "SELECT status FROM tasks WHERE task_id=?1 AND workspace_id=?2",
            params![command.approval.task_id, command.workspace_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    if !task_status.is_some_and(|status| {
        matches!(
            status.as_str(),
            "OPEN" | "RUNNING" | "WAITING_USER" | "WAITING_DEPENDENCY"
        )
    }) {
        return Err(StoreError::Invalid(
            "Approval must be requested for an active Task in its Workspace".to_owned(),
        ));
    }
    if let Some(attempt_id) = command.approval.requested_by_attempt.as_deref() {
        let active_attempt: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM attempts WHERE task_id=?1 AND attempt_id=?2 AND status IN ('CREATED','PREPARING','RUNNING','WAITING_APPROVAL','WAITING_RESOURCE','CHECKPOINTING'))",
                params![command.approval.task_id, attempt_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_database_error)?
            .unwrap_or(false);
        if !active_attempt {
            return Err(StoreError::Invalid(
                "Approval Attempt must belong to the active Task".to_owned(),
            ));
        }
    }
    let existing: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM approvals WHERE approval_id=?1)",
            params![command.approval.approval_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if existing {
        return Err(StoreError::Conflict {
            expected: None,
            actual: Some(1),
        });
    }

    let approval = &command.approval;
    tx.execute(
        "INSERT INTO approvals(approval_id,task_id,requested_by_attempt,kind,action_summary,target_ref_json,scope_digest,action_digest,risk,required_assurance,status,requested_at,expires_at,resolved_by_json,resolved_at,version) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'PENDING',?11,?12,NULL,NULL,1)",
        params![approval.approval_id, approval.task_id, approval.requested_by_attempt, approval.kind, approval.action_summary, encode(&approval.target_ref)?, approval.scope_digest, approval.action_digest, approval.risk, approval.required_assurance, approval.requested_at, approval.expires_at],
    )
    .map_err(map_database_error)?;

    let mut event = command.event.clone();
    event.payload = json!({
        "approval_id": approval.approval_id,
        "task_id": approval.task_id,
        "action_digest": approval.action_digest,
        "required_assurance": approval.required_assurance,
        "to": "PENDING",
    });
    let committed_event = insert_domain_event(&tx, &event, &state_ref)?;
    tx.execute(
        "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at) VALUES (?1,'approval',?2,1,1,?3,?4,?5)",
        params![command.workspace_id, approval.approval_id, state_ref.blob.digest, encode(&state_ref.blob)?, command.event.recorded_at],
    )
    .map_err(map_database_error)?;
    let committed = CommittedApproval {
        approval: approval.clone(),
        event: committed_event,
        replayed: false,
    };
    let response = encode(&committed)?;
    let request_digest = request_approval_digest(&command)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![command.actor.principal_id, command.request_id, request_digest, response, digest(response.as_bytes()), command.event.recorded_at],
    )
    .map_err(map_database_error)?;
    let principal = json!({
        "principal_id": command.actor.principal_id,
        "kind": command.actor.principal_kind,
    });
    let audit_digest = digest(&canonical_json(&json!({
        "approval_id": approval.approval_id,
        "task_id": approval.task_id,
        "attempt_id": approval.requested_by_attempt,
        "action_digest": approval.action_digest,
        "scope_digest": approval.scope_digest,
    }))?);
    tx.execute(
        "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest) VALUES (?1,?2,?3,'APPROVAL_REQUESTED',?4,'REQUIRE_APPROVAL','ACTION_REQUIRES_OWNER_APPROVAL',?5,?6,?7)",
        params![format!("{}:audit", command.event.event_id), command.workspace_id, encode(&principal)?, encode(&approval.target_ref)?, command.event.correlation_id, command.event.recorded_at, audit_digest],
    )
    .map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn find_replay(
    tx: &Transaction<'_>,
    command: &ResolveApprovalCommit,
) -> Result<Option<CommittedApproval>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest,response_json,response_digest FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            params![command.actor.principal_id, command.request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((stored_request_digest, response, response_digest)) = prior else {
        return Ok(None);
    };
    let request_digest = resolve_request_digest(command)?;
    if stored_request_digest != request_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response.ok_or_else(|| {
        StoreError::Integrity("Approval idempotency receipt has no response".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "Approval idempotency response digest does not match".to_owned(),
        ));
    }
    let mut committed: CommittedApproval = serde_json::from_str(&response).map_err(|_| {
        StoreError::Integrity("Approval idempotency response is invalid".to_owned())
    })?;
    committed.replayed = true;
    Ok(Some(committed))
}

fn find_request_replay(
    tx: &Transaction<'_>,
    command: &RequestApprovalCommit,
) -> Result<Option<CommittedApproval>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest,response_json,response_digest FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            params![command.actor.principal_id, command.request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((stored_request_digest, response, response_digest)) = prior else {
        return Ok(None);
    };
    if stored_request_digest != request_approval_digest(command)? {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response.ok_or_else(|| {
        StoreError::Integrity("Approval request receipt has no response".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "Approval request response digest does not match".to_owned(),
        ));
    }
    let mut committed: CommittedApproval = serde_json::from_str(&response)
        .map_err(|_| StoreError::Integrity("Approval request receipt is invalid".to_owned()))?;
    committed.replayed = true;
    Ok(Some(committed))
}

fn request_approval_digest(command: &RequestApprovalCommit) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "workspace_id": command.workspace_id,
        // Approval identity, event identity, and timestamps are generated by
        // Core while handling the request. A retry may regenerate them after a
        // lost response; deduplicate on the stable requested action instead and
        // return the original committed receipt.
        "intent": {
            "task_id": command.approval.task_id,
            "requested_by_attempt": command.approval.requested_by_attempt,
            "kind": command.approval.kind,
            "action_summary": command.approval.action_summary,
            "target_ref": command.approval.target_ref,
            "scope_digest": command.approval.scope_digest,
            "action_digest": command.approval.action_digest,
            "risk": command.approval.risk,
            "required_assurance": command.approval.required_assurance,
        },
    }))?))
}

fn resolve_request_digest(command: &ResolveApprovalCommit) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "workspace_id":command.workspace_id,
        "approval_id":command.approval_id,
        "expected_version":command.expected_version,
        "decision":command.decision,
        "confirmation_digest":command.confirmation_digest,
    }))?))
}

fn put_approval_state(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    approval: &ApprovalRecord,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(approval)?;
    let blob = store.inner.blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        "application/vnd.litecow.approval+json",
    )?;
    if blob.digest != digest(&bytes)
        || blob.size_bytes != bytes.len() as u64
        || store
            .inner
            .blobs
            .get(workspace_id, BlobPurpose::AggregateState, &blob)?
            != bytes
    {
        return Err(StoreError::Integrity(
            "Approval aggregate-state blob failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: approval.version,
        record_schema_version: 1,
    })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn stored_timestamp(value: &str, field: &str) -> Result<String, StoreError> {
    canonicalize_utc_timestamp(value)
        .map_err(|_| StoreError::Integrity(format!("stored {field} timestamp is invalid")))
}

fn assurance_level(value: &str) -> Option<u8> {
    match value {
        "VIEW_ONLY" => Some(0),
        "STEER_SAFE" => Some(1),
        "APPROVE_SAFE" => Some(2),
        "APPROVE_SENSITIVE" => Some(3),
        "LOCAL_STRONG" => Some(4),
        _ => None,
    }
}

fn assurance_satisfies(actor: &str, required: &str) -> bool {
    assurance_level(actor)
        .zip(assurance_level(required))
        .is_some_and(|(actor, required)| actor >= required)
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn encode<T: Serialize>(value: &T) -> Result<String, StoreError> {
    String::from_utf8(canonical_json(value)?)
        .map_err(|error| StoreError::Integrity(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use std::collections::HashMap;
    use storage_core::{ApprovalActor, CapabilityGrantStore};
    use zeroize::Zeroizing;

    fn target_ref() -> Value {
        json!({
            "capability_ref": {
                "capability_id": "mail.send",
                "identity_kind": "PACKAGE_COMPONENT",
                "source": "test:litespm",
                "package_version": "1.0.0",
                "digest": DIGEST,
            },
            "target_digest": DIGEST,
            "execution_method": "STRUCTURED_API",
            "effect_id": "effect-1",
        })
    }

    #[derive(Default)]
    struct MemorySecretCredentialBackend {
        values: Mutex<HashMap<String, Zeroizing<String>>>,
        fail_after_write: bool,
    }

    #[test]
    fn secret_kind_and_placement_serde_are_closed_contract_values() {
        assert_eq!(
            serde_json::to_string(&SecretKind::ApiKey).unwrap(),
            "\"API_KEY\""
        );
        assert_eq!(
            serde_json::to_string(&SecretPlacement::LocalOnly).unwrap(),
            "\"LOCAL_ONLY\""
        );
        assert!(serde_json::from_str::<SecretKind>("\"ARBITRARY\"").is_err());
        assert!(serde_json::from_str::<SecretPlacement>("\"ARBITRARY\"").is_err());
    }

    impl SecretCredentialBackend for MemorySecretCredentialBackend {
        fn set_secret(&self, account: &str, secret: &str) -> Result<(), StoreError> {
            self.values
                .lock()
                .expect("secret test backend lock")
                .insert(account.to_owned(), Zeroizing::new(secret.to_owned()));
            if self.fail_after_write {
                Err(StoreError::Blob(
                    "simulated keychain write ambiguity".to_owned(),
                ))
            } else {
                Ok(())
            }
        }

        fn remove_secret(&self, account: &str) -> Result<(), StoreError> {
            self.values
                .lock()
                .expect("secret test backend lock")
                .remove(account);
            Ok(())
        }
    }

    #[test]
    fn secret_write_returns_only_opaque_local_ref_and_zeroizes_input() {
        let backend = MemorySecretCredentialBackend::default();
        let mut secret = b"super-secret-provider-token".to_vec();

        let secret_ref = write_secret_with(
            &backend,
            "workspace-secret-test",
            SecretKind::Token,
            &mut secret,
        )
        .expect("fake OS SecretStore write succeeds");

        assert!(secret.iter().all(|byte| *byte == 0));
        assert_eq!(secret_ref.placement, SecretPlacement::LocalOnly);
        assert!(secret_ref.provider_ref.starts_with("os-keyring:v1:"));
        assert!(!secret_ref.provider_ref.contains("workspace-secret-test"));
        assert!(
            !secret_ref
                .provider_ref
                .contains("super-secret-provider-token")
        );
        let serialized = serde_json::to_string(&secret_ref).expect("serialize opaque ref");
        assert!(!serialized.contains("super-secret-provider-token"));
        assert!(!serialized.contains("c3VwZXItc2VjcmV0"));

        let values = backend.values.lock().expect("secret test backend lock");
        assert_eq!(values.len(), 1);
        let (account, encoded_secret) = values.iter().next().unwrap();
        assert!(account.starts_with("v1:"));
        assert!(!account.contains("workspace-secret-test"));
        let decoded = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(encoded_secret.as_bytes())
                .expect("OS credential value is encoded bytes"),
        );
        assert_eq!(decoded.as_slice(), b"super-secret-provider-token");
        // This adapter has no SQLite, event, or logging write in its contract;
        // only the test keychain backend receives the encoded secret.
    }

    #[test]
    fn secret_write_failure_cleans_up_ambiguous_keychain_entry_and_zeroizes() {
        let backend = MemorySecretCredentialBackend {
            values: Mutex::new(HashMap::new()),
            fail_after_write: true,
        };
        let mut secret = b"failed-write-secret".to_vec();

        assert!(matches!(
            write_secret_with(
                &backend,
                "workspace-secret-test",
                SecretKind::ApiKey,
                &mut secret,
            ),
            Err(StoreError::Blob(_))
        ));
        assert!(secret.iter().all(|byte| *byte == 0));
        assert!(
            backend
                .values
                .lock()
                .expect("secret test backend lock")
                .is_empty()
        );
    }

    #[test]
    fn secret_write_rejects_empty_or_oversized_input_before_keychain_access() {
        let backend = MemorySecretCredentialBackend::default();
        let mut empty = Vec::new();
        assert!(matches!(
            write_secret_with(&backend, "workspace", SecretKind::Password, &mut empty),
            Err(StoreError::Invalid(_))
        ));
        let mut oversized = vec![b'x'; MAX_SECRET_BYTES + 1];
        assert!(matches!(
            write_secret_with(&backend, "workspace", SecretKind::Other, &mut oversized,),
            Err(StoreError::Invalid(_))
        ));
        assert!(empty.is_empty());
        assert!(oversized.iter().all(|byte| *byte == 0));
        assert!(
            backend
                .values
                .lock()
                .expect("secret test backend lock")
                .is_empty()
        );
    }

    #[test]
    fn approval_target_ref_accepts_only_the_canonical_typed_shape() {
        let canonical = target_ref();
        assert!(approval_target_ref_matches_contract(&canonical));

        let mut unknown_field = canonical.clone();
        unknown_field["display_name"] = Value::String("Mail".into());
        assert!(!approval_target_ref_matches_contract(&unknown_field));

        let mut invalid_method = canonical.clone();
        invalid_method["execution_method"] = Value::String("NATIVE_MAGIC".into());
        assert!(!approval_target_ref_matches_contract(&invalid_method));

        let mut invalid_package_ref = canonical;
        invalid_package_ref["capability_ref"]["identity_kind"] =
            Value::String("ARBITRARY_PACKAGE".into());
        assert!(!approval_target_ref_matches_contract(&invalid_package_ref));
    }

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn secret_lease_issue_command() -> IssueSecretLeaseCommit {
        let now = canonicalize_utc_timestamp(
            &time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .expect("format SecretLease test time"),
        )
        .expect("canonicalize SecretLease test time");
        let secret_ref = SecretRef {
            secret_ref_id: "secret-ref-1".into(),
            provider_ref: "os-keyring:v1:opaque".into(),
            placement: SecretPlacement::LocalOnly,
        };
        let lease = SecretLeaseRecord {
            secret_lease_id: "lease-1".into(),
            secret_ref,
            task_id: "task-1".into(),
            attempt_id: "attempt-1".into(),
            capability_ref: json!({"capability_id":"mail.send","digest":DIGEST}),
            runtime_id: "runtime-1".into(),
            allowed_usage: vec!["mail.send".into()],
            status: "ACTIVE".into(),
            issued_at: now.clone(),
            expires_at: "2099-01-01T00:00:00Z".into(),
            version: 1,
        };
        IssueSecretLeaseCommit {
            workspace_id: "ws-1".into(),
            actor: ApprovalActor {
                principal_id: "owner-1".into(),
                principal_kind: "USER".into(),
                owner_principal_id: "owner-1".into(),
                assurance_level: "APPROVE_SENSITIVE".into(),
            },
            request_id: "issue-lease-1".into(),
            event: EventDraft {
                event_id: "event-lease-1".into(),
                workspace_id: "ws-1".into(),
                entity_type: "secret_lease".into(),
                entity_id: "lease-1".into(),
                origin_runtime_id: "runtime-1".into(),
                entity_revision: 1,
                hlc_timestamp: format!("{now}#0@runtime-1"),
                correlation_id: "corr-lease-1".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "secret.lease.issued.v1".into(),
                payload: Value::Null,
                recorded_at: now,
            },
            lease,
        }
    }

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(
            "CREATE TABLE workspaces(workspace_id TEXT PRIMARY KEY, owner_principal_id TEXT NOT NULL, status TEXT NOT NULL);
             CREATE TABLE conversations(conversation_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL);
             CREATE TABLE tasks(task_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, status TEXT NOT NULL);
             CREATE TABLE resources(resource_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, current_revision_id TEXT, context_document_json TEXT);
             CREATE TABLE resource_revisions(resource_revision_id TEXT PRIMARY KEY, resource_id TEXT NOT NULL);
             CREATE TABLE attempts(attempt_id TEXT PRIMARY KEY, task_id TEXT NOT NULL, status TEXT NOT NULL, runtime_id TEXT NOT NULL DEFAULT 'runtime-1', runtime_incarnation_id TEXT NOT NULL DEFAULT 'inc-1', capability_grant_ids_json TEXT NOT NULL DEFAULT '[]');
             CREATE TABLE runtimes(runtime_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, availability TEXT NOT NULL, current_incarnation_id TEXT);
             CREATE TABLE runtime_incarnations(runtime_id TEXT NOT NULL, runtime_incarnation_id TEXT NOT NULL, recovery_state TEXT NOT NULL, PRIMARY KEY(runtime_id,runtime_incarnation_id));
             CREATE TABLE capability_grants(capability_grant_id TEXT PRIMARY KEY, scope_kind TEXT NOT NULL, status TEXT NOT NULL, conversation_id TEXT, task_id TEXT, attempt_id TEXT, capability_ref_json TEXT NOT NULL, allowed_operations_json TEXT NOT NULL, resource_scope_json TEXT NOT NULL DEFAULT '{}', secret_refs_json TEXT NOT NULL, granted_by_json TEXT NOT NULL DEFAULT '{}', expires_at TEXT, created_at TEXT NOT NULL DEFAULT '2026-10-17T10:00:00Z', version INTEGER NOT NULL DEFAULT 1);
             CREATE TABLE secret_leases(secret_lease_id TEXT PRIMARY KEY, secret_ref_json TEXT NOT NULL, task_id TEXT NOT NULL, attempt_id TEXT, capability_ref_json TEXT, runtime_id TEXT NOT NULL, allowed_usage_json TEXT NOT NULL, status TEXT NOT NULL, issued_at TEXT NOT NULL, expires_at TEXT NOT NULL, version INTEGER NOT NULL);
             CREATE TABLE execution_leases(lease_id TEXT PRIMARY KEY, task_id TEXT NOT NULL, attempt_id TEXT NOT NULL, runtime_id TEXT NOT NULL, runtime_incarnation_id TEXT NOT NULL, state TEXT NOT NULL, expires_at TEXT NOT NULL);
             CREATE TABLE approvals(approval_id TEXT PRIMARY KEY, task_id TEXT NOT NULL, requested_by_attempt TEXT, kind TEXT NOT NULL, action_summary TEXT NOT NULL, target_ref_json TEXT NOT NULL, scope_digest TEXT NOT NULL, action_digest TEXT NOT NULL, risk TEXT NOT NULL, required_assurance TEXT NOT NULL, status TEXT NOT NULL, requested_at TEXT NOT NULL, expires_at TEXT, resolved_by_json TEXT, resolved_at TEXT, version INTEGER NOT NULL);
             CREATE TABLE workspace_origin_sequences(workspace_id TEXT NOT NULL, origin_runtime_id TEXT NOT NULL, last_sequence INTEGER NOT NULL, PRIMARY KEY(workspace_id,origin_runtime_id));
             CREATE TABLE domain_events(event_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id TEXT NOT NULL, origin_runtime_id TEXT NOT NULL, origin_sequence INTEGER NOT NULL, entity_revision INTEGER NOT NULL, hlc_timestamp TEXT NOT NULL, correlation_id TEXT NOT NULL, causation_id TEXT, schema_version INTEGER NOT NULL, type TEXT NOT NULL, payload_json TEXT NOT NULL, aggregate_state_ref_json TEXT NOT NULL, recorded_at TEXT NOT NULL, payload_digest TEXT NOT NULL);
             CREATE TABLE aggregate_snapshots(workspace_id TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id TEXT NOT NULL, through_revision INTEGER NOT NULL, projection_schema_version INTEGER NOT NULL, state_digest TEXT NOT NULL, blob_ref_json TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(workspace_id,entity_type,entity_id,through_revision));
             CREATE TABLE request_dedup(principal_id TEXT NOT NULL, request_id TEXT NOT NULL, request_digest TEXT NOT NULL, response_json TEXT, response_digest TEXT, created_at TEXT NOT NULL, expires_at TEXT, PRIMARY KEY(principal_id,request_id));
             CREATE TABLE audit_records(audit_record_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, principal_json TEXT NOT NULL, action TEXT NOT NULL, resource_ref_json TEXT, decision TEXT NOT NULL, reason_code TEXT NOT NULL, correlation_id TEXT NOT NULL, occurred_at TEXT NOT NULL, payload_digest TEXT);
             INSERT INTO runtimes VALUES ('runtime-1','ws-1','ONLINE','inc-1');
             INSERT INTO runtime_incarnations VALUES ('runtime-1','inc-1','READY');
             INSERT INTO workspaces VALUES ('ws-1','owner-1','ACTIVE');
             INSERT INTO tasks VALUES ('task-1','ws-1','WAITING_USER');
             INSERT INTO resources VALUES ('resource-1','ws-1','revision-1',NULL);
             INSERT INTO resource_revisions VALUES ('revision-1','resource-1');
             INSERT INTO resources VALUES ('resource-other','ws-other','revision-other',NULL);
             INSERT INTO resource_revisions VALUES ('revision-other','resource-other');
             INSERT INTO attempts(attempt_id,task_id,status) VALUES ('attempt-1','task-1','WAITING_APPROVAL');
             INSERT INTO approvals VALUES ('approval-1','task-1',NULL,'CAPABILITY_ESCALATION','Send this exact message','{\"capability_ref\":{\"capability_id\":\"mail.send\",\"identity_kind\":\"PACKAGE_COMPONENT\",\"source\":\"test:litespm\",\"package_version\":\"1.0.0\",\"digest\":\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"},\"target_digest\":\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"execution_method\":\"STRUCTURED_API\",\"effect_id\":\"effect-1\"}',
               'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
               'SENSITIVE','APPROVE_SENSITIVE','PENDING','2026-10-17T10:00:00Z',NULL,NULL,NULL,1);",
        ).unwrap();
        connection
    }

    #[test]
    fn secret_lease_issue_fails_closed_without_exact_active_attempt_grant() {
        let mut connection = connection();
        let command = secret_lease_issue_command();
        validate_secret_lease_issue(&command).expect("well-formed lease request");
        let state = AggregateStateRef {
            blob: BlobRef {
                digest: DIGEST.into(),
                size_bytes: 1,
                media_type: "application/vnd.litecow.secret-lease+json".into(),
            },
            entity_revision: 1,
            record_schema_version: 1,
        };
        assert!(matches!(
            issue_secret_lease_transaction(&mut connection, command, state),
            Err(StoreError::Invalid(_))
        ));
        let leases: i64 = connection
            .query_row("SELECT COUNT(*) FROM secret_leases", [], |row| row.get(0))
            .unwrap();
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(leases, 0);
        assert_eq!(events, 0);
    }

    #[test]
    fn secret_lease_issue_requires_sensitive_assurance_not_an_owner_label() {
        let mut low_assurance = secret_lease_issue_command();
        low_assurance.actor.assurance_level = "VIEW_ONLY".into();
        assert!(matches!(
            validate_secret_lease_issue(&low_assurance),
            Err(StoreError::Invalid(_))
        ));

        let mut forged_assurance = secret_lease_issue_command();
        forged_assurance.actor.assurance_level = "OWNER_APPROVED".into();
        assert!(matches!(
            validate_secret_lease_issue(&forged_assurance),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn secret_lease_revoke_rejects_unrecognized_assurance_labels() {
        let now = "2026-10-17T12:00:00Z";
        let mut command = RevokeSecretLeaseCommit {
            workspace_id: "ws-1".into(),
            actor: ApprovalActor {
                principal_id: "owner-1".into(),
                principal_kind: "USER".into(),
                owner_principal_id: "owner-1".into(),
                assurance_level: "OWNER_APPROVED".into(),
            },
            request_id: "revoke-lease-invalid-assurance".into(),
            secret_lease_id: "lease-1".into(),
            expected_version: 1,
            event: EventDraft {
                event_id: "event-lease-revoked-invalid-assurance".into(),
                workspace_id: "ws-1".into(),
                entity_type: "secret_lease".into(),
                entity_id: "lease-1".into(),
                origin_runtime_id: "runtime-1".into(),
                entity_revision: 2,
                hlc_timestamp: format!("{now}#0@runtime-1"),
                correlation_id: "corr-lease-revoked-invalid-assurance".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "secret.lease.revoked.v1".into(),
                payload: Value::Null,
                recorded_at: now.into(),
            },
        };
        assert!(matches!(
            validate_secret_lease_revoke(&command),
            Err(StoreError::Invalid(_))
        ));

        command.actor.assurance_level = "VIEW_ONLY".into();
        assert!(validate_secret_lease_revoke(&command).is_ok());
    }

    fn capability_grant_issue_command() -> IssueCapabilityGrantCommit {
        let created_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format grant time");
        let expires_at = time::OffsetDateTime::parse(
            &created_at,
            &time::format_description::well_known::Rfc3339,
        )
        .expect("parse grant time")
        .checked_add(time::Duration::minutes(10))
        .expect("grant expiry within range")
        .format(&time::format_description::well_known::Rfc3339)
        .expect("format grant expiry");
        IssueCapabilityGrantCommit {
            workspace_id: "ws-1".into(),
            actor: ApprovalActor {
                principal_id: "owner-1".into(),
                principal_kind: "USER".into(),
                owner_principal_id: "owner-1".into(),
                assurance_level: "APPROVE_SAFE".into(),
            },
            request_id: "issue-grant-1".into(),
            approval_id: None,
            grant: CapabilityGrantRecord {
                capability_grant_id: "grant-new".into(),
                scope: CapabilityGrantScope::TaskPlanning {
                    task_id: "task-1".into(),
                },
                capability_ref: json!({
                    "capability_id":"resource.read",
                    "identity_kind":"PACKAGE_COMPONENT",
                    "source":"test:litespm",
                    "package_version":"1.0.0",
                    "digest":DIGEST
                }),
                allowed_operations: vec!["workspace.resource.read".into()],
                resource_scope: json!({
                    "resource_refs":[{"workspace_id":"ws-1","resource_id":"resource-1","revision_id":"revision-1"}],
                    "operation_ids":["workspace.resource.read"],
                    "constraints":{}
                }),
                secret_refs: Vec::new(),
                granted_by: json!({"principal_id":"owner-1","kind":"USER"}),
                expires_at: Some(expires_at),
                status: CapabilityGrantStatus::Active,
                created_at: created_at.clone(),
                version: 1,
            },
            event: EventDraft {
                event_id: "event-grant-created-1".into(),
                workspace_id: "ws-1".into(),
                entity_type: "capability_grant".into(),
                entity_id: "grant-new".into(),
                origin_runtime_id: "runtime-1".into(),
                entity_revision: 1,
                hlc_timestamp: format!("{created_at}#0@runtime-1"),
                correlation_id: "corr-grant-created-1".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "capability.grant.created.v1".into(),
                payload: Value::Null,
                recorded_at: created_at,
            },
        }
    }

    fn grant_store_fixture() -> (tempfile::TempDir, SqliteWorkspaceStore) {
        let directory = tempfile::tempdir().expect("temporary Trust store directory");
        let database_path = directory.path().join("state").join("state.sqlite3");
        let blobs = std::sync::Arc::new(crate::FileBlobStore::new(
            directory.path().join("blobs"),
            GrantRaceKeys,
        ));
        let store = SqliteWorkspaceStore::open(
            &database_path,
            blobs,
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: std::time::Duration::from_secs(2),
            },
        )
        .expect("SQLite Trust store opens");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: "ws-1".into(),
                name: "Trust replay workspace".into(),
                owner_principal_id: "owner-1".into(),
                event: EventContext {
                    event_id: "workspace-created".into(),
                    origin_runtime_id: "runtime-1".into(),
                    hlc_timestamp: "2026-10-17T10:00:00Z#0@runtime-1".into(),
                    correlation_id: "workspace-created-correlation".into(),
                    causation_id: None,
                    recorded_at: "2026-10-17T10:00:00Z".into(),
                },
            })
            .expect("owner Workspace exists");
        let fixture = Connection::open(&database_path).expect("open Task fixture connection");
        fixture
            .execute_batch(
                "PRAGMA foreign_keys=OFF;
                 INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version)
                 VALUES ('task-1','ws-1',1,'RUNNING','fixture-lead','NORMAL','{}','2026-10-17T10:00:00Z','2026-10-17T10:00:00Z',1);
                 INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,provenance_json,created_at,updated_at,version)
                 VALUES ('resource-1','ws-1','FILE','{}','Grant test resource','revision-1','PERSONAL','{}','2026-10-17T10:00:00Z','2026-10-17T10:00:00Z',1);
                 INSERT INTO resource_revisions(resource_revision_id,resource_id,observed_at,created_by_json)
                 VALUES ('revision-1','resource-1','2026-10-17T10:00:00Z','{}');
                 PRAGMA foreign_keys=ON;",
            )
            .expect("insert minimal Task and Resource fixtures");
        drop(fixture);
        (directory, store)
    }

    fn unpinned_resource_grant_command() -> IssueCapabilityGrantCommit {
        let mut command = capability_grant_issue_command();
        command.grant.resource_scope["resource_refs"] = json!([{
            "workspace_id":"ws-1",
            "resource_id":"resource-1",
        }]);
        command
    }

    #[test]
    fn exact_grant_replay_survives_resource_head_advance_and_keeps_original_pin() {
        let (directory, store) = grant_store_fixture();
        let approval_store = SqliteApprovalStore::new(store);
        let mut command = unpinned_resource_grant_command();
        let committed = approval_store
            .issue_capability_grant(command.clone())
            .expect("first request resolves and pins current Resource revision");
        assert_eq!(
            committed.grant.resource_scope["resource_refs"][0]["revision_id"],
            "revision-1"
        );

        let connection = Connection::open(directory.path().join("state/state.sqlite3"))
            .expect("open database for head advancement");
        connection
            .execute_batch(
                "INSERT INTO resource_revisions(resource_revision_id,resource_id,observed_at,created_by_json)
                 VALUES ('revision-2','resource-1','2026-10-18T10:00:00Z','{}');
                 UPDATE resources SET current_revision_id='revision-2' WHERE resource_id='resource-1';",
            )
            .expect("advance Resource current head");
        command.grant.capability_grant_id = "retry-grant-generated-new-id".into();
        command.event.event_id = "retry-event-generated-new-id".into();
        command.event.entity_id = "retry-grant-generated-new-id".into();
        let replay = approval_store
            .issue_capability_grant(command)
            .expect("exact replay is returned before resolving the new current head");
        assert!(replay.replayed);
        assert_eq!(replay.grant, committed.grant);
        assert_eq!(replay.event, committed.event);
    }

    #[test]
    fn exact_grant_replay_survives_scoped_resource_deletion() {
        let (directory, store) = grant_store_fixture();
        let approval_store = SqliteApprovalStore::new(store);
        let mut command = unpinned_resource_grant_command();
        let committed = approval_store
            .issue_capability_grant(command.clone())
            .expect("first request resolves and pins Resource revision");

        let connection = Connection::open(directory.path().join("state/state.sqlite3"))
            .expect("open database for Resource deletion");
        connection
            .execute_batch(
                "PRAGMA foreign_keys=OFF;
                 DELETE FROM resource_revisions WHERE resource_id='resource-1';
                 DELETE FROM resources WHERE resource_id='resource-1';",
            )
            .expect("delete scoped Resource after grant commit");
        command.grant.capability_grant_id = "retry-grant-generated-new-id".into();
        command.event.event_id = "retry-event-generated-new-id".into();
        command.event.entity_id = "retry-grant-generated-new-id".into();
        let replay = approval_store
            .issue_capability_grant(command)
            .expect("exact replay is returned before mutable Resource existence checks");
        assert!(replay.replayed);
        assert_eq!(replay.grant, committed.grant);
        assert_eq!(replay.event, committed.event);
    }

    fn capability_grant_state() -> AggregateStateRef {
        AggregateStateRef {
            blob: BlobRef {
                digest: DIGEST.into(),
                size_bytes: 1,
                media_type: "application/vnd.litecow.capability-grant+json".into(),
            },
            entity_revision: 1,
            record_schema_version: 1,
        }
    }

    #[derive(Clone, Copy)]
    struct GrantRaceKeys;

    impl crate::WorkspaceBlobKeyProvider for GrantRaceKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<crate::WorkspaceBlobKey, StoreError> {
            Ok(crate::WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([41_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<crate::WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob(
                    "unknown grant-race test key version".into(),
                ));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    struct GrantRaceBlobs {
        inner: crate::FileBlobStore<GrantRaceKeys>,
        concurrent_grant_state_writes: std::sync::Arc<std::sync::Barrier>,
    }

    impl BlobStore for GrantRaceBlobs {
        fn put(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            bytes: &[u8],
            media_type: &str,
        ) -> Result<storage_core::BlobRef, StoreError> {
            if media_type == "application/vnd.litecow.capability-grant+json" {
                self.concurrent_grant_state_writes.wait();
            }
            self.inner.put(workspace_id, purpose, bytes, media_type)
        }

        fn get(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            blob: &storage_core::BlobRef,
        ) -> Result<Vec<u8>, StoreError> {
            self.inner.get(workspace_id, purpose, blob)
        }

        fn remove(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            blob: &storage_core::BlobRef,
        ) -> Result<(), StoreError> {
            self.inner.remove(workspace_id, purpose, blob)
        }
    }

    #[test]
    fn concurrent_exact_grant_issue_returns_one_durable_receipt_and_event() {
        use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
        use storage_core::CapabilityGrantStore;

        let directory = tempfile::tempdir().expect("temporary Trust store directory");
        let grant_state_barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let blobs = std::sync::Arc::new(GrantRaceBlobs {
            inner: crate::FileBlobStore::new(directory.path().join("blobs"), GrantRaceKeys),
            concurrent_grant_state_writes: std::sync::Arc::clone(&grant_state_barrier),
        });
        let database_path = directory.path().join("state").join("state.sqlite3");
        let store = SqliteWorkspaceStore::open(
            &database_path,
            blobs,
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: std::time::Duration::from_secs(2),
            },
        )
        .expect("SQLite Trust store opens");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: "ws-1".into(),
                name: "Trust race workspace".into(),
                owner_principal_id: "owner-1".into(),
                event: EventContext {
                    event_id: "workspace-created".into(),
                    origin_runtime_id: "runtime-1".into(),
                    hlc_timestamp: "2026-10-17T10:00:00Z#0@runtime-1".into(),
                    correlation_id: "workspace-created-correlation".into(),
                    causation_id: None,
                    recorded_at: "2026-10-17T10:00:00Z".into(),
                },
            })
            .expect("owner Workspace exists");

        // Scope foreign keys are deliberately disabled only for this minimal Task
        // fixture; the production grant transaction still checks its Workspace,
        // status and exact Task scope under its own immediate transaction.
        let fixture = Connection::open(&database_path).expect("open Task fixture connection");
        fixture
            .execute_batch(
                "PRAGMA foreign_keys=OFF;
                 INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version)
                 VALUES ('task-1','ws-1',1,'RUNNING','fixture-lead','NORMAL','{}','2026-10-17T10:00:00Z','2026-10-17T10:00:00Z',1);
                 INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,provenance_json,created_at,updated_at,version)
                 VALUES ('resource-1','ws-1','FILE','{}','Grant test resource','revision-1','PERSONAL','{}','2026-10-17T10:00:00Z','2026-10-17T10:00:00Z',1);
                 INSERT INTO resource_revisions(resource_revision_id,resource_id,observed_at,created_by_json)
                 VALUES ('revision-1','resource-1','2026-10-17T10:00:00Z','{}');
                 PRAGMA foreign_keys=ON;",
            )
            .expect("insert minimal active Task fixture");
        drop(fixture);

        let approval_store = SqliteApprovalStore::new(store.clone());
        let left_store = approval_store.clone();
        let right_store = approval_store;
        let left_command = capability_grant_issue_command();
        let mut right_command = left_command.clone();
        right_command.grant.capability_grant_id = "grant-regenerated".into();
        right_command.event.event_id = "event-regenerated".into();
        right_command.event.entity_id = "grant-regenerated".into();
        let left = std::thread::spawn(move || left_store.issue_capability_grant(left_command));
        let right = std::thread::spawn(move || right_store.issue_capability_grant(right_command));

        let left = left
            .join()
            .expect("first grant request thread finishes")
            .expect("first grant request commits");
        let right = right
            .join()
            .expect("retry grant request thread finishes")
            .expect("concurrent retry replays");
        let (committed, replay) = if left.replayed {
            (&right, &left)
        } else {
            (&left, &right)
        };
        assert!(!committed.replayed);
        assert!(replay.replayed);
        assert!(
            ["grant-new", "grant-regenerated"]
                .contains(&committed.grant.capability_grant_id.as_str())
        );
        assert_eq!(committed.grant.version, 1);
        assert_eq!(committed.grant.status, CapabilityGrantStatus::Active);
        assert_eq!(replay.grant, committed.grant);
        assert_eq!(
            committed.event.entity_id,
            committed.grant.capability_grant_id
        );
        assert_eq!(committed.event.event_type, "capability.grant.created.v1");
        assert_eq!(replay.event, committed.event);

        let connection = Connection::open(&database_path).expect("inspect durable Trust state");
        for (table, query) in [
            (
                "capability_grants",
                "SELECT COUNT(*) FROM capability_grants",
            ),
            (
                "domain_events",
                "SELECT COUNT(*) FROM domain_events WHERE type='capability.grant.created.v1'",
            ),
            (
                "aggregate_snapshots",
                "SELECT COUNT(*) FROM aggregate_snapshots WHERE entity_type='capability_grant' AND entity_id=?1",
            ),
            (
                "audit_records",
                "SELECT COUNT(*) FROM audit_records WHERE action='CAPABILITY_GRANT_ISSUED'",
            ),
            (
                "request_dedup",
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id='owner-1' AND request_id='issue-grant-1'",
            ),
        ] {
            let count: i64 = if table == "aggregate_snapshots" {
                connection.query_row(
                    query,
                    [committed.grant.capability_grant_id.as_str()],
                    |row| row.get(0),
                )
            } else {
                connection.query_row(query, [], |row| row.get(0))
            }
            .expect("read durable row count");
            assert_eq!(count, 1, "concurrent exact retries create one {table} row");
        }
    }

    #[test]
    fn owner_grant_issue_commits_redacted_event_audit_and_exact_replay() {
        let mut connection = connection();
        let mut command = capability_grant_issue_command();
        command.grant.granted_by = json!({"principal_id":"owner-1","kind":"USER"});
        let request_digest = capability_grant_issue_digest(&command).unwrap();
        let committed = issue_capability_grant_transaction(
            &mut connection,
            command.clone(),
            capability_grant_state(),
            request_digest.clone(),
        )
        .expect("owner creates a scoped read-only grant");
        assert_eq!(committed.grant.capability_grant_id, "grant-new");
        assert_eq!(committed.grant.status, CapabilityGrantStatus::Active);
        let event = serde_json::to_string(&committed.event).unwrap();
        assert!(event.contains("capability.grant.created.v1"));
        assert!(!event.contains("provider_ref"));
        let audit: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM audit_records WHERE action='CAPABILITY_GRANT_ISSUED'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(audit, 1);
        let mut retry = command;
        retry.event.event_id = "regenerated-grant-event".into();
        retry.event.entity_id = "regenerated-grant-id".into();
        retry.grant.capability_grant_id = "regenerated-grant-id".into();
        let replay = issue_capability_grant_transaction(
            &mut connection,
            retry,
            capability_grant_state(),
            request_digest,
        )
        .expect("exact idempotency replay returns the committed grant");
        assert!(replay.replayed);
        assert_eq!(replay.grant.capability_grant_id, "grant-new");
        let mut changed = capability_grant_issue_command();
        changed.grant.expires_at = Some(
            time::OffsetDateTime::parse(
                changed.grant.expires_at.as_deref().unwrap(),
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap()
            .checked_add(time::Duration::seconds(1))
            .unwrap()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap(),
        );
        let changed_digest = capability_grant_issue_digest(&changed).unwrap();
        assert!(matches!(
            issue_capability_grant_transaction(
                &mut connection,
                changed,
                capability_grant_state(),
                changed_digest,
            ),
            Err(StoreError::IdempotencyConflict)
        ));
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM capability_grants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn grant_issue_rolls_back_state_event_snapshot_audit_and_receipt_together() {
        let mut connection = connection();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_grant_audit BEFORE INSERT ON audit_records
                 WHEN NEW.action='CAPABILITY_GRANT_ISSUED'
                 BEGIN SELECT RAISE(ABORT, 'simulated audit writer failure'); END;",
            )
            .unwrap();
        let command = capability_grant_issue_command();
        let request_digest = capability_grant_issue_digest(&command).unwrap();
        assert!(
            issue_capability_grant_transaction(
                &mut connection,
                command,
                capability_grant_state(),
                request_digest,
            )
            .is_err()
        );
        for table in [
            "capability_grants",
            "domain_events",
            "aggregate_snapshots",
            "audit_records",
            "request_dedup",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "{table} must roll back with the failed grant");
        }
    }

    #[test]
    fn grant_issue_denies_foreign_owner_and_approval_without_atomic_consumption() {
        let mut connection = connection();
        let mut foreign_owner = capability_grant_issue_command();
        foreign_owner.actor.principal_id = "attacker".into();
        foreign_owner.actor.owner_principal_id = "attacker".into();
        foreign_owner.grant.granted_by = json!({"principal_id":"attacker","kind":"USER"});
        foreign_owner.request_id = "foreign-owner-grant".into();
        assert!(matches!(
            issue_capability_grant_transaction(
                &mut connection,
                foreign_owner,
                capability_grant_state(),
                "foreign-owner-digest".into()
            ),
            Err(StoreError::NotFound)
        ));

        let mut forged_approval = capability_grant_issue_command();
        forged_approval.request_id = "forged-approval-grant".into();
        forged_approval.approval_id = Some("approval-1".into());
        assert!(matches!(
            issue_capability_grant_transaction(
                &mut connection,
                forged_approval,
                capability_grant_state(),
                "forged-approval-digest".into()
            ),
            Err(StoreError::Invalid(_))
        ));
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM capability_grants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn task_planning_grant_rejects_inactive_or_cross_workspace_task_without_writes() {
        for (task_workspace, status) in [("ws-1", "COMPLETED"), ("ws-other", "RUNNING")] {
            let mut connection = connection();
            connection
                .execute(
                    "UPDATE tasks SET workspace_id=?1,status=?2 WHERE task_id='task-1'",
                    params![task_workspace, status],
                )
                .expect("mutate Task fixture to an unauthorized state");
            let mut command = capability_grant_issue_command();
            command.request_id = format!("grant-denied-{task_workspace}-{status}");
            let request_digest = capability_grant_issue_digest(&command).unwrap();
            assert!(matches!(
                issue_capability_grant_transaction(
                    &mut connection,
                    command,
                    capability_grant_state(),
                    request_digest,
                ),
                Err(StoreError::Invalid(_))
            ));
            for table in [
                "capability_grants",
                "domain_events",
                "aggregate_snapshots",
                "audit_records",
                "request_dedup",
            ] {
                let count: i64 = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .expect("read rejected grant side effects");
                assert_eq!(count, 0, "rejected grant must not write {table}");
            }
        }
    }

    #[test]
    fn task_planning_grant_hides_missing_foreign_and_mismatched_resource_refs() {
        for (resource_id, revision_id) in [
            ("resource-missing", "revision-missing"),
            ("resource-other", "revision-other"),
            ("resource-1", "revision-other"),
        ] {
            let mut connection = connection();
            let mut command = capability_grant_issue_command();
            command.request_id = format!("resource-scope-{resource_id}-{revision_id}");
            command.grant.resource_scope["resource_refs"] = json!([{
                "workspace_id":"ws-1",
                "resource_id":resource_id,
                "revision_id":revision_id,
            }]);
            let request_digest = capability_grant_issue_digest(&command).unwrap();

            assert!(matches!(
                issue_capability_grant_transaction(
                    &mut connection,
                    command,
                    capability_grant_state(),
                    request_digest,
                ),
                Err(StoreError::NotFound)
            ));
            for table in [
                "capability_grants",
                "domain_events",
                "aggregate_snapshots",
                "audit_records",
                "request_dedup",
            ] {
                let count: i64 = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .unwrap();
                assert_eq!(count, 0, "invalid ResourceRef must not write {table}");
            }
        }
    }

    #[test]
    fn task_planning_grant_rejects_revoked_context_document_without_writes() {
        let mut connection = connection();
        connection
            .execute(
                "UPDATE resources SET context_document_json=?1 WHERE resource_id='resource-1'",
                [json!({
                    "kind":"WORKSPACE_NOTES",
                    "status":"REVOKED",
                    "owner_ref":{"kind":"WORKSPACE","workspace_id":"ws-1"}
                })
                .to_string()],
            )
            .expect("mark scoped ContextDocument revoked");
        let command = capability_grant_issue_command();
        let request_digest = capability_grant_issue_digest(&command).unwrap();

        assert!(matches!(
            issue_capability_grant_transaction(
                &mut connection,
                command,
                capability_grant_state(),
                request_digest,
            ),
            Err(StoreError::NotFound)
        ));
        for table in [
            "capability_grants",
            "domain_events",
            "aggregate_snapshots",
            "audit_records",
            "request_dedup",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "revoked ContextDocument must not write {table}");
        }
    }

    #[test]
    fn standalone_attempt_grant_and_unqualified_operation_are_denied() {
        let mut weak_assurance = capability_grant_issue_command();
        weak_assurance.actor.assurance_level = "VIEW_ONLY".into();
        assert!(matches!(
            validate_capability_grant_issue(&weak_assurance),
            Err(StoreError::Invalid(_))
        ));

        let mut unbound_conversation = capability_grant_issue_command();
        unbound_conversation.grant.scope = CapabilityGrantScope::Conversation {
            conversation_id: "conversation-1".into(),
        };
        assert!(matches!(
            validate_capability_grant_issue(&unbound_conversation),
            Err(StoreError::Invalid(_))
        ));

        let mut malformed_capability = capability_grant_issue_command();
        malformed_capability.grant.capability_ref["display_name"] = json!("Mail");
        assert!(matches!(
            validate_capability_grant_issue(&malformed_capability),
            Err(StoreError::Invalid(_))
        ));

        let mut foreign_resource = capability_grant_issue_command();
        foreign_resource.grant.resource_scope["resource_refs"] = json!([{
            "workspace_id":"workspace-other",
            "resource_id":"resource-1",
            "revision_id":"revision-1"
        }]);
        assert!(matches!(
            validate_capability_grant_issue(&foreign_resource),
            Err(StoreError::Invalid(_))
        ));

        let mut attempt = capability_grant_issue_command();
        attempt.grant.scope = CapabilityGrantScope::AttemptExecution {
            task_id: "task-1".into(),
            attempt_id: "attempt-1".into(),
        };
        attempt.request_id = "standalone-attempt-grant".into();
        assert!(matches!(
            validate_capability_grant_issue(&attempt),
            Err(StoreError::Invalid(_))
        ));

        let mut mutation = capability_grant_issue_command();
        mutation.grant.allowed_operations = vec!["external.message.send".into()];
        mutation.grant.resource_scope["operation_ids"] = json!(["external.message.send"]);
        mutation.request_id = "unqualified-mutation-grant".into();
        assert!(matches!(
            validate_capability_grant_issue(&mutation),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn task_planning_grant_rejects_mutation_and_unbounded_expiry() {
        let mut connection = connection();
        let mut mutation = capability_grant_issue_command();
        mutation.grant.allowed_operations = vec!["mail.send".into()];
        mutation.grant.resource_scope["operation_ids"] = json!(["mail.send"]);
        mutation.request_id = "planning-mutation-grant".into();
        assert!(matches!(
            issue_capability_grant_transaction(
                &mut connection,
                mutation,
                capability_grant_state(),
                "mutation-digest".into(),
            ),
            Err(StoreError::Invalid(_))
        ));

        let mut no_expiry = capability_grant_issue_command();
        no_expiry.grant.expires_at = None;
        no_expiry.request_id = "planning-no-expiry-grant".into();
        assert!(matches!(
            issue_capability_grant_transaction(
                &mut connection,
                no_expiry,
                capability_grant_state(),
                "no-expiry-digest".into()
            ),
            Err(StoreError::Invalid(_))
        ));
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM capability_grants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn owner_revocation_fences_an_active_attempt_grant_and_replays_exact_receipt() {
        let mut connection = connection();
        connection.execute(
            "INSERT INTO capability_grants(capability_grant_id,scope_kind,status,task_id,attempt_id,capability_ref_json,allowed_operations_json,resource_scope_json,secret_refs_json,granted_by_json,expires_at,created_at,version) VALUES ('grant-1','ATTEMPT_EXECUTION','ACTIVE','task-1','attempt-1',?1,'[\"mail.send\"]','{\"resource_refs\":[{\"workspace_id\":\"ws-1\",\"resource_id\":\"resource-1\",\"revision_id\":\"revision-1\"}],\"operation_ids\":[\"mail.send\"],\"constraints\":{}}','[]','{\"kind\":\"USER\",\"principal_id\":\"owner-1\"}',NULL,'2026-10-17T10:00:00Z',1)",
            [serde_json::to_string(&target_ref()["capability_ref"]).unwrap()],
        ).unwrap();

        let commit = CapabilityGrantRevocationCommit {
            workspace_id: "ws-1".into(),
            actor: ApprovalActor {
                principal_id: "owner-1".into(),
                principal_kind: "USER".into(),
                owner_principal_id: "owner-1".into(),
                assurance_level: "LOCAL_STRONG".into(),
            },
            request_id: "revoke-grant-1".into(),
            capability_grant_id: "grant-1".into(),
            expected_version: 1,
            event: EventDraft {
                event_id: "event-grant-revoked-1".into(),
                workspace_id: "ws-1".into(),
                entity_type: "capability_grant".into(),
                entity_id: "grant-1".into(),
                origin_runtime_id: "runtime-1".into(),
                entity_revision: 2,
                hlc_timestamp: "2026-10-17T10:01:00.000Z~00000000~runtime-1".into(),
                correlation_id: "corr-revoke-grant-1".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "capability.grant.revoked.v1".into(),
                payload: Value::Null,
                recorded_at: "2026-10-17T10:01:00Z".into(),
            },
        };
        let state = AggregateStateRef {
            blob: storage_core::BlobRef {
                digest: DIGEST.into(),
                size_bytes: 1,
                media_type: "application/vnd.litecow.capability-grant+json".into(),
            },
            entity_revision: 2,
            record_schema_version: 1,
        };
        let current = {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            let current = load_capability_grant(&tx, "ws-1", "grant-1")
                .unwrap()
                .expect("fixture grant exists");
            tx.commit().unwrap();
            current
        };
        let request_digest = grant_revocation_request_digest(&commit).unwrap();
        let mut unknown_assurance = commit.clone();
        unknown_assurance.actor.assurance_level = "OWNER_ADMIN".into();
        assert!(matches!(
            validate_grant_revocation_command(&unknown_assurance),
            Err(StoreError::Invalid(_))
        ));

        let mut non_owner = commit.clone();
        non_owner.actor.principal_id = "intruder-1".into();
        non_owner.actor.owner_principal_id = "intruder-1".into();
        non_owner.request_id = "intruder-revoke-grant-1".into();
        assert!(matches!(
            revoke_capability_grant_transaction(
                &mut connection,
                non_owner,
                current.clone(),
                state.clone(),
                "not-the-owner-request".into(),
            ),
            Err(StoreError::NotFound)
        ));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );

        let committed = revoke_capability_grant_transaction(
            &mut connection,
            commit.clone(),
            current.clone(),
            state.clone(),
            request_digest.clone(),
        )
        .expect("owner revokes grant");
        assert_eq!(committed.capability_grant_id, "grant-1");
        assert_eq!(committed.status, CapabilityGrantStatus::Revoked);
        assert_eq!(committed.version, 2);
        assert!(!committed.replayed);
        let stored: (String, i64) = connection
            .query_row(
                "SELECT status,version FROM capability_grants WHERE capability_grant_id='grant-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, ("REVOKED".into(), 2));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM audit_records WHERE action='CAPABILITY_GRANT_REVOKED'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );

        let replay = revoke_capability_grant_transaction(
            &mut connection,
            commit,
            current,
            state,
            request_digest,
        )
        .expect("lost response replays");
        assert!(replay.replayed);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM audit_records WHERE action='CAPABILITY_GRANT_REVOKED'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn secret_lease_aggregate_snapshot_redacts_local_provider_locator() {
        let command = secret_lease_issue_command();
        let serialized =
            serde_json::to_string(&secret_lease_aggregate_state(&command.lease)).unwrap();
        assert!(!serialized.contains(&command.lease.secret_ref.provider_ref));
        assert!(serialized.contains(&digest(command.lease.secret_ref.provider_ref.as_bytes())));
        assert!(serialized.contains(&command.lease.secret_ref.secret_ref_id));
    }

    #[test]
    fn secret_lease_issue_revoke_and_replay_are_durable_and_redacted() {
        let mut connection = connection();
        let command = secret_lease_issue_command();
        let secret_refs = encode(&[command.lease.secret_ref.clone()]).unwrap();
        let capability_ref = encode(&command.lease.capability_ref).unwrap();
        connection
            .execute(
                "UPDATE tasks SET status='RUNNING' WHERE task_id='task-1'",
                [],
            )
            .unwrap();
        connection.execute(
            "UPDATE attempts SET status='RUNNING',capability_grant_ids_json='[\"grant-1\"]' WHERE attempt_id='attempt-1'",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO capability_grants(capability_grant_id,scope_kind,status,task_id,attempt_id,expires_at,secret_refs_json,capability_ref_json,allowed_operations_json) VALUES ('grant-1','ATTEMPT_EXECUTION','ACTIVE','task-1','attempt-1','2099-01-01T00:00:00Z',?1,?2,'[\"mail.send\"]')",
            params![secret_refs, capability_ref],
        ).unwrap();
        connection.execute(
            "INSERT INTO execution_leases VALUES ('execution-lease-1','task-1','attempt-1','runtime-1','inc-1','ACTIVE','2099-01-01T00:00:00Z')",
            [],
        ).unwrap();

        let state = |version| AggregateStateRef {
            blob: BlobRef {
                digest: DIGEST.into(),
                size_bytes: 1,
                media_type: "application/vnd.litecow.secret-lease+json".into(),
            },
            entity_revision: version,
            record_schema_version: 1,
        };
        let committed = issue_secret_lease_transaction(&mut connection, command.clone(), state(1))
            .expect("exact active Attempt grant issues the lease");
        let event_json = serde_json::to_string(&committed.event).unwrap();
        assert!(!event_json.contains("os-keyring:v1:opaque"));
        assert!(committed.event.payload.get("scope_digest").is_some());

        let mut retry = command.clone();
        retry.lease.secret_lease_id = "regenerated-lease-id".into();
        retry.event.event_id = "regenerated-event-id".into();
        retry.event.entity_id = "regenerated-lease-id".into();
        let replay = issue_secret_lease_transaction(&mut connection, retry, state(1))
            .expect("same idempotency key returns committed issue");
        assert!(replay.replayed);
        assert_eq!(replay.lease.secret_lease_id, "lease-1");

        let mut widened = command.clone();
        widened.request_id = "issue-lease-widened".into();
        widened.lease.secret_lease_id = "lease-widened".into();
        widened.lease.allowed_usage = vec!["mail.delete_all".into()];
        widened.event.event_id = "event-lease-widened".into();
        widened.event.entity_id = "lease-widened".into();
        assert!(matches!(
            issue_secret_lease_transaction(&mut connection, widened, state(1)),
            Err(StoreError::Invalid(_))
        ));

        let mut overlong = command.clone();
        overlong.request_id = "issue-lease-overlong".into();
        overlong.lease.secret_lease_id = "lease-overlong".into();
        overlong.lease.expires_at = "2100-01-01T00:00:00Z".into();
        overlong.event.event_id = "event-lease-overlong".into();
        overlong.event.entity_id = "lease-overlong".into();
        assert!(matches!(
            issue_secret_lease_transaction(&mut connection, overlong, state(1)),
            Err(StoreError::Invalid(_))
        ));
        let leases: i64 = connection
            .query_row("SELECT COUNT(*) FROM secret_leases", [], |row| row.get(0))
            .unwrap();
        assert_eq!(leases, 1);

        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format revoke time");
        let revoke = RevokeSecretLeaseCommit {
            workspace_id: "ws-1".into(),
            actor: command.actor.clone(),
            request_id: "revoke-lease-1".into(),
            secret_lease_id: "lease-1".into(),
            expected_version: 1,
            event: EventDraft {
                event_id: "event-lease-revoked".into(),
                workspace_id: "ws-1".into(),
                entity_type: "secret_lease".into(),
                entity_id: "lease-1".into(),
                origin_runtime_id: "runtime-1".into(),
                entity_revision: 2,
                hlc_timestamp: format!("{now}#0@runtime-1"),
                correlation_id: "corr-lease-revoked".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "secret.lease.revoked.v1".into(),
                payload: Value::Null,
                recorded_at: now,
            },
        };
        let current = load_secret_lease(&connection.transaction().unwrap(), "ws-1", "lease-1")
            .unwrap()
            .unwrap();
        let mut next = current;
        next.status = "REVOKED".into();
        next.version = 2;
        let mut tampered_next = next.clone();
        tampered_next.allowed_usage = vec!["mail.delete_all".into()];
        let mut tampered_revoke = revoke.clone();
        tampered_revoke.request_id = "revoke-lease-tampered-scope".into();
        tampered_revoke.event.event_id = "event-lease-tampered-scope".into();
        assert!(matches!(
            revoke_secret_lease_transaction(
                &mut connection,
                tampered_revoke,
                tampered_next,
                state(2)
            ),
            Err(StoreError::Invalid(_))
        ));
        let revoked =
            revoke_secret_lease_transaction(&mut connection, revoke.clone(), next, state(2))
                .expect("owner revokes active lease");
        assert_eq!(revoked.lease.status, "REVOKED");
        let replay = revoke_secret_lease_transaction(
            &mut connection,
            revoke,
            revoked.lease.clone(),
            state(2),
        )
        .expect("same idempotency key returns committed revoke");
        assert!(replay.replayed);
        assert_eq!(replay.lease.version, 2);
    }

    fn command(decision: ApprovalDecision) -> ResolveApprovalCommit {
        ResolveApprovalCommit {
            workspace_id: "ws-1".into(),
            actor: ApprovalActor {
                principal_id: "owner-1".into(),
                principal_kind: "USER".into(),
                owner_principal_id: "owner-1".into(),
                assurance_level: "APPROVE_SENSITIVE".into(),
            },
            request_id: "resolve-approval-1".into(),
            approval_id: "approval-1".into(),
            expected_version: 1,
            decision,
            confirmation_digest: Some(
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            ),
            event: EventDraft {
                event_id: "event-approval-1".into(),
                workspace_id: "ws-1".into(),
                entity_type: "approval".into(),
                entity_id: "approval-1".into(),
                origin_runtime_id: "runtime-1".into(),
                entity_revision: 2,
                hlc_timestamp: "2026-10-17T10:01:00Z#0@runtime-1".into(),
                correlation_id: "corr-1".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "approval.resolved.v1".into(),
                payload: Value::Null,
                recorded_at: "2026-10-17T10:01:00Z".into(),
            },
        }
    }

    fn state_ref(revision: u64) -> AggregateStateRef {
        AggregateStateRef {
            blob: storage_core::BlobRef {
                digest: DIGEST.into(),
                size_bytes: 1,
                media_type: "application/vnd.litecow.approval+json".into(),
            },
            entity_revision: revision,
            record_schema_version: 1,
        }
    }

    fn next_record(status: &str) -> ApprovalRecord {
        ApprovalRecord {
            approval_id: "approval-1".into(),
            task_id: "task-1".into(),
            requested_by_attempt: None,
            kind: "CAPABILITY_ESCALATION".into(),
            action_summary: "Send this exact message".into(),
            target_ref: target_ref(),
            scope_digest: format!("sha256:{}", "a".repeat(64)),
            action_digest: format!("sha256:{}", "b".repeat(64)),
            risk: "SENSITIVE".into(),
            required_assurance: "APPROVE_SENSITIVE".into(),
            status: status.into(),
            requested_at: "2026-10-17T10:00:00Z".into(),
            expires_at: None,
            resolved_by: Some(json!({"principal_id":"owner-1","kind":"USER"})),
            resolved_at: Some("2026-10-17T10:01:00Z".into()),
            version: 2,
        }
    }

    fn request_command() -> RequestApprovalCommit {
        RequestApprovalCommit {
            workspace_id: "ws-1".into(),
            actor: ApprovalActor {
                principal_id: "agent-1".into(),
                principal_kind: "AGENT".into(),
                owner_principal_id: "owner-1".into(),
                assurance_level: "STEER_SAFE".into(),
            },
            request_id: "request-approval-1".into(),
            approval: ApprovalRecord {
                approval_id: "approval-request-1".into(),
                task_id: "task-1".into(),
                requested_by_attempt: Some("attempt-1".into()),
                kind: "EXTERNAL_MUTATION".into(),
                action_summary: "Publish the selected document".into(),
                target_ref: target_ref(),
                scope_digest: format!("sha256:{}", "c".repeat(64)),
                action_digest: String::new(),
                risk: "HIGH_IMPACT".into(),
                required_assurance: "APPROVE_SENSITIVE".into(),
                status: "PENDING".into(),
                requested_at: "2026-10-17T10:00:00Z".into(),
                expires_at: Some("2026-10-17T10:15:00Z".into()),
                resolved_by: None,
                resolved_at: None,
                version: 1,
            },
            event: EventDraft {
                event_id: "event-approval-request-1".into(),
                workspace_id: "ws-1".into(),
                entity_type: "approval".into(),
                entity_id: "approval-request-1".into(),
                origin_runtime_id: "runtime-1".into(),
                entity_revision: 1,
                hlc_timestamp: "2026-10-17T10:00:00Z#0@runtime-1".into(),
                correlation_id: "corr-approval-request-1".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "approval.requested.v1".into(),
                payload: Value::Null,
                recorded_at: "2026-10-17T10:00:00Z".into(),
            },
        }
    }

    #[test]
    fn approval_request_is_task_bound_atomic_and_exactly_replayed() {
        let mut connection = connection();
        let mut command = request_command();
        command.approval.action_digest = approval_action_digest(&command.approval).unwrap();

        let created = request_transaction(&mut connection, command.clone(), state_ref(1)).unwrap();
        assert_eq!(created.approval.status, "PENDING");
        assert_eq!(created.event.event_type, "approval.requested.v1");
        assert_eq!(created.event.payload["to"], "PENDING");
        assert_eq!(
            created.event.payload["action_digest"],
            created.approval.action_digest
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM approvals", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        let mut changed = request_command();
        changed.approval.action_digest = approval_action_digest(&changed.approval).unwrap();
        changed.approval.action_summary = "Publish a different document".into();
        assert!(matches!(
            request_transaction(&mut connection, changed, state_ref(1)),
            Err(StoreError::IdempotencyConflict)
        ));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM aggregate_snapshots", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        let replay = request_transaction(&mut connection, command, state_ref(1)).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.event.event_id, created.event.event_id);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM approvals", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        // A caller may lose the first response and regenerate its event IDs and
        // clock values before retrying. The idempotency key identifies the
        // approval intent; generated receipt metadata must not turn that retry
        // into a different action.
        let mut regenerated = request_command();
        regenerated.approval.action_digest = approval_action_digest(&regenerated.approval).unwrap();
        regenerated.approval.approval_id = "approval-request-regenerated".into();
        regenerated.approval.requested_at = "2026-10-17T10:01:00Z".into();
        regenerated.approval.expires_at = Some("2026-10-17T10:16:00Z".into());
        regenerated.event.event_id = "event-approval-request-regenerated".into();
        regenerated.event.entity_id = "approval-request-regenerated".into();
        regenerated.event.hlc_timestamp = "2026-10-17T10:01:00Z#0@runtime-1".into();
        regenerated.event.correlation_id = "corr-approval-request-regenerated".into();
        regenerated.event.recorded_at = "2026-10-17T10:01:00Z".into();
        let regenerated_replay =
            request_transaction(&mut connection, regenerated, state_ref(1)).unwrap();
        assert!(regenerated_replay.replayed);
        assert_eq!(
            regenerated_replay.approval.approval_id,
            created.approval.approval_id
        );
        assert_eq!(regenerated_replay.event.event_id, created.event.event_id);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM approvals", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn approval_request_rejects_attempt_task_mismatch_and_wrong_action_digest() {
        let mut connection = connection();
        let mut mismatch = request_command();
        mismatch.approval.requested_by_attempt = Some("attempt-other-task".into());
        mismatch.approval.action_digest = approval_action_digest(&mismatch.approval).unwrap();
        assert!(matches!(
            request_transaction(&mut connection, mismatch, state_ref(1)),
            Err(StoreError::Invalid(_))
        ));

        let mut wrong_task = request_command();
        wrong_task.approval.task_id = "task-outside-workspace".into();
        wrong_task.approval.action_digest = approval_action_digest(&wrong_task.approval).unwrap();
        assert!(matches!(
            request_transaction(&mut connection, wrong_task, state_ref(1)),
            Err(StoreError::Invalid(_))
        ));

        let mut wrong_digest = request_command();
        wrong_digest.approval.action_digest = DIGEST.into();
        assert!(matches!(
            request_transaction(&mut connection, wrong_digest, state_ref(1)),
            Err(StoreError::Invalid(_))
        ));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM approvals", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn approval_request_rejects_secret_named_and_oversized_target_refs() {
        let mut connection = connection();
        let mut secret = request_command();
        secret.approval.target_ref = json!({
            "capability_ref": null,
            "target_digest": DIGEST,
            "execution_method": "STRUCTURED_API",
            "effect_id": "effect-1",
            "secret_value": "must-not-be-persisted",
        });
        secret.approval.action_digest = approval_action_digest(&secret.approval).unwrap();
        assert!(matches!(
            request_transaction(&mut connection, secret, state_ref(1)),
            Err(StoreError::Invalid(_))
        ));

        let mut oversized = request_command();
        oversized.approval.target_ref["capability_ref"]["capability_id"] =
            Value::String("x".repeat(20_000));
        oversized.approval.action_digest = approval_action_digest(&oversized.approval).unwrap();
        assert!(matches!(
            request_transaction(&mut connection, oversized, state_ref(1)),
            Err(StoreError::Invalid(_))
        ));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM approvals", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn approval_request_rolls_back_all_rows_when_receipt_commit_fails() {
        let mut connection = connection();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_approval_receipt BEFORE INSERT ON request_dedup
                 WHEN NEW.request_id='request-approval-1'
                 BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;",
            )
            .unwrap();
        let mut command = request_command();
        command.approval.action_digest = approval_action_digest(&command.approval).unwrap();

        assert!(request_transaction(&mut connection, command, state_ref(1)).is_err());
        for table in [
            "approvals",
            "domain_events",
            "aggregate_snapshots",
            "request_dedup",
            "audit_records",
        ] {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            let count: i64 = connection.query_row(&sql, [], |row| row.get(0)).unwrap();
            let expected = if table == "approvals" { 1 } else { 0 };
            assert_eq!(
                count, expected,
                "{table} must roll back with the Approval request"
            );
        }
    }

    #[test]
    fn owner_resolution_commits_decision_event_audit_snapshot_and_receipt_atomically() {
        let mut connection = connection();
        let committed = resolve_transaction(
            &mut connection,
            command(ApprovalDecision::Approve),
            next_record("APPROVED"),
            state_ref(2),
        )
        .unwrap();

        assert_eq!(committed.approval.status, "APPROVED");
        assert_eq!(committed.event.event_type, "approval.resolved.v1");
        assert_eq!(
            committed.event.payload["action_digest"],
            format!("sha256:{}", "b".repeat(64))
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM audit_records", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM aggregate_snapshots", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        let replay = resolve_transaction(
            &mut connection,
            command(ApprovalDecision::Approve),
            next_record("APPROVED"),
            state_ref(2),
        )
        .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.event.event_id, committed.event.event_id);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM audit_records", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn approval_resolution_rejects_cross_principal_replay_and_stale_versions() {
        let mut connection = connection();
        let mut wrong_owner = command(ApprovalDecision::Approve);
        wrong_owner.actor.principal_id = "other-user".into();
        wrong_owner.actor.owner_principal_id = "other-user".into();
        assert!(matches!(
            resolve_transaction(
                &mut connection,
                wrong_owner,
                next_record("APPROVED"),
                state_ref(2)
            ),
            Err(StoreError::NotFound)
        ));

        let mut stale = command(ApprovalDecision::Approve);
        stale.expected_version = 7;
        stale.event.entity_revision = 8;
        assert!(matches!(
            resolve_transaction(
                &mut connection,
                stale,
                next_record("APPROVED"),
                state_ref(2)
            ),
            Err(StoreError::Conflict { .. })
        ));
        assert_eq!(
            connection
                .query_row(
                    "SELECT status FROM approvals WHERE approval_id='approval-1'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "PENDING"
        );
    }

    #[test]
    fn approval_requires_the_assurance_level_pinned_on_the_request() {
        let mut connection = connection();
        let mut insufficient = command(ApprovalDecision::Approve);
        insufficient.actor.assurance_level = "APPROVE_SAFE".into();
        assert!(matches!(
            resolve_transaction(
                &mut connection,
                insufficient,
                next_record("APPROVED"),
                state_ref(2),
            ),
            Err(StoreError::Invalid(_))
        ));
        assert_eq!(
            connection
                .query_row(
                    "SELECT status FROM approvals WHERE approval_id='approval-1'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "PENDING"
        );
    }

    #[test]
    fn approval_idempotency_key_cannot_replay_a_different_decision() {
        let mut connection = connection();
        resolve_transaction(
            &mut connection,
            command(ApprovalDecision::Approve),
            next_record("APPROVED"),
            state_ref(2),
        )
        .unwrap();
        assert!(matches!(
            resolve_transaction(
                &mut connection,
                command(ApprovalDecision::Deny),
                next_record("DENIED"),
                state_ref(2),
            ),
            Err(StoreError::IdempotencyConflict)
        ));
    }
}
