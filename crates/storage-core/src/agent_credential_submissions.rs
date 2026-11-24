//! Runtime-local receipts that reconcile credential writes across SQLite and an OS SecretStore.
//!
//! The receipt stores no credential bytes and no unkeyed credential verifier. Its
//! fingerprint is keyed outside SQLite by the Runtime installation identity.

use crate::{SecretKind, StoreError};
use serde::{Deserialize, Serialize};

pub const AGENT_CREDENTIAL_RECEIPT_ID_MAX_BYTES: usize = 512;
pub const AGENT_CREDENTIAL_RECEIPT_SLOT_MAX_BYTES: usize = 128;
pub const AGENT_CREDENTIAL_IDEMPOTENCY_DIGEST_PREFIX: &str = "sha256:";
pub const AGENT_CREDENTIAL_REQUEST_FINGERPRINT_PREFIX: &str = "hmac-sha256:";
pub const AGENT_CREDENTIAL_SECRET_REF_PREFIX: &str = "secret_";

/// Owner-scoped identity for one credential submission. The raw Idempotency-Key is
/// intentionally not retained; callers provide its SHA-256 digest.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCredentialSubmissionKey {
    pub workspace_id: String,
    pub principal_id: String,
    pub agent_profile_id: String,
    pub credential_slot_id: String,
    pub idempotency_key_digest: String,
}

/// Durable reservation request. A unique key may already exist if two identical
/// sends race; the store returns that prior receipt after verifying its fingerprint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCredentialSubmissionReservation {
    pub key: AgentCredentialSubmissionKey,
    pub request_fingerprint: String,
    pub secret_kind: SecretKind,
    pub secret_ref_id: String,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentCredentialSubmissionState {
    Pending,
    Committed,
}

impl AgentCredentialSubmissionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Committed => "COMMITTED",
        }
    }

    pub fn from_str(value: &str) -> Result<Self, StoreError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "COMMITTED" => Ok(Self::Committed),
            _ => Err(StoreError::CorruptSchema(
                "agent credential submission receipt has an unknown state".to_owned(),
            )),
        }
    }
}

/// Runtime-local durable state for one exact secret submission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCredentialSubmissionReceipt {
    pub key: AgentCredentialSubmissionKey,
    pub request_fingerprint: String,
    pub secret_kind: SecretKind,
    pub secret_ref_id: String,
    pub state: AgentCredentialSubmissionState,
    pub provider_ref: Option<String>,
    pub created_at: String,
    pub committed_at: Option<String>,
}

/// Result of an atomic reservation. `created` is decided under the same IMMEDIATE
/// transaction as the unique-key check, so concurrent exact requests get stable
/// 201 (creator) versus 200 (replay) semantics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCredentialSubmissionReservationOutcome {
    pub receipt: AgentCredentialSubmissionReceipt,
    pub created: bool,
}

/// Runtime-local transaction owner for credential submission replay and settlement.
pub trait AgentCredentialSubmissionStore: Send + Sync {
    /// Returns a receipt only when the selected Workspace still belongs to the
    /// authenticated principal. Archived Workspace ownership is sufficient to finish
    /// an already-reserved operation; new reservations require ACTIVE state.
    fn get_agent_credential_submission(
        &self,
        key: AgentCredentialSubmissionKey,
    ) -> Result<Option<AgentCredentialSubmissionReceipt>, StoreError>;

    /// Creates a PENDING receipt after requiring an active Workspace owner. Exact
    /// concurrent reservations return the existing row; a changed fingerprint is a
    /// StoreError::IdempotencyConflict. External SecretStore work must follow commit.
    fn reserve_agent_credential_submission(
        &self,
        reservation: AgentCredentialSubmissionReservation,
    ) -> Result<AgentCredentialSubmissionReservationOutcome, StoreError>;

    /// Settles the exact reserved ref and writes the metadata-only audit row in the
    /// same transaction. This reconciles an already-authorized operation and is safe
    /// to retry after a lost commit response.
    fn commit_agent_credential_submission(
        &self,
        key: AgentCredentialSubmissionKey,
        request_fingerprint: String,
        provider_ref: String,
        committed_at: String,
    ) -> Result<AgentCredentialSubmissionReceipt, StoreError>;
}

pub fn validate_agent_credential_submission_key(
    key: &AgentCredentialSubmissionKey,
) -> Result<(), StoreError> {
    let valid_id = |value: &str, max_bytes: usize| {
        !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
    };
    if !valid_id(&key.workspace_id, AGENT_CREDENTIAL_RECEIPT_ID_MAX_BYTES)
        || !valid_id(&key.principal_id, AGENT_CREDENTIAL_RECEIPT_ID_MAX_BYTES)
        || !valid_id(&key.agent_profile_id, AGENT_CREDENTIAL_RECEIPT_ID_MAX_BYTES)
        || !valid_id(
            &key.credential_slot_id,
            AGENT_CREDENTIAL_RECEIPT_SLOT_MAX_BYTES,
        )
        || !valid_hex_digest(
            &key.idempotency_key_digest,
            AGENT_CREDENTIAL_IDEMPOTENCY_DIGEST_PREFIX,
        )
    {
        return Err(StoreError::Invalid(
            "agent credential submission identity is invalid".to_owned(),
        ));
    }
    Ok(())
}

pub fn validate_agent_credential_submission_reservation(
    reservation: &AgentCredentialSubmissionReservation,
) -> Result<(), StoreError> {
    validate_agent_credential_submission_key(&reservation.key)?;
    if !valid_hex_digest(
        &reservation.request_fingerprint,
        AGENT_CREDENTIAL_REQUEST_FINGERPRINT_PREFIX,
    ) || !valid_secret_ref_id(&reservation.secret_ref_id)
        || reservation.created_at.trim().is_empty()
        || reservation.created_at.len() > 64
        || reservation.created_at.chars().any(char::is_control)
    {
        return Err(StoreError::Invalid(
            "agent credential submission reservation is invalid".to_owned(),
        ));
    }
    Ok(())
}

pub fn validate_agent_credential_submission_settlement(
    key: &AgentCredentialSubmissionKey,
    request_fingerprint: &str,
    provider_ref: &str,
    committed_at: &str,
) -> Result<(), StoreError> {
    validate_agent_credential_submission_key(key)?;
    if !valid_hex_digest(
        request_fingerprint,
        AGENT_CREDENTIAL_REQUEST_FINGERPRINT_PREFIX,
    ) || provider_ref.trim().is_empty()
        || provider_ref.len() > 512
        || provider_ref.chars().any(char::is_control)
        || committed_at.trim().is_empty()
        || committed_at.len() > 64
        || committed_at.chars().any(char::is_control)
    {
        return Err(StoreError::Invalid(
            "agent credential submission settlement is invalid".to_owned(),
        ));
    }
    Ok(())
}

pub fn validate_secret_ref_id(value: &str) -> Result<(), StoreError> {
    if !valid_secret_ref_id(value) {
        return Err(StoreError::Invalid(
            "agent credential SecretRef ID is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn valid_secret_ref_id(value: &str) -> bool {
    value.len() == AGENT_CREDENTIAL_SECRET_REF_PREFIX.len() + 64
        && value.starts_with(AGENT_CREDENTIAL_SECRET_REF_PREFIX)
        && value[AGENT_CREDENTIAL_SECRET_REF_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_hex_digest(value: &str, prefix: &str) -> bool {
    value.len() == prefix.len() + 64
        && value.starts_with(prefix)
        && value[prefix.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> AgentCredentialSubmissionKey {
        AgentCredentialSubmissionKey {
            workspace_id: "workspace".to_owned(),
            principal_id: "owner".to_owned(),
            agent_profile_id: "profile".to_owned(),
            credential_slot_id: "api-key".to_owned(),
            idempotency_key_digest: format!(
                "{AGENT_CREDENTIAL_IDEMPOTENCY_DIGEST_PREFIX}{}",
                "a".repeat(64)
            ),
        }
    }

    #[test]
    fn validates_receipt_identity_and_stable_secret_ref_shape() {
        let valid = AgentCredentialSubmissionReservation {
            key: key(),
            request_fingerprint: format!(
                "{AGENT_CREDENTIAL_REQUEST_FINGERPRINT_PREFIX}{}",
                "b".repeat(64)
            ),
            secret_kind: SecretKind::ApiKey,
            secret_ref_id: format!("{AGENT_CREDENTIAL_SECRET_REF_PREFIX}{}", "c".repeat(64)),
            created_at: "2026-11-23T12:00:00Z".to_owned(),
        };
        assert_eq!(
            validate_agent_credential_submission_reservation(&valid),
            Ok(())
        );
    }

    #[test]
    fn rejects_unkeyed_or_malformed_secret_fingerprints() {
        let mut reservation = AgentCredentialSubmissionReservation {
            key: key(),
            request_fingerprint: format!(
                "{AGENT_CREDENTIAL_REQUEST_FINGERPRINT_PREFIX}{}",
                "A".repeat(64)
            ),
            secret_kind: SecretKind::ApiKey,
            secret_ref_id: format!("{AGENT_CREDENTIAL_SECRET_REF_PREFIX}{}", "c".repeat(64)),
            created_at: "2026-11-23T12:00:00Z".to_owned(),
        };
        assert!(validate_agent_credential_submission_reservation(&reservation).is_err());
        reservation.request_fingerprint = format!("sha256:{}", "b".repeat(64));
        assert!(validate_agent_credential_submission_reservation(&reservation).is_err());
    }
}
