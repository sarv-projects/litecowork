//! Write-only local AgentModule credential submission boundary.
//!
//! The router is responsible for local Operator authentication and obtaining the
//! current descriptor from the trusted AgentModule. This module validates the
//! write-only request and projects its bounded secret into zeroizing memory. Durable
//! replay and SecretStore writes are coordinated by the route and receipt owner.

use crate::agents::control_descriptor::{
    AgentControlDescriptor, AgentSecretKind, CredentialStorageOwner as DescriptorStorageOwner,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize};
use std::time::SystemTime;
use storage_core::{SecretKind, SecretRef, StoreError};
use zeroize::Zeroizing;

const MAX_ENCODED_SECRET_BYTES: usize = 65_536;
const MAX_DECODED_SECRET_BYTES: usize = 48 * 1024;
const MAX_PROFILE_ID_BYTES: usize = 512;
const MAX_SLOT_ID_BYTES: usize = 128;

/// Request body matching `AgentCredentialSecretSubmission` in the Operator schema.
/// Do not add `Debug`/logging support to this type.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentCredentialSecretSubmission {
    pub(crate) descriptor_digest: String,
    #[serde(deserialize_with = "deserialize_zeroizing_string")]
    pub(crate) secret_base64: Zeroizing<String>,
}

fn deserialize_zeroizing_string<'de, D>(deserializer: D) -> Result<Zeroizing<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Zeroizing::new)
}

/// Trusted, current descriptor projection obtained from the selected AgentModule.
/// It is deliberately supplied by the authenticated route, not deserialized from
/// the request body or persisted as an AgentProfile claim.
pub(crate) struct AgentCredentialDescriptorSnapshot {
    pub(crate) agent_profile_id: String,
    pub(crate) digest: String,
    pub(crate) expires_at: SystemTime,
    pub(crate) credential_slots: Vec<AgentCredentialSlot>,
}

impl AgentCredentialDescriptorSnapshot {
    /// Converts a validated, fresh AgentModule descriptor into the narrow snapshot
    /// consumed by the write-only SecretStore boundary. The caller must obtain the
    /// descriptor from the selected trusted module, never from request data.
    pub(crate) fn from_control_descriptor(
        agent_profile_id: &str,
        descriptor: &AgentControlDescriptor,
        now: SystemTime,
    ) -> Result<Self, CredentialSubmissionError> {
        if agent_profile_id.trim().is_empty() || agent_profile_id.len() > MAX_PROFILE_ID_BYTES {
            return Err(CredentialSubmissionError::InvalidRequest);
        }
        descriptor
            .validate()
            .map_err(|_| CredentialSubmissionError::DescriptorUnavailable)?;
        if !descriptor.is_fresh_at(now) {
            return Err(CredentialSubmissionError::DescriptorUnavailable);
        }
        let expiry = time::OffsetDateTime::parse(
            &descriptor.expires_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|_| CredentialSubmissionError::DescriptorUnavailable)?;
        let expiry_seconds = u64::try_from(expiry.unix_timestamp())
            .map_err(|_| CredentialSubmissionError::DescriptorUnavailable)?;
        let expires_at = SystemTime::UNIX_EPOCH
            .checked_add(std::time::Duration::from_secs(expiry_seconds))
            .and_then(|value| {
                value.checked_add(std::time::Duration::from_nanos(u64::from(
                    expiry.nanosecond(),
                )))
            })
            .ok_or(CredentialSubmissionError::DescriptorUnavailable)?;
        let credential_slots = descriptor
            .auth
            .credential_slots
            .iter()
            .map(|slot| AgentCredentialSlot {
                credential_slot_id: slot.credential_slot_id.clone(),
                secret_kind: match slot.secret_kind {
                    AgentSecretKind::ApiKey => SecretKind::ApiKey,
                    AgentSecretKind::Token => SecretKind::Token,
                    AgentSecretKind::Password => SecretKind::Password,
                    AgentSecretKind::Other => SecretKind::Other,
                },
                storage_owner: match slot.storage_owner {
                    DescriptorStorageOwner::NativeAgent => CredentialStorageOwner::NativeAgent,
                    DescriptorStorageOwner::LiteCoworkSecretStore => {
                        CredentialStorageOwner::LiteCoworkSecretStore
                    }
                },
            })
            .collect();
        Ok(Self {
            agent_profile_id: agent_profile_id.to_owned(),
            digest: descriptor.descriptor_digest.clone(),
            expires_at,
            credential_slots,
        })
    }

    /// Returns the SecretKind for exactly one fresh slot owned by LiteCowork.
    /// Native-agent slots are never converted into a LiteCowork SecretRef.
    pub(crate) fn secret_kind_for_slot(
        &self,
        requested_agent_profile_id: &str,
        requested_descriptor_digest: &str,
        requested_slot_id: &str,
        now: SystemTime,
    ) -> Result<SecretKind, CredentialSubmissionError> {
        if self.agent_profile_id != requested_agent_profile_id
            || self.digest != requested_descriptor_digest
            || self.expires_at <= now
        {
            return Err(CredentialSubmissionError::DescriptorUnavailable);
        }
        let mut matching_slots = self
            .credential_slots
            .iter()
            .filter(|slot| slot.credential_slot_id == requested_slot_id);
        let slot = matching_slots
            .next()
            .ok_or(CredentialSubmissionError::SlotUnavailable)?;
        if matching_slots.next().is_some()
            || slot.credential_slot_id.is_empty()
            || slot.credential_slot_id.len() > MAX_SLOT_ID_BYTES
            || slot.storage_owner != CredentialStorageOwner::LiteCoworkSecretStore
        {
            return Err(CredentialSubmissionError::SlotUnavailable);
        }
        Ok(slot.secret_kind)
    }
}

pub(crate) struct AgentCredentialSlot {
    pub(crate) credential_slot_id: String,
    pub(crate) secret_kind: SecretKind,
    pub(crate) storage_owner: CredentialStorageOwner,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CredentialStorageOwner {
    NativeAgent,
    LiteCoworkSecretStore,
}

/// Safe response projection. This contains only a non-secret slot ID and opaque
/// reference metadata, never the request body or credential bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentCredentialBindingResponse {
    pub(crate) credential_slot_id: String,
    pub(crate) secret_ref: SecretRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CredentialSubmissionError {
    InvalidRequest,
    DescriptorUnavailable,
    SlotUnavailable,
    SecretStoreUnavailable,
}

impl From<StoreError> for CredentialSubmissionError {
    fn from(_: StoreError) -> Self {
        Self::SecretStoreUnavailable
    }
}

/// Validates the bounded write-only request without consulting a descriptor or
/// touching SecretStore. The route uses this before reporting an unavailable
/// module, so malformed payloads do not get masked by a dependency error.
/// Validates and decodes once into zeroizing memory. Callers must not copy the
/// returned bytes into ordinary buffers; the route fingerprints them in place,
/// then lends this same allocation to the idempotent SecretStore operation.
pub(crate) fn decode_agent_credential_request(
    workspace_id: &str,
    requested_agent_profile_id: &str,
    requested_slot_id: &str,
    request: &AgentCredentialSecretSubmission,
) -> Result<Zeroizing<Vec<u8>>, CredentialSubmissionError> {
    let encoded_secret = &request.secret_base64;
    if request.descriptor_digest.len() != 71
        || !request.descriptor_digest.starts_with("sha256:")
        || !request.descriptor_digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || encoded_secret.is_empty()
        || encoded_secret.len() > MAX_ENCODED_SECRET_BYTES
        || requested_agent_profile_id.trim().is_empty()
        || requested_agent_profile_id.len() > MAX_PROFILE_ID_BYTES
        || requested_slot_id.trim().is_empty()
        || requested_slot_id.len() > MAX_SLOT_ID_BYTES
        || workspace_id.trim().is_empty()
    {
        return Err(CredentialSubmissionError::InvalidRequest);
    }

    let mut decoded = Zeroizing::new(Vec::with_capacity(encoded_secret.len() * 3 / 4));
    STANDARD
        .decode_vec(encoded_secret.as_bytes(), &mut decoded)
        .map_err(|_| CredentialSubmissionError::InvalidRequest)?;
    if decoded.is_empty() || decoded.len() > MAX_DECODED_SECRET_BYTES {
        return Err(CredentialSubmissionError::InvalidRequest);
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    const WORKSPACE: &str = "workspace-test";

    fn request(secret: &[u8]) -> AgentCredentialSecretSubmission {
        AgentCredentialSecretSubmission {
            descriptor_digest:
                "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
            secret_base64: Zeroizing::new(STANDARD.encode(secret)),
        }
    }

    #[test]
    fn decoded_secret_is_zeroizing_and_matches_the_write_only_payload() {
        let secret = decode_agent_credential_request(
            WORKSPACE,
            "agent-profile-test",
            "api-key",
            &request(b"private-api-key"),
        )
        .expect("valid write-only credential decodes");
        assert_eq!(secret.as_slice(), b"private-api-key");
    }

    #[test]
    fn malformed_or_unbounded_secret_request_is_rejected() {
        let mut malformed = request(b"private-api-key");
        malformed.secret_base64 = Zeroizing::new("not base64".to_owned());
        assert_eq!(
            decode_agent_credential_request(
                WORKSPACE,
                "agent-profile-test",
                "api-key",
                &malformed,
            )
            .unwrap_err(),
            CredentialSubmissionError::InvalidRequest
        );
        assert_eq!(
            decode_agent_credential_request(
                WORKSPACE,
                "agent-profile-test",
                "api-key",
                &request(&vec![b'x'; 48 * 1024 + 1]),
            )
            .unwrap_err(),
            CredentialSubmissionError::InvalidRequest
        );
    }
}
