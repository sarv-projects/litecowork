//! Write-only local AgentModule credential submission boundary.
//!
//! The router is responsible for local Operator authentication and obtaining the
//! current descriptor from the trusted AgentModule. This module validates the
//! descriptor binding and slot, then sends secret bytes directly to SecretStore.

use crate::agents::control_descriptor::{
    AgentControlDescriptor, AgentSecretKind, CredentialStorageOwner as DescriptorStorageOwner,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize};
use std::time::SystemTime;
use storage_core::{SecretKind, SecretPlacement, SecretRef, SecretStore, StoreError};
use zeroize::{Zeroize, Zeroizing};

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
pub(crate) fn validate_agent_credential_request(
    workspace_id: &str,
    requested_agent_profile_id: &str,
    requested_slot_id: &str,
    request: &AgentCredentialSecretSubmission,
) -> Result<(), CredentialSubmissionError> {
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
        .decode_vec(encoded_secret.as_bytes(), &mut *decoded)
        .map_err(|_| CredentialSubmissionError::InvalidRequest)?;
    if decoded.is_empty() || decoded.len() > MAX_DECODED_SECRET_BYTES {
        return Err(CredentialSubmissionError::InvalidRequest);
    }
    Ok(())
}

pub(crate) fn submit_agent_credential(
    workspace_id: &str,
    requested_agent_profile_id: &str,
    requested_slot_id: &str,
    request: AgentCredentialSecretSubmission,
    descriptor: Option<&AgentCredentialDescriptorSnapshot>,
    secret_store: &dyn SecretStore,
    now: SystemTime,
) -> Result<AgentCredentialBindingResponse, CredentialSubmissionError> {
    // Own the incoming JSON string in zeroizing memory immediately. No error path
    // formats or returns it, and the response type has no field for secret input.
    validate_agent_credential_request(
        workspace_id,
        requested_agent_profile_id,
        requested_slot_id,
        &request,
    )?;
    let encoded_secret = request.secret_base64;
    let request_digest = request.descriptor_digest;

    let descriptor = descriptor.ok_or(CredentialSubmissionError::DescriptorUnavailable)?;
    if descriptor.agent_profile_id != requested_agent_profile_id
        || descriptor.digest != request_digest
        || descriptor.expires_at <= now
    {
        return Err(CredentialSubmissionError::DescriptorUnavailable);
    }

    let mut matching_slots = descriptor
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

    // Decode only after request/descriptor/slot checks. The decoded allocation is
    // zeroizing from its first owned value and is handed directly to SecretStore.
    let mut decoded = Zeroizing::new(Vec::with_capacity(encoded_secret.len() * 3 / 4));
    STANDARD
        .decode_vec(encoded_secret.as_bytes(), &mut *decoded)
        .map_err(|_| CredentialSubmissionError::InvalidRequest)?;
    let mut secret_bytes = decoded;
    let mut secret_ref = secret_store
        .write(workspace_id, slot.secret_kind, secret_bytes.as_mut_slice())
        .map_err(CredentialSubmissionError::from)?;
    // SecretStore returns opaque, non-secret metadata. Still reject malformed
    // implementations so invalid data cannot leak into a public Operator response.
    if secret_ref.secret_ref_id.is_empty()
        || secret_ref.secret_ref_id.len() > 512
        || secret_ref.provider_ref.is_empty()
        || secret_ref.provider_ref.len() > 512
        || secret_ref.placement != SecretPlacement::LocalOnly
    {
        secret_ref.secret_ref_id.zeroize();
        secret_ref.provider_ref.zeroize();
        return Err(CredentialSubmissionError::SecretStoreUnavailable);
    }

    Ok(AgentCredentialBindingResponse {
        credential_slot_id: requested_slot_id.to_owned(),
        secret_ref,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingSecretStore {
        writes: Arc<Mutex<Vec<(String, SecretKind, Vec<u8>)>>>,
    }

    impl SecretStore for RecordingSecretStore {
        fn write(
            &self,
            workspace_id: &str,
            secret_kind: SecretKind,
            secret_bytes: &mut [u8],
        ) -> Result<SecretRef, StoreError> {
            self.writes.lock().expect("recording store lock").push((
                workspace_id.to_owned(),
                secret_kind,
                secret_bytes.to_vec(),
            ));
            secret_bytes.fill(0);
            Ok(SecretRef {
                secret_ref_id: "secret_ref_test_1".to_owned(),
                provider_ref: "os-keyring:v1:test".to_owned(),
                placement: SecretPlacement::LocalOnly,
            })
        }
    }

    struct FailingSecretStore;

    impl SecretStore for FailingSecretStore {
        fn write(
            &self,
            _workspace_id: &str,
            _secret_kind: SecretKind,
            secret_bytes: &mut [u8],
        ) -> Result<SecretRef, StoreError> {
            secret_bytes.fill(0);
            Err(StoreError::Busy)
        }
    }

    const PROFILE: &str = "agent-profile-test";
    const SLOT: &str = "api-key";
    const WORKSPACE: &str = "workspace-test";
    const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn descriptor(owner: CredentialStorageOwner) -> AgentCredentialDescriptorSnapshot {
        AgentCredentialDescriptorSnapshot {
            agent_profile_id: PROFILE.to_owned(),
            digest: DIGEST.to_owned(),
            expires_at: SystemTime::now() + std::time::Duration::from_secs(60),
            credential_slots: vec![AgentCredentialSlot {
                credential_slot_id: SLOT.to_owned(),
                secret_kind: SecretKind::ApiKey,
                storage_owner: owner,
            }],
        }
    }

    fn request(secret: &[u8]) -> AgentCredentialSecretSubmission {
        AgentCredentialSecretSubmission {
            descriptor_digest: DIGEST.to_owned(),
            secret_base64: Zeroizing::new(STANDARD.encode(secret)),
        }
    }

    #[test]
    fn valid_secret_is_written_and_response_contains_only_slot_and_reference() {
        let store = RecordingSecretStore::default();
        let result = submit_agent_credential(
            WORKSPACE,
            PROFILE,
            SLOT,
            request(b"private-api-key"),
            Some(&descriptor(CredentialStorageOwner::LiteCoworkSecretStore)),
            &store,
            SystemTime::now(),
        )
        .expect("declared secure credential slot accepts a local write");

        assert_eq!(
            store
                .writes
                .lock()
                .expect("recording store lock")
                .as_slice(),
            &[(
                WORKSPACE.to_owned(),
                SecretKind::ApiKey,
                b"private-api-key".to_vec()
            )]
        );
        let response = serde_json::to_string(&result).expect("safe response serializes");
        assert!(response.contains(SLOT));
        assert!(response.contains("secret_ref_test_1"));
        assert!(!response.contains("private-api-key"));
    }

    #[test]
    fn stale_or_mismatched_descriptor_and_native_owned_slots_never_write() {
        let store = RecordingSecretStore::default();
        let mut stale = descriptor(CredentialStorageOwner::LiteCoworkSecretStore);
        stale.expires_at = SystemTime::now() - std::time::Duration::from_secs(1);
        assert_eq!(
            submit_agent_credential(
                WORKSPACE,
                PROFILE,
                SLOT,
                request(b"private-api-key"),
                Some(&stale),
                &store,
                SystemTime::now(),
            )
            .unwrap_err(),
            CredentialSubmissionError::DescriptorUnavailable
        );

        let mut mismatched = descriptor(CredentialStorageOwner::LiteCoworkSecretStore);
        mismatched.digest = format!("{}0", mismatched.digest);
        assert_eq!(
            submit_agent_credential(
                WORKSPACE,
                PROFILE,
                SLOT,
                request(b"private-api-key"),
                Some(&mismatched),
                &store,
                SystemTime::now(),
            )
            .unwrap_err(),
            CredentialSubmissionError::DescriptorUnavailable
        );

        assert_eq!(
            submit_agent_credential(
                WORKSPACE,
                PROFILE,
                SLOT,
                request(b"private-api-key"),
                Some(&descriptor(CredentialStorageOwner::NativeAgent)),
                &store,
                SystemTime::now(),
            )
            .unwrap_err(),
            CredentialSubmissionError::SlotUnavailable
        );
        assert!(
            store
                .writes
                .lock()
                .expect("recording store lock")
                .is_empty()
        );
    }

    #[test]
    fn unknown_duplicate_slots_and_invalid_base64_fail_closed() {
        let store = RecordingSecretStore::default();
        assert_eq!(
            submit_agent_credential(
                WORKSPACE,
                PROFILE,
                "missing-slot",
                request(b"private-api-key"),
                Some(&descriptor(CredentialStorageOwner::LiteCoworkSecretStore)),
                &store,
                SystemTime::now(),
            )
            .unwrap_err(),
            CredentialSubmissionError::SlotUnavailable
        );

        let mut duplicated = descriptor(CredentialStorageOwner::LiteCoworkSecretStore);
        duplicated.credential_slots.push(AgentCredentialSlot {
            credential_slot_id: SLOT.to_owned(),
            secret_kind: SecretKind::Token,
            storage_owner: CredentialStorageOwner::LiteCoworkSecretStore,
        });
        assert_eq!(
            submit_agent_credential(
                WORKSPACE,
                PROFILE,
                SLOT,
                request(b"private-api-key"),
                Some(&duplicated),
                &store,
                SystemTime::now(),
            )
            .unwrap_err(),
            CredentialSubmissionError::SlotUnavailable
        );

        let mut malformed = request(b"private-api-key");
        malformed.secret_base64 = Zeroizing::new("not base64".to_owned());
        assert_eq!(
            submit_agent_credential(
                WORKSPACE,
                PROFILE,
                SLOT,
                malformed,
                Some(&descriptor(CredentialStorageOwner::LiteCoworkSecretStore)),
                &store,
                SystemTime::now(),
            )
            .unwrap_err(),
            CredentialSubmissionError::InvalidRequest
        );
        assert!(
            store
                .writes
                .lock()
                .expect("recording store lock")
                .is_empty()
        );
    }

    #[test]
    fn secret_store_failures_are_mapped_without_exposing_provider_errors() {
        let result = submit_agent_credential(
            WORKSPACE,
            PROFILE,
            SLOT,
            request(b"private-api-key"),
            Some(&descriptor(CredentialStorageOwner::LiteCoworkSecretStore)),
            &FailingSecretStore,
            SystemTime::now(),
        );
        assert_eq!(
            result.unwrap_err(),
            CredentialSubmissionError::SecretStoreUnavailable
        );
    }
}
