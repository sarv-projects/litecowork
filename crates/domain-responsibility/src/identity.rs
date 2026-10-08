use crate::DomainError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const OCCURRENCE_PREFIX: &[u8] = b"LiteCowork/AutomationOccurrence/v2\0";

pub fn encode_occurrence_fields(fields: &[&str]) -> Result<Vec<u8>, DomainError> {
    let mut encoded = OCCURRENCE_PREFIX.to_vec();
    for field in fields {
        let len = u32::try_from(field.len()).map_err(|_| DomainError::InvalidDefinition)?;
        encoded.extend_from_slice(&len.to_be_bytes());
        encoded.extend_from_slice(field.as_bytes());
    }
    Ok(encoded)
}

/// No revision field: definition edits cannot replay an accepted delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OccurrenceIdentity {
    Schedule { trigger_id: String, scheduled_epoch_ms: i64 },
    OneShot { trigger_id: String, scheduled_epoch_ms: i64 },
    Manual { trigger_id: String, principal_id: String, request_id: String, automation_id: String },
    ConnectorEvent { trigger_id: String, connection_id: String, provider_event_id: String },
    Webhook { trigger_id: String, source_identity: String, delivery_id: String },
}
impl OccurrenceIdentity {
    pub fn key(&self) -> Result<String, DomainError> {
        let fields = match self {
            Self::Schedule { trigger_id, scheduled_epoch_ms } => vec!["SCHEDULE".into(), trigger_id.clone(), scheduled_epoch_ms.to_string()],
            Self::OneShot { trigger_id, scheduled_epoch_ms } => vec!["ONE_SHOT".into(), trigger_id.clone(), scheduled_epoch_ms.to_string()],
            Self::Manual { trigger_id, principal_id, request_id, automation_id } => vec!["MANUAL".into(), trigger_id.clone(), principal_id.clone(), request_id.clone(), automation_id.clone()],
            Self::ConnectorEvent { trigger_id, connection_id, provider_event_id } => vec!["CONNECTOR_EVENT".into(), trigger_id.clone(), connection_id.clone(), provider_event_id.clone()],
            Self::Webhook { trigger_id, source_identity, delivery_id } => vec!["WEBHOOK".into(), trigger_id.clone(), source_identity.clone(), delivery_id.clone()],
        };
        if fields.iter().any(String::is_empty) { return Err(DomainError::InvalidDefinition); }
        let refs: Vec<_> = fields.iter().map(String::as_str).collect();
        Ok(hex::encode(Sha256::digest(encode_occurrence_fields(&refs)?)))
    }
}

/// Internal idempotency identity for a proposed exact-revision test Task.
/// This is neither a public occurrence key nor an AutomationOccurrence.
/// The Task adapter must atomically reserve this identity with ordinary Task
/// admission; it must not write a trigger receipt, cursor, or occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestRunIdentity {
    pub automation_id: String,
    pub automation_revision: u64,
    pub principal_id: String,
    pub request_id: String,
}
impl TestRunIdentity {
    pub fn key(&self) -> Result<String, DomainError> {
        if self.automation_revision == 0 || [&self.automation_id, &self.principal_id, &self.request_id].iter().any(|s| s.is_empty()) {
            return Err(DomainError::InvalidDefinition);
        }
        // A private command fingerprint; there is intentionally no public
        // AutomationOccurrence encoding for tests in the current contracts.
        let bytes = serde_json_canonicalizer::to_vec(self).map_err(|_| DomainError::InvalidDefinition)?;
        let mut hasher = Sha256::new();
        hasher.update(b"LiteCowork/AutomationTestCommand/v1\0");
        hasher.update(bytes);
        Ok(hex::encode(hasher.finalize()))
    }
}

pub(crate) fn definition_digest<T: Serialize>(value: &T) -> Result<String, DomainError> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|_| DomainError::InvalidDefinition)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}
